//! Load scenes from the `.escene` JSON format.

use std::path::Path;

use elderforge_ecs::EntityBuilder;

use crate::format::SceneDoc;
use crate::{Scene, SceneError};

/// Load a scene from a `.escene` JSON file.
///
/// The returned scene is fully reconstructed except for GPU resources: its
/// `assets` table names every mesh/texture/material the entities reference, but
/// uploading those into the renderer's cache is the caller's job (the app does
/// this in its asset-realization step).
pub fn load_scene(path: &Path) -> Result<Scene, SceneError> {
    let text = std::fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Err(SceneError::Parse("empty scene file".to_string()));
    }
    let doc: SceneDoc = serde_json::from_str(&text)
        .map_err(|e| SceneError::Parse(format!("parse '{}': {e}", path.display())))?;
    let scene = scene_from_doc(doc)?;
    log::info!("loaded scene '{}' from {}", scene.name, path.display());
    Ok(scene)
}

/// Reconstruct a live [`Scene`] from a parsed document. Public so callers can
/// build a scene from an in-memory document (and for testing).
pub fn scene_from_doc(doc: SceneDoc) -> Result<Scene, SceneError> {
    if doc.version != crate::format::FORMAT_VERSION {
        return Err(SceneError::Parse(format!(
            "unsupported .escene version {} (this build reads version {})",
            doc.version,
            crate::format::FORMAT_VERSION
        )));
    }

    let mut scene = Scene::new();
    scene.name = doc.name;

    // Physics: world config, then bodies in handle-index order (so each body
    // lands at the index its serialized handle referenced).
    scene.physics.gravity = doc.physics.gravity;
    scene.physics.substeps = doc.physics.substeps;
    scene.physics.iterations = doc.physics.iterations;
    for body in doc.physics.bodies {
        scene.physics.add_rigid_body(body.into_body());
    }

    // Assets: list position is the handle index, matching what was written.
    scene.assets = doc.assets.into_assets();

    // Entities: spawn each with exactly the components it carried.
    for entity in doc.entities {
        let mut builder = EntityBuilder::new();
        if let Some(transform) = entity.transform {
            builder.add(transform);
        }
        if let Some(physics_body) = entity.physics_body {
            builder.add(physics_body);
        }
        if let Some(mesh_renderer) = entity.mesh_renderer {
            builder.add(mesh_renderer);
        }
        if let Some(collider) = entity.collider {
            builder.add(collider);
        }
        if let Some(joint) = entity.joint {
            builder.add(joint);
        }
        if let Some(camera) = entity.camera {
            builder.add(camera);
        }
        scene.world.spawn(builder.build());
    }

    Ok(scene)
}
