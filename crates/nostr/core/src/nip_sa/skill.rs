//! Skill Events (kinds:38020, 38021)
//!
//! Skills are capabilities that agents can acquire, typically through purchase
//! from a marketplace. Skills are protected by licenses and delivered encrypted.
//!
//! ## Skill License (kind:38020)
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
//! ## Skill Delivery (kind:38021)
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

// TODO: Implement Event builders for kinds 38020, 38021
// TODO: Implement license validation logic
// TODO: Implement NIP-59 gift wrap integration
// TODO: Implement threshold ECDH for license-gated decryption
// TODO: Add unit tests
