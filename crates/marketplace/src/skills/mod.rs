//! Skills marketplace - NIP-SA skill licensing and execution

// Existing modules
pub mod agentskill;
pub mod execution;
pub mod versioning;

// d-008 new modules
pub mod browse;
pub mod install;
pub mod invoke;
pub mod license;
pub mod publish;

// Re-exports for backwards compatibility
pub use agentskill::{
    Skill, SkillError, SkillManifest, SkillMetadata, discover_skills, validate_skill_name,
};

// Re-export NIP-SA skill types
pub use nostr::{
    KIND_SKILL_DELIVERY, KIND_SKILL_LICENSE, SkillDelivery, SkillDeliveryContent, SkillLicense,
    SkillLicenseContent,
};

// Re-export license manager
pub use license::{LicenseError, LicenseManager};

// Re-export browse functionality
pub use browse::{BrowseError, SearchFilters, SkillBrowser, SkillCategory, SkillListing, SortBy};
