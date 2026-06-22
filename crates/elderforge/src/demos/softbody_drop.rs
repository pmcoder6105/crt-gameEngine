//! Softbody-drop demo: three soft balls of increasing softness dropped onto a
//! table one at a time, two seconds apart.
//!
//! Each ball is a volume-constrained tet-lattice sphere ([`SoftBodyDef::ball`]),
//! but with a different edge/volume compliance: the first is stiff and barely
//! deforms, the second wobbles, and the third is so soft it flattens noticeably
//! on impact. All three are spawned at once but **pinned frozen** in mid-air
//! (zero inverse mass); the app's [`StagedDrop`](super::DemoAnim::StagedDrop)
//! animation restores each ball's masses on a 0 s / 2 s / 4 s schedule, so they
//! fall in sequence. Fixed camera, angled slightly down at the table.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{MeshRenderer, Transform};
use elderforge_physics::{Collider, RigidBody, SoftBodyDef};
use elderforge_scene::Scene;

use super::{spawn_camera, DemoAnim, DemoAssets, DemoConfig, StagedRelease};

/// Ball radius and lattice resolution (cells across the diameter).
const RADIUS: f32 = 0.5;
const RESOLUTION: u32 = 5;
const MASS: f32 = 3.0;

pub fn setup(scene: &mut Scene, assets: &DemoAssets) -> DemoConfig {
    // Fixed camera, looking slightly down at the table from the front.
    spawn_camera(scene, Vec3::new(0.0, 4.5, 8.0), Vec3::new(0.0, 0.8, 0.0));

    // Ground plane + half-space, for depth beneath the table.
    scene.world.spawn((
        Transform::default(),
        MeshRenderer { mesh: assets.plane, material: assets.material },
    ));
    scene.physics.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));

    // A wide static table; its top surface is at y = table top.
    let table_center = Vec3::new(0.0, 1.0, 0.0);
    let table_half = Vec3::new(3.0, 0.1, 1.6);
    scene.world.spawn((
        Transform {
            position: table_center,
            rotation: Quat::IDENTITY,
            scale: table_half * 2.0,
        },
        MeshRenderer { mesh: assets.cube, material: assets.material },
    ));
    scene.physics.add_rigid_body(RigidBody::fixed(
        table_center,
        Collider::Box { half_extents: table_half },
    ));

    // Three balls in a row, dropped from the same height. `(x, distance
    // compliance, volume compliance, release time)` — compliance grows left to
    // right, so the rightmost ball is the squashiest.
    let table_top = table_center.y + table_half.y;
    let drop_y = table_top + RADIUS + 1.2;
    let balls = [
        (-1.6_f32, 1e-6_f32, 1e-7_f32, 0.0_f32), // stiff: barely deforms
        (0.0, 8e-5, 3e-5, 2.0),                   // medium: wobbles
        (1.6, 8e-4, 8e-4, 4.0),                   // soft: flattens noticeably
    ];

    let mut releases = Vec::new();
    for &(x, distance_compliance, volume_compliance, release_at) in &balls {
        let center = Vec3::new(x, drop_y, 0.0);
        let mut def = SoftBodyDef::ball(center, RADIUS, RESOLUTION, MASS);
        def.distance_compliance = distance_compliance;
        def.volume_compliance = volume_compliance;
        def.particle_radius = 0.06;
        let handle = scene.physics.add_soft_body(&def);

        // Snapshot the body's particle run, then pin every particle (zero
        // inverse mass) so it hangs motionless until its release time.
        let (base, count) = {
            let sb = scene.physics.soft_body(handle).expect("just added");
            (sb.base(), sb.particle_count())
        };
        let particles = scene.physics.particles_mut();
        let inv_masses: Vec<f32> = (0..count).map(|k| particles[base + k].inv_mass).collect();
        for k in 0..count {
            particles[base + k].inv_mass = 0.0;
        }
        releases.push(StagedRelease { base, inv_masses, release_at });
    }

    DemoConfig {
        anim: DemoAnim::StagedDrop(releases),
        light: None,
    }
}
