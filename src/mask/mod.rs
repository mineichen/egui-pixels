use std::{collections::BinaryHeap, num::NonZeroU16};

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

            for (group_id, subgroups) in self.subgroups() {
                let [r, g, b] = generate_rgb_color(group_id as u16);
                let group_color = Color32::from_rgba_premultiplied(r, g, b, 64);
                for &(pos, len) in subgroups {
                    let pos = pos as usize;
                    pixels[pos..(pos + len.get() as usize)].fill(group_color);
                }
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

    fn subgroups(&self) -> impl Iterator<Item = (usize, &Vec<(u32, NonZeroU16)>)> {
        self.annotations
            .0
            .iter()
            .map(|(_, b)| b)
            .chain(self.history.iter())
            .enumerate()
    }

    fn subgroups_ordered(&self) -> impl Iterator<Item = (usize, u32, NonZeroU16)> + '_ {
        struct HeapItem<T>((u32, NonZeroU16), usize, T);

        impl<T> Eq for HeapItem<T> {}
        impl<T> PartialEq for HeapItem<T> {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl<T> PartialOrd for HeapItem<T> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                self.0 .0.partial_cmp(&other.0 .0).map(|x| x.reverse())
            }
        }
        impl<T> Ord for HeapItem<T> {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0 .0.cmp(&other.0 .0).reverse()
            }
        }

        struct GroupIterator<'a>(BinaryHeap<HeapItem<std::slice::Iter<'a, (u32, NonZeroU16)>>>);

        let x: BinaryHeap<_> = self
            .subgroups()
            .map(|(group_id, x)| {
                let mut iter = x.iter();
                HeapItem(
                    *iter.next().expect("No empty groups available"),
                    group_id,
                    iter,
                )
            })
            .collect();

        impl<'a> Iterator for GroupIterator<'a> {
            type Item = (usize, u32, NonZeroU16);

            fn next(&mut self) -> Option<Self::Item> {
                if let Some(HeapItem((pos, len), group_id, mut rest)) = self.0.pop() {
                    if let Some(x) = rest.next() {
                        self.0.push(HeapItem(*x, group_id, rest));
                    }
                    Some((group_id, pos, len))
                } else {
                    None
                }
            }
        }
        GroupIterator(x)
    }
}

fn generate_rgb_color(group: u16) -> [u8; 3] {
    let group = group.wrapping_shl(2);
    let r = ((group.wrapping_mul(17)) as u8).max(50);
    let g = ((group.wrapping_mul(23)) as u8).max(50);
    let b = ((group.wrapping_mul(29)) as u8).max(50);
    [r, g, b]
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iter_sorted() {
        let mut history = History::default();
        history.push((
            "Foo".into(),
            vec![
                (22, NonZeroU16::try_from(7).unwrap()),
                (39, NonZeroU16::try_from(1).unwrap()),
                (42, NonZeroU16::try_from(7).unwrap()),
            ],
        ));
        let x = MaskImage {
            size: [10, 10],
            annotations: Annotations(vec![
                (
                    "Test".into(),
                    vec![
                        (2, NonZeroU16::try_from(5).unwrap()),
                        (12, NonZeroU16::try_from(5).unwrap()),
                    ],
                ),
                ("Test2".into(), vec![(32, NonZeroU16::try_from(5).unwrap())]),
            ]),
            history,
            texture_handle: None,
        };
        let group_sequence: Vec<_> = x
            .subgroups_ordered()
            .map(|(group_id, _, _)| group_id)
            .collect();
        assert_eq!(group_sequence, vec![0, 0, 2, 1, 2, 2]);
    }
}
