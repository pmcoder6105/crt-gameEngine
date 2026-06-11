use elderforge_physics::BodyHandle;

/// Links an entity to a body in the `PhysicsWorld`. Stores only the handle —
/// never inline body data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicsBody {
    pub handle: BodyHandle,
}
