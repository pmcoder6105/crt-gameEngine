//! Type-safe generational handles used to reference engine resources
//! (meshes, textures, materials) without holding the data inline.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// A generational handle tagged with a marker type so handles to different
/// resource kinds cannot be mixed up.
pub struct Handle<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub fn new(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            _marker: PhantomData,
        }
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }
}

// Manual impls: derives would put unnecessary bounds on `T`.
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}
impl<T> Eq for Handle<T> {}
impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}
impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle({}, gen {})", self.index, self.generation)
    }
}

/// Marker types for engine-wide resource handles.
pub enum MeshMarker {}
pub enum TextureMarker {}
pub enum MaterialMarker {}

pub type MeshHandle = Handle<MeshMarker>;
pub type TextureHandle = Handle<TextureMarker>;
pub type MaterialHandle = Handle<MaterialMarker>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_compare_by_index_and_generation() {
        let a = MeshHandle::new(1, 0);
        let b = MeshHandle::new(1, 0);
        let c = MeshHandle::new(1, 1);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.index(), 1);
        assert_eq!(c.generation(), 1);
    }
}
