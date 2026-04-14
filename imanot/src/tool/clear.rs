use std::num::{NonZero, NonZeroU64};

use futures::FutureExt;
use imask::{ImaskSet, NonZeroRange};

use crate::{RectSelection, Tool, ToolContext, ToolFactory};

#[derive(Default)]
#[non_exhaustive]
pub struct ClearTool {
    rect_selection: RectSelection,
}

impl ClearTool {
    pub fn create_factory() -> ToolFactory {
        Box::new(|_| async { Ok(Box::new(ClearTool::default()) as Box<dyn Tool>) }.boxed_local())
    }
}

impl Tool for ClearTool {
    fn handle_interaction(&mut self, mut ctx: ToolContext) {
        let selection = self.rect_selection.drag_finished(&mut ctx);
        if let Some(region) = selection {
            ctx.image.masks.clear_ranges(region.iter_ranges());
        } else if ctx.response.clicked()
            && let Some((x, y)) = ctx.cursor_image_pos()
        {
            let image_width = ctx.image.image.original.width();
            let pos = y as u64 * image_width.get() as u64 + x as u64;
            let range = NonZeroRange::from_span(pos, NonZeroU64::MIN);
            let height = NonZero::new(u32::try_from(y).unwrap()).unwrap();
            let width = ctx.image.image.original.width();
            let single_pixel = std::iter::once(range).with_bounds(width, height);

            ctx.image.masks.clear_ranges(single_pixel);
        }
    }
}
