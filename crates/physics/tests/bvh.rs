//! BVH broadphase tests: correctness against brute force, a performance
//! crossover at scale, and balance maintenance under heavy movement.

use std::time::Instant;

use elderforge_core::math::Vec3;
use elderforge_physics::broadphase::{naive_pairs, Aabb, Bvh};

/// Tiny deterministic xorshift64 RNG so the tests are reproducible.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }
    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        (x >> 32) as u32
    }
    fn unit(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.unit() * (hi - lo)
    }
}

fn box_at(center: Vec3, half: f32) -> Aabb {
    Aabb::new(center - Vec3::splat(half), center + Vec3::splat(half))
}

/// `n` random small boxes in a cube of side `space`.
fn random_boxes(rng: &mut Rng, n: usize, space: f32, half: f32) -> Vec<Aabb> {
    (0..n)
        .map(|_| {
            box_at(
                Vec3::new(
                    rng.range(0.0, space),
                    rng.range(0.0, space),
                    rng.range(0.0, space),
                ),
                half,
            )
        })
        .collect()
}

fn sorted(mut pairs: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    pairs.sort_unstable();
    pairs
}

#[test]
fn bvh_matches_brute_force_on_1000_random_aabbs() {
    let mut rng = Rng::new(0x1234_5678);
    // A fairly dense field so there are plenty of real overlaps to find.
    let aabbs = random_boxes(&mut rng, 1000, 25.0, 1.0);

    let bvh = Bvh::build(&aabbs);
    let bvh_pairs = sorted(bvh.query_pairs());
    let brute_pairs = sorted(naive_pairs(&aabbs));

    assert!(
        !brute_pairs.is_empty(),
        "test should exercise real overlaps"
    );
    assert_eq!(
        bvh_pairs, brute_pairs,
        "BVH found {} pairs, brute force found {}",
        bvh_pairs.len(),
        brute_pairs.len()
    );
}

#[test]
fn bvh_is_faster_than_brute_force_at_10k() {
    // Sparse boxes in a big space: few overlaps, so this measures broadphase
    // culling rather than result-set construction.
    fn measure(n: usize) -> (std::time::Duration, std::time::Duration, usize, usize) {
        let mut rng = Rng::new(0xBEEF_0000 ^ n as u64);
        let space = (n as f32).cbrt() * 6.0;
        let aabbs = random_boxes(&mut rng, n, space, 0.5);

        let t0 = Instant::now();
        let brute = naive_pairs(&aabbs);
        let brute_time = t0.elapsed();

        let t1 = Instant::now();
        let bvh = Bvh::build(&aabbs);
        let pairs = bvh.query_pairs();
        let bvh_time = t1.elapsed();

        // Same answer, regardless of speed.
        assert_eq!(sorted(pairs.clone()), sorted(brute.clone()));
        (brute_time, bvh_time, pairs.len(), n)
    }

    let (b1k, v1k, p1k, _) = measure(1_000);
    eprintln!("  1k: brute {b1k:?}, bvh {v1k:?} ({p1k} pairs)");
    let (b10k, v10k, p10k, _) = measure(10_000);
    eprintln!("10k: brute {b10k:?}, bvh {v10k:?} ({p10k} pairs)");

    assert!(
        v10k < b10k,
        "BVH ({v10k:?}) must beat brute force ({b10k:?}) at 10k bodies"
    );
}

#[test]
fn tree_stays_balanced_under_heavy_movement() {
    const N: usize = 1024;
    let mut rng = Rng::new(0x0BAD_F00D);
    let space = 100.0;
    let half = 0.5;

    let mut aabbs = random_boxes(&mut rng, N, space, half);
    let mut centers: Vec<Vec3> = aabbs.iter().map(|a| a.center()).collect();

    let mut bvh = Bvh::build(&aabbs);
    let limit = (2.0 * (N as f64).log2()).floor() as usize; // 2 * log2(1024) = 20

    for frame in 0..100 {
        // Move half the bodies by a random walk, clamped to the play area.
        for i in (0..N).filter(|i| (i + frame) % 2 == 0) {
            let delta = Vec3::new(
                rng.range(-6.0, 6.0),
                rng.range(-6.0, 6.0),
                rng.range(-6.0, 6.0),
            );
            centers[i] = (centers[i] + delta).clamp(Vec3::ZERO, Vec3::splat(space));
            aabbs[i] = box_at(centers[i], half);
            bvh.refit(i, aabbs[i]);
        }

        let depth = bvh.max_depth();
        assert!(
            depth <= limit,
            "frame {frame}: depth {depth} exceeds 2*log2(n) = {limit}"
        );
    }

    // Refit must also keep the tree *correct*, not just balanced.
    assert_eq!(
        sorted(bvh.query_pairs()),
        sorted(naive_pairs(&aabbs)),
        "after 100 frames of refit the candidate set must still match brute force"
    );
}
