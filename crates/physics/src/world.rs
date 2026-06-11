//! PhysicsWorld — owns all bodies and the solver.

use elderforge_core::math::Vec3;

use crate::body::{BodyHandle, RigidBody};
use crate::solver::XpbdSolver;
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
        // TODO: reuse freed slots via a free list.
        let index = self.bodies.len() as u32;
        self.bodies.push(body);
        self.generations.push(0);
        BodyHandle { index, generation: 0 }
    }

    /// Invalidates the handle by bumping the slot's generation.
    pub fn remove_rigid_body(&mut self, handle: BodyHandle) -> Result<(), PhysicsError> {
        let generation = self
            .generations
            .get_mut(handle.index as usize)
            .ok_or(PhysicsError::InvalidHandle)?;
        if *generation != handle.generation {
            return Err(PhysicsError::InvalidHandle);
        }
        *generation += 1;
        // The slot stays allocated; park the body so the solver skips it.
        // TODO: free list so removed slots get reused.
        if let Some(body) = self.bodies.get_mut(handle.index as usize) {
            body.sleeping = true;
        }
        log::debug!("removed rigid body {handle:?}");
        Ok(())
    }

    pub fn body(&self, handle: BodyHandle) -> Option<&RigidBody> {
        if *self.generations.get(handle.index as usize)? != handle.generation {
            return None;
        }
        self.bodies.get(handle.index as usize)
    }

    pub fn body_mut(&mut self, handle: BodyHandle) -> Option<&mut RigidBody> {
        if *self.generations.get(handle.index as usize)? != handle.generation {
            return None;
        }
        self.bodies.get_mut(handle.index as usize)
    }

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Advance the simulation by `dt` seconds (fixed timestep, 120 Hz).
    pub fn step(&mut self, dt: f32) {
        // TODO: broadphase -> narrowphase -> contact constraints before solving.
        self.solver.step(&mut self.bodies, self.gravity, dt);
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
}
