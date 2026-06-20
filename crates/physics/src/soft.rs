//! Soft bodies and cloth: the particle cloud and the topology that ties it
//! together, both solved XPBD-natively by [`PhysicsWorld`](crate::PhysicsWorld).
//!
//! A **particle** ([`Particle`]) is the shared degree of freedom for both: a
//! point mass with a position, a previous position (for XPBD velocity
//! derivation), a velocity, an inverse mass (`0` pins it in place), and a small
//! collision radius (its thickness against rigid bodies).
//!
//! A [`SoftBody`] is a volumetric blob: a particle cloud knit together by a
//! **tetrahedral mesh**. Each tet contributes a volume-preservation constraint
//! and its edges contribute distance constraints, so the body resists both
//! shearing and squashing. [`SoftBodyDef::box_lattice`] / [`SoftBodyDef::ball`]
//! build one from a regular lattice using Kuhn's six-tet decomposition (which
//! tiles space without the parity mismatch of the five-tet split) and extract
//! the boundary triangles for rendering.
//!
//! A [`Cloth`] is a 2D grid of particles wired with three families of distance
//! constraints — **structural** (orthogonal neighbours), **shear** (diagonal
//! neighbours), and **bending** (every other particle) — which is the standard
//! mass-spring cloth model recast as XPBD distance constraints.
//!
//! The [`*Def`](SoftBodyDef) types are *builders*: they hold local particle
//! indices and rest measurements. Handing one to the world appends its
//! particles, offsets the indices into the world's flat particle array, and
//! stores the live constraints; the world keeps a [`SoftBody`] / [`Cloth`]
//! describing where the particles live and how to draw them.

use elderforge_core::math::Vec3;

/// A single soft-body / cloth particle: the shared XPBD degree of freedom.
#[derive(Debug, Clone, Copy)]
pub struct Particle {
    /// Current world position.
    pub position: Vec3,
    /// Position at the start of the current substep; velocity is derived from
    /// `position - prev_position` after the constraint solve.
    pub prev_position: Vec3,
    pub velocity: Vec3,
    /// Inverse mass in 1/kg. Zero pins the particle (a flag corner, a held
    /// vertex): it is never integrated and absorbs none of a constraint.
    pub inv_mass: f32,
    /// Collision thickness against rigid bodies — the particle is treated as a
    /// sphere of this radius so cloth/soft surfaces rest *on* geometry instead
    /// of passing through it. Zero is a bare point.
    pub radius: f32,
}

impl Particle {
    /// A particle at `position` with the given inverse mass and collision radius.
    pub fn new(position: Vec3, inv_mass: f32, radius: f32) -> Self {
        Self {
            position,
            prev_position: position,
            velocity: Vec3::ZERO,
            inv_mass,
            radius,
        }
    }

    /// Whether this particle is movable (finite mass and not pinned).
    pub fn is_dynamic(&self) -> bool {
        self.inv_mass > 0.0
    }
}

