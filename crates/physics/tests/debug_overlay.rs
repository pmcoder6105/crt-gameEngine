//! Integration tests for `PhysicsWorld::emit_debug`: each overlay layer emits
//! the right kind of geometry on a small but representative world, and the
//! buffers clear-and-reuse rather than accumulate.

use elderforge_core::math::Vec3;
use elderforge_physics::{Collider, DebugDraw, DebugLayers, PhysicsWorld, RigidBody};

/// Ground half-space, a box penetrating it (so a contact exists immediately),
/// and a sphere above, linked to the box by a distance constraint.
fn world() -> PhysicsWorld {
    let mut w = PhysicsWorld::new();
    w.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));
    let a = w.add_rigid_body(RigidBody::dynamic(
        Vec3::new(0.0, 0.4, 0.0), // half-extent 0.5 → penetrates the ground
        1.0,
        Collider::Box { half_extents: Vec3::splat(0.5) },
    ));
    let b = w.add_rigid_body(RigidBody::dynamic(
        Vec3::new(0.0, 2.0, 0.0),
        1.0,
        Collider::Sphere { radius: 0.5 },
    ));
    w.add_distance_constraint(a, b, 1.5, 0.0);
    w
}

fn only(set: impl FnOnce(&mut DebugLayers)) -> DebugLayers {
    let mut l = DebugLayers::default();
    set(&mut l);
    l
}

fn emit(w: &PhysicsWorld, layers: DebugLayers) -> DebugDraw {
    let mut d = DebugDraw::default();
    w.emit_debug(layers, &mut d);
    d
}

#[test]
fn all_layers_off_emits_nothing() {
    let d = emit(&world(), DebugLayers::default());
    assert!(d.is_empty());
}

#[test]
fn collision_shapes_emits_wireframe_lines_only() {
    let d = emit(&world(), only(|l| l.collision_shapes = true));
    assert!(!d.lines.is_empty());
    assert!(d.points.is_empty(), "wireframes are lines, not points");
}

#[test]
fn contact_points_emit_markers_and_normals() {
    // The box penetrates the ground, so there is a contact to draw.
    let d = emit(&world(), only(|l| l.contact_points = true));
    assert!(!d.points.is_empty(), "expected a contact-point marker dot");
    assert!(!d.lines.is_empty(), "expected a marker sphere + normal arrow");
}

#[test]
fn velocity_arrow_length_scales_with_speed() {
    let mut w = world();
    // The box is body index 1 (ground is 0), generation 0.
    let handle = elderforge_physics::BodyHandle::new(1, 0);

    w.body_mut(handle).expect("box body").linear_velocity = Vec3::new(0.0, 0.0, 1.0);
    let slow = emit(&w, only(|l| l.velocity_vectors = true));
    let slow_shaft = shaft_len(&slow);

    w.body_mut(handle).expect("box body").linear_velocity = Vec3::new(0.0, 0.0, 4.0);
    let fast = emit(&w, only(|l| l.velocity_vectors = true));
    let fast_shaft = shaft_len(&fast);

    // 4× the speed → ~4× the arrow shaft.
    assert!((fast_shaft / slow_shaft - 4.0).abs() < 0.2, "shaft did not scale with speed");
}

/// Length of the first emitted line (the velocity arrow's shaft).
fn shaft_len(d: &DebugDraw) -> f32 {
    let l = d.lines.first().expect("a velocity arrow");
    (l.end - l.start).length()
}

#[test]
fn bvh_layer_emits_boxes() {
    let d = emit(&world(), only(|l| l.bvh_aabbs = true));
    assert!(!d.lines.is_empty(), "BVH over finite bodies should draw boxes");
}

#[test]
fn constraint_anchors_emit_markers() {
    let d = emit(&world(), only(|l| l.constraint_anchors = true));
    assert!(!d.points.is_empty(), "anchors are marker dots");
    assert!(!d.lines.is_empty(), "anchors include cubes + a connection line");
}

#[test]
fn sleep_state_and_force_layers_emit() {
    let w = world();
    assert!(!emit(&w, only(|l| l.sleep_state = true)).lines.is_empty());
    assert!(!emit(&w, only(|l| l.force_accumulators = true)).lines.is_empty());
}

#[test]
fn emit_clears_between_calls() {
    let w = world();
    let mut d = DebugDraw::default();
    w.emit_debug(only(|l| l.collision_shapes = true), &mut d);
    let first = d.lines.len();
    w.emit_debug(only(|l| l.collision_shapes = true), &mut d);
    assert_eq!(d.lines.len(), first, "emit must clear, not accumulate");
}
