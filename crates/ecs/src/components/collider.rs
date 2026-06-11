use elderforge_physics::{ColliderShape, PhysicsMaterial};

#[derive(Debug, Clone)]
pub struct Collider {
    pub shape: ColliderShape,
    pub material: PhysicsMaterial,
}
