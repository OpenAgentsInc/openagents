//! Agent protocol types and helpers for NIP-90 compute marketplace
//!
//! This module implements the NIP-90 Data Vending Machine protocol for compute jobs.
//! The primary flow uses direct Nostr events - NIP-28 channels are optional for coordination.
//!
//! # Primary Flow (Direct NIP-90 Events)
//!
//! 1. Provider publishes NIP-89 handler info (kind:31990)
//! 2. Customer discovers providers and sends job request (kind:5xxx)
//! 3. Provider sends job feedback with invoice (kind:7000)
//! 4. Customer pays Lightning invoice
//! 5. Provider delivers job result (kind:6xxx)
//!
//! # Optional: NIP-28 Channel Coordination
//!
//! For multi-party coordination or real-time chat, agents can optionally use NIP-28 channels.
//! This is NOT required for the core compute flow.
//!
//! # Network Field (NIP-89 Extension)
//!
//! The `network` field follows NIP-89 conventions for service provider discoverability.
//! This allows customers to filter providers by Lightning network before requesting jobs.
//!
//! Valid networks: `mainnet`, `testnet`, `signet`, `regtest`

use crate::agents::RelayApi;
use nostr::{
    finalize_event, ChannelMessageEvent, ChannelMetadata, Event, EventTemplate, Keypair,
    KIND_CHANNEL_CREATION, KIND_CHANNEL_MESSAGE,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ============================================================================
// NIP-90 Event Kinds
// ============================================================================

/// NIP-90 Job Request for text generation (customer -> provider)
pub const KIND_JOB_REQUEST_TEXT: u16 = 5050;

/// NIP-90 Job Result for text generation (provider -> customer)
pub const KIND_JOB_RESULT_TEXT: u16 = 6050;

/// NIP-90 Job Feedback (provider -> customer, includes payment-required status)
pub const KIND_JOB_FEEDBACK: u16 = 7000;

/// Job status values for feedback events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobStatus {
    /// Payment required before processing
    PaymentRequired,
    /// Job is being processed
    Processing,
    /// Job completed successfully
    Success,
    /// Job failed with error
    Error,
    /// Job was cancelled
    Cancelled,
}

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
        /// Provider's public key for targeting
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_pubkey: Option<String>,
        /// Available models
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        models: Vec<String>,
        /// Capabilities (e.g., "text-generation", "code-completion")
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        capabilities: Vec<String>,
    },
    /// Customer requests a job
    JobRequest {
        kind: u16,
        prompt: String,
        max_tokens: u32,
        /// Target specific provider by pubkey (for multi-provider channels)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_provider: Option<String>,
    },
    /// Provider sends invoice for payment
    Invoice {
        job_id: String,
        bolt11: String,
        amount_msats: u64,
        /// Payment hash for verification (hex-encoded)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payment_hash: Option<String>,
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
    /// Streaming chunk from provider (for real-time token delivery)
    StreamChunk {
        job_id: String,
        chunk: String,
        is_final: bool,
    },
    /// Customer notifies provider that HTLC payment is locked (escrow)
    ///
    /// Used with --htlc mode for trustless conditional payments.
    /// Funds are held in escrow until the preimage is revealed.
    HtlcLocked {
        job_id: String,
        /// Payment hash (hex-encoded) - provider needs this to verify HTLC
        payment_hash: String,
        /// Amount locked in millisatoshis
        amount_msats: u64,
        /// Time until HTLC expires (seconds from now)
        expiry_secs: u64,
    },
    /// Customer releases preimage after receiving result
    ///
    /// Once the customer receives the JobResult and is satisfied,
    /// they release the preimage allowing the provider to claim the payment.
    PreimageRelease {
        job_id: String,
        /// Preimage (hex-encoded) - provider claims payment with this
        preimage: String,
    },
}

/// Get current unix timestamp in seconds
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// ============================================================================
// NIP-90 Direct Event Functions (Primary Flow)
// ============================================================================

