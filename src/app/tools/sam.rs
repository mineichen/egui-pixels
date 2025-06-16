use std::sync::Arc;

use eframe::egui;
use futures::FutureExt;
use image::DynamicImage;
use log::warn;

use crate::{
    app::{ImageState, ImageStateLoaded, ImageViewerApp},
    async_task::AsyncRefTask,
    inference::{InferenceError, SamEmbeddings, SamSession},
};

pub struct SamTool(
    AsyncRefTask<Result<SamEmbeddings, InferenceError>>,
    SamSession,
);

impl SamTool {
    pub fn new(session: SamSession, img: Arc<DynamicImage>) -> Self {
        Self(
            AsyncRefTask::new(session.get_image_embeddings(img).boxed()),
            session,
        )
    }
}

impl super::Tool for SamTool {
    fn handle_interaction(
        &mut self,
        app: &mut ImageViewerApp,
        response: egui::Response,
        cursor_image_pos: (usize, usize),
        ctx: &egui::Context,
    ) {
        if let (
            ImageState::Loaded(ImageStateLoaded { masks, .. }),
            Some([[top_x, top_y], [bottom_x, bottom_y]]),
        ) = (
            &mut app.image_state,
            app.tools.drag_stopped(cursor_image_pos, &response, ctx),
        ) {
            if let Some(Ok(loaded_embeddings)) = self.0.data() {
                let new_mask = self
                    .1
                    .decode_prompt(
                        top_x as f32,
                        top_y as f32,
                        bottom_x as f32,
                        bottom_y as f32,
                        loaded_embeddings,
                    )
                    .unwrap();

                masks.add_subgroups(new_mask);

                if let Some((_, _, loaded)) = app.selector.current() {
                    *loaded = true;
                } else {
                    warn!("Couldn't mark URL as containing masks")
                }
            }
        }
    }
}
