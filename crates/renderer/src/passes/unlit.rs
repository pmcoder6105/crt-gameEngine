//! Unlit bootstrap pass: clears the frame and draws meshes with no camera,
//! no lighting, vertex "normals" as colors. Verifies the full surface ->
//! pipeline -> draw path; real scenes go through the PBR pass.

use crate::mesh::{GpuMesh, Vertex};
use crate::pipeline::PipelineBuilder;

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.012,
    g: 0.012,
    b: 0.022,
    a: 1.0,
};

/// Pipeline for unlit, untransformed geometry.
pub struct UnlitPass {
    pipeline: wgpu::RenderPipeline,
}

impl UnlitPass {
    /// Builds the unlit pipeline targeting the given surface format.
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let pipeline = PipelineBuilder::new("unlit", include_str!("../shaders/unlit.wgsl"))
            // No depth buffer exists yet; the pass has no depth attachment.
            .depth_test(false)
            .build(device, color_format, &[Vertex::layout()], &[]);
        Self { pipeline }
    }

    /// Records a pass that clears `view` and draws `mesh` into it.
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        mesh: &GpuMesh,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("unlit"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}
