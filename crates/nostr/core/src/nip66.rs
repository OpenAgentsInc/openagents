//! NIP-66: Relay Discovery and Liveness Monitoring
//!
//! This module implements NIP-66, which defines events for relay discovery and monitoring.
//! Monitors can publish relay characteristics, performance metrics, and announce their
//! monitoring services.
//!
//! ## Event Kinds
//!
//! - `30166`: Relay Discovery (addressable, characteristics and metrics)
//! - `10166`: Relay Monitor Announcement (monitor metadata and capabilities)
//!
//! ## Use Cases
//!
//! - Relay discovery and selection
//! - Performance monitoring and comparison
//! - Network topology mapping
//! - Relay capability discovery
//! - Automated relay health checking
//!
//! # Example
//!
//! ```
//! use nostr_core::nip66::{RelayDiscovery, validate_relay_discovery};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event) {
//! // Validate a relay discovery event
//! match validate_relay_discovery(event) {
//!     Ok(_) => println!("Valid relay discovery event"),
//!     Err(e) => println!("Invalid: {}", e),
//! }
//! # }
//! ```

use crate::nip01::Event;
use std::str::FromStr;
use thiserror::Error;

/// Event kind for relay discovery (addressable)
pub const RELAY_DISCOVERY_KIND: u16 = 30166;

/// Event kind for relay monitor announcements
pub const RELAY_MONITOR_ANNOUNCEMENT_KIND: u16 = 10166;

/// Tag names for relay discovery
pub const RTT_OPEN_TAG: &str = "rtt-open";
pub const RTT_READ_TAG: &str = "rtt-read";
pub const RTT_WRITE_TAG: &str = "rtt-write";
pub const NETWORK_TYPE_TAG: &str = "n";
pub const RELAY_TYPE_TAG: &str = "T";
pub const NIP_SUPPORT_TAG: &str = "N";
pub const REQUIREMENT_TAG: &str = "R";
pub const TOPIC_TAG: &str = "t";
pub const KIND_TAG: &str = "k";
pub const GEOHASH_TAG: &str = "g";

/// Tag names for monitor announcements
pub const FREQUENCY_TAG: &str = "frequency";
pub const TIMEOUT_TAG: &str = "timeout";
pub const CHECK_TYPE_TAG: &str = "c";

/// Network types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkType {
    Clearnet,
    Tor,
    I2P,
    Loki,
    Other(String),
}

impl NetworkType {
    pub fn as_str(&self) -> &str {
        match self {
            NetworkType::Clearnet => "clearnet",
            NetworkType::Tor => "tor",
            NetworkType::I2P => "i2p",
            NetworkType::Loki => "loki",
            NetworkType::Other(s) => s,
        }
    }
}

impl std::str::FromStr for NetworkType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "clearnet" => NetworkType::Clearnet,
            "tor" => NetworkType::Tor,
            "i2p" => NetworkType::I2P,
            "loki" => NetworkType::Loki,
            _ => NetworkType::Other(s.to_string()),
        })
    }
}

/// Relay requirement (can be required or prohibited)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub name: String,
    pub required: bool,
}

impl Requirement {
    pub fn parse(s: &str) -> Self {
        if let Some(stripped) = s.strip_prefix('!') {
            Requirement {
                name: stripped.to_string(),
                required: false,
            }
        } else {
            Requirement {
                name: s.to_string(),
                required: true,
            }
        }
    }
}

impl std::fmt::Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.required {
            write!(f, "{}", self.name)
        } else {
            write!(f, "!{}", self.name)
        }
    }
}

/// Accepted/unaccepted kind (can be accepted or rejected)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindPolicy {
    pub kind: u16,
    pub accepted: bool,
}

impl KindPolicy {
    pub fn parse(s: &str) -> Result<Self, Nip66Error> {
        if let Some(stripped) = s.strip_prefix('!') {
            Ok(KindPolicy {
                kind: stripped
                    .parse()
                    .map_err(|_| Nip66Error::InvalidKindValue(s.to_string()))?,
                accepted: false,
            })
        } else {
            Ok(KindPolicy {
                kind: s
                    .parse()
                    .map_err(|_| Nip66Error::InvalidKindValue(s.to_string()))?,
                accepted: true,
            })
        }
    }
}

