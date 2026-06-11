//! Fixed-step physics update; syncs body poses back into ECS transforms.

use elderforge_ecs::components::{PhysicsBody, Transform};
use elderforge_scene::Scene;

pub fn run(scene: &mut Scene, dt: f32) {
    scene.physics.step(dt);

    let Scene { world, physics, .. } = scene;
    for (_entity, (transform, body)) in world.query_mut::<(&mut Transform, &PhysicsBody)>() {
        if let Some(rigid_body) = physics.body(body.handle) {
            transform.position = rigid_body.position;
            transform.rotation = rigid_body.rotation;
        }
    }
}
