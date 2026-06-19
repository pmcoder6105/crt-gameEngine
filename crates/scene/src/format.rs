//! The `.escene` file format: serde data-transfer objects for a whole scene
//! document, plus conversions to and from the live engine types.
//!
//! A scene file is a single JSON object with four parts:
//!
//! - `name` — the scene's display name.
//! - `physics` — world configuration (gravity, substeps, iterations) and every
//!   rigid body, in handle-index order so `PhysicsBody` handles resolve.
//! - `assets` — the [`SceneAssets`](crate::assets::SceneAssets) table: meshes
//!   and textures referenced by path/builtin name, and material parameters.
//!   List position is the resource handle's index.
//! - `entities` — one record per ECS entity, with an optional value for each
//!   component kind.
//!
//! Most engine types serialize directly (their crates derive serde). The one
//! exception is [`RigidBody`], which carries `mass = INFINITY` for immovable
//! bodies — not representable in JSON — and several derived fields; it goes
//! through [`RigidBodyDoc`], which stores immovable mass as `None` and lets the
//! constructors recompute inverse mass/inertia on load.

use elderforge_core::math::{Quat, Vec3};
use elderforge_ecs::components::{Camera, Collider, Joint, MeshRenderer, PhysicsBody, Transform};
use elderforge_physics::{BodyKind, Collider as BodyCollider, PhysicsMaterial, RigidBody};
use serde::{Deserialize, Serialize};

use crate::assets::{MaterialDef, MeshSource, SceneAssets, TextureSource};

/// On-disk format version, bumped on breaking changes to the layout.
pub const FORMAT_VERSION: u32 = 1;

/// The root of an `.escene` document.
#[derive(Debug, Serialize, Deserialize)]
pub struct SceneDoc {
    pub version: u32,
    pub name: String,
    pub physics: PhysicsDoc,
    #[serde(default)]
    pub assets: AssetsDoc,
    #[serde(default)]
    pub entities: Vec<EntityDoc>,
}

/// World configuration plus every rigid body.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsDoc {
    pub gravity: Vec3,
    pub substeps: u32,
    pub iterations: u32,
    #[serde(default)]
    pub bodies: Vec<RigidBodyDoc>,
}

/// The serializable form of a [`RigidBody`]: only the authoritative state, with
/// inverse mass / inertia recomputed by the constructors on load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RigidBodyDoc {
    pub kind: BodyKind,
    pub position: Vec3,
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    /// Mass in kg, or `None` for an immovable (infinite-mass) body.
    pub mass: Option<f32>,
    pub material: PhysicsMaterial,
    pub collider: BodyCollider,
    #[serde(default)]
    pub sleeping: bool,
}

impl RigidBodyDoc {
    /// Capture a live body's serializable state.
    pub fn from_body(body: &RigidBody) -> Self {
        Self {
            kind: body.kind,
            position: body.position,
            rotation: body.rotation,
            linear_velocity: body.linear_velocity,
            angular_velocity: body.angular_velocity,
            // inv_mass is the authority for "is this body movable"; a zero
            // inverse mass means infinite mass, which we store as `None`.
            mass: (body.inv_mass != 0.0).then_some(body.mass),
            material: body.material,
            collider: body.collider,
            sleeping: body.sleeping,
        }
    }

    /// Rebuild a live body. Inverse mass and the inertia tensor are derived from
    /// `mass` + `collider` by the constructors; `prev_*` are seeded to the
    /// current pose so the first substep derives zero velocity from them.
    pub fn into_body(self) -> RigidBody {
        let mut body = match self.mass {
            Some(mass) if matches!(self.kind, BodyKind::Dynamic | BodyKind::Kinematic) => {
                RigidBody::dynamic(self.position, mass, self.collider)
            }
            // Static, or any body with no finite mass, is immovable.
            _ => RigidBody::fixed(self.position, self.collider),
        };
        body.kind = self.kind;
        body.rotation = self.rotation;
        body.prev_position = self.position;
        body.prev_rotation = self.rotation;
        body.linear_velocity = self.linear_velocity;
        body.angular_velocity = self.angular_velocity;
        body.material = self.material;
        body.sleeping = self.sleeping;
        body
    }
}

/// The asset table: list position is the resource handle index.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetsDoc {
    #[serde(default)]
    pub meshes: Vec<MeshSource>,
    #[serde(default)]
    pub textures: Vec<TextureSource>,
    #[serde(default)]
    pub materials: Vec<MaterialDef>,
}

impl AssetsDoc {
    pub fn from_assets(assets: &SceneAssets) -> Self {
        Self {
            meshes: assets.meshes().map(|(_, s)| s.clone()).collect(),
            textures: assets.textures().map(|(_, s)| s.clone()).collect(),
            materials: assets.materials().map(|(_, m)| m.clone()).collect(),
        }
    }

    pub fn into_assets(self) -> SceneAssets {
        SceneAssets::from_parts(self.meshes, self.textures, self.materials)
    }
}

/// One entity: an optional value for each component kind. Absent components are
/// omitted from the JSON (and default to `None` when missing on load).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EntityDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<Transform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physics_body: Option<PhysicsBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_renderer: Option<MeshRenderer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collider: Option<Collider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint: Option<Joint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Camera>,
}

impl EntityDoc {
    /// True if this entity carries no components at all (used to skip empties).
    pub fn is_empty(&self) -> bool {
        self.transform.is_none()
            && self.physics_body.is_none()
            && self.mesh_renderer.is_none()
            && self.collider.is_none()
            && self.joint.is_none()
            && self.camera.is_none()
    }
}
