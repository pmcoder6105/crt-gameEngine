//! Physics material: friction, restitution, density.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsMaterial {
    pub friction: f32,
    pub restitution: f32,
    /// kg/m^3.
    pub density: f32,
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self {
            friction: 0.5,
            restitution: 0.0,
            density: 1000.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_material_is_sane() {
        let material = PhysicsMaterial::default();
        assert!(material.friction >= 0.0);
        assert!((0.0..=1.0).contains(&material.restitution));
        assert!(material.density > 0.0);
    }
}
