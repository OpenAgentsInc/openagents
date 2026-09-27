//! data_runtime: data schemas and loaders (SRD-aligned).
//!
//! Extracted from the root crate's `core::data` so client/server/sim can
//! depend on a stable data API.

pub mod ability;
pub mod class;
pub mod encounter;
pub mod ids;
pub mod loader;
pub mod monster;
pub mod scenario;
pub mod specdb;
pub mod spell;
pub mod zone;
pub mod zone_scene;
pub mod zone_snapshot;
pub mod specs {
    pub mod archetypes;
    pub mod projectiles;
}
pub mod scene;
pub mod configs {
    pub mod destructible;
    pub mod input_camera;
    pub mod npc_unique;
    pub mod pc_animations;
    pub mod sorceress;
    pub mod telemetry;
}
