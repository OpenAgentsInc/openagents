//! Agent protocol types and helpers for NIP-28 communication
//!
//! This module implements agent-to-agent communication via NIP-28 public chat channels.
//! Agents exchange structured JSON messages to negotiate NIP-90 jobs and payments.
//!
//! # Network Field (NIP-89 Extension)
//!
//! The `network` field in `ServiceAnnouncement` follows NIP-89 conventions for service
//! provider discoverability. This allows customers to filter providers by Lightning network
//! before requesting jobs, rather than discovering network mismatch only when parsing bolt11.
//!
//! Valid networks: `mainnet`, `testnet`, `signet`, `regtest`

use nostr::{
    finalize_event, ChannelMessageEvent, ChannelMetadata, Event, EventTemplate, Keypair,
    KIND_CHANNEL_CREATION, KIND_CHANNEL_MESSAGE,
};
use nostr_client::RelayConnection;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Result type for agent operations
pub type AgentResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Default relay URL
pub const DEFAULT_RELAY: &str = "wss://relay.damus.io";

/// Provider mnemonic (for testing - in production use secure storage)
pub const PROVIDER_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Customer mnemonic (for testing - in production use secure storage)
pub const CUSTOMER_MNEMONIC: &str =
    "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

/// NIP-90 kind for text generation jobs
pub const KIND_JOB_TEXT_GENERATION: u16 = 5050;

/// Lightning network type for service announcements
///
/// Per NIP-89 service provider discoverability, providers should advertise
/// which Lightning network they operate on. This allows customers to filter
/// providers before requesting jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    /// Bitcoin mainnet (production)
    Mainnet,
    /// Bitcoin testnet
    Testnet,
    /// Bitcoin signet
    Signet,
    /// Bitcoin regtest (local development)
    Regtest,
}

impl Network {
    /// Returns the bolt11 invoice prefix for this network
    pub fn bolt11_prefix(&self) -> &'static str {
        match self {
            Network::Mainnet => "lnbc",
            Network::Testnet => "lntb",
            Network::Signet => "lntbs",
            Network::Regtest => "lnbcrt",
        }
    }

    /// Parse network from a bolt11 invoice prefix
    pub fn from_bolt11(invoice: &str) -> Option<Self> {
        if invoice.starts_with("lnbcrt") {
            Some(Network::Regtest)
        } else if invoice.starts_with("lntbs") {
            Some(Network::Signet)
        } else if invoice.starts_with("lntb") {
            Some(Network::Testnet)
        } else if invoice.starts_with("lnbc") {
            Some(Network::Mainnet)
        } else {
            None
        }
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Testnet => write!(f, "testnet"),
            Network::Signet => write!(f, "signet"),
            Network::Regtest => write!(f, "regtest"),
        }
    }
}

/// Messages exchanged between agents in the NIP-28 channel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AgentMessage {
    /// Provider announces available service
    ///
    /// The `network` field follows NIP-89 conventions, allowing customers
    /// to discover providers operating on their desired Lightning network.
    ServiceAnnouncement {
        kind: u16,
        price_msats: u64,
        spark_address: String,
        /// Lightning network (mainnet, testnet, signet, regtest)
        network: Network,
    },
    /// Customer requests a job
    JobRequest {
        kind: u16,
        prompt: String,
        max_tokens: u32,
    },
    /// Provider sends invoice for payment
    Invoice {
        job_id: String,
        bolt11: String,
        amount_msats: u64,
    },
    /// Customer confirms payment was sent
    PaymentSent {
        job_id: String,
        payment_id: String,
    },
    /// Provider delivers job result
    JobResult {
        job_id: String,
        result: String,
    },
}

/// Get current unix timestamp in seconds
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Create a NIP-28 channel for agent communication
pub async fn create_channel(
    relay: &RelayConnection,
    keypair: &Keypair,
    name: &str,
    description: &str,
) -> AgentResult<String> {
    let metadata = ChannelMetadata::new(name, description, "")
        .with_relays(vec![DEFAULT_RELAY.to_string()]);

    let template = EventTemplate {
        created_at: now(),
        kind: KIND_CHANNEL_CREATION,
        tags: vec![],
        content: metadata.to_json()?,
    };

    let event = finalize_event(&template, &keypair.private_key)?;
    let event_id = event.id.clone();

    relay
        .publish_event(&event, std::time::Duration::from_secs(10))
        .await?;

    Ok(event_id)
}

/// Send a message to a NIP-28 channel
pub async fn send_channel_message(
    relay: &RelayConnection,
    channel_id: &str,
    keypair: &Keypair,
    msg: &AgentMessage,
) -> AgentResult<String> {
    let msg_json = serde_json::to_string(msg)?;

    let channel_msg = ChannelMessageEvent::new(channel_id, DEFAULT_RELAY, &msg_json, now());

    let template = EventTemplate {
        created_at: now(),
        kind: KIND_CHANNEL_MESSAGE,
        tags: channel_msg.to_tags(),
        content: msg_json,
    };

    let event = finalize_event(&template, &keypair.private_key)?;
    let event_id = event.id.clone();

    relay
        .publish_event(&event, std::time::Duration::from_secs(10))
        .await?;

    Ok(event_id)
}

/// Subscribe to messages in a NIP-28 channel
pub async fn subscribe_to_channel(
    relay: &RelayConnection,
    channel_id: &str,
    subscription_id: &str,
) -> AgentResult<tokio::sync::mpsc::Receiver<Event>> {
    let filters = vec![serde_json::json!({
        "kinds": [KIND_CHANNEL_MESSAGE as u64],
        "#e": [channel_id]
    })];

    let rx = relay.subscribe_with_channel(subscription_id, &filters).await?;
    Ok(rx)
}

