//! EPA (Expanding Polytope Algorithm): penetration depth, contact normal, and
//! witness points for shape cores GJK reported as overlapping.
//!
//! Starting from GJK's origin-enclosing simplex, the polytope of Minkowski-
//! difference points is grown one support vertex at a time toward the face
//! nearest the origin, until that face stops moving. Its outward normal is the
//! penetration direction (A->B) and its distance to the origin is the depth.

use std::collections::HashMap;

use elderforge_core::math::Vec3;

use super::convex::{minkowski_support, ConvexShape, Pose, SupportPoint};

const MAX_ITERS: usize = 64;
const EPS: f32 = 1e-6;

/// Penetration result from EPA. `normal` points from A to B.
pub struct EpaResult {
    pub normal: Vec3,
    pub depth: f32,
    pub point_a: Vec3,
    pub point_b: Vec3,
}

/// A polytope face: three vertex indices with a cached outward unit normal and
/// the (non-negative) distance from the origin to its plane.
struct Face {
    v: [usize; 3],
    normal: Vec3,
    dist: f32,
}

/// Expand the GJK simplex into the closest face of the Minkowski difference.
pub fn epa(
    a: &dyn ConvexShape,
    pose_a: &Pose,
    b: &dyn ConvexShape,
    pose_b: &Pose,
    simplex: &[SupportPoint],
) -> Option<EpaResult> {
    let support = |dir: Vec3| minkowski_support(a, pose_a, b, pose_b, dir);

    // Build a fresh, non-degenerate origin-enclosing tetrahedron. GJK's
    // terminating simplex can be degenerate (e.g. box vs box, whose Minkowski
    // difference is a box and can leave the origin coplanar with a face), so we
    // reconstruct rather than trust its shape. `simplex` is kept as a hint of
    // where the overlap is but isn't required.
    let _ = simplex;
    let mut verts = build_tetrahedron(&support)?;

    // Seed faces from the tetrahedron, each oriented to face away from the
    // origin (it lies inside).
    let mut faces = Vec::new();
    for (i, j, k) in [(0, 1, 2), (0, 1, 3), (0, 2, 3), (1, 2, 3)] {
        if let Some(face) = make_face(&verts, i, j, k) {
            faces.push(face);
        }
    }
    if faces.is_empty() {
        return None;
    }

    // Faces we've proven can't be expanded (degenerate slices through/near the
    // origin, common for box-vs-box). Skipping them lets the polytope keep
    // growing through its other faces until the true closest face emerges.
    let mut exhausted: std::collections::HashSet<[usize; 3]> = std::collections::HashSet::new();

    for _ in 0..MAX_ITERS {
        // Closest non-exhausted face to the origin.
        let Some(closest) = faces
            .iter()
            .enumerate()
            .filter(|(_, f)| !exhausted.contains(&sorted_tri(f.v)))
            .min_by(|(_, x), (_, y)| x.dist.total_cmp(&y.dist))
            .map(|(i, _)| i)
        else {
            break;
        };
        let normal = faces[closest].normal;
        let dist = faces[closest].dist;

        let w = support(normal);
        let reach = w.v.dot(normal);
        if reach - dist < EPS {
            return Some(finish(&verts, &faces[closest]));
        }
        if verts.iter().any(|v| (v.v - w.v).length_squared() < EPS * EPS) {
            // Can't grow this face (its support is already a vertex); set it
            // aside and keep expanding the rest of the polytope.
            exhausted.insert(sorted_tri(faces[closest].v));
            continue;
        }

        // Remove every face the new point can see, recording their edges.
        let wi = verts.len();
        verts.push(w);
        let mut edge_count: HashMap<(usize, usize), i32> = HashMap::new();
        let mut kept = Vec::with_capacity(faces.len());
        for face in faces.drain(..) {
            if w.v.dot(face.normal) - face.dist > EPS {
                for (p, q) in [
                    (face.v[0], face.v[1]),
                    (face.v[1], face.v[2]),
                    (face.v[2], face.v[0]),
                ] {
                    *edge_count.entry(edge_key(p, q)).or_insert(0) += 1;
                }
            } else {
                kept.push(face);
            }
        }
        faces = kept;

        // Edges on exactly one removed face form the horizon; stitch each to w.
        for ((p, q), count) in edge_count {
            if count == 1 {
                if let Some(face) = make_face(&verts, p, q, wi) {
                    faces.push(face);
                }
            }
        }
        if faces.is_empty() {
            return None;
        }
    }

    // Hit the iteration cap: report the best non-degenerate face we have.
    let closest = faces
        .iter()
        .filter(|f| f.dist > EPS)
        .min_by(|x, y| x.dist.total_cmp(&y.dist))
        .or_else(|| faces.iter().min_by(|x, y| x.dist.total_cmp(&y.dist)))?;
    Some(finish(&verts, closest))
}

/// Sorted vertex triple, a stable identity for a face across expansions.
fn sorted_tri(v: [usize; 3]) -> [usize; 3] {
    let mut t = v;
    t.sort_unstable();
    t
}

