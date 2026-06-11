//! Component inspector: view and edit components of the selected entity.

use elderforge_ecs::components::Transform;
use elderforge_ecs::Entity;
use elderforge_scene::Scene;

#[derive(Default)]
pub struct Inspector {
    pub selected: Option<Entity>,
}

impl Inspector {
    pub fn ui(&mut self, ui: &mut egui::Ui, scene: &mut Scene) {
        let Some(entity) = self.selected else {
            ui.label("No entity selected");
            return;
        };
        ui.label(format!("Entity {:?}", entity));

        if let Ok(mut transform) = scene.world.get::<&mut Transform>(entity) {
            ui.separator();
            ui.label("Transform");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut transform.position.x).prefix("x: ").speed(0.05));
                ui.add(egui::DragValue::new(&mut transform.position.y).prefix("y: ").speed(0.05));
                ui.add(egui::DragValue::new(&mut transform.position.z).prefix("z: ").speed(0.05));
            });
        }
        // TODO: PhysicsBody, MeshRenderer, Collider, Joint, Camera editors.
    }
}
