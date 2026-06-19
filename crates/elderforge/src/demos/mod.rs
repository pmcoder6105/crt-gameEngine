//! Demo scenes that show off the physics solver. Each demo sets up its own
//! camera and spawns its own entities into a fresh [`Scene`]; the binary then
//! runs the normal engine loop over whichever one was selected on the command
//! line (`--demo <name>`).
//!
//! The actual mesh upload happens in the caller (it needs a GPU device); each
//! demo receives the resulting handles in [`DemoAssets`] and decides which it
//! needs and how to place them.

mod avalanche;
mod pendulum;
mod stacking;

use elderforge_core::handles::{MaterialHandle, MeshHandle};
use elderforge_core::math::{Mat4, Vec3};
use elderforge_ecs::components::{Camera, Transform};
use elderforge_physics::PhysicsMaterial;
use elderforge_scene::Scene;

/// Mesh and material handles shared across the demos, uploaded once by the
/// caller. A demo uses whichever subset it needs (the stacking demo ignores
/// `sphere`, the pendulum ignores `cube`, and so on).
#[derive(Debug, Clone, Copy)]
pub struct DemoAssets {
    /// Unit cube (half-extent 0.5, edge length 1.0).
    pub cube: MeshHandle,
    /// Unit-radius UV sphere; scale a body's `Transform` by its radius.
    pub sphere: MeshHandle,
    /// Large flat ground/ramp quad in the XZ plane with a +Y normal.
    pub plane: MeshHandle,
    /// Default surface material for everything in the demos.
    pub material: MaterialHandle,
}

/// Which demo scene to build. Parsed from the `--demo <name>` argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demo {
    /// A tower of boxes settling into a stable stack on the ground.
    Stacking,
    /// A pinned chain of spheres swinging as an XPBD rope.
    Pendulum,
    /// 200 spheres tumbling down a ramp and piling at the bottom.
    Avalanche,
}

impl Demo {
    /// Parse the `--demo` value. Case-insensitive.
    pub fn from_name(name: &str) -> Option<Demo> {
        match name.trim().to_ascii_lowercase().as_str() {
            "stacking" => Some(Demo::Stacking),
            "pendulum" => Some(Demo::Pendulum),
            "avalanche" => Some(Demo::Avalanche),
            _ => None,
        }
    }

    /// The canonical name, matching what [`from_name`](Demo::from_name) accepts.
    pub fn name(self) -> &'static str {
        match self {
            Demo::Stacking => "stacking",
            Demo::Pendulum => "pendulum",
            Demo::Avalanche => "avalanche",
        }
    }

    /// Every demo, for help text and exhaustive testing.
    pub fn all() -> [Demo; 3] {
        [Demo::Stacking, Demo::Pendulum, Demo::Avalanche]
    }

    /// Build this demo's scene: spawn the camera and all entities.
    pub fn setup(self, scene: &mut Scene, assets: &DemoAssets) {
        scene.name = self.name().to_string();
        match self {
            Demo::Stacking => stacking::setup(scene, assets),
            Demo::Pendulum => pendulum::setup(scene, assets),
            Demo::Avalanche => avalanche::setup(scene, assets),
        }
    }
}

/// Spawn the active camera looking from `eye` toward `target`. The view is
/// baked into a `Transform` (as the render system expects) by inverting the
/// look-at matrix into a world transform.
pub(crate) fn spawn_camera(scene: &mut Scene, eye: Vec3, target: Vec3) {
    let camera_world = Mat4::look_at_rh(eye, target, Vec3::Y).inverse();
    let (_, rotation, position) = camera_world.to_scale_rotation_translation();
    scene.world.spawn((
        Camera::default(),
        Transform { position, rotation, scale: Vec3::ONE },
    ));
}

/// A material with the given restitution (bounciness), otherwise default.
pub(crate) fn material_with_restitution(restitution: f32) -> PhysicsMaterial {
    PhysicsMaterial { restitution, ..PhysicsMaterial::default() }
}

/// Tiny deterministic xorshift64 RNG, so demo layouts are reproducible without
/// pulling in the `rand` crate. (Same generator the original scene used.)
pub(crate) struct Rng {
    state: u64,
}

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        // xorshift requires a non-zero seed.
        Self { state: seed | 1 }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        (x >> 32) as u32
    }

    /// A uniform float in `[lo, hi)`.
    pub(crate) fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let unit = self.next_u32() as f32 / u32::MAX as f32;
        lo + unit * (hi - lo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for demo in Demo::all() {
            assert_eq!(Demo::from_name(demo.name()), Some(demo));
        }
        assert_eq!(Demo::from_name("STACKING"), Some(Demo::Stacking));
        assert_eq!(Demo::from_name("nope"), None);
    }
}