impl std::fmt::Display for KindPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.accepted {
            write!(f, "{}", self.kind)
        } else {
            write!(f, "!{}", self.kind)
        }
    }
}

/// Round-trip time metrics
#[derive(Debug, Clone, Default)]
pub struct RttMetrics {
    pub open_ms: Option<u32>,
    pub read_ms: Option<u32>,
    pub write_ms: Option<u32>,
}

/// Errors that can occur during NIP-66 operations.
#[derive(Debug, Error)]
pub enum Nip66Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("missing required 'd' tag")]
    MissingDTag,

    #[error("invalid RTT value: {0}")]
    InvalidRttValue(String),

    #[error("invalid NIP number: {0}")]
    InvalidNipNumber(String),

    #[error("invalid kind value: {0}")]
    InvalidKindValue(String),

    #[error("invalid frequency value: {0}")]
    InvalidFrequencyValue(String),

    #[error("invalid timeout value: {0}")]
    InvalidTimeoutValue(String),
}

/// A relay discovery event (kind 30166).
///
/// Documents relay characteristics, performance metrics, and capabilities.
#[derive(Debug, Clone)]
pub struct RelayDiscovery {
    /// The underlying Nostr event
    pub event: Event,
    /// Relay identifier (URL or hex pubkey)
    pub relay_id: String,
    /// Round-trip time metrics
    pub rtt: RttMetrics,
    /// Network type
    pub network_type: Option<NetworkType>,
    /// Relay type (e.g., "PrivateInbox")
    pub relay_type: Option<String>,
    /// Supported NIPs
    pub nips: Vec<u16>,
    /// Requirements (auth, writes, pow, payment, etc.)
    pub requirements: Vec<Requirement>,
    /// Topics
    pub topics: Vec<String>,
    /// Accepted/unaccepted kinds
    pub kind_policies: Vec<KindPolicy>,
    /// Geohash
    pub geohash: Option<String>,
    /// Optional NIP-11 document
    pub nip11_doc: Option<String>,
}

impl RelayDiscovery {
    /// Parse a relay discovery event from a generic event.
    pub fn from_event(event: Event) -> Result<Self, Nip66Error> {
        if event.kind != RELAY_DISCOVERY_KIND {
            return Err(Nip66Error::InvalidKind {
                expected: RELAY_DISCOVERY_KIND,
                actual: event.kind,
            });
        }

        // Extract d tag (required)
        let relay_id = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "d")
            .map(|tag| tag[1].clone())
            .ok_or(Nip66Error::MissingDTag)?;

        // Extract RTT metrics
        let mut rtt = RttMetrics::default();
        for tag in &event.tags {
            if tag.len() >= 2 {
                match tag[0].as_str() {
                    RTT_OPEN_TAG => {
                        rtt.open_ms = Some(
                            tag[1]
                                .parse()
                                .map_err(|_| Nip66Error::InvalidRttValue(tag[1].clone()))?,
                        );
                    }
                    RTT_READ_TAG => {
                        rtt.read_ms = Some(
                            tag[1]
                                .parse()
                                .map_err(|_| Nip66Error::InvalidRttValue(tag[1].clone()))?,
                        );
                    }
                    RTT_WRITE_TAG => {
                        rtt.write_ms = Some(
                            tag[1]
                                .parse()
                                .map_err(|_| Nip66Error::InvalidRttValue(tag[1].clone()))?,
                        );
                    }
                    _ => {}
                }
            }
        }

