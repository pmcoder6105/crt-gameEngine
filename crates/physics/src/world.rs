//! PhysicsWorld — owns the bodies, the constraints, and the XPBD substep loop.

use elderforge_core::math::{Quat, Vec3};

use crate::body::{BodyHandle, BodyKind, Collider, RigidBody};
use crate::broadphase::Bvh;
use crate::narrowphase::{collide, surface_support, AnyShape, ContactManifold, Pose};
use crate::shapes::{BoxShape, Capsule, Sphere};
use crate::solver::{
    BallJoint, Constraint, ContactConstraint, DistanceConstraint, FixedJoint, HingeJoint, Joint,
    PrismaticJoint,
};
use crate::PhysicsError;

/// Default XPBD substeps per [`step`](PhysicsWorld::step). Many small substeps
/// give XPBD its stiffness and stability; this is the primary quality knob.
pub const DEFAULT_SUBSTEPS: u32 = 20;
/// Default constraint-projection iterations per substep.
pub const DEFAULT_ITERATIONS: u32 = 4;

/// Default linear speed (m/s) below which a body counts as "quiet" for sleeping.
pub const DEFAULT_LINEAR_SLEEP_THRESHOLD: f32 = 0.05;
/// Default angular speed (rad/s) below which a body counts as "quiet".
pub const DEFAULT_ANGULAR_SLEEP_THRESHOLD: f32 = 0.05;
/// Default number of consecutive quiet frames before an island sleeps.
pub const DEFAULT_SLEEP_FRAMES: u32 = 30;

