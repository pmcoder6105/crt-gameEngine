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

/// Error returned when an operation is given a handle that does not refer
/// to a live slot (already freed, never allocated, or from another allocator).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("stale handle: index {index}, generation {generation}")]
pub struct StaleHandle {
    /// Slot index the handle pointed at.
    pub index: u32,
    /// Generation the handle carried when the operation was attempted.
    pub generation: u32,
}

/// Allocates and validates [`Handle`]s for one resource kind.
///
/// Each slot carries a generation counter. Freeing a slot bumps its
/// generation, so handles issued before the free no longer validate —
/// even after the slot index is reused by a later allocation.
///
/// The allocator only manages handle lifetimes; the resource storage
/// itself lives elsewhere (e.g. a `Vec<T>` indexed by [`Handle::index`]).
pub struct HandleAllocator<T> {
    /// Current generation of every slot ever created. A slot is live when
    /// its index is not on the free list.
    generations: Vec<u32>,
    /// Indices available for reuse.
    free: Vec<u32>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Default for HandleAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> HandleAllocator<T> {
    /// Creates an empty allocator with no slots.
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            free: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Allocates a new handle, reusing a freed slot if one is available.
    pub fn allocate(&mut self) -> Handle<T> {
        match self.free.pop() {
            Some(index) => Handle::new(index, self.generations[index as usize]),
            None => {
                let index = self.generations.len() as u32;
                self.generations.push(0);
                Handle::new(index, 0)
            }
        }
    }

    /// Frees a handle, invalidating it and every copy of it.
    ///
    /// The slot's generation is bumped so stale handles fail
    /// [`is_valid`](Self::is_valid), then the index is queued for reuse.
    pub fn free(&mut self, handle: Handle<T>) -> Result<(), StaleHandle> {
        if !self.is_valid(handle) {
            return Err(StaleHandle {
                index: handle.index(),
                generation: handle.generation(),
            });
        }
        let slot = &mut self.generations[handle.index() as usize];
        *slot = slot.wrapping_add(1);
        self.free.push(handle.index());
        Ok(())
    }

    /// Returns true if the handle refers to a currently live slot.
    ///
    /// Freeing bumps the slot's generation before any new handle is issued
    /// for it, so a generation match is sufficient proof of liveness.
    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.generations
            .get(handle.index() as usize)
            .is_some_and(|&gen| gen == handle.generation())
    }

    /// Number of currently live (allocated, not freed) handles.
    pub fn live_count(&self) -> usize {
        self.generations.len() - self.free.len()
    }
}

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

    #[test]
    fn allocate_issues_valid_handles() {
        let mut alloc = HandleAllocator::<MeshMarker>::new();
        let a = alloc.allocate();
        let b = alloc.allocate();
        assert_ne!(a, b);
        assert!(alloc.is_valid(a));
        assert!(alloc.is_valid(b));
        assert_eq!(alloc.live_count(), 2);
    }

    #[test]
    fn freed_handle_does_not_validate() {
        let mut alloc = HandleAllocator::<MeshMarker>::new();
        let h = alloc.allocate();
        alloc.free(h).expect("first free must succeed");
        assert!(!alloc.is_valid(h));
        assert_eq!(alloc.live_count(), 0);
    }

    #[test]
    fn stale_handle_stays_invalid_after_slot_reuse() {
        let mut alloc = HandleAllocator::<MeshMarker>::new();
        let old = alloc.allocate();
        alloc.free(old).expect("first free must succeed");

        let reused = alloc.allocate();
        assert_eq!(reused.index(), old.index(), "slot should be reused");
        assert_ne!(reused.generation(), old.generation());
        assert!(alloc.is_valid(reused));
        assert!(!alloc.is_valid(old), "stale handle must not validate");
    }

    #[test]
    fn double_free_is_an_error() {
        let mut alloc = HandleAllocator::<MeshMarker>::new();
        let h = alloc.allocate();
        alloc.free(h).expect("first free must succeed");
        assert_eq!(
            alloc.free(h),
            Err(StaleHandle {
                index: h.index(),
                generation: h.generation()
            })
        );
        assert_eq!(alloc.live_count(), 0, "double free must not corrupt counts");
    }

    #[test]
    fn never_allocated_handle_does_not_validate() {
        let alloc = HandleAllocator::<MeshMarker>::new();
        assert!(!alloc.is_valid(MeshHandle::new(7, 0)));
    }
}
