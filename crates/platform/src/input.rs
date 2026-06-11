//! Aggregated input state, updated from engine events each frame.
//! // TODO: gamepad support.

use std::collections::HashSet;

use crate::event::{EngineEvent, Key, MouseButton};

#[derive(Default)]
pub struct InputState {
    keys_down: HashSet<Key>,
    buttons_down: HashSet<MouseButton>,
    mouse_position: (f64, f64),
    mouse_delta: (f64, f64),
    scroll_delta: (f32, f32),
}

impl InputState {
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
                self.mouse_delta = (x - self.mouse_position.0, y - self.mouse_position.1);
                self.mouse_position = (*x, *y);
            }
            EngineEvent::MouseWheel { dx, dy } => {
                self.scroll_delta.0 += *dx;
                self.scroll_delta.1 += *dy;
            }
            _ => {}
        }
    }

    /// Clear per-frame deltas. Call once at the end of each frame.
    pub fn end_frame(&mut self) {
        self.mouse_delta = (0.0, 0.0);
        self.scroll_delta = (0.0, 0.0);
    }

    pub fn is_key_down(&self, key: Key) -> bool {
        self.keys_down.contains(&key)
    }

    pub fn is_button_down(&self, button: MouseButton) -> bool {
        self.buttons_down.contains(&button)
    }

    pub fn mouse_position(&self) -> (f64, f64) {
        self.mouse_position
    }

    pub fn mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    pub fn scroll_delta(&self) -> (f32, f32) {
        self.scroll_delta
    }
}
