//! Simulation controls: play / pause / single-step / rewind, timestep
//! multiplier, and substep count.

pub struct SimControls {
    pub playing: bool,
    /// Set for one frame when the user clicks Step while paused.
    pub single_step_requested: bool,
    pub timestep_multiplier: f32,
    pub substeps: u32,
}

impl Default for SimControls {
    fn default() -> Self {
        Self {
            // Start running so a freshly launched demo animates immediately;
            // the user can pause to confirm rendering continues while stopped.
            playing: true,
            single_step_requested: false,
            timestep_multiplier: 1.0,
            // Overwritten by the app with the loaded scene's substep count.
            substeps: 20,
        }
    }
}

impl SimControls {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .button(if self.playing { "Pause" } else { "Play" })
                .clicked()
            {
                self.playing = !self.playing;
            }
            if ui.button("Step").clicked() {
                self.single_step_requested = true;
            }
            if ui.button("Rewind").clicked() {
                // TODO: rewind needs physics state snapshots.
                log::warn!("rewind is not implemented yet");
            }
        });
        ui.add(
            egui::Slider::new(&mut self.timestep_multiplier, 0.1..=4.0)
                .text("Timestep multiplier"),
        );
        ui.add(egui::Slider::new(&mut self.substeps, 1..=32).text("Substeps"));
    }
}
