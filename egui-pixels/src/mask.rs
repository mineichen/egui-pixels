use std::{
    collections::BinaryHeap,
    iter::FusedIterator,
    ops::{Range, RangeInclusive},
};

use egui::{
    self, Color32, ColorImage, ImageSource, TextureHandle, TextureOptions, load::SizedTexture,
};
use imagemask::SortedRangesMap;
use log::{debug, info};
use range_set_blaze::SortedDisjointMap;

use crate::{Meta, MetaRange, PixelArea};

mod history;
mod merge_sorted;
mod random_color;

use merge_sorted::MergeSortedOverlapping;

pub use history::*;
pub use random_color::random_color_from_seed;

pub struct Annotations(Vec<PixelArea>);
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaskSettings {
    pub default_opacity: u8,
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

    /// Generate the next color based on the current seed
    pub fn next_color(&self) -> [u8; 3] {
        random_color_from_seed(self.random_seed())
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
                for (range, meta) in subgroups.pixels.iter::<Range<usize>>() {
                    let a = self.default_opacity_lut[meta.confidence() as usize];
                    let group_color = Color32::from_rgba_premultiplied(r, g, b, a);
                    pixels[range].fill(group_color);
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

    pub fn clear_ranges(&mut self, ranges: impl Iterator<Item = MetaRange>) {
        let action = HistoryAction::Clear(
            imagemask::SortedRanges::try_from_ordered_iter(ranges.map(|r| r.range.into())).unwrap(),
        );

        self.add_history_action(action)
    }

    pub fn add_area_non_overlapping_parts(&mut self, subgroups: PixelArea) {
        let remaining = subgroups.map_inplace(|x| {
            x.map_and_set_difference(MergeSortedOverlapping::new(
                self.subgroups_ordered()
                    .map(|x| RangeInclusive::<u64>::from(x.1.range)),
            ))
        });
        if let Some(x) = remaining {
            self.add_area_overlapping(x)
        } else {
            debug!("All Pixels are in a other subgroup already");
        }
    }

    pub fn add_area_overlapping(&mut self, subgroups: PixelArea) {
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
        if let Some((visible, _, _)) = &mut self.texture_handle
            && cmd_d_pressed
        {
            *visible = !*visible;
        }
    }

    pub fn subgroups(&self) -> Vec<PixelArea> {
        let base = self.annotations.0.clone();
        self.history.iter().fold(base, |acc, r| r.apply(acc))
    }

    fn subgroups_ordered(&self) -> impl Iterator<Item = (usize, MetaRange)> + FusedIterator + '_ {
        struct HeapItem<T>(MetaRange, usize, T);

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
                self.0.range.start.cmp(&other.0.range.start).reverse()
            }
        }

        struct GroupIterator(
            BinaryHeap<HeapItem<<SortedRangesMap<u32, u32, Vec<Meta>> as IntoIterator>::IntoIter>>,
        );

        let x: BinaryHeap<_> = self
            .subgroups()
            .into_iter()
            .enumerate()
            .filter_map(|(group_id, x)| {
                let mut iter = x.pixels.into_iter();
                Some(HeapItem(iter.next()?, group_id, iter))
            })
            .collect();

        impl Iterator for GroupIterator {
            type Item = (usize, MetaRange);

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
        // self.0.pop() at some point relies on Vec::pop(), which can be called multiple times
        impl FusedIterator for GroupIterator {}
        GroupIterator(x)
    }
}

#[cfg(test)]
mod tests {
    use imagemask::NonZeroRange;

    use crate::{CreateTotal, MetaRange};

    use super::*;
    use std::num::NonZero;

    const NON_ZERO_1: NonZero<u32> = NonZero::<u32>::MIN;
    const NON_ZERO_2: NonZero<u32> = NonZero::new(2).unwrap();
    const NON_ZERO_3: NonZero<u32> = NonZero::new(3).unwrap();
    const NON_ZERO_4: NonZero<u32> = NonZero::new(4).unwrap();
    const NON_ZERO_5: NonZero<u32> = NonZero::new(5).unwrap();
    const NON_ZERO_6: NonZero<u32> = NonZero::new(6).unwrap();
    const NON_ZERO_8: NonZero<u32> = NonZero::new(8).unwrap();

    // Helper function to convert bounds to an iterator of MetaRange for tests
    fn bounds_to_ranges(
        [[x_top, y_top], [x_bottom, y_bottom]]: [[usize; 2]; 2],
        image_width: u32,
    ) -> impl Iterator<Item = MetaRange> {
        let x_left = x_top as u64;
        let x_right = x_bottom as u64;
        let x_width = NonZero::new((x_right - x_left + 1) as u64).unwrap();
        let y_range = y_top as u64..=y_bottom as u64;
        y_range.map(move |y| MetaRange::new_total(y * image_width as u64 + x_left, x_width))
    }

