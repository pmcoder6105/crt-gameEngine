//! Render pipeline builder helpers.

/// Depth format used by every depth-tested pipeline and depth texture in
/// the engine.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Builds a render pipeline from WGSL source with the engine's conventions:
/// entry points `vs_main`/`fs_main`, triangle lists, no culling.
///
/// Pipelines built with `depth_test` enabled target [`DEPTH_FORMAT`] and
/// must be used in passes with a matching depth attachment.
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

    pub fn depth_test(mut self, enabled: bool) -> Self {
        self.depth_test = enabled;
        self
    }

    /// Creates the pipeline targeting `color_format`.
    // TODO: bind group layouts once passes need uniforms/textures.
    pub fn build(
        &self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        vertex_layouts: &[wgpu::VertexBufferLayout],
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(self.label),
            source: wgpu::ShaderSource::Wgsl(self.shader_source.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(self.label),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(self.label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: vertex_layouts,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(color_format.into())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: self.depth_test.then(|| wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }
}
