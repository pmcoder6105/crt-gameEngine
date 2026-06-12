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
- Next: winit event loop in the platform crate + renderer surface
  creation, so `cargo run` opens the editor window.

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
