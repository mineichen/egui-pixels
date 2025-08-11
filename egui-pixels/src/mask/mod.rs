use std::{collections::BinaryHeap, num::NonZeroU16};

use egui::{
    self, Color32, ColorImage, ImageSource, TextureHandle, TextureOptions, load::SizedTexture,
};
use log::{debug, info};

use crate::{SubGroup, SubGroups};
use history::{History, HistoryAction};

mod flat_map_inplace;
mod history;

pub(crate) use flat_map_inplace::*;

struct Annotations(Vec<SubGroups>);

pub struct MaskImage {
    size: [usize; 2],
    annotations: Annotations,
    history: History,
    texture_handle: Option<(bool, TextureHandle, ImageSource<'static>)>,
    // Cannot remove handle immediately, as it might be used already previously in this epoch.
    texture_handle_dirty: bool,
}

impl MaskImage {
    pub fn new(size: [usize; 2], annotations: Vec<SubGroups>, history: History) -> Self {
        Self {
            size,
            annotations: Annotations(annotations),
            history,
            texture_handle: None,
            texture_handle_dirty: false,
        }
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

            for (group_id, subgroups) in self.subgroups().into_iter().enumerate() {
                let [r, g, b] = generate_rgb_color(group_id as u16);
                let group_color = Color32::from_rgba_premultiplied(r, g, b, 64);
                for subgroup in subgroups {
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
            .map(|y| SubGroup::new_total(y * image_width + x_left, x_width))
            .collect();

        self.add_history_action(HistoryAction::Clear(region))
    }

    pub fn add_subgroups(&mut self, mut subgroups: SubGroups) {
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

    pub fn subgroups(&self) -> Vec<SubGroups> {
        let base = self.annotations.0.clone();
        self.history.iter().fold(base, |acc, r| r.apply(acc))
    }

    fn subgroups_ordered(&self) -> impl Iterator<Item = (usize, SubGroup)> + '_ {
        struct HeapItem<T>(SubGroup, usize, T);

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
                self.0.position.cmp(&other.0.position).reverse()
            }
        }

        struct GroupIterator(BinaryHeap<HeapItem<std::vec::IntoIter<SubGroup>>>);

        let x: BinaryHeap<_> = self
            .subgroups()
            .into_iter()
            .enumerate()
            .map(|(group_id, x)| {
                let mut iter = x.into_iter();
                HeapItem(
                    iter.next().expect("No empty groups available"),
                    group_id,
                    iter,
                )
            })
            .collect();

        impl Iterator for GroupIterator {
            type Item = (usize, SubGroup);

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

fn generate_rgb_color(group: u16) -> [u8; 3] {
    let group = group.wrapping_shl(2).max(2);
    let r = (group.wrapping_mul(17)) as u8;
    let g = (group.wrapping_mul(23)) as u8;
    let b = (group.wrapping_mul(29)) as u8;
    [r, g, b]
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iter_sorted() {
        let mut history = History::default();
        history.push(HistoryAction::Add(vec![
            SubGroup::new_total(22, NonZeroU16::try_from(7).unwrap()),
            SubGroup::new_total(39, NonZeroU16::try_from(1).unwrap()),
            SubGroup::new_total(42, NonZeroU16::try_from(7).unwrap()),
        ]));
        let x = MaskImage::new(
            [10, 10],
            vec![
                vec![
                    SubGroup::new_total(2, NonZeroU16::try_from(5).unwrap()),
                    SubGroup::new_total(12, NonZeroU16::try_from(5).unwrap()),
                ],
                vec![SubGroup::new_total(32, NonZeroU16::try_from(5).unwrap())],
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
