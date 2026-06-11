//! Render pipeline builder helpers.

pub struct PipelineBuilder<'a> {
    pub label: &'a str,
    pub shader_source: &'a str,
    pub depth_test: bool,
}

impl<'a> PipelineBuilder<'a> {
    pub fn new(label: &'a str, shader_source: &'a str) -> Self {
        Self {
            label,
            shader_source,
            depth_test: true,
        }
    }

    // TODO: build(device, bind group layouts, color targets) -> wgpu::RenderPipeline
}
