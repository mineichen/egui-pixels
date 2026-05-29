use std::num::{NonZero, NonZeroU16};

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use futures::FutureExt;
use imask::{ImaskSet, NonZeroRange, Rect};

use crate::{ImagePainter, MaskActionBuilder, PixelArea, Tool, ToolContext, ToolFactory};

struct StrokeState {
    width: usize,
    height: usize,
    mask: Vec<bool>,
    last_pos: Option<(usize, usize)>,
    texture: Option<TextureHandle>,
    dirty: Option<Rect<usize>>,
}

impl StrokeState {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            mask: vec![false; width * height],
            last_pos: None,
            texture: None,
            dirty: None,
        }
    }

    fn stamp_square(&mut self, cx: usize, cy: usize, half_size: usize) {
        let x_min = cx.saturating_sub(half_size);
        let x_max = (cx + half_size).min(self.width - 1);
        let y_min = cy.saturating_sub(half_size);
        let y_max = (cy + half_size).min(self.height - 1);

        for py in y_min..=y_max {
            let row = py * self.width;
            for px in x_min..=x_max {
                unsafe {
                    *self.mask.get_unchecked_mut(row + px) = true;
                }
            }
        }

        let stamp_rect = Rect::new(
            x_min,
            y_min,
            NonZero::new(x_max - x_min + 1).unwrap(),
            NonZero::new(y_max - y_min + 1).unwrap(),
        );
        self.dirty = Some(match self.dirty {
            Some(d) => d.bounds(&stamp_rect),
            None => stamp_rect,
        });
    }

    fn stamp_line(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, half_size: usize) {
        let dx = x1 as f64 - x0 as f64;
        let dy = y1 as f64 - y0 as f64;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 1.0 {
            return;
        }
        let step = (half_size as f64).max(1.0);
        let steps = (dist / step).ceil() as usize;

        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = ((x0 as f64 + dx * t).round() as usize).min(self.width - 1);
            let y = ((y0 as f64 + dy * t).round() as usize).min(self.height - 1);
            self.stamp_square(x, y, half_size);
        }
    }

    fn stamp_to(&mut self, x: usize, y: usize, half_size: usize) {
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);
        if let Some((lx, ly)) = self.last_pos {
            self.stamp_line(lx, ly, x, y, half_size);
        } else {
            self.stamp_square(x, y, half_size);
        }
        self.last_pos = Some((x, y));
    }

    fn update_texture(&mut self, ctx: &egui::Context, color: [u8; 4]) {
        let Some(dirty) = self.dirty.take() else {
            return;
        };

        if self.texture.is_none() {
            let pixels = vec![Color32::TRANSPARENT; self.width * self.height];
            let image = ColorImage::new([self.width, self.height], pixels);
            self.texture = Some(ctx.load_texture("brush_preview", image, TextureOptions::NEAREST));
        }

        let handle = self.texture.as_mut().unwrap();
        let dx = dirty.x;
        let dy = dirty.y;
        let dw = dirty.width.get();
        let dh = dirty.height.get();
        let mask = &self.mask;
        let width = self.width;
        let pixels = (dy..dy + dh)
            .flat_map(move |py| {
                (dx..dx + dw).map(move |px| {
                    if mask[py * width + px] {
                        Color32::from_rgba_premultiplied(color[0], color[1], color[2], color[3])
                    } else {
                        Color32::TRANSPARENT
                    }
                })
            })
            .collect();
        let image = ColorImage::new([dw, dh], pixels);
        handle.set_partial([dx, dy], image, TextureOptions::NEAREST);
    }

    fn render(&self, painter: &mut ImagePainter) {
        if let Some(handle) = &self.texture {
            let uv = egui::Rect::from_min_max(egui::Pos2::new(0.0, 0.0), egui::Pos2::new(1.0, 1.0));
            painter
                .painter()
                .image(handle.id(), painter.image_rect(), uv, egui::Color32::WHITE);
        }
    }

    fn into_pixel_area(
        self,
        color: [u8; 4],
        image_width: NonZero<u32>,
        image_height: NonZero<u32>,
    ) -> Option<PixelArea> {
        let w = self.width as u64;
        let mut ranges: Vec<NonZeroRange<u64>> = Vec::new();

        for y in 0..self.height {
            let mut x = 0;
            while x < self.width {
                if self.mask[y * self.width + x] {
                    let start_x = x;
                    while x < self.width && self.mask[y * self.width + x] {
                        x += 1;
                    }
                    let span = NonZero::new((x - start_x) as u64).unwrap();
                    ranges.push(NonZeroRange::from_span(y as u64 * w + start_x as u64, span));
                } else {
                    x += 1;
                }
            }
        }

        PixelArea::new(
            ranges.into_iter().with_bounds(image_width, image_height),
            color,
        )
    }
}

