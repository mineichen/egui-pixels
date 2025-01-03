use std::{io::BufRead, ops::Deref, str::FromStr};

use crate::{
    history::History,
    inference::{self, DefaultSession, InferenceError, SamEmbeddings},
    viewer::{ImageViewer, ImageViewerInteraction},
    Annotation, SubGroups,
};
use eframe::egui::{
    self,
    load::{BytesPoll, SizedTexture},
    Color32, ColorImage, ComboBox, ImageSource, InnerResponse, Sense, TextureHandle,
    TextureOptions, UiBuilder,
};
use futures::{future::BoxFuture, FutureExt};
use image::{ImageBuffer, Luma, Rgb};
use log::info;

pub(crate) struct ImageViewerApp {
    pub url_idx: usize,
    pub urls: Vec<String>,
    pub viewer: ImageViewer,
    pub current_raw_data: Option<(
        TextureHandle,
        Result<inference::SamEmbeddings, BoxFuture<'static, Result<SamEmbeddings, InferenceError>>>,
    )>,
    pub mask_image: Option<([usize; 2], Vec<Annotation>, History, Option<TextureHandle>)>,
    pub last_drag_start: Option<(usize, usize)>,
    pub session: DefaultSession,
}

impl ImageViewerApp {
    pub fn new() -> Self {
        let base = "file:///Users/mineichen/Downloads/2024-10-31_13/";
        Self {
            url_idx: 0,
            urls: vec![
                format!("{base}2024-10-31_13-46-28-194.png"),
                format!("{base}2024-10-31_13-46-36-599.png"),
                format!("{base}2024-10-31_13-46-27-278.png"),
                format!("{base}2024-10-31_13-46-33-767.png"),
                format!("{base}2024-10-31_13-46-17-933.png"),
            ],
            viewer: ImageViewer::new(vec![]),
            current_raw_data: None,
            mask_image: None,
            last_drag_start: None,
            session: DefaultSession::new().unwrap(),
        }
    }
}

