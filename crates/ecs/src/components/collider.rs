use elderforge_physics::{ColliderShape, PhysicsMaterial};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collider {
    pub shape: ColliderShape,
    pub material: PhysicsMaterial,
}
