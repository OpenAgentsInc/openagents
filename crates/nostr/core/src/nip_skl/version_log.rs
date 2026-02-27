//! SKL Skill Version Log (`kind:33401`).

use crate::nip01::{Event, EventTemplate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// SKL skill version log kind.
pub const KIND_SKILL_VERSION_LOG: u16 = 33401;

/// Errors for SKL version log operations.
#[derive(Debug, Error)]
pub enum VersionLogError {
    #[error("missing required tag: {0}")]
    MissingRequiredTag(&'static str),

    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid manifest hash; expected 64 lowercase hex chars")]
    InvalidManifestHash,

    #[error("invalid change type: {0}")]
    InvalidChangeType(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),
}

/// Change type for SKL version-log entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Added,
    Changed,
    Fixed,
    Deprecated,
    Security,
}

impl ChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Fixed => "fixed",
            Self::Deprecated => "deprecated",
            Self::Security => "security",
        }
    }

    pub fn from_str(value: &str) -> Result<Self, VersionLogError> {
        match value.to_lowercase().as_str() {
            "added" => Ok(Self::Added),
            "changed" => Ok(Self::Changed),
            "fixed" => Ok(Self::Fixed),
            "deprecated" => Ok(Self::Deprecated),
            "security" => Ok(Self::Security),
            _ => Err(VersionLogError::InvalidChangeType(value.to_string())),
        }
    }
}

/// SKL `kind:33401` version-log entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillVersionLog {
    /// `d` tag (same as manifest identifier).
    pub identifier: String,
    /// Current semantic version.
    pub version: String,
    /// Optional previous version.
    pub previous_version: Option<String>,
    /// Referenced manifest event id.
    pub manifest_event_id: String,
    /// Referenced manifest hash.
    pub manifest_hash: String,
    /// Declared change type.
    pub change_type: ChangeType,
    /// Event content.
    pub content: String,
}

impl SkillVersionLog {
    /// Create a version-log entry.
    pub fn new(
        identifier: impl Into<String>,
        version: impl Into<String>,
        manifest_event_id: impl Into<String>,
        manifest_hash: impl Into<String>,
        change_type: ChangeType,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            version: version.into(),
            previous_version: None,
            manifest_event_id: manifest_event_id.into(),
            manifest_hash: manifest_hash.into(),
            change_type,
            content: String::new(),
        }
    }

    /// Set previous version.
    pub fn with_previous_version(mut self, previous_version: impl Into<String>) -> Self {
        self.previous_version = Some(previous_version.into());
        self
    }

    /// Set content.
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Validate fields.
    pub fn validate(&self) -> Result<(), VersionLogError> {
        if self.identifier.trim().is_empty() {
            return Err(VersionLogError::MissingRequiredTag("d"));
        }
        if self.version.trim().is_empty() {
            return Err(VersionLogError::MissingRequiredTag("version"));
        }
        if self.manifest_event_id.trim().is_empty() {
            return Err(VersionLogError::MissingRequiredTag("manifest_event"));
        }
        if !is_lower_hex_64(&self.manifest_hash) {
            return Err(VersionLogError::InvalidManifestHash);
        }
        Ok(())
    }

    /// Deterministic tag encoding.
    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, VersionLogError> {
        self.validate()?;

        let mut tags = vec![
            vec!["d".to_string(), self.identifier.clone()],
            vec!["version".to_string(), self.version.clone()],
        ];

        if let Some(previous_version) = &self.previous_version {
            tags.push(vec!["prev_version".to_string(), previous_version.clone()]);
        }

        tags.push(vec![
            "manifest_event".to_string(),
            self.manifest_event_id.clone(),
        ]);
        tags.push(vec![
            "manifest_hash".to_string(),
            self.manifest_hash.clone(),
        ]);
        tags.push(vec![
            "change_type".to_string(),
            self.change_type.as_str().to_string(),
        ]);

        Ok(tags)
    }

    /// Convert to unsigned event template.
    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, VersionLogError> {
        Ok(EventTemplate {
            created_at,
            kind: KIND_SKILL_VERSION_LOG,
            tags: self.to_tags()?,
            content: self.content.clone(),
        })
    }

    /// Parse from event.
    pub fn from_event(event: &Event) -> Result<Self, VersionLogError> {
        if event.kind != KIND_SKILL_VERSION_LOG {
            return Err(VersionLogError::InvalidKind {
                expected: KIND_SKILL_VERSION_LOG,
                actual: event.kind,
            });
        }

        let identifier = find_required_tag_value(&event.tags, "d")?;
        let version = find_required_tag_value(&event.tags, "version")?;
        let previous_version = find_first_tag_value(&event.tags, "prev_version");
        let manifest_event_id = find_required_tag_value(&event.tags, "manifest_event")?;
        let manifest_hash = find_required_tag_value(&event.tags, "manifest_hash")?;
        let change_type =
            ChangeType::from_str(&find_required_tag_value(&event.tags, "change_type")?)?;

        let version_log = SkillVersionLog {
            identifier,
            version,
            previous_version,
            manifest_event_id,
            manifest_hash,
            change_type,
            content: event.content.clone(),
        };

        version_log.validate()?;
        Ok(version_log)
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, VersionLogError> {
        serde_json::to_string(self).map_err(|e| VersionLogError::Serialization(e.to_string()))
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, VersionLogError> {
        let version_log: SkillVersionLog = serde_json::from_str(json)
            .map_err(|e| VersionLogError::Deserialization(e.to_string()))?;
        version_log.validate()?;
        Ok(version_log)
    }
}

fn find_required_tag_value(
    tags: &[Vec<String>],
    tag_name: &'static str,
) -> Result<String, VersionLogError> {
    find_first_tag_value(tags, tag_name).ok_or(VersionLogError::MissingRequiredTag(tag_name))
}

fn find_first_tag_value(tags: &[Vec<String>], tag_name: &str) -> Option<String> {
    tags.iter()
        .find(|tag| tag.first().map(String::as_str) == Some(tag_name))
        .and_then(|tag| tag.get(1))
        .cloned()
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn test_version_log_validation() {
        let log = SkillVersionLog::new(
            "research-assistant",
            "1.4.2",
            "manifest-event-id",
            HASH,
            ChangeType::Security,
        )
        .with_previous_version("1.4.1");

        assert!(log.validate().is_ok());
    }

    #[test]
    fn test_version_log_tags_deterministic() {
        let log = SkillVersionLog::new(
            "research-assistant",
            "1.4.2",
            "manifest-event-id",
            HASH,
            ChangeType::Changed,
        )
        .with_previous_version("1.4.1");

        let tags_a = log.to_tags().unwrap();
        let tags_b = log.to_tags().unwrap();
        assert_eq!(tags_a, tags_b);
    }

    #[test]
    fn test_version_log_round_trip_from_event() {
        let log = SkillVersionLog::new(
            "research-assistant",
            "1.4.2",
            "manifest-event-id",
            HASH,
            ChangeType::Security,
        )
        .with_previous_version("1.4.1")
        .with_content("Patched prompt-injection path");

        let template = log.to_event_template(1_740_400_500).unwrap();
        let event = Event {
            id: "event-id".to_string(),
            pubkey: "publisherpubkey".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "sig".to_string(),
        };

        let parsed = SkillVersionLog::from_event(&event).unwrap();
        assert_eq!(parsed.identifier, "research-assistant");
        assert_eq!(parsed.version, "1.4.2");
        assert_eq!(parsed.previous_version, Some("1.4.1".to_string()));
        assert_eq!(parsed.change_type, ChangeType::Security);
    }
}
