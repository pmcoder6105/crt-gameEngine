//! wgpu render pipeline: PBR with IBL, cascaded shadow maps, and a separate
//! debug pass for physics visualization overlays. WGSL shaders only.

pub mod cache;
pub mod camera;
pub mod context;
pub mod material;
pub mod mesh;
pub mod passes;
pub mod pipeline;
pub mod primitives;
pub mod texture;

use thiserror::Error;

pub use cache::ResourceCache;
pub use camera::Camera;
pub use context::{FrameContext, RenderContext};
pub use mesh::{GpuMesh, Vertex};
pub use passes::debug::{DebugPass, DebugVertex};
pub use passes::forward::{DirectionalLight, Draw, ForwardPass};
pub use texture::GpuTexture;

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("no suitable GPU adapter found")]
    NoAdapter,
    #[error("failed to request device: {0}")]
    DeviceRequest(String),
    #[error("surface error: {0}")]
    Surface(String),
}
