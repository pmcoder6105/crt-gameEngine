//! SAT (Separating Axis Theorem) for polyhedra.

use elderforge_core::math::Vec3;

use crate::shapes::BoxShape;

use super::Contact;

/// Box-vs-box SAT test.
pub fn box_box_contact(
    _a: &BoxShape,
    _pos_a: Vec3,
    _b: &BoxShape,
    _pos_b: Vec3,
) -> Option<Contact> {
    // TODO: test 6 face normals + 9 edge cross products, keep the axis of
    // least penetration.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_none() {
        let a = BoxShape { half_extents: Vec3::ONE };
        let b = BoxShape { half_extents: Vec3::ONE };
        assert!(box_box_contact(&a, Vec3::ZERO, &b, Vec3::splat(0.5)).is_none());
    }
}
