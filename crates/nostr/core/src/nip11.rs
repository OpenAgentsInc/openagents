//! NIP-11: Relay Information Document
//!
//! This module implements NIP-11, which defines a JSON metadata format that relays
//! provide via HTTP to describe their capabilities, policies, and operational constraints.
//!
//! ## How It Works
//!
//! When a relay receives an HTTP(S) request with an `Accept: application/nostr+json` header
//! to a URI supporting WebSocket upgrades, it should return a JSON document describing the
//! relay's capabilities and policies.
//!
//! ## Example
//!
//! ```
//! use nostr_core::nip11::{RelayInformationDocument, RelayLimitation};
//!
//! let info = RelayInformationDocument {
//!     name: Some("My Relay".to_string()),
//!     description: Some("A friendly Nostr relay".to_string()),
//!     supported_nips: Some(vec![1, 2, 9, 11, 12, 15, 16, 20, 22]),
//!     software: Some("https://github.com/myrelay".to_string()),
//!     version: Some("1.0.0".to_string()),
//!     limitation: Some(RelayLimitation {
//!         max_message_length: Some(16384),
//!         max_subscriptions: Some(20),
//!         max_limit: Some(5000),
//!         ..Default::default()
//!     }),
//!     ..Default::default()
//! };
//!
//! let json = serde_json::to_string(&info).unwrap();
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The HTTP Accept header value for requesting relay information
pub const RELAY_INFO_ACCEPT_HEADER: &str = "application/nostr+json";

/// Errors that can occur during NIP-11 operations.
#[derive(Debug, Error)]
pub enum Nip11Error {
    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),

    #[error("invalid relay information: {0}")]
    InvalidInformation(String),
}

/// Relay Information Document as specified in NIP-11.
///
/// All fields are optional. Clients MUST ignore any additional fields they do not understand.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RelayInformationDocument {
    /// Relay name (should be less than 30 characters to avoid client truncation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Detailed plain-text information about the relay
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Administrative contact's 32-byte hex secp256k1 public key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,

    /// Relay's independent identity (32-byte hex public key)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "self")]
    pub self_pubkey: Option<String>,

    /// Alternative contact URI (mailto or https schemes preferred)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,

    /// Array of NIP numbers implemented by the relay
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_nips: Option<Vec<u16>>,

    /// URL to the relay implementation's project homepage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software: Option<String>,

    /// Version identifier (version number or commit identifier)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// URL to a relay branding image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,

    /// URL to a compact visual representation (preferably square)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// Link to privacy policy document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy_policy: Option<String>,

    /// Link to terms of service document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_of_service: Option<String>,

    /// Server limitations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limitation: Option<RelayLimitation>,

    /// Event retention policies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention: Option<Vec<RetentionPolicy>>,

    /// ISO 3166-1 alpha-2 codes indicating jurisdictions affecting the relay
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_countries: Option<Vec<String>>,

    /// IETF language tags spoken on relay
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_tags: Option<Vec<String>>,

    /// Community topic restrictions (e.g., "sfw-only", "bitcoin-only")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// URL to detailed community guidelines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_policy: Option<String>,

    /// Payment processing endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payments_url: Option<String>,

    /// Fee schedules for admission, subscription, and publication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees: Option<RelayFees>,
}

/// Server limitations and constraints.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RelayLimitation {
    /// Maximum incoming JSON bytes (WebSocket frame size)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_message_length: Option<u32>,

    /// Maximum active subscriptions per connection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_subscriptions: Option<u32>,

    /// Maximum filter limit value (clamped)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_limit: Option<u32>,

    /// Maximum subscription ID string length
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_subid_length: Option<u32>,

    /// Maximum tags per event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_event_tags: Option<u32>,

    /// Maximum content field characters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_content_length: Option<u32>,

    /// Required PoW difficulty (NIP-13)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_pow_difficulty: Option<u32>,

    /// NIP-42 authentication required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_required: Option<bool>,

    /// Payment prerequisite
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_required: Option<bool>,

    /// Special conditions for event acceptance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restricted_writes: Option<bool>,

    /// Earliest acceptable timestamp (Unix time in seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at_lower_limit: Option<i64>,

    /// Latest acceptable timestamp (Unix time in seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at_upper_limit: Option<i64>,

    /// Default returned events without limit parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_limit: Option<u32>,
}

/// Event retention policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetentionPolicy {
    /// Event kinds or ranges this policy applies to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<KindOrRange>>,

    /// Retention duration in seconds (null = infinite)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,

    /// Maximum event count for specified kinds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
}

/// Event kind or range of kinds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum KindOrRange {
    /// Single kind
    Single(u16),
    /// Range of kinds [start, end]
    Range(Vec<u16>),
}

/// Relay fee schedules.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RelayFees {
    /// Admission fee
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission: Option<Vec<FeeSchedule>>,

    /// Subscription fee
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription: Option<Vec<FeeSchedule>>,

    /// Publication fee
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication: Option<Vec<FeeSchedule>>,
}

/// Fee schedule entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeeSchedule {
    /// Fee amount
    pub amount: u64,

    /// Fee unit (e.g., "msats", "sats")
    pub unit: String,

    /// Optional period for recurring fees (e.g., "month", "year")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,

    /// Optional event kinds this fee applies to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<u16>>,
}

