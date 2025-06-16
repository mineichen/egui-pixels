use eframe::egui;
use futures::{future::BoxFuture, FutureExt};

use crate::{
    app::tools::sam::SamTool, async_task::AsyncRefTask, image_utils::ImageLoadOk,
    inference::SamSession,
};

mod clear;
mod sam;

pub struct Tools {
    last_drag_start: Option<(usize, usize)>,
    active_tool_idx: usize,
    tool_factories: ToolFactories,
    tool: AsyncRefTask<Result<Box<dyn Tool + Send>, String>>,
}

type ToolFactories = Vec<(
    String,
    Box<dyn Fn(&ImageLoadOk) -> BoxFuture<'static, Result<Box<dyn Tool + Send>, String>>>,
)>;

pub fn default_tools(session: SamSession) -> ToolFactories {
    vec![
        (
            "SAM".to_string(),
            Box::new(move |img| {
                let tool = SamTool::new(session.clone(), img.adjust.clone());
                async move { Ok(Box::new(tool) as Box<dyn Tool + Send>) }.boxed()
            }),
        ),
        (
            "Clear".to_string(),
            Box::new(|_| {
                async { Ok(Box::new(clear::ClearTool::default()) as Box<dyn Tool + Send>) }.boxed()
            }),
        ),
    ]
}

impl<'a> From<&'a crate::config::Config> for Tools {
    fn from(config: &'a crate::config::Config) -> Self {
        let session = SamSession::new(&config.sam_path).unwrap();
        Self::new(default_tools(session))
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
            last_drag_start: None,
            active_tool_idx: 0,
            tool: AsyncRefTask::new_ready(Ok(Box::new(NopTool))),
            tool_factories,
        }
    }

    fn drag_stopped(
        &mut self,
        (start_x, start_y): (usize, usize),
        response: &egui::Response,
        ctx: &egui::Context,
    ) -> Option<[[usize; 2]; 2]> {
        if let (Some((cursor_x, cursor_y)), true) = (
            self.last_drag_start,
            response.drag_stopped() && !ctx.input(|i| i.modifiers.command || i.modifiers.ctrl),
        ) {
            self.last_drag_start = None;
            Some([
                [cursor_x.min(start_x), cursor_y.min(start_y)],
                [cursor_x.max(start_x), cursor_y.max(start_y)],
            ])
        } else {
            None
        }
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui, img: &ImageLoadOk) {
        let before = self.active_tool_idx;
        egui::ComboBox::from_label("Tool")
            .selected_text(&self.tool_factories[self.active_tool_idx].0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.active_tool_idx, 0, "Sam");
                ui.selectable_value(&mut self.active_tool_idx, 1, "Clear");
            });
        if before != self.active_tool_idx {
            self.load_tool(img);
        };
    }
}

impl super::ImageViewerApp {
    pub(super) fn handle_interaction(
        &mut self,
        response: egui::Response,
        cursor_image_pos: (usize, usize),
        ctx: &egui::Context,
    ) {
        let mut temp_tool: Box<dyn Tool + Send> = Box::new(NopTool);
        if let Some(Ok(ref mut tool)) = &mut self.tools.tool.data() {
            std::mem::swap(&mut temp_tool, tool);
        }

        temp_tool.handle_interaction(self, response, cursor_image_pos, ctx);

        self.tools.tool = AsyncRefTask::new_ready(Ok(temp_tool));

        if ctx.input(|i| !i.pointer.primary_down()) {
            self.tools.last_drag_start = Some(cursor_image_pos);
        }
    }
}

/// Used to swap the tool in and out of the tools vector to borrow ImageViewerApp mutably
struct NopTool;
impl Tool for NopTool {
    fn handle_interaction(
        &mut self,
        _app: &mut super::ImageViewerApp,
        _response: egui::Response,
        _cursor_image_pos: (usize, usize),
        _ctx: &egui::Context,
    ) {
        log::warn!("NopTool should not be called");
    }
}
pub trait Tool {
    fn handle_interaction(
        &mut self,
        app: &mut super::ImageViewerApp,
        response: egui::Response,
        cursor_image_pos: (usize, usize),
        ctx: &egui::Context,
    );
}
