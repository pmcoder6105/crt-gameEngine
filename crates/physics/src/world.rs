//! PhysicsWorld — owns the bodies, the constraints, and the XPBD substep loop.

use elderforge_core::math::{Quat, Vec3};

use crate::body::{BodyHandle, BodyKind, Collider, RigidBody};
use crate::broadphase::Bvh;
use crate::narrowphase::{collide, surface_support, AnyShape, ContactManifold, Pose};
use crate::shapes::{BoxShape, Sphere};
use crate::solver::{Constraint, ContactConstraint, DistanceConstraint};
use crate::PhysicsError;

/// Default XPBD substeps per [`step`](PhysicsWorld::step). Many small substeps
/// give XPBD its stiffness and stability; this is the primary quality knob.
pub const DEFAULT_SUBSTEPS: u32 = 20;
/// Default constraint-projection iterations per substep.
pub const DEFAULT_ITERATIONS: u32 = 4;

pub struct PhysicsWorld {
    bodies: Vec<RigidBody>,
    generations: Vec<u32>,
    distance_constraints: Vec<DistanceConstraint>,
    pub gravity: Vec3,
    /// Substeps per frame (configurable). See [`DEFAULT_SUBSTEPS`].
    pub substeps: u32,
    /// Projection iterations per substep.
    pub iterations: u32,
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            generations: Vec::new(),
            distance_constraints: Vec::new(),
            gravity: Vec3::new(0.0, -9.81, 0.0),
            substeps: DEFAULT_SUBSTEPS,
            iterations: DEFAULT_ITERATIONS,
        }
    }

    pub fn add_rigid_body(&mut self, body: RigidBody) -> BodyHandle {
        // TODO: reuse freed slots via a free list (mirrors core's HandleAllocator).
        let index = self.bodies.len() as u32;
        self.bodies.push(body);
        self.generations.push(0);
        BodyHandle::new(index, 0)
    }

    /// Invalidates the handle by bumping the slot's generation.
    pub fn remove_rigid_body(&mut self, handle: BodyHandle) -> Result<(), PhysicsError> {
        let generation = self
            .generations
            .get_mut(handle.index() as usize)
            .ok_or(PhysicsError::InvalidHandle)?;
        if *generation != handle.generation() {
            return Err(PhysicsError::InvalidHandle);
        }
        *generation += 1;
        // The slot stays allocated; park the body so the solver and broadphase
        // skip it. TODO: free list so removed slots get reused.
        if let Some(body) = self.bodies.get_mut(handle.index() as usize) {
            body.sleeping = true;
        }
        log::debug!("removed rigid body {handle:?}");
        Ok(())
    }

    pub fn body(&self, handle: BodyHandle) -> Option<&RigidBody> {
        if *self.generations.get(handle.index() as usize)? != handle.generation() {
            return None;
        }
        self.bodies.get(handle.index() as usize)
    }

    pub fn body_mut(&mut self, handle: BodyHandle) -> Option<&mut RigidBody> {
        if *self.generations.get(handle.index() as usize)? != handle.generation() {
            return None;
        }
        self.bodies.get_mut(handle.index() as usize)
    }

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Index of a live body, validating the handle's generation.
    fn index_of(&self, handle: BodyHandle) -> Option<usize> {
        if *self.generations.get(handle.index() as usize)? != handle.generation() {
            return None;
        }
        Some(handle.index() as usize)
    }

    /// Add a distance constraint between two bodies (a rope link, a pendulum
    /// arm). `compliance` is inverse stiffness: 0 is a rigid rod.
    pub fn add_distance_constraint(
        &mut self,
        a: BodyHandle,
        b: BodyHandle,
        rest_length: f32,
        compliance: f32,
    ) {
        if let (Some(ia), Some(ib)) = (self.index_of(a), self.index_of(b)) {
            self.distance_constraints
                .push(DistanceConstraint::new(ia, ib, rest_length, compliance));
        }
    }

    /// Advance the simulation by `frame_dt` seconds via the XPBD substep loop.
    pub fn step(&mut self, frame_dt: f32) {
        let substeps = self.substeps.max(1);
        let dt = frame_dt / substeps as f32;
        for _ in 0..substeps {
            self.substep(dt);
        }
    }

    /// One XPBD substep: predict, generate contacts, project, derive velocity,
    /// apply restitution.
    fn substep(&mut self, dt: f32) {
        let gravity = self.gravity;

        // 1. Predict positions (and integrate free rotation).
        for body in &mut self.bodies {
            body.prev_position = body.position;
            if body.kind != BodyKind::Dynamic || body.inv_mass == 0.0 || body.sleeping {
                continue;
            }
            body.position += body.linear_velocity * dt + gravity * (dt * dt);
            let omega = body.angular_velocity;
            if omega != Vec3::ZERO {
                let spin = Quat::from_xyzw(omega.x, omega.y, omega.z, 0.0) * body.rotation;
                body.rotation = (body.rotation + spin * (0.5 * dt)).normalize();
            }
        }

        // 2. Broadphase + narrowphase -> contact constraints (at predicted state).
        let mut contacts = self.generate_contacts();

        // 3. Project all constraints for several iterations.
        for c in &mut self.distance_constraints {
            c.reset();
        }
        for _ in 0..self.iterations.max(1) {
            for c in &mut self.distance_constraints {
                c.project(&mut self.bodies, dt);
            }
            for c in &mut contacts {
                c.project(&mut self.bodies, dt);
            }
        }

        // 4. Derive velocities from the position change.
        for body in &mut self.bodies {
            if body.kind != BodyKind::Dynamic || body.inv_mass == 0.0 || body.sleeping {
                continue;
            }
            body.linear_velocity = (body.position - body.prev_position) / dt;
        }

        // 5. Velocity-level restitution.
        for c in &contacts {
            c.apply_restitution(&mut self.bodies);
        }
    }

    /// Build contact constraints for this substep. Finite-AABB bodies go through
    /// the BVH; unbounded half-spaces are tested against every finite body.
    fn generate_contacts(&self) -> Vec<ContactConstraint> {
        let mut finite_idx = Vec::new();
        let mut finite_aabbs = Vec::new();
        let mut unbounded = Vec::new();
        for (i, body) in self.bodies.iter().enumerate() {
            if body.sleeping {
                continue;
            }
            let aabb = body.collider.aabb(body.position);
            if aabb.is_finite() {
                finite_idx.push(i);
                finite_aabbs.push(aabb);
            } else {
                unbounded.push(i);
            }
        }

        let mut contacts = Vec::new();
        let bvh = Bvh::build(&finite_aabbs);
        for (la, lb) in bvh.query_pairs() {
            if let Some(c) = self.make_contact(finite_idx[la], finite_idx[lb]) {
                contacts.push(c);
            }
        }
        for &u in &unbounded {
            for &f in &finite_idx {
                if let Some(c) = self.make_contact(u, f) {
                    contacts.push(c);
                }
            }
        }
        contacts
    }

    /// Narrowphase a candidate pair into a contact constraint, ordered so the
    /// constraint's normal points from the lower-indexed body to the higher.
    fn make_contact(&self, i: usize, j: usize) -> Option<ContactConstraint> {
        if i == j {
            return None;
        }
        let (lo, hi) = if i < j { (i, j) } else { (j, i) };
        let a = &self.bodies[lo];
        let b = &self.bodies[hi];
        if a.inv_mass == 0.0 && b.inv_mass == 0.0 {
            return None; // two static bodies never collide
        }
        let manifold = world_collide(a.collider, pose_of(a), b.collider, pose_of(b))?;
        let restitution = a.material.restitution.min(b.material.restitution);
        Some(ContactConstraint::new(
            lo,
            hi,
            manifold.normal,
            manifold.depth,
            restitution,
            0.0, // rigid contacts
            &self.bodies,
        ))
    }
}

