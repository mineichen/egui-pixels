use eframe::egui;

use crate::app::{ImageState, ImageStateLoaded, ImageViewerApp};

impl ImageViewerApp {
    pub(super) fn handle_clear_interaction(
        &mut self,
        response: egui::Response,
        cursor_image_pos: (usize, usize),
        ctx: &egui::Context,
    ) {
        if let ImageState::Loaded(ImageStateLoaded { masks, .. }) = &mut self.image_state {
            if let Some(region) = self.tools.drag_stopped(cursor_image_pos, &response, ctx) {
                masks.clear_region(region);
            } else if response.clicked() {
                masks.clear_region([[cursor_image_pos.0, cursor_image_pos.1]; 2]);
            }
        }
    }
}
