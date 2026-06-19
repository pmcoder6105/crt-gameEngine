//! Articulated joints as XPBD constraints.
//!
//! Unlike the linear-only [`ContactConstraint`](super::xpbd::ContactConstraint),
//! joints couple the *full* rigid-body state — they apply corrections at anchor
//! points offset from each body's center of mass, so they generate torque, and
//! they constrain orientation directly. The two building blocks are:
//!
//! * [`solve_positional`] — drive a world-space error vector measured between
//!   two anchor points to zero, distributing the correction over both bodies'
//!   linear and angular degrees of freedom (the generalized inverse mass
//!   `w = 1/m + (r × n)ᵀ I⁻¹ (r × n)`).
//! * [`solve_angular`] — drive an orientation error (a rotation vector) to zero
//!   using only the angular degrees of freedom.
//!
//! Everything else (ball / hinge / prismatic / fixed) is expressed in terms of
//! those two, following Müller et al., *Detailed Rigid Body Simulation with
//! Extended Position Based Dynamics* (2020).

use elderforge_core::math::{Mat3, Quat, Vec3};

use crate::body::RigidBody;

const EPS: f32 = 1e-9;

/// World-space inverse inertia tensor of a body: `R · I_body⁻¹ · Rᵀ`.
fn world_inv_inertia(body: &RigidBody) -> Mat3 {
    let r = Mat3::from_quat(body.rotation);
    r * body.inv_inertia_tensor * r.transpose()
}

/// Apply a world-space rotation vector `delta` (axis · angle) to a quaternion,
/// the XPBD way: `q ← normalize(q + ½ · (delta, 0) · q)`. Matches the free-spin
/// integrator in the world stepper, so the conventions stay consistent.
fn apply_rotation(q: &mut Quat, delta: Vec3) {
    if delta == Vec3::ZERO {
        return;
    }
    let dq = Quat::from_xyzw(delta.x, delta.y, delta.z, 0.0) * *q;
    *q = (*q + dq * 0.5).normalize();
}

/// The shortest-arc rotation vector of a quaternion (≈ axis · angle for small
/// angles). Used to turn an orientation error quaternion into a correction.
fn rotation_vector(q: Quat) -> Vec3 {
    // Pick the hemisphere with positive scalar part so we rotate the short way.
    let q = if q.w < 0.0 { -q } else { q };
    // 2·imag(q) = 2·sin(θ/2)·axis ≈ θ·axis; exact enough since we iterate.
    Vec3::new(q.x, q.y, q.z) * 2.0
}

/// Any unit vector perpendicular to `v` (assumed unit). Deterministic.
fn any_perpendicular(v: Vec3) -> Vec3 {
    let seed = if v.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    v.cross(seed).normalize()
}

/// XPBD positional correction between anchor points on two bodies.
///
/// `correction` is the world-space error vector `C` (typically
/// `anchor_a_world - anchor_b_world`, optionally projected onto a subspace);
/// the bodies are nudged — translation *and* rotation — to drive `C → 0`.
/// `r_a` / `r_b` are the world-space anchor offsets from each center of mass.
/// `lambda` accumulates the Lagrange multiplier across iterations. Returns the
/// signed `Δλ` applied (its magnitude is the impulse, used for friction-style
/// cone limits).
#[allow(clippy::too_many_arguments)]
fn solve_positional(
    bodies: &mut [RigidBody],
    a: usize,
    b: usize,
    r_a: Vec3,
    r_b: Vec3,
    correction: Vec3,
    compliance: f32,
    dt: f32,
    lambda: &mut f32,
) -> f32 {
    let c = correction.length();
    if c < EPS {
        return 0.0;
    }
    let n = correction / c;

    let inv_i_a = world_inv_inertia(&bodies[a]);
    let inv_i_b = world_inv_inertia(&bodies[b]);
    let w_lin_a = bodies[a].inv_mass;
    let w_lin_b = bodies[b].inv_mass;

    let ra_x_n = r_a.cross(n);
    let rb_x_n = r_b.cross(n);
    let w_a = w_lin_a + ra_x_n.dot(inv_i_a * ra_x_n);
    let w_b = w_lin_b + rb_x_n.dot(inv_i_b * rb_x_n);
    let w = w_a + w_b;
    if w <= 0.0 {
        return 0.0;
    }

    let alpha = compliance / (dt * dt);
    let delta_lambda = (-c - alpha * *lambda) / (w + alpha);
    *lambda += delta_lambda;
    let p = n * delta_lambda;

    // Body A moves along +p; body B along -p (Newton's third law).
    bodies[a].position += p * w_lin_a;
    let rot_a = inv_i_a * r_a.cross(p);
    apply_rotation(&mut bodies[a].rotation, rot_a);

    bodies[b].position -= p * w_lin_b;
    let rot_b = inv_i_b * r_b.cross(p);
    apply_rotation(&mut bodies[b].rotation, -rot_b);

    delta_lambda
}

