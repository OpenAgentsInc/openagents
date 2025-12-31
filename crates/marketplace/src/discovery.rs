//! Skill discovery and search functionality

use crate::skills::{SkillMetadata, discover_skills};
use std::path::PathBuf;

/// Discover skills from multiple local directories
pub fn discover_local_skills(dirs: &[PathBuf]) -> Vec<SkillMetadata> {
    let mut all_metadata = Vec::new();

    for dir in dirs {
        match discover_skills(dir) {
            Ok(skills) => {
                // Extract just the metadata (progressive disclosure - only name + description)
                for skill in skills {
                    all_metadata.push(skill.metadata);
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to discover skills in {}: {}",
                    dir.display(),
                    e
                );
            }
        }
    }

    all_metadata
}

/// Sort order for skill listings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Most recently added first
    Recent,
    /// Alphabetical by name
    Alphabetical,
    /// Most popular (by install count)
    Popular,
}

/// Search filters for skill queries
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Filter by status
    pub status: Option<String>,
    /// Filter by category (from metadata)
    pub category: Option<String>,
    /// Only free skills
    pub free_only: bool,
}

/// Skill listing result (lightweight view for browse/search)
#[derive(Debug, Clone)]
pub struct SkillListing {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub author: Option<String>,
    pub version: String,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_local_skills_empty() {
        let result = discover_local_skills(&[]);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_discover_local_skills_multiple_dirs() {
        // Create two temporary directories with skills
        let temp_dir1 = std::env::temp_dir().join("test_discovery_1");
        let temp_dir2 = std::env::temp_dir().join("test_discovery_2");

        let _ = fs::remove_dir_all(&temp_dir1);
        let _ = fs::remove_dir_all(&temp_dir2);

        fs::create_dir_all(&temp_dir1).unwrap();
        fs::create_dir_all(&temp_dir2).unwrap();

        // Skill in dir 1
        let skill1_dir = temp_dir1.join("skill-one");
        fs::create_dir_all(&skill1_dir).unwrap();
        fs::write(
            skill1_dir.join("SKILL.md"),
            r#"---
name: skill-one
description: First skill
---
# Skill One"#,
        )
        .unwrap();

        // Skill in dir 2
        let skill2_dir = temp_dir2.join("skill-two");
        fs::create_dir_all(&skill2_dir).unwrap();
        fs::write(
            skill2_dir.join("SKILL.md"),
            r#"---
name: skill-two
description: Second skill
---
# Skill Two"#,
        )
        .unwrap();

        // Discover from both directories
        let metadata = discover_local_skills(&[temp_dir1.clone(), temp_dir2.clone()]);

        assert_eq!(metadata.len(), 2);
        let names: Vec<String> = metadata.iter().map(|m| m.name.clone()).collect();
        assert!(names.contains(&"skill-one".to_string()));
        assert!(names.contains(&"skill-two".to_string()));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir1);
        let _ = fs::remove_dir_all(&temp_dir2);
    }
}
