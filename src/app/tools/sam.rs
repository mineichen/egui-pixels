use eframe::egui;
use log::warn;

use crate::app::{ImageState, ImageStateLoaded, ImageViewerApp};

impl ImageViewerApp {
    pub(super) fn handle_sam_interaction(
        &mut self,
        response: egui::Response,
        cursor_image_pos: Option<(usize, usize)>,
        ctx: &egui::Context,
    ) {
        if let (
            ImageState::Loaded(ImageStateLoaded {
                masks, embeddings, ..
            }),
            Some([[top_x, top_y], [bottom_x, bottom_y]]),
        ) = (
            &mut self.image_state,
            self.tools.drag_stopped(cursor_image_pos, &response, ctx),
        ) {
            if let Some(Ok(loaded_embeddings)) = embeddings.data() {
                let new_mask = self
                    .tools
                    .session
                    .decode_prompt(
                        top_x as f32,
                        top_y as f32,
                        bottom_x as f32,
                        bottom_y as f32,
                        loaded_embeddings,
                    )
                    .unwrap();

                masks.add_subgroup(("New group".into(), new_mask));

                if let Some((_, _, loaded)) = self.selector.current() {
                    *loaded = true;
                } else {
                    warn!("Couldn't mark URL as containing masks")
                }
            }
        }
    }
}
