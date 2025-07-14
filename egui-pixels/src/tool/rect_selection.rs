use crate::ToolContext;

#[derive(Default)]
pub struct RectSelection {
    last_drag_start: Option<(usize, usize)>,
}

impl RectSelection {
    pub fn drag_stopped(&mut self, ctx: &mut ToolContext) -> Option<[[usize; 2]; 2]> {
        let result = if let (Some((cursor_x, cursor_y)), true) = (
            self.last_drag_start,
            ctx.response.drag_stopped()
                && !ctx.egui.input(|i| i.modifiers.command || i.modifiers.ctrl),
        ) {
            let (start_x, start_y) = ctx.cursor_image_pos;
            self.last_drag_start = None;
            Some([
                [cursor_x.min(start_x), cursor_y.min(start_y)],
                [cursor_x.max(start_x), cursor_y.max(start_y)],
            ])
        } else {
            None
        };

        if ctx.egui.input(|i| !i.pointer.primary_down()) {
            self.last_drag_start = Some(ctx.cursor_image_pos);
        }
        result
    }
}
