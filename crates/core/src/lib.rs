//! Elderforge core: math (glam re-exports), type-safe handles, logging,
//! profiling, and fixed-timestep accumulation. Every other crate in the
//! workspace depends on this one.

pub mod handles;
pub mod logging;
pub mod math;
pub mod profiling;
pub mod time;

pub use handles::{Handle, HandleAllocator, MaterialHandle, MeshHandle, TextureHandle};
pub use logging::init_logging;
pub use profiling::TimingScope;
pub use time::FixedTimestep;
