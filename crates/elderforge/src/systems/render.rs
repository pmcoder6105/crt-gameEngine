//! Collect draw calls from the scene and submit them to the renderer.

use elderforge_renderer::passes::unlit::UnlitPass;
use elderforge_renderer::{GpuMesh, RenderContext};
use elderforge_scene::Scene;

pub fn run(
    _scene: &Scene,
    renderer: Option<&mut RenderContext>,
    triangle: Option<&(UnlitPass, GpuMesh)>,
) {
    let Some(context) = renderer else {
        // No surface yet (headless or window not created).
        return;
    };
    let mut frame = match context.frame() {
        Ok(frame) => frame,
        Err(err) => {
            // Transient (e.g. surface timeout mid-resize): skip this frame.
            log::warn!("render: skipping frame: {err}");
            return;
        }
    };
    // TODO: gather (Transform, MeshRenderer) pairs into a draw list and
    // submit through the shadow -> PBR -> debug passes. Until then, draw
    // the bootstrap triangle to prove the path.
    if let Some((pass, mesh)) = triangle {
        pass.draw(&mut frame.encoder, &frame.view, mesh);
    }
    context.present(frame);
}
