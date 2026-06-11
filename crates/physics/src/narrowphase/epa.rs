//! EPA (Expanding Polytope Algorithm) — penetration depth and contact normal
//! for shape pairs that GJK reports as intersecting.

use elderforge_core::math::Vec3;

use super::Contact;

/// Expand the GJK termination simplex into a contact.
pub fn penetration_contact(_simplex: &[Vec3]) -> Option<Contact> {
    // TODO: expand the polytope until the closest face to the origin converges.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_none() {
        assert!(penetration_contact(&[Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z]).is_none());
    }
}
