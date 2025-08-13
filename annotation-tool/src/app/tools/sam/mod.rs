use std::sync::Arc;

use egui_pixels::{AsyncRefTask, PixelArea, RectSelection, ToolContext};
use futures::FutureExt;
use image::DynamicImage;

use inference::{InferenceError, SamEmbeddings};

mod inference;
mod native_ort;

pub use native_ort::SamSession;

pub struct SamTool {
    embeddings: AsyncRefTask<Result<SamEmbeddings, InferenceError>>,
    session: SamSession,
    rect_selection: RectSelection,
    // If selection starts, before embeddings are ready
    last_pos: Option<[[usize; 2]; 2]>,
}

impl SamTool {
    pub fn new(session: SamSession, img: Arc<DynamicImage>) -> Self {
        Self {
            embeddings: AsyncRefTask::new(session.get_image_embeddings(img).boxed()),
            session,
            rect_selection: RectSelection::default(),
            last_pos: None,
        }
    }
}

impl super::Tool for SamTool {
    fn handle_interaction(&mut self, mut ctx: ToolContext) {
        if let Some(x) = self.rect_selection.drag_stopped(&mut ctx) {
            self.last_pos = Some(x);
        }
        if let (Some([[top_x, top_y], [bottom_x, bottom_y]]), Some(Ok(loaded_embeddings))) =
            (self.last_pos, self.embeddings.data())
        {
            let new_mask = self
                .session
                .decode_prompt(
                    top_x as f32,
                    top_y as f32,
                    bottom_x as f32,
                    bottom_y as f32,
                    loaded_embeddings,
                )
                .unwrap();

            ctx.image.masks.add_area(PixelArea::with_random_color(
                new_mask,
                ctx.image.masks.random_seed(),
            ));
            self.last_pos = None;

            // if let Some(x) = ctx.app.selector.current() {
            //     x.has_masks = true;
            // } else {
            //     warn!("Couldn't mark URL as containing masks")
            // }
        }
    }
}
