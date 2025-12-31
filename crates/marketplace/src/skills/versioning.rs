//! Skill versioning and upgrade path tracking
//!
//! Implements semantic versioning for skills with deprecation tracking
//! and upgrade path management.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Errors that can occur when working with skill versions
#[derive(Debug, Error)]
pub enum VersionError {
    #[error("Invalid version string: {0}")]
    InvalidVersion(String),

    #[error("Version not found: {0}")]
    VersionNotFound(String),

    #[error("Incompatible version: requires {required}, got {actual}")]
    IncompatibleVersion { required: String, actual: String },

    #[error("Version already deprecated: {0}")]
    AlreadyDeprecated(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Semantic version for a skill
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SkillVersion {
    /// Major version number
    pub major: u32,

    /// Minor version number
    pub minor: u32,

    /// Patch version number
    pub patch: u32,

    /// Optional pre-release identifier (e.g., "alpha.1", "beta.2")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre: Option<String>,

    /// Changelog entry for this version
    pub changelog: String,

    /// When this version was published
    pub published_at: DateTime<Utc>,

    /// Whether this version is deprecated
    #[serde(default)]
    pub deprecated: bool,

    /// Deprecation notice if deprecated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecation_notice: Option<String>,

    /// Recommended replacement version if deprecated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_version: Option<String>,
}

impl SkillVersion {
    /// Create a new skill version
    pub fn new(major: u32, minor: u32, patch: u32, changelog: impl Into<String>) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
            changelog: changelog.into(),
            published_at: Utc::now(),
            deprecated: false,
            deprecation_notice: None,
            replacement_version: None,
        }
    }

    /// Create a pre-release version
    pub fn new_prerelease(
        major: u32,
        minor: u32,
        patch: u32,
        pre: impl Into<String>,
        changelog: impl Into<String>,
    ) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: Some(pre.into()),
            changelog: changelog.into(),
            published_at: Utc::now(),
            deprecated: false,
            deprecation_notice: None,
            replacement_version: None,
        }
    }

    /// Parse a version string (e.g., "1.2.3", "2.0.0-beta.1")
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        let parts: Vec<&str> = s.split('-').collect();
        let version_part = parts[0];
        let pre = parts.get(1).map(|p| p.to_string());

        let nums: Vec<&str> = version_part.split('.').collect();
        if nums.len() != 3 {
            return Err(VersionError::InvalidVersion(format!(
                "Expected major.minor.patch, got '{}'",
                s
            )));
        }

        let major = nums[0]
            .parse()
            .map_err(|_| VersionError::ParseError(format!("Invalid major version: {}", nums[0])))?;
        let minor = nums[1]
            .parse()
            .map_err(|_| VersionError::ParseError(format!("Invalid minor version: {}", nums[1])))?;
        let patch = nums[2]
            .parse()
            .map_err(|_| VersionError::ParseError(format!("Invalid patch version: {}", nums[2])))?;

        Ok(Self {
            major,
            minor,
            patch,
            pre,
            changelog: String::new(),
            published_at: Utc::now(),
            deprecated: false,
            deprecation_notice: None,
            replacement_version: None,
        })
    }

    /// Check if this version is compatible with a required version
    /// Compatible means: same major version, and >= minor.patch
    pub fn is_compatible(&self, required: &SkillVersion) -> bool {
        // Pre-release versions are not compatible with stable releases
        if self.pre.is_some() || required.pre.is_some() {
            return self == required;
        }

        // Different major versions are incompatible
        if self.major != required.major {
            return false;
        }

        // Same major version: check minor.patch
        if self.minor > required.minor {
            return true;
        }

        if self.minor == required.minor {
            return self.patch >= required.patch;
        }

        false
    }

    /// Check if this is a breaking change from another version
    pub fn is_breaking_change(&self, from: &SkillVersion) -> bool {
        self.major > from.major
    }

    /// Deprecate this version
    pub fn deprecate(&mut self, notice: impl Into<String>, replacement: Option<impl Into<String>>) {
        self.deprecated = true;
        self.deprecation_notice = Some(notice.into());
        self.replacement_version = replacement.map(|r| r.into());
    }

    /// Get the version string (e.g., "1.2.3" or "2.0.0-beta.1")
    pub fn as_string(&self) -> String {
        format!("{}", self)
    }
}

impl fmt::Display for SkillVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref pre) = self.pre {
            write!(f, "-{}", pre)?;
        }
        Ok(())
    }
}

/// Upgrade path from one version to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradePath {
    /// Version upgrading from
    pub from_version: String,

    /// Version upgrading to
    pub to_version: String,

    /// List of breaking changes
    pub breaking_changes: Vec<String>,

    /// Migration notes and instructions
    pub migration_notes: String,

    /// Whether this is an automatic (non-breaking) upgrade
    pub automatic: bool,
}

