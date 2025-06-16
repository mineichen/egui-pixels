use std::{
    fs::DirEntry,
    io::{self, ErrorKind, Read, Write},
    num::NonZeroU16,
    ops::Deref,
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};

use futures::{future::BoxFuture, FutureExt};
use image::DynamicImage;
use itertools::Itertools;
use log::info;

use crate::{Annotation, SubGroup, SubGroups};

pub struct Storage {
    base: String,
}

#[derive(PartialEq, Clone, Eq, PartialOrd, Ord, Debug)]
pub struct ImageId(Arc<str>);

pub struct ImageData {
    pub id: ImageId,
    pub original_image: Arc<DynamicImage>,
    pub adjust_image: Arc<DynamicImage>,
    pub masks: Vec<Annotation>,
}

const PREAMBLE: [u8; 5] = [b'a', b'n', b'n', b'o', b't'];
const VERSION: u16 = 1;

impl Storage {
    pub fn new(base: impl Into<String>) -> Self {
        Self { base: base.into() }
    }
    // uri -> Display
    pub fn list_images(&self) -> BoxFuture<'static, std::io::Result<Vec<(ImageId, String, bool)>>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        let image_path = self.get_image_path();

        let handle = std::thread::spawn(|| {
            let r = Self::list_images_blocking(image_path);
            tx.send(r)
        });
        async move {
            let r = rx.await.map_err(std::io::Error::other).and_then(|a| a);
            handle.join().unwrap().expect("Channel cant be gone");
            r
        }
        .boxed()
    }

    pub fn load_image(&self, id: &ImageId) -> BoxFuture<'static, std::io::Result<ImageData>> {
        let id = id.clone();
        async move {
            let image_bytes = std::fs::read(id.0.deref())?;
            let mask_path = Self::get_mask_path(&id)?;

            let image_load_result = crate::image_utils::load_image(&image_bytes)?;
            let masks = match std::fs::File::open(mask_path) {
                Ok(mut f) => {
                    let mut preamble = [0; PREAMBLE.len()];
                    f.read_exact(&mut preamble)?;
                    if preamble != PREAMBLE {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Invalid preamble",
                        ));
                    }
                    let mut version_bytes = [0; 2];
                    f.read_exact(&mut version_bytes)?;
                    assert_eq!(VERSION, u16::from_le_bytes(version_bytes));

                    let mut f = brotli::Decompressor::new(f, 4096);
                    let mut sub_groups_bytes = [0; 2];
                    let mut all = Vec::new();
                    let mut starts = Vec::new();
                    let mut lens = Vec::new();

                    while f.read_exact(&mut sub_groups_bytes).is_ok() {
                        let sub_len = u16::from_le_bytes(sub_groups_bytes) as usize;

                        starts.resize(sub_len, 0);
                        lens.resize(sub_len, 0);
                        f.read_exact(bytemuck::cast_slice_mut(&mut starts))?;
                        f.read_exact(bytemuck::cast_slice_mut(&mut lens))?;
                        all.push((
                            "Foo".into(),
                            starts
                                .iter()
                                .zip(lens.iter())
                                .map(|(start, len)| match NonZeroU16::try_from(*len) {
                                    Ok(l) => Ok(SubGroup::new(*start, l)),
                                    Err(e) => Err(std::io::Error::new(
                                        ErrorKind::InvalidData,
                                        format!("position {start},{len}: {e:?}"),
                                    )),
                                })
                                .collect::<Result<Vec<_>, _>>()?,
                        ));
                    }

                    all
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
                Err(e) => return Err(e),
            };

            Ok(ImageData {
                id,
                masks,
                adjust_image: image_load_result.adjust,
                original_image: image_load_result.original,
            })
        }
        .boxed()
    }

    pub fn store_masks(
        &self,
        id: ImageId,
        masks: Vec<SubGroups>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let path = Self::get_mask_path(&id);

        async move {
            info!("Store at: {path:?}");
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

                let mut f = brotli::CompressorWriter::new(f, 4096, 11, 22);
                for sub in masks {
                    if sub.len() > u16::MAX as _ {
                        return Err(std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!(
                                "Version1 allows for MAX {} subgroups, got {}",
                                u16::MAX,
                                sub.len()
                            ),
                        ));
                    }
                    let sub_len = sub.len() as u16;

                    f.write_all(&sub_len.to_le_bytes())?;
                    for subgroup in sub.iter() {
                        f.write_all(&subgroup.position.to_le_bytes())?;
                    }
                    for subgroup in sub {
                        f.write_all(&subgroup.length.get().to_le_bytes())?;
                    }
                }

                f.flush()?;
            }
            Ok(())
        }
        .boxed()
    }

    fn list_images_blocking(path: PathBuf) -> std::io::Result<Vec<(ImageId, String, bool)>> {
        Ok(visit_directory_files(path)
            .filter_map(|x| {
                let x = x.ok()?;
                let path = x.path();
                let kind = path
                    .extension()?
                    .to_str()
                    .and_then(|s| Kind::from_str(s).ok())?;
                Some((
                    path.file_stem()
                        .expect("exists_if_extension_exists")
                        .to_string_lossy()
                        .to_string(),
                    kind,
                    ImageId(path.to_str()?.into()),
                ))
            })
            .sorted_unstable()
            .chunk_by(|x| x.0.to_string()) // Pitty...
            .into_iter()
            .filter_map(|(_, mut members)| {
                let (name, kind, id) = members.next().expect("Needs one item to form a group");
                match (kind, members.next()) {
                    (Kind::Mask, None) => None,
                    (Kind::Mask, Some((_, Kind::Mask, _))) => {
                        unreachable!("Cannot have multiple file_stem.mask")
                    }
                    // Takeing any image is fine, ignore the rest
                    (Kind::Mask, Some((name, Kind::Image, id))) => Some((id, name, true)),
                    (Kind::Image, _) => Some((id, name, false)),
                }
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

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Mask,
    Image,
}

impl FromStr for Kind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jpeg" => Ok(Self::Image),
            "jpg" => Ok(Self::Image),
            "masks" => Ok(Self::Mask),
            "png" => Ok(Self::Image),
            "tiff" => Ok(Self::Image),
            "tif" => Ok(Self::Image),
            _ => Err(()),
        }
    }
}

pub fn visit_directory_files(
    path: impl Into<PathBuf>,
) -> impl Iterator<Item = std::io::Result<DirEntry>> {
    fn one_level(path: PathBuf) -> MaybeOneOrMany<std::io::Result<DirEntry>> {
        match std::fs::read_dir(path) {
            Ok(readdir) => MaybeOneOrMany::Many(Box::new(readdir.flat_map(|entry| match entry {
                Ok(entry) => match entry.file_type() {
                    Ok(filetype) => {
                        if filetype.is_dir() {
                            one_level(entry.path())
                        } else {
                            MaybeOneOrMany::MaybeOne(Some(Ok(entry)))
                        }
                    }
                    Err(e) => MaybeOneOrMany::MaybeOne(Some(Err(e))),
                },
                Err(e) => MaybeOneOrMany::MaybeOne(Some(Err(e))),
            }))),
            Err(e) => MaybeOneOrMany::MaybeOne(Some(Err(e))),
        }
    }
    one_level(path.into())
}

enum MaybeOneOrMany<T> {
    MaybeOne(Option<T>),
    Many(Box<dyn Iterator<Item = T>>),
}

impl<T> Iterator for MaybeOneOrMany<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            MaybeOneOrMany::MaybeOne(x) => x.take(),
            MaybeOneOrMany::Many(iterator) => iterator.next(),
        }
    }
}
