//! Snapshot schema split scaffold.
//!
//! Phase-one: keep the original implementation via `legacy` while
//! introducing domain modules. External API remains unchanged by
//! re-exporting `legacy::*`.

pub mod actors;
pub mod destructibles;
pub mod encode;
pub mod hud;
pub mod projectiles;

#[path = "../snapshot_body.rs"]
mod legacy;

pub use legacy::*;
