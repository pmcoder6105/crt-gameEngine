//! Bounding-volume hierarchy broadphase.
//!
//! A binary BVH built top-down with a binned surface-area heuristic (SAH).
//! Each leaf bounds exactly one body; each internal node bounds its two
//! children. Bodies are referenced by a caller-supplied index (their position
//! in the slice passed to [`Bvh::build`]).
//!
//! ## Incremental maintenance
//!
//! Every leaf remembers an *expanded* AABB — its tight bounds at build time,
//! fattened by a margin. [`Bvh::refit`] uses it as a slack region:
//!
//! * If a body's new AABB still fits its expanded box, only the node AABBs
//!   from the leaf up to the root are refitted; the tree topology is untouched
//!   (cheap, and depth is unchanged).
//! * Otherwise the body has left its slack region, so the smallest subtree
//!   still containing it is rebuilt from scratch with the SAH. Because a
//!   rebuilt subtree holds the same leaves (one of them moved) and is built
//!   balanced, the tree stays within a small factor of `log2(n)` deep no
//!   matter how the bodies move.

use elderforge_core::math::Vec3;

use super::Aabb;

/// Sentinel node index meaning "none" (used for the root's parent and during
/// construction before children are linked).
const NIL: u32 = u32::MAX;

/// SAH bin count per axis. 12 is the usual sweet spot (PBRT).
const BINS: usize = 12;

/// A node in the [`Bvh`]: an AABB plus either two child node indices
/// (internal) or one body index (leaf).
#[derive(Debug, Clone, Copy)]
pub struct BvhNode {
    pub aabb: Aabb,
    parent: u32,
    kind: NodeKind,
}

#[derive(Debug, Clone, Copy)]
enum NodeKind {
    Internal { left: u32, right: u32 },
    Leaf { body: u32 },
}

impl BvhNode {
    pub fn is_leaf(&self) -> bool {
        matches!(self.kind, NodeKind::Leaf { .. })
    }

    /// The body index for a leaf, or `None` for an internal node.
    pub fn body(&self) -> Option<usize> {
        match self.kind {
            NodeKind::Leaf { body } => Some(body as usize),
            NodeKind::Internal { .. } => None,
        }
    }
}

/// Scratch record for one primitive during construction.
#[derive(Clone, Copy)]
struct PrimRef {
    body: u32,
    aabb: Aabb,
    centroid: Vec3,
}

/// A bounding-volume hierarchy over a set of body AABBs.
#[derive(Debug, Clone, Default)]
pub struct Bvh {
    /// Node pool. Freed nodes are recycled via `free`.
    nodes: Vec<BvhNode>,
    root: u32,
    free: Vec<u32>,
    /// body index -> its leaf node index (`NIL` if the body has no leaf).
    body_leaf: Vec<u32>,
    /// body index -> the leaf's expanded (slack) AABB.
    expanded: Vec<Aabb>,
    /// Fattening margin applied to leaf AABBs.
    margin: f32,
}

impl Bvh {
    /// Build a BVH over `aabbs`, where body index `i` is `aabbs[i]`.
    pub fn build(aabbs: &[Aabb]) -> Self {
        let mut bvh = Bvh {
            root: NIL,
            margin: margin_for(aabbs),
            body_leaf: vec![NIL; aabbs.len()],
            expanded: vec![Aabb::new(Vec3::ZERO, Vec3::ZERO); aabbs.len()],
            ..Default::default()
        };
        if aabbs.is_empty() {
            return bvh;
        }
        let mut prims: Vec<PrimRef> = aabbs
            .iter()
            .enumerate()
            .map(|(i, &aabb)| PrimRef {
                body: i as u32,
                aabb,
                centroid: aabb.center(),
            })
            .collect();
        bvh.root = bvh.build_subtree(&mut prims, NIL);
        bvh
    }

    pub fn is_empty(&self) -> bool {
        self.root == NIL
    }

