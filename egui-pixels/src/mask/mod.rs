use std::{collections::BinaryHeap, num::NonZeroU16};

use egui::{
    self, Color32, ColorImage, ImageSource, TextureHandle, TextureOptions, load::SizedTexture,
};
use log::{debug, info};

use crate::{PixelArea, PixelRange};

mod flat_map_inplace;
mod history;

pub(crate) use flat_map_inplace::*;
pub use history::*;

pub struct Annotations(Vec<PixelArea>);

#[cfg_attr(feature = "serde", derive(Debug, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[non_exhaustive]
pub struct MaskSettings {
    default_opacity: u8,
}

impl Default for MaskSettings {
    fn default() -> Self {
        Self {
            default_opacity: 128,
        }
    }
}

pub struct MaskImage {
    size: [usize; 2],
    annotations: Annotations,
    history: History,
    texture_handle: Option<(bool, TextureHandle, ImageSource<'static>)>,
    // Cannot remove handle immediately, as it might be used already previously in this epoch.
    texture_handle_dirty: bool,
    settings: MaskSettings,
    default_opacity_lut: [u8; 256],
}

impl MaskImage {
    pub fn new(size: [usize; 2], annotations: Vec<PixelArea>, history: History) -> Self {
        let settings = MaskSettings::default();
        Self {
            size,
            annotations: Annotations(annotations),
            history,
            texture_handle: None,
            texture_handle_dirty: false,
            default_opacity_lut: Self::build_opacity_lut(settings.default_opacity),
            settings: MaskSettings::default(),
        }
    }

    pub fn set_settings(&mut self, settings: MaskSettings) {
        self.default_opacity_lut = Self::build_opacity_lut(settings.default_opacity);
        self.settings = settings;
    }

    fn build_opacity_lut(opacity: u8) -> [u8; 256] {
        std::array::from_fn(|i| ((i as f32 / 255.0) * (opacity as f32 / 255.0) * 255.0) as u8)
    }

    pub fn random_seed(&self) -> u16 {
        (self.annotations.0.len() as u16).wrapping_add(self.history.random_seed())
    }

