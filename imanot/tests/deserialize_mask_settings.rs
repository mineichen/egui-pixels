#[test]
fn serialize_deserialize_mask_settings() {
    let mask_settings = imanot::MaskSettings::default();
    let serialized = serde_json::to_string(&mask_settings).unwrap();
    let deserialized: imanot::MaskSettings = serde_json::from_str(&serialized).unwrap();
    assert_eq!(mask_settings, deserialized);
}
