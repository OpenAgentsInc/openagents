//! Agent Skills types per agentskills.io specification
//!
//! Implements the open standard for agent-executable skills with SKILL.md files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur when working with Agent Skills
#[derive(Debug, Error)]
pub enum SkillError {
    #[error("Invalid skill name: {0}")]
    InvalidName(String),

    #[error("Invalid description: {0}")]
    InvalidDescription(String),

    #[error("YAML frontmatter parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Skill metadata from SKILL.md frontmatter
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillMetadata {
    /// Skill name (1-64 chars, lowercase alphanumeric + hyphens)
    pub name: String,

    /// Description of purpose and usage (1-1024 chars)
    pub description: String,

    /// License terms (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Environment requirements (max 500 chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<String>,

    /// Custom key-value properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,

    /// Space-delimited pre-approved tools (experimental)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<String>,
}

impl SkillMetadata {
    /// Validate the skill metadata
    pub fn validate(&self) -> Result<(), SkillError> {
        // Validate name
        validate_skill_name(&self.name)?;

        // Validate description
        if self.description.is_empty() || self.description.len() > 1024 {
            return Err(SkillError::InvalidDescription(format!(
                "Description must be 1-1024 characters, got {}",
                self.description.len()
            )));
        }

        // Validate compatibility length if present
        if let Some(ref compat) = self.compatibility {
            if compat.len() > 500 {
                return Err(SkillError::InvalidDescription(format!(
                    "Compatibility field must be max 500 characters, got {}",
                    compat.len()
                )));
            }
        }

        Ok(())
    }
}

/// Full skill with metadata, content, and filesystem path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Skill metadata from frontmatter
    pub metadata: SkillMetadata,

    /// Markdown content body (after frontmatter)
    pub content: String,

    /// Path to skill directory
    pub path: PathBuf,
}

impl Skill {
    /// Parse a SKILL.md file into a Skill
    pub fn from_file(path: PathBuf) -> Result<Self, SkillError> {
        let content = std::fs::read_to_string(&path)?;
        Self::from_content(content, path.parent().unwrap_or(&path).to_path_buf())
    }

    /// Parse SKILL.md content into a Skill
    pub fn from_content(content: String, path: PathBuf) -> Result<Self, SkillError> {
        // Split frontmatter and body
        let (metadata_str, body) = parse_frontmatter(&content)?;

        // Parse metadata YAML
        let metadata: SkillMetadata = serde_yaml::from_str(&metadata_str)?;

        // Validate metadata
        metadata.validate()?;

        // Validate name matches directory
        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
            if dir_name != metadata.name {
                return Err(SkillError::InvalidName(format!(
                    "Skill name '{}' must match directory name '{}'",
                    metadata.name, dir_name
                )));
            }
        }

        Ok(Self {
            metadata,
            content: body,
            path,
        })
    }
}

/// Skill manifest for marketplace listing (extends Skill with creator/pricing info)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Skill metadata
    #[serde(flatten)]
    pub metadata: SkillMetadata,

    /// Creator's public key (Nostr npub)
    pub creator_pubkey: String,

    /// Price in sats (optional, 0 = free)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_sats: Option<u64>,

    /// Marketplace-specific metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketplace_metadata: Option<HashMap<String, String>>,
}

/// Validate skill name according to spec
pub fn validate_skill_name(name: &str) -> Result<(), SkillError> {
    // Length check
    if name.is_empty() || name.len() > 64 {
        return Err(SkillError::InvalidName(format!(
            "Name must be 1-64 characters, got {}",
            name.len()
        )));
    }

    // Character check: lowercase alphanumeric and hyphens only
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(SkillError::InvalidName(
            "Name must contain only lowercase alphanumeric characters and hyphens".to_string(),
        ));
    }

    // Cannot start or end with hyphen
    if name.starts_with('-') || name.ends_with('-') {
        return Err(SkillError::InvalidName(
            "Name cannot start or end with a hyphen".to_string(),
        ));
    }

    // No consecutive hyphens
    if name.contains("--") {
        return Err(SkillError::InvalidName(
            "Name cannot contain consecutive hyphens".to_string(),
        ));
    }

    Ok(())
}

/// Parse YAML frontmatter from markdown content
fn parse_frontmatter(content: &str) -> Result<(String, String), SkillError> {
    let lines: Vec<&str> = content.lines().collect();

    // Must start with ---
    if lines.first().is_none_or(|l| l.trim() != "---") {
        return Err(SkillError::MissingField(
            "SKILL.md must start with YAML frontmatter (---)".to_string(),
        ));
    }

    // Find closing ---
    let end_idx = lines[1..]
        .iter()
        .position(|l| l.trim() == "---")
        .ok_or_else(|| {
            SkillError::MissingField("SKILL.md frontmatter must end with ---".to_string())
        })?;

    // Extract frontmatter (between the two ---)
    let frontmatter = lines[1..=end_idx].join("\n");

    // Extract body (after closing ---)
    let body = lines[(end_idx + 2)..].join("\n");

    Ok((frontmatter, body))
}

