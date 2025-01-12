use std::{io, sync::Arc};

use crate::{
    async_task::{AsyncRefTask, AsyncTask},
    inference::{InferenceError, SamEmbeddings, SamSession},
    mask::MaskImage,
    mask_generator::MaskGenerator,
    storage::{ImageData, ImageId, Storage},
    viewer::{ImageViewer, ImageViewerInteraction},
};
use eframe::egui::{
    self, load::SizedTexture, Color32, ColorImage, ComboBox, ImageSource, InnerResponse, Key,
    Sense, TextureHandle, TextureOptions, UiBuilder,
};
use futures::FutureExt;
use image::{DynamicImage, GenericImageView};
use log::warn;

pub(crate) struct ImageViewerApp {
    storage: Storage,
    url_idx: usize,
    urls: AsyncRefTask<io::Result<Vec<(ImageId, String, bool)>>>,
    viewer: ImageViewer,
    image_state: ImageState,
    last_drag_start: Option<(usize, usize)>,
    session: SamSession,
    save_job: AsyncRefTask<std::io::Result<()>>,
    mask_generator: MaskGenerator,
}

enum ImageState {
    NotLoaded,
    LoadingImageData(AsyncTask<io::Result<ImageData>>),
    Loaded(ImageStateLoaded),
    Error(String),
}
struct ImageStateLoaded {
    id: ImageId,
    _texture: TextureHandle,
    original_image: Arc<DynamicImage>,
    masks: MaskImage,
    embeddings: AsyncRefTask<Result<SamEmbeddings, InferenceError>>,
}

const ICON_SAM: &str = "\u{2728}";
const ICON_RELOAD: &str = "\u{21BB}";
const ICON_PREV_ANNOTATED: &str = "\u{23EA}";
const ICON_NEXT_ANNOTATED: &str = "\u{23E9}";
const ICON_PREV: &str = "\u{23F4}";
const ICON_NEXT: &str = "\u{23F5}";
const ICON_SAVE: &str = "\u{1F4BE}";

