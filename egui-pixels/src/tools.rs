use crate::{AsyncRefTask, ImageLoadOk, PanTool, Tool, ToolTask};

/// Tool factory function that creates a tool for a given image
pub type ToolFactory =
    Box<dyn Fn(&ImageLoadOk) -> crate::BoxFuture<'static, Result<Box<dyn Tool + Send>, String>>>;

/// Core tool management without UI concerns
pub struct Tools {
    active_primary_idx: usize,
    active_secondary_idx: usize,
    tool_factories: Vec<(String, ToolFactory)>,
    pub primary_tool: ToolTask,
    pub secondary_tool: ToolTask,
}

impl Tools {
    /// Create a new Tools instance with the given tool factories
    /// The first non-Pan tool will be selected as primary, and Pan as secondary
    pub fn new(tool_factories: Vec<(String, ToolFactory)>) -> Self {
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

    /// Load the primary tool for the given image
    pub fn load_primary_tool(&mut self, img: &ImageLoadOk) {
        let (name, factory) = &mut self.tool_factories[self.active_primary_idx];
        log::debug!("Loading primary tool: {name}");
        self.primary_tool = AsyncRefTask::new(factory(img));
    }

    /// Load the secondary tool for the given image
    pub fn load_secondary_tool(&mut self, img: &ImageLoadOk) {
        let (name, factory) = &mut self.tool_factories[self.active_secondary_idx];
        log::debug!("Loading secondary tool: {name}");
        self.secondary_tool = AsyncRefTask::new(factory(img));
    }

    /// Get the list of available tool names
    pub fn tool_names(&self) -> impl Iterator<Item = &str> {
        self.tool_factories.iter().map(|(name, _)| name.as_str())
    }

    /// Get the index of the currently active primary tool
    pub fn active_primary_idx(&self) -> usize {
        self.active_primary_idx
    }

    /// Get the index of the currently active secondary tool
    pub fn active_secondary_idx(&self) -> usize {
        self.active_secondary_idx
    }

    /// Set the active primary tool by index
    /// Returns true if the tool changed and needs to be loaded
    pub fn set_primary_idx(&mut self, idx: usize, img: &ImageLoadOk) {
        if idx < self.tool_factories.len() && idx != self.active_primary_idx {
            self.active_primary_idx = idx;
            self.load_primary_tool(img);
        }
    }

    /// Set the active secondary tool by index
    /// Returns true if the tool changed and needs to be loaded
    pub fn set_secondary_idx(&mut self, idx: usize, img: &ImageLoadOk) {
        if idx < self.tool_factories.len() && idx != self.active_secondary_idx {
            self.active_secondary_idx = idx;
            self.load_secondary_tool(img);
        }
    }

    /// Get the name of the active primary tool
    pub fn active_primary_name(&self) -> &str {
        &self.tool_factories[self.active_primary_idx].0
    }

    /// Get the name of the active secondary tool
    pub fn active_secondary_name(&self) -> &str {
        &self.tool_factories[self.active_secondary_idx].0
    }
}

/// A no-operation tool used as placeholder
struct NopTool;
impl Tool for NopTool {
    fn handle_interaction(&mut self, _ctx: crate::ToolContext) {
        log::debug!("NopTool was called");
    }
}
