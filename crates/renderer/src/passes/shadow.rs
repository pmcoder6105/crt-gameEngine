//! Cascaded shadow map pass.

pub const SHADER_SOURCE: &str = include_str!("../shaders/shadow.wgsl");
pub const CASCADE_COUNT: u32 = 4;

pub struct ShadowPass {
    pub pipeline: Option<wgpu::RenderPipeline>,
    pub cascade_count: u32,
}

impl ShadowPass {
    pub fn new() -> Self {
        Self {
            pipeline: None,
            cascade_count: CASCADE_COUNT,
        }
    }

    // TODO: per-cascade depth targets + light-space matrices.
}

impl Default for ShadowPass {
    fn default() -> Self {
        Self::new()
    }
}