        // Extract network type
        let network_type = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == NETWORK_TYPE_TAG)
            .and_then(|tag| NetworkType::from_str(&tag[1]).ok());

        // Extract relay type
        let relay_type = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == RELAY_TYPE_TAG)
            .map(|tag| tag[1].clone());

        // Extract NIPs
        let nips: Result<Vec<u16>, Nip66Error> = event
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == NIP_SUPPORT_TAG)
            .map(|tag| {
                tag[1]
                    .parse()
                    .map_err(|_| Nip66Error::InvalidNipNumber(tag[1].clone()))
            })
            .collect();
        let nips = nips?;

        // Extract requirements
        let requirements: Vec<Requirement> = event
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == REQUIREMENT_TAG)
            .map(|tag| Requirement::parse(&tag[1]))
            .collect();

        // Extract topics
        let topics: Vec<String> = event
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == TOPIC_TAG)
            .map(|tag| tag[1].clone())
            .collect();

        // Extract kind policies
        let kind_policies: Result<Vec<KindPolicy>, Nip66Error> = event
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == KIND_TAG)
            .map(|tag| KindPolicy::parse(&tag[1]))
            .collect();
        let kind_policies = kind_policies?;

        // Extract geohash
        let geohash = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == GEOHASH_TAG)
            .map(|tag| tag[1].clone());

        // Extract NIP-11 document from content
        let nip11_doc = if event.content.is_empty() {
            None
        } else {
            Some(event.content.clone())
        };

        Ok(Self {
            event,
            relay_id,
            rtt,
            network_type,
            relay_type,
            nips,
            requirements,
            topics,
            kind_policies,
            geohash,
            nip11_doc,
        })
    }
}

/// Timeout specification for a check type
#[derive(Debug, Clone)]
pub struct CheckTimeout {
    pub check_type: Option<String>,
    pub timeout_ms: u32,
}

/// A relay monitor announcement event (kind 10166).
///
/// Announces a monitor's intent to publish relay discovery events.
#[derive(Debug, Clone)]
pub struct RelayMonitorAnnouncement {
    /// The underlying Nostr event
    pub event: Event,
    /// Update frequency in seconds
    pub frequency: Option<u32>,
    /// Timeout specifications
    pub timeouts: Vec<CheckTimeout>,
    /// Check types conducted (open, read, write, auth, nip11, dns, geo)
    pub check_types: Vec<String>,
    /// Monitor geohash
    pub geohash: Option<String>,
}

impl RelayMonitorAnnouncement {
    /// Parse a relay monitor announcement from a generic event.
    pub fn from_event(event: Event) -> Result<Self, Nip66Error> {
        if event.kind != RELAY_MONITOR_ANNOUNCEMENT_KIND {
            return Err(Nip66Error::InvalidKind {
                expected: RELAY_MONITOR_ANNOUNCEMENT_KIND,
                actual: event.kind,
            });
        }

        // Extract frequency
        let frequency = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == FREQUENCY_TAG)
            .map(|tag| {
                tag[1]
                    .parse()
                    .map_err(|_| Nip66Error::InvalidFrequencyValue(tag[1].clone()))
            })
            .transpose()?;

        // Extract timeouts
        let timeouts: Result<Vec<CheckTimeout>, Nip66Error> = event
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == TIMEOUT_TAG)
            .map(|tag| {
                if tag.len() >= 3 {
                    // timeout with check type: ["timeout", "open", "5000"]
                    Ok(CheckTimeout {
                        check_type: Some(tag[1].clone()),
                        timeout_ms: tag[2]
                            .parse()
                            .map_err(|_| Nip66Error::InvalidTimeoutValue(tag[2].clone()))?,
                    })
                } else {
                    // global timeout: ["timeout", "5000"]
                    Ok(CheckTimeout {
                        check_type: None,
                        timeout_ms: tag[1]
                            .parse()
                            .map_err(|_| Nip66Error::InvalidTimeoutValue(tag[1].clone()))?,
                    })
                }
            })
            .collect();
        let timeouts = timeouts?;

        // Extract check types
        let check_types: Vec<String> = event
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == CHECK_TYPE_TAG)
            .map(|tag| tag[1].clone())
            .collect();

        // Extract geohash
        let geohash = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == GEOHASH_TAG)
            .map(|tag| tag[1].clone());

        Ok(Self {
            event,
            frequency,
            timeouts,
            check_types,
            geohash,
        })
    }

    /// Get the timeout for a specific check type.
    ///
    /// Falls back to the global timeout if no specific timeout is defined.
    pub fn get_timeout(&self, check_type: &str) -> Option<u32> {
        // Look for specific timeout first
        if let Some(timeout) = self
            .timeouts
            .iter()
            .find(|t| t.check_type.as_deref() == Some(check_type))
        {
            return Some(timeout.timeout_ms);
        }

        // Fall back to global timeout
        self.timeouts
            .iter()
            .find(|t| t.check_type.is_none())
            .map(|t| t.timeout_ms)
    }
}

