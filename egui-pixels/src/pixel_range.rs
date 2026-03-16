use std::num::{NonZeroU32, NonZeroU64};
use std::ops::RangeInclusive;

use imask::{NonZeroRange, SortedRangesMap, SourceIteratorMap};

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

pub type MetaRange = imask::MetaRange<NonZeroRange<u64>, Meta>;

pub trait CreateTotal {
    fn new_total(start: u64, length: NonZeroU64) -> Self;
}
impl CreateTotal for MetaRange {
    fn new_total(start: u64, length: NonZeroU64) -> Self {
        Self {
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

    pub fn map_inplace<TIter, TFun>(self, f: TFun) -> Option<Self>
    where
        TIter: Iterator<Item = (RangeInclusive<u64>, Meta)>,
        TFun: FnOnce(SourceIteratorMap<u32, u32, Meta>) -> TIter,
    {
        Some(Self {
            pixels: self.pixels.map_inplace(f)?,
            color: self.color,
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

    pub fn single_range_total_black(start: u32, len: NonZeroU32) -> Self {
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
