//! Scene graph, serialization (.escene JSON), and asset loading.

pub mod assets;
pub mod format;
pub mod loader;
pub mod scene;
pub mod serializer;

pub use scene::Scene;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SceneError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("unsupported asset format: {0}")]
    UnsupportedFormat(String),
}
