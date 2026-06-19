//! Top toolbar: scene file path plus Save / Load buttons.
//!
//! The panel only records *intent*: clicking a button sets `save_requested` /
//! `load_requested`, which the app consumes each frame and clears. Loading in
//! particular can't happen here — it replaces the whole scene and must re-upload
//! assets through the renderer's GPU cache, which the app owns — so the editor
//! hands the request up rather than acting on it directly. The app writes the
//! outcome back into `status` for display.

/// Where a freshly saved/loaded scene defaults to, relative to the working dir.
const DEFAULT_PATH: &str = "scene.escene";

pub struct Toolbar {
    /// The `.escene` file path the buttons act on.
    pub path: String,
    /// Set for one frame when the user clicks Save; the app clears it.
    pub save_requested: bool,
    /// Set for one frame when the user clicks Load; the app clears it.
    pub load_requested: bool,
    /// Result of the last save/load, filled in by the app and shown here.
    pub status: String,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self {
            path: DEFAULT_PATH.to_string(),
            save_requested: false,
            load_requested: false,
            status: String::new(),
        }
    }
}

impl Toolbar {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Scene:");
            ui.add(
                egui::TextEdit::singleline(&mut self.path)
                    .desired_width(220.0)
                    .hint_text("path/to/scene.escene"),
            );
            if ui.button("Save Scene").clicked() {
                self.save_requested = true;
            }
            if ui.button("Load Scene").clicked() {
                self.load_requested = true;
            }
            if !self.status.is_empty() {
                ui.separator();
                ui.label(&self.status);
            }
        });
    }
}
