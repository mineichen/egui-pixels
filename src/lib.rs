use std::num::NonZeroU16;

use image::DynamicImage;

mod app;
mod async_task;
mod config;
mod image_utils;
mod inference;
mod mask;
mod storage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubGroup(u32, NonZeroU16);
type SubGroups = Vec<SubGroup>;
type Annotation = (String, SubGroups);

pub use app::run_native;
pub use storage::Storage;

type ImageCallbackMap = Vec<(String, Box<dyn Fn(&DynamicImage) -> Vec<SubGroups>>)>;
