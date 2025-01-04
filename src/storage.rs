use std::{io::BufRead, ops::Deref, path::PathBuf, str::FromStr, sync::Arc};

use futures::{future::BoxFuture, FutureExt};
use image::DynamicImage;

use crate::{Annotation, SubGroups};

pub struct Storage {
    base: PathBuf,
}

pub struct ImageId(Arc<str>);

pub struct ImageData {
    pub id: ImageId,
    pub adjust_image: DynamicImage,
    pub masks: Vec<Annotation>,
}

impl ImageId {
    pub fn uri(&self) -> String {
        format!("file://{}", self.0)
    }
}

impl Storage {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self { base: base.into() }
    }
    // uri -> Display
    pub fn list_images(&self) -> BoxFuture<'static, std::io::Result<Vec<(ImageId, String)>>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        let path = self.base.to_path_buf();
        std::thread::spawn(|| {
            let r = Self::list_images_blocking(path);
            tx.send(r)
        });
        async move { rx.await.map_err(std::io::Error::other).and_then(|a| a) }.boxed()
    }

    pub fn load_image(&self, id: &ImageId) -> BoxFuture<'static, std::io::Result<ImageData>> {
        let id = id.0.clone();
        async move {
            let image_bytes = std::fs::read(id.deref())?;
            let adjust_image = crate::image_utils::load_image(&image_bytes)?;
            let mask_bytes = std::fs::read("masks.csv")?;
            let masks = parse_masks(&mask_bytes);

            Ok(ImageData {
                id: ImageId(id),
                masks,
                adjust_image,
            })
        }
        .boxed()
    }

    fn list_images_blocking(path: PathBuf) -> std::io::Result<Vec<(ImageId, String)>> {
        let files = std::fs::read_dir(path)?;
        Ok(files
            .into_iter()
            .filter_map(|x| {
                let x = x.ok()?;
                Some((
                    ImageId(x.path().to_str()?.into()),
                    x.file_name().to_string_lossy().to_string(),
                ))
            })
            .collect::<Vec<_>>())
    }
}

pub fn parse_masks(bytes: &[u8]) -> Vec<Annotation> {
    bytes
        .lines()
        .filter_map(|x| {
            let s = x.ok()?;
            let mut parts = s.split(';');
            let label = parts.next()?;
            let lines = parts
                .map(|x| {
                    let (start, end) = x.split_once(',')?;
                    Some((u32::from_str(start).ok()?, end.parse().ok()?))
                })
                .collect::<Option<SubGroups>>()?;
            Some((label.into(), lines))
        })
        .collect::<Vec<_>>()
}
