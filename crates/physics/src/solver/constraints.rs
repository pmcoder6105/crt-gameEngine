//! Joint constraint types. The active XPBD constraints (distance, contact)
//! live in [`xpbd`](super::xpbd); this module keeps the joint vocabulary the
//! ECS `Joint` component references.

use elderforge_core::math::Vec3;

use crate::body::BodyHandle;

/// XPBD compliance (inverse stiffness). Zero means perfectly rigid.
pub type Compliance = f32;

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

    #[test]
    fn joint_constructs() {
        let joint = JointConstraint {
            body_a: BodyHandle::new(0, 0),
            body_b: BodyHandle::new(1, 0),
            kind: JointKind::Hinge,
            anchor_a: Vec3::ZERO,
            anchor_b: Vec3::X,
        };
        assert_eq!(joint.kind, JointKind::Hinge);
    }
}