impl ImageViewerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        storage: Storage,
        session: SamSession,
        mask_generator: MaskGenerator,
    ) -> Self {
        let urls = AsyncRefTask::new(storage.list_images());

        Self {
            storage,
            url_idx: 0,
            urls,
            image_state: ImageState::NotLoaded,
            viewer: ImageViewer::new(vec![]),
            last_drag_start: None,
            session,
            save_job: AsyncRefTask::new_ready(Ok(())),
            mask_generator,
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
                        let is_image_dirty = matches!(
                            &self.image_state,
                            ImageState::Loaded(ImageStateLoaded {
                                masks,
                                ..
                            }) if masks.is_dirty()
                        );

                        if !is_image_dirty {
                            if ui
                                .button(ICON_PREV_ANNOTATED)
                                .on_hover_text("Previous annotated (shift + ArrowLeft)")
                                .clicked()
                                || ui.input(|i| i.key_pressed(Key::ArrowLeft) && i.modifiers.shift)
                            {
                                let start_idx = self.url_idx;
                                loop {
                                    let next_idx =
                                        (self.url_idx.checked_sub(1)).unwrap_or(urls.len() - 1);
                                    self.url_idx = next_idx;

                                    if urls[next_idx].2 || self.url_idx == start_idx {
                                        break;
                                    }
                                }

                                self.image_state = ImageState::NotLoaded;
                            }
                            if ui
                                .button(ICON_PREV)
                                .on_hover_text("Previous (ArrowLeft)")
                                .clicked()
                                || ui.input(|i| i.key_pressed(Key::ArrowLeft) && !i.modifiers.shift)
                            {
                                self.url_idx =
                                    (self.url_idx.checked_sub(1)).unwrap_or(urls.len() - 1);
                                self.image_state = ImageState::NotLoaded;
                            }
                        }

                        if ComboBox::from_id_salt("url_selector")
                            .show_index(ui, &mut self.url_idx, urls.len(), |x| {
                                urls.get(x).map(|x| x.1.as_str()).unwrap_or("")
                            })
                            .changed()
                        {
                            self.image_state = ImageState::NotLoaded;
                        }
                        if ui
                            .button(ICON_RELOAD)
                            .on_hover_text("Reload Files")
                            .clicked()
                        {
                            reload_images = true;
                        }
                        if !is_image_dirty {
                            if ui
                                .button(ICON_NEXT)
                                .on_hover_text("Next (ArrowRight)")
                                .clicked()
                                || ui
                                    .input(|i| i.key_pressed(Key::ArrowRight) && !i.modifiers.shift)
                            {
                                self.url_idx = (self.url_idx + 1) % urls.len();
                                self.image_state = ImageState::NotLoaded;
                            }
                            if ui
                                .button(ICON_NEXT_ANNOTATED)
                                .on_hover_text("Next annotated (Shift + ArrowRight)")
                                .clicked()
                                || ui.input(|i| i.key_pressed(Key::ArrowRight) && i.modifiers.shift)
                            {
                                let start_idx = self.url_idx;
                                loop {
                                    let next_idx = (self.url_idx + 1) % urls.len();
                                    self.url_idx = next_idx;

                                    if urls[next_idx].2 || self.url_idx == start_idx {
                                        break;
                                    }
                                }

                                self.image_state = ImageState::NotLoaded;
                            }
                        }

                        match (self.save_job.data(), &mut self.image_state) {
                            (
                                Some(last_save),
                                ImageState::Loaded(ImageStateLoaded {
                                    id,
                                    masks,
                                    original_image,
                                    ..
                                }),
                            ) => {
                                if let Err(e) = last_save {
                                    ui.label(format!("Error during save: {e}"));
                                }
                                if is_image_dirty {
                                    if ui
                                        .button(ICON_SAVE)
                                        .on_hover_text("Save (cmd + S)")
                                        .clicked()
                                        || ui
                                            .input(|i| i.modifiers.command && i.key_pressed(Key::S))
                                    {
                                        masks.mark_not_dirty();
                                        self.save_job = AsyncRefTask::new(
                                            self.storage
                                                .store_masks(id.clone(), masks.subgroups())
                                                .boxed(),
                                        );
                                    }
                                }

                                if ui.button("Reset").clicked() {
                                    masks.reset();
                                }

                                if let Some(x) = self.mask_generator.ui(&original_image, ui) {
                                    println!("Add {} groups", x.len());
                                    for group in x {
                                        masks.add_subgroup(("annotation".into(), group));
                                    }
                                }
                            }
                            _ => {}
                        }
                    });
                    if let Some((image_id, _, _)) = urls.get(self.url_idx) {
                        match &mut self.image_state {
                            ImageState::NotLoaded => {
                                self.image_state = ImageState::LoadingImageData(AsyncTask::new(
                                    self.storage.load_image(image_id),
                                ))
                            }
                            ImageState::LoadingImageData(t) => {
                                if let Some(x) = t.data() {
                                    self.image_state = match x {
                                        Ok(i) => {
                                            let handle = ctx.load_texture(
                                                "Overlays",
                                                ColorImage {
                                                    size: [
                                                        i.adjust_image.width() as _,
                                                        i.adjust_image.height() as _,
                                                    ],
                                                    pixels: i
                                                        .adjust_image
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
                                            self.viewer.reset();
                                            self.viewer.sources =
                                                vec![ImageSource::Texture(texture)];
                                            let embeddings = self
                                                .session
                                                .get_image_embeddings(i.adjust_image.clone())
                                                .boxed();

                                            let x = i.adjust_image.width() as usize;
                                            let y = i.adjust_image.height() as usize;

                                            ImageState::Loaded(ImageStateLoaded {
                                                id: i.id,
                                                original_image: i.original_image,
                                                _texture: handle,
                                                masks: MaskImage::new(
                                                    [x, y],
                                                    i.masks.clone(),
                                                    Default::default(),
                                                ),
                                                embeddings: AsyncRefTask::new(embeddings),
                                            })
                                        }
                                        Err(e) => ImageState::Error(format!("Error: {e}")),
                                    }
                                }
                            }
                            ImageState::Loaded(ImageStateLoaded { masks, .. }) => {
                                self.viewer.sources.truncate(1);
                                if let Some(x) = masks.ui_events(ui) {
                                    self.viewer.sources.push(ImageSource::Texture(x));
                                }
                            }
                            ImageState::Error(error) => {
                                ui.label(format!("Error: {error}"));
                            }
                        }
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
                Some(&(cursor_x, cursor_y)),
                ImageState::Loaded(ImageStateLoaded {
                    masks, embeddings, ..
                }),
                Some(&(start_x, start_y)),
                true,
            ) = (
                cursor_image_pos.as_ref(),
                &mut self.image_state,
                self.last_drag_start.as_ref(),
                response.drag_stopped() && !ui.input(|i| i.modifiers.command || i.modifiers.ctrl),
            ) {
                if let Some(Ok(loaded_embeddings)) = embeddings.data() {
                    let new_mask = self
                        .session
                        .decode_prompt(
                            cursor_x.min(start_x) as f32,
                            cursor_y.min(start_y) as f32,
                            cursor_x.max(start_x) as f32,
                            cursor_y.max(start_y) as f32,
                            loaded_embeddings,
                        )
                        .unwrap();

                    masks.add_subgroup(("New group".into(), new_mask));
                    if let Some(Ok(x)) = self.urls.data() {
                        x[self.url_idx].2 = true;
                    } else {
                        warn!("Couldn't mark URL as containing masks")
                    }

                    self.last_drag_start = None;
                }
            }

            // Zoom level display
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
