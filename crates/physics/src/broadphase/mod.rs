//! Broadphase: BVH tree with incremental AABB updates.

pub mod bvh;

pub use bvh::Bvh;

use elderforge_core::math::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.min.cmple(other.max).all() && other.min.cmple(self.max).all()
    }

    pub fn merge(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

/// Naive O(n²) broadphase: tests every pair of AABBs and returns the indices
/// of those that overlap, as `(i, j)` with `i < j`.
///
/// Placeholder until the BVH (Phase 6) replaces the all-pairs scan with an
/// incremental tree. Pair indices map back into the caller's body slice.
pub fn naive_pairs(aabbs: &[Aabb]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for i in 0..aabbs.len() {
        for j in (i + 1)..aabbs.len() {
            if aabbs[i].overlaps(&aabbs[j]) {
                pairs.push((i, j));
            }
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_detection() {
        let a = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let b = Aabb::new(Vec3::splat(0.5), Vec3::splat(1.5));
        let c = Aabb::new(Vec3::splat(2.0), Vec3::splat(3.0));
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn merge_contains_both() {
        let a = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let b = Aabb::new(Vec3::splat(2.0), Vec3::splat(3.0));
        let merged = a.merge(&b);
        assert!(merged.overlaps(&a));
        assert!(merged.overlaps(&b));
        assert_eq!(merged.min, Vec3::ZERO);
        assert_eq!(merged.max, Vec3::splat(3.0));
    }

    #[test]
    fn naive_pairs_reports_only_overlaps() {
        let aabbs = [
            Aabb::new(Vec3::ZERO, Vec3::ONE),          // 0
            Aabb::new(Vec3::splat(0.5), Vec3::splat(1.5)), // 1, overlaps 0
            Aabb::new(Vec3::splat(10.0), Vec3::splat(11.0)), // 2, isolated
        ];
        assert_eq!(naive_pairs(&aabbs), vec![(0, 1)]);
    }

    #[test]
    fn naive_pairs_empty_for_fewer_than_two() {
        assert!(naive_pairs(&[]).is_empty());
        assert!(naive_pairs(&[Aabb::new(Vec3::ZERO, Vec3::ONE)]).is_empty());
    }
}
