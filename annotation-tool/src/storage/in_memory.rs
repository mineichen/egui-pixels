use egui_pixels::{ImageListTaskItem, ImageLoadOk};
use futures::{FutureExt, future::BoxFuture};
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
};

use super::{ImageData, ImageId, Storage};
use crate::SubGroups;

pub struct InMemoryStorage {
    data: Arc<Mutex<HashMap<ImageId, ImageData>>>,
}

impl InMemoryStorage {
    pub fn chessboard() -> Self {
        Self {
            data: Arc::new(Mutex::new(
                (0..2)
                    .map(|i| {
                        let id = ImageId::from(format!("image{}", i + 1).as_str());
                        (
                            id.clone(),
                            ImageData {
                                id,
                                masks: vec![],
                                image: {
                                    let width = 400;
                                    let height = 400;
                                    let square_size = width / 8;
                                    let mut image_data = vec![0u8; width * height * 3];
                                    let (color_1, color_2) =
                                        if i == 0 { (0, 255) } else { (255, 0) };

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
                                    let rgb_image = image::RgbImage::from_raw(
                                        width as _,
                                        height as _,
                                        image_data,
                                    )
                                    .unwrap();
                                    let dyn_img =
                                        Arc::new(image::DynamicImage::ImageRgb8(rgb_image));
                                    ImageLoadOk {
                                        adjust: dyn_img.clone(),
                                        original: dyn_img,
                                    }
                                },
                            },
                        )
                    })
                    .collect(),
            )),
        }
    }
}

impl Storage for InMemoryStorage {
    fn list_images(&self) -> BoxFuture<'static, io::Result<Vec<ImageListTaskItem>>> {
        let data = self.data.lock().unwrap();
        let result = data
            .iter()
            .map(|(id, data)| {
                let name = id
                    .chars()
                    .enumerate()
                    .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                    .collect::<String>();
                ImageListTaskItem {
                    id: id.clone(),
                    name,
                    has_masks: !data.masks.is_empty(),
                }
            })
            .collect::<Vec<_>>();
        std::future::ready(Ok(result)).boxed()
    }

    fn load_image(&self, id: &ImageId) -> BoxFuture<'static, io::Result<ImageData>> {
        let id = id.clone();
        let data = self
            .data
            .lock()
            .unwrap()
            .get(&id)
            .map(ImageData::clone)
            .ok_or_else(|| std::io::Error::other(format!("Unknown image_id {id:?}")));
        std::future::ready(data).boxed()
    }

    fn store_masks(
        &self,
        id: ImageId,
        masks: Vec<SubGroups>,
    ) -> BoxFuture<'static, io::Result<()>> {
        if let Some(x) = self.data.lock().unwrap().get_mut(&id) {
            x.masks = masks;
        };
        async move {
            // Implement storing masks to web storage or IndexedDB
            Ok(())
        }
        .boxed()
    }
}
