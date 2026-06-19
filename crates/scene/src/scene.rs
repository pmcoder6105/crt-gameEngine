//! Scene — owns the hecs World, the PhysicsWorld, and asset references.

use elderforge_ecs::World;
use elderforge_physics::PhysicsWorld;

use crate::assets::SceneAssets;

pub struct Scene {
    pub world: World,
    pub physics: PhysicsWorld,
    pub name: String,
    /// Maps the resource handles stored on `MeshRenderer`/material components to
    /// stable, serializable descriptions (file paths, builtin names, PBR params)
    /// so a scene can be saved and reloaded. The app realizes these into the
    /// renderer's GPU `ResourceCache` at the same handles.
    pub assets: SceneAssets,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            physics: PhysicsWorld::new(),
            name: "untitled".to_string(),
            assets: SceneAssets::new(),
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
