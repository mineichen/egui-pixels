use crate::storage::Storage;
use egui::{self, InnerResponse, Sense, UiBuilder};
use egui_pixels::{
    AsyncRefTask, AsyncTask, CursorImage, CursorImageSystem, ImageLoadOk, ImageViewerInteraction,
    State, Tools,
};

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
    pub cursor_image: CursorImageSystem,
}

// https://www.svgrepo.com/svg/437030/lasso
const UGLY_POINTER_IMAGE: CursorImage = CursorImage {
    bytes: "iVBORw0KGgoAAAANSUhEUgAAABoAAAAaCAYAAACpSkzOAAABhWlDQ1BJQ0MgcHJvZmlsZQAAKJF9kT1Iw0AcxV9bpUWqDnZQcchQneyiooJLqWIRLJS2QqsOJpd+QZOGJMXFUXAtOPixWHVwcdbVwVUQBD9A3AUnRRcp8X9JoUWMB8f9eHfvcfcO8DYqTDG6ooCimnoqHhOyuVXB/4oAhtCHGcyJzNAS6cUMXMfXPTx8vYvwLPdzf45eOW8wwCMQR5mmm8QbxNObpsZ5nzjESqJMfE48rtMFiR+5Ljn8xrlos5dnhvRMap44RCwUO1jqYFbSFeIp4rCsqJTvzTosc97irFRqrHVP/sJgXl1Jc53mCOJYQgJJCJBQQxkVmIjQqpJiIEX7MRf/sO1PkksiVxmMHAuoQoFo+8H/4He3RmFywkkKxoDuF8v6GAX8u0Czblnfx5bVPAF8z8CV2vZXG8DsJ+n1thY+Avq3gYvrtibtAZc7wOCTJuqiLfloegsF4P2MvikHDNwCPWtOb619nD4AGepq+QY4OATGipS97vLuQGdv/55p9fcDUmtzAIjlR5QAAAAGYktHRAAAAAAAAPlDu38AAAAJcEhZcwAADdcAAA3XAUIom3gAAAAHdElNRQfpCBkPAB2IpJjaAAABr0lEQVRIx+3WPUiVYRQH8F8ZDYFGViDUZJlLNCW4REtJRBASZGtTe5CLLg1BH9DY0ORtdmhoKkSyxCLRagiKHHKIiEtGF0HNbi0neLB73/vxXofAAy/n5fA/n895/8/LljQp2+rEdeMkjqMLHSjhK+bwFO/zFHIWUyjjd43nJS422lEHxjCY2N5hBp/wA+04iH4cS2JN4BKKtbrYjTdRZRkFHK3hcwT38DP8PsaIM2U8wEsYaHDU/fgS/pNZO9AXoF841eS59mE14pypBrobgEc5t3ks4hT+GrZvAPSEfpYz0VRydhUTrYfemzNRe+jlaolehT6PnTlIYCje56uBDmAl5juLb/iOJzhRZ6JryUL1ZAFHq3z5azid4bcLdxL89VoV3QzgAs5FJ4/D9qLCmHoxgsUkyf0Kx/KPFAL8ILFdCdt6sMZzvI3Rpl0XA1uXDCWOs8HMaxlkWsZrXMWeRkn1BoaxI7EVg507Y/1LQTfzsTBNywCmk8onWnkR7sMtfN4wnlIQZkvkUIUEH4L+D7eym4cRfBGXsX+z/h8WItGFzQjelrx3Rhe346r+P+UPJi6EyWu6XtcAAAAASUVORK5CYII=",
    offset_x: 10,
    offset_y: 10,
};
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
            cursor_image: CursorImageSystem::from(Box::new(|_: Option<&CursorImage>| {})),
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

            if let InnerResponse {
                inner:
                    Some(ImageViewerInteraction {
                        original_image_size,
                        cursor_image_pos,
                        tool_painter,
                    }),
                response,
            } = ui.reserve_bottom_space(80., |ui| {
                self.state.viewer.ui(
                    ui,
                    self.state.image_state.sources(ui.ctx()),
                    Some(Sense::click()),
                )
            }) {
                if cursor_image_pos.is_some() {
                    self.state
                        .handle_tool_interaction(response, ui.ctx(), tool_painter);
                }
                ui.label(format!(
                    "Original Size: ({original_image_size:?}), \navail: {:?}, \nspacing: {:?}",
                    original_image_size,
                    ui.spacing().item_spacing
                ));

                if let Some((x, y)) = cursor_image_pos {
                    ui.label(format!("Pixel Coordinates: ({}, {})", x, y,));
                    self.cursor_image.set(UGLY_POINTER_IMAGE);
                }
                self.cursor_image.apply();
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
