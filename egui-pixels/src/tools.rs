use futures::FutureExt;

use crate::{AsyncRefTask, ClearTool, ImageLoadOk, PanTool, Tool, ToolTask};

/// Tool factory function that creates a tool for a given image
pub type ToolFactory =
    Box<dyn Fn(&ImageLoadOk) -> crate::BoxFuture<'static, Result<Box<dyn Tool + Send>, String>>>;
type ToolFactories = Vec<(String, ToolFactory)>;

/// Core tool management without UI concerns
pub struct Tools {
    tool_factories: ToolFactories,
    primary_idx: usize,
    primary_tool: ToolTask,
    secondary_idx: usize,
    secondary_tool: ToolTask,
}

pub struct ToolHandle<'a> {
    idx: &'a mut usize,
    tool: &'a mut ToolTask,
    tool_factories: &'a mut ToolFactories,
}

impl<'a> ToolHandle<'a> {
    pub fn load(&mut self, img: &ImageLoadOk) {
        let (name, factory) = &mut self.tool_factories[*self.idx];
        log::debug!("Loading tool: {name}");
        *self.tool = AsyncRefTask::new(factory(img));
    }
    /// Get the index of the currently active primary tool
    pub fn idx(&self) -> usize {
        *self.idx
    }
    /// Set the active primary tool by index
    /// Returns true if the tool changed and needs to be loaded
    pub fn set_idx(&mut self, idx: usize, img: &ImageLoadOk) {
        if idx < self.tool_factories.len() && idx != *self.idx {
            *self.idx = idx;
            self.load(img);
        }
    }

    pub fn name(&self) -> &str {
        &self.tool_factories[*self.idx].0
    }
    pub fn data(&mut self) -> Option<&mut Result<Box<dyn Tool + Send + 'static>, String>> {
        self.tool.data()
    }
    /// Get the list of available tool names
    pub fn tool_names(&self) -> impl Iterator<Item = &str> {
        self.tool_factories.iter().map(|(name, _)| name.as_str())
    }
}

impl Tools {
    /// Create a new Tools instance with the given tool factories
    /// The first non-Pan tool will be selected as primary, and Pan as secondary
    pub fn new(tool_factories: Vec<(String, ToolFactory)>) -> Self {
        let tool_factories: ToolFactories = match tool_factories.len() {
            0 => vec![
                (
                    "Nop".to_string(),
                    Box::new(|_| async { Ok(Box::new(NopTool) as Box<dyn Tool + Send>) }.boxed()),
                ),
                (
                    "Pan".to_string(),
                    Box::new(|_| {
                        async { Ok(Box::new(PanTool::default()) as Box<dyn Tool + Send>) }.boxed()
                    }),
                ),
            ],
            1 => {
                let mut tool_factories = tool_factories;
                tool_factories.push((
                    "Pan".to_string(),
                    Box::new(|_| {
                        async { Ok(Box::new(PanTool::default()) as Box<dyn Tool + Send>) }.boxed()
                    }),
                ));
                tool_factories
            }
            _ => tool_factories,
        };

        Self {
            tool_factories,
            primary_idx: 0,
            primary_tool: AsyncRefTask::new_ready(Ok(Box::new(NopTool))),
            secondary_idx: 1,
            secondary_tool: AsyncRefTask::new_ready(Ok(Box::new(PanTool::default()))),
        }
    }

    pub fn primary(&mut self) -> ToolHandle<'_> {
        ToolHandle {
            idx: &mut self.primary_idx,
            tool: &mut self.primary_tool,
            tool_factories: &mut self.tool_factories,
        }
    }
    pub fn secondary(&mut self) -> ToolHandle<'_> {
        ToolHandle {
            idx: &mut self.secondary_idx,
            tool: &mut self.secondary_tool,
            tool_factories: &mut self.tool_factories,
        }
    }
}

/// A no-operation tool used as placeholder
struct NopTool;
impl Tool for NopTool {
    fn handle_interaction(&mut self, _ctx: crate::ToolContext) {
        log::debug!("NopTool was called");
    }
}
