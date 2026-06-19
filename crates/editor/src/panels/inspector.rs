//! Component inspector: view and edit components of the selected entity.

use elderforge_ecs::components::{PhysicsBody, Transform};
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

        // Edit the Transform in place; remember the result if the user touched it
        // so we can push it into the physics body afterwards.
        let mut edited: Option<Transform> = None;
        if let Ok(mut transform) = scene.world.get::<&mut Transform>(entity) {
            ui.separator();
            ui.label("Transform");

            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("position");
                changed |= drag(ui, &mut transform.position.x, "x");
                changed |= drag(ui, &mut transform.position.y, "y");
                changed |= drag(ui, &mut transform.position.z, "z");
            });
            ui.horizontal(|ui| {
                ui.label("scale   ");
                changed |= drag(ui, &mut transform.scale.x, "x");
                changed |= drag(ui, &mut transform.scale.y, "y");
                changed |= drag(ui, &mut transform.scale.z, "z");
            });

            // Rotation is read-only here (editing a quaternion component-wise is
            // unintuitive); show it as an axis-angle for orientation feedback.
            let (axis, angle) = transform.rotation.to_axis_angle();
            ui.label(format!(
                "rotation  {:.1}° @ ({:.2}, {:.2}, {:.2})",
                angle.to_degrees(),
                axis.x,
                axis.y,
                axis.z
            ));

            if changed {
                edited = Some(*transform);
            }
        } else {
            ui.label("(no Transform component)");
        }

        // If this entity is physics-driven, mirror the edit into its rigid body
        // (and wake it) — otherwise the solver overwrites the transform on the
        // next step and the edit appears to do nothing.
        if let Some(transform) = edited {
            let body_handle = scene
                .world
                .get::<&PhysicsBody>(entity)
                .ok()
                .map(|pb| pb.handle);
            if let Some(handle) = body_handle {
                if let Some(body) = scene.physics.body_mut(handle) {
                    body.position = transform.position;
                    body.rotation = transform.rotation;
                    body.sleeping = false;
                }
            }
        }
        // TODO: PhysicsBody, MeshRenderer, Collider, Joint, Camera editors.
    }
}

/// A compact labelled `DragValue`; returns whether the user changed it.
fn drag(ui: &mut egui::Ui, value: &mut f32, prefix: &str) -> bool {
    ui.add(egui::DragValue::new(value).prefix(format!("{prefix}: ")).speed(0.05))
        .changed()
}