/// Signed volume of the tetrahedron `(a, b, c, d)`, ⅙·(b−a)·((c−a)×(d−a)).
///
/// The sign depends on vertex ordering; the volume constraint stores this signed
/// rest value and drives the live volume back to it, so only *consistency*
/// between rest and runtime matters, not the absolute sign.
pub fn signed_tet_volume(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f32 {
    (b - a).dot((c - a).cross(d - a)) / 6.0
}

/// Builder for a [`SoftBody`]: a particle cloud, its tetrahedral mesh (edge
/// distance constraints + per-tet volume constraints), and the boundary
/// triangles used to render its surface. Build one with [`box_lattice`] or
/// [`ball`], then hand it to [`PhysicsWorld::add_soft_body`].
///
/// [`box_lattice`]: SoftBodyDef::box_lattice
/// [`ball`]: SoftBodyDef::ball
/// [`PhysicsWorld::add_soft_body`]: crate::PhysicsWorld::add_soft_body
#[derive(Debug, Clone)]
pub struct SoftBodyDef {
    /// Particle positions (world space), indexed by the local indices used in
    /// `edges`, `tets`, and `surface`.
    pub particles: Vec<Vec3>,
    /// Inverse mass per particle, parallel to `particles`.
    pub inv_masses: Vec<f32>,
    /// Unique tetrahedron edges as `(a, b, rest_length)` — the distance
    /// constraints holding the lattice together.
    pub edges: Vec<(u32, u32, f32)>,
    /// Tetrahedra as `(indices, signed_rest_volume)` — the volume constraints.
    pub tets: Vec<([u32; 4], f32)>,
    /// Outward-oriented boundary triangles (faces belonging to a single tet),
    /// for rendering the surface.
    pub surface: Vec<[u32; 3]>,
    /// Compliance (inverse stiffness) of the edge distance constraints.
    pub distance_compliance: f32,
    /// Compliance of the per-tet volume constraints.
    pub volume_compliance: f32,
    /// Collision radius given to every particle.
    pub particle_radius: f32,
}

impl SoftBodyDef {
    /// A solid soft block: a `(nx, ny, nz)`-cell lattice spanning
    /// `center ± half_extents`, every cell present. `total_mass` is split evenly
    /// across the nodes. See [`lattice`](Self::lattice) for the general form.
    pub fn box_lattice(
        center: Vec3,
        half_extents: Vec3,
        cells: [u32; 3],
        total_mass: f32,
    ) -> Self {
        Self::lattice(center, half_extents, cells, total_mass, |_| true)
    }

    /// A soft ball: a lattice over the bounding cube of `radius`, keeping only
    /// the cells whose centre lies within the sphere — a rounded blob whose
    /// surface triangles are extracted from the kept tets. `resolution` is the
    /// cell count across the diameter.
    pub fn ball(center: Vec3, radius: f32, resolution: u32, total_mass: f32) -> Self {
        let r = radius.max(1e-4);
        let res = resolution.max(1);
        Self::lattice(
            center,
            Vec3::splat(r),
            [res; 3],
            total_mass,
            |cell_center| cell_center.length() <= r,
        )
    }

    /// General lattice builder. Nodes sit on a regular `(nx+1)(ny+1)(nz+1)`
    /// grid over `center ± half_extents`; a cell is included when `keep_cell`
    /// returns true for its centre (expressed relative to `center`). Each kept
    /// cell is split into six tetrahedra sharing its main diagonal (Kuhn's
    /// triangulation, which tiles consistently so shared faces always match).
    /// Surface triangles are the faces belonging to exactly one tet, oriented
    /// outward.
    pub fn lattice(
        center: Vec3,
        half_extents: Vec3,
        cells: [u32; 3],
        total_mass: f32,
        keep_cell: impl Fn(Vec3) -> bool,
    ) -> Self {
        let (nx, ny, nz) = (cells[0].max(1), cells[1].max(1), cells[2].max(1));
        let nodes = [nx + 1, ny + 1, nz + 1];
        let extent = 2.0 * half_extents;
        let step = Vec3::new(
            extent.x / nx as f32,
            extent.y / ny as f32,
            extent.z / nz as f32,
        );
        let origin = center - half_extents;

        // Node index helper into the dense grid.
        let node_at = |i: u32, j: u32, k: u32| -> u32 {
            (k * nodes[1] + j) * nodes[0] + i
        };
        let node_pos = |i: u32, j: u32, k: u32| -> Vec3 {
            origin + Vec3::new(i as f32 * step.x, j as f32 * step.y, k as f32 * step.z)
        };

        // Dense node positions (some interior nodes of dropped cells may end up
        // unused; harmless, they just carry no constraints and aren't drawn).
        let mut particles = Vec::new();
        for k in 0..nodes[2] {
            for j in 0..nodes[1] {
                for i in 0..nodes[0] {
                    particles.push(node_pos(i, j, k));
                }
            }
        }

        // Kuhn's six tetrahedra of a cell, by corner bit pattern
        // (corner = x + 2y + 4z); all share the 0→7 main diagonal.
        const KUHN: [[usize; 4]; 6] = [
            [0, 1, 3, 7],
            [0, 1, 5, 7],
            [0, 2, 3, 7],
            [0, 2, 6, 7],
            [0, 4, 5, 7],
            [0, 4, 6, 7],
        ];

        let mut tet_indices: Vec<[u32; 4]> = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let cell_center = Vec3::new(
                        (i as f32 + 0.5) * step.x,
                        (j as f32 + 0.5) * step.y,
                        (k as f32 + 0.5) * step.z,
                    ) - half_extents;
                    if !keep_cell(cell_center) {
                        continue;
                    }
                    // The 8 corners of this cell, indexed by bit pattern.
                    let corner = |bits: usize| -> u32 {
                        let dx = (bits & 1) as u32;
                        let dy = ((bits >> 1) & 1) as u32;
                        let dz = ((bits >> 2) & 1) as u32;
                        node_at(i + dx, j + dy, k + dz)
                    };
                    for tet in KUHN {
                        tet_indices.push([
                            corner(tet[0]),
                            corner(tet[1]),
                            corner(tet[2]),
                            corner(tet[3]),
                        ]);
                    }
                }
            }
        }

        Self::from_topology(particles, tet_indices, total_mass)
    }

    /// Assemble a def from raw particle positions and tetrahedra: distribute
    /// mass over the nodes actually used by a tet, build the unique edge set
    /// with rest lengths, the per-tet rest volumes, and extract the outward
    /// boundary surface.
    fn from_topology(particles: Vec<Vec3>, tets: Vec<[u32; 4]>, total_mass: f32) -> Self {
        use std::collections::HashMap;

        // Only nodes touched by a tet carry mass (and get a finite inv_mass).
        let mut used = vec![false; particles.len()];
        for t in &tets {
            for &v in t {
                used[v as usize] = true;
            }
        }
        let used_count = used.iter().filter(|&&u| u).count().max(1);
        let per_node = total_mass / used_count as f32;
        let inv_mass = if per_node > 0.0 { 1.0 / per_node } else { 0.0 };
        let inv_masses = used
            .iter()
            .map(|&u| if u { inv_mass } else { 0.0 })
            .collect();

        // Unique edges (the 6 of each tet), with rest lengths.
        let mut edge_set: HashMap<(u32, u32), f32> = HashMap::new();
        const TET_EDGES: [(usize, usize); 6] =
            [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
        let mut tet_out = Vec::with_capacity(tets.len());
        for t in &tets {
            for (a, b) in TET_EDGES {
                let (u, v) = (t[a], t[b]);
                let key = if u < v { (u, v) } else { (v, u) };
                edge_set.entry(key).or_insert_with(|| {
                    (particles[u as usize] - particles[v as usize]).length()
                });
            }
            let vol = signed_tet_volume(
                particles[t[0] as usize],
                particles[t[1] as usize],
                particles[t[2] as usize],
                particles[t[3] as usize],
            );
            tet_out.push((*t, vol));
        }
        let edges = edge_set.into_iter().map(|((a, b), l)| (a, b, l)).collect();

        let surface = extract_surface(&particles, &tets);

        Self {
            particles,
            inv_masses,
            edges,
            tets: tet_out,
            surface,
            distance_compliance: 0.0,
            volume_compliance: 0.0,
            particle_radius: 0.05,
        }
    }

    /// Number of particles in the def.
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }
}

