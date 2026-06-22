//! Window creation and the engine event loop.
//!
//! winit 0.30 only allows window creation from inside the event loop
//! (`ApplicationHandler::resumed`), so this module owns both: call
//! [`run_event_loop`] with a [`WindowConfig`] and a per-frame closure,
//! and the runner creates the window and drives frames until the closure
//! returns [`FrameControl::Exit`] or the window is closed.

use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use thiserror::Error;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::event::EngineEvent;
use crate::input::InputState;

/// Errors from window creation or the event loop itself.
#[derive(Debug, Error)]
pub enum WindowError {
    #[error("failed to create window: {0}")]
    Creation(String),
    #[error("event loop error: {0}")]
    EventLoop(String),
}

/// Settings used when creating the main engine window.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// Window title bar text.
    pub title: String,
    /// Initial inner width in logical pixels.
    pub width: u32,
    /// Initial inner height in logical pixels.
    pub height: u32,
    /// Whether the user can resize the window.
    pub resizable: bool,
    /// Whether the OS draws a title bar and frame. `false` gives a borderless
    /// window — used for clean, chrome-free demo capture.
    pub decorations: bool,
    /// Whether presentation should wait for vertical sync. The window
    /// itself doesn't act on this; the renderer reads it via
    /// [`WindowHandle::vsync`] when choosing a surface present mode.
    pub vsync: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Elderforge".to_string(),
            width: 1600,
            height: 900,
            resizable: true,
            decorations: true,
            vsync: true,
        }
    }
}

/// Owns the winit window — the only place winit window types are visible.
///
/// The window is held in an `Arc` so the renderer can later share ownership
/// for surface creation without the handle giving up the window.
pub struct WindowHandle {
    window: Arc<Window>,
    vsync: bool,
}

impl WindowHandle {
    /// Only the event-loop runner constructs handles; windows cannot exist
    /// outside a running event loop in winit 0.30.
    pub(crate) fn new(window: Window, vsync: bool) -> Self {
        Self {
            window: Arc::new(window),
            vsync,
        }
    }

    /// Current inner size in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        (size.width, size.height)
    }

    /// DPI scale factor of the monitor the window is on.
    pub fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    /// Whether the window was configured for vsync presentation.
    pub fn vsync(&self) -> bool {
        self.vsync
    }

    /// Ask the OS for another redraw. The runner calls this every frame to
    /// keep the loop continuous; manual calls are only needed for one-shot
    /// repaints from event-driven code.
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Shared ownership of the OS window as a `raw-window-handle` trait
    /// object, for the renderer to create its GPU surface from. This is the
    /// only window escape hatch, and it deliberately speaks
    /// `raw-window-handle` rather than winit.
    pub fn surface_provider(&self) -> Arc<dyn SurfaceProvider> {
        self.window.clone()
    }

    /// The underlying winit window. This is the one place a winit type leaves
    /// the platform crate, and it exists solely for `egui_winit`, which needs
    /// the concrete window to translate input and report platform output. No
    /// other consumer should reach for it.
    pub fn winit_window(&self) -> &Window {
        &self.window
    }
}

/// A raw winit window event, re-exported so the editor's `egui_winit` bridge
/// can be fed without the binary naming `winit` directly. These are delivered
/// to the frame closure alongside the normalized [`EngineEvent`]s.
pub type RawWindowEvent = WindowEvent;

/// Window-system handles a GPU surface can be created from. Blanket-implemented;
/// consumers (the renderer) only need the supertraits, which `wgpu` accepts
/// directly as a surface target.
pub trait SurfaceProvider: HasWindowHandle + HasDisplayHandle + Send + Sync {}

impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync> SurfaceProvider for T {}

/// What the per-frame closure tells the event loop to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameControl {
    /// Keep running; another frame will follow.
    Continue,
    /// Shut the event loop down after this frame.
    Exit,
}

/// Runs the engine event loop until the closure returns
/// [`FrameControl::Exit`] or the user closes the window.
///
/// The closure is called once per frame with the accumulated
/// [`InputState`], the [`EngineEvent`]s received since the previous frame
/// (already applied to the input state), the corresponding **raw**
/// [`RawWindowEvent`]s (for `egui_winit` to consume directly), and the
/// [`WindowHandle`]. [`EngineEvent::CloseRequested`] is delivered to the
/// closure for any last-frame work, then the loop exits regardless of the
/// returned value.
///
/// Blocks until the loop ends; on most platforms it must be called from
/// the main thread.
pub fn run_event_loop<F>(config: WindowConfig, frame: F) -> Result<(), WindowError>
where
    F: FnMut(&mut InputState, &[EngineEvent], &[RawWindowEvent], &WindowHandle) -> FrameControl,
{
    let event_loop = EventLoop::new().map_err(|e| WindowError::EventLoop(e.to_string()))?;
    // Poll: a simulation engine re-renders continuously instead of waiting
    // for OS events.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = EventLoopApp {
        config,
        frame,
        window: None,
        input: InputState::new(),
        pending_events: Vec::new(),
        pending_raw: Vec::new(),
        error: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| WindowError::EventLoop(e.to_string()))?;

    match app.error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// winit `ApplicationHandler` driving [`run_event_loop`].
struct EventLoopApp<F> {
    config: WindowConfig,
    frame: F,
    window: Option<WindowHandle>,
    input: InputState,
    pending_events: Vec<EngineEvent>,
    /// Raw winit events for this frame, forwarded to the closure for `egui`.
    pending_raw: Vec<RawWindowEvent>,
    /// Failure recorded mid-loop, returned by `run_event_loop` after exit.
    error: Option<WindowError>,
}

impl<F> ApplicationHandler for EventLoopApp<F>
where
    F: FnMut(&mut InputState, &[EngineEvent], &[RawWindowEvent], &WindowHandle) -> FrameControl,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Also fired when a suspended app resumes; the window already exists then.
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height))
            .with_resizable(self.config.resizable)
            .with_decorations(self.config.decorations);
        match event_loop.create_window(attributes) {
            Ok(window) => {
                window.request_redraw();
                self.window = Some(WindowHandle::new(window, self.config.vsync));
            }
            Err(err) => {
                self.error = Some(WindowError::Creation(err.to_string()));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => {
                let Some(window) = &self.window else {
                    return;
                };
                let events = std::mem::take(&mut self.pending_events);
                let raw = std::mem::take(&mut self.pending_raw);
                for event in &events {
                    self.input.handle_event(event);
                }
                let close_requested = events
                    .iter()
                    .any(|e| matches!(e, EngineEvent::CloseRequested));

                let control = (self.frame)(&mut self.input, &events, &raw, window);
                self.input.end_frame();

                if close_requested || control == FrameControl::Exit {
                    event_loop.exit();
                } else {
                    window.request_redraw();
                }
            }
            other => {
                if let Some(event) = EngineEvent::from_winit(&other) {
                    self.pending_events.push(event);
                }
                // Keep the raw event too: egui_winit consumes winit events
                // directly and needs ones the normalizer drops (scroll, IME…).
                self.pending_raw.push(other);
            }
        }
    }
}
