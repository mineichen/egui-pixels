use std::path::PathBuf;

#[derive(serde::Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub sam_path: PathBuf,
    pub image_dir: Option<PathBuf>,
    pub egui: crate::app::Config,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sam_path: "sam".into(),
            image_dir: None,
            egui: Default::default(),
        }
    }
}
