use std::{num::NonZeroU16, sync::atomic::AtomicU16};

use itertools::Itertools;

use crate::NextInPlaceExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubGroup {
    pub position: u32,
    pub length: NonZeroU16,
    // 255 means no other group is associated with these positions
    pub association: u8,
}

impl SubGroup {
    pub fn new(position: u32, length: NonZeroU16, association: u8) -> Self {
        Self {
            position,
            length,
            association,
        }
    }

    pub fn new_total(position: u32, length: NonZeroU16) -> Self {
        Self {
            position,
            length,
            association: 255,
        }
    }

    pub fn as_range(&self) -> std::ops::Range<usize> {
        let start = self.position as usize;
        let end = start + self.length.get() as usize;
        start..end
    }

    pub fn association(&self) -> u8 {
        self.association
    }

    pub fn start_position(&self) -> u32 {
        self.position
    }

    pub fn end_position(&self) -> u32 {
        self.position + self.length.get() as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Annotation {
    pub pixels: Vec<SubGroup>,
    pub color: [u8; 3],
}

static mut COLOR_SEED: AtomicU16 = AtomicU16::new(0);

impl Annotation {
    pub fn new(pixels: Vec<SubGroup>, color: [u8; 3]) -> Self {
        Self { pixels, color }
    }

    pub fn with_black_color(pixels: Vec<SubGroup>) -> Self {
        Self {
            pixels,
            color: [0, 0, 0],
        }
    }

    pub fn with_random_color(pixels: Vec<SubGroup>, seed: u16) -> Self {
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

    pub fn sub_group_len(&self) -> usize {
        self.pixels.len()
    }
}

pub(crate) fn remove_overlaps(
    annotations: &mut Annotation,
    ordered_existing: impl Iterator<Item = SubGroup>,
) {
    let mut peekable_ordered_existing = ordered_existing
        .map(|subgroup| (subgroup.position, subgroup.end_position()))
        .peekable();

    annotations.pixels.flat_map_inplace(|subgroup, i| {
        let mut new_pos = subgroup.position;
        let mut new_len = subgroup.length;
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
                i.insert(SubGroup::new_total(new_pos, len));
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

        i.insert(SubGroup::new_total(new_pos, new_len));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_before() {
        let mut annotation =
            Annotation::with_black_color(vec![SubGroup::new_total(2, 4.try_into().unwrap())]);
        let existing = vec![SubGroup::new_total(0, 3.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(
            annotation.pixels,
            vec![SubGroup::new_total(3, 3.try_into().unwrap())]
        )
    }

    #[test]
    fn existing_within_new() {
        let mut annotation =
            Annotation::with_black_color(vec![SubGroup::new_total(0, 6.try_into().unwrap())]);
        let existing = vec![SubGroup::new_total(1, 4.try_into().unwrap())];

        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(
            annotation.pixels,
            vec![
                SubGroup::new_total(0, 1.try_into().unwrap()),
                SubGroup::new_total(5, 1.try_into().unwrap())
            ]
        )
    }

    #[test]
    fn overlapping_both() {
        let mut annotation =
            Annotation::with_black_color(vec![SubGroup::new_total(2, 4.try_into().unwrap())]);
        let existing = vec![SubGroup::new_total(0, 6.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation.pixels, vec![])
    }

    #[test]
    fn overlapping_twice() {
        let mut annotation = Annotation::with_black_color(vec![
            SubGroup::new_total(2, 1.try_into().unwrap()),
            SubGroup::new_total(4, 1.try_into().unwrap()),
        ]);
        let existing = vec![SubGroup::new_total(0, 6.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation.pixels, vec![]);
    }

    #[test]
    fn overlapping_end() {
        let mut annotation =
            Annotation::with_black_color(vec![SubGroup::new_total(1, 4.try_into().unwrap())]);
        let existing = vec![SubGroup::new_total(2, 6.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(
            annotation.pixels,
            vec![SubGroup::new_total(1, NonZeroU16::MIN)]
        )
    }

    #[test]
    fn overlapping_between() {
        let mut annotation =
            Annotation::with_black_color(vec![SubGroup::new_total(2, 4.try_into().unwrap())]);
        let existing = vec![
            SubGroup::new_total(0, 3.try_into().unwrap()),
            SubGroup::new_total(0, 8.try_into().unwrap()),
        ];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation.pixels, vec![])
    }

    #[test]
    fn no_overlap_before() {
        let mut annotation =
            Annotation::with_black_color(vec![SubGroup::new_total(2, 4.try_into().unwrap())]);
        let existing = vec![SubGroup::new_total(0, 2.try_into().unwrap())];
        remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(
            annotation.pixels,
            vec![SubGroup::new_total(2, 4.try_into().unwrap())]
        )
    }
}
