//! Rigid and soft bodies plus the handle type used to reference them.

use elderforge_core::math::{Quat, Vec3};

use crate::material::PhysicsMaterial;

/// Generational handle into the `PhysicsWorld` body storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyHandle {
    pub index: u32,
    pub generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Dynamic,
    Kinematic,
    Static,
}

#[derive(Debug, Clone)]
pub struct RigidBody {
    pub kind: BodyKind,
    pub position: Vec3,
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    /// Inverse mass in 1/kg. Zero means infinite mass (unmovable).
    pub inv_mass: f32,
    pub material: PhysicsMaterial,
    pub sleeping: bool,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self {
            kind: BodyKind::Dynamic,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            inv_mass: 1.0,
            material: PhysicsMaterial::default(),
            sleeping: false,
        }
    }
}

/// Particle-based soft body, solved with XPBD distance constraints.
#[derive(Debug, Clone, Default)]
pub struct SoftBody {
    pub particles: Vec<Vec3>,
    pub inv_masses: Vec<f32>,
    // TODO: distance-constraint network + rest shape for shape matching.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_handle_equality_includes_generation() {
        let a = BodyHandle { index: 3, generation: 0 };
        let b = BodyHandle { index: 3, generation: 1 };
        assert_ne!(a, b);
        assert_eq!(a, BodyHandle { index: 3, generation: 0 });
    }

    #[test]
    fn rigid_body_default_is_awake_dynamic() {
        let body = RigidBody::default();
        assert_eq!(body.kind, BodyKind::Dynamic);
        assert!(!body.sleeping);
        assert!(body.inv_mass > 0.0);
    }

    #[test]
    fn soft_body_default_is_empty() {
        let body = SoftBody::default();
        assert!(body.particles.is_empty());
        assert!(body.inv_masses.is_empty());
    }
}
