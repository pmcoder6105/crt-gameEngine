//! Cloth-drape capture showcase: a high-resolution sheet draping over a slowly
//! rotating cube, lit warm, filmed by an orbiting camera.
//!
//! This is the footage-grade cousin of [`cloth_drape`](super::cloth_drape): a
//! 40×40 cloth is suspended by its two top corners just above a dynamic cube
//! that turns slowly about its vertical axis. The bulk of the sheet falls and
//! drapes over the cube; particle↔rigid friction drags the draped fabric around
//! as the cube rotates. The camera orbits the scene once every 30 seconds and a
//! warm directional key light comes from the upper left.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{ClothDef, Collider, RigidBody};
use elderforge_renderer::DirectionalLight;
use elderforge_scene::Scene;

use super::{spawn_camera, DemoAnim, DemoAssets, DemoConfig};

/// Camera orbit: where it looks, how far out it circles, and how high.
const ORBIT_CENTER: Vec3 = Vec3::new(0.0, 1.2, 0.0);
const ORBIT_RADIUS: f32 = 6.0;
const ORBIT_HEIGHT: f32 = 4.0;
/// One full revolution per 30 seconds.
const ORBIT_PERIOD: f32 = 30.0;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) -> DemoConfig {
    // Start the camera at the t = 0 point on the orbit so the opening frame
    // matches where the animation picks up.
    spawn_camera(
        scene,
        ORBIT_CENTER + Vec3::new(ORBIT_RADIUS, ORBIT_HEIGHT, 0.0),
        ORBIT_CENTER,
    );

    // A 40×40 cloth is a lot of constraints; trade a few substeps for a smooth
    // capture frame rate. Keep the scene awake so the cube never stops turning.
    scene.physics.substeps = 15;
    scene.physics.sleeping_enabled = false;

    // Ground: render plane + static half-space.
    scene.world.spawn((
        Transform::default(),
        MeshRenderer { mesh: assets.plane, material: assets.material },
    ));
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));

    // The slowly rotating cube the cloth drapes over.
    let cube_half = 0.9;
    let cube_pos = Vec3::new(0.0, cube_half, 0.0);
    let handle = scene.physics.add_rigid_body(
        RigidBody::dynamic(
            cube_pos,
            8.0,
            Collider::Box { half_extents: Vec3::splat(cube_half) },
        )
        // ~0.5 rad/s about Y: one slow turn every ~12 s.
        .with_angular_velocity(Vec3::new(0.0, 0.5, 0.0)),
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

    // A 40×40 sheet, laid flat just above the cube and pinned at its two front
    // corners so one edge is held up while the rest drapes over the cube.
    let (cols, rows) = (40usize, 40usize);
    let spacing = 0.08;
    let w = (cols - 1) as f32 * spacing;
    let d = (rows - 1) as f32 * spacing;
    let drop_y = 2.2;
    let mut def = ClothDef::grid(
        cols,
        rows,
        3.0,
        |c, r| Vec3::new(c as f32 * spacing - w / 2.0, drop_y, r as f32 * spacing - d / 2.0),
        // Pin the two corners of the r = 0 edge.
        |c, r| r == 0 && (c == 0 || c == cols - 1),
    );
    def.shear_compliance = 1e-4;
    def.bending_compliance = 2e-3; // soft bending so it folds over the cube edges
    def.particle_radius = 0.03;
    scene.physics.add_cloth(&def);

    DemoConfig {
        anim: DemoAnim::OrbitCamera {
            center: ORBIT_CENTER,
            radius: ORBIT_RADIUS,
            height: ORBIT_HEIGHT,
            period: ORBIT_PERIOD,
        },
        // Warm key light from the upper left.
        light: Some(DirectionalLight {
            direction: Vec3::new(-0.6, 0.85, 0.3),
            color: Vec3::new(1.0, 0.86, 0.66),
        }),
    }
}
