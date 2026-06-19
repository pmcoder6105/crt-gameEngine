use elderforge_physics::BodyHandle;
use serde::{Deserialize, Serialize};

/// Links an entity to a body in the `PhysicsWorld`. Stores only the handle —
/// never inline body data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicsBody {
    pub handle: BodyHandle,
}
