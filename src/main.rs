use std::num::NonZeroU16;

use app::ImageViewerApp;
use eframe::egui::{self};

mod app;
mod history;
mod inference;
mod viewer;

type SubGroup = (u32, NonZeroU16);
type SubGroups = Vec<SubGroup>;
type Annotation = (String, SubGroups);

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Image Viewer",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Ok(Box::new(ImageViewerApp::new()))
        }),
    )
}
