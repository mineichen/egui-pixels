use std::num::NonZeroU16;

use crate::{PixelArea, PixelRange, RectSelection, Tool, ToolContext};

#[derive(Default)]
#[non_exhaustive]
pub struct RectTool {
    rect_selection: RectSelection,
}

impl Tool for RectTool {
    fn handle_interaction(&mut self, mut ctx: ToolContext) {
        let selection = self.rect_selection.drag_finished(&mut ctx);
        let color = ctx.image.masks.next_color();
        if let Some(rect_result) = selection {
            let pixel_area = rect_result.into_pixel_area(255, color);
            ctx.image.masks.add_area_non_overlapping_parts(pixel_area);
        } else if ctx.response.clicked() {
            if let Some((x, y)) = ctx.cursor_image_pos() {
                let width = ctx.image.image.original.width().get() as usize;
                let start = (y * width + x) as u32;
                let length = NonZeroU16::MIN;
                let pixel_area = PixelArea::new(vec![PixelRange::new(start, length, 255)], color);
                ctx.image.masks.add_area_non_overlapping_parts(pixel_area);
            }
        }
    }
}
