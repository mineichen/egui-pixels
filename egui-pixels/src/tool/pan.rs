use egui::Vec2;
use futures::FutureExt;

use crate::{Tool, ToolContext, ToolFactory};

/// Pan tool for moving the viewport around the image
#[derive(Default)]
#[non_exhaustive]
pub struct PanTool;

impl PanTool {
    pub fn create_factory() -> ToolFactory {
        Box::new(|_| async { Ok(Box::new(PanTool::default()) as Box<dyn Tool + Send>) }.boxed())
    }
}

impl Tool for PanTool {
    fn handle_interaction(&mut self, ctx: ToolContext) {
        // Panning logic will be moved here from ImageViewer
        let viewer = ctx.viewer;
        let response = &ctx.response;

        let drag_delta = response.drag_delta();
        if drag_delta != Vec2::default() {
            let original_image_size = Vec2::new(
                ctx.image.image.original.width().get() as f32,
                ctx.image.image.original.height().get() as f32,
            );
            let viewport_size = response.rect.size();
            let fit_scale = (viewport_size.x / original_image_size.x)
                .min(viewport_size.y / original_image_size.y);
            let render_scale = fit_scale / viewer.zoom();

            let delta_norm = drag_delta / (render_scale * original_image_size);
            let mut new_offset = viewer.pan_offset() - delta_norm;

            let (min_pan, max_pan) =
                viewer.pan_bounds(original_image_size, viewport_size, render_scale);

            let move_left = viewer.pan_offset().x < new_offset.x;
            let move_top = viewer.pan_offset().y < new_offset.y;

            if !move_left && new_offset.x < min_pan.x {
                new_offset.x = viewer.pan_offset().x.min(min_pan.x);
            }

            if move_left && new_offset.x > max_pan.x {
                new_offset.x = viewer.pan_offset().x.max(max_pan.x);
            }

            if !move_top && new_offset.y < min_pan.y {
                new_offset.y = viewer.pan_offset().y.min(min_pan.y);
            }

            if move_top && new_offset.y > max_pan.y {
                new_offset.y = viewer.pan_offset().y.max(max_pan.y);
            }

            viewer.set_pan_offset(new_offset);
        }
    }
}
