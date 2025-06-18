use futures::{future::BoxFuture, FutureExt};
use std::{
    io,
    sync::{Arc, Mutex},
};
use wasm_bindgen::JsCast;
use web_sys::{window, FileList};

use super::{ImageData, ImageId, Storage};
use crate::{image_utils::ImageLoadOk, SubGroups};

pub struct InMemoryStorage {
    masks: Arc<Mutex<Vec<SubGroups>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            masks: Default::default(),
        }
    }
}

impl Storage for InMemoryStorage {
    fn list_images(&self) -> BoxFuture<'static, io::Result<Vec<(ImageId, String, bool)>>> {
        async move { Ok(vec![(ImageId("test".into()), "Text".into(), true)]) }.boxed()
    }

    fn load_image(&self, id: &ImageId) -> BoxFuture<'static, io::Result<ImageData>> {
        let id = id.clone();
        async move {
            let width = 400;
            let height = 400;
            let square_size = width / 8;
            let mut image_data = vec![0u8; width * height * 3];

            for y in 0..height {
                for x in 0..width {
                    let square_x = x / square_size;
                    let square_y = y / square_size;
                    let is_white = (square_x + square_y) % 2 == 0;

                    let idx = (y * width + x) * 3;
                    let color = if is_white { 255 } else { 0 };

                    image_data[idx] = color;
                    image_data[idx + 1] = color;
                    image_data[idx + 2] = color;
                }
            }
            let rgb_image = image::RgbImage::from_raw(width as _, height as _, image_data).unwrap();
            let dyn_img = Arc::new(image::DynamicImage::ImageRgb8(rgb_image));

            Ok(ImageData {
                id,
                masks: vec![],
                image: ImageLoadOk {
                    adjust: dyn_img.clone(),
                    original: dyn_img,
                },
            })
        }
        .boxed()
    }

    fn store_masks(
        &self,
        _id: ImageId,
        _masks: Vec<SubGroups>,
    ) -> BoxFuture<'static, io::Result<()>> {
        async move {
            // Implement storing masks to web storage or IndexedDB
            Err(io::Error::new(io::ErrorKind::Other, "Not implemented"))
        }
        .boxed()
    }
}
