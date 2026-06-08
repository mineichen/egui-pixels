use std::{
    collections::BinaryHeap,
    iter::FusedIterator,
    num::NonZeroU32,
    ops::{Range, RangeInclusive},
};

use egui::{
    self, Color32, ColorImage, ImageSource, TextureHandle, TextureOptions, load::SizedTexture,
};
use imask::{ImageDimension, NonZeroRange, SanitizeSortedDisjoint, SortedRanges, SortedRangesIter};
use log::{debug, info};
use range_set_blaze::SortedDisjoint;

use crate::PixelArea;

mod history;
mod pixel_area_stack;
mod random_color;

pub use history::*;
pub use pixel_area_stack::*;
pub use random_color::random_color_from_seed;

#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaskSettings {
    pub default_opacity: u8,
    pub active_opacity: u8,
}

impl MaskSettings {
    pub fn opacity(&self, active: bool) -> u8 {
        if active {
            self.active_opacity
        } else {
            self.default_opacity
        }
    }
}

impl Default for MaskSettings {
    fn default() -> Self {
        Self {
            default_opacity: 128,
            active_opacity: 200,
        }
    }
}

pub struct MaskImage {
    size: [usize; 2],
    // Often the only reference.
    base: PixelAreaStack,
    applied: AppliedPixelAreaStack,
    history: History,
    texture_handle: Option<LoadedMaskImage>,
    settings: MaskSettings,
    active_subgroup: Option<usize>,
}

struct LoadedMaskImage {
    visible: bool,
    #[allow(dead_code, reason = "Keeps GPU buffer alive")]
    handle: TextureHandle,
    source: ImageSource<'static>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AffectedLayer {
    Unspecified,
    Layer(usize),
}

impl MaskImage {
    pub fn new(size: [usize; 2], base: impl Into<PixelAreaStack>, history: History) -> Self {
        let base = base.into();
        let applied = AppliedPixelAreaStack::new(&base, &history);
        Self {
            size,
            base: base.clone(),
            applied,
            history,
            texture_handle: None,
            settings: MaskSettings::default(),
            active_subgroup: None,
        }
    }

    pub fn set_settings(&mut self, settings: MaskSettings) {
        self.settings = settings;
    }

    pub fn random_seed(&self) -> u16 {
        (self.base.max_layer() as u16).wrapping_add(self.history.random_seed())
    }

    pub fn next_color(&self) -> [u8; 4] {
        random_color_from_seed(self.random_seed())
    }

    pub fn sources(
        &mut self,
        ctx: &egui::Context,
    ) -> impl Iterator<Item = ImageSource<'static>> + '_ {
        if self.texture_handle.is_none() || self.applied.requires_redraw {
            self.applied.requires_redraw = false;

            let texture_options = TextureOptions {
                magnification: egui::TextureFilter::Nearest,
                ..Default::default()
            };

            let mut pixels = vec![Color32::TRANSPARENT; self.size[0] * self.size[1]];

            for (i, subgroups) in self.applied.stack.iter() {
                let [r, g, b, a] = subgroups.color;
                let is_active = self.active_subgroup == Some(i);
                let opacity = self.settings.opacity(is_active);
                let a = (a as u16 * opacity as u16 / 255) as u8;
                let group_color = Color32::from_rgba_premultiplied(r, g, b, a);
                for range in subgroups.pixels.iter_roi::<Range<usize>>() {
                    pixels[range].fill(group_color);
                }
            }

            let handle = ctx.load_texture(
                "Overlays",
                ColorImage::new(self.size, pixels),
                texture_options,
            );
            let source = ImageSource::Texture(SizedTexture::from_handle(&handle));

            self.texture_handle = Some(LoadedMaskImage {
                visible: true,
                handle,
                source,
            });
        }

        match &self.texture_handle {
            Some(x) if x.visible => Some(x.source.clone()).into_iter(),
            _ => None.into_iter(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.history.is_dirty()
    }

    pub fn mark_not_dirty(&mut self) {
        self.history.mark_not_dirty();
    }

    pub fn take_dirty(&mut self) -> Option<AffectedLayer> {
        if self.is_dirty() {
            self.mark_not_dirty();
            Some(match self.history.iter().rev().next()?.layer() {
                Some(x) => AffectedLayer::Layer(x),
                None => AffectedLayer::Unspecified,
            })
        } else {
            None
        }
    }

    pub fn add_history_action(&mut self, action: HistoryAction) {
        if let Some((_, x)) = self.base.iter().next() {
            match &action.kind {
                HistoryActionKind::Add(add) => assert_eq!(
                    add.pixel_area.pixels.bounds().width,
                    x.pixels.bounds().width,
                    "Imanot cannot handle pixel_area with different sizes yet",
                ),
                HistoryActionKind::Reset => {}
                HistoryActionKind::Clear(clear) => assert_eq!(
                    clear.ranges.bounds().width,
                    x.pixels.bounds().width,
                    "Imanot cannot handle pixel_area with different sizes yet",
                ),
            }
        }
        self.history.push(action);
        self.applied.mark_redraw(&self.base, &self.history);
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
                self.applied.mark_redraw(&self.base, &self.history);
            };
        }
        if let Some(x) = &mut self.texture_handle
            && cmd_d_pressed
        {
            x.visible = !x.visible;
        }
    }