    #[test]
    fn add_area_with_overlap() {
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(1, NON_ZERO_4));
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(2, NON_ZERO_2));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                PixelArea::single_range_total_black(1, NON_ZERO_4),
                PixelArea::single_range_total_black(2, NON_ZERO_2),
            ]
        );
    }

    #[test]
    fn add_area_non_overlapping_parts_remove_completely() {
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        mask_image
            .add_area_non_overlapping_parts(PixelArea::single_range_total_black(1, NON_ZERO_4));
        mask_image
            .add_area_non_overlapping_parts(PixelArea::single_range_total_black(2, NON_ZERO_2));
        assert_eq!(
            mask_image.subgroups(),
            vec![PixelArea::single_range_total_black(1, NON_ZERO_4,),]
        );
    }

    #[test]
    fn add_area_non_overlapping_parts_remove_partially() {
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        mask_image
            .add_area_non_overlapping_parts(PixelArea::single_range_total_black(1, NON_ZERO_4));
        mask_image
            .add_area_non_overlapping_parts(PixelArea::single_range_total_black(2, NON_ZERO_4));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                PixelArea::single_range_total_black(1, NON_ZERO_4,),
                PixelArea::single_range_total_black(5, NON_ZERO_1),
            ]
        );
    }

    #[test]
    fn clear_should_remove_multiple_overlapping_areas_start() {
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(1, NON_ZERO_8));
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(2, NON_ZERO_6));
        mask_image.clear_ranges(bounds_to_ranges([[0, 0], [4, 1]], 10));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                PixelArea::single_range_total_black(5, NON_ZERO_4,),
                PixelArea::single_range_total_black(5, NON_ZERO_3),
            ]
        );
    }

    #[test]
    fn clear_should_remove_multiple_overlapping_areas_end() {
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(1, NON_ZERO_8));
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(2, NON_ZERO_6));
        mask_image.clear_ranges(bounds_to_ranges([[5, 0], [10, 1]], 10));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                PixelArea::single_range_total_black(1, NON_ZERO_4,),
                PixelArea::single_range_total_black(2, NON_ZERO_3,),
            ]
        );
    }

    #[test]
    fn clear_should_remove_multiple_overlapping_areas_within() {
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(1, NON_ZERO_8));
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(2, NON_ZERO_6));
        mask_image.clear_ranges(bounds_to_ranges([[4, 0], [5, 1]], 10));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                PixelArea::with_black_color([
                    MetaRange {
                        range: NonZeroRange::from_span(1, 3.try_into().unwrap()),
                        meta: Default::default()
                    },
                    MetaRange::new_total(6, 3.try_into().unwrap())
                ])
                .unwrap(),
                PixelArea::with_black_color([
                    MetaRange::new_total(2, 2.try_into().unwrap(),),
                    MetaRange::new_total(6, 2.try_into().unwrap(),)
                ])
                .unwrap(),
            ]
        );
    }

    #[test]
    fn clear_should_remove_overlapping_areas_first() {
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(1, NON_ZERO_8));
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(4, NON_ZERO_2));
        mask_image.clear_ranges(bounds_to_ranges([[0, 0], [3, 1]], 10));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                PixelArea::single_range_total_black(4, NON_ZERO_5),
                PixelArea::single_range_total_black(4, NON_ZERO_2,),
            ]
        );
    }

    #[test]
    fn clear_should_remove_overlapping_areas_last() {
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(4, NON_ZERO_2));
        mask_image.add_area_overlapping(PixelArea::single_range_total_black(1, NON_ZERO_8));
        mask_image.clear_ranges(bounds_to_ranges([[0, 0], [3, 1]], 10));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                PixelArea::single_range_total_black(4, NON_ZERO_2,),
                PixelArea::single_range_total_black(4, NON_ZERO_5),
            ]
        );
    }

    #[test]
    fn iter_sorted() {
        let mut history = History::default();
        history.push(HistoryAction::Add(
            PixelArea::with_black_color(vec![
                MetaRange::new_total(22, 7.try_into().unwrap()),
                MetaRange::new_total(39, 1.try_into().unwrap()),
                MetaRange::new_total(42, 7.try_into().unwrap()),
            ])
            .unwrap(),
        ));
        let x = MaskImage::new(
            [10, 10],
            vec![
                PixelArea::with_black_color(vec![
                    MetaRange::new_total(2, 5.try_into().unwrap()),
                    MetaRange::new_total(12, 5.try_into().unwrap()),
                ])
                .unwrap(),
                PixelArea::single_range_total_black(32, NON_ZERO_5),
            ],
            history,
        );
        let group_sequence: Vec<_> = x
            .subgroups_ordered()
            .map(|(group_id, _)| group_id)
            .collect();
        assert_eq!(group_sequence, vec![0, 0, 2, 1, 2, 2]);
    }

    #[test]
    fn add_to_existing_overlapping_doesnt_fail() {
        let mut history = History::default();
        history.push(HistoryAction::Add(
            PixelArea::with_black_color(vec![MetaRange::new_total(0, 2.try_into().unwrap())])
                .unwrap(),
        ));
        history.push(HistoryAction::Add(
            PixelArea::with_black_color(vec![MetaRange::new_total(1, 4.try_into().unwrap())])
                .unwrap(),
        ));
        let mut x = MaskImage::new([10, 10], vec![], history);
        x.add_area_non_overlapping_parts(
            PixelArea::with_black_color(vec![MetaRange::new_total(2, 4.try_into().unwrap())])
                .unwrap(),
        );
        let group_sequence: Vec<_> = x
            .subgroups_ordered()
            .map(|(group_id, _)| group_id)
            .collect();
        assert_eq!(group_sequence, vec![0, 1, 2]);
    }
}
