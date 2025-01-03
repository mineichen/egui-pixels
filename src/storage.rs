use eframe::egui::ImageSource;

pub struct Storage {}

impl Storage {
    async fn list_images(&self) -> std::io::Result<Vec<ImageSource>> {
        Ok(vec![])
    }
}
