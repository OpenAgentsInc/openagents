//! Marketplace for plugins, skills, and agents
//!
//! This crate provides infrastructure for discovering, installing, and managing
//! marketplace items including plugins, skills, and agents.

pub mod db;
pub mod discovery;
pub mod repository;
pub mod skills;
pub mod types;

pub use discovery::{SearchFilters, SkillListing, SortOrder, discover_local_skills};
pub use repository::{Repository, Skill as SkillRecord, SkillRepository, SkillVersion};
pub use skills::{Skill, SkillError, SkillManifest, SkillMetadata, discover_skills, validate_skill_name};
pub use types::{ItemStatus, MarketplaceItemType};
