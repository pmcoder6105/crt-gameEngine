//! XPBD solver scenario tests: rope convergence, box-stack stability, and an
//! analytically-correct pendulum period.

use elderforge_core::math::Vec3;
use elderforge_physics::{Collider, PhysicsMaterial, PhysicsWorld, RigidBody};

const FRAME_DT: f32 = 1.0 / 60.0;

#[test]
fn stiff_rope_converges_within_one_frame_of_substeps() {
    // Anchor + 10 rigid links laid out horizontally, each starting 20% over its
    // rest length. Released under gravity, one frame of 20 substeps must pull
    // every link to its rest length (the stiff solver converges and stays
    // stable — with too few substeps a rigid rope like this explodes).
    let mut world = PhysicsWorld::new();
    world.substeps = 20;
    let rest = 0.5;
    let start = 0.6; // stretched

    let anchor = world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::Sphere { radius: 0.01 },
    ));
    let mut prev = anchor;
    let mut links = Vec::new();
    for i in 1..=10 {
        let body = world.add_rigid_body(RigidBody::dynamic(
            Vec3::new(i as f32 * start, 0.0, 0.0),
            1.0,
            Collider::Sphere { radius: 0.01 },
        ));
        world.add_distance_constraint(prev, body, rest, 0.0);
        links.push((prev, body));
        prev = body;
    }

    world.step(FRAME_DT);

    let mut max_err = 0.0f32;
    for (a, b) in links {
        let pa = world.body(a).unwrap().position;
        let pb = world.body(b).unwrap().position;
        let len = (pa - pb).length();
        max_err = max_err.max((len - rest).abs());
    }
    assert!(
        max_err < 1e-2,
        "rope did not converge: max link error {max_err}"
    );
}

#[test]
fn stack_of_boxes_is_stable_for_600_frames() {
    let mut world = PhysicsWorld::new();
    let half = 0.5;
    let no_bounce = PhysicsMaterial { restitution: 0.0, ..PhysicsMaterial::default() };

    // Ground plane.
    world.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 })
            .with_material(no_bounce),
    );

    // 10 unit cubes stacked exactly touching: centers at y = 0.5, 1.5, ... 9.5.
    let mut handles = Vec::new();
    for i in 0..10 {
        let y = half + i as f32 * (2.0 * half);
        handles.push(world.add_rigid_body(
            RigidBody::dynamic(Vec3::new(0.0, y, 0.0), 1.0, Collider::Box {
                half_extents: Vec3::splat(half),
            })
            .with_material(no_bounce),
        ));
    }

    for _ in 0..600 {
        world.step(FRAME_DT);
    }

    // No popping: every box stayed near its initial height.
    for (i, &h) in handles.iter().enumerate() {
        let y = world.body(h).unwrap().position.y;
        let expected = half + i as f32;
        assert!(
            (y - expected).abs() < 0.05,
            "box {i} drifted: y {y}, expected ~{expected}"
        );
    }

    // No penetration drift: neighbouring boxes stay ~1.0 apart.
    for w in handles.windows(2) {
        let lower = world.body(w[0]).unwrap().position.y;
        let upper = world.body(w[1]).unwrap().position.y;
        let gap = upper - lower;
        assert!(
            (gap - 1.0).abs() < 0.05,
            "boxes interpenetrated or separated: gap {gap}"
        );
    }
}

#[test]
fn pendulum_period_matches_analytic() {
    let mut world = PhysicsWorld::new();
    world.substeps = 20;
    world.gravity = Vec3::new(0.0, -9.81, 0.0);
    let g = 9.81f32;
    let length = 1.0f32;

    // Small release angle so the period matches the small-angle formula.
    let theta0 = 0.15f32;
    let pivot = world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::Sphere { radius: 0.01 },
    ));
    let bob = world.add_rigid_body(RigidBody::dynamic(
        Vec3::new(length * theta0.sin(), -length * theta0.cos(), 0.0),
        1.0,
        Collider::Sphere { radius: 0.01 },
    ));
    world.add_distance_constraint(pivot, bob, length, 0.0);

    // Record the bob's horizontal position over ~4 periods.
    let mut samples = Vec::new();
    let steps = 300; // 5 s at 60 Hz
    for k in 0..steps {
        world.step(FRAME_DT);
        samples.push((k as f32 * FRAME_DT, world.body(bob).unwrap().position.x));
    }

    // Zero-crossings of x (the bob passing through the bottom) are half a period
    // apart; average several for a robust estimate.
    let mut crossings = Vec::new();
    for pair in samples.windows(2) {
        let (t0, x0) = pair[0];
        let (t1, x1) = pair[1];
        if x0 == 0.0 || (x0 < 0.0) != (x1 < 0.0) {
            // Linear interpolation of the crossing time.
            let t = t0 + (t1 - t0) * (-x0 / (x1 - x0));
            crossings.push(t);
        }
    }
    assert!(crossings.len() >= 4, "expected several swings, got {}", crossings.len());

    let half_periods: Vec<f32> = crossings.windows(2).map(|w| w[1] - w[0]).collect();
    let measured = 2.0 * half_periods.iter().sum::<f32>() / half_periods.len() as f32;
    let analytic = std::f32::consts::TAU * (length / g).sqrt();

    let error = (measured - analytic).abs() / analytic;
    eprintln!("pendulum: measured {measured:.4}s, analytic {analytic:.4}s, error {:.2}%", error * 100.0);
    assert!(error < 0.02, "period off by {:.2}% (>2%)", error * 100.0);
}
