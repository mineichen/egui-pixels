use std::{collections::BinaryHeap, num::NonZeroU16};

use eframe::egui::{self, load::SizedTexture, Color32, ColorImage, TextureHandle, TextureOptions};
use flat_map_inplace::NextInPlaceExt;
use history::{History, HistoryAction};
use itertools::Itertools;
use log::{debug, info};

use crate::Annotation;

mod flat_map_inplace;
mod history;

struct Annotations(Vec<Annotation>);

pub(crate) struct MaskImage {
    size: [usize; 2],
    annotations: Annotations,
    history: History,
    texture_handle: Option<(bool, TextureHandle)>,
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

    pub fn is_dirty(&self) -> bool {
        self.history.is_dirty()
    }

    pub fn mark_not_dirty(&mut self) {
        self.history.mark_not_dirty();
    }

    pub fn reset(&mut self) {
        self.history.push(HistoryAction::Reset);
        self.texture_handle = None;
    }

    pub fn clear_region(&mut self, region: [[usize; 2]; 2]) {
        self.add_history_action(HistoryAction::Clear(
            region,
            (self.size[0] as u32).try_into().unwrap(), // Todo: Move Constraint to MaskImage.size
        ))
    }

    pub fn add_subgroup(&mut self, mut annotation: Annotation) {
        Self::remove_overlaps(
            &mut annotation.1,
            self.subgroups_ordered().map(|(_, a, b)| (a, b)),
        );
        if annotation.1.is_empty() {
            debug!("All Pixels are in a other subgroup already");
            return;
        }
        if let Some((visibility @ false, _)) = &mut self.texture_handle {
            *visibility = true;
        }
        self.add_history_action(HistoryAction::Add(annotation.0, annotation.1))
    }

    pub fn add_history_action(&mut self, action: HistoryAction) {
        self.history.push(action);
        self.texture_handle = None;
    }

