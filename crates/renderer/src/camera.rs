//! Camera, projection, and view matrices.

use elderforge_core::math::{Mat4, Quat, Vec3};

#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec3,
    pub rotation: Quat,
    pub fov_y_radians: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.position).inverse()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y_radians, self.aspect, self.near, self.far)
    }

    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 6.0),
            rotation: Quat::IDENTITY,
            fov_y_radians: 60f32.to_radians(),
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}
