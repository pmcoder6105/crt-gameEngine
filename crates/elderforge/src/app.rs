//! App — holds the Scene, GPU resources, the editor, and the frame clock, and
//! drives one frame: editor UI → physics (under the sim controls) → render
//! (3D + egui in a single surface frame).

use std::time::Instant;

use anyhow::Context as _;
use elderforge_core::time::FixedTimestep;
use elderforge_editor::{EditorState, EditorStats};
use elderforge_platform::time::Clock;
use elderforge_platform::{RawWindowEvent, WindowHandle};
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
    demo: Demo,
    clock: Clock,
    fixed: FixedTimestep,
    gpu: Option<Gpu>,
    /// Created lazily alongside `gpu` (it needs the device, surface format, and
    /// window). `None` until the first frame.
    editor: Option<EditorState>,
    /// Last frame's timings, shown by the stats panel one frame later.
    last_frame_ms: f32,
    last_physics_ms: f32,
}

impl App {
    pub fn new(demo: Demo) -> Self {
        Self {
            scene: Scene::new(),
            demo,
            clock: Clock::new(),
            fixed: FixedTimestep::new(1.0 / PHYSICS_HZ, MAX_STEPS_PER_FRAME),
            gpu: None,
            editor: None,
            last_frame_ms: 0.0,
            last_physics_ms: 0.0,
        }
    }

    /// Forward a raw window event to the editor's egui input. No-op until the
    /// editor exists (the first frame creates it). Returns whether egui used it.
    pub fn integrate_event(&mut self, window: &WindowHandle, event: &RawWindowEvent) -> bool {
        match self.editor.as_mut() {
            Some(editor) => editor.integrate_event(window.winit_window(), event),
            None => false,
        }
    }

    /// One frame: run the editor UI, step physics under the sim controls, then
    /// draw the 3D scene and the editor into a single surface frame.
    pub fn update(&mut self, window: &WindowHandle) -> anyhow::Result<()> {
        if self.gpu.is_none() {
            self.init(window).context("renderer initialization failed")?;
        }

        let frame_dt = self.clock.tick();
        let stats = EditorStats {
            frame_time_ms: self.last_frame_ms,
            physics_time_ms: self.last_physics_ms,
        };

        let Self { scene, gpu, editor, fixed, .. } = self;
        let gpu = gpu.as_mut().expect("gpu initialized above");
        let editor = editor.as_mut().expect("editor initialized above");

        // Acquire the surface frame up front so the 3D pass and the egui pass
        // share one encoder and one present.
        let mut frame = match gpu.context.frame() {
            Ok(frame) => frame,
            Err(err) => {
                // Transient (e.g. surface timeout mid-resize): skip this frame.
                log::warn!("render: skipping frame: {err}");
                return Ok(());
            }
        };

        // 1. Editor UI — reads/edits the scene, toggles the sim controls.
        let editor_frame = editor.run_frame(window.winit_window(), scene, stats);

        // 2. Step physics according to the simulation controls. Pause stops the
        //    stepping (but not the rendering below); Step advances exactly one
        //    fixed tick; the multiplier scales simulated time; the substep slider
        //    drives the solver's substep count.
        let controls = &mut editor.editor.sim_controls;
        let (playing, single_step, multiplier, substeps) = (
            controls.playing,
            std::mem::take(&mut controls.single_step_requested),
            controls.timestep_multiplier,
            controls.substeps,
        );
        scene.physics.substeps = substeps;

        let physics_start = Instant::now();
        if playing {
            for _ in 0..fixed.integrate(frame_dt * multiplier) {
                systems::physics::run(scene, fixed.target_dt());
            }
        } else if single_step {
            systems::physics::run(scene, fixed.target_dt());
        }
        let physics_ms = physics_start.elapsed().as_secs_f32() * 1000.0;

        // 3. Render: 3D scene first, then the editor on top, then present.
        systems::render::record(scene, &gpu.context, &gpu.cache, &mut gpu.forward, &mut frame);
        let (width, height) = gpu.context.size();
        editor.paint(
            &gpu.context.device,
            &gpu.context.queue,
            &mut frame.encoder,
            &frame.view,
            [width, height],
            &editor_frame,
        );
        gpu.context.present(frame);

        // Remember this frame's timings for the stats panel to show next frame.
        self.last_frame_ms = frame_dt * 1000.0;
        self.last_physics_ms = physics_ms;
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

        // The editor needs the device + surface format; create it here, then
        // seed the substep slider with the scene's own substep count so it
        // starts in agreement with the demo.
        let mut editor = EditorState::new(&context.device, context.surface_format(), window.winit_window());
        editor.editor.sim_controls.substeps = self.scene.physics.substeps;

        self.gpu = Some(Gpu { context, cache, forward });
        self.editor = Some(editor);
        Ok(())
    }
}
