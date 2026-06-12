//! wgpu Instance, Adapter, Device, Queue, and Surface setup.

use crate::RendererError;

/// Owns the wgpu instance, adapter, device, queue, and the window surface.
///
/// Created once the window exists, since the surface borrows window-system
/// handles. wgpu setup is async; init blocks on it via pollster.
pub struct RenderContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
}

/// One frame in flight: the surface texture to draw into, a view of it,
/// and a command encoder to record passes into. Hand it back to
/// [`RenderContext::present`] when the frame's passes are recorded.
pub struct FrameContext {
    pub surface_texture: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
}

impl RenderContext {
    /// Creates the full rendering context for a window.
    ///
    /// `window` is anything wgpu can make a surface from — the platform
    /// crate's `WindowHandle::surface_provider()` return value qualifies.
    /// `width`/`height` are the initial surface size in physical pixels;
    /// `vsync` selects the present mode.
    pub fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<Self, RendererError> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .map_err(|err| RendererError::Surface(err.to_string()))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or(RendererError::NoAdapter)?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .map_err(|err| RendererError::DeviceRequest(err.to_string()))?;

        let capabilities = surface.get_capabilities(&adapter);
        // Prefer an sRGB format so PBR output lands in the right color space.
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| {
                RendererError::Surface("surface reports no supported formats".to_string())
            })?;

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: if vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let info = adapter.get_info();
        log::info!(
            "renderer: {} via {:?}, surface {}x{} {:?}",
            info.name,
            info.backend,
            surface_config.width,
            surface_config.height,
            format
        );

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface,
            surface_config,
        })
    }

    /// Reconfigures the surface for a new window size in physical pixels.
    /// Zero-sized requests (minimized window) are ignored.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    /// Texture format of the surface; render pipelines must target it.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    /// Current surface size in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    /// Begins a frame: acquires the surface texture and creates a command
    /// encoder. A lost or outdated surface is reconfigured and retried once
    /// before giving up.
    pub fn frame(&mut self) -> Result<FrameContext, RendererError> {
        let surface_texture = match self.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                self.surface
                    .get_current_texture()
                    .map_err(|err| RendererError::Surface(err.to_string()))?
            }
            Err(err) => return Err(RendererError::Surface(err.to_string())),
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });
        Ok(FrameContext {
            surface_texture,
            view,
            encoder,
        })
    }

    /// Submits the frame's recorded commands and presents the surface texture.
    pub fn present(&self, frame: FrameContext) {
        self.queue.submit([frame.encoder.finish()]);
        frame.surface_texture.present();
    }
}