/// Publish a NIP-90 job request event (kind:5xxx)
///
/// This is the primary way to request compute from a provider.
/// The provider will respond with a feedback event (kind:7000) containing an invoice.
pub async fn publish_job_request(
    relay: &dyn RelayApi,
    keypair: &Keypair,
    provider_pubkey: &str,
    prompt: &str,
    max_tokens: u32,
    kind: u16,
) -> AgentResult<String> {
    // Build tags per NIP-90 spec
    let tags = vec![
        vec!["p".to_string(), provider_pubkey.to_string()],
        vec!["param".to_string(), "max_tokens".to_string(), max_tokens.to_string()],
    ];

    let template = EventTemplate {
        created_at: now(),
        kind,
        tags,
        content: prompt.to_string(),
    };

    let event = finalize_event(&template, &keypair.private_key)?;
    let event_id = event.id.clone();

    relay.publish_event(&event, Duration::from_secs(10)).await?;

    Ok(event_id)
}

/// Publish a NIP-90 job feedback event (kind:7000)
///
/// Used by providers to send status updates including payment-required with invoice.
pub async fn publish_job_feedback(
    relay: &dyn RelayApi,
    keypair: &Keypair,
    job_request_id: &str,
    customer_pubkey: &str,
    status: JobStatus,
    bolt11: Option<&str>,
    amount_msats: Option<u64>,
) -> AgentResult<String> {
    let status_str = match status {
        JobStatus::PaymentRequired => "payment-required",
        JobStatus::Processing => "processing",
        JobStatus::Success => "success",
        JobStatus::Error => "error",
        JobStatus::Cancelled => "cancelled",
    };

    // Build tags per NIP-90 spec
    let mut tags = vec![
        vec!["e".to_string(), job_request_id.to_string()],
        vec!["p".to_string(), customer_pubkey.to_string()],
        vec!["status".to_string(), status_str.to_string()],
    ];

    // Add amount tag if provided
    if let Some(amount) = amount_msats {
        tags.push(vec!["amount".to_string(), amount.to_string(), "msats".to_string()]);
    }

    // Content is the bolt11 invoice for payment-required status
    let content = bolt11.unwrap_or("").to_string();

    let template = EventTemplate {
        created_at: now(),
        kind: KIND_JOB_FEEDBACK,
        tags,
        content,
    };

    let event = finalize_event(&template, &keypair.private_key)?;
    let event_id = event.id.clone();

    relay.publish_event(&event, Duration::from_secs(10)).await?;

    Ok(event_id)
}

/// Publish a NIP-90 job result event (kind:6xxx)
///
/// Used by providers to deliver the completed job result.
pub async fn publish_job_result(
    relay: &dyn RelayApi,
    keypair: &Keypair,
    job_request_id: &str,
    customer_pubkey: &str,
    result: &str,
    kind: u16,
) -> AgentResult<String> {
    // Build tags per NIP-90 spec
    let tags = vec![
        vec!["e".to_string(), job_request_id.to_string()],
        vec!["p".to_string(), customer_pubkey.to_string()],
        vec!["request".to_string(), job_request_id.to_string()],
    ];

    let template = EventTemplate {
        created_at: now(),
        kind,
        tags,
        content: result.to_string(),
    };

    let event = finalize_event(&template, &keypair.private_key)?;
    let event_id = event.id.clone();

    relay.publish_event(&event, Duration::from_secs(10)).await?;

    Ok(event_id)
}

/// Subscribe to job requests targeting a specific provider pubkey
///
/// Used by providers to listen for incoming job requests.
pub async fn subscribe_job_requests(
    relay: &dyn RelayApi,
    provider_pubkey: &str,
    kinds: &[u16],
) -> AgentResult<tokio::sync::mpsc::Receiver<Event>> {
    let kind_values: Vec<u64> = kinds.iter().map(|k| *k as u64).collect();

    let filters = vec![serde_json::json!({
        "kinds": kind_values,
        "#p": [provider_pubkey]
    })];

    let subscription_id = format!("job-requests-{}-{}", provider_pubkey, Uuid::new_v4());
    let rx = relay
        .subscribe_with_channel(&subscription_id, &filters)
        .await?;
    Ok(rx)
}

