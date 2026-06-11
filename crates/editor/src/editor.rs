//! Top-level editor: owns all panels and lays them out each frame.

use elderforge_scene::Scene;

use crate::panels::{AssetBrowser, Hierarchy, Inspector, Overlays, Profiler, SimControls};

#[derive(Default)]
pub struct Editor {
    pub hierarchy: Hierarchy,
    pub inspector: Inspector,
    pub sim_controls: SimControls,
    pub overlays: Overlays,
    pub profiler: Profiler,
    pub asset_browser: AssetBrowser,
}

impl Editor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run one editor frame. Call inside the egui pass.
    pub fn ui(&mut self, ctx: &egui::Context, scene: &mut Scene) {
        egui::Window::new("Scene Hierarchy").show(ctx, |ui| self.hierarchy.ui(ui, scene));
        // Selection flows from the hierarchy into the inspector.
        self.inspector.selected = self.hierarchy.selected;
        egui::Window::new("Inspector").show(ctx, |ui| self.inspector.ui(ui, scene));
        egui::Window::new("Simulation").show(ctx, |ui| self.sim_controls.ui(ui));
        egui::Window::new("Physics Overlays").show(ctx, |ui| self.overlays.ui(ui));
        egui::Window::new("Profiler").show(ctx, |ui| self.profiler.ui(ui, scene));
        egui::Window::new("Assets").show(ctx, |ui| self.asset_browser.ui(ui));
    }
}
