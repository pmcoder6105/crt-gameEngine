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
  body.rs           RigidBody, SoftBody, BodyHandle
  solver/
    mod.rs
    xpbd.rs         XPBD constraint solver (main integration loop)
    constraints.rs  Distance, contact, joint constraint types
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
