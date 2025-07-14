use std::{pin::Pin, sync::Arc};

mod async_task;
mod image_state;
mod image_utils;
mod mask;
mod sub_group;
mod tool;
mod viewer;

pub use async_task::*;
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
    pub masks: Vec<SubGroups>,
}

#[derive(Debug)]
pub struct ImageListTaskItem {
    pub id: ImageId,
    pub name: String,
    pub has_masks: bool,
}
