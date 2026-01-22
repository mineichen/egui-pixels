use egui::Pos2;

use crate::{RectSelection, Tool, ToolContext};

#[derive(Default)]
#[non_exhaustive]
pub struct ClearTool {
    rect_selection: RectSelection,
    /// Screen position where drag started (for visual feedback)
    drag_start_screen: Option<Pos2>,
}

impl Tool for ClearTool {
    fn handle_interaction(&mut self, mut ctx: ToolContext) {
        // Track drag start position in screen coordinates
        if ctx.response.drag_started() {
            self.drag_start_screen = ctx.response.interact_pointer_pos();
        }

        // Draw dotted rectangle while dragging
        if ctx.response.dragged() {
            if let (Some(start), Some(current)) =
                (self.drag_start_screen, ctx.response.interact_pointer_pos())
            {
                ctx.painter.draw_dotted_rect(start, current);
            }
        }

        // Clear drag start when drag stops
        if ctx.response.drag_stopped() {
            self.drag_start_screen = None;
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
