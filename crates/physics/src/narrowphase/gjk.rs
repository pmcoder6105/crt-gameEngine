//! GJK on convex shape cores.
//!
//! Iterates support points on the Minkowski difference `A ⊖ B`, maintaining the
//! sub-simplex closest to the origin. If the origin lies outside the difference
//! the shapes are [separated](GjkOutcome::Separated) and we report the core
//! distance and closest points; if the simplex comes to enclose the origin the
//! cores [overlap](GjkOutcome::Penetrating) and the enclosing simplex is handed
//! to EPA.

use elderforge_core::math::Vec3;

use super::convex::{minkowski_support, ConvexShape, Pose, SupportPoint};

const MAX_ITERS: usize = 64;
const EPS: f32 = 1e-10;

/// Result of running GJK on two shape cores.
pub enum GjkOutcome {
    /// Cores are apart. `normal` points from A to B; `distance` is the gap
    /// between the cores (margins not yet applied).
    Separated {
        distance: f32,
        normal: Vec3,
        point_a: Vec3,
        point_b: Vec3,
    },
    /// Cores overlap; `simplex` encloses (or touches) the origin for EPA.
    Penetrating { simplex: Vec<SupportPoint> },
}

/// Run GJK between the cores of `a` and `b` at the given poses.
pub fn gjk(a: &dyn ConvexShape, pose_a: &Pose, b: &dyn ConvexShape, pose_b: &Pose) -> GjkOutcome {
    let support = |dir: Vec3| minkowski_support(a, pose_a, b, pose_b, dir);

    // Seed with a single support point.
    let mut dir = pose_b.position - pose_a.position;
    if dir.length_squared() < EPS {
        dir = Vec3::X;
    }
    let mut simplex = vec![support(dir)];
    let mut weights = vec![1.0f32];
    let mut closest = simplex[0].v;

    for _ in 0..MAX_ITERS {
        if closest.length_squared() < EPS {
            // Origin sits on the current simplex: the cores touch / overlap.
            return GjkOutcome::Penetrating { simplex };
        }
        let dir = -closest;
        let w = support(dir);

        // No support point lies further toward the origin: converged, apart.
        let progress = closest.length_squared() - closest.dot(w.v);
        if progress <= 1e-8 * closest.length_squared() {
            return separated(&simplex, &weights, closest);
        }
        // Guard against re-adding a vertex already in the simplex.
        if simplex.iter().any(|s| (s.v - w.v).length_squared() < EPS) {
            return separated(&simplex, &weights, closest);
        }

        simplex.push(w);
        match solve(&simplex) {
            Solved::Enclosed => return GjkOutcome::Penetrating { simplex },
            Solved::Closest { reduced, new_weights, point } => {
                simplex = reduced;
                weights = new_weights;
                closest = point;
            }
        }
    }
    separated(&simplex, &weights, closest)
}

/// Build a `Separated` outcome from the reduced simplex and its weights.
fn separated(simplex: &[SupportPoint], weights: &[f32], closest: Vec3) -> GjkOutcome {
    let mut point_a = Vec3::ZERO;
    let mut point_b = Vec3::ZERO;
    for (s, &w) in simplex.iter().zip(weights) {
        point_a += s.a * w;
        point_b += s.b * w;
    }
    let distance = closest.length();
    // `closest == point_a - point_b`, so the A->B normal is `point_b - point_a`.
    let normal = if distance > EPS {
        (point_b - point_a) / distance
    } else {
        Vec3::Y
    };
    GjkOutcome::Separated { distance, normal, point_a, point_b }
}

enum Solved {
    Enclosed,
    Closest {
        reduced: Vec<SupportPoint>,
        new_weights: Vec<f32>,
        point: Vec3,
    },
}

/// Reduce a 2/3/4-point simplex to the sub-feature closest to the origin.
fn solve(simplex: &[SupportPoint]) -> Solved {
    match simplex.len() {
        2 => from_indices(simplex, segment(simplex[0].v, simplex[1].v)),
        3 => from_indices(simplex, triangle(simplex[0].v, simplex[1].v, simplex[2].v)),
        4 => tetrahedron(simplex),
        _ => from_indices(simplex, (vec![0], vec![1.0], simplex[0].v)),
    }
}

/// Map `(indices, weights, closest)` back onto support points.
fn from_indices(
    simplex: &[SupportPoint],
    (idx, weights, point): (Vec<usize>, Vec<f32>, Vec3),
) -> Solved {
    Solved::Closest {
        reduced: idx.iter().map(|&i| simplex[i]).collect(),
        new_weights: weights,
        point,
    }
}

/// Closest feature of segment AB to the origin: indices into `[A, B]`, weights,
/// and the closest point.
fn segment(a: Vec3, b: Vec3) -> (Vec<usize>, Vec<f32>, Vec3) {
    let ab = b - a;
    let t = (-a).dot(ab);
    if t <= 0.0 {
        return (vec![0], vec![1.0], a);
    }
    let denom = ab.length_squared();
    if t >= denom {
        return (vec![1], vec![1.0], b);
    }
    let u = t / denom;
    (vec![0, 1], vec![1.0 - u, u], a + ab * u)
}

