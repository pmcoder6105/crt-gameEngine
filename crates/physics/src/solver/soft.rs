//! XPBD constraints over [`Particle`](crate::soft::Particle)s — the solver half
//! of soft bodies and cloth — plus the particle↔rigid contact that couples them
//! to the rigid-body world.
//!
//! These mirror the rigid [`xpbd`](crate::solver::xpbd) constraints but act on
//! the world's flat particle array. Two kinds are intrinsic to a soft body /
//! cloth:
//!
//! * [`ParticleDistance`] — holds two particles a rest length apart. Cloth's
//!   structural, shear, and bending springs are all this constraint with
//!   different particle pairs; a soft body's tet edges are too.
//! * [`ParticleVolume`] — preserves the signed volume of a tetrahedron, which is
//!   what stops a soft body from collapsing under load.
//!
//! and one couples to rigid bodies:
//!
//! * [`ParticleBodyContact`] — non-penetration of a particle against a rigid
//!   collider, with Coulomb friction. Like the rigid contact path it is
//!   *linear-only* (it pushes the rigid body at its centre of mass, applying no
//!   torque), so a tumbling cube keeps tumbling under its own angular momentum
//!   while the cloth rides its faces.

use elderforge_core::math::Vec3;

use crate::body::{Collider, RigidBody};
use crate::soft::{signed_tet_volume, Particle};

const EPS: f32 = 1e-9;

/// Distance constraint between two particles. Used for cloth structural / shear
/// / bending springs and for soft-body tet edges. `compliance` is inverse
/// stiffness (0 = a rigid link).
#[derive(Debug, Clone, Copy)]
pub struct ParticleDistance {
    pub a: usize,
    pub b: usize,
    pub rest_length: f32,
    pub compliance: f32,
    lambda: f32,
}

impl ParticleDistance {
    pub fn new(a: usize, b: usize, rest_length: f32, compliance: f32) -> Self {
        Self { a, b, rest_length, compliance, lambda: 0.0 }
    }

    pub fn reset(&mut self) {
        self.lambda = 0.0;
    }

    /// One projection iteration. Identical in spirit to the rigid
    /// [`DistanceConstraint`](crate::solver::DistanceConstraint) but over the
    /// particle array.
    pub fn project(&mut self, particles: &mut [Particle], dt: f32) {
        if self.a == self.b {
            return;
        }
        let wa = particles[self.a].inv_mass;
        let wb = particles[self.b].inv_mass;
        let w = wa + wb;
        if w <= 0.0 {
            return;
        }
        let delta = particles[self.a].position - particles[self.b].position;
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
        particles[self.a].position += p * wa;
        particles[self.b].position -= p * wb;
    }
}

/// Tetrahedral volume-preservation constraint. Drives the signed volume of
/// `(a, b, c, d)` back to `rest_volume`, distributing the correction along the
/// per-vertex volume gradients — the constraint that gives a soft body its
/// resistance to being squashed.
#[derive(Debug, Clone, Copy)]
pub struct ParticleVolume {
    pub a: usize,
    pub b: usize,
    pub c: usize,
    pub d: usize,
    pub rest_volume: f32,
    pub compliance: f32,
    lambda: f32,
}

