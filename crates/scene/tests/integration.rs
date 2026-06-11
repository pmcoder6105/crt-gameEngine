//! Cross-crate integration: ECS components + physics world inside a Scene.

use elderforge_ecs::components::{PhysicsBody, Transform};
use elderforge_physics::RigidBody;
use elderforge_scene::Scene;

#[test]
fn spawn_physics_entity_and_step() {
    let mut scene = Scene::new();
    let handle = scene.physics.add_rigid_body(RigidBody::default());
    let entity = scene
        .world
        .spawn((Transform::default(), PhysicsBody { handle }));

    scene.physics.step(1.0 / 120.0);

    let body = scene.physics.body(handle).expect("body should exist");
    assert!(body.position.y < 0.0, "gravity should pull the body down");
    assert!(scene.world.contains(entity));
}

#[test]
fn save_and_reload_scene() {
    let dir = std::env::temp_dir().join("elderforge_scene_test");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("roundtrip.escene");

    let scene = Scene::new();
    elderforge_scene::serializer::save_scene(&scene, &path).expect("save scene");
    let loaded = elderforge_scene::loader::load_scene(&path).expect("load scene");
    assert_eq!(loaded.name, "roundtrip");
}
