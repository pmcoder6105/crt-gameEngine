//! XPBD (Extended Position-Based Dynamics) constraints.
//!
//! Each constraint is *projected* once or more per substep: it reads the
//! current (predicted) body positions, measures its violation `C`, and shifts
//! the bodies along the constraint gradient to drive `C` toward zero. The
//! correction is weighted by inverse mass and softened by a `compliance`
//! (inverse stiffness, in m/N): zero compliance is perfectly rigid, larger
//! values give springier behavior. The substep loop that drives these lives in
//! [`PhysicsWorld::step`](crate::PhysicsWorld::step).

use elderforge_core::math::Vec3;

use crate::body::RigidBody;

const EPS: f32 = 1e-9;

/// A position-level constraint solved by the XPBD substep loop.
pub trait Constraint {
    /// Apply one projection iteration over `bodies` for substep length `dt`.
    fn project(&mut self, bodies: &mut [RigidBody], dt: f32);

    /// Clear the accumulated Lagrange multiplier; called once per substep
    /// before the first projection iteration.
    fn reset(&mut self) {}
}

/// Keeps two bodies a fixed `rest_length` apart. With `compliance == 0` it is a
/// rigid rod (a pendulum arm, a taut rope link); positive compliance makes it a
/// spring.
#[derive(Debug, Clone, Copy)]
pub struct DistanceConstraint {
    pub body_a: usize,
    pub body_b: usize,
    pub rest_length: f32,
    pub compliance: f32,
    lambda: f32,
}

impl DistanceConstraint {
    pub fn new(body_a: usize, body_b: usize, rest_length: f32, compliance: f32) -> Self {
        Self { body_a, body_b, rest_length, compliance, lambda: 0.0 }
    }
}

impl Constraint for DistanceConstraint {
    fn reset(&mut self) {
        self.lambda = 0.0;
    }

    fn project(&mut self, bodies: &mut [RigidBody], dt: f32) {
        if self.body_a == self.body_b {
            return;
        }
        let xa = bodies[self.body_a].position;
        let xb = bodies[self.body_b].position;
        let wa = bodies[self.body_a].inv_mass;
        let wb = bodies[self.body_b].inv_mass;
        let w = wa + wb;
        if w <= 0.0 {
            return;
        }
        let delta = xa - xb;
        let len = delta.length();
        if len < EPS {
            return;
        }
        let dir = delta / len;
        let c = len - self.rest_length;
        let alpha = self.compliance / (dt * dt);
        let delta_lambda = (-c - alpha * self.lambda) / (w + alpha);
        self.lambda += delta_lambda;
        let p = dir * delta_lambda;
        bodies[self.body_a].position += p * wa;
        bodies[self.body_b].position -= p * wb;
    }
}

/// A non-penetration contact built from a [`ContactManifold`] each substep. It
/// resolves penetration along a fixed `normal` (pointing from A to B) and, in a
/// separate velocity pass, applies restitution.
///
/// [`ContactManifold`]: crate::narrowphase::ContactManifold
#[derive(Debug, Clone, Copy)]
pub struct ContactConstraint {
    pub body_a: usize,
    pub body_b: usize,
    /// Contact normal, pointing from A toward B.
    pub normal: Vec3,
    pub compliance: f32,
    pub restitution: f32,
    /// Penetration depth when the contact was generated.
    depth0: f32,
    /// Body positions when generated, so penetration can be re-measured as the
    /// solver moves them (keeps multiple iterations from over-correcting).
    anchor_a: Vec3,
    anchor_b: Vec3,
    /// Normal-direction relative velocity at generation, for restitution.
    pre_normal_velocity: f32,
    lambda: f32,
}

impl ContactConstraint {
    /// Build a contact from a generated manifold. `bodies` is read for the
    /// anchor positions and pre-solve relative velocity.
    pub fn new(
        body_a: usize,
        body_b: usize,
        normal: Vec3,
        depth: f32,
        restitution: f32,
        compliance: f32,
        bodies: &[RigidBody],
    ) -> Self {
        let rel = bodies[body_a].linear_velocity - bodies[body_b].linear_velocity;
        Self {
            body_a,
            body_b,
            normal,
            compliance,
            restitution,
            depth0: depth,
            anchor_a: bodies[body_a].position,
            anchor_b: bodies[body_b].position,
            pre_normal_velocity: rel.dot(normal),
            lambda: 0.0,
        }
    }

