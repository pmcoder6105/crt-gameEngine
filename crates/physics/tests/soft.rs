//! Integration tests for soft bodies and cloth driven through the full
//! `PhysicsWorld` substep loop: a cloth flag hangs from pinned corners without
//! stretching, a soft ball drops onto the ground while preserving its volume,
//! and soft particles couple to a rigid body through contacts.

use elderforge_core::math::Vec3;
use elderforge_physics::soft::signed_tet_volume;
use elderforge_physics::{ClothDef, Collider, PhysicsWorld, RigidBody, SoftBodyDef};

/// Total (unsigned) volume of a soft body's tets at the current particle
/// positions. The Kuhn tets alternate in winding, so their *signed* volumes sum
/// to ~zero; the body's bulk is the sum of magnitudes.
fn current_volume(world: &PhysicsWorld, base: usize, def: &SoftBodyDef) -> f32 {
    let ps = world.particles();
    def.tets
        .iter()
        .map(|(t, _)| {
            signed_tet_volume(
                ps[base + t[0] as usize].position,
                ps[base + t[1] as usize].position,
                ps[base + t[2] as usize].position,
                ps[base + t[3] as usize].position,
            )
            .abs()
        })
        .sum()
}

#[test]
fn cloth_flag_billows_in_wind_from_pinned_corners() {
    let (cols, rows) = (12usize, 8usize);
    let spacing = 0.25;
    let top = 3.0;
    // A flag in the XY plane; pin the two top corners.
    let def = ClothDef::grid(
        cols,
        rows,
        1.0,
        |c, r| Vec3::new(c as f32 * spacing, top - r as f32 * spacing, 0.0),
        |c, r| r == 0 && (c == 0 || c == cols - 1),
    );
    let pinned_left = def.particles[0];
    let pinned_right = def.particles[cols - 1];

    let mut world = PhysicsWorld::new();
    world.iterations = 8; // a 2D sheet of rigid links wants a few more sweeps
    world.wind = Vec3::new(0.0, 0.0, 6.0); // steady breeze, out of the flag's plane
    let handle = world.add_cloth(&def);

    for _ in 0..300 {
        world.step(1.0 / 60.0);
    }

    let ps = world.cloth_particles(handle).expect("cloth particles");
    assert!(ps.iter().all(|p| p.position.is_finite()), "cloth went non-finite");

    // The pinned corners never move.
    assert!((ps[0].position - pinned_left).length() < 1e-5);
    assert!((ps[cols - 1].position - pinned_right).length() < 1e-5);

    // The free interior billows out of the original plane in the wind.
    let bottom_mid = ps[(rows - 1) * cols + cols / 2].position;
    assert!(bottom_mid.z > 0.2, "flag did not billow: z = {}", bottom_mid.z);
    // And it still hangs — the bottom edge sits below the pinned top.
    assert!(bottom_mid.y < top, "bottom edge above the pins: y = {}", bottom_mid.y);

    // Structural links stay close to their rest length — the fabric billows and
    // hangs, it does not stretch like rubber (structural compliance is zero).
    for &(a, b, rest) in &def.structural {
        let len = (ps[a as usize].position - ps[b as usize].position).length();
        assert!(
            len < rest * 1.15 + 1e-3,
            "structural link stretched {len} vs rest {rest}"
        );
    }
}

#[test]
fn soft_ball_drops_onto_ground_and_preserves_volume() {
    let center = Vec3::new(0.0, 2.0, 0.0);
    let radius = 0.6;
    let def = SoftBodyDef::ball(center, radius, 4, 3.0);
    let rest_volume: f32 = def.tets.iter().map(|(_, v)| v.abs()).sum();

    let mut world = PhysicsWorld::new();
    world.iterations = 6;
    // Static ground plane at y = 0.
    world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));
    let handle = world.add_soft_body(&def);
    let base = world.soft_body(handle).expect("soft body").base();

    for _ in 0..240 {
        world.step(1.0 / 120.0);
    }

    let ps = world.soft_body_particles(handle).expect("soft particles");
    assert!(ps.iter().all(|p| p.position.is_finite()), "soft body went non-finite");

    // It came to rest on (not through) the ground — particles have a small
    // collision radius, so allow a hair below the plane.
    let min_y = ps.iter().map(|p| p.position.y).fold(f32::INFINITY, f32::min);
    assert!(min_y > -0.15, "soft body sank through the ground: min y = {min_y}");
    // And it actually fell — its lowest point is well below the drop height.
    assert!(min_y < 1.0, "soft body never fell: min y = {min_y}");

    // Volume is preserved within a modest tolerance despite the impact.
    let vol = current_volume(&world, base, &def);
    let ratio = (vol / rest_volume).abs();
    assert!(
        (0.8..1.2).contains(&ratio),
        "volume not preserved: {vol} vs rest {rest_volume} (ratio {ratio})"
    );
}

#[test]
fn soft_ball_pushes_a_dynamic_body_it_lands_on() {
    // A soft ball dropped onto a light, free rigid sphere should shove it: the
    // particle↔rigid contact is two-way.
    let mut world = PhysicsWorld::new();
    world.gravity = Vec3::new(0.0, -9.81, 0.0);
    let ball = world.add_rigid_body(RigidBody::dynamic(
        Vec3::new(0.0, 0.0, 0.0),
        0.5,
        Collider::Sphere { radius: 0.4 },
    ));
    let soft = SoftBodyDef::ball(Vec3::new(0.0, 1.2, 0.0), 0.5, 3, 4.0);
    world.add_soft_body(&soft);

    let start = world.body(ball).expect("body").position;
    for _ in 0..120 {
        world.step(1.0 / 120.0);
    }
    let end = world.body(ball).expect("body").position;
    assert!(end.is_finite());
    // The rigid sphere was driven downward (and possibly aside) by the soft
    // body landing on it — it did not stay exactly put.
    assert!((end - start).length() > 0.05, "rigid body was not pushed: moved {:?}", end - start);
}
