//! Nostr relay transport for Bifrost protocol

use crate::bifrost::BifrostMessage;
use crate::{Error, Result};
use nostr_client::{PoolConfig, RelayPool};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};
use tokio::time::{Duration, sleep};

// NIP-44 encryption support
use bitcoin::secp256k1::{PublicKey, SECP256K1, SecretKey};
use nostr::{Event, decrypt_v2, encrypt_v2};

/// Event kind for Bifrost messages (ephemeral, not stored by relays)
pub const BIFROST_EVENT_KIND: u16 = 28000;

/// Configuration for Nostr transport
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Relay URLs to connect to
    pub relays: Vec<String>,
    /// Our secret key (secp256k1)
    pub secret_key: [u8; 32],
    /// Threshold peer public keys
    pub peer_pubkeys: Vec<[u8; 32]>,
    /// Event kind for Bifrost messages (default: 28000)
    pub event_kind: u16,
    /// Message timeout in seconds
    pub message_timeout: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
            ],
            secret_key: [0; 32],
            peer_pubkeys: Vec::new(),
            event_kind: BIFROST_EVENT_KIND,
            message_timeout: 30,
            max_retries: 3,
        }
    }
}

/// Message envelope for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageEnvelope {
    /// Session ID for request/response matching
    session_id: String,
    /// Message type (sign_req, sign_res, etc.)
    msg_type: String,
    /// Serialized Bifrost message
    message: String,
    /// Timestamp
    timestamp: u64,
}

/// Pending request tracker
#[derive(Debug)]
struct PendingRequest {
    /// Request timestamp
    started_at: u64,
    /// Response channel
    tx: mpsc::Sender<BifrostMessage>,
    /// Number of responses received
    responses_received: usize,
    /// Required number of responses
    responses_required: usize,
}

/// Nostr transport for Bifrost messages
pub struct NostrTransport {
    /// Configuration
    config: TransportConfig,
    /// Relay pool for Nostr connections
    relay_pool: Option<Arc<RelayPool>>,
    /// Pending requests (by session ID)
    pending: Arc<RwLock<HashMap<String, PendingRequest>>>,
    /// Received messages (deduplicated by message ID)
    seen_messages: Arc<RwLock<HashMap<String, u64>>>,
    /// Incoming message channel
    incoming_tx: mpsc::Sender<BifrostMessage>,
    /// Incoming message receiver
    incoming_rx: Arc<RwLock<mpsc::Receiver<BifrostMessage>>>,
    /// Connection state tracking
    connected: Arc<RwLock<bool>>,
}

impl NostrTransport {
    /// Create a new Nostr transport (not yet connected)
    pub fn new(config: TransportConfig) -> Result<Self> {
        let (incoming_tx, incoming_rx) = mpsc::channel(100);

        Ok(Self {
            config,
            relay_pool: None,
            pending: Arc::new(RwLock::new(HashMap::new())),
            seen_messages: Arc::new(RwLock::new(HashMap::new())),
            incoming_tx,
            incoming_rx: Arc::new(RwLock::new(incoming_rx)),
            connected: Arc::new(RwLock::new(false)),
        })
    }

    /// Encrypt content for a specific peer using NIP-44
    fn encrypt_for_peer(&self, content: &str, peer_pubkey: &[u8; 32]) -> Result<String> {
        // Convert peer pubkey to compressed format for NIP-44
        let pk = PublicKey::from_slice(&[vec![0x02], peer_pubkey.to_vec()].concat())
            .map_err(|e| Error::Crypto(format!("Invalid peer public key: {}", e)))?;

        let pk_bytes = pk.serialize();

        encrypt_v2(&self.config.secret_key, &pk_bytes, content)
            .map_err(|e| Error::Crypto(format!("NIP-44 encryption failed: {}", e)))
    }

    /// Decrypt content from a specific peer using NIP-44
    #[allow(dead_code)]
    fn decrypt_from_peer(&self, encrypted: &str, peer_pubkey: &[u8; 32]) -> Result<String> {
        // Convert peer pubkey to compressed format for NIP-44
        let pk = PublicKey::from_slice(&[vec![0x02], peer_pubkey.to_vec()].concat())
            .map_err(|e| Error::Crypto(format!("Invalid peer public key: {}", e)))?;

        let pk_bytes = pk.serialize();

        decrypt_v2(&self.config.secret_key, &pk_bytes, encrypted)
            .map_err(|e| Error::Crypto(format!("NIP-44 decryption failed: {}", e)))
    }

