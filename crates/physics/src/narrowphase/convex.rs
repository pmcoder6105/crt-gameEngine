//! Convex shapes expressed through a support function, for GJK/EPA.
//!
//! A shape is modelled as a convex **core** (returned by [`ConvexShape::support`])
//! plus a rounding **margin** ([`ConvexShape::margin`]). A sphere is a point with
//! margin = radius; a capsule is a segment with margin = radius; boxes and convex
//! hulls are exact polytopes with zero margin. Running GJK/EPA on the cores and
//! folding the margins back in afterward keeps rounded-shape contacts analytically
//! exact and reserves the polytope expansion for genuinely faceted shapes.

use elderforge_core::math::{Quat, Vec3};

use crate::shapes::{BoxShape, Capsule, ConvexHull, Sphere};

/// Rigid placement of a shape: a translation and rotation.
#[derive(Debug, Clone, Copy)]
pub struct Pose {
    pub position: Vec3,
    pub orientation: Quat,
}

impl Pose {
    pub fn new(position: Vec3, orientation: Quat) -> Self {
        Self { position, orientation }
    }

    pub fn from_position(position: Vec3) -> Self {
        Self { position, orientation: Quat::IDENTITY }
    }
}

impl Default for Pose {
    fn default() -> Self {
        Self::from_position(Vec3::ZERO)
    }
}

/// A convex shape that can answer support queries on its core.
pub trait ConvexShape {
    /// Farthest point of the convex core along `dir`, in the shape's local
    /// frame. `dir` need not be normalized.
    fn support(&self, dir: Vec3) -> Vec3;

    /// Rounding radius wrapped around the core. Zero for exact polytopes.
    fn margin(&self) -> f32 {
        0.0
    }
}

impl ConvexShape for Sphere {
    fn support(&self, _dir: Vec3) -> Vec3 {
        // The core of a sphere is its center point.
        Vec3::ZERO
    }
    fn margin(&self) -> f32 {
        self.radius
    }
}

impl ConvexShape for BoxShape {
    fn support(&self, dir: Vec3) -> Vec3 {
        // Farthest corner: pick the sign of each axis from `dir`.
        self.half_extents * dir.signum()
    }
}

impl ConvexShape for Capsule {
    fn support(&self, dir: Vec3) -> Vec3 {
        // Core is the central segment along local Y; margin is the radius.
        Vec3::new(0.0, self.half_height.copysign(dir.y), 0.0)
    }
    fn margin(&self) -> f32 {
        self.radius
    }
}

impl ConvexShape for ConvexHull {
    fn support(&self, dir: Vec3) -> Vec3 {
        let mut best = Vec3::ZERO;
        let mut best_dot = f32::NEG_INFINITY;
        for &p in &self.points {
            let d = p.dot(dir);
            if d > best_dot {
                best_dot = d;
                best = p;
            }
        }
        best
    }
}

/// Support point on the Minkowski difference `A ⊖ B` (cores only), retaining
/// the world-space witness points on each shape for closest-point recovery.
#[derive(Debug, Clone, Copy)]
pub struct SupportPoint {
    /// Point on `A ⊖ B`: `a - b`.
    pub v: Vec3,
    /// Witness point on A's core (world space).
    pub a: Vec3,
    /// Witness point on B's core (world space).
    pub b: Vec3,
}

/// A convex shape held by value, dispatching the support function. Lets callers
/// (e.g. the physics world) turn a collider enum into a `ConvexShape` without
/// heap allocation.
#[derive(Debug, Clone, Copy)]
pub enum AnyShape {
    Sphere(Sphere),
    Cuboid(BoxShape),
    Capsule(Capsule),
}

impl ConvexShape for AnyShape {
    fn support(&self, dir: Vec3) -> Vec3 {
        match self {
            AnyShape::Sphere(s) => s.support(dir),
            AnyShape::Cuboid(b) => b.support(dir),
            AnyShape::Capsule(c) => c.support(dir),
        }
    }
    fn margin(&self) -> f32 {
        match self {
            AnyShape::Sphere(s) => s.margin(),
            AnyShape::Cuboid(b) => b.margin(),
            AnyShape::Capsule(c) => c.margin(),
        }
    }
}

/// World-space support point of a shape's core (plus its margin) along
/// world-space `dir`. Unlike the GJK-internal core support, this includes the
/// rounding margin, giving the true surface point — used for half-space
/// contacts where there's no second shape to run GJK against.
pub fn surface_support(shape: &dyn ConvexShape, pose: &Pose, dir: Vec3) -> Vec3 {
    let core = world_support(shape, pose, dir);
    let len = dir.length();
    if len > 1e-12 {
        core + dir * (shape.margin() / len)
    } else {
        core
    }
}

/// World-space support point of a shape's core along world-space `dir`.
fn world_support(shape: &dyn ConvexShape, pose: &Pose, dir: Vec3) -> Vec3 {
    let local_dir = pose.orientation.inverse() * dir;
    let local = shape.support(local_dir);
    pose.position + pose.orientation * local
}

/// Support point of the Minkowski difference of the two cores along `dir`.
pub fn minkowski_support(
    a: &dyn ConvexShape,
    pose_a: &Pose,
    b: &dyn ConvexShape,
    pose_b: &Pose,
    dir: Vec3,
) -> SupportPoint {
    let pa = world_support(a, pose_a, dir);
    let pb = world_support(b, pose_b, -dir);
    SupportPoint { v: pa - pb, a: pa, b: pb }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_support_picks_corner() {
        let b = BoxShape { half_extents: Vec3::new(1.0, 2.0, 3.0) };
        assert_eq!(b.support(Vec3::new(1.0, -1.0, 1.0)), Vec3::new(1.0, -2.0, 3.0));
    }

    #[test]
    fn sphere_core_is_a_point_with_radius_margin() {
        let s = Sphere { radius: 2.5 };
        assert_eq!(s.support(Vec3::X), Vec3::ZERO);
        assert_eq!(s.margin(), 2.5);
    }

    #[test]
    fn capsule_support_picks_segment_end() {
        let c = Capsule { radius: 0.5, half_height: 2.0 };
        assert_eq!(c.support(Vec3::new(0.1, 1.0, 0.0)), Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(c.support(Vec3::new(0.0, -3.0, 0.0)), Vec3::new(0.0, -2.0, 0.0));
        assert_eq!(c.margin(), 0.5);
    }

    #[test]
    fn world_support_applies_pose() {
        let b = BoxShape { half_extents: Vec3::ONE };
        let pose = Pose::from_position(Vec3::new(10.0, 0.0, 0.0));
        // Farthest point along +X is the +X face corner, translated by the pose.
        let p = world_support(&b, &pose, Vec3::X);
        assert_eq!(p.x, 11.0);
    }
}
