use egui::{
    self, ImageSource, InnerResponse, Pos2, Rect, Sense, TextureOptions, Vec2,
    load::{SizedTexture, TexturePoll},
};

pub struct ImageViewer {
    // Zoom level (0.05..1.0)
    // 1.0 means, that image width or height fits the viewport and the other dimension is smaller than the viewport
    zoom: f32,
    // Normalized image coordinate of the viewport center per axis in [0, 1]:
    // 0.0 = left/top image edge is at the viewport center
    // 0.5 = image center is at the viewport center (fully centered)
    // 1.0 = right/bottom image edge is at the viewport center
    pub pan_offset: Vec2,
}

impl ImageViewer {
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = Vec2::splat(0.5);
    }

    pub fn modify_zoom(&mut self, zoom: impl Fn(f32) -> f32) {
        self.zoom = zoom(self.zoom).clamp(0.05, 1.0);
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

            let drag_delta = response.drag_delta();
            if drag_delta != Vec2::default()
                && ui.input(|i| i.modifiers.command || i.modifiers.ctrl)
            {
                let delta_norm = drag_delta / (render_scale * original_image_size);
                self.pan_offset -= delta_norm;
            }

            let is_zoomed_out = (self.zoom - 1.0).abs() <= f32::EPSILON;
            if is_zoomed_out {
                self.pan_offset = Vec2::splat(0.5);
            }

            response.hover_pos().map(|hover| {
                // Where to place the image so that the image point at `pan_offset`
                // (normalized) appears at the viewport center.
                let center_img_px = self.pan_offset * original_image_size;
                let pixel_offset = viewport_size * 0.5 - center_img_px * render_scale;
                let screen_rel = (hover - viewport_rect.min.to_vec2()).to_vec2();

                // Use zoom relative to fit, so p stays constant in original image space
                let rel_zoom = self.zoom / fit_scale;
                let p = (screen_rel - pixel_offset) * rel_zoom;

                let delta = ui.input(|i| i.zoom_delta());
                if delta != 1.0 {
                    self.modify_zoom(|x| x / delta);
                    let rel_zoom_new = self.zoom / fit_scale;
                    let render_scale_new = fit_scale / self.zoom;
                    let desired_pixel_offset = screen_rel - (p / rel_zoom_new);

                    let pan = (viewport_size * 0.5 - desired_pixel_offset)
                        / (original_image_size * render_scale_new);

                    self.pan_offset = pan;
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
        let pixel_offset = viewport_size * 0.5 - self.pan_offset * image_size_px;

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
            pan_offset: Vec2::splat(0.5),
        }
    }
}

pub struct ImageViewerInteraction {
    pub original_image_size: Vec2,
    // Cursor position relative to image
    pub cursor_image_pos: Option<(usize, usize)>,
}
