# Elderforge Engine — CLAUDE.md

## What this is
Elderforge is a physics-first game engine written in Rust. The primary goal
is deep, accurate physics simulation — rigid bodies, soft bodies, fluids,
GPU-accelerated particles — with a renderer built specifically to visualize
simulation state in real time. This is a simulation engine that can run
games, not a general game engine that happens to have physics.

---

## Workspace layout

```
Elderforge/
  Cargo.toml          (workspace root)
  CLAUDE.md
  crates/
    core/             math (glam), handles, logging, profiling
    platform/         winit window, raw input, file I/O abstraction
    renderer/         wgpu render pipeline, PBR, physics debug viz
    physics/          XPBD solver, broadphase, narrowphase, bodies
    ecs/              hecs wrapper + all component definitions
    scene/            scene graph, serialization, asset loading
    editor/           egui editor, sim controls, inspector panel
    elderforge/       main binary — wires everything together
```

---

## Dependency versions (from workspace Cargo.toml)

- wgpu 22 — GPU backend (Metal/Vulkan/DX12/WebGPU)
- winit 0.30 — windowing and input
- glam 0.29 — all math (Vec3, Mat4, Quat, etc.)
- hecs 0.10 — ECS
- egui 0.29 + egui-wgpu + egui-winit — editor UI
- bytemuck — safe GPU buffer casting
- thiserror + anyhow — error handling
- log + env_logger — logging
- pollster — blocking async executor for wgpu init

---

## Coding conventions

- No `unwrap()` in library code — always `Result<T, E>` with `thiserror`
- No `todo!()` left in committed code — use a `// TODO:` comment instead
- All GPU resources go through the `ResourceCache` in the renderer crate
- `PhysicsBody` is an hecs component; it stores a handle into the physics
  world, not inline data
- WGSL shaders live in a `shaders/` folder inside the crate that uses them
- Every public type in the physics crate gets a unit test
- Every cross-crate integration gets a test in the scene or elderforge crate

---

## Physics architecture

```
Solver:      XPBD (Extended Position-Based Dynamics)
Broadphase:  BVH tree with incremental AABB updates
Narrowphase: GJK + EPA for convex shapes; SAT for polyhedra
Timestep:    Fixed at 120Hz with configurable substeps
Debug viz:   Collision shapes, constraint anchors, velocity vectors,
             angular momentum, sleep state — all renderable as overlays
             toggled at runtime from the editor
```

### Physics crate internal layout

```
physics/src/
  lib.rs
  world.rs          PhysicsWorld — owns all bodies and the solver
  body.rs           RigidBody, BodyHandle
  soft.rs           Particle, SoftBody, Cloth + tet-lattice / grid builders
  solver/
    mod.rs
    xpbd.rs         XPBD constraint solver (main integration loop)
    constraints.rs  Distance, contact, joint constraint types
    soft.rs         Particle distance/volume + particle↔rigid contacts
  broadphase/
    mod.rs
    bvh.rs          BVH tree with incremental AABB updates
  narrowphase/
    mod.rs
    gjk.rs          GJK distance/collision algorithm
    epa.rs          EPA penetration depth
    sat.rs          SAT for polyhedra
  shapes/
    mod.rs
    sphere.rs
    box_.rs
    capsule.rs
    convex_hull.rs
    trimesh.rs
  material.rs       PhysicsMaterial (friction, restitution, density)
  query.rs          Ray casts, shape casts, point queries
  fluid/
    mod.rs
    sph.rs          Smoothed Particle Hydrodynamics
  debug.rs          Debug draw data emitted each frame
```

---

## Renderer architecture

```
Backend:     wgpu (cross-platform — Metal on Mac, Vulkan on Linux/Win)
Pipeline:    PBR (physically-based rendering) with IBL lighting
Shadows:     Cascaded shadow maps
Debug layer: Separate render pass for physics visualization overlays
Shaders:     WGSL only — no GLSL, no HLSL
```

### Renderer crate internal layout

```
renderer/src/
  lib.rs
  context.rs        wgpu Device, Queue, Surface setup
  pipeline.rs       render pipeline builder
  cache.rs          ResourceCache — meshes, textures, materials by handle
  passes/
    pbr.rs          main PBR geometry pass
    shadow.rs       cascaded shadow map pass
    debug.rs        physics debug overlay pass
    ui.rs           egui render pass
  mesh.rs           GpuMesh, vertex/index buffer upload
  texture.rs        GpuTexture, sampler management
  material.rs       PbrMaterial (albedo, roughness, metallic, normal map)
  camera.rs         Camera, projection, view matrix
  shaders/
    pbr.wgsl
    shadow.wgsl
    debug.wgsl
```

---

## ECS design

```
Library:  hecs (minimal, fast, no macros required)
Pattern:  Systems are plain functions that take &mut World
          No global state — world is passed through explicitly
```

### Components (defined in ecs/src/components/)

```
Transform       position: Vec3, rotation: Quat, scale: Vec3
PhysicsBody     handle: BodyHandle (index into PhysicsWorld)
MeshRenderer    mesh: MeshHandle, material: MaterialHandle
Collider        shape: ColliderShape, material: PhysicsMaterial
Joint           body_a: BodyHandle, body_b: BodyHandle, kind: JointKind
Camera          fov, near, far, is_active
```

---

## Editor

```
Library:  egui rendered via egui-wgpu
```

### Panels

```
Scene hierarchy     entity tree, select/rename/delete entities
Component inspector select entity, view and edit all components live
Simulation controls play / pause / single-step / rewind
                    timestep multiplier slider (0.1x — 4x)
                    substep count control
Physics overlays    toggle per-layer: collision shapes, velocity vectors,
                    angular momentum, constraint anchors, sleep state,
                    broadphase AABB, contact normals
Profiler            frame time, physics step time, render time,
                    entity count, active body count
Asset browser       drag meshes/textures into the scene
```

---

## Platform crate

Thin abstraction over winit. Nothing outside the platform crate touches
winit types directly.

```
platform/src/
  lib.rs
  window.rs       WindowHandle, creation, resize events
  input.rs        InputState — keyboard, mouse, gamepad
  event.rs        EngineEvent enum (re-exports from winit, normalized)
  time.rs         Clock, delta time, fixed timestep accumulator
```

---

## Scene crate

```
scene/src/
  lib.rs
  scene.rs        Scene — owns hecs World + PhysicsWorld + asset handles
  loader.rs       load scene from .escene JSON format
  serializer.rs   serialize scene to .escene
  assets/
    mesh.rs       load .obj and .gltf meshes
    texture.rs    load PNG/JPEG/KTX textures
```

---

## Main binary (elderforge crate)

Wires everything together. Owns the event loop.

```
elderforge/src/
  main.rs
  app.rs          App struct — holds Scene, Renderer, Editor, Clock
  systems/
    physics.rs    run physics step, sync transforms back to ECS
    render.rs     collect draw calls, submit to renderer
    editor.rs     run egui frame, handle inspector edits
```

