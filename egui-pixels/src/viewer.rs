use egui::{
    self, ImageSource, InnerResponse, Pos2, Rect, Sense, TextureOptions, Vec2,
    load::{SizedTexture, TexturePoll},
};

pub struct ImageViewer {
    // Zoom level (0.05..1.0)
    // 1.0 means, that image width or height fits the viewport and the other dimension is smaller than the viewport
    zoom: f32,
    // Offset of the top left-corner (in percent)
    pub pan_offset: Vec2,
}

impl ImageViewer {
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = Vec2::default();
    }

    pub fn modify_zoom(&mut self, zoom: impl Fn(f32) -> f32) {
        self.zoom = zoom(self.zoom).clamp(0.05, 1.0);
    }

    fn update_pan_axis<F>(
        &mut self,
        tentative_pan: Vec2,
        mut image_size_px: Vec2,
        mut viewport_size: Vec2,
        mut prev_pixel_offset: Vec2,
        axis: F,
    ) where
        F: Fn(&mut Vec2) -> &mut f32,
    {
        let mut pixel_offset_after_vec = (image_size_px) * -(tentative_pan);

        let after_component_offset = *axis(&mut pixel_offset_after_vec);
        let prev_component_offset = *axis(&mut prev_pixel_offset);
        let image_component = *axis(&mut image_size_px);
        let viewport_component = *axis(&mut viewport_size);

        let prev_blank_start = prev_component_offset.max(0.0);
        let prev_blank_end =
            (viewport_component - (prev_component_offset + image_component)).max(0.0);
        let after_blank_start = after_component_offset.max(0.0);
        let after_blank_end =
            (viewport_component - (after_component_offset + image_component)).max(0.0);

        let prev_sum_blank = prev_blank_start + prev_blank_end;
        let after_sum_blank = after_blank_start + after_blank_end;

        if after_sum_blank < prev_sum_blank || after_sum_blank <= f32::EPSILON {
            *axis(&mut self.pan_offset) = -(after_component_offset / image_component);
        } else if prev_sum_blank <= f32::EPSILON {
            let clamped = after_component_offset.clamp(viewport_component - image_component, 0.0);
            *axis(&mut self.pan_offset) = -(clamped / image_component);
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        sources: impl Iterator<Item = ImageSource<'static>>,
        sense: Option<Sense>,
    ) -> InnerResponse<Option<ImageViewerInteraction>> {
        let available_size = ui.available_size();
        let viewport_rect = ui.available_rect_before_wrap();

        let mut iter = sources.map(|i| {
            egui::Image::new(i)
                .maintain_aspect_ratio(true)
                // Important for Texture-ImageSources
                .fit_to_exact_size(available_size)
                .texture_options(TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    ..Default::default()
                })
        });
        fn next_loaded(
            iter: impl Iterator<Item = egui::Image<'static>>,
            ui: &egui::Ui,
        ) -> Option<(SizedTexture, egui::Image<'static>)> {
            iter.filter_map(|image| {
                let tlr = image.load_for_size(ui.ctx(), ui.available_size());
                match tlr {
                    Ok(TexturePoll::Ready { texture }) => Some((texture, image)),
                    _ => None,
                }
            })
            .next()
        }

        let Some((first_texture, _image)) = next_loaded(&mut iter, ui) else {
            return InnerResponse {
                inner: None,
                response: ui.response(),
            };
        };

        let original_image_size = first_texture.size;
        let my_sense = Sense::hover().union(Sense::drag());
        let combined_sense = sense.map(|s| s.union(my_sense)).unwrap_or(my_sense);

        let response = ui.allocate_rect(viewport_rect, combined_sense);
        let p = ui.painter().with_clip_rect(viewport_rect);
        // p.rect(
        //     viewport_rect,
        //     10.0,
        //     egui::Color32::WHITE,
        //     egui::Stroke::NONE,
        //     egui::StrokeKind::Inside,
        // );

        let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));

        // Compute scale so that at zoom=1.0 the whole image fits the viewport (letterboxed/pillarboxed)
        let viewport_size = viewport_rect.size();
        let fit_scale =
            (viewport_size.x / original_image_size.x).min(viewport_size.y / original_image_size.y);

        let cursor_image_pos = {
            let render_scale = fit_scale / self.zoom;
            let image_size_px = original_image_size * render_scale;

            let drag_delta = response.drag_delta();
            if drag_delta != Vec2::default()
                && ui.input(|i| i.modifiers.command || i.modifiers.ctrl)
            {
                // Pan offset is expressed in fractions of the rendered image size
                // Apply raw pan delta, but constrain per-axis to only allow reducing existing blank space
                let prev_pixel_offset = image_size_px * -self.pan_offset;
                let tentative_pan = self.pan_offset - (drag_delta / image_size_px);

                self.update_pan_axis(
                    tentative_pan,
                    image_size_px,
                    viewport_size,
                    prev_pixel_offset,
                    |v| &mut v.x,
                );

                self.update_pan_axis(
                    tentative_pan,
                    image_size_px,
                    viewport_size,
                    prev_pixel_offset,
                    |v| &mut v.y,
                );
            }

            let is_zoomed_out = (self.zoom - 1.0).abs() <= f32::EPSILON;
            if is_zoomed_out {
                let s_new = fit_scale / self.zoom;
                let image_size_px_new = original_image_size * s_new;
                let center_x = ((viewport_size.x - image_size_px_new.x) * 0.5).max(0.0);
                let center_y = ((viewport_size.y - image_size_px_new.y) * 0.5).max(0.0);
                self.pan_offset.x = -(center_x / image_size_px_new.x);
                self.pan_offset.y = -(center_y / image_size_px_new.y);
            }

            response.hover_pos().map(|hover| {
                let pixel_offset = image_size_px * -self.pan_offset;
                let screen_rel = (hover - viewport_rect.min.to_vec2()).to_vec2();

                // Use zoom relative to fit, so p stays constant in original image space
                let rel_zoom = self.zoom / fit_scale;
                let p = (screen_rel - pixel_offset) * rel_zoom;

                let delta = ui.input(|i| i.zoom_delta());
                if delta != 1.0 {
                    self.modify_zoom(|x| x / delta);
                    let rel_zoom_new = self.zoom / fit_scale;
                    self.pan_offset = (p - screen_rel * rel_zoom_new) / original_image_size;
                }

                log::info!(
                    "Hover: {:?}, pan_offset: {:?}, zoom: {:?}, pixel_offset: {:?}",
                    (p.x, p.y),
                    (self.pan_offset.x, self.pan_offset.y),
                    self.zoom,
                    pixel_offset,
                );
                (p.x as _, p.y as _)
            })
        };

        let render_scale = fit_scale / self.zoom;
        let image_size_px = original_image_size * render_scale;
        let pixel_offset = image_size_px * -self.pan_offset;

        let image_rect_unclipped =
            Rect::from_min_size(viewport_rect.min + pixel_offset, image_size_px);

        p.image(
            first_texture.id,
            image_rect_unclipped,
            uv,
            egui::Color32::WHITE,
        );
        while let Some((texture, _)) = next_loaded(&mut iter, ui) {
            p.image(texture.id, image_rect_unclipped, uv, egui::Color32::WHITE);
        }

        let interaction = ImageViewerInteraction {
            original_image_size,
            cursor_image_pos,
        };

        InnerResponse {
            inner: Some(interaction),
            response: response,
        }
    }
}

impl Default for ImageViewer {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
        }
    }
}

pub struct ImageViewerInteraction {
    pub original_image_size: Vec2,
    // Cursor position relative to image
    pub cursor_image_pos: Option<(usize, usize)>,
}
