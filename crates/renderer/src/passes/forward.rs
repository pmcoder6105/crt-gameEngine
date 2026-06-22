//! Forward scene pass: draws meshes positioned by per-object model matrices
//! under a shared camera, with depth testing. A bridge between the bootstrap
//! `unlit` pass and the real PBR pipeline — enough to render scene geometry.

use elderforge_core::math::Mat4;

use crate::mesh::{GpuMesh, Vertex};
use crate::pipeline::{PipelineBuilder, DEPTH_FORMAT};

/// Default background clear color (linear). A dim blue-grey sky.
pub const DEFAULT_CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.05,
    g: 0.06,
    b: 0.09,
    a: 1.0,
};

/// One mesh to draw, with the world transform to place it.
pub struct Draw<'a> {
    pub model: Mat4,
    pub mesh: &'a GpuMesh,
}

/// A directional light: the direction *toward* the light and its color/tint.
#[derive(Debug, Clone, Copy)]
pub struct DirectionalLight {
    pub direction: elderforge_core::math::Vec3,
    pub color: elderforge_core::math::Vec3,
}

impl Default for DirectionalLight {
    /// The engine's original hard-coded key light: from above and slightly to
    /// the side, neutral white. Reproduces the look from before the light was
    /// made configurable.
    fn default() -> Self {
        Self {
            direction: elderforge_core::math::Vec3::new(0.3, 0.9, 0.35),
            color: elderforge_core::math::Vec3::ONE,
        }
    }
}

/// Owns the forward pipeline, the camera and per-object uniform buffers, and
/// the depth target. Recreate the depth target on resize via [`Self::resize`].
pub struct ForwardPass {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    model_layout: wgpu::BindGroupLayout,
    model_buffer: wgpu::Buffer,
    model_bind_group: wgpu::BindGroup,
    /// Number of model slots the buffer holds; grows as the scene grows.
    model_capacity: u32,
    /// Per-slot stride in bytes (a model matrix padded to the uniform offset
    /// alignment required for dynamic offsets).
    model_stride: u64,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    /// Surface color format, kept so MSAA / depth targets can be rebuilt on
    /// resize.
    color_format: wgpu::TextureFormat,
    /// MSAA sample count. When `> 1`, geometry renders into [`msaa_view`] and
    /// resolves into the surface view; when `1`, it renders directly.
    sample_count: u32,
    /// Multisampled color target, present only when `sample_count > 1`.
    msaa_view: Option<wgpu::TextureView>,
    /// Background clear color (linear).
    clear_color: wgpu::Color,
    /// Scene key light, written into the camera uniform each frame.
    light: DirectionalLight,
}

const MAT4_SIZE: u64 = 64;
/// Camera/globals uniform: view-projection matrix (64) + light direction
/// vec4 (16) + light color vec4 (16).
const GLOBALS_SIZE: u64 = 96;