/// XPBD angular correction. `correction` is a world-space rotation vector
/// (axis · angle); the bodies are counter-rotated to drive it to zero using
/// only their angular degrees of freedom. Body A rotates by ∝ `-correction`,
/// body B by ∝ `+correction`.
fn solve_angular(
    bodies: &mut [RigidBody],
    a: usize,
    b: usize,
    correction: Vec3,
    compliance: f32,
    dt: f32,
    lambda: &mut f32,
) {
    let theta = correction.length();
    if theta < EPS {
        return;
    }
    let n = correction / theta;

    let inv_i_a = world_inv_inertia(&bodies[a]);
    let inv_i_b = world_inv_inertia(&bodies[b]);
    let w_a = n.dot(inv_i_a * n);
    let w_b = n.dot(inv_i_b * n);
    let w = w_a + w_b;
    if w <= 0.0 {
        return;
    }

    let alpha = compliance / (dt * dt);
    let delta_lambda = (-theta - alpha * *lambda) / (w + alpha);
    *lambda += delta_lambda;
    let p = n * delta_lambda;

    apply_rotation(&mut bodies[a].rotation, inv_i_a * p);
    apply_rotation(&mut bodies[b].rotation, -(inv_i_b * p));
}

/// World-space anchor offset (`r`, from the center of mass) and the world-space
/// anchor point itself, for a body and a local anchor.
fn anchor(body: &RigidBody, local: Vec3) -> (Vec3, Vec3) {
    let r = body.rotation * local;
    (r, body.position + r)
}

/// Constrain the signed angle from `n1` to `n2` about the common axis `n` to lie
/// within `[lo, hi]` radians. When the angle is outside the range, rotates `n1`
/// to the nearest limit and drives `n2` back onto it. (Müller et al.'s
/// `limit_angle`.)
#[allow(clippy::too_many_arguments)]
fn limit_angle(
    bodies: &mut [RigidBody],
    a: usize,
    b: usize,
    axis: Vec3,
    n1: Vec3,
    n2: Vec3,
    lo: f32,
    hi: f32,
    dt: f32,
    lambda: &mut f32,
) {
    use std::f32::consts::{PI, TAU};
    let mut phi = n1.cross(n2).dot(axis).clamp(-1.0, 1.0).asin();
    if n1.dot(n2) < 0.0 {
        phi = PI - phi;
    }
    if phi > PI {
        phi -= TAU;
    }
    if phi < -PI {
        phi += TAU;
    }
    if phi < lo || phi > hi {
        let target = phi.clamp(lo, hi);
        let n1_limited = Quat::from_axis_angle(axis, target) * n1;
        // Drive n2 onto the (rotated) limit reference. Same convention as the
        // axis-alignment correction below: corr = target × current.
        let corr = n1_limited.cross(n2);
        solve_angular(bodies, a, b, corr, 0.0, dt, lambda);
    }
}

/// A point-to-point (spherical) joint: keeps two local anchor points coincident
/// while leaving all three relative rotations free.
#[derive(Debug, Clone, Copy)]
pub struct BallJoint {
    pub body_a: usize,
    pub body_b: usize,
    pub local_anchor_a: Vec3,
    pub local_anchor_b: Vec3,
    pub compliance: f32,
    lambda_pos: f32,
}

impl BallJoint {
    pub fn new(
        body_a: usize,
        body_b: usize,
        local_anchor_a: Vec3,
        local_anchor_b: Vec3,
        compliance: f32,
    ) -> Self {
        Self { body_a, body_b, local_anchor_a, local_anchor_b, compliance, lambda_pos: 0.0 }
    }

    fn reset(&mut self) {
        self.lambda_pos = 0.0;
    }

