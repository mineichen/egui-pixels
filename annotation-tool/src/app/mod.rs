use crate::storage::Storage;
use egui::{self, InnerResponse, UiBuilder};
use egui_pixels::{AsyncRefTask, AsyncTask, ImageViewerInteraction, State, Tools};

use image_selector::ImageSelector;

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
    state: State,
    save_job: AsyncRefTask<Result<(), String>>,
    mask_generator: MaskGenerator,
}
impl ImageViewerApp {
    pub fn new(storage: Box<dyn Storage>, tools: Tools, mask_generator: MaskGenerator) -> Self {
        let url_loader = Some(AsyncTask::new(storage.list_images()));
        let state = State::new(tools);

        Self {
            storage,
            selector: ImageSelector::new(url_loader),
            state,
            save_job: AsyncRefTask::new_ready(Ok(())),
            mask_generator,
        }
    }

    fn handle_image_transition(&mut self, ctx: &egui::Context) {
        if let Some(x) = self.selector.current() {
            self.state.update(ctx, &|| self.storage.load_image(&x.id));
        }
    }
}

impl eframe::App for ImageViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Image pixel selector");
            self.menu_ui(ui);
            self.handle_image_transition(ui.ctx());

            let response = ui.reserve_bottom_space(80., |ui| self.state.ui(ui));
            if let Some(x) = response.inner {
                let ImageViewerInteraction {
                    original_image_size,
                    cursor_image_pos,
                } = x;
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

        let r = self.scope_builder(UiBuilder::new().max_rect(available), inner);

        let InnerResponse { inner, .. } = r;
        inner
    }
}