/// Pose (position + orientation) of a body.
fn pose_of(body: &RigidBody) -> Pose {
    Pose::new(body.position, body.rotation)
}

/// Convert a collider into a GJK-ready convex shape (`None` for the unbounded
/// half-space, which is handled separately).
fn as_convex(collider: Collider) -> Option<AnyShape> {
    match collider {
        Collider::Sphere { radius } => Some(AnyShape::Sphere(Sphere { radius })),
        Collider::Box { half_extents } => Some(AnyShape::Cuboid(BoxShape { half_extents })),
        Collider::HalfSpace { .. } => None,
    }
}

/// Collide two world colliders, returning a manifold whose normal points from
/// the first (A) toward the second (B).
fn world_collide(
    ca: Collider,
    pa: Pose,
    cb: Collider,
    pb: Pose,
) -> Option<ContactManifold> {
    match (ca, cb) {
        // A is the plane: normal already points plane -> convex == A -> B.
        (Collider::HalfSpace { normal, offset }, _) => halfspace_contact(normal, offset, cb, pb),
        // B is the plane: flip so the normal points convex -> plane == A -> B.
        (_, Collider::HalfSpace { normal, offset }) => {
            halfspace_contact(normal, offset, ca, pa).map(|m| ContactManifold {
                normal: -m.normal,
                ..m
            })
        }
        // Two finite convex shapes: GJK/EPA.
        (_, _) => {
            let sa = as_convex(ca)?;
            let sb = as_convex(cb)?;
            collide(&sa, &pa, &sb, &pb)
        }
    }
}

