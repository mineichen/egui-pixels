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
        self.zoom = zoom(self.zoom).clamp(0.05, 10.0);
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
        let pixel_offset = original_image_size * -self.pan_offset / self.zoom;
        let image_rect_unclipped =
            Rect::from_min_size(viewport_rect.min, original_image_size / self.zoom)
                .translate(pixel_offset);

        p.image(
            first_texture.id,
            image_rect_unclipped,
            uv,
            egui::Color32::WHITE,
        );
        while let Some((texture, _)) = next_loaded(&mut iter, ui) {
            p.image(texture.id, image_rect_unclipped, uv, egui::Color32::WHITE);
        }
        let drag_delta = response.drag_delta();
        if drag_delta != Vec2::default() && ui.input(|i| i.modifiers.command || i.modifiers.ctrl) {
            let pan_delta = drag_delta / image_rect_unclipped.size();
            self.pan_offset -= pan_delta;
        }

        let interaction = ImageViewerInteraction {
            original_image_size,
            cursor_image_pos: response.hover_pos().map(|hover| {
                let hover_relative_px = (hover - viewport_rect.min.to_vec2()).to_vec2();
                let p = hover_relative_px * self.zoom - (original_image_size * -self.pan_offset);

                let delta = ui.input(|i| i.zoom_delta());
                if delta != 1.0 {
                    self.modify_zoom(|x| x / delta);
                    self.pan_offset = (-(hover_relative_px * self.zoom - p) / original_image_size);
                    //.clamp(Vec2::ZERO, Vec2::splat(1. - self.zoom));
                }

                log::info!(
                    "Hover: {:?}, pan_offset: {:?}, zoom: {:?}, pixel_offset: {:?}",
                    (p.x, p.y),
                    (self.pan_offset.x, self.pan_offset.y),
                    self.zoom,
                    pixel_offset,
                );
                (p.x as _, p.y as _)
            }),
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
