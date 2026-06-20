//! GPU mesh data and vertex layout.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub tangent: [f32; 4],
}

impl Vertex {
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
        3 => Float32x4,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    /// Number of `Vertex` slots the vertex buffer holds. For a static mesh this
    /// equals the uploaded vertex count; for a dynamic mesh it is the capacity
    /// [`update_vertices`](Self::update_vertices) may not exceed.
    pub vertex_capacity: u32,
}

impl GpuMesh {
    /// Uploads vertex and index data to the GPU. Indices are `u32`
    /// throughout the engine; passes bind with `IndexFormat::Uint32`.
    pub fn upload(
        device: &wgpu::Device,
        label: &str,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> Self {
        use wgpu::util::DeviceExt;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label}.vertices")),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label}.indices")),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            vertex_capacity: vertices.len() as u32,
        }
    }

    /// Uploads a mesh whose vertices are rewritten every frame (soft bodies,
    /// cloth). The index buffer is static (topology is fixed); the vertex buffer
    /// is `COPY_DST` so [`update_vertices`](Self::update_vertices) can stream new
    /// positions/normals into it. The initial `vertices` set the capacity.
    pub fn upload_dynamic(
        device: &wgpu::Device,
        label: &str,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> Self {
        use wgpu::util::DeviceExt;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label}.vertices.dynamic")),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label}.indices")),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            vertex_capacity: vertices.len() as u32,
        }
    }

    /// Overwrite the vertex buffer of a dynamic mesh with new data (same layout,
    /// same or fewer vertices than the capacity). The index buffer is untouched,
    /// so `vertices` must keep the topology the indices reference. Vertices past
    /// the capacity are dropped (the buffer is not reallocated here).
    pub fn update_vertices(&self, queue: &wgpu::Queue, vertices: &[Vertex]) {
        let n = (vertices.len() as u32).min(self.vertex_capacity) as usize;
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices[..n]));
    }
}
