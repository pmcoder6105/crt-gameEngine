//! End-to-end check of the app's scene save/load path *with real GPU asset
//! realization* — the seam the editor's Save/Load buttons drive.
//!
//! It mirrors `App::init`: `SceneAssets` is the handle authority (builtin meshes
//! + a material are registered there), a demo populates the world, and the asset
//! table is realized into a `ResourceCache` at those handles. Then it saves the
//! scene, loads it into a fresh `Scene`, re-realizes, and asserts the reloaded
//! scene matches and all its mesh/material handles resolve in the rebuilt cache.
//!
//! Skips (with a note) when no GPU adapter is available, e.g. headless CI.

use elderforge::assets::AssetManager;
use elderforge::demos::{Demo, DemoAssets};
use elderforge_scene::assets::{MaterialDef, MeshSource};
use elderforge_scene::loader::load_scene;
use elderforge_scene::serializer::{save_scene, scene_to_doc};
use elderforge_scene::Scene;

/// Build a scene exactly the way `App::init` does: register the builtin meshes
/// and default material in the scene's asset table, then run the demo setup.
fn build_demo_scene(demo: Demo) -> (Scene, DemoAssets) {
    let mut scene = Scene::new();
    let cube = scene.assets.register_mesh(MeshSource::Builtin("cube".into()));
    let sphere = scene.assets.register_mesh(MeshSource::Builtin("sphere".into()));
    let capsule = scene.assets.register_mesh(MeshSource::Builtin("capsule".into()));
    let plane = scene.assets.register_mesh(MeshSource::Builtin("plane".into()));
    let material = scene.assets.register_material(MaterialDef::default());
    let assets = DemoAssets { cube, sphere, capsule, plane, material };
    demo.setup(&mut scene, &assets);
    (scene, assets)
}

#[test]
fn realize_save_load_rerealize() {
    let instance = wgpu::Instance::default();
    let Some(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("no GPU adapter available; skipping scene IO test");
        return;
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("device request failed");

    // The stress demo uses cubes, spheres, and capsules, so every builtin mesh
    // gets exercised by the realize step.
    let (scene, assets) = build_demo_scene(Demo::Stress);
    let mut manager = AssetManager::new();

    // Realize the asset table into a GPU cache and confirm each builtin mesh +
    // the material landed at the handle the scene assigned it.
    let cache = manager
        .realize(&scene, &device, &queue)
        .expect("realize demo assets");
    for handle in [assets.cube, assets.sphere, assets.capsule, assets.plane] {
        assert!(cache.mesh(handle).is_some(), "builtin mesh should be resident");
    }
    assert!(cache.material(assets.material).is_some(), "material should be resident");

    // Save to disk, then load into a brand-new scene.
    let path = std::env::temp_dir().join("elderforge_scene_io.escene");
    save_scene(&scene, &path).expect("save scene");
    let loaded = load_scene(&path).expect("load scene");

    // The reloaded scene matches the original (name, world config + bodies,
    // asset table, entity count).
    let before = scene_to_doc(&scene);
    let after = scene_to_doc(&loaded);
    assert_eq!(before.name, after.name);
    assert_eq!(before.physics, after.physics);
    assert_eq!(before.assets, after.assets);
    assert_eq!(scene.world.len(), loaded.world.len());

    // Re-realizing the loaded scene rebuilds an equivalent cache, and every
    // MeshRenderer's handles resolve against it — proving a round-tripped scene
    // is fully renderable, not just structurally equal.
    let reloaded_cache = manager
        .realize(&loaded, &device, &queue)
        .expect("realize reloaded assets");
    let mut checked = 0usize;
    for (_e, mr) in loaded
        .world
        .query::<&elderforge_ecs::components::MeshRenderer>()
        .iter()
    {
        assert!(reloaded_cache.mesh(mr.mesh).is_some(), "mesh handle resolves after reload");
        assert!(reloaded_cache.material(mr.material).is_some(), "material handle resolves after reload");
        checked += 1;
    }
    assert!(checked > 0, "scene should have drawable entities");
}
