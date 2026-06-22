//! Debug-capture demo: a sphere pile that settles, sleeps, and is burst awake by
//! a contained shockwave — all under every debug overlay at once.
//!
//! Sixteen spheres drop into a walled pit and settle into a sleeping heap, at
//! which point the sleep-state overlay dims them and the scene goes quiet. Five
//! seconds in, a one-shot radial shockwave ([`DemoAnim::Shockwave`]) blasts
//! every sphere outward from the pile's center, waking the whole island; they
//! scatter, ricochet off the (invisible) pit walls, and re-settle. With all
//! eight overlay layers on, the burst lights up the entire visualization —
//! velocity vectors, contacts, BVH refitting, force arrows, and the sleep
//! overlay flicking from dim back to bright.
//!
//! This is the sphere cousin of a toppling box stack. The engine's box-box
//! contacts are linear-only and detonate when a settled box stack is disturbed,
//! so a contained sphere pile is what reliably delivers the settle → sleep →
//! wake-burst this capture is after.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{Collider, DebugLayers, RigidBody};
use elderforge_scene::Scene;

use super::{
    material_with_restitution, spawn_camera, DebugScript, DemoAnim, DemoAssets, DemoConfig, Rng,
};

/// Number of spheres in the pile.
const COUNT: usize = 16;
/// Radius of each sphere.
const RADIUS: f32 = 0.4;
/// Half-width of the square pit (invisible static walls at ±this in X and Z).
const PIT_HALF: f32 = 2.5;
/// When (seconds) the shockwave fires — after the pile has settled and slept.
const BURST_AT: f32 = 5.0;
/// Outward speed the shockwave imparts (m/s). Moderate, so the spheres stay
/// contained in the pit and re-settle instead of being flung out.
const BURST_SPEED: f32 = 5.0;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) -> DemoConfig {
    // Spheres settle quickly; a modest substep count keeps the pile stable while
    // staying cheap enough to run all overlays on top.
    scene.physics.substeps = 15;

    // Look down into the pit from a front corner, framing the whole pile and the
    // walls it bounces off.
    spawn_camera(scene, Vec3::new(5.5, 4.5, 7.0), Vec3::new(0.0, 0.8, 0.0));

    // Bouncy enough to scatter and ring on the burst, not so bouncy it never
    // sleeps in the first place.
    let mat = material_with_restitution(0.3);

    // Ground: render plane + static half-space.
    scene
        .world
        .spawn((Transform::default(), MeshRenderer { mesh: assets.plane, material: assets.material }));
    scene.physics.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 })
            .with_material(mat),
    );

    // Invisible pit walls (like the avalanche) so the burst stays in frame.
    for normal in [
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, -1.0),
        Vec3::new(0.0, 0.0, 1.0),
    ] {
        scene.physics.add_rigid_body(RigidBody::fixed(
            Vec3::ZERO,
            Collider::HalfSpace { normal, offset: -PIT_HALF },
        ));
    }

    // Drop the spheres in a loose column with a little horizontal spread, so they
    // tumble into a natural heap rather than a perfect tower.
    let mut rng = Rng::new(0x5111_F1A6);
    let mut handles = Vec::with_capacity(COUNT);
    for i in 0..COUNT {
        let position = Vec3::new(
            rng.range(-1.3, 1.3),
            RADIUS + 0.5 + i as f32 * 0.85,
            rng.range(-1.3, 1.3),
        );
        let handle = scene.physics.add_rigid_body(
            RigidBody::dynamic(position, 1.0, Collider::Sphere { radius: RADIUS })
                .with_material(mat),
        );
        handles.push(handle);
        scene.world.spawn((
            Transform { position, rotation: Quat::IDENTITY, scale: Vec3::splat(RADIUS) },
            PhysicsBody { handle },
            MeshRenderer { mesh: assets.sphere, material: assets.material },
        ));
    }

    DemoConfig {
        // Burst the settled, sleeping pile outward from its center on cue.
        anim: DemoAnim::Shockwave {
            handles,
            center: Vec3::new(0.0, 0.8, 0.0),
            speed: BURST_SPEED,
            at: BURST_AT,
            fired: false,
        },
        // Every overlay on, all at once.
        debug: DebugScript::Always(DebugLayers {
            collision_shapes: true,
            velocity_vectors: true,
            angular_velocity: true,
            contact_points: true,
            constraint_anchors: true,
            bvh_aabbs: true,
            sleep_state: true,
            force_accumulators: true,
        }),
        ..DemoConfig::default()
    }
}