impl ParticleVolume {
    pub fn new(indices: [usize; 4], rest_volume: f32, compliance: f32) -> Self {
        Self {
            a: indices[0],
            b: indices[1],
            c: indices[2],
            d: indices[3],
            rest_volume,
            compliance,
            lambda: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.lambda = 0.0;
    }

    pub fn project(&mut self, particles: &mut [Particle], dt: f32) {
        let (pa, pb, pc, pd) = (
            particles[self.a].position,
            particles[self.b].position,
            particles[self.c].position,
            particles[self.d].position,
        );
        // ∇V wrt each vertex (the ⅙ factor carries through into the gradients).
        let grad_b = (pc - pa).cross(pd - pa) / 6.0;
        let grad_c = (pd - pa).cross(pb - pa) / 6.0;
        let grad_d = (pb - pa).cross(pc - pa) / 6.0;
        let grad_a = -(grad_b + grad_c + grad_d);

        let (wa, wb, wc, wd) = (
            particles[self.a].inv_mass,
            particles[self.b].inv_mass,
            particles[self.c].inv_mass,
            particles[self.d].inv_mass,
        );
        let w = wa * grad_a.length_squared()
            + wb * grad_b.length_squared()
            + wc * grad_c.length_squared()
            + wd * grad_d.length_squared();
        if w < EPS {
            return;
        }
        let vol = signed_tet_volume(pa, pb, pc, pd);
        let c = vol - self.rest_volume;
        let alpha = self.compliance / (dt * dt);
        let delta_lambda = (-c - alpha * self.lambda) / (w + alpha);
        self.lambda += delta_lambda;
        particles[self.a].position += grad_a * (wa * delta_lambda);
        particles[self.b].position += grad_b * (wb * delta_lambda);
        particles[self.c].position += grad_c * (wc * delta_lambda);
        particles[self.d].position += grad_d * (wd * delta_lambda);
    }
}

/// Non-penetration contact between a particle and a rigid body, with Coulomb
/// friction. `normal` points out of the rigid collider toward the particle (the
/// direction the particle is pushed); `depth0` is the penetration measured when
/// the contact was generated and is re-measured from displacement during the
/// solve (so multiple iterations don't over-correct).
///
/// Linear-only and two-way: the particle is pushed out along `+normal` and the
/// rigid body recoils along `−normal`, split by inverse mass. Friction cancels
/// tangential sliding while it fits inside the cone `λ_t ≤ μ · λ_n`.
#[derive(Debug, Clone, Copy)]
pub struct ParticleBodyContact {
    pub particle: usize,
    pub body: usize,
    pub normal: Vec3,
    pub friction: f32,
    depth0: f32,
    anchor_particle: Vec3,
    anchor_body: Vec3,
    lambda: f32,
    lambda_t: f32,
}

impl ParticleBodyContact {
    /// Test `particle` against the rigid `body`, returning a contact if the
    /// particle (a sphere of its collision radius) overlaps the collider.
    pub fn generate(
        particle_idx: usize,
        particle: &Particle,
        body_idx: usize,
        body: &RigidBody,
    ) -> Option<Self> {
        // Two immovable things never need a contact (a pinned particle on a
        // static body would have nowhere to go anyway).
        if particle.inv_mass == 0.0 && body.inv_mass == 0.0 {
            return None;
        }
        let (normal, depth) = closest_on_collider(
            particle.position,
            particle.radius,
            body.collider,
            body.position,
            body.rotation,
        )?;
        Some(Self {
            particle: particle_idx,
            body: body_idx,
            normal,
            friction: body.material.static_friction,
            depth0: depth,
            anchor_particle: particle.position,
            anchor_body: body.position,
            lambda: 0.0,
            lambda_t: 0.0,
        })
    }

    pub fn reset(&mut self) {
        self.lambda = 0.0;
        self.lambda_t = 0.0;
    }

    /// Penetration re-measured from how far the particle and body have moved
    /// along the (fixed) contact normal since generation.
    fn current_depth(&self, particles: &[Particle], bodies: &[RigidBody]) -> f32 {
        let moved_p = particles[self.particle].position - self.anchor_particle;
        let moved_b = bodies[self.body].position - self.anchor_body;
        self.depth0 - (moved_p - moved_b).dot(self.normal)
    }

    /// One projection iteration: resolve penetration, then static friction. A
    /// particle↔rigid contact is zero-compliance, so `dt` only carries through
    /// for signature symmetry with the other constraints.
    pub fn project(&mut self, particles: &mut [Particle], bodies: &mut [RigidBody], _dt: f32) {
        let wp = particles[self.particle].inv_mass;
        let wb = bodies[self.body].inv_mass;
        let w = wp + wb;
        if w <= 0.0 {
            return;
        }
        let depth = self.current_depth(particles, bodies);
        if depth > 0.0 {
            let delta_lambda = depth / w; // rigid (zero-compliance) contact
            self.lambda += delta_lambda;
            let p = self.normal * delta_lambda;
            particles[self.particle].position += p * wp; // particle moves +normal (out)
            bodies[self.body].position -= p * wb; // body recoils -normal
        }
        if self.lambda > 0.0 {
            self.apply_static_friction(particles, bodies);
        }
    }

