//! Window creation and resize handling.

use thiserror::Error;
use winit::window::Window;

#[derive(Debug, Error)]
pub enum WindowError {
    #[error("failed to create window: {0}")]
    Creation(String),
}

/// Settings used when creating the main engine window.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Elderforge".to_string(),
            width: 1600,
            height: 900,
        }
    }
}

/// Owns the winit window — the only place winit window types are visible.
pub struct WindowHandle {
    window: Window,
}

impl WindowHandle {
    // TODO: event-loop runner in this crate constructs this from WindowConfig
    // (winit 0.30 requires creation inside ApplicationHandler::resumed).
    pub fn new(window: Window) -> Self {
        Self { window }
    }

    pub fn size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        (size.width, size.height)
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}
