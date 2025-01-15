use crate::{
    async_task::{AsyncRefTask, AsyncTask},
    mask::MaskImage,
};
use eframe::egui::{
    self, load::SizedTexture, Color32, ColorImage, ImageSource, Key, TextureOptions,
};
use futures::FutureExt;
use image::GenericImageView;

use super::{ImageState, ImageStateLoaded};

// const ICON_SAM: &str = "\u{2728}";
const ICON_SAVE: &str = "\u{1F4BE}";

impl crate::app::ImageViewerApp {
    pub(super) fn menu(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let is_image_dirty = matches!(
                &self.image_state,
                ImageState::Loaded(ImageStateLoaded {
                    masks,
                    ..
                }) if masks.is_dirty()
            );
            ui.scope(|ui| {
                if is_image_dirty {
                    ui.disable();
                }
                if self.selector.ui(&self.storage, ui) {
                    self.image_state = ImageState::NotLoaded;
                }
            });

            if let (
                Some(last_save),
                ImageState::Loaded(ImageStateLoaded {
                    id,
                    masks,
                    original_image,
                    ..
                }),
            ) = (self.save_job.data(), &mut self.image_state)
            {
                if let Err(e) = last_save {
                    ui.label(format!("Error during save: {e}"));
                }
                ui.scope(|ui| {
                    if !is_image_dirty {
                        ui.disable();
                    }
                    if ui
                        .button(ICON_SAVE)
                        .on_hover_text("Save (cmd + S)")
                        .clicked()
                        || ui.input(|i| {
                            i.modifiers.command && i.key_pressed(Key::S) && ui.is_enabled()
                        })
                    {
                        masks.mark_not_dirty();
                        self.save_job = AsyncRefTask::new(
                            self.storage
                                .store_masks(id.clone(), masks.subgroups())
                                .boxed(),
                        );
                    }
                });

                if ui.button("Reset").clicked() {
                    masks.reset();
                }

                if let Some(x) = self.mask_generator.ui(original_image, ui) {
                    println!("Add {} groups", x.len());
                    for group in x {
                        masks.add_subgroup(("annotation".into(), group));
                    }
                }
            }

            if let Some((image_id, _, _)) = self.selector.current() {
                match &mut self.image_state {
                    ImageState::NotLoaded => {
                        self.image_state = ImageState::LoadingImageData(AsyncTask::new(
                            self.storage.load_image(image_id),
                        ))
                    }
                    ImageState::LoadingImageData(t) => {
                        if let Some(x) = t.data() {
                            self.image_state = match x {
                                Ok(i) => {
                                    let handle = ui.ctx().load_texture(
                                        "Overlays",
                                        ColorImage {
                                            size: [
                                                i.adjust_image.width() as _,
                                                i.adjust_image.height() as _,
                                            ],
                                            pixels: i
                                                .adjust_image
                                                .pixels()
                                                .map(|(_, _, image::Rgba([r, g, b, _]))| {
                                                    Color32::from_rgb(r, g, b)
                                                })
                                                .collect(),
                                        },
                                        TextureOptions {
                                            magnification: egui::TextureFilter::Nearest,
                                            ..Default::default()
                                        },
                                    );
                                    let texture = SizedTexture::from_handle(&handle);
                                    self.viewer.reset();
                                    self.viewer.sources = vec![ImageSource::Texture(texture)];
                                    let embeddings = AsyncRefTask::new(
                                        self.session
                                            .get_image_embeddings(i.adjust_image.clone())
                                            .boxed(),
                                    );

                                    let x = i.adjust_image.width() as usize;
                                    let y = i.adjust_image.height() as usize;

                                    ImageState::Loaded(ImageStateLoaded {
                                        id: i.id,
                                        original_image: i.original_image,
                                        texture: handle,
                                        masks: MaskImage::new(
                                            [x, y],
                                            i.masks.clone(),
                                            Default::default(),
                                        ),
                                        embeddings,
                                    })
                                }
                                Err(e) => ImageState::Error(format!("Error: {e}")),
                            }
                        }
                    }
                    ImageState::Loaded(ImageStateLoaded { masks, .. }) => {
                        self.viewer.sources.truncate(1);
                        if let Some(x) = masks.ui_events(ui) {
                            self.viewer.sources.push(ImageSource::Texture(x));
                        }
                    }
                    ImageState::Error(error) => {
                        ui.label(format!("Error: {error}"));
                    }
                }
            }
        });
    }
}
