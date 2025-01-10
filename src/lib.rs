use std::num::NonZeroU16;

mod app;
mod async_task;
mod image_utils;
mod inference;
mod mask;
mod storage;
mod viewer;

type SubGroup = (u32, NonZeroU16);
type SubGroups = Vec<SubGroup>;
type Annotation = (String, SubGroups);

pub use app::ImageViewerApp;
pub use storage::Storage;
