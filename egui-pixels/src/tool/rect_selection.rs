use std::num::{NonZeroU16, NonZeroU32, NonZeroUsize};

use egui::Pos2;

use crate::{PixelArea, PixelRange, ToolContext};

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
        if max_x > min_x && max_y > min_y {
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

    /// Iterate over pixel ranges for each row in the rectangle
    pub fn iter_ranges(&self, confidence: u8) -> impl Iterator<Item = PixelRange> + '_ {
        (self.min_y..=self.max_y).map(move |y| {
            let start = y as u32 * self.image_width.get() + self.min_x as u32;
            let length = (self.max_x - self.min_x + 1) as u16;
            let length_nonzero = NonZeroU16::new(length)
                .expect("Rectangle width should be non-zero due to validation in new()");
            PixelRange::new(start, length_nonzero, confidence)
        })
    }

    /// Convert to a PixelArea with the given confidence and color
    pub fn into_pixel_area(self, confidence: u8, color: [u8; 3]) -> PixelArea {
        let pixel_ranges: Vec<PixelRange> = self.iter_ranges(confidence).collect();
        PixelArea::new(pixel_ranges, color)
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
        if ctx.response.dragged() {
            if let (Some(start_image), Some(current_screen)) =
                (self.drag_start_image, ctx.response.interact_pointer_pos())
            {
                // Convert stored image coordinates back to current screen coordinates
                let start_screen = ctx.painter.image_to_screen(start_image);
                ctx.painter.draw_dotted_rect(start_screen, current_screen);
            }
        }

        // Check if drag stopped (without CTRL, which is for panning)
        let result = if ctx.response.drag_stopped()
            && !ctx.egui.input(|i| i.modifiers.command || i.modifiers.ctrl)
        {
            if let (Some(start_image), Some((end_x, end_y))) =
                (self.drag_start_image, ctx.cursor_image_pos())
            {
                let start_x = start_image.x as usize;
                let start_y = start_image.y as usize;
                self.drag_start_image = None;

                let min_x = start_x.min(end_x);
                let min_y = start_y.min(end_y);
                let max_x = start_x.max(end_x);
                let max_y = start_y.max(end_y);
                let image_width = ctx.image.image.original.width();

                RectSelectionResult::new(min_x, min_y, max_x, max_y, image_width)
            } else {
                self.drag_start_image = None;
                None
            }
        } else {
            None
        };

        result
    }
}
