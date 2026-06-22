//! Headless GPU check for the physics debug overlay: build a small world, emit
//! every debug layer through the real `DebugOverlay`, then render the overlay
//! (line + point pipelines) over a cleared offscreen target and read it back,
//! asserting the overlay actually drew. Validates the debug pipelines on this
//! machine's backend (vertex layout, line-list/point-list topologies, blend).
//!
//! Skips (with a note) when no GPU adapter is available.

use elderforge::debug_overlay::DebugOverlay;
use elderforge_core::math::{Mat4, Vec3};
use elderforge_physics::{Collider, DebugLayers, PhysicsWorld, RigidBody};
use elderforge_renderer::DebugPass;

const W: u32 = 320;
const H: u32 = 240;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn all_layers() -> DebugLayers {
    DebugLayers {
        collision_shapes: true,
        velocity_vectors: true,
        angular_velocity: true,
        contact_points: true,
        constraint_anchors: true,
        bvh_aabbs: true,
        sleep_state: true,
        force_accumulators: true,
    }
}

#[test]
fn debug_overlay_draws_offscreen() {
    let instance = wgpu::Instance::default();
    let Some(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("no GPU adapter available; skipping debug render test");
        return;
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("device request failed");

    // A world with a box penetrating the ground (a contact), a sphere, a
    // distance constraint, and some velocity/spin — exercises every layer.
    let mut world = PhysicsWorld::new();
    world.add_rigid_body(RigidBody::fixed(
        Vec3::ZERO,
        Collider::HalfSpace { normal: Vec3::Y, offset: 0.0 },
    ));
    let a = world.add_rigid_body(
        RigidBody::dynamic(Vec3::new(0.0, 0.4, 0.0), 1.0, Collider::Box { half_extents: Vec3::splat(0.5) })
            .with_linear_velocity(Vec3::new(1.0, 0.0, 0.0))
            .with_angular_velocity(Vec3::new(0.0, 3.0, 0.0)),
    );
    let b = world.add_rigid_body(RigidBody::dynamic(
        Vec3::new(1.5, 1.5, 0.0),
        1.0,
        Collider::Sphere { radius: 0.4 },
    ));
    world.add_distance_constraint(a, b, 1.5, 0.0);

    let mut overlay = DebugOverlay::new();
    overlay.update(&world, all_layers());
    assert!(!overlay.lines().is_empty(), "expected overlay line geometry");
    assert!(!overlay.points().is_empty(), "expected overlay point geometry");

    // Camera framing the scene.
    let view = Mat4::look_at_rh(Vec3::new(3.0, 2.5, 4.0), Vec3::new(0.3, 0.6, 0.0), Vec3::Y);
    let proj = Mat4::perspective_rh(60f32.to_radians(), W as f32 / H as f32, 0.1, 100.0);
    let view_proj = proj * view;

    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view_tex = color.create_view(&wgpu::TextureViewDescriptor::default());

    let mut debug = DebugPass::new(&device, FORMAT);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    // Clear to black first (the debug pass loads, not clears).
    {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view_tex,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }
    debug.render(
        &device,
        &queue,
        &mut encoder,
        &view_tex,
        view_proj,
        overlay.lines(),
        overlay.points(),
    );

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

    // The overlay drew over pure black, so any non-black pixel is overlay ink.
    let lit = pixels
        .chunks_exact(4)
        .filter(|px| px[0] as u32 + px[1] as u32 + px[2] as u32 > 24)
        .count();
    eprintln!("debug overlay lit {lit} pixels");
    assert!(lit > 50, "debug overlay rendered almost nothing ({lit} px)");

    // Dump a PPM (overlay over black) for human inspection of the geometry.
    let path = std::env::temp_dir().join("elderforge_debug_overlay.ppm");
    let mut ppm = format!("P6\n{W} {H}\n255\n").into_bytes();
    for px in pixels.chunks_exact(4) {
        ppm.extend_from_slice(&px[..3]);
    }
    match std::fs::write(&path, ppm) {
        Ok(()) => eprintln!("  image: {}", path.display()),
        Err(err) => eprintln!("  could not write {}: {err}", path.display()),
    }
}