impl eframe::App for ImageViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Image pixel selector");

            ui.horizontal(|ui| {
                if ui.button("<").clicked() {
                    self.url_idx = (self.url_idx.checked_sub(1)).unwrap_or(self.urls.len() - 1);
                    self.viewer.reset();
                }
                if ComboBox::from_id_salt("url_selector")
                    .show_index(ui, &mut self.url_idx, self.urls.len(), |x| {
                        self.urls[x].deref()
                    })
                    .changed()
                {
                    self.viewer.reset();
                }
                if ui.button(">").clicked() {
                    self.url_idx = (self.url_idx + 1) % self.urls.len();
                    self.viewer.reset();
                }
            });
            if let Some(url) = self.urls.get(self.url_idx) {
                if self.viewer.sources.is_empty() {
                    if let BytesPoll::Ready { bytes, .. } = ctx.try_load_bytes(&url).unwrap() {
                        let image = match image::load_from_memory(&bytes)
                            .expect("Expected valid imagedata")
                        {
                            image::DynamicImage::ImageLuma16(i) => {
                                image::DynamicImage::ImageLuma16(fix_image_contrast(i))
                            }
                            image::DynamicImage::ImageLuma8(i) => {
                                image::DynamicImage::ImageLuma8(fix_image_contrast(i))
                            }
                            image => image,
                        };
                        let rgb_image = image.to_rgb8();
                        let handle = ctx.load_texture(
                            "Overlays",
                            ColorImage {
                                size: [rgb_image.width() as _, rgb_image.height() as _],
                                pixels: rgb_image
                                    .pixels()
                                    .map(|&Rgb([r, g, b])| Color32::from_rgb(r, g, b))
                                    .collect(),
                            },
                            TextureOptions {
                                magnification: egui::TextureFilter::Nearest,
                                ..Default::default()
                            },
                        );
                        let texture = SizedTexture::from_handle(&handle);
                        self.viewer.sources = vec![ImageSource::Texture(texture)];
                        let embeddings = self.session.get_image_embeddings(image).boxed();
                        self.current_raw_data = Some((handle, Err(embeddings)));
                    }
                }
            } else {
                self.viewer.sources = Vec::new();
            }

            let mut available = ui.available_rect_before_wrap();
            available.max.y = (available.max.y - 80.).max(0.);

            let r = ui.allocate_new_ui(UiBuilder::new().max_rect(available), |ui| {
                self.viewer.ui_meta(ui, Some(Sense::click()))
            });
            let InnerResponse {
                inner:
                    InnerResponse {
                        inner:
                            Some(ImageViewerInteraction {
                                original_image_size,
                                cursor_image_pos,
                            }),
                        response,
                        ..
                    },
                ..
            } = r
            else {
                return;
            };
            if response.drag_started() {
                self.last_drag_start = cursor_image_pos;
            }

            if let (Some((_, _, history, handle)), (shift_pressed, true)) = (
                self.mask_image.as_mut(),
                ui.input(|i| {
                    (
                        i.modifiers.shift,
                        i.key_pressed(egui::Key::Z) && i.modifiers.command,
                    )
                }),
            ) {
                let require_redraw = if shift_pressed {
                    info!("Redo");
                    history.redo().is_some()
                } else {
                    info!("Undo");
                    history.undo().is_some()
                };
                if require_redraw {
                    *handle = None;
                };
            }

            if let (
                Some((_, _, history, handle)),
                Some(&(cursor_x, cursor_y)),
                Some(e),
                Some(&(start_x, start_y)),
                true,
            ) = (
                self.mask_image.as_mut(),
                cursor_image_pos.as_ref(),
                self.current_raw_data.as_mut(),
                self.last_drag_start.as_ref(),
                response.drag_stopped() && !ui.input(|i| i.modifiers.command || i.modifiers.ctrl),
            ) {
                info!("Dragging ended");
                if let Err(pending) = e.1.as_mut() {
                    let x = futures::executor::block_on(pending).unwrap();
                    e.1 = Ok(x);
                };
                let Ok(loaded_embeddings) = e.1.as_ref() else {
                    unreachable!("Loaded above")
                };
                let mask = self
                    .session
                    .decode_prompt(
                        cursor_x.min(start_x) as f32,
                        cursor_y.min(start_y) as f32,
                        cursor_x.max(start_x) as f32,
                        cursor_y.max(start_y) as f32,
                        loaded_embeddings,
                    )
                    .unwrap();

                history.push(("New group".into(), mask));

                self.last_drag_start = None;
                *handle = None;
            }

            if self.viewer.sources.len() == 1 {
                match ctx.try_load_bytes("file://masks.csv") {
                    Ok(BytesPoll::Ready { bytes, mime, .. }) => {
                        let x = original_image_size.x as usize;
                        let y = original_image_size.y as usize;

                        let lines = bytes
                            .lines()
                            .filter_map(|x| {
                                let s = x.ok()?;
                                let mut parts = s.split(';');
                                let label = parts.next()?;
                                let lines = parts
                                    .map(|x| {
                                        let (start, end) = x.split_once(',')?;
                                        Some((u32::from_str(start).ok()?, end.parse().ok()?))
                                    })
                                    .collect::<Option<SubGroups>>()?;
                                Some((label.into(), lines))
                            })
                            .collect::<Vec<_>>();

                        self.mask_image = Some(([x, y], lines, Default::default(), None));

                        println!(
                            "Got {:?} bytes of type {mime:?}: {}",
                            bytes.len(),
                            String::from_utf8_lossy(&bytes)
                        )
                    }
                    _ => {}
                }
            }
            if let Some((size, groups, history, x @ None)) = self.mask_image.as_mut() {
                let texture_options = TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    ..Default::default()
                };

                let mut pixels = vec![Color32::TRANSPARENT; size[0] * size[1]];
                let group_color = Color32::from_rgba_unmultiplied(255, 255, 0, 10);
                for (pos, len) in groups
                    .iter()
                    .flat_map(|(_, b)| b)
                    .chain(history.iter().flatten())
                {
                    let pos = *pos as usize;
                    pixels[pos..(pos + len.get() as usize)].fill(group_color);
                }

                let handle = ctx.load_texture(
                    "Overlays",
                    ColorImage {
                        size: *size,
                        pixels,
                    },
                    texture_options,
                );

                let mask = ImageSource::Texture(SizedTexture::from_handle(&handle));
                self.viewer.sources.push(mask);
                *x = Some(handle);
            }

            // Zoom level display
            ui.label(format!("Zoom (Ctrl+Wheel): {:.2}x", self.viewer.zoom));
            ui.label(format!(
                "Original Size: ({original_image_size:?}), \navail: {:?}, \nspacing: {:?}",
                original_image_size,
                ui.spacing().item_spacing
            ));

            if let Some((x, y)) = cursor_image_pos {
                ui.label(format!("Pixel Coordinates: ({}, {})", x, y,));
            }
        });
    }
}

fn fix_image_contrast<T: image::Primitive + Ord>(
    i: ImageBuffer<Luma<T>, Vec<T>>,
) -> ImageBuffer<Luma<T>, Vec<T>>
where
    f32: From<T>,
{
    let mut pixels = i.pixels().map(|Luma([p])| p).collect::<Vec<_>>();
    pixels.sort_unstable();
    let five_percent_pos = pixels.len() / 20;
    let lower: f32 = (*pixels[five_percent_pos]).into();
    let upper: f32 = (*pixels[five_percent_pos * 18]).into();
    let max_pixel_value: f32 = T::DEFAULT_MAX_VALUE.into();
    let range = max_pixel_value / (upper - lower);

    ImageBuffer::from_raw(
        i.width(),
        i.height(),
        i.pixels()
            .map(|Luma([p])| {
                let as_f: f32 = (*p).into();

                num_traits::cast::NumCast::from(
                    ((as_f - lower) * range).clamp(0.0, max_pixel_value),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
