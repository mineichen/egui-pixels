mod clear;
mod rect_selection;

pub use clear::*;
pub use rect_selection::*;

use crate::ImageStateLoaded;

pub trait Tool {
    fn handle_interaction(&mut self, ctx: ToolContext);
}

#[non_exhaustive]
pub struct ToolContext<'a> {
    pub image: &'a mut ImageStateLoaded,
    pub response: egui::Response,
    pub cursor_image_pos: (usize, usize),
    pub egui: &'a egui::Context,
}

impl<'a> ToolContext<'a> {
    pub fn new(
        image: &'a mut ImageStateLoaded,
        response: egui::Response,
        cursor_image_pos: (usize, usize),
        egui: &'a egui::Context,
    ) -> Self {
        Self {
            image,
            response,
            cursor_image_pos,
            egui,
        }
    }
}
