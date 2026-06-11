//! Thin abstraction over winit: windowing, raw input, normalized events,
//! and the fixed-timestep clock. Nothing outside this crate touches winit
//! types directly.

pub mod event;
pub mod input;
pub mod time;
pub mod window;
