//! Sleeping: a settled stack stops being simulated (and stops costing
//! narrowphase work), and an impact wakes it back up.

use elderforge_core::math::Vec3;
use elderforge_physics::{Collider, PhysicsWorld, RigidBody};

const FRAME_DT: f32 = 1.0 / 60.0;

/// Ground half-space plus a stack of `n` unit boxes resting on it, returning the
/// box handles.
fn build_stack(world: &mut PhysicsWorld, n: usize) -> Vec<elderforge_physics::BodyHandle> {
    world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));
    let half = 0.5;
    (0..n)
        .map(|i| {
            let y = half + i as f32 * (2.0 * half);
            world.add_rigid_body(RigidBody::dynamic(
                Vec3::new(0.0, y, 0.0),
                1.0,
                Collider::Box { half_extents: Vec3::splat(half) },
            ))
        })
        .collect()
}

#[test]
fn settled_stack_sleeps_and_costs_no_narrowphase() {
    let mut world = PhysicsWorld::new();
    world.sleep_frames = 20;
    let boxes = build_stack(&mut world, 5);

    // Plenty of time to settle and then go quiet for `sleep_frames`.
    for _ in 0..300 {
        world.step(FRAME_DT);
    }

    assert_eq!(world.awake_body_count(), 0, "settled stack should be fully asleep");
    for h in &boxes {
        assert!(world.body(*h).unwrap().sleeping, "every box in the island should sleep");
    }

    // The payoff: a step over a sleeping scene runs no broadphase/narrowphase.
    world.step(FRAME_DT);
    assert_eq!(
        world.last_narrowphase_tests(),
        0,
        "an asleep scene must not run narrowphase — that's the whole point of sleeping"
    );
}

#[test]
fn impact_wakes_a_sleeping_stack() {
    let mut world = PhysicsWorld::new();
    world.sleep_frames = 20;
    let boxes = build_stack(&mut world, 5);
    for _ in 0..300 {
        world.step(FRAME_DT);
    }
    assert_eq!(world.awake_body_count(), 0, "precondition: stack asleep");

    // Drop a fast sphere straight onto the top of the stack.
    world.add_rigid_body(
        RigidBody::dynamic(Vec3::new(0.0, 9.0, 0.0), 2.0, Collider::Sphere { radius: 0.5 })
            .with_linear_velocity(Vec3::new(0.0, -25.0, 0.0)),
    );

    let mut stack_woke = false;
    for _ in 0..120 {
        world.step(FRAME_DT);
        // More than the projectile is awake → the stack got woken by the impact.
        if world.awake_body_count() > 1 {
            stack_woke = true;
            break;
        }
    }
    assert!(stack_woke, "the impact should have woken the sleeping stack");
    // The top box specifically is the one the projectile struck.
    let top = *boxes.last().unwrap();
    assert!(!world.body(top).unwrap().sleeping, "the struck box must be awake");
}
