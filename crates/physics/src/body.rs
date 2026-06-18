//! Rigid and soft bodies plus the handle type used to reference them.

use elderforge_core::math::{Mat3, Quat, Vec3};
use elderforge_core::Handle;

use crate::broadphase::Aabb;
use crate::material::PhysicsMaterial;

/// Generational handle into the `PhysicsWorld` body storage.
///
/// This is the core crate's generic [`Handle`] tagged with [`RigidBody`], so
/// body handles can't be confused with mesh/texture/material handles. The
/// world still owns the body storage; the handle only indexes into it.
pub type BodyHandle = Handle<RigidBody>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Dynamic,
    Kinematic,
    Static,
}

/// Minimal collision shape carried by a body for the bring-up rigid-body
/// pipeline (semi-implicit Euler + impulse resolution). The full
/// [`ColliderShape`](crate::ColliderShape) / GJK-EPA path supersedes this
/// once the broadphase BVH lands in Phase 6.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Collider {
    /// Solid sphere of the given radius, centered on the body position.
    Sphere { radius: f32 },
    /// Box with the given half-extents, oriented by the body's rotation.
    Box { half_extents: Vec3 },
    /// Static half-space (e.g. a ground plane): the solid region lies on the
    /// `-normal` side of the plane `dot(normal, x) = offset`. `normal` is the
    /// unit outward normal (the direction bodies are pushed out of the solid).
    HalfSpace { normal: Vec3, offset: f32 },
}

