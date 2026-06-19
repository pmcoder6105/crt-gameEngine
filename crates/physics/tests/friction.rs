//! Coulomb friction: a box on an inclined plane stays put while `tan θ < μ` and
//! slides once `tan θ > μ`, so the slide/stay transition sits at `θ = arctan μ`
//! — the textbook static-friction result.

use elderforge_core::math::Vec3;
use elderforge_physics::{Collider, PhysicsMaterial, PhysicsWorld, RigidBody};

const FRAME_DT: f32 = 1.0 / 60.0;
const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

/// Build a box resting on a half-space inclined at `theta_deg`, both with
/// friction `mu`, let it settle, then return how far the box slid *down the
/// slope* over the following three seconds.
fn slide_distance(theta_deg: f32, mu: f32) -> f32 {
    let theta = theta_deg.to_radians();
    // Surface normal tilted `theta` from vertical (about Z): the slope falls
    // toward -X.
    let normal = Vec3::new(theta.sin(), theta.cos(), 0.0);

    let mut world = PhysicsWorld::new();
    world.gravity = GRAVITY;
    // Keep it awake the whole run so a held box can't "sleep" out of the
    // measurement — we want to see genuine zero sliding, not a frozen body.
    world.sleeping_enabled = false;
    let material = PhysicsMaterial {
        static_friction: mu,
        dynamic_friction: mu,
        restitution: 0.0,
        density: 1000.0,
    };

    world.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal, offset: 0.0 })
            .with_material(material),
    );

    let half = 0.5;
    // Rest height of an axis-aligned cube touching the incline along `normal`.
    let support = half * (theta.sin() + theta.cos());
    let start = normal * (support + 0.002);
    let b = world.add_rigid_body(
        RigidBody::dynamic(start, 1.0, Collider::Box { half_extents: Vec3::splat(half) })
            .with_material(material),
    );

    // Let the box settle onto the surface (kills the initial normal transient).
    for _ in 0..30 {
        world.step(FRAME_DT);
    }
    let p0 = world.body(b).unwrap().position;
    for _ in 0..180 {
        world.step(FRAME_DT);
    }
    let p1 = world.body(b).unwrap().position;

    // Downhill unit tangent: gravity with its normal component removed.
    let tangent = (GRAVITY - GRAVITY.dot(normal) * normal).normalize();
    (p1 - p0).dot(tangent)
}

#[test]
fn box_holds_below_critical_angle_and_slides_above() {
    let mu = 0.5f32;
    let critical = mu.atan().to_degrees(); // ≈ 26.57°

    // Comfortably below the friction angle: static friction holds it in place.
    let held = slide_distance(critical - 5.0, mu);
    assert!(held.abs() < 0.05, "box should stay put below μ-angle, slid {held}");

    // Comfortably above: it accelerates down the slope.
    let slid = slide_distance(critical + 5.0, mu);
    assert!(slid > 0.5, "box should slide above μ-angle, only moved {slid}");
}

#[test]
fn slide_threshold_matches_arctan_mu() {
    let mu = 0.5f32;
    let critical = mu.atan().to_degrees(); // ≈ 26.57°

    // Sweep angles and find where the box transitions from holding to sliding.
    let mut last_held = 0.0f32;
    let mut first_slid = 90.0f32;
    for deg in [15.0, 18.0, 21.0, 24.0, 27.0, 30.0, 33.0, 36.0] {
        let slid = slide_distance(deg, mu);
        if slid.abs() < 0.05 {
            last_held = last_held.max(deg);
        } else if slid > 0.3 {
            first_slid = first_slid.min(deg);
        }
    }
    assert!(last_held < first_slid, "no clean slide/hold transition found");
    let measured = 0.5 * (last_held + first_slid);
    assert!(
        (measured - critical).abs() < 4.0,
        "slide threshold {measured:.1}° should track arctan(μ) = {critical:.1}°"
    );
}
