//! Bridge between the physics crate's per-frame [`DebugDraw`] and the renderer's
//! [`DebugPass`](elderforge_renderer::DebugPass). The world emits debug geometry
//! as world-space lines/points; this turns those into the renderer's flat
//! [`DebugVertex`] arrays (two vertices per line, one per point), reusing all
//! backing buffers so nothing allocates per frame in steady state.
//!
//! Mirrors [`DeformableMeshes`](crate::deformable::DeformableMeshes): a small
//! piece of glue that lives in the binary's library half because it depends on
//! both the physics and renderer crates, which neither crate may depend on.

use elderforge_physics::{DebugDraw, DebugLayers, PhysicsWorld};
use elderforge_renderer::DebugVertex;

/// Owns the physics-side debug buffer and the renderer-side vertex lists, all
/// reused across frames.
#[derive(Default)]
pub struct DebugOverlay {
    draw: DebugDraw,
    lines: Vec<DebugVertex>,
    points: Vec<DebugVertex>,
}

impl DebugOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    /// Refill the overlay vertex lists from `world`'s current state for the
    /// enabled `layers`. Clears and reuses the backing buffers; an all-off
    /// [`DebugLayers`] leaves them empty almost for free.
    pub fn update(&mut self, world: &PhysicsWorld, layers: DebugLayers) {
        world.emit_debug(layers, &mut self.draw);
        self.lines.clear();
        for line in &self.draw.lines {
            self.lines.push(DebugVertex::new(line.start.to_array(), line.color));
            self.lines.push(DebugVertex::new(line.end.to_array(), line.color));
        }
        self.points.clear();
        for point in &self.draw.points {
            self.points.push(DebugVertex::new(point.position.to_array(), point.color));
        }
    }

    /// Line-segment vertices (two per segment), for the line-list pipeline.
    pub fn lines(&self) -> &[DebugVertex] {
        &self.lines
    }

    /// Point vertices (one per marker), for the point-list pipeline.
    pub fn points(&self) -> &[DebugVertex] {
        &self.points
    }

    /// Whether there is any overlay geometry to draw this frame.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.points.is_empty()
    }
}