impl UpgradePath {
    /// Create a new upgrade path
    pub fn new(
        from_version: impl Into<String>,
        to_version: impl Into<String>,
        breaking_changes: Vec<String>,
        migration_notes: impl Into<String>,
    ) -> Self {
        let automatic = breaking_changes.is_empty();
        Self {
            from_version: from_version.into(),
            to_version: to_version.into(),
            breaking_changes,
            migration_notes: migration_notes.into(),
            automatic,
        }
    }

    /// Check if this upgrade requires manual intervention
    pub fn requires_manual_intervention(&self) -> bool {
        !self.automatic
    }
}

/// Version registry for a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRegistry {
    /// Skill name
    pub skill_name: String,

    /// All versions of this skill
    pub versions: Vec<SkillVersion>,

    /// Upgrade paths between versions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_paths: Option<Vec<UpgradePath>>,
}

impl VersionRegistry {
    /// Create a new version registry
    pub fn new(skill_name: impl Into<String>) -> Self {
        Self {
            skill_name: skill_name.into(),
            versions: Vec::new(),
            upgrade_paths: None,
        }
    }

    /// Add a new version
    pub fn add_version(&mut self, version: SkillVersion) {
        self.versions.push(version);
        self.versions.sort_by(|a, b| b.cmp(a)); // Sort descending (latest first)
    }

    /// Get the latest stable version
    pub fn latest_stable(&self) -> Option<&SkillVersion> {
        self.versions
            .iter()
            .find(|v| v.pre.is_none() && !v.deprecated)
    }

    /// Get the latest version (including pre-releases)
    pub fn latest(&self) -> Option<&SkillVersion> {
        self.versions.iter().find(|v| !v.deprecated)
    }

    /// Find a specific version
    pub fn find_version(&self, version_str: &str) -> Option<&SkillVersion> {
        self.versions.iter().find(|v| v.as_string() == version_str)
    }

    /// List all versions
    pub fn list_versions(&self, include_deprecated: bool) -> Vec<&SkillVersion> {
        if include_deprecated {
            self.versions.iter().collect()
        } else {
            self.versions.iter().filter(|v| !v.deprecated).collect()
        }
    }

    /// Deprecate a version
    pub fn deprecate_version(
        &mut self,
        version_str: &str,
        notice: impl Into<String>,
        replacement: Option<impl Into<String>>,
    ) -> Result<(), VersionError> {
        let version = self
            .versions
            .iter_mut()
            .find(|v| v.as_string() == version_str)
            .ok_or_else(|| VersionError::VersionNotFound(version_str.to_string()))?;

        if version.deprecated {
            return Err(VersionError::AlreadyDeprecated(version_str.to_string()));
        }

        version.deprecate(notice, replacement);
        Ok(())
    }

    /// Get upgrade path from one version to another
    pub fn get_upgrade_path(&self, from: &str, to: &str) -> Option<&UpgradePath> {
        self.upgrade_paths
            .as_ref()?
            .iter()
            .find(|path| path.from_version == from && path.to_version == to)
    }

    /// Add an upgrade path
    pub fn add_upgrade_path(&mut self, path: UpgradePath) {
        if self.upgrade_paths.is_none() {
            self.upgrade_paths = Some(Vec::new());
        }
        self.upgrade_paths.as_mut().unwrap().push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parse() {
        let v = SkillVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.pre, None);

        let v_pre = SkillVersion::parse("2.0.0-beta.1").unwrap();
        assert_eq!(v_pre.major, 2);
        assert_eq!(v_pre.minor, 0);
        assert_eq!(v_pre.patch, 0);
        assert_eq!(v_pre.pre.as_deref(), Some("beta.1"));

        assert!(SkillVersion::parse("invalid").is_err());
        assert!(SkillVersion::parse("1.2").is_err());
    }

    #[test]
    fn test_version_display() {
        let v1 = SkillVersion::new(1, 2, 3, "Initial release");
        assert_eq!(v1.to_string(), "1.2.3");

        let v2 = SkillVersion::new_prerelease(2, 0, 0, "alpha.1", "Alpha release");
        assert_eq!(v2.to_string(), "2.0.0-alpha.1");
    }

    #[test]
    fn test_version_compatibility() {
        let v1_0_0 = SkillVersion::parse("1.0.0").unwrap();
        let v1_2_0 = SkillVersion::parse("1.2.0").unwrap();
        let v1_2_3 = SkillVersion::parse("1.2.3").unwrap();
        let v2_0_0 = SkillVersion::parse("2.0.0").unwrap();

        // 1.2.3 is compatible with 1.0.0 (same major, higher minor.patch)
        assert!(v1_2_3.is_compatible(&v1_0_0));

        // 1.2.3 is compatible with 1.2.0 (same major.minor, higher patch)
        assert!(v1_2_3.is_compatible(&v1_2_0));

        // 1.0.0 is not compatible with 1.2.0 (lower minor)
        assert!(!v1_0_0.is_compatible(&v1_2_0));

        // 2.0.0 is not compatible with 1.x.x (different major)
        assert!(!v2_0_0.is_compatible(&v1_2_3));
        assert!(!v1_2_3.is_compatible(&v2_0_0));

        // Pre-releases only match exact versions
        let v1_2_3_beta = SkillVersion::parse("1.2.3-beta.1").unwrap();
        assert!(!v1_2_3_beta.is_compatible(&v1_2_3));
        assert!(v1_2_3_beta.is_compatible(&v1_2_3_beta));
    }

