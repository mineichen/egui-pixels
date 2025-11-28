use std::num::NonZeroU32;

use image_buffer::{LumaImage, RgbImageInterleaved, RgbaImageInterleaved};

#[cfg(feature = "image")]
mod image;

/// Different image formats supported for the original image
#[derive(Clone)]
pub enum OriginalImage {
    Luma8(LumaImage<u8>),
    Luma16(LumaImage<u16>),
    Rgb8(RgbImageInterleaved<u8>),
    Rgba8(RgbaImageInterleaved<u8>),
}

impl OriginalImage {
    pub fn width(&self) -> NonZeroU32 {
        match self {
            OriginalImage::Luma8(img) => img.dimensions().0,
            OriginalImage::Luma16(img) => img.dimensions().0,
            OriginalImage::Rgb8(img) => img.dimensions().0,
            OriginalImage::Rgba8(img) => img.dimensions().0,
        }
    }

    pub fn height(&self) -> NonZeroU32 {
        match self {
            OriginalImage::Luma8(img) => img.dimensions().1,
            OriginalImage::Luma16(img) => img.dimensions().1,
            OriginalImage::Rgb8(img) => img.dimensions().1,
            OriginalImage::Rgba8(img) => img.dimensions().1,
        }
    }
}

/// Represents a loaded image using image-buffer
#[derive(Clone)]
pub struct ImageLoadOk {
    pub original: OriginalImage,
    pub adjust: RgbImageInterleaved<u8>,
}

impl ImageLoadOk {
    pub fn adjust_pixels(&self) -> impl Iterator<Item = (u32, u32, [u8; 3])> + '_ {
        let (width, _) = self.adjust.dimensions();
        let width = width.get();
        // flat_buffer() returns &[u8] for RgbImageInterleaved (flattened)
        self.adjust
            .flat_buffer()
            .chunks_exact(3)
            .enumerate()
            .map(move |(idx, chunk)| {
                let x = (idx % width as usize) as u32;
                let y = (idx / width as usize) as u32;
                (x, y, [chunk[0], chunk[1], chunk[2]])
            })
    }
}

#[cfg(feature = "image")]
pub use image::load_image;
