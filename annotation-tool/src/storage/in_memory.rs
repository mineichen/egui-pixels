use egui_pixels::ImageLoadOk;
use futures::{FutureExt, future::BoxFuture};
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
};

use super::{ImageData, ImageId, Storage};
use crate::SubGroups;

#[derive(Default)]
pub struct InMemoryStorage {
    masks: Arc<Mutex<HashMap<ImageId, Vec<SubGroups>>>>,
}

impl Storage for InMemoryStorage {
    fn list_images(&self) -> BoxFuture<'static, io::Result<Vec<(ImageId, String, bool)>>> {
        async move {
            Ok(vec![
                (ImageId("image1".into()), "Image2".into(), true),
                (ImageId("image2".into()), "Image2".into(), true),
            ])
        }
        .boxed()
    }

    fn load_image(&self, id: &ImageId) -> BoxFuture<'static, io::Result<ImageData>> {
        let id = id.clone();
        let masks = self
            .masks
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .unwrap_or_default();
        async move {
            let width = 400;
            let height = 400;
            let square_size = width / 8;
            let mut image_data = vec![0u8; width * height * 3];
            let (color_1, color_2) = if &*id.0 == "image1" {
                (0, 255)
            } else {
                (255, 0)
            };

            for y in 0..height {
                for x in 0..width {
                    let square_x = x / square_size;
                    let square_y = y / square_size;
                    let is_white = (square_x + square_y) % 2 == 0;

                    let idx = (y * width + x) * 3;
                    let color = if is_white { color_1 } else { color_2 };

                    image_data[idx] = color;
                    image_data[idx + 1] = color;
                    image_data[idx + 2] = color;
                }
            }
            let rgb_image = image::RgbImage::from_raw(width as _, height as _, image_data).unwrap();
            let dyn_img = Arc::new(image::DynamicImage::ImageRgb8(rgb_image));

            Ok(ImageData {
                id,
                masks,
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
        id: ImageId,
        masks: Vec<SubGroups>,
    ) -> BoxFuture<'static, io::Result<()>> {
        self.masks.lock().unwrap().insert(id, masks);
        async move {
            // Implement storing masks to web storage or IndexedDB
            Ok(())
        }
        .boxed()
    }
}
