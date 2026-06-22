//! Per-frame physics debug-draw data, consumed by the renderer's debug overlay
//! pass. The world fills a [`DebugDraw`] each frame via
//! [`PhysicsWorld::emit_debug`](crate::PhysicsWorld::emit_debug); the layers it
//! emits map 1:1 to the editor's overlay toggles ([`DebugLayers`]).
//!
//! Everything reduces to two GPU-friendly primitive kinds — **line segments**
//! (wireframes, vectors, arrows, arcs, AABBs, connections) and **points**
//! (contact / anchor markers) — so the renderer needs only a line-list and a
//! point-list pipeline. The [`DebugDraw`] buffers are cleared and refilled in
//! place each frame, reusing their capacity, so steady-state emission does not
//! allocate.

use elderforge_core::math::{Quat, Vec3};

/// One debug line segment in world space with an RGBA color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DebugLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: [f32; 4],
}

/// One debug point in world space with an RGBA color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DebugPoint {
    pub position: Vec3,
    pub color: [f32; 4],
}

/// Which debug overlay layers to emit. Mirrors the editor's overlay toggles
/// 1:1. [`emit_debug`](crate::PhysicsWorld::emit_debug) only fills the enabled
/// layers, so a disabled layer costs nothing (no geometry, no broadphase).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DebugLayers {
    /// Collider wireframes, colored by body kind.
    pub collision_shapes: bool,
    /// Linear-velocity arrows, length scaled by speed.
    pub velocity_vectors: bool,
    /// Angular-velocity arcs around each body's spin axis.
    pub angular_velocity: bool,
    /// Contact points (markers) and contact normals (arrows).
    pub contact_points: bool,
    /// Joint / distance-constraint anchors (markers) and their connections.
    pub constraint_anchors: bool,
    /// Broadphase BVH node AABBs, colored by tree depth.
    pub bvh_aabbs: bool,
    /// Collider wireframes colored by sleep state (sleeping bodies dimmed).
    pub sleep_state: bool,
    /// Net external-force arrows from each body's center of mass.
    pub force_accumulators: bool,
}

impl DebugLayers {
    /// Whether any layer is enabled.
    pub fn any(&self) -> bool {
        self.collision_shapes
            || self.velocity_vectors
            || self.angular_velocity
            || self.contact_points
            || self.constraint_anchors
            || self.bvh_aabbs
            || self.sleep_state
            || self.force_accumulators
    }
}

/// Accumulated debug geometry for one frame: a flat list of line segments and a
/// flat list of points, each already filtered to the enabled layers. Cleared
/// and refilled in place every frame (capacity is retained).
#[derive(Debug, Default, Clone)]
pub struct DebugDraw {
    pub lines: Vec<DebugLine>,
    pub points: Vec<DebugPoint>,
}

/// Segments per full circle / arc sweep in wireframe primitives — coarse enough
/// to stay cheap with hundreds of bodies, fine enough to read as round.
const CIRCLE_SEGMENTS: usize = 20;

