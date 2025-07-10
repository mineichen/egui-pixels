use std::io;

use crate::{
    async_task::{AsyncRefTask, AsyncTask},
    image_utils::ImageLoadOk,
    storage::{ImageData, ImageId, Storage},
};
use egui::{
    self, Color32, ColorImage, ImageSource, InnerResponse, Sense, TextureHandle, TextureOptions,
    UiBuilder, load::SizedTexture,
};

use egui_pixels::{ImageViewer, ImageViewerInteraction, MaskImage};
use image::GenericImageView;
use image_selector::ImageSelector;
use tools::Tools;

mod config;
mod image_selector;
mod mask_generator;
mod menu;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod tools;
#[cfg(target_arch = "wasm32")]
mod web;

pub(crate) use config::Config;
pub(crate) use mask_generator::MaskGenerator;
#[cfg(not(target_arch = "wasm32"))]
pub use native::run_native;
#[cfg(target_arch = "wasm32")]
pub use web::run_web;

pub(crate) struct ImageViewerApp {
    storage: Box<dyn Storage>,
    selector: ImageSelector,
    viewer: ImageViewer,
    image_state: ImageState,
    tools: Tools,
    save_job: AsyncRefTask<std::io::Result<()>>,
    mask_generator: MaskGenerator,
}

#[allow(clippy::large_enum_variant)]
enum ImageState {
    NotLoaded,
    LoadingImageData(AsyncTask<io::Result<ImageData>>),
    Loaded(ImageStateLoaded),
    Error(String),
}

impl ImageState {
    fn sources(&mut self, ctx: &egui::Context) -> impl Iterator<Item = ImageSource<'static>> + '_ {
        match self {
            ImageState::Loaded(x) => itertools::Either::Left(
                std::iter::once(x.texture.1.clone()).chain(x.masks.sources(ctx)),
            ),
            _ => itertools::Either::Right(std::iter::empty()),
        }
    }

    pub fn update(
        &mut self,
        ctx: &egui::Context,
        mut on_image_load: impl FnMut(&ImageLoadOk),
        image_id: &mut ImageId,
        storage: &dyn Storage,
    ) {
        match self {
            ImageState::NotLoaded => {
                *self = ImageState::LoadingImageData(AsyncTask::new(storage.load_image(image_id)))
            }
            ImageState::LoadingImageData(t) => {
                if let Some(image_data_result) = t.data() {
                    *self = match image_data_result {
                        Ok(i) => {
                            let handle = ctx.load_texture(
                                "Overlays",
                                ColorImage {
                                    size: [
                                        i.image.adjust.width() as _,
                                        i.image.adjust.height() as _,
                                    ],
                                    pixels: i
                                        .image
                                        .adjust
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
                            on_image_load(&i.image);

                            let source = ImageSource::Texture(texture);
                            let x = i.image.adjust.width() as usize;
                            let y = i.image.adjust.height() as usize;

                            ImageState::Loaded(ImageStateLoaded {
                                id: i.id,
                                image: i.image,
                                texture: (handle, source),
                                masks: MaskImage::new([x, y], i.masks.clone(), Default::default()),
                            })
                        }
                        Err(e) => ImageState::Error(format!("Error: {e}")),
                    }
                }
            }
            ImageState::Loaded(ImageStateLoaded { masks, .. }) => {
                masks.handle_events(ctx);
            }
            ImageState::Error(_error) => {}
        }
    }
}

struct ImageStateLoaded {
    id: ImageId,
    #[allow(
        dead_code,
        reason = "Acts as Strong reference for SizedTexture. SizedTexture would not render a image if TextureHandle is dropped"
    )]
    texture: (TextureHandle, ImageSource<'static>),
    image: ImageLoadOk,
    masks: MaskImage,
}

impl ImageViewerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        storage: Box<dyn Storage>,
        tools: Tools,
        mask_generator: MaskGenerator,
    ) -> Self {
        let url_loader = Some(AsyncTask::new(storage.list_images()));
        Self {
            storage,
            selector: ImageSelector::new(url_loader),
            image_state: ImageState::NotLoaded,
            viewer: ImageViewer::default(),
            tools,
            save_job: AsyncRefTask::new_ready(Ok(())),
            mask_generator,
        }
    }

    fn handle_image_transition(&mut self, ctx: &egui::Context) {
        if let Some((image_id, _, _)) = self.selector.current() {
            let on_image_load = |i: &ImageLoadOk| {
                self.viewer.reset();
                self.tools.load_tool(&i);
            };
            self.image_state
                .update(ctx, on_image_load, image_id, self.storage.as_ref());
        }
    }
}

impl eframe::App for ImageViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Image pixel selector");
            self.menu_ui(ui);
            self.handle_image_transition(ui.ctx());

            if let InnerResponse {
                inner:
                    Some(ImageViewerInteraction {
                        original_image_size,
                        cursor_image_pos,
                    }),
                response,
            } = ui.reserve_bottom_space(80., |ui| {
                self.viewer
                    .ui_meta(ui, self.image_state.sources(ui.ctx()), Some(Sense::click()))
            }) {
                if let Some(cursor_image_pos) = cursor_image_pos {
                    self.handle_interaction(response, cursor_image_pos, ui.ctx());
                }
                ui.label(format!(
                    "Original Size: ({original_image_size:?}), \navail: {:?}, \nspacing: {:?}",
                    original_image_size,
                    ui.spacing().item_spacing
                ));

                if let Some((x, y)) = cursor_image_pos {
                    ui.label(format!("Pixel Coordinates: ({}, {})", x, y,));
                }
            }
        });
    }
}

trait UiExt {
    fn reserve_bottom_space<T>(&mut self, size: f32, inner: impl FnOnce(&mut egui::Ui) -> T) -> T;
}

impl UiExt for egui::Ui {
    fn reserve_bottom_space<T>(&mut self, size: f32, inner: impl FnOnce(&mut egui::Ui) -> T) -> T {
        let mut available = self.available_rect_before_wrap();
        available.max.y = (available.max.y - size).max(0.);

        let r = self.allocate_new_ui(UiBuilder::new().max_rect(available), inner);

        let InnerResponse { inner, .. } = r;
        inner
    }
}
