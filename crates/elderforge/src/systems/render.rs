//! Collect draw calls from the scene and submit them to the renderer.

use elderforge_renderer::context::RenderContext;
use elderforge_scene::Scene;

pub fn run(_scene: &Scene, renderer: Option<&RenderContext>) {
    let Some(_context) = renderer else {
        // No surface yet (headless or window not created).
        return;
    };
    // TODO: gather (Transform, MeshRenderer) pairs into a draw list and
    // submit through the shadow -> PBR -> debug passes.
}
