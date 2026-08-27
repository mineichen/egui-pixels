use egui::{InnerResponse, Sense};

use crate::{
    CursorImage, CursorImageSystem, ImageData, ImageLoadOk, ImageState, ImageViewer,
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
    pub config: StateConfig,
    /// Set to `true` by the active tool to suspend loading of the next image.
    /// Reset to `false` before tools are called each frame.
    postpone_new_images: bool,
    /// Image whose loading was postponed by a tool, waiting until no tool
    /// requests suspension anymore. Overridden by each incoming image.
    pending_image: Option<ImageData>,
}

#[derive(Default)]
pub struct StateConfig {
    pub reset_viewport_on_image_load: bool,
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
            config: StateConfig::default(),
            postpone_new_images: false,
            pending_image: None,
        }
    }
    /// If a tool currently requests image postpone, the load is
    /// postponed and stored in a pending slot instead. Each incoming image
    /// overrides the pending one. The pending image is loaded as soon as no
    /// tool requests suspension anymore.
    pub fn set_image(&mut self, data: ImageData) {
        // Can only inherit stuff from current if Loaded()
        // Maybe, the History should be extracted from ImageState
        if matches!(self.image_state, ImageState::Loaded(_)) {
            log::debug!("Set image: postponeed: {:?}", self.postpone_new_images);
            self.pending_image = Some(data);
        } else {
            self.image_state = ImageState::new_with_image_data(data);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> InnerResponse<Option<ImageViewerInteraction>> {
        self.image_state.update(ui.ctx(), |i: &ImageLoadOk| {
            if self.config.reset_viewport_on_image_load {
                self.viewer.reset();
            }
            self.tools.load(i);
        });
        let InnerResponse { inner, response } =
            self.viewer
                .ui(ui, self.image_state.sources(ui.ctx()), Some(Sense::click()));
        let result = InnerResponse {
            inner: if let Some(mut r) = inner {
                if let crate::ImageState::Loaded(image) = &mut self.image_state {
                    image.masks.update_hover_layer(r.cursor_image_pos);
                }
                // Reset the suspension flag before tools are called, so that only atool actively setting it to `true` will cause image loading to be delayed.
                self.postpone_new_images = response.is_pointer_button_down_on();
                self.handle_tool_interaction(&response, ui.ctx(), &mut r.image_painter);
                // If no tool requested suspension, load any image that was
                // postponed while a tool held the mouse button down.
                if !self.postpone_new_images
                    && let Some(p) = self.pending_image.take()
                {
                    let tools = &mut self.tools;
                    let sr = self.image_state.set_image_data(p, ui.ctx(), tools);
                    if let Err(e) = sr {
                        self.image_state = ImageState::Error(e.to_string());
                    }
                }
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
                    &mut self.postpone_new_images,
                ));
            }
        }
    }
}