    /// Get our public key from secret key
    #[allow(dead_code)]
    fn get_our_pubkey(&self) -> Result<[u8; 32]> {
        let sk = SecretKey::from_slice(&self.config.secret_key)
            .map_err(|e| Error::Crypto(format!("Invalid secret key: {}", e)))?;

        let pk = PublicKey::from_secret_key(SECP256K1, &sk);

        // Return x-only pubkey (32 bytes)
        Ok(pk.x_only_public_key().0.serialize())
    }

    /// Process an incoming event from a relay
    async fn process_incoming_event(
        event: Event,
        secret_key: &[u8; 32],
        peer_pubkeys: &[[u8; 32]],
        incoming_tx: &mpsc::Sender<BifrostMessage>,
        pending: Arc<RwLock<HashMap<String, PendingRequest>>>,
        relay_pool: Option<Arc<RelayPool>>,
        _relays: &[String],
        event_kind: u16,
    ) -> Result<()> {
        // Find the sender's pubkey from the event author
        let author_pubkey_hex = event.pubkey.clone();
        let author_pubkey_bytes = hex::decode(&author_pubkey_hex)
            .map_err(|e| Error::Encoding(format!("Invalid author pubkey: {}", e)))?;

        if author_pubkey_bytes.len() != 32 {
            return Err(Error::Encoding(
                "Author pubkey must be 32 bytes".to_string(),
            ));
        }

        let mut author_pubkey = [0u8; 32];
        author_pubkey.copy_from_slice(&author_pubkey_bytes);

        // Verify sender is a known peer
        if !peer_pubkeys.contains(&author_pubkey) {
            return Err(Error::Protocol(format!(
                "Received message from unknown peer: {}",
                author_pubkey_hex
            )));
        }

        // Decrypt the event content using NIP-44
        let pk = PublicKey::from_slice(&[vec![0x02], author_pubkey.to_vec()].concat())
            .map_err(|e| Error::Crypto(format!("Invalid peer public key: {}", e)))?;

        let pk_bytes = pk.serialize();
        let decrypted_content = decrypt_v2(secret_key, &pk_bytes, &event.content)
            .map_err(|e| Error::Crypto(format!("Failed to decrypt event: {}", e)))?;

        // Deserialize the envelope
        let envelope: MessageEnvelope = serde_json::from_str(&decrypted_content)
            .map_err(|e| Error::Encoding(format!("Invalid message envelope: {}", e)))?;

        // Deserialize the inner message
        let message: BifrostMessage = serde_json::from_str(&envelope.message)
            .map_err(|e| Error::Encoding(format!("Invalid Bifrost message: {}", e)))?;

        // Auto-respond to ping messages with pong
        if let BifrostMessage::Ping(ping) = &message {
            // Send pong response automatically
            let pong = crate::bifrost::Pong {
                session_id: ping.session_id.clone(),
                ping_timestamp: ping.timestamp,
                pong_timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            };

            let pong_message = BifrostMessage::Pong(pong);

            // Encrypt and send pong back to sender
            if let Some(pool) = relay_pool {
                let pong_json = serde_json::to_string(&pong_message)
                    .map_err(|e| Error::Encoding(format!("Failed to serialize pong: {}", e)))?;

                let envelope = MessageEnvelope {
                    session_id: ping.session_id.clone(),
                    msg_type: "pong".to_string(),
                    message: pong_json,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                let envelope_json = serde_json::to_string(&envelope)
                    .map_err(|e| Error::Encoding(format!("Failed to serialize envelope: {}", e)))?;

                // Encrypt for the sender
                let encrypted = encrypt_v2(secret_key, &pk_bytes, &envelope_json)
                    .map_err(|e| Error::Crypto(format!("Failed to encrypt pong: {}", e)))?;

                // Create and publish event
                let sk = SecretKey::from_slice(secret_key)
                    .map_err(|e| Error::Crypto(format!("Invalid secret key: {}", e)))?;

                let _our_pk = PublicKey::from_secret_key(SECP256K1, &sk);

                // Create event template
                let event_template = nostr::EventTemplate {
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    kind: event_kind,
                    tags: vec![vec!["p".to_string(), author_pubkey_hex.clone()]],
                    content: encrypted,
                };

                // Sign the event
                let event = nostr::finalize_event(&event_template, secret_key)
                    .map_err(|e| Error::Signing(format!("Failed to sign pong event: {}", e)))?;

                // Publish to all relays
                let _ = pool.publish(&event).await;
            }
        }

        // Check if this is a response to a pending request
        let session_id = Self::extract_session_id(&message);
        let is_response = matches!(
            &message,
            // Two-phase FROST signing responses
            BifrostMessage::CommitmentResponse(_)
            | BifrostMessage::PartialSignature(_)
            // ECDH responses
            | BifrostMessage::EcdhResponse(_)
            // Utility responses
            | BifrostMessage::Pong(_)
        );

        if is_response && let Some(session_id) = session_id {
            // Try to route to pending request
            let pending_guard = pending.write().await;
            if let Some(pending_req) = pending_guard.get(&session_id) {
                // Send to the pending request's channel
                if pending_req.tx.send(message.clone()).await.is_ok() {
                    // Successfully routed to pending request
                    return Ok(());
                }
            }
        }

        // Forward to incoming channel for responder processing
        incoming_tx
            .send(message)
            .await
            .map_err(|e| Error::Transport(format!("Failed to forward message: {}", e)))?;

        Ok(())
    }

    /// Connect to Nostr relays and start listening for messages
    pub async fn connect(&mut self) -> Result<()> {
        // Create relay pool with config appropriate for Bifrost
        // We only require 1 confirmation since Bifrost may use a single relay
        let pool_config = PoolConfig {
            min_write_confirmations: 1,
            ..PoolConfig::default()
        };
        let pool = Arc::new(RelayPool::new(pool_config));

        // Add all configured relays
        for relay_url in &self.config.relays {
            pool.add_relay(relay_url).await.map_err(|e| {
                Error::Transport(format!("Failed to add relay {}: {}", relay_url, e))
            })?;
        }

        // Connect to all relays
        pool.connect_all()
            .await
            .map_err(|e| Error::Transport(format!("Failed to connect to relays: {}", e)))?;

        self.relay_pool = Some(pool.clone());

        // Mark as connected
        *self.connected.write().await = true;

        // Start subscription listener for incoming Bifrost messages
        let our_pubkey = self.get_our_pubkey()?;
        let our_pubkey_hex = hex::encode(our_pubkey);

        // Create filter for Bifrost messages directed to us
        let filter = serde_json::json!({
            "kinds": [self.config.event_kind],
            "#p": [our_pubkey_hex],
        });

        // Subscribe to relays
        let subscription_id = format!("bifrost-{}", hex::encode(&our_pubkey[..8]));
        let mut event_rx = pool
            .subscribe(&subscription_id, &[filter])
            .await
            .map_err(|e| Error::Transport(format!("Failed to subscribe to relays: {}", e)))?;

        // Spawn background task to process incoming events
        let incoming_tx = self.incoming_tx.clone();
        let secret_key = self.config.secret_key;
        let peer_pubkeys = self.config.peer_pubkeys.clone();
        let relay_pool_clone = pool.clone();
        let relays = self.config.relays.clone();
        let event_kind = self.config.event_kind;
        let pending = Arc::clone(&self.pending);

        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                // Try to decrypt and process the event
                if let Err(e) = Self::process_incoming_event(
                    event,
                    &secret_key,
                    &peer_pubkeys,
                    &incoming_tx,
                    Arc::clone(&pending),
                    Some(relay_pool_clone.clone()),
                    &relays,
                    event_kind,
                )
                .await
                {
                    // Log error but continue processing
                    eprintln!("Failed to process incoming Bifrost event: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Broadcast a Bifrost message to threshold peers
    pub async fn broadcast(&self, message: &BifrostMessage) -> Result<()> {
        // Check if connected
        if !*self.connected.read().await {
            return Err(Error::Transport("Not connected to relays".to_string()));
        }

        let pool = self
            .relay_pool
            .as_ref()
            .ok_or_else(|| Error::Transport("Relay pool not initialized".to_string()))?;

        // Serialize message
        let message_json = serde_json::to_string(message)
            .map_err(|e| Error::Encoding(format!("failed to serialize message: {}", e)))?;

        // Extract session_id from message or generate new one
        let session_id =
            Self::extract_session_id(message).unwrap_or_else(|| self.generate_session_id());

        // Create envelope
        let envelope = MessageEnvelope {
            session_id,
            msg_type: self.message_type(message),
            message: message_json,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Serialize envelope
        let envelope_json = serde_json::to_string(&envelope)
            .map_err(|e| Error::Encoding(format!("failed to serialize envelope: {}", e)))?;

        // Publish encrypted message to each peer individually
        // This ensures only the intended recipient can read the message
        for peer_pk in &self.config.peer_pubkeys {
            // Encrypt content for this specific peer using NIP-44
            let encrypted_content = self.encrypt_for_peer(&envelope_json, peer_pk)?;

            let peer_hex = hex::encode(peer_pk);

            // Build tags for this peer
            let tags = vec![
                vec!["p".to_string(), peer_hex],
                vec!["protocol".to_string(), "bifrost".to_string()],
                vec!["msg_type".to_string(), envelope.msg_type.clone()],
            ];

            let event_template = nostr::EventTemplate {
                created_at: envelope.timestamp,
                kind: self.config.event_kind,
                tags,
                content: encrypted_content,
            };

            // Sign event
            let event = nostr::finalize_event(&event_template, &self.config.secret_key)
                .map_err(|e| Error::Signing(format!("Failed to sign event: {}", e)))?;

            // Publish to relay pool
            pool.publish(&event)
                .await
                .map_err(|e| Error::Transport(format!("Failed to publish to relays: {}", e)))?;
        }

        Ok(())
    }

    /// Extract session_id from a Bifrost message
    fn extract_session_id(message: &BifrostMessage) -> Option<String> {
        match message {
            // Two-phase FROST signing
            BifrostMessage::CommitmentRequest(req) => Some(req.session_id.clone()),
            BifrostMessage::CommitmentResponse(res) => Some(res.session_id.clone()),
            BifrostMessage::SigningPackage(pkg) => Some(pkg.session_id.clone()),
            BifrostMessage::PartialSignature(sig) => Some(sig.session_id.clone()),
            BifrostMessage::SignResult(res) => Some(res.session_id.clone()),
            BifrostMessage::SignError(err) => Some(err.session_id.clone()),
            // ECDH
            BifrostMessage::EcdhRequest(req) => Some(req.session_id.clone()),
            BifrostMessage::EcdhResponse(res) => Some(res.session_id.clone()),
            BifrostMessage::EcdhResult(res) => Some(res.session_id.clone()),
            BifrostMessage::EcdhError(err) => Some(err.session_id.clone()),
            // Utility
            BifrostMessage::Ping(ping) => Some(ping.session_id.clone()),
            BifrostMessage::Pong(pong) => Some(pong.session_id.clone()),
        }
    }

    /// Publish a message and wait for responses
    pub async fn publish_and_wait(
        &self,
        message: &BifrostMessage,
        required_responses: usize,
    ) -> Result<Vec<BifrostMessage>> {
        // Use the session_id from the message itself
        let session_id = Self::extract_session_id(message)
            .ok_or_else(|| Error::Protocol("Message must have a session_id".to_string()))?;

        // Create response channel
        let (tx, mut rx) = mpsc::channel(required_responses);

        // Register pending request
        {
            let mut pending = self.pending.write().await;
            pending.insert(
                session_id.clone(),
                PendingRequest {
                    started_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    tx,
                    responses_received: 0,
                    responses_required: required_responses,
                },
            );
        }

        // Broadcast message
        self.broadcast(message).await?;

        // Wait for responses with timeout
        let timeout = Duration::from_secs(self.config.message_timeout);
        let mut responses = Vec::new();

        let result = tokio::time::timeout(timeout, async {
            while responses.len() < required_responses {
                if let Some(response) = rx.recv().await {
                    responses.push(response);
                } else {
                    break;
                }
            }
            Ok::<_, Error>(responses)
        })
        .await;

        // Cleanup pending request
        {
            let mut pending = self.pending.write().await;
            pending.remove(&session_id);
        }

        match result {
            Ok(Ok(responses)) => Ok(responses),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(Error::Timeout),
        }
    }

    /// Receive incoming Bifrost messages
    pub async fn receive(&self) -> Result<BifrostMessage> {
        let mut rx = self.incoming_rx.write().await;
        rx.recv()
            .await
            .ok_or_else(|| Error::Protocol("incoming channel closed".to_string()))
    }

    /// Handle an incoming Nostr event
    pub async fn handle_event(&self, event_json: &str) -> Result<()> {
        // Parse envelope
        let envelope: MessageEnvelope = serde_json::from_str(event_json)
            .map_err(|e| Error::Encoding(format!("failed to parse envelope: {}", e)))?;

        // Check for duplicates
        let message_id = format!("{}:{}", envelope.session_id, envelope.timestamp);
        {
            let mut seen = self.seen_messages.write().await;
            if seen.contains_key(&message_id) {
                // Duplicate message, discard
                return Ok(());
            }
            seen.insert(message_id, envelope.timestamp);
        }

        // Cleanup old seen messages (older than 5 minutes)
        self.cleanup_seen_messages().await;

        // Parse Bifrost message
        let message: BifrostMessage = serde_json::from_str(&envelope.message)
            .map_err(|e| Error::Encoding(format!("failed to parse message: {}", e)))?;

        // Check if this is a response to a pending request
        {
            let mut pending = self.pending.write().await;
            if let Some(request) = pending.get_mut(&envelope.session_id) {
                // Send to waiting request
                let _ = request.tx.send(message.clone()).await;
                request.responses_received += 1;

                // Remove if complete
                if request.responses_received >= request.responses_required {
                    pending.remove(&envelope.session_id);
                }
                return Ok(());
            }
        }

        // Not a response to our request, route to incoming channel
        self.incoming_tx
            .send(message)
            .await
            .map_err(|_| Error::Protocol("failed to route incoming message".to_string()))?;

        Ok(())
    }

    /// Retry a message send
    pub async fn retry_send(&self, message: &BifrostMessage) -> Result<()> {
        let mut attempts = 0;
        let max_retries = self.config.max_retries;

        while attempts < max_retries {
            match self.broadcast(message).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        return Err(e);
                    }
                    // Exponential backoff
                    let delay = Duration::from_secs(2_u64.pow(attempts));
                    sleep(delay).await;
                }
            }
        }

        Err(Error::Protocol("max retries exceeded".to_string()))
    }

    /// Cleanup old seen messages
    async fn cleanup_seen_messages(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut seen = self.seen_messages.write().await;
        seen.retain(|_, &mut timestamp| now - timestamp < 300); // Keep last 5 minutes
    }

    /// Cleanup timed out pending requests
    pub async fn cleanup_timeouts(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut pending = self.pending.write().await;
        pending.retain(|_, request| now - request.started_at < self.config.message_timeout);
    }

    /// Generate a unique session ID
    fn generate_session_id(&self) -> String {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        let id = u128::from_be_bytes(bytes);
        format!("{:032x}", id)
    }

    /// Get message type string
    fn message_type(&self, message: &BifrostMessage) -> String {
        match message {
            // Two-phase FROST signing
            BifrostMessage::CommitmentRequest(_) => "commit_req".to_string(),
            BifrostMessage::CommitmentResponse(_) => "commit_res".to_string(),
            BifrostMessage::SigningPackage(_) => "sign_pkg".to_string(),
            BifrostMessage::PartialSignature(_) => "partial_sig".to_string(),
            BifrostMessage::SignResult(_) => "sign_ret".to_string(),
            BifrostMessage::SignError(_) => "sign_err".to_string(),
            // ECDH
            BifrostMessage::EcdhRequest(_) => "ecdh_req".to_string(),
            BifrostMessage::EcdhResponse(_) => "ecdh_res".to_string(),
            BifrostMessage::EcdhResult(_) => "ecdh_ret".to_string(),
            BifrostMessage::EcdhError(_) => "ecdh_err".to_string(),
            // Utility
            BifrostMessage::Ping(_) => "ping".to_string(),
            BifrostMessage::Pong(_) => "pong".to_string(),
        }
    }

    /// Check connection health
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    /// Get list of connected relays
    pub async fn connected_relays(&self) -> Vec<String> {
        if let Some(pool) = &self.relay_pool {
            pool.connected_relays().await
        } else {
            Vec::new()
        }
    }

    /// Get configuration
    pub fn config(&self) -> &TransportConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bifrost::CommitmentRequest;

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.event_kind, BIFROST_EVENT_KIND);
        assert_eq!(config.message_timeout, 30);
        assert_eq!(config.max_retries, 3);
    }

