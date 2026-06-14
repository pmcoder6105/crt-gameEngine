//! Headless end-to-end check of the physics + forward-render path: builds the
//! same scene the binary does (ground plane + falling cubes), steps physics so
//! the cubes fall toward the plane, renders one frame through `ForwardPass`
//! into an offscreen texture, and reads it back. Asserts the frame is actually
//! populated (ground + cubes, not just the clear color) and dumps a PPM to
//! `$TMPDIR/elderforge_scene.ppm` for eyeballs.
//!
//! Skips (with a note) when no GPU adapter is available, e.g. headless CI.

use elderforge_core::math::{Mat4, Quat, Vec3};
use elderforge_ecs::components::{Camera, MeshRenderer, PhysicsBody, Transform};
use elderforge_ecs::World;
use elderforge_physics::{Collider, PhysicsMaterial, PhysicsWorld, RigidBody};
use elderforge_renderer::{primitives, Draw, ForwardPass, GpuMesh, ResourceCache};

const W: u32 = 640;
const H: u32 = 480;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DT: f32 = 1.0 / 120.0;

#[test]
fn scene_renders_cubes_on_ground() {
    let instance = wgpu::Instance::default();
    let Some(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("no GPU adapter available; skipping scene render test");
        return;
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("device request failed");

    // --- Build the scene (mirrors the binary's spawn_scene) ---
    let mut cache = ResourceCache::new();
    let (cv, ci) = primitives::cube(0.5);
    let cube = cache.insert_mesh(GpuMesh::upload(&device, "cube", &cv, &ci));
    let (gv, gi) = primitives::plane(40.0);
    let ground = cache.insert_mesh(GpuMesh::upload(&device, "ground", &gv, &gi));
    let material = cache.insert_material(Default::default());

    let mut world = World::new();
    let mut physics = PhysicsWorld::new();
    let bouncy = PhysicsMaterial { restitution: 0.6, ..PhysicsMaterial::default() };

    // Fixed camera, matching the binary.
    let camera_world = Mat4::look_at_rh(
        Vec3::new(0.0, 9.0, 24.0),
        Vec3::new(0.0, 3.0, 0.0),
        Vec3::Y,
    )
    .inverse();
    let (_, rotation, position) = camera_world.to_scale_rotation_translation();
    world.spawn((Camera::default(), Transform { position, rotation, scale: Vec3::ONE }));

    // Ground render entity + static body.
    world.spawn((Transform::default(), MeshRenderer { mesh: ground, material }));
    physics.add_rigid_body(
        RigidBody::fixed(Vec3::ZERO, Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 })
            .with_material(bouncy),
    );

    // A grid of cubes above the plane (deterministic positions for the test).
    for gx in -2..=2 {
        for gz in -2..=2 {
            let pos = Vec3::new(gx as f32 * 2.5, 6.0 + (gx + gz) as f32 * 0.5, gz as f32 * 2.5);
            let handle = physics.add_rigid_body(
                RigidBody::dynamic(pos, 1.0, Collider::Sphere { radius: 0.5 })
                    .with_material(bouncy),
            );
            world.spawn((
                Transform { position: pos, rotation: Quat::IDENTITY, scale: Vec3::ONE },
                PhysicsBody { handle },
                MeshRenderer { mesh: cube, material },
            ));
        }
    }

    // --- Step physics so the cubes fall and start bouncing (~1.25 s) ---
    for _ in 0..150 {
        physics.step(DT);
        for (_e, (transform, body)) in world.query_mut::<(&mut Transform, &PhysicsBody)>() {
            if let Some(rb) = physics.body(body.handle) {
                transform.position = rb.position;
                transform.rotation = rb.rotation;
            }
        }
    }

    // A cube must actually have fallen toward the plane.
    let lowest = world
        .query::<(&Transform, &PhysicsBody)>()
        .iter()
        .map(|(_, (t, _))| t.position.y)
        .fold(f32::INFINITY, f32::min);
    assert!(lowest < 2.0, "cubes should have fallen; lowest y = {lowest}");

    // --- Render one frame into an offscreen texture ---
    let mut forward = ForwardPass::new(&device, FORMAT, (W, H));
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen color"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());

    let aspect = W as f32 / H as f32;
    let view_proj = {
        let (cam, tf) = world
            .query::<(&Camera, &Transform)>()
            .iter()
            .map(|(_, (c, t))| (*c, *t))
            .next()
            .expect("a camera exists");
        let view = Mat4::from_rotation_translation(tf.rotation, tf.position).inverse();
        Mat4::perspective_rh(cam.fov_y_radians, aspect, cam.near, cam.far) * view
    };

    let mut draws = Vec::new();
    for (_e, (transform, mr)) in world.query::<(&Transform, &MeshRenderer)>().iter() {
        if let Some(mesh) = cache.mesh(mr.mesh) {
            draws.push(Draw { model: transform.matrix(), mesh });
        }
    }
    assert_eq!(draws.len(), 26, "25 cubes + 1 ground");

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    forward.render(&device, &queue, &mut encoder, &color_view, view_proj, &draws);

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (W * H * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(W * 4), // 640*4 = 2560 = 256*10, aligned
                rows_per_image: None,
            },
        },
        wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map failed"));
    device.poll(wgpu::Maintain::Wait);
    let pixels = slice.get_mapped_range().to_vec();
    drop(buffer);

    // --- Verify the frame is populated, not just the clear color ---
    // Clear color is ~ (13, 15, 23) in 8-bit; anything brighter is geometry.
    let mut lit = 0usize;
    let mut green_ground = 0usize;
    for px in pixels.chunks_exact(4) {
        let (r, g, b) = (px[0] as u32, px[1] as u32, px[2] as u32);
        if r + g + b > 120 {
            lit += 1;
        }
        // The lit ground tints strongly green (normal +Y).
        if g > 140 && g > r + 30 && g > b + 30 {
            green_ground += 1;
        }
    }
    let total = (W * H) as usize;
    let lit_frac = lit as f32 / total as f32;
    eprintln!("lit pixels: {:.1}%, green-ground pixels: {green_ground}", lit_frac * 100.0);
    assert!(lit_frac > 0.05, "scene looks empty: only {:.1}% lit", lit_frac * 100.0);
    assert!(green_ground > 1000, "expected a visible green ground plane");

    // Dump a PPM for human inspection.
    let path = std::env::temp_dir().join("elderforge_scene.ppm");
    let mut ppm = format!("P6\n{W} {H}\n255\n").into_bytes();
    for px in pixels.chunks_exact(4) {
        ppm.extend_from_slice(&px[..3]);
    }
    match std::fs::write(&path, ppm) {
        Ok(()) => eprintln!("scene image: {}", path.display()),
        Err(err) => eprintln!("could not write {}: {err}", path.display()),
    }
}
