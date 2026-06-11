//! Math types. All engine math goes through glam; downstream crates import
//! these re-exports instead of depending on glam directly.

pub use glam::{BVec3, Mat3, Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};

/// Engine-wide epsilon for float comparisons.
pub const EPSILON: f32 = 1e-6;
