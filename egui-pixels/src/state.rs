use std::io;

use crate::{BoxFuture, ImageData, ImageLoadOk, ImageViewer, Tools};

/// State container for handling tool interactions with the image viewer.
/// Contains all the necessary components to process tool events and render tools on the image.
#[non_exhaustive]
pub struct State {
    pub image_state: crate::ImageState,
    pub viewer: ImageViewer,
    pub tools: Tools,
}

impl State {
    /// Create a new State with the given tools
    pub fn new(tools: Tools) -> Self {
        Self {
            image_state: crate::ImageState::NotLoaded,
            viewer: ImageViewer::default(),
            tools,
        }
    }

    pub fn update(
        &mut self,
        ctx: &egui::Context,
        image_loader: &dyn Fn() -> BoxFuture<'static, io::Result<ImageData>>,
    ) {
        self.image_state.update(
            ctx,
            |i: &ImageLoadOk| {
                self.viewer.reset();
                self.tools.primary().load(&i);
                self.tools.secondary().load(&i);
            },
            image_loader,
        );
    }

    /// Handle tool interaction based on user input.
    /// This method checks which tool should be active (primary or secondary based on modifier keys),
    /// and delegates the interaction handling to the appropriate tool.
    pub fn handle_tool_interaction(
        &mut self,
        response: egui::Response,
        ctx: &egui::Context,
        tool_painter: crate::ToolPainter,
    ) {
        if let crate::ImageState::Loaded(image) = &mut self.image_state {
            // Check if CTRL is pressed to determine which tool to use
            let use_secondary = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);

            let mut tool_opt = if use_secondary {
                self.tools.secondary()
            } else {
                self.tools.primary()
            };

            if let Some(Ok(tool)) = tool_opt.data() {
                tool.handle_interaction(crate::ToolContext::new(
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
