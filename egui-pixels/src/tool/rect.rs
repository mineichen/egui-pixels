use std::num::NonZeroU16;

use futures::FutureExt;

use crate::{CursorImage, PixelArea, PixelRange, RectSelection, Tool, ToolContext, ToolFactory};

// https://www.svgrepo.com/svg/437030/lasso
const RECT_CURSOR_IMAGE: CursorImage = CursorImage {
    bytes: "iVBORw0KGgoAAAANSUhEUgAAABoAAAAaCAYAAACpSkzOAAABhWlDQ1BJQ0MgcHJvZmlsZQAAKJF9kT1Iw0AcxV9bpUWqDnZQcchQneyiooJLqWIRLJS2QqsOJpd+QZOGJMXFUXAtOPixWHVwcdbVwVUQBD9A3AUnRRcp8X9JoUWMB8f9eHfvcfcO8DYqTDG6ooCimnoqHhOyuVXB/4oAhtCHGcyJzNAS6cUMXMfXPTx8vYvwLPdzf45eOW8wwCMQR5mmm8QbxNObpsZ5nzjESqJMfE48rtMFiR+5Ljn8xrlos5dnhvRMap44RCwUO1jqYFbSFeIp4rCsqJTvzTosc97irFRqrHVP/sJgXl1Jc53mCOJYQgJJCJBQQxkVmIjQqpJiIEX7MRf/sO1PkksiVxmMHAuoQoFo+8H/4He3RmFywkkKxoDuF8v6GAX8u0Czblnfx5bVPAF8z8CV2vZXG8DsJ+n1thY+Avq3gYvrtibtAZc7wOCTJuqiLfloegsF4P2MvikHDNwCPWtOb619nD4AGepq+QY4OATGipS97vLuQGdv/55p9fcDUmtzAIjlR5QAAAAGYktHRAAAAAAAAPlDu38AAAAJcEhZcwAADdcAAA3XAUIom3gAAAAHdElNRQfpCBkPAB2IpJjaAAABr0lEQVRIx+3WPUiVYRQH8F8ZDYFGViDUZJlLNCW4REtJRBASZGtTe5CLLg1BH9DY0ORtdmhoKkSyxCLRagiKHHKIiEtGF0HNbi0neLB73/vxXofAAy/n5fA/n895/8/LljQp2+rEdeMkjqMLHSjhK+bwFO/zFHIWUyjjd43nJS422lEHxjCY2N5hBp/wA+04iH4cS2JN4BKKtbrYjTdRZRkFHK3hcwT38DP8PsaIM2U8wEsYaHDU/fgS/pNZO9AXoF841eS59mE14pypBrobgEc5t3ks4hT+GrZvAPSEfpYz0VRydhUTrYfemzNRe+jlaolehT6PnTlIYCje56uBDmAl5juLb/iOJzhRZ6JryUL1ZAFHq3z5azid4bcLdxL89VoV3QzgAs5FJ4/D9qLCmHoxgsUkyf0Kx/KPFAL8ILFdCdt6sMZzvI3Rpl0XA1uXDCWOs8HMaxlkWsZrXMWeRkn1BoaxI7EVg507Y/1LQTfzsTBNywCmk8onWnkR7sMtfN4wnlIQZkvkUIUEH4L+D7eym4cRfBGXsX+z/h8WItGFzQjelrx3Rhe346r+P+UPJi6EyWu6XtcAAAAASUVORK5CYII=",
    offset_x: 10,
    offset_y: 10,
};

#[derive(Default)]
#[non_exhaustive]
pub struct RectTool {
    rect_selection: RectSelection,
}

impl RectTool {
    pub fn create_factory() -> ToolFactory {
        Box::new(|_| async { Ok(Box::new(RectTool::default()) as Box<dyn Tool + Send>) }.boxed())
    }
}

impl Tool for RectTool {
    fn handle_interaction(&mut self, mut ctx: ToolContext) {
        ctx.cursor_image.set(RECT_CURSOR_IMAGE);

        let selection = self.rect_selection.drag_finished(&mut ctx);
        let color = ctx.image.masks.next_color();
        if let Some(rect_result) = selection {
            let color = ctx.image.masks.next_color();
            let pixel_area = rect_result.into_pixel_area(255, color);
            ctx.image.masks.add_area_non_overlapping_parts(pixel_area);
        } else if ctx.response.clicked()
            && let Some((x, y)) = ctx.cursor_image_pos()
        {
            let color = ctx.image.masks.next_color();
            let width = ctx.image.image.original.width().get() as usize;
            let start = (y * width + x) as u32;
            let length = NonZeroU16::MIN;
            let pixel_area = PixelArea::new(vec![PixelRange::new(start, length, 255)], color);
            ctx.image.masks.add_area_non_overlapping_parts(pixel_area);
        }
    }
}
