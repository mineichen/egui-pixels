use std::sync::Arc;

use futures::FutureExt;

use crate::{AsyncRefTask, ImageLoadOk, PanTool, Tool, ToolTask};

/// Tool factory function that creates a tool for a given image.
/// Cheaply cloneable (`Arc`) so it can live in a tool registry while also being
/// handed to the active primary/secondary slots.
pub type ToolFactory =
    Arc<dyn Fn(&ImageLoadOk) -> crate::LocalBoxFuture<'static, Result<Box<dyn Tool>, String>>>;

/// Core tool management without UI concerns.
///
/// Holds a single [`ToolFactory`] per slot (primary/secondary) so tools can be
/// recreated whenever the underlying image changes. The registry of available
/// tools and their display names lives in the embedding application.
pub struct Tools {
    primary_factory: ToolFactory,
    primary_tool: ToolTask,
    secondary_factory: ToolFactory,
    secondary_tool: ToolTask,
}

pub struct ToolHandle<'a> {
    factory: &'a mut ToolFactory,
    tool: &'a mut ToolTask,
}

impl<'a> ToolHandle<'a> {
    /// (Re)create the tool from its factory for the given image.
    pub fn load(&mut self, img: &ImageLoadOk) {
        log::debug!("Loading tool");
        *self.tool = AsyncRefTask::new((self.factory)(img));
    }
    /// Replace the slot's factory and immediately (re)load with the given image.
    /// Returns true if the factory actually changed.
    pub fn set_factory(&mut self, factory: ToolFactory, img: &ImageLoadOk) -> bool {
        if Arc::ptr_eq(self.factory, &factory) {
            return false;
        }
        *self.factory = factory;
        self.load(img);
        true
    }
    pub fn data(&mut self) -> Option<&mut Result<Box<dyn Tool + 'static>, String>> {
        self.tool.data()
    }
}

impl Default for Tools {
    fn default() -> Self {
        Self::new(nop_factory(), PanTool::create_factory())
    }
}

impl Tools {
    /// Create a new Tools instance with the given primary and secondary factories.
    /// The actual tool instances are created lazily on the first [`ToolHandle::load`]
    /// (e.g. when an image becomes available); until then placeholder tools are used.
    pub fn new(primary_factory: ToolFactory, secondary_factory: ToolFactory) -> Self {
        Self {
            primary_factory,
            primary_tool: AsyncRefTask::new_ready(Ok(Box::new(NopTool))),
            secondary_factory,
            secondary_tool: AsyncRefTask::new_ready(Ok(Box::new(PanTool::default()))),
        }
    }

    pub(crate) fn load(&mut self, i: &ImageLoadOk) {
        self.primary().load(i);
        self.secondary().load(i);
    }

    pub fn primary(&mut self) -> ToolHandle<'_> {
        let [p, _s] = self.handles();
        p
    }
    pub fn secondary(&mut self) -> ToolHandle<'_> {
        let [_p, s] = self.handles();
        s
    }
    pub fn handles(&mut self) -> [ToolHandle<'_>; 2] {
        [
            ToolHandle {
                factory: &mut self.primary_factory,
                tool: &mut self.primary_tool,
            },
            ToolHandle {
                factory: &mut self.secondary_factory,
                tool: &mut self.secondary_tool,
            },
        ]
    }
}

fn nop_factory() -> ToolFactory {
    Arc::new(|_| async { Ok(Box::new(NopTool) as Box<dyn Tool>) }.boxed_local())
}

/// A no-operation tool used as placeholder
struct NopTool;
impl Tool for NopTool {
    fn handle_interaction(&mut self, _ctx: crate::ToolContext) {
        log::debug!("NopTool was called");
    }
}