    fn project(&mut self, bodies: &mut [RigidBody], dt: f32) {
        let (ra, wa) = anchor(&bodies[self.body_a], self.local_anchor_a);
        let (rb, wb) = anchor(&bodies[self.body_b], self.local_anchor_b);
        solve_positional(
            bodies,
            self.body_a,
            self.body_b,
            ra,
            rb,
            wa - wb,
            self.compliance,
            dt,
            &mut self.lambda_pos,
        );
    }
}

/// A hinge (revolute) joint: coincident anchors, the two hinge axes kept
/// parallel, and an optional limit on the swing angle about that axis.
#[derive(Debug, Clone, Copy)]
pub struct HingeJoint {
    pub body_a: usize,
    pub body_b: usize,
    pub local_anchor_a: Vec3,
    pub local_anchor_b: Vec3,
    /// Hinge axis in each body's local frame (unit).
    pub axis_a: Vec3,
    pub axis_b: Vec3,
    /// Perpendicular reference in each local frame, chosen at construction so
    /// the initial swing angle is zero; the angle limit is measured between them.
    ref_a: Vec3,
    ref_b: Vec3,
    /// `(min, max)` swing angle in radians, relative to the initial pose.
    pub limits: Option<(f32, f32)>,
    pub compliance: f32,
    lambda_pos: f32,
    lambda_align: f32,
    lambda_limit: f32,
}

impl HingeJoint {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bodies: &[RigidBody],
        body_a: usize,
        body_b: usize,
        local_anchor_a: Vec3,
        local_anchor_b: Vec3,
        axis_a: Vec3,
        axis_b: Vec3,
        limits: Option<(f32, f32)>,
        compliance: f32,
    ) -> Self {
        let axis_a = axis_a.normalize();
        let axis_b = axis_b.normalize();
        // Reference on A perpendicular to its axis, and the matching reference on
        // B that currently aligns with it → initial measured angle is 0.
        let ref_a = any_perpendicular(axis_a);
        let ref_a_world = bodies[body_a].rotation * ref_a;
        let ref_b = bodies[body_b].rotation.inverse() * ref_a_world;
        Self {
            body_a,
            body_b,
            local_anchor_a,
            local_anchor_b,
            axis_a,
            axis_b,
            ref_a,
            ref_b,
            limits,
            compliance,
            lambda_pos: 0.0,
            lambda_align: 0.0,
            lambda_limit: 0.0,
        }
    }

    fn reset(&mut self) {
        self.lambda_pos = 0.0;
        self.lambda_align = 0.0;
        self.lambda_limit = 0.0;
    }

    fn project(&mut self, bodies: &mut [RigidBody], dt: f32) {
        // 1. Keep the hinge axes parallel.
        let axis_a_w = bodies[self.body_a].rotation * self.axis_a;
        let axis_b_w = bodies[self.body_b].rotation * self.axis_b;
        solve_angular(
            bodies,
            self.body_a,
            self.body_b,
            axis_b_w.cross(axis_a_w),
            self.compliance,
            dt,
            &mut self.lambda_align,
        );

        // 2. Optional swing-angle limit about the (now aligned) axis.
        if let Some((lo, hi)) = self.limits {
            let axis = (bodies[self.body_a].rotation * self.axis_a).normalize();
            let n1 = bodies[self.body_a].rotation * self.ref_a;
            let n2 = bodies[self.body_b].rotation * self.ref_b;
            limit_angle(bodies, self.body_a, self.body_b, axis, n1, n2, lo, hi, dt, &mut self.lambda_limit);
        }

        // 3. Keep the anchor points coincident.
        let (ra, wa) = anchor(&bodies[self.body_a], self.local_anchor_a);
        let (rb, wb) = anchor(&bodies[self.body_b], self.local_anchor_b);
        solve_positional(
            bodies,
            self.body_a,
            self.body_b,
            ra,
            rb,
            wa - wb,
            self.compliance,
            dt,
            &mut self.lambda_pos,
        );
    }
}

/// A prismatic (sliding) joint: relative motion is confined to a single axis —
/// no relative rotation, no perpendicular translation — with an optional limit
/// on how far the bodies may slide apart along it.
#[derive(Debug, Clone, Copy)]
pub struct PrismaticJoint {
    pub body_a: usize,
    pub body_b: usize,
    pub local_anchor_a: Vec3,
    pub local_anchor_b: Vec3,
    /// Slide axis in body A's local frame (unit).
    pub axis_a: Vec3,
    /// Locked relative orientation (B-in-A) captured at construction.
    rest_rotation: Quat,
    /// `(min, max)` signed displacement along the axis, in metres.
    pub limits: Option<(f32, f32)>,
    pub compliance: f32,
    lambda_perp: f32,
    lambda_ang: f32,
    lambda_limit: f32,
}

