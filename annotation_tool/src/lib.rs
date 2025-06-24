use egui_pixels::{SubGroup, SubGroups};
use image::DynamicImage;

mod app;
mod async_task;
mod config;
mod image_utils;
mod mask;
mod storage;

#[cfg(not(target_arch = "wasm32"))]
pub use app::run_native;
#[cfg(target_arch = "wasm32")]
pub use app::run_web;

#[cfg(not(target_arch = "wasm32"))]
pub use storage::file::FileStorage;
#[cfg(target_arch = "wasm32")]
pub use storage::in_memory::InMemoryStorage;

type ImageCallbackMap = Vec<(String, Box<dyn Fn(&DynamicImage) -> Vec<SubGroups>>)>;
