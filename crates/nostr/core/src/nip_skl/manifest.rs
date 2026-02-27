//! SKL Skill Manifest (`kind:33400`).

use crate::nip01::{Event, EventTemplate};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

/// SKL skill manifest kind.
pub const KIND_SKILL_MANIFEST: u16 = 33400;

/// Required topic tag for SKL manifests.
pub const TOPIC_AGENT_SKILL: &str = "agent-skill";

/// Errors for SKL manifest validation and conversion.
#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("missing required tag: {0}")]
    MissingRequiredTag(&'static str),

    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid manifest hash; expected 64 lowercase hex chars")]
    InvalidManifestHash,

    #[error("invalid expiry timestamp")]
    InvalidExpiry,

    #[error("invalid capability declaration: {0}")]
    InvalidCapabilities(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),
}

/// SKL `kind:33400` manifest model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Stable skill slug (`d` tag).
    pub identifier: String,
    /// Human-readable name.
    pub name: String,
    /// Semantic version.
    pub version: String,
    /// Short summary.
    pub description: String,
    /// SHA-256 hash of canonical payload bytes.
    pub manifest_hash: String,
    /// Declared capabilities.
    pub capabilities: Vec<String>,
    /// Unix timestamp after which manifest is stale.
    pub expiry: u64,
    /// Optional canonical author npub.
    pub author_npub: Option<String>,
    /// Optional canonical author pubkey.
    pub author_pubkey: Option<String>,
    /// Optional previous manifest event id.
    pub previous_manifest_event_id: Option<String>,
    /// Event content field.
    pub content: String,
    /// Optional extra topics (`t` tags), excluding `agent-skill`.
    pub topics: Vec<String>,
}

