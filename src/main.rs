use eframe::egui::{self};
use egui_pixels::ImageViewerApp;
use egui_pixels::Storage;

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
            Ok(Box::new(ImageViewerApp::new(
                cc,
                Storage::new(std::env::args().nth(1).unwrap_or_else(|| ".".to_string())),
            )))
        }),
    )
}