    /// Position-level static friction: undo the tangential slide of the particle
    /// relative to the body since the substep began, provided it fits inside the
    /// Coulomb cone `λ_t ≤ μ · λ_n`.
    fn apply_static_friction(&mut self, particles: &mut [Particle], bodies: &mut [RigidBody]) {
        let wp = particles[self.particle].inv_mass;
        let wb = bodies[self.body].inv_mass;
        let w = wp + wb;
        if w <= 0.0 {
            return;
        }
        let rel = (particles[self.particle].position - particles[self.particle].prev_position)
            - (bodies[self.body].position - bodies[self.body].prev_position);
        let rel_t = rel - rel.dot(self.normal) * self.normal;
        let c = rel_t.length();
        if c < EPS {
            return;
        }
        let needed = c / w;
        if self.lambda_t + needed > self.friction * self.lambda {
            return; // sliding — leave it (no dynamic-friction pass for particles)
        }
        self.lambda_t += needed;
        let p = rel_t / w;
        particles[self.particle].position -= p * wp;
        bodies[self.body].position += p * wb;
    }
}

/// Closest-surface query of a point (a sphere of `radius`) against a rigid
/// `collider` posed at `position`/`rotation`. Returns `(normal, depth)` where
/// `normal` points out of the collider toward the point and `depth > 0` is the
/// penetration, or `None` when the sphere is clear of the surface.
pub fn closest_on_collider(
    point: Vec3,
    radius: f32,
    collider: Collider,
    position: Vec3,
    rotation: elderforge_core::math::Quat,
) -> Option<(Vec3, f32)> {
    match collider {
        Collider::HalfSpace { normal, offset } => {
            let signed = point.dot(normal) - offset;
            let depth = radius - signed;
            (depth > 0.0).then_some((normal, depth))
        }
        Collider::Sphere { radius: sr } => {
            let d = point - position;
            let len = d.length();
            let depth = sr + radius - len;
            if depth <= 0.0 {
                return None;
            }
            let normal = if len > EPS { d / len } else { Vec3::Y };
            Some((normal, depth))
        }
        Collider::Box { half_extents } => {
            // Work in the box's local frame, then rotate the normal back.
            let local = rotation.inverse() * (point - position);
            let clamped = local.clamp(-half_extents, half_extents);
            let diff = local - clamped;
            let dist = diff.length();
            if dist > EPS {
                // Outside the box: contact if within the particle radius.
                let depth = radius - dist;
                if depth <= 0.0 {
                    return None;
                }
                Some((rotation * (diff / dist), depth))
            } else {
                // Inside the box: push out through the nearest face.
                let pen = half_extents - local.abs();
                let (axis, &min_pen) = [pen.x, pen.y, pen.z]
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.total_cmp(b.1))
                    .expect("three axes");
                let mut n = Vec3::ZERO;
                let sign = if local[axis] >= 0.0 { 1.0 } else { -1.0 };
                n[axis] = sign;
                Some((rotation * n, min_pen + radius))
            }
        }
        Collider::Capsule { radius: cr, half_height } => {
            // Closest point on the local Y-aligned segment, then sphere-style.
            let local = rotation.inverse() * (point - position);
            let t = local.y.clamp(-half_height, half_height);
            let seg = Vec3::new(0.0, t, 0.0);
            let d = local - seg;
            let len = d.length();
            let depth = cr + radius - len;
            if depth <= 0.0 {
                return None;
            }
            let n_local = if len > EPS { d / len } else { Vec3::X };
            Some((rotation * n_local, depth))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::RigidBody;
    use elderforge_core::math::Quat;

    fn particle(pos: Vec3, inv_mass: f32) -> Particle {
        Particle::new(pos, inv_mass, 0.1)
    }

    #[test]
    fn distance_pulls_particles_to_rest_length() {
        let mut ps = vec![
            particle(Vec3::ZERO, 1.0),
            particle(Vec3::new(3.0, 0.0, 0.0), 1.0),
        ];
        let mut c = ParticleDistance::new(0, 1, 1.0, 0.0);
        for _ in 0..20 {
            c.reset();
            c.project(&mut ps, 1.0 / 60.0);
        }
        let len = (ps[0].position - ps[1].position).length();
        assert!((len - 1.0).abs() < 1e-4, "len {len}");
    }

    #[test]
    fn pinned_particle_absorbs_no_correction() {
        let mut ps = vec![
            particle(Vec3::ZERO, 0.0), // pinned
            particle(Vec3::new(3.0, 0.0, 0.0), 1.0),
        ];
        let mut c = ParticleDistance::new(0, 1, 1.0, 0.0);
        c.project(&mut ps, 1.0 / 60.0);
        assert_eq!(ps[0].position, Vec3::ZERO);
        assert!(ps[1].position.x < 3.0);
    }

    #[test]
    fn volume_constraint_restores_a_squashed_tet() {
        // A unit-corner tet; squash vertex d toward the base, then let the
        // volume constraint pull it back to restore the rest volume.
        let rest = [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z];
        let rest_vol = signed_tet_volume(rest[0], rest[1], rest[2], rest[3]);
        let mut ps = vec![
            particle(rest[0], 1.0),
            particle(rest[1], 1.0),
            particle(rest[2], 1.0),
            particle(Vec3::new(0.0, 0.0, 0.3), 1.0), // d squashed inward
        ];
        let mut c = ParticleVolume::new([0, 1, 2, 3], rest_vol, 0.0);
        for _ in 0..40 {
            c.reset();
            c.project(&mut ps, 1.0 / 60.0);
        }
        let vol = signed_tet_volume(ps[0].position, ps[1].position, ps[2].position, ps[3].position);
        assert!((vol - rest_vol).abs() < 1e-4, "vol {vol} != rest {rest_vol}");
    }

    #[test]
    fn particle_resting_on_halfspace_is_pushed_out() {
        let ground = RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 });
        // Particle radius 0.1, centre at y = -0.05 → penetrating by 0.15.
        let mut ps = vec![particle(Vec3::new(0.0, -0.05, 0.0), 1.0)];
        let mut bodies = vec![ground];
        let mut contact = ParticleBodyContact::generate(0, &ps[0], 0, &bodies[0]).expect("contact");
        contact.project(&mut ps, &mut bodies, 1.0 / 60.0);
        // Pushed up to rest on the plane at y = radius.
        assert!((ps[0].position.y - 0.1).abs() < 1e-5, "y {}", ps[0].position.y);
        assert_eq!(bodies[0].position, Vec3::ZERO, "static body unmoved");
    }