    /// Current penetration along the normal, re-measured from how far the
    /// bodies have moved since the contact was generated.
    fn current_depth(&self, bodies: &[RigidBody]) -> f32 {
        let moved_a = bodies[self.body_a].position - self.anchor_a;
        let moved_b = bodies[self.body_b].position - self.anchor_b;
        self.depth0 - (moved_b - moved_a).dot(self.normal)
    }

    /// Velocity-level restitution pass, run after positions and velocities are
    /// finalized for the substep. Reverses the approach speed scaled by the
    /// restitution coefficient.
    pub fn apply_restitution(&self, bodies: &mut [RigidBody]) {
        let wa = bodies[self.body_a].inv_mass;
        let wb = bodies[self.body_b].inv_mass;
        let w = wa + wb;
        if w <= 0.0 || self.pre_normal_velocity <= 0.0 {
            return;
        }
        let rel = bodies[self.body_a].linear_velocity - bodies[self.body_b].linear_velocity;
        let vn = rel.dot(self.normal);
        let target = -self.restitution * self.pre_normal_velocity;
        let delta = (target - vn) / w;
        let p = self.normal * delta;
        bodies[self.body_a].linear_velocity += p * wa;
        bodies[self.body_b].linear_velocity -= p * wb;
    }
}

impl Constraint for ContactConstraint {
    fn reset(&mut self) {
        self.lambda = 0.0;
    }

    fn project(&mut self, bodies: &mut [RigidBody], dt: f32) {
        let wa = bodies[self.body_a].inv_mass;
        let wb = bodies[self.body_b].inv_mass;
        let w = wa + wb;
        if w <= 0.0 {
            return;
        }
        let depth = self.current_depth(bodies);
        if depth <= 0.0 {
            // Non-penetration is one-sided: only push apart, never pull together.
            return;
        }
        let alpha = self.compliance / (dt * dt);
        let delta_lambda = (depth - alpha * self.lambda) / (w + alpha);
        self.lambda += delta_lambda;
        let p = self.normal * delta_lambda;
        bodies[self.body_a].position -= p * wa; // A moves -normal
        bodies[self.body_b].position += p * wb; // B moves +normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{Collider, RigidBody};

    fn point_body(pos: Vec3, mass: f32) -> RigidBody {
        RigidBody::dynamic(pos, mass, Collider::Sphere { radius: 0.1 })
    }

    #[test]
    fn distance_constraint_pulls_to_rest_length() {
        // Two equal masses 3 apart, rest length 1: each moves to be 1 apart.
        let mut bodies = vec![
            point_body(Vec3::ZERO, 1.0),
            point_body(Vec3::new(3.0, 0.0, 0.0), 1.0),
        ];
        let mut c = DistanceConstraint::new(0, 1, 1.0, 0.0);
        for _ in 0..20 {
            c.reset();
            c.project(&mut bodies, 1.0 / 60.0);
        }
        let len = (bodies[0].position - bodies[1].position).length();
        assert!((len - 1.0).abs() < 1e-4, "length {len} should converge to 1");
    }

    #[test]
    fn static_anchor_only_moves_dynamic_body() {
        let mut bodies = vec![
            RigidBody::fixed(Vec3::ZERO, Collider::Sphere { radius: 0.1 }),
            point_body(Vec3::new(3.0, 0.0, 0.0), 1.0),
        ];
        let mut c = DistanceConstraint::new(0, 1, 1.0, 0.0);
        c.project(&mut bodies, 1.0 / 60.0);
        assert_eq!(bodies[0].position, Vec3::ZERO, "anchor stays put");
        // The dynamic body absorbs the whole correction toward the anchor.
        assert!(bodies[1].position.x < 3.0);
    }

    #[test]
    fn contact_resolves_penetration() {
        let mut bodies = vec![
            point_body(Vec3::ZERO, 1.0),
            point_body(Vec3::new(0.0, -0.5, 0.0), 1.0),
        ];
        // A above B, overlapping by 0.5; normal A->B is -Y.
        let mut c =
            ContactConstraint::new(0, 1, -Vec3::Y, 0.5, 0.0, 0.0, &bodies);
        c.project(&mut bodies, 1.0 / 60.0);
        // Equal mass: they separate by the full depth, half each.
        assert!((bodies[0].position.y - 0.25).abs() < 1e-5);
        assert!((bodies[1].position.y - (-0.75)).abs() < 1e-5);
        // Re-projecting does nothing now that they no longer overlap.
        c.project(&mut bodies, 1.0 / 60.0);
        assert!((bodies[0].position.y - 0.25).abs() < 1e-5);
    }
}
