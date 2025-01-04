use image::{ImageBuffer, Luma};

pub fn load_image(bytes: &[u8]) -> std::io::Result<image::DynamicImage> {
    Ok(
        match image::load_from_memory(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        {
            image::DynamicImage::ImageLuma16(i) => {
                image::DynamicImage::ImageLuma16(fix_image_contrast(i))
            }
            image::DynamicImage::ImageLuma8(i) => {
                image::DynamicImage::ImageLuma8(fix_image_contrast(i))
            }
            image => image,
        },
    )
}

fn fix_image_contrast<T: image::Primitive + Ord>(
    i: ImageBuffer<Luma<T>, Vec<T>>,
) -> ImageBuffer<Luma<T>, Vec<T>>
where
    f32: From<T>,
{
    let mut pixels = i.pixels().map(|Luma([p])| p).collect::<Vec<_>>();
    pixels.sort_unstable();
    let five_percent_pos = pixels.len() / 20;
    let lower: f32 = (*pixels[five_percent_pos]).into();
    let upper: f32 = (*pixels[five_percent_pos * 18]).into();
    let max_pixel_value: f32 = T::DEFAULT_MAX_VALUE.into();
    let range = max_pixel_value / (upper - lower);

    ImageBuffer::from_raw(
        i.width(),
        i.height(),
        i.pixels()
            .map(|Luma([p])| {
                let as_f: f32 = (*p).into();

                num_traits::cast::NumCast::from(
                    ((as_f - lower) * range).clamp(0.0, max_pixel_value),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
