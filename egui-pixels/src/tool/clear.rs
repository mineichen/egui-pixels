use crate::{RectSelection, Tool, ToolContext};

#[derive(Default)]
#[non_exhaustive]
pub struct ClearTool {
    rect_selection: RectSelection,
}

impl Tool for ClearTool {
    fn handle_interaction(&mut self, mut ctx: ToolContext) {
        let drag_pos = self.rect_selection.drag_stopped(&mut ctx);
        let masks = &mut ctx.image.masks;
        if let Some(region) = drag_pos {
            masks.clear_rect(region);
        } else if ctx.response.clicked() {
            masks.clear_rect([[ctx.cursor_image_pos.0, ctx.cursor_image_pos.1]; 2]);
        }
    }
}
