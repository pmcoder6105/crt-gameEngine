//! Mesh loading from .obj and .gltf.

use std::path::Path;

use elderforge_core::math::{Vec2, Vec3};

use crate::SceneError;

/// CPU-side mesh data, ready to upload through the renderer's ResourceCache.
#[derive(Debug, Clone, Default)]
pub struct MeshData {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub indices: Vec<u32>,
}

/// Load a mesh, picking the parser from the file extension.
pub fn load_mesh(path: &Path) -> Result<MeshData, SceneError> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("obj") => load_obj(path),
        Some("gltf") | Some("glb") => load_gltf(path),
        other => Err(SceneError::UnsupportedFormat(
            other.unwrap_or("<none>").to_string(),
        )),
    }
}

fn load_obj(path: &Path) -> Result<MeshData, SceneError> {
    let _text = std::fs::read_to_string(path)?;
    // TODO: parse OBJ positions/normals/uvs/faces.
    Ok(MeshData::default())
}

fn load_gltf(path: &Path) -> Result<MeshData, SceneError> {
    let _bytes = std::fs::read(path)?;
    // TODO: parse glTF buffers and mesh primitives.
    Ok(MeshData::default())
}
