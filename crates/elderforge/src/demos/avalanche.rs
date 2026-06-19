//! Avalanche demo: 200 spheres spawned above a tilted ramp, tumbling down and
//! piling against a wall at the bottom.
//!
//! This is the broadphase-and-solver stress demo: a couple hundred dynamic
//! spheres means the BVH broadphase has real work to do every substep, and the
//! pile at the bottom is a dense knot of simultaneous contacts for the XPBD
//! solver. The bottom is boxed in by invisible static half-space walls so the
//! spheres accumulate in view instead of scattering off to infinity.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{Collider, RigidBody};
use elderforge_scene::Scene;

use super::{material_with_restitution, spawn_camera, DemoAssets, Rng};

/// Number of spheres in the avalanche.
const COUNT: usize = 200;
/// Radius of each sphere.
const RADIUS: f32 = 0.3;
/// Ramp tilt from horizontal, in degrees. Downhill is +X.
const TILT_DEG: f32 = 26.0;
/// X of the wall the spheres pile against at the bottom.
const WALL_X: f32 = 10.0;
/// Half-width of the channel in Z (side walls at ±HALF_WIDTH).
const HALF_WIDTH: f32 = 4.0;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    // Many bodies make every substep expensive, so trade a little stiffness for
    // throughput — a frictionless sphere pile doesn't need the full 20.
    scene.physics.substeps = 10;

    // A wide, slightly elevated front view that frames both the ramp (back
    // left) and the pile against the far wall (front right).
    spawn_camera(
        scene,
        Vec3::new(2.0, 12.0, 28.0),
        Vec3::new(-1.0, 3.0, 0.0),
    );

    let tilt = TILT_DEG.to_radians();
    let (sin_t, cos_t) = (tilt.sin(), tilt.cos());
    // Ramp plane normal: +Y rotated toward +X, so the surface descends as x
    // grows and gravity drives bodies downhill in +X.
    let ramp_normal = Vec3::new(sin_t, cos_t, 0.0);
    let slope = sin_t / cos_t; // ramp surface y at a given x is -slope * x.

    // Low restitution so the avalanche loses energy and settles into a pile
    // rather than bouncing forever.
    let gravelly = material_with_restitution(0.1);

    // --- Static geometry (half-spaces). Only the ramp and floor are drawn. ---
    // Ramp through the origin.
    scene.physics.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: ramp_normal, offset: 0.0 })
            .with_material(gravelly),
    );
    // Flat floor at y = 0 (the higher surface for x > 0, where the ramp dips
    // below it — this is what the spheres come to rest on).
    scene.physics.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 })
            .with_material(gravelly),
    );
    // Back wall the spheres pile against (solid for x > WALL_X).
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::new(-1.0, 0.0, 0.0), offset: -WALL_X },
    ));
    // Side walls keep the channel narrow so the pile stays in frame.
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::new(0.0, 0.0, 1.0), offset: -HALF_WIDTH },
    ));
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::new(0.0, 0.0, -1.0), offset: -HALF_WIDTH },
    ));

    // Render the floor (full ground plane) and the ramp (a scaled plane rotated
    // to match the ramp collider, centered up the slope).
    scene
        .world
        .spawn((Transform::default(), MeshRenderer { mesh: assets.plane, material: assets.material }));
    let ramp_center_x = -6.0;
    scene.world.spawn((
        Transform {
            position: Vec3::new(ramp_center_x, -slope * ramp_center_x, 0.0),
            rotation: Quat::from_rotation_z(-tilt),
            scale: Vec3::splat(0.3),
        },
        MeshRenderer { mesh: assets.plane, material: assets.material },
    ));

    // --- The avalanche: a cloud of spheres above the upper (−X) ramp. ---
    let mut rng = Rng::new(0x5EED_1234);
    for _ in 0..COUNT {
        let x = rng.range(-13.0, -5.0);
        let z = rng.range(-(HALF_WIDTH - 0.7), HALF_WIDTH - 0.7);
        // Stack them above the ramp surface at this x.
        let y = -slope * x + rng.range(1.0, 7.0);
        let position = Vec3::new(x, y, z);
        let handle = scene.physics.add_rigid_body(
            RigidBody::dynamic(position, 1.0, Collider::Sphere { radius: RADIUS })
                .with_material(gravelly),
        );
        scene.world.spawn((
            Transform { position, rotation: Quat::IDENTITY, scale: Vec3::splat(RADIUS) },
            PhysicsBody { handle },
            MeshRenderer { mesh: assets.sphere, material: assets.material },
        ));
    }
}
