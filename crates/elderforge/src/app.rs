//! App — holds the Scene, GPU resources, the editor, and the frame clock, and
//! drives one frame: editor UI → scene save/load (from the toolbar) → physics
//! (under the sim controls) → render (3D + egui in a single surface frame).

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Context as _;
use elderforge_core::time::FixedTimestep;
use elderforge_editor::{EditorState, EditorStats};
use elderforge_platform::time::Clock;
use elderforge_platform::{RawWindowEvent, WindowHandle};
use elderforge_physics::DebugLayers;
use elderforge_renderer::{DebugPass, ForwardPass, RenderContext, ResourceCache};
use elderforge_scene::assets::{MaterialDef, MeshSource};
use elderforge_scene::Scene;

use elderforge::assets::AssetManager;
use elderforge::debug_overlay::DebugOverlay;
use elderforge::deformable::DeformableMeshes;
use elderforge::demos::{Demo, DemoAnim, DemoAssets};

use crate::systems;

/// Launch-time options parsed from the command line and threaded into the app.
#[derive(Debug, Clone, Copy)]
pub struct LaunchOptions {
    /// Hide all editor chrome and clear to black — clean viewport for capture.
    pub borderless: bool,
    /// MSAA sample count for the forward pass (`1` disables it).
    pub msaa: u32,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self { borderless: false, msaa: 1 }
    }
}

/// Physics runs at a fixed 120 Hz; clamp to 8 steps so a slow frame can't
/// spiral.
const PHYSICS_HZ: f32 = 120.0;
const MAX_STEPS_PER_FRAME: u32 = 8;

/// GPU-side state, created lazily once the window (and thus a surface) exists.
pub struct Gpu {
    pub context: RenderContext,
    pub cache: ResourceCache,
    pub forward: ForwardPass,
    /// Physics debug overlay pass (line + point pipelines, reused buffers).
    pub debug: DebugPass,
    /// Per-frame meshes for the scene's soft bodies and cloth, rebuilt from
    /// particle positions each step.
    pub deformables: DeformableMeshes,
}

pub struct App {
    pub scene: Scene,
    demo: Demo,
    options: LaunchOptions,
    clock: Clock,
    fixed: FixedTimestep,
    gpu: Option<Gpu>,
    /// Created lazily alongside `gpu` (it needs the device, surface format, and
    /// window). `None` until the first frame, and always `None` in borderless
    /// capture mode (no editor chrome).
    editor: Option<EditorState>,
    /// This demo's scripted per-frame animation (camera orbits, staged drops),
    /// captured from `Demo::setup` and applied each frame with the sim time.
    anim: DemoAnim,
    /// Accumulated simulation time in seconds, advanced only while physics
    /// steps; drives [`anim`](Self::anim).
    sim_time: f32,
    /// Bridges the physics debug data into renderer vertices; reused each frame.
    debug_overlay: DebugOverlay,
    /// Decodes and memoizes file-backed assets; used to (re)build the GPU cache
    /// from a scene's asset table on startup and after a Load.
    asset_manager: AssetManager,
    /// Last frame's timings, shown by the stats panel one frame later.
    last_frame_ms: f32,
    last_physics_ms: f32,
}

