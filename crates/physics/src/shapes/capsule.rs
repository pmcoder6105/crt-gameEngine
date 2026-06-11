use elderforge_core::math::Vec3;

use crate::broadphase::Aabb;

/// Capsule aligned with the local Y axis: a segment of `2 * half_height`
/// swept by `radius`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub radius: f32,
    pub half_height: f32,
}

impl Capsule {
    pub fn aabb(&self, position: Vec3) -> Aabb {
        let extents = Vec3::new(self.radius, self.half_height + self.radius, self.radius);
        Aabb::new(position - extents, position + extents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_includes_caps() {
        let capsule = Capsule {
            radius: 0.5,
            half_height: 1.0,
        };
        let aabb = capsule.aabb(Vec3::ZERO);
        assert_eq!(aabb.max, Vec3::new(0.5, 1.5, 0.5));
    }
}