    #[test]
    fn test_breaking_change() {
        let v1_0_0 = SkillVersion::parse("1.0.0").unwrap();
        let v1_2_0 = SkillVersion::parse("1.2.0").unwrap();
        let v2_0_0 = SkillVersion::parse("2.0.0").unwrap();

        assert!(!v1_2_0.is_breaking_change(&v1_0_0));
        assert!(v2_0_0.is_breaking_change(&v1_2_0));
    }

    #[test]
    fn test_version_deprecation() {
        let mut v = SkillVersion::new(1, 0, 0, "Old version");
        assert!(!v.deprecated);

        v.deprecate("Use 2.0.0 instead", Some("2.0.0"));
        assert!(v.deprecated);
        assert_eq!(v.deprecation_notice.as_deref(), Some("Use 2.0.0 instead"));
        assert_eq!(v.replacement_version.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn test_upgrade_path() {
        let path = UpgradePath::new(
            "1.0.0",
            "2.0.0",
            vec!["API signature changed".to_string()],
            "Update your code to use new API",
        );

        assert!(!path.automatic);
        assert!(path.requires_manual_intervention());
        assert_eq!(path.breaking_changes.len(), 1);

        let auto_path = UpgradePath::new("1.0.0", "1.1.0", vec![], "No breaking changes");
        assert!(auto_path.automatic);
        assert!(!auto_path.requires_manual_intervention());
    }

    #[test]
    fn test_version_registry() {
        let mut registry = VersionRegistry::new("my-skill");

        let v1 = SkillVersion::new(1, 0, 0, "Initial release");
        let v2 = SkillVersion::new(1, 1, 0, "Feature update");
        let v3 = SkillVersion::new(2, 0, 0, "Breaking changes");

        registry.add_version(v1);
        registry.add_version(v2);
        registry.add_version(v3);

        // Latest stable should be 2.0.0
        assert_eq!(registry.latest_stable().unwrap().to_string(), "2.0.0");

        // List all versions
        let versions = registry.list_versions(false);
        assert_eq!(versions.len(), 3);

        // Find specific version
        assert!(registry.find_version("1.1.0").is_some());
        assert!(registry.find_version("3.0.0").is_none());
    }

    #[test]
    fn test_registry_deprecation() {
        let mut registry = VersionRegistry::new("my-skill");
        let v1 = SkillVersion::new(1, 0, 0, "Old version");
        registry.add_version(v1);

        // Deprecate version
        registry
            .deprecate_version("1.0.0", "Use 2.0.0", Some("2.0.0"))
            .unwrap();

        let version = registry.find_version("1.0.0").unwrap();
        assert!(version.deprecated);

        // Cannot deprecate again
        assert!(
            registry
                .deprecate_version("1.0.0", "Already deprecated", None::<&str>)
                .is_err()
        );

        // Cannot deprecate non-existent version
        assert!(
            registry
                .deprecate_version("3.0.0", "N/A", None::<&str>)
                .is_err()
        );
    }

    #[test]
    fn test_registry_upgrade_paths() {
        let mut registry = VersionRegistry::new("my-skill");

        let path = UpgradePath::new(
            "1.0.0",
            "2.0.0",
            vec!["Breaking change".to_string()],
            "Migration guide",
        );
        registry.add_upgrade_path(path);

        let found = registry.get_upgrade_path("1.0.0", "2.0.0");
        assert!(found.is_some());
        assert_eq!(found.unwrap().breaking_changes.len(), 1);

        assert!(registry.get_upgrade_path("1.0.0", "3.0.0").is_none());
    }

    #[test]
    fn test_version_ordering() {
        let v1 = SkillVersion::parse("1.0.0").unwrap();
        let v2 = SkillVersion::parse("1.2.0").unwrap();
        let v3 = SkillVersion::parse("2.0.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 > v1);
    }

    #[test]
    fn test_latest_with_deprecated() {
        let mut registry = VersionRegistry::new("my-skill");

        let mut v1 = SkillVersion::new(1, 0, 0, "Old");
        v1.deprecate("Use 2.0.0", Some("2.0.0"));
        let v2 = SkillVersion::new(2, 0, 0, "Current");

        registry.add_version(v1);
        registry.add_version(v2);

        // Latest should skip deprecated
        assert_eq!(registry.latest().unwrap().to_string(), "2.0.0");

        // But list_versions with include_deprecated should show both
        assert_eq!(registry.list_versions(true).len(), 2);
        assert_eq!(registry.list_versions(false).len(), 1);
    }
}
