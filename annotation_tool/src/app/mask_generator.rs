use egui::ComboBox;
use image::DynamicImage;

use crate::{ImageCallbackMap, SubGroups};

pub(crate) struct MaskGenerator {
    pos: usize,
    map: ImageCallbackMap,
}

impl MaskGenerator {
    pub fn new(map: ImageCallbackMap) -> Self {
        Self { map, pos: 0 }
    }

    pub(super) fn ui(&mut self, image: &DynamicImage, ui: &mut egui::Ui) -> Option<Vec<SubGroups>> {
        if !self.map.is_empty() {
            ComboBox::from_id_salt("algo_selector")
                .show_index(ui, &mut self.pos, self.map.len(), |x| {
                    self.map.get(x).map(|x| x.0.as_str()).unwrap_or("")
                })
                .changed();
            if let (true, Some((_, algo))) =
                (ui.button("annotate").clicked(), self.map.get_mut(self.pos))
            {
                return Some(algo(image));
            }
        }
        None
    }
}