    pub fn set_active_subgroup(&mut self, index: Option<usize>) {
        if self.active_subgroup != index {
            self.active_subgroup = index;
            self.applied.mark_redraw(&self.base, &self.history);
        }
    }

    pub fn active_subgroup_at(
        &self,
        cursor_pos: Option<(usize, usize)>,
        image_width: NonZeroU32,
    ) -> Option<usize> {
        let (x, y) = cursor_pos?;
        let width_usize: usize = image_width.get().try_into().ok()?;
        let idx = y * width_usize + x;

        self.subgroups_stack().iter().find_map(|(i, area)| {
            let contains = area
                .pixels
                .iter_roi::<Range<u64>>()
                .any(|range| range.contains(&(idx as u64)));
            if contains { Some(i) } else { None }
        })
    }

    #[deprecated]
    pub fn subgroups(&self) -> Vec<Option<PixelArea>> {
        self.applied.stack.to_option_vec()
    }

    pub fn subgroups_stack(&self) -> &PixelAreaStack {
        &self.applied.stack
    }

    /// Returns the old value
    pub fn set_base_layer(
        &mut self,
        index: usize,
        mut area: Option<PixelArea>,
    ) -> Option<PixelArea> {
        std::mem::swap(&mut area, self.base.make_mut(index));
        self.applied.mark_redraw(&self.base, &self.history);
        area
    }

    fn subgroups_ordered(
        &self,
    ) -> impl Iterator<Item = (usize, NonZeroRange<u64>)> + FusedIterator + '_ {
        struct HeapItem<T>(NonZeroRange<u64>, usize, T);

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
                self.0.start.cmp(&other.0.start).reverse()
            }
        }

        struct GroupIterator<'a>(
            BinaryHeap<
                HeapItem<
                    SortedRangesIter<
                        std::iter::Copied<std::slice::Iter<'a, u32>>,
                        std::iter::Copied<std::slice::Iter<'a, u32>>,
                        NonZeroRange<u64>,
                    >,
                >,
            >,
        );

        let x: BinaryHeap<_> = self
            .subgroups_stack()
            .iter()
            .filter_map(|(group_id, x)| {
                let mut iter = x.pixels.iter_roi::<NonZeroRange<u64>>();
                Some(HeapItem(iter.next()?, group_id, iter))
            })
            .collect();

        impl<'a> Iterator for GroupIterator<'a> {
            type Item = (usize, NonZeroRange<u64>);

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
        impl<'a> FusedIterator for GroupIterator<'a> {}
        GroupIterator(x)
    }
}

struct AppliedPixelAreaStack {
    // Required, because texture musten't be dropped in this frame, as it would cause a use after free
    requires_redraw: bool,
    stack: PixelAreaStack,
}

impl AppliedPixelAreaStack {
    fn new(base: &PixelAreaStack, history: &History) -> Self {
        let base = base.to_option_vec();
        Self {
            requires_redraw: false,
            stack: PixelAreaStack::from_option_vec(
                history.iter().fold(base, |acc, r| r.apply(acc)),
            ),
        }
    }
    fn mark_redraw(&mut self, base: &PixelAreaStack, history: &History) {
        *self = Self::new(base, history);
        self.requires_redraw = true;
    }
}

