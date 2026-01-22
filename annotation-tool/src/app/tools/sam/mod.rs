use egui_pixels::{AsyncRefTask, PixelArea, RectSelection, ToolContext};
use futures::FutureExt;
use imbuf::Image;

use inference::{InferenceError, SamEmbeddings};

mod inference;
mod native_ort;

pub use native_ort::SamSession;

type RgbImageInterleaved<T> = imbuf::Image<[T; 3], 1>;

pub struct SamTool {
    embeddings: AsyncRefTask<Result<SamEmbeddings, InferenceError>>,
    session: SamSession,
    rect_selection: RectSelection,
    // If selection starts, before embeddings are ready
    last_pos: Option<[[usize; 2]; 2]>,
}

impl SamTool {
    pub fn new(session: SamSession, img: Image<[u8; 3], 1>) -> Self {
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
        if let Some(x) = self.rect_selection.drag_finished(&mut ctx) {
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

            ctx.image
                .masks
                .add_area_non_overlapping_parts(PixelArea::with_random_color(
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