/// Contact of a convex collider against a static half-space. The returned
/// normal is the plane normal (pointing out of the solid, toward the shape).
fn halfspace_contact(
    plane_normal: Vec3,
    offset: f32,
    collider: Collider,
    pose: Pose,
) -> Option<ContactManifold> {
    let shape = as_convex(collider)?;
    // Deepest point of the shape into the solid (down the plane normal).
    let deepest = surface_support(&shape, &pose, -plane_normal);
    let signed = deepest.dot(plane_normal) - offset;
    if signed >= 0.0 {
        return None;
    }
    Some(ContactManifold {
        contact_point: deepest,
        normal: plane_normal,
        depth: -signed,
    })
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_fetch_body() {
        let mut world = PhysicsWorld::new();
        let handle = world.add_rigid_body(RigidBody::default());
        assert_eq!(world.body_count(), 1);
        assert!(world.body(handle).is_some());
        assert!(world.body_mut(handle).is_some());
    }

    #[test]
    fn removed_handle_is_stale() {
        let mut world = PhysicsWorld::new();
        let handle = world.add_rigid_body(RigidBody::default());
        world.remove_rigid_body(handle).expect("first removal works");
        assert!(world.body(handle).is_none());
        assert_eq!(
            world.remove_rigid_body(handle),
            Err(PhysicsError::InvalidHandle)
        );
    }

    #[test]
    fn gravity_pulls_dynamic_bodies_down() {
        let mut world = PhysicsWorld::new();
        let handle = world.add_rigid_body(RigidBody::default());
        world.step(1.0 / 60.0);
        let body = world.body(handle).expect("body exists");
        assert!(body.position.y < 0.0);
        assert!(body.linear_velocity.y < 0.0);
    }

    #[test]
    fn configurable_gravity_direction() {
        let mut world = PhysicsWorld::new();
        world.gravity = Vec3::new(5.0, 0.0, 0.0);
        let handle = world.add_rigid_body(RigidBody::default());
        world.step(1.0 / 60.0);
        let body = world.body(handle).expect("body exists");
        assert!(body.linear_velocity.x > 0.0);
        assert_eq!(body.linear_velocity.y, 0.0);
    }

    #[test]
    fn substep_count_is_configurable() {
        let mut world = PhysicsWorld::new();
        world.substeps = 5;
        assert_eq!(world.substeps, 5);
        world.add_rigid_body(RigidBody::default());
        world.step(1.0 / 60.0); // must not panic with a custom substep count
    }
}