impl DebugDraw {
    /// Drop all geometry, keeping the backing capacity for reuse next frame.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.points.clear();
    }

    /// Total primitive count (lines + points), for tests and diagnostics.
    pub fn len(&self) -> usize {
        self.lines.len() + self.points.len()
    }

    /// Whether anything was emitted.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.points.is_empty()
    }

    /// Push one line segment.
    pub fn line(&mut self, start: Vec3, end: Vec3, color: [f32; 4]) {
        self.lines.push(DebugLine { start, end, color });
    }

    /// Push one point.
    pub fn point(&mut self, position: Vec3, color: [f32; 4]) {
        self.points.push(DebugPoint { position, color });
    }

    /// Wireframe of an oriented box: 12 edges of the box centered at `center`
    /// with the given half-extents, rotated by `rotation`.
    pub fn wire_box(&mut self, center: Vec3, half: Vec3, rotation: Quat, color: [f32; 4]) {
        // 8 corners by sign pattern.
        let corner = |sx: f32, sy: f32, sz: f32| {
            center + rotation * Vec3::new(sx * half.x, sy * half.y, sz * half.z)
        };
        let c = [
            corner(-1.0, -1.0, -1.0),
            corner(1.0, -1.0, -1.0),
            corner(1.0, 1.0, -1.0),
            corner(-1.0, 1.0, -1.0),
            corner(-1.0, -1.0, 1.0),
            corner(1.0, -1.0, 1.0),
            corner(1.0, 1.0, 1.0),
            corner(-1.0, 1.0, 1.0),
        ];
        // Bottom ring, top ring, verticals.
        const EDGES: [(usize, usize); 12] = [
            (0, 1), (1, 2), (2, 3), (3, 0),
            (4, 5), (5, 6), (6, 7), (7, 4),
            (0, 4), (1, 5), (2, 6), (3, 7),
        ];
        for (a, b) in EDGES {
            self.line(c[a], c[b], color);
        }
    }

    /// Wireframe of an axis-aligned box from `min` to `max`.
    pub fn wire_aabb(&mut self, min: Vec3, max: Vec3, color: [f32; 4]) {
        let center = (min + max) * 0.5;
        let half = (max - min) * 0.5;
        self.wire_box(center, half, Quat::IDENTITY, color);
    }

    /// Wireframe sphere: three orthogonal great circles.
    pub fn wire_sphere(&mut self, center: Vec3, radius: f32, color: [f32; 4]) {
        self.circle(center, Vec3::X * radius, Vec3::Y * radius, color);
        self.circle(center, Vec3::Y * radius, Vec3::Z * radius, color);
        self.circle(center, Vec3::Z * radius, Vec3::X * radius, color);
    }

    /// Wireframe of a Y-aligned capsule (segment of half-length `half_height`
    /// swept by `radius`), oriented by `rotation`.
    pub fn wire_capsule(
        &mut self,
        center: Vec3,
        radius: f32,
        half_height: f32,
        rotation: Quat,
        color: [f32; 4],
    ) {
        let up = rotation * Vec3::Y;
        let x = rotation * Vec3::X;
        let z = rotation * Vec3::Z;
        let top = center + up * half_height;
        let bot = center - up * half_height;
        // Cap rings (in the local XZ plane) and four side seams.
        self.circle(top, x * radius, z * radius, color);
        self.circle(bot, x * radius, z * radius, color);
        for dir in [x, -x, z, -z] {
            self.line(top + dir * radius, bot + dir * radius, color);
        }
        // Hemispherical cap outlines: half circles in the two vertical planes.
        self.half_arc(top, x * radius, up * radius, color);
        self.half_arc(top, z * radius, up * radius, color);
        self.half_arc(bot, x * radius, -up * radius, color);
        self.half_arc(bot, z * radius, -up * radius, color);
    }

    /// An arrow from `start` to `end` with a small two-line head at the tip.
    pub fn arrow(&mut self, start: Vec3, end: Vec3, color: [f32; 4]) {
        self.line(start, end, color);
        let dir = end - start;
        let len = dir.length();
        if len < 1e-5 {
            return;
        }
        let d = dir / len;
        let head = (0.25 * len).min(0.15);
        // Two barbs in an arbitrary plane perpendicular to the shaft.
        let side = perpendicular(d);
        let back = end - d * head;
        self.line(end, back + side * (head * 0.5), color);
        self.line(end, back - side * (head * 0.5), color);
    }

    /// An arc of `sweep` radians around `axis`, centered at `center` with the
    /// given `radius`, starting from a reference perpendicular to the axis. A
    /// small arrowhead at the end shows the spin direction.
    pub fn arc(&mut self, center: Vec3, axis: Vec3, radius: f32, sweep: f32, color: [f32; 4]) {
        let n = axis.normalize_or_zero();
        if n == Vec3::ZERO || radius < 1e-5 {
            return;
        }
        let u = perpendicular(n) * radius;
        let v = n.cross(u.normalize_or_zero()) * radius;
        let steps = ((CIRCLE_SEGMENTS as f32 * sweep / std::f32::consts::TAU).ceil() as usize).max(2);
        let mut prev = center + u;
        for i in 1..=steps {
            let t = sweep * i as f32 / steps as f32;
            let p = center + u * t.cos() + v * t.sin();
            self.line(prev, p, color);
            prev = p;
        }
        // Arrowhead at the arc tip, tangent to the circle.
        let tangent = (-u * sweep.sin() + v * sweep.cos()).normalize_or_zero();
        let head = (radius * 0.3).min(0.12);
        self.line(prev, prev - tangent * head + n * (head * 0.4), color);
        self.line(prev, prev - tangent * head - n * (head * 0.4), color);
    }

    /// A small cube marker plus a center point — used for constraint anchors.
    pub fn marker(&mut self, center: Vec3, size: f32, color: [f32; 4]) {
        self.wire_box(center, Vec3::splat(size), Quat::IDENTITY, color);
        self.point(center, color);
    }

    /// One closed circle through `center` spanned by the two radius vectors
    /// `u` and `v` (which encode the plane and radius).
    fn circle(&mut self, center: Vec3, u: Vec3, v: Vec3, color: [f32; 4]) {
        let mut prev = center + u;
        for i in 1..=CIRCLE_SEGMENTS {
            let t = std::f32::consts::TAU * i as f32 / CIRCLE_SEGMENTS as f32;
            let p = center + u * t.cos() + v * t.sin();
            self.line(prev, p, color);
            prev = p;
        }
    }

    /// Half circle (π sweep) from `+u` to `+v`, for capsule cap outlines.
    fn half_arc(&mut self, center: Vec3, u: Vec3, v: Vec3, color: [f32; 4]) {
        let half = CIRCLE_SEGMENTS / 2;
        let mut prev = center + u;
        for i in 1..=half {
            let t = std::f32::consts::PI * i as f32 / half as f32;
            let p = center + u * t.cos() + v * t.sin();
            self.line(prev, p, color);
            prev = p;
        }
    }
}