impl Collider {
    /// World-space AABB of the collider at `position`. A half-space is
    /// unbounded, so it returns an infinite box that overlaps everything —
    /// the broadphase then leaves narrowphase to reject non-contacts. The box
    /// AABB ignores rotation (uses the extents directly), which is a safe
    /// over-estimate for broadphase.
    pub fn aabb(&self, position: Vec3) -> Aabb {
        match self {
            Collider::Sphere { radius } => {
                Aabb::new(position - Vec3::splat(*radius), position + Vec3::splat(*radius))
            }
            Collider::Box { half_extents } => {
                // Conservative: a rotated box fits within the sphere of its
                // diagonal, so pad by the longest half-extent on every axis.
                let r = Vec3::splat(half_extents.length());
                Aabb::new(position - r, position + r)
            }
            Collider::HalfSpace { .. } => {
                Aabb::new(Vec3::splat(f32::NEG_INFINITY), Vec3::splat(f32::INFINITY))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RigidBody {
    pub kind: BodyKind,
    pub position: Vec3,
    /// Position at the start of the current XPBD substep. The solver writes it
    /// each substep and derives velocity from `position - prev_position`.
    pub prev_position: Vec3,
    /// Orientation. Quaternion; integrated from `angular_velocity`.
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    /// Mass in kg. `f32::INFINITY` for immovable bodies (see `inv_mass`).
    pub mass: f32,
    /// Inverse mass in 1/kg. Zero means infinite mass (unmovable); this is the
    /// value the solver actually uses, so it stays authoritative over `mass`.
    pub inv_mass: f32,
    /// Inverse inertia tensor in body space. Zero for bodies that should not
    /// pick up angular velocity from torques (static or point-mass bodies).
    pub inv_inertia_tensor: Mat3,
    pub material: PhysicsMaterial,
    pub collider: Collider,
    pub sleeping: bool,
}

impl RigidBody {
    /// A dynamic body of finite `mass` with the given `collider`. Inverse
    /// inertia is derived from the collider (a solid sphere for `Sphere`).
    pub fn dynamic(position: Vec3, mass: f32, collider: Collider) -> Self {
        let inv_mass = if mass > 0.0 { 1.0 / mass } else { 0.0 };
        Self {
            kind: BodyKind::Dynamic,
            position,
            prev_position: position,
            rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass,
            inv_mass,
            inv_inertia_tensor: inv_inertia_for(&collider, mass),
            material: PhysicsMaterial::default(),
            collider,
            sleeping: false,
        }
    }

    /// An immovable body (infinite mass, zero inverse mass and inertia). Used
    /// for ground planes and other static geometry.
    pub fn fixed(position: Vec3, collider: Collider) -> Self {
        Self {
            kind: BodyKind::Static,
            position,
            prev_position: position,
            rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass: f32::INFINITY,
            inv_mass: 0.0,
            inv_inertia_tensor: Mat3::ZERO,
            material: PhysicsMaterial::default(),
            collider,
            sleeping: false,
        }
    }

    /// Replace the material (e.g. to set restitution) and return `self`, for
    /// terse construction at call sites.
    pub fn with_material(mut self, material: PhysicsMaterial) -> Self {
        self.material = material;
        self
    }

    /// Set the linear velocity and return `self`.
    pub fn with_linear_velocity(mut self, velocity: Vec3) -> Self {
        self.linear_velocity = velocity;
        self
    }

    /// Translational kinetic energy, ½·m·v². Zero for immovable bodies.
    pub fn kinetic_energy(&self) -> f32 {
        if self.inv_mass == 0.0 {
            0.0
        } else {
            0.5 * self.mass * self.linear_velocity.length_squared()
        }
    }
}

/// Inverse inertia tensor for a collider of the given mass. A `Sphere` uses
/// the solid-sphere tensor I = ⅖·m·r²; anything else gets zero (no rotational
/// response), which is fine for the minimal pipeline.
fn inv_inertia_for(collider: &Collider, mass: f32) -> Mat3 {
    match collider {
        Collider::Sphere { radius } if mass > 0.0 && *radius > 0.0 => {
            let inertia = 0.4 * mass * radius * radius;
            Mat3::from_diagonal(Vec3::splat(1.0 / inertia))
        }
        _ => Mat3::ZERO,
    }
}

impl Default for RigidBody {
    fn default() -> Self {
        Self::dynamic(Vec3::ZERO, 1.0, Collider::Sphere { radius: 0.5 })
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
        let a = BodyHandle::new(3, 0);
        let b = BodyHandle::new(3, 1);
        assert_ne!(a, b);
        assert_eq!(a, BodyHandle::new(3, 0));
    }

    #[test]
    fn rigid_body_default_is_awake_dynamic() {
        let body = RigidBody::default();
        assert_eq!(body.kind, BodyKind::Dynamic);
        assert!(!body.sleeping);
        assert!(body.inv_mass > 0.0);
    }

    #[test]
    fn dynamic_sets_consistent_mass_and_inverse() {
        let body = RigidBody::dynamic(Vec3::ZERO, 4.0, Collider::Sphere { radius: 1.0 });
        assert_eq!(body.mass, 4.0);
        assert!((body.inv_mass - 0.25).abs() < 1e-6);
        // Solid sphere I = 0.4 * 4 * 1 = 1.6 -> inverse 0.625 on the diagonal.
        assert!((body.inv_inertia_tensor.x_axis.x - 0.625).abs() < 1e-6);
    }

    #[test]
    fn fixed_body_is_immovable() {
        let body = RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 });
        assert_eq!(body.inv_mass, 0.0);
        assert_eq!(body.inv_inertia_tensor, Mat3::ZERO);
        assert_eq!(body.kinetic_energy(), 0.0);
    }

    #[test]
    fn sphere_collider_aabb_is_symmetric() {
        let aabb = Collider::Sphere { radius: 1.5 }.aabb(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(aabb.min, Vec3::new(-0.5, -1.5, -1.5));
        assert_eq!(aabb.max, Vec3::new(2.5, 1.5, 1.5));
    }

    #[test]
    fn halfspace_aabb_overlaps_everything() {
        let ground = Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 };
        let far = Aabb::new(Vec3::splat(1e6), Vec3::splat(1e6 + 1.0));
        assert!(ground.aabb(Vec3::ZERO).overlaps(&far));
    }

    #[test]
    fn soft_body_default_is_empty() {
        let body = SoftBody::default();
        assert!(body.particles.is_empty());
        assert!(body.inv_masses.is_empty());
    }
}
