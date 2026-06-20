//! Cloth-drape demo: a sheet dropped onto a spinning cube.
//!
//! A free (un-pinned) cloth grid falls under gravity onto a dynamic cube that
//! carries an initial angular velocity. The particle↔rigid contacts catch the
//! cloth on the cube's faces, and contact friction drags the draped fabric
//! around as the cube turns — a direct showcase of two-way soft/rigid coupling.
//!
//! The cube spins about a near-vertical axis: the engine's contacts are
//! linear-only (they apply no torque), so a flat-resting cube tumbling
//! end-over-end isn't yet well supported, but a cube spinning on its base is
//! stable and drags the cloth convincingly.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{ClothDef, Collider, RigidBody};
use elderforge_scene::Scene;

use super::{spawn_camera, DemoAssets};

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    spawn_camera(scene, Vec3::new(4.5, 4.0, 5.5), Vec3::new(0.0, 1.0, 0.0));

    // Ground: render plane + static half-space.
    scene.world.spawn((
        Transform::default(),
        MeshRenderer { mesh: assets.plane, material: assets.material },
    ));
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));

    // The spinning cube, resting on the ground.
    let cube_half = 0.9;
    let cube_pos = Vec3::new(0.0, cube_half, 0.0);
    let handle = scene.physics.add_rigid_body(
        RigidBody::dynamic(
            cube_pos,
            8.0,
            Collider::Box { half_extents: Vec3::splat(cube_half) },
        )
        .with_angular_velocity(Vec3::new(0.0, 1.6, 0.0)),
    );
    scene.world.spawn((
        Transform {
            position: cube_pos,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(cube_half * 2.0),
        },
        PhysicsBody { handle },
        MeshRenderer { mesh: assets.cube, material: assets.material },
    ));

    // The cloth sheet, centered above the cube so it drapes over every edge.
    let (cols, rows) = (26usize, 26usize);
    let spacing = 0.12;
    let w = (cols - 1) as f32 * spacing;
    let d = (rows - 1) as f32 * spacing;
    let drop_y = 3.0;
    let mut def = ClothDef::grid(
        cols,
        rows,
        2.0,
        |c, r| Vec3::new(c as f32 * spacing - w / 2.0, drop_y, r as f32 * spacing - d / 2.0),
        |_, _| false,
    );
    def.shear_compliance = 1e-4;
    def.bending_compliance = 1e-3;
    def.particle_radius = 0.03;
    scene.physics.add_cloth(&def);

    // Keep the cube spinning for the showcase rather than letting it sleep.
    scene.physics.sleeping_enabled = false;
}
