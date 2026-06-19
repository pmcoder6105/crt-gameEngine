//! Pendulum demo: a chain of 10 spheres pinned at the top and connected by
//! XPBD distance constraints, released from horizontal so it swings.
//!
//! The links are rigid (zero compliance), so this is the same stiff-rope setup
//! the solver test exercises — released sideways it swings as a lively
//! multi-link pendulum, showing how XPBD holds a chain of distance constraints
//! together without the links stretching.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{Collider, RigidBody};
use elderforge_scene::Scene;

use super::{material_with_restitution, spawn_camera, DemoAssets};

/// Number of swinging spheres (not counting the fixed anchor).
const LINKS: usize = 10;
/// Rest length of each link (distance between sphere centers). Kept larger than
/// a sphere diameter so neighbours never touch — the chain's shape comes from
/// the constraints, not contacts.
const LINK: f32 = 1.15;
/// Radius of each swinging sphere.
const RADIUS: f32 = 0.5;
/// Where the chain is pinned.
const ANCHOR: Vec3 = Vec3::new(0.0, 13.0, 0.0);

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    // A slightly angled view that frames the whole arc the chain sweeps, with
    // the ground for depth reference.
    spawn_camera(
        scene,
        Vec3::new(3.0, 9.0, 25.0),
        Vec3::new(0.0, 6.0, 0.0),
    );

    let material = material_with_restitution(0.0);

    // A ground plane well below the swing, purely for depth reference (the
    // chain hangs to ~y = 1.5 at most, so it never reaches the floor).
    scene
        .world
        .spawn((Transform::default(), MeshRenderer { mesh: assets.plane, material: assets.material }));
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));

    // Fixed anchor: a small static sphere, rendered so the pin is visible.
    let anchor = scene.physics.add_rigid_body(RigidBody::fixed(
        ANCHOR,
        Collider::Sphere { radius: 0.25 },
    ));
    spawn_sphere(scene, assets, ANCHOR, 0.25);

    // The chain, laid out horizontally in +X so it falls and swings when
    // released. Each link is a rigid (zero-compliance) distance constraint to
    // the previous body.
    let mut prev = anchor;
    for i in 1..=LINKS {
        let position = ANCHOR + Vec3::new(i as f32 * LINK, 0.0, 0.0);
        let handle = scene.physics.add_rigid_body(
            RigidBody::dynamic(position, 1.0, Collider::Sphere { radius: RADIUS })
                .with_material(material),
        );
        scene.physics.add_distance_constraint(prev, handle, LINK, 0.0);

        scene.world.spawn((
            Transform { position, rotation: Quat::IDENTITY, scale: Vec3::splat(RADIUS) },
            PhysicsBody { handle },
            MeshRenderer { mesh: assets.sphere, material: assets.material },
        ));
        prev = handle;
    }
}

/// Spawn a render-only sphere (no physics body) at `position` scaled to
/// `radius`. Used for the static anchor marker.
fn spawn_sphere(scene: &mut Scene, assets: &DemoAssets, position: Vec3, radius: f32) {
    scene.world.spawn((
        Transform { position, rotation: Quat::IDENTITY, scale: Vec3::splat(radius) },
        MeshRenderer { mesh: assets.sphere, material: assets.material },
    ));
}
