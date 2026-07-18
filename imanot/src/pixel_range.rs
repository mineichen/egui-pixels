use std::num::NonZeroU32;

use imask::{ImageDimension, SortedRanges, SortedRangesSpanIter, SourceIterator, Span};

type Ranges = SortedRanges<u32, u32>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PixelArea {
    pub pixels: Ranges,
    pub color: [u8; 4],
}

impl PixelArea {
    pub fn new(
        pixels: impl IntoIterator<Item = Span<u32>, IntoIter: ImageDimension>,
        color: [u8; 4],
    ) -> Option<Self> {
        Some(Self {
            pixels: Self::try_from_spans(pixels)?,
            color,
        })
    }

    pub fn map_inplace<TIter, TFun>(self, f: TFun) -> Option<Self>
    where
        TIter: Iterator<Item = std::ops::RangeInclusive<u64>>,
        TFun: FnOnce(SourceIterator<u32, u32>) -> TIter,
    {
        Some(Self {
            pixels: self.pixels.map_inplace(f)?,
            color: self.color,
        })
    }

    pub fn map_span_inplace<TIter, TFun>(self, f: TFun) -> Option<Self>
    where
        TIter: Iterator<Item = Span<u64>>,
        TFun: FnOnce(SortedRangesSpanIter<SourceIterator<u32, u32>>) -> TIter,
    {
        Some(Self {
            pixels: self.pixels.map_span_inplace(f)?,
            color: self.color,
        })
    }

    pub fn with_black_color(
        pixels: impl IntoIterator<Item = Span<u32>, IntoIter: ImageDimension>,
    ) -> Option<Self> {
        Some(Self {
            pixels: Self::try_from_spans(pixels)?,
            color: [0, 0, 0, 255],
        })
    }

    pub fn single_pixel_total_color(x: u32, y: u32, len: NonZeroU32, color: [u8; 4]) -> Self {
        Self {
            pixels: Ranges::from(Span::new(x..x + len.get(), y)),
            color,
        }
    }
    #[cfg(test)]
    pub fn single_range_total_black(x: u32, y: u32, len: NonZeroU32) -> Self {
        Self::single_pixel_total_color(x, y, len, [0, 0, 0, 255])
    }

    fn try_from_spans(
        pixels: impl IntoIterator<Item = Span<u32>, IntoIter: ImageDimension>,
    ) -> Option<Ranges> {
        let iter = pixels.into_iter();
        Ranges::try_from_span_iter(iter).ok()
    }

    pub fn from_ranges(pixels: Ranges, color: [u8; 4]) -> Self {
        Self { pixels, color }
    }

    pub fn range_len(&self) -> usize {
        self.pixels.len()
    }
}
