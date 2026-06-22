//! Debug-capture demo: the cloth-drape showcase, lit up by its own constraint
//! web and a velocity vector on every particle.
//!
//! This reuses [`cloth_drape_showcase`](super::cloth_drape_showcase) verbatim —
//! same 40×40 sheet draping over a slowly turning cube, same warm key light,
//! same orbiting camera — and only adds a debug overlay schedule. With the
//! constraint-anchor and velocity-vector layers on, the dense grid of
//! structural / shear / bending springs renders as a glowing cyan wireframe and
//! every moving particle trails a velocity arrow, so the cloth itself becomes
//! the visualization.

use elderforge_physics::DebugLayers;
use elderforge_scene::Scene;

use super::{DebugScript, DemoAssets, DemoConfig};

pub fn setup(scene: &mut Scene, assets: &DemoAssets) -> DemoConfig {
    // Build the showcase scene + its camera orbit and warm light, then overlay
    // the particle constraint web and per-particle velocity arrows on top.
    let mut config = super::cloth_drape_showcase::setup(scene, assets);
    config.debug = DebugScript::Always(DebugLayers {
        constraint_anchors: true,
        velocity_vectors: true,
        ..DebugLayers::default()
    });
    config
}
