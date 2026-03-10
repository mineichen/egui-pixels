use std::num::{NonZeroU32, NonZeroU64};
use std::ops::RangeInclusive;

use imagemask::{NonZeroRange, SortedRangesMap};

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

pub type MetaRange = imagemask::MetaRange<NonZeroRange<u64>, Meta>;

pub trait CreateTotal {
    fn new_total(start: u64, length: NonZeroU64) -> Self;
}
impl CreateTotal for MetaRange {
    fn new_total(start: u64, length: NonZeroU64) -> Self {
        imagemask::MetaRange {
            range: NonZeroRange::from_span(start, length),
            meta: Default::default(),
        }
    }
}

type MetaRanges = SortedRangesMap<u32, u32, Vec<Meta>>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PixelArea {
    pub pixels: MetaRanges,
    pub color: [u8; 3],
}

impl PixelArea {
    pub fn new(pixels: impl IntoIterator<Item = MetaRange>, color: [u8; 3]) -> Option<Self> {
        Some(Self {
            pixels: Self::try_from_iter(pixels)?,
            color,
        })
    }

    pub fn with_black_color(pixels: impl IntoIterator<Item = MetaRange>) -> Option<Self> {
        Some(Self {
            pixels: Self::try_from_iter(pixels)?,
            color: [0, 0, 0],
        })
    }

    pub fn single_pixel_total_color(start: u32, len: NonZeroU32, color: [u8; 3]) -> Self {
        Self {
            pixels: MetaRanges::new(NonZeroRange::from_span(start, len), Meta::default()),
            color,
        }
    }

    pub fn single_pixel_total_black(start: u32, len: NonZeroU32) -> Self {
        Self::single_pixel_total_color(start, len, [0, 0, 0])
    }

    fn try_from_iter(pixels: impl IntoIterator<Item = MetaRange>) -> Option<MetaRanges> {
        MetaRanges::try_from_ordered_iter(
            pixels
                .into_iter()
                .map(|r| (r.range.start..r.range.end, r.meta)),
        )
        .ok()
    }

    pub fn from_ranges(pixels: MetaRanges, color: [u8; 3]) -> Self {
        Self { pixels, color }
    }

    pub fn range_len(&self) -> usize {
        self.pixels.len()
    }
}

#[derive(std::fmt::Debug)]
pub struct RemovedAll;

pub(crate) fn remove_overlaps(
    annotations: PixelArea,
    ordered_existing: impl IntoIterator<Item = MetaRange>,
) -> Result<PixelArea, RemovedAll> {
    let ordered_existing = ordered_existing.into_iter();
    let color = annotations.color;
    let ranges = &annotations.pixels;
    let mut peekable_ordered_existing = ordered_existing
        .map(|subgroup| (subgroup.range.start, subgroup.range.end))
        .peekable();

    let mut new_ranges: Vec<(std::ops::Range<u64>, Meta)> = Vec::new();

    for (range, meta) in ranges.iter::<RangeInclusive<u64>>() {
        let mut new_pos = *range.start();
        let new_end = *range.end() + 1;
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
        pixels: MetaRanges::try_from_ordered_iter(new_ranges).map_err(|_| RemovedAll)?,
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use super::*;

    const NON_ZERO_4: NonZero<u32> = NonZero::new(4).unwrap();

    fn collect_pixels(area: &PixelArea) -> Vec<MetaRange> {
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
        let existing = vec![MetaRange::new_total(0, 3.try_into().unwrap())];
        let annotation = remove_overlaps(annotation, existing.into_iter()).unwrap();
        assert_eq!(
            collect_pixels(&annotation),
            vec![MetaRange::new_total(3, 3.try_into().unwrap())]
        )
    }

    #[test]
    fn existing_within_new() {
        let annotation = PixelArea::single_pixel_total_black(0, 6.try_into().unwrap());
        let existing = vec![MetaRange::new_total(1, 4.try_into().unwrap())];

        let annotation = remove_overlaps(annotation, existing.into_iter()).unwrap();
        assert_eq!(
            collect_pixels(&annotation),
            vec![
                MetaRange::new_total(0, 1.try_into().unwrap()),
                MetaRange::new_total(5, 1.try_into().unwrap())
            ]
        )
    }

    #[test]
    fn overlapping_both() {
        let annotation = PixelArea::single_pixel_total_black(2, 4.try_into().unwrap());
        let existing = vec![MetaRange::new_total(0, 6.try_into().unwrap())];
        remove_overlaps(annotation, existing.into_iter()).unwrap_err();
    }

    #[test]
    fn overlapping_twice() {
        let annotation = PixelArea::with_black_color(vec![
            MetaRange::new_total(2, 1.try_into().unwrap()),
            MetaRange::new_total(4, 1.try_into().unwrap()),
        ])
        .unwrap();
        let existing = vec![MetaRange::new_total(0, 6.try_into().unwrap())];
        remove_overlaps(annotation, existing.into_iter()).unwrap_err();
    }

    #[test]
    fn overlapping_end() {
        let annotation = PixelArea::single_pixel_total_black(1, 4.try_into().unwrap());
        let existing = vec![MetaRange::new_total(2, 6.try_into().unwrap())];
        let annotation = remove_overlaps(annotation, existing.into_iter()).unwrap();
        assert_eq!(
            collect_pixels(&annotation),
            vec![MetaRange::new_total(1, NonZeroU64::MIN)]
        )
    }

    #[test]
    fn overlapping_between() {
        let annotation = PixelArea::single_pixel_total_black(2, 4.try_into().unwrap());
        let existing = vec![
            MetaRange::new_total(0, 3.try_into().unwrap()),
            MetaRange::new_total(0, 8.try_into().unwrap()),
        ];
        remove_overlaps(annotation, existing.into_iter()).unwrap_err();
    }

    #[test]
    fn no_overlap_before() {
        let annotation = PixelArea::single_pixel_total_black(2, 4.try_into().unwrap());
        let existing = vec![MetaRange::new_total(0, 2.try_into().unwrap())];
        let annotation = remove_overlaps(annotation, existing.into_iter()).unwrap();
        assert_eq!(
            collect_pixels(&annotation),
            vec![MetaRange::new_total(2, 4.try_into().unwrap())]
        )
    }
}
