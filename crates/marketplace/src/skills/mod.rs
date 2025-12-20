//! Skills module
//!
//! Skills are predefined task templates that can be installed and executed.
//! This module implements the Agent Skills open standard (agentskills.io).

pub mod agentskill;

pub use agentskill::{
    Skill, SkillError, SkillManifest, SkillMetadata,
    validate_skill_name,
};
