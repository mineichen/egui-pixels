use egui::ComboBox;
use egui_pixels::{OriginalImage, PixelArea};

use crate::ImageCallbackMap;

pub(crate) struct MaskGenerator {
    pos: usize,
    map: ImageCallbackMap,
}

impl MaskGenerator {
    pub fn new(map: ImageCallbackMap) -> Self {
        Self { map, pos: 0 }
    }

    pub(super) fn ui(
        &mut self,
        image: &OriginalImage,
        ui: &mut egui::Ui,
    ) -> Option<Vec<PixelArea>> {
        if !self.map.is_empty() {
            ComboBox::from_id_salt("algo_selector")
                .show_index(ui, &mut self.pos, self.map.len(), |x| {
                    self.map.get(x).map(|x| x.0.as_str()).unwrap_or("")
                })
                .changed();
            if let (true, Some((_, algo))) =
                (ui.button("annotate").clicked(), self.map.get_mut(self.pos))
            {
                let dyn_img = image.to_dynamic_image();
                return Some(algo(&dyn_img));
            }
        }
        None
    }
}