pub struct AddAction {
    overlapping: bool,
}

pub struct HistoryActionBuilder<'a, A = ()> {
    mask: &'a mut MaskImage,
    layer: Option<usize>,
    tracked: bool,
    action: A,
}

pub trait MaskActionBuilder<'a>: Sized {
    type Builder;
    fn on_layer(self, layer: Option<usize>) -> Self::Builder;
    fn without_tracking(self) -> Self::Builder;
    fn keep_overlapping(self, overlapping: bool) -> HistoryActionBuilder<'a, AddAction>;
}

pub trait MaskDefaultActions: Sized {
    fn add(self, subgroups: PixelArea);
    fn clear<I>(self, ranges: I)
    where
        I: Iterator<Item = NonZeroRange<u64>> + ImageDimension;
    fn reset(self);
}

impl<'a> MaskActionBuilder<'a> for &'a mut MaskImage {
    type Builder = HistoryActionBuilder<'a>;

    fn on_layer(self, layer: Option<usize>) -> HistoryActionBuilder<'a> {
        HistoryActionBuilder {
            mask: self,
            layer,
            tracked: true,
            action: (),
        }
    }

    fn without_tracking(self) -> HistoryActionBuilder<'a> {
        HistoryActionBuilder {
            mask: self,
            layer: None,
            tracked: false,
            action: (),
        }
    }

    fn keep_overlapping(self, overlapping: bool) -> HistoryActionBuilder<'a, AddAction> {
        HistoryActionBuilder {
            mask: self,
            layer: None,
            tracked: true,
            action: AddAction { overlapping },
        }
    }
}

impl<'a, A> MaskActionBuilder<'a> for HistoryActionBuilder<'a, A> {
    type Builder = Self;

    fn on_layer(mut self, layer: Option<usize>) -> Self {
        self.layer = layer;
        self
    }

    fn without_tracking(mut self) -> Self {
        self.tracked = false;
        self
    }

    fn keep_overlapping(self, overlapping: bool) -> HistoryActionBuilder<'a, AddAction> {
        HistoryActionBuilder {
            mask: self.mask,
            layer: self.layer,
            tracked: self.tracked,
            action: AddAction { overlapping },
        }
    }
}

impl<'a> MaskDefaultActions for &'a mut MaskImage {
    fn add(self, subgroups: PixelArea) {
        self.keep_overlapping(true).add(subgroups);
    }

    fn clear<I: Iterator<Item = NonZeroRange<u64>> + ImageDimension>(self, ranges: I) {
        HistoryActionBuilder {
            mask: self,
            layer: None,
            tracked: true,
            action: (),
        }
        .clear(ranges);
    }

    fn reset(self) {
        HistoryActionBuilder {
            mask: self,
            layer: None,
            tracked: true,
            action: (),
        }
        .reset();
    }
}

impl<'a> MaskDefaultActions for HistoryActionBuilder<'a, ()> {
    fn add(self, subgroups: PixelArea) {
        self.keep_overlapping(true).add(subgroups);
    }

    fn clear<I: Iterator<Item = NonZeroRange<u64>> + ImageDimension>(self, ranges: I) {
        self.mask.add_history_action(HistoryAction {
            kind: HistoryActionKind::Clear(HistoryActionClear {
                ranges: SortedRanges::try_from_ordered_iter(ranges).unwrap(),
            }),
            layer: self.layer,
            tracked: self.tracked,
        });
    }

    fn reset(self) {
        self.mask.add_history_action(HistoryAction {
            kind: HistoryActionKind::Reset,
            layer: self.layer,
            tracked: self.tracked,
        });
    }
}

