//! Aggregated input state, updated from engine events each frame.
//! // TODO: gamepad support.

use std::collections::HashSet;

use crate::event::{EngineEvent, Key, MouseButton};

/// Per-frame input snapshot: which keys/buttons are held, where the mouse
/// is, and how much it moved or scrolled since the last frame.
///
/// The event-loop runner feeds events in via [`handle_event`](Self::handle_event)
/// and clears the deltas with [`end_frame`](Self::end_frame) after each frame,
/// so the per-frame closure can just query.
#[derive(Default)]
pub struct InputState {
    keys_down: HashSet<Key>,
    buttons_down: HashSet<MouseButton>,
    mouse_position: (f64, f64),
    mouse_delta: (f64, f64),
    scroll_delta: (f32, f32),
}

impl InputState {
    /// Creates an empty input state: nothing held, mouse at the origin.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one event into the state. Call for every event, every frame.
    pub fn handle_event(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::KeyPressed(key) => {
                self.keys_down.insert(*key);
            }
            EngineEvent::KeyReleased(key) => {
                self.keys_down.remove(key);
            }
            EngineEvent::MouseButtonPressed(button) => {
                self.buttons_down.insert(*button);
            }
            EngineEvent::MouseButtonReleased(button) => {
                self.buttons_down.remove(button);
            }
            EngineEvent::MouseMoved { x, y } => {
                self.mouse_delta.0 += x - self.mouse_position.0;
                self.mouse_delta.1 += y - self.mouse_position.1;
                self.mouse_position = (*x, *y);
            }
            EngineEvent::MouseWheel { dx, dy } => {
                self.scroll_delta.0 += *dx;
                self.scroll_delta.1 += *dy;
            }
            // Losing focus drops key/button state: the OS won't deliver the
            // release events while another window has focus.
            EngineEvent::FocusChanged(false) => {
                self.keys_down.clear();
                self.buttons_down.clear();
            }
            _ => {}
        }
    }

    /// Clear per-frame deltas. The runner calls this once per frame.
    pub fn end_frame(&mut self) {
        self.mouse_delta = (0.0, 0.0);
        self.scroll_delta = (0.0, 0.0);
    }

    /// True while the key is held down.
    pub fn is_key_down(&self, key: Key) -> bool {
        self.keys_down.contains(&key)
    }

    /// True while the mouse button is held down.
    pub fn is_button_down(&self, button: MouseButton) -> bool {
        self.buttons_down.contains(&button)
    }

    /// Cursor position in physical pixels, relative to the window's
    /// top-left corner.
    pub fn mouse_position(&self) -> (f64, f64) {
        self.mouse_position
    }

    /// Cursor movement accumulated this frame, in physical pixels.
    pub fn mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    /// Scroll accumulated this frame (lines or pixels depending on device).
    pub fn scroll_delta(&self) -> (f32, f32) {
        self.scroll_delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_track_press_and_release() {
        let mut input = InputState::new();
        input.handle_event(&EngineEvent::KeyPressed(Key::KeyW));
        assert!(input.is_key_down(Key::KeyW));
        assert!(!input.is_key_down(Key::KeyS));

        // Held keys survive frame boundaries; only deltas are per-frame.
        input.end_frame();
        assert!(input.is_key_down(Key::KeyW));

        input.handle_event(&EngineEvent::KeyReleased(Key::KeyW));
        assert!(!input.is_key_down(Key::KeyW));
    }

    #[test]
    fn mouse_motion_accumulates_into_delta_and_resets() {
        let mut input = InputState::new();
        input.handle_event(&EngineEvent::MouseMoved { x: 10.0, y: 5.0 });
        input.handle_event(&EngineEvent::MouseMoved { x: 13.0, y: 9.0 });
        assert_eq!(input.mouse_position(), (13.0, 9.0));
        assert_eq!(input.mouse_delta(), (13.0, 9.0));

        input.end_frame();
        assert_eq!(input.mouse_delta(), (0.0, 0.0));
        assert_eq!(input.mouse_position(), (13.0, 9.0), "position persists");
    }

    #[test]
    fn scroll_accumulates_within_a_frame() {
        let mut input = InputState::new();
        input.handle_event(&EngineEvent::MouseWheel { dx: 0.0, dy: 1.0 });
        input.handle_event(&EngineEvent::MouseWheel { dx: 0.5, dy: 2.0 });
        assert_eq!(input.scroll_delta(), (0.5, 3.0));
        input.end_frame();
        assert_eq!(input.scroll_delta(), (0.0, 0.0));
    }

    #[test]
    fn losing_focus_releases_held_keys_and_buttons() {
        let mut input = InputState::new();
        input.handle_event(&EngineEvent::KeyPressed(Key::Space));
        input.handle_event(&EngineEvent::MouseButtonPressed(MouseButton::Left));
        input.handle_event(&EngineEvent::FocusChanged(false));
        assert!(!input.is_key_down(Key::Space));
        assert!(!input.is_button_down(MouseButton::Left));
    }
}
