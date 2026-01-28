use egui_pixels::{ClearTool, ImageLoadOk, PanTool, RectTool, ToolFactory};

#[cfg(feature = "sam")]
mod sam;

type ToolFactories = Vec<(String, ToolFactory)>;

#[allow(unused_variables)]
pub fn default_tools(config: &crate::config::Config) -> ToolFactories {
    #[cfg(feature = "sam")]
    let session = sam::SamSession::new(&config.sam_path).unwrap();
    vec![
        ("Clear".to_string(), ClearTool::create_factory()),
        ("Pan".to_string(), PanTool::create_factory()),
        #[cfg(feature = "sam")]
        ("SAM".to_string(), sam::SamTool::create_factory(session)),
        ("Rect".to_string(), RectTool::create_factory()),
    ]
}

impl<'a> From<&'a crate::config::Config> for egui_pixels::Tools {
    fn from(config: &'a crate::config::Config) -> Self {
        Self::new(default_tools(config))
    }
}

pub(super) fn ui(ui: &mut egui::Ui, img: &ImageLoadOk, core: &mut egui_pixels::Tools) {
    let mut primary = core.primary();
    ui.horizontal(|ui| {
        ui.label("Primary:");
        let mut active_idx = primary.idx();
        egui::ComboBox::from_id_salt("primary_tool")
            .selected_text(primary.name())
            .show_ui(ui, |ui| {
                for (i, name) in primary.tool_names().enumerate() {
                    ui.selectable_value(&mut active_idx, i, name);
                }
            });
        primary.set_idx(active_idx, &img);
    });

    let mut secondary = core.secondary();
    ui.horizontal(|ui| {
        ui.label("Secondary (CTRL):");
        let mut active_idx = secondary.idx();
        egui::ComboBox::from_id_salt("secondary_tool")
            .selected_text(secondary.name())
            .show_ui(ui, |ui| {
                for (i, name) in secondary.tool_names().enumerate() {
                    ui.selectable_value(&mut active_idx, i, name);
                }
            });
        secondary.set_idx(active_idx, img);
    });
}