---

## Build and run

```bash
cargo check              # verify everything compiles
cargo build              # debug build
cargo build --release    # release build
cargo run                # launch the editor window
cargo test               # run all unit + integration tests
```

---

## Active work

[ Update this at the start and end of every session. ]

- Completed: Phase 0 — workspace bootstrap. All 8 crates created with
  module skeletons matching this file's layout; `cargo check` is clean
  and `cargo test` passes (31 tests: physics unit tests + scene
  integration tests). XPBD solver integrates gravity only; broadphase,
  narrowphase, queries, fluids, renderer passes, and the editor panels
  are stubs with TODOs at the implementation points.
- Completed: core crate foundations. `HandleAllocator<T>` (generational
  alloc/free/validate, stale handles fail validation), `TimingScope`
  profiler with thread-local span collector + aggregated table output
  (`profiling::report`), `init_logging()` (re-exported at crate root),
  and `FixedTimestep` accumulator in `core/src/time.rs` (clamps to
  max_steps_per_frame and drops backlog — no death spirals). 15 core
  unit tests; full workspace suite at 46 tests, all green.
- Completed: platform windowing layer (winit 0.30). `WindowConfig`
  (title/size/resizable/vsync), `WindowHandle` owning an `Arc<Window>`
  (Arc so the renderer can share it for surface creation), `InputState`
  with per-frame deltas, `EngineEvent` normalization, and
  `run_event_loop(config, closure)` built on `ApplicationHandler` —
  the closure runs once per frame with `&mut InputState`, the frame's
  `&[EngineEvent]`, and `&WindowHandle`, and returns
  `FrameControl::{Continue, Exit}`. `cargo run` opens the editor
  window; `cargo run -- --smoke-test` exits clean after 30 frames and
  is exercised by `crates/elderforge/tests/window_smoke.rs` (spawns
  the binary as a subprocess — winit needs the process main thread,
  so it can't run in-process under `#[test]`; needs a GUI session).
  Hard rule holds: no crate outside platform imports winit.
- Completed: renderer wgpu context + bootstrap triangle. `RenderContext`
  (Instance/Adapter/Device/Queue/Surface/SurfaceConfiguration) created
  from `WindowHandle::surface_provider()`, async init blocked on via
  pollster, sRGB surface format preferred. `resize(w, h)` reconfigures
  (ignores zero-size/minimized); `frame() -> FrameContext` acquires the
  surface texture + view + encoder, retrying once on Lost/Outdated;
  `present()` submits and presents. `GpuMesh::upload(device, label,
  &[Vertex], &[u32])` for vertex/index buffers (u32 indices engine-wide).
  `UnlitPass` (passes/unlit.rs + shaders/unlit.wgsl) clears to the dark
  bg and draws geometry with the vertex normal slot reinterpreted as RGB
  color — no camera/lighting, just to prove surface -> pipeline -> draw.
  Wired through `App::init_renderer` (lazy, on first frame once the
  window exists) and `systems::render::run`; `cargo run` opens the
  window and shows the RGB triangle. Verified on this Mac (Metal):
  `tests/triangle_readback.rs` renders offscreen and reads pixels back
  (passes), `cargo run -- --smoke-test` runs 30 frames and exits clean.
  Full workspace suite green (54 tests).
- Completed: minimal rigid-body simulation in the physics crate.
  `RigidBody` gained `mass`, `inv_inertia_tensor` (Mat3), and a minimal
  `Collider` (`Sphere`/`HalfSpace`), with `dynamic`/`fixed` constructors
  (solid-sphere inertia ⅖mr²) and `kinetic_energy()`. `BodyHandle` is now
  `Handle<RigidBody>` from core (see decisions log). `PhysicsWorld::step`
  is integrate -> broadphase -> narrowphase -> resolve: semi-implicit
  Euler (now also integrates orientation from angular velocity in
  `XpbdSolver`), `broadphase::naive_pairs` O(n²) AABB pairs,
  `narrowphase::sphere_sphere`/`sphere_halfspace` contacts, and
  `solver::impulse::resolve_contact` (frictionless linear normal impulse
  + inverse-mass-split positional correction; restitution combined via
  min). Linear-only impulse is exact for spheres/half-spaces (lever arm ∥
  normal -> zero torque), so elastic hits conserve energy to fp epsilon.
  Scenario tests in `crates/physics/tests/rigid_body_sim.rs`: a ball
  settles on a static ground plane at y=radius with ~zero velocity, and
  an e=1 head-on pair conserves KE within 1e-3, and a restitution-0.8 ball
  rebounds then loses energy. Physics crate at 46 lib + 3 integration
  tests; full workspace green.
- Completed: ECS + binary scene loop (phase 5). The ECS crate already had
  the six components (Transform/PhysicsBody/MeshRenderer/Collider/Joint/
  Camera) and the hecs `World`/`Entity` re-export from the skeleton —
  verified complete, no changes needed. Renderer gained a camera+depth
  forward path: `primitives::{cube,plane}`, `ForwardPass` (group 0 camera
  view-proj uniform, group 1 per-object model matrix via dynamic offset,
  Depth32Float target, simple directional shade in `shaders/forward.wgsl`),
  and `PipelineBuilder::build` now takes bind-group layouts. The binary
  loop (`app.rs` + `systems/`) steps physics via core `FixedTimestep`
  (120 Hz, ≤8 steps/frame), `systems::physics::run` syncs each body pose
  into its `Transform`, and `systems::render::run` draws one call per
  `(Transform, MeshRenderer)` through the active `Camera` entity. `App`
  spawns a fixed camera, a ground plane, and 50 cubes (sphere bodies,
  restitution 0.6) at random heights via a tiny xorshift RNG. Verified on
  this Mac (Metal): headless `crates/elderforge/tests/scene_render.rs`
  renders the scene offscreen (green ground + 5×5 cube grid, 58% lit) and
  `cargo run -- --smoke-test` builds the 52-entity scene and exits clean.
  Added wgpu+pollster as elderforge dev-deps for the offscreen test.
- Completed: BVH broadphase (phase 6). `broadphase/bvh.rs` rebuilt as a
  real binary BVH: `BvhNode { aabb, parent, kind: Internal{left,right} |
  Leaf{body} }`, top-down **binned SAH** construction, incremental
  `refit` (cheap ancestor refit while a body stays in its leaf's expanded
  box; otherwise rebuilds the lowest enclosing subtree, keeping depth ~
  log2(n)), `query_pairs`, and `debug_iter_aabbs`. `PhysicsWorld` now
  builds a BVH each substep over finite-AABB bodies (half-spaces, infinite
  AABB, paired separately) instead of `naive_pairs` (kept as the test
  oracle). Tests (`tests/bvh.rs`): 1000-AABB query == brute force; 10k
  bodies BVH 21ms vs brute 1.24s; tree stays within 2·log2(n) deep over
  100 frames of moving half the bodies.
- Completed: GJK + EPA narrowphase. `narrowphase/convex.rs` adds a
  `ConvexShape` trait (`support` + rounding `margin`) implemented for
  Sphere/Box/Capsule/ConvexHull, plus `Pose` and `AnyShape`. `gjk.rs` is a
  distance GJK (Voronoi sub-simplex solver, degenerate-tetra guard for
  planar Minkowski sets); `epa.rs` expands the polytope (exhausted-face
  skipping for the box degeneracy). `collide(a, pose_a, b, pose_b) ->
  Option<ContactManifold{contact_point, normal, depth}>` runs GJK on the
  cores then folds margins back in (exact for rounded shapes). Tests
  (`tests/narrowphase.rs`): box-box edge, sphere-box face, capsule-capsule
  angled, boxes barely overlapping/separated — all vs analytic answers.
- Completed: XPBD solver (replaces the impulse bring-up). `solver/xpbd.rs`
  has a `Constraint` trait (`project` + `reset`), `DistanceConstraint`
  (rest length + compliance) and `ContactConstraint` (built from a
  `ContactManifold` each substep, re-evaluated so multiple iterations
  don't over-correct, + a velocity-level restitution pass).
  `PhysicsWorld::step` is now the XPBD substep loop (predict → broadphase +
  narrowphase contacts → project → derive velocity → restitution),
  `substeps` (default 20) and `iterations` (default 4) configurable;
  `add_distance_constraint(a, b, rest, compliance)` for ropes/joints.
  Bodies gained `prev_position` and a `Box` collider. Tests
  (`tests/xpbd.rs`): stiff 10-link rope converges in one 20-substep frame;
  10-box stack stable 600 frames (no drift/popping); pendulum period 0.34%
  off analytic. Full workspace green (100 tests); binary still falls/
  settles the 50-body scene under XPBD.
- Completed: three demo scenes in the elderforge crate, selectable with
  `cargo run -- --demo <stacking|pendulum|avalanche>` (defaults to stacking;
  unknown names error with the valid list). Split the elderforge crate into a
  lib (`src/lib.rs` -> `pub mod demos`) + bin so both the binary and the
  headless tests build the identical scenes; `App::new(demo)` now calls
  `Demo::setup(scene, &DemoAssets)` instead of the old hard-coded 50-cube
  `spawn_scene`. Added a `primitives::sphere(radius, sectors, stacks)` UV
  sphere (smooth outward normals) to the renderer, uploaded alongside cube +
  plane into `DemoAssets`. Demos: **stacking** — 20 unit boxes stacked with a
  small gap, matte (restitution 0), settling into a stable axis-aligned tower
  (the solver's best case, no jitter); **pendulum** — a fixed anchor + 10
  spheres on rigid (compliance-0) distance constraints, released horizontal so
  it swings as a multi-link rope, over a ground plane for depth; **avalanche**
  — 200 spheres dropped above a tilted half-space ramp (downhill +X) that feeds
  onto a flat floor, boxed in by invisible static half-space walls so they pile
  against the far wall (substeps lowered to 10 for throughput, restitution 0.1
  to bleed energy). Verified: `tests/demos_render.rs` builds each demo through
  the real `Demo::setup`, steps 90 frames (asserting positions stay finite),
  renders offscreen + reads back (stacking 46%, pendulum 49%, avalanche 64%
  lit; PPMs dumped to `$TMPDIR/elderforge_demo_<name>.ppm`), plus each runs
  clean under `cargo run -- --demo <name> --smoke-test` (22 / 13 / 203
  entities). Full workspace suite green.
- Completed: production joints, contact friction, sleeping, and a split
  friction material. `solver/joints.rs` adds the full XPBD rigid-body
  primitives — world-space inverse inertia, a positional solve with lever-arm
  generalized mass `w = 1/m + (r×n)ᵀI⁻¹(r×n)`, an angular solve, and an
  angle-limit helper (Müller et al. 2020) — and four joints on top:
  `BallJoint` (point-to-point), `HingeJoint` (axis alignment + optional swing
  limit), `PrismaticJoint` (slide axis, perpendicular + orientation lock +
  travel limit), `FixedJoint` (weld). The world exposes
  `add_{ball,hinge,prismatic,fixed}_joint`, projects joints inside the substep
  loop, and now derives **angular** velocity from the orientation delta (it
  only derived linear before). `Collider::Box` gained a real solid-cuboid
  inverse inertia tensor (was zero) so jointed boxes actually respond to torque
  — safe for the existing linear-only contact path, which applies no torque.
  `ContactConstraint` gained Coulomb friction: position-level **static**
  friction that cancels tangential sliding while it stays inside the cone
  `λ_t ≤ μ_s·λ_n`, plus a velocity-level **dynamic** friction pass (μ_d).
  `PhysicsMaterial` split `friction` into `static_friction`/`dynamic_friction`
  and grew `combine()` → `CombinedMaterial` (geometric-mean friction, max
  restitution), used by `make_contact`. **Sleeping** is island-based: a body
  accrues quiet frames when its linear+angular speed stays under the world
  thresholds, and an island (union-find over contacts/joints/distance
  constraints) sleeps only when every member is ready, so a stack never
  half-sleeps; asleep bodies skip integration, contact with an awake body wakes
  them, and `generate_contacts` short-circuits to zero broadphase/narrowphase
  cost when nothing is awake (`awake_body_count()` / `last_narrowphase_tests()`
  expose this). Removed bodies now use a dedicated `removed` tombstone instead
  of overloading `sleeping`. Tests: `tests/joints.rs` (5 — each joint's
  invariant + free DOF), `tests/friction.rs` (box on a slope holds below and
  slides above the friction angle; transition tracks arctan μ), `tests/
  sleeping.rs` (settled 5-box stack → 0 narrowphase tests; impact wakes it).
  Physics at 62 lib tests; full workspace green.
- Completed: live egui editor rendered through egui-wgpu, wired into the
  binary. New `editor::state::EditorState` owns the three egui pieces —
  `egui::Context`, `egui_winit::State`, `egui_wgpu::Renderer` — plus the panel
  `Editor`, with `integrate_event` (winit event → egui input), `run_frame`
  (lays out the panels over the `Scene`, tessellates → `EditorFrame` paint
  jobs), and `paint` (uploads textures/buffers, records a `LoadOp::Load` pass
  over the finished 3D frame; uses `RenderPass::forget_lifetime` for
  egui-wgpu's `'static` pass). The editor crate gained egui-winit/egui-wgpu/wgpu
  deps; the binary dropped its direct egui deps (the glue moved here). Platform
  now forwards raw winit events to the frame closure (`&[RawWindowEvent]`, a
  re-export) and exposes `WindowHandle::winit_window()` so egui_winit can read
  the window — the one winit leak outside platform, for the egui bridge only.
  `App` creates the `EditorState` lazily with the GPU, and each frame:
  acquires one surface frame, runs the editor UI, steps physics **under the sim
  controls**, records the 3D pass then the egui pass into the same encoder, and
  presents. Panels: **Scene Hierarchy** (entities by id, click to select),
  **Inspector** (edit Transform position/scale with live `DragValue`s, mirrored
  into the entity's rigid body + wake so edits stick while simulating; rotation
  shown as axis-angle), **Simulation** (Play/Pause, Step, timestep multiplier
  0.1×–4×, substep slider seeded from the scene), **Stats** (frame time, physics
  step time, FPS, entity/body/awake counts). Pause stops physics stepping while
  rendering continues; Step advances exactly one fixed tick; the multiplier
  scales `FixedTimestep` input; the substep slider drives `physics.substeps`.
  `systems::render::run` became `record` (no acquire/present — the app owns the
  frame so egui can share it); the old `systems::editor` stub is gone. Verified
  on Metal: `--smoke-test` opens the window, paints 30 editor+3D frames, exits
  clean; `window_smoke`/`scene_render`/`demos_render` all green.
- Completed: two new demos + a capsule body collider + a launch-resolution
  flag (phase 11). The world's fast-path `body::Collider` gained a `Capsule
  { radius, half_height }` variant (Y-aligned), wired through `aabb`
  (conservative bounding-sphere, rotation-safe), `inv_inertia_for` (solid-
  cylinder approximation — inert for the linear-only contact path and unused
  by any current joint, so only its symmetry/positivity matter), and
  `world::as_convex` -> `AnyShape::Capsule` so it collides via the existing
  GJK/EPA path (the narrowphase `Capsule` core already existed). Renderer
  gained `primitives::capsule(radius, half_height, sectors, cap_stacks)` — a
  cylinder + two hemisphere caps whose equator normals are horizontal (so the
  cap/wall seam shades continuously); `DemoAssets` carries the new mesh,
  baked at `CAPSULE_BASE_{RADIUS,HALF_HEIGHT}` (0.3 / 0.5) so a capsule body
  rendered at uniform scale `s` pairs with `Collider::Capsule { radius: base*s,
  .. }` and mesh+collider stay in lockstep. New demos (now 5 total, all via
  `cargo run --release -- --demo <name>`): **sandbox** — ground + 5 cubes a
  short drop above rest, an editor showcase (small legible hierarchy, live
  Inspector transform edits, obvious Play/Pause/Step); **stress** — 500 mixed
  spheres/boxes/capsules poured into a walled square pit, substeps lowered to
  8 for throughput, the Stats panel the headline (frame/physics time climb as
  the cloud lands, ease off as islands sleep); boxes/capsules get a fixed
  random orientation with zero angular velocity (contacts are linear-only, so
  initial spin would never decay). The editor is already always-on for every
  demo, so the three existing demos already run with it visible — unchanged.
  Binary gained `--resolution <W>x<H>` (default 1920x1080, case-insensitive
  `x`, errors on malformed/zero) feeding `WindowConfig`. Unit tests: capsule
  collider aabb + axially-symmetric inertia (physics, now 64 lib tests),
  `capsule_is_well_formed` (renderer); `demos_render` updated to build the
  capsule asset and now exercises all 5 demos (sandbox 66% / stress 66% lit).
  Full workspace green; both new demos verified clean under
  `--demo <name> --smoke-test` (7 / 502 entities).
- Completed: asset pipeline + `.escene` scene serialization + editor save/load
  (phase 12). **Mesh loading** (`scene/src/assets/mesh.rs`): a hand-rolled OBJ
  parser (v/vn/vt + `f` in all four vertex forms, negative indices, polygon fan
  triangulation, smooth-normal recompute when a file omits normals) and a glTF
  importer via the `gltf` crate (concatenates every primitive of every mesh,
  offsetting index runs), both producing a `MeshData` (parallel positions/
  normals/uvs + triangle indices). **Texture loading** (`assets/texture.rs`):
  PNG/JPEG → RGBA8 `TextureData` via the `image` crate (KTX stays
  UnsupportedFormat — `image` can't decode it); `renderer::GpuTexture::from_pixels`
  uploads it as an sRGB 2D texture (the old TODO). **`.escene` format** (serde →
  JSON): leaf physics/ECS types now derive Serialize/Deserialize (glam `serde`
  feature enabled in core; `Handle<T>` gets manual `[index, generation]` impls),
  so the format crate (`scene/src/format.rs`) only adds the document structs +
  a `RigidBodyDoc` (immovable mass serialized as `None`, not JSON-illegal
  `INFINITY`; inverse mass/inertia recomputed on load). A `SceneAssets` table on
  `Scene` is the resource-handle authority: it maps each `MeshHandle`/`TextureHandle`/
  `MaterialHandle` to a stable `MeshSource`(`Builtin`/`File`)/`TextureSource`/
  `MaterialDef`, deduping by source, so handles survive a round-trip and the same
  path loads once. `serializer::save_scene` / `loader::load_scene` write/read the
  whole scene (name, world config, all bodies, asset table, every entity's
  components). **Editor**: a top `Toolbar` panel (path field + Save/Load buttons)
  records requests; the app's `handle_scene_io` services them — save writes
  directly, load parses + rebuilds the GPU cache from the new scene's asset table
  before swapping it in. **App asset realization** (`elderforge::assets::AssetManager`,
  now in the lib): builds a `ResourceCache` from a scene's asset table, inserting
  each resource at the handle the scene assigned (`ResourceCache::insert_*_at`),
  builtins regenerated from `primitives::*` and files loaded (CPU decode memoized
  by path). `App::init` now registers builtins in `scene.assets` then realizes,
  so demos serialize losslessly. Tests: OBJ/texture/registry unit tests; a
  20-entity mixed-component `roundtrip` test (save → reload → exact match of
  name/world/bodies/assets + per-component counts + handle resolution); and a
  GPU `scene_io` test (realize → save → load → re-realize, all handles resolve).
  Full workspace green (133 tests).
- Completed: soft bodies + cloth, both XPBD-native (phase 13). New `physics/
  src/soft.rs` adds `Particle` (position/prev/velocity/inv_mass + a collision
  radius — the shared DOF for both), `SoftBodyDef` (a tet-lattice builder:
  `box_lattice`/`ball` clip a regular grid and split each cell into six tets via
  **Kuhn's decomposition** so shared faces always match, dedupe edges, and
  extract the outward boundary surface for rendering), and `ClothDef::grid`
  (structural/shear/bending distance constraints over a 2D grid, two top corners
  pinnable). `physics/src/solver/soft.rs` adds the XPBD particle constraints —
  `ParticleDistance` (cloth springs + soft-body edges), `ParticleVolume`
  (per-tet volume preservation, the anti-collapse constraint), and
  `ParticleBodyContact` (particle vs sphere/box/capsule/half-space, two-way
  **linear-only** coupling + Coulomb friction, matching the rigid contact
  convention). `PhysicsWorld` gained a flat `particles` array (soft bodies /
  cloths own contiguous runs via `base`/`count`), `add_soft_body`/`add_cloth`,
  particle integration interleaved into the substep loop (predict → particle
  contacts → project distance/volume/contacts → derive velocity + viscous
  `particle_damping`), and a `wind` acceleration field (a flat flag at its
  natural width hangs taut, so wind is what makes it billow). Renderer:
  `GpuMesh::upload_dynamic`/`update_vertices` (COPY_DST vertex buffer streamed
  each frame) and a two-sided `forward.wgsl` (flips the normal on back faces so
  cloth is lit from both sides). Elderforge: `deformable.rs` (`DeformableMeshes`
  builds/updates one dynamic mesh per soft body + cloth from particle positions,
  recomputing smoothed normals, drawn in world space alongside the ECS meshes),
  wired through `App`/`systems::render::record`. Three demos (`--demo
  soft_ball | cloth_flag | cloth_drape`): a soft ball squashing on a table, a
  flag billowing in wind from two pinned corners, and a cloth draping over a
  spinning cube (friction drags the fabric around). Tests: physics `tests/soft.rs`
  (flag billows + doesn't stretch, ball preserves volume on impact, soft body
  shoves a dynamic body) plus soft/cloth unit tests; `demos_render` now exercises
  all 8 demos with deformables and asserts particle finiteness; all three new
  demos verified clean under `--smoke-test`. Full workspace green (151 tests).
- Completed: demo-capture tooling (phase 14). Two launch flags + four
  footage-grade demos + a configurable key light. **`--borderless`**: hides all
  editor chrome and clears to pure black, rendering just the viewport. The app
  creates no `EditorState` in this mode (`editor: None`), so `update` runs a
  unified path that plays at the scene's own settings (no sim controls), skips
  the egui pass, and `ForwardPass::set_clear_color(BLACK)`; the binary also
  drops OS window decorations via a new `WindowConfig.decorations` (winit
  `with_decorations`). **`--msaa <N>`** (1/2/4/8, validated): `PipelineBuilder`
  gained `.sample_count()` and `ForwardPass::new` a `sample_count` arg — it
  renders into a multisampled color+depth target and resolves into the surface
  view (egui then paints over the resolved single-sample view). Unsupported
  counts don't crash: `RenderContext::supported_sample_count` queries the
  adapter's `MULTISAMPLE_X{2,4,8,16}` flags (color ∩ depth) and the app clamps
  down (e.g. 8×→4× on this Metal surface) with a warning. **Per-demo runtime
  config**: `Demo::setup` now returns a `DemoConfig { anim: DemoAnim, light:
  Option<DirectionalLight> }` (demos needing neither return `()`, lifted via
  `From<()>`). `DemoAnim` is applied each frame by the app against an
  accumulated `sim_time`: `OrbitCamera` rewrites the active camera's `Transform`
  on a circle; `StagedDrop` restores saved particle inverse masses at a release
  time (needs the new `PhysicsWorld::particles_mut`). The **light** is a new
  `forward.wgsl` uniform — `Globals` grew `light_dir`/`light_color` vec4s
  (uniform now 96 B, visibility VERTEX_FRAGMENT); the default
  (`DirectionalLight::default`, dir (0.3,0.9,0.35), white) reproduces the old
  hard-coded look exactly, so only demos that override it change. The four new
  demos (`--demo <name>`, hyphenated canonical names; all default to 1920×1080):
  **cloth-drape** (a 40×40 sheet pinned at two top corners draping over a slowly
  Y-spinning cube, warm key light from upper-left, camera orbiting once per
  30 s) — note this is distinct from the older underscore `cloth_drape`;
  **softbody-drop** (three soft balls of increasing compliance frozen in air
  then released onto a table at 0/2/4 s — the squashiest pancakes; fixed angled
  camera); **cloth-tear** (a curtain pinned along its whole top edge taking a
  heavy 50 kg sphere on its center — tearing isn't implemented, so it shows
  extreme stretch via compliant structural springs); **mixed** (a soft ball
  launched down a slick ramp scattering a 5-box stack, beside a wind-blown cloth
  flag). `demos_render` now exercises all 12 demos (the 4 new ones 56–95% lit);
  full workspace suite green, and all four verified clean under
  `--demo <name> --borderless --msaa 4 --smoke-test` at 1920×1080.
- Completed: physics debug overlay pass (phase 15). A toggle-able, layered
  visualization of physics state rendered on top of the 3D scene. **Physics
  side** (`physics/src/debug.rs`): `DebugDraw { lines: Vec<DebugLine>, points:
  Vec<DebugPoint> }` (cleared/refilled in place each frame — capacity reused, no
  steady-state alloc) with geometry builders on it (`wire_box`/`wire_aabb`/
  `wire_sphere`/`wire_capsule`/`arrow`/`arc`/`marker`), and `DebugLayers` (eight
  bools, mirrors the editor toggles). `PhysicsWorld::emit_debug(layers, &mut
  out)` fills only the enabled layers: **collision shapes** (collider wireframe
  colored by `BodyKind`), **velocity vectors** (arrow length ∝ speed),
  **angular velocity** (an arc around the spin axis), **contact points** (a
  marker sphere + center point + normal arrow — recomputed from a fresh
  broadphase/narrowphase so it's correct even while the scene sleeps, ignoring
  the solver's sleep short-circuit), **constraint anchors** (cube markers +
  connection lines for distance constraints and all four joint types, via the
  new `Joint::world_anchors`), **BVH AABBs** (every node, colored by depth via
  the new `Bvh::debug_iter_levels` → red root → blue leaves), **sleep state**
  (dynamic-body wireframe, sleeping bodies dimmed/low-alpha), and **force
  accumulators** (arrow of net external force m·g from the CoM). Contacts/BVH
  share one finite-body broadphase build, gated on either layer being on.
  **Renderer side** (`renderer/src/passes/debug.rs`): `DebugPass` with two
  pipelines — a **line-list** and a **point-list** — sharing `debug.wgsl` and a
  camera uniform; a `DebugVertex { position, color }` (`Pod`); and a `GrowBuffer`
  that **reuses** its GPU vertex buffer across frames, growing only when a frame
  exceeds capacity. It renders single-sampled `LoadOp::Load` over the resolved
  surface (no MSAA/no depth — overlays sit on top), so it composes after the
  forward pass regardless of the scene's sample count. **Bridge**
  (`elderforge::debug_overlay::DebugOverlay`, in the lib like `deformable`):
  owns the physics `DebugDraw` + reusable renderer-vertex lists, converting
  Vec3 lines/points → flat `DebugVertex` arrays each frame. **Wiring**: the
  editor `Overlays` panel now has the eight matching checkboxes (+ "Clear all");
  `App` maps them into `DebugLayers`, calls `DebugOverlay::update` after the
  physics step, and `systems::render::record` draws the overlay right after the
  forward pass under the same camera (borderless capture has no editor → all
  layers off → empty/cheap). Tests: physics `debug.rs` unit tests (6) +
  `tests/debug_overlay.rs` (8, one per-layer + clear/reuse), and a headless GPU
  `tests/debug_render.rs` that emits every layer and reads back the overlay over
  black (4.6k lit px; dumps `$TMPDIR/elderforge_debug_overlay.ppm`). Full
  workspace suite green; verified live in windowed and borderless+MSAA runs.
- Next: angular contact response (contacts are still linear-only — fine for
  centered/axis-aligned cases, but a box can't yet tip over a contact edge or
  pick up spin from an off-center hit), persistent BVH refit inside the world
  (currently rebuilt per substep), and the real PBR render pass.

---

## Decisions log

[ Record major architecture decisions here as they are made. ]

2026-06-11 — Crate packages are named `elderforge-*` (e.g. crates/core is
`elderforge-core`) because `core` collides with Rust's built-in crate.
Directory names stay as listed in the workspace layout.

2026-06-11 — Only elderforge-core depends on glam. All other crates
import math types via `elderforge_core::math` re-exports, so the math
backend is swappable in one place and versions can't drift.

2026-06-11 — Generic generational `Handle<T>` lives in core
(MeshHandle/TextureHandle/MaterialHandle); the physics crate keeps its
own plain `BodyHandle` since PhysicsWorld owns body storage. Scene/asset
serialization deliberately has no serde dependency yet — loader and
serializer are hand-rolled stubs until the .escene format is designed.

2026-06-14 — Supersedes the BodyHandle half of the 2026-06-11 entry:
`BodyHandle` is now `pub type BodyHandle = Handle<RigidBody>` (core's
generic handle), not a bespoke struct. The world still owns body storage
and tracks generations itself (append-only + bump-on-remove); only the
handle *type* moved to core, so body handles can't be confused with
mesh/texture/material handles. Existing `BodyHandle` users (ECS
`PhysicsBody`/`Joint`, query, bvh, constraints) were unaffected —
construction switched from `BodyHandle { .. }` to `BodyHandle::new(..)`.

2026-06-14 — The first rigid-body pipeline is an impulse/velocity bring-up
(semi-implicit Euler + `solver::impulse`), NOT XPBD. XPBD stays the target
solver (per the architecture section); the impulse path exists so collision
response works before XPBD contacts land, and is expected to be replaced.
For the same reason `body::Collider` (`Sphere`/`HalfSpace`) is a minimal
fast-path shape set kept separate from the full `shapes::ColliderShape`
(GJK/EPA) enum — don't merge them; the half-space has no GJK support.

2026-06-16 — Supersedes the solver half of the 2026-06-14 entry: the world
solver is now XPBD (`PhysicsWorld::step` substep loop + `solver::xpbd`
constraints), as targeted. `solver::impulse` stays as a tested module but
is no longer called by the world. `body::Collider` grew a `Box` variant and
is mapped to the GJK `ConvexShape`s via `narrowphase::AnyShape`; half-spaces
keep their dedicated contact generator (`world::halfspace_contact`) since
they're unbounded and can't go through GJK. XPBD contacts are linear-only
(no angular term) — exact for centered/axis-aligned contacts, which is why
the box-stack test uses axis-aligned cubes.

2026-06-17 — The elderforge crate now has BOTH a lib (`src/lib.rs`) and a bin
(`src/main.rs`). Demo scene definitions live in the lib (`elderforge::demos`)
so the binary and the headless render tests construct byte-identical scenes
from one source; the event loop, `App`, and per-frame `systems` stay bin-only.
Demos are selected at runtime via `--demo <name>` (one binary dispatching to
scene setups), NOT separate `[[bin]]` targets — `cargo run -- --demo stacking`
is the intended invocation. `DemoAssets` carries only renderer handles (cube /
sphere / plane meshes + material); the caller uploads the meshes (it has the
GPU device) and each demo picks what it needs.

2026-06-16 — GJK/EPA run on shape *cores* with a separate rounding `margin`
(sphere = point + r, capsule = segment + r, box/hull = exact polytope,
margin 0). Collision distance/penetration is computed on the cores, then
the margins are folded back in. This keeps sphere/capsule contacts
analytically exact (no EPA on curved surfaces) and limits EPA to genuine
polytopes. EPA reconstructs its own origin-enclosing tetrahedron and skips
faces it can't expand, to survive the box-vs-box degeneracy (Minkowski
difference of two boxes is a box, often leaving the origin on a face).

2026-06-18 — Joints (`solver/joints.rs`) use the FULL XPBD rigid-body
machinery — anchor lever arms + world-space inverse inertia, so they apply
torque and constrain orientation — even though world *contacts* stay
linear-only (per the 2026-06-16 entry). The two paths are deliberately
asymmetric: contacts are exact for centered/axis-aligned cases and angular
contact response is still future work, but joints would be meaningless
without it. This is why `Collider::Box` now carries a real inverse inertia
tensor (was `Mat3::ZERO`): joints need it, and it's inert for linear contacts
(which never apply torque), so the box-stack/avalanche behavior is unchanged.
Joints are stored as a non-`dyn` `Joint` enum (Ball/Hinge/Prismatic/Fixed) in
`PhysicsWorld::joints` and projected in the substep loop alongside distance
and contact constraints. The substep now also derives angular velocity from
the orientation delta (`2·imag(q·q_prevᵀ)/dt`); it derived only linear before.

2026-06-18 — Sleeping is ISLAND-based, not per-body. A union-find over the
substep's contact pairs plus joints and distance constraints groups dynamic
bodies; an island sleeps only when its least-rested member has been quiet for
`sleep_frames` frames, and any restless member keeps (or wakes) the whole
island. Per-body sleeping was rejected because a stack would flicker — the
last awake box perpetually re-waking the one beneath it. `generate_contacts`
short-circuits to zero work when no dynamic body is awake (the cost win), and
includes sleeping bodies in the broadphase only while something *is* awake, so
an impact can find and wake them. `RigidBody::removed` is a separate tombstone
for `remove_rigid_body` (it used to overload `sleeping`, which now conflicts
with real sleeping — a removed body must never be woken by a contact).

2026-06-18 — `PhysicsMaterial.friction` split into `static_friction` /
`dynamic_friction` (Coulomb's two regimes). Contact friction is the paper's
two-level scheme: position-level static friction fully cancels tangential
slide while inside the cone `λ_t ≤ μ_s·λ_n` (all-or-nothing, not clamped, so
it cleanly hands off), and a velocity-level pass applies dynamic friction μ_d
to genuinely sliding contacts. Pair coefficients come from
`PhysicsMaterial::combine` → `CombinedMaterial`: friction by geometric mean,
restitution by max (this changes the old `restitution.min` combine in
`make_contact`, but no test pinned it and equal-restitution scenarios are
unaffected).

2026-06-19 — The egui integration (`EditorState`) lives in the EDITOR crate,
which therefore depends on egui-winit and egui-wgpu. This is the one
deliberate exception to "no winit outside platform": egui_winit IS the
winit↔egui input bridge, so the editor reaches winit *only* through it
(`egui_winit::winit`), never the `winit` crate directly. To feed it, platform
gained `WindowHandle::winit_window()` and forwards raw events to the frame
closure as `&[RawWindowEvent]` (a re-export of `winit::event::WindowEvent`), so
the binary wires egui without naming winit itself. The 3D pass and the egui
pass share ONE surface frame/encoder: `App::update` acquires the frame, calls
`systems::render::record` (which no longer presents — that's why it was renamed
from `run`), then `EditorState::paint` (a `LoadOp::Load` pass over the 3D
output), then presents. The simulation controls truly gate `PhysicsWorld`
stepping — `App` reads `playing`/`single_step`/`timestep_multiplier`/`substeps`
each frame; pause skips the step loop entirely (render still runs), and the
multiplier scales the `FixedTimestep` input. Inspector Transform edits are
mirrored back into the entity's rigid body (and wake it) so they're not
immediately overwritten by the solver.

2026-06-19 — `body::Collider` grew a `Capsule { radius, half_height }` variant
(Y-aligned) so the world can simulate capsules, not just sphere/box/half-space.
It maps straight to the pre-existing narrowphase `Capsule` core via
`world::as_convex` → `AnyShape::Capsule`, so capsules collide through the same
GJK/EPA path as boxes (no new contact code). Its `inv_inertia_for` arm is a
documented solid-cylinder *approximation* (caps folded in), deliberately not
exact: like the box tensor it is inert for the linear-only contact path (no
torque) and no current joint uses a capsule, so only finiteness/positivity/
axial symmetry matter. The AABB is the conservative bounding sphere
(`half_height + radius`), rotation-safe like the box's diagonal trick.

2026-06-19 — Demo capsule rendering relies on UNIFORM scale to keep mesh and
collider identical. The shared capsule mesh is baked once at
`CAPSULE_BASE_{RADIUS,HALF_HEIGHT}` (0.3 / 0.5); a capsule body is spawned with
`Transform` scale `splat(s)` and `Collider::Capsule { radius: base*s,
half_height: base*s }`, so the single scalar `s` scales the drawn mesh and the
collider in lockstep (uniform scaling preserves the spherical caps; non-uniform
would not). Boxes follow the existing convention (cube mesh half-extent 0.5,
scale `splat(half/0.5)`); spheres scale by radius. This is why the stress demo
uses one base capsule mesh for all 500 bodies rather than per-body meshes.

2026-06-19 — The elderforge binary defaults its launch window to 1920×1080
(via `--resolution <W>x<H>`), NOT `WindowConfig::default()` (which stays the
platform-general 1600×900). The binary parses the flag and overrides
`WindowConfig.{width,height}`; the platform default is left alone so the
windowing layer keeps a sensible standalone default. Demo selection
(`--demo`) and resolution (`--resolution`) are independent flags parsed the
same hand-rolled way (no clap dependency).

2026-06-19 — The `.escene` format derives serde DIRECTLY on the leaf physics
and ECS value types (PhysicsMaterial, body `Collider`, BodyKind, ColliderShape
+ shape structs, JointKind, and all six components) rather than mirroring them
with DTOs — single source of truth, far less boilerplate. To support this,
core enables glam's `serde` feature (so Vec3/Quat/Vec4 serialize) and gives
`Handle<T>` MANUAL serde impls serializing as `[index, generation]` (a derive
would wrongly demand `T: Serialize`, but the markers are uninhabited). physics
and ecs gained a `serde` dependency. The ONE type that does NOT serialize
directly is `RigidBody`: it carries `mass = INFINITY` for immovable bodies
(not representable in JSON — serde_json emits `null`) plus derived/runtime
fields, so it goes through `RigidBodyDoc` (immovable mass stored as `None`;
inverse mass/inertia and `prev_*` recomputed by the constructors on load). The
trade-off accepted: the file format is now coupled to internal type/field
names, so renaming a serialized field is a format change (caught at compile
time, which is the point).

2026-06-19 — `SceneAssets` (on `Scene`) is the single authority for resource
handles, NOT the renderer's `ResourceCache`. Registering a `MeshSource`/
`TextureSource`/`MaterialDef` returns a handle (deduped by source); the
serializer writes the table in handle-index order (so handles are implicit in
list position), and the app realizes the table into a fresh `ResourceCache` at
exactly those handles via `insert_*_at`. This is why a loaded scene's
`MeshRenderer` handles resolve with no remap: the cache is rebuilt to match the
scene, not the other way round. The scene crate stays GPU-free (it only names
assets); the app (`elderforge::assets::AssetManager`, in the LIB so tests can
reach it) owns decode + upload, memoizing CPU decode by path. Builtin
primitives are stored as `MeshSource::Builtin(name)` and regenerated from
`primitives::*` on realize — so demos serialize and reload losslessly without
baking primitive geometry into the file.

2026-06-19 — Editor Save/Load follows the same intent-recording pattern as the
sim controls: the `Toolbar` panel only sets `save_requested`/`load_requested`
(+ a path field), and the app's `handle_scene_io` consumes them each frame.
Loading can't live in the editor crate — it replaces the whole scene and must
rebuild the GPU cache through the renderer, which only the app owns — so the
editor hands the request up. On a successful load the app swaps the scene,
rebuilds the cache from its asset table, clears the (now-dangling) selection,
and reseeds the substep slider; failures are reported in the toolbar status
line and leave the running scene untouched.

2026-06-20 — Soft bodies and cloth share one flat `PhysicsWorld::particles`
array, NOT a per-body `Vec<Particle>`. A `Particle` (position/prev/velocity/
inv_mass + a collision radius) is the single DOF type; a `SoftBody`/`Cloth` is
just metadata (`base`/`count`, surface or grid topology) pointing into the
shared array, and constraints store absolute particle indices. This keeps the
substep loop a flat sweep over all particles/constraints (mirroring the rigid
path) instead of nested per-body loops, and lets one constraint list mix soft
and cloth. Trade-off: no per-soft-body removal yet (handles are plain indices,
no generations) — fine for the demos, which build once.

2026-06-20 — Soft-body tet meshes use Kuhn's six-tetrahedron cell decomposition
(all six tets share the cell's 0→7 main diagonal), NOT the five-tet split. The
five-tet split must alternate parity per cell or shared faces mismatch; Kuhn's
tiles space consistently with one orientation, so adjacent cells always share a
face and the boundary-extraction (a triangular face is on the surface iff it
belongs to exactly one tet) is watertight. `ball()` clips the lattice to a
sphere by cell centre. Consequence: the six tets of a cell alternate in winding,
so their *signed* volumes sum to ~zero — "total volume" must sum magnitudes
(the volume constraint preserves each tet's own signed volume, which is correct).

2026-06-20 — Particle↔rigid contacts (`ParticleBodyContact`) are LINEAR-ONLY and
two-way, deliberately matching the rigid-contact convention (the 2026-06-16
entry): the particle is pushed out along the surface normal and the rigid body
recoils at its centre of mass with no torque. So a soft body can shove a rigid
body and cloth rides a moving collider, but a tumbling cube keeps tumbling under
its own angular momentum (contacts add no spin). This is why the cloth-drape
demo spins its cube about a near-vertical axis — end-over-end tumbling on the
ground would need angular contact response, which is still future work. Particle
contacts use an O(particles × bodies) sweep with an AABB reject, not the BVH:
rigid counts are tiny in soft scenes. Bending constraints are plain
`ParticleDistance` links between every-other particle (not dihedral angle
constraints) — the simplest stable bending, per the task.

2026-06-20 — Deforming geometry (soft-body surfaces, cloth grids) is rendered
OUTSIDE the ECS `MeshRenderer` path: there is no entity and the vertices move
every step. `elderforge::deformable::DeformableMeshes` owns one dynamic
`GpuMesh` per soft body / cloth (static index buffer, `COPY_DST` vertex buffer
restreamed each frame from particle positions via `GpuMesh::update_vertices`,
normals recomputed CPU-side), drawn with an identity model matrix (particles are
world-space) appended to the forward pass's draw list. The deformable→`Vertex`
assembly lives in the elderforge crate (it bridges physics + renderer); the
physics crate stays renderer-free. Soft bodies / cloth are NOT serialized to
`.escene` yet — a loaded scene rebuilds an empty `DeformableMeshes` — so the
demos build them fresh at startup. The `forward.wgsl` fragment shader became
two-sided (flip the normal when `!front_facing`) because culling is off
engine-wide and a flag must shade on whichever face shows; this is inert for
solid meshes (their back faces are occluded). A `PhysicsWorld::wind` field (a
uniform acceleration applied only to particles) was added so the flag billows —
a flat sheet pinned at two corners at its natural width is taut and otherwise
just hangs flat.

2026-06-21 — `--borderless` means "no editor chrome", not just a frameless OS
window. In borderless mode the app constructs NO `EditorState` (`editor: None`);
`update` branches on `editor.as_mut()` so the editorless path always plays at
the scene's own substeps/timestep, skips the egui pass entirely, and clears to
black. (It also turns off OS window decorations via the new
`WindowConfig.decorations`, but the defining behavior is the absent editor.)
Sim controls only exist when the editor does, so a borderless capture can't be
paused — intended.

2026-06-21 — MSAA lives in the forward pass, not the surface: `ForwardPass`
renders into an owned multisampled color+depth target and RESOLVES into the
single-sample surface view, so egui (which has no MSAA) paints over the resolved
view unchanged. The CLI accepts 1/2/4/8 but the app clamps to
`RenderContext::supported_sample_count` (adapter `MULTISAMPLE_X*` flags, color ∩
depth) BEFORE building the pipeline — wgpu treats an unsupported sample count as
a fatal validation error, so the clamp (8×→4× on this Metal surface, logged)
must happen up front, never as a fallback after a failed pipeline build.

2026-06-21 — `Demo::setup` returns a `DemoConfig { anim, light }` instead of
`()`; demos with neither lift `()` via `From<()>`. Per-frame demo motion goes
through `DemoAnim` applied by the app against an accumulated `sim_time` (so it
pauses when physics pauses): `OrbitCamera` overwrites the active camera entity's
`Transform` each frame, and `StagedDrop` releases soft bodies by restoring saved
particle inverse masses at a release time — the bodies are spawned then PINNED
(zero inv_mass) at setup, hanging frozen until their cue, which is why
`PhysicsWorld::particles_mut` was added. This keeps all timed behavior out of
the physics crate and in the demo/app layer.

2026-06-21 — The key light is a forward-pass uniform, not per-material. `Globals`
(group 0) grew `light_dir`/`light_color` vec4s (uniform 64→96 B, visibility
VERTEX→VERTEX_FRAGMENT); `ForwardPass::set_light` writes it each frame.
`DirectionalLight::default` (dir (0.3,0.9,0.35), white) reproduces the previous
hard-coded shader light EXACTLY (same `0.35 + 0.65·diffuse` shade, same
orientation tint), so every demo that doesn't override it looks identical to
before — only `cloth-drape` sets a warm upper-left light.

2026-06-21 — The capture demos use HYPHENATED canonical names (`cloth-drape`,
`softbody-drop`, `cloth-tear`, `mixed`) and `from_name` matches them as exact
lowercased strings — it deliberately does NOT normalize `-`↔`_`, because
`cloth-drape` (the new 40×40 capture showcase) and `cloth_drape` (the older
spinning-cube drape from phase 13) are DIFFERENT demos that must stay
distinguishable. Underscore spellings of the other three are accepted as
aliases since they don't collide.

2026-06-21 — The physics debug overlay is a self-contained per-frame emit, not
solver instrumentation: `PhysicsWorld::emit_debug` recomputes whatever a layer
needs (it rebuilds a BVH and re-runs narrowphase for the contact layer) rather
than retaining the solver's per-substep scratch. This keeps the hot path
untouched and, crucially, makes the contact/BVH overlays correct even while the
scene SLEEPS (the solver's `generate_contacts` short-circuits to zero work when
nothing is awake; the debug path ignores that). The cost is duplicated
broadphase/narrowphase, but only when a layer is enabled — an inspection-time
expense, not a runtime one.

2026-06-21 — The renderer crate stays independent of the physics crate: the
debug `DebugPass` consumes its own `DebugVertex`, and the
`elderforge::debug_overlay::DebugOverlay` bridge (in the binary's lib half,
alongside `deformable`) converts physics `DebugLine`/`DebugPoint` → renderer
vertices. "Don't allocate per-frame" holds at three levels: the physics
`DebugDraw` Vecs, the bridge's vertex Vecs, and the pass's GPU `GrowBuffer` are
all cleared/reused, growing only when a frame's geometry exceeds capacity.

2026-06-21 — The debug overlay renders SINGLE-SAMPLED, `LoadOp::Load`, no depth,
over the already-RESOLVED surface — deliberately on top of the scene, not
depth-tested into it. This sidesteps sharing the forward pass's multisampled
color/depth targets (the overlay would otherwise need to render into the MSAA
target before resolve) and matches the "overlay on top" intent: a velocity
vector behind a box is still visible. Frame order is forward (→resolve) → debug
→ egui, all writing the one surface texture.
