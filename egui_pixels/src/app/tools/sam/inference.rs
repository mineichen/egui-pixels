use std::{
    num::{NonZeroU16, NonZeroU32, TryFromIntError},
    sync::Arc,
};

use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgba};
use itertools::Itertools;
use ndarray::{Array, ArrayBase, Dim, IxDyn, IxDynImpl, OwnedRepr};

use crate::{SubGroup, SubGroups};

impl From<TryFromIntError> for InferenceError {
    fn from(value: TryFromIntError) -> Self {
        Self::Other(Arc::new(value))
    }
}

pub type SamEmbeddings = ResizedImageData<Array<f32, IxDyn>>;
pub type SamInputData = ResizedImageData<ArrayBase<OwnedRepr<f32>, Dim<IxDynImpl>>>;

#[derive(Debug, thiserror::Error, Clone)]
pub enum InferenceError {
    #[error("Allocation: {0:?}")]
    AllocationError(Arc<dyn std::error::Error + Send + Sync>),

    #[error("Other: {0:?}")]
    Other(Arc<dyn std::error::Error + Send + Sync>),

    #[error("Unexpected network output")]
    UnexpectedOutput(String),
}

pub(super) fn prepare_image_input(img: &DynamicImage) -> Result<SamInputData, InferenceError> {
    let (original_width, original_height) = img.dimensions();
    let (original_width, original_height) = (
        NonZeroU32::try_from(original_width)?,
        NonZeroU32::try_from(original_height)?,
    );
    let img_resized = img.resize(1024, 1024, FilterType::CatmullRom);
    let (resized_width, resized_height) = img_resized.dimensions();
    let (resized_width, resized_height) = (
        NonZeroU32::try_from(resized_width)?,
        NonZeroU32::try_from(resized_height)?,
    );

    let mut input = Array::zeros((1, 3, 1024, 1024));
    let rgb = input
        .as_slice_mut()
        .expect("zeros always returns continuous slice");
    let (r, gb) = rgb.split_at_mut(1024 * 1024);
    let (g, b) = gb.split_at_mut(1024 * 1024);

    match img_resized {
        DynamicImage::ImageLuma16(i) => {
            //let x = streaming - stats::mean();
            let mut image_vec = i.into_vec();
            let mut s = rolling_stats::Stats::new();
            image_vec.iter().for_each(|v| s.update(*v as f32));

            for (src_c, dst_c) in image_vec
                .chunks_exact_mut(resized_width.get() as usize)
                .zip(r.chunks_exact_mut(1024))
            {
                for (src, dst) in src_c.iter_mut().zip(dst_c) {
                    *dst = (*src as f32 - s.mean) / s.std_dev;
                }
            }
            g.copy_from_slice(r);
            b.copy_from_slice(r);
        }
        image => {
            // Copy the image pixels to the tensor, normalizing them using mean and standard deviations
            // for each color channel

            let mut rs = rolling_stats::Stats::new();
            let mut gs = rolling_stats::Stats::new();
            let mut bs = rolling_stats::Stats::new();

            for (_, _, Rgba([r, g, b, _])) in image.pixels() {
                rs.update(r as f32);
                gs.update(g as f32);
                bs.update(b as f32);
            }

            for (((input_chunk, r_chunk), g_chunk), b_chunk) in image
                .pixels()
                .chunks(image.width() as _)
                .into_iter()
                .zip(r.chunks_exact_mut(1024))
                .zip(g.chunks_exact_mut(1024))
                .zip(b.chunks_exact_mut(1024))
            {
                for ((((_, _, Rgba([r, g, b, _])), r_dest), g_dest), b_dest) in
                    input_chunk.zip(r_chunk).zip(g_chunk).zip(b_chunk)
                {
                    *r_dest = (r as f32 - rs.mean) / rs.std_dev;
                    *g_dest = (g as f32 - gs.mean) / gs.std_dev;
                    *b_dest = (b as f32 - bs.mean) / bs.std_dev;
                }
            }
        }
    };

    Ok(ResizedImageData {
        image_data: input.into_dyn(),
        resized_width,
        resized_height,
        original_width,
        original_height,
    })
}

pub(super) fn extract_subgroups(iter: impl Iterator<Item = f32>, width: NonZeroU32) -> SubGroups {
    let mut result = vec![];
    iter.enumerate()
        .filter_map(|(pos, item)| (item > 0.0).then_some(pos as u32))
        .chunk_by(|&x| x / width)
        .into_iter()
        .for_each(|(_, mut b)| {
            let first = b.next().expect("Doesn't yield if group is empty");
            result.push(SubGroup::new_total(first, NonZeroU16::MIN));
            b.fold(first, |last, x| {
                if x - 1 == last {
                    let item = result.last_mut().unwrap();
                    item.length = item
                        .length
                        .checked_add(1)
                        .expect("image.width is never > u16::MAX");
                } else {
                    result.push(SubGroup::new_total(x, NonZeroU16::MIN));
                }
                x
            });
        });
    result
}

#[derive(Debug)]
pub struct ResizedImageData<T> {
    pub(super) image_data: T,
    pub(super) original_width: NonZeroU32,
    pub(super) original_height: NonZeroU32,
    pub(super) resized_width: NonZeroU32,
    pub(super) resized_height: NonZeroU32,
}

impl<T> ResizedImageData<T> {
    pub fn map<TNew>(self, x: impl FnOnce(T) -> TNew) -> ResizedImageData<TNew> {
        ResizedImageData {
            image_data: (x)(self.image_data),
            original_width: self.original_width,
            original_height: self.original_height,
            resized_width: self.resized_width,
            resized_height: self.resized_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_subgroups_summarizes_pixels() {
        assert_eq!(
            vec![SubGroup::new_total(0, 3.try_into().unwrap())],
            extract_subgroups([1., 1., 1.].iter().copied(), 3.try_into().unwrap())
        );
    }
}
