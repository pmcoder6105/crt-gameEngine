//! Demo scenes that show off the physics solver. Each demo sets up its own
//! camera and spawns its own entities into a fresh [`Scene`]; the binary then
//! runs the normal engine loop over whichever one was selected on the command
//! line (`--demo <name>`).
//!
//! The actual mesh upload happens in the caller (it needs a GPU device); each
//! demo receives the resulting handles in [`DemoAssets`] and decides which it
//! needs and how to place them.

mod avalanche;
mod cloth_drape;
mod cloth_flag;
mod pendulum;
mod sandbox;
mod soft_ball;
mod stacking;
mod stress;

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
    /// Capsule of [`CAPSULE_BASE_RADIUS`] / [`CAPSULE_BASE_HALF_HEIGHT`], aligned
    /// with local Y. Scale a body's `Transform` uniformly by `s`, paired with a
    /// `Collider::Capsule` whose dimensions are the base values times `s`, so the
    /// drawn capsule matches the collider exactly.
    pub capsule: MeshHandle,
    /// Large flat ground/ramp quad in the XZ plane with a +Y normal.
    pub plane: MeshHandle,
    /// Default surface material for everything in the demos.
    pub material: MaterialHandle,
}

/// Radius the shared capsule mesh ([`DemoAssets::capsule`]) is built at. A
/// capsule body rendered at uniform `Transform` scale `s` uses a
/// `Collider::Capsule { radius: CAPSULE_BASE_RADIUS * s, .. }`; the same `s`
/// scales the mesh, so collider and mesh stay in lockstep.
pub const CAPSULE_BASE_RADIUS: f32 = 0.3;
/// Half-height the shared capsule mesh is built at. See [`CAPSULE_BASE_RADIUS`].
pub const CAPSULE_BASE_HALF_HEIGHT: f32 = 0.5;

/// Which demo scene to build. Parsed from the `--demo <name>` argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demo {
    /// A tower of boxes settling into a stable stack on the ground.
    Stacking,
    /// A pinned chain of spheres swinging as an XPBD rope.
    Pendulum,
    /// 200 spheres tumbling down a ramp and piling at the bottom.
    Avalanche,
    /// A near-empty scene (ground + 5 cubes) for showing off the editor.
    Sandbox,
    /// 500 mixed shapes poured onto the ground — a solver/broadphase stress test.
    Stress,
    /// A tetrahedral soft body dropped onto a table — volume-preserving squash.
    SoftBall,
    /// A cloth banner pinned at two corners, billowing in the wind.
    ClothFlag,
    /// A cloth sheet draped over a spinning cube — soft/rigid contact coupling.
    ClothDrape,
}

impl Demo {
    /// Parse the `--demo` value. Case-insensitive.
    pub fn from_name(name: &str) -> Option<Demo> {
        match name.trim().to_ascii_lowercase().as_str() {
            "stacking" => Some(Demo::Stacking),
            "pendulum" => Some(Demo::Pendulum),
            "avalanche" => Some(Demo::Avalanche),
            "sandbox" => Some(Demo::Sandbox),
            "stress" => Some(Demo::Stress),
            "soft_ball" | "softball" => Some(Demo::SoftBall),
            "cloth_flag" | "clothflag" => Some(Demo::ClothFlag),
            "cloth_drape" | "clothdrape" => Some(Demo::ClothDrape),
            _ => None,
        }
    }

    /// The canonical name, matching what [`from_name`](Demo::from_name) accepts.
    pub fn name(self) -> &'static str {
        match self {
            Demo::Stacking => "stacking",
            Demo::Pendulum => "pendulum",
            Demo::Avalanche => "avalanche",
            Demo::Sandbox => "sandbox",
            Demo::Stress => "stress",
            Demo::SoftBall => "soft_ball",
            Demo::ClothFlag => "cloth_flag",
            Demo::ClothDrape => "cloth_drape",
        }
    }

    /// Every demo, for help text and exhaustive testing.
    pub fn all() -> [Demo; 8] {
        [
            Demo::Stacking,
            Demo::Pendulum,
            Demo::Avalanche,
            Demo::Sandbox,
            Demo::Stress,
            Demo::SoftBall,
            Demo::ClothFlag,
            Demo::ClothDrape,
        ]
    }

    /// Build this demo's scene: spawn the camera and all entities.
    pub fn setup(self, scene: &mut Scene, assets: &DemoAssets) {
        scene.name = self.name().to_string();
        match self {
            Demo::Stacking => stacking::setup(scene, assets),
            Demo::Pendulum => pendulum::setup(scene, assets),
            Demo::Avalanche => avalanche::setup(scene, assets),
            Demo::Sandbox => sandbox::setup(scene, assets),
            Demo::Stress => stress::setup(scene, assets),
            Demo::SoftBall => soft_ball::setup(scene, assets),
            Demo::ClothFlag => cloth_flag::setup(scene, assets),
            Demo::ClothDrape => cloth_drape::setup(scene, assets),
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
