//! ResourceCache: all GPU resources are owned here and referenced by handle.
//! Nothing outside the renderer holds wgpu resources directly.

use std::collections::HashMap;

use elderforge_core::handles::{MaterialHandle, MeshHandle, TextureHandle};

use crate::material::PbrMaterial;
use crate::mesh::GpuMesh;
use crate::texture::GpuTexture;

#[derive(Default)]
pub struct ResourceCache {
    meshes: HashMap<MeshHandle, GpuMesh>,
    textures: HashMap<TextureHandle, GpuTexture>,
    materials: HashMap<MaterialHandle, PbrMaterial>,
    next_index: u32,
}

impl ResourceCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_index(&mut self) -> u32 {
        let index = self.next_index;
        self.next_index += 1;
        index
    }

    pub fn insert_mesh(&mut self, mesh: GpuMesh) -> MeshHandle {
        let handle = MeshHandle::new(self.next_index(), 0);
        self.meshes.insert(handle, mesh);
        handle
    }

    pub fn insert_texture(&mut self, texture: GpuTexture) -> TextureHandle {
        let handle = TextureHandle::new(self.next_index(), 0);
        self.textures.insert(handle, texture);
        handle
    }

    pub fn insert_material(&mut self, material: PbrMaterial) -> MaterialHandle {
        let handle = MaterialHandle::new(self.next_index(), 0);
        self.materials.insert(handle, material);
        handle
    }

    pub fn mesh(&self, handle: MeshHandle) -> Option<&GpuMesh> {
        self.meshes.get(&handle)
    }

    pub fn texture(&self, handle: TextureHandle) -> Option<&GpuTexture> {
        self.textures.get(&handle)
    }

    pub fn material(&self, handle: MaterialHandle) -> Option<&PbrMaterial> {
        self.materials.get(&handle)
    }
}
