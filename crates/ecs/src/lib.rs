//! hecs wrapper and all engine component definitions.
//!
//! Systems are plain functions that take `&mut World`; there is no global
//! state — the world is always passed through explicitly.

pub mod components;

pub use hecs::{Entity, EntityBuilder, World};