    pub fn handle_events(&mut self, ctx: &egui::Context) -> Option<SizedTexture> {
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
                self.texture_handle = None;
            };
        }

        match &mut self.texture_handle {
            Some((visible, handle)) => {
                if cmd_d_pressed {
                    *visible = !*visible;
                }
                visible.then(|| SizedTexture::from_handle(handle))
            }
            None => {
                let texture_options = TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    ..Default::default()
                };

                let mut pixels = vec![Color32::TRANSPARENT; self.size[0] * self.size[1]];

                for (group_id, subgroups) in self.subgroups().into_iter().enumerate() {
                    let [r, g, b] = generate_rgb_color(group_id as u16);
                    let group_color = Color32::from_rgba_premultiplied(r, g, b, 64);
                    for (pos, len) in subgroups {
                        let pos = pos as usize;
                        pixels[pos..(pos + len.get() as usize)].fill(group_color);
                    }
                }

                let handle = ctx.load_texture(
                    "Overlays",
                    ColorImage {
                        size: self.size,
                        pixels,
                    },
                    texture_options,
                );
                let result = Some(SizedTexture::from_handle(&handle));
                self.texture_handle = Some((true, handle));
                result
            }
        }
    }

    pub fn subgroups(&self) -> Vec<Vec<(u32, NonZeroU16)>> {
        let base = self.annotations.0.iter().map(|(_, b)| b.clone()).collect();

        self.history.iter().fold(base, |acc, r| r.apply(acc))
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
                Some(self.cmp(other))
            }
        }
        impl<T> Ord for HeapItem<T> {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0 .0.cmp(&other.0 .0).reverse()
            }
        }

        struct GroupIterator(BinaryHeap<HeapItem<std::vec::IntoIter<(u32, NonZeroU16)>>>);

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
            type Item = (usize, u32, NonZeroU16);

            fn next(&mut self) -> Option<Self::Item> {
                if let Some(HeapItem((pos, len), group_id, mut rest)) = self.0.pop() {
                    if let Some(x) = rest.next() {
                        self.0.push(HeapItem(x, group_id, rest));
                    }
                    Some((group_id, pos, len))
                } else {
                    None
                }
            }
        }
        GroupIterator(x)
    }
    fn remove_overlaps(
        annotations: &mut Vec<(u32, NonZeroU16)>,
        ordered_existing: impl Iterator<Item = (u32, NonZeroU16)>,
    ) {
        let mut peekable_ordered_existing = ordered_existing
            .map(|(pos, len)| (pos, pos + len.get() as u32))
            .peekable();

        annotations.flat_map_inplace(|(mut new_pos, mut new_len), i| {
            let new_end = new_pos + new_len.get() as u32;

            // Overlap start or within new
            for (existing_pos, existing_end) in peekable_ordered_existing
                .peeking_take_while(|(_, existing_end)| new_end > *existing_end)
            {
                if new_pos > existing_end {
                    continue;
                } else if let Ok(len) =
                    NonZeroU16::try_from(existing_pos.saturating_sub(new_pos) as u16)
                {
                    i.insert((new_pos, len));
                }
                if existing_end > new_pos {
                    let offset = existing_end - new_pos;
                    if let Ok(x) = NonZeroU16::try_from(new_len.get().saturating_sub(offset as _)) {
                        new_pos += offset;
                        new_len = x;
                    } else {
                        return;
                    }
                }
            }
            // Overlaps end of new
            if let Some((existing_pos, _)) = peekable_ordered_existing.peek() {
                if let Ok(x) = NonZeroU16::try_from(
                    new_len
                        .get()
                        .saturating_sub(new_end.saturating_sub(*existing_pos) as _),
                ) {
                    new_len = x;
                } else {
                    return;
                }
            }

            i.insert((new_pos, new_len));
        });
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
    fn overlapping_before() {
        let mut annotation = vec![(2, 4.try_into().unwrap())];
        let existing = vec![(0, 3.try_into().unwrap())];
        MaskImage::remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation, vec![(3, 3.try_into().unwrap())])
    }

    #[test]
    fn existing_within_new() {
        let mut annotation = vec![(0, 6.try_into().unwrap())];
        let existing = vec![(1, 4.try_into().unwrap())];

        MaskImage::remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(
            annotation,
            vec![(0, 1.try_into().unwrap()), (5, 1.try_into().unwrap())]
        )
    }

    #[test]
    fn overlapping_both() {
        let mut annotation = vec![(2, 4.try_into().unwrap())];
        let existing = vec![(0, 6.try_into().unwrap())];
        MaskImage::remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation, vec![])
    }

    #[test]
    fn overlapping_twice() {
        let mut annotation = vec![(2, 1.try_into().unwrap()), (4, 1.try_into().unwrap())];
        let existing = vec![(0, 6.try_into().unwrap())];
        MaskImage::remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation, vec![]);
    }

    #[test]
    fn overlapping_end() {
        let mut annotation = vec![(1, 4.try_into().unwrap())];
        let existing = vec![(2, 6.try_into().unwrap())];
        MaskImage::remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation, vec![(1, 1.try_into().unwrap())])
    }

    #[test]
    fn overlapping_between() {
        let mut annotation = vec![(2, 4.try_into().unwrap())];
        let existing = vec![(0, 3.try_into().unwrap()), (0, 8.try_into().unwrap())];
        MaskImage::remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation, vec![])
    }

    #[test]
    fn no_overlap_before() {
        let mut annotation = vec![(2, 4.try_into().unwrap())];
        let existing = vec![(0, 2.try_into().unwrap())];
        MaskImage::remove_overlaps(&mut annotation, existing.into_iter());
        assert_eq!(annotation, vec![(2, 4.try_into().unwrap())])
    }

    #[test]
    fn iter_sorted() {
        let mut history = History::default();
        history.push(HistoryAction::Add(
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
