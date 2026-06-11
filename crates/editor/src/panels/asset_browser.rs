//! Asset browser: browse meshes/textures and drag them into the scene.
//! // TODO: directory listing, thumbnails, drag-and-drop spawning.

#[derive(Default)]
pub struct AssetBrowser {
    pub root: String,
}

impl AssetBrowser {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Root:");
            ui.text_edit_singleline(&mut self.root);
        });
        ui.label("(asset listing not implemented yet)");
    }
}
