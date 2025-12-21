//! Skills marketplace - NIP-SA skill licensing and execution

// Existing modules
pub mod agentskill;
pub mod execution;
pub mod versioning;

// d-008 new modules
pub mod publish;
pub mod install;
pub mod invoke;
pub mod license;

// Re-exports for backwards compatibility
pub use agentskill::{Skill, SkillError, SkillManifest, SkillMetadata, discover_skills, validate_skill_name};
