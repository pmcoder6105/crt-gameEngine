//! egui editor: scene hierarchy, component inspector, simulation controls,
//! physics overlay toggles, stats, and asset browser.
//!
//! [`Editor`] is the pure-egui UI (panels over a [`Scene`](elderforge_scene));
//! [`EditorState`] wraps it with the `egui_winit` + `egui_wgpu` plumbing that
//! lets it run inside the windowed app.

pub mod editor;
pub mod panels;
pub mod state;

pub use editor::Editor;
pub use state::{EditorFrame, EditorState, EditorStats};
