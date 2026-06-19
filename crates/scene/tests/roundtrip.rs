//! Full save → reload round-trip over a scene with 20 entities of mixed
//! component types and a populated physics world + asset table.
//!
//! "Restart" is modeled by serializing to a file and loading it back in a fresh
//! `Scene` (the loader builds everything from the JSON alone). The reloaded
//! scene must match the original: same name, world config, bodies, asset table,
//! entity count, per-component counts, and resolvable handles.

use elderforge_core::math::{Quat, Vec3, Vec4};
use elderforge_ecs::components::{Camera, Collider, Joint, MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::shapes::{BoxShape, Capsule, ConvexHull, Sphere, TriMesh};
use elderforge_physics::solver::constraints::JointKind;
use elderforge_physics::{Collider as BodyCollider, ColliderShape, PhysicsMaterial, RigidBody};
use elderforge_scene::assets::{MaterialDef, MeshSource, TextureSource};
use elderforge_scene::loader::load_scene;
use elderforge_scene::serializer::{save_scene, scene_to_doc};
use elderforge_scene::Scene;

/// Build a scene with exactly 20 entities spanning every component type, plus a
/// non-default physics world and a populated asset table.
fn build_scene() -> Scene {
    let mut scene = Scene::new();
    scene.name = "roundtrip-20".to_string();
    // Non-default world config, to prove it round-trips (not just the defaults).
    scene.physics.gravity = Vec3::new(0.0, -3.0, 1.0);
    scene.physics.substeps = 12;
    scene.physics.iterations = 6;

    // Assets: a builtin mesh, two file meshes (OBJ + glTF), a texture, and two
    // materials (one referencing the texture).
    let cube = scene.assets.register_mesh(MeshSource::Builtin("cube".into()));
    let ship = scene.assets.register_mesh(MeshSource::File("models/ship.obj".into()));
    let rock = scene.assets.register_mesh(MeshSource::File("models/rock.gltf".into()));
    let albedo = scene.assets.register_texture(TextureSource::File("tex/albedo.png".into()));
    let mat_plain = scene.assets.register_material(MaterialDef::default());
    let mat_tex = scene.assets.register_material(MaterialDef {
        albedo: Vec4::new(0.8, 0.2, 0.1, 1.0),
        roughness: 0.3,
        metallic: 0.7,
        albedo_map: Some(albedo),
        normal_map: None,
    });
    let meshes = [cube, ship, rock];
    let mats = [mat_plain, mat_tex];

    // 12 dynamic bodies, each Transform + PhysicsBody + MeshRenderer, cycling
    // through sphere/box/capsule colliders and the two materials.
    let mut bodies = Vec::new();
    for i in 0..12 {
        let pos = Vec3::new(i as f32, 5.0 + i as f32 * 0.5, -(i as f32));
        let collider = match i % 3 {
            0 => BodyCollider::Sphere { radius: 0.5 },
            1 => BodyCollider::Box { half_extents: Vec3::splat(0.5) },
            _ => BodyCollider::Capsule { radius: 0.3, half_height: 0.5 },
        };
        let material = PhysicsMaterial { restitution: 0.05 * i as f32, ..PhysicsMaterial::default() };
        let body = RigidBody::dynamic(pos, 1.0 + i as f32, collider)
            .with_material(material)
            .with_linear_velocity(Vec3::new(0.0, -1.0, 0.0));
        let handle = scene.physics.add_rigid_body(body);
        bodies.push(handle);
        scene.world.spawn((
            Transform {
                position: pos,
                rotation: Quat::from_rotation_y(i as f32 * 0.1),
                scale: Vec3::ONE,
            },
            PhysicsBody { handle },
            MeshRenderer { mesh: meshes[i % 3], material: mats[i % 2] },
        ));
    }

    // A static ground body (immovable, infinite mass).
    let ground = scene
        .physics
        .add_rigid_body(RigidBody::fixed(Vec3::ZERO, BodyCollider::HalfSpace { normal: Vec3::Y, offset: 0.0 }));

    // Entity 13: a ground render plane (Transform + MeshRenderer, no body).
    scene
        .world
        .spawn((Transform::default(), MeshRenderer { mesh: cube, material: mat_plain }));

    // Entity 14: the camera (Camera + Transform).
    scene.world.spawn((
        Camera::default(),
        Transform {
            position: Vec3::new(0.0, 5.0, 15.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    ));

    // Entities 15-18: ECS Collider components spanning four ColliderShape kinds.
    let shapes = [
        ColliderShape::Sphere(Sphere { radius: 1.0 }),
        ColliderShape::Box(BoxShape { half_extents: Vec3::new(1.0, 2.0, 3.0) }),
        ColliderShape::Capsule(Capsule { radius: 0.4, half_height: 0.9 }),
        ColliderShape::ConvexHull(ConvexHull {
            points: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
        }),
    ];
    for (k, shape) in shapes.into_iter().enumerate() {
        scene.world.spawn((
            Transform { position: Vec3::new(-(k as f32), 1.0, 0.0), ..Default::default() },
            Collider { shape, material: PhysicsMaterial::default() },
        ));
    }

    // Entity 19: a TriMesh collider AND a hinge Joint between two bodies.
    scene.world.spawn((
        Collider {
            shape: ColliderShape::TriMesh(TriMesh {
                vertices: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
                indices: vec![[0, 1, 2]],
            }),
            material: PhysicsMaterial::default(),
        },
        Joint { body_a: bodies[0], body_b: bodies[1], kind: JointKind::Hinge },
    ));

    // Entity 20: a ball Joint that references the static ground body.
    scene.world.spawn((Joint { body_a: ground, body_b: bodies[2], kind: JointKind::Ball },));

    scene
}

#[test]
fn twenty_entity_save_load_matches() {
    let original = build_scene();
    assert_eq!(original.world.len(), 20, "test fixture should have 20 entities");

    // Save, then load into a brand-new scene (the "restart").
    let dir = std::env::temp_dir().join("elderforge_roundtrip");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("scene20.escene");
    save_scene(&original, &path).expect("save scene");
    let loaded = load_scene(&path).expect("load scene");

    // Name, world config, every body, and the whole asset table compare exactly
    // via the (order-stable) document form.
    let before = scene_to_doc(&original);
    let after = scene_to_doc(&loaded);
    assert_eq!(before.name, after.name, "scene name");
    assert_eq!(before.physics, after.physics, "world config + bodies");
    assert_eq!(before.assets, after.assets, "asset table");

    // 13 bodies (12 dynamic + 1 static ground).
    assert_eq!(loaded.physics.body_count(), 13);

    // Entity count and per-component-type counts survive. (Each query is
    // monomorphic, so we count inline rather than via a generic helper — the
    // scene crate doesn't depend on hecs directly.)
    assert_eq!(loaded.world.len(), 20);
    assert_eq!(loaded.world.query::<&Transform>().iter().count(), 18);
    assert_eq!(loaded.world.query::<&PhysicsBody>().iter().count(), 12);
    assert_eq!(loaded.world.query::<&MeshRenderer>().iter().count(), 13);
    assert_eq!(loaded.world.query::<&Collider>().iter().count(), 5);
    assert_eq!(loaded.world.query::<&Joint>().iter().count(), 2);
    assert_eq!(loaded.world.query::<&Camera>().iter().count(), 1);

    // Every MeshRenderer's handles resolve through the loaded asset table.
    for (_e, mr) in loaded.world.query::<&MeshRenderer>().iter() {
        assert!(loaded.assets.mesh_source(mr.mesh).is_some(), "mesh handle resolves");
        assert!(loaded.assets.material(mr.material).is_some(), "material handle resolves");
    }
    // Every PhysicsBody handle points at a live body in the reloaded world.
    for (_e, pb) in loaded.world.query::<&PhysicsBody>().iter() {
        assert!(loaded.physics.body(pb.handle).is_some(), "body handle resolves");
    }
    // Every Joint's referenced bodies still resolve.
    for (_e, joint) in loaded.world.query::<&Joint>().iter() {
        assert!(loaded.physics.body(joint.body_a).is_some());
        assert!(loaded.physics.body(joint.body_b).is_some());
    }

    // The textured material kept its texture reference, and that texture
    // resolves to its file source.
    let (_h, textured) = loaded
        .assets
        .materials()
        .find(|(_, m)| m.albedo_map.is_some())
        .expect("textured material survives");
    let tex = textured.albedo_map.unwrap();
    assert_eq!(
        loaded.assets.texture_source(tex),
        Some(&TextureSource::File("tex/albedo.png".into()))
    );
}
