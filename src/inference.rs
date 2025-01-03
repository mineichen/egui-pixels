use std::{
    future::Future,
    num::{NonZero, NonZeroU16, NonZeroU32, TryFromIntError},
    sync::Arc,
};

use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgba};
use itertools::Itertools;
use ndarray::{Array, IxDyn};
use ort::{Environment, OrtError, Session, SessionBuilder, Value};

use crate::SubGroups;

pub struct SamSession {
    encoder: Arc<Session>,
    decoder: Session,
}

impl From<TryFromIntError> for InferenceError {
    fn from(value: TryFromIntError) -> Self {
        Self::Other(Arc::new(value))
    }
}

impl SamSession {
    pub fn new() -> Result<Self, InferenceError> {
        let env = Arc::new(Environment::builder().with_name("SAM").build()?);
        let encoder = SessionBuilder::new(&env)?.with_model_from_file("sam/vit_t_encoder.onnx")?;
        let decoder = SessionBuilder::new(&env)?.with_model_from_file("sam/vit_t_decoder.onnx")?;
        Ok(Self {
            encoder: Arc::new(encoder),
            decoder,
        })
    }

    pub fn get_image_embeddings(
        &self,
        img: DynamicImage,
    ) -> impl Future<Output = Result<SamEmbeddings, InferenceError>> {
        let (tx, rx) = futures::channel::oneshot::channel();

        let session = self.encoder.clone();
        std::thread::spawn(|| {
            let r = Self::get_image_embeddings_blocking(session, img);
            tx.send(r)
        });
        async move {
            rx.await
                .map_err(|e| InferenceError::Other(Arc::new(e)))
                .and_then(|a| a)
        }
    }
    pub fn get_image_embeddings_blocking(
        encoder: Arc<Session>,
        img: DynamicImage,
    ) -> Result<SamEmbeddings, InferenceError> {
        // Resize the image and save original size.
        let (orig_width, orig_height) = img.dimensions();
        let (orig_width, orig_height) = (
            (orig_width as u32).try_into()?,
            (orig_height as u32).try_into()?,
        );

        println!("Original: {:?}", std::mem::discriminant(&img));
        let img_resized = img.resize(1024, 1024, FilterType::CatmullRom);
        let (resized_width, resized_height) = img_resized.dimensions();
        let (resized_width, resized_height) = (
            NonZeroU32::try_from(resized_width as u32)?,
            NonZeroU32::try_from(resized_height as u32)?,
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

        // Prepare tensor for the SAM encoder model
        let input_as_values = input.into_dyn();
        let input_as_values = &input_as_values.as_standard_layout();
        let encoder_inputs = vec![Value::from_array(encoder.allocator(), input_as_values)?];

        // Run encoder to get image embeddings
        let outputs = encoder.run(encoder_inputs)?;
        let embeddings = outputs
            .get(0)
            .ok_or_else(|| InferenceError::UnexpectedOutput("Expected a output".into()))?
            .try_extract::<f32>()
            .map_err(|e| InferenceError::UnexpectedOutput(format!("Expected f32: {e:?}")))?
            .view()
            .t()
            .reversed_axes()
            .into_owned();

        Ok(SamEmbeddings {
            embeddings,
            orig_width,
            orig_height,
            resized_width,
            resized_height,
        })
    }

    pub fn decode_prompt(
        &self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        embeddings: &SamEmbeddings,
    ) -> Result<SubGroups, InferenceError> {
        // Prepare input for decoder

        // Get embeddings, image sizes and ONNX model instances from Web Application state
        let orig_width = embeddings.orig_width.get() as f32;
        let orig_height = embeddings.orig_height.get() as f32;
        let resized_width = embeddings.resized_width.get() as f32;
        let resized_height = embeddings.resized_height.get() as f32;
        let decoder = &self.decoder;
        let embeddings_as_values = &embeddings.embeddings.as_standard_layout();

        // Encode points prompt
        // let point_coords = Array::from_shape_vec(
        //     (1, 2, 2),
        //     vec![
        //         x1 * (resized_width / orig_width),
        //         y1 * (resized_height / orig_height),
        //         x2 * (resized_height / orig_height),
        //         y2 * (resized_height / orig_height),
        //     ],
        // )
        // .expect("Shape always matches")
        // .into_dyn()
        let point_coords = ndarray::array![[
            [
                x1 * (resized_width / orig_width),
                y1 * (resized_height / orig_height),
            ],
            [
                x2 * (resized_height / orig_height),
                y2 * (resized_height / orig_height),
            ]
        ]]
        .into_dyn();
        let point_coords_as_values = &point_coords.as_standard_layout();

        // Labels
        let point_labels = ndarray::array![[2.0_f32, 3.0_f32]].into_dyn();
        let point_labels_as_values = &point_labels.as_standard_layout();

        // Encode mask prompt (dummy)
        let mask_input = Array::<f32, _>::zeros((1, 1, 256, 256)).into_dyn();
        let mask_input_as_values = &mask_input.as_standard_layout();
        let has_mask_input = ndarray::array![0.0_f32].into_dyn();
        let has_mask_input_as_values = &has_mask_input.as_standard_layout();

        // Add original image size
        let orig_im_size = ndarray::array![orig_height, orig_width].into_dyn();
        let orig_im_size_as_values = &orig_im_size.as_standard_layout();

        // Prepare inputs for SAM decoder
        let decoder_inputs = vec![
            Value::from_array(decoder.allocator(), embeddings_as_values)?,
            Value::from_array(decoder.allocator(), point_coords_as_values)?,
            Value::from_array(decoder.allocator(), point_labels_as_values)?,
            Value::from_array(decoder.allocator(), mask_input_as_values)?,
            Value::from_array(decoder.allocator(), has_mask_input_as_values)?,
            Value::from_array(decoder.allocator(), orig_im_size_as_values)?,
        ];

        // Run the SAM decoder
        let outputs = decoder.run(decoder_inputs)?;
        println!(
            "Outputs {:?}",
            outputs
                .iter()
                .map(|x| x.try_extract::<f32>().map(|x| x.view().len()))
                .collect::<Vec<_>>()
        );

        // Process and return output mask (replace negative pixel values to 0 and positive to 1)
        let pixels = outputs
            .get(0)
            .ok_or_else(|| InferenceError::UnexpectedOutput("No output".into()))?
            .try_extract::<f32>()
            .map_err(|e| InferenceError::UnexpectedOutput(format!("Output of type f32: {e:?}")))?;
        let pixel_view = pixels.view();

        let mut result = vec![];
        pixel_view
            .iter()
            .enumerate()
            .filter_map(|(pos, item)| (*item > 0.0).then_some(pos as u32))
            .chunk_by(|&x| x / embeddings.orig_width)
            .into_iter()
            .for_each(|(_, mut b)| {
                let first = b.next().expect("Doesn't yield if group is empty");
                result.push((first, NonZeroU16::MIN));
                b.fold(first, |last, x| {
                    if x == last {
                        let item = result.last_mut().unwrap();
                        item.1 = item
                            .1
                            .checked_add(1)
                            .expect("image.width is never > u16::MAX");
                    } else {
                        result.push((x, NonZeroU16::MIN));
                    }
                    x
                });
            });
        Ok(result)
    }
}

pub struct SamEmbeddings {
    pub embeddings: Array<f32, IxDyn>,
    pub orig_width: NonZero<u32>,
    pub orig_height: NonZero<u32>,
    pub resized_width: NonZero<u32>,
    pub resized_height: NonZero<u32>,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum InferenceError {
    #[error("Allocation: {0:?}")]
    AllocationError(Arc<dyn std::error::Error + Send + Sync>),

    #[error("Other: {0:?}")]
    Other(Arc<dyn std::error::Error + Send + Sync>),

    #[error("Unexpected network output")]
    UnexpectedOutput(String),
}

impl From<OrtError> for InferenceError {
    fn from(value: OrtError) -> Self {
        match value {
            e @ OrtError::CreateIoBinding(_) | e @ OrtError::CreateAllocator(_) => {
                InferenceError::AllocationError(Arc::new(e))
            }
            e => InferenceError::Other(Arc::new(e)),
        }
    }
}
