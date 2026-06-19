//! Serialize scenes to the `.escene` JSON format.

use std::path::Path;

use elderforge_ecs::components::{Camera, Collider, Joint, MeshRenderer, PhysicsBody, Transform};
use elderforge_ecs::Entity;

use crate::format::{
    AssetsDoc, EntityDoc, PhysicsDoc, RigidBodyDoc, SceneDoc, FORMAT_VERSION,
};
use crate::{Scene, SceneError};

/// Serialize a scene to `.escene` JSON at `path` (pretty-printed).
pub fn save_scene(scene: &Scene, path: &Path) -> Result<(), SceneError> {
    let doc = scene_to_doc(scene);
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| SceneError::Parse(format!("serialize scene: {e}")))?;
    std::fs::write(path, json)?;
    log::info!(
        "saved scene '{}' ({} entities, {} bodies) to {}",
        scene.name,
        doc.entities.len(),
        doc.physics.bodies.len(),
        path.display()
    );
    Ok(())
}

/// Build the serializable document for a scene. Public so callers can inspect or
/// re-serialize a scene without touching the filesystem (and for testing).
pub fn scene_to_doc(scene: &Scene) -> SceneDoc {
    let physics = PhysicsDoc {
        gravity: scene.physics.gravity,
        substeps: scene.physics.substeps,
        iterations: scene.physics.iterations,
        bodies: scene
            .physics
            .bodies()
            .iter()
            .map(RigidBodyDoc::from_body)
            .collect(),
    };

    // Collect entity ids first so we don't hold the iterator's borrow while
    // issuing per-component `get`s below.
    let entities: Vec<Entity> = scene.world.iter().map(|e| e.entity()).collect();
    let entities = entities
        .into_iter()
        .map(|entity| EntityDoc {
            transform: scene.world.get::<&Transform>(entity).ok().map(|c| *c),
            physics_body: scene.world.get::<&PhysicsBody>(entity).ok().map(|c| *c),
            mesh_renderer: scene.world.get::<&MeshRenderer>(entity).ok().map(|c| *c),
            collider: scene.world.get::<&Collider>(entity).ok().map(|c| (*c).clone()),
            joint: scene.world.get::<&Joint>(entity).ok().map(|c| *c),
            camera: scene.world.get::<&Camera>(entity).ok().map(|c| *c),
        })
        // Drop entities with no serializable components (hecs has none, but be
        // robust to future bookkeeping-only entities).
        .filter(|doc| !doc.is_empty())
        .collect();

    SceneDoc {
        version: FORMAT_VERSION,
        name: scene.name.clone(),
        physics,
        assets: AssetsDoc::from_assets(&scene.assets),
        entities,
    }
}
