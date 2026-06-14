//! Collect draw calls from the scene and submit them to the renderer.

use elderforge_core::math::Mat4;
use elderforge_ecs::components::{Camera, MeshRenderer, Transform};
use elderforge_renderer::{Draw, ForwardPass, RenderContext, ResourceCache};
use elderforge_scene::Scene;

pub fn run(
    scene: &Scene,
    context: &mut RenderContext,
    cache: &ResourceCache,
    forward: &mut ForwardPass,
) {
    let mut frame = match context.frame() {
        Ok(frame) => frame,
        Err(err) => {
            // Transient (e.g. surface timeout mid-resize): skip this frame.
            log::warn!("render: skipping frame: {err}");
            return;
        }
    };

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
    context.present(frame);
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
