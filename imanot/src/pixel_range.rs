use std::num::{NonZero, NonZeroU32};

use imask::{ImageDimension, ImaskSet, NonZeroRange, SortedRanges, SourceIterator};

type Ranges = SortedRanges<u32, u32>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PixelArea {
    pub pixels: Ranges,
    pub color: [u8; 4],
}

impl PixelArea {
    pub fn new(
        pixels: impl IntoIterator<
            Item = NonZeroRange<u64>,
            IntoIter: ImageDimension,
        >,
        color: [u8; 4],
    ) -> Option<Self> {
        Some(Self {
            pixels: Self::try_from_iter(pixels)?,
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

    pub fn with_black_color(
        pixels: impl IntoIterator<
            Item = NonZeroRange<u64>,
            IntoIter: ImageDimension,
        >,
    ) -> Option<Self> {
        Some(Self {
            pixels: Self::try_from_iter(pixels)?,
            color: [0, 0, 0, 255],
        })
    }

    pub fn single_pixel_total_color(
        x: u32,
        y: u32,
        len: NonZeroU32,
        color: [u8; 4],
        image_width: NonZeroU32,
    ) -> Self {
        use imask::Rect;
        let start = x + y * image_width.get();
        let height = NonZero::new(y + 1).expect("Cannot be zero without overflow");
        Self {
            pixels: Ranges::new(
                NonZeroRange::from_span(start, len),
                Rect::new(0, 0, image_width, height),
            ),
            color,
        }
    }
    #[cfg(test)]
    pub fn single_range_total_black(x: u32, y: u32, len: NonZeroU32, width: NonZeroU32) -> Self {
        Self::single_pixel_total_color(x, y, len, [0, 0, 0, 255], width)
    }

    fn try_from_iter(
        pixels: impl IntoIterator<
            Item = NonZeroRange<u64>,
            IntoIter: ImageDimension,
        >,
    ) -> Option<Ranges> {
        let iter = pixels.into_iter();
        let roi = iter.bounds();
        Ranges::try_from_ordered_iter(iter.map(|r| r.start..r.end).with_roi(roi)).ok()
    }

    pub fn from_ranges(pixels: Ranges, color: [u8; 4]) -> Self {
        Self { pixels, color }
    }

    pub fn range_len(&self) -> usize {
        self.pixels.len()
    }
}
