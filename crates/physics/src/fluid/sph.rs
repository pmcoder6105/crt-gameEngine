//! Smoothed Particle Hydrodynamics.
//! // TODO: GPU-accelerated particle path.

use elderforge_core::math::Vec3;

#[derive(Debug, Clone)]
pub struct SphFluid {
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub smoothing_radius: f32,
    /// kg/m^3.
    pub rest_density: f32,
}

impl SphFluid {
    pub fn new(smoothing_radius: f32, rest_density: f32) -> Self {
        Self {
            positions: Vec::new(),
            velocities: Vec::new(),
            smoothing_radius,
            rest_density,
        }
    }

    pub fn spawn(&mut self, position: Vec3) {
        self.positions.push(position);
        self.velocities.push(Vec3::ZERO);
    }

    pub fn particle_count(&self) -> usize {
        self.positions.len()
    }

    pub fn step(&mut self, _dt: f32) {
        // TODO: neighbor search, density/pressure solve, then integration.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_tracks_particles() {
        let mut fluid = SphFluid::new(0.1, 1000.0);
        fluid.spawn(Vec3::ZERO);
        fluid.spawn(Vec3::ONE);
        assert_eq!(fluid.particle_count(), 2);
        assert_eq!(fluid.positions.len(), fluid.velocities.len());
    }
}