/// Extract outward-oriented boundary triangles from a tet mesh: a triangular
/// face shared by two tets is interior; one belonging to a single tet is on the
/// surface. Each surface triangle is wound so its normal points away from the
/// tet's opposite (interior) vertex.
fn extract_surface(positions: &[Vec3], tets: &[[u32; 4]]) -> Vec<[u32; 3]> {
    use std::collections::HashMap;

    // The four faces of a tet [a,b,c,d], each paired with its opposite vertex.
    const FACES: [([usize; 3], usize); 4] = [
        ([1, 2, 3], 0),
        ([0, 2, 3], 1),
        ([0, 1, 3], 2),
        ([0, 1, 2], 3),
    ];

    struct Record {
        count: u32,
        tri: [u32; 3],
        opposite: u32,
    }
    let mut faces: HashMap<[u32; 3], Record> = HashMap::new();
    for t in tets {
        for (idx, opp) in FACES {
            let tri = [t[idx[0]], t[idx[1]], t[idx[2]]];
            let mut key = tri;
            key.sort_unstable();
            faces
                .entry(key)
                .and_modify(|r| r.count += 1)
                .or_insert(Record { count: 1, tri, opposite: t[opp] });
        }
    }

    let mut surface = Vec::new();
    for r in faces.values() {
        if r.count != 1 {
            continue;
        }
        let [i0, i1, i2] = r.tri;
        let (p0, p1, p2) = (
            positions[i0 as usize],
            positions[i1 as usize],
            positions[i2 as usize],
        );
        let normal = (p1 - p0).cross(p2 - p0);
        // Outward = pointing away from the interior (opposite) vertex.
        let inward = positions[r.opposite as usize] - p0;
        if normal.dot(inward) > 0.0 {
            surface.push([i0, i2, i1]); // flip winding to face outward
        } else {
            surface.push([i0, i1, i2]);
        }
    }
    surface
}

