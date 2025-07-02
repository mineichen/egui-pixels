use egui::Vec2;

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    pub viewport: Vec2,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            viewport: [800.0, 800.0].into(),
        }
    }
}
