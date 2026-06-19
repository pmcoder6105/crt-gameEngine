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
use elderforge_renderer::{ForwardPass, RenderContext, ResourceCache};
use elderforge_scene::assets::{MaterialDef, MeshSource};
use elderforge_scene::Scene;

use elderforge::assets::AssetManager;
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
    /// Decodes and memoizes file-backed assets; used to (re)build the GPU cache
    /// from a scene's asset table on startup and after a Load.
    asset_manager: AssetManager,
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

        let Self { scene, gpu, editor, fixed, asset_manager, .. } = self;
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

        // 1. Editor UI — reads/edits the scene, toggles the sim controls, and
        //    records save/load requests in the toolbar.
        let editor_frame = editor.run_frame(window.winit_window(), scene, stats);

        // 2. Service the toolbar's scene save/load. Loading swaps in a new scene
        //    and rebuilds the GPU cache from its asset table before anything
        //    below renders it.
        handle_scene_io(scene, gpu, editor, asset_manager);

        // 3. Step physics according to the simulation controls. Pause stops the
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

        // 4. Render: 3D scene first, then the editor on top, then present.
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
        let forward = ForwardPass::new(&context.device, context.surface_format(), (width, height));

        // `SceneAssets` is the resource-handle authority: register the builtin
        // primitive meshes and the default material, hand the handles to the
        // demo, then realize the whole table into the GPU cache at those handles.
        let cube = self.scene.assets.register_mesh(MeshSource::Builtin("cube".into()));
        let sphere = self.scene.assets.register_mesh(MeshSource::Builtin("sphere".into()));
        let capsule = self.scene.assets.register_mesh(MeshSource::Builtin("capsule".into()));
        let plane = self.scene.assets.register_mesh(MeshSource::Builtin("plane".into()));
        let material = self.scene.assets.register_material(MaterialDef::default());

        let assets = DemoAssets { cube, sphere, capsule, plane, material };
        self.demo.setup(&mut self.scene, &assets);
        log::info!("demo '{}': {} entities", self.demo.name(), self.scene.world.len());

        let cache = self
            .asset_manager
            .realize(&self.scene, &context.device, &context.queue)
            .context("realizing demo assets")?;

        // The editor needs the device + surface format; create it here, then
        // seed the substep slider with the scene's own substep count so it
        // starts in agreement with the demo.
        let mut editor =
            EditorState::new(&context.device, context.surface_format(), window.winit_window());
        editor.editor.sim_controls.substeps = self.scene.physics.substeps;
        editor.editor.toolbar.status = format!("demo: {}", self.demo.name());

        self.gpu = Some(Gpu { context, cache, forward });
        self.editor = Some(editor);
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
