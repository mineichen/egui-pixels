use std::{
    io::{self},
    str::FromStr,
    sync::Arc,
};

use futures::future::BoxFuture;

use crate::{image_utils::ImageLoadOk, SubGroups};

#[cfg(not(target_arch = "wasm32"))]
pub mod file;
pub mod in_memory;

#[derive(PartialEq, Clone, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct ImageId(Arc<str>);

pub struct ImageData {
    pub id: ImageId,
    pub image: ImageLoadOk,
    pub masks: Vec<SubGroups>,
}

const PREAMBLE: [u8; 5] = [b'a', b'n', b'n', b'o', b't'];
const VERSION: u16 = 1;

pub trait Storage {
    fn list_images(&self) -> BoxFuture<'static, std::io::Result<Vec<(ImageId, String, bool)>>>;
    fn load_image(&self, id: &ImageId) -> BoxFuture<'static, std::io::Result<ImageData>>;
    fn store_masks(&self, id: ImageId, masks: Vec<SubGroups>)
        -> BoxFuture<'static, io::Result<()>>;
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
