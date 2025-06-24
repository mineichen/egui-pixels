#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    annotation_tool::run_native(Vec::new())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    annotation_tool::run_web(Vec::new());
}
