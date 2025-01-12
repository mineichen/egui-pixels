use std::{io, num::NonZeroU16, path::PathBuf};

use eframe::egui;
use image::DynamicImage;
use log::info;

use app::ImageViewerApp;

mod app;
mod async_task;
mod image_utils;
mod inference;
mod mask;
mod mask_generator;
mod storage;
mod viewer;

type SubGroup = (u32, NonZeroU16);
type SubGroups = Vec<SubGroup>;
type Annotation = (String, SubGroups);

use inference::SamSession;
use mask_generator::MaskGenerator;
pub use storage::Storage;

#[derive(serde::Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    sam_path: PathBuf,
    image_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sam_path: "sam".into(),
            image_dir: None,
        }
    }
}

type ImageCallbackMap = Vec<(String, Box<dyn Fn(&DynamicImage) -> Vec<SubGroups>>)>;

pub fn run_native(mappers: ImageCallbackMap) -> Result<(), eframe::Error> {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 800.0]),
        ..Default::default()
    };

    let config = match std::fs::File::open("config.json") {
        Ok(f) => serde_json::from_reader(f).map_err(|e| eframe::Error::AppCreation(Box::new(e)))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Config::default(),
        Err(e) => Err(eframe::Error::AppCreation(Box::new(e)))?,
    };

    let image_dir = std::env::args().nth(1).unwrap_or_else(|| {
        config
            .image_dir
            .as_ref()
            .and_then(|s| Some(s.to_str()?.to_string()))
            .unwrap_or_else(|| ".".into())
    });

    info!("Run with config: {config:?}");
    eframe::run_native(
        "Image Viewer",
        options,
        Box::new(|cc| {
            Ok(Box::new(ImageViewerApp::new(
                cc,
                Storage::new(image_dir),
                SamSession::new(&config.sam_path).unwrap(),
                MaskGenerator::new(mappers),
            )))
        }),
    )
}
