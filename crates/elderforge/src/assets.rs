//! App-level asset realization: turn a scene's [`SceneAssets`] table (which
//! names meshes/textures/materials by path or builtin) into live GPU resources
//! in the renderer's [`ResourceCache`].
//!
//! This is the layer that bridges the scene crate (no GPU) and the renderer
//! (owns the device): the scene assigns resource handles, and we populate the
//! cache *at those exact handles* so a loaded scene's `MeshRenderer` components
//! resolve with no remapping. CPU decode (file read + OBJ/glTF/image parse) is
//! memoized by path so re-realizing the same asset — e.g. reloading a scene —
//! skips the expensive step.

use std::collections::HashMap;
use std::path::PathBuf;

use elderforge_core::math::{Vec2, Vec3};
use elderforge_renderer::material::PbrMaterial;
use elderforge_renderer::{primitives, GpuMesh, GpuTexture, ResourceCache, Vertex};
use elderforge_scene::assets::mesh::MeshData;
use elderforge_scene::assets::texture::TextureData;
use elderforge_scene::assets::{MaterialDef, MeshSource, TextureSource};
use elderforge_scene::Scene;

use crate::demos::{CAPSULE_BASE_HALF_HEIGHT, CAPSULE_BASE_RADIUS};

/// Memoizes decoded CPU asset data by file path, so re-realizing a scene (or
/// sharing an asset across scenes) doesn't re-read or re-parse the file.
#[derive(Default)]
pub struct AssetManager {
    mesh_data: HashMap<PathBuf, MeshData>,
    texture_data: HashMap<PathBuf, TextureData>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode (or fetch the cached) mesh data for a file path.
    fn mesh_data(&mut self, path: &PathBuf) -> anyhow::Result<&MeshData> {
        if !self.mesh_data.contains_key(path) {
            let data = elderforge_scene::assets::mesh::load_mesh(path)?;
            self.mesh_data.insert(path.clone(), data);
        }
        Ok(&self.mesh_data[path])
    }

    /// Decode (or fetch the cached) texture data for a file path.
    fn texture_data(&mut self, path: &PathBuf) -> anyhow::Result<&TextureData> {
        if !self.texture_data.contains_key(path) {
            let data = elderforge_scene::assets::texture::load_texture(path)?;
            self.texture_data.insert(path.clone(), data);
        }
        Ok(&self.texture_data[path])
    }

    /// Build a fresh [`ResourceCache`] holding every asset `scene` references,
    /// each resident at the handle the scene assigned it. Builtin meshes are
    /// regenerated from the renderer's primitives; file meshes/textures are
    /// loaded (memoized) and uploaded.
    pub fn realize(
        &mut self,
        scene: &Scene,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<ResourceCache> {
        let mut cache = ResourceCache::new();

        for (handle, source) in scene.assets.textures() {
            let TextureSource::File(path) = source;
            let data = self.texture_data(path)?;
            let label = path.to_string_lossy();
            let texture =
                GpuTexture::from_pixels(device, queue, &label, data.width, data.height, &data.pixels);
            cache.insert_texture_at(handle, texture);
        }

        for (handle, source) in scene.assets.meshes() {
            let (vertices, indices) = match source {
                MeshSource::Builtin(name) => match builtin_mesh(name) {
                    Some(mesh) => mesh,
                    None => {
                        log::warn!("unknown builtin mesh '{name}'; entity will not draw");
                        continue;
                    }
                },
                MeshSource::File(path) => {
                    let data = self.mesh_data(path)?;
                    (mesh_data_to_vertices(data), data.indices.clone())
                }
            };
            let label = match source {
                MeshSource::Builtin(name) => name.clone(),
                MeshSource::File(path) => path.to_string_lossy().into_owned(),
            };
            cache.insert_mesh_at(handle, GpuMesh::upload(device, &label, &vertices, &indices));
        }

        for (handle, def) in scene.assets.materials() {
            cache.insert_material_at(handle, pbr_material(def));
        }

        Ok(cache)
    }
}

/// Generate a builtin primitive mesh by name. These parameters are the single
/// source of truth for the demo primitives (the app and any reloaded scene both
/// regenerate from here, so geometry stays identical).
pub fn builtin_mesh(name: &str) -> Option<(Vec<Vertex>, Vec<u32>)> {
    match name {
        "cube" => Some(primitives::cube(0.5)),
        "sphere" => Some(primitives::sphere(1.0, 24, 16)),
        "capsule" => Some(primitives::capsule(
            CAPSULE_BASE_RADIUS,
            CAPSULE_BASE_HALF_HEIGHT,
            16,
            8,
        )),
        "plane" => Some(primitives::plane(40.0)),
        _ => None,
    }
}

/// Interleave a [`MeshData`]'s parallel arrays into the renderer's vertex
/// layout. Normals/uvs are padded to the position count by the loader; tangents
/// aren't carried by the asset formats yet, so a placeholder is used (the
/// forward pass shades from the normal and ignores it).
fn mesh_data_to_vertices(data: &MeshData) -> Vec<Vertex> {
    (0..data.positions.len())
        .map(|i| Vertex {
            position: data.positions[i].to_array(),
            normal: data.normals.get(i).copied().unwrap_or(Vec3::Y).to_array(),
            uv: data.uvs.get(i).copied().unwrap_or(Vec2::ZERO).to_array(),
            tangent: [1.0, 0.0, 0.0, 1.0],
        })
        .collect()
}

/// Convert a serializable [`MaterialDef`] into the renderer's PBR material.
fn pbr_material(def: &MaterialDef) -> PbrMaterial {
    PbrMaterial {
        albedo: def.albedo,
        roughness: def.roughness,
        metallic: def.metallic,
        albedo_map: def.albedo_map,
        normal_map: def.normal_map,
    }
}