impl<'a> HistoryActionBuilder<'a, AddAction> {
    pub fn add(self, subgroups: PixelArea) {
        let subgroups = if self.action.overlapping {
            subgroups
        } else {
            let reduced = subgroups.map_inplace(|x| {
                x.difference(SanitizeSortedDisjoint::new(
                    self.mask
                        .subgroups_ordered()
                        .map(|x| RangeInclusive::<u64>::from(x.1)),
                ))
            });
            let Some(x) = reduced else {
                debug!("All Pixels are in a other subgroup already");
                return;
            };
            x
        };
        if let Some(x @ LoadedMaskImage { visible: true, .. }) = &mut self.mask.texture_handle {
            x.visible = true;
        }
        self.mask.add_history_action(HistoryAction {
            kind: HistoryActionKind::Add(HistoryActionAdd {
                pixel_area: subgroups,
            }),
            layer: self.layer,
            tracked: self.tracked,
        });
    }
}

#[cfg(test)]
mod tests {
    use imask::{ImaskSet, NonZeroRange};

    use super::*;
    use std::num::{NonZero, NonZeroU32};

    const NON_ZERO_1: NonZero<u32> = NonZero::<u32>::MIN;
    const NON_ZERO_2: NonZero<u32> = NonZero::new(2).unwrap();
    const NON_ZERO_3: NonZero<u32> = NonZero::new(3).unwrap();
    const NON_ZERO_4: NonZero<u32> = NonZero::new(4).unwrap();
    const NON_ZERO_5: NonZero<u32> = NonZero::new(5).unwrap();
    const NON_ZERO_6: NonZero<u32> = NonZero::new(6).unwrap();
    const NON_ZERO_7: NonZero<u32> = NonZero::new(7).unwrap();
    const NON_ZERO_8: NonZero<u32> = NonZero::new(8).unwrap();

    const WIDTH_10: NonZero<u32> = NonZero::new(10).unwrap();

    fn build_mask_10(items: impl IntoIterator<Item = (u32, NonZeroU32)>) -> MaskImage {
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        for (x, len) in items {
            let area = PixelArea::single_range_total_black(x, 0, len, WIDTH_10);
            mask_image.add(area);
        }
        mask_image
    }

    fn bounds_to_ranges(
        [[x_top, y_top], [x_bottom, y_bottom]]: [[usize; 2]; 2],
        image_width: NonZero<u32>,
    ) -> impl Iterator<Item = NonZeroRange<u64>> + ImageDimension {
        let x_left = x_top as u64;
        let x_right = x_bottom as u64;
        let x_width = NonZero::new((x_right - x_left + 1) as u64).unwrap();
        let y_range = y_top as u64..=y_bottom as u64;

        y_range
            .map(move |y| NonZeroRange::from_span(y * image_width.get() as u64 + x_left, x_width))
            .with_bounds(image_width, image_width)
    }

    #[test]
    fn add_area_with_overlap() {
        let mask_image = build_mask_10([(1, NON_ZERO_4), (2, NON_ZERO_2)]);
        assert_eq!(
            mask_image.subgroups(),
            vec![
                Some(PixelArea::single_range_total_black(
                    1, 0, NON_ZERO_4, WIDTH_10
                )),
                Some(PixelArea::single_range_total_black(
                    2, 0, NON_ZERO_2, WIDTH_10
                )),
            ]
        );
    }

    #[test]
    fn add_area_non_overlapping_parts_remove_completely() {
        let items = [(1, NON_ZERO_4), (2, NON_ZERO_2)];
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        for (x, len) in items {
            let area = PixelArea::single_range_total_black(x, 0, len, WIDTH_10);
            mask_image.keep_overlapping(false).add(area);
        }
        assert_eq!(
            mask_image.subgroups(),
            vec![Some(PixelArea::single_range_total_black(
                1, 0, NON_ZERO_4, WIDTH_10
            ))]
        );
    }

    #[test]
    fn add_area_non_overlapping_parts_remove_partially() {
        let items = [(1, NON_ZERO_4), (2, NON_ZERO_4)];
        let history = History::default();
        let mut mask_image = MaskImage::new([10, 10], vec![], history);
        for (x, len) in items {
            let area = PixelArea::single_range_total_black(x, 0, len, WIDTH_10);
            mask_image.keep_overlapping(false).add(area);
        }
        assert_eq!(
            mask_image.subgroups(),
            vec![
                Some(PixelArea::single_range_total_black(
                    1, 0, NON_ZERO_4, WIDTH_10
                )),
                Some(PixelArea::single_range_total_black(
                    5, 0, NON_ZERO_1, WIDTH_10
                )),
            ]
        );
    }