/// Subscribe to job feedback/results for a specific job request
///
/// Used by customers to listen for provider responses.
pub async fn subscribe_job_responses(
    relay: &dyn RelayApi,
    job_request_id: &str,
) -> AgentResult<tokio::sync::mpsc::Receiver<Event>> {
    let filters = vec![serde_json::json!({
        "kinds": [KIND_JOB_FEEDBACK as u64, KIND_JOB_RESULT_TEXT as u64],
        "#e": [job_request_id]
    })];

    let subscription_id = format!("job-responses-{}-{}", job_request_id, Uuid::new_v4());
    let rx = relay
        .subscribe_with_channel(&subscription_id, &filters)
        .await?;
    Ok(rx)
}

/// Parse a NIP-90 job request event
///
/// Returns (prompt, max_tokens, target_provider) if valid
pub fn parse_job_request(event: &Event) -> Option<(String, u32, Option<String>)> {
    // Content is the prompt
    let prompt = event.content.clone();

    // Parse max_tokens from param tag
    let max_tokens = event.tags.iter()
        .find(|t| t.len() >= 3 && t[0] == "param" && t[1] == "max_tokens")
        .and_then(|t| t[2].parse().ok())
        .unwrap_or(256);

    // Get target provider from p tag
    let target_provider = event.tags.iter()
        .find(|t| t.len() >= 2 && t[0] == "p")
        .map(|t| t[1].clone());

    Some((prompt, max_tokens, target_provider))
}

/// Parse a NIP-90 job feedback event
///
/// Returns (job_request_id, status, bolt11, amount_msats) if valid
pub fn parse_job_feedback(event: &Event) -> Option<(String, JobStatus, Option<String>, Option<u64>)> {
    // Get job request ID from e tag
    let job_id = event.tags.iter()
        .find(|t| t.len() >= 2 && t[0] == "e")
        .map(|t| t[1].clone())?;

    // Parse status from status tag
    let status_str = event.tags.iter()
        .find(|t| t.len() >= 2 && t[0] == "status")
        .map(|t| t[1].as_str())?;

    let status = match status_str {
        "payment-required" => JobStatus::PaymentRequired,
        "processing" => JobStatus::Processing,
        "success" => JobStatus::Success,
        "error" => JobStatus::Error,
        "cancelled" => JobStatus::Cancelled,
        _ => return None,
    };

    // Bolt11 is in content for payment-required
    let bolt11 = if !event.content.is_empty() && event.content.starts_with("ln") {
        Some(event.content.clone())
    } else {
        None
    };

    // Parse amount from amount tag
    let amount = event.tags.iter()
        .find(|t| t.len() >= 2 && t[0] == "amount")
        .and_then(|t| t[1].parse().ok());

    Some((job_id, status, bolt11, amount))
}

/// Parse a NIP-90 job result event
///
/// Returns (job_request_id, result_content) if valid
pub fn parse_job_result(event: &Event) -> Option<(String, String)> {
    // Get job request ID from e tag
    let job_id = event.tags.iter()
        .find(|t| t.len() >= 2 && t[0] == "e")
        .map(|t| t[1].clone())?;

    Some((job_id, event.content.clone()))
}

// ============================================================================
// NIP-28 Channel Functions (Optional Coordination Layer)
// ============================================================================