pub struct PhysicsWorld {
    bodies: Vec<RigidBody>,
    generations: Vec<u32>,
    distance_constraints: Vec<DistanceConstraint>,
    joints: Vec<Joint>,
    pub gravity: Vec3,
    /// Substeps per frame (configurable). See [`DEFAULT_SUBSTEPS`].
    pub substeps: u32,
    /// Projection iterations per substep.
    pub iterations: u32,
    /// Master switch for the sleeping system. When off, every dynamic body is
    /// integrated every frame (handy for tests that want to isolate behavior).
    pub sleeping_enabled: bool,
    /// Linear speed below which a body is considered quiet for sleeping.
    pub linear_sleep_threshold: f32,
    /// Angular speed below which a body is considered quiet for sleeping.
    pub angular_sleep_threshold: f32,
    /// Consecutive quiet frames an island must accrue before it sleeps.
    pub sleep_frames: u32,
    /// Narrowphase candidate pairs actually tested during the last [`step`].
    /// A fully asleep world drives this to zero — the sleeping system's payoff.
    last_narrowphase_tests: usize,
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            generations: Vec::new(),
            distance_constraints: Vec::new(),
            joints: Vec::new(),
            gravity: Vec3::new(0.0, -9.81, 0.0),
            substeps: DEFAULT_SUBSTEPS,
            iterations: DEFAULT_ITERATIONS,
            sleeping_enabled: true,
            linear_sleep_threshold: DEFAULT_LINEAR_SLEEP_THRESHOLD,
            angular_sleep_threshold: DEFAULT_ANGULAR_SLEEP_THRESHOLD,
            sleep_frames: DEFAULT_SLEEP_FRAMES,
            last_narrowphase_tests: 0,
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
        // The slot stays allocated; tombstone the body so every solver phase
        // skips it (and it can never be woken). TODO: free list to reuse slots.
        if let Some(body) = self.bodies.get_mut(handle.index() as usize) {
            body.removed = true;
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

    /// Number of dynamic bodies currently awake (being integrated). Drops to
    /// zero once everything has settled and gone to sleep.
    pub fn awake_body_count(&self) -> usize {
        self.bodies
            .iter()
            .filter(|b| b.is_dynamic() && !b.sleeping)
            .count()
    }

    /// Narrowphase candidate pairs tested during the most recent [`step`]. Near
    /// zero when the scene is asleep — a deterministic proxy for solver cost.
    pub fn last_narrowphase_tests(&self) -> usize {
        self.last_narrowphase_tests
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

    /// Add a point-to-point (ball) joint pinning `local_anchor_a` on body `a` to
    /// `local_anchor_b` on body `b`, leaving all relative rotation free.
    pub fn add_ball_joint(
        &mut self,
        a: BodyHandle,
        b: BodyHandle,
        local_anchor_a: Vec3,
        local_anchor_b: Vec3,
        compliance: f32,
    ) {
        if let (Some(ia), Some(ib)) = (self.index_of(a), self.index_of(b)) {
            self.joints.push(Joint::Ball(BallJoint::new(
                ia,
                ib,
                local_anchor_a,
                local_anchor_b,
                compliance,
            )));
        }
    }

    /// Add a hinge (revolute) joint: coincident anchors, the per-body hinge axes
    /// kept parallel, and an optional `(min, max)` swing-angle limit in radians
    /// measured relative to the current pose.
    #[allow(clippy::too_many_arguments)]
    pub fn add_hinge_joint(
        &mut self,
        a: BodyHandle,
        b: BodyHandle,
        local_anchor_a: Vec3,
        local_anchor_b: Vec3,
        axis_a: Vec3,
        axis_b: Vec3,
        limits: Option<(f32, f32)>,
        compliance: f32,
    ) {
        if let (Some(ia), Some(ib)) = (self.index_of(a), self.index_of(b)) {
            self.joints.push(Joint::Hinge(HingeJoint::new(
                &self.bodies,
                ia,
                ib,
                local_anchor_a,
                local_anchor_b,
                axis_a,
                axis_b,
                limits,
                compliance,
            )));
        }
    }

    /// Add a prismatic (sliding) joint along `axis_a` (in body `a`'s frame):
    /// relative rotation and perpendicular translation are locked, with an
    /// optional `(min, max)` travel limit in metres along the axis.
    #[allow(clippy::too_many_arguments)]
    pub fn add_prismatic_joint(
        &mut self,
        a: BodyHandle,
        b: BodyHandle,
        local_anchor_a: Vec3,
        local_anchor_b: Vec3,
        axis_a: Vec3,
        limits: Option<(f32, f32)>,
        compliance: f32,
    ) {
        if let (Some(ia), Some(ib)) = (self.index_of(a), self.index_of(b)) {
            self.joints.push(Joint::Prismatic(PrismaticJoint::new(
                &self.bodies,
                ia,
                ib,
                local_anchor_a,
                local_anchor_b,
                axis_a,
                limits,
                compliance,
            )));
        }
    }

    /// Add a fixed (weld) joint locking both the relative position and the
    /// relative orientation of the two bodies at their current offset.
    pub fn add_fixed_joint(
        &mut self,
        a: BodyHandle,
        b: BodyHandle,
        local_anchor_a: Vec3,
        local_anchor_b: Vec3,
        compliance: f32,
    ) {
        if let (Some(ia), Some(ib)) = (self.index_of(a), self.index_of(b)) {
            self.joints.push(Joint::Fixed(FixedJoint::new(
                &self.bodies,
                ia,
                ib,
                local_anchor_a,
                local_anchor_b,
                compliance,
            )));
        }
    }

    /// Advance the simulation by `frame_dt` seconds via the XPBD substep loop.
    pub fn step(&mut self, frame_dt: f32) {
        let substeps = self.substeps.max(1);
        let dt = frame_dt / substeps as f32;
        self.last_narrowphase_tests = 0;
        let mut island_pairs = Vec::new();
        for _ in 0..substeps {
            island_pairs = self.substep(dt);
        }
        if self.sleeping_enabled {
            self.update_sleeping(&island_pairs);
        }
    }

    /// One XPBD substep: predict, generate contacts, project, derive velocity,
    /// apply restitution + dynamic friction. Returns the dynamic-body contact
    /// pairs (for sleeping-island grouping).
    fn substep(&mut self, dt: f32) -> Vec<(usize, usize)> {
        let gravity = self.gravity;

        // 1. Predict positions (and integrate free rotation).
        for body in &mut self.bodies {
            body.prev_position = body.position;
            body.prev_rotation = body.rotation;
            if body.removed || body.kind != BodyKind::Dynamic || body.inv_mass == 0.0 || body.sleeping {
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
        let GenContacts { mut contacts, island_pairs, wake, tests } = self.generate_contacts();
        self.last_narrowphase_tests += tests;
        // Wake any sleeping body that an awake body has run into.
        for i in wake {
            self.bodies[i].sleeping = false;
            self.bodies[i].low_energy_frames = 0;
        }

        // 3. Project all constraints for several iterations.
        for c in &mut self.distance_constraints {
            c.reset();
        }
        for j in &mut self.joints {
            j.reset();
        }
        for c in &mut contacts {
            c.reset();
        }
        for _ in 0..self.iterations.max(1) {
            for c in &mut self.distance_constraints {
                c.project(&mut self.bodies, dt);
            }
            for j in &mut self.joints {
                j.project(&mut self.bodies, dt);
            }
            for c in &mut contacts {
                c.project(&mut self.bodies, dt);
            }
        }

        // 4. Derive velocities from the position and orientation change.
        for body in &mut self.bodies {
            if body.removed || body.kind != BodyKind::Dynamic || body.inv_mass == 0.0 || body.sleeping {
                continue;
            }
            body.linear_velocity = (body.position - body.prev_position) / dt;
            // Angular velocity from the quaternion delta (shortest arc).
            let dq = body.rotation * body.prev_rotation.inverse();
            let dq = if dq.w < 0.0 { -dq } else { dq };
            body.angular_velocity = Vec3::new(dq.x, dq.y, dq.z) * (2.0 / dt);
        }

        // 5. Velocity-level restitution and dynamic friction.
        for c in &contacts {
            c.apply_restitution(&mut self.bodies);
            c.apply_dynamic_friction(&mut self.bodies, dt);
        }

        island_pairs
    }

    /// Build contact constraints for this substep. Finite-AABB bodies go through
    /// the BVH; unbounded half-spaces are tested against every finite body. When
    /// the whole scene is asleep this short-circuits to no work at all.
    fn generate_contacts(&self) -> GenContacts {
        // Nothing moving → no contacts to generate, and (crucially) no broadphase
        // or narrowphase cost. This is what makes a settled scene nearly free.
        let any_awake = self.bodies.iter().any(|b| b.is_dynamic() && !b.sleeping);
        if !any_awake {
            return GenContacts::default();
        }

        let mut finite_idx = Vec::new();
        let mut finite_aabbs = Vec::new();
        let mut unbounded = Vec::new();
        for (i, body) in self.bodies.iter().enumerate() {
            if body.removed {
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

        let mut out = GenContacts::default();
        let bvh = Bvh::build(&finite_aabbs);
        for (la, lb) in bvh.query_pairs() {
            self.consider_pair(finite_idx[la], finite_idx[lb], &mut out);
        }
        for &u in &unbounded {
            for &f in &finite_idx {
                self.consider_pair(u, f, &mut out);
            }
        }
        out
    }

    /// Narrowphase one broadphase candidate pair, folding the result into `out`:
    /// the contact constraint, the dynamic-pair link (for islands), and any
    /// sleeping body that should be woken by the contact.
    fn consider_pair(&self, i: usize, j: usize, out: &mut GenContacts) {
        if i == j {
            return;
        }
        // Two sleeping bodies can't disturb each other — skip the narrowphase.
        if self.bodies[i].sleeping && self.bodies[j].sleeping {
            return;
        }
        out.tests += 1;
        if let Some(c) = self.make_contact(i, j) {
            let (lo, hi) = (c.body_a, c.body_b);
            // Contact with a non-sleeping body wakes a sleeper.
            if self.bodies[lo].sleeping && !self.bodies[hi].sleeping {
                out.wake.push(lo);
            } else if self.bodies[hi].sleeping && !self.bodies[lo].sleeping {
                out.wake.push(hi);
            }
            if self.bodies[lo].is_dynamic() && self.bodies[hi].is_dynamic() {
                out.island_pairs.push((lo, hi));
            }
            out.contacts.push(c);
        }
    }

    /// Narrowphase a candidate pair into a contact constraint, ordered so the
    /// constraint's normal points from the lower-indexed body to the higher.
    fn make_contact(&self, i: usize, j: usize) -> Option<ContactConstraint> {
        let (lo, hi) = if i < j { (i, j) } else { (j, i) };
        let a = &self.bodies[lo];
        let b = &self.bodies[hi];
        if a.inv_mass == 0.0 && b.inv_mass == 0.0 {
            return None; // two static bodies never collide
        }
        let manifold = world_collide(a.collider, pose_of(a), b.collider, pose_of(b))?;
        let combined = a.material.combine(&b.material);
        Some(ContactConstraint::new(
            lo,
            hi,
            manifold.normal,
            manifold.depth,
            combined.restitution,
            combined.static_friction,
            combined.dynamic_friction,
            0.0, // rigid contacts
            &self.bodies,
        ))
    }

    /// Per-frame sleeping update. Bodies that have been quiet for
    /// `sleep_frames` frames sleep — but only as whole **islands**: a body sleeps
    /// only when every dynamic body it is connected to (via contacts, joints, or
    /// distance constraints) is also ready, so a stack never half-sleeps and a
    /// disturbance to any member wakes them all together.
    fn update_sleeping(&mut self, contact_pairs: &[(usize, usize)]) {
        let n = self.bodies.len();
        let lin_thresh_sq = self.linear_sleep_threshold * self.linear_sleep_threshold;
        let ang_thresh_sq = self.angular_sleep_threshold * self.angular_sleep_threshold;

        // 1. Update each dynamic body's quiet-frame counter.
        for body in &mut self.bodies {
            if !body.is_dynamic() {
                continue;
            }
            if body.sleeping {
                body.low_energy_frames = body.low_energy_frames.saturating_add(1);
                continue;
            }
            let quiet = body.linear_velocity.length_squared() < lin_thresh_sq
                && body.angular_velocity.length_squared() < ang_thresh_sq;
            if quiet {
                body.low_energy_frames = body.low_energy_frames.saturating_add(1);
            } else {
                body.low_energy_frames = 0;
            }
        }

        // 2. Union dynamic bodies into islands across all couplings (contacts,
        //    joints, distance constraints).
        let mut pairs: Vec<(usize, usize)> = contact_pairs.to_vec();
        pairs.extend(self.joints.iter().map(|j| j.bodies()));
        pairs.extend(self.distance_constraints.iter().map(|c| (c.body_a, c.body_b)));
        let mut uf = UnionFind::new(n);
        for (a, b) in pairs {
            if self.bodies[a].is_dynamic() && self.bodies[b].is_dynamic() {
                uf.union(a, b);
            }
        }

        // 3. An island is ready only if its least-rested member has waited long.
        let mut min_frames = vec![u32::MAX; n];
        for i in 0..n {
            if self.bodies[i].is_dynamic() {
                let r = uf.find(i);
                min_frames[r] = min_frames[r].min(self.bodies[i].low_energy_frames);
            }
        }

        // 4. Apply: sleep ready islands, wake any island with a restless member.
        for i in 0..n {
            if !self.bodies[i].is_dynamic() {
                continue;
            }
            let ready = min_frames[uf.find(i)] >= self.sleep_frames;
            if ready && !self.bodies[i].sleeping {
                self.bodies[i].sleeping = true;
                self.bodies[i].linear_velocity = Vec3::ZERO;
                self.bodies[i].angular_velocity = Vec3::ZERO;
            } else if !ready && self.bodies[i].sleeping {
                self.bodies[i].sleeping = false;
            }
        }
    }
}

/// Output of [`PhysicsWorld::generate_contacts`].
#[derive(Default)]
struct GenContacts {
    contacts: Vec<ContactConstraint>,
    /// Dynamic↔dynamic contact links, for sleeping islands.
    island_pairs: Vec<(usize, usize)>,
    /// Sleeping bodies that a contact with an awake body should wake.
    wake: Vec<usize>,
    /// Narrowphase tests performed (cost metric).
    tests: usize,
}

/// Minimal union-find for grouping bodies into sleeping islands.
struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect() }
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]]; // path halving
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra] = rb;
        }
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
        Collider::Capsule { radius, half_height } => {
            Some(AnyShape::Capsule(Capsule { radius, half_height }))
        }
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

    #[test]
    fn lone_quiet_body_eventually_sleeps() {
        // A free body at rest with no gravity should fall asleep after the
        // configured number of quiet frames, then stop costing narrowphase work.
        let mut world = PhysicsWorld::new();
        world.gravity = Vec3::ZERO;
        world.sleep_frames = 5;
        let h = world.add_rigid_body(RigidBody::dynamic(
            Vec3::ZERO,
            1.0,
            Collider::Sphere { radius: 0.5 },
        ));
        for _ in 0..10 {
            world.step(1.0 / 60.0);
        }
        assert!(world.body(h).expect("body").sleeping);
        assert_eq!(world.awake_body_count(), 0);
        world.step(1.0 / 60.0);
        assert_eq!(world.last_narrowphase_tests(), 0);
    }
}