/// Discover all SKILL.md files recursively in a directory
pub fn discover_skills(dir: &std::path::Path) -> Result<Vec<Skill>, SkillError> {
    let mut skills = Vec::new();

    if !dir.is_dir() {
        return Ok(skills);
    }

    // Walk directory recursively
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Check if this is a SKILL.md file
        if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
            // Try to parse the skill
            match Skill::from_file(path.to_path_buf()) {
                Ok(skill) => skills.push(skill),
                Err(e) => {
                    // Log error but continue discovering other skills
                    eprintln!(
                        "Warning: Failed to parse skill at {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }
    }

    Ok(skills)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_skill_name_valid() {
        assert!(validate_skill_name("my-skill").is_ok());
        assert!(validate_skill_name("skill123").is_ok());
        assert!(validate_skill_name("a").is_ok());
        assert!(validate_skill_name("a-b-c-d").is_ok());
    }

    #[test]
    fn test_validate_skill_name_invalid() {
        assert!(validate_skill_name("").is_err());
        assert!(validate_skill_name("MySkill").is_err()); // uppercase
        assert!(validate_skill_name("my_skill").is_err()); // underscore
        assert!(validate_skill_name("-skill").is_err()); // starts with hyphen
        assert!(validate_skill_name("skill-").is_err()); // ends with hyphen
        assert!(validate_skill_name("my--skill").is_err()); // consecutive hyphens
        assert!(validate_skill_name(&"a".repeat(65)).is_err()); // too long
    }

    #[test]
    fn test_skill_metadata_validation() {
        let valid = SkillMetadata {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };
        assert!(valid.validate().is_ok());

        let invalid_desc = SkillMetadata {
            name: "test-skill".to_string(),
            description: "".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };
        assert!(invalid_desc.validate().is_err());
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
---
# Test Skill

This is the body content."#;

        let (frontmatter, body) = parse_frontmatter(content).unwrap();
        assert!(frontmatter.contains("name: test-skill"));
        assert!(body.contains("# Test Skill"));
    }

    #[test]
    fn test_skill_from_content() {
        let content = r#"---
name: pdf-tools
description: Extracts text and tables from PDF files
license: MIT
---
# PDF Tools

Use this skill for PDF operations."#;

        let skill = Skill::from_content(content.to_string(), PathBuf::from("pdf-tools")).unwrap();

        assert_eq!(skill.metadata.name, "pdf-tools");
        assert_eq!(skill.metadata.license, Some("MIT".to_string()));
        assert!(skill.content.contains("# PDF Tools"));
    }

    #[test]
    fn test_skill_manifest_serde() {
        let manifest = SkillManifest {
            metadata: SkillMetadata {
                name: "test-skill".to_string(),
                description: "A test skill".to_string(),
                license: None,
                compatibility: None,
                metadata: None,
                allowed_tools: None,
            },
            creator_pubkey: "npub1test".to_string(),
            price_sats: Some(1000),
            marketplace_metadata: None,
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: SkillManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.metadata.name, "test-skill");
        assert_eq!(deserialized.creator_pubkey, "npub1test");
    }

    #[test]
    fn test_discover_skills() {
        use std::fs;

        // Create a temporary directory structure with skills
        let temp_dir = std::env::temp_dir().join("test_skills_discovery");
        let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
        fs::create_dir_all(&temp_dir).unwrap();

        // Create skill 1
        let skill1_dir = temp_dir.join("my-skill");
        fs::create_dir_all(&skill1_dir).unwrap();
        fs::write(
            skill1_dir.join("SKILL.md"),
            r#"---
name: my-skill
description: My test skill
---
# My Skill"#,
        )
        .unwrap();

        // Create skill 2 in a subdirectory
        let skill2_dir = temp_dir.join("subdir").join("another-skill");
        fs::create_dir_all(&skill2_dir).unwrap();
        fs::write(
            skill2_dir.join("SKILL.md"),
            r#"---
name: another-skill
description: Another test skill
---
# Another Skill"#,
        )
        .unwrap();

        // Discover skills
        let skills = discover_skills(&temp_dir).unwrap();

        // Should find both skills
        assert_eq!(skills.len(), 2);
        let names: Vec<String> = skills.iter().map(|s| s.metadata.name.clone()).collect();
        assert!(names.contains(&"my-skill".to_string()));
        assert!(names.contains(&"another-skill".to_string()));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
