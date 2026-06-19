//! Sandbox demo: a near-empty scene — a ground plane and five cubes — built to
//! show off the editor rather than the physics.
//!
//! With so few entities every panel is legible: the hierarchy lists the five
//! cubes by id, the inspector edits a selected cube's transform live, and the
//! simulation controls are easy to reason about (the cubes start just above the
//! ground, so Play drops them a short way and they settle — pause/step/the
//! timestep multiplier all have an obvious, immediate effect). The stats panel
//! shows the tiny entity/body counts and a comfortably low frame time.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{Collider, RigidBody};
use elderforge_scene::Scene;

use super::{material_with_restitution, spawn_camera, DemoAssets};

/// Number of cubes to spawn.
const COUNT: usize = 5;
/// Half-extent of each cube; the cube mesh is half-extent 0.5, so a body of this
/// size renders at unit scale.
const HALF: f32 = 0.5;
/// Spacing between cubes along X.
const SPACING: f32 = 2.5;
/// Height the cubes start at, a short drop above their resting height (`HALF`),
/// so pressing Play produces a small, clear settle.
const START_Y: f32 = 1.5;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    // A head-on, slightly raised view that frames the whole row of cubes and the
    // ground in front of them — comfortable for clicking entities to select.
    spawn_camera(scene, Vec3::new(0.0, 5.0, 14.0), Vec3::new(0.0, 1.0, 0.0));

    // A touch of bounce so the settle is visible but doesn't ring.
    let material = material_with_restitution(0.2);

    // Ground: a render plane plus a static half-space at y = 0.
    scene
        .world
        .spawn((Transform::default(), MeshRenderer { mesh: assets.plane, material: assets.material }));
    scene.physics.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 })
            .with_material(material),
    );

    // A row of cubes, centered on the origin, each a separate selectable entity.
    let start_x = -(COUNT as f32 - 1.0) * 0.5 * SPACING;
    for i in 0..COUNT {
        let position = Vec3::new(start_x + i as f32 * SPACING, START_Y, 0.0);
        let handle = scene.physics.add_rigid_body(
            RigidBody::dynamic(position, 1.0, Collider::Box { half_extents: Vec3::splat(HALF) })
                .with_material(material),
        );
        scene.world.spawn((
            Transform { position, rotation: Quat::IDENTITY, scale: Vec3::ONE },
            PhysicsBody { handle },
            MeshRenderer { mesh: assets.cube, material: assets.material },
        ));
    }
}
