//! Scenario tests for the four XPBD joint types. Each builds a static anchor
//! body plus a dynamic body, drives it with gravity / an initial velocity, and
//! checks the joint's defining invariant holds (and that the *free* degrees of
//! freedom really are free).

use elderforge_core::math::Vec3;
use elderforge_physics::{Collider, PhysicsWorld, RigidBody};

const FRAME_DT: f32 = 1.0 / 60.0;

fn unit_box(pos: Vec3, mass: f32) -> RigidBody {
    RigidBody::dynamic(pos, mass, Collider::Box { half_extents: Vec3::splat(0.5) })
}

fn anchor_world(body: &RigidBody, local: Vec3) -> Vec3 {
    body.position + body.rotation * local
}

#[test]
fn ball_joint_pins_a_point_while_leaving_rotation_free() {
    // A ball joint pins B's anchor (1 above its COM) to a fixed pivot at the
    // origin. Pushed sideways, B should swing as a pendulum: the anchor stays
    // glued to the pivot, and the COM stays one arm-length away the whole time.
    let mut world = PhysicsWorld::new();
    let anchor = world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::Sphere { radius: 0.01 },
    ));
    let arm = Vec3::new(0.0, 1.0, 0.0);
    // B hangs straight down (COM at -arm), shoved in +X so it swings.
    let b = world.add_rigid_body(unit_box(-arm, 1.0).with_linear_velocity(Vec3::new(2.0, 0.0, 0.0)));
    world.add_ball_joint(anchor, b, Vec3::ZERO, arm, 0.0);

    let mut max_anchor_err = 0.0f32;
    let mut swung = false;
    for _ in 0..240 {
        world.step(FRAME_DT);
        let body = world.body(b).unwrap();
        let err = (anchor_world(body, arm) - Vec3::ZERO).length();
        max_anchor_err = max_anchor_err.max(err);
        // COM rides a sphere of radius |arm| about the pivot.
        let radius = body.position.length();
        assert!((radius - 1.0).abs() < 0.05, "arm length drifted: {radius}");
        if body.position.x.abs() > 0.2 {
            swung = true;
        }
    }
    assert!(max_anchor_err < 0.02, "ball joint let the anchor separate: {max_anchor_err}");
    assert!(swung, "pendulum never swung — the rotational DOF is stuck");
}

#[test]
fn hinge_joint_keeps_axis_aligned_but_spins_freely() {
    // Hinge about world Z. B is spun about Z (the free axis) and *also* kicked
    // about X (which the hinge must resist). Its hinge axis must stay glued to
    // Z while it keeps spinning about it.
    let mut world = PhysicsWorld::new();
    world.gravity = Vec3::ZERO;
    let anchor = world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::Sphere { radius: 0.01 },
    ));
    let b = world.add_rigid_body(unit_box(Vec3::ZERO, 1.0).with_angular_velocity(Vec3::new(4.0, 0.0, 6.0)));
    world.add_hinge_joint(anchor, b, Vec3::ZERO, Vec3::ZERO, Vec3::Z, Vec3::Z, None, 0.0);

    let mut min_align = 1.0f32;
    let mut spun_past_90 = false;
    for _ in 0..120 {
        world.step(FRAME_DT);
        let q = world.body(b).unwrap().rotation;
        let axis_world = q * Vec3::Z;
        min_align = min_align.min(axis_world.dot(Vec3::Z));
        // Local +X swept around: at some point it points opposite its start.
        if (q * Vec3::X).dot(Vec3::X) < 0.0 {
            spun_past_90 = true;
        }
    }
    assert!(min_align > 0.97, "hinge let its axis tilt off Z: align {min_align}");
    assert!(spun_past_90, "hinge never rotated about its free axis");
}