    pub fn sources(
        &mut self,
        ctx: &egui::Context,
    ) -> impl Iterator<Item = ImageSource<'static>> + '_ {
        if self.texture_handle.is_none() || self.texture_handle_dirty {
            self.texture_handle_dirty = false;

            let texture_options = TextureOptions {
                magnification: egui::TextureFilter::Nearest,
                ..Default::default()
            };

            let mut pixels = vec![Color32::TRANSPARENT; self.size[0] * self.size[1]];

            for subgroups in self.subgroups().into_iter() {
                let [r, g, b] = subgroups.color;
                for subgroup in subgroups.pixels {
                    let a = self.default_opacity_lut[subgroup.confidence() as usize];
                    let group_color = Color32::from_rgba_premultiplied(r, g, b, a);
                    pixels[subgroup.as_range()].fill(group_color);
                }
            }

            let handle = ctx.load_texture(
                "Overlays",
                ColorImage::new(self.size, pixels),
                texture_options,
            );
            let source = ImageSource::Texture(SizedTexture::from_handle(&handle));

            self.texture_handle = Some((true, handle, source));
        }

        match &self.texture_handle {
            Some((visibility, _, source)) if *visibility => Some(source.clone()).into_iter(),
            _ => None.into_iter(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.history.is_dirty()
    }

    pub fn mark_not_dirty(&mut self) {
        self.history.mark_not_dirty();
    }

    pub fn reset(&mut self) {
        self.history.push(HistoryAction::Reset);
        self.texture_handle_dirty = true;
    }

    pub fn clear_rect(&mut self, [[x_top, y_top], [x_bottom, y_bottom]]: [[usize; 2]; 2]) {
        let x_left = x_top as u32;
        let x_right = x_bottom as u32;
        let x_width = NonZeroU16::try_from((x_right - x_left + 1) as u16).unwrap();

        let y_range = y_top as u32..=y_bottom as u32;
        let image_width = self.size[0] as u32;

        assert!(image_width > 0, "Todo: Move Constraint to MaskImage.size");

        let region = y_range
            .map(|y| PixelRange::new_total(y * image_width + x_left, x_width))
            .collect();

        self.add_history_action(HistoryAction::Clear(region))
    }

    pub fn add_area(&mut self, mut subgroups: PixelArea) {
        crate::remove_overlaps(&mut subgroups, self.subgroups_ordered().map(|(_, g)| g));
        if subgroups.is_empty() {
            debug!("All Pixels are in a other subgroup already");
            return;
        }
        if let Some((visibility @ false, _, _)) = &mut self.texture_handle {
            *visibility = true;
        }
        self.add_history_action(HistoryAction::Add(subgroups))
    }

    pub fn add_history_action(&mut self, action: HistoryAction) {
        self.history.push(action);
        self.texture_handle_dirty = true;
    }

    pub fn handle_events(&mut self, ctx: &egui::Context) {
        let (shift_pressed, cmd_z_pressed, cmd_d_pressed) = ctx.input(|i| {
            (
                i.modifiers.shift,
                i.key_pressed(egui::Key::Z) && i.modifiers.command,
                i.key_pressed(egui::Key::D) && i.modifiers.command,
            )
        });

        if cmd_z_pressed {
            let require_redraw = if shift_pressed {
                info!("Redo");
                self.history.redo().is_some()
            } else {
                info!("Undo");
                self.history.undo().is_some()
            };
            if require_redraw {
                self.texture_handle_dirty = true;
            };
        }
        if let Some((visible, _, _)) = &mut self.texture_handle {
            if cmd_d_pressed {
                *visible = !*visible;
            }
        }
    }

    pub fn subgroups(&self) -> Vec<PixelArea> {
        let base = self.annotations.0.clone();
        self.history.iter().fold(base, |acc, r| r.apply(acc))
    }

    fn subgroups_ordered(&self) -> impl Iterator<Item = (usize, PixelRange)> + '_ {
        struct HeapItem<T>(PixelRange, usize, T);

        impl<T> Eq for HeapItem<T> {}
        impl<T> PartialEq for HeapItem<T> {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl<T> PartialOrd for HeapItem<T> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl<T> Ord for HeapItem<T> {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0.start().cmp(&other.0.start()).reverse()
            }
        }

        struct GroupIterator(BinaryHeap<HeapItem<std::vec::IntoIter<PixelRange>>>);

        let x: BinaryHeap<_> = self
            .subgroups()
            .into_iter()
            .enumerate()
            .map(|(group_id, x)| {
                let mut iter = x.pixels.into_iter();
                HeapItem(
                    iter.next().expect("No empty groups available"),
                    group_id,
                    iter,
                )
            })
            .collect();

        impl Iterator for GroupIterator {
            type Item = (usize, PixelRange);

            fn next(&mut self) -> Option<Self::Item> {
                if let Some(HeapItem(subgroup, group_id, mut rest)) = self.0.pop() {
                    if let Some(x) = rest.next() {
                        self.0.push(HeapItem(x, group_id, rest));
                    }
                    Some((group_id, subgroup))
                } else {
                    None
                }
            }
        }
        GroupIterator(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iter_sorted() {
        let mut history = History::default();
        history.push(HistoryAction::Add(PixelArea::with_black_color(vec![
            PixelRange::new_total(22, NonZeroU16::try_from(7).unwrap()),
            PixelRange::new_total(39, NonZeroU16::try_from(1).unwrap()),
            PixelRange::new_total(42, NonZeroU16::try_from(7).unwrap()),
        ])));
        let x = MaskImage::new(
            [10, 10],
            vec![
                PixelArea::with_black_color(vec![
                    PixelRange::new_total(2, NonZeroU16::try_from(5).unwrap()),
                    PixelRange::new_total(12, NonZeroU16::try_from(5).unwrap()),
                ]),
                PixelArea::with_black_color(vec![PixelRange::new_total(
                    32,
                    NonZeroU16::try_from(5).unwrap(),
                )]),
            ],
            history,
        );
        let group_sequence: Vec<_> = x
            .subgroups_ordered()
            .map(|(group_id, _)| group_id)
            .collect();
        assert_eq!(group_sequence, vec![0, 0, 2, 1, 2, 2]);
    }
}
