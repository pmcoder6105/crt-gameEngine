//! Cloth-tear demo: a sheet pinned along its whole top edge, taking a heavy
//! rigid sphere on its center.
//!
//! The cloth hangs as a vertical curtain, pinned across the entire top row. A
//! heavy sphere is launched into the upper-center and then dragged down by
//! gravity, stretching the fabric into a deep pocket. Tearing (constraint
//! breakage on overstretch) is not implemented in the solver, so this instead
//! showcases the **extreme deformation** the stretchy structural constraints
//! allow under a concentrated heavy load.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{ClothDef, Collider, RigidBody};
use elderforge_scene::Scene;

use super::{spawn_camera, DemoAssets};

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    // Fixed camera framing the whole curtain from the front-right.
    spawn_camera(scene, Vec3::new(5.0, 4.0, 7.5), Vec3::new(0.0, 3.5, 0.0));

    // Ground plane for depth (the sheet hangs well above it).
    scene.world.spawn((
        Transform::default(),
        MeshRenderer { mesh: assets.plane, material: assets.material },
    ));
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));

    // The curtain: a vertical sheet in the XY plane (normal +Z), pinned across
    // the entire top edge so it can stretch but never fall.
    let (cols, rows) = (36usize, 30usize);
    let spacing = 0.18;
    let width = (cols - 1) as f32 * spacing;
    let top = 6.0;
    let x0 = -width / 2.0;
    let mut def = ClothDef::grid(
        cols,
        rows,
        4.0,
        |c, r| Vec3::new(x0 + c as f32 * spacing, top - r as f32 * spacing, 0.0),
        |_, r| r == 0, // pin the whole top edge
    );
    // Stretchy structural springs so a heavy load deforms the sheet a lot
    // (standing in for tearing, which the solver doesn't model).
    def.structural_compliance = 5e-5;
    def.shear_compliance = 1e-4;
    def.bending_compliance = 1e-3;
    def.particle_radius = 0.04;
    scene.physics.add_cloth(&def);

    // A heavy sphere launched into the upper-center of the sheet; gravity then
    // drags it down, stretching the fabric into a deep pocket.
    let radius = 0.7;
    let start = Vec3::new(0.0, top - 0.6, 1.2);
    let handle = scene.physics.add_rigid_body(
        RigidBody::dynamic(start, 50.0, Collider::Sphere { radius })
            .with_linear_velocity(Vec3::new(0.0, 0.0, -3.0)),
    );
    scene.world.spawn((
        Transform { position: start, rotation: Quat::IDENTITY, scale: Vec3::splat(radius) },
        PhysicsBody { handle },
        MeshRenderer { mesh: assets.sphere, material: assets.material },
    ));
}
