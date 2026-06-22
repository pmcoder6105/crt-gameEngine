//! The debug-capture demos must actually drive their overlays. This builds each
//! one through `Demo::setup` and emits its scripted layers at a given sim time —
//! the exact path the app runs in borderless capture mode (no editor to toggle
//! the overlays) — asserting the right geometry shows up. It's GPU-free: the
//! demo setups only attach asset handles to components, and `emit_debug` is
//! CPU-only, so no adapter is needed.

use elderforge::demos::{Demo, DemoAssets};
use elderforge_core::handles::{MaterialHandle, MeshHandle};
use elderforge_physics::DebugDraw;
use elderforge_scene::Scene;

/// Dummy asset handles — the demo setups only stash these in `MeshRenderer`
/// components; nothing here touches the GPU.
fn assets() -> DemoAssets {
    DemoAssets {
        cube: MeshHandle::new(0, 0),
        sphere: MeshHandle::new(1, 0),
        capsule: MeshHandle::new(2, 0),
        plane: MeshHandle::new(3, 0),
        material: MaterialHandle::new(0, 0),
    }
}

/// Build `demo`, step physics `steps` times, then emit the overlay geometry its
/// debug schedule calls for at sim time `at`.
fn emit_after_steps(demo: Demo, steps: usize, at: f32) -> DebugDraw {
    let mut scene = Scene::new();
    let config = demo.setup(&mut scene, &assets());
    for _ in 0..steps {
        scene.physics.step(1.0 / 120.0);
    }
    let layers = config.debug.layers_at(at);
    let mut draw = DebugDraw::default();
    scene.physics.emit_debug(layers, &mut draw);
    draw
}

#[test]
fn debug_stack_shows_overlays_from_the_start() {
    // `Always` schedule → every layer is on from t = 0.
    let draw = emit_after_steps(Demo::DebugStack, 60, 0.0);
    assert!(!draw.is_empty(), "all-on overlay should produce geometry");
}

#[test]
fn debug_cloth_glows_with_particle_springs() {
    let draw = emit_after_steps(Demo::DebugCloth, 30, 0.0);
    assert!(!draw.lines.is_empty(), "cloth springs should render as lines");
    assert!(!draw.points.is_empty(), "every particle gets an anchor point");
}

#[test]
fn debug_bvh_emits_only_tree_boxes() {
    let draw = emit_after_steps(Demo::DebugBvh, 30, 0.0);
    assert!(!draw.lines.is_empty(), "BVH overlay should draw node boxes");
    // The BVH layer is wire boxes only — no marker points.
    assert!(draw.points.is_empty(), "bvh-only overlay emits no points");
}

#[test]
fn debug_layers_opens_clean_then_reveals() {
    // Cumulative: nothing at t = 0, geometry once the first interval elapses.
    assert!(emit_after_steps(Demo::DebugLayers, 30, 0.0).is_empty());
    assert!(!emit_after_steps(Demo::DebugLayers, 30, 5.0).is_empty());
}

#[test]
fn debug_solo_shows_a_layer_immediately() {
    // Solo opens on its first (populated) layer at t = 0.
    assert!(!emit_after_steps(Demo::DebugSolo, 30, 0.0).is_empty());
}
