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

/// A capsule aligned with the Y axis: a cylinder of the given `radius` and
/// height `2 * half_height`, capped by a hemisphere of `radius` at each end,
/// centered at the origin (so it spans `y ∈ [-(half_height + radius),
/// half_height + radius]`). `sectors` divides longitude; `cap_stacks` divides
/// each hemispherical cap's latitude.
///
/// Normals are the unit radial from the nearest cap center, which is purely
/// horizontal at the equator — exactly the cylinder-wall normal — so shading
/// stays continuous across the cap/cylinder seam. Degenerate triangles that
/// collapse onto a pole vertex are skipped, as in [`sphere`].
pub fn capsule(radius: f32, half_height: f32, sectors: u32, cap_stacks: u32) -> (Vec<Vertex>, Vec<u32>) {
    use std::f32::consts::PI;
    let sectors = sectors.max(3);
    let cap_stacks = cap_stacks.max(1);
    let row = sectors + 1;

    // Latitude rings, north pole to south pole. The top hemisphere's cap center
    // is at +half_height and the bottom's at -half_height; the two equator rings
    // (phi = 0) sit at y = ±half_height and bound the cylinder wall between them.
    let mut ring_specs = Vec::with_capacity(2 * (cap_stacks as usize + 1));
    for i in 0..=cap_stacks {
        // Top hemisphere: phi from +PI/2 (pole) down to 0 (equator).
        ring_specs.push((half_height, PI / 2.0 * (1.0 - i as f32 / cap_stacks as f32)));
    }
    for i in 0..=cap_stacks {
        // Bottom hemisphere: phi from 0 (equator) down to -PI/2 (pole).
        ring_specs.push((-half_height, -PI / 2.0 * (i as f32 / cap_stacks as f32)));
    }

    let total_height = 2.0 * (half_height + radius);
    let mut vertices = Vec::with_capacity(ring_specs.len() * row as usize);
    for &(center_y, phi) in &ring_specs {
        let (sphi, cphi) = phi.sin_cos();
        let ring_r = radius * cphi;
        let y = center_y + radius * sphi;
        let v = if total_height > 0.0 {
            (half_height + radius - y) / total_height
        } else {
            0.0
        };
        for j in 0..=sectors {
            let theta = 2.0 * PI * (j as f32 / sectors as f32);
            let (sth, cth) = theta.sin_cos();
            let position = [ring_r * cth, y, ring_r * sth];
            // Unit radial (cos²φ + sin²φ = 1, so no normalization needed).
            let normal = [cphi * cth, sphi, cphi * sth];
            let uv = [j as f32 / sectors as f32, v];
            vertices.push(vertex(position, normal, uv));
        }
    }

    let rings = ring_specs.len() as u32;
    let mut indices = Vec::new();
    for i in 0..rings - 1 {
        for j in 0..sectors {
            let a = i * row + j;
            let b = a + row;
            // Skip the triangle that collapses onto a pole vertex.
            if i != 0 {
                indices.extend_from_slice(&[a, b, a + 1]);
            }
            if i != rings - 2 {
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
    fn capsule_is_well_formed() {
        let (sectors, cap_stacks) = (16u32, 6u32);
        let (radius, half_height) = (0.5f32, 1.0f32);
        let (vertices, indices) = capsule(radius, half_height, sectors, cap_stacks);
        // Two hemispheres of (cap_stacks + 1) rings, each row sectors + 1 wide.
        let rings = 2 * (cap_stacks + 1);
        assert_eq!(vertices.len(), (rings * (sectors + 1)) as usize);
        assert_eq!(indices.len() % 3, 0);
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
        // Every vertex lies inside the capsule's bounding box with a unit normal.
        for v in &vertices {
            assert!(v.position[0].abs() <= radius + 1e-4);
            assert!(v.position[2].abs() <= radius + 1e-4);
            assert!(v.position[1].abs() <= half_height + radius + 1e-4);
            let nlen = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((nlen - 1.0).abs() < 1e-4, "non-unit normal: {nlen}");
        }
        // The widest ring sits at exactly the cylinder radius.
        let max_xz = vertices
            .iter()
            .map(|v| (v.position[0].powi(2) + v.position[2].powi(2)).sqrt())
            .fold(0.0_f32, f32::max);
        assert!((max_xz - radius).abs() < 1e-4, "widest ring {max_xz} != radius {radius}");
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
