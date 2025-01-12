use std::{io, sync::Arc};

use crate::{
    async_task::{AsyncRefTask, AsyncTask},
    inference::{InferenceError, SamEmbeddings, SamSession},
    mask::MaskImage,
    mask_generator::MaskGenerator,
    storage::{ImageData, ImageId, Storage},
    url_state::UrlState,
    viewer::{ImageViewer, ImageViewerInteraction},
};
use eframe::egui::{self, InnerResponse, Sense, TextureHandle, UiBuilder};
use image::DynamicImage;
use log::warn;

mod menu;

pub(crate) struct ImageViewerApp {
    storage: Storage,
    url: UrlState,
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

impl ImageViewerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        storage: Storage,
        session: SamSession,
        mask_generator: MaskGenerator,
    ) -> Self {
        let url_loader = Some(AsyncTask::new(storage.list_images()));
        Self {
            storage,
            url: UrlState::new(url_loader),
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
            self.menu(ui);

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

                    if let Some((_, _, loaded)) = self.url.current() {
                        *loaded = true;
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
