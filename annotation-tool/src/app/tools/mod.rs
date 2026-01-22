use egui_pixels::{ImageLoadOk, PanTool, Tool, ToolFactory};
use futures::FutureExt;

#[cfg(feature = "sam")]
mod sam;

type ToolFactories = Vec<(String, ToolFactory)>;

#[allow(unused_variables)]
pub fn default_tools(config: &crate::config::Config) -> ToolFactories {
    #[cfg(feature = "sam")]
    let session = sam::SamSession::new(&config.sam_path).unwrap();
    vec![
        (
            "Pan".to_string(),
            Box::new(|_| {
                async { Ok(Box::new(PanTool::default()) as Box<dyn Tool + Send>) }.boxed()
            }),
        ),
        #[cfg(feature = "sam")]
        (
            "SAM".to_string(),
            Box::new(move |img| {
                let tool = sam::SamTool::new(session.clone(), img.adjust.clone());
                async move { Ok(Box::new(tool) as Box<dyn Tool + Send>) }.boxed()
            }),
        ),
        (
            "Rect".to_string(),
            Box::new(|_| {
                async { Ok(Box::new(egui_pixels::RectTool::default()) as Box<dyn Tool + Send>) }
                    .boxed()
            }),
        ),
        (
            "Clear".to_string(),
            Box::new(|_| {
                async { Ok(Box::new(egui_pixels::ClearTool::default()) as Box<dyn Tool + Send>) }
                    .boxed()
            }),
        ),
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
