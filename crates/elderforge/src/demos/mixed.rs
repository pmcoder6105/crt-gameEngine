//! Mixed demo: rigid, soft, and cloth bodies all interacting in one scene.
//!
//! A soft ball is launched down a tilted ramp and plows into a stack of light
//! rigid boxes at the bottom, scattering them (two-way particle↔rigid contact
//! does the shoving; contacts are linear-only, so the boxes are knocked apart
//! rather than tumbling end-over-end). Off to the side a cloth flag billows in a
//! steady wind. The camera is pulled back and up to frame the whole sequence —
//! ramp, impact, and flag — at once.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{ClothDef, Collider, PhysicsMaterial, RigidBody, SoftBodyDef};
use elderforge_scene::Scene;

use super::{spawn_camera, DemoAssets};

/// Ramp tilt from horizontal (downhill is +X).
const TILT_DEG: f32 = 22.0;
/// X position of the box stack at the bottom of the ramp.
const STACK_X: f32 = 5.0;
/// Number of boxes in the stack.
const STACK_HEIGHT: usize = 5;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) {
    // A soft body, a cloth, and a toppling stack make every substep busy; trim
    // the substep count a little for throughput.
    scene.physics.substeps = 12;

    // Pulled-back elevated view framing the ramp (left), the stack (right), and
    // the flag (back-left) together.
    spawn_camera(scene, Vec3::new(3.0, 9.0, 21.0), Vec3::new(1.5, 2.5, -1.5));

    let tilt = (TILT_DEG as f32).to_radians();
    let (sin_t, cos_t) = (tilt.sin(), tilt.cos());
    let ramp_normal = Vec3::new(sin_t, cos_t, 0.0);
    let slope = sin_t / cos_t; // ramp surface y at x is -slope * x.

    // Slippery ramp so the soft ball keeps its speed into the stack.
    let slick = PhysicsMaterial {
        static_friction: 0.1,
        dynamic_friction: 0.05,
        ..PhysicsMaterial::default()
    };

    // --- Static geometry. ---
    // Ramp through the origin (downhill +X) and a flat floor at y = 0.
    scene.physics.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: ramp_normal, offset: 0.0 })
            .with_material(slick),
    );
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));

    // Render the floor and the ramp surface.
    scene.world.spawn((
        Transform::default(),
        MeshRenderer { mesh: assets.plane, material: assets.material },
    ));
    let ramp_center_x = -5.0;
    scene.world.spawn((
        Transform {
            position: Vec3::new(ramp_center_x, -slope * ramp_center_x, 0.0),
            rotation: Quat::from_rotation_z(-tilt),
            scale: Vec3::splat(0.4),
        },
        MeshRenderer { mesh: assets.plane, material: assets.material },
    ));

    // --- The rigid box stack at the bottom of the ramp. ---
    let box_half = 0.5;
    for i in 0..STACK_HEIGHT {
        let pos = Vec3::new(STACK_X, box_half + i as f32 * (box_half * 2.0), 0.0);
        let handle = scene.physics.add_rigid_body(RigidBody::dynamic(
            pos,
            1.0,
            Collider::Box { half_extents: Vec3::splat(box_half) },
        ));
        scene.world.spawn((
            Transform { position: pos, rotation: Quat::IDENTITY, scale: Vec3::splat(box_half * 2.0) },
            PhysicsBody { handle },
            MeshRenderer { mesh: assets.cube, material: assets.material },
        ));
    }

    // --- The soft ball, started up the ramp and launched down it. ---
    let ball_x = -7.0;
    let ball_radius = 0.7;
    let ball_center = Vec3::new(ball_x, -slope * ball_x + ball_radius + 0.1, 0.0);
    let mut ball = SoftBodyDef::ball(ball_center, ball_radius, 4, 8.0);
    ball.distance_compliance = 1e-5; // fairly stiff so it holds shape as it hits
    ball.volume_compliance = 1e-6;
    ball.particle_radius = 0.06;
    let handle = scene.physics.add_soft_body(&ball);
    // Give it a running start down the slope so it reaches the stack with punch.
    let (base, count) = {
        let sb = scene.physics.soft_body(handle).expect("just added");
        (sb.base(), sb.particle_count())
    };
    let launch = Vec3::new(5.0, -slope * 5.0, 0.0); // along the downhill direction
    let particles = scene.physics.particles_mut();
    for k in 0..count {
        particles[base + k].velocity = launch;
    }

    // --- The cloth flag off to the back-left, billowing in the wind. ---
    let (cols, rows) = (24usize, 16usize);
    let spacing = 0.16;
    let pole_x = -3.0;
    let pole_z = -4.0;
    let top = 6.0;
    // A render-only pole to hang the flag from.
    scene.world.spawn((
        Transform {
            position: Vec3::new(pole_x, top / 2.0, pole_z),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(0.1, top, 0.1),
        },
        MeshRenderer { mesh: assets.cube, material: assets.material },
    ));
    let mut flag = ClothDef::grid(
        cols,
        rows,
        1.5,
        |c, r| Vec3::new(pole_x + c as f32 * spacing, top - r as f32 * spacing, pole_z),
        // Pin the two corners of the edge against the pole.
        |c, r| c == 0 && (r == 0 || r == rows - 1),
    );
    flag.shear_compliance = 1e-4;
    flag.bending_compliance = 2e-3;
    flag.particle_radius = 0.02;
    scene.physics.add_cloth(&flag);

    // Wind blows the flag out to +X (away from the pole) and keeps it stirring.
    scene.physics.wind = Vec3::new(6.0, 0.0, 2.0);
    scene.physics.particle_damping = 0.2;
}