#[test]
fn hinge_joint_angle_limit_stops_the_swing() {
    // Hinge about Z with the swing limited to [-0.5, 0.5] rad. B sticks out in
    // +X from the pivot, so gravity swings it clockwise (negative angle). The
    // limit must catch it at -0.5 and never let it past.
    let mut world = PhysicsWorld::new();
    let anchor = world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::Sphere { radius: 0.01 },
    ));
    // COM at +X, anchor (the pivot) at the box's -X face center.
    let b = world.add_rigid_body(unit_box(Vec3::new(0.5, 0.0, 0.0), 1.0));
    let limit = 0.5;
    world.add_hinge_joint(
        anchor,
        b,
        Vec3::ZERO,
        Vec3::new(-0.5, 0.0, 0.0),
        Vec3::Z,
        Vec3::Z,
        Some((-limit, limit)),
        0.0,
    );

    let mut min_angle = 0.0f32;
    let mut max_angle = 0.0f32;
    for _ in 0..300 {
        world.step(FRAME_DT);
        let x = world.body(b).unwrap().rotation * Vec3::X;
        let angle = x.y.atan2(x.x); // signed rotation about Z
        min_angle = min_angle.min(angle);
        max_angle = max_angle.max(angle);
    }
    // It swung down to the limit and was held there...
    assert!(min_angle < -limit + 0.15, "hinge never reached its limit: min {min_angle}");
    // ...without ever breaking through it (small tolerance for substep overshoot).
    assert!(min_angle > -limit - 0.05, "hinge blew past its lower limit: min {min_angle}");
    assert!(max_angle < limit + 0.05, "hinge blew past its upper limit: max {max_angle}");
}

#[test]
fn prismatic_joint_slides_along_axis_and_stops_at_limit() {
    // Slide axis = Y, travel limited to [-3, -1]. Gravity has a sideways (+X)
    // component the joint must absorb: B may only move along Y, must not rotate,
    // and must stop at the lower travel limit.
    let mut world = PhysicsWorld::new();
    world.gravity = Vec3::new(4.0, -9.81, 0.0);
    let anchor = world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::Sphere { radius: 0.01 },
    ));
    let b = world.add_rigid_body(unit_box(Vec3::new(0.0, -1.0, 0.0), 1.0));
    world.add_prismatic_joint(anchor, b, Vec3::ZERO, Vec3::ZERO, Vec3::Y, Some((-3.0, -1.0)), 0.0);

    let mut min_y = 0.0f32;
    for _ in 0..240 {
        world.step(FRAME_DT);
        let body = world.body(b).unwrap();
        min_y = min_y.min(body.position.y);
        // Perpendicular translation locked despite the +X gravity pull.
        assert!(body.position.x.abs() < 0.03, "slid off-axis in X: {}", body.position.x);
        assert!(body.position.z.abs() < 0.03, "slid off-axis in Z: {}", body.position.z);
        // Orientation locked.
        assert!(body.rotation.w.abs() > 0.999, "prismatic body rotated: {:?}", body.rotation);
    }
    assert!(min_y < -3.0 + 0.1, "did not slide to the lower limit: min_y {min_y}");
    assert!(min_y > -3.0 - 0.05, "slid past the travel limit: min_y {min_y}");
}

#[test]
fn fixed_joint_welds_position_and_orientation() {
    // A weld: B is held at a fixed offset from a static anchor with locked
    // orientation. Gravity and an initial spin must both be fully resisted.
    let mut world = PhysicsWorld::new();
    let anchor = world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::Sphere { radius: 0.01 },
    ));
    let start = Vec3::new(1.0, 0.0, 0.0);
    let b = world.add_rigid_body(unit_box(start, 1.0).with_angular_velocity(Vec3::new(1.0, 1.0, 1.0)));
    // Anchor on A at the hold point; B's COM coincides with it.
    world.add_fixed_joint(anchor, b, start, Vec3::ZERO, 0.0);

    for _ in 0..240 {
        world.step(FRAME_DT);
    }
    let body = world.body(b).unwrap();
    assert!(
        (body.position - start).length() < 0.02,
        "weld drifted in position: {:?}",
        body.position
    );
    // Orientation stayed locked to identity (the rest relative rotation).
    let angle = 2.0 * body.rotation.w.clamp(-1.0, 1.0).acos();
    assert!(angle < 0.05, "weld let the body rotate: {angle} rad");
}
