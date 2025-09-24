use std::num::NonZeroU16;

use itertools::Itertools;

use crate::NextInPlaceExt;

#[cfg(feature = "serde")]
mod serde;

#[cfg(feature = "serde")]
pub use serde::FromStartEndPixelRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PixelRange {
    start: u32,
    // Is always between 1..u16::MAX after start
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

    pub fn as_range(&self) -> std::ops::Range<usize> {
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[non_exhaustive]
pub struct PixelArea {
    pub pixels: Vec<PixelRange>,
    pub color: [u8; 3],
}

impl PixelArea {
    pub fn new(pixels: Vec<PixelRange>, color: [u8; 3]) -> Self {
        Self { pixels, color }
    }

    pub fn with_black_color(pixels: Vec<PixelRange>) -> Self {
        Self {
            pixels,
            color: [0, 0, 0],
        }
    }

    pub fn with_random_color(pixels: Vec<PixelRange>, seed: u16) -> Self {
        fn pseudo_random_permutation(seed: u16) -> f32 {
            let mut num = (seed & 0xFF) as u8;

            for _ in 0..2 {
                num = num.wrapping_mul(197).rotate_left(5) ^ 0x5A;
            }

            num as f32 / (u8::MAX as f32)
        }

        fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
            let h_i = (h * 6.0).floor() as u32 % 6;
            let f = h * 6.0 - h_i as f32;
            let p = v * (1.0 - s);
            let q = v * (1.0 - f * s);
            let t = v * (1.0 - (1.0 - f) * s);

            let (r, g, b) = match h_i {
                0 => (v, t, p),
                1 => (q, v, p),
                2 => (p, v, t),
                3 => (p, q, v),
                4 => (t, p, v),
                _ => (v, p, q),
            };

            [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
        }

        Self {
            pixels,
            color: hsv_to_rgb(pseudo_random_permutation(seed), 0.7, 0.95),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pixels.is_empty()
    }

    pub fn range_len(&self) -> usize {
        self.pixels.len()
    }
}

pub(crate) fn remove_overlaps(
    annotations: &mut PixelArea,
    ordered_existing: impl Iterator<Item = PixelRange>,
) {
    let mut peekable_ordered_existing = ordered_existing
        .map(|subgroup| (subgroup.start, subgroup.end()))
        .peekable();

    annotations.pixels.flat_map_inplace(|subgroup, i| {
        let mut new_pos = subgroup.start;
        let mut new_len = subgroup.length();
        let new_end = new_pos + new_len.get() as u32;

        // Overlap start or within new
        for (existing_pos, existing_end) in peekable_ordered_existing
            .peeking_take_while(|(_, existing_end)| new_end > *existing_end)
        {
            if new_pos > existing_end {
                continue;
            } else if let Ok(len) =
                NonZeroU16::try_from(existing_pos.saturating_sub(new_pos) as u16)
            {
                i.insert(PixelRange::new_total(new_pos, len));
            }
            if existing_end > new_pos {
                let offset = existing_end - new_pos;
                if let Ok(x) = NonZeroU16::try_from(new_len.get().saturating_sub(offset as _)) {
                    new_pos += offset;
                    new_len = x;
                } else {
                    return;
                }
            }
        }
        // Overlaps end of new
        if let Some((existing_pos, _)) = peekable_ordered_existing.peek() {
            if let Ok(x) = NonZeroU16::try_from(
                new_len
                    .get()
                    .saturating_sub(new_end.saturating_sub(*existing_pos) as _),
            ) {
                new_len = x;
            } else {
                return;
            }
        }

        i.insert(PixelRange::new_total(new_pos, new_len));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_before() {
        let mut annotation =
            PixelArea::with_black_color(vec![PixelRange::new_total(2, 4.try_into().unwrap())]);
        let existing = vec![PixelRange::new_total(0, 3.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(
            annotation.pixels,
            vec![PixelRange::new_total(3, 3.try_into().unwrap())]
        )
    }

    #[test]
    fn existing_within_new() {
        let mut annotation =
            PixelArea::with_black_color(vec![PixelRange::new_total(0, 6.try_into().unwrap())]);
        let existing = vec![PixelRange::new_total(1, 4.try_into().unwrap())];

        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(
            annotation.pixels,
            vec![
                PixelRange::new_total(0, 1.try_into().unwrap()),
                PixelRange::new_total(5, 1.try_into().unwrap())
            ]
        )
    }

    #[test]
    fn overlapping_both() {
        let mut annotation =
            PixelArea::with_black_color(vec![PixelRange::new_total(2, 4.try_into().unwrap())]);
        let existing = vec![PixelRange::new_total(0, 6.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation.pixels, vec![])
    }

    #[test]
    fn overlapping_twice() {
        let mut annotation = PixelArea::with_black_color(vec![
            PixelRange::new_total(2, 1.try_into().unwrap()),
            PixelRange::new_total(4, 1.try_into().unwrap()),
        ]);
        let existing = vec![PixelRange::new_total(0, 6.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation.pixels, vec![]);
    }

    #[test]
    fn overlapping_end() {
        let mut annotation =
            PixelArea::with_black_color(vec![PixelRange::new_total(1, 4.try_into().unwrap())]);
        let existing = vec![PixelRange::new_total(2, 6.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(
            annotation.pixels,
            vec![PixelRange::new_total(1, NonZeroU16::MIN)]
        )
    }

    #[test]
    fn overlapping_between() {
        let mut annotation =
            PixelArea::with_black_color(vec![PixelRange::new_total(2, 4.try_into().unwrap())]);
        let existing = vec![
            PixelRange::new_total(0, 3.try_into().unwrap()),
            PixelRange::new_total(0, 8.try_into().unwrap()),
        ];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation.pixels, vec![])
    }

    #[test]
    fn no_overlap_before() {
        let mut annotation =
            PixelArea::with_black_color(vec![PixelRange::new_total(2, 4.try_into().unwrap())]);
        let existing = vec![PixelRange::new_total(0, 2.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(
            annotation.pixels,
            vec![PixelRange::new_total(2, 4.try_into().unwrap())]
        )
    }
}
