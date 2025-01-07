use std::num::NonZeroU16;

use eframe::egui::{
    self, load::SizedTexture, Color32, ColorImage, TextureHandle, TextureOptions, Ui,
};
use history::History;
use log::info;

use crate::Annotation;

mod history;

struct Annotations(Vec<Annotation>);

pub(crate) struct MaskImage {
    size: [usize; 2],
    annotations: Annotations,
    history: History,
    texture_handle: Option<TextureHandle>,
}

impl MaskImage {
    pub fn new(size: [usize; 2], annotations: Vec<crate::Annotation>, history: History) -> Self {
        Self {
            size,
            annotations: Annotations(annotations),
            history,
            texture_handle: None,
        }
    }

    pub fn add_subgroup(&mut self, annotation: Annotation) {
        self.history.push(annotation);
        self.texture_handle = None;
    }

    pub fn ui_events(&mut self, ui: &mut Ui) -> Option<SizedTexture> {
        if let ((shift_pressed, true),) = (ui.input(|i| {
            (
                i.modifiers.shift,
                i.key_pressed(egui::Key::Z) && i.modifiers.command,
            )
        }),)
        {
            let require_redraw = if shift_pressed {
                info!("Redo");
                self.history.redo().is_some()
            } else {
                info!("Undo");
                self.history.undo().is_some()
            };
            if require_redraw {
                self.texture_handle = None;
            };
        }

        if self.texture_handle.is_none() {
            let texture_options = TextureOptions {
                magnification: egui::TextureFilter::Nearest,
                ..Default::default()
            };

            let mut pixels = vec![Color32::TRANSPARENT; self.size[0] * self.size[1]];

            for (pos, len) in self.subgroups() {
                let group_color = Color32::from_rgba_premultiplied(64, 64, 0, 64);
                let pos = pos as usize;
                pixels[pos..(pos + len.get() as usize)].fill(group_color);
            }

            let handle = ui.ctx().load_texture(
                "Overlays",
                ColorImage {
                    size: self.size,
                    pixels,
                },
                texture_options,
            );
            let result = Some(SizedTexture::from_handle(&handle));
            self.texture_handle = Some(handle);
            result
        } else {
            None
        }
    }
    fn subgroups(&self) -> impl Iterator<Item = (u32, NonZeroU16)> + '_ {
        self.annotations
            .0
            .iter()
            .flat_map(|(_, b)| b)
            .chain(self.history.iter().flatten())
            .copied()
    }
}
