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

// Re-export NIP-SA skill types
pub use nostr::{
    SkillLicense, SkillLicenseContent, SkillDelivery, SkillDeliveryContent,
    KIND_SKILL_LICENSE, KIND_SKILL_DELIVERY,
};

// Re-export license manager
pub use license::{LicenseManager, LicenseError};
