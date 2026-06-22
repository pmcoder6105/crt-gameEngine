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
mod cloth_drape_showcase;
mod cloth_flag;
mod cloth_tear;
mod mixed;
mod pendulum;
mod sandbox;
mod soft_ball;
mod softbody_drop;
mod stacking;
mod stress;

use elderforge_core::handles::{MaterialHandle, MeshHandle};
use elderforge_core::math::{Mat4, Vec3};
use elderforge_ecs::components::{Camera, Transform};
use elderforge_physics::PhysicsMaterial;
use elderforge_renderer::DirectionalLight;
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
    /// Capture-grade cloth drape: a 40×40 sheet pinned at two top corners over
    /// a slowly rotating cube, lit warm, with the camera orbiting once per 30s.
    ClothDrapeShowcase,
    /// Three soft balls of increasing softness dropped onto a table in sequence.
    SoftbodyDrop,
    /// A cloth pinned along its top edge, taking a heavy sphere on its center.
    ClothTear,
    /// A combined scene: a cloth flag, a soft body rolling down a ramp, and a
    /// rigid box stack it scatters — everything interacting at once.
    Mixed,
}

impl Demo {
    /// Parse the `--demo` value. Case-insensitive. The capture demos use
    /// hyphenated canonical names (`cloth-drape`, `softbody-drop`, `cloth-tear`);
    /// note `cloth-drape` is the 40×40 capture showcase, distinct from the older
    /// `cloth_drape` (underscore) draping demo.
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
            "cloth-drape" | "clothdrapeshowcase" => Some(Demo::ClothDrapeShowcase),
            "softbody-drop" | "softbody_drop" | "softbodydrop" => Some(Demo::SoftbodyDrop),
            "cloth-tear" | "cloth_tear" | "clothtear" => Some(Demo::ClothTear),
            "mixed" => Some(Demo::Mixed),
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
            Demo::ClothDrapeShowcase => "cloth-drape",
            Demo::SoftbodyDrop => "softbody-drop",
            Demo::ClothTear => "cloth-tear",
            Demo::Mixed => "mixed",
        }
    }

    /// Every demo, for help text and exhaustive testing.
    pub fn all() -> [Demo; 12] {
        [
            Demo::Stacking,
            Demo::Pendulum,
            Demo::Avalanche,
            Demo::Sandbox,
            Demo::Stress,
            Demo::SoftBall,
            Demo::ClothFlag,
            Demo::ClothDrape,
            Demo::ClothDrapeShowcase,
            Demo::SoftbodyDrop,
            Demo::ClothTear,
            Demo::Mixed,
        ]
    }

    /// Build this demo's scene: spawn the camera and all entities. Returns the
    /// demo's runtime configuration — any per-frame [animation](DemoAnim) and an
    /// optional [key light](DirectionalLight) override.
    pub fn setup(self, scene: &mut Scene, assets: &DemoAssets) -> DemoConfig {
        scene.name = self.name().to_string();
        match self {
            Demo::Stacking => stacking::setup(scene, assets).into(),
            Demo::Pendulum => pendulum::setup(scene, assets).into(),
            Demo::Avalanche => avalanche::setup(scene, assets).into(),
            Demo::Sandbox => sandbox::setup(scene, assets).into(),
            Demo::Stress => stress::setup(scene, assets).into(),
            Demo::SoftBall => soft_ball::setup(scene, assets).into(),
            Demo::ClothFlag => cloth_flag::setup(scene, assets).into(),
            Demo::ClothDrape => cloth_drape::setup(scene, assets).into(),
            Demo::ClothDrapeShowcase => cloth_drape_showcase::setup(scene, assets),
            Demo::SoftbodyDrop => softbody_drop::setup(scene, assets),
            Demo::ClothTear => cloth_tear::setup(scene, assets).into(),
            Demo::Mixed => mixed::setup(scene, assets).into(),
        }
    }
}

/// Runtime configuration a demo's `setup` hands back to the app: a per-frame
/// [animation](DemoAnim) (camera moves, staged releases) and an optional key
/// [light](DirectionalLight) override. Demos with neither return the default.
///
/// Demo `setup` functions that need neither return `()`, lifted into the
/// default via `.into()` at the dispatch site.
#[derive(Default)]
pub struct DemoConfig {
    pub anim: DemoAnim,
    pub light: Option<DirectionalLight>,
}

impl From<()> for DemoConfig {
    fn from(_: ()) -> Self {
        DemoConfig::default()
    }
}

impl From<DemoAnim> for DemoConfig {
    fn from(anim: DemoAnim) -> Self {
        DemoConfig { anim, ..DemoConfig::default() }
    }
}

/// Per-frame demo animation, applied by the app with the running simulation
/// time. Most demos are purely physics-driven and use [`DemoAnim::None`].
#[derive(Default)]
pub enum DemoAnim {
    /// No scripted motion; the simulation drives everything.
    #[default]
    None,
    /// Orbit the active camera around `center` on a circle of `radius` at a
    /// fixed `height`, completing one revolution every `period` seconds.
    OrbitCamera {
        center: Vec3,
        radius: f32,
        height: f32,
        period: f32,
    },
    /// Release pinned soft bodies on a schedule: each entry restores its
    /// particles' inverse masses once the sim time reaches `release_at`, so a
    /// body held frozen in the air begins to fall on cue.
    StagedDrop(Vec<StagedRelease>),
}

/// One soft body in a [`DemoAnim::StagedDrop`]: the contiguous particle run to
/// release and the time to release it. Until `release_at`, the body sits frozen
/// (its particles pinned with zero inverse mass at setup); at release the saved
/// `inv_masses` are restored and gravity takes over.
pub struct StagedRelease {
    /// First particle index of the body in the world's particle array.
    pub base: usize,
    /// Inverse masses to restore, one per particle from `base`.
    pub inv_masses: Vec<f32>,
    /// Simulation time (seconds) at which to release the body.
    pub release_at: f32,
}

impl DemoAnim {
    /// Apply the animation for the current simulation time. Cheap and idempotent
    /// (a released body just has its masses re-set to the same values).
    pub fn apply(&self, scene: &mut Scene, time: f32) {
        match self {
            DemoAnim::None => {}
            DemoAnim::OrbitCamera { center, radius, height, period } => {
                let theta = std::f32::consts::TAU * time / period.max(1e-3);
                let eye = *center + Vec3::new(radius * theta.cos(), *height, radius * theta.sin());
                let world = Mat4::look_at_rh(eye, *center, Vec3::Y).inverse();
                let (_, rotation, position) = world.to_scale_rotation_translation();
                for (_e, (camera, transform)) in
                    scene.world.query_mut::<(&Camera, &mut Transform)>()
                {
                    if camera.is_active {
                        transform.position = position;
                        transform.rotation = rotation;
                        break;
                    }
                }
            }
            DemoAnim::StagedDrop(releases) => {
                let particles = scene.physics.particles_mut();
                for release in releases {
                    if time >= release.release_at {
                        for (i, &w) in release.inv_masses.iter().enumerate() {
                            particles[release.base + i].inv_mass = w;
                        }
                    }
                }
            }
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
