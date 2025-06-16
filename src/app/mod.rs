use std::io;

use crate::{
    async_task::{AsyncRefTask, AsyncTask},
    image_utils::ImageLoadOk,
    mask::MaskImage,
    storage::{ImageData, ImageId, Storage},
};
use eframe::egui::{self, InnerResponse, Sense, TextureHandle, UiBuilder};

use image_selector::ImageSelector;
use tools::Tools;
use viewer::{ImageViewer, ImageViewerInteraction};

mod config;
mod image_selector;
mod mask_generator;
mod menu;
mod native;
mod tools;
mod viewer;

pub(crate) use config::Config;
pub(crate) use mask_generator::MaskGenerator;
pub use native::run_native;

pub(crate) struct ImageViewerApp {
    storage: Storage,
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
struct ImageStateLoaded {
    id: ImageId,
    #[allow(
        dead_code,
        reason = "Acts as Strong reference for SizedTexture. SizedTexture would not render a image if TextureHandle is dropped"
    )]
    texture: TextureHandle,
    image: ImageLoadOk,
    masks: MaskImage,
}

impl ImageViewerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        storage: Storage,
        tools: Tools,
        mask_generator: MaskGenerator,
    ) -> Self {
        let url_loader = Some(AsyncTask::new(storage.list_images()));
        Self {
            storage,
            selector: ImageSelector::new(url_loader),
            image_state: ImageState::NotLoaded,
            viewer: ImageViewer::new(vec![]),
            tools,
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

            if let InnerResponse {
                inner:
                    Some(ImageViewerInteraction {
                        original_image_size,
                        cursor_image_pos,
                    }),
                response,
            } = ui.reserve_bottom_space(80., |ui| self.viewer.ui_meta(ui, Some(Sense::click())))
            {
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
