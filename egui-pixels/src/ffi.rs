use egui::{InnerResponse, Sense};
use wasm_bindgen::prelude::*;

use crate::{
    ClearTool, ImageData, ImageStateLoaded, ImageViewer, ImageViewerInteraction, Tool, ToolContext,
};

#[wasm_bindgen]
pub struct App {
    image_state: ImageStateLoaded,
    viewer: ImageViewer,
    tool: Box<dyn Tool>,
}

#[wasm_bindgen]
pub fn run_web() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();
    log::info!("TEst");

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("image_viewer")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        log::info!("About to start eframe");

        let start_result = eframe::WebRunner::new()
            .start(canvas, web_options, Box::new(|cc| Ok(Box::new(App::new()))))
            .await;
        log::info!("ended eframe");

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

impl App {
    pub fn new() -> Self {
        let image = ImageData::chessboard().next().unwrap();
        let image_state = ImageStateLoaded::from_image_data(image, &egui::Context::default())
            .expect("Default image should load");
        Self {
            image_state,
            viewer: ImageViewer::default(),
            tool: Box::new(ClearTool::default()),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let InnerResponse {
                inner:
                    Some(ImageViewerInteraction {
                        original_image_size,
                        cursor_image_pos,
                        tool_painter,
                    }),
                response,
            } = self
                .viewer
                .ui_meta(ui, self.image_state.sources(ui.ctx()), Some(Sense::click()))
            {
                if let Some(cursor_image_pos) = cursor_image_pos {
                    self.tool.handle_interaction(ToolContext::new(
                        &mut self.image_state,
                        response,
                        cursor_image_pos,
                        ctx,
                        tool_painter,
                    ));
                }
                ui.label(format!(
                    "Original Size: ({original_image_size:?}), \navail: {:?}, \nspacing: {:?}",
                    original_image_size,
                    ui.spacing().item_spacing
                ));

                if let Some((x, y)) = cursor_image_pos {
                    ui.label(format!("Pixel Coordinates: ({}, {})", x, y,));
                }
            }
        });
    }
}
