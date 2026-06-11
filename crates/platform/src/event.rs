//! Engine events, normalized from winit. Key and mouse-button types are
//! re-exported from winit so downstream crates never import winit directly.

pub use winit::event::MouseButton;
pub use winit::keyboard::KeyCode as Key;

#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    CloseRequested,
    Resized { width: u32, height: u32 },
    FocusChanged(bool),
    KeyPressed(Key),
    KeyReleased(Key),
    MouseMoved { x: f64, y: f64 },
    MouseButtonPressed(MouseButton),
    MouseButtonReleased(MouseButton),
    MouseWheel { dx: f32, dy: f32 },
}

impl EngineEvent {
    /// Normalize a winit window event. Returns `None` for events the engine
    /// does not care about.
    pub fn from_winit(event: &winit::event::WindowEvent) -> Option<Self> {
        use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
        match event {
            WindowEvent::CloseRequested => Some(Self::CloseRequested),
            WindowEvent::Resized(size) => Some(Self::Resized {
                width: size.width,
                height: size.height,
            }),
            WindowEvent::Focused(focused) => Some(Self::FocusChanged(*focused)),
            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => Some(Self::KeyPressed(code)),
                        ElementState::Released => Some(Self::KeyReleased(code)),
                    }
                } else {
                    None
                }
            }
            WindowEvent::CursorMoved { position, .. } => Some(Self::MouseMoved {
                x: position.x,
                y: position.y,
            }),
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => Some(Self::MouseButtonPressed(*button)),
                ElementState::Released => Some(Self::MouseButtonReleased(*button)),
            },
            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(dx, dy) => {
                    Some(Self::MouseWheel { dx: *dx, dy: *dy })
                }
                MouseScrollDelta::PixelDelta(pos) => Some(Self::MouseWheel {
                    dx: pos.x as f32,
                    dy: pos.y as f32,
                }),
            },
            _ => None,
        }
    }
}