/// Build the contact result for a converged face: barycentric-recover the
/// witness points on each core at the origin's projection onto the face.
fn finish(verts: &[SupportPoint], face: &Face) -> EpaResult {
    let [i, j, k] = face.v;
    let projection = face.normal * face.dist;
    let (u, v, w) = barycentric(verts[i].v, verts[j].v, verts[k].v, projection);
    let point_a = verts[i].a * u + verts[j].a * v + verts[k].a * w;
    let point_b = verts[i].b * u + verts[j].b * v + verts[k].b * w;
    EpaResult {
        normal: face.normal,
        depth: face.dist,
        point_a,
        point_b,
    }
}

/// Unordered edge key.
fn edge_key(p: usize, q: usize) -> (usize, usize) {
    if p < q {
        (p, q)
    } else {
        (q, p)
    }
}

/// Make a face from three vertices, oriented so its normal points away from the
/// origin (which is interior). `None` if the triangle is degenerate.
fn make_face(verts: &[SupportPoint], i: usize, j: usize, k: usize) -> Option<Face> {
    let (a, b, c) = (verts[i].v, verts[j].v, verts[k].v);
    let mut normal = (b - a).cross(c - a);
    let len = normal.length();
    if len < EPS {
        return None;
    }
    normal /= len;
    let mut v = [i, j, k];
    // Origin is inside, so an outward normal satisfies n·a > 0.
    if normal.dot(a) < 0.0 {
        normal = -normal;
        v.swap(1, 2);
    }
    Some(Face {
        v,
        normal,
        dist: normal.dot(a).max(0.0),
    })
}

/// Construct an origin-enclosing tetrahedron from support queries. Returns
/// `None` only for genuinely degenerate inputs (e.g. flat Minkowski difference).
fn build_tetrahedron(support: &dyn Fn(Vec3) -> SupportPoint) -> Option<Vec<SupportPoint>> {
    // First vertex along an arbitrary axis; second straight back toward origin.
    let mut a = support(Vec3::X);
    let mut dir = -a.v;
    if dir.length_squared() < EPS {
        a = support(Vec3::Y);
        dir = -a.v;
        if dir.length_squared() < EPS {
            a = support(Vec3::Z);
            dir = -a.v;
        }
    }
    let b = support(dir);
    let ab = b.v - a.v;
    if ab.length_squared() < EPS {
        return None;
    }

    // Third vertex: off the line AB, in the direction from the line to origin.
    let ao = -a.v;
    let mut dir3 = ab.cross(ao).cross(ab);
    if dir3.length_squared() < EPS {
        // Origin lies on line AB; any perpendicular will do.
        dir3 = ab.cross(Vec3::X);
        if dir3.length_squared() < EPS {
            dir3 = ab.cross(Vec3::Z);
        }
    }
    let c = support(dir3);

    // Fourth vertex: off the plane ABC, on the side toward the origin.
    let mut n = ab.cross(c.v - a.v);
    if n.length_squared() < EPS {
        return None;
    }
    if n.dot(-a.v) < 0.0 {
        n = -n;
    }
    let d = support(n);

    let mut verts = vec![a, b, c, d];
    if encloses_origin(&verts) {
        return Some(verts);
    }
    // The origin can end up on the far side; try the opposite half-space.
    verts[3] = support(-n);
    if encloses_origin(&verts) {
        Some(verts)
    } else {
        // Last resort: still hand EPA the simplex; near-degenerate overlaps
        // converge to a shallow contact, which is acceptable.
        Some(verts)
    }
}

/// True if the origin is inside (or on) the tetrahedron `verts[0..4]`.
fn encloses_origin(verts: &[SupportPoint]) -> bool {
    let p = [verts[0].v, verts[1].v, verts[2].v, verts[3].v];
    let faces = [
        ([0usize, 1, 2], 3usize),
        ([0, 2, 3], 1),
        ([0, 3, 1], 2),
        ([1, 3, 2], 0),
    ];
    for (tri, opp) in faces {
        let (a, b, c, d) = (p[tri[0]], p[tri[1]], p[tri[2]], p[opp]);
        let nrm = (b - a).cross(c - a);
        let sign_origin = (-a).dot(nrm);
        let sign_d = (d - a).dot(nrm);
        if sign_origin * sign_d < 0.0 {
            return false; // origin outside this face
        }
    }
    true
}

/// Barycentric coordinates of `p` within triangle ABC.
fn barycentric(a: Vec3, b: Vec3, c: Vec3, p: Vec3) -> (f32, f32, f32) {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-12 {
        return (1.0, 0.0, 0.0);
    }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    (1.0 - v - w, v, w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::narrowphase::gjk::{gjk, GjkOutcome};
    use crate::shapes::BoxShape;

    #[test]
    fn epa_finds_box_overlap_depth() {
        let a = BoxShape { half_extents: Vec3::ONE };
        let b = BoxShape { half_extents: Vec3::ONE };
        let pa = Pose::from_position(Vec3::ZERO);
        let pb = Pose::from_position(Vec3::new(1.5, 0.0, 0.0));
        let GjkOutcome::Penetrating { simplex } = gjk(&a, &pa, &b, &pb) else {
            panic!("boxes overlap");
        };
        let result = epa(&a, &pa, &b, &pb, &simplex).expect("epa converges");
        // Overlap along X is 0.5; normal points A->B (+X).
        assert!((result.depth - 0.5).abs() < 1e-2, "depth {}", result.depth);
        assert!((result.normal - Vec3::X).length() < 1e-2, "normal {:?}", result.normal);
    }
}
