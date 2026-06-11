//! Narrowphase: GJK + EPA for convex shapes; SAT for polyhedra.

pub mod epa;
pub mod gjk;
pub mod sat;

use elderforge_core::math::Vec3;

/// A single contact point produced by the narrowphase.
#[derive(Debug, Clone, Copy)]
pub struct Contact {
    pub point: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_constructs() {
        let contact = Contact {
            point: Vec3::ZERO,
            normal: Vec3::Y,
            penetration: 0.05,
        };
        assert_eq!(contact.normal, Vec3::Y);
    }
}
