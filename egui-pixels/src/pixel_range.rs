use std::num::NonZeroU16;

use itertools::Itertools;

use crate::NextInPlaceExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PixelRange {
    pub position: u32,
    pub length: NonZeroU16,
    pub confidence: u8,
}

#[cfg(feature = "serde")]
impl serde::Serialize for PixelRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeTuple;
        if self.confidence == 255 {
            let mut tuple = serializer.serialize_tuple(2)?;
            tuple.serialize_element(&self.position)?;
            tuple.serialize_element(&self.length.get())?;
            tuple.end()
        } else {
            let mut tuple = serializer.serialize_tuple(3)?;
            tuple.serialize_element(&self.position)?;
            tuple.serialize_element(&self.length.get())?;
            tuple.serialize_element(&self.confidence)?;
            tuple.end()
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PixelRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PixelRangeVisitor;

        impl<'de> serde::de::Visitor<'de> for PixelRangeVisitor {
            type Value = PixelRange;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an array of 2 or 3 numbers, or a map with position, length, and optional confidence fields")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let position = seq
                    .next_element::<u32>()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;

                let length = seq
                    .next_element::<NonZeroU16>()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;

                // Check if there's a third element (confidence)
                if let Some(confidence) = seq.next_element::<u8>()? {
                    // 3-element array: [position, length, confidence]
                    Ok(PixelRange::new(position, length, confidence))
                } else {
                    // 2-element array: [position, length] with default confidence
                    Ok(PixelRange::new(position, length, 255))
                }
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut position = None;
                let mut length = None;
                let mut confidence = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "position" => {
                            if position.is_some() {
                                return Err(serde::de::Error::duplicate_field("position"));
                            }
                            position = Some(map.next_value::<u32>()?);
                        }
                        "length" => {
                            if length.is_some() {
                                return Err(serde::de::Error::duplicate_field("length"));
                            }
                            length = Some(map.next_value::<NonZeroU16>()?);
                        }
                        "confidence" => {
                            if confidence.is_some() {
                                return Err(serde::de::Error::duplicate_field("confidence"));
                            }
                            confidence = Some(map.next_value::<u8>()?);
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["position", "length", "confidence"],
                            ));
                        }
                    }
                }

                Ok(PixelRange::new(
                    position.ok_or_else(|| serde::de::Error::missing_field("position"))?,
                    length.ok_or_else(|| serde::de::Error::missing_field("length"))?,
                    confidence.unwrap_or(255),
                ))
            }
        }

        deserializer.deserialize_any(PixelRangeVisitor)
    }
}

impl PixelRange {
    pub fn new(position: u32, length: NonZeroU16, confidence: u8) -> Self {
        Self {
            position,
            length,
            confidence,
        }
    }

    pub fn new_total(position: u32, length: NonZeroU16) -> Self {
        Self {
            position,
            length,
            confidence: 255,
        }
    }

    pub fn as_range(&self) -> std::ops::Range<usize> {
        let start = self.position as usize;
        let end = start + self.length.get() as usize;
        start..end
    }

    pub fn confidence(&self) -> u8 {
        self.confidence
    }

    pub fn start_position(&self) -> u32 {
        self.position
    }

    pub fn end_position(&self) -> u32 {
        self.position + self.length.get() as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    use serde::Deserialize;
    use serde_json::json;

    #[test]
    fn deserialize_two_component() {
        let json = json!({ "pixels": [[1, 10]], "color": [0, 0, 0] });
        let pixel_area = PixelArea::deserialize(&json).unwrap();
        assert_eq!(
            pixel_area.pixels,
            vec![PixelRange::new_total(1, NonZeroU16::new(10).unwrap())]
        );
    }

    #[test]
    fn deserialize_pixel_area_with_three_component() {
        let json = json!({ "pixels": [[1, 10, 42]], "color": [0, 0, 0] });
        let pixel_area = PixelArea::deserialize(&json).unwrap();
        assert_eq!(
            pixel_area.pixels,
            vec![PixelRange::new(1, NonZeroU16::new(10).unwrap(), 42)]
        );
    }

    #[test]
    fn deserialize_pixel_area_with_two_component_with_default_confidence() {
        let json = json!({ "pixels": [[5, 3]], "color": [0, 0, 0] });
        let pixel_area = PixelArea::deserialize(&json).unwrap();
        assert_eq!(
            pixel_area.pixels,
            vec![PixelRange::new(5, NonZeroU16::new(3).unwrap(), 255)]
        );
    }

    #[test]
    fn deserialize_named_pixel_area_without_confidence() {
        let json = json!({ "pixels": [{ "position": 5, "length": 3 }], "color": [0, 0, 0] });
        let pixel_area = PixelArea::deserialize(&json).unwrap();
        assert_eq!(
            pixel_area.pixels,
            vec![PixelRange::new(5, NonZeroU16::new(3).unwrap(), 255)]
        );
        let json_text = serde_json::to_string(&pixel_area.pixels).unwrap();
        assert_eq!(json_text, "[[5,3]]");
    }
    #[test]
    fn deserialize_named_pixel_area_with_confidence() {
        let json = json!({ "pixels": [{ "position": 5, "length": 3, "confidence": 42 }], "color": [0, 0, 0] });
        let pixel_area = PixelArea::deserialize(&json).unwrap();
        assert_eq!(
            pixel_area.pixels,
            vec![PixelRange::new(5, NonZeroU16::new(3).unwrap(), 42)]
        );
        let json_text = serde_json::to_string(&pixel_area.pixels).unwrap();
        assert_eq!(json_text, "[[5,3,42]]");
    }

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
