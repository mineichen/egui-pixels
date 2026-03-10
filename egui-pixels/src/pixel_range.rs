use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};
use std::ops::RangeInclusive;

use imagemask::{MetaRange, NonZeroRange, SortedRangesMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub struct Meta {
    confidence: u8,
}

impl Meta {
    pub const fn new(confidence: u8) -> Self {
        Self { confidence }
    }

    pub const fn confidence(self) -> u8 {
        self.confidence
    }
}

impl Default for Meta {
    fn default() -> Self {
        Self { confidence: 255 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PixelRange(imagemask::MetaRange<NonZeroRange<u64>, Meta>);

impl From<MetaRange<NonZeroRange<u64>, Meta>> for PixelRange {
    fn from(value: MetaRange<NonZeroRange<u64>, Meta>) -> Self {
        Self(value)
    }
}

impl PixelRange {
    pub fn new(start: u32, length: NonZeroU16, meta: Meta) -> Self {
        let len = NonZeroU64::from(length);
        Self(imagemask::MetaRange {
            range: NonZeroRange::from_span(start as u64, len),
            meta,
        })
    }

    pub fn new_total(start: u32, length: NonZeroU16) -> Self {
        Self::new(start, length, Meta::default())
    }

    pub fn meta(&self) -> Meta {
        self.0.meta
    }

    pub fn start(&self) -> u32 {
        self.0.range.start as _
    }

    pub fn length(&self) -> NonZeroU16 {
        self.0
            .range
            .len_non_zero()
            .try_into()
            .expect("Cannot create a Range<u16> from Range<u64>")
    }

    pub fn increment_length(&mut self) {
        self.0.range.increment_length();
        debug_assert!(
            u16::try_from(self.0.range.end - self.0.range.start).is_ok(),
            "length is never > u16::MAX"
        );
    }

    pub fn end(&self) -> u32 {
        self.0.range.end.try_into().expect("Never bigger than u32")
    }
}

type PixelRanges = SortedRangesMap<u32, u32, Vec<Meta>>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PixelArea {
    pub pixels: PixelRanges,
    pub color: [u8; 3],
}

impl PixelArea {
    pub fn new(pixels: impl IntoIterator<Item = PixelRange>, color: [u8; 3]) -> Option<Self> {
        Some(Self {
            pixels: Self::try_from_iter(pixels)?,
            color,
        })
    }

    pub fn with_black_color(pixels: impl IntoIterator<Item = PixelRange>) -> Option<Self> {
        Some(Self {
            pixels: Self::try_from_iter(pixels)?,
            color: [0, 0, 0],
        })
    }

    pub fn single_pixel_total_color(start: u32, len: NonZeroU32, color: [u8; 3]) -> Self {
        Self {
            pixels: PixelRanges::new(NonZeroRange::from_span(start, len), Meta::default()),
            color,
        }
    }

    pub fn single_pixel_total_black(start: u32, len: NonZeroU32) -> Self {
        Self::single_pixel_total_color(start, len, [0, 0, 0])
    }

    fn try_from_iter(pixels: impl IntoIterator<Item = PixelRange>) -> Option<PixelRanges> {
        PixelRanges::try_from_ordered_iter(
            pixels
                .into_iter()
                .map(|r| (r.0.range.start..r.0.range.end, r.0.meta)),
        )
        .ok()
    }

    pub fn from_ranges(pixels: PixelRanges, color: [u8; 3]) -> Self {
        Self { pixels, color }
    }

    pub fn range_len(&self) -> usize {
        self.pixels.len()
    }
}

pub struct PixelRangeIter {
    inner: imagemask::SortedRangesMapIter<
        std::vec::IntoIter<u32>,
        std::vec::IntoIter<u32>,
        std::vec::IntoIter<Meta>,
        NonZeroRange<u64>,
    >,
}

impl Iterator for PixelRangeIter {
    type Item = PixelRange;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|r| PixelRange(r))
    }
}

impl IntoIterator for PixelArea {
    type Item = PixelRange;
    type IntoIter = PixelRangeIter;

    fn into_iter(self) -> Self::IntoIter {
        PixelRangeIter {
            inner: self.pixels.iter_owned(),
        }
    }
}
#[derive(std::fmt::Debug)]
pub struct RemovedAll;

pub(crate) fn remove_overlaps(
    annotations: PixelArea,
    ordered_existing: impl IntoIterator<Item = PixelRange>,
) -> Result<PixelArea, RemovedAll> {
    let ordered_existing = ordered_existing.into_iter();
    let color = annotations.color;
    let ranges = &annotations.pixels;
    let mut peekable_ordered_existing = ordered_existing
        .map(|subgroup| (subgroup.start(), subgroup.end()))
        .peekable();

    let mut new_ranges: Vec<(std::ops::Range<u32>, Meta)> = Vec::new();

    for (range, meta) in ranges.iter::<RangeInclusive<u32>>() {
        let mut new_pos = *range.start() as u32;
        let new_end = (*range.end() + 1) as u32;
        let meta = *meta;

        // Skip existing ranges that end before our current position
        while let Some((_, existing_end)) = peekable_ordered_existing.peek() {
            if *existing_end <= new_pos {
                peekable_ordered_existing.next();
            } else {
                break;
            }
        }

        // Process existing ranges that overlap with [new_pos, new_end)
        while let Some((existing_start, existing_end)) = peekable_ordered_existing.peek() {
            if *existing_start >= new_end {
                // No more overlaps
                break;
            }

            let (existing_start, existing_end) = (*existing_start, *existing_end);

            if existing_start > new_pos {
                // There's a gap before this existing range
                new_ranges.push((new_pos..existing_start, meta));
            }

            if existing_end >= new_end {
                // Existing range covers the rest of our range
                new_pos = new_end;
                break;
            } else {
                // Existing range ends before our end, continue from there
                new_pos = existing_end;
                peekable_ordered_existing.next();
            }
        }

        // Add remaining range if any
        if new_pos < new_end {
            new_ranges.push((new_pos..new_end, meta));
        }
    }

    Ok(PixelArea {
        color,
        pixels: PixelRanges::try_from_ordered_iter(new_ranges).map_err(|_| RemovedAll)?,
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use super::*;

    const NON_ZERO_4: NonZero<u32> = NonZero::new(4).unwrap();

    fn collect_pixels(area: &PixelArea) -> Vec<PixelRange> {
        area.pixels
            .iter::<NonZeroRange<u64>>()
            .map(|x| {
                MetaRange {
                    range: x.range,
                    meta: *x.meta,
                }
                .into()
            })
            .collect()
    }

    #[test]
    fn overlapping_before() {
        let annotation = PixelArea::single_pixel_total_black(2, NON_ZERO_4);
        let existing = vec![PixelRange::new_total(0, 3.try_into().unwrap())];
        let annotation = remove_overlaps(annotation, existing.into_iter()).unwrap();
        assert_eq!(
            collect_pixels(&annotation),
            vec![PixelRange::new_total(3, 3.try_into().unwrap())]
        )
    }

    #[test]
    fn existing_within_new() {
        let annotation = PixelArea::single_pixel_total_black(0, 6.try_into().unwrap());
        let existing = vec![PixelRange::new_total(1, 4.try_into().unwrap())];

        let annotation = remove_overlaps(annotation, existing.into_iter()).unwrap();
        assert_eq!(
            collect_pixels(&annotation),
            vec![
                PixelRange::new_total(0, 1.try_into().unwrap()),
                PixelRange::new_total(5, 1.try_into().unwrap())
            ]
        )
    }

    #[test]
    fn overlapping_both() {
        let annotation = PixelArea::single_pixel_total_black(2, 4.try_into().unwrap());
        let existing = vec![PixelRange::new_total(0, 6.try_into().unwrap())];
        remove_overlaps(annotation, existing.into_iter()).unwrap_err();
    }

    #[test]
    fn overlapping_twice() {
        let annotation = PixelArea::with_black_color(vec![
            PixelRange::new_total(2, 1.try_into().unwrap()),
            PixelRange::new_total(4, 1.try_into().unwrap()),
        ])
        .unwrap();
        let existing = vec![PixelRange::new_total(0, 6.try_into().unwrap())];
        remove_overlaps(annotation, existing.into_iter()).unwrap_err();
    }

    #[test]
    fn overlapping_end() {
        let annotation = PixelArea::single_pixel_total_black(1, 4.try_into().unwrap());
        let existing = vec![PixelRange::new_total(2, 6.try_into().unwrap())];
        let annotation = remove_overlaps(annotation, existing.into_iter()).unwrap();
        assert_eq!(
            collect_pixels(&annotation),
            vec![PixelRange::new_total(1, NonZeroU16::MIN)]
        )
    }

    #[test]
    fn overlapping_between() {
        let annotation = PixelArea::single_pixel_total_black(2, 4.try_into().unwrap());
        let existing = vec![
            PixelRange::new_total(0, 3.try_into().unwrap()),
            PixelRange::new_total(0, 8.try_into().unwrap()),
        ];
        remove_overlaps(annotation, existing.into_iter()).unwrap_err();
    }

    #[test]
    fn no_overlap_before() {
        let annotation = PixelArea::single_pixel_total_black(2, 4.try_into().unwrap());
        let existing = vec![PixelRange::new_total(0, 2.try_into().unwrap())];
        let annotation = remove_overlaps(annotation, existing.into_iter()).unwrap();
        assert_eq!(
            collect_pixels(&annotation),
            vec![PixelRange::new_total(2, 4.try_into().unwrap())]
        )
    }
}
