//! Load scenes from the .escene JSON format.

use std::path::Path;

use crate::{Scene, SceneError};

/// Load a scene from a `.escene` JSON file.
pub fn load_scene(path: &Path) -> Result<Scene, SceneError> {
    let text = std::fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Err(SceneError::Parse("empty scene file".to_string()));
    }
    // TODO: parse entities/components from JSON and spawn them into the
    // world (needs a serde_json workspace dependency).
    let mut scene = Scene::new();
    scene.name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("untitled")
        .to_string();
    log::info!("loaded scene '{}' from {}", scene.name, path.display());
    Ok(scene)
}
