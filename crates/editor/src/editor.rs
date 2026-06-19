//! Top-level editor: owns all panels and lays them out each frame.

use elderforge_scene::Scene;

use crate::panels::{AssetBrowser, Hierarchy, Inspector, Overlays, SimControls, Stats};
use crate::state::EditorStats;

#[derive(Default)]
pub struct Editor {
    pub hierarchy: Hierarchy,
    pub inspector: Inspector,
    pub sim_controls: SimControls,
    pub overlays: Overlays,
    pub stats: Stats,
    pub asset_browser: AssetBrowser,
}

impl Editor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed in this frame's measured timings, shown by the stats panel.
    pub fn set_stats(&mut self, stats: EditorStats) {
        self.stats.frame_time_ms = stats.frame_time_ms;
        self.stats.physics_time_ms = stats.physics_time_ms;
    }

    /// Run one editor frame. Call inside the egui pass.
    pub fn ui(&mut self, ctx: &egui::Context, scene: &mut Scene) {
        egui::Window::new("Scene Hierarchy").show(ctx, |ui| self.hierarchy.ui(ui, scene));
        // Selection flows from the hierarchy into the inspector.
        self.inspector.selected = self.hierarchy.selected;
        egui::Window::new("Inspector").show(ctx, |ui| self.inspector.ui(ui, scene));
        egui::Window::new("Simulation").show(ctx, |ui| self.sim_controls.ui(ui));
        egui::Window::new("Physics Overlays").show(ctx, |ui| self.overlays.ui(ui));
        egui::Window::new("Stats").show(ctx, |ui| self.stats.ui(ui, scene));
        egui::Window::new("Assets").show(ctx, |ui| self.asset_browser.ui(ui));
    }
}
