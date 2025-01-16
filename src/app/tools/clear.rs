use eframe::egui;

use crate::app::{ImageState, ImageStateLoaded, ImageViewerApp};

impl ImageViewerApp {
    pub(super) fn handle_clear_interaction(
        &mut self,
        response: egui::Response,
        cursor_image_pos: Option<(usize, usize)>,
        ctx: &egui::Context,
    ) {
        if let (ImageState::Loaded(ImageStateLoaded { masks, .. }), Some(region)) = (
            &mut self.image_state,
            self.tools.drag_stopped(cursor_image_pos, &response, ctx),
        ) {
            //println!("Enqueue region drop {region:?}");
            masks.clear_region(region);

            // if let Some((_, _, loaded)) = self.selector.current() {
            //     *loaded = true;
            // } else {
            //     warn!("Couldn't mark URL as containing masks")
            // }
        }
    }
}