/// Closest feature of triangle ABC to the origin (Ericson, RTCD §5.1.5).
fn triangle(a: Vec3, b: Vec3, c: Vec3) -> (Vec<usize>, Vec<f32>, Vec3) {
    let ab = b - a;
    let ac = c - a;
    let ap = -a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (vec![0], vec![1.0], a);
    }
    let bp = -b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (vec![1], vec![1.0], b);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (vec![0, 1], vec![1.0 - v, v], a + ab * v);
    }
    let cp = -c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (vec![2], vec![1.0], c);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (vec![0, 2], vec![1.0 - w, w], a + ac * w);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (vec![1, 2], vec![1.0 - w, w], b + (c - b) * w);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    (vec![0, 1, 2], vec![1.0 - v - w, v, w], a + ab * v + ac * w)
}

/// Closest feature of tetrahedron ABCD to the origin, or `Enclosed`.
fn tetrahedron(s: &[SupportPoint]) -> Solved {
    let p = [s[0].v, s[1].v, s[2].v, s[3].v];
    // Faces as (triangle vertices, opposite vertex).
    let faces = [
        ([0usize, 1, 2], 3usize),
        ([0, 2, 3], 1),
        ([0, 3, 1], 2),
        ([1, 3, 2], 0),
    ];

    // A flat tetrahedron (e.g. the planar Minkowski difference of two segments)
    // has no interior, so it can never enclose the origin: pick the closest of
    // its faces directly. This avoids the unreliable side tests below.
    let volume = (p[1] - p[0]).dot((p[2] - p[0]).cross(p[3] - p[0]));
    if volume.abs() < 1e-9 {
        return closest_face(s, &faces, true);
    }

    let mut best: Option<(Vec<usize>, Vec<f32>, Vec3)> = None;
    let mut best_d2 = f32::INFINITY;
    let mut outside_any = false;

    for (tri, opp) in faces {
        if !origin_outside_face(p[tri[0]], p[tri[1]], p[tri[2]], p[opp]) {
            continue;
        }
        outside_any = true;
        let (idx, weights, point) = triangle(p[tri[0]], p[tri[1]], p[tri[2]]);
        let d2 = point.length_squared();
        if d2 < best_d2 {
            best_d2 = d2;
            // Remap local triangle indices (0..3) to the tetra's indices.
            let remapped = idx.iter().map(|&i| tri[i]).collect();
            best = Some((remapped, weights, point));
        }
    }

    if !outside_any {
        return Solved::Enclosed;
    }
    let (idx, weights, point) = best.expect("a face was outside");
    from_indices(s, (idx, weights, point))
}

/// Closest of the four faces to the origin. With `force = true` every face is
/// considered (used for degenerate tetrahedra).
fn closest_face(s: &[SupportPoint], faces: &[([usize; 3], usize); 4], force: bool) -> Solved {
    let p = [s[0].v, s[1].v, s[2].v, s[3].v];
    let mut best: Option<(Vec<usize>, Vec<f32>, Vec3)> = None;
    let mut best_d2 = f32::INFINITY;
    for &(tri, opp) in faces {
        if !force && !origin_outside_face(p[tri[0]], p[tri[1]], p[tri[2]], p[opp]) {
            continue;
        }
        let (idx, weights, point) = triangle(p[tri[0]], p[tri[1]], p[tri[2]]);
        let d2 = point.length_squared();
        if d2 < best_d2 {
            best_d2 = d2;
            best = Some((idx.iter().map(|&i| tri[i]).collect(), weights, point));
        }
    }
    match best {
        Some(triple) => from_indices(s, triple),
        None => Solved::Enclosed,
    }
}

/// Is the origin on the opposite side of plane(a,b,c) from `d`?
fn origin_outside_face(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> bool {
    let n = (b - a).cross(c - a);
    let sign_origin = (-a).dot(n); // (origin - a) . n
    let sign_d = (d - a).dot(n);
    sign_origin * sign_d < 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::BoxShape;

    fn boxshape(h: f32) -> BoxShape {
        BoxShape { half_extents: Vec3::splat(h) }
    }

    #[test]
    fn separated_boxes_report_gap() {
        let a = boxshape(1.0);
        let b = boxshape(1.0);
        let pa = Pose::from_position(Vec3::ZERO);
        let pb = Pose::from_position(Vec3::new(5.0, 0.0, 0.0));
        match gjk(&a, &pa, &b, &pb) {
            GjkOutcome::Separated { distance, normal, .. } => {
                // Boxes span [-1,1] and [4,6]: a 3-unit gap along +X.
                assert!((distance - 3.0).abs() < 1e-3, "distance {distance}");
                assert!((normal - Vec3::X).length() < 1e-3, "normal {normal:?}");
            }
            GjkOutcome::Penetrating { .. } => panic!("should be separated"),
        }
    }

    #[test]
    fn overlapping_boxes_report_penetration() {
        let a = boxshape(1.0);
        let b = boxshape(1.0);
        let pa = Pose::from_position(Vec3::ZERO);
        let pb = Pose::from_position(Vec3::new(1.5, 0.0, 0.0));
        assert!(matches!(gjk(&a, &pa, &b, &pb), GjkOutcome::Penetrating { .. }));
    }
}