impl App {
    pub fn new(demo: Demo, options: LaunchOptions) -> Self {
        Self {
            scene: Scene::new(),
            demo,
            options,
            clock: Clock::new(),
            fixed: FixedTimestep::new(1.0 / PHYSICS_HZ, MAX_STEPS_PER_FRAME),
            gpu: None,
            editor: None,
            anim: DemoAnim::None,
            sim_time: 0.0,
            debug_overlay: DebugOverlay::new(),
            asset_manager: AssetManager::new(),
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

    /// One frame: run the editor UI, honor any save/load request, step physics
    /// under the sim controls, then draw the 3D scene and the editor into a
    /// single surface frame.
    pub fn update(&mut self, window: &WindowHandle) -> anyhow::Result<()> {
        if self.gpu.is_none() {
            self.init(window).context("renderer initialization failed")?;
        }

        let frame_dt = self.clock.tick();
        let stats = EditorStats {
            frame_time_ms: self.last_frame_ms,
            physics_time_ms: self.last_physics_ms,
        };

        let Self {
            scene, gpu, editor, fixed, anim, sim_time, debug_overlay, asset_manager, ..
        } = self;
        let gpu = gpu.as_mut().expect("gpu initialized above");

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

        // 1. Editor UI (windowed only) — reads/edits the scene, toggles the sim
        //    controls, and records save/load requests in the toolbar. Borderless
        //    capture has no editor: it just plays at the scene's own settings.
        let editor_frame;
        let (playing, single_step, multiplier, substeps);
        match editor.as_mut() {
            Some(editor) => {
                editor_frame = Some(editor.run_frame(window.winit_window(), scene, stats));
                // Service the toolbar's scene save/load. Loading swaps in a new
                // scene and rebuilds the GPU cache before anything below renders.
                handle_scene_io(scene, gpu, editor, asset_manager);
                let controls = &mut editor.editor.sim_controls;
                playing = controls.playing;
                single_step = std::mem::take(&mut controls.single_step_requested);
                multiplier = controls.timestep_multiplier;
                substeps = controls.substeps;
            }
            None => {
                editor_frame = None;
                playing = true;
                single_step = false;
                multiplier = 1.0;
                substeps = scene.physics.substeps;
            }
        }
        scene.physics.substeps = substeps;

        // 2. Step physics. Pause stops stepping (but not rendering); Step
        //    advances exactly one fixed tick; the multiplier scales simulated
        //    time. Advance the sim clock by the work actually done, then run the
        //    demo's scripted animation against it.
        let physics_start = Instant::now();
        let mut steps = 0u32;
        if playing {
            steps = fixed.integrate(frame_dt * multiplier);
            for _ in 0..steps {
                systems::physics::run(scene, fixed.target_dt());
            }
        } else if single_step {
            systems::physics::run(scene, fixed.target_dt());
            steps = 1;
        }
        let physics_ms = physics_start.elapsed().as_secs_f32() * 1000.0;
        *sim_time += steps as f32 * fixed.target_dt();
        anim.apply(scene, *sim_time);

        // 3. Rebuild the physics debug overlay for this frame from the enabled
        //    editor toggles (no editor → all layers off → empty/cheap).
        let layers = editor
            .as_ref()
            .map(|e| {
                let o = &e.editor.overlays;
                DebugLayers {
                    collision_shapes: o.collision_shapes,
                    velocity_vectors: o.velocity_vectors,
                    angular_velocity: o.angular_velocity,
                    contact_points: o.contact_points,
                    constraint_anchors: o.constraint_anchors,
                    bvh_aabbs: o.bvh_aabbs,
                    sleep_state: o.sleep_state,
                    force_accumulators: o.force_accumulators,
                }
            })
            .unwrap_or_default();
        debug_overlay.update(&scene.physics, layers);

        // 4. Restream the soft-body / cloth meshes from their post-step particle
        //    positions, then render: 3D scene, debug overlay, editor; then present.
        gpu.deformables.update(&gpu.context.queue, &scene.physics);
        systems::render::record(
            scene,
            &gpu.context,
            &gpu.cache,
            &gpu.deformables,
            &mut gpu.forward,
            &mut gpu.debug,
            debug_overlay,
            &mut frame,
        );
        let (width, height) = gpu.context.size();
        if let (Some(editor), Some(editor_frame)) = (editor.as_mut(), editor_frame.as_ref()) {
            editor.paint(
                &gpu.context.device,
                &gpu.context.queue,
                &mut frame.encoder,
                &frame.view,
                [width, height],
                editor_frame,
            );
        }
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
        // Clamp the requested MSAA count to what this GPU/format actually
        // supports, so e.g. `--msaa 8` falls back to 4× instead of crashing.
        let samples = context.supported_sample_count(self.options.msaa);
        if samples != self.options.msaa {
            log::warn!(
                "msaa {}x is unsupported for this surface; using {}x",
                self.options.msaa,
                samples
            );
        }
        let mut forward =
            ForwardPass::new(&context.device, context.surface_format(), (width, height), samples);

        // `SceneAssets` is the resource-handle authority: register the builtin
        // primitive meshes and the default material, hand the handles to the
        // demo, then realize the whole table into the GPU cache at those handles.
        let cube = self.scene.assets.register_mesh(MeshSource::Builtin("cube".into()));
        let sphere = self.scene.assets.register_mesh(MeshSource::Builtin("sphere".into()));
        let capsule = self.scene.assets.register_mesh(MeshSource::Builtin("capsule".into()));
        let plane = self.scene.assets.register_mesh(MeshSource::Builtin("plane".into()));
        let material = self.scene.assets.register_material(MaterialDef::default());

        let assets = DemoAssets { cube, sphere, capsule, plane, material };
        let config = self.demo.setup(&mut self.scene, &assets);
        log::info!("demo '{}': {} entities", self.demo.name(), self.scene.world.len());

        // Apply the demo's optional key-light override, and (in borderless
        // capture mode) clear to pure black instead of the editor sky.
        if let Some(light) = config.light {
            forward.set_light(light);
        }
        if self.options.borderless {
            forward.set_clear_color(wgpu::Color::BLACK);
        }
        self.anim = config.anim;

        let cache = self
            .asset_manager
            .realize(&self.scene, &context.device, &context.queue)
            .context("realizing demo assets")?;

        // Dynamic meshes for the demo's soft bodies / cloth (if any).
        let deformables = DeformableMeshes::build(&context.device, &self.scene.physics);

        // Debug overlay pass renders single-sampled over the resolved surface.
        let debug = DebugPass::new(&context.device, context.surface_format());

        // The editor exists only in windowed mode; borderless capture renders
        // just the viewport. When present, seed its substep slider with the
        // scene's own count so it starts in agreement with the demo.
        let editor = if self.options.borderless {
            None
        } else {
            let mut editor =
                EditorState::new(&context.device, context.surface_format(), window.winit_window());
            editor.editor.sim_controls.substeps = self.scene.physics.substeps;
            editor.editor.toolbar.status = format!("demo: {}", self.demo.name());
            Some(editor)
        };

        self.gpu = Some(Gpu { context, cache, forward, debug, deformables });
        self.editor = editor;
        Ok(())
    }
}

/// Consume the toolbar's save/load requests for this frame. Save writes the
/// current scene; Load parses a scene, rebuilds the GPU cache from its asset
/// table, and (only on success) swaps it in. Outcomes are written back to the
/// toolbar's status line.
fn handle_scene_io(
    scene: &mut Scene,
    gpu: &mut Gpu,
    editor: &mut EditorState,
    asset_manager: &mut AssetManager,
) {
    let save = std::mem::take(&mut editor.editor.toolbar.save_requested);
    let load = std::mem::take(&mut editor.editor.toolbar.load_requested);
    if !save && !load {
        return;
    }
    let path = PathBuf::from(&editor.editor.toolbar.path);

    if save {
        editor.editor.toolbar.status = match elderforge_scene::serializer::save_scene(scene, &path) {
            Ok(()) => format!("saved {}", path.display()),
            Err(err) => format!("save failed: {err}"),
        };
    }

    if load {
        match elderforge_scene::loader::load_scene(&path) {
            Ok(loaded) => {
                match asset_manager.realize(&loaded, &gpu.context.device, &gpu.context.queue) {
                    Ok(new_cache) => {
                        gpu.cache = new_cache;
                        *scene = loaded;
                        // Rebuild the deformable meshes for the new scene (loaded
                        // scenes carry no soft bodies yet, so this is usually empty).
                        gpu.deformables =
                            DeformableMeshes::build(&gpu.context.device, &scene.physics);
                        // The selected entity belonged to the old scene; clear
                        // it, and re-sync the substep slider to the new scene.
                        editor.editor.hierarchy.selected = None;
                        editor.editor.sim_controls.substeps = scene.physics.substeps;
                        editor.editor.toolbar.status = format!("loaded {}", path.display());
                    }
                    Err(err) => {
                        editor.editor.toolbar.status = format!("load failed (assets): {err}");
                    }
                }
            }
            Err(err) => {
                editor.editor.toolbar.status = format!("load failed: {err}");
            }
        }
    }
}
