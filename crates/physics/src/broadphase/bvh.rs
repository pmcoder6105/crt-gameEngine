//! BVH tree with incremental AABB updates.
//! // TODO: real tree topology (internal nodes, SAH insertion, refit); the
//! current storage is a flat leaf list so the interface can stabilize first.

use crate::body::BodyHandle;

use super::Aabb;

#[derive(Debug, Clone)]
struct BvhNode {
    aabb: Aabb,
    body: BodyHandle,
}

#[derive(Debug, Default, Clone)]
pub struct Bvh {
    nodes: Vec<BvhNode>,
}

impl Bvh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, body: BodyHandle, aabb: Aabb) {
        self.nodes.push(BvhNode { aabb, body });
    }

    /// Update a leaf's AABB in place (incremental refit).
    pub fn update(&mut self, body: BodyHandle, aabb: Aabb) {
        for node in &mut self.nodes {
            if node.body == body {
                node.aabb = aabb;
                return;
            }
        }
    }

    /// Collect bodies whose AABBs overlap `aabb`.
    pub fn query(&self, aabb: &Aabb, out: &mut Vec<BodyHandle>) {
        for node in &self.nodes {
            if node.aabb.overlaps(aabb) {
                out.push(node.body);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use elderforge_core::math::Vec3;

    use super::*;

    fn handle(index: u32) -> BodyHandle {
        BodyHandle::new(index, 0)
    }

    #[test]
    fn insert_and_query() {
        let mut bvh = Bvh::new();
        bvh.insert(handle(0), Aabb::new(Vec3::ZERO, Vec3::ONE));
        bvh.insert(handle(1), Aabb::new(Vec3::splat(10.0), Vec3::splat(11.0)));
        assert_eq!(bvh.len(), 2);

        let mut hits = Vec::new();
        bvh.query(&Aabb::new(Vec3::splat(0.5), Vec3::splat(1.5)), &mut hits);
        assert_eq!(hits, vec![handle(0)]);
    }

    #[test]
    fn update_moves_leaf() {
        let mut bvh = Bvh::new();
        bvh.insert(handle(0), Aabb::new(Vec3::ZERO, Vec3::ONE));
        bvh.update(handle(0), Aabb::new(Vec3::splat(5.0), Vec3::splat(6.0)));

        let mut hits = Vec::new();
        bvh.query(&Aabb::new(Vec3::ZERO, Vec3::ONE), &mut hits);
        assert!(hits.is_empty());
        bvh.query(&Aabb::new(Vec3::splat(5.5), Vec3::splat(5.6)), &mut hits);
        assert_eq!(hits, vec![handle(0)]);
    }
}
