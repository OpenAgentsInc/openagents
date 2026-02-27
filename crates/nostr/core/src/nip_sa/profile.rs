//! Agent Profile Event (kind:39200)
//!
//! Agents publish a profile event similar to `kind:0` user metadata, but with
//! additional agent-specific fields.
//!
//! ## Fields
//!
//! - `name` - Agent display name
//! - `about` - Agent description
//! - `picture` - Avatar URL
//! - `capabilities` - List of supported features
//! - `autonomy_level` - supervised | bounded | autonomous
//! - `version` - Agent version string
//!
//! ## Tags
//!
//! - `["d", "profile"]` - Addressable event marker
//! - `["threshold", "2", "3"]` - Threshold signature scheme (2-of-3)
//! - `["signer", "<pubkey>"]` - Marketplace/guardian signer pubkeys
//! - `["operator", "<pubkey>"]` - Human operator pubkey
//! - `["lud16", "<address>"]` - Lightning address for payments
//!
//! # Examples
//!
//! ```
//! use nostr::nip_sa::profile::{AgentProfile, AgentMetadata, AutonomyLevel};
//!
//! // Create agent profile metadata
//! let metadata = AgentMetadata {
//!     name: "ResearchBot".to_string(),
//!     about: Some("I research topics and provide summaries".to_string()),
//!     picture: Some("https://example.com/avatar.png".to_string()),
//!     capabilities: vec!["research".to_string(), "summarization".to_string()],
//!     autonomy_level: AutonomyLevel::Bounded,
//!     version: Some("1.0.0".to_string()),
//! };
//!
//! // Create profile with threshold config
//! let profile = AgentProfile::new(metadata)
//!     .with_threshold(2, 3)
//!     .with_operator("operator-pubkey")
//!     .with_signer("marketplace-pubkey")
//!     .with_lightning_address("agent@example.com");
//!
//! // Metadata can be serialized to event content
//! assert_eq!(profile.metadata.name, "ResearchBot");
//! assert_eq!(profile.threshold, Some((2, 3)));
//! ```
//!
//! ## Event JSON Structure
//!
//! ```json
//! {
//!   "kind": 39200,
//!   "pubkey": "<agent-pubkey>",
//!   "content": "{\"name\":\"ResearchBot\",\"about\":\"I research topics\"}",
//!   "tags": [
//!     ["d", "profile"],
//!     ["threshold", "2", "3"],
//!     ["signer", "<marketplace-pubkey>"],
//!     ["operator", "<operator-pubkey>"]
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for agent profile event
pub const KIND_AGENT_PROFILE: u16 = 39200;

/// Errors that can occur during NIP-SA profile operations
#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("invalid profile metadata: {0}")]
    InvalidMetadata(String),

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("invalid threshold configuration: {0}")]
    InvalidThreshold(String),
}

/// Autonomy level of the agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// Agent requests approval before major actions
    Supervised,
    /// Agent acts within defined constraints without approval
    Bounded,
    /// Agent acts freely toward goals
    Autonomous,
}

/// Threshold signature configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThresholdConfig {
    /// Threshold (t) - number of shares required
    pub threshold: u32,
    /// Total shares (n)
    pub total_shares: u32,
    /// Marketplace signer pubkey (hex)
    pub marketplace_signer: String,
    /// Optional guardian signer pubkey (hex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_signer: Option<String>,
}

impl ThresholdConfig {
    /// Create a new threshold configuration
    pub fn new(
        threshold: u32,
        total_shares: u32,
        marketplace_signer: impl Into<String>,
    ) -> Result<Self, ProfileError> {
        if threshold == 0 || total_shares == 0 {
            return Err(ProfileError::InvalidThreshold(
                "threshold and total_shares must be > 0".to_string(),
            ));
        }
        if threshold > total_shares {
            return Err(ProfileError::InvalidThreshold(
                "threshold cannot exceed total_shares".to_string(),
            ));
        }
        Ok(Self {
            threshold,
            total_shares,
            marketplace_signer: marketplace_signer.into(),
            guardian_signer: None,
        })
    }

    /// Add a guardian signer
    pub fn with_guardian(mut self, guardian: impl Into<String>) -> Self {
        self.guardian_signer = Some(guardian.into());
        self
    }
}

