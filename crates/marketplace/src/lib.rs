//! Marketplace for plugins, skills, and agents
//!
//! This crate provides infrastructure for discovering, installing, and managing
//! marketplace items including plugins, skills, and agents.

pub mod skills;
pub mod types;

pub use types::{ItemStatus, MarketplaceItemType};