impl PrismaticJoint {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bodies: &[RigidBody],
        body_a: usize,
        body_b: usize,
        local_anchor_a: Vec3,
        local_anchor_b: Vec3,
        axis_a: Vec3,
        limits: Option<(f32, f32)>,
        compliance: f32,
    ) -> Self {
        let rest_rotation = bodies[body_a].rotation.inverse() * bodies[body_b].rotation;
        Self {
            body_a,
            body_b,
            local_anchor_a,
            local_anchor_b,
            axis_a: axis_a.normalize(),
            rest_rotation,
            limits,
            compliance,
            lambda_perp: 0.0,
            lambda_ang: 0.0,
            lambda_limit: 0.0,
        }
    }

    fn reset(&mut self) {
        self.lambda_perp = 0.0;
        self.lambda_ang = 0.0;
        self.lambda_limit = 0.0;
    }

    fn project(&mut self, bodies: &mut [RigidBody], dt: f32) {
        // 1. Lock the relative orientation (no relative rotation).
        let q_target = bodies[self.body_a].rotation * self.rest_rotation;
        let q_err = q_target * bodies[self.body_b].rotation.inverse();
        solve_angular(
            bodies,
            self.body_a,
            self.body_b,
            rotation_vector(q_err),
            self.compliance,
            dt,
            &mut self.lambda_ang,
        );

        let axis = (bodies[self.body_a].rotation * self.axis_a).normalize();
        let (ra, wa) = anchor(&bodies[self.body_a], self.local_anchor_a);
        let (rb, wb) = anchor(&bodies[self.body_b], self.local_anchor_b);
        let sep = wa - wb;

        // 2. Remove the component of the separation perpendicular to the axis,
        //    leaving the slide along the axis free.
        let perp = sep - sep.dot(axis) * axis;
        solve_positional(
            bodies,
            self.body_a,
            self.body_b,
            ra,
            rb,
            perp,
            self.compliance,
            dt,
            &mut self.lambda_perp,
        );

        // 3. Optional travel limit along the axis.
        if let Some((lo, hi)) = self.limits {
            // Signed slide of B's anchor past A's anchor along the axis.
            let dist = (wb - wa).dot(axis);
            if dist < lo || dist > hi {
                let target = dist.clamp(lo, hi);
                // Excess part of (anchor_a - anchor_b) along the axis.
                let correction = axis * (target - dist);
                solve_positional(
                    bodies,
                    self.body_a,
                    self.body_b,
                    ra,
                    rb,
                    correction,
                    0.0,
                    dt,
                    &mut self.lambda_limit,
                );
            }
        }
    }
}

/// A fixed (weld) joint: locks both relative position and relative orientation,
/// rigidly welding the two bodies together at the captured offset.
#[derive(Debug, Clone, Copy)]
pub struct FixedJoint {
    pub body_a: usize,
    pub body_b: usize,
    pub local_anchor_a: Vec3,
    pub local_anchor_b: Vec3,
    rest_rotation: Quat,
    pub compliance: f32,
    lambda_pos: f32,
    lambda_ang: f32,
}

impl FixedJoint {
    pub fn new(
        bodies: &[RigidBody],
        body_a: usize,
        body_b: usize,
        local_anchor_a: Vec3,
        local_anchor_b: Vec3,
        compliance: f32,
    ) -> Self {
        let rest_rotation = bodies[body_a].rotation.inverse() * bodies[body_b].rotation;
        Self {
            body_a,
            body_b,
            local_anchor_a,
            local_anchor_b,
            rest_rotation,
            compliance,
            lambda_pos: 0.0,
            lambda_ang: 0.0,
        }
    }

    fn reset(&mut self) {
        self.lambda_pos = 0.0;
        self.lambda_ang = 0.0;
    }

