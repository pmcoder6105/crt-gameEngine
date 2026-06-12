//! Thin abstraction over winit: windowing, raw input, normalized events,
//! and the fixed-timestep clock. Nothing outside this crate touches winit
//! types directly.

pub mod event;
pub mod input;
pub mod time;
pub mod window;

pub use event::{EngineEvent, Key, KeyCode, MouseButton};
pub use input::InputState;
pub use window::{
    run_event_loop, FrameControl, SurfaceProvider, WindowConfig, WindowError, WindowHandle,
};
