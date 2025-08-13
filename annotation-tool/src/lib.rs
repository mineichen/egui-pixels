use egui_pixels::{PixelArea, PixelRange};
use image::DynamicImage;

mod app;
mod config;
mod storage;

#[cfg(not(target_arch = "wasm32"))]
pub use app::run_native;
#[cfg(target_arch = "wasm32")]
pub use app::run_web;

#[cfg(not(target_arch = "wasm32"))]
pub use storage::file::FileStorage;
#[cfg(target_arch = "wasm32")]
pub use storage::in_memory::InMemoryStorage;

type ImageCallbackMap = Vec<(String, Box<dyn Fn(&DynamicImage) -> Vec<PixelArea>>)>;
