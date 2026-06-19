//! App — holds the Scene, GPU resources, Editor, and the frame clock.

use anyhow::Context as _;
use elderforge_core::time::FixedTimestep;
use elderforge_editor::Editor;
use elderforge_platform::time::Clock;
use elderforge_platform::WindowHandle;
use elderforge_renderer::material::PbrMaterial;
use elderforge_renderer::{primitives, ForwardPass, GpuMesh, RenderContext, ResourceCache};
use elderforge_scene::Scene;

use elderforge::demos::{Demo, DemoAssets};

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
    demo: Demo,
    clock: Clock,
    fixed: FixedTimestep,
    gpu: Option<Gpu>,
}

impl App {
    pub fn new(demo: Demo) -> Self {
        Self {
            scene: Scene::new(),
            editor: Editor::new(),
            demo,
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

        // Upload the primitive meshes once and reference them by handle. Every
        // demo draws from this shared set; the demo picks what it needs.
        let mut cache = ResourceCache::new();
        let (cube_v, cube_i) = primitives::cube(0.5);
        let cube = cache.insert_mesh(GpuMesh::upload(&context.device, "cube", &cube_v, &cube_i));
        let (sphere_v, sphere_i) = primitives::sphere(1.0, 24, 16);
        let sphere = cache.insert_mesh(GpuMesh::upload(&context.device, "sphere", &sphere_v, &sphere_i));
        let (plane_v, plane_i) = primitives::plane(40.0);
        let plane =
            cache.insert_mesh(GpuMesh::upload(&context.device, "plane", &plane_v, &plane_i));
        let material = cache.insert_material(PbrMaterial::default());

        let forward = ForwardPass::new(&context.device, context.surface_format(), (width, height));

        let assets = DemoAssets { cube, sphere, plane, material };
        self.demo.setup(&mut self.scene, &assets);
        log::info!("demo '{}': {} entities", self.demo.name(), self.scene.world.len());

        self.gpu = Some(Gpu { context, cache, forward });
        Ok(())
    }
}
