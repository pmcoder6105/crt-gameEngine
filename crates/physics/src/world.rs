//! PhysicsWorld — owns all bodies and the solver.

use elderforge_core::math::Vec3;

use crate::body::{BodyHandle, Collider, RigidBody};
use crate::broadphase::{self, Aabb};
use crate::narrowphase::{self, Contact};
use crate::solver::{impulse, XpbdSolver};
use crate::PhysicsError;

pub struct PhysicsWorld {
    bodies: Vec<RigidBody>,
    generations: Vec<u32>,
    pub gravity: Vec3,
    pub solver: XpbdSolver,
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            generations: Vec::new(),
            gravity: Vec3::new(0.0, -9.81, 0.0),
            solver: XpbdSolver::default(),
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

    /// Advance the simulation by `dt` seconds (fixed timestep, 120 Hz):
    /// integrate, then detect and resolve contacts.
    pub fn step(&mut self, dt: f32) {
        // 1. Integrate velocities and positions (semi-implicit Euler).
        self.solver.step(&mut self.bodies, self.gravity, dt);
        // 2-4. Broadphase pairs -> narrowphase contacts -> impulse resolution.
        self.resolve_collisions();
    }

    /// Naive O(n²) broadphase over body AABBs, sphere/half-space narrowphase on
    /// each candidate pair, and impulse resolution of every contact found.
    fn resolve_collisions(&mut self) {
        let aabbs: Vec<Aabb> = self
            .bodies
            .iter()
            .map(|body| body.collider.aabb(body.position))
            .collect();

        for (i, j) in broadphase::naive_pairs(&aabbs) {
            let (a, b) = (&self.bodies[i], &self.bodies[j]);
            if a.sleeping || b.sleeping {
                continue;
            }
            let Some(contact) = contact_between(a, b) else {
                continue;
            };
            let restitution = impulse::combine_restitution(a, b);
            // `naive_pairs` guarantees i < j, so this split puts `i` in the
            // left half and `j` at the head of the right half.
            let (left, right) = self.bodies.split_at_mut(j);
            impulse::resolve_contact(
                &mut left[i],
                &mut right[0],
                contact.normal,
                contact.penetration,
                restitution,
            );
        }
    }
}

/// Narrowphase dispatch for the minimal collider set, returning a contact whose
/// `normal` points from `a` toward `b` (the convention `resolve_contact`
/// expects). `None` when the pair isn't touching or can't collide.
fn contact_between(a: &RigidBody, b: &RigidBody) -> Option<Contact> {
    match (a.collider, b.collider) {
        (Collider::Sphere { radius: ra }, Collider::Sphere { radius: rb }) => {
            narrowphase::sphere_sphere(a.position, ra, b.position, rb)
        }
        // Sphere is `a`, plane is `b`: the plane normal points toward the
        // sphere, so flip it to point a -> b.
        (Collider::Sphere { radius }, Collider::HalfSpace { normal, offset }) => {
            narrowphase::sphere_halfspace(a.position, radius, normal, offset).map(|c| Contact {
                normal: -c.normal,
                ..c
            })
        }
        // Plane is `a`, sphere is `b`: the plane normal already points a -> b.
        (Collider::HalfSpace { normal, offset }, Collider::Sphere { radius }) => {
            narrowphase::sphere_halfspace(b.position, radius, normal, offset)
        }
        (Collider::HalfSpace { .. }, Collider::HalfSpace { .. }) => None,
    }
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
        world.step(1.0 / 120.0);
        let body = world.body(handle).expect("body exists");
        assert!(body.position.y < 0.0);
        assert!(body.linear_velocity.y < 0.0);
    }

    #[test]
    fn configurable_gravity_direction() {
        let mut world = PhysicsWorld::new();
        world.gravity = Vec3::new(5.0, 0.0, 0.0);
        let handle = world.add_rigid_body(RigidBody::default());
        world.step(1.0 / 120.0);
        let body = world.body(handle).expect("body exists");
        assert!(body.linear_velocity.x > 0.0);
        assert_eq!(body.linear_velocity.y, 0.0);
    }
}
