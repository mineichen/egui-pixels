use std::{pin::Pin, sync::Arc};

mod async_task;
#[cfg(all(feature = "ffi", target_arch = "wasm32"))]
mod ffi;
mod image_state;
mod image_utils;
mod mask;
mod sub_group;
mod tool;
mod viewer;

pub use async_task::*;
#[cfg(all(feature = "ffi", target_arch = "wasm32"))]
pub use ffi::*;
pub use image_state::*;
pub use image_utils::*;
pub use mask::*;
pub use sub_group::*;
pub use tool::*;
pub use viewer::*;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(PartialEq, Clone, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct ImageId(Arc<str>);

impl<'a> From<&'a str> for ImageId {
    fn from(s: &'a str) -> Self {
        Self(Arc::from(s))
    }
}

impl std::ops::Deref for ImageId {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub struct ImageData {
    pub id: ImageId,
    pub image: ImageLoadOk,
    pub masks: Vec<Annotation>,
}

impl ImageData {
    pub fn chessboard() -> impl Iterator<Item = Self> {
        (0..2).map(|i| ImageData {
            id: ImageId::from(format!("image{}", i + 1).as_str()),
            masks: vec![],
            image: {
                let width = 400;
                let height = 400;
                let square_size = width / 8;
                let (color_1, color_2) = if i == 0 { (0, 255) } else { (255, 0) };

                let image_data = (0..height)
                    .flat_map(|y| {
                        (0..width).flat_map(move |x| {
                            let square_x = x / square_size;
                            let square_y = y / square_size;
                            let is_white = (square_x + square_y) % 2 == 0;
                            let color = if is_white { color_1 } else { color_2 };
                            [color, color, color]
                        })
                    })
                    .collect::<Vec<_>>();
                let rgb_image =
                    image::RgbImage::from_raw(width as _, height as _, image_data).unwrap();
                let dyn_img = Arc::new(image::DynamicImage::ImageRgb8(rgb_image));
                ImageLoadOk {
                    adjust: dyn_img.clone(),
                    original: dyn_img,
                }
            },
        })
    }
}

#[derive(Debug)]
pub struct ImageListTaskItem {
    pub id: ImageId,
    pub name: String,
    pub has_masks: bool,
}
