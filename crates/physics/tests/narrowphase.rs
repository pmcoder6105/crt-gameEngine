//! Narrowphase (GJK + EPA) checked against hand-computed analytical contacts.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use elderforge_core::math::{Quat, Vec3};
use elderforge_physics::narrowphase::{collide, Pose};
use elderforge_physics::shapes::{BoxShape, Capsule, Sphere};

fn unit_box() -> BoxShape {
    BoxShape { half_extents: Vec3::ONE }
}

fn assert_normal(actual: Vec3, expected: Vec3, ctx: &str) {
    assert!(
        (actual - expected).length() < 2e-2,
        "{ctx}: normal {actual:?} != expected {expected:?}"
    );
}

#[test]
fn box_box_edge_contact() {
    // A is axis-aligned; B is rolled 45° about Z so its underside is a single
    // edge (along Z), resting on A's top face. B's lowest extent sits sqrt(2)
    // below its center, so with the center at y = 2.314 it dips 0.1 into A.
    let a = unit_box();
    let b = unit_box();
    let pose_a = Pose::from_position(Vec3::ZERO);
    let pose_b = Pose::new(Vec3::new(0.0, 2.314, 0.0), Quat::from_rotation_z(FRAC_PI_4));

    let m = collide(&a, &pose_a, &b, &pose_b).expect("edge contact exists");
    assert_normal(m.normal, Vec3::Y, "box-box edge");
    assert!((m.depth - 0.1).abs() < 2e-2, "depth {} != ~0.1", m.depth);
}

#[test]
fn sphere_box_face_contact() {
    // Sphere center is 0.4 in front of A's +X face; radius 0.5 -> 0.1 overlap.
    let a = unit_box();
    let b = Sphere { radius: 0.5 };
    let pose_a = Pose::from_position(Vec3::ZERO);
    let pose_b = Pose::from_position(Vec3::new(1.4, 0.0, 0.0));

    let m = collide(&a, &pose_a, &b, &pose_b).expect("sphere touches face");
    assert_normal(m.normal, Vec3::X, "sphere-box face");
    assert!((m.depth - 0.1).abs() < 1e-3, "depth {} != ~0.1", m.depth);
}

#[test]
fn capsule_capsule_angled_contact() {
    // A is a vertical capsule on the Y axis; B is rotated 90° about Z (so it
    // runs along X) and offset 0.8 in Z. The segments pass 0.8 apart; with two
    // 0.5 radii that is a 0.2 overlap along +Z.
    let a = Capsule { radius: 0.5, half_height: 2.0 };
    let b = Capsule { radius: 0.5, half_height: 2.0 };
    let pose_a = Pose::from_position(Vec3::ZERO);
    let pose_b = Pose::new(Vec3::new(0.0, 0.5, 0.8), Quat::from_rotation_z(FRAC_PI_2));

    let m = collide(&a, &pose_a, &b, &pose_b).expect("capsules overlap");
    assert_normal(m.normal, Vec3::Z, "capsule-capsule angled");
    assert!((m.depth - 0.2).abs() < 2e-2, "depth {} != ~0.2", m.depth);
}

#[test]
fn boxes_barely_overlapping() {
    // 0.001 of overlap along X: EPA must resolve a tiny but non-zero depth.
    let a = unit_box();
    let b = unit_box();
    let pose_a = Pose::from_position(Vec3::ZERO);
    let pose_b = Pose::from_position(Vec3::new(1.999, 0.0, 0.0));

    let m = collide(&a, &pose_a, &b, &pose_b).expect("a hair of overlap is still contact");
    assert_normal(m.normal, Vec3::X, "barely overlapping");
    assert!(m.depth > 0.0 && m.depth < 0.01, "depth {} should be tiny+positive", m.depth);
    assert!((m.depth - 0.001).abs() < 5e-4, "depth {} != ~0.001", m.depth);
}

#[test]
fn boxes_barely_separated() {
    // 0.001 of gap along X: must report no contact.
    let a = unit_box();
    let b = unit_box();
    let pose_a = Pose::from_position(Vec3::ZERO);
    let pose_b = Pose::from_position(Vec3::new(2.001, 0.0, 0.0));

    assert!(
        collide(&a, &pose_a, &b, &pose_b).is_none(),
        "a hair of separation must not register a contact"
    );
}
