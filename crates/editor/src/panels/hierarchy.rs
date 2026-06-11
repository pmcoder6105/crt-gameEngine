//! Scene hierarchy: entity tree with selection.
//! // TODO: rename and delete entities.

use elderforge_ecs::Entity;
use elderforge_scene::Scene;

#[derive(Default)]
pub struct Hierarchy {
    pub selected: Option<Entity>,
}

impl Hierarchy {
    pub fn ui(&mut self, ui: &mut egui::Ui, scene: &mut Scene) {
        for entity_ref in scene.world.iter() {
            let entity = entity_ref.entity();
            let label = format!("Entity {:?}", entity);
            if ui
                .selectable_label(self.selected == Some(entity), label)
                .clicked()
            {
                self.selected = Some(entity);
            }
        }
    }
}
