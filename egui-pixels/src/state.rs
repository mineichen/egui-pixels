use std::io;

use egui::{InnerResponse, Sense};

use crate::{
    BoxFuture, CursorImage, CursorImageSystem, ImageData, ImageLoadOk, ImageViewer,
    ImageViewerInteraction, Tools,
};

/// State container for handling tool interactions with the image viewer.
/// Contains all the necessary components to process tool events and render tools on the image.
#[non_exhaustive]
pub struct State {
    pub image_state: crate::ImageState,
    pub viewer: ImageViewer,
    pub tools: Tools,
    pub cursor_image: CursorImageSystem,
}

impl State {
    /// Create a new State with the given tools
    pub fn new(tools: Tools) -> Self {
        Self {
            image_state: crate::ImageState::NotLoaded,
            viewer: ImageViewer::default(),
            tools,
            cursor_image: CursorImageSystem::from(Box::new(|_: Option<&CursorImage>| {
                #[cfg(target_arch = "wasm32")]
                log::warn!(
                    "WebCursors have to be enabled manually with `state.cursor_image.enable_web(canvas), probably in your egui::Webrunner::start() callback`"
                );
            })),
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

    pub fn ui(&mut self, ui: &mut egui::Ui) -> InnerResponse<Option<ImageViewerInteraction>> {
        let InnerResponse { inner, response } =
            self.viewer
                .ui(ui, self.image_state.sources(ui.ctx()), Some(Sense::click()));
        let result = InnerResponse {
            inner: if let Some((mut r)) = inner {
                self.handle_tool_interaction(&response, ui.ctx(), &mut r.image_painter);
                Some(r)
            } else {
                None
            },
            response,
        };
        self.cursor_image.apply(
            result
                .inner
                .as_ref()
                .and_then(|r| r.cursor_image_pos)
                .is_some(),
        );

        result
    }

    /// Handle tool interaction based on user input.
    /// This method checks which tool should be active (primary or secondary based on modifier keys),
    /// and delegates the interaction handling to the appropriate tool.
    pub fn handle_tool_interaction(
        &mut self,
        response: &egui::Response,
        ctx: &egui::Context,
        tool_painter: &mut crate::ImagePainter,
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
                    &mut self.cursor_image,
                ));
            }
        }
    }
}
