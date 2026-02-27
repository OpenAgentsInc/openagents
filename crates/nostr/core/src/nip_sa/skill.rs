//! Skill Events (kinds:39220, 39221)
//!
//! Skills are capabilities that agents can acquire, typically through purchase
//! from a marketplace. Skills are protected by licenses and delivered encrypted.
//!
//! ## Skill License (kind:39220)
//!
//! Addressable event issued by the marketplace when an agent purchases a skill.
//!
//! Tags:
//! - `["d", "<license-id>"]` - Unique license identifier
//! - `["p", "<agent-pubkey>"]` - Licensed agent
//! - `["a", "33400:<skill-pubkey>:<skill-d-tag>"]` - SKL canonical skill address
//! - `["e", "<skill-manifest-event-id>", "<relay-hint>"]` - Pinned manifest version
//! - `["version", "<skill-semver>"]` - Licensed manifest version
//! - `["expires_at", "1703000000"]` - Expiration timestamp (optional)
//! - `["price_sats", "1000"]` - Price paid in satoshis
//!
//! Content: License metadata
//!
//! ```json
//! {
//!   "skill_id": "skill-123",
//!   "skill_name": "web-scraper",
//!   "version": "1.0.0",
//!   "granted_at": 1703000000,
//!   "expires_at": null,
//!   "capabilities": ["fetch", "parse", "extract"],
//!   "restrictions": {
//!     "max_requests_per_day": 1000
//!   }
//! }
//! ```
//!
//! ## Skill Delivery (kind:39221)
//!
//! Ephemeral event that delivers the actual skill content to the agent.
//! Uses NIP-59 gift wrap for privacy.
//!
//! Tags:
//! - `["p", "<agent-pubkey>"]` - Recipient agent
//! - `["a", "33400:<skill-pubkey>:<skill-d-tag>"]` - SKL canonical skill address
//! - `["e", "<skill-manifest-event-id>", "<relay-hint>"]` - Pinned manifest version
//! - `["e", "<skill-license-event-id>", "<relay-hint>"]` - License event reference
//!
//! The skill content is encrypted to the agent's pubkey. Decryption requires
//! threshold ECDH with the marketplace signer, which verifies the license
//! before participating.
//!
//! Content: Encrypted skill data (code, prompts, etc.)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Kind for skill license event
pub const KIND_SKILL_LICENSE: u16 = 39220;

/// Kind for skill delivery event
pub const KIND_SKILL_DELIVERY: u16 = 39221;

/// Errors that can occur during NIP-SA skill operations
#[derive(Debug, Error)]
pub enum SkillError {
    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("license expired: {0}")]
    LicenseExpired(String),

    #[error("invalid hash: {0}")]
    InvalidHash(String),

    #[error("missing capability: {0}")]
    MissingCapability(String),

    #[error("invalid SKL reference: {0}")]
    InvalidReference(String),
}

/// Skill license metadata (stored in content field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLicenseContent {
    /// Unique skill identifier
    pub skill_id: String,
    /// Skill name
    pub skill_name: String,
    /// Skill version
    pub version: String,
    /// When license was granted (Unix seconds)
    pub granted_at: u64,
    /// When license expires (Unix seconds, None = perpetual)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Capabilities provided by this skill
    pub capabilities: Vec<String>,
    /// Usage restrictions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restrictions: Option<HashMap<String, serde_json::Value>>,
}

impl SkillLicenseContent {
    /// Create new skill license content
    pub fn new(
        skill_id: impl Into<String>,
        skill_name: impl Into<String>,
        version: impl Into<String>,
        granted_at: u64,
        capabilities: Vec<String>,
    ) -> Self {
        Self {
            skill_id: skill_id.into(),
            skill_name: skill_name.into(),
            version: version.into(),
            granted_at,
            expires_at: None,
            capabilities,
            restrictions: None,
        }
    }

