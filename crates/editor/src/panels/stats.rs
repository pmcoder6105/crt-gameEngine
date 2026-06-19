//! Stats panel: frame time, physics step time, and body / entity counts.
//! Timings are fed in by the app each frame via [`Editor::set_stats`].
//!
//! [`Editor::set_stats`]: crate::Editor::set_stats

use elderforge_scene::Scene;

#[derive(Default)]
pub struct Stats {
    /// Wall-clock time of the previous whole frame, in milliseconds.
    pub frame_time_ms: f32,
    /// Time spent stepping the physics world last frame, in milliseconds.
    pub physics_time_ms: f32,
}

impl Stats {
    pub fn ui(&mut self, ui: &mut egui::Ui, scene: &Scene) {
        ui.label(format!("Frame time:   {:.2} ms", self.frame_time_ms));
        ui.label(format!("Physics step: {:.2} ms", self.physics_time_ms));
        if self.frame_time_ms > 0.0 {
            ui.label(format!("FPS:          {:.0}", 1000.0 / self.frame_time_ms));
        }
        ui.separator();
        ui.label(format!("Entities: {}", scene.world.len()));
        ui.label(format!("Bodies:   {}", scene.physics.body_count()));
        ui.label(format!("Awake:    {}", scene.physics.awake_body_count()));
    }
}
