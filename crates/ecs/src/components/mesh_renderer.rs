use elderforge_core::handles::{MaterialHandle, MeshHandle};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshRenderer {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
}
