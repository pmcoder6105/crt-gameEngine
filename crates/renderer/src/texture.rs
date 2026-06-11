//! GPU textures and sampler management.

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

// TODO: GpuTexture::from_pixels(device, queue, width, height, rgba8) upload helper.
