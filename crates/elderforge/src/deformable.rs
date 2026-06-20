//! GPU meshes for the deforming geometry in a [`PhysicsWorld`] — soft-body
//! surfaces and cloth grids — rebuilt from particle positions each frame.
//!
//! Unlike the static [`MeshRenderer`](elderforge_ecs::components::MeshRenderer)
//! entities (which carry a fixed [`MeshHandle`](elderforge_core::handles::MeshHandle)
//! into the renderer's cache), soft bodies and cloth have no entity and their
//! vertices move every step. [`DeformableMeshes`] owns one dynamic
//! [`GpuMesh`] per soft body and per cloth: the index buffer is uploaded once
//! (topology is fixed), and [`update`](DeformableMeshes::update) streams fresh
//! positions and recomputed normals into the vertex buffers. The meshes are
//! drawn in world space (identity model matrix — the particles already are).

use elderforge_core::math::{Mat4, Vec3};
use elderforge_physics::{Cloth, PhysicsWorld, SoftBody};
use elderforge_renderer::{Draw, GpuMesh, Vertex};

/// Dynamic GPU meshes for every soft body and cloth in a world, kept parallel
/// to [`PhysicsWorld::soft_bodies`] / [`PhysicsWorld::cloths`].
pub struct DeformableMeshes {
    soft: Vec<GpuMesh>,
    cloth: Vec<GpuMesh>,
}

impl DeformableMeshes {
    /// Allocate a dynamic mesh for each soft body and cloth, seeded with the
    /// world's current particle positions.
    pub fn build(device: &wgpu::Device, world: &PhysicsWorld) -> Self {
        let soft = world
            .soft_bodies()
            .iter()
            .enumerate()
            .map(|(i, sb)| {
                let (verts, indices) = soft_geometry(world, sb);
                GpuMesh::upload_dynamic(device, &format!("soft.{i}"), &verts, &indices)
            })
            .collect();
        let cloth = world
            .cloths()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let (verts, indices) = cloth_geometry(world, c);
                GpuMesh::upload_dynamic(device, &format!("cloth.{i}"), &verts, &indices)
            })
            .collect();
        Self { soft, cloth }
    }

    /// Restream every mesh's vertices from the world's current particle state.
    /// Cheap when there are no deformables (the loops are empty).
    pub fn update(&self, queue: &wgpu::Queue, world: &PhysicsWorld) {
        for (mesh, sb) in self.soft.iter().zip(world.soft_bodies()) {
            let (verts, _) = soft_geometry(world, sb);
            mesh.update_vertices(queue, &verts);
        }
        for (mesh, c) in self.cloth.iter().zip(world.cloths()) {
            let (verts, _) = cloth_geometry(world, c);
            mesh.update_vertices(queue, &verts);
        }
    }

    /// Append a world-space draw for each deformable mesh to `draws`.
    pub fn append_draws<'a>(&'a self, draws: &mut Vec<Draw<'a>>) {
        for mesh in self.soft.iter().chain(self.cloth.iter()) {
            draws.push(Draw { model: Mat4::IDENTITY, mesh });
        }
    }

    /// Whether there is any deformable geometry at all.
    pub fn is_empty(&self) -> bool {
        self.soft.is_empty() && self.cloth.is_empty()
    }
}

/// Vertices + indices for a soft body's surface: one vertex per particle
/// (interior particles are present but unreferenced), normals smoothed from the
/// boundary triangles. Indices are the body's outward-wound surface faces.
fn soft_geometry(world: &PhysicsWorld, sb: &SoftBody) -> (Vec<Vertex>, Vec<u32>) {
    let particles = &world.particles()[sb.base()..sb.base() + sb.particle_count()];
    let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
    let mut indices = Vec::with_capacity(sb.surface().len() * 3);
    for tri in sb.surface() {
        indices.extend_from_slice(tri);
    }
    let normals = smooth_normals(&positions, &indices);
    let verts = positions
        .iter()
        .zip(&normals)
        .map(|(&pos, &n)| vertex(pos, n, [0.0, 0.0]))
        .collect();
    (verts, indices)
}

/// Vertices + indices for a cloth grid: one vertex per particle, grid UVs, and
/// normals smoothed from the two-triangles-per-quad topology.
fn cloth_geometry(world: &PhysicsWorld, cloth: &Cloth) -> (Vec<Vertex>, Vec<u32>) {
    let particles = &world.particles()[cloth.base()..cloth.base() + cloth.particle_count()];
    let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
    let indices = cloth.indices();
    let normals = smooth_normals(&positions, &indices);
    let (cols, rows) = cloth.dims();
    let inv_c = 1.0 / (cols.max(2) - 1) as f32;
    let inv_r = 1.0 / (rows.max(2) - 1) as f32;
    let verts = positions
        .iter()
        .zip(&normals)
        .enumerate()
        .map(|(i, (&pos, &n))| {
            let (c, r) = (i % cols, i / cols);
            vertex(pos, n, [c as f32 * inv_c, r as f32 * inv_r])
        })
        .collect();
    (verts, indices)
}

/// Area-weighted smoothed vertex normals from a triangle list. Vertices touched
/// by no triangle (soft-body interior nodes) fall back to +Y; they are never
/// drawn, so the value only needs to be finite.
fn smooth_normals(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        // Cross product magnitude is twice the triangle area, so summing these
        // raw cross products area-weights the contribution automatically.
        let face = (positions[i1] - positions[i0]).cross(positions[i2] - positions[i0]);
        normals[i0] += face;
        normals[i1] += face;
        normals[i2] += face;
    }
    for n in &mut normals {
        *n = n.normalize_or_zero();
        if *n == Vec3::ZERO {
            *n = Vec3::Y;
        }
    }
    normals
}

fn vertex(position: Vec3, normal: Vec3, uv: [f32; 2]) -> Vertex {
    Vertex {
        position: position.to_array(),
        normal: normal.to_array(),
        uv,
        tangent: [0.0; 4],
    }
}
