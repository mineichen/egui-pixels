use std::io;

use crate::{
    async_task::AsyncTask,
    storage::{ImageId, Storage},
};
use eframe::egui::{self, ComboBox, Key};
use log::info;

const ICON_RELOAD: &str = "\u{21BB}";
const ICON_PREV_ANNOTATED: &str = "\u{23EA}";
const ICON_NEXT_ANNOTATED: &str = "\u{23E9}";
const ICON_PREV: &str = "\u{23F4}";
const ICON_NEXT: &str = "\u{23F5}";

pub(crate) struct ImageSelector {
    idx: usize,
    values: io::Result<Vec<(ImageId, String, bool)>>,
    loader: Option<AsyncTask<io::Result<Vec<(ImageId, String, bool)>>>>,
}

impl ImageSelector {
    pub fn new(loader: Option<AsyncTask<io::Result<Vec<(ImageId, String, bool)>>>>) -> Self {
        Self {
            idx: 0,
            values: Ok(Vec::new()),
            loader,
        }
    }

    pub fn current(&mut self) -> Option<&mut (ImageId, String, bool)> {
        self.values.as_mut().ok().and_then(|x| x.get_mut(self.idx))
    }

    /// Returns, wether image-state has to be reset
    pub fn ui(&mut self, is_image_dirty: bool, storage: &Storage, ui: &mut egui::Ui) -> bool {
        if let Some(loader) = self.loader.as_mut() {
            if let Some(values) = loader.data() {
                info!("Reloaded {:?} urls", values.as_ref().map(|x| x.len()));
                self.loader = None;
                self.values = values;
            }
        }

        let mut reset_image_state = false;

        match &mut self.values {
            Err(e) => {
                ui.label(format!("{e}"));
            }
            Ok(urls) => {
                if !is_image_dirty {
                    if ui
                        .button(ICON_PREV_ANNOTATED)
                        .on_hover_text("Previous annotated (shift + ArrowLeft)")
                        .clicked()
                        || ui.input(|i| i.key_pressed(Key::ArrowLeft) && i.modifiers.shift)
                    {
                        let start_idx = self.idx;
                        loop {
                            let next_idx = (self.idx.checked_sub(1)).unwrap_or(urls.len() - 1);
                            self.idx = next_idx;

                            if urls.get(next_idx).map(|x| x.2).unwrap_or_default()
                                || self.idx == start_idx
                            {
                                break;
                            }
                        }

                        reset_image_state = true;
                    }
                    if ui
                        .button(ICON_PREV)
                        .on_hover_text("Previous (ArrowLeft)")
                        .clicked()
                        || ui.input(|i| i.key_pressed(Key::ArrowLeft) && !i.modifiers.shift)
                    {
                        self.idx = (self.idx.checked_sub(1)).unwrap_or(urls.len() - 1);
                        reset_image_state = true;
                    }
                }

                if ComboBox::from_id_salt("url_selector")
                    .show_index(ui, &mut self.idx, urls.len(), |x| {
                        urls.get(x).map(|x| x.1.as_str()).unwrap_or("")
                    })
                    .changed()
                {
                    reset_image_state = true;
                }
                if ui
                    .button(ICON_RELOAD)
                    .on_hover_text("Reload Files")
                    .clicked()
                {
                    self.loader = Some(AsyncTask::new(storage.list_images()));
                }
                if !is_image_dirty {
                    if ui
                        .button(ICON_NEXT)
                        .on_hover_text("Next (ArrowRight)")
                        .clicked()
                        || ui.input(|i| i.key_pressed(Key::ArrowRight) && !i.modifiers.shift)
                    {
                        self.idx = (self.idx + 1) % urls.len();
                        reset_image_state = true;
                    }
                    if ui
                        .button(ICON_NEXT_ANNOTATED)
                        .on_hover_text("Next annotated (Shift + ArrowRight)")
                        .clicked()
                        || ui.input(|i| i.key_pressed(Key::ArrowRight) && i.modifiers.shift)
                    {
                        let start_idx = self.idx;
                        loop {
                            let next_idx = (self.idx + 1) % urls.len();
                            self.idx = next_idx;

                            if urls.get(next_idx).map(|x| x.2).unwrap_or_default()
                                || self.idx == start_idx
                            {
                                break;
                            }
                        }

                        reset_image_state = true;
                    }
                }
            }
        }
        reset_image_state
    }
}
