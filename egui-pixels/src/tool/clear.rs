use egui::Pos2;

use crate::{RectSelection, Tool, ToolContext};

#[derive(Default)]
#[non_exhaustive]
pub struct ClearTool {
    rect_selection: RectSelection,
    /// Image position where drag started (in image pixel coordinates)
    drag_start_image: Option<Pos2>,
}

impl Tool for ClearTool {
    fn handle_interaction(&mut self, mut ctx: ToolContext) {
        // Track drag start position in image coordinates
        if ctx.response.drag_started() {
            self.drag_start_image = ctx
                .response
                .interact_pointer_pos()
                .map(|screen_pos| ctx.painter.screen_to_image(screen_pos));
        }

        // Draw dotted rectangle while dragging
        if ctx.response.dragged() {
            if let (Some(start_image), Some(current_screen)) =
                (self.drag_start_image, ctx.response.interact_pointer_pos())
            {
                // Convert stored image coordinates back to current screen coordinates
                let start_screen = ctx.painter.image_to_screen(start_image);
                ctx.painter.draw_dotted_rect(start_screen, current_screen);
            }
        }

        // Clear drag start when drag stops
        if ctx.response.drag_stopped() {
            self.drag_start_image = None;
        }

        let drag_pos = self.rect_selection.drag_stopped(&mut ctx);
        let masks = &mut ctx.image.masks;
        if let Some(region) = drag_pos {
            masks.clear_rect(region);
        } else if ctx.response.clicked() {
            masks.clear_rect([[ctx.cursor_image_pos.0, ctx.cursor_image_pos.1]; 2]);
        }
    }
}
