//! Physics debug overlay pass. Draws world-space line and point primitives
//! (collision wireframes, velocity/force arrows, contact markers, BVH boxes, …)
//! over the finished 3D frame, in a separate `LoadOp::Load` pass so the overlay
//! composites on top and can be toggled per layer at runtime.
//!
//! Two pipelines share one shader and one camera uniform: a **line-list** for
//! all wireframe/arrow geometry and a **point-list** for marker dots. The app
//! builds the vertex data from the physics crate's per-frame `DebugDraw`; this
//! pass owns the GPU buffers and **reuses** them across frames, growing only
//! when a frame needs more vertices than the current capacity holds.

use elderforge_core::math::Mat4;

pub const SHADER_SOURCE: &str = include_str!("../shaders/debug.wgsl");

/// One debug vertex: a world-space position and an RGBA color. Shared by the
/// line-list and point-list pipelines (a line is two of these, a point is one).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DebugVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl DebugVertex {
    pub fn new(position: [f32; 3], color: [f32; 4]) -> Self {
        Self { position, color }
    }

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
            0 => Float32x3, // position
            1 => Float32x4, // color
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DebugVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

const MAT4_SIZE: u64 = 64;
/// Vertices a freshly-created buffer can hold before it must grow.
const INITIAL_CAPACITY: u64 = 1024;

/// Renders debug line/point overlays. Owns both pipelines, the shared camera
/// uniform, and the reusable line and point vertex buffers.
pub struct DebugPass {
    line_pipeline: wgpu::RenderPipeline,
    point_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    line_buffer: GrowBuffer,
    point_buffer: GrowBuffer,
}

impl DebugPass {
    /// Build the pass for a target of `color_format`. The overlay always renders
    /// single-sampled over the resolved surface (no MSAA, no depth) so it sits
    /// cleanly on top of the 3D frame regardless of the scene pass's sample count.
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("debug.camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(MAT4_SIZE),
                },
                count: None,
            }],
        });
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("debug.camera.buffer"),
            size: MAT4_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("debug.camera.bind_group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("debug"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("debug"),
            bind_group_layouts: &[&camera_layout],
            push_constant_ranges: &[],
        });
        let line_pipeline = make_pipeline(
            device,
            &layout,
            &shader,
            color_format,
            wgpu::PrimitiveTopology::LineList,
        );
        let point_pipeline = make_pipeline(
            device,
            &layout,
            &shader,
            color_format,
            wgpu::PrimitiveTopology::PointList,
        );

        Self {
            line_pipeline,
            point_pipeline,
            camera_buffer,
            camera_bind_group,
            line_buffer: GrowBuffer::new(device, "debug.lines", INITIAL_CAPACITY),
            point_buffer: GrowBuffer::new(device, "debug.points", INITIAL_CAPACITY),
        }
    }

    /// Record the overlay over `view` (loaded, not cleared) under `view_proj`.
    /// `lines` is a flat list of segment endpoints (two vertices per segment);
    /// `points` is one vertex per marker. A frame with no geometry is a no-op.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        view_proj: Mat4,
        lines: &[DebugVertex],
        points: &[DebugVertex],
    ) {
        if lines.is_empty() && points.is_empty() {
            return;
        }
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&view_proj.to_cols_array()));
        self.line_buffer.upload(device, queue, lines);
        self.point_buffer.upload(device, queue, points);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("debug"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        if !lines.is_empty() {
            pass.set_pipeline(&self.line_pipeline);
            pass.set_vertex_buffer(0, self.line_buffer.buffer.slice(..));
            pass.draw(0..lines.len() as u32, 0..1);
        }
        if !points.is_empty() {
            pass.set_pipeline(&self.point_pipeline);
            pass.set_vertex_buffer(0, self.point_buffer.buffer.slice(..));
            pass.draw(0..points.len() as u32, 0..1);
        }
    }
}

/// A vertex buffer that is reused across frames and only reallocated when a
/// frame's vertex count exceeds its capacity (then grown to the next power of
/// two). This is the "don't allocate per-frame" buffer.
struct GrowBuffer {
    label: String,
    buffer: wgpu::Buffer,
    /// Capacity in vertices.
    capacity: u64,
}

impl GrowBuffer {
    fn new(device: &wgpu::Device, label: &str, capacity: u64) -> Self {
        Self {
            label: label.to_string(),
            buffer: make_vertex_buffer(device, label, capacity),
            capacity,
        }
    }

    /// Stream `verts` into the buffer, growing it first if needed.
    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, verts: &[DebugVertex]) {
        if verts.is_empty() {
            return;
        }
        if verts.len() as u64 > self.capacity {
            self.capacity = (verts.len() as u64).next_power_of_two();
            self.buffer = make_vertex_buffer(device, &self.label, self.capacity);
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(verts));
    }
}

fn make_vertex_buffer(device: &wgpu::Device, label: &str, capacity: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: capacity * std::mem::size_of::<DebugVertex>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Build a debug pipeline for the given primitive topology: alpha-blended, no
/// depth test (overlays draw on top), no culling.
fn make_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    topology: wgpu::PrimitiveTopology,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("debug"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            buffers: &[DebugVertex::layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
