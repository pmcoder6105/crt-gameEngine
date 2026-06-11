//! Profiler panel: frame/physics/render time, entity and body counts.
//! Timings are fed in by the app each frame.

use elderforge_scene::Scene;

#[derive(Default)]
pub struct Profiler {
    pub frame_time_ms: f32,
    pub physics_time_ms: f32,
    pub render_time_ms: f32,
}

impl Profiler {
    pub fn ui(&mut self, ui: &mut egui::Ui, scene: &Scene) {
        ui.label(format!("Frame: {:.2} ms", self.frame_time_ms));
        ui.label(format!("Physics: {:.2} ms", self.physics_time_ms));
        ui.label(format!("Render: {:.2} ms", self.render_time_ms));
        ui.separator();
        ui.label(format!("Entities: {}", scene.world.len()));
        ui.label(format!("Bodies: {}", scene.physics.body_count()));
        // TODO: active (non-sleeping) body count.
    }
}