    /// Set expiration timestamp
    pub fn with_expires_at(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Set restrictions
    pub fn with_restrictions(mut self, restrictions: HashMap<String, serde_json::Value>) -> Self {
        self.restrictions = Some(restrictions);
        self
    }

    /// Check if license is expired
    pub fn is_expired(&self, current_time: u64) -> bool {
        self.expires_at.is_some_and(|exp| current_time >= exp)
    }

    /// Check if license has a specific capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, SkillError> {
        serde_json::to_string(self).map_err(|e| SkillError::Serialization(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, SkillError> {
        serde_json::from_str(json).map_err(|e| SkillError::Deserialization(e.to_string()))
    }
}

/// Skill delivery content (encrypted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDeliveryContent {
    /// Skill identifier (matches license)
    pub skill_id: String,
    /// Skill code/prompts/data
    pub content: String,
    /// Content type (rust, python, prompt, etc.)
    pub content_type: String,
    /// SHA-256 hash of content
    pub content_hash: String,
}

impl SkillDeliveryContent {
    /// Create new skill delivery content
    pub fn new(
        skill_id: impl Into<String>,
        content: impl Into<String>,
        content_type: impl Into<String>,
        content_hash: impl Into<String>,
    ) -> Self {
        Self {
            skill_id: skill_id.into(),
            content: content.into(),
            content_type: content_type.into(),
            content_hash: content_hash.into(),
        }
    }

    /// Verify content hash
    pub fn verify_hash(&self, expected_hash: &str) -> Result<(), SkillError> {
        if self.content_hash != expected_hash {
            return Err(SkillError::InvalidHash(format!(
                "expected {} but got {}",
                expected_hash, self.content_hash
            )));
        }
        Ok(())
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, SkillError> {
        serde_json::to_string(self).map_err(|e| SkillError::Serialization(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, SkillError> {
        serde_json::from_str(json).map_err(|e| SkillError::Deserialization(e.to_string()))
    }
}

/// Skill license event wrapper
#[derive(Debug, Clone)]
pub struct SkillLicense {
    /// License content
    pub content: SkillLicenseContent,
    /// Licensed agent pubkey
    pub agent_pubkey: String,
    /// SKL canonical skill address (`33400:<skill-pubkey>:<d-tag>`)
    pub skill_address: String,
    /// Pinned SKL manifest event id
    pub manifest_event_id: String,
    /// Optional relay hint for manifest event
    pub manifest_relay_hint: Option<String>,
    /// Price paid in satoshis
    pub price_sats: u64,
}

impl SkillLicense {
    /// Create new skill license
    pub fn new(
        content: SkillLicenseContent,
        agent_pubkey: impl Into<String>,
        skill_address: impl Into<String>,
        manifest_event_id: impl Into<String>,
        price_sats: u64,
    ) -> Self {
        Self {
            content,
            agent_pubkey: agent_pubkey.into(),
            skill_address: skill_address.into(),
            manifest_event_id: manifest_event_id.into(),
            manifest_relay_hint: None,
            price_sats,
        }
    }

    /// Set relay hint for pinned manifest event
    pub fn with_manifest_relay_hint(mut self, relay_hint: impl Into<String>) -> Self {
        self.manifest_relay_hint = Some(relay_hint.into());
        self
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.content.skill_id.clone()],
            vec!["p".to_string(), self.agent_pubkey.clone()],
            vec!["a".to_string(), self.skill_address.clone()],
            vec!["e".to_string(), self.manifest_event_id.clone()],
            vec!["version".to_string(), self.content.version.clone()],
            vec![
                "licensed_at".to_string(),
                self.content.granted_at.to_string(),
            ],
            vec!["price_sats".to_string(), self.price_sats.to_string()],
        ];

        if let Some(relay_hint) = &self.manifest_relay_hint {
            tags[3].push(relay_hint.clone());
        }

        if let Some(expires_at) = self.content.expires_at {
            tags.push(vec!["expires_at".to_string(), expires_at.to_string()]);
        }

        tags
    }

    /// Validate the license
    pub fn validate(&self, current_time: u64) -> Result<(), SkillError> {
        self.validate_skl_references()?;
        if self.content.is_expired(current_time) {
            let expires_at = self.content.expires_at.unwrap_or_default();
            return Err(SkillError::LicenseExpired(format!(
                "license for {} expired at {}",
                self.content.skill_id,
                expires_at
            )));
        }
        Ok(())
    }

    /// Validate SKL canonical references carried by the license.
    pub fn validate_skl_references(&self) -> Result<(), SkillError> {
        if !self.skill_address.starts_with("33400:") {
            return Err(SkillError::InvalidReference(format!(
                "license skill address must start with 33400:, got {}",
                self.skill_address
            )));
        }
        if self.manifest_event_id.trim().is_empty() {
            return Err(SkillError::InvalidReference(
                "manifest event id cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Skill delivery event wrapper
#[derive(Debug, Clone)]
pub struct SkillDelivery {
    /// Delivery content
    pub content: SkillDeliveryContent,
    /// Recipient agent pubkey
    pub agent_pubkey: String,
    /// SKL canonical skill address (`33400:<skill-pubkey>:<d-tag>`)
    pub skill_address: String,
    /// Pinned SKL manifest event id
    pub manifest_event_id: String,
    /// Optional relay hint for manifest event
    pub manifest_relay_hint: Option<String>,
    /// License event ID
    pub license_id: String,
    /// Optional relay hint for license event
    pub license_relay_hint: Option<String>,
}

impl SkillDelivery {
    /// Create new skill delivery
    pub fn new(
        content: SkillDeliveryContent,
        agent_pubkey: impl Into<String>,
        skill_address: impl Into<String>,
        manifest_event_id: impl Into<String>,
        license_id: impl Into<String>,
    ) -> Self {
        Self {
            content,
            agent_pubkey: agent_pubkey.into(),
            skill_address: skill_address.into(),
            manifest_event_id: manifest_event_id.into(),
            manifest_relay_hint: None,
            license_id: license_id.into(),
            license_relay_hint: None,
        }
    }

    /// Set relay hint for pinned manifest event
    pub fn with_manifest_relay_hint(mut self, relay_hint: impl Into<String>) -> Self {
        self.manifest_relay_hint = Some(relay_hint.into());
        self
    }

    /// Set relay hint for license event
    pub fn with_license_relay_hint(mut self, relay_hint: impl Into<String>) -> Self {
        self.license_relay_hint = Some(relay_hint.into());
        self
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut manifest_ref = vec!["e".to_string(), self.manifest_event_id.clone()];
        if let Some(relay_hint) = &self.manifest_relay_hint {
            manifest_ref.push(relay_hint.clone());
        }

        let mut license_ref = vec!["e".to_string(), self.license_id.clone()];
        if let Some(relay_hint) = &self.license_relay_hint {
            license_ref.push(relay_hint.clone());
        }

        vec![
            vec!["p".to_string(), self.agent_pubkey.clone()],
            vec!["a".to_string(), self.skill_address.clone()],
            manifest_ref,
            license_ref,
            vec!["type".to_string(), self.content.content_type.clone()],
            vec!["hash".to_string(), self.content.content_hash.clone()],
        ]
    }

    /// Validate SKL canonical references carried by the delivery.
    pub fn validate_skl_references(&self) -> Result<(), SkillError> {
        if !self.skill_address.starts_with("33400:") {
            return Err(SkillError::InvalidReference(format!(
                "delivery skill address must start with 33400:, got {}",
                self.skill_address
            )));
        }
        if self.manifest_event_id.trim().is_empty() {
            return Err(SkillError::InvalidReference(
                "manifest event id cannot be empty".to_string(),
            ));
        }
        if self.license_id.trim().is_empty() {
            return Err(SkillError::InvalidReference(
                "license event id cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Trust-gated fulfillment decision for SA skill delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FulfillmentGateOutcome {
    pub allowed: bool,
    pub reason: String,
}

impl FulfillmentGateOutcome {
    fn allow() -> Self {
        Self {
            allowed: true,
            reason: "allowed".to_string(),
        }
    }

    fn deny(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: reason.into(),
        }
    }
}

/// Evaluate SA skill fulfillment gate using SKL manifest validation, trust policy,
/// and revocation state.
///
/// Deny conditions:
/// - license/delivery SKL references are malformed
/// - license/delivery references are inconsistent with each other
/// - provided manifest event is invalid or mismatched against license/delivery refs
/// - trust policy does not return `Trusted`
/// - manifest has authoritative publisher-origin revocation
pub fn evaluate_fulfillment_gate(
    license: &SkillLicense,
    delivery: &SkillDelivery,
    manifest_event: &crate::Event,
    label_events: &[crate::Event],
    deletion_events: &[crate::Event],
    trust_policy: &crate::nip_skl::TrustPolicy,
) -> FulfillmentGateOutcome {
    if let Err(error) = license.validate_skl_references() {
        return FulfillmentGateOutcome::deny(format!("license reference check failed: {}", error));
    }
    if let Err(error) = delivery.validate_skl_references() {
        return FulfillmentGateOutcome::deny(format!("delivery reference check failed: {}", error));
    }
    if license.skill_address != delivery.skill_address {
        return FulfillmentGateOutcome::deny(
            "license/delivery mismatch: skill address differs".to_string(),
        );
    }
    if license.manifest_event_id != delivery.manifest_event_id {
        return FulfillmentGateOutcome::deny(
            "license/delivery mismatch: manifest event id differs".to_string(),
        );
    }

    let manifest = match crate::nip_skl::SkillManifest::from_event(manifest_event) {
        Ok(manifest) => manifest,
        Err(error) => {
            return FulfillmentGateOutcome::deny(format!("manifest validation failed: {}", error));
        }
    };

    let canonical_address = format!("33400:{}:{}", manifest_event.pubkey, manifest.identifier);
    if license.skill_address != canonical_address || delivery.skill_address != canonical_address {
        return FulfillmentGateOutcome::deny(format!(
            "canonical address mismatch: expected {}",
            canonical_address
        ));
    }
    if license.manifest_event_id != manifest_event.id
        || delivery.manifest_event_id != manifest_event.id
    {
        return FulfillmentGateOutcome::deny("manifest event id mismatch".to_string());
    }

    let trust = crate::nip_skl::evaluate_skill_trust(
        &canonical_address,
        Some(&manifest_event.id),
        &manifest_event.pubkey,
        label_events,
        trust_policy,
    );
    if trust.decision != crate::nip_skl::TrustDecision::Trusted {
        return FulfillmentGateOutcome::deny(format!(
            "trust gate denied: {}",
            trust.reasons.join("; ")
        ));
    }

    let revocation = crate::nip_skl::manifest_revocation_status(
        &manifest_event.pubkey,
        &manifest_event.id,
        &canonical_address,
        deletion_events,
    );
    if revocation.revoked {
        return FulfillmentGateOutcome::deny(format!(
            "manifest revoked{}",
            revocation
                .reason
                .as_ref()
                .map(|reason| format!(": {}", reason))
                .unwrap_or_default()
        ));
    }

    FulfillmentGateOutcome::allow()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip_skl::{SkillManifest, TrustPolicy};

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn manifest_event() -> crate::Event {
        let manifest = SkillManifest::new(
            "web-scraper",
            "Web Scraper",
            "1.0.0",
            "Fetch and parse pages",
            HASH,
            vec!["http:outbound".to_string()],
            1_756_000_000,
        );
        let template = manifest
            .to_event_template("skill-pubkey", 1_703_000_000)
            .unwrap();
        crate::Event {
            id: "manifest-event-id".to_string(),
            pubkey: "skill-pubkey".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags,
            content: template.content,
            sig: "sig".to_string(),
        }
    }

    fn trusted_label_event(skill_address: &str, manifest_event_id: &str) -> crate::Event {
        crate::Event {
            id: "label-event-id".to_string(),
            pubkey: "auditor-pubkey".to_string(),
            created_at: 1_703_000_100,
            kind: crate::nip32::KIND_LABEL as u16,
            tags: vec![
                vec!["L".to_string(), "skill-security".to_string()],
                vec![
                    "l".to_string(),
                    "audit-passed".to_string(),
                    "skill-security".to_string(),
                ],
                vec!["a".to_string(), skill_address.to_string()],
                vec!["e".to_string(), manifest_event_id.to_string()],
            ],
            content: String::new(),
            sig: "sig".to_string(),
        }
    }

    fn revocation_event(skill_address: &str, manifest_event_id: &str) -> crate::Event {
        crate::Event {
            id: "delete-event-id".to_string(),
            pubkey: "skill-pubkey".to_string(),
            created_at: 1_703_000_200,
            kind: crate::nip09::DELETION_REQUEST_KIND,
            tags: vec![
                vec!["e".to_string(), manifest_event_id.to_string()],
                vec!["a".to_string(), skill_address.to_string()],
                vec!["k".to_string(), "33400".to_string()],
            ],
            content: "critical-vuln".to_string(),
            sig: "sig".to_string(),
        }
    }

    #[test]
    fn test_skill_license_content_creation() {
        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string(), "parse".to_string()],
        );

        assert_eq!(content.skill_id, "skill-123");
        assert_eq!(content.skill_name, "web-scraper");
        assert_eq!(content.version, "1.0.0");
        assert_eq!(content.granted_at, 1703000000);
        assert_eq!(content.expires_at, None);
        assert_eq!(content.capabilities.len(), 2);
    }

    #[test]
    fn test_skill_license_content_with_expiration() {
        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string()],
        )
        .with_expires_at(1703086400);

        assert_eq!(content.expires_at, Some(1703086400));
    }

    #[test]
    fn test_skill_license_content_with_restrictions() {
        let mut restrictions = HashMap::new();
        restrictions.insert(
            "max_requests_per_day".to_string(),
            serde_json::Value::Number(1000.into()),
        );

        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string()],
        )
        .with_restrictions(restrictions);

        assert!(content.restrictions.is_some());
    }

    #[test]
    fn test_skill_license_content_is_expired() {
        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string()],
        )
        .with_expires_at(1703086400);

        assert!(!content.is_expired(1703000000));
        assert!(content.is_expired(1703086400));
        assert!(content.is_expired(1703100000));
    }

    #[test]
    fn test_skill_license_content_perpetual() {
        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string()],
        );

        assert!(!content.is_expired(u64::MAX));
    }

    #[test]
    fn test_skill_license_content_has_capability() {
        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string(), "parse".to_string()],
        );

        assert!(content.has_capability("fetch"));
        assert!(content.has_capability("parse"));
        assert!(!content.has_capability("extract"));
    }

