//! NIP-11: Relay Information Document
//!
//! Provides relay metadata to clients via HTTP endpoint with Accept: application/nostr+json header.

use serde::{Deserialize, Serialize};

/// NIP-11 Relay Information Document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayInformation {
    /// Relay name (should be <30 chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Detailed description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Banner image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,

    /// Icon image URL (should be square)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// Administrative contact pubkey (hex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,

    /// Relay's own pubkey (hex)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "self")]
    pub self_pubkey: Option<String>,

    /// Administrative contact (URI: mailto:, https:, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,

    /// List of supported NIP numbers
    pub supported_nips: Vec<u16>,

    /// Relay software URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software: Option<String>,

    /// Software version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Privacy policy URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy_policy: Option<String>,

    /// Terms of service URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_of_service: Option<String>,

    /// Server limitations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limitation: Option<Limitation>,

    /// Event retention policies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention: Option<Vec<RetentionPolicy>>,

    /// Relay country codes (ISO 3166-1 alpha-2)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_countries: Option<Vec<String>>,

    /// Language tags (IETF)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_tags: Option<Vec<String>>,

    /// Community tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Posting policy URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_policy: Option<String>,

    /// Payments URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payments_url: Option<String>,

    /// Fee schedules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees: Option<Fees>,
}

/// Server limitations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limitation {
    /// Maximum message length in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_message_length: Option<usize>,

    /// Maximum active subscriptions per connection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_subscriptions: Option<usize>,

    /// Maximum subscription ID length
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_subid_length: Option<usize>,

    /// Maximum limit value in filters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_limit: Option<usize>,

    /// Maximum event tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_event_tags: Option<usize>,

    /// Maximum content length (unicode characters)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_content_length: Option<usize>,

    /// Minimum PoW difficulty (NIP-13)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_pow_difficulty: Option<u32>,

    /// Authentication required (NIP-42)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_required: Option<bool>,

    /// Payment required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_required: Option<bool>,

    /// Restricted writes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restricted_writes: Option<bool>,

    /// created_at lower limit (seconds ago)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at_lower_limit: Option<u64>,

    /// created_at upper limit (seconds in future)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at_upper_limit: Option<u64>,

    /// Default limit if not specified in filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_limit: Option<usize>,
}

/// Event retention policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Event kinds (or ranges) this policy applies to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<KindOrRange>>,

    /// Retention time in seconds (null = infinity, 0 = no storage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,

    /// Maximum count of events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}

/// Kind number or range [start, end]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KindOrRange {
    Single(u16),
    Range(u16, u16),
}

/// Fee schedules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fees {
    /// Admission fees
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission: Option<Vec<FeeSchedule>>,

    /// Subscription fees
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription: Option<Vec<FeeSchedule>>,

    /// Publication fees (per event)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication: Option<Vec<FeeSchedule>>,
}

/// Fee schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeSchedule {
    /// Fee amount
    pub amount: u64,

    /// Unit (sats, msats, etc.)
    pub unit: String,

    /// Period in seconds (for subscriptions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<u64>,

    /// Applicable event kinds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<u16>>,
}

impl RelayInformation {
    /// Create a new relay information document with defaults
    pub fn new() -> Self {
        // List of all NIPs implemented in crates/nostr/core
        let supported_nips = vec![
            1, 2, 4, 5, 6, 7, 9, 10, 11, 12, 13, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
            27, 28, 29, 30, 31, 32, 33, 35, 36, 37, 38, 39, 40, 42, 44, 45, 46, 47, 48, 49, 50, 51,
            52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 64, 65, 68, 69, 70, 71, 72, 73, 75, 78, 84, 86,
            89, 90, 92, 94, 95, 96, 98, 99,
        ];

        Self {
            name: Some("OpenAgents Relay".to_string()),
            description: Some(
                "Nostr relay for OpenAgents network with comprehensive NIP support".to_string(),
            ),
            banner: None,
            icon: None,
            pubkey: None,
            self_pubkey: None,
            contact: Some("admin@openagents.com".to_string()),
            supported_nips,
            software: Some("https://github.com/OpenAgentsInc/openagents".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            privacy_policy: None,
            terms_of_service: None,
            limitation: Some(Limitation::default()),
            retention: None,
            relay_countries: None,
            language_tags: None,
            tags: None,
            posting_policy: None,
            payments_url: None,
            fees: None,
        }
    }

    /// Set supported NIPs
    pub fn with_supported_nips(mut self, nips: Vec<u16>) -> Self {
        self.supported_nips = nips;
        self
    }

    /// Set limitation
    pub fn with_limitation(mut self, limitation: Limitation) -> Self {
        self.limitation = Some(limitation);
        self
    }
}

impl Default for RelayInformation {
    fn default() -> Self {
        Self::new()
    }
}

impl Limitation {
    /// Create default limitation based on relay config
    pub fn from_config(max_message_size: usize, max_subscriptions: usize) -> Self {
        Self {
            max_message_length: Some(max_message_size),
            max_subscriptions: Some(max_subscriptions),
            max_subid_length: Some(256),
            max_limit: Some(5000),
            max_event_tags: Some(2000),
            max_content_length: Some(102400),
            min_pow_difficulty: Some(0),
            auth_required: Some(false),
            payment_required: Some(false),
            restricted_writes: Some(false),
            created_at_lower_limit: None,
            created_at_upper_limit: None,
            default_limit: Some(100),
        }
    }
}

impl Default for Limitation {
    fn default() -> Self {
        Self::from_config(512 * 1024, 20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_information_serialization() {
        let info = RelayInformation::new();
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"OpenAgents Relay\""));
        assert!(json.contains("\"supported_nips\""));
    }

    #[test]
    fn test_relay_information_with_limitation() {
        let limitation = Limitation::from_config(1024 * 1024, 50);
        let info = RelayInformation::new().with_limitation(limitation);

        assert!(info.limitation.is_some());
        let lim = info.limitation.unwrap();
        assert_eq!(lim.max_message_length, Some(1024 * 1024));
        assert_eq!(lim.max_subscriptions, Some(50));
    }

    #[test]
    fn test_limitation_serialization() {
        let limitation = Limitation::default();
        let json = serde_json::to_string(&limitation).unwrap();
        assert!(json.contains("\"max_message_length\""));
        assert!(json.contains("\"max_subscriptions\""));
    }

    #[test]
    fn test_omit_none_fields() {
        let info = RelayInformation {
            name: Some("Test".to_string()),
            description: None,
            banner: None,
            icon: None,
            pubkey: None,
            self_pubkey: None,
            contact: None,
            supported_nips: vec![1],
            software: None,
            version: None,
            privacy_policy: None,
            terms_of_service: None,
            limitation: None,
            retention: None,
            relay_countries: None,
            language_tags: None,
            tags: None,
            posting_policy: None,
            payments_url: None,
            fees: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        // Should not contain fields that are None
        assert!(!json.contains("\"description\""));
        assert!(!json.contains("\"banner\""));
        assert!(json.contains("\"name\":\"Test\""));
    }
}
