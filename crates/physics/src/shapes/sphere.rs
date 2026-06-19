use elderforge_core::math::Vec3;
use serde::{Deserialize, Serialize};

use crate::broadphase::Aabb;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn aabb(&self, position: Vec3) -> Aabb {
        Aabb::new(
            position - Vec3::splat(self.radius),
            position + Vec3::splat(self.radius),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_is_symmetric() {
        let aabb = Sphere { radius: 1.5 }.aabb(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(aabb.min, Vec3::new(-0.5, -1.5, -1.5));
        assert_eq!(aabb.max, Vec3::new(2.5, 1.5, 1.5));
    }
}
