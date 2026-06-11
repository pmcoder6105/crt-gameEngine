//! wgpu Device, Queue, and Surface setup.

use crate::RendererError;

pub struct RenderContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    // TODO: surface + surface configuration once window integration lands.
}

impl RenderContext {
    /// Create a headless context (no surface yet).
    pub fn new() -> Result<Self, RendererError> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(
            instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
        )
        .ok_or(RendererError::NoAdapter)?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .map_err(|err| RendererError::DeviceRequest(err.to_string()))?;
        log::info!("renderer: created wgpu device ({:?})", adapter.get_info().backend);
        Ok(Self { device, queue })
    }
}
