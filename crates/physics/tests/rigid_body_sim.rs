//! Scenario tests for the minimal rigid-body pipeline: a ball settling on a
//! static ground plane, and an elastic head-on collision conserving energy.

use elderforge_physics::{Collider, PhysicsMaterial, PhysicsWorld, RigidBody};
use elderforge_physics::body::BodyKind;
use elderforge_core::math::Vec3;

const DT: f32 = 1.0 / 120.0;

/// Total translational kinetic energy of the dynamic bodies in the world.
fn dynamic_kinetic_energy(world: &PhysicsWorld, handles: &[elderforge_physics::BodyHandle]) -> f32 {
    handles
        .iter()
        .filter_map(|&h| world.body(h))
        .filter(|b| b.kind == BodyKind::Dynamic)
        .map(|b| b.kinetic_energy())
        .sum()
}

#[test]
fn ball_falls_and_rests_on_ground_plane() {
    let mut world = PhysicsWorld::new(); // default gravity (0, -9.81, 0)

    // Static ground: the plane y = 0 with the solid region below it.
    world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));

    // A unit ball dropped from y = 5 with no bounce (restitution 0).
    let radius = 0.5;
    let ball = world.add_rigid_body(RigidBody::dynamic(
        Vec3::new(0.0, 5.0, 0.0),
        1.0,
        Collider::Sphere { radius },
    ));

    // Two seconds is comfortably long enough to fall ~4.5 m and settle.
    for _ in 0..240 {
        world.step(DT);
    }

    let body = world.body(ball).expect("ball still exists");
    // Rests with its center one radius above the plane, and at rest.
    assert!(
        (body.position.y - radius).abs() < 1e-2,
        "ball should settle at y = radius, got {}",
        body.position.y
    );
    assert!(body.position.y > 0.0, "ball must not tunnel through the ground");
    assert!(
        body.linear_velocity.y.abs() < 1e-2,
        "ball should be at rest, got v_y = {}",
        body.linear_velocity.y
    );
}

#[test]
fn ball_bounces_back_up_with_restitution() {
    let mut world = PhysicsWorld::new(); // default gravity
    let bouncy = PhysicsMaterial { restitution: 0.8, ..PhysicsMaterial::default() };

    world.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 })
            .with_material(bouncy),
    );
    let radius = 0.5;
    let drop_y = 5.0;
    let ball = world.add_rigid_body(
        RigidBody::dynamic(Vec3::new(0.0, drop_y, 0.0), 1.0, Collider::Sphere { radius })
            .with_material(bouncy),
    );

    // Simulate 5 s; record whether it touched down and how high it rebounded.
    let mut touched_down = false;
    let mut rebound_peak = radius;
    for _ in 0..600 {
        world.step(DT);
        let y = world.body(ball).expect("ball exists").position.y;
        if y <= radius + 0.05 {
            touched_down = true;
        }
        if touched_down {
            rebound_peak = rebound_peak.max(y);
        }
    }

    assert!(touched_down, "ball should reach the ground");
    // It bounced meaningfully back up, but lost energy (restitution < 1) so it
    // never returns to the drop height.
    assert!(rebound_peak > radius + 0.5, "ball should rebound, peak y = {rebound_peak}");
    assert!(rebound_peak < drop_y, "ball should lose energy, peak y = {rebound_peak}");
}

#[test]
fn elastic_head_on_collision_conserves_energy() {
    let mut world = PhysicsWorld::new();
    world.gravity = Vec3::ZERO; // isolate the collision from gravity

    let elastic = PhysicsMaterial { restitution: 1.0, ..PhysicsMaterial::default() };
    let radius = 0.5;

    // Equal-mass spheres approaching head-on at 2 m/s each.
    let a = world.add_rigid_body(
        RigidBody::dynamic(Vec3::new(-2.0, 0.0, 0.0), 1.0, Collider::Sphere { radius })
            .with_linear_velocity(Vec3::new(2.0, 0.0, 0.0))
            .with_material(elastic),
    );
    let b = world.add_rigid_body(
        RigidBody::dynamic(Vec3::new(2.0, 0.0, 0.0), 1.0, Collider::Sphere { radius })
            .with_linear_velocity(Vec3::new(-2.0, 0.0, 0.0))
            .with_material(elastic),
    );

    let energy_before = dynamic_kinetic_energy(&world, &[a, b]);
    assert!((energy_before - 4.0).abs() < 1e-6, "sanity: KE should start at 4 J");

    // Long enough to close the 3 m gap, collide, and separate again.
    for _ in 0..200 {
        world.step(DT);
    }

    let energy_after = dynamic_kinetic_energy(&world, &[a, b]);
    assert!(
        (energy_after - energy_before).abs() / energy_before < 1e-3,
        "elastic collision must conserve KE: before {energy_before}, after {energy_after}"
    );

    // They actually bounced: velocities reversed and the pair is separating.
    let (va, vb) = (
        world.body(a).unwrap().linear_velocity.x,
        world.body(b).unwrap().linear_velocity.x,
    );
    assert!(va < 0.0, "A should rebound in -x, got {va}");
    assert!(vb > 0.0, "B should rebound in +x, got {vb}");
}
