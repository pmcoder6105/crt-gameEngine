//! Asset loading: meshes (.obj/.gltf) and textures (PNG/JPEG), plus the scene's
//! asset table that maps resource handles to stable, serializable sources.

pub mod mesh;
pub mod registry;
pub mod texture;

pub use mesh::MeshData;
pub use registry::{MaterialDef, MeshSource, SceneAssets, TextureSource};
pub use texture::TextureData;
