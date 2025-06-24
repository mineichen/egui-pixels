use eframe::egui::{
    self, load::TexturePoll, ImageSource, InnerResponse, Rect, Sense, TextureOptions, Vec2, Widget,
};

pub struct ImageViewer {
    // Zoom level
    zoom: f32,
    // Offset of the top left-corner (in percent)
    pub pan_offset: Vec2,
    // Images
    pub sources: Vec<ImageSource<'static>>,
}

impl ImageViewer {
    pub fn new(sources: Vec<ImageSource<'static>>) -> Self {
        Self {
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            sources,
        }
    }

    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = Vec2::default();
        self.sources.clear();
    }

    pub fn ui_meta(
        &mut self,
        ui: &mut egui::Ui,
        sense: Option<Sense>,
    ) -> InnerResponse<Option<ImageViewerInteraction>> {
        let centered = ui.vertical_centered(|ui| {
            let mut iter = self
                .sources
                .iter()
                .filter_map(|i| {
                    let image = egui::Image::new(i.clone())
                        .maintain_aspect_ratio(true)
                        // Important for Texture-ImageSources
                        .fit_to_exact_size(ui.available_size())
                        .texture_options(TextureOptions {
                            magnification: egui::TextureFilter::Nearest,
                            ..Default::default()
                        });
                    let tlr = image.load_for_size(ui.ctx(), ui.available_size());
                    match tlr {
                        Ok(TexturePoll::Ready { texture }) => Some((
                            texture,
                            image.calc_size(ui.available_size(), Some(texture.size)),
                        )),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>()
                .into_iter();

            let (first_texture, size) = iter.next()?;
            let original_image_size = first_texture.size;
            let my_sense = Sense::hover().union(Sense::drag());
            let (response, p) = ui.allocate_painter(
                size,
                match sense {
                    Some(x) => x.union(my_sense),
                    None => my_sense,
                },
            );
            let draw_rect = response.rect;

            let uv = Rect::from_min_max(
                self.pan_offset.to_pos2(),
                self.pan_offset.to_pos2() + Vec2::splat(self.zoom),
            );
            p.image(first_texture.id, draw_rect, uv, egui::Color32::WHITE);
            for (texture, _) in iter {
                p.image(texture.id, draw_rect, uv, egui::Color32::WHITE);
            }
            let drag_delta = response.drag_delta();

            if drag_delta != Vec2::default()
                && ui.input(|i| i.modifiers.command || i.modifiers.ctrl)
            {
                let pan_delta = drag_delta / draw_rect.size() * Vec2::splat(self.zoom);
                self.pan_offset =
                    (self.pan_offset - pan_delta).clamp(Vec2::ZERO, Vec2::splat(1. - self.zoom));
            }

            let interaction = ImageViewerInteraction {
                original_image_size,
                cursor_image_pos: if let Some(hover) = response.hover_pos() {
                    let mut viewport_pos_percentual = hover - draw_rect.min.to_vec2();
                    viewport_pos_percentual.x /= draw_rect.width();
                    viewport_pos_percentual.y /= draw_rect.height();

                    let delta = ui.input(|i| i.zoom_delta());
                    if delta != 1.0 {
                        let locked_image_pixel =
                            self.pan_offset + self.zoom * viewport_pos_percentual.to_vec2();
                        self.zoom = (self.zoom / delta).clamp(0.05, 1.0);
                        self.pan_offset = (locked_image_pixel
                            - self.zoom * viewport_pos_percentual.to_vec2())
                        .clamp(Vec2::ZERO, Vec2::splat(1. - self.zoom));
                    }
                    Some({
                        let p = (self.pan_offset + viewport_pos_percentual.to_vec2() * self.zoom)
                            * original_image_size;
                        (p.x as _, p.y as _)
                    })
                } else {
                    None
                },
            };

            Some((response, interaction))
        });
        match centered {
            InnerResponse {
                inner: Some((response, x)),
                response: outer_response,
            } => InnerResponse {
                inner: Some(x),
                response: response.union(outer_response),
            },
            InnerResponse {
                inner: None,
                response,
            } => InnerResponse {
                inner: None,
                response,
            },
        }
    }
}

impl Widget for &mut ImageViewer {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.ui_meta(ui, None).response
    }
}

pub struct ImageViewerInteraction {
    pub original_image_size: Vec2,
    // Cursor position relative to image
    pub cursor_image_pos: Option<(usize, usize)>,
}
