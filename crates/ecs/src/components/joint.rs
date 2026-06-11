use elderforge_physics::solver::constraints::JointKind;
use elderforge_physics::BodyHandle;

#[derive(Debug, Clone, Copy)]
pub struct Joint {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    pub kind: JointKind,
}
