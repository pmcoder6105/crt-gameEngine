//! PBR material parameters (albedo, roughness, metallic, normal map).

use elderforge_core::handles::TextureHandle;
use elderforge_core::math::Vec4;

#[derive(Debug, Clone)]
pub struct PbrMaterial {
    pub albedo: Vec4,
    pub roughness: f32,
    pub metallic: f32,
    pub albedo_map: Option<TextureHandle>,
    pub normal_map: Option<TextureHandle>,
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self {
            albedo: Vec4::ONE,
            roughness: 0.5,
            metallic: 0.0,
            albedo_map: None,
            normal_map: None,
        }
    }
}
