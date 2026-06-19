//! Engine systems, run once per frame by the App.
//!
//! The editor's egui pass is driven directly by [`App`](crate::app::App) (it
//! interleaves with the 3D pass inside one surface frame), so there is no
//! standalone editor system.

pub mod physics;
pub mod render;
