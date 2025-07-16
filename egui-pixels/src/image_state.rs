use std::io;

use egui::{
    self, Color32, ColorImage, ImageSource, TextureHandle, TextureOptions, load::SizedTexture,
};
use image::GenericImageView;

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
                    *self = match image_data_result {
                        Ok(i) => {
                            let loaded = ImageStateLoaded::from_image_data(i, ctx);
                            on_image_load(&loaded.image);
                            ImageState::Loaded(loaded)
                        }
                        Err(e) => ImageState::Error(format!("Error: {e}")),
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

impl ImageStateLoaded {
    pub(super) fn from_image_data(i: ImageData, ctx: &egui::Context) -> Self {
        let handle = ctx.load_texture(
            "Overlays",
            ColorImage {
                size: [i.image.adjust.width() as _, i.image.adjust.height() as _],
                pixels: i
                    .image
                    .adjust
                    .pixels()
                    .map(|(_, _, image::Rgba([r, g, b, _]))| Color32::from_rgb(r, g, b))
                    .collect(),
            },
            TextureOptions {
                magnification: egui::TextureFilter::Nearest,
                ..Default::default()
            },
        );
        let texture = SizedTexture::from_handle(&handle);

        let source = ImageSource::Texture(texture);
        let x = i.image.adjust.width() as usize;
        let y = i.image.adjust.height() as usize;
        ImageStateLoaded {
            id: i.id,
            image: i.image,
            texture: (handle, source),
            masks: MaskImage::new([x, y], i.masks.clone(), Default::default()),
        }
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
