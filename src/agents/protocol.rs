//! Agent protocol types and helpers for NIP-28 communication

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

/// Messages exchanged between agents in the NIP-28 channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentMessage {
    /// Provider announces available service
    ServiceAnnouncement {
        kind: u16,
        price_msats: u64,
        spark_address: String,
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