    fn project(&mut self, bodies: &mut [RigidBody], dt: f32) {
        // Orientation: drive B toward A's orientation composed with the rest
        // offset.
        let q_target = bodies[self.body_a].rotation * self.rest_rotation;
        let q_err = q_target * bodies[self.body_b].rotation.inverse();
        solve_angular(
            bodies,
            self.body_a,
            self.body_b,
            rotation_vector(q_err),
            self.compliance,
            dt,
            &mut self.lambda_ang,
        );

        // Position: keep the anchor points coincident.
        let (ra, wa) = anchor(&bodies[self.body_a], self.local_anchor_a);
        let (rb, wb) = anchor(&bodies[self.body_b], self.local_anchor_b);
        solve_positional(
            bodies,
            self.body_a,
            self.body_b,
            ra,
            rb,
            wa - wb,
            self.compliance,
            dt,
            &mut self.lambda_pos,
        );
    }
}

/// A joint stored in the world, dispatched without heap allocation.
#[derive(Debug, Clone, Copy)]
pub enum Joint {
    Ball(BallJoint),
    Hinge(HingeJoint),
    Prismatic(PrismaticJoint),
    Fixed(FixedJoint),
}

impl Joint {
    /// Clear accumulated Lagrange multipliers; called once per substep.
    pub fn reset(&mut self) {
        match self {
            Joint::Ball(j) => j.reset(),
            Joint::Hinge(j) => j.reset(),
            Joint::Prismatic(j) => j.reset(),
            Joint::Fixed(j) => j.reset(),
        }
    }

    /// Apply one projection iteration.
    pub fn project(&mut self, bodies: &mut [RigidBody], dt: f32) {
        match self {
            Joint::Ball(j) => j.project(bodies, dt),
            Joint::Hinge(j) => j.project(bodies, dt),
            Joint::Prismatic(j) => j.project(bodies, dt),
            Joint::Fixed(j) => j.project(bodies, dt),
        }
    }

    /// The two bodies this joint couples (for sleeping-island grouping).
    pub fn bodies(&self) -> (usize, usize) {
        match self {
            Joint::Ball(j) => (j.body_a, j.body_b),
            Joint::Hinge(j) => (j.body_a, j.body_b),
            Joint::Prismatic(j) => (j.body_a, j.body_b),
            Joint::Fixed(j) => (j.body_a, j.body_b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Collider;

    fn point(pos: Vec3) -> RigidBody {
        RigidBody::dynamic(pos, 1.0, Collider::Sphere { radius: 0.1 })
    }

    fn unit_box(pos: Vec3) -> RigidBody {
        RigidBody::dynamic(pos, 1.0, Collider::Box { half_extents: Vec3::splat(0.5) })
    }

    #[test]
    fn solve_positional_pulls_anchors_together() {
        // Two equal-mass points 2 apart, anchors at their centers: a ball-joint
        // correction (C = anchor_a - anchor_b) must close the gap symmetrically.
        let mut bodies = vec![point(Vec3::ZERO), point(Vec3::new(2.0, 0.0, 0.0))];
        for _ in 0..30 {
            let mut lambda = 0.0;
            let c = bodies[0].position - bodies[1].position;
            solve_positional(&mut bodies, 0, 1, Vec3::ZERO, Vec3::ZERO, c, 0.0, 1.0 / 60.0, &mut lambda);
        }
        assert!((bodies[0].position - bodies[1].position).length() < 1e-3);
        // Symmetric: they met in the middle.
        assert!((bodies[0].position.x - 1.0).abs() < 1e-3);
        assert!((bodies[1].position.x - 1.0).abs() < 1e-3);
    }

    #[test]
    fn solve_angular_aligns_axes() {
        // Body A's local X aligned to body B's local X, starting 90° apart.
        let mut bodies = vec![unit_box(Vec3::ZERO), unit_box(Vec3::ZERO)];
        bodies[1].rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        for _ in 0..60 {
            let a_w = bodies[0].rotation * Vec3::X;
            let b_w = bodies[1].rotation * Vec3::X;
            // Align u_a → u_b: correction Δθ = u_b × u_a.
            let mut lambda = 0.0;
            solve_angular(&mut bodies, 0, 1, b_w.cross(a_w), 0.0, 1.0 / 60.0, &mut lambda);
        }
        let a_w = bodies[0].rotation * Vec3::X;
        let b_w = bodies[1].rotation * Vec3::X;
        assert!(a_w.dot(b_w) > 0.999, "axes did not align: dot {}", a_w.dot(b_w));
    }

    #[test]
    fn rotation_vector_of_small_rotation() {
        let q = Quat::from_axis_angle(Vec3::Y, 0.1);
        let v = rotation_vector(q);
        assert!((v - Vec3::new(0.0, 0.1, 0.0)).length() < 1e-3);
    }
}
