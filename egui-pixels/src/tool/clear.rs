use std::num::NonZeroU16;

use futures::FutureExt;

use crate::{PixelRange, RectSelection, Tool, ToolContext, ToolFactory};

#[derive(Default)]
#[non_exhaustive]
pub struct ClearTool {
    rect_selection: RectSelection,
}

impl ClearTool {
    pub fn create_factory() -> ToolFactory {
        Box::new(|_| async { Ok(Box::new(ClearTool::default()) as Box<dyn Tool + Send>) }.boxed())
    }
}

impl Tool for ClearTool {
    fn handle_interaction(&mut self, mut ctx: ToolContext) {
        let selection = self.rect_selection.drag_finished(&mut ctx);
        if let Some(region) = selection {
            ctx.image.masks.clear_ranges(region.iter_ranges(255));
        } else if ctx.response.clicked()
            && let Some((x, y)) = ctx.cursor_image_pos()
        {
            let image_width = ctx.image.image.original.width();
            let pos = y as u32 * image_width.get() + x as u32;
            let single_pixel = std::iter::once(PixelRange::new_total(pos, NonZeroU16::MIN));
            ctx.image.masks.clear_ranges(single_pixel);
        }
    }
}
