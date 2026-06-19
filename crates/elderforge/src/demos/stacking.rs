//! Stacking demo: a tower of 20 unit boxes dropped onto a ground plane.
//!
//! The boxes start axis-aligned and centered, each with a small gap above the
//! one below, so they fall a short distance and settle into contact. This is
//! the scenario the XPBD contact solver handles best (axis-aligned, centered
//! contacts are linear-exact), so the finished stack holds without jitter or
//! slow sink — a direct demonstration of constraint stability.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{Collider, RigidBody};
use elderforge_scene::Scene;

use super::{material_with_restitution, spawn_camera, DemoAssets};

/// Number of boxes in the tower.
const COUNT: usize = 20;
/// Half-extent of each box; the cube mesh is half-extent 0.5, so a body of this
/// size renders with unit scale.
const HALF: f32 = 0.5;
/// Vertical gap between resting boxes, before settling. Small, so the drop is a
/// brief settle rather than a slam that could bounce the tower.
const GAP: f32 = 0.04;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    // A three-quarter view angled to look up the tower and show its depth.
    spawn_camera(
        scene,
        Vec3::new(13.0, 11.0, 20.0),
        Vec3::new(0.0, 8.0, 0.0),
    );

    // Matte material: no bounce, so the stack settles instead of ringing.
    let matte = material_with_restitution(0.0);

    // Ground: a render plane plus a static half-space at y = 0.
    scene
        .world
        .spawn((Transform::default(), MeshRenderer { mesh: assets.plane, material: assets.material }));
    scene.physics.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 })
            .with_material(matte),
    );

    // The tower: COUNT unit boxes stacked along +Y with a small starting gap.
    let pitch = 2.0 * HALF + GAP;
    for i in 0..COUNT {
        let y = HALF + i as f32 * pitch;
        let position = Vec3::new(0.0, y, 0.0);
        let handle = scene.physics.add_rigid_body(
            RigidBody::dynamic(position, 1.0, Collider::Box { half_extents: Vec3::splat(HALF) })
                .with_material(matte),
        );
        scene.world.spawn((
            Transform { position, rotation: Quat::IDENTITY, scale: Vec3::ONE },
            PhysicsBody { handle },
            MeshRenderer { mesh: assets.cube, material: assets.material },
        ));
    }
}
