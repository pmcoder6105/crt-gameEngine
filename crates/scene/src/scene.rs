//! Scene — owns the hecs World, the PhysicsWorld, and asset references.

use elderforge_ecs::World;
use elderforge_physics::PhysicsWorld;

pub struct Scene {
    pub world: World,
    pub physics: PhysicsWorld,
    pub name: String,
    // TODO: asset handles referenced by this scene, for load/unload tracking.
}

impl Scene {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            physics: PhysicsWorld::new(),
            name: "untitled".to_string(),
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
