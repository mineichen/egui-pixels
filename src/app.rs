use std::{
    num::NonZeroU16,
    task::{Context, Poll},
};

use crate::{
    async_task::{AsyncRefTask, AsyncTask},
    history::History,
    inference::{self, DefaultSession, InferenceError, SamEmbeddings},
    storage::{ImageData, ImageId, Storage},
    viewer::{ImageViewer, ImageViewerInteraction},
    Annotation,
};
use eframe::egui::{
    self, load::SizedTexture, Color32, ColorImage, ComboBox, ImageSource, InnerResponse, Key,
    Sense, TextureHandle, TextureOptions, UiBuilder,
};
use futures::{future::BoxFuture, FutureExt};
use image::GenericImageView;
use log::info;

pub(crate) struct ImageViewerApp {
    pub storage: Storage,
    pub url_idx: usize,
    pub urls: AsyncRefTask<std::io::Result<Vec<(ImageId, String)>>>,
    pub viewer: ImageViewer,
    pub image_state: ImageState,
    pub selected: Option<AsyncRefTask<std::io::Result<ImageData>>>,
    pub current_raw_data: Option<(
        TextureHandle,
        Result<inference::SamEmbeddings, BoxFuture<'static, Result<SamEmbeddings, InferenceError>>>,
    )>,
    pub mask_image: Option<MaskImage>,
    pub last_drag_start: Option<(usize, usize)>,
    pub session: DefaultSession,
}

enum ImageState {
    NotLoaded,
    LoadingImageData(AsyncTask<std::io::Result<ImageData>>),
    LoadingEmbeddings(ImageData, AsyncTask<Result<SamEmbeddings, InferenceError>>),
    Loaded(ImageData, SamEmbeddings),
    Error(String),
}

pub(crate) struct MaskImage {
    size: [usize; 2],
    annotations: Annotations,
    history: History,
    texture_handle: Option<TextureHandle>,
}

impl MaskImage {
    fn subgroups(&self) -> impl Iterator<Item = (u32, NonZeroU16)> + '_ {
        self.annotations
            .0
            .iter()
            .flat_map(|(_, b)| b)
            .chain(self.history.iter().flatten())
            .copied()
    }
}

pub(crate) struct Annotations(Vec<Annotation>);

impl ImageViewerApp {
    pub fn new() -> Self {
        let storage = Storage::new("/Users/mineichen/Downloads/2024-10-31_13/");
        let urls = AsyncRefTask::new(storage.list_images());

        Self {
            storage,
            url_idx: 0,
            urls,
            image_state: ImageState::NotLoaded,
            viewer: ImageViewer::new(vec![]),
            selected: None,
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

            let mut reload_images = false;
            match self.urls.data() {
                None => {}
                Some(Err(e)) => {
                    ui.label(format!("{e}"));
                }
                Some(Ok(urls)) => {
                    ui.horizontal(|ui| {
                        if ui.button("<").clicked() || ui.input(|i| i.key_pressed(Key::ArrowLeft)) {
                            self.url_idx = (self.url_idx.checked_sub(1)).unwrap_or(urls.len() - 1);
                            self.viewer.reset();
                        }
                        if ComboBox::from_id_salt("url_selector")
                            .show_index(ui, &mut self.url_idx, urls.len(), |x| {
                                urls.get(x).map(|x| x.1.as_str()).unwrap_or("")
                            })
                            .changed()
                        {
                            self.viewer.reset();
                        }
                        if ui.button(">").clicked() || ui.input(|i| i.key_pressed(Key::ArrowRight))
                        {
                            self.url_idx = (self.url_idx + 1) % urls.len();
                            self.viewer.reset();
                        }
                        if ui.button("reload").clicked() {
                            reload_images = true;
                        }
                    });
                    if let Some((image_id, _)) = urls.get(self.url_idx) {
                        match self
                            .selected
                            .get_or_insert(AsyncRefTask::new(self.storage.load_image(image_id)))
                            .data()
                        {
                            None => {
                                ui.label("Loading...");
                            }
                            Some(Err(e)) => {
                                ui.label(format!("{e:?}"));
                            }
                            Some(Ok(ImageData {
                                adjust_image,
                                id,
                                masks,
                                ..
                            })) if id == image_id => {
                                if self.viewer.sources.is_empty() {
                                    let handle = ctx.load_texture(
                                        "Overlays",
                                        ColorImage {
                                            size: [
                                                adjust_image.width() as _,
                                                adjust_image.height() as _,
                                            ],
                                            pixels: adjust_image
                                                .pixels()
                                                .map(|(_, _, image::Rgba([r, g, b, _]))| {
                                                    Color32::from_rgb(r, g, b)
                                                })
                                                .collect(),
                                        },
                                        TextureOptions {
                                            magnification: egui::TextureFilter::Nearest,
                                            ..Default::default()
                                        },
                                    );
                                    let texture = SizedTexture::from_handle(&handle);
                                    self.viewer.sources = vec![ImageSource::Texture(texture)];
                                    let embeddings = self
                                        .session
                                        .get_image_embeddings(adjust_image.clone())
                                        .boxed();
                                    self.current_raw_data = Some((handle, Err(embeddings)));
                                    let x = adjust_image.width() as usize;
                                    let y = adjust_image.height() as usize;

                                    self.mask_image = Some(MaskImage {
                                        size: [x, y],
                                        annotations: Annotations(masks.clone()),
                                        history: Default::default(),
                                        texture_handle: None,
                                    });
                                }
                            }
                            _ => self.selected = None,
                        }
                    } else {
                        self.viewer.sources = Vec::new();
                    }
                }
            }
            if reload_images {
                self.urls = AsyncRefTask::new(self.storage.list_images());
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

            if let (
                Some(MaskImage {
                    history,
                    texture_handle,
                    ..
                }),
                (shift_pressed, true),
            ) = (
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
                    *texture_handle = None;
                };
            }

            if let (
                Some(MaskImage {
                    history,
                    texture_handle,
                    ..
                }),
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
                *texture_handle = None;
            }

            if let Some(
                i @ MaskImage {
                    texture_handle: None,
                    ..
                },
            ) = self.mask_image.as_mut()
            {
                let texture_options = TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    ..Default::default()
                };

                let mut pixels = vec![Color32::TRANSPARENT; i.size[0] * i.size[1]];

                for (pos, len) in i.subgroups() {
                    let group_color = Color32::from_rgba_premultiplied(64, 64, 0, 64);
                    let pos = pos as usize;
                    pixels[pos..(pos + len.get() as usize)].fill(group_color);
                }

                let handle = ctx.load_texture(
                    "Overlays",
                    ColorImage {
                        size: i.size,
                        pixels,
                    },
                    texture_options,
                );

                let mask = ImageSource::Texture(SizedTexture::from_handle(&handle));
                self.viewer.sources.push(mask);
                i.texture_handle = Some(handle);
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
