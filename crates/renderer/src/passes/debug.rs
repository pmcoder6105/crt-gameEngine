//! Physics debug overlay pass. Renders line lists handed over by the app
//! (built from the physics crate's per-frame DebugDraw data), in a separate
//! pass over the PBR output so overlays can be toggled at runtime.

pub const SHADER_SOURCE: &str = include_str!("../shaders/debug.wgsl");

pub struct DebugPass {
    pub pipeline: Option<wgpu::RenderPipeline>,
}

impl DebugPass {
    pub fn new() -> Self {
        Self { pipeline: None }
    }

    // TODO: dynamic vertex buffer for line data, rebuilt each frame.
}

impl Default for DebugPass {
    fn default() -> Self {
        Self::new()
    }
}