fn draw_brush_outline(
    painter: &ImagePainter,
    ix: usize,
    iy: usize,
    half_size: usize,
    width: usize,
    height: usize,
) {
    let x_min = ix.saturating_sub(half_size);
    let x_max = (ix + half_size).min(width - 1);
    let y_min = iy.saturating_sub(half_size);
    let y_max = (iy + half_size).min(height - 1);

    let top_left = painter.image_to_screen(egui::Pos2::new(x_min as f32, y_min as f32));
    let bottom_right =
        painter.image_to_screen(egui::Pos2::new((x_max + 1) as f32, (y_max + 1) as f32));

    painter.draw_dotted_rect(top_left, bottom_right);
}

const DEFAULT_BRUSH_SIZE: NonZeroU16 = NonZeroU16::new(10).unwrap();

#[non_exhaustive]
pub struct BrushTool {
    pub brush_size: NonZeroU16,
    layer: Option<usize>,
    fix_color: Option<[u8; 4]>,
    stroke: Option<StrokeState>,
}

impl Default for BrushTool {
    fn default() -> Self {
        Self {
            brush_size: DEFAULT_BRUSH_SIZE,
            layer: None,
            fix_color: None,
            stroke: None,
        }
    }
}

impl BrushTool {
    pub fn set_layer(&mut self, layer: usize) -> &mut Self {
        self.layer = Some(layer);
        self
    }

    pub fn set_color(&mut self, color: [u8; 4]) -> &mut Self {
        self.fix_color = Some(color);
        self
    }

    pub fn set_brush_size(&mut self, size: NonZeroU16) -> &mut Self {
        self.brush_size = size;
        self
    }

    pub fn create_factory() -> ToolFactory {
        Box::new(|_| async { Ok(Box::new(BrushTool::default()) as Box<dyn Tool>) }.boxed_local())
    }

