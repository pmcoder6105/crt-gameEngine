//! App — holds the Scene, GPU resources, Editor, and the frame clock.

use anyhow::Context as _;
use elderforge_core::math::{Mat4, Quat, Vec3};
use elderforge_core::time::FixedTimestep;
use elderforge_ecs::components::{Camera, MeshRenderer, PhysicsBody, Transform};
use elderforge_editor::Editor;
use elderforge_physics::{Collider, PhysicsMaterial, RigidBody};
use elderforge_platform::time::Clock;
use elderforge_platform::WindowHandle;
use elderforge_renderer::material::PbrMaterial;
use elderforge_renderer::{primitives, ForwardPass, GpuMesh, RenderContext, ResourceCache};
use elderforge_scene::Scene;

use crate::systems;

/// Physics runs at a fixed 120 Hz; clamp to 8 steps so a slow frame can't
/// spiral.
const PHYSICS_HZ: f32 = 120.0;
const MAX_STEPS_PER_FRAME: u32 = 8;

/// GPU-side state, created lazily once the window (and thus a surface) exists.
pub struct Gpu {
    pub context: RenderContext,
    pub cache: ResourceCache,
    pub forward: ForwardPass,
}

pub struct App {
    pub scene: Scene,
    pub editor: Editor,
    clock: Clock,
    fixed: FixedTimestep,
    gpu: Option<Gpu>,
}

impl App {
    pub fn new() -> Self {
        Self {
            scene: Scene::new(),
            editor: Editor::new(),
            clock: Clock::new(),
            fixed: FixedTimestep::new(1.0 / PHYSICS_HZ, MAX_STEPS_PER_FRAME),
            gpu: None,
        }
    }

    /// One frame: fixed-step physics (with pose sync), then rendering, then
    /// the editor UI.
    pub fn update(&mut self, window: &WindowHandle) -> anyhow::Result<()> {
        if self.gpu.is_none() {
            self.init(window).context("renderer initialization failed")?;
        }

        // 1. Step the physics world a whole number of fixed steps for the
        //    wall-clock time elapsed since the last frame. `physics::run` also
        //    syncs each body's pose into its Transform.
        let frame_dt = self.clock.tick();
        for _ in 0..self.fixed.integrate(frame_dt) {
            systems::physics::run(&mut self.scene, self.fixed.target_dt());
        }

        // 2-3. Draw every (Transform, MeshRenderer) through the active camera.
        if let Some(gpu) = self.gpu.as_mut() {
            systems::render::run(&self.scene, &mut gpu.context, &gpu.cache, &mut gpu.forward);
        }

        systems::editor::run(&mut self.editor, &mut self.scene);
        Ok(())
    }

    /// Propagate a window resize to the surface and the depth target.
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.context.resize(width, height);
            gpu.forward.resize(&gpu.context.device, (width, height));
        }
    }

    fn init(&mut self, window: &WindowHandle) -> anyhow::Result<()> {
        let (width, height) = window.size();
        let context = RenderContext::new(window.surface_provider(), width, height, window.vsync())?;

        // Upload the primitive meshes once and reference them by handle.
        let mut cache = ResourceCache::new();
        let (cube_v, cube_i) = primitives::cube(0.5);
        let cube = cache.insert_mesh(GpuMesh::upload(&context.device, "cube", &cube_v, &cube_i));
        let (ground_v, ground_i) = primitives::plane(40.0);
        let ground =
            cache.insert_mesh(GpuMesh::upload(&context.device, "ground", &ground_v, &ground_i));
        let material = cache.insert_material(PbrMaterial::default());

        let forward = ForwardPass::new(&context.device, context.surface_format(), (width, height));

        spawn_scene(&mut self.scene, cube, ground, material);
        log::info!("scene: {} entities", self.scene.world.len());

        self.gpu = Some(Gpu { context, cache, forward });
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// A slightly bouncy material so cubes rebound off the ground and each other.
fn bouncy() -> PhysicsMaterial {
    PhysicsMaterial { restitution: 0.6, ..PhysicsMaterial::default() }
}

/// Populate the scene: a fixed camera, a ground plane, and 50 cubes dropped
/// from random heights. Each cube is a render entity (Transform + cube mesh)
/// backed by a dynamic sphere body in the physics world.
fn spawn_scene(
    scene: &mut Scene,
    cube: elderforge_core::handles::MeshHandle,
    ground: elderforge_core::handles::MeshHandle,
    material: elderforge_core::handles::MaterialHandle,
) {
    // Camera: a fixed three-quarter view of the drop zone. Encode the look-at
    // into the Transform so the render system can derive it generically.
    let eye = Vec3::new(0.0, 9.0, 24.0);
    let target = Vec3::new(0.0, 3.0, 0.0);
    let camera_world = Mat4::look_at_rh(eye, target, Vec3::Y).inverse();
    let (_, rotation, position) = camera_world.to_scale_rotation_translation();
    scene.world.spawn((
        Camera::default(),
        Transform { position, rotation, scale: Vec3::ONE },
    ));

    // Ground: a render entity at the origin plus a static half-space body.
    scene.world.spawn((Transform::default(), MeshRenderer { mesh: ground, material }));
    scene.physics.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 })
            .with_material(bouncy()),
    );

    // 50 cubes at random positions above the plane.
    let mut rng = Rng::new(0x00C0_FFEE);
    for _ in 0..50 {
        let position = Vec3::new(
            rng.range(-7.0, 7.0),
            rng.range(5.0, 20.0),
            rng.range(-7.0, 7.0),
        );
        let handle = scene.physics.add_rigid_body(
            RigidBody::dynamic(position, 1.0, Collider::Sphere { radius: 0.5 })
                .with_material(bouncy()),
        );
        scene.world.spawn((
            Transform { position, rotation: Quat::IDENTITY, scale: Vec3::ONE },
            PhysicsBody { handle },
            MeshRenderer { mesh: cube, material },
        ));
    }
}

/// Tiny deterministic xorshift64 RNG — avoids pulling in the `rand` crate just
/// to scatter some cubes. Deterministic so the scene is reproducible.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
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
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let unit = self.next_u32() as f32 / u32::MAX as f32;
        lo + unit * (hi - lo)
    }
}
