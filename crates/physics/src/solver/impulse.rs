//! Impulse-based contact resolution for the minimal rigid-body pipeline.
//!
//! Frictionless, linear-only normal impulses plus a positional correction.
//! For spheres and half-spaces the contact lever arm is parallel to the
//! contact normal, so the angular impulse term vanishes exactly — linear-only
//! resolution is not an approximation here, and an elastic (restitution = 1)
//! collision conserves kinetic energy to floating-point precision. The full
//! angular contact solver arrives with XPBD contacts in a later phase.

use elderforge_core::math::Vec3;

use crate::body::RigidBody;

/// Combine two bodies' restitution into a single coefficient. Uses the minimum,
/// so a contact is only as bouncy as its least-bouncy participant (a ball on a
/// dead floor doesn't bounce; two perfectly elastic balls stay elastic).
pub fn combine_restitution(a: &RigidBody, b: &RigidBody) -> f32 {
    a.material.restitution.min(b.material.restitution)
}

/// Resolve a single contact between bodies `a` and `b`.
///
/// `normal` is the unit contact normal pointing from `a` toward `b`, and
/// `penetration` is the positive overlap depth along it. The normal impulse
/// removes the approaching relative velocity (scaled by `1 + restitution`),
/// and a position split proportional to inverse mass pushes the pair apart so
/// resting contacts don't sink. Two immovable bodies (zero inverse mass each)
/// are left untouched.
pub fn resolve_contact(
    a: &mut RigidBody,
    b: &mut RigidBody,
    normal: Vec3,
    penetration: f32,
    restitution: f32,
) {
    let inv_mass_sum = a.inv_mass + b.inv_mass;
    if inv_mass_sum <= 0.0 {
        return;
    }

    // Normal impulse: only resolve if the bodies are approaching.
    let relative_velocity = b.linear_velocity - a.linear_velocity;
    let normal_speed = relative_velocity.dot(normal);
    if normal_speed < 0.0 {
        let impulse_magnitude = -(1.0 + restitution) * normal_speed / inv_mass_sum;
        let impulse = normal * impulse_magnitude;
        a.linear_velocity -= impulse * a.inv_mass;
        b.linear_velocity += impulse * b.inv_mass;
    }

    // Positional correction: shove the bodies out of overlap along the normal,
    // split by inverse mass so the lighter body moves more.
    if penetration > 0.0 {
        let correction = normal * (penetration / inv_mass_sum);
        a.position -= correction * a.inv_mass;
        b.position += correction * b.inv_mass;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Collider;

    fn ball(position: Vec3, velocity: Vec3) -> RigidBody {
        RigidBody::dynamic(position, 1.0, Collider::Sphere { radius: 1.0 })
            .with_linear_velocity(velocity)
    }

    #[test]
    fn head_on_equal_masses_swap_velocity() {
        // A moving +x into a stationary B; elastic, equal mass -> velocities swap.
        let mut a = ball(Vec3::new(-0.9, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0));
        let mut b = ball(Vec3::new(0.9, 0.0, 0.0), Vec3::ZERO);
        resolve_contact(&mut a, &mut b, Vec3::X, 0.2, 1.0);
        assert!(a.linear_velocity.x.abs() < 1e-5, "A should stop");
        assert!((b.linear_velocity.x - 2.0).abs() < 1e-5, "B should carry A's speed");
    }

    #[test]
    fn restitution_zero_kills_normal_velocity() {
        let mut a = ball(Vec3::new(-0.9, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0));
        let mut b = ball(Vec3::new(0.9, 0.0, 0.0), Vec3::ZERO);
        resolve_contact(&mut a, &mut b, Vec3::X, 0.2, 0.0);
        // Inelastic, equal mass: both end at the average velocity (+1).
        assert!((a.linear_velocity.x - 1.0).abs() < 1e-5);
        assert!((b.linear_velocity.x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn separating_bodies_are_not_impulsed() {
        // Already moving apart: no impulse, but penetration is still corrected.
        let mut a = ball(Vec3::new(-0.9, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0));
        let mut b = ball(Vec3::new(0.9, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        resolve_contact(&mut a, &mut b, Vec3::X, 0.2, 1.0);
        assert_eq!(a.linear_velocity.x, -1.0);
        assert_eq!(b.linear_velocity.x, 1.0);
        assert!(a.position.x < -0.9, "A pushed further -x");
        assert!(b.position.x > 0.9, "B pushed further +x");
    }

    #[test]
    fn static_body_absorbs_correction() {
        // Dynamic sphere A (radius 1) sinking into immovable plane B below it.
        // A is above B, so the A->B normal points down (-Y); only A moves.
        let mut a = ball(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, -3.0, 0.0));
        let mut b = RigidBody::fixed(
            Vec3::ZERO,
            Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
        );
        let before_b = b.position;
        resolve_contact(&mut a, &mut b, -Vec3::Y, 0.5, 0.0);
        assert!(a.linear_velocity.y.abs() < 1e-5, "A's downward speed removed");
        assert!((a.position.y - 1.0).abs() < 1e-5, "A lifted to rest on the surface");
        assert_eq!(b.position, before_b, "static body never moves");
    }

    #[test]
    fn combine_restitution_takes_the_minimum() {
        let bouncy = ball(Vec3::ZERO, Vec3::ZERO)
            .with_material(crate::PhysicsMaterial { restitution: 1.0, ..Default::default() });
        let dead = ball(Vec3::ZERO, Vec3::ZERO)
            .with_material(crate::PhysicsMaterial { restitution: 0.0, ..Default::default() });
        assert_eq!(combine_restitution(&bouncy, &dead), 0.0);
    }
}
