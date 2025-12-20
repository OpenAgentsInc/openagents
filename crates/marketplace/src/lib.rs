//! Marketplace for plugins, skills, and agents
//!
//! This crate provides infrastructure for discovering, installing, and managing
//! marketplace items including plugins, skills, and agents.

pub mod db;
pub mod repository;
pub mod skills;
pub mod types;

pub use repository::{Repository, Skill, SkillRepository, SkillVersion};
pub use types::{ItemStatus, MarketplaceItemType};
