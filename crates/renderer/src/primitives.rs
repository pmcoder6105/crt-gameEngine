//! Procedural primitive meshes (cube, UV sphere, ground plane) as
//! `(Vec<Vertex>, Vec<u32>)` ready for [`GpuMesh::upload`](crate::GpuMesh::upload).

use crate::mesh::Vertex;

fn vertex(position: [f32; 3], normal: [f32; 3], uv: [f32; 2]) -> Vertex {
    Vertex {
        position,
        normal,
        uv,
        // Tangents are unused by the unlit/forward path; leave them zeroed.
        tangent: [0.0; 4],
    }
}

/// An axis-aligned cube centered at the origin with the given half-extent
/// (so edge length is `2 * half_extent`). Each of the six faces has its own
/// four vertices and an outward normal, giving flat per-face shading.
pub fn cube(half_extent: f32) -> (Vec<Vertex>, Vec<u32>) {
    let h = half_extent;
    // (face normal, four corner positions wound counter-clockwise when viewed
    // from outside the cube).
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        // +X
        ([1.0, 0.0, 0.0], [[h, -h, h], [h, -h, -h], [h, h, -h], [h, h, h]]),
        // -X
        ([-1.0, 0.0, 0.0], [[-h, -h, -h], [-h, -h, h], [-h, h, h], [-h, h, -h]]),
        // +Y
        ([0.0, 1.0, 0.0], [[-h, h, h], [h, h, h], [h, h, -h], [-h, h, -h]]),
        // -Y
        ([0.0, -1.0, 0.0], [[-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h]]),
        // +Z
        ([0.0, 0.0, 1.0], [[-h, -h, h], [h, -h, h], [h, h, h], [-h, h, h]]),
        // -Z
        ([0.0, 0.0, -1.0], [[h, -h, -h], [-h, -h, -h], [-h, h, -h], [h, h, -h]]),
    ];

    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (normal, corners) in faces {
        let base = vertices.len() as u32;
        for (corner, uv) in corners.iter().zip(uvs.iter()) {
            vertices.push(vertex(*corner, normal, *uv));
        }
        // Two triangles per quad.
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

/// A UV sphere of the given `radius` centered at the origin, with `sectors`
/// divisions of longitude and `stacks` divisions of latitude. Normals point
/// radially outward (smooth shading) and UVs wrap longitude in `u`, latitude
/// in `v`. Degenerate triangles that collapse at the poles are skipped.
pub fn sphere(radius: f32, sectors: u32, stacks: u32) -> (Vec<Vertex>, Vec<u32>) {
    use std::f32::consts::PI;
    let sectors = sectors.max(3);
    let stacks = stacks.max(2);
    let inv_r = if radius != 0.0 { 1.0 / radius } else { 0.0 };

    let row = sectors + 1;
    let mut vertices = Vec::with_capacity((row * (stacks + 1)) as usize);
    for i in 0..=stacks {
        // From the north pole (+PI/2) down to the south pole (-PI/2).
        let stack_angle = PI / 2.0 - PI * (i as f32 / stacks as f32);
        let xz = radius * stack_angle.cos();
        let y = radius * stack_angle.sin();
        for j in 0..=sectors {
            let sector_angle = 2.0 * PI * (j as f32 / sectors as f32);
            let x = xz * sector_angle.cos();
            let z = xz * sector_angle.sin();
            let normal = [x * inv_r, y * inv_r, z * inv_r];
            let uv = [j as f32 / sectors as f32, i as f32 / stacks as f32];
            vertices.push(vertex([x, y, z], normal, uv));
        }
    }

    // Two triangles per quad; the top and bottom rings have only one each
    // because the far edge degenerates to the pole vertex.
    let mut indices = Vec::new();
    for i in 0..stacks {
        for j in 0..sectors {
            let a = i * row + j;
            let b = a + row;
            if i != 0 {
                indices.extend_from_slice(&[a, b, a + 1]);
            }
            if i != stacks - 1 {
                indices.extend_from_slice(&[a + 1, b, b + 1]);
            }
        }
    }
    (vertices, indices)
}

/// A flat ground quad in the XZ plane at `y = 0`, spanning `[-half_size,
/// half_size]` on both axes, with an upward (+Y) normal.
pub fn plane(half_size: f32) -> (Vec<Vertex>, Vec<u32>) {
    let s = half_size;
    let n = [0.0, 1.0, 0.0];
    let vertices = vec![
        vertex([-s, 0.0, s], n, [0.0, 1.0]),
        vertex([s, 0.0, s], n, [1.0, 1.0]),
        vertex([s, 0.0, -s], n, [1.0, 0.0]),
        vertex([-s, 0.0, -s], n, [0.0, 0.0]),
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];
    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_has_24_vertices_and_36_indices() {
        let (vertices, indices) = cube(0.5);
        assert_eq!(vertices.len(), 24);
        assert_eq!(indices.len(), 36);
        // Every index is in range.
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
    }

    #[test]
    fn cube_corners_are_within_half_extent() {
        let (vertices, _) = cube(2.0);
        assert!(vertices
            .iter()
            .all(|v| v.position.iter().all(|c| c.abs() <= 2.0 + 1e-6)));
    }

    #[test]
    fn sphere_is_well_formed() {
        let (sectors, stacks) = (16, 12);
        let (vertices, indices) = sphere(2.0, sectors, stacks);
        // One vertex per (sector seam included) × (stack ring).
        assert_eq!(vertices.len(), ((sectors + 1) * (stacks + 1)) as usize);
        // Whole triangles, every index in range.
        assert_eq!(indices.len() % 3, 0);
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
        // Every vertex sits on the sphere with a unit outward normal.
        for v in &vertices {
            let r = (v.position[0].powi(2) + v.position[1].powi(2) + v.position[2].powi(2)).sqrt();
            assert!((r - 2.0).abs() < 1e-4, "vertex off the sphere: r = {r}");
            let nlen =
                (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((nlen - 1.0).abs() < 1e-4, "non-unit normal: {nlen}");
        }
    }

    #[test]
    fn plane_lies_flat_with_up_normal() {
        let (vertices, indices) = plane(10.0);
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 6);
        assert!(vertices.iter().all(|v| v.position[1] == 0.0));
        assert!(vertices.iter().all(|v| v.normal == [0.0, 1.0, 0.0]));
    }
}
