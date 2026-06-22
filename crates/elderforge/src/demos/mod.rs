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
mod debug_bvh;
mod debug_cloth;
mod debug_layers;
mod debug_solo;
mod debug_stack;
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
use elderforge_physics::{BodyHandle, DebugLayers, PhysicsMaterial};
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
    /// Debug-capture: a 15-box tower with every overlay on, toppled by a timed
    /// sideways shove so the debug viz explodes with activity as bodies wake.
    DebugStack,
    /// Debug-capture: the cloth-drape showcase glowing with its constraint
    /// springs and a velocity vector on every particle.
    DebugCloth,
    /// Debug-capture: the avalanche with *only* the depth-colored BVH overlay,
    /// so the broadphase tree is seen restructuring as the spheres pour down.
    DebugBvh,
    /// Debug-capture: the mixed scene revealing one more overlay layer every
    /// few seconds until all are on (a cumulative timed reveal).
    DebugLayers,
    /// Debug-capture: the mixed scene cycling through each overlay layer alone,
    /// one at a time, for clean isolated shots of every layer.
    DebugSolo,
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
            "debug-stack" | "debug_stack" | "debugstack" => Some(Demo::DebugStack),
            "debug-cloth" | "debug_cloth" | "debugcloth" => Some(Demo::DebugCloth),
            "debug-bvh" | "debug_bvh" | "debugbvh" => Some(Demo::DebugBvh),
            "debug-layers" | "debug_layers" | "debuglayers" => Some(Demo::DebugLayers),
            "debug-solo" | "debug_solo" | "debugsolo" => Some(Demo::DebugSolo),
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
            Demo::DebugStack => "debug-stack",
            Demo::DebugCloth => "debug-cloth",
            Demo::DebugBvh => "debug-bvh",
            Demo::DebugLayers => "debug-layers",
            Demo::DebugSolo => "debug-solo",
        }
    }

    /// Every demo, for help text and exhaustive testing.
    pub fn all() -> [Demo; 17] {
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
            Demo::DebugStack,
            Demo::DebugCloth,
            Demo::DebugBvh,
            Demo::DebugLayers,
            Demo::DebugSolo,
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
            Demo::DebugStack => debug_stack::setup(scene, assets),
            Demo::DebugCloth => debug_cloth::setup(scene, assets),
            Demo::DebugBvh => debug_bvh::setup(scene, assets),
            Demo::DebugLayers => debug_layers::setup(scene, assets),
            Demo::DebugSolo => debug_solo::setup(scene, assets),
        }
    }
}

/// Runtime configuration a demo's `setup` hands back to the app: a per-frame
/// [animation](DemoAnim) (camera moves, staged releases), an optional key
/// [light](DirectionalLight) override, and an optional [debug-overlay
/// schedule](DebugScript) the app drives even with no editor. Demos that want
/// none of these return the default.
///
/// Demo `setup` functions that need none return `()`, lifted into the default
/// via `.into()` at the dispatch site.
#[derive(Default)]
pub struct DemoConfig {
    pub anim: DemoAnim,
    pub light: Option<DirectionalLight>,
    /// Demo-driven physics debug overlays, evaluated each frame against the sim
    /// time. The app unions these with the editor's manual toggles, so the
    /// debug-capture demos light up their overlays with no editor present.
    pub debug: DebugScript,
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

/// One physics debug overlay layer, the unit a [`DebugScript`] turns on and off.
/// Mirrors the fields of [`DebugLayers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugLayer {
    CollisionShapes,
    VelocityVectors,
    AngularVelocity,
    ContactPoints,
    ConstraintAnchors,
    BvhAabbs,
    SleepState,
    ForceAccumulators,
}

impl DebugLayer {
    /// Enable this layer in `layers`.
    fn set(self, layers: &mut DebugLayers) {
        match self {
            DebugLayer::CollisionShapes => layers.collision_shapes = true,
            DebugLayer::VelocityVectors => layers.velocity_vectors = true,
            DebugLayer::AngularVelocity => layers.angular_velocity = true,
            DebugLayer::ContactPoints => layers.contact_points = true,
            DebugLayer::ConstraintAnchors => layers.constraint_anchors = true,
            DebugLayer::BvhAabbs => layers.bvh_aabbs = true,
            DebugLayer::SleepState => layers.sleep_state = true,
            DebugLayer::ForceAccumulators => layers.force_accumulators = true,
        }
    }
}

