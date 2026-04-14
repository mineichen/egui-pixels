use std::num::{NonZero, NonZeroU32, NonZeroUsize};

use egui::Pos2;
use imask::{ImageDimension, ImaskSet, NonZeroRange, RectIterator};

use crate::{Meta, MetaRange, PixelArea, ToolContext};

/// Result of a rectangular selection with guaranteed non-zero dimensions
pub struct RectSelectionResult {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
    image_width: NonZeroU32,
}

impl RectSelectionResult {
    /// Create a new RectSelectionResult. Returns None if width or height would be zero.
    /// This ensures max_x >= min_x and max_y >= min_y, guaranteeing non-zero dimensions.
    pub fn new(
        min_x: usize,
        min_y: usize,
        max_x: usize,
        max_y: usize,
        image_width: NonZeroU32,
    ) -> Option<Self> {
        if max_x > min_x
            && max_y > min_y
            && max_x < usize::try_from(image_width.get()).expect("Width is < usize::MAX")
        {
            Some(Self {
                min_x,
                min_y,
                max_x,
                max_y,
                image_width,
            })
        } else {
            None
        }
    }

    /// Get the width of the rectangle (guaranteed non-zero)
    pub fn width(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.max_x - self.min_x + 1)
            .expect("Width should always be non-zero due to validation in new()")
    }

    /// Get the height of the rectangle (guaranteed non-zero)
    pub fn height(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.max_y - self.min_y + 1)
            .expect("Height should always be non-zero due to validation in new()")
    }

    /// Get the bounds as [[min_x, min_y], [max_x, max_y]]
    pub fn bounds(&self) -> [[usize; 2]; 2] {
        [[self.min_x, self.min_y], [self.max_x, self.max_y]]
    }
    pub fn iter_ranges(&self) -> RectIterator<NonZeroRange<u64>> {
        let width = NonZero::new((self.max_x - self.min_x) as u64 + 1).unwrap();
        let height = NonZero::new((self.max_y - self.min_y) as u64 + 1).unwrap();
        let rect = imask::Rect::<u64>::new(
            self.min_x.try_into().unwrap(),
            self.min_y.try_into().unwrap(),
            width,
            height,
        );
        rect.into_rect_iter(self.image_width.into())
    }

    /// Iterate over pixel ranges for each row in the rectangle
    pub fn iter_ranges_meta(
        &self,
        meta: Meta,
    ) -> impl Iterator<Item = MetaRange> + ImageDimension + '_ {
        let iter = self.iter_ranges();
        let bounds = iter.bounds();
        iter.map(move |range| MetaRange { range, meta })
            .with_roi(bounds)
    }

    /// Convert to a PixelArea with the given meta and color
    pub fn into_pixel_area(self, meta: Meta, color: [u8; 3]) -> Option<PixelArea> {
        PixelArea::new(self.iter_ranges_meta(meta), color)
    }
}

#[derive(Default)]
pub struct RectSelection {
    /// Image position where drag started (in image pixel coordinates)
    /// This is necessary to allow paning during the selection
    drag_start_image: Option<Pos2>,
}

impl RectSelection {
    pub fn drag_finished(&mut self, ctx: &mut ToolContext) -> Option<RectSelectionResult> {
        // Track drag start position in image coordinates
        if ctx.response.drag_started() {
            let drag_delta = ctx.response.drag_delta();
            self.drag_start_image = ctx
                .response
                .interact_pointer_pos()
                .map(|screen_pos| ctx.painter.screen_to_image(screen_pos - drag_delta));
        }

        // Draw dotted rectangle while dragging
        if ctx.response.dragged()
            && let (Some(start_image), Some(current_screen)) =
                (self.drag_start_image, ctx.response.interact_pointer_pos())
        {
            // Convert stored image coordinates back to current screen coordinates
            let start_screen = ctx.painter.image_to_screen(start_image);
            ctx.painter.draw_dotted_rect(start_screen, current_screen);
        }

        // Check if drag stopped (without CTRL, which is for panning)
        if ctx.response.drag_stopped()
            && !ctx.egui.input(|i| i.modifiers.command || i.modifiers.ctrl)
        {
            if let (Some(start_image), Some((end_x, end_y))) =
                (self.drag_start_image, ctx.cursor_image_pos())
            {
                let start_x = start_image.x as usize;
                let start_y = start_image.y as usize;
                self.drag_start_image = None;

                let image_width = ctx.image.image.original.width();
                let min_x = start_x.min(end_x);
                let min_y = start_y.min(end_y);
                let max_x = start_x
                    .max(end_x)
                    .min(usize::try_from(image_width.get()).expect("Width < usize::MAX") - 1);
                let max_y = start_y.max(end_y);

                RectSelectionResult::new(min_x, min_y, max_x, max_y, image_width)
            } else {
                self.drag_start_image = None;
                None
            }
        } else {
            None
        }
    }
}
