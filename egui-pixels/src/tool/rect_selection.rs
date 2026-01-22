use egui::Pos2;

use crate::ToolContext;

#[derive(Default)]
pub struct RectSelection {
    /// Image position where drag started (in image pixel coordinates)
    drag_start_image: Option<Pos2>,
}

impl RectSelection {
    pub fn drag_finished(&mut self, ctx: &mut ToolContext) -> Option<[[usize; 2]; 2]> {
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

        // Check if drag stopped (without CTRL, which is for panning)
        let result = if ctx.response.drag_stopped()
            && !ctx.egui.input(|i| i.modifiers.command || i.modifiers.ctrl)
        {
            if let (Some(start_image), Some((end_x, end_y))) =
                (self.drag_start_image, ctx.cursor_image_pos())
            {
                let start_x = start_image.x as usize;
                let start_y = start_image.y as usize;
                self.drag_start_image = None;
                Some([
                    [start_x.min(end_x), start_y.min(end_y)],
                    [start_x.max(end_x), start_y.max(end_y)],
                ])
            } else {
                self.drag_start_image = None;
                None
            }
        } else {
            None
        };

        result
    }
}
