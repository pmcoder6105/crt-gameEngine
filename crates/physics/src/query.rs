//! Spatial queries: ray casts, shape casts, point queries.

use elderforge_core::math::Vec3;

use crate::body::BodyHandle;
use crate::world::PhysicsWorld;

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    /// Must be normalized.
    pub direction: Vec3,
    pub max_distance: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub body: BodyHandle,
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}

/// Cast a ray against all bodies; returns the closest hit.
pub fn ray_cast(_world: &PhysicsWorld, _ray: &Ray) -> Option<RayHit> {
    // TODO: traverse the broadphase BVH, then narrowphase the candidates.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_cast_on_empty_world_misses() {
        let world = PhysicsWorld::new();
        let ray = Ray {
            origin: Vec3::ZERO,
            direction: Vec3::NEG_Y,
            max_distance: 100.0,
        };
        assert!(ray_cast(&world, &ray).is_none());
    }
}
