//! Skills module
//!
//! Skills are predefined task templates that can be installed and executed.
//! This module implements the Agent Skills open standard (agentskills.io).

pub mod agentskill;
pub mod versioning;

pub use agentskill::{
    Skill, SkillError, SkillManifest, SkillMetadata,
    discover_skills, validate_skill_name,
};

pub use versioning::{
    SkillVersion, UpgradePath, VersionError, VersionRegistry,
};