/// Create a NIP-28 channel for agent communication (OPTIONAL)
///
/// NIP-28 channels are NOT required for the core NIP-90 compute flow.
/// Use this for multi-party coordination or real-time discussion.
pub async fn create_channel(
    relay: &dyn RelayApi,
    keypair: &Keypair,
    name: &str,
    description: &str,
) -> AgentResult<String> {
    let metadata = ChannelMetadata::new(name, description, "").with_relays(relay.relay_urls());

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
    relay: &dyn RelayApi,
    channel_id: &str,
    keypair: &Keypair,
    msg: &AgentMessage,
) -> AgentResult<String> {
    let msg_json = serde_json::to_string(msg)?;

    let relay_url = relay.relay_url();
    let channel_msg = ChannelMessageEvent::new(channel_id, &relay_url, &msg_json, now());

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
    relay: &dyn RelayApi,
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
            provider_pubkey: None,
            models: vec![],
            capabilities: vec![],
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
    fn test_service_announcement_with_models() {
        let msg = AgentMessage::ServiceAnnouncement {
            kind: 5050,
            price_msats: 10_000,
            spark_address: "sp1abc...".to_string(),
            network: Network::Regtest,
            provider_pubkey: Some("abc123".to_string()),
            models: vec!["llama3.2".to_string(), "codellama".to_string()],
            capabilities: vec!["text-generation".to_string()],
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"models\":[\"llama3.2\",\"codellama\"]"));
        assert!(json.contains("\"provider_pubkey\":\"abc123\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_job_request_serialization() {
        let msg = AgentMessage::JobRequest {
            kind: 5050,
            prompt: "What is the meaning of life?".to_string(),
            max_tokens: 100,
            target_provider: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"JobRequest\""));
        assert!(json.contains("\"prompt\":\"What is the meaning of life?\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_job_request_with_target() {
        let msg = AgentMessage::JobRequest {
            kind: 5050,
            prompt: "Hello".to_string(),
            max_tokens: 100,
            target_provider: Some("provider_pubkey_123".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"target_provider\":\"provider_pubkey_123\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_invoice_serialization() {
        let msg = AgentMessage::Invoice {
            job_id: "job_abc123".to_string(),
            bolt11: "lnbcrt100n1pj...".to_string(),
            amount_msats: 10_000,
            payment_hash: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"Invoice\""));
        assert!(json.contains("\"bolt11\":\"lnbcrt100n1pj...\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_invoice_with_payment_hash() {
        let msg = AgentMessage::Invoice {
            job_id: "job_abc123".to_string(),
            bolt11: "lnbcrt100n1pj...".to_string(),
            amount_msats: 10_000,
            payment_hash: Some("abc123def456".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"payment_hash\":\"abc123def456\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_stream_chunk_serialization() {
        let msg = AgentMessage::StreamChunk {
            job_id: "job_abc123".to_string(),
            chunk: "Hello ".to_string(),
            is_final: false,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"StreamChunk\""));
        assert!(json.contains("\"chunk\":\"Hello \""));
        assert!(json.contains("\"is_final\":false"));

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

    #[test]
    fn test_htlc_locked_serialization() {
        let msg = AgentMessage::HtlcLocked {
            job_id: "job_abc123".to_string(),
            payment_hash: "deadbeef1234567890abcdef".to_string(),
            amount_msats: 10_000,
            expiry_secs: 3600,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"HtlcLocked\""));
        assert!(json.contains("\"payment_hash\":\"deadbeef1234567890abcdef\""));
        assert!(json.contains("\"expiry_secs\":3600"));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_preimage_release_serialization() {
        let msg = AgentMessage::PreimageRelease {
            job_id: "job_abc123".to_string(),
            preimage: "cafebabe1234567890abcdef".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"PreimageRelease\""));
        assert!(json.contains("\"preimage\":\"cafebabe1234567890abcdef\""));

        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    // ========================================================================
    // NIP-90 Direct Event Tests
    // ========================================================================

    #[test]
    fn test_job_status_serialization() {
        assert_eq!(
            serde_json::to_string(&JobStatus::PaymentRequired).unwrap(),
            "\"payment-required\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Processing).unwrap(),
            "\"processing\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Success).unwrap(),
            "\"success\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }

    #[test]
    fn test_job_status_deserialization() {
        assert_eq!(
            serde_json::from_str::<JobStatus>("\"payment-required\"").unwrap(),
            JobStatus::PaymentRequired
        );
        assert_eq!(
            serde_json::from_str::<JobStatus>("\"processing\"").unwrap(),
            JobStatus::Processing
        );
        assert_eq!(
            serde_json::from_str::<JobStatus>("\"success\"").unwrap(),
            JobStatus::Success
        );
    }

    #[test]
    fn test_parse_job_request() {
        use nostr::Event;

        // Create a mock event
        let event = Event {
            id: "abc123".to_string(),
            pubkey: "customer_pubkey".to_string(),
            created_at: 1234567890,
            kind: KIND_JOB_REQUEST_TEXT,
            tags: vec![
                vec!["p".to_string(), "provider_pubkey".to_string()],
                vec!["param".to_string(), "max_tokens".to_string(), "500".to_string()],
            ],
            content: "What is the meaning of life?".to_string(),
            sig: "".to_string(),
        };

        let result = parse_job_request(&event);
        assert!(result.is_some());

        let (prompt, max_tokens, target) = result.unwrap();
        assert_eq!(prompt, "What is the meaning of life?");
        assert_eq!(max_tokens, 500);
        assert_eq!(target, Some("provider_pubkey".to_string()));
    }

    #[test]
    fn test_parse_job_request_default_tokens() {
        use nostr::Event;

        // Event without max_tokens param should default to 256
        let event = Event {
            id: "abc123".to_string(),
            pubkey: "customer_pubkey".to_string(),
            created_at: 1234567890,
            kind: KIND_JOB_REQUEST_TEXT,
            tags: vec![
                vec!["p".to_string(), "provider_pubkey".to_string()],
            ],
            content: "Hello".to_string(),
            sig: "".to_string(),
        };

        let (_, max_tokens, _) = parse_job_request(&event).unwrap();
        assert_eq!(max_tokens, 256);
    }

    #[test]
    fn test_parse_job_feedback_payment_required() {
        use nostr::Event;

        let event = Event {
            id: "feedback123".to_string(),
            pubkey: "provider_pubkey".to_string(),
            created_at: 1234567890,
            kind: KIND_JOB_FEEDBACK,
            tags: vec![
                vec!["e".to_string(), "job_request_id".to_string()],
                vec!["p".to_string(), "customer_pubkey".to_string()],
                vec!["status".to_string(), "payment-required".to_string()],
                vec!["amount".to_string(), "10000".to_string(), "msats".to_string()],
            ],
            content: "lnbcrt100n1pj...".to_string(),
            sig: "".to_string(),
        };

        let result = parse_job_feedback(&event);
        assert!(result.is_some());

        let (job_id, status, bolt11, amount) = result.unwrap();
        assert_eq!(job_id, "job_request_id");
        assert_eq!(status, JobStatus::PaymentRequired);
        assert_eq!(bolt11, Some("lnbcrt100n1pj...".to_string()));
        assert_eq!(amount, Some(10000));
    }

    #[test]
    fn test_parse_job_feedback_processing() {
        use nostr::Event;

        let event = Event {
            id: "feedback456".to_string(),
            pubkey: "provider_pubkey".to_string(),
            created_at: 1234567890,
            kind: KIND_JOB_FEEDBACK,
            tags: vec![
                vec!["e".to_string(), "job_request_id".to_string()],
                vec!["p".to_string(), "customer_pubkey".to_string()],
                vec!["status".to_string(), "processing".to_string()],
            ],
            content: "".to_string(),
            sig: "".to_string(),
        };

        let (_, status, bolt11, _) = parse_job_feedback(&event).unwrap();
        assert_eq!(status, JobStatus::Processing);
        assert_eq!(bolt11, None);
    }

    #[test]
    fn test_parse_job_result() {
        use nostr::Event;

        let event = Event {
            id: "result789".to_string(),
            pubkey: "provider_pubkey".to_string(),
            created_at: 1234567890,
            kind: KIND_JOB_RESULT_TEXT,
            tags: vec![
                vec!["e".to_string(), "job_request_id".to_string()],
                vec!["p".to_string(), "customer_pubkey".to_string()],
                vec!["request".to_string(), "job_request_id".to_string()],
            ],
            content: "The meaning of life is 42.".to_string(),
            sig: "".to_string(),
        };

        let result = parse_job_result(&event);
        assert!(result.is_some());

        let (job_id, content) = result.unwrap();
        assert_eq!(job_id, "job_request_id");
        assert_eq!(content, "The meaning of life is 42.");
    }

    #[test]
    fn test_nip90_event_kinds() {
        // Verify event kinds match NIP-90 spec
        assert_eq!(KIND_JOB_REQUEST_TEXT, 5050);
        assert_eq!(KIND_JOB_RESULT_TEXT, 6050);
        assert_eq!(KIND_JOB_FEEDBACK, 7000);
    }
}
