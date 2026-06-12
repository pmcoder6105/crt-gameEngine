//! App — holds Scene, Renderer, Editor, and the Clock.

use anyhow::Context as _;
use elderforge_editor::Editor;
use elderforge_platform::time::Clock;
use elderforge_platform::WindowHandle;
use elderforge_renderer::passes::unlit::UnlitPass;
use elderforge_renderer::{GpuMesh, RenderContext, Vertex};
use elderforge_scene::Scene;

use crate::systems;

pub struct App {
    pub scene: Scene,
    pub renderer: Option<RenderContext>,
    pub editor: Editor,
    pub clock: Clock,
    /// Bootstrap geometry drawn until scene rendering lands: a clip-space
    /// RGB triangle (colors ride in the vertex normal slot).
    pub triangle: Option<(UnlitPass, GpuMesh)>,
}

impl App {
    pub fn new() -> Self {
        Self {
            scene: Scene::new(),
            // Created on the first frame, once the window exists.
            renderer: None,
            editor: Editor::new(),
            clock: Clock::new(),
            triangle: None,
        }
    }

    /// One frame: fixed-step physics, then rendering, then editor UI.
    pub fn update(&mut self, window: &WindowHandle) -> anyhow::Result<()> {
        if self.renderer.is_none() {
            self.init_renderer(window)
                .context("renderer initialization failed")?;
        }

        self.clock.tick();
        while self.clock.consume_fixed_step() {
            systems::physics::run(&mut self.scene, self.clock.fixed_dt());
        }
        systems::render::run(
            &self.scene,
            self.renderer.as_mut(),
            self.triangle.as_ref(),
        );
        systems::editor::run(&mut self.editor, &mut self.scene);
        Ok(())
    }

    /// Propagate a window resize to the render surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(width, height);
        }
    }

    fn init_renderer(&mut self, window: &WindowHandle) -> anyhow::Result<()> {
        let (width, height) = window.size();
        let renderer =
            RenderContext::new(window.surface_provider(), width, height, window.vsync())?;

        let pass = UnlitPass::new(&renderer.device, renderer.surface_format());
        let vertex = |position: [f32; 3], color: [f32; 3]| Vertex {
            position,
            normal: color,
            uv: [0.0, 0.0],
            tangent: [0.0; 4],
        };
        let mesh = GpuMesh::upload(
            &renderer.device,
            "bootstrap triangle",
            &[
                vertex([0.0, 0.6, 0.0], [1.0, 0.0, 0.0]),
                vertex([-0.6, -0.6, 0.0], [0.0, 1.0, 0.0]),
                vertex([0.6, -0.6, 0.0], [0.0, 0.0, 1.0]),
            ],
            &[0, 1, 2],
        );

        self.triangle = Some((pass, mesh));
        self.renderer = Some(renderer);
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
