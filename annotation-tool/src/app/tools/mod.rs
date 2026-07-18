use std::num::NonZeroU16;

use std::any::Any;

use imanot::{BrushTool, ImageLoadOk, Mode, PanTool, RectTool, ToolFactory};

#[cfg(feature = "sam")]
mod sam;

type ToolFactories = Vec<(String, ToolFactory)>;

#[allow(unused_variables)]
pub fn default_tools(config: &crate::config::Config) -> ToolFactories {
    #[cfg(feature = "sam")]
    let session = {
        eprintln!("[DEBUG] Creating SAM session from path: {:?}", config.sam_path);
        match sam::SamSession::new(&config.sam_path) {
            Ok(s) => { eprintln!("[DEBUG] SAM session created successfully"); s }
            Err(e) => { eprintln!("[DEBUG] SAM session FAILED: {:?}", e); panic!("SAM session error: {:?}", e); }
        }
    };
    vec![
        ("Pan".to_string(), PanTool::create_factory()),
        #[cfg(feature = "sam")]
        ("SAM".to_string(), sam::SamTool::create_factory(session)),
        ("Rect".to_string(), RectTool::create_factory()),
        ("Brush".to_string(), BrushTool::create_factory()),
        (
            "Brush clear".to_string(),
            BrushTool::create_factory_with(|t| {
                t.set_mode(Mode::Clear);
            }),
        ),
        (
            "Rect clear".to_string(),
            RectTool::create_factory_with(|t| {
                t.set_mode(Mode::Clear);
            }),
        ),
    ]
}

impl<'a> From<&'a crate::config::Config> for imanot::Tools {
    fn from(config: &'a crate::config::Config) -> Self {
        Self::new(default_tools(config))
    }
}

pub(super) fn ui(ui: &mut egui::Ui, img: &ImageLoadOk, core: &mut imanot::Tools) {
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
        primary.set_idx(active_idx, img);
    });

    if let Some(Ok(tool)) = primary.data() {
        let any: &mut dyn Any = &mut **tool;
        if let Some(brush) = any.downcast_mut::<BrushTool>() {
            ui.horizontal(|ui| {
                ui.label("Brush Size:");
                let mut raw = brush.brush_size.get();
                if ui
                    .add(egui::Slider::new(&mut raw, 1..=100).step_by(1.0))
                    .changed()
                {
                    brush.brush_size = NonZeroU16::new(raw).unwrap();
                }
            });
        }
    }

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
