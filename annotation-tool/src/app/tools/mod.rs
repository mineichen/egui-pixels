use egui_pixels::{AsyncRefTask, ImageLoadOk, ImageState, Tool, ToolContext};
use futures::{FutureExt, future::BoxFuture};

#[cfg(feature = "sam")]
mod sam;

pub struct Tools {
    active_tool_idx: usize,
    tool_factories: ToolFactories,
    tool: AsyncRefTask<Result<Box<dyn Tool + Send>, String>>,
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
    pub(super) fn load_tool(&mut self, img: &ImageLoadOk) {
        let (name, factory) = &mut self.tool_factories[self.active_tool_idx];
        log::debug!("Loading tool: {name}");
        self.tool = AsyncRefTask::new(factory(img));
    }

    pub(super) fn new(tool_factories: ToolFactories) -> Self {
        Self {
            active_tool_idx: 0,
            tool: AsyncRefTask::new_ready(Ok(Box::new(NopTool))),
            tool_factories,
        }
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui, img: &ImageLoadOk) {
        let before = self.active_tool_idx;
        egui::ComboBox::from_label("Tool")
            .selected_text(&self.tool_factories[self.active_tool_idx].0)
            .show_ui(ui, |ui| {
                for (i, (name, _)) in self.tool_factories.iter().enumerate() {
                    ui.selectable_value(&mut self.active_tool_idx, i, name);
                }
            });
        if before != self.active_tool_idx {
            self.load_tool(img);
        };
    }
}

impl super::ImageViewerApp {
    pub(super) fn handle_tool_interaction(
        &mut self,
        response: egui::Response,
        cursor_image_pos: (usize, usize),
        ctx: &egui::Context,
    ) {
        if let (ImageState::Loaded(image), Some(Ok(tool))) =
            (&mut self.image_state, self.tools.tool.data())
        {
            tool.handle_interaction(ToolContext::new(image, response, cursor_image_pos, ctx));
        }
    }
}

/// Used to swap the tool in and out of the tools vector to borrow ImageViewerApp mutably
struct NopTool;
impl Tool for NopTool {
    fn handle_interaction(&mut self, _ctx: ToolContext) {
        log::warn!("NopTool should not be called");
    }
}
