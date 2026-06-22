//! Debug-capture demo: the mixed scene cycling through each overlay layer
//! alone, one at a time.
//!
//! This reuses [`mixed`](super::mixed) — the same soft ball, rigid box stack,
//! and wind-blown flag as [`debug_layers`](super::debug_layers) — but instead
//! of accumulating layers it shows exactly one at a time, advancing every four
//! seconds and wrapping around ([`DebugScript::Solo`]). Each layer gets a clean
//! isolated four-second shot with nothing else drawn, which is what a video edit
//! wants for cutting between individual visualizations.
//!
//! All eight layers are cycled. On this settled, linear-contact scene the
//! angular-velocity and force-accumulator shots are sparse (contacts impart no
//! spin, and a body's force arrow vanishes once it sleeps) — an honest
//! reflection of the solver; trim them in the edit if undesired.

use elderforge_scene::Scene;

use super::DebugLayer::{
    AngularVelocity, BvhAabbs, CollisionShapes, ConstraintAnchors, ContactPoints,
    ForceAccumulators, SleepState, VelocityVectors,
};
use super::{DebugScript, DemoAssets, DemoConfig};

/// Seconds each layer is shown before advancing to the next.
const INTERVAL: f32 = 4.0;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) -> DemoConfig {
    super::mixed::setup(scene, assets);
    DemoConfig {
        debug: DebugScript::Solo {
            // Populated layers first, so the cycle opens strong.
            order: vec![
                CollisionShapes,
                SleepState,
                VelocityVectors,
                ContactPoints,
                ConstraintAnchors,
                BvhAabbs,
                AngularVelocity,
                ForceAccumulators,
            ],
            interval: INTERVAL,
        },
        ..DemoConfig::default()
    }
}
