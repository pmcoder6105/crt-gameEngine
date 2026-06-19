//! Elderforge physics: XPBD solver, BVH broadphase, GJK/EPA/SAT narrowphase,
//! rigid and soft bodies, SPH fluids, and per-frame debug draw data.

pub mod body;
pub mod broadphase;
pub mod debug;
pub mod fluid;
pub mod material;
pub mod narrowphase;
pub mod query;
pub mod shapes;
pub mod solver;
pub mod world;

pub use body::{BodyHandle, BodyKind, Collider, RigidBody, SoftBody};
pub use material::{CombinedMaterial, PhysicsMaterial};
pub use shapes::ColliderShape;
pub use solver::{BallJoint, FixedJoint, HingeJoint, Joint, PrismaticJoint};
pub use world::PhysicsWorld;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PhysicsError {
    #[error("invalid or stale body handle")]
    InvalidHandle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physics_error_displays() {
        assert_eq!(
            PhysicsError::InvalidHandle.to_string(),
            "invalid or stale body handle"
        );
    }
}
