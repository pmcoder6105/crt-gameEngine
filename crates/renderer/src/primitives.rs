//! Procedural primitive meshes (cube, ground plane) as `(Vec<Vertex>,
//! Vec<u32>)` ready for [`GpuMesh::upload`](crate::GpuMesh::upload).

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
    fn plane_lies_flat_with_up_normal() {
        let (vertices, indices) = plane(10.0);
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 6);
        assert!(vertices.iter().all(|v| v.position[1] == 0.0));
        assert!(vertices.iter().all(|v| v.normal == [0.0, 1.0, 0.0]));
    }
}