    #[test]
    fn clear_should_remove_multiple_overlapping_areas_start() {
        let mut mask_image = build_mask_10([(1, NON_ZERO_8), (2, NON_ZERO_6)]);
        mask_image.clear(bounds_to_ranges([[0, 0], [4, 1]], WIDTH_10));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                Some(PixelArea::single_range_total_black(
                    5, 0, NON_ZERO_4, WIDTH_10
                )),
                Some(PixelArea::single_range_total_black(
                    5, 0, NON_ZERO_3, WIDTH_10
                )),
            ]
        );
    }

    #[test]
    fn clear_should_remove_multiple_overlapping_areas_end() {
        let mut mask_image = build_mask_10([(1, NON_ZERO_8), (2, NON_ZERO_6)]);
        mask_image.clear(bounds_to_ranges([[5, 0], [10, 1]], WIDTH_10));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                Some(PixelArea::single_range_total_black(
                    1, 0, NON_ZERO_4, WIDTH_10
                )),
                Some(PixelArea::single_range_total_black(
                    2, 0, NON_ZERO_3, WIDTH_10
                )),
            ]
        );
    }

    #[test]
    fn clear_should_remove_multiple_overlapping_areas_within() {
        let mut mask_image = build_mask_10([(1, NON_ZERO_8), (2, NON_ZERO_6)]);
        mask_image.clear(bounds_to_ranges([[4, 0], [5, 1]], WIDTH_10));
        assert_eq!(
            mask_image.subgroups(),
            vec![
                Some(
                    PixelArea::with_black_color(
                        [
                            NonZeroRange::from_span(1, NON_ZERO_3.into()),
                            NonZeroRange::from_span(6, NON_ZERO_3.into())
                        ]
                        .with_bounds(WIDTH_10, NON_ZERO_1)
                    )
                    .unwrap()
                ),
                Some(
                    PixelArea::with_black_color(
                        [
                            NonZeroRange::from_span(2, NON_ZERO_2.into()),
                            NonZeroRange::from_span(6, NON_ZERO_2.into())
                        ]
                        .with_bounds(WIDTH_10, NON_ZERO_1)
                    )
                    .unwrap()
                ),
            ]
        );
    }

    #[test]
    fn clear_should_remove_overlapping_areas_first() {
        let mut mask_image = build_mask_10([(1, NON_ZERO_8), (4, NON_ZERO_2)]);
        mask_image.clear(bounds_to_ranges([[0, 0], [3, 1]], WIDTH_10));
        assert_eq!(
            mask_image
                .subgroups_stack()
                .iter()
                .map(|(_, x)| x.clone())
                .collect::<Vec<_>>(),
            vec![
                PixelArea::single_range_total_black(4, 0, NON_ZERO_5, WIDTH_10),
                PixelArea::single_range_total_black(4, 0, NON_ZERO_2, WIDTH_10),
            ]
        );
    }

    #[test]
    fn clear_should_remove_overlapping_areas_last() {
        let mut mask_image = build_mask_10([(4, NON_ZERO_2), (1, NON_ZERO_8)]);
        mask_image.clear(bounds_to_ranges([[0, 0], [3, 1]], WIDTH_10));
        assert_eq!(
            mask_image
                .subgroups_stack()
                .iter()
                .map(|(_, x)| x.clone())
                .collect::<Vec<_>>(),
            vec![
                PixelArea::single_range_total_black(4, 0, NON_ZERO_2, WIDTH_10),
                PixelArea::single_range_total_black(4, 0, NON_ZERO_5, WIDTH_10),
            ]
        );
    }

    #[test]
    fn iter_sorted() {
        let mut history = History::default();
        history.push(HistoryAction {
            kind: HistoryActionKind::Add(HistoryActionAdd {
                pixel_area: PixelArea::with_black_color(
                    [
                        NonZeroRange::from_span(22, NON_ZERO_7.into()),
                        NonZeroRange::from_span(39, NON_ZERO_1.into()),
                        NonZeroRange::from_span(42, NON_ZERO_7.into()),
                    ]
                    .with_bounds(WIDTH_10, WIDTH_10),
                )
                .unwrap(),
            }),
            layer: None,
            tracked: true,
        });
        let x = MaskImage::new(
            [10, 10],
            vec![
                PixelArea::with_black_color(
                    [
                        NonZeroRange::from_span(2, NON_ZERO_5.into()),
                        NonZeroRange::from_span(12, NON_ZERO_5.into()),
                    ]
                    .with_bounds(WIDTH_10, WIDTH_10),
                )
                .unwrap(),
                PixelArea::single_range_total_black(32, 0, NON_ZERO_5, WIDTH_10),
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
    fn non_overlapping_no_pixel_overlap_with_multiple_layers() {
        let mut mask_image = MaskImage::new([10, 10], vec![], History::default());

        let layer0 = PixelArea::with_black_color(
            [
                NonZeroRange::from_span(0, NON_ZERO_5.into()),
                NonZeroRange::from_span(12, NON_ZERO_5.into()),
            ]
            .with_bounds(WIDTH_10, WIDTH_10),
        )
        .unwrap();
        mask_image.add(layer0);

        let layer1 = PixelArea::with_black_color(
            [
                NonZeroRange::from_span(30, NON_ZERO_5.into()),
                NonZeroRange::from_span(50, NON_ZERO_3.into()),
            ]
            .with_bounds(WIDTH_10, WIDTH_10),
        )
        .unwrap();
        mask_image.add(layer1);

        let new_mask = PixelArea::with_black_color(
            [
                NonZeroRange::from_span(3, NON_ZERO_5.into()),
                NonZeroRange::from_span(28, NON_ZERO_5.into()),
                NonZeroRange::from_span(49, NON_ZERO_5.into()),
            ]
            .with_bounds(WIDTH_10, WIDTH_10),
        )
        .unwrap();
        mask_image.keep_overlapping(false).add(new_mask);

        let subgroups = mask_image.subgroups();
        assert_eq!(subgroups.len(), 3);

        let all_existing_pixels: Vec<u64> = subgroups[..2]
            .iter()
            .flatten()
            .flat_map(|area| {
                area.pixels
                    .iter_roi::<std::ops::Range<u64>>()
                    .flat_map(|r| r.start..r.end)
            })
            .collect();

        let new_layer_pixels: Vec<u64> = subgroups[2]
            .as_ref()
            .unwrap()
            .pixels
            .iter_roi::<std::ops::Range<u64>>()
            .flat_map(|r| r.start..r.end)
            .collect();

        for pixel in &new_layer_pixels {
            assert!(
                !all_existing_pixels.contains(pixel),
                "Pixel {pixel} exists in both new layer and existing layers"
            );
        }
    }

    #[test]
    fn add_to_existing_overlapping_doesnt_fail() {
        let mut history = History::default();
        history.push(HistoryAction {
            kind: HistoryActionKind::Add(HistoryActionAdd {
                pixel_area: PixelArea::with_black_color(
                    [NonZeroRange::from_span(0, NON_ZERO_2.into())].with_bounds(WIDTH_10, WIDTH_10),
                )
                .unwrap(),
            }),
            layer: None,
            tracked: true,
        });
        history.push(HistoryAction {
            kind: HistoryActionKind::Add(HistoryActionAdd {
                pixel_area: PixelArea::with_black_color(
                    [NonZeroRange::from_span(1, NON_ZERO_4.into())].with_bounds(WIDTH_10, WIDTH_10),
                )
                .unwrap(),
            }),
            layer: None,
            tracked: true,
        });
        let mut x = MaskImage::new([10, 10], vec![], history);
        x.keep_overlapping(false).add(
            PixelArea::with_black_color(
                [NonZeroRange::from_span(2, NON_ZERO_4.into())].with_bounds(WIDTH_10, WIDTH_10),
            )
            .unwrap(),
        );
        let group_sequence: Vec<_> = x
            .subgroups_ordered()
            .map(|(group_id, _)| group_id)
            .collect();
        assert_eq!(group_sequence, vec![0, 1, 2]);
    }
}