impl SkillManifest {
    /// Create a new SKL manifest.
    pub fn new(
        identifier: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
        manifest_hash: impl Into<String>,
        capabilities: Vec<String>,
        expiry: u64,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            name: name.into(),
            version: version.into(),
            description: description.into(),
            manifest_hash: manifest_hash.into(),
            capabilities,
            expiry,
            author_npub: None,
            author_pubkey: None,
            previous_manifest_event_id: None,
            content: String::new(),
            topics: Vec::new(),
        }
    }

    /// Add optional author npub.
    pub fn with_author_npub(mut self, author_npub: impl Into<String>) -> Self {
        self.author_npub = Some(author_npub.into());
        self
    }

    /// Add optional author pubkey (`p` tag).
    pub fn with_author_pubkey(mut self, author_pubkey: impl Into<String>) -> Self {
        self.author_pubkey = Some(author_pubkey.into());
        self
    }

    /// Set previous manifest event id (`v` tag).
    pub fn with_previous_manifest_event(mut self, event_id: impl Into<String>) -> Self {
        self.previous_manifest_event_id = Some(event_id.into());
        self
    }

    /// Set event content.
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Add an additional topic (`t` tag).
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    /// Canonical `skill_scope_id` for this manifest + version.
    pub fn skill_scope_id(&self, publisher_pubkey: &str) -> String {
        format!(
            "{}:{}:{}:{}",
            KIND_SKILL_MANIFEST, publisher_pubkey, self.identifier, self.version
        )
    }

    /// Verify payload bytes against the declared `manifest_hash`.
    pub fn verify_payload_hash(&self, payload: &[u8]) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let hash = hex::encode(hasher.finalize());
        hash == self.manifest_hash
    }

    /// Enforce that all requested capabilities are declared by the manifest.
    pub fn enforce_declared_capabilities(&self, requested: &[String]) -> Result<(), ManifestError> {
        let declared = self.canonical_capabilities()?;
        if declared.len() == 1 && declared[0] == "none" {
            if requested.is_empty() {
                return Ok(());
            }
            return Err(ManifestError::InvalidCapabilities(
                "manifest declares capability:none but runtime requested capabilities".to_string(),
            ));
        }

        let declared_set: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
        let missing: Vec<String> = requested
            .iter()
            .filter(|cap| !declared_set.contains(cap.as_str()))
            .cloned()
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(ManifestError::InvalidCapabilities(format!(
                "requested undeclared capabilities: {}",
                missing.join(",")
            )))
        }
    }

    /// Validate manifest fields.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.identifier.trim().is_empty() {
            return Err(ManifestError::MissingRequiredTag("d"));
        }
        if self.name.trim().is_empty() {
            return Err(ManifestError::MissingRequiredTag("name"));
        }
        if self.version.trim().is_empty() {
            return Err(ManifestError::MissingRequiredTag("version"));
        }
        if self.description.trim().is_empty() {
            return Err(ManifestError::MissingRequiredTag("description"));
        }
        if self.expiry == 0 {
            return Err(ManifestError::InvalidExpiry);
        }
        if !is_lower_hex_64(&self.manifest_hash) {
            return Err(ManifestError::InvalidManifestHash);
        }

        self.canonical_capabilities()?;
        Ok(())
    }

    /// Deterministic tag encoding for `kind:33400`.
    pub fn to_tags(&self, publisher_pubkey: &str) -> Result<Vec<Vec<String>>, ManifestError> {
        self.validate()?;

        let mut tags = vec![
            vec!["d".to_string(), self.identifier.clone()],
            vec!["name".to_string(), self.name.clone()],
            vec!["version".to_string(), self.version.clone()],
            vec!["description".to_string(), self.description.clone()],
            vec!["manifest_hash".to_string(), self.manifest_hash.clone()],
        ];

        for capability in self.canonical_capabilities()? {
            tags.push(vec!["capability".to_string(), capability]);
        }

        tags.push(vec!["expiry".to_string(), self.expiry.to_string()]);
        tags.push(vec!["t".to_string(), TOPIC_AGENT_SKILL.to_string()]);

        if let Some(author_npub) = &self.author_npub {
            tags.push(vec!["author_npub".to_string(), author_npub.clone()]);
        }
        if let Some(author_pubkey) = &self.author_pubkey {
            tags.push(vec!["p".to_string(), author_pubkey.clone()]);
        }

        tags.push(vec![
            "skill_scope_id".to_string(),
            self.skill_scope_id(publisher_pubkey),
        ]);

        if let Some(previous_event_id) = &self.previous_manifest_event_id {
            tags.push(vec!["v".to_string(), previous_event_id.clone()]);
        }

        for topic in self.canonical_topics() {
            tags.push(vec!["t".to_string(), topic]);
        }

        Ok(tags)
    }

    /// Convert to unsigned event template.
    pub fn to_event_template(
        &self,
        publisher_pubkey: &str,
        created_at: u64,
    ) -> Result<EventTemplate, ManifestError> {
        Ok(EventTemplate {
            created_at,
            kind: KIND_SKILL_MANIFEST,
            tags: self.to_tags(publisher_pubkey)?,
            content: self.content.clone(),
        })
    }

    /// Parse a manifest from an event.
    pub fn from_event(event: &Event) -> Result<Self, ManifestError> {
        if event.kind != KIND_SKILL_MANIFEST {
            return Err(ManifestError::InvalidKind {
                expected: KIND_SKILL_MANIFEST,
                actual: event.kind,
            });
        }

        let identifier = find_required_tag_value(&event.tags, "d")?;
        let name = find_required_tag_value(&event.tags, "name")?;
        let version = find_required_tag_value(&event.tags, "version")?;
        let description = find_required_tag_value(&event.tags, "description")?;
        let manifest_hash = find_required_tag_value(&event.tags, "manifest_hash")?;
        let expiry = find_required_tag_value(&event.tags, "expiry")?
            .parse::<u64>()
            .map_err(|_| ManifestError::InvalidExpiry)?;

        let capabilities = find_repeated_tag_values(&event.tags, "capability");
        let author_npub = find_first_tag_value(&event.tags, "author_npub");
        let author_pubkey = find_first_tag_value(&event.tags, "p");
        let previous_manifest_event_id = find_first_tag_value(&event.tags, "v");

        let mut manifest = SkillManifest::new(
            identifier,
            name,
            version,
            description,
            manifest_hash,
            capabilities,
            expiry,
        )
        .with_content(event.content.clone());

        manifest.author_npub = author_npub;
        manifest.author_pubkey = author_pubkey;
        manifest.previous_manifest_event_id = previous_manifest_event_id;

        manifest.topics = find_repeated_tag_values(&event.tags, "t")
            .into_iter()
            .filter(|topic| topic != TOPIC_AGENT_SKILL)
            .collect();

        manifest.validate()?;
        Ok(manifest)
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, ManifestError> {
        serde_json::to_string(self).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        let manifest: SkillManifest = serde_json::from_str(json)
            .map_err(|e| ManifestError::Deserialization(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn canonical_capabilities(&self) -> Result<Vec<String>, ManifestError> {
        let mut normalized: Vec<String> = if self.capabilities.is_empty() {
            vec!["none".to_string()]
        } else {
            self.capabilities
                .iter()
                .map(|c| c.trim().to_lowercase())
                .filter(|c| !c.is_empty())
                .collect()
        };

        normalized.sort();
        normalized.dedup();

        if normalized.contains(&"none".to_string()) && normalized.len() > 1 {
            return Err(ManifestError::InvalidCapabilities(
                "capability:none cannot be combined with other capabilities".to_string(),
            ));
        }

        if normalized.is_empty() {
            return Err(ManifestError::InvalidCapabilities(
                "at least one capability is required".to_string(),
            ));
        }

        Ok(normalized)
    }

    fn canonical_topics(&self) -> Vec<String> {
        let mut topics: Vec<String> = self
            .topics
            .iter()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty() && t != TOPIC_AGENT_SKILL)
            .collect();
        topics.sort();
        topics.dedup();
        topics
    }
}

fn find_required_tag_value(
    tags: &[Vec<String>],
    tag_name: &'static str,
) -> Result<String, ManifestError> {
    find_first_tag_value(tags, tag_name).ok_or(ManifestError::MissingRequiredTag(tag_name))
}

fn find_first_tag_value(tags: &[Vec<String>], tag_name: &str) -> Option<String> {
    tags.iter()
        .find(|tag| tag.first().map(String::as_str) == Some(tag_name))
        .and_then(|tag| tag.get(1))
        .cloned()
}

fn find_repeated_tag_values(tags: &[Vec<String>], tag_name: &str) -> Vec<String> {
    tags.iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(tag_name))
        .filter_map(|tag| tag.get(1))
        .cloned()
        .collect()
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
    fn test_manifest_validation() {
        let manifest = SkillManifest::new(
            "research-assistant",
            "Research Assistant",
            "1.0.0",
            "Summarizes technical research",
            HASH,
            vec!["filesystem:read".to_string(), "http:outbound".to_string()],
            1_756_000_000,
        );

        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_manifest_capability_none_cannot_mix() {
        let manifest = SkillManifest::new(
            "skill",
            "Skill",
            "1.0.0",
            "Description",
            HASH,
            vec!["none".to_string(), "http:outbound".to_string()],
            1_756_000_000,
        );

        let err = manifest.validate().unwrap_err();
        assert!(matches!(err, ManifestError::InvalidCapabilities(_)));
    }

    #[test]
    fn test_manifest_tags_are_deterministic() {
        let manifest = SkillManifest::new(
            "research-assistant",
            "Research Assistant",
            "1.4.2",
            "Summarize technical docs",
            HASH,
            vec!["filesystem:read".to_string(), "http:outbound".to_string()],
            1_756_000_000,
        )
        .with_topic("research")
        .with_topic("agent-skill")
        .with_author_npub("npub1author")
        .with_author_pubkey("authorpubkey");

        let tags_a = manifest.to_tags("publisherpubkey").unwrap();
        let tags_b = manifest.to_tags("publisherpubkey").unwrap();
        assert_eq!(tags_a, tags_b);
        assert!(
            tags_a
                .iter()
                .any(|tag| tag[0] == "skill_scope_id"
                    && tag[1].contains(":research-assistant:1.4.2"))
        );
    }

    #[test]
    fn test_manifest_enforces_declared_capabilities() {
        let manifest = SkillManifest::new(
            "research-assistant",
            "Research Assistant",
            "1.0.0",
            "Summarizer",
            HASH,
            vec!["filesystem:read".to_string(), "http:outbound".to_string()],
            1_756_000_000,
        );

        assert!(
            manifest
                .enforce_declared_capabilities(&["filesystem:read".to_string()])
                .is_ok()
        );
        assert!(
            manifest
                .enforce_declared_capabilities(&["filesystem:write".to_string()])
                .is_err()
        );
    }

    #[test]
    fn test_manifest_round_trip_from_event() {
        let manifest = SkillManifest::new(
            "research-assistant",
            "Research Assistant",
            "1.0.0",
            "Summarizer",
            HASH,
            vec!["http:outbound".to_string()],
            1_756_000_000,
        )
        .with_content("payload notes")
        .with_previous_manifest_event("previous-event-id");

        let template = manifest
            .to_event_template("publisherpubkey", 1_740_400_000)
            .unwrap();
        let event = Event {
            id: "event-id".to_string(),
            pubkey: "publisherpubkey".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "signature".to_string(),
        };

        let parsed = SkillManifest::from_event(&event).unwrap();
        assert_eq!(parsed.identifier, "research-assistant");
        assert_eq!(parsed.version, "1.0.0");
        assert_eq!(parsed.manifest_hash, HASH);
        assert_eq!(
            parsed.previous_manifest_event_id,
            Some("previous-event-id".to_string())
        );
    }

    #[test]
    fn test_manifest_verify_payload_hash() {
        let payload = b"skill payload";
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let hash = hex::encode(hasher.finalize());

        let manifest = SkillManifest::new(
            "skill",
            "Skill",
            "1.0.0",
            "Description",
            hash,
            vec!["none".to_string()],
            1_756_000_000,
        );

        assert!(manifest.verify_payload_hash(payload));
        assert!(!manifest.verify_payload_hash(b"other payload"));
    }
}
