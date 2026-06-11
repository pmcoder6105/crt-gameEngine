//! Serialize scenes to the .escene JSON format.

use std::path::Path;

use crate::{Scene, SceneError};

/// Serialize a scene to `.escene` JSON.
pub fn save_scene(scene: &Scene, path: &Path) -> Result<(), SceneError> {
    // TODO: serialize entities/components; this writes a valid empty scene.
    let json = format!(
        "{{\n  \"name\": \"{}\",\n  \"entities\": []\n}}\n",
        scene.name
    );
    std::fs::write(path, json)?;
    log::info!("saved scene '{}' to {}", scene.name, path.display());
    Ok(())
}