/// A soft body resident in the world: where its particles live in the world's
/// flat particle array, and the surface triangles (local particle indices) used
/// to render its deforming skin.
#[derive(Debug, Clone)]
pub struct SoftBody {
    /// Index of this body's first particle in [`PhysicsWorld::particles`].
    pub(crate) base: usize,
    /// Number of particles this body owns (a contiguous run from `base`).
    pub(crate) count: usize,
    /// Boundary triangles, indexed *locally* (`0..count`).
    pub(crate) surface: Vec<[u32; 3]>,
}

impl SoftBody {
    /// First particle index in the world's particle array.
    pub fn base(&self) -> usize {
        self.base
    }

    /// Number of particles owned by this body.
    pub fn particle_count(&self) -> usize {
        self.count
    }

    /// Surface triangles as local particle indices (`0..particle_count`), wound
    /// outward — ready to index the slice from
    /// [`PhysicsWorld::soft_body_particles`](crate::PhysicsWorld::soft_body_particles).
    pub fn surface(&self) -> &[[u32; 3]] {
        &self.surface
    }
}

/// Handle to a [`SoftBody`] stored in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftBodyHandle(pub(crate) usize);

/// Builder for a [`Cloth`]: a 2D grid of particles plus the three families of
/// distance constraints. Build one with [`grid`](ClothDef::grid).
#[derive(Debug, Clone)]
pub struct ClothDef {
    /// Columns (particles across the width) and rows (down the height).
    pub cols: usize,
    pub rows: usize,
    /// Particle positions, row-major: index `r * cols + c`.
    pub particles: Vec<Vec3>,
    /// Inverse mass per particle (zero pins it), parallel to `particles`.
    pub inv_masses: Vec<f32>,
    /// Structural constraints — orthogonal neighbours `(a, b, rest_length)`.
    pub structural: Vec<(u32, u32, f32)>,
    /// Shear constraints — diagonal neighbours.
    pub shear: Vec<(u32, u32, f32)>,
    /// Bending constraints — every-other particle along each axis.
    pub bending: Vec<(u32, u32, f32)>,
    pub structural_compliance: f32,
    pub shear_compliance: f32,
    pub bending_compliance: f32,
    pub particle_radius: f32,
}

