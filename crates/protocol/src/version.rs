//! Schema versioning with semver semantics.
//!
//! Every job type has a schema version that follows semantic versioning:
//! - **Patch** (1.0.0 → 1.0.1): Bug fixes, no schema changes
//! - **Minor** (1.0.0 → 1.1.0): New optional fields, backward compatible
//! - **Major** (v1 → v2): Breaking changes → new job type name

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur during version parsing.
#[derive(Debug, Error)]
pub enum VersionError {
    /// Invalid version format.
    #[error("invalid version format: {0}")]
    InvalidFormat(String),
}

/// A semantic version for job schemas.
///
/// # Example
///
/// ```
/// use protocol::version::SchemaVersion;
///
/// let v1 = SchemaVersion::new(1, 0, 0);
/// let v2 = SchemaVersion::new(1, 1, 0);
///
/// assert!(v1.is_compatible_with(&v2)); // Minor bump is compatible
/// assert!(v2.is_compatible_with(&v1)); // Backward compatible
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SchemaVersion {
    /// Create a new schema version.
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse a version string (e.g., "1.0.0").
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        Self::from_str(s)
    }

    /// Check if this version is compatible with another.
    ///
    /// Compatibility rules:
    /// - Same major version → compatible (minor/patch differences ok)
    /// - Different major version → incompatible
    pub fn is_compatible_with(&self, other: &SchemaVersion) -> bool {
        self.major == other.major
    }

    /// Check if this version can read data from an older version.
    ///
    /// A newer version can read older data if:
    /// - Same major version
    /// - This version is >= the other version
    pub fn can_read(&self, other: &SchemaVersion) -> bool {
        self.major == other.major && self >= other
    }

    /// Check if this version can write data readable by an older version.
    ///
    /// A version can write backward-compatible data if:
    /// - Same major version
    pub fn can_write_for(&self, other: &SchemaVersion) -> bool {
        self.major == other.major
    }
}

impl Default for SchemaVersion {
    fn default() -> Self {
        Self::new(1, 0, 0)
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for SchemaVersion {
    type Err = VersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(VersionError::InvalidFormat(format!(
                "expected 3 parts, got {}",
                parts.len()
            )));
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| VersionError::InvalidFormat(format!("invalid major: {}", parts[0])))?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| VersionError::InvalidFormat(format!("invalid minor: {}", parts[1])))?;
        let patch = parts[2]
            .parse::<u32>()
            .map_err(|_| VersionError::InvalidFormat(format!("invalid patch: {}", parts[2])))?;

        Ok(Self::new(major, minor, patch))
    }
}

impl PartialOrd for SchemaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SchemaVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => self.patch.cmp(&other.patch),
                ord => ord,
            },
            ord => ord,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        let v = SchemaVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_display_version() {
        let v = SchemaVersion::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_version_ordering() {
        let v100 = SchemaVersion::new(1, 0, 0);
        let v101 = SchemaVersion::new(1, 0, 1);
        let v110 = SchemaVersion::new(1, 1, 0);
        let v200 = SchemaVersion::new(2, 0, 0);

        assert!(v100 < v101);
        assert!(v101 < v110);
        assert!(v110 < v200);
    }

    #[test]
    fn test_compatibility_same_major() {
        let v100 = SchemaVersion::new(1, 0, 0);
        let v110 = SchemaVersion::new(1, 1, 0);
        let v111 = SchemaVersion::new(1, 1, 1);

        assert!(v100.is_compatible_with(&v110));
        assert!(v110.is_compatible_with(&v111));
        assert!(v100.is_compatible_with(&v111));
    }

    #[test]
    fn test_compatibility_different_major() {
        let v1 = SchemaVersion::new(1, 0, 0);
        let v2 = SchemaVersion::new(2, 0, 0);

        assert!(!v1.is_compatible_with(&v2));
        assert!(!v2.is_compatible_with(&v1));
    }

    #[test]
    fn test_can_read() {
        let v110 = SchemaVersion::new(1, 1, 0);
        let v100 = SchemaVersion::new(1, 0, 0);
        let v200 = SchemaVersion::new(2, 0, 0);

        // Newer can read older
        assert!(v110.can_read(&v100));
        // Older cannot read newer
        assert!(!v100.can_read(&v110));
        // Different major cannot read
        assert!(!v200.can_read(&v110));
    }

    #[test]
    fn test_invalid_version_format() {
        assert!(SchemaVersion::parse("1.0").is_err());
        assert!(SchemaVersion::parse("1.0.0.0").is_err());
        assert!(SchemaVersion::parse("a.b.c").is_err());
        assert!(SchemaVersion::parse("").is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let v = SchemaVersion::new(1, 2, 3);
        let json = serde_json::to_string(&v).unwrap();
        let parsed: SchemaVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, parsed);
    }
}
