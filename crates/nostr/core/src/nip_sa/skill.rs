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
//! - `["d", "<skill-id>"]` - Unique skill identifier
//! - `["agent", "<agent-pubkey>"]` - Licensed agent
//! - `["skill", "<skill-name>"]` - Skill name
//! - `["expires", "1703000000"]` - Expiration timestamp (optional)
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
        self.expires_at.map_or(false, |exp| current_time >= exp)
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
    /// Price paid in satoshis
    pub price_sats: u64,
}

impl SkillLicense {
    /// Create new skill license
    pub fn new(
        content: SkillLicenseContent,
        agent_pubkey: impl Into<String>,
        price_sats: u64,
    ) -> Self {
        Self {
            content,
            agent_pubkey: agent_pubkey.into(),
            price_sats,
        }
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.content.skill_id.clone()],
            vec!["agent".to_string(), self.agent_pubkey.clone()],
            vec!["skill".to_string(), self.content.skill_name.clone()],
            vec!["price_sats".to_string(), self.price_sats.to_string()],
        ];

        if let Some(expires_at) = self.content.expires_at {
            tags.push(vec!["expires".to_string(), expires_at.to_string()]);
        }

        tags
    }

    /// Validate the license
    pub fn validate(&self, current_time: u64) -> Result<(), SkillError> {
        if self.content.is_expired(current_time) {
            return Err(SkillError::LicenseExpired(format!(
                "license for {} expired at {}",
                self.content.skill_id,
                self.content.expires_at.unwrap()
            )));
        }
        Ok(())
    }
}

/// Skill delivery event wrapper
#[derive(Debug, Clone)]
pub struct SkillDelivery {
    /// Delivery content
    pub content: SkillDeliveryContent,
    /// License event ID
    pub license_id: String,
}

impl SkillDelivery {
    /// Create new skill delivery
    pub fn new(content: SkillDeliveryContent, license_id: impl Into<String>) -> Self {
        Self {
            content,
            license_id: license_id.into(),
        }
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        vec![
            vec!["license".to_string(), self.license_id.clone()],
            vec!["skill".to_string(), self.content.skill_id.clone()],
            vec!["type".to_string(), self.content.content_type.clone()],
            vec!["hash".to_string(), self.content.content_hash.clone()],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let license = SkillLicense::new(content, "agent-pubkey", 1000);

        assert_eq!(license.agent_pubkey, "agent-pubkey");
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
        let license = SkillLicense::new(content, "agent-pubkey", 1000);

        let tags = license.build_tags();

        assert_eq!(tags[0], vec!["d", "skill-123"]);
        assert_eq!(tags[1], vec!["agent", "agent-pubkey"]);
        assert_eq!(tags[2], vec!["skill", "web-scraper"]);
        assert_eq!(tags[3], vec!["price_sats", "1000"]);
        assert_eq!(tags[4], vec!["expires", "1703086400"]);
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
        let license = SkillLicense::new(content, "agent-pubkey", 1000);

        assert!(license.validate(1703000000).is_ok());
        assert!(license.validate(1703086400).is_err());
    }

    #[test]
    fn test_skill_delivery_creation() {
        let content = SkillDeliveryContent::new("skill-123", "fn main() {}", "rust", "abc123");
        let delivery = SkillDelivery::new(content, "license-event-id");

        assert_eq!(delivery.license_id, "license-event-id");
    }

    #[test]
    fn test_skill_delivery_tags() {
        let content = SkillDeliveryContent::new("skill-123", "fn main() {}", "rust", "abc123");
        let delivery = SkillDelivery::new(content, "license-event-id");

        let tags = delivery.build_tags();

        assert_eq!(tags[0], vec!["license", "license-event-id"]);
        assert_eq!(tags[1], vec!["skill", "skill-123"]);
        assert_eq!(tags[2], vec!["type", "rust"]);
        assert_eq!(tags[3], vec!["hash", "abc123"]);
    }
}
