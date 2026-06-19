//! Collect draw calls from the scene and record them into the current frame.

use elderforge_core::math::Mat4;
use elderforge_ecs::components::{Camera, MeshRenderer, Transform};
use elderforge_renderer::{Draw, ForwardPass, FrameContext, RenderContext, ResourceCache};
use elderforge_scene::Scene;

/// Record the 3D forward pass for `scene` into `frame`. The caller owns the
/// frame lifecycle (acquire / present) so it can also record the editor's egui
/// pass into the same encoder before presenting.
pub fn record(
    scene: &Scene,
    context: &RenderContext,
    cache: &ResourceCache,
    forward: &mut ForwardPass,
    frame: &mut FrameContext,
) {
    let (width, height) = context.size();
    let aspect = width as f32 / height.max(1) as f32;
    let view_proj = active_camera(scene, aspect).unwrap_or(Mat4::IDENTITY);

    // One draw per (Transform, MeshRenderer) whose mesh is resident in the cache.
    let mut draws = Vec::new();
    for (_entity, (transform, mesh_renderer)) in
        scene.world.query::<(&Transform, &MeshRenderer)>().iter()
    {
        if let Some(mesh) = cache.mesh(mesh_renderer.mesh) {
            draws.push(Draw { model: transform.matrix(), mesh });
        }
    }

    forward.render(
        &context.device,
        &context.queue,
        &mut frame.encoder,
        &frame.view,
        view_proj,
        &draws,
    );
}

/// View-projection of the first active camera entity, built from its `Camera`
/// component and `Transform`. `None` if the scene has no active camera.
fn active_camera(scene: &Scene, aspect: f32) -> Option<Mat4> {
    for (_entity, (camera, transform)) in scene.world.query::<(&Camera, &Transform)>().iter() {
        if camera.is_active {
            let view = Mat4::from_rotation_translation(transform.rotation, transform.position)
                .inverse();
            let proj =
                Mat4::perspective_rh(camera.fov_y_radians, aspect, camera.near, camera.far);
            return Some(proj * view);
        }
    }
    None
}
