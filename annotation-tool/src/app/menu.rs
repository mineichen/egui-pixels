use crate::async_task::AsyncRefTask;
use egui::Key;
use futures::FutureExt;
use log::info;

use super::image_state::{ImageState, ImageStateLoaded};

// const ICON_SAM: &str = "\u{2728}";
const ICON_SAVE: &str = "\u{1F4BE}";

impl crate::app::ImageViewerApp {
    pub(super) fn menu_ui(&mut self, ui: &mut egui::Ui) {
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

            match &mut self.image_state {
                ImageState::Loaded(ImageStateLoaded { image, .. }) => {
                    self.tools.ui(ui, image);
                }
                ImageState::Error(error) => {
                    ui.label(format!("Error: {error}"));
                }
                _ => (),
            }
        });
    }
}
