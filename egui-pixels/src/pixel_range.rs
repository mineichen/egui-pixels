use std::num::{NonZeroU16, NonZeroU32};
use std::ops::Range;

use imagemask::{MetaRange, NonEmptyOrderedRanges, NonZeroRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PixelRange {
    start: u32,
    end: u32,
    pub confidence: u8,
}

impl PixelRange {
    pub fn new(start: u32, length: NonZeroU16, confidence: u8) -> Self {
        Self {
            start,
            end: start + length.get() as u32,
            confidence,
        }
    }

    pub fn new_total(start: u32, length: NonZeroU16) -> Self {
        Self::new(start, length, 255)
    }

    pub fn as_range(&self) -> Range<usize> {
        let start = self.start as usize;
        let end = self.end as usize;
        start..end
    }

    pub fn confidence(&self) -> u8 {
        self.confidence
    }

    pub fn start(&self) -> u32 {
        self.start
    }

    pub fn length(&self) -> NonZeroU16 {
        let unchecked = self.end - self.start;
        debug_assert!(
            u16::try_from(unchecked).is_ok(),
            "length is never > u16::MAX"
        );
        NonZeroU16::new(unchecked as u16).expect("length is never > u16::MAX")
    }

    pub fn increment_length(&mut self) {
        self.end = self.end.checked_add(1).expect("length is never > u16::MAX");
        debug_assert!(
            u16::try_from(self.end - self.start).is_ok(),
            "length is never > u16::MAX"
        );
    }

    pub fn end(&self) -> u32 {
        self.end
    }
}

type PixelRanges = NonEmptyOrderedRanges<u32, u32, Vec<u8>>;

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
            pixels: NonEmptyOrderedRanges::new(NonZeroRange::from_span(start, len), 255),
            color,
        }
    }

    pub fn single_pixel_total_black(start: u32, len: NonZeroU32) -> Self {
        Self::single_pixel_total_color(start, len, [0, 0, 0])
    }

    fn try_from_iter(pixels: impl IntoIterator<Item = PixelRange>) -> Option<PixelRanges> {
        NonEmptyOrderedRanges::try_from_ordered_iter(
            pixels.into_iter().map(|r| (r.start..r.end, r.confidence)),
        )
        .ok()
    }

    pub fn from_ranges(pixels: PixelRanges, color: [u8; 3]) -> Self {
        Self { pixels, color }
    }

    pub fn range_len(&self) -> usize {
        self.pixels.iter().count()
    }

    pub fn iter_pixel_ranges(&self) -> impl Iterator<Item = PixelRange> + '_ {
        self.pixels
            .iter()
            .map(|MetaRange { range, meta }| PixelRange {
                start: range.start as u32,
                end: range.end as u32,
                confidence: *meta,
            })
    }
}

pub struct PixelRangeIter {
    inner: imagemask::OrderedRangeIter<
        std::vec::IntoIter<u32>,
        std::vec::IntoIter<u32>,
        std::vec::IntoIter<u8>,
    >,
}

impl Iterator for PixelRangeIter {
    type Item = PixelRange;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|MetaRange { range, meta }| PixelRange {
                start: range.start as u32,
                end: range.end as u32,
                confidence: meta,
            })
    }
}

impl IntoIterator for PixelArea {
    type Item = PixelRange;
    type IntoIter = PixelRangeIter;

    fn into_iter(self) -> Self::IntoIter {
        PixelRangeIter {
            inner: self.pixels.into_iter(),
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

    let mut new_ranges: Vec<(std::ops::Range<u32>, u8)> = Vec::new();

    for MetaRange { range, meta } in ranges.iter() {
        let mut new_pos = range.start as u32;
        let new_end = range.end as u32;
        let confidence = *meta;

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
                new_ranges.push((new_pos..existing_start, confidence));
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
            new_ranges.push((new_pos..new_end, confidence));
        }
    }

    Ok(PixelArea {
        color,
        pixels: NonEmptyOrderedRanges::try_from_ordered_iter(new_ranges).map_err(|_| RemovedAll)?,
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use super::*;

    const NON_ZERO_4: NonZero<u32> = NonZero::new(4).unwrap();

    fn collect_pixels(area: &PixelArea) -> Vec<PixelRange> {
        area.iter_pixel_ranges().collect()
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
