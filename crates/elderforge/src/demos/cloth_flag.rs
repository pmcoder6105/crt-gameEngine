//! Cloth-flag demo: a banner pinned at its two top corners, billowing in a
//! steady wind.
//!
//! The cloth is a grid of particles wired with structural, shear, and bending
//! distance constraints; pinning the two top corners (zero inverse mass) leaves
//! the rest free. A flat sheet at its natural width just hangs taut, so the
//! demo turns on a steady out-of-plane [`wind`](elderforge_physics::PhysicsWorld::wind)
//! to make it billow into a curved surface, lightly damped so it keeps moving.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, Transform};
use elderforge_physics::ClothDef;
use elderforge_scene::Scene;

use super::{spawn_camera, DemoAssets};

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    let (cols, rows) = (28usize, 18usize);
    let spacing = 0.16;
    let width = (cols - 1) as f32 * spacing;
    let top = 4.0;
    let x0 = -width / 2.0;

    spawn_camera(
        scene,
        Vec3::new(3.5, top - 1.2, 6.0),
        Vec3::new(0.0, top - 1.6, 0.5),
    );

    // Ground plane far below, for depth.
    scene.world.spawn((
        Transform::default(),
        MeshRenderer { mesh: assets.plane, material: assets.material },
    ));

    // A horizontal bar along the top edge the flag hangs from (render only).
    scene.world.spawn((
        Transform {
            position: Vec3::new(0.0, top + 0.04, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(width + 0.5, 0.08, 0.08),
        },
        MeshRenderer { mesh: assets.cube, material: assets.material },
    ));

    // The flag, in the XY plane, pinned at the two top corners.
    let mut def = ClothDef::grid(
        cols,
        rows,
        1.5,
        |c, r| Vec3::new(x0 + c as f32 * spacing, top - r as f32 * spacing, 0.0),
        |c, r| r == 0 && (c == 0 || c == cols - 1),
    );
    def.shear_compliance = 1e-4;
    def.bending_compliance = 2e-3; // soft bending so the fabric ripples
    def.particle_radius = 0.02;
    scene.physics.add_cloth(&def);

    // A steady breeze out of the flag's plane, lightly damped so it billows and
    // keeps stirring rather than snapping to a static pose.
    scene.physics.wind = Vec3::new(0.0, 0.0, 7.0);
    scene.physics.particle_damping = 0.2;
}