/// Agent profile metadata (stored in content field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfileContent {
    /// Agent display name
    pub name: String,
    /// Agent description
    pub about: String,
    /// Avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// List of supported capabilities
    pub capabilities: Vec<String>,
    /// Autonomy level
    pub autonomy_level: AutonomyLevel,
    /// Agent version
    pub version: String,
}

impl AgentProfileContent {
    /// Create new agent profile content
    pub fn new(
        name: impl Into<String>,
        about: impl Into<String>,
        autonomy_level: AutonomyLevel,
        version: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            about: about.into(),
            picture: None,
            capabilities: Vec::new(),
            autonomy_level,
            version: version.into(),
        }
    }

    /// Set the picture URL
    pub fn with_picture(mut self, picture: impl Into<String>) -> Self {
        self.picture = Some(picture.into());
        self
    }

    /// Add capabilities
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, ProfileError> {
        serde_json::to_string(self).map_err(|e| ProfileError::Serialization(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, ProfileError> {
        serde_json::from_str(json).map_err(|e| ProfileError::InvalidMetadata(e.to_string()))
    }

    /// Validate the profile content
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.name.trim().is_empty() {
            return Err(ProfileError::MissingField("name".to_string()));
        }
        if self.about.trim().is_empty() {
            return Err(ProfileError::MissingField("about".to_string()));
        }
        if self.version.trim().is_empty() {
            return Err(ProfileError::MissingField("version".to_string()));
        }
        Ok(())
    }
}

/// Agent profile with tags
#[derive(Debug, Clone)]
pub struct AgentProfile {
    /// Profile content
    pub content: AgentProfileContent,
    /// Threshold configuration
    pub threshold: ThresholdConfig,
    /// Operator pubkey (hex)
    pub operator: String,
    /// Optional Lightning address
    pub lud16: Option<String>,
}

impl AgentProfile {
    /// Create a new agent profile
    pub fn new(
        content: AgentProfileContent,
        threshold: ThresholdConfig,
        operator: impl Into<String>,
    ) -> Self {
        Self {
            content,
            threshold,
            operator: operator.into(),
            lud16: None,
        }
    }

    /// Set Lightning address
    pub fn with_lud16(mut self, lud16: impl Into<String>) -> Self {
        self.lud16 = Some(lud16.into());
        self
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), "profile".to_string()],
            vec![
                "threshold".to_string(),
                self.threshold.threshold.to_string(),
                self.threshold.total_shares.to_string(),
            ],
            vec![
                "signer".to_string(),
                self.threshold.marketplace_signer.clone(),
            ],
            vec!["operator".to_string(), self.operator.clone()],
        ];

        // Add guardian signer if present
        if let Some(guardian) = &self.threshold.guardian_signer {
            tags.push(vec!["signer".to_string(), guardian.clone()]);
        }

        // Add lud16 if present
        if let Some(lud16) = &self.lud16 {
            tags.push(vec!["lud16".to_string(), lud16.clone()]);
        }

        tags
    }

    /// Validate the profile
    pub fn validate(&self) -> Result<(), ProfileError> {
        self.content.validate()?;
        if self.operator.trim().is_empty() {
            return Err(ProfileError::MissingField("operator".to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_config_valid() {
        let config = ThresholdConfig::new(2, 3, "marketplace_pubkey").unwrap();
        assert_eq!(config.threshold, 2);
        assert_eq!(config.total_shares, 3);
        assert_eq!(config.marketplace_signer, "marketplace_pubkey");
        assert!(config.guardian_signer.is_none());
    }

    #[test]
    fn test_threshold_config_with_guardian() {
        let config = ThresholdConfig::new(2, 3, "marketplace_pubkey")
            .unwrap()
            .with_guardian("guardian_pubkey");
        assert_eq!(config.guardian_signer, Some("guardian_pubkey".to_string()));
    }

    #[test]
    fn test_threshold_config_invalid() {
        assert!(ThresholdConfig::new(0, 3, "marketplace_pubkey").is_err());
        assert!(ThresholdConfig::new(2, 0, "marketplace_pubkey").is_err());
        assert!(ThresholdConfig::new(4, 3, "marketplace_pubkey").is_err());
    }

    #[test]
    fn test_profile_content_creation() {
        let content =
            AgentProfileContent::new("TestBot", "A test bot", AutonomyLevel::Supervised, "1.0.0")
                .with_picture("https://example.com/avatar.png")
                .with_capabilities(vec!["test".to_string(), "demo".to_string()]);

        assert_eq!(content.name, "TestBot");
        assert_eq!(content.about, "A test bot");
        assert_eq!(
            content.picture,
            Some("https://example.com/avatar.png".to_string())
        );
        assert_eq!(content.capabilities.len(), 2);
        assert_eq!(content.autonomy_level, AutonomyLevel::Supervised);
        assert_eq!(content.version, "1.0.0");
    }

    #[test]
    fn test_profile_content_serialization() {
        let content = AgentProfileContent::new(
            "ResearchBot",
            "I research topics and summarize findings",
            AutonomyLevel::Supervised,
            "1.0.0",
        )
        .with_capabilities(vec![
            "research".to_string(),
            "summarization".to_string(),
            "translation".to_string(),
        ]);

        let json = content.to_json().unwrap();
        let parsed = AgentProfileContent::from_json(&json).unwrap();

        assert_eq!(parsed.name, content.name);
        assert_eq!(parsed.about, content.about);
        assert_eq!(parsed.capabilities, content.capabilities);
        assert_eq!(parsed.autonomy_level, content.autonomy_level);
    }

    #[test]
    fn test_profile_content_validation() {
        let valid =
            AgentProfileContent::new("TestBot", "A test bot", AutonomyLevel::Supervised, "1.0.0");
        assert!(valid.validate().is_ok());

        let invalid_name =
            AgentProfileContent::new("", "A test bot", AutonomyLevel::Supervised, "1.0.0");
        assert!(invalid_name.validate().is_err());

        let invalid_about =
            AgentProfileContent::new("TestBot", "", AutonomyLevel::Supervised, "1.0.0");
        assert!(invalid_about.validate().is_err());

        let invalid_version =
            AgentProfileContent::new("TestBot", "A test bot", AutonomyLevel::Supervised, "");
        assert!(invalid_version.validate().is_err());
    }

    #[test]
    fn test_agent_profile_tags() {
        let content =
            AgentProfileContent::new("TestBot", "A test bot", AutonomyLevel::Supervised, "1.0.0");
        let threshold = ThresholdConfig::new(2, 3, "marketplace_pubkey")
            .unwrap()
            .with_guardian("guardian_pubkey");
        let profile = AgentProfile::new(content, threshold, "operator_pubkey")
            .with_lud16("testbot@getalby.com");

        let tags = profile.build_tags();

        // Check d tag
        assert_eq!(tags[0], vec!["d", "profile"]);

        // Check threshold tag
        assert_eq!(tags[1], vec!["threshold", "2", "3"]);

        // Check marketplace signer tag
        assert_eq!(tags[2], vec!["signer", "marketplace_pubkey"]);

        // Check operator tag
        assert_eq!(tags[3], vec!["operator", "operator_pubkey"]);

        // Check guardian signer tag
        assert_eq!(tags[4], vec!["signer", "guardian_pubkey"]);

        // Check lud16 tag
        assert_eq!(tags[5], vec!["lud16", "testbot@getalby.com"]);
    }

    #[test]
    fn test_agent_profile_validation() {
        let content =
            AgentProfileContent::new("TestBot", "A test bot", AutonomyLevel::Supervised, "1.0.0");
        let threshold = ThresholdConfig::new(2, 3, "marketplace_pubkey").unwrap();
        let profile = AgentProfile::new(content, threshold, "operator_pubkey");

        assert!(profile.validate().is_ok());

        let content_invalid =
            AgentProfileContent::new("", "A test bot", AutonomyLevel::Supervised, "1.0.0");
        let profile_invalid = AgentProfile::new(
            content_invalid,
            ThresholdConfig::new(2, 3, "marketplace_pubkey").unwrap(),
            "operator_pubkey",
        );
        assert!(profile_invalid.validate().is_err());
    }

    #[test]
    fn test_autonomy_level_serialization() {
        let supervised = AutonomyLevel::Supervised;
        let json = serde_json::to_string(&supervised).unwrap();
        assert_eq!(json, "\"supervised\"");

        let bounded = AutonomyLevel::Bounded;
        let json = serde_json::to_string(&bounded).unwrap();
        assert_eq!(json, "\"bounded\"");

        let autonomous = AutonomyLevel::Autonomous;
        let json = serde_json::to_string(&autonomous).unwrap();
        assert_eq!(json, "\"autonomous\"");
    }
}