impl RelayInformationDocument {
    /// Create a new relay information document with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the relay name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the relay description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the supported NIPs.
    pub fn supported_nips(mut self, nips: Vec<u16>) -> Self {
        self.supported_nips = Some(nips);
        self
    }

    /// Set the software URL.
    pub fn software(mut self, software: impl Into<String>) -> Self {
        self.software = Some(software.into());
        self
    }

    /// Set the version.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the admin public key.
    pub fn pubkey(mut self, pubkey: impl Into<String>) -> Self {
        self.pubkey = Some(pubkey.into());
        self
    }

    /// Set the contact information.
    pub fn contact(mut self, contact: impl Into<String>) -> Self {
        self.contact = Some(contact.into());
        self
    }

    /// Set the relay limitations.
    pub fn limitation(mut self, limitation: RelayLimitation) -> Self {
        self.limitation = Some(limitation);
        self
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, Nip11Error> {
        Ok(serde_json::to_string(self)?)
    }

    /// Serialize to pretty JSON string.
    pub fn to_json_pretty(&self) -> Result<String, Nip11Error> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, Nip11Error> {
        Ok(serde_json::from_str(json)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_document() {
        let doc = RelayInformationDocument::new()
            .name("Test Relay")
            .description("A test relay");

        let json = doc.to_json().unwrap();
        let parsed: RelayInformationDocument = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, Some("Test Relay".to_string()));
        assert_eq!(parsed.description, Some("A test relay".to_string()));
    }

    #[test]
    fn test_full_document() {
        let doc = RelayInformationDocument {
            name: Some("Full Test Relay".to_string()),
            description: Some("A fully configured test relay".to_string()),
            pubkey: Some("a".repeat(64)),
            contact: Some("mailto:admin@relay.example".to_string()),
            supported_nips: Some(vec![1, 2, 9, 11, 12, 15, 16, 20, 22]),
            software: Some("https://github.com/example/relay".to_string()),
            version: Some("1.0.0".to_string()),
            limitation: Some(RelayLimitation {
                max_message_length: Some(16384),
                max_subscriptions: Some(20),
                max_limit: Some(5000),
                max_event_tags: Some(100),
                max_content_length: Some(8192),
                min_pow_difficulty: Some(20),
                auth_required: Some(false),
                payment_required: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };

        let json = doc.to_json().unwrap();
        let parsed: RelayInformationDocument = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, doc.name);
        assert_eq!(parsed.supported_nips, doc.supported_nips);
        assert_eq!(parsed.limitation, doc.limitation);
    }

    #[test]
    fn test_retention_policy() {
        let retention = RetentionPolicy {
            kinds: Some(vec![
                KindOrRange::Single(1),
                KindOrRange::Range(vec![10000, 20000]),
            ]),
            time: Some(2592000), // 30 days
            count: Some(1000),
        };

        let json = serde_json::to_string(&retention).unwrap();
        let parsed: RetentionPolicy = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, retention);
    }

    #[test]
    fn test_relay_fees() {
        let fees = RelayFees {
            admission: Some(vec![FeeSchedule {
                amount: 1000000,
                unit: "msats".to_string(),
                period: None,
                kinds: None,
            }]),
            subscription: Some(vec![FeeSchedule {
                amount: 5000000,
                unit: "msats".to_string(),
                period: Some("month".to_string()),
                kinds: None,
            }]),
            publication: Some(vec![FeeSchedule {
                amount: 100,
                unit: "msats".to_string(),
                period: None,
                kinds: Some(vec![1]),
            }]),
        };

        let json = serde_json::to_string(&fees).unwrap();
        let parsed: RelayFees = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, fees);
    }

    #[test]
    fn test_builder_pattern() {
        let doc = RelayInformationDocument::new()
            .name("Builder Test")
            .description("Testing builder pattern")
            .supported_nips(vec![1, 11])
            .software("https://example.com")
            .version("0.1.0")
            .pubkey("a".repeat(64))
            .contact("admin@example.com");

        assert_eq!(doc.name, Some("Builder Test".to_string()));
        assert_eq!(doc.version, Some("0.1.0".to_string()));
    }

    #[test]
    fn test_optional_fields_omitted() {
        let doc = RelayInformationDocument {
            name: Some("Minimal".to_string()),
            ..Default::default()
        };

        let json = doc.to_json().unwrap();
        assert!(!json.contains("description"));
        assert!(!json.contains("supported_nips"));
        assert!(json.contains("name"));
    }

    #[test]
    fn test_from_json() {
        let json = r#"{"name":"Test","supported_nips":[1,2,11]}"#;
        let doc = RelayInformationDocument::from_json(json).unwrap();

        assert_eq!(doc.name, Some("Test".to_string()));
        assert_eq!(doc.supported_nips, Some(vec![1, 2, 11]));
    }

    #[test]
    fn test_limitation_defaults() {
        let limitation = RelayLimitation::default();
        assert_eq!(limitation.max_message_length, None);
        assert_eq!(limitation.auth_required, None);
    }

    #[test]
    fn test_kind_or_range_serialization() {
        let single = KindOrRange::Single(1);
        let range = KindOrRange::Range(vec![1000, 2000]);

        let single_json = serde_json::to_string(&single).unwrap();
        let range_json = serde_json::to_string(&range).unwrap();

        assert_eq!(single_json, "1");
        assert_eq!(range_json, "[1000,2000]");
    }
}
