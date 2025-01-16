use eframe::egui;

use crate::inference::SamSession;

mod clear;
mod sam;

pub(super) struct Tools {
    last_drag_start: Option<(usize, usize)>,
    active_tool: ToolVariant,
    pub session: SamSession,
}

#[derive(Default, Debug, PartialEq, Eq)]
enum ToolVariant {
    #[default]
    Sam,
    Clear,
}

impl Tools {
    pub(super) fn new(session: SamSession) -> Self {
        Self {
            last_drag_start: None,
            active_tool: ToolVariant::default(),
            session,
        }
    }

    fn drag_stopped(
        &mut self,
        cursor_image_pos: Option<(usize, usize)>,
        response: &egui::Response,
        ctx: &egui::Context,
    ) -> Option<[[usize; 2]; 2]> {
        if let (Some((start_x, start_y)), Some((cursor_x, cursor_y)), true) = (
            cursor_image_pos,
            self.last_drag_start,
            response.drag_stopped() && !ctx.input(|i| i.modifiers.command || i.modifiers.ctrl),
        ) {
            self.last_drag_start = None;
            Some([
                [cursor_x.min(start_x), cursor_y.min(start_y)],
                [cursor_x.max(start_x), cursor_y.max(start_y)],
            ])
        } else {
            None
        }
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_label("Tool")
            .selected_text(format!("{:?}", self.active_tool))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.active_tool, ToolVariant::Sam, "Sam");
                ui.selectable_value(&mut self.active_tool, ToolVariant::Clear, "Clear");
            });
    }
}

impl super::ImageViewerApp {
    pub(super) fn handle_interaction(
        &mut self,
        response: egui::Response,
        cursor_image_pos: Option<(usize, usize)>,
        ctx: &egui::Context,
    ) {
        if response.drag_started() {
            self.tools.last_drag_start = cursor_image_pos;
        }
        match self.tools.active_tool {
            ToolVariant::Sam => self.handle_sam_interaction(response, cursor_image_pos, ctx),
            ToolVariant::Clear => self.handle_clear_interaction(response, cursor_image_pos, ctx),
        }
    }
}
