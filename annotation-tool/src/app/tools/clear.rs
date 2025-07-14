use egui_pixels::{ImageState, ImageStateLoaded};

use crate::app::tools::Tool;

#[derive(Default)]
#[non_exhaustive]
pub struct ClearTool {}

impl Tool for ClearTool {
    fn handle_interaction(
        &mut self,
        app: &mut crate::app::ImageViewerApp,
        response: egui::Response,
        cursor_image_pos: (usize, usize),
        ctx: &egui::Context,
    ) {
        if let ImageState::Loaded(ImageStateLoaded { masks, .. }) = &mut app.image_state {
            if let Some(region) = app.tools.drag_stopped(cursor_image_pos, &response, ctx) {
                masks.clear_region(region);
            } else if response.clicked() {
                masks.clear_region([[cursor_image_pos.0, cursor_image_pos.1]; 2]);
            }
        }
    }
}
