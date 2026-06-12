//! Headless GPU integration test: renders the bootstrap triangle through
//! the unlit pass into an offscreen texture, reads the pixels back, and
//! checks the triangle is actually there. Needs a real GPU adapter; the
//! test skips (with a note) when none is available, e.g. headless CI.
//!
//! Also dumps the readback to `$TMPDIR/elderforge_triangle.ppm` for eyeballs.

use elderforge_renderer::passes::unlit::UnlitPass;
use elderforge_renderer::{GpuMesh, Vertex};

const SIZE: u32 = 256;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[test]
fn unlit_triangle_renders_offscreen() {
    let instance = wgpu::Instance::default();
    let Some(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("no GPU adapter available; skipping render test");
        return;
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("device request failed");

    // Same geometry the binary draws: RGB color in the normal slot.
    let vertex = |position: [f32; 3], color: [f32; 3]| Vertex {
        position,
        normal: color,
        uv: [0.0, 0.0],
        tangent: [0.0; 4],
    };
    let mesh = GpuMesh::upload(
        &device,
        "test triangle",
        &[
            vertex([0.0, 0.6, 0.0], [1.0, 0.0, 0.0]),
            vertex([-0.6, -0.6, 0.0], [0.0, 1.0, 0.0]),
            vertex([0.6, -0.6, 0.0], [0.0, 0.0, 1.0]),
        ],
        &[0, 1, 2],
    );
    let pass = UnlitPass::new(&device, FORMAT);

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("readback target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // 4 bytes/pixel at SIZE=256 keeps rows at the 256-byte alignment
    // copy_texture_to_buffer requires.
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback buffer"),
        size: (SIZE * SIZE * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    pass.draw(&mut encoder, &view, &mesh);
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("readback map failed")
    });
    device.poll(wgpu::Maintain::Wait);
    let pixels = slice.get_mapped_range().to_vec();
    drop(buffer);

    let pixel = |x: u32, y: u32| {
        let offset = ((y * SIZE + x) * 4) as usize;
        (
            pixels[offset],
            pixels[offset + 1],
            pixels[offset + 2],
            pixels[offset + 3],
        )
    };

    // Center of the texture is inside the triangle; corners are background.
    let center = pixel(SIZE / 2, SIZE / 2);
    let corner = pixel(4, 4);
    assert_eq!(center.3, 255, "triangle pixel must be opaque");
    assert!(
        center.0 as u32 + center.1 as u32 + center.2 as u32 > 100,
        "center pixel should be brightly colored, got {center:?}"
    );
    assert!(
        corner.0 < 20 && corner.1 < 20 && corner.2 < 20,
        "corner should be near the dark clear color, got {corner:?}"
    );
    // Vertex colors interpolate: near the top vertex red dominates.
    let near_top = pixel(SIZE / 2, SIZE / 5 + 4);
    assert!(
        near_top.0 > near_top.1 && near_top.0 > near_top.2,
        "pixel near top vertex should be red-dominant, got {near_top:?}"
    );

    // Dump a PPM next to the temp dir for human inspection.
    let path = std::env::temp_dir().join("elderforge_triangle.ppm");
    let mut ppm = format!("P6\n{SIZE} {SIZE}\n255\n").into_bytes();
    for chunk in pixels.chunks_exact(4) {
        ppm.extend_from_slice(&chunk[..3]);
    }
    if let Err(err) = std::fs::write(&path, ppm) {
        eprintln!("could not write {}: {err}", path.display());
    } else {
        eprintln!("readback image: {}", path.display());
    }
}
