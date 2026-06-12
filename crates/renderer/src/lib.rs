//! wgpu render pipeline: PBR with IBL, cascaded shadow maps, and a separate
//! debug pass for physics visualization overlays. WGSL shaders only.

pub mod cache;
pub mod camera;
pub mod context;
pub mod material;
pub mod mesh;
pub mod passes;
pub mod pipeline;
pub mod texture;

use thiserror::Error;

pub use context::{FrameContext, RenderContext};
pub use mesh::{GpuMesh, Vertex};

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("no suitable GPU adapter found")]
    NoAdapter,
    #[error("failed to request device: {0}")]
    DeviceRequest(String),
    #[error("surface error: {0}")]
    Surface(String),
}
