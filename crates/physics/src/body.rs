//! Rigid and soft bodies plus the handle type used to reference them.

use elderforge_core::math::{Mat3, Quat, Vec3};
use elderforge_core::Handle;
use serde::{Deserialize, Serialize};

use crate::broadphase::Aabb;
use crate::material::PhysicsMaterial;

/// Generational handle into the `PhysicsWorld` body storage.
///
/// This is the core crate's generic [`Handle`] tagged with [`RigidBody`], so
/// body handles can't be confused with mesh/texture/material handles. The
/// world still owns the body storage; the handle only indexes into it.
pub type BodyHandle = Handle<RigidBody>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyKind {
    Dynamic,
    Kinematic,
    Static,
}

/// Minimal collision shape carried by a body for the bring-up rigid-body
/// pipeline (semi-implicit Euler + impulse resolution). The full
/// [`ColliderShape`](crate::ColliderShape) / GJK-EPA path supersedes this
/// once the broadphase BVH lands in Phase 6.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Collider {
    /// Solid sphere of the given radius, centered on the body position.
    Sphere { radius: f32 },
    /// Box with the given half-extents, oriented by the body's rotation.
    Box { half_extents: Vec3 },
    /// Capsule aligned with the body's local Y axis: a segment of length
    /// `2 * half_height` swept by `radius` (a hemispherical cap on each end),
    /// oriented by the body's rotation. Mapped to the GJK `Capsule` core +
    /// radius margin in narrowphase.
    Capsule { radius: f32, half_height: f32 },
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
            Collider::Capsule { radius, half_height } => {
                // Conservative and rotation-agnostic: the capsule fits inside the
                // sphere of its half-length (`half_height + radius`), so pad every
                // axis by that — a safe broadphase over-estimate at any orientation.
                let r = Vec3::splat(half_height + radius);
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
    /// Orientation at the start of the current XPBD substep. Mirrors
    /// `prev_position` for the angular degrees of freedom: the solver derives
    /// angular velocity from `rotation` relative to `prev_rotation`.
    pub prev_rotation: Quat,
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
    /// Whether the body is asleep. Asleep bodies skip integration and are only
    /// re-tested for collisions when an awake body is nearby; contact with a
    /// non-sleeping body wakes them. See [`PhysicsWorld`](crate::PhysicsWorld).
    pub sleeping: bool,
    /// Consecutive frames the body's linear and angular speed have stayed below
    /// the world's sleep thresholds. The solver puts an island to sleep once
    /// every member has been quiet for long enough.
    pub(crate) low_energy_frames: u32,
    /// Tombstone set by [`PhysicsWorld::remove_rigid_body`]. Removed bodies keep
    /// their slot (handles stay generationally stale) but are skipped by every
    /// solver phase and never woken.
    pub(crate) removed: bool,
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
            prev_rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass,
            inv_mass,
            inv_inertia_tensor: inv_inertia_for(&collider, mass),
            material: PhysicsMaterial::default(),
            collider,
            sleeping: false,
            low_energy_frames: 0,
            removed: false,
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
            prev_rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass: f32::INFINITY,
            inv_mass: 0.0,
            inv_inertia_tensor: Mat3::ZERO,
            material: PhysicsMaterial::default(),
            collider,
            sleeping: false,
            low_energy_frames: 0,
            removed: false,
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

    /// Set the angular velocity and return `self`.
    pub fn with_angular_velocity(mut self, velocity: Vec3) -> Self {
        self.angular_velocity = velocity;
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

    /// A body the solver should integrate and move: dynamic, finite-mass, and
    /// not tombstoned. (Asleep bodies are still simulatable; they're just
    /// resting.)
    pub(crate) fn is_dynamic(&self) -> bool {
        !self.removed && self.kind == BodyKind::Dynamic && self.inv_mass > 0.0
    }
}

/// Inverse inertia tensor for a collider of the given mass, in body space.
///
/// `Sphere` uses the solid-sphere tensor I = ⅖·m·r²; `Box` uses the solid-box
/// tensor (per axis I = ⅓·m·(h_j² + h_k²) for the two *other* half-extents).
/// The unbounded half-space gets zero (it is only ever a static body).
fn inv_inertia_for(collider: &Collider, mass: f32) -> Mat3 {
    if mass <= 0.0 {
        return Mat3::ZERO;
    }
    match collider {
        Collider::Sphere { radius } if *radius > 0.0 => {
            let inertia = 0.4 * mass * radius * radius;
            Mat3::from_diagonal(Vec3::splat(1.0 / inertia))
        }
        Collider::Box { half_extents } => {
            let h = *half_extents;
            // Solid cuboid: I_x = m/3·(h_y² + h_z²), and cyclically.
            let i = Vec3::new(
                mass / 3.0 * (h.y * h.y + h.z * h.z),
                mass / 3.0 * (h.x * h.x + h.z * h.z),
                mass / 3.0 * (h.x * h.x + h.y * h.y),
            );
            let inv = Vec3::new(
                if i.x > 0.0 { 1.0 / i.x } else { 0.0 },
                if i.y > 0.0 { 1.0 / i.y } else { 0.0 },
                if i.z > 0.0 { 1.0 / i.z } else { 0.0 },
            );
            Mat3::from_diagonal(inv)
        }
        Collider::Capsule { radius, half_height } if *radius > 0.0 => {
            // Approximate the capsule as a solid cylinder (radius r, length
            // 2·half_height) carrying the whole mass; the rounding caps are
            // folded into the cylinder. Like the box tensor this is inert for
            // the linear-only contact path (which applies no torque) and no
            // current joint uses a capsule, so the cap contribution is never
            // exercised — the approximation only needs to be finite, positive,
            // and symmetric about the local Y axis.
            let r = *radius;
            let l = 2.0 * *half_height;
            let axial = 0.5 * mass * r * r; // about local Y (the capsule axis)
            let perp = mass * (r * r / 4.0 + l * l / 12.0); // about X and Z
            let inv = Vec3::new(
                if perp > 0.0 { 1.0 / perp } else { 0.0 },
                if axial > 0.0 { 1.0 / axial } else { 0.0 },
                if perp > 0.0 { 1.0 / perp } else { 0.0 },
            );
            Mat3::from_diagonal(inv)
        }
        _ => Mat3::ZERO,
    }
}

impl Default for RigidBody {
    fn default() -> Self {
        Self::dynamic(Vec3::ZERO, 1.0, Collider::Sphere { radius: 0.5 })
    }
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
    fn box_collider_gets_solid_cuboid_inertia() {
        // Unit cube (half-extents 0.5), mass 2: I_x = 2/3·(0.25+0.25) = 1/3.
        let body = RigidBody::dynamic(Vec3::ZERO, 2.0, Collider::Box { half_extents: Vec3::splat(0.5) });
        let expected_inv = 1.0 / (2.0 / 3.0 * 0.5);
        assert!((body.inv_inertia_tensor.x_axis.x - expected_inv).abs() < 1e-5);
        assert!((body.inv_inertia_tensor.y_axis.y - expected_inv).abs() < 1e-5);
        assert!((body.inv_inertia_tensor.z_axis.z - expected_inv).abs() < 1e-5);
        // Off-diagonal terms stay zero for an axis-aligned cuboid.
        assert_eq!(body.inv_inertia_tensor.x_axis.y, 0.0);
    }

    #[test]
    fn capsule_collider_aabb_covers_the_caps() {
        // Conservative bounding sphere of radius half_height + radius = 1.5.
        let aabb = Collider::Capsule { radius: 0.5, half_height: 1.0 }.aabb(Vec3::ZERO);
        assert_eq!(aabb.min, Vec3::splat(-1.5));
        assert_eq!(aabb.max, Vec3::splat(1.5));
    }

    #[test]
    fn capsule_collider_inertia_is_axially_symmetric() {
        let body =
            RigidBody::dynamic(Vec3::ZERO, 2.0, Collider::Capsule { radius: 0.5, half_height: 1.0 });
        let i = body.inv_inertia_tensor;
        // Symmetric about the local Y axis: the two perpendicular inverse
        // inertias match, and every diagonal term is finite and positive.
        assert!((i.x_axis.x - i.z_axis.z).abs() < 1e-6);
        assert!(i.x_axis.x > 0.0 && i.y_axis.y > 0.0 && i.z_axis.z > 0.0);
        // Off-diagonal terms stay zero for an axis-aligned capsule.
        assert_eq!(i.x_axis.y, 0.0);
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
}
