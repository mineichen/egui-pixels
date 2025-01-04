use std::task::{Context, Poll};

use crate::{
    history::History,
    image_utils::load_image,
    inference::{self, DefaultSession, InferenceError, SamEmbeddings},
    storage::{parse_masks, ImageData, ImageId, Storage},
    viewer::{ImageViewer, ImageViewerInteraction},
    Annotation,
};
use eframe::egui::{
    self,
    load::{BytesPoll, SizedTexture},
    Color32, ColorImage, ComboBox, ImageSource, InnerResponse, Key, Sense, TextureHandle,
    TextureOptions, UiBuilder,
};
use futures::{
    future::{BoxFuture, Either},
    FutureExt,
};
use image::GenericImageView;
use log::{debug, info};

type LoadedOrLoading<T> = Either<T, BoxFuture<'static, T>>;

pub(crate) struct ImageViewerApp {
    pub storage: Storage,
    pub url_idx: usize,
    pub urls: LoadedOrLoading<std::io::Result<Vec<(ImageId, String)>>>,
    pub viewer: ImageViewer,
    pub selected: Option<LoadedOrLoading<std::io::Result<ImageData>>>,
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
        let storage = Storage::new("//Users/mineichen/Downloads/2024-10-31_13/");
        let urls = Either::Right(storage.list_images());

        Self {
            storage,
            url_idx: 0,
            urls,
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
        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Image pixel selector");

            let mut reload_images = false;
            match &mut self.urls {
                Either::Right(x) => {
                    if let std::task::Poll::Ready(x) = x.poll_unpin(&mut context) {
                        self.urls = Either::Left(x);
                    }
                }
                Either::Left(Err(e)) => {
                    ui.label(format!("{e}"));
                }
                Either::Left(Ok(urls)) => {
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
                        if self.selected.is_none() {
                            self.selected = Some(Either::Right(self.storage.load_image(image_id)));
                        }
                        let selected = self.selected.as_mut().expect("If empty, it was set above");
                        match selected {
                            Either::Right(x) => {
                                if let Poll::Ready(x) = x.poll_unpin(&mut context) {
                                    *selected = Either::Left(x);
                                }
                            }
                            Either::Left(image_data) => {}
                        }
                        if self.viewer.sources.is_empty() {
                            if let BytesPoll::Ready { bytes, .. } =
                                ctx.try_load_bytes(image_id.uri().as_str()).unwrap()
                            {
                                let image = load_image(&bytes).unwrap();
                                let handle = ctx.load_texture(
                                    "Overlays",
                                    ColorImage {
                                        size: [image.width() as _, image.height() as _],
                                        pixels: image
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
                                let embeddings = self.session.get_image_embeddings(image).boxed();
                                self.current_raw_data = Some((handle, Err(embeddings)));
                            }
                        }
                    } else {
                        self.viewer.sources = Vec::new();
                    }
                }
            }
            if reload_images {
                self.urls = Either::Right(self.storage.list_images());
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
                if let Ok(BytesPoll::Ready { bytes, mime, .. }) =
                    ctx.try_load_bytes("file://masks.csv")
                {
                    let x = original_image_size.x as usize;
                    let y = original_image_size.y as usize;

                    let lines = parse_masks(&bytes);
                    self.mask_image = Some(([x, y], lines, Default::default(), None));

                    debug!(
                        "Got {:?} bytes of type {mime:?}: {}",
                        bytes.len(),
                        String::from_utf8_lossy(&bytes)
                    )
                }
            }
            if let Some((size, groups, history, x @ None)) = self.mask_image.as_mut() {
                let texture_options = TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    ..Default::default()
                };

                let mut pixels = vec![Color32::TRANSPARENT; size[0] * size[1]];
                let group_color = Color32::from_rgba_premultiplied(64, 64, 0, 64);
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
