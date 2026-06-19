//! Stress demo: 500 mixed shapes (spheres, boxes, capsules) poured from a tall
//! cloud onto the ground, boxed into a square pit so they pile up in view.
//!
//! This is the throughput demo — the headline is the **Stats** panel, where the
//! frame time and physics-step time climb as the cloud lands and the contact
//! count peaks, then ease off as the pile settles and islands fall asleep. The
//! mix of shape types exercises every world contact path: sphere/sphere,
//! box/box, capsule/capsule and the cross pairs all run through GJK/EPA, and the
//! four invisible walls plus the floor are half-space contacts.
//!
//! Contacts are linear-only (no angular term), so bodies don't pick up spin from
//! collisions; each box and capsule is therefore poured at a *fixed* random
//! orientation (zero angular velocity) so the settled pile still looks varied
//! without anything spinning in place forever.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{Collider, RigidBody};
use elderforge_scene::Scene;

use super::{
    material_with_restitution, spawn_camera, DemoAssets, Rng, CAPSULE_BASE_HALF_HEIGHT,
    CAPSULE_BASE_RADIUS,
};

/// Number of dynamic shapes poured into the pit.
const COUNT: usize = 500;
/// Half-width of the square pit (walls at ±EXTENT on X and Z).
const EXTENT: f32 = 5.0;
/// Half-extent of the cube mesh, so a box body of half-extent `h` renders at
/// uniform scale `h / CUBE_MESH_HALF`.
const CUBE_MESH_HALF: f32 = 0.5;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    // 500 bodies make every substep expensive; trade some stiffness for
    // throughput so the demo stays interactive while the Stats panel shows the
    // load. The substep slider in the editor can push it back up live.
    scene.physics.substeps = 8;

    // A raised three-quarter view framing the whole pit and the column of
    // falling shapes above it.
    spawn_camera(scene, Vec3::new(12.0, 16.0, 24.0), Vec3::new(0.0, 5.0, 0.0));

    // Low restitution so the pour bleeds energy and settles into a pile.
    let material = material_with_restitution(0.1);

    // --- Static geometry: floor (drawn) + four invisible containing walls. ---
    scene
        .world
        .spawn((Transform::default(), MeshRenderer { mesh: assets.plane, material: assets.material }));
    scene.physics.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 })
            .with_material(material),
    );
    // Walls: each half-space's solid region is the outside of the pit, so bodies
    // are pushed back in. (normal, offset) pairs mirror the avalanche walls.
    for (normal, offset) in [
        (Vec3::new(1.0, 0.0, 0.0), -EXTENT),  // left  (x > -EXTENT)
        (Vec3::new(-1.0, 0.0, 0.0), -EXTENT), // right (x <  EXTENT)
        (Vec3::new(0.0, 0.0, 1.0), -EXTENT),  // back  (z > -EXTENT)
        (Vec3::new(0.0, 0.0, -1.0), -EXTENT), // front (z <  EXTENT)
    ] {
        scene
            .physics
            .add_rigid_body(RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal, offset }));
    }

    // --- The pour: COUNT shapes in a tall cloud above the pit. ---
    let mut rng = Rng::new(0xDEAD_BEEF);
    // Keep spawn points inset from the walls by the largest body radius so
    // nothing starts already intersecting a wall.
    let span = EXTENT - 0.8;
    for _ in 0..COUNT {
        let x = rng.range(-span, span);
        let z = rng.range(-span, span);
        let y = rng.range(3.0, 34.0);
        let position = Vec3::new(x, y, z);

        // Pick a shape type: 0 sphere, 1 box, 2 capsule.
        let kind = (rng.range(0.0, 3.0) as u32).min(2);
        let (collider, mesh, scale, rotation) = match kind {
            0 => {
                let radius = rng.range(0.25, 0.4);
                (
                    Collider::Sphere { radius },
                    assets.sphere,
                    Vec3::splat(radius),
                    Quat::IDENTITY,
                )
            }
            1 => {
                let half = rng.range(0.25, 0.4);
                (
                    Collider::Box { half_extents: Vec3::splat(half) },
                    assets.cube,
                    Vec3::splat(half / CUBE_MESH_HALF),
                    random_orientation(&mut rng),
                )
            }
            _ => {
                let s = rng.range(0.7, 1.1);
                (
                    Collider::Capsule {
                        radius: CAPSULE_BASE_RADIUS * s,
                        half_height: CAPSULE_BASE_HALF_HEIGHT * s,
                    },
                    assets.capsule,
                    Vec3::splat(s),
                    random_orientation(&mut rng),
                )
            }
        };

        let mut body = RigidBody::dynamic(position, 1.0, collider).with_material(material);
        body.rotation = rotation;
        body.prev_rotation = rotation;
        let handle = scene.physics.add_rigid_body(body);
        scene.world.spawn((
            Transform { position, rotation, scale },
            PhysicsBody { handle },
            MeshRenderer { mesh, material: assets.material },
        ));
    }
}

/// A random orientation: a random unit axis (falling back to +Y if the draw
/// degenerates) turned by a random angle. Used to vary the poured boxes and
/// capsules so the settled pile looks natural.
fn random_orientation(rng: &mut Rng) -> Quat {
    let axis = Vec3::new(
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
    )
    .normalize_or_zero();
    let axis = if axis == Vec3::ZERO { Vec3::Y } else { axis };
    let angle = rng.range(0.0, std::f32::consts::TAU);
    Quat::from_axis_angle(axis, angle)
}
