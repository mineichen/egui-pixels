use std::num::NonZeroU16;

use crate::PixelRange;

impl ::serde::Serialize for PixelRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        use ::serde::ser::SerializeTuple;
        if self.confidence == 255 {
            let mut tuple = serializer.serialize_tuple(2)?;
            tuple.serialize_element(&self.start)?;
            tuple.serialize_element(&self.length().get())?;
            tuple.end()
        } else {
            let mut tuple = serializer.serialize_tuple(3)?;
            tuple.serialize_element(&self.start)?;
            tuple.serialize_element(&self.length().get())?;
            tuple.serialize_element(&self.confidence)?;
            tuple.end()
        }
    }
}

impl<'de> ::serde::Deserialize<'de> for PixelRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        struct PixelRangeVisitor;

        impl<'de> ::serde::de::Visitor<'de> for PixelRangeVisitor {
            type Value = PixelRange;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an array of 2 or 3 numbers, or a map with position, length, and optional confidence fields")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: ::serde::de::SeqAccess<'de>,
            {
                let position = seq
                    .next_element::<u32>()?
                    .ok_or_else(|| ::serde::de::Error::invalid_length(0, &self))?;

                let length = seq
                    .next_element::<NonZeroU16>()?
                    .ok_or_else(|| ::serde::de::Error::invalid_length(1, &self))?;

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

#[cfg(test)]
mod tests {
    use crate::PixelArea;

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
}