    /// Number of live (reachable) nodes — leaves plus internal nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len() - self.free.len()
    }

    /// All candidate pairs of bodies whose leaf AABBs overlap, each as
    /// `(i, j)` with `i < j`. This is the broadphase output: a superset of the
    /// truly-colliding pairs that narrowphase then confirms.
    pub fn query_pairs(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        if self.root == NIL {
            return out;
        }

        // First, every internal node contributes a "cross" test between its two
        // child subtrees; recursing into each child covers within-subtree pairs.
        let mut cross: Vec<(u32, u32)> = Vec::new();
        let mut stack = vec![self.root];
        while let Some(n) = stack.pop() {
            if let NodeKind::Internal { left, right } = self.nodes[n as usize].kind {
                cross.push((left, right));
                stack.push(left);
                stack.push(right);
            }
        }

        // Resolve each cross test down to overlapping leaf pairs.
        while let Some((a, b)) = cross.pop() {
            let (na, nb) = (&self.nodes[a as usize], &self.nodes[b as usize]);
            if !na.aabb.overlaps(&nb.aabb) {
                continue;
            }
            match (na.kind, nb.kind) {
                (NodeKind::Leaf { body: ba }, NodeKind::Leaf { body: bb }) => {
                    let (lo, hi) = (ba.min(bb) as usize, ba.max(bb) as usize);
                    out.push((lo, hi));
                }
                (NodeKind::Internal { left, right }, NodeKind::Leaf { .. }) => {
                    cross.push((left, b));
                    cross.push((right, b));
                }
                (NodeKind::Leaf { .. }, NodeKind::Internal { left, right }) => {
                    cross.push((a, left));
                    cross.push((a, right));
                }
                (NodeKind::Internal { left, right }, NodeKind::Internal { .. }) => {
                    // Descend `a`; once it bottoms out, the leaf/internal arms
                    // above descend `b`. Every leaf pair is reached once.
                    cross.push((left, b));
                    cross.push((right, b));
                }
            }
        }
        out
    }

    /// Update body `body` to `new_aabb`. Cheap refit up the tree while the body
    /// stays within its leaf's expanded box; otherwise rebuilds the smallest
    /// subtree that still contains it (see the module docs).
    pub fn refit(&mut self, body: usize, new_aabb: Aabb) {
        let leaf = self.body_leaf[body];
        if leaf == NIL {
            return;
        }
        self.nodes[leaf as usize].aabb = new_aabb;

        if self.expanded[body].contains(&new_aabb) {
            // Slack still covers the body: just pull ancestor AABBs in/out.
            let parent = self.nodes[leaf as usize].parent;
            self.refit_ancestors(parent);
        } else {
            self.rebuild_around(leaf, new_aabb);
        }
    }

    /// Walk from `node` to the root, recomputing each internal AABB as the
    /// union of its children.
    fn refit_ancestors(&mut self, mut node: u32) {
        while node != NIL {
            if let NodeKind::Internal { left, right } = self.nodes[node as usize].kind {
                self.nodes[node as usize].aabb =
                    self.nodes[left as usize].aabb.merge(&self.nodes[right as usize].aabb);
            }
            node = self.nodes[node as usize].parent;
        }
    }

    /// A body escaped its leaf's slack box: find the lowest ancestor whose AABB
    /// still contains the body (or the root), rebuild that subtree's leaves
    /// with the SAH, and splice it back in.
    fn rebuild_around(&mut self, leaf: u32, new_aabb: Aabb) {
        // Lowest ancestor containing the new AABB, defaulting to the root.
        let mut top = self.nodes[leaf as usize].parent;
        while top != NIL && !self.nodes[top as usize].aabb.contains(&new_aabb) {
            top = self.nodes[top as usize].parent;
        }
        if top == NIL {
            top = self.root;
        }

        let grandparent = self.nodes[top as usize].parent;
        // Remember which child of the grandparent `top` was.
        let top_is_left = grandparent != NIL
            && matches!(self.nodes[grandparent as usize].kind, NodeKind::Internal { left, .. } if left == top);

        // Collect `top`'s leaves (with their current AABBs) and free its nodes.
        let mut prims = Vec::new();
        self.collect_and_free(top, &mut prims);

        let new_root = self.build_subtree(&mut prims, grandparent);

        if grandparent == NIL {
            self.root = new_root;
        } else if let NodeKind::Internal { left, right } = &mut self.nodes[grandparent as usize].kind {
            if top_is_left {
                *left = new_root;
            } else {
                *right = new_root;
            }
        }
        self.refit_ancestors(grandparent);
    }

    /// Recursively gather the leaves under `node` into `out` and push every
    /// visited node onto the free list for reuse.
    fn collect_and_free(&mut self, node: u32, out: &mut Vec<PrimRef>) {
        match self.nodes[node as usize].kind {
            NodeKind::Leaf { body } => {
                let aabb = self.nodes[node as usize].aabb;
                out.push(PrimRef { body, aabb, centroid: aabb.center() });
            }
            NodeKind::Internal { left, right } => {
                self.collect_and_free(left, out);
                self.collect_and_free(right, out);
            }
        }
        self.free.push(node);
    }

    /// All internal-node AABBs, for debug visualization of the tree structure.
    pub fn debug_iter_aabbs(&self) -> impl Iterator<Item = Aabb> {
        let mut boxes = Vec::new();
        if self.root != NIL {
            let mut stack = vec![self.root];
            while let Some(n) = stack.pop() {
                if let NodeKind::Internal { left, right } = self.nodes[n as usize].kind {
                    boxes.push(self.nodes[n as usize].aabb);
                    stack.push(left);
                    stack.push(right);
                }
            }
        }
        boxes.into_iter()
    }

    /// Maximum leaf depth (edges from the root). 0 for a single-leaf or empty
    /// tree. Used to check the tree stays balanced.
    pub fn max_depth(&self) -> usize {
        if self.root == NIL {
            return 0;
        }
        let mut max = 0;
        let mut stack = vec![(self.root, 0usize)];
        while let Some((n, depth)) = stack.pop() {
            match self.nodes[n as usize].kind {
                NodeKind::Leaf { .. } => max = max.max(depth),
                NodeKind::Internal { left, right } => {
                    stack.push((left, depth + 1));
                    stack.push((right, depth + 1));
                }
            }
        }
        max
    }

    // --- construction ---

    /// Allocate a node, reusing a freed slot when available.
    fn alloc(&mut self, node: BvhNode) -> u32 {
        if let Some(i) = self.free.pop() {
            self.nodes[i as usize] = node;
            i
        } else {
            self.nodes.push(node);
            (self.nodes.len() - 1) as u32
        }
    }

    /// Build a subtree over `prims` in place, returning its root node index.
    fn build_subtree(&mut self, prims: &mut [PrimRef], parent: u32) -> u32 {
        if prims.len() == 1 {
            let p = prims[0];
            let node = self.alloc(BvhNode {
                aabb: p.aabb,
                parent,
                kind: NodeKind::Leaf { body: p.body },
            });
            self.body_leaf[p.body as usize] = node;
            self.expanded[p.body as usize] = p.aabb.expanded(self.margin);
            return node;
        }

        let mid = sah_partition(prims);
        // Allocate the internal node before recursing so children can point back.
        let node = self.alloc(BvhNode {
            aabb: prims[0].aabb,
            parent,
            kind: NodeKind::Internal { left: NIL, right: NIL },
        });
        let (left_prims, right_prims) = prims.split_at_mut(mid);
        let left = self.build_subtree(left_prims, node);
        let right = self.build_subtree(right_prims, node);
        let aabb = self.nodes[left as usize].aabb.merge(&self.nodes[right as usize].aabb);
        self.nodes[node as usize].kind = NodeKind::Internal { left, right };
        self.nodes[node as usize].aabb = aabb;
        node
    }
}

