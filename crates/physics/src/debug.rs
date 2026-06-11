//! Debug draw data emitted each frame, consumed by the renderer's debug pass.
//! Layers map 1:1 to the editor's physics overlay toggles.

use elderforge_core::math::Vec3;

/// One debug line in world space with an RGBA color.
#[derive(Debug, Clone, Copy)]
pub struct DebugLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: [f32; 4],
}

/// Which overlay a primitive belongs to; toggled at runtime from the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugLayer {
    CollisionShapes,
    VelocityVectors,
    AngularMomentum,
    ConstraintAnchors,
    SleepState,
    BroadphaseAabb,
    ContactNormals,
}

/// Debug draw data for one frame. Cleared and refilled every physics step.
#[derive(Debug, Default, Clone)]
pub struct DebugDraw {
    pub lines: Vec<(DebugLayer, DebugLine)>,
}

impl DebugDraw {
    pub fn line(&mut self, layer: DebugLayer, start: Vec3, end: Vec3, color: [f32; 4]) {
        self.lines.push((layer, DebugLine { start, end, color }));
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_accumulate_and_clear() {
        let mut draw = DebugDraw::default();
        draw.line(
            DebugLayer::VelocityVectors,
            Vec3::ZERO,
            Vec3::Y,
            [0.0, 1.0, 0.0, 1.0],
        );
        assert_eq!(draw.lines.len(), 1);
        assert_eq!(draw.lines[0].0, DebugLayer::VelocityVectors);
        draw.clear();
        assert!(draw.lines.is_empty());
    }
}