/// A scripted debug-overlay timeline a demo hands to the app. Each frame the app
/// evaluates it against the running sim time to decide which overlay layers are
/// on, then unions the result with the editor's manual toggles. This is what
/// lets the debug-capture demos drive their overlays in `--borderless` mode,
/// where there is no editor to tick the checkboxes.
#[derive(Default, Clone)]
pub enum DebugScript {
    /// No demo-driven overlays; only the editor's toggles apply.
    #[default]
    None,
    /// A fixed set of layers, on for the whole run.
    Always(DebugLayers),
    /// Cumulative reveal: start with nothing, then enable one more layer (in
    /// `order`) every `interval` seconds, keeping the earlier ones on. Once all
    /// of `order` is revealed they stay on.
    Cumulative { order: Vec<DebugLayer>, interval: f32 },
    /// Solo cycle: exactly one layer on at a time, advancing to the next in
    /// `order` every `interval` seconds and wrapping around. The first layer is
    /// on from `t = 0`.
    Solo { order: Vec<DebugLayer>, interval: f32 },
}

impl DebugScript {
    /// Which overlay layers are enabled at simulation time `time` (seconds).
    pub fn layers_at(&self, time: f32) -> DebugLayers {
        let mut layers = DebugLayers::default();
        match self {
            DebugScript::None => {}
            DebugScript::Always(set) => layers = *set,
            DebugScript::Cumulative { order, interval } => {
                // Number of layers revealed so far: one per elapsed interval,
                // capped at the list length. Zero until the first interval
                // elapses, so the capture opens on a clean, overlay-free scene.
                let step = interval.max(1e-3);
                let revealed = (time / step).floor().max(0.0) as usize;
                for layer in order.iter().take(revealed.min(order.len())) {
                    layer.set(&mut layers);
                }
            }
            DebugScript::Solo { order, interval } => {
                if !order.is_empty() {
                    let step = interval.max(1e-3);
                    let idx = (time / step).floor().max(0.0) as usize % order.len();
                    order[idx].set(&mut layers);
                }
            }
        }
        layers
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
    /// Blast a set of bodies radially outward from `center` (with a slight
    /// upward bias) at `speed` once the sim time reaches `at`, waking them — a
    /// one-shot, contained "shockwave" that bursts a settled, sleeping pile back
    /// into motion. `fired` latches it so it happens exactly once.
    ///
    /// Used by the debug-stack demo: after the sphere pile has gone to sleep,
    /// the shockwave wakes the whole island and the bodies scatter and re-settle
    /// within the pit, lighting up every debug overlay. (Spheres, not a box
    /// tower: the engine's box-box contacts are linear-only and detonate when a
    /// settled stack is disturbed, so a contained sphere pile is what reliably
    /// gives a bounded settle → sleep → burst.)
    Shockwave {
        /// Bodies to blast.
        handles: Vec<BodyHandle>,
        /// Point the blast emanates from (roughly the pile's center).
        center: Vec3,
        /// Outward speed imparted to each body, in m/s.
        speed: f32,
        /// Sim time (seconds) at which to fire the blast.
        at: f32,
        /// Latched once the blast has fired, so it happens exactly once.
        fired: bool,
    },
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
    /// for the continuous animations (a released body just has its masses re-set
    /// to the same values); the one-shot [`ImpulseAt`](DemoAnim::ImpulseAt)
    /// latches itself so it fires exactly once, which is why this takes
    /// `&mut self`.
    pub fn apply(&mut self, scene: &mut Scene, time: f32) {
        match self {
            DemoAnim::None => {}
            DemoAnim::OrbitCamera { center, radius, height, period } => {
                let theta = std::f32::consts::TAU * time / period.max(1e-3);
                let eye =
                    *center + Vec3::new(*radius * theta.cos(), *height, *radius * theta.sin());
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
            DemoAnim::Shockwave { handles, center, speed, at, fired } => {
                if !*fired && time >= *at {
                    for &handle in handles.iter() {
                        if let Some(body) = scene.physics.body_mut(handle) {
                            // Radial direction from the blast center, biased
                            // upward so the pile bursts up and out. Bodies at the
                            // exact center fall back on straight up.
                            let mut dir =
                                (body.position - *center + Vec3::Y * 0.5).normalize_or_zero();
                            if dir == Vec3::ZERO {
                                dir = Vec3::Y;
                            }
                            body.linear_velocity = dir * *speed;
                            // Wake it directly: a sleeping body skips integration,
                            // so the blast would otherwise be ignored. Its motion
                            // then keeps it (and its island) awake.
                            body.sleeping = false;
                        }
                    }
                    *fired = true;
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
    use elderforge_physics::{Collider, RigidBody};

    #[test]
    fn names_round_trip() {
        for demo in Demo::all() {
            assert_eq!(Demo::from_name(demo.name()), Some(demo));
        }
        assert_eq!(Demo::from_name("STACKING"), Some(Demo::Stacking));
        assert_eq!(Demo::from_name("debug-stack"), Some(Demo::DebugStack));
        assert_eq!(Demo::from_name("debug_layers"), Some(Demo::DebugLayers));
        assert_eq!(Demo::from_name("nope"), None);
    }

    #[test]
    fn cumulative_reveals_layers_one_interval_at_a_time() {
        let script = DebugScript::Cumulative {
            order: vec![DebugLayer::CollisionShapes, DebugLayer::BvhAabbs],
            interval: 4.0,
        };
        // Opens on a clean, overlay-free scene.
        assert_eq!(script.layers_at(0.0), DebugLayers::default());
        assert_eq!(script.layers_at(3.9), DebugLayers::default());
        // First layer once the first interval elapses; the second still off.
        let at5 = script.layers_at(5.0);
        assert!(at5.collision_shapes && !at5.bvh_aabbs);
        // Both on (and latched) well past the last interval.
        let at100 = script.layers_at(100.0);
        assert!(at100.collision_shapes && at100.bvh_aabbs);
    }

    #[test]
    fn solo_cycles_exactly_one_layer_and_wraps() {
        let script = DebugScript::Solo {
            order: vec![DebugLayer::CollisionShapes, DebugLayer::BvhAabbs],
            interval: 4.0,
        };
        let t0 = script.layers_at(0.0);
        assert!(t0.collision_shapes && !t0.bvh_aabbs);
        let t5 = script.layers_at(5.0);
        assert!(!t5.collision_shapes && t5.bvh_aabbs);
        // Past the end of the list it wraps back to the first.
        let t9 = script.layers_at(9.0);
        assert!(t9.collision_shapes && !t9.bvh_aabbs);
    }

    #[test]
    fn shockwave_fires_once_and_blasts_bodies_outward() {
        let mut scene = Scene::new();
        // Two bodies either side of the origin, so the radial blast sends them
        // in opposite directions.
        let left = scene.physics.add_rigid_body(RigidBody::dynamic(
            Vec3::new(-1.0, 0.0, 0.0),
            1.0,
            Collider::Sphere { radius: 0.3 },
        ));
        let right = scene.physics.add_rigid_body(RigidBody::dynamic(
            Vec3::new(1.0, 0.0, 0.0),
            1.0,
            Collider::Sphere { radius: 0.3 },
        ));
        let mut anim = DemoAnim::Shockwave {
            handles: vec![left, right],
            center: Vec3::ZERO,
            speed: 5.0,
            at: 5.0,
            fired: false,
        };
        // Before the cue: untouched.
        anim.apply(&mut scene, 4.9);
        assert_eq!(scene.physics.body(left).expect("body").linear_velocity, Vec3::ZERO);
        // At the cue: each body is blasted outward (opposite X directions).
        anim.apply(&mut scene, 5.1);
        let vl = scene.physics.body(left).expect("body").linear_velocity;
        let vr = scene.physics.body(right).expect("body").linear_velocity;
        assert!(vl.x < 0.0 && vr.x > 0.0, "bodies should fly apart: {vl:?} {vr:?}");
        assert!((vl.length() - 5.0).abs() < 1e-3, "blast speed should be 5");
        // Later: it does not fire again.
        let captured = vl;
        anim.apply(&mut scene, 6.0);
        assert_eq!(
            scene.physics.body(left).expect("body").linear_velocity,
            captured,
            "the blast must fire exactly once"
        );
    }
}
