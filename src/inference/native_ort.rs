use std::{future::Future, path::Path, sync::Arc};

use image::DynamicImage;
use log::debug;
use ndarray::Array;
use ort::{Environment, OrtError, Session, SessionBuilder, Value};

use crate::{inference::extract_subgroups, SubGroups};

use super::{InferenceError, SamEmbeddings};

#[derive(Clone)]
pub struct SamSession {
    encoder: Arc<Session>,
    decoder: Arc<Session>,
}

impl SamSession {
    pub fn new(path: &Path) -> Result<Self, InferenceError> {
        let env = Arc::new(Environment::builder().with_name("SAM").build()?);
        let encoder =
            SessionBuilder::new(&env)?.with_model_from_file(path.join("vit_t_encoder.onnx"))?;
        let decoder =
            SessionBuilder::new(&env)?.with_model_from_file(path.join("vit_t_decoder.onnx"))?;
        Ok(Self {
            encoder: Arc::new(encoder),
            decoder: Arc::new(decoder),
        })
    }

    pub fn get_image_embeddings(
        &self,
        img: Arc<DynamicImage>,
    ) -> impl Future<Output = Result<SamEmbeddings, InferenceError>> + Send {
        let (tx, rx) = futures::channel::oneshot::channel();

        let session = self.encoder.clone();
        let handle = std::thread::spawn(|| {
            let r = Self::get_image_embeddings_blocking(session, img);
            tx.send(r)
        });
        async move {
            let r = rx
                .await
                .map_err(|e| InferenceError::Other(Arc::new(e)))
                .and_then(|a| a);
            handle.join().unwrap().expect("Channel cant be gone");
            r
        }
    }
    pub fn get_image_embeddings_blocking(
        encoder: Arc<Session>,
        img: Arc<DynamicImage>,
    ) -> Result<SamEmbeddings, InferenceError> {
        let image_input = super::prepare_image_input(&img)?;
        // Prepare tensor for the SAM encoder model
        let input_as_values = &image_input.image_data.as_standard_layout();
        let encoder_inputs = vec![Value::from_array(encoder.allocator(), input_as_values)?];

        // Run encoder to get image embeddings
        let outputs = encoder.run(encoder_inputs)?;
        // return Err(InferenceError::Other(Arc::new(std::io::Error::other(
        //     "Testing purpose",
        // ))));
        let embeddings = outputs
            .first()
            .ok_or_else(|| InferenceError::UnexpectedOutput("Expected a output".into()))?
            .try_extract::<f32>()
            .map_err(|e| InferenceError::UnexpectedOutput(format!("Expected f32: {e:?}")))?
            .view()
            .t()
            .reversed_axes()
            .into_owned();

        Ok(image_input.map(|_| embeddings))
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
        let orig_width = embeddings.original_width.get() as f32;
        let orig_height = embeddings.original_height.get() as f32;
        let resized_width = embeddings.resized_width.get() as f32;
        let resized_height = embeddings.resized_height.get() as f32;
        let decoder = &self.decoder;
        let embeddings_as_values = &embeddings.image_data.as_standard_layout();

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
        debug!(
            "Outputs {:?}",
            outputs
                .iter()
                .map(|x| x.try_extract::<f32>().map(|x| x.view().len()))
                .collect::<Vec<_>>()
        );

        // Process and return output mask (replace negative pixel values to 0 and positive to 1)
        let pixels = outputs
            .first()
            .ok_or_else(|| InferenceError::UnexpectedOutput("No output".into()))?
            .try_extract::<f32>()
            .map_err(|e| InferenceError::UnexpectedOutput(format!("Output of type f32: {e:?}")))?;
        let pixel_view = pixels.view();

        Ok(extract_subgroups(
            pixel_view.iter().copied(),
            embeddings.original_width,
        ))
    }
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
