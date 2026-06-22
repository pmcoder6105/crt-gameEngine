//! Headless smoke + render check for the three demo scenes. For each demo it
//! builds the exact scene the binary would (`Demo::setup`), steps physics for a
//! while (asserting nothing goes NaN or explodes), then renders one frame
//! offscreen through `ForwardPass` and reads it back, asserting the frame is
//! actually populated (not just the clear color). A PPM per demo is dumped to
//! `$TMPDIR/elderforge_demo_<name>.ppm` for eyeballs.
//!
//! Skips (with a note) when no GPU adapter is available, e.g. headless CI.

use elderforge::deformable::DeformableMeshes;
use elderforge::demos::{Demo, DemoAssets, CAPSULE_BASE_HALF_HEIGHT, CAPSULE_BASE_RADIUS};
use elderforge_core::math::Mat4;
use elderforge_ecs::components::{Camera, MeshRenderer, PhysicsBody, Transform};
use elderforge_renderer::{primitives, Draw, ForwardPass, GpuMesh, ResourceCache};
use elderforge_scene::Scene;

const W: u32 = 640;
const H: u32 = 480;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DT: f32 = 1.0 / 120.0;
/// Frames to simulate before rendering. Enough for each scene to get moving.
const FRAMES: usize = 90;

#[test]
fn every_demo_builds_steps_and_renders() {
    let instance = wgpu::Instance::default();
    let Some(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("no GPU adapter available; skipping demo render test");
        return;
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("device request failed");

    // Upload the shared primitive meshes once, exactly as the binary does.
    let mut cache = ResourceCache::new();
    let (cv, ci) = primitives::cube(0.5);
    let cube = cache.insert_mesh(GpuMesh::upload(&device, "cube", &cv, &ci));
    let (sv, si) = primitives::sphere(1.0, 24, 16);
    let sphere = cache.insert_mesh(GpuMesh::upload(&device, "sphere", &sv, &si));
    let (cap_v, cap_i) = primitives::capsule(CAPSULE_BASE_RADIUS, CAPSULE_BASE_HALF_HEIGHT, 16, 8);
    let capsule = cache.insert_mesh(GpuMesh::upload(&device, "capsule", &cap_v, &cap_i));
    let (pv, pi) = primitives::plane(40.0);
    let plane = cache.insert_mesh(GpuMesh::upload(&device, "plane", &pv, &pi));
    let material = cache.insert_material(Default::default());
    let assets = DemoAssets { cube, sphere, capsule, plane, material };

    let mut forward = ForwardPass::new(&device, FORMAT, (W, H), 1);

    for demo in Demo::all() {
        let mut scene = Scene::new();
        demo.setup(&mut scene, &assets);

        // Step physics, syncing body poses into transforms, and check the sim
        // stays finite (a blown-up solver produces NaN/inf positions).
        for _ in 0..FRAMES {
            scene.physics.step(DT);
            let Scene { world, physics, .. } = &mut scene;
            for (_e, (transform, body)) in world.query_mut::<(&mut Transform, &PhysicsBody)>() {
                if let Some(rb) = physics.body(body.handle) {
                    transform.position = rb.position;
                    transform.rotation = rb.rotation;
                }
            }
        }
        for (_e, (t, _)) in scene.world.query::<(&Transform, &PhysicsBody)>().iter() {
            assert!(
                t.position.is_finite(),
                "{}: body position went non-finite: {:?}",
                demo.name(),
                t.position
            );
        }
        // Soft-body / cloth particles must stay finite too.
        for p in scene.physics.particles() {
            assert!(
                p.position.is_finite(),
                "{}: particle went non-finite: {:?}",
                demo.name(),
                p.position
            );
        }

        // --- Render one frame into an offscreen texture ---
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

        let view_proj = active_camera(&scene, W as f32 / H as f32)
            .unwrap_or_else(|| panic!("{}: no active camera", demo.name()));

        // Build the deforming soft-body / cloth meshes from the post-step
        // particle state, exactly as the app does each frame.
        let deformables = DeformableMeshes::build(&device, &scene.physics);

        let mut draws = Vec::new();
        for (_e, (transform, mr)) in scene.world.query::<(&Transform, &MeshRenderer)>().iter() {
            if let Some(mesh) = cache.mesh(mr.mesh) {
                draws.push(Draw { model: transform.matrix(), mesh });
            }
        }
        deformables.append_draws(&mut draws);
        assert!(!draws.is_empty(), "{}: nothing to draw", demo.name());

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
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
                    bytes_per_row: Some(W * 4),
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

        // The frame must be more than the clear color (~ (13,15,23) in 8-bit).
        let mut lit = 0usize;
        for px in pixels.chunks_exact(4) {
            let (r, g, b) = (px[0] as u32, px[1] as u32, px[2] as u32);
            if r + g + b > 120 {
                lit += 1;
            }
        }
        let lit_frac = lit as f32 / (W * H) as f32;
        eprintln!("demo '{}': {:.1}% lit pixels", demo.name(), lit_frac * 100.0);
        assert!(
            lit_frac > 0.03,
            "{}: scene looks empty: only {:.1}% lit",
            demo.name(),
            lit_frac * 100.0
        );

        // Dump a PPM for human inspection.
        let path = std::env::temp_dir().join(format!("elderforge_demo_{}.ppm", demo.name()));
        let mut ppm = format!("P6\n{W} {H}\n255\n").into_bytes();
        for px in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&px[..3]);
        }
        match std::fs::write(&path, ppm) {
            Ok(()) => eprintln!("  image: {}", path.display()),
            Err(err) => eprintln!("  could not write {}: {err}", path.display()),
        }
    }
}

/// View-projection of the first active camera, matching the render system.
fn active_camera(scene: &Scene, aspect: f32) -> Option<Mat4> {
    for (_e, (camera, transform)) in scene.world.query::<(&Camera, &Transform)>().iter() {
        if camera.is_active {
            let view =
                Mat4::from_rotation_translation(transform.rotation, transform.position).inverse();
            let proj = Mat4::perspective_rh(camera.fov_y_radians, aspect, camera.near, camera.far);
            return Some(proj * view);
        }
    }
    None
}