/// Validate a relay discovery event.
pub fn validate_relay_discovery(event: &Event) -> Result<(), Nip66Error> {
    if event.kind != RELAY_DISCOVERY_KIND {
        return Err(Nip66Error::InvalidKind {
            expected: RELAY_DISCOVERY_KIND,
            actual: event.kind,
        });
    }

    // Check for d tag
    if !event.tags.iter().any(|tag| tag.len() >= 2 && tag[0] == "d") {
        return Err(Nip66Error::MissingDTag);
    }

    Ok(())
}

/// Validate a relay monitor announcement event.
pub fn validate_relay_monitor_announcement(event: &Event) -> Result<(), Nip66Error> {
    if event.kind != RELAY_MONITOR_ANNOUNCEMENT_KIND {
        return Err(Nip66Error::InvalidKind {
            expected: RELAY_MONITOR_ANNOUNCEMENT_KIND,
            actual: event.kind,
        });
    }

    Ok(())
}

/// Check if an event is a relay discovery event.
pub fn is_relay_discovery(event: &Event) -> bool {
    event.kind == RELAY_DISCOVERY_KIND
}

/// Check if an event is a relay monitor announcement.
pub fn is_relay_monitor_announcement(event: &Event) -> bool {
    event.kind == RELAY_MONITOR_ANNOUNCEMENT_KIND
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_event(kind: u16, pubkey: &str, tags: Vec<Vec<String>>, content: &str) -> Event {
        Event {
            id: String::new(),
            kind,
            pubkey: pubkey.to_string(),
            tags,
            content: content.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            sig: String::new(),
        }
    }

    #[test]
    fn test_network_type_parse() {
        assert!(matches!(
            NetworkType::from_str("clearnet"),
            Ok(NetworkType::Clearnet)
        ));
        assert!(matches!(NetworkType::from_str("tor"), Ok(NetworkType::Tor)));
        assert!(matches!(NetworkType::from_str("i2p"), Ok(NetworkType::I2P)));
        assert!(matches!(
            NetworkType::from_str("loki"),
            Ok(NetworkType::Loki)
        ));
        assert!(matches!(
            NetworkType::from_str("custom"),
            Ok(NetworkType::Other(s)) if s == "custom"
        ));
    }

    #[test]
    fn test_requirement_parse() {
        let req1 = Requirement::parse("auth");
        assert_eq!(req1.name, "auth");
        assert!(req1.required);

        let req2 = Requirement::parse("!payment");
        assert_eq!(req2.name, "payment");
        assert!(!req2.required);
    }

    #[test]
    fn test_kind_policy_parse() {
        let policy1 = KindPolicy::parse("1").unwrap();
        assert_eq!(policy1.kind, 1);
        assert!(policy1.accepted);

        let policy2 = KindPolicy::parse("!4").unwrap();
        assert_eq!(policy2.kind, 4);
        assert!(!policy2.accepted);
    }

    #[test]
    fn test_relay_discovery_parsing() {
        let event = mock_event(
            RELAY_DISCOVERY_KIND,
            "monitor123",
            vec![
                vec!["d".to_string(), "wss://relay.example.com".to_string()],
                vec![RTT_OPEN_TAG.to_string(), "234".to_string()],
                vec![RTT_READ_TAG.to_string(), "100".to_string()],
                vec![NETWORK_TYPE_TAG.to_string(), "clearnet".to_string()],
                vec![NIP_SUPPORT_TAG.to_string(), "40".to_string()],
                vec![NIP_SUPPORT_TAG.to_string(), "33".to_string()],
                vec![REQUIREMENT_TAG.to_string(), "!payment".to_string()],
                vec![REQUIREMENT_TAG.to_string(), "auth".to_string()],
                vec![TOPIC_TAG.to_string(), "nsfw".to_string()],
                vec![KIND_TAG.to_string(), "1".to_string()],
                vec![KIND_TAG.to_string(), "!4".to_string()],
                vec![GEOHASH_TAG.to_string(), "ww8p1r4t8".to_string()],
            ],
            "{\"name\": \"Example Relay\"}",
        );

        let discovery = RelayDiscovery::from_event(event).unwrap();

        assert_eq!(discovery.relay_id, "wss://relay.example.com");
        assert_eq!(discovery.rtt.open_ms, Some(234));
        assert_eq!(discovery.rtt.read_ms, Some(100));
        assert_eq!(discovery.network_type, Some(NetworkType::Clearnet));
        assert_eq!(discovery.nips, vec![40, 33]);
        assert_eq!(discovery.requirements.len(), 2);
        assert_eq!(discovery.topics, vec!["nsfw"]);
        assert_eq!(discovery.kind_policies.len(), 2);
        assert_eq!(discovery.geohash, Some("ww8p1r4t8".to_string()));
        assert!(discovery.nip11_doc.is_some());
    }

    #[test]
    fn test_relay_discovery_missing_d_tag() {
        let event = mock_event(RELAY_DISCOVERY_KIND, "monitor123", vec![], "");

        let result = RelayDiscovery::from_event(event);
        assert!(matches!(result, Err(Nip66Error::MissingDTag)));
    }

    #[test]
    fn test_relay_monitor_announcement_parsing() {
        let event = mock_event(
            RELAY_MONITOR_ANNOUNCEMENT_KIND,
            "monitor456",
            vec![
                vec![
                    TIMEOUT_TAG.to_string(),
                    "open".to_string(),
                    "5000".to_string(),
                ],
                vec![
                    TIMEOUT_TAG.to_string(),
                    "read".to_string(),
                    "3000".to_string(),
                ],
                vec![FREQUENCY_TAG.to_string(), "3600".to_string()],
                vec![CHECK_TYPE_TAG.to_string(), "ws".to_string()],
                vec![CHECK_TYPE_TAG.to_string(), "nip11".to_string()],
                vec![GEOHASH_TAG.to_string(), "ww8p1r4t8".to_string()],
            ],
            "",
        );

        let announcement = RelayMonitorAnnouncement::from_event(event).unwrap();

        assert_eq!(announcement.frequency, Some(3600));
        assert_eq!(announcement.timeouts.len(), 2);
        assert_eq!(announcement.check_types, vec!["ws", "nip11"]);
        assert_eq!(announcement.geohash, Some("ww8p1r4t8".to_string()));

        // Test timeout lookup
        assert_eq!(announcement.get_timeout("open"), Some(5000));
        assert_eq!(announcement.get_timeout("read"), Some(3000));
        assert_eq!(announcement.get_timeout("unknown"), None);
    }

    #[test]
    fn test_relay_monitor_global_timeout() {
        let event = mock_event(
            RELAY_MONITOR_ANNOUNCEMENT_KIND,
            "monitor789",
            vec![
                vec![TIMEOUT_TAG.to_string(), "5000".to_string()],
                vec![CHECK_TYPE_TAG.to_string(), "ws".to_string()],
            ],
            "",
        );

        let announcement = RelayMonitorAnnouncement::from_event(event).unwrap();

        // Should return global timeout for any check type
        assert_eq!(announcement.get_timeout("ws"), Some(5000));
        assert_eq!(announcement.get_timeout("read"), Some(5000));
        assert_eq!(announcement.get_timeout("anything"), Some(5000));
    }

    #[test]
    fn test_validate_relay_discovery() {
        let event = mock_event(
            RELAY_DISCOVERY_KIND,
            "monitor123",
            vec![vec!["d".to_string(), "wss://relay.com".to_string()]],
            "",
        );

        assert!(validate_relay_discovery(&event).is_ok());
    }

    #[test]
    fn test_is_relay_discovery() {
        let event = mock_event(RELAY_DISCOVERY_KIND, "monitor123", vec![], "");

        assert!(is_relay_discovery(&event));
    }

    #[test]
    fn test_is_relay_monitor_announcement() {
        let event = mock_event(RELAY_MONITOR_ANNOUNCEMENT_KIND, "monitor123", vec![], "");

        assert!(is_relay_monitor_announcement(&event));
    }
}
