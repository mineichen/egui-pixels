use crate::{RectSelection, Tool, ToolContext};

#[derive(Default)]
#[non_exhaustive]
pub struct ClearTool {
    rect_selection: RectSelection,
}

impl Tool for ClearTool {
    fn handle_interaction(&mut self, mut ctx: ToolContext) {
        let selection = self.rect_selection.drag_finished(&mut ctx);
        if let Some(region) = selection {
            (&mut ctx.image.masks).clear_rect(region.bounds());
        } else if ctx.response.clicked() {
            if let Some((x, y)) = ctx.cursor_image_pos() {
                (&mut ctx.image.masks).clear_rect([[x, y]; 2]);
            }
        }
    }
}
