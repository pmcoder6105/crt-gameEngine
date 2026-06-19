//! Library half of the elderforge crate. Holds the demo scene definitions so
//! both the binary (which selects one via `--demo`) and the headless render
//! tests can build the exact same scenes. The event loop, `App`, and per-frame
//! systems live in the binary (`main.rs`).

pub mod demos;
