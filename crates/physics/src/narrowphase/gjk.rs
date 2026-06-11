//! GJK distance / intersection test for convex shapes.

use elderforge_core::math::Vec3;

use crate::shapes::ColliderShape;

#[derive(Debug, Clone, Copy)]
pub struct GjkResult {
    pub intersecting: bool,
    /// Separation distance; meaningless when `intersecting` is true.
    pub distance: f32,
    pub closest_a: Vec3,
    pub closest_b: Vec3,
}

/// Run GJK between two convex shapes at the given world positions.
pub fn gjk(
    _shape_a: &ColliderShape,
    _pos_a: Vec3,
    _shape_b: &ColliderShape,
    _pos_b: Vec3,
) -> GjkResult {
    // TODO: simplex loop over Minkowski-difference support points.
    GjkResult {
        intersecting: false,
        distance: f32::INFINITY,
        closest_a: Vec3::ZERO,
        closest_b: Vec3::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use crate::shapes::Sphere;

    use super::*;

    #[test]
    fn stub_reports_no_intersection() {
        let a = ColliderShape::Sphere(Sphere { radius: 1.0 });
        let b = ColliderShape::Sphere(Sphere { radius: 1.0 });
        let result = gjk(&a, Vec3::ZERO, &b, Vec3::splat(10.0));
        assert!(!result.intersecting);
        assert!(result.distance.is_infinite());
    }
}
