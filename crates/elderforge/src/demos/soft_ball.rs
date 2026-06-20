//! Soft-ball demo: a tetrahedral soft body dropped onto a static table.
//!
//! The ball is a lattice sphere ([`SoftBodyDef::ball`]) knit together by edge
//! distance constraints and per-tet volume constraints, so when it lands it
//! squashes against the table top and then springs back while keeping its
//! volume — the volume constraints are what stop it from simply collapsing. Its
//! deforming surface is rendered by the deformable mesh path; the table and
//! ground are ordinary static render entities with matching colliders.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, Transform};
use elderforge_physics::{Collider, RigidBody, SoftBodyDef};
use elderforge_scene::Scene;

use super::{spawn_camera, DemoAssets};

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    spawn_camera(scene, Vec3::new(4.0, 3.2, 6.0), Vec3::new(0.0, 1.3, 0.0));

    // Ground: a render plane plus a static half-space, for depth under the table.
    scene.world.spawn((
        Transform::default(),
        MeshRenderer { mesh: assets.plane, material: assets.material },
    ));
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));

    // A static table: a wide, thin slab with its top surface at y = top.
    let table_center = Vec3::new(0.0, 1.0, 0.0);
    let table_half = Vec3::new(1.6, 0.1, 1.6);
    scene.world.spawn((
        Transform {
            position: table_center,
            rotation: Quat::IDENTITY,
            // Cube mesh is half-extent 0.5, so scale = full size = 2 · half.
            scale: table_half * 2.0,
        },
        MeshRenderer { mesh: assets.cube, material: assets.material },
    ));
    scene.physics.add_rigid_body(RigidBody::fixed(
        table_center,
        Collider::Box { half_extents: table_half },
    ));

    // The soft ball, dropped a short distance above the table top.
    let top = table_center.y + table_half.y;
    let radius = 0.6;
    let center = Vec3::new(0.0, top + radius + 0.4, 0.0);
    let mut ball = SoftBodyDef::ball(center, radius, 5, 4.0);
    // Springy edges over an (almost) incompressible interior: it wobbles and
    // squashes on impact but keeps its bulk.
    ball.distance_compliance = 2e-5;
    ball.volume_compliance = 1e-6;
    ball.particle_radius = 0.06;
    scene.physics.add_soft_body(&ball);
}
