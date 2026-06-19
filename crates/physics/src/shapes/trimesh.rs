use elderforge_core::math::Vec3;
use serde::{Deserialize, Serialize};

use crate::broadphase::Aabb;

/// Triangle mesh collider (static geometry only).
/// // TODO: internal BVH over triangles for fast queries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TriMesh {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<[u32; 3]>,
}

impl TriMesh {
    pub fn aabb(&self, position: Vec3) -> Aabb {
        let mut min = Vec3::ZERO;
        let mut max = Vec3::ZERO;
        for vertex in &self.vertices {
            min = min.min(*vertex);
            max = max.max(*vertex);
        }
        Aabb::new(position + min, position + max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_bounds_vertices() {
        let mesh = TriMesh {
            vertices: vec![Vec3::ZERO, Vec3::ONE, Vec3::new(0.0, 5.0, 0.0)],
            indices: vec![[0, 1, 2]],
        };
        let aabb = mesh.aabb(Vec3::ZERO);
        assert_eq!(aabb.max, Vec3::new(1.0, 5.0, 1.0));
    }
}
