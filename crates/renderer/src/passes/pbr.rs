//! Main PBR geometry pass with IBL lighting.

pub const SHADER_SOURCE: &str = include_str!("../shaders/pbr.wgsl");

pub struct PbrPass {
    pub pipeline: Option<wgpu::RenderPipeline>,
}

impl PbrPass {
    pub fn new() -> Self {
        Self { pipeline: None }
    }

    // TODO: prepare(device, surface_format) builds the pipeline;
    // record(encoder, view, draw_list, cache) draws the scene.
}

impl Default for PbrPass {
    fn default() -> Self {
        Self::new()
    }
}