/// Parse a channel message into an AgentMessage
pub fn parse_agent_message(content: &str) -> Option<AgentMessage> {
    serde_json::from_str(content).ok()
}

/// Format timestamp for logging
pub fn log_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Simple timestamp format
    format!("{}", now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_bolt11_prefix() {
        assert_eq!(Network::Mainnet.bolt11_prefix(), "lnbc");
        assert_eq!(Network::Testnet.bolt11_prefix(), "lntb");
        assert_eq!(Network::Signet.bolt11_prefix(), "lntbs");
        assert_eq!(Network::Regtest.bolt11_prefix(), "lnbcrt");
    }

    #[test]
    fn test_network_from_bolt11() {
        // Mainnet
        assert_eq!(
            Network::from_bolt11("lnbc100n1pj..."),
            Some(Network::Mainnet)
        );

        // Testnet
        assert_eq!(
            Network::from_bolt11("lntb100n1pj..."),
            Some(Network::Testnet)
        );

        // Signet
        assert_eq!(
            Network::from_bolt11("lntbs100n1pj..."),
            Some(Network::Signet)
        );

        // Regtest
        assert_eq!(
            Network::from_bolt11("lnbcrt100n1pj..."),
            Some(Network::Regtest)
        );

        // Invalid
        assert_eq!(Network::from_bolt11("invalid"), None);
        assert_eq!(Network::from_bolt11("ln"), None);
    }

    #[test]
    fn test_network_display() {
        assert_eq!(format!("{}", Network::Mainnet), "mainnet");
        assert_eq!(format!("{}", Network::Testnet), "testnet");
        assert_eq!(format!("{}", Network::Signet), "signet");
        assert_eq!(format!("{}", Network::Regtest), "regtest");
    }

    #[test]
    fn test_network_serialization() {
        // Serialize
        assert_eq!(
            serde_json::to_string(&Network::Regtest).unwrap(),
            "\"regtest\""
        );
        assert_eq!(
            serde_json::to_string(&Network::Mainnet).unwrap(),
            "\"mainnet\""
        );

        // Deserialize
        assert_eq!(
            serde_json::from_str::<Network>("\"regtest\"").unwrap(),
            Network::Regtest
        );
        assert_eq!(
            serde_json::from_str::<Network>("\"mainnet\"").unwrap(),
            Network::Mainnet
        );
    }

    #[test]
    fn test_service_announcement_serialization() {
        let msg = AgentMessage::ServiceAnnouncement {
            kind: 5050,
            price_msats: 10_000,
            spark_address: "sp1abc...".to_string(),
            network: Network::Regtest,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"ServiceAnnouncement\""));
        assert!(json.contains("\"network\":\"regtest\""));
        assert!(json.contains("\"kind\":5050"));
        assert!(json.contains("\"price_msats\":10000"));

        // Round-trip
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_job_request_serialization() {
        let msg = AgentMessage::JobRequest {
            kind: 5050,
            prompt: "What is the meaning of life?".to_string(),
            max_tokens: 100,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"JobRequest\""));
        assert!(json.contains("\"prompt\":\"What is the meaning of life?\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_invoice_serialization() {
        let msg = AgentMessage::Invoice {
            job_id: "job_abc123".to_string(),
            bolt11: "lnbcrt100n1pj...".to_string(),
            amount_msats: 10_000,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"Invoice\""));
        assert!(json.contains("\"bolt11\":\"lnbcrt100n1pj...\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_payment_sent_serialization() {
        let msg = AgentMessage::PaymentSent {
            job_id: "job_abc123".to_string(),
            payment_id: "pay_xyz789".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"PaymentSent\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_job_result_serialization() {
        let msg = AgentMessage::JobResult {
            job_id: "job_abc123".to_string(),
            result: "The answer is 42.".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"JobResult\""));
        assert!(json.contains("\"result\":\"The answer is 42.\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_parse_agent_message() {
        let json = r#"{"type":"ServiceAnnouncement","kind":5050,"price_msats":10000,"spark_address":"sp1...","network":"regtest"}"#;
        let msg = parse_agent_message(json);
        assert!(msg.is_some());

        match msg.unwrap() {
            AgentMessage::ServiceAnnouncement { kind, network, .. } => {
                assert_eq!(kind, 5050);
                assert_eq!(network, Network::Regtest);
            }
            _ => panic!("Expected ServiceAnnouncement"),
        }
    }

    #[test]
    fn test_parse_agent_message_invalid() {
        assert!(parse_agent_message("not json").is_none());
        assert!(parse_agent_message("{}").is_none());
        assert!(parse_agent_message("{\"type\":\"Unknown\"}").is_none());
    }

    #[test]
    fn test_now_returns_reasonable_timestamp() {
        let ts = now();
        // Should be after 2024-01-01 (1704067200) and before 2100-01-01 (4102444800)
        assert!(ts > 1704067200);
        assert!(ts < 4102444800);
    }

    #[test]
    fn test_network_invoice_validation() {
        // Helper to validate invoice matches announced network
        fn validate_invoice(announced: Network, bolt11: &str) -> bool {
            Network::from_bolt11(bolt11) == Some(announced)
        }

        // Valid: regtest provider, regtest invoice
        assert!(validate_invoice(Network::Regtest, "lnbcrt100n1..."));

        // Invalid: regtest provider, mainnet invoice
        assert!(!validate_invoice(Network::Regtest, "lnbc100n1..."));

        // Valid: mainnet provider, mainnet invoice
        assert!(validate_invoice(Network::Mainnet, "lnbc100n1..."));

        // Invalid: mainnet provider, testnet invoice
        assert!(!validate_invoice(Network::Mainnet, "lntb100n1..."));
    }
}
