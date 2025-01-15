use eframe::egui;
use log::warn;

use super::{ImageState, ImageStateLoaded};

#[derive(Debug, Default)]
pub(super) struct Tools {
    last_drag_start: Option<(usize, usize)>,
}

impl Tools {
    fn ui(&mut self, ui: &mut egui::Ui) {}
}

impl super::ImageViewerApp {
    pub(super) fn handle_interaction(
        &mut self,
        response: egui::Response,
        cursor_image_pos: Option<(usize, usize)>,
        ui: &mut egui::Ui,
    ) {
        if response.drag_started() {
            self.tools.last_drag_start = cursor_image_pos;
        }

        if let (
            Some(&(cursor_x, cursor_y)),
            ImageState::Loaded(ImageStateLoaded {
                masks, embeddings, ..
            }),
            Some(&(start_x, start_y)),
            true,
        ) = (
            cursor_image_pos.as_ref(),
            &mut self.image_state,
            self.tools.last_drag_start.as_ref(),
            response.drag_stopped() && !ui.input(|i| i.modifiers.command || i.modifiers.ctrl),
        ) {
            if let Some(Ok(loaded_embeddings)) = embeddings.data() {
                let new_mask = self
                    .session
                    .decode_prompt(
                        cursor_x.min(start_x) as f32,
                        cursor_y.min(start_y) as f32,
                        cursor_x.max(start_x) as f32,
                        cursor_y.max(start_y) as f32,
                        loaded_embeddings,
                    )
                    .unwrap();

                masks.add_subgroup(("New group".into(), new_mask));

                if let Some((_, _, loaded)) = self.selector.current() {
                    *loaded = true;
                } else {
                    warn!("Couldn't mark URL as containing masks")
                }

                self.tools.last_drag_start = None;
            }
        }
    }
}