impl ClothDef {
    /// Build a `cols × rows` cloth. `place(c, r)` gives each particle's world
    /// position and `pinned(c, r)` marks the immovable ones (a pinned particle
    /// gets zero inverse mass; `total_mass` is split over the rest). Structural,
    /// shear, and bending constraints are generated from the grid topology with
    /// rest lengths taken from the placed positions.
    pub fn grid(
        cols: usize,
        rows: usize,
        total_mass: f32,
        place: impl Fn(usize, usize) -> Vec3,
        pinned: impl Fn(usize, usize) -> bool,
    ) -> Self {
        let cols = cols.max(2);
        let rows = rows.max(2);
        let idx = |c: usize, r: usize| -> u32 { (r * cols + c) as u32 };

        let mut particles = Vec::with_capacity(cols * rows);
        let mut pins = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                particles.push(place(c, r));
                pins.push(pinned(c, r));
            }
        }
        let free = pins.iter().filter(|&&p| !p).count().max(1);
        let per = total_mass / free as f32;
        let inv = if per > 0.0 { 1.0 / per } else { 0.0 };
        let inv_masses = pins.iter().map(|&p| if p { 0.0 } else { inv }).collect();

        let rest = |a: u32, b: u32| (particles[a as usize] - particles[b as usize]).length();

        let mut structural = Vec::new();
        let mut shear = Vec::new();
        let mut bending = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let a = idx(c, r);
                // Structural: right and down neighbours.
                if c + 1 < cols {
                    let b = idx(c + 1, r);
                    structural.push((a, b, rest(a, b)));
                }
                if r + 1 < rows {
                    let b = idx(c, r + 1);
                    structural.push((a, b, rest(a, b)));
                }
                // Shear: both diagonals of the quad to the lower-right.
                if c + 1 < cols && r + 1 < rows {
                    let br = idx(c + 1, r + 1);
                    shear.push((a, br, rest(a, br)));
                    let tr = idx(c + 1, r);
                    let bl = idx(c, r + 1);
                    shear.push((tr, bl, rest(tr, bl)));
                }
                // Bending: skip-one neighbours, right and down.
                if c + 2 < cols {
                    let b = idx(c + 2, r);
                    bending.push((a, b, rest(a, b)));
                }
                if r + 2 < rows {
                    let b = idx(c, r + 2);
                    bending.push((a, b, rest(a, b)));
                }
            }
        }

        Self {
            cols,
            rows,
            particles,
            inv_masses,
            structural,
            shear,
            bending,
            structural_compliance: 0.0,
            shear_compliance: 1e-5,
            bending_compliance: 1e-4,
            particle_radius: 0.02,
        }
    }

    /// Total particle count (`cols * rows`).
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }
}

/// A cloth resident in the world: where its grid of particles lives and how big
/// the grid is, so the renderer can build the triangle mesh and per-vertex UVs.
#[derive(Debug, Clone)]
pub struct Cloth {
    pub(crate) base: usize,
    pub(crate) cols: usize,
    pub(crate) rows: usize,
}

impl Cloth {
    /// Index of this cloth's first particle in the world's particle array.
    pub fn base(&self) -> usize {
        self.base
    }

