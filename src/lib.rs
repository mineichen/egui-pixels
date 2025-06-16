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
pub struct SubGroup {
    position: u32,
    length: NonZeroU16,
    opacity: u8,
}

impl SubGroup {
    pub fn new(position: u32, length: NonZeroU16, opacity: u8) -> Self {
        Self {
            position,
            length,
            opacity,
        }
    }

    pub fn new_opaque(position: u32, length: NonZeroU16) -> Self {
        Self {
            position,
            length,
            opacity: 255,
        }
    }

    pub fn as_range(&self) -> std::ops::Range<usize> {
        let start = self.position as usize;
        let end = start + self.length.get() as usize;
        start..end
    }

    pub fn end_position(&self) -> u32 {
        self.position + self.length.get() as u32
    }
}

type SubGroups = Vec<SubGroup>;
type Annotation = (String, SubGroups);

pub use app::run_native;
pub use storage::Storage;

type ImageCallbackMap = Vec<(String, Box<dyn Fn(&DynamicImage) -> Vec<SubGroups>>)>;
