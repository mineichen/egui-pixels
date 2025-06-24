#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    egui_pixels::run_native(Vec::new())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    egui_pixels::run_web(Vec::new());
}
