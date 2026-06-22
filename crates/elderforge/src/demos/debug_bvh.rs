//! Debug-capture demo: the avalanche, seen through *only* the BVH overlay.
//!
//! This reuses [`avalanche`](super::avalanche) — 200 spheres pouring down a
//! ramp and piling against a wall — and turns on a single overlay layer: the
//! broadphase BVH AABBs, colored from red at the root to blue at the leaves.
//! With nothing else drawn, the capture is purely the broadphase tree
//! restructuring frame to frame as the cloud of bodies moves: nodes splitting,
//! merging, and refitting around the avalanche.

use elderforge_physics::DebugLayers;
use elderforge_scene::Scene;

use super::{DebugScript, DemoAssets, DemoConfig};

pub fn setup(scene: &mut Scene, assets: &DemoAssets) -> DemoConfig {
    super::avalanche::setup(scene, assets);
    DemoConfig {
        debug: DebugScript::Always(DebugLayers { bvh_aabbs: true, ..DebugLayers::default() }),
        ..DemoConfig::default()
    }
}