/// Component `axis` (0=x, 1=y, 2=z) of a vector.
fn axis_of(v: Vec3, axis: usize) -> f32 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// Fattening margin for leaf slack boxes: a fraction of the mean AABB extent,
/// with a small floor so degenerate (point) inputs still get nonzero slack.
fn margin_for(aabbs: &[Aabb]) -> f32 {
    if aabbs.is_empty() {
        return 1e-4;
    }
    let mean: f32 = aabbs
        .iter()
        .map(|a| (a.max - a.min).max_element())
        .sum::<f32>()
        / aabbs.len() as f32;
    (0.2 * mean).max(1e-4)
}

/// Choose a split with the binned SAH and partition `prims` in place, returning
/// the number that went left (always in `1..len`). Falls back to a median index
/// split when the centroids are degenerate.
fn sah_partition(prims: &mut [PrimRef]) -> usize {
    let n = prims.len();

    // Bounds of the centroids; SAH splits along the axis they spread most.
    let mut cmin = prims[0].centroid;
    let mut cmax = prims[0].centroid;
    for p in &prims[1..] {
        cmin = cmin.min(p.centroid);
        cmax = cmax.max(p.centroid);
    }
    let extent = cmax - cmin;
    let split_axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    let axis_extent = axis_of(extent, split_axis);
    if axis_extent <= 1e-12 {
        return n / 2; // all centroids coincide on this axis
    }

    // Bin the primitives by centroid along the split axis.
    let scale = BINS as f32 / axis_extent;
    let lo = axis_of(cmin, split_axis);
    let bin_of = |p: &PrimRef| -> usize {
        (((axis_of(p.centroid, split_axis) - lo) * scale) as usize).min(BINS - 1)
    };

    let mut bin_count = [0usize; BINS];
    let mut bin_box: [Option<Aabb>; BINS] = [None; BINS];
    for p in prims.iter() {
        let b = bin_of(p);
        bin_count[b] += 1;
        bin_box[b] = Some(match bin_box[b] {
            Some(box_) => box_.merge(&p.aabb),
            None => p.aabb,
        });
    }

    // Prefix (left) and suffix (right) sweeps over the bins.
    let mut left_area = [0.0f32; BINS];
    let mut left_count = [0usize; BINS];
    let mut acc: Option<Aabb> = None;
    let mut cnt = 0usize;
    for i in 0..BINS {
        if let Some(b) = bin_box[i] {
            acc = Some(match acc {
                Some(a) => a.merge(&b),
                None => b,
            });
        }
        cnt += bin_count[i];
        left_area[i] = acc.map_or(0.0, |a| a.surface_area());
        left_count[i] = cnt;
    }
    let mut right_area = [0.0f32; BINS];
    let mut right_count = [0usize; BINS];
    acc = None;
    cnt = 0;
    for i in (0..BINS).rev() {
        if let Some(b) = bin_box[i] {
            acc = Some(match acc {
                Some(a) => a.merge(&b),
                None => b,
            });
        }
        cnt += bin_count[i];
        right_area[i] = acc.map_or(0.0, |a| a.surface_area());
        right_count[i] = cnt;
    }

    // Pick the split (left = bins 0..=best) of minimum SAH cost.
    let mut best = 0usize;
    let mut best_cost = f32::INFINITY;
    for i in 0..BINS - 1 {
        let cost = left_count[i] as f32 * left_area[i]
            + right_count[i + 1] as f32 * right_area[i + 1];
        if cost < best_cost {
            best_cost = cost;
            best = i;
        }
    }

    // Partition: bins <= best to the left.
    let mut i = 0;
    for j in 0..n {
        if bin_of(&prims[j]) <= best {
            prims.swap(i, j);
            i += 1;
        }
    }
    if i == 0 || i == n {
        n / 2 // degenerate split; fall back to halves so both sides are non-empty
    } else {
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aabb_at(x: f32, y: f32, z: f32, r: f32) -> Aabb {
        Aabb::new(Vec3::new(x - r, y - r, z - r), Vec3::new(x + r, y + r, z + r))
    }

    #[test]
    fn empty_tree_has_no_pairs() {
        let bvh = Bvh::build(&[]);
        assert!(bvh.is_empty());
        assert!(bvh.query_pairs().is_empty());
        assert_eq!(bvh.max_depth(), 0);
    }

    #[test]
    fn single_body_is_a_leaf_root() {
        let bvh = Bvh::build(&[aabb_at(0.0, 0.0, 0.0, 1.0)]);
        assert!(!bvh.is_empty());
        assert_eq!(bvh.max_depth(), 0);
        assert!(bvh.query_pairs().is_empty());
        assert_eq!(bvh.debug_iter_aabbs().count(), 0); // no internal nodes
    }

    #[test]
    fn finds_one_overlapping_pair() {
        let aabbs = [
            aabb_at(0.0, 0.0, 0.0, 1.0),
            aabb_at(1.0, 0.0, 0.0, 1.0),  // overlaps 0
            aabb_at(20.0, 0.0, 0.0, 1.0), // isolated
        ];
        let mut pairs = Bvh::build(&aabbs).query_pairs();
        pairs.sort_unstable();
        assert_eq!(pairs, vec![(0, 1)]);
    }

    #[test]
    fn debug_aabbs_bound_their_bodies() {
        let aabbs: Vec<Aabb> = (0..8).map(|i| aabb_at(i as f32 * 3.0, 0.0, 0.0, 1.0)).collect();
        let bvh = Bvh::build(&aabbs);
        // The first internal AABB (the root) must contain every body.
        let root_box = bvh.debug_iter_aabbs().next().expect("has internal nodes");
        for a in &aabbs {
            assert!(root_box.contains(a));
        }
    }

    #[test]
    fn refit_within_slack_preserves_topology() {
        let aabbs: Vec<Aabb> = (0..16).map(|i| aabb_at(i as f32 * 4.0, 0.0, 0.0, 0.5)).collect();
        let mut bvh = Bvh::build(&aabbs);
        let depth_before = bvh.max_depth();
        let nodes_before = bvh.node_count();
        // Nudge a body a hair (well inside its slack): topology unchanged.
        bvh.refit(5, aabb_at(20.0 + 0.01, 0.0, 0.0, 0.5));
        assert_eq!(bvh.max_depth(), depth_before);
        assert_eq!(bvh.node_count(), nodes_before);
    }

    #[test]
    fn refit_after_large_move_stays_correct() {
        let aabbs: Vec<Aabb> = (0..32).map(|i| aabb_at(i as f32 * 4.0, 0.0, 0.0, 0.5)).collect();
        let mut bvh = Bvh::build(&aabbs);
        // Teleport body 0 next to body 31; they should become a candidate pair.
        let moved = aabb_at(31.0 * 4.0 + 0.2, 0.0, 0.0, 0.5);
        bvh.refit(0, moved);
        let pairs = bvh.query_pairs();
        assert!(
            pairs.contains(&(0, 31)),
            "moved body should overlap its new neighbor: {pairs:?}"
        );
    }
}