    pub fn create_factory_with(modifier: impl Fn(&mut BrushTool) + 'static) -> ToolFactory {
        Box::new(move |_| {
            let mut tool = BrushTool::default();
            modifier(&mut tool);
            async { Ok(Box::new(tool) as Box<dyn Tool>) }.boxed_local()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZero;

    fn new_state(w: usize, h: usize) -> StrokeState {
        StrokeState::new(w, h)
    }

    fn count_mask_true(mask: &[bool]) -> usize {
        mask.iter().filter(|&&b| b).count()
    }

    fn mask_at(stroke: &StrokeState, x: usize, y: usize) -> bool {
        stroke.mask[y * stroke.width + x]
    }

    #[test]
    fn stamp_square_center() {
        let mut s = new_state(20, 20);
        s.stamp_square(10, 10, 3);
        assert!(mask_at(&s, 7, 7));
        assert!(mask_at(&s, 13, 13));
        assert!(!mask_at(&s, 6, 10));
        assert!(!mask_at(&s, 10, 14));
        assert_eq!(count_mask_true(&s.mask), 7 * 7);
    }

    #[test]
    fn stamp_square_clamps_top_left() {
        let mut s = new_state(20, 20);
        s.stamp_square(1, 1, 5);
        assert!(mask_at(&s, 0, 0));
        assert!(mask_at(&s, 6, 6));
        assert!(!mask_at(&s, 7, 1));
        assert_eq!(count_mask_true(&s.mask), 7 * 7);
    }

    #[test]
    fn stamp_square_clamps_bottom_right() {
        let mut s = new_state(20, 20);
        s.stamp_square(18, 18, 5);
        assert!(mask_at(&s, 13, 13));
        assert!(mask_at(&s, 19, 19));
        assert!(!mask_at(&s, 12, 18));
        assert_eq!(count_mask_true(&s.mask), 7 * 7);
    }

    #[test]
    fn stamp_square_half_size_zero_single_pixel() {
        let mut s = new_state(10, 10);
        s.stamp_square(5, 5, 0);
        assert!(mask_at(&s, 5, 5));
        assert_eq!(count_mask_true(&s.mask), 1);
    }

    #[test]
    fn stamp_square_exact_corner() {
        let mut s = new_state(10, 10);
        s.stamp_square(0, 0, 3);
        assert!(mask_at(&s, 0, 0));
        assert!(mask_at(&s, 3, 3));
        assert!(!mask_at(&s, 4, 0));
        assert_eq!(count_mask_true(&s.mask), 4 * 4);
    }

    #[test]
    fn stamp_line_horizontal() {
        let mut s = new_state(20, 20);
        s.stamp_line(5, 10, 15, 10, 2);
        assert!(mask_at(&s, 5, 10));
        assert!(mask_at(&s, 15, 10));
        assert!(mask_at(&s, 10, 10));
    }

    #[test]
    fn stamp_line_vertical() {
        let mut s = new_state(20, 20);
        s.stamp_line(10, 5, 10, 15, 2);
        assert!(mask_at(&s, 10, 5));
        assert!(mask_at(&s, 10, 15));
        assert!(mask_at(&s, 10, 10));
    }

    #[test]
    fn stamp_line_same_position_returns_early() {
        let mut s = new_state(10, 10);
        s.stamp_line(5, 5, 5, 5, 2);
        assert_eq!(count_mask_true(&s.mask), 0);
    }

    #[test]
    fn stamp_line_diagonal_covers_path() {
        let mut s = new_state(20, 20);
        s.stamp_line(0, 0, 10, 10, 1);
        assert!(mask_at(&s, 0, 0));
        assert!(mask_at(&s, 5, 5));
        assert!(mask_at(&s, 10, 10));
    }

    #[test]
    fn stamp_to_first_call_stamps_square() {
        let mut s = new_state(20, 20);
        s.stamp_to(10, 10, 2);
        assert!(mask_at(&s, 8, 8));
        assert!(mask_at(&s, 12, 12));
        assert_eq!(s.last_pos, Some((10, 10)));
    }

    #[test]
    fn stamp_to_second_call_uses_line() {
        let mut s = new_state(20, 20);
        s.stamp_to(5, 10, 2);
        s.stamp_to(15, 10, 2);
        assert!(mask_at(&s, 10, 10));
    }

    #[test]
    fn into_pixel_area_empty_mask_returns_none() {
        let s = new_state(10, 10);
        let result = s.into_pixel_area(
            [255, 0, 0, 255],
            NonZero::new(10u32).unwrap(),
            NonZero::new(10u32).unwrap(),
        );
        assert!(result.is_none());
    }

    #[test]
    fn into_pixel_area_single_pixel() {
        let mut s = new_state(10, 10);
        s.stamp_square(5, 5, 0);
        let result = s.into_pixel_area(
            [255, 0, 0, 255],
            NonZero::new(10u32).unwrap(),
            NonZero::new(10u32).unwrap(),
        );
        assert!(result.is_some());
    }

    #[test]
    fn into_pixel_area_full_row() {
        let mut s = new_state(20, 10);
        s.stamp_square(10, 5, 5);
        let result = s.into_pixel_area(
            [255, 0, 0, 255],
            NonZero::new(20u32).unwrap(),
            NonZero::new(10u32).unwrap(),
        );
        assert!(result.is_some());
        let area = result.unwrap();
        assert_eq!(area.color, [255, 0, 0, 255]);
    }

    #[test]
    fn into_pixel_area_disconnected_spans() {
        let mut s = new_state(20, 10);
        s.stamp_square(3, 5, 1);
        s.stamp_square(15, 5, 1);
        let result = s.into_pixel_area(
            [255, 0, 0, 255],
            NonZero::new(20u32).unwrap(),
            NonZero::new(10u32).unwrap(),
        );
        assert!(result.is_some());
    }

    #[test]
    fn stamp_square_overlapping_idempotent() {
        let mut s = new_state(20, 20);
        s.stamp_square(10, 10, 3);
        let count1 = count_mask_true(&s.mask);
        s.stamp_square(10, 10, 3);
        let count2 = count_mask_true(&s.mask);
        assert_eq!(count1, count2);
    }
}

impl Tool for BrushTool {
    fn handle_interaction(&mut self, ctx: ToolContext) {
        let image_width = ctx.image.image.original.width();
        let image_height = ctx.image.image.original.height();
        let width = image_width.get() as usize;
        let height = image_height.get() as usize;
        let half_size = self.brush_size.get() as usize;

        let color = self
            .fix_color
            .unwrap_or_else(|| ctx.image.masks.next_color());

        let cursor_pos = ctx
            .response
            .interact_pointer_pos()
            .or_else(|| ctx.response.hover_pos())
            .map(|screen_pos| {
                let image_pos = ctx.painter.screen_to_image(screen_pos);
                let x = image_pos.x.round().clamp(0.0, (width - 1) as f32) as usize;
                let y = image_pos.y.round().clamp(0.0, (height - 1) as f32) as usize;
                (x, y)
            });

        if ctx.response.drag_started() {
            self.stroke = Some(StrokeState::new(width, height));

            if let Some((x, y)) = cursor_pos {
                self.stroke.as_mut().unwrap().stamp_to(x, y, half_size);
            }
        }

        if ctx.response.dragged() {
            if let Some(stroke) = &mut self.stroke {
                if let Some((x, y)) = cursor_pos {
                    stroke.stamp_to(x, y, half_size);
                }
            }
        }

        if let Some(stroke) = &mut self.stroke {
            stroke.update_texture(ctx.egui, color);
            stroke.render(ctx.painter);
        }

        if let Some((x, y)) = cursor_pos {
            draw_brush_outline(ctx.painter, x, y, half_size, width, height);
        }

        if ctx.response.drag_stopped() {
            if !ctx.egui.input(|i| i.modifiers.command || i.modifiers.ctrl) {
                if let Some(stroke) = self.stroke.take() {
                    if let Some(pixel_area) =
                        stroke.into_pixel_area(color, image_width, image_height)
                    {
                        ctx.image
                            .masks
                            .on_layer(self.layer)
                            .keep_overlapping(self.layer.is_some())
                            .add(pixel_area);
                    }
                }
            } else {
                self.stroke = None;
            }
        } else if ctx.response.clicked() {
            if let Some((x, y)) = cursor_pos {
                let mut stroke = StrokeState::new(width, height);
                stroke.stamp_to(x, y, half_size);
                if let Some(pixel_area) = stroke.into_pixel_area(color, image_width, image_height) {
                    ctx.image
                        .masks
                        .on_layer(self.layer)
                        .keep_overlapping(false)
                        .add(pixel_area);
                }
            }
        }
    }
}
