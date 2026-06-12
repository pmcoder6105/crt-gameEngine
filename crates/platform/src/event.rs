//! Engine events, normalized from winit. Key and mouse-button types are
//! re-exported from winit so downstream crates never import winit directly.

pub use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;

/// Shorthand alias for [`KeyCode`].
pub type Key = KeyCode;

/// A window or input event, normalized from winit into the small set the
/// engine cares about. Coordinates are physical pixels.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    /// The user asked to close the window (close button, Cmd-Q, ...).
    CloseRequested,
    /// The window's inner size changed, in physical pixels.
    Resized { width: u32, height: u32 },
    /// The window gained (`true`) or lost (`false`) keyboard focus.
    FocusChanged(bool),
    /// A physical key went down. Repeats while held, as delivered by the OS.
    KeyPressed(Key),
    /// A physical key was released.
    KeyReleased(Key),
    /// The cursor moved to this position in physical pixels, relative to
    /// the window's top-left corner.
    MouseMoved { x: f64, y: f64 },
    /// A mouse button went down.
    MouseButtonPressed(MouseButton),
    /// A mouse button was released.
    MouseButtonReleased(MouseButton),
    /// Scroll wheel or trackpad scroll. Line-based deltas are in lines,
    /// pixel-based deltas in pixels; consumers treat both as relative.
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

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalSize;
    use winit::event::WindowEvent;

    #[test]
    fn normalizes_events_the_engine_cares_about() {
        assert_eq!(
            EngineEvent::from_winit(&WindowEvent::CloseRequested),
            Some(EngineEvent::CloseRequested)
        );
        assert_eq!(
            EngineEvent::from_winit(&WindowEvent::Resized(PhysicalSize::new(800, 600))),
            Some(EngineEvent::Resized {
                width: 800,
                height: 600
            })
        );
        assert_eq!(
            EngineEvent::from_winit(&WindowEvent::Focused(true)),
            Some(EngineEvent::FocusChanged(true))
        );
    }

    #[test]
    fn ignores_events_the_engine_does_not_care_about() {
        assert_eq!(EngineEvent::from_winit(&WindowEvent::Destroyed), None);
        assert_eq!(EngineEvent::from_winit(&WindowEvent::HoveredFileCancelled), None);
    }
}
