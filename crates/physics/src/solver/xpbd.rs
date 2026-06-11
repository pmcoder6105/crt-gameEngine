//! XPBD (Extended Position-Based Dynamics) constraint solver — the main
//! integration loop. Runs `substeps` position-based substeps per fixed step.

use elderforge_core::math::Vec3;

use crate::body::{BodyKind, RigidBody};

#[derive(Debug, Clone)]
pub struct XpbdSolver {
    pub substeps: u32,
    /// Constraint solver iterations per substep.
    pub iterations: u32,
}

impl Default for XpbdSolver {
    fn default() -> Self {
        Self {
            substeps: 4,
            iterations: 1,
        }
    }
}

impl XpbdSolver {
    pub fn step(&mut self, bodies: &mut [RigidBody], gravity: Vec3, dt: f32) {
        let substeps = self.substeps.max(1);
        let sub_dt = dt / substeps as f32;
        for _ in 0..substeps {
            for body in bodies.iter_mut() {
                if body.kind != BodyKind::Dynamic || body.sleeping || body.inv_mass == 0.0 {
                    continue;
                }
                body.linear_velocity += gravity * sub_dt;
                body.position += body.linear_velocity * sub_dt;
                // TODO: predict rotations, solve position constraints
                // (contacts/joints), then derive velocities from position
                // deltas — the actual XPBD loop.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrates_gravity_on_dynamic_bodies() {
        let mut solver = XpbdSolver::default();
        let mut bodies = vec![RigidBody::default()];
        solver.step(&mut bodies, Vec3::new(0.0, -9.81, 0.0), 1.0 / 120.0);
        assert!(bodies[0].position.y < 0.0);
    }

    #[test]
    fn skips_static_and_sleeping_bodies() {
        let mut solver = XpbdSolver::default();
        let mut bodies = vec![
            RigidBody {
                kind: BodyKind::Static,
                ..RigidBody::default()
            },
            RigidBody {
                sleeping: true,
                ..RigidBody::default()
            },
        ];
        solver.step(&mut bodies, Vec3::new(0.0, -9.81, 0.0), 1.0 / 120.0);
        assert_eq!(bodies[0].position, Vec3::ZERO);
        assert_eq!(bodies[1].position, Vec3::ZERO);
    }
}
