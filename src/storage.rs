use std::{
    io::{self, ErrorKind, Write},
    num::NonZeroU16,
    ops::Deref,
    path::PathBuf,
    sync::Arc,
};

use futures::{future::BoxFuture, FutureExt};
use image::DynamicImage;

use crate::Annotation;

pub struct Storage {
    base: String,
}

#[derive(PartialEq, Clone)]
pub struct ImageId(Arc<str>);

pub struct ImageData {
    pub id: ImageId,
    pub adjust_image: Arc<DynamicImage>,
    pub masks: Vec<Annotation>,
}

const PREAMBLE: [u8; 11] = [
    b'a', b'n', b'n', b'o', b't', b'a', b't', b'i', b'o', b'n', b's',
];
const VERSION: u16 = 1;

impl Storage {
    pub fn new(base: impl Into<String>) -> Self {
        Self { base: base.into() }
    }
    // uri -> Display
    pub fn list_images(&self) -> BoxFuture<'static, std::io::Result<Vec<(ImageId, String)>>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        let image_path = self.get_image_path();

        std::thread::spawn(|| {
            let r = Self::list_images_blocking(image_path);
            tx.send(r)
        });
        async move { rx.await.map_err(std::io::Error::other).and_then(|a| a) }.boxed()
    }

    pub fn load_image(&self, id: &ImageId) -> BoxFuture<'static, std::io::Result<ImageData>> {
        let id = id.clone();
        async move {
            let image_bytes = std::fs::read(id.0.deref())?;
            let mask_path = Self::get_mask_path(&id)?;

            let adjust_image = Arc::new(crate::image_utils::load_image(&image_bytes)?);
            let masks = match std::fs::read(mask_path) {
                Ok(data) => {
                    if data.get(0..PREAMBLE.len()) != Some(&PREAMBLE) {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Invalid preamble",
                        ));
                    }
                    assert_eq!(
                        Some(VERSION),
                        data.get(PREAMBLE.len()..PREAMBLE.len() + 2)
                            .and_then(|bytes| Some(u16::from_le_bytes(bytes.try_into().ok()?)))
                    );
                    let stored: StoredData = bincode::deserialize(&data[PREAMBLE.len() + 2..])
                        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;

                    stored
                        .masks
                        .into_iter()
                        .map(|d| ("foo".to_string(), d))
                        .collect::<Vec<_>>()
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
                Err(e) => return Err(e),
            };

            Ok(ImageData {
                id,
                masks,
                adjust_image,
            })
        }
        .boxed()
    }

    pub fn store_masks<'a>(
        &self,
        id: ImageId,
        masks: impl Iterator<Item = &'a Vec<(u32, NonZeroU16)>>,
    ) -> BoxFuture<'static, io::Result<()>> {
        //let file = std::fs::File::open(path)
        let masks = masks.cloned().collect::<Vec<_>>();
        let path = Self::get_mask_path(&id);

        async move {
            println!("Store at: {path:?}");
            let path = path?;
            if masks.is_empty() {
                match std::fs::remove_file(path) {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            } else {
                let mut f = std::fs::File::create(path)?;
                f.write_all(&PREAMBLE)?;
                f.write_all(&VERSION.to_le_bytes())?;
                bincode::serialize_into(f, &StoredData { masks }).map_err(std::io::Error::other)?;
            }
            Ok(())
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
    fn get_image_path(&self) -> PathBuf {
        self.base.as_str().into()
    }

    fn get_mask_path(id: &ImageId) -> std::io::Result<PathBuf> {
        let file_path = std::path::Path::new(id.0.deref());

        let filename = file_path
            .file_stem()
            .and_then(|x| x.to_str())
            .ok_or_else(|| std::io::Error::other("File has no filename"))?;
        let images_path = file_path
            .parent()
            .ok_or_else(|| std::io::Error::other("Base musten't be a root-dir"))?;

        Ok(images_path.join(format!("{filename}.masks")))
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct StoredData {
    masks: Vec<Vec<(u32, NonZeroU16)>>,
}
