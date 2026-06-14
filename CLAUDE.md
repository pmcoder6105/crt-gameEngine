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
- Next: feed real contacts into the XPBD solver (replace the bring-up
  impulse path), add friction + box colliders (cubes are sphere-approx in
  physics for now), and render via the real PBR pass instead of the
  forward bootstrap. BVH broadphase is still Phase 6; `naive_pairs` is the
  placeholder until then.

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
