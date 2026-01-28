use std::{io, num::NonZeroU32};

use egui::{
    self, Color32, ColorImage, ImageSource, TextureHandle, TextureOptions, load::SizedTexture,
};

use crate::{AsyncTask, BoxFuture, ImageData, ImageId, ImageLoadOk, MaskImage};

#[allow(clippy::large_enum_variant)]
pub enum ImageState {
    NotLoaded,
    LoadingImageData(AsyncTask<io::Result<ImageData>>),
    Loaded(ImageStateLoaded),
    Error(String),
}

impl ImageState {
    pub fn sources(
        &mut self,
        ctx: &egui::Context,
    ) -> impl Iterator<Item = ImageSource<'static>> + '_ {
        match self {
            ImageState::Loaded(x) => itertools::Either::Left(x.sources(ctx)),
            _ => itertools::Either::Right(std::iter::empty()),
        }
    }

    pub fn update(
        &mut self,
        ctx: &egui::Context,
        mut on_image_load: impl FnMut(&ImageLoadOk),
        image_loader: &dyn Fn() -> BoxFuture<'static, io::Result<ImageData>>,
    ) {
        match self {
            ImageState::NotLoaded => {
                *self = ImageState::LoadingImageData(AsyncTask::new(image_loader()))
            }
            ImageState::LoadingImageData(t) => {
                if let Some(image_data_result) = t.data() {
                    *self = match image_data_result
                        .map_err(|e| format!("IO Error: {}", e))
                        .and_then(|i| {
                            ImageStateLoaded::from_image_data(i, ctx).map_err(|e| e.to_string())
                        }) {
                        Ok(loaded) => {
                            on_image_load(&loaded.image);
                            ImageState::Loaded(loaded)
                        }
                        Err(e) => ImageState::Error(e),
                    }
                }
            }
            ImageState::Loaded(ImageStateLoaded { masks, .. }) => {
                masks.handle_events(ctx);
            }
            ImageState::Error(_error) => {}
        }
    }
}

impl From<ImageStateLoaded> for ImageState {
    fn from(value: ImageStateLoaded) -> Self {
        Self::Loaded(value)
    }
}

impl ImageStateLoaded {
    pub fn from_image_data(i: ImageData, ctx: &egui::Context) -> Result<Self, TextureExceedsLimit> {
        let (width, height) = i.image.adjust.dimensions();
        let max_texture_side = ctx.input(|i| i.max_texture_side);
        if width.get() as usize > max_texture_side || height.get() as usize > max_texture_side {
            return Err(TextureExceedsLimit::new(width, height, max_texture_side));
        }
        let handle = ctx.load_texture(
            "Overlays",
            ColorImage::new(
                [width.get() as _, height.get() as _],
                i.image
                    .adjust_pixels()
                    .map(|(_, _, [r, g, b])| Color32::from_rgb(r, g, b))
                    .collect(),
            ),
            TextureOptions {
                magnification: egui::TextureFilter::Nearest,
                ..Default::default()
            },
        );
        let texture = SizedTexture::from_handle(&handle);

        let source = ImageSource::Texture(texture);
        Ok(ImageStateLoaded {
            id: i.id,
            image: i.image,
            texture: (handle, source),
            masks: MaskImage::new(
                [width.get() as usize, height.get() as usize],
                i.masks.clone(),
                Default::default(),
            ),
        })
    }
}

pub struct ImageStateLoaded {
    pub id: ImageId,
    #[allow(
        dead_code,
        reason = "Acts as Strong reference for SizedTexture. SizedTexture would not render a image if TextureHandle is dropped"
    )]
    pub texture: (TextureHandle, ImageSource<'static>),
    pub image: ImageLoadOk,
    pub masks: MaskImage,
}

impl ImageStateLoaded {
    pub fn sources(
        &mut self,
        ctx: &egui::Context,
    ) -> impl Iterator<Item = ImageSource<'static>> + '_ {
        std::iter::once(self.texture.1.clone()).chain(self.masks.sources(ctx))
    }
}

#[derive(Debug, thiserror::Error)]
#[error(
    "Image too large: {}x{}, max texture side is {}",
    width,
    height,
    max_texture_side
)]
pub struct TextureExceedsLimit {
    width: NonZeroU32,
    height: NonZeroU32,
    max_texture_side: usize,
}

impl TextureExceedsLimit {
    pub fn new(width: NonZeroU32, height: NonZeroU32, max_texture_side: usize) -> Self {
        Self {
            width,
            height,
            max_texture_side,
        }
    }
}
