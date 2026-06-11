//! egui render pass, drawn last over the frame.

pub struct UiPass {
    pub renderer: Option<egui_wgpu::Renderer>,
}

impl UiPass {
    pub fn new() -> Self {
        Self { renderer: None }
    }

    // TODO: prepare(device, surface_format) creates the egui_wgpu::Renderer;
    // record(encoder, view, clipped_primitives, textures_delta) draws the UI.
}

impl Default for UiPass {
    fn default() -> Self {
        Self::new()
    }
}
