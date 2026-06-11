//! Per-layer physics debug overlay toggles. Mirrors the physics crate's
//! DebugLayer enum.

#[derive(Default)]
pub struct Overlays {
    pub collision_shapes: bool,
    pub velocity_vectors: bool,
    pub angular_momentum: bool,
    pub constraint_anchors: bool,
    pub sleep_state: bool,
    pub broadphase_aabb: bool,
    pub contact_normals: bool,
}

impl Overlays {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.collision_shapes, "Collision shapes");
        ui.checkbox(&mut self.velocity_vectors, "Velocity vectors");
        ui.checkbox(&mut self.angular_momentum, "Angular momentum");
        ui.checkbox(&mut self.constraint_anchors, "Constraint anchors");
        ui.checkbox(&mut self.sleep_state, "Sleep state");
        ui.checkbox(&mut self.broadphase_aabb, "Broadphase AABB");
        ui.checkbox(&mut self.contact_normals, "Contact normals");
    }
}
