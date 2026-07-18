use std::{collections::BinaryHeap, iter::FusedIterator, num::NonZeroU32, ops::Range};

use egui::{
    self, Color32, ColorImage, ImageSource, TextureHandle, TextureOptions, load::SizedTexture,
};
use imask::{
    CreateRange, ImageDimension, ImaskSet, NonZeroRange, SortedRanges, SortedRangesIter,
    SortedRangesSpanIter, Span,
};
use log::{debug, info};

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
            let size = NonZeroU32::new(self.size[0].try_into().expect("Size can be u32"))
                .expect("Size is not zero");
            for (i, subgroups) in self.applied.stack.iter() {
                let [r, g, b, a] = subgroups.color;
                let is_active = self.active_subgroup == Some(i);
                let opacity = self.settings.opacity(is_active);
                let a = (a as u16 * opacity as u16 / 255) as u8;
                let group_color = Color32::from_rgba_unmultiplied(r, g, b, a);
                for range in subgroups.pixels.iter_global_with::<Range<usize>>(size) {
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

    pub fn active_subgroup(&self) -> Option<usize> {
        self.active_subgroup
    }

    pub fn set_active_subgroup(&mut self, index: Option<usize>) {
        if self.active_subgroup != index {
            self.active_subgroup = index;
            self.applied.mark_redraw(&self.base, &self.history);
        }
    }

    pub fn active_subgroup_at(
        &self,
        (x, y): (usize, usize),
        image_width: NonZeroU32,
    ) -> Option<usize> {
        let width_usize: usize = image_width.get().try_into().ok()?;
        let idx = y * width_usize + x;

        self.subgroups_stack().iter().rev().find_map(|(i, area)| {
            area.pixels
                .iter_global_with::<Range<u64>>(image_width)
                .any(|range| range.contains(&(idx as u64)))
                .then_some(i)
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
    fn subgroups_ordered_spans(
        &self,
    ) -> impl Iterator<Item = (usize, Span<u32>)> + FusedIterator + '_ {
        struct HeapItem<T>(Span<u32>, usize, T);

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
                (self.0.y, self.0.x.start)
                    .cmp(&(other.0.y, other.0.x.start))
                    .reverse()
            }
        }

        struct GroupIterator<'a>(
            BinaryHeap<
                HeapItem<
                    SortedRangesSpanIter<
                        SortedRangesIter<
                            std::iter::Copied<std::slice::Iter<'a, u32>>,
                            std::iter::Copied<std::slice::Iter<'a, u32>>,
                            NonZeroRange<u32>,
                        >,
                    >,
                >,
            >,
        );

        let x: BinaryHeap<_> = self
            .subgroups_stack()
            .iter()
            .filter_map(|(group_id, x)| {
                let mut iter = x.pixels.spans::<u32>();
                Some(HeapItem(iter.next()?, group_id, iter))
            })
            .collect();

        impl<'a> Iterator for GroupIterator<'a> {
            type Item = (usize, Span<u32>);

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
    fn add(self, ranges: SortedRanges<u32, u32>);
    fn clear<I>(self, ranges: I)
    where
        I: Iterator<Item = Span<u32>> + ImageDimension;
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
    fn add(self, subgroups: SortedRanges<u32, u32>) {
        self.keep_overlapping(true).add(subgroups);
    }

    fn clear<I: Iterator<Item = Span<u32>> + ImageDimension>(self, ranges: I) {
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
    fn add(self, subgroups: SortedRanges<u32, u32>) {
        self.keep_overlapping(true).add(subgroups);
    }

    fn clear<I: Iterator<Item = Span<u32>> + ImageDimension>(self, ranges: I) {
        self.mask.add_history_action(HistoryAction {
            kind: HistoryActionKind::Clear(HistoryActionClear {
                ranges: SortedRanges::try_from_span_iter(ranges).unwrap(),
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
    pub fn add(self, subgroups: SortedRanges<u32, u32>) {
        let subgroups = if self.action.overlapping {
            subgroups
        } else {
            let reduced = subgroups.map_span_inplace(|new_spans| {
                // Todo: Workaround, as map_span_inplace only works with u64 at the time of writing this
                let b_spans = self.mask.subgroups_ordered_spans().map(|(_layer, x)| Span {
                    y: x.y.into(),
                    x: NonZeroRange::new_debug_checked_zeroable(x.x.start.into(), x.x.end.into()),
                });
                new_spans.subtract(b_spans)
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
    use imask::{ImaskSet, Rect};

    use super::*;
    use std::num::NonZero;

    const NON_ZERO_1: NonZero<u32> = NonZero::<u32>::MIN;
    const NON_ZERO_2: NonZero<u32> = NonZero::new(2).unwrap();
    const NON_ZERO_4: NonZero<u32> = NonZero::new(4).unwrap();
    const NON_ZERO_5: NonZero<u32> = NonZero::new(5).unwrap();

    const WIDTH_10: NonZero<u32> = NonZero::new(10).unwrap();

    fn mask_10(init: impl Into<PixelAreaStack>) -> MaskImage {
        MaskImage::new([10, 10], init, History::default())
    }

    impl MaskImage {
        fn subgroup_spans_flat(&self) -> impl Iterator<Item = (usize, Span<u32>)> {
            self.subgroups_stack()
                .iter()
                .map(|(i, x)| (i, &x.pixels))
                .flat_map(|(i, x)| x.spans::<u32>().map(move |s| (i, s)))
        }
    }

    #[test]
    fn add_area_with_overlap() {
        let mut mask_image = mask_10(Vec::new());
        mask_image.add(SortedRanges::from(Span::new(1..5, 0)));
        mask_image.add(SortedRanges::from(Span::new(2..4, 0)));
        assert_eq!(
            mask_image.subgroup_spans_flat().collect::<Vec<_>>(),
            vec![(0, Span::new(1..5, 0)), (1, Span::new(2..4, 0)),]
        );
    }

    #[test]
    fn add_area_non_overlapping_parts_remove_completely() {
        let mut mask_image = mask_10(Vec::new());

        let ranges = SortedRanges::from(Span::new(1..5, 0));
        mask_image.keep_overlapping(false).add(ranges);
        let ranges = SortedRanges::from(Span::new(2..4, 0));
        mask_image.keep_overlapping(false).add(ranges);

        assert_eq!(
            mask_image.subgroup_spans_flat().collect::<Vec<_>>(),
            vec![(0, Span::new(1..5, 0)),]
        );
    }
    #[test]
    fn add_area_non_overlapping_parts_remove_partially() {
        let mut mask_image = mask_10(Vec::new());

        let ranges = SortedRanges::from(Span::new(1..5, 0));
        mask_image.keep_overlapping(false).add(ranges);
        let ranges = SortedRanges::from(Span::new(2..6, 0));
        mask_image.keep_overlapping(false).add(ranges);

        assert_eq!(
            mask_image.subgroup_spans_flat().collect::<Vec<_>>(),
            vec![(0, Span::new(1..5, 0)), (1, Span::new(5..6, 0)),]
        );
    }

    #[test]
    fn clear_should_remove_multiple_overlapping_areas_start() {
        let mut mask_image = mask_10(Vec::new());

        let ranges = SortedRanges::from(Span::new(1..9, 0));
        mask_image.add(ranges);
        let ranges = SortedRanges::from(Span::new(2..8, 0));
        mask_image.add(ranges);

        mask_image.clear(Rect::new(0u32, 0, NON_ZERO_4, NON_ZERO_1).into_spans());

        assert_eq!(
            mask_image.subgroup_spans_flat().collect::<Vec<_>>(),
            vec![(0, Span::new(4..9, 0)), (1, Span::new(4..8, 0)),]
        );
    }

    #[test]
    fn clear_should_remove_multiple_overlapping_areas_end() {
        let mut mask_image = mask_10(Vec::new());

        let ranges = SortedRanges::from(Span::new(2..8, 0));
        mask_image.add(ranges);
        let ranges = SortedRanges::from(Span::new(1..9, 0));
        mask_image.add(ranges);

        mask_image.clear(Rect::new(5u32, 0, NON_ZERO_5, NON_ZERO_1).into_spans());

        assert_eq!(
            mask_image.subgroup_spans_flat().collect::<Vec<_>>(),
            vec![(0, Span::new(2..5, 0)), (1, Span::new(1..5, 0)),]
        );
    }

    #[test]
    fn clear_should_remove_multiple_overlapping_areas_within() {
        let mut mask_image = mask_10(Vec::new());

        let ranges = SortedRanges::from(Span::new(1..9, 0));
        mask_image.add(ranges);
        let ranges = SortedRanges::from(Span::new(2..8, 0));
        mask_image.add(ranges);

        mask_image.clear(Rect::new(4u32, 0, NON_ZERO_2, NON_ZERO_1).into_spans());

        assert_eq!(
            mask_image.subgroup_spans_flat().collect::<Vec<_>>(),
            vec![
                (0, Span::new(1..4, 0)),
                (0, Span::new(6..9, 0)),
                (1, Span::new(2..4, 0)),
                (1, Span::new(6..8, 0)),
            ]
        );
    }

    #[test]
    fn iter_sorted() {
        let history = History::default();
        let mut x = MaskImage::new(
            [10, 10],
            vec![
                PixelArea::with_black_color(
                    [Span::new(2..7, 0), Span::new(2..7, 1)].with_bounds(WIDTH_10, WIDTH_10),
                )
                .unwrap(),
                PixelArea {
                    pixels: SortedRanges::from(Span::new(2u32..7, 3)),
                    color: [0, 0, 0, 255],
                },
            ],
            history,
        );
        x.add(
            SortedRanges::try_from_span_iter(
                [
                    Span::new(2u32..9, 2),
                    Span::new(9..10, 3),
                    Span::new(2..9, 4),
                ]
                .with_bounds(WIDTH_10, WIDTH_10),
            )
            .unwrap(),
        );
        let group_sequence: Vec<_> = x
            .subgroups_ordered_spans()
            .map(|(group_id, _)| group_id)
            .collect();
        assert_eq!(group_sequence, vec![0, 0, 2, 1, 2, 2]);
    }

    #[test]
    fn non_overlapping_no_pixel_overlap_with_multiple_layers() {
        let mut mask_image = MaskImage::new([10, 10], vec![], History::default());

        let layer0 = SortedRanges::try_from_span_iter(
            [Span::new(0u32..5, 0), Span::new(2..5, 1)].with_bounds(WIDTH_10, WIDTH_10),
        )
        .unwrap();
        mask_image.add(layer0);

        let spans1 = [Span::new(0u32..5, 3), Span::new(0u32..3, 5)].with_bounds(WIDTH_10, WIDTH_10);
        let layer1 = SortedRanges::try_from_span_iter(spans1).unwrap();
        mask_image.add(layer1);

        let new_mask = SortedRanges::try_from_span_iter(
            [
                Span::new(3u32..8, 0),
                Span::new(8..10, 2),
                Span::new(0..3, 3),
                Span::new(9..10, 4),
                Span::new(0..4, 5),
            ]
            .with_bounds(WIDTH_10, WIDTH_10),
        )
        .unwrap();
        mask_image.keep_overlapping(false).add(new_mask);

        let subgroups = mask_image.subgroups_stack();
        assert_eq!(subgroups.max_layer(), 3);

        let mut subgroups_iter = subgroups.iter();
        let all_existing_pixels: Vec<u64> = (&mut subgroups_iter)
            .take(2)
            .flat_map(|(_i, area)| {
                area.pixels
                    .iter_roi::<std::ops::Range<u64>>()
                    .flat_map(|r| r.start..r.end)
            })
            .collect();

        let new_layer_pixels: Vec<u64> = subgroups_iter
            .next()
            .unwrap()
            .1
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
                pixel_area: SortedRanges::from(Span::new(0u32..2, 0)),
            }),
            layer: None,
            tracked: true,
        });
        history.push(HistoryAction {
            kind: HistoryActionKind::Add(HistoryActionAdd {
                pixel_area: SortedRanges::from(Span::new(1..5, 0)),
            }),
            layer: None,
            tracked: true,
        });
        let mut x = MaskImage::new([10, 10], vec![], history);
        x.keep_overlapping(false)
            .add(SortedRanges::from(Span::new(2..6, 0)));
        let group_sequence: Vec<_> = x
            .subgroups_ordered_spans()
            .map(|(group_id, _)| group_id)
            .collect();
        assert_eq!(group_sequence, vec![0, 1, 2]);
    }
}
