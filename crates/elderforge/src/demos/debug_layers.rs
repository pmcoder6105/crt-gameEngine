//! Debug-capture demo: the mixed scene revealing one overlay layer at a time,
//! cumulatively, on a timer.
//!
//! This reuses [`mixed`](super::mixed) — a soft ball plowing into a rigid box
//! stack beside a wind-blown cloth flag — and starts with no overlays at all.
//! Every four seconds one more layer switches on and *stays* on, in the order
//! collision wireframes → velocity vectors → contact normals → constraint
//! anchors → BVH AABBs, so by twenty seconds the full stack is visible. The
//! reveal is fully automatic ([`DebugScript::Cumulative`]); no input required.

use elderforge_scene::Scene;

use super::DebugLayer::{
    BvhAabbs, CollisionShapes, ConstraintAnchors, ContactPoints, VelocityVectors,
};
use super::{DebugScript, DemoAssets, DemoConfig};

/// Seconds between each layer switching on.
const INTERVAL: f32 = 4.0;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) -> DemoConfig {
    super::mixed::setup(scene, assets);
    DemoConfig {
        debug: DebugScript::Cumulative {
            order: vec![
                CollisionShapes,
                VelocityVectors,
                ContactPoints,
                ConstraintAnchors,
                BvhAabbs,
            ],
            interval: INTERVAL,
        },
        ..DemoConfig::default()
    }
}