    #[tokio::test]
    async fn test_transport_new() {
        let config = TransportConfig::default();
        let transport = NostrTransport::new(config).unwrap();
        // Transport starts disconnected until connect() is called
        assert!(!transport.is_connected().await);
    }

    #[tokio::test]
    async fn test_broadcast_message() {
        let config = TransportConfig::default();
        let transport = NostrTransport::new(config).unwrap();

        let message = BifrostMessage::CommitmentRequest(CommitmentRequest {
            event_hash: [0x42; 32],
            session_id: "test-session".to_string(),
            participants: vec![1, 2, 3],
            initiator_id: 1,
        });

        // Broadcast should fail if not connected
        let result = transport.broadcast(&message).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_duplicate_detection() {
        let config = TransportConfig::default();
        let transport = NostrTransport::new(config).unwrap();

        let message_id = "test-session:1234567890";

        // Add message to seen messages
        {
            let mut seen = transport.seen_messages.write().await;
            seen.insert(message_id.to_string(), 1234567890);
        }

        // Check that duplicate is detected
        {
            let seen = transport.seen_messages.read().await;
            assert!(seen.contains_key(message_id));
        }

        // Verify deduplication works
        let envelope = MessageEnvelope {
            session_id: "test-session".to_string(),
            msg_type: "commit_req".to_string(),
            message: r#"{"type":"/sign/commit/req","event_hash":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"session_id":"test-session","participants":[1],"initiator_id":1}"#.to_string(),
            timestamp: 1234567890,
        };

        let event_json = serde_json::to_string(&envelope).unwrap();

        // This should be detected as duplicate and return Ok without processing
        let result = transport.handle_event(&event_json).await;
        assert!(result.is_ok());

        // Still only one message in seen_messages
        let seen = transport.seen_messages.read().await;
        assert_eq!(seen.len(), 1);
    }

    #[tokio::test]
    async fn test_message_type_detection() {
        let config = TransportConfig::default();
        let transport = NostrTransport::new(config).unwrap();

        let commit_req = BifrostMessage::CommitmentRequest(CommitmentRequest {
            event_hash: [0; 32],
            session_id: "test".to_string(),
            participants: vec![1],
            initiator_id: 1,
        });
        assert_eq!(transport.message_type(&commit_req), "commit_req");

        let commit_res = BifrostMessage::CommitmentResponse(crate::bifrost::CommitmentResponse {
            session_id: "test".to_string(),
            participant_id: 1,
            nonce_commitment: [0; 66],
        });
        assert_eq!(transport.message_type(&commit_res), "commit_res");
    }

    #[tokio::test]
    async fn test_session_id_generation() {
        let config = TransportConfig::default();
        let transport = NostrTransport::new(config).unwrap();

        let id1 = transport.generate_session_id();
        let id2 = transport.generate_session_id();

        // Session IDs should be unique
        assert_ne!(id1, id2);
        // Should be hex strings of length 32
        assert_eq!(id1.len(), 32);
        assert_eq!(id2.len(), 32);
    }

    #[tokio::test]
    async fn test_cleanup_seen_messages() {
        let config = TransportConfig::default();
        let transport = NostrTransport::new(config).unwrap();

        // Add some old messages
        {
            let mut seen = transport.seen_messages.write().await;
            seen.insert("old1".to_string(), 1000); // Very old
            seen.insert("old2".to_string(), 2000); // Very old

            // Add recent message
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            seen.insert("recent".to_string(), now);
        }

        // Cleanup should remove old messages
        transport.cleanup_seen_messages().await;

        let seen = transport.seen_messages.read().await;
        assert_eq!(seen.len(), 1);
        assert!(seen.contains_key("recent"));
    }

    #[tokio::test]
    async fn test_cleanup_timeouts() {
        let mut config = TransportConfig::default();
        config.message_timeout = 1; // 1 second timeout
        let transport = NostrTransport::new(config).unwrap();

        // Add a pending request
        let (tx, _rx) = mpsc::channel(1);
        {
            let mut pending = transport.pending.write().await;
            pending.insert(
                "test-session".to_string(),
                PendingRequest {
                    started_at: 1000, // Very old timestamp
                    tx,
                    responses_received: 0,
                    responses_required: 2,
                },
            );
        }

        // Cleanup should remove timed out request
        transport.cleanup_timeouts().await;

        let pending = transport.pending.read().await;
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_connected_relays() {
        let config = TransportConfig {
            relays: vec![
                "wss://relay1.com".to_string(),
                "wss://relay2.com".to_string(),
            ],
            ..Default::default()
        };
        let transport = NostrTransport::new(config).unwrap();

        // Transport is not connected, so no relays
        let relays = transport.connected_relays().await;
        assert_eq!(relays.len(), 0);
    }
}
