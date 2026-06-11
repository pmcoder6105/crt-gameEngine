use elderforge_core::math::Vec3;

use crate::broadphase::Aabb;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShape {
    pub half_extents: Vec3,
}

impl BoxShape {
    pub fn aabb(&self, position: Vec3) -> Aabb {
        Aabb::new(position - self.half_extents, position + self.half_extents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_uses_half_extents() {
        let shape = BoxShape {
            half_extents: Vec3::new(1.0, 2.0, 3.0),
        };
        let aabb = shape.aabb(Vec3::ZERO);
        assert_eq!(aabb.min, Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(aabb.max, Vec3::new(1.0, 2.0, 3.0));
    }
}
