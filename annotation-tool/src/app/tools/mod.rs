use egui_pixels::{AsyncRefTask, ImageLoadOk, ImageState, PanTool, Tool, ToolContext, ToolPainter};
use futures::{FutureExt, future::BoxFuture};

#[cfg(feature = "sam")]
mod sam;

pub struct Tools {
    active_primary_idx: usize,
    active_secondary_idx: usize,
    tool_factories: ToolFactories,
    primary_tool: AsyncRefTask<Result<Box<dyn Tool + Send>, String>>,
    secondary_tool: AsyncRefTask<Result<Box<dyn Tool + Send>, String>>,
}

type ToolFactories = Vec<(
    String,
    Box<dyn Fn(&ImageLoadOk) -> BoxFuture<'static, Result<Box<dyn Tool + Send>, String>>>,
)>;

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
            "Clear".to_string(),
            Box::new(|_| {
                async { Ok(Box::new(egui_pixels::ClearTool::default()) as Box<dyn Tool + Send>) }
                    .boxed()
            }),
        ),
    ]
}

impl<'a> From<&'a crate::config::Config> for Tools {
    fn from(config: &'a crate::config::Config) -> Self {
        Self::new(default_tools(config))
    }
}

impl Tools {
    pub(super) fn load_primary_tool(&mut self, img: &ImageLoadOk) {
        let (name, factory) = &mut self.tool_factories[self.active_primary_idx];
        log::debug!("Loading primary tool: {name}");
        self.primary_tool = AsyncRefTask::new(factory(img));
    }

    pub(super) fn load_secondary_tool(&mut self, img: &ImageLoadOk) {
        let (name, factory) = &mut self.tool_factories[self.active_secondary_idx];
        log::debug!("Loading secondary tool: {name}");
        self.secondary_tool = AsyncRefTask::new(factory(img));
    }

    pub(super) fn new(tool_factories: ToolFactories) -> Self {
        // Default: primary = first non-Pan tool, secondary = Pan
        let pan_idx = tool_factories
            .iter()
            .position(|(name, _)| name == "Pan")
            .unwrap_or(0);
        let primary_idx = (pan_idx == 0 && tool_factories.len() > 1) as usize;

        Self {
            active_primary_idx: primary_idx,
            active_secondary_idx: pan_idx,
            tool_factories,
            primary_tool: AsyncRefTask::new_ready(Ok(Box::new(NopTool))),
            secondary_tool: AsyncRefTask::new_ready(Ok(Box::new(PanTool::default()))),
        }
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui, img: &ImageLoadOk) {
        ui.horizontal(|ui| {
            ui.label("Primary:");
            let before = self.active_primary_idx;
            egui::ComboBox::from_id_salt("primary_tool")
                .selected_text(&self.tool_factories[self.active_primary_idx].0)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in self.tool_factories.iter().enumerate() {
                        ui.selectable_value(&mut self.active_primary_idx, i, name);
                    }
                });
            if before != self.active_primary_idx {
                self.load_primary_tool(img);
            }
        });

        ui.horizontal(|ui| {
            ui.label("Secondary (CTRL):");
            let before = self.active_secondary_idx;
            egui::ComboBox::from_id_salt("secondary_tool")
                .selected_text(&self.tool_factories[self.active_secondary_idx].0)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in self.tool_factories.iter().enumerate() {
                        ui.selectable_value(&mut self.active_secondary_idx, i, name);
                    }
                });
            if before != self.active_secondary_idx {
                self.load_secondary_tool(img);
            }
        });
    }
}

impl super::ImageViewerApp {
    pub(super) fn handle_tool_interaction(
        &mut self,
        response: egui::Response,
        ctx: &egui::Context,
        tool_painter: ToolPainter,
    ) {
        if let ImageState::Loaded(image) = &mut self.image_state {
            // Check if CTRL is pressed to determine which tool to use
            let use_secondary = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);

            let tool_opt = if use_secondary {
                self.tools.secondary_tool.data()
            } else {
                self.tools.primary_tool.data()
            };

            if let Some(Ok(tool)) = tool_opt {
                tool.handle_interaction(ToolContext::new(
                    image,
                    response,
                    ctx,
                    tool_painter,
                    &mut self.viewer,
                ));
            }
        }
    }
}

/// Used to swap the tool in and out of the tools vector to borrow ImageViewerApp mutably
struct NopTool;
impl Tool for NopTool {
    fn handle_interaction(&mut self, _ctx: ToolContext) {
        log::debug!("NopTool was called");
    }
}
