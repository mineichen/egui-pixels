use crate::{
    async_task::{AsyncRefTask, AsyncTask},
    mask::MaskImage,
};
use eframe::egui::{
    self, load::SizedTexture, Color32, ColorImage, ImageSource, Key, TextureOptions,
};
use futures::FutureExt;
use image::GenericImageView;
use log::info;

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
                if self.selector.ui(&*self.storage, ui) {
                    self.image_state = ImageState::NotLoaded;
                }
            });

            if let (
                Some(last_save),
                ImageState::Loaded(ImageStateLoaded {
                    id, masks, image, ..
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

                if let Some(x) = self.mask_generator.ui(&image.original, ui) {
                    info!("Add {} groups", x.len());
                    for group in x {
                        masks.add_subgroups(group);
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
                                    self.tools.ui(ui, &i.image);
                                    let handle = ui.ctx().load_texture(
                                        "Overlays",
                                        ColorImage {
                                            size: [
                                                i.image.adjust.width() as _,
                                                i.image.adjust.height() as _,
                                            ],
                                            pixels: i
                                                .image
                                                .adjust
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

                                    self.tools.load_tool(&i.image);

                                    let x = i.image.adjust.width() as usize;
                                    let y = i.image.adjust.height() as usize;

                                    ImageState::Loaded(ImageStateLoaded {
                                        id: i.id,
                                        image: i.image,
                                        texture: handle,
                                        masks: MaskImage::new(
                                            [x, y],
                                            i.masks.clone(),
                                            Default::default(),
                                        ),
                                    })
                                }
                                Err(e) => ImageState::Error(format!("Error: {e}")),
                            }
                        }
                    }
                    ImageState::Loaded(ImageStateLoaded { masks, image, .. }) => {
                        self.tools.ui(ui, image);
                        if let Some(x) = masks.handle_events(ui.ctx()) {
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
