//! Chat state management.
//!
//! This module provides the central state machine for chat, managing:
//! - User identity (NIP-06 keypair)
//! - Channels (NIP-28)
//! - Messages
//! - DVM jobs (NIP-90)

use crate::channel::Channel;
use crate::message::ChatMessage;
use nostr::{
    derive_keypair, Event, EventTemplate, Keypair, Nip06Error, finalize_event,
    KIND_CHANNEL_CREATION, KIND_CHANNEL_MESSAGE, KIND_CHANNEL_METADATA,
    JobInput, JobRequest,
};
use nostr_relay::{Filter, PoolEvent, RelayPool};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

/// Errors that can occur in chat state management.
#[derive(Debug, Error)]
pub enum ChatError {
    #[error("identity error: {0}")]
    Identity(#[from] Nip06Error),

    #[error("relay error: {0}")]
    Relay(#[from] nostr_relay::ConnectionError),

    #[error("not connected")]
    NotConnected,

    #[error("no identity set")]
    NoIdentity,

    #[error("channel not found: {0}")]
    ChannelNotFound(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("nip-90 error: {0}")]
    Nip90(#[from] nostr::Nip90Error),

    #[error("event signing error: {0}")]
    Signing(#[from] nostr::Nip01Error),
}

/// Events emitted by the chat state.
#[derive(Debug, Clone)]
pub enum ChatEvent {
    /// Connected to relays
    Connected { relay_count: usize },
    /// Disconnected from all relays
    Disconnected,
    /// A new channel was discovered
    ChannelDiscovered { channel_id: String, name: String },
    /// Joined a channel
    ChannelJoined { channel_id: String },
    /// Left a channel
    ChannelLeft { channel_id: String },
    /// A new message was received
    MessageReceived {
        channel_id: String,
        message: ChatMessage,
    },
    /// A message was sent
    MessageSent { channel_id: String, event_id: String },
    /// DVM job submitted
    JobSubmitted { job_id: String, kind: u16 },
    /// DVM job status update
    JobStatusUpdate {
        job_id: String,
        status: String,
    },
    /// DVM job result received
    JobResult { job_id: String, content: String },
    /// Error occurred
    Error { message: String },
}

/// DVM job state.
#[derive(Debug, Clone)]
pub struct DvmJob {
    /// Job request event ID
    pub id: String,
    /// Job kind (5000-5999)
    pub kind: u16,
    /// Current status
    pub status: DvmJobStatus,
    /// Input data
    pub input: String,
    /// Result content (when completed)
    pub result: Option<String>,
    /// Timestamp
    pub created_at: u64,
}

/// DVM job status (aligned with NIP-90 JobStatus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DvmJobStatus {
    /// Job submitted, waiting for response
    Pending,
    /// Service provider requires payment before processing
    PaymentRequired {
        amount_msats: Option<u64>,
        bolt11: Option<String>,
    },
    /// Service provider is processing
    Processing,
    /// Job completed successfully
    Completed,
    /// Service provider returned partial results
    Partial,
    /// Job failed
    Failed(String),
}

/// The main chat state.
pub struct ChatState {
    /// User's Nostr identity
    identity: Option<Keypair>,
    /// Relay pool
    pool: RelayPool,
    /// Joined channels indexed by event ID
    channels: Arc<RwLock<HashMap<String, Channel>>>,
    /// All messages indexed by channel ID
    messages: Arc<RwLock<HashMap<String, Vec<ChatMessage>>>>,
    /// Active DVM jobs indexed by job ID
    dvm_jobs: Arc<RwLock<HashMap<String, DvmJob>>>,
    /// Event broadcast channel
    events_tx: broadcast::Sender<ChatEvent>,
    /// Active subscription IDs
    subscriptions: Arc<RwLock<Vec<String>>>,
}

impl ChatState {
    /// Create a new chat state with default relays.
    pub fn new() -> Self {
        let pool = nostr_relay::default_pool();
        let (events_tx, _) = broadcast::channel(1000);

        Self {
            identity: None,
            pool,
            channels: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            dvm_jobs: Arc::new(RwLock::new(HashMap::new())),
            events_tx,
            subscriptions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new chat state with custom relays.
    pub fn with_relays(relay_urls: Vec<String>) -> Self {
        let pool = RelayPool::with_relays(relay_urls);
        let (events_tx, _) = broadcast::channel(1000);

        Self {
            identity: None,
            pool,
            channels: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            dvm_jobs: Arc::new(RwLock::new(HashMap::new())),
            events_tx,
            subscriptions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Subscribe to chat events.
    pub fn subscribe(&self) -> broadcast::Receiver<ChatEvent> {
        self.events_tx.subscribe()
    }

    /// Set the user's identity from a mnemonic.
    pub fn set_identity_from_mnemonic(&mut self, mnemonic: &str) -> Result<(), ChatError> {
        let keypair = derive_keypair(mnemonic)?;
        info!("Identity set: npub={}", keypair.npub().unwrap_or_default());
        self.identity = Some(keypair);
        Ok(())
    }

    /// Set the user's identity directly.
    pub fn set_identity(&mut self, keypair: Keypair) {
        info!("Identity set: {}", keypair.public_key_hex());
        self.identity = Some(keypair);
    }

    /// Get the user's public key hex.
    pub fn pubkey(&self) -> Option<String> {
        self.identity.as_ref().map(|k| k.public_key_hex())
    }

    /// Get the user's npub.
    pub fn npub(&self) -> Option<String> {
        self.identity.as_ref().and_then(|k| k.npub().ok())
    }

    /// Check if identity is set.
    pub fn has_identity(&self) -> bool {
        self.identity.is_some()
    }

    /// Connect to relays.
    pub async fn connect(&self) -> Result<usize, ChatError> {
        info!("Connecting to relays...");

        let results = self.pool.connect_default().await;

        let connected_count = results.iter().filter(|(_, r)| r.is_ok()).count();
        info!("Connected to {} relays", connected_count);

        // Start listening for pool events
        self.spawn_event_handler();

        // Emit connected event
        let _ = self.events_tx.send(ChatEvent::Connected {
            relay_count: connected_count,
        });

        Ok(connected_count)
    }

    /// Spawn the event handler task.
    fn spawn_event_handler(&self) {
        let mut pool_events = self.pool.subscribe();
        let events_tx = self.events_tx.clone();
        let channels = self.channels.clone();
        let messages = self.messages.clone();
        let dvm_jobs = self.dvm_jobs.clone();
        let own_pubkey = self.pubkey();

        tokio::spawn(async move {
            while let Ok(event) = pool_events.recv().await {
                match event {
                    PoolEvent::Event {
                        subscription_id: _,
                        event,
                        ..
                    } => {
                        // Process different event kinds
                        let kind = event.kind;

                        if kind == KIND_CHANNEL_CREATION {
                            // New channel discovered
                            if let Ok(metadata) = nostr::ChannelMetadata::from_json(&event.content) {
                                let mut chs = channels.write().await;
                                if !chs.contains_key(&event.id) {
                                    let channel = Channel {
                                        id: event.id.clone(),
                                        metadata,
                                        creator_pubkey: event.pubkey.clone(),
                                        created_at: event.created_at,
                                        relay_url: None,
                                    };
                                    let name = channel.metadata.name.clone();
                                    chs.insert(event.id.clone(), channel);

                                    let _ = events_tx.send(ChatEvent::ChannelDiscovered {
                                        channel_id: event.id,
                                        name,
                                    });
                                }
                            }
                        } else if kind == KIND_CHANNEL_MESSAGE {
                            // Channel message
                            if let Some(channel_id) = Self::extract_channel_id(&event) {
                                let is_own = own_pubkey
                                    .as_ref()
                                    .map(|p| p == &event.pubkey)
                                    .unwrap_or(false);

                                let msg = ChatMessage::from_event(event, is_own);
                                let channel_id_clone = channel_id.clone();

                                // Store message
                                {
                                    let mut msgs = messages.write().await;
                                    msgs.entry(channel_id.clone())
                                        .or_insert_with(Vec::new)
                                        .push(msg.clone());
                                }

                                let _ = events_tx.send(ChatEvent::MessageReceived {
                                    channel_id: channel_id_clone,
                                    message: msg,
                                });
                            }
                        } else if kind == KIND_CHANNEL_METADATA {
                            // Channel metadata update
                            if let Some(channel_id) = Self::extract_channel_id(&event) {
                                if let Ok(metadata) = nostr::ChannelMetadata::from_json(&event.content) {
                                    let mut chs = channels.write().await;
                                    if let Some(channel) = chs.get_mut(&channel_id) {
                                        channel.metadata = metadata;
                                    }
                                }
                            }
                        } else if kind >= 6000 && kind <= 6999 {
                            // DVM job result
                            if let Some(job_id) = Self::extract_job_id(&event) {
                                let mut jobs = dvm_jobs.write().await;
                                if let Some(job) = jobs.get_mut(&job_id) {
                                    job.status = DvmJobStatus::Completed;
                                    job.result = Some(event.content.clone());
                                }

                                let _ = events_tx.send(ChatEvent::JobResult {
                                    job_id,
                                    content: event.content,
                                });
                            }
                        } else if kind == 7000 {
                            // DVM job feedback
                            if let Some(job_id) = Self::extract_job_id(&event) {
                                if let Some(status) = Self::extract_job_status(&event) {
                                    let mut jobs = dvm_jobs.write().await;
                                    if let Some(job) = jobs.get_mut(&job_id) {
                                        job.status = match status.as_str() {
                                            "processing" => DvmJobStatus::Processing,
                                            "error" => DvmJobStatus::Failed(event.content.clone()),
                                            "partial" => DvmJobStatus::Partial,
                                            "payment-required" => {
                                                // Extract amount and bolt11 from tags
                                                let amount = Self::extract_amount(&event);
                                                let bolt11 = Self::extract_bolt11(&event);
                                                DvmJobStatus::PaymentRequired { amount_msats: amount, bolt11 }
                                            }
                                            _ => job.status.clone(),
                                        };
                                    }

                                    let _ = events_tx.send(ChatEvent::JobStatusUpdate {
                                        job_id,
                                        status,
                                    });
                                }
                            }
                        }
                    }
                    PoolEvent::Disconnected { relay_url } => {
                        warn!("Relay disconnected: {}", relay_url);
                    }
                    PoolEvent::Error { relay_url, error } => {
                        warn!("Relay error on {}: {}", relay_url, error);
                        let _ = events_tx.send(ChatEvent::Error { message: error });
                    }
                    _ => {}
                }
            }
        });
    }

    /// Extract channel ID from event tags.
    fn extract_channel_id(event: &Event) -> Option<String> {
        for tag in &event.tags {
            if tag.len() >= 2 && tag[0] == "e" {
                // Check for root marker
                if tag.len() >= 4 && tag[3] == "root" {
                    return Some(tag[1].clone());
                }
                // Default to first e tag if no markers
                return Some(tag[1].clone());
            }
        }
        None
    }

    /// Extract job ID from event tags.
    fn extract_job_id(event: &Event) -> Option<String> {
        for tag in &event.tags {
            if tag.len() >= 2 && tag[0] == "e" {
                return Some(tag[1].clone());
            }
        }
        None
    }

    /// Extract job status from event tags.
    fn extract_job_status(event: &Event) -> Option<String> {
        for tag in &event.tags {
            if tag.len() >= 2 && tag[0] == "status" {
                return Some(tag[1].clone());
            }
        }
        None
    }

    /// Extract amount (in millisats) from event tags.
    fn extract_amount(event: &Event) -> Option<u64> {
        for tag in &event.tags {
            if tag.len() >= 2 && tag[0] == "amount" {
                return tag[1].parse().ok();
            }
        }
        None
    }

    /// Extract bolt11 invoice from event tags.
    fn extract_bolt11(event: &Event) -> Option<String> {
        for tag in &event.tags {
            // Check for amount tag with bolt11 as second element
            if tag.len() >= 3 && tag[0] == "amount" {
                return Some(tag[2].clone());
            }
        }
        None
    }

    /// Get all known channels.
    pub async fn channels(&self) -> Vec<Channel> {
        self.channels.read().await.values().cloned().collect()
    }

    /// Get a specific channel.
    pub async fn channel(&self, id: &str) -> Option<Channel> {
        self.channels.read().await.get(id).cloned()
    }

    /// Get messages for a channel.
    pub async fn messages(&self, channel_id: &str) -> Vec<ChatMessage> {
        self.messages
            .read()
            .await
            .get(channel_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all DVM jobs.
    pub async fn jobs(&self) -> Vec<DvmJob> {
        self.dvm_jobs.read().await.values().cloned().collect()
    }

    /// Get a specific job.
    pub async fn job(&self, id: &str) -> Option<DvmJob> {
        self.dvm_jobs.read().await.get(id).cloned()
    }

    /// Submit a DVM job request to relays (NIP-90).
    ///
    /// # Arguments
    /// * `kind` - The job kind (5000-5999)
    /// * `input` - The input data for the job
    /// * `params` - Optional key-value parameters
    /// * `preferred_providers` - Optional list of preferred service provider pubkeys
    /// * `max_bid_msats` - Optional maximum bid in millisats
    ///
    /// # Returns
    /// The event ID of the published job request
    pub async fn submit_job(
        &self,
        kind: u16,
        input: String,
        params: Vec<(String, String)>,
        preferred_providers: Option<Vec<String>>,
        max_bid_msats: Option<u64>,
    ) -> Result<String, ChatError> {
        let keypair = self.identity.as_ref().ok_or(ChatError::NoIdentity)?;

        // Build job request using NIP-90 builder
        let mut request = JobRequest::new(kind)?
            .add_input(JobInput::text(&input));

        // Add parameters
        for (key, value) in params {
            request = request.add_param(key, value);
        }

        // Add bid if specified
        if let Some(bid) = max_bid_msats {
            request = request.with_bid(bid);
        }

        // Add preferred service providers
        if let Some(providers) = preferred_providers {
            for provider in providers {
                request = request.add_service_provider(provider);
            }
        }

        // Get current timestamp
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Create event template
        let template = EventTemplate {
            kind: request.kind,
            tags: request.to_tags(),
            content: request.content.clone(),
            created_at,
        };

        // Sign the event
        let event = finalize_event(&template, &keypair.private_key)?;
        let event_id = event.id.clone();

        info!("Submitting DVM job: id={}, kind={}", event_id, kind);

        // Store job locally
        {
            let mut jobs = self.dvm_jobs.write().await;
            jobs.insert(
                event_id.clone(),
                DvmJob {
                    id: event_id.clone(),
                    kind,
                    status: DvmJobStatus::Pending,
                    input: input.clone(),
                    result: None,
                    created_at,
                },
            );
        }

        // Publish to relays
        self.pool.publish(event).await;

        // Emit event
        let _ = self.events_tx.send(ChatEvent::JobSubmitted {
            job_id: event_id.clone(),
            kind,
        });

        Ok(event_id)
    }

    /// Subscribe to DVM results and feedback for jobs we've submitted.
    pub async fn subscribe_to_dvm_results(&self) -> Result<(), ChatError> {
        let pubkey = self.pubkey().ok_or(ChatError::NoIdentity)?;

        // Subscribe to job results (6000-6999) and feedback (7000)
        // where we are tagged as the customer (p tag)
        let kinds: Vec<u16> = (6000..=6999).chain(std::iter::once(7000)).collect();

        let filter = Filter::new()
            .kinds(kinds)
            .pubkey_refs(vec![pubkey]);

        let sub_id = self.pool.subscribe_all(vec![filter]).await?;

        {
            let mut subs = self.subscriptions.write().await;
            subs.push(sub_id);
        }

        info!("Subscribed to DVM results and feedback");
        Ok(())
    }

    /// Join a channel by subscribing to its messages.
    pub async fn join_channel(&self, channel_id: &str) -> Result<(), ChatError> {
        info!("Joining channel: {}", channel_id);

        // Subscribe to channel messages
        let filter = Filter::new()
            .kinds(vec![KIND_CHANNEL_MESSAGE])
            .event_refs(vec![channel_id.to_string()])
            .limit(100);

        let sub_id = self.pool.subscribe_all(vec![filter]).await?;

        {
            let mut subs = self.subscriptions.write().await;
            subs.push(sub_id);
        }

        let _ = self.events_tx.send(ChatEvent::ChannelJoined {
            channel_id: channel_id.to_string(),
        });

        Ok(())
    }

    /// Disconnect from all relays.
    pub async fn disconnect(&self) {
        info!("Disconnecting from all relays");
        self.pool.disconnect_all().await;
        let _ = self.events_tx.send(ChatEvent::Disconnected);
    }

    /// Get the number of connected relays.
    pub async fn connected_count(&self) -> usize {
        self.pool.connected_count().await
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_state_new() {
        let state = ChatState::new();
        assert!(!state.has_identity());
        assert!(state.pubkey().is_none());
    }

    #[test]
    fn test_set_identity_from_mnemonic() {
        let mut state = ChatState::new();
        let mnemonic = "leader monkey parrot ring guide accident before fence cannon height naive bean";

        state.set_identity_from_mnemonic(mnemonic).unwrap();

        assert!(state.has_identity());
        assert!(state.pubkey().is_some());
        assert!(state.npub().is_some());
    }

    #[test]
    fn test_dvm_job_status() {
        let job = DvmJob {
            id: "job123".to_string(),
            kind: 5050,
            status: DvmJobStatus::Pending,
            input: "test input".to_string(),
            result: None,
            created_at: 1234567890,
        };

        assert_eq!(job.status, DvmJobStatus::Pending);
    }

    #[tokio::test]
    async fn test_channels_empty() {
        let state = ChatState::new();
        let channels = state.channels().await;
        assert!(channels.is_empty());
    }

    #[tokio::test]
    async fn test_messages_empty() {
        let state = ChatState::new();
        let messages = state.messages("nonexistent").await;
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_jobs_empty() {
        let state = ChatState::new();
        let jobs = state.jobs().await;
        assert!(jobs.is_empty());
    }
}