    /// Grid dimensions: `(cols, rows)`.
    pub fn dims(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    /// Number of particles (`cols * rows`).
    pub fn particle_count(&self) -> usize {
        self.cols * self.rows
    }

    /// Triangle indices (two per quad) into the cloth's local particle grid
    /// (`0..particle_count`), row-major. Stable for the life of the cloth.
    pub fn indices(&self) -> Vec<u32> {
        let idx = |c: usize, r: usize| -> u32 { (r * self.cols + c) as u32 };
        let mut out = Vec::with_capacity((self.cols - 1) * (self.rows - 1) * 6);
        for r in 0..self.rows - 1 {
            for c in 0..self.cols - 1 {
                let (tl, tr, bl, br) = (idx(c, r), idx(c + 1, r), idx(c, r + 1), idx(c + 1, r + 1));
                out.extend_from_slice(&[tl, bl, br, tl, br, tr]);
            }
        }
        out
    }
}

/// Handle to a [`Cloth`] stored in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClothHandle(pub(crate) usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_new_seeds_prev_position() {
        let p = Particle::new(Vec3::new(1.0, 2.0, 3.0), 2.0, 0.1);
        assert_eq!(p.position, p.prev_position);
        assert_eq!(p.velocity, Vec3::ZERO);
        assert!(p.is_dynamic());
        assert!(!Particle::new(Vec3::ZERO, 0.0, 0.0).is_dynamic());
    }

    #[test]
    fn signed_volume_of_unit_tet() {
        // The standard corner tetrahedron has volume 1/6.
        let v = signed_tet_volume(Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z);
        assert!((v - 1.0 / 6.0).abs() < 1e-6);
        // Swapping two vertices flips the sign.
        let v2 = signed_tet_volume(Vec3::ZERO, Vec3::Y, Vec3::X, Vec3::Z);
        assert!((v2 + 1.0 / 6.0).abs() < 1e-6);
    }

    #[test]
    fn box_lattice_has_six_tets_per_cell() {
        let def = SoftBodyDef::box_lattice(Vec3::ZERO, Vec3::splat(0.5), [1, 1, 1], 1.0);
        // One cell → 8 nodes, 6 tets.
        assert_eq!(def.particle_count(), 8);
        assert_eq!(def.tets.len(), 6);
        // A single cube's boundary is 6 faces × 2 triangles = 12.
        assert_eq!(def.surface.len(), 12);
        // Every tet has a non-negligible rest volume.
        assert!(def.tets.iter().all(|(_, v)| v.abs() > 1e-9));
    }

    #[test]
    fn box_lattice_total_volume_matches_box() {
        // The six tets of a unit cube must sum (in magnitude) to the cube volume.
        let def = SoftBodyDef::box_lattice(Vec3::ZERO, Vec3::splat(0.5), [2, 2, 2], 1.0);
        let total: f32 = def.tets.iter().map(|(_, v)| v.abs()).sum();
        assert!((total - 1.0).abs() < 1e-5, "summed tet volume {total} != 1");
    }

    #[test]
    fn lattice_indices_are_in_range() {
        let def = SoftBodyDef::box_lattice(Vec3::ZERO, Vec3::splat(1.0), [3, 2, 4], 5.0);
        let n = def.particle_count() as u32;
        assert!(def.edges.iter().all(|&(a, b, _)| a < n && b < n && a != b));
        assert!(def.tets.iter().all(|(t, _)| t.iter().all(|&v| v < n)));
        assert!(def.surface.iter().all(|t| t.iter().all(|&v| v < n)));
    }

    #[test]
    fn ball_keeps_only_interior_cells_and_is_lighter_than_its_box() {
        let ball = SoftBodyDef::ball(Vec3::ZERO, 1.0, 6, 1.0);
        let boxed = SoftBodyDef::box_lattice(Vec3::ZERO, Vec3::splat(1.0), [6; 3], 1.0);
        // Carving the corners off leaves strictly fewer tets than the full box.
        assert!(ball.tets.len() < boxed.tets.len());
        assert!(!ball.surface.is_empty());
        // Mass is distributed only over used nodes, so each used node's inverse
        // mass is finite and positive.
        assert!(ball.inv_masses.iter().any(|&w| w > 0.0));
    }

    #[test]
    fn cloth_grid_constraint_topology() {
        let cloth = ClothDef::grid(4, 3, 1.0, |c, r| Vec3::new(c as f32, 0.0, r as f32), |_, _| false);
        assert_eq!(cloth.particle_count(), 12);
        // Structural: horizontal (cols-1)*rows + vertical cols*(rows-1).
        assert_eq!(cloth.structural.len(), 3 * 3 + 4 * 2);
        // Shear: 2 per interior quad.
        assert_eq!(cloth.shear.len(), 2 * 3 * 2);
        // Bending: (cols-2)*rows + cols*(rows-2).
        assert_eq!(cloth.bending.len(), 2 * 3 + 4 * 1);
        // Rest lengths come from the placed positions (unit spacing here).
        assert!(cloth.structural.iter().all(|&(_, _, l)| (l - 1.0).abs() < 1e-6));
    }

    #[test]
    fn cloth_pins_get_zero_inverse_mass() {
        let cloth = ClothDef::grid(3, 3, 9.0, |c, r| Vec3::new(c as f32, 0.0, r as f32), |c, r| {
            r == 0 && (c == 0 || c == 2)
        });
        assert_eq!(cloth.inv_masses[0], 0.0); // (0,0) pinned
        assert_eq!(cloth.inv_masses[2], 0.0); // (2,0) pinned
        assert!(cloth.inv_masses[4] > 0.0); // (1,1) free
    }

    #[test]
    fn cloth_indices_cover_every_quad() {
        let cloth = Cloth { base: 0, cols: 4, rows: 3 };
        // (cols-1)(rows-1) quads × 2 triangles × 3 indices.
        assert_eq!(cloth.indices().len(), 3 * 2 * 2 * 3);
        let n = (4 * 3) as u32;
        assert!(cloth.indices().iter().all(|&i| i < n));
    }
}