    #[test]
    fn point_inside_box_exits_through_nearest_face() {
        // Point just inside the +X face of a unit box.
        let (n, depth) = closest_on_collider(
            Vec3::new(0.45, 0.0, 0.0),
            0.0,
            Collider::Box { half_extents: Vec3::splat(0.5) },
            Vec3::ZERO,
            Quat::IDENTITY,
        )
        .expect("inside the box");
        assert!((n - Vec3::X).length() < 1e-6, "normal {n:?}");
        assert!((depth - 0.05).abs() < 1e-6, "depth {depth}");
    }

    #[test]
    fn point_clear_of_box_has_no_contact() {
        let hit = closest_on_collider(
            Vec3::new(2.0, 0.0, 0.0),
            0.1,
            Collider::Box { half_extents: Vec3::splat(0.5) },
            Vec3::ZERO,
            Quat::IDENTITY,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn two_way_contact_pushes_dynamic_body() {
        // A movable particle and a movable sphere overlapping head-on: both move.
        let body = RigidBody::dynamic(Vec3::ZERO, 1.0, Collider::Sphere { radius: 0.5 });
        let mut ps = vec![particle(Vec3::new(0.4, 0.0, 0.0), 1.0)]; // radius 0.1
        let mut bodies = vec![body];
        let mut contact = ParticleBodyContact::generate(0, &ps[0], 0, &bodies[0]).expect("contact");
        contact.project(&mut ps, &mut bodies, 1.0 / 60.0);
        assert!(ps[0].position.x > 0.4, "particle pushed +X");
        assert!(bodies[0].position.x < 0.0, "body recoils -X");
    }
}
