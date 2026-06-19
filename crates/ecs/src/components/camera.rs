use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub fov_y_radians: f32,
    pub near: f32,
    pub far: f32,
    pub is_active: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            fov_y_radians: 60f32.to_radians(),
            near: 0.1,
            far: 1000.0,
            is_active: true,
        }
    }
}
