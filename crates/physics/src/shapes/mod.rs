//! Collision shapes.

mod box_;
mod capsule;
mod convex_hull;
mod sphere;
mod trimesh;

pub use box_::BoxShape;
pub use capsule::Capsule;
pub use convex_hull::ConvexHull;
pub use sphere::Sphere;
pub use trimesh::TriMesh;

use elderforge_core::math::Vec3;
use serde::{Deserialize, Serialize};

use crate::broadphase::Aabb;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColliderShape {
    Sphere(Sphere),
    Box(BoxShape),
    Capsule(Capsule),
    ConvexHull(ConvexHull),
    TriMesh(TriMesh),
}

impl ColliderShape {
    /// World-space AABB for the shape at `position`.
    /// // TODO: account for rotation; this assumes identity orientation.
    pub fn aabb(&self, position: Vec3) -> Aabb {
        match self {
            Self::Sphere(shape) => shape.aabb(position),
            Self::Box(shape) => shape.aabb(position),
            Self::Capsule(shape) => shape.aabb(position),
            Self::ConvexHull(shape) => shape.aabb(position),
            Self::TriMesh(shape) => shape.aabb(position),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_dispatches_aabb() {
        let shape = ColliderShape::Sphere(Sphere { radius: 2.0 });
        let aabb = shape.aabb(Vec3::ZERO);
        assert_eq!(aabb.min, Vec3::splat(-2.0));
        assert_eq!(aabb.max, Vec3::splat(2.0));
    }
}
