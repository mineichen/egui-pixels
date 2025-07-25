use std::io;

use crate::storage::Storage;
use egui::{self, ComboBox, Key};
use egui_pixels::{AsyncTask, ImageListTaskItem};
use log::info;

const ICON_RELOAD: &str = "\u{21BB}";
const ICON_PREV_ANNOTATED: &str = "\u{23EA}";
const ICON_NEXT_ANNOTATED: &str = "\u{23E9}";
const ICON_PREV: &str = "\u{23F4}";
const ICON_NEXT: &str = "\u{23F5}";

pub(crate) struct ImageSelector {
    idx: usize,
    values: io::Result<Vec<ImageListTaskItem>>,
    loader: Option<ImageListTask>,
}

type ImageListTask = AsyncTask<io::Result<Vec<ImageListTaskItem>>>;

impl ImageSelector {
    pub fn new(loader: Option<ImageListTask>) -> Self {
        Self {
            idx: 0,
            values: Ok(Vec::new()),
            loader,
        }
    }

    pub fn current(&mut self) -> Option<&mut ImageListTaskItem> {
        if let Some(loader) = self.loader.as_mut() {
            if let Some(values) = loader.data() {
                info!("Reloaded {:?} urls", values.as_ref().map(|x| x.len()));
                self.loader = None;
                self.values = values;
            }
        }
        self.values.as_mut().ok().and_then(|x| x.get_mut(self.idx))
    }

    /// Returns, wether image-state has to be reset
    pub fn ui(&mut self, storage: &dyn Storage, ui: &mut egui::Ui) -> bool {
        let mut reset_image_state = false;

        match &mut self.values {
            Err(e) => {
                ui.label(format!("{e}"));
            }
            Ok(urls) => {
                if !urls.is_empty() {
                    if ui
                        .button(ICON_PREV_ANNOTATED)
                        .on_hover_text("Previous annotated (shift + ArrowLeft)")
                        .clicked()
                        || ui.input(|i| i.key_pressed(Key::ArrowLeft) && i.modifiers.shift)
                            && ui.is_enabled()
                    {
                        let start_idx = self.idx;
                        loop {
                            let next_idx = (self.idx.checked_sub(1)).unwrap_or(urls.len() - 1);
                            self.idx = next_idx;

                            if urls.get(next_idx).map(|x| x.has_masks).unwrap_or_default()
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
                        || ui.input(|i| {
                            i.key_pressed(Key::ArrowLeft) && !i.modifiers.shift && ui.is_enabled()
                        })
                    {
                        self.idx = (self.idx.checked_sub(1)).unwrap_or(urls.len() - 1);
                        reset_image_state = true;
                    }
                }

                if ComboBox::from_id_salt("url_selector")
                    .show_index(ui, &mut self.idx, urls.len(), |x| {
                        urls.get(x).map(|x| x.name.as_str()).unwrap_or("")
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
                if !urls.is_empty() {
                    if ui
                        .button(ICON_NEXT)
                        .on_hover_text("Next (ArrowRight)")
                        .clicked()
                        || ui.input(|i| {
                            i.key_pressed(Key::ArrowRight) && !i.modifiers.shift && ui.is_enabled()
                        })
                    {
                        self.idx = (self.idx + 1) % urls.len();
                        reset_image_state = true;
                    }
                    if ui
                        .button(ICON_NEXT_ANNOTATED)
                        .on_hover_text("Next annotated (Shift + ArrowRight)")
                        .clicked()
                        || ui.input(|i| {
                            i.key_pressed(Key::ArrowRight) && i.modifiers.shift && ui.is_enabled()
                        })
                    {
                        let start_idx = self.idx;
                        loop {
                            let next_idx = (self.idx + 1) % urls.len();
                            self.idx = next_idx;

                            if urls.get(next_idx).map(|x| x.has_masks).unwrap_or_default()
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