    #[test]
    fn test_skill_license_content_serialization() {
        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string()],
        );

        let json = content.to_json().unwrap();
        let parsed = SkillLicenseContent::from_json(&json).unwrap();

        assert_eq!(parsed.skill_id, "skill-123");
        assert_eq!(parsed.capabilities.len(), 1);
    }

    #[test]
    fn test_skill_delivery_content_creation() {
        let content = SkillDeliveryContent::new("skill-123", "fn main() {}", "rust", "abc123");

        assert_eq!(content.skill_id, "skill-123");
        assert_eq!(content.content, "fn main() {}");
        assert_eq!(content.content_type, "rust");
        assert_eq!(content.content_hash, "abc123");
    }

    #[test]
    fn test_skill_delivery_content_verify_hash() {
        let content = SkillDeliveryContent::new("skill-123", "fn main() {}", "rust", "abc123");

        assert!(content.verify_hash("abc123").is_ok());
        assert!(content.verify_hash("def456").is_err());
    }

    #[test]
    fn test_skill_delivery_content_serialization() {
        let content = SkillDeliveryContent::new("skill-123", "fn main() {}", "rust", "abc123");

        let json = content.to_json().unwrap();
        let parsed = SkillDeliveryContent::from_json(&json).unwrap();

        assert_eq!(parsed.skill_id, "skill-123");
        assert_eq!(parsed.content_type, "rust");
    }

    #[test]
    fn test_skill_license_creation() {
        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string()],
        );
        let license = SkillLicense::new(
            content,
            "agent-pubkey",
            "33400:skill-pubkey:web-scraper",
            "manifest-event-id",
            1000,
        );

        assert_eq!(license.agent_pubkey, "agent-pubkey");
        assert_eq!(license.skill_address, "33400:skill-pubkey:web-scraper");
        assert_eq!(license.manifest_event_id, "manifest-event-id");
        assert_eq!(license.price_sats, 1000);
    }

    #[test]
    fn test_skill_license_tags() {
        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string()],
        )
        .with_expires_at(1703086400);
        let license = SkillLicense::new(
            content,
            "agent-pubkey",
            "33400:skill-pubkey:web-scraper",
            "manifest-event-id",
            1000,
        )
        .with_manifest_relay_hint("wss://relay.example");

        let tags = license.build_tags();

        assert_eq!(tags[0], vec!["d", "skill-123"]);
        assert_eq!(tags[1], vec!["p", "agent-pubkey"]);
        assert_eq!(tags[2], vec!["a", "33400:skill-pubkey:web-scraper"]);
        assert_eq!(
            tags[3],
            vec!["e", "manifest-event-id", "wss://relay.example"]
        );
        assert_eq!(tags[4], vec!["version", "1.0.0"]);
        assert_eq!(tags[5], vec!["licensed_at", "1703000000"]);
        assert_eq!(tags[6], vec!["price_sats", "1000"]);
        assert_eq!(tags[7], vec!["expires_at", "1703086400"]);
    }

    #[test]
    fn test_skill_license_validation() {
        let content = SkillLicenseContent::new(
            "skill-123",
            "web-scraper",
            "1.0.0",
            1703000000,
            vec!["fetch".to_string()],
        )
        .with_expires_at(1703086400);
        let license = SkillLicense::new(
            content,
            "agent-pubkey",
            "33400:skill-pubkey:web-scraper",
            "manifest-event-id",
            1000,
        );

        assert!(license.validate(1703000000).is_ok());
        assert!(license.validate(1703086400).is_err());
    }

    #[test]
    fn test_skill_delivery_creation() {
        let content = SkillDeliveryContent::new("skill-123", "fn main() {}", "rust", "abc123");
        let delivery = SkillDelivery::new(
            content,
            "agent-pubkey",
            "33400:skill-pubkey:web-scraper",
            "manifest-event-id",
            "license-event-id",
        );

        assert_eq!(delivery.agent_pubkey, "agent-pubkey");
        assert_eq!(delivery.skill_address, "33400:skill-pubkey:web-scraper");
        assert_eq!(delivery.manifest_event_id, "manifest-event-id");
        assert_eq!(delivery.license_id, "license-event-id");
    }

    #[test]
    fn test_skill_delivery_tags() {
        let content = SkillDeliveryContent::new("skill-123", "fn main() {}", "rust", "abc123");
        let delivery = SkillDelivery::new(
            content,
            "agent-pubkey",
            "33400:skill-pubkey:web-scraper",
            "manifest-event-id",
            "license-event-id",
        )
        .with_manifest_relay_hint("wss://manifest.relay")
        .with_license_relay_hint("wss://license.relay");

        let tags = delivery.build_tags();

        assert_eq!(tags[0], vec!["p", "agent-pubkey"]);
        assert_eq!(tags[1], vec!["a", "33400:skill-pubkey:web-scraper"]);
        assert_eq!(
            tags[2],
            vec!["e", "manifest-event-id", "wss://manifest.relay"]
        );
        assert_eq!(
            tags[3],
            vec!["e", "license-event-id", "wss://license.relay"]
        );
        assert_eq!(tags[4], vec!["type", "rust"]);
        assert_eq!(tags[5], vec!["hash", "abc123"]);
    }

    #[test]
    fn test_fulfillment_gate_blocks_untrusted_manifest() {
        let manifest_event = manifest_event();
        let skill_address = "33400:skill-pubkey:web-scraper";

        let license = SkillLicense::new(
            SkillLicenseContent::new(
                "license-123",
                "web-scraper",
                "1.0.0",
                1_703_000_000,
                vec!["http:outbound".to_string()],
            ),
            "agent-pubkey",
            skill_address,
            "manifest-event-id",
            1000,
        );

        let delivery = SkillDelivery::new(
            SkillDeliveryContent::new("license-123", "encrypted", "prompt", "abc123"),
            "agent-pubkey",
            skill_address,
            "manifest-event-id",
            "license-event-id",
        );

        let outcome = evaluate_fulfillment_gate(
            &license,
            &delivery,
            &manifest_event,
            &[],
            &[],
            &TrustPolicy::default(),
        );
        assert!(!outcome.allowed);
        assert!(outcome.reason.contains("trust gate denied"));
    }

    #[test]
    fn test_fulfillment_gate_blocks_revoked_manifest() {
        let manifest_event = manifest_event();
        let skill_address = "33400:skill-pubkey:web-scraper";

        let license = SkillLicense::new(
            SkillLicenseContent::new(
                "license-123",
                "web-scraper",
                "1.0.0",
                1_703_000_000,
                vec!["http:outbound".to_string()],
            ),
            "agent-pubkey",
            skill_address,
            "manifest-event-id",
            1000,
        );

        let delivery = SkillDelivery::new(
            SkillDeliveryContent::new("license-123", "encrypted", "prompt", "abc123"),
            "agent-pubkey",
            skill_address,
            "manifest-event-id",
            "license-event-id",
        );

        let labels = vec![trusted_label_event(skill_address, "manifest-event-id")];
        let deletions = vec![revocation_event(skill_address, "manifest-event-id")];

        let outcome = evaluate_fulfillment_gate(
            &license,
            &delivery,
            &manifest_event,
            &labels,
            &deletions,
            &TrustPolicy::default(),
        );

        assert!(!outcome.allowed);
        assert!(outcome.reason.contains("manifest revoked"));
    }

    #[test]
    fn test_fulfillment_gate_allows_trusted_active_manifest() {
        let manifest_event = manifest_event();
        let skill_address = "33400:skill-pubkey:web-scraper";

        let license = SkillLicense::new(
            SkillLicenseContent::new(
                "license-123",
                "web-scraper",
                "1.0.0",
                1_703_000_000,
                vec!["http:outbound".to_string()],
            ),
            "agent-pubkey",
            skill_address,
            "manifest-event-id",
            1000,
        );

        let delivery = SkillDelivery::new(
            SkillDeliveryContent::new("license-123", "encrypted", "prompt", "abc123"),
            "agent-pubkey",
            skill_address,
            "manifest-event-id",
            "license-event-id",
        );

        let labels = vec![trusted_label_event(skill_address, "manifest-event-id")];

        let outcome = evaluate_fulfillment_gate(
            &license,
            &delivery,
            &manifest_event,
            &labels,
            &[],
            &TrustPolicy::default(),
        );

        assert!(outcome.allowed);
        assert_eq!(outcome.reason, "allowed");
    }
}
