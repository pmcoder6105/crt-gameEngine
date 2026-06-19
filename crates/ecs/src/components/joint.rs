use elderforge_physics::solver::constraints::JointKind;
use elderforge_physics::BodyHandle;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Joint {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    pub kind: JointKind,
}
