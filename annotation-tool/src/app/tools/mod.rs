use egui_pixels::{AsyncRefTask, ImageLoadOk, ImageState, ImageStateLoaded};
use futures::{FutureExt, future::BoxFuture};

mod clear;
#[cfg(feature = "sam")]
mod sam;

pub struct Tools {
    active_tool_idx: usize,
    tool_factories: ToolFactories,
    tool: AsyncRefTask<Result<Box<dyn Tool + Send>, String>>,
}

#[derive(Default)]
pub struct RectSelection {
    last_drag_start: Option<(usize, usize)>,
}

impl RectSelection {
    pub fn new() -> Self {
        Self {
            last_drag_start: None,
        }
    }

    pub fn drag_stopped(&mut self, ctx: &mut ToolContext) -> Option<[[usize; 2]; 2]> {
        let (start_x, start_y) = ctx.cursor_image_pos;
        let result = if let (Some((cursor_x, cursor_y)), true) = (
            self.last_drag_start,
            ctx.response.drag_stopped()
                && !ctx.egui.input(|i| i.modifiers.command || i.modifiers.ctrl),
        ) {
            self.last_drag_start = None;
            Some([
                [cursor_x.min(start_x), cursor_y.min(start_y)],
                [cursor_x.max(start_x), cursor_y.max(start_y)],
            ])
        } else {
            None
        };

        if ctx.egui.input(|i| !i.pointer.primary_down()) {
            self.last_drag_start = Some(ctx.cursor_image_pos);
        }
        result
    }
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
                async { Ok(Box::new(clear::ClearTool::default()) as Box<dyn Tool + Send>) }.boxed()
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
    pub(super) fn handle_interaction(
        &mut self,
        response: egui::Response,
        cursor_image_pos: (usize, usize),
        ctx: &egui::Context,
    ) {
        if let ImageState::Loaded(image) = &mut self.image_state {
            let mut temp_tool: Box<dyn Tool + Send> = Box::new(NopTool);
            if let Some(Ok(tool)) = &mut self.tools.tool.data() {
                std::mem::swap(&mut temp_tool, tool);
            }

            temp_tool.handle_interaction(ToolContext {
                image,
                response,
                cursor_image_pos,
                egui: ctx,
            });

            self.tools.tool = AsyncRefTask::new_ready(Ok(temp_tool));
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

pub struct ToolContext<'a> {
    pub image: &'a mut ImageStateLoaded,
    pub response: egui::Response,
    pub cursor_image_pos: (usize, usize),
    pub egui: &'a egui::Context,
}

pub trait Tool {
    fn handle_interaction(&mut self, ctx: ToolContext);
}