/// Some unit vector perpendicular to `n` (assumed roughly unit). Picks the axis
/// least aligned with `n` to stay numerically stable.
fn perpendicular(n: Vec3) -> Vec3 {
    let a = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    (a - n * a.dot(n)).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_keeps_capacity_and_empties() {
        let mut draw = DebugDraw::default();
        draw.line(Vec3::ZERO, Vec3::Y, [1.0; 4]);
        draw.point(Vec3::X, [1.0; 4]);
        assert_eq!(draw.len(), 2);
        let cap = draw.lines.capacity();
        draw.clear();
        assert!(draw.is_empty());
        // Reused, not reallocated.
        assert_eq!(draw.lines.capacity(), cap);
    }

    #[test]
    fn wire_box_has_twelve_edges() {
        let mut draw = DebugDraw::default();
        draw.wire_box(Vec3::ZERO, Vec3::splat(1.0), Quat::IDENTITY, [1.0; 4]);
        assert_eq!(draw.lines.len(), 12);
    }

    #[test]
    fn wire_aabb_matches_corners() {
        let mut draw = DebugDraw::default();
        draw.wire_aabb(Vec3::splat(-1.0), Vec3::splat(1.0), [1.0; 4]);
        assert_eq!(draw.lines.len(), 12);
        // Every endpoint is a corner of the box.
        for l in &draw.lines {
            for p in [l.start, l.end] {
                assert!(p.x.abs() == 1.0 && p.y.abs() == 1.0 && p.z.abs() == 1.0);
            }
        }
    }

    #[test]
    fn layers_any_reflects_toggles() {
        assert!(!DebugLayers::default().any());
        let mut l = DebugLayers::default();
        l.bvh_aabbs = true;
        assert!(l.any());
    }

    #[test]
    fn marker_emits_a_point() {
        let mut draw = DebugDraw::default();
        draw.marker(Vec3::ZERO, 0.1, [1.0; 4]);
        assert_eq!(draw.points.len(), 1);
        assert_eq!(draw.lines.len(), 12); // the cube
    }
}
