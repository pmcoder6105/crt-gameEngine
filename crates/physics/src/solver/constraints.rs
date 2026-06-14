//! Distance, contact, and joint constraint types consumed by the XPBD solver.

use elderforge_core::math::Vec3;

use crate::body::BodyHandle;

/// XPBD compliance (inverse stiffness). Zero means perfectly rigid.
pub type Compliance = f32;

#[derive(Debug, Clone, Copy)]
pub struct DistanceConstraint {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    pub rest_length: f32,
    pub compliance: Compliance,
}

#[derive(Debug, Clone, Copy)]
pub struct ContactConstraint {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    pub point: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointKind {
    Fixed,
    Ball,
    Hinge,
    Slider,
}

#[derive(Debug, Clone, Copy)]
pub struct JointConstraint {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    pub kind: JointKind,
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(index: u32) -> BodyHandle {
        BodyHandle::new(index, 0)
    }

    #[test]
    fn constraints_construct() {
        let distance = DistanceConstraint {
            body_a: handle(0),
            body_b: handle(1),
            rest_length: 2.0,
            compliance: 0.0,
        };
        assert_eq!(distance.rest_length, 2.0);

        let contact = ContactConstraint {
            body_a: handle(0),
            body_b: handle(1),
            point: Vec3::ZERO,
            normal: Vec3::Y,
            penetration: 0.01,
        };
        assert!(contact.penetration > 0.0);

        let joint = JointConstraint {
            body_a: handle(0),
            body_b: handle(1),
            kind: JointKind::Hinge,
            anchor_a: Vec3::ZERO,
            anchor_b: Vec3::X,
        };
        assert_eq!(joint.kind, JointKind::Hinge);
    }
}