impl ForwardPass {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        size: (u32, u32),
        sample_count: u32,
    ) -> Self {
        let sample_count = sample_count.max(1);
        // Camera/globals uniform (group 0): view-projection plus the key light.
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("forward.camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(GLOBALS_SIZE),
                },
                count: None,
            }],
        });
        // Per-object model uniform (group 1): one matrix per draw, addressed by
        // a dynamic offset.
        let model_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("forward.model"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(MAT4_SIZE),
                },
                count: None,
            }],
        });

        let pipeline = PipelineBuilder::new("forward", include_str!("../shaders/forward.wgsl"))
            .depth_test(true)
            .sample_count(sample_count)
            .build(
                device,
                color_format,
                &[Vertex::layout()],
                &[&camera_layout, &model_layout],
            );

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("forward.camera.buffer"),
            size: GLOBALS_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("forward.camera.bind_group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Dynamic-offset uniforms must be aligned to this; pad each slot up.
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let model_stride = MAT4_SIZE.div_ceil(alignment) * alignment;
        let model_capacity = 64;
        let (model_buffer, model_bind_group) =
            make_model_buffer(device, &model_layout, model_stride, model_capacity);

        Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            model_layout,
            model_buffer,
            model_bind_group,
            model_capacity,
            model_stride,
            depth_view: make_depth(device, size, sample_count),
            depth_size: size,
            color_format,
            sample_count,
            msaa_view: make_msaa(device, color_format, size, sample_count),
            clear_color: DEFAULT_CLEAR_COLOR,
            light: DirectionalLight::default(),
        }
    }

    /// Set the background clear color (linear). Demo capture uses pure black.
    pub fn set_clear_color(&mut self, color: wgpu::Color) {
        self.clear_color = color;
    }

    /// Set the scene's key light. Takes effect on the next [`render`](Self::render).
    pub fn set_light(&mut self, light: DirectionalLight) {
        self.light = light;
    }

    /// Recreate the depth (and MSAA color) targets for a new surface size. No-op
    /// for zero-size (minimized) windows or when the size is unchanged.
    pub fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if size.0 == 0 || size.1 == 0 || size == self.depth_size {
            return;
        }
        self.depth_view = make_depth(device, size, self.sample_count);
        self.msaa_view = make_msaa(device, self.color_format, size, self.sample_count);
        self.depth_size = size;
    }

    /// Record the forward pass: clear color+depth, then draw every item under
    /// `view_proj`. Uniform writes are queued before the pass is recorded.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        view_proj: Mat4,
        draws: &[Draw],
    ) {
        // Grow the per-object buffer if the scene outgrew it.
        if draws.len() as u32 > self.model_capacity {
            self.model_capacity = (draws.len() as u32).next_power_of_two();
            let (buffer, bind_group) = make_model_buffer(
                device,
                &self.model_layout,
                self.model_stride,
                self.model_capacity,
            );
            self.model_buffer = buffer;
            self.model_bind_group = bind_group;
        }

        // Globals = view-projection (16) + light direction (4) + light color (4).
        let dir = self.light.direction.normalize_or_zero();
        let mut globals = [0.0f32; 24];
        globals[..16].copy_from_slice(&view_proj.to_cols_array());
        globals[16..19].copy_from_slice(&dir.to_array());
        globals[20..23].copy_from_slice(&self.light.color.to_array());
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&globals));
        for (i, draw) in draws.iter().enumerate() {
            let offset = i as u64 * self.model_stride;
            queue.write_buffer(&self.model_buffer, offset, bytemuck::bytes_of(&draw.model.to_cols_array()));
        }

        // With MSAA, draw into the multisampled target and resolve into the
        // surface view; without it, draw straight into the surface view.
        let (color_attachment, resolve_target) = match &self.msaa_view {
            Some(msaa) => (msaa, Some(color_view)),
            None => (color_view, None),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("forward"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_attachment,
                resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        for (i, draw) in draws.iter().enumerate() {
            let offset = (i as u64 * self.model_stride) as u32;
            pass.set_bind_group(1, &self.model_bind_group, &[offset]);
            pass.set_vertex_buffer(0, draw.mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(draw.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..draw.mesh.index_count, 0, 0..1);
        }
    }
}

/// Allocate the dynamic-offset model uniform buffer and its bind group.
fn make_model_buffer(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    stride: u64,
    capacity: u32,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("forward.model.buffer"),
        size: stride * capacity as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("forward.model.bind_group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            // One slot's worth is visible at the bound dynamic offset.
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(MAT4_SIZE),
            }),
        }],
    });
    (buffer, bind_group)
}

/// Create a depth texture/view sized to the surface, at the given MSAA sample
/// count (it must match the color attachments in the same pass).
fn make_depth(device: &wgpu::Device, size: (u32, u32), sample_count: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("forward.depth"),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Create the multisampled color target the geometry renders into before
/// resolving to the surface. Returns `None` when `sample_count == 1` (no MSAA,
/// geometry renders straight to the surface).
fn make_msaa(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    sample_count: u32,
) -> Option<wgpu::TextureView> {
    if sample_count <= 1 {
        return None;
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("forward.msaa"),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    Some(texture.create_view(&wgpu::TextureViewDescriptor::default()))
}
