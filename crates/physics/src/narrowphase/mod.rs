//! Narrowphase: GJK + EPA for convex shapes; SAT for polyhedra.

pub mod epa;
pub mod gjk;
pub mod sat;

use elderforge_core::math::{Vec3, EPSILON};

/// A single contact point produced by the narrowphase.
///
/// `normal` is a unit vector along which the bodies must separate, and
/// `penetration` is how deep they overlap along it (always `> 0` for a real
/// contact). The direction convention (which body `normal` points toward) is
/// fixed by whichever detector produced the contact; see each function.
#[derive(Debug, Clone, Copy)]
pub struct Contact {
    pub point: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
}

/// Sphere–sphere contact. `normal` points from sphere A toward sphere B, so a
/// resolver pushes B along `+normal` and A along `-normal`. Returns `None`
/// when the spheres are separated.
pub fn sphere_sphere(
    center_a: Vec3,
    radius_a: f32,
    center_b: Vec3,
    radius_b: f32,
) -> Option<Contact> {
    let delta = center_b - center_a;
    let radius_sum = radius_a + radius_b;
    let dist_sq = delta.length_squared();
    if dist_sq >= radius_sum * radius_sum {
        return None;
    }
    let dist = dist_sq.sqrt();
    // Coincident centers have no defined direction; pick an arbitrary axis.
    let normal = if dist > EPSILON { delta / dist } else { Vec3::Y };
    let penetration = radius_sum - dist;
    // Contact point: on A's surface, halfway into the overlap region.
    let point = center_a + normal * (radius_a - penetration * 0.5);
    Some(Contact {
        point,
        normal,
        penetration,
    })
}

/// Sphere vs. static half-space. The half-space solid lies on the `-normal`
/// side of the plane `dot(normal, x) = offset`. The returned `normal` equals
/// the plane normal (pointing from the surface toward the sphere), so a
/// resolver pushes the sphere along `+normal` out of the solid. Returns `None`
/// when the sphere is clear of the surface.
pub fn sphere_halfspace(center: Vec3, radius: f32, normal: Vec3, offset: f32) -> Option<Contact> {
    let signed_distance = center.dot(normal) - offset;
    let penetration = radius - signed_distance;
    if penetration <= 0.0 {
        return None;
    }
    // Deepest point of the sphere, projected onto the surface.
    let point = center - normal * radius;
    Some(Contact {
        point,
        normal,
        penetration,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_constructs() {
        let contact = Contact {
            point: Vec3::ZERO,
            normal: Vec3::Y,
            penetration: 0.05,
        };
        assert_eq!(contact.normal, Vec3::Y);
    }

    #[test]
    fn spheres_overlapping_along_x_report_contact() {
        // Centers 1.5 apart, radii 1 each -> 0.5 penetration, normal +X.
        let contact = sphere_sphere(Vec3::ZERO, 1.0, Vec3::new(1.5, 0.0, 0.0), 1.0)
            .expect("overlapping spheres must contact");
        assert!((contact.penetration - 0.5).abs() < 1e-6);
        assert!((contact.normal - Vec3::X).length() < 1e-6);
    }

    #[test]
    fn separated_spheres_report_no_contact() {
        assert!(sphere_sphere(Vec3::ZERO, 1.0, Vec3::new(3.0, 0.0, 0.0), 1.0).is_none());
    }

    #[test]
    fn coincident_spheres_pick_a_fallback_normal() {
        let contact = sphere_sphere(Vec3::ZERO, 1.0, Vec3::ZERO, 1.0)
            .expect("coincident spheres overlap fully");
        assert!(contact.normal.is_normalized());
        assert!((contact.penetration - 2.0).abs() < 1e-6);
    }

    #[test]
    fn sphere_below_ground_penetrates() {
        // Sphere radius 1 centered at y = 0.25 over the y = 0 plane (normal +Y).
        let contact = sphere_halfspace(Vec3::new(0.0, 0.25, 0.0), 1.0, Vec3::Y, 0.0)
            .expect("sphere dips below the surface");
        assert!((contact.penetration - 0.75).abs() < 1e-6);
        assert_eq!(contact.normal, Vec3::Y);
    }

    #[test]
    fn sphere_above_ground_clears() {
        assert!(sphere_halfspace(Vec3::new(0.0, 5.0, 0.0), 1.0, Vec3::Y, 0.0).is_none());
    }
}
