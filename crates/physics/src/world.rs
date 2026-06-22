//! PhysicsWorld — owns the bodies, the constraints, and the XPBD substep loop.

use elderforge_core::math::{Quat, Vec3};

use crate::body::{BodyHandle, BodyKind, Collider, RigidBody};
use crate::broadphase::{Aabb, Bvh};
use crate::debug::{DebugDraw, DebugLayers};
use crate::narrowphase::{collide, surface_support, AnyShape, ContactManifold, Pose};
use crate::shapes::{BoxShape, Capsule, Sphere};
use crate::soft::{Cloth, ClothDef, ClothHandle, Particle, SoftBody, SoftBodyDef, SoftBodyHandle};
use crate::solver::{
    BallJoint, Constraint, ContactConstraint, DistanceConstraint, FixedJoint, HingeJoint, Joint,
    ParticleBodyContact, ParticleDistance, ParticleVolume, PrismaticJoint,
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

/// Default per-second velocity damping applied to soft-body / cloth particles so
/// they settle instead of ringing forever. Light enough to leave a flag waving.
pub const DEFAULT_PARTICLE_DAMPING: f32 = 0.5;

pub struct PhysicsWorld {
    bodies: Vec<RigidBody>,
    generations: Vec<u32>,
    distance_constraints: Vec<DistanceConstraint>,
    joints: Vec<Joint>,
    /// Flat array of every soft-body and cloth particle. Soft bodies and cloths
    /// own contiguous runs of it (see [`SoftBody::base`]/[`Cloth::base`]).
    particles: Vec<Particle>,
    /// Distance constraints over [`particles`](Self::particles): soft-body tet
    /// edges and cloth structural/shear/bending springs.
    particle_distance: Vec<ParticleDistance>,
    /// Tetrahedral volume-preservation constraints over the particles.
    particle_volume: Vec<ParticleVolume>,
    /// Soft bodies resident in the world (metadata; their DOFs live in
    /// `particles`).
    soft_bodies: Vec<SoftBody>,
    /// Cloths resident in the world.
    cloths: Vec<Cloth>,
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
    /// Per-second velocity damping for particles. See [`DEFAULT_PARTICLE_DAMPING`].
    pub particle_damping: f32,
    /// Steady wind acceleration applied to soft-body / cloth particles (on top
    /// of gravity), in m/s². A crude uniform aerodynamic push — enough to make a
    /// flag billow. Zero by default; rigid bodies ignore it.
    pub wind: Vec3,
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
            particles: Vec::new(),
            particle_distance: Vec::new(),
            particle_volume: Vec::new(),
            soft_bodies: Vec::new(),
            cloths: Vec::new(),
            gravity: Vec3::new(0.0, -9.81, 0.0),
            substeps: DEFAULT_SUBSTEPS,
            iterations: DEFAULT_ITERATIONS,
            sleeping_enabled: true,
            linear_sleep_threshold: DEFAULT_LINEAR_SLEEP_THRESHOLD,
            angular_sleep_threshold: DEFAULT_ANGULAR_SLEEP_THRESHOLD,
            sleep_frames: DEFAULT_SLEEP_FRAMES,
            particle_damping: DEFAULT_PARTICLE_DAMPING,
            wind: Vec3::ZERO,
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

    /// Read-only view of every body slot in handle-index order, for
    /// serialization. Includes tombstoned (`removed`) slots so a serialized
    /// handle's index still lines up with its body on reload.
    pub fn bodies(&self) -> &[RigidBody] {
        &self.bodies
    }

    /// Slot generation for each body, parallel to [`bodies`](Self::bodies).
    /// A handle is `BodyHandle::new(index, generations()[index])`.
    pub fn generations(&self) -> &[u32] {
        &self.generations
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

    /// Add a soft body from a [`SoftBodyDef`]: its particles are appended to the
    /// world's particle array, and its edge (distance) and tet (volume)
    /// constraints are stored with indices offset into that array. Returns a
    /// handle to the resident [`SoftBody`] (its surface mesh, for rendering).
    pub fn add_soft_body(&mut self, def: &SoftBodyDef) -> SoftBodyHandle {
        let base = self.particles.len();
        for (i, &pos) in def.particles.iter().enumerate() {
            self.particles
                .push(Particle::new(pos, def.inv_masses[i], def.particle_radius));
        }
        for &(a, b, rest) in &def.edges {
            self.particle_distance.push(ParticleDistance::new(
                base + a as usize,
                base + b as usize,
                rest,
                def.distance_compliance,
            ));
        }
        for &(idx, rest_volume) in &def.tets {
            self.particle_volume.push(ParticleVolume::new(
                [
                    base + idx[0] as usize,
                    base + idx[1] as usize,
                    base + idx[2] as usize,
                    base + idx[3] as usize,
                ],
                rest_volume,
                def.volume_compliance,
            ));
        }
        let handle = SoftBodyHandle(self.soft_bodies.len());
        self.soft_bodies.push(SoftBody {
            base,
            count: def.particles.len(),
            surface: def.surface.clone(),
        });
        handle
    }

    /// Add a cloth from a [`ClothDef`]: its grid of particles is appended and
    /// the structural / shear / bending distance constraints stored (each family
    /// with its own compliance). Returns a handle to the resident [`Cloth`].
    pub fn add_cloth(&mut self, def: &ClothDef) -> ClothHandle {
        let base = self.particles.len();
        for (i, &pos) in def.particles.iter().enumerate() {
            self.particles
                .push(Particle::new(pos, def.inv_masses[i], def.particle_radius));
        }
        let mut push_group = |group: &[(u32, u32, f32)], compliance: f32| {
            for &(a, b, rest) in group {
                self.particle_distance.push(ParticleDistance::new(
                    base + a as usize,
                    base + b as usize,
                    rest,
                    compliance,
                ));
            }
        };
        push_group(&def.structural, def.structural_compliance);
        push_group(&def.shear, def.shear_compliance);
        push_group(&def.bending, def.bending_compliance);
        let handle = ClothHandle(self.cloths.len());
        self.cloths.push(Cloth { base, cols: def.cols, rows: def.rows });
        handle
    }

    /// All particles, in world order. Soft bodies and cloths index contiguous
    /// runs of this via their `base`/`count`.
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// Mutable view of all particles, for animation that pokes the cloud
    /// directly — e.g. a staged drop that pins a soft body (zero inverse mass)
    /// until its release time, then restores its masses so gravity takes it.
    pub fn particles_mut(&mut self) -> &mut [Particle] {
        &mut self.particles
    }

    /// Total particle count across all soft bodies and cloths.
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    /// Resident soft bodies (metadata + surface topology).
    pub fn soft_bodies(&self) -> &[SoftBody] {
        &self.soft_bodies
    }

    /// Resident cloths.
    pub fn cloths(&self) -> &[Cloth] {
        &self.cloths
    }

    /// A soft body by handle, or `None` for a stale handle.
    pub fn soft_body(&self, handle: SoftBodyHandle) -> Option<&SoftBody> {
        self.soft_bodies.get(handle.0)
    }

    /// A cloth by handle, or `None` for a stale handle.
    pub fn cloth(&self, handle: ClothHandle) -> Option<&Cloth> {
        self.cloths.get(handle.0)
    }

    /// The particle slice owned by a soft body, or `None` for a stale handle.
    pub fn soft_body_particles(&self, handle: SoftBodyHandle) -> Option<&[Particle]> {
        let sb = self.soft_bodies.get(handle.0)?;
        Some(&self.particles[sb.base..sb.base + sb.count])
    }

    /// The particle slice owned by a cloth, or `None` for a stale handle.
    pub fn cloth_particles(&self, handle: ClothHandle) -> Option<&[Particle]> {
        let c = self.cloths.get(handle.0)?;
        Some(&self.particles[c.base..c.base + c.particle_count()])
    }

    /// Emit this frame's debug-overlay geometry for the enabled `layers` into
    /// `out` (cleared first), reusing its buffers. Only enabled layers are
    /// computed, so an all-off [`DebugLayers`] returns almost immediately.
    ///
    /// The contact and BVH layers recompute their own broadphase/narrowphase
    /// from the current state rather than reading the solver's per-substep
    /// scratch (which isn't retained), so the overlay is correct even while the
    /// scene is asleep. This is debug-only work, gated on the layer being on.
    pub fn emit_debug(&self, layers: DebugLayers, out: &mut DebugDraw) {
        out.clear();
        if !layers.any() {
            return;
        }

        // --- Per-body overlays. ---
        for body in &self.bodies {
            if body.removed {
                continue;
            }
            if layers.collision_shapes {
                self.emit_collider_wire(out, body, kind_color(body.kind));
            }
            if !body.is_dynamic() {
                continue;
            }
            // Sleep state only means something for dynamic bodies.
            if layers.sleep_state {
                let color = if body.sleeping { SLEEP_ASLEEP } else { SLEEP_AWAKE };
                self.emit_collider_wire(out, body, color);
            }
            if layers.velocity_vectors {
                let v = body.linear_velocity;
                if v.length_squared() > VELOCITY_MIN_SQ {
                    out.arrow(body.position, body.position + v * VELOCITY_TIME_SCALE, VELOCITY_COLOR);
                }
            }
            if layers.angular_velocity {
                let w = body.angular_velocity;
                let speed = w.length();
                if speed > ANGULAR_MIN {
                    let radius = body_extent(body).max(0.25);
                    let sweep = (speed * ANGULAR_TIME_SCALE)
                        .clamp(0.2, 1.75 * std::f32::consts::PI);
                    out.arc(body.position, w, radius, sweep, ANGULAR_COLOR);
                }
            }
            if layers.force_accumulators && !body.sleeping {
                // The only persistent external force in the solver is gravity, so
                // the net accumulated force on a free body is m·g.
                let force = self.gravity * body.mass;
                if force.length_squared() > 1e-6 {
                    out.arrow(body.position, body.position + force * FORCE_SCALE, FORCE_COLOR);
                }
            }
        }

        // --- Particle (soft-body / cloth) velocity arrows. ---
        // Particles are a separate DOF type from rigid bodies, so the velocity
        // layer covers them with their own pass: an arrow per moving particle.
        if layers.velocity_vectors {
            for p in &self.particles {
                if p.inv_mass == 0.0 {
                    continue; // pinned particle (a held corner) — no motion to show
                }
                let v = p.velocity;
                if v.length_squared() > VELOCITY_MIN_SQ {
                    out.arrow(p.position, p.position + v * VELOCITY_TIME_SCALE, VELOCITY_COLOR);
                }
            }
        }

        // --- Constraint anchors + connections. ---
        if layers.constraint_anchors {
            for c in &self.distance_constraints {
                if c.body_a >= self.bodies.len() || c.body_b >= self.bodies.len() {
                    continue;
                }
                let pa = self.bodies[c.body_a].position;
                let pb = self.bodies[c.body_b].position;
                out.line(pa, pb, CONSTRAINT_LINK);
                out.marker(pa, ANCHOR_SIZE, ANCHOR_COLOR);
                out.marker(pb, ANCHOR_SIZE, ANCHOR_COLOR);
            }
            for j in &self.joints {
                let (a, b) = j.bodies();
                if a >= self.bodies.len() || b >= self.bodies.len() {
                    continue;
                }
                let (pa, pb) = j.world_anchors(&self.bodies);
                out.line(pa, pb, CONSTRAINT_LINK);
                out.marker(pa, ANCHOR_SIZE, ANCHOR_COLOR);
                out.marker(pb, ANCHOR_SIZE, ANCHOR_COLOR);
            }
            // Particle constraints: cloth structural/shear/bending springs and
            // soft-body tet edges. Drawing every spring as a line renders the
            // cloth/soft mesh as a glowing wireframe; a point marks each particle
            // anchor. (Per-particle cube markers would be far too heavy for a
            // thousand-particle sheet, so anchors are bare points here.)
            for c in &self.particle_distance {
                let pa = self.particles[c.a].position;
                let pb = self.particles[c.b].position;
                out.line(pa, pb, PARTICLE_SPRING);
            }
            for p in &self.particles {
                out.point(p.position, ANCHOR_COLOR);
            }
        }

        // --- BVH + contacts: both ride one broadphase build over finite bodies. ---
        if layers.bvh_aabbs || layers.contact_points {
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
            let bvh = Bvh::build(&finite_aabbs);

            if layers.bvh_aabbs {
                let levels = bvh.debug_iter_levels();
                let max_depth = levels.iter().map(|(_, d)| *d).max().unwrap_or(0);
                for (aabb, depth) in levels {
                    out.wire_aabb(aabb.min, aabb.max, depth_color(depth, max_depth));
                }
            }

            if layers.contact_points {
                for (la, lb) in bvh.query_pairs() {
                    self.emit_contact(out, finite_idx[la], finite_idx[lb]);
                }
                for &u in &unbounded {
                    for &f in &finite_idx {
                        self.emit_contact(out, u, f);
                    }
                }
            }
        }
    }

    /// Narrowphase one body pair and, if they touch, emit a contact-point marker
    /// and a normal arrow.
    fn emit_contact(&self, out: &mut DebugDraw, i: usize, j: usize) {
        if let Some(m) = self.debug_manifold(i, j) {
            out.wire_sphere(m.contact_point, CONTACT_MARKER_RADIUS, CONTACT_COLOR);
            out.point(m.contact_point, CONTACT_COLOR);
            out.arrow(
                m.contact_point,
                m.contact_point + m.normal * CONTACT_NORMAL_LEN,
                CONTACT_NORMAL_COLOR,
            );
        }
    }

    /// Contact manifold for a body pair at the current state, for debug
    /// visualization. Unlike [`make_contact`](Self::make_contact) it keeps the
    /// manifold's contact point and ignores the sleeping short-circuit.
    fn debug_manifold(&self, i: usize, j: usize) -> Option<ContactManifold> {
        if i == j {
            return None;
        }
        let (lo, hi) = if i < j { (i, j) } else { (j, i) };
        let a = &self.bodies[lo];
        let b = &self.bodies[hi];
        if a.removed || b.removed || (a.inv_mass == 0.0 && b.inv_mass == 0.0) {
            return None;
        }
        world_collide(a.collider, pose_of(a), b.collider, pose_of(b))
    }

    /// Emit a body's collider as a wireframe in `color`.
    fn emit_collider_wire(&self, out: &mut DebugDraw, body: &RigidBody, color: [f32; 4]) {
        match body.collider {
            Collider::Sphere { radius } => out.wire_sphere(body.position, radius, color),
            Collider::Box { half_extents } => {
                out.wire_box(body.position, half_extents, body.rotation, color)
            }
            Collider::Capsule { radius, half_height } => {
                out.wire_capsule(body.position, radius, half_height, body.rotation, color)
            }
            Collider::HalfSpace { normal, offset } => {
                emit_halfspace(out, normal, offset, color)
            }
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
        let particle_accel = self.gravity + self.wind;

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

        // 1b. Predict particle positions (soft bodies + cloth). Pinned
        //     particles (zero inverse mass) stay put.
        for p in &mut self.particles {
            p.prev_position = p.position;
            if p.inv_mass == 0.0 {
                continue;
            }
            p.position += p.velocity * dt + particle_accel * (dt * dt);
        }

        // 2. Broadphase + narrowphase -> contact constraints (at predicted state).
        let GenContacts { mut contacts, island_pairs, wake, tests } = self.generate_contacts();
        self.last_narrowphase_tests += tests;
        // Wake any sleeping body that an awake body has run into.
        for i in wake {
            self.bodies[i].sleeping = false;
            self.bodies[i].low_energy_frames = 0;
        }

        // 2b. Particle ↔ rigid contacts at the predicted state (none when the
        //     scene has no particles).
        let mut particle_contacts = self.generate_particle_contacts();

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
        for c in &mut self.particle_distance {
            c.reset();
        }
        for c in &mut self.particle_volume {
            c.reset();
        }
        for c in &mut particle_contacts {
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
            // Soft-body / cloth internal constraints, then their rigid contacts.
            for c in &mut self.particle_distance {
                c.project(&mut self.particles, dt);
            }
            for c in &mut self.particle_volume {
                c.project(&mut self.particles, dt);
            }
            for c in &mut particle_contacts {
                c.project(&mut self.particles, &mut self.bodies, dt);
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

        // 4b. Derive particle velocities from the position change, then apply
        //     viscous damping so cloth/soft bodies settle.
        let damp = (1.0 - self.particle_damping * dt).clamp(0.0, 1.0);
        for p in &mut self.particles {
            if p.inv_mass == 0.0 {
                p.velocity = Vec3::ZERO;
                continue;
            }
            p.velocity = (p.position - p.prev_position) / dt * damp;
        }

        // 5. Velocity-level restitution and dynamic friction.
        for c in &contacts {
            c.apply_restitution(&mut self.bodies);
            c.apply_dynamic_friction(&mut self.bodies, dt);
        }

        island_pairs
    }

    /// Particle ↔ rigid contacts for this substep. Each particle is tested
    /// against every (live) rigid body it overlaps; the rigid count is small in
    /// soft scenes, so this is a straightforward O(particles × bodies) sweep
    /// with an AABB reject rather than a dedicated broadphase.
    fn generate_particle_contacts(&self) -> Vec<ParticleBodyContact> {
        let mut out = Vec::new();
        if self.particles.is_empty() || self.bodies.is_empty() {
            return out;
        }
        for (pi, p) in self.particles.iter().enumerate() {
            let pbox = Aabb::new(
                p.position - Vec3::splat(p.radius),
                p.position + Vec3::splat(p.radius),
            );
            for (bi, b) in self.bodies.iter().enumerate() {
                if b.removed {
                    continue;
                }
                // Half-spaces report an infinite AABB, so they always pass the
                // reject and fall through to the exact test.
                if !b.collider.aabb(b.position).overlaps(&pbox) {
                    continue;
                }
                if let Some(c) = ParticleBodyContact::generate(pi, p, bi, b) {
                    out.push(c);
                }
            }
        }
        out
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

// --- Debug overlay palette and scales (see `emit_debug`). ---
const VELOCITY_COLOR: [f32; 4] = [1.0, 0.95, 0.2, 1.0];
const ANGULAR_COLOR: [f32; 4] = [1.0, 0.3, 0.9, 1.0];
const FORCE_COLOR: [f32; 4] = [1.0, 0.5, 0.15, 1.0];
const CONTACT_COLOR: [f32; 4] = [1.0, 0.55, 0.1, 1.0];
const CONTACT_NORMAL_COLOR: [f32; 4] = [1.0, 0.2, 0.2, 1.0];
const ANCHOR_COLOR: [f32; 4] = [0.3, 0.95, 0.95, 1.0];
const CONSTRAINT_LINK: [f32; 4] = [0.85, 0.85, 0.9, 1.0];
/// Cloth / soft-body spring color — a bright translucent cyan so the dense web
/// of particle constraints reads as a glowing wireframe.
const PARTICLE_SPRING: [f32; 4] = [0.35, 0.85, 1.0, 0.7];
const SLEEP_AWAKE: [f32; 4] = [0.3, 1.0, 0.4, 1.0];
const SLEEP_ASLEEP: [f32; 4] = [0.5, 0.5, 0.55, 0.35];

/// Seconds of travel an arrow represents, so its length scales with speed.
const VELOCITY_TIME_SCALE: f32 = 0.15;
/// Below this speed (m/s) a velocity arrow is omitted (avoids jitter noise).
const VELOCITY_MIN_SQ: f32 = 0.05 * 0.05;
/// Below this angular speed (rad/s) the spin arc is omitted.
const ANGULAR_MIN: f32 = 0.1;
/// Radians of arc per (rad/s) of spin, before clamping.
const ANGULAR_TIME_SCALE: f32 = 0.25;
/// Metres of arrow per newton of force.
const FORCE_SCALE: f32 = 0.03;
/// Contact-point marker sphere radius.
const CONTACT_MARKER_RADIUS: f32 = 0.06;
/// Length of a contact-normal arrow.
const CONTACT_NORMAL_LEN: f32 = 0.5;
/// Half-size of a constraint-anchor cube marker.
const ANCHOR_SIZE: f32 = 0.07;
/// Half-extent of the grid patch drawn for an (infinite) half-space.
const HALFSPACE_GRID_HALF: f32 = 10.0;
/// Grid divisions across a half-space patch.
const HALFSPACE_GRID_LINES: usize = 8;

/// Collider wireframe color by body kind.
fn kind_color(kind: BodyKind) -> [f32; 4] {
    match kind {
        BodyKind::Dynamic => [0.2, 1.0, 0.35, 1.0],
        BodyKind::Static => [0.45, 0.55, 0.75, 1.0],
        BodyKind::Kinematic => [0.2, 0.9, 0.95, 1.0],
    }
}

/// A BVH node's color by depth: a red (root) → green → blue (deep) ramp, at low
/// alpha so nested boxes stay legible.
fn depth_color(depth: usize, max_depth: usize) -> [f32; 4] {
    let t = if max_depth == 0 {
        0.0
    } else {
        depth as f32 / max_depth as f32
    };
    // Two-segment lerp: red→green over [0,0.5], green→blue over [0.5,1].
    let (r, g, b) = if t < 0.5 {
        let k = t * 2.0;
        (1.0 - 0.8 * k, 0.2 + 0.8 * k, 0.2)
    } else {
        let k = (t - 0.5) * 2.0;
        (0.2, 1.0 - 0.7 * k, 0.2 + 0.8 * k)
    };
    [r, g, b, 0.55]
}

/// Rough bounding radius of a collider, for sizing angular-velocity arcs.
fn body_extent(body: &RigidBody) -> f32 {
    match body.collider {
        Collider::Sphere { radius } => radius,
        Collider::Box { half_extents } => half_extents.length(),
        Collider::Capsule { radius, half_height } => half_height + radius,
        Collider::HalfSpace { .. } => 0.5,
    }
}

/// Draw a finite grid patch (plus a normal arrow) standing in for an infinite
/// half-space, centered on the foot of the plane normal from the origin.
fn emit_halfspace(out: &mut DebugDraw, normal: Vec3, offset: f32, color: [f32; 4]) {
    let n = normal.normalize_or_zero();
    if n == Vec3::ZERO {
        return;
    }
    let center = n * offset;
    // In-plane basis: pick the axis least aligned with the normal.
    let seed = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let u = (seed - n * seed.dot(n)).normalize();
    let v = n.cross(u);
    let half = HALFSPACE_GRID_HALF;
    let lines = HALFSPACE_GRID_LINES;
    for k in 0..=lines {
        let t = -half + 2.0 * half * k as f32 / lines as f32;
        out.line(center + u * t - v * half, center + u * t + v * half, color);
        out.line(center + v * t - u * half, center + v * t + u * half, color);
    }
    out.arrow(center, center + n, color);
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
