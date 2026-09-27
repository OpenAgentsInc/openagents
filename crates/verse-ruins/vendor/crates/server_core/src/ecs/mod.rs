//! Minimal server-side ECS wrapper (phase 1 of ECS refactor).
//!
//! This module provides a tiny ECS-like world specialized for authoritative
//! actors. It replaces the previous `ActorStore` Vec while keeping the public
//! spawn/query surface stable for ServerState.
//!
//! Components covered in phase 1:
//! - Transform (pos, yaw) + Radius
//! - Kind (Wizard/Zombie/Boss) + Faction
//! - Health (hp/max)
//!
//! Later phases will add systems/schedule and additional components (speed,
//! melee cooldowns, projectiles, homing, targets, etc.).

mod world;
pub use world::*;
pub mod geom;
pub mod schedule;
