//! App — holds Scene, Renderer, Editor, and the Clock.

use elderforge_editor::Editor;
use elderforge_platform::time::Clock;
use elderforge_renderer::context::RenderContext;
use elderforge_scene::Scene;

use crate::systems;

pub struct App {
    pub scene: Scene,
    pub renderer: Option<RenderContext>,
    pub editor: Editor,
    pub clock: Clock,
}

impl App {
    pub fn new() -> Self {
        Self {
            scene: Scene::new(),
            // Created once the window and surface exist.
            renderer: None,
            editor: Editor::new(),
            clock: Clock::new(),
        }
    }

    /// One frame: fixed-step physics, then rendering, then editor UI.
    pub fn update(&mut self) {
        self.clock.tick();
        while self.clock.consume_fixed_step() {
            systems::physics::run(&mut self.scene, self.clock.fixed_dt());
        }
        systems::render::run(&self.scene, self.renderer.as_ref());
        systems::editor::run(&mut self.editor, &mut self.scene);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
