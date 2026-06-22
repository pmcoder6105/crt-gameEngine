//! Library half of the elderforge crate. Holds the demo scene definitions and
//! the app-level asset realization (scene asset table → GPU cache) so both the
//! binary (which selects a demo via `--demo`) and the headless render/IO tests
//! share the exact same code. The event loop, `App`, and per-frame systems live
//! in the binary (`main.rs`).

pub mod assets;
pub mod debug_overlay;
pub mod deformable;
pub mod demos;
