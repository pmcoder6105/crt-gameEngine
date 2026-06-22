//! Per-layer physics debug overlay toggles. Mirrors the physics crate's
//! `DebugLayers` 1:1; the app maps these bools into that struct each frame.

#[derive(Default)]
pub struct Overlays {
    pub collision_shapes: bool,
    pub velocity_vectors: bool,
    pub angular_velocity: bool,
    pub contact_points: bool,
    pub constraint_anchors: bool,
    pub bvh_aabbs: bool,
    pub sleep_state: bool,
    pub force_accumulators: bool,
}

impl Overlays {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.collision_shapes, "Collision shapes");
        ui.checkbox(&mut self.velocity_vectors, "Velocity vectors");
        ui.checkbox(&mut self.angular_velocity, "Angular velocity");
        ui.checkbox(&mut self.contact_points, "Contact points + normals");
        ui.checkbox(&mut self.constraint_anchors, "Constraint anchors");
        ui.checkbox(&mut self.bvh_aabbs, "BVH AABBs");
        ui.checkbox(&mut self.sleep_state, "Sleep state");
        ui.checkbox(&mut self.force_accumulators, "Force accumulators");
        ui.separator();
        if ui.button("Clear all").clicked() {
            *self = Overlays::default();
        }
    }
}
