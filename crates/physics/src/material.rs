//! Physics material: friction (static + dynamic), restitution, density.

/// Surface and bulk properties of a body, sampled by the contact solver.
///
/// Friction is split into a *static* coefficient (the Coulomb cone that holds a
/// resting contact in place) and a *dynamic* coefficient (the kinetic friction
/// that resists an already-sliding contact). Physically `static >= dynamic`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsMaterial {
    /// Static friction coefficient (μ_s): the tangential force a contact can
    /// resist before it starts to slide, as a fraction of the normal force.
    pub static_friction: f32,
    /// Dynamic (kinetic) friction coefficient (μ_d): the resistance of a
    /// contact that is already sliding. Usually `<= static_friction`.
    pub dynamic_friction: f32,
    /// Bounciness in `[0, 1]`: fraction of approach speed returned on impact.
    pub restitution: f32,
    /// Density in kg/m^3 (used to derive mass from collider volume).
    pub density: f32,
}

/// Friction and restitution coefficients for a *pair* of materials in contact,
/// produced by [`PhysicsMaterial::combine`]. These are what the contact solver
/// actually uses, so the per-body materials stay independent of each other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombinedMaterial {
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub restitution: f32,
}

impl PhysicsMaterial {
    /// Combine two materials into the coefficients used for the contact between
    /// them. Friction is mixed with the geometric mean (the conventional choice
    /// — two slick surfaces stay slick, two grippy surfaces stay grippy), and
    /// restitution takes the max (the bouncier surface dominates).
    pub fn combine(&self, other: &Self) -> CombinedMaterial {
        CombinedMaterial {
            static_friction: (self.static_friction * other.static_friction).max(0.0).sqrt(),
            dynamic_friction: (self.dynamic_friction * other.dynamic_friction).max(0.0).sqrt(),
            restitution: self.restitution.max(other.restitution),
        }
    }
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self {
            static_friction: 0.6,
            dynamic_friction: 0.5,
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
        assert!(material.static_friction >= material.dynamic_friction);
        assert!(material.dynamic_friction >= 0.0);
        assert!((0.0..=1.0).contains(&material.restitution));
        assert!(material.density > 0.0);
    }

    #[test]
    fn combine_uses_geometric_mean_friction_and_max_restitution() {
        let a = PhysicsMaterial {
            static_friction: 0.9,
            dynamic_friction: 0.8,
            restitution: 0.2,
            density: 1000.0,
        };
        let b = PhysicsMaterial {
            static_friction: 0.1,
            dynamic_friction: 0.2,
            restitution: 0.7,
            density: 500.0,
        };
        let c = a.combine(&b);
        assert!((c.static_friction - (0.9f32 * 0.1).sqrt()).abs() < 1e-6);
        assert!((c.dynamic_friction - (0.8f32 * 0.2).sqrt()).abs() < 1e-6);
        assert!((c.restitution - 0.7).abs() < 1e-6);
        // Combining is symmetric.
        assert_eq!(c, b.combine(&a));
    }
}
