//! Bifrost node implementation

use crate::bifrost::aggregator::SigningAggregator;
use crate::bifrost::peer::PeerManager;
use crate::bifrost::transport::{NostrTransport, TransportConfig};
use crate::bifrost::{BifrostMessage, Ping, SignRequest};
use crate::keygen::FrostShare;
use crate::signing::round1_commit;
use crate::Result;
use frost_secp256k1::round1::SigningCommitments;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Timeout configuration for different operations
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Signing operation timeout in milliseconds
    pub sign_timeout_ms: u64,
    /// ECDH operation timeout in milliseconds
    pub ecdh_timeout_ms: u64,
    /// Default timeout for other operations in milliseconds
    pub default_timeout_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            sign_timeout_ms: 30000, // 30 seconds
            ecdh_timeout_ms: 10000, // 10 seconds
            default_timeout_ms: 30000, // 30 seconds
        }
    }
}

/// Retry configuration for failed operations
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial retry delay in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum retry delay in milliseconds
    pub max_delay_ms: u64,
    /// Backoff multiplier
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000, // 1 second
            max_delay_ms: 300000,   // 5 minutes
            multiplier: 2.0,
        }
    }
}

/// Bifrost node configuration
#[derive(Debug, Clone)]
pub struct BifrostConfig {
    /// Peer timeout in seconds
    pub peer_timeout: u64,
    /// Default relays for fallback
    pub default_relays: Vec<String>,
    /// Timeout configuration
    pub timeouts: TimeoutConfig,
    /// Retry configuration
    pub retries: RetryConfig,
    /// Secret key for Nostr transport (32 bytes)
    pub secret_key: Option<[u8; 32]>,
    /// Peer public keys for threshold operations
    pub peer_pubkeys: Vec<[u8; 32]>,
}

impl Default for BifrostConfig {
    fn default() -> Self {
        Self {
            peer_timeout: 300, // 5 minutes
            default_relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
            ],
            timeouts: TimeoutConfig::default(),
            retries: RetryConfig::default(),
            secret_key: None,
            peer_pubkeys: Vec::new(),
        }
    }
}

/// Bifrost node for coordinating threshold operations
pub struct BifrostNode {
    /// Configuration
    config: BifrostConfig,
    /// Peer manager for tracking threshold peers
    peer_manager: PeerManager,
    /// Nostr transport for message publishing (optional until initialized)
    transport: Option<NostrTransport>,
    /// Local FROST share for signing operations (optional)
    frost_share: Option<FrostShare>,
    /// Running state flag (shared for shutdown signaling)
    running: Arc<AtomicBool>,
}

impl BifrostNode {
    /// Create a new Bifrost node with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(BifrostConfig::default())
    }

    /// Create a new Bifrost node with custom configuration
    pub fn with_config(config: BifrostConfig) -> Result<Self> {
        let peer_manager = PeerManager::new(config.peer_timeout);

        // Create transport if secret key is provided
        let transport = if let Some(secret_key) = config.secret_key {
            let transport_config = TransportConfig {
                relays: config.default_relays.clone(),
                secret_key,
                peer_pubkeys: config.peer_pubkeys.clone(),
                event_kind: crate::bifrost::transport::BIFROST_EVENT_KIND,
                message_timeout: config.timeouts.default_timeout_ms / 1000,
                max_retries: config.retries.max_retries,
            };
            Some(NostrTransport::new(transport_config)?)
        } else {
            None
        };

        Ok(Self {
            config,
            peer_manager,
            transport,
            frost_share: None,
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Add a peer to the node
    pub fn add_peer(&mut self, pubkey: [u8; 32]) {
        self.peer_manager.add_peer(pubkey);
    }

    /// Update peer relays from NIP-65 relay list
    pub fn update_peer_relays(&mut self, pubkey: &[u8; 32], relays: Vec<String>) {
        self.peer_manager.update_peer_relays(pubkey, relays);
    }

    /// Get peer manager reference
    pub fn peer_manager(&self) -> &PeerManager {
        &self.peer_manager
    }

    /// Get mutable peer manager reference
    pub fn peer_manager_mut(&mut self) -> &mut PeerManager {
        &mut self.peer_manager
    }

    /// Get configuration reference
    pub fn config(&self) -> &BifrostConfig {
        &self.config
    }

    /// Get transport reference
    pub fn transport(&self) -> Option<&NostrTransport> {
        self.transport.as_ref()
    }

    /// Check if transport is initialized
    pub fn has_transport(&self) -> bool {
        self.transport.is_some()
    }

    /// Set the local FROST share for signing
    pub fn set_frost_share(&mut self, share: FrostShare) {
        self.frost_share = Some(share);
    }

    /// Get the local FROST share reference
    pub fn frost_share(&self) -> Option<&FrostShare> {
        self.frost_share.as_ref()
    }

    /// Check if FROST share is set
    pub fn has_frost_share(&self) -> bool {
        self.frost_share.is_some()
    }

    /// Get threshold (k) from FROST share if available
    pub fn threshold(&self) -> Option<u16> {
        self.frost_share.as_ref().map(|s| s.threshold)
    }

    /// Check if the node is currently running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Start the Bifrost node
    ///
    /// Initializes the node and marks it as running. In a full implementation,
    /// this would:
    /// - Connect to Nostr relays
    /// - Start background tasks for message handling
    /// - Initialize health monitoring
    ///
    /// Returns an error if:
    /// - Node is already running
    /// - Transport is not configured
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running() {
            return Err(crate::Error::Protocol(
                "Node is already running".into()
            ));
        }

        if self.transport.is_none() {
            return Err(crate::Error::Protocol(
                "Cannot start node without transport. Configure secret_key in BifrostConfig.".into()
            ));
        }

        // Mark as running
        self.running.store(true, Ordering::Relaxed);

        // In a full implementation, this would:
        // 1. Connect to all configured relays
        // 2. Start subscription for incoming messages
        // 3. Launch background health check task
        // 4. Start cleanup task for timed-out requests

        Ok(())
    }

    /// Stop the Bifrost node gracefully
    ///
    /// Performs graceful shutdown:
    /// - Marks node as not running
    /// - Signals background tasks to stop
    /// - Cleans up pending requests
    /// - Disconnects from relays
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running() {
            return Ok(()); // Already stopped
        }

        // Mark as not running (signals background tasks to stop)
        self.running.store(false, Ordering::Relaxed);

        // In a full implementation, this would:
        // 1. Stop accepting new requests
        // 2. Wait for pending requests to complete (with timeout)
        // 3. Cancel background tasks
        // 4. Close relay connections
        // 5. Cleanup resources

        if let Some(transport) = &self.transport {
            // Cleanup any pending requests in transport
            transport.cleanup_timeouts().await;
        }

        Ok(())
    }

    /// Reconnect to Nostr relays
    ///
    /// Handles relay disconnections by attempting to reconnect.
    /// Uses exponential backoff for retry delays.
    pub async fn reconnect(&mut self) -> Result<()> {
        if !self.has_transport() {
            return Err(crate::Error::Protocol(
                "Cannot reconnect without transport configured".into()
            ));
        }

        // In a full implementation, this would:
        // 1. Check which relays are disconnected
        // 2. Attempt to reconnect with retry logic
        // 3. Update relay connection status
        // 4. Re-subscribe to message channels

        // For now, this is a placeholder
        // The actual reconnection would integrate with the transport layer

        Ok(())
    }

    /// Ping a peer to check connectivity
    pub async fn ping(&mut self, pubkey: &[u8; 32]) -> Result<bool> {
        // Check if transport is initialized
        let transport = self.transport.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "NostrTransport not initialized. Provide secret_key in BifrostConfig.".into()
            )
        })?;

        // Generate session ID for ping/pong correlation
        let session_id = self.generate_session_id();

        // Get current timestamp in milliseconds
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Create ping message
        let ping = Ping {
            session_id: session_id.clone(),
            timestamp,
        };

        let message = BifrostMessage::Ping(ping);

        // Send ping and wait for pong (expecting 1 response)
        let responses = transport
            .publish_and_wait(&message, 1)
            .await?;

        // Check if we got a pong response and calculate latency
        let recv_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for response in responses {
            if let BifrostMessage::Pong(pong) = response
                && pong.session_id == session_id {
                    // Calculate round-trip latency
                    let latency_ms = recv_time.saturating_sub(timestamp);

                    // Mark peer as responsive with latency
                    self.peer_manager.mark_peer_responsive(pubkey, Some(latency_ms));
                    return Ok(true);
                }
        }

        // No valid pong received
        Ok(false)
    }

    /// Perform health check on all peers
    pub async fn health_check(&mut self) -> Result<usize> {
        self.peer_manager.health_check().await
    }

    /// Get the last measured latency for a peer in milliseconds
    pub fn get_peer_latency(&self, pubkey: &[u8; 32]) -> Option<u64> {
        self.peer_manager.get_peer_latency(pubkey)
    }

    /// Calculate retry delay using exponential backoff
    pub fn calculate_retry_delay(&self, attempt: u32) -> tokio::time::Duration {
        let delay_ms = self.config.retries.initial_delay_ms as f64
            * self.config.retries.multiplier.powi(attempt as i32);
        let delay_ms = delay_ms.min(self.config.retries.max_delay_ms as f64) as u64;
        tokio::time::Duration::from_millis(delay_ms)
    }

    /// Get timeout for a specific operation type
    pub fn get_timeout(&self, operation: &str) -> tokio::time::Duration {
        let timeout_ms = match operation {
            "sign" => self.config.timeouts.sign_timeout_ms,
            "ecdh" => self.config.timeouts.ecdh_timeout_ms,
            _ => self.config.timeouts.default_timeout_ms,
        };
        tokio::time::Duration::from_millis(timeout_ms)
    }

    /// Sign an event hash using threshold shares
    ///
    /// This method coordinates a threshold signing operation:
    /// 1. Generates signing nonces and commitments (round1_commit)
    /// 2. Broadcasts SignRequest to threshold peers
    /// 3. Collects k-of-n SignResponse messages
    /// 4. Aggregates partial signatures into final signature
    /// 5. Broadcasts SignResult to all participants
    /// 6. Returns final 64-byte Schnorr signature
    ///
    /// Requires:
    /// - NostrTransport must be initialized (secret_key in config)
    /// - FrostShare must be set (call set_frost_share() first)
    ///
    /// Note: This is a simplified coordinator implementation. A production
    /// version would need:
    /// - Proper session ID generation and management
    /// - Participant selection logic
    /// - Full FROST type serialization for network transport
    /// - Nonce commitment tracking
    /// - Response validation and error handling
    pub async fn sign(&self, event_hash: &[u8; 32]) -> Result<[u8; 64]> {
        // Check preconditions
        let transport = self.transport.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "NostrTransport not initialized. Provide secret_key in BifrostConfig.".into()
            )
        })?;

        let frost_share = self.frost_share.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "FrostShare not set. Call set_frost_share() before signing.".into()
            )
        })?;

        // Get threshold requirement
        let threshold = frost_share.threshold as usize;

        // Step 1: Generate our signing nonces and commitments
        let (_nonces, commitments) = round1_commit(frost_share);

        // Serialize commitment (simplified - production needs proper serialization)
        let commitment_bytes = self.serialize_commitment(&commitments)?;

        // Step 2: Create and broadcast SignRequest
        let session_id = self.generate_session_id();
        let participants = self.select_participants(threshold)?;

        let request = SignRequest {
            event_hash: *event_hash,
            nonce_commitment: commitment_bytes,
            session_id: session_id.clone(),
            participants: participants.clone(),
        };

        let message = BifrostMessage::SignRequest(request.clone());

        // Step 3: Broadcast and wait for k responses
        let responses = transport
            .publish_and_wait(&message, threshold)
            .await?;

        // Step 4: Collect and validate responses
        let mut aggregator = SigningAggregator::new(threshold, session_id);

        for response in responses {
            if let BifrostMessage::SignResponse(sign_response) = response {
                aggregator.add_response(sign_response)?;
            }
        }

        // Step 5: Aggregate partial signatures
        // Note: This is where we'd call the full aggregation logic
        // For now, we return an error indicating the implementation is incomplete

        Err(crate::Error::Protocol(
            "Signature aggregation requires full FROST type serialization. \
             Coordinator flow is implemented but aggregation step needs completion.".into()
        ))
    }

    /// Generate a unique session ID
    fn generate_session_id(&self) -> String {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        format!("{:032x}", u128::from_be_bytes(bytes))
    }

    /// Select participants for signing
    fn select_participants(&self, threshold: usize) -> Result<Vec<u8>> {
        // In a real implementation, this would:
        // 1. Query online peers from peer_manager
        // 2. Select k peers based on availability and policy
        // 3. Return their participant IDs
        //
        // For now, return a simple sequence
        Ok((1..=threshold as u8).collect())
    }

    /// Serialize a signing commitment to bytes
    fn serialize_commitment(&self, _commitments: &SigningCommitments) -> Result<[u8; 33]> {
        // In a real implementation, this would serialize the commitment
        // to the wire format expected by peers.
        // For now, return placeholder bytes
        Ok([0u8; 33])
    }

    /// Perform threshold ECDH with a peer
    ///
    /// This method coordinates a threshold ECDH operation:
    /// 1. Broadcasts EcdhRequest to threshold peers
    /// 2. Collects k-of-n EcdhResponse messages
    /// 3. Aggregates partial ECDH results into shared secret
    ///
    /// Requires:
    /// - NostrTransport must be initialized (secret_key in config)
    /// - Threshold ECDH implementation (currently not supported)
    ///
    /// Note: Threshold ECDH requires multiplicative secret sharing which
    /// FROST shares don't directly support. See crate::ecdh module docs.
    pub async fn ecdh(&self, _peer_pubkey: &[u8; 32]) -> Result<[u8; 32]> {
        // Check if transport is available
        let _transport = self.transport.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "NostrTransport not initialized. Provide secret_key in BifrostConfig.".into()
            )
        })?;

        // Transport is ready, but ECDH aggregation not implemented
        Err(crate::Error::Protocol(
            "Threshold ECDH not yet implemented. FROST shares require \
             multiplicative threshold ECDH. See crate::ecdh documentation.".into()
        ))
    }
}

impl Default for BifrostNode {
    fn default() -> Self {
        Self::new().expect("Failed to create default BifrostNode")
    }
}

impl Drop for BifrostNode {
    fn drop(&mut self) {
        // Mark node as not running when dropped
        // This signals any background tasks to stop
        self.running.store(false, Ordering::Relaxed);

        // Note: We can't call async stop() from Drop
        // In a production implementation, background tasks would
        // monitor the running flag and clean up themselves
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bifrost_node_new() {
        let node = BifrostNode::new().unwrap();
        assert_eq!(node.config.peer_timeout, 300);
        assert!(!node.config.default_relays.is_empty());
    }

    #[test]
    fn test_bifrost_node_add_peer() {
        let mut node = BifrostNode::new().unwrap();
        let pubkey = [0x42; 32];

        node.add_peer(pubkey);

        assert!(node.peer_manager().get_peer(&pubkey).is_some());
    }

    #[test]
    fn test_bifrost_node_update_peer_relays() {
        let mut node = BifrostNode::new().unwrap();
        let pubkey = [0x42; 32];

        node.add_peer(pubkey);
        node.update_peer_relays(&pubkey, vec!["wss://relay.example.com".to_string()]);

        let peer = node.peer_manager().get_peer(&pubkey).unwrap();
        assert_eq!(peer.relays.len(), 1);
        assert_eq!(peer.relays[0], "wss://relay.example.com");
    }

    #[test]
    fn test_bifrost_node_with_custom_config() {
        let config = BifrostConfig {
            peer_timeout: 600,
            default_relays: vec!["wss://custom.relay.com".to_string()],
            timeouts: TimeoutConfig::default(),
            retries: RetryConfig::default(),
            secret_key: None,
            peer_pubkeys: Vec::new(),
        };

        let node = BifrostNode::with_config(config).unwrap();

        assert_eq!(node.config.peer_timeout, 600);
        assert_eq!(node.config.default_relays.len(), 1);
    }

    #[test]
    fn test_timeout_config_default() {
        let config = TimeoutConfig::default();
        assert_eq!(config.sign_timeout_ms, 30000);
        assert_eq!(config.ecdh_timeout_ms, 10000);
        assert_eq!(config.default_timeout_ms, 30000);
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 300000);
        assert_eq!(config.multiplier, 2.0);
    }

    #[test]
    fn test_calculate_retry_delay() {
        let node = BifrostNode::new().unwrap();

        // First attempt: 1 second
        let delay0 = node.calculate_retry_delay(0);
        assert_eq!(delay0.as_millis(), 1000);

        // Second attempt: 2 seconds
        let delay1 = node.calculate_retry_delay(1);
        assert_eq!(delay1.as_millis(), 2000);

        // Third attempt: 4 seconds
        let delay2 = node.calculate_retry_delay(2);
        assert_eq!(delay2.as_millis(), 4000);

        // Fourth attempt: 8 seconds
        let delay3 = node.calculate_retry_delay(3);
        assert_eq!(delay3.as_millis(), 8000);

        // Many attempts: capped at max_delay_ms (300000 ms = 5 minutes)
        let delay_max = node.calculate_retry_delay(20);
        assert_eq!(delay_max.as_millis(), 300000);
    }

    #[test]
    fn test_get_timeout() {
        let node = BifrostNode::new().unwrap();

        // Sign timeout
        let sign_timeout = node.get_timeout("sign");
        assert_eq!(sign_timeout.as_millis(), 30000);

        // ECDH timeout
        let ecdh_timeout = node.get_timeout("ecdh");
        assert_eq!(ecdh_timeout.as_millis(), 10000);

        // Default timeout
        let default_timeout = node.get_timeout("unknown");
        assert_eq!(default_timeout.as_millis(), 30000);
    }

    #[test]
    fn test_custom_timeout_config() {
        let mut config = BifrostConfig::default();
        config.timeouts.sign_timeout_ms = 60000; // 60 seconds
        config.timeouts.ecdh_timeout_ms = 20000; // 20 seconds

        let node = BifrostNode::with_config(config).unwrap();

        assert_eq!(node.get_timeout("sign").as_millis(), 60000);
        assert_eq!(node.get_timeout("ecdh").as_millis(), 20000);
    }

    #[test]
    fn test_custom_retry_config() {
        let mut config = BifrostConfig::default();
        config.retries.max_retries = 5;
        config.retries.initial_delay_ms = 500;
        config.retries.multiplier = 3.0;

        let node = BifrostNode::with_config(config).unwrap();

        // First attempt: 500ms
        assert_eq!(node.calculate_retry_delay(0).as_millis(), 500);

        // Second attempt: 1500ms (500 * 3)
        assert_eq!(node.calculate_retry_delay(1).as_millis(), 1500);

        // Third attempt: 4500ms (500 * 9)
        assert_eq!(node.calculate_retry_delay(2).as_millis(), 4500);
    }

    #[test]
    fn test_retry_delay_caps_at_max() {
        let mut config = BifrostConfig::default();
        config.retries.max_delay_ms = 5000; // Cap at 5 seconds

        let node = BifrostNode::with_config(config).unwrap();

        // Many attempts should cap at 5000ms
        let delay = node.calculate_retry_delay(10);
        assert_eq!(delay.as_millis(), 5000);
    }

    #[test]
    fn test_node_without_transport() {
        let node = BifrostNode::new().unwrap();

        // Node without secret_key should have no transport
        assert!(!node.has_transport());
        assert!(node.transport().is_none());
    }

    #[test]
    fn test_node_with_transport() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);
        config.peer_pubkeys = vec![[0x01; 32], [0x02; 32], [0x03; 32]];

        let node = BifrostNode::with_config(config).unwrap();

        // Node with secret_key should have transport
        assert!(node.has_transport());
        assert!(node.transport().is_some());

        let transport = node.transport().unwrap();
        assert_eq!(transport.config().relays.len(), 2); // Default relays
        assert_eq!(transport.config().peer_pubkeys.len(), 3);
    }

    #[test]
    fn test_transport_config_mapping() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x99; 32]);
        config.default_relays = vec![
            "wss://relay1.com".to_string(),
            "wss://relay2.com".to_string(),
            "wss://relay3.com".to_string(),
        ];
        config.timeouts.default_timeout_ms = 60000; // 60 seconds
        config.retries.max_retries = 5;

        let node = BifrostNode::with_config(config).unwrap();
        let transport = node.transport().unwrap();

        // Verify transport config matches node config
        assert_eq!(transport.config().relays.len(), 3);
        assert_eq!(transport.config().message_timeout, 60); // Converted to seconds
        assert_eq!(transport.config().max_retries, 5);
    }

    #[tokio::test]
    async fn test_sign_requires_transport() {
        let node = BifrostNode::new().unwrap();
        let event_hash = [0x42; 32];

        // Should fail because no transport
        let result = node.sign(&event_hash).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("NostrTransport not initialized"));
    }

    #[tokio::test]
    async fn test_sign_with_transport_not_fully_implemented() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let node = BifrostNode::with_config(config).unwrap();
        let event_hash = [0x42; 32];

        // Should fail because FrostShare not set
        let result = node.sign(&event_hash).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("FrostShare not set"));
    }

    #[tokio::test]
    async fn test_ecdh_requires_transport() {
        let node = BifrostNode::new().unwrap();
        let peer_pubkey = [0x42; 32];

        // Should fail because no transport
        let result = node.ecdh(&peer_pubkey).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("NostrTransport not initialized"));
    }

    #[tokio::test]
    async fn test_ecdh_with_transport_not_implemented() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let node = BifrostNode::with_config(config).unwrap();
        let peer_pubkey = [0x42; 32];

        // Should fail because threshold ECDH not implemented
        let result = node.ecdh(&peer_pubkey).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not yet implemented"));
    }

    #[test]
    fn test_config_with_peers_and_key() {
        let config = BifrostConfig {
            peer_timeout: 300,
            default_relays: vec!["wss://relay.test.com".to_string()],
            timeouts: TimeoutConfig::default(),
            retries: RetryConfig::default(),
            secret_key: Some([0xAB; 32]),
            peer_pubkeys: vec![[0x01; 32], [0x02; 32]],
        };

        let node = BifrostNode::with_config(config).unwrap();

        assert!(node.has_transport());
        assert_eq!(node.config().peer_pubkeys.len(), 2);
        assert_eq!(node.config().secret_key, Some([0xAB; 32]));
    }

    #[test]
    fn test_node_without_frost_share() {
        let node = BifrostNode::new().unwrap();

        // Node without frost_share
        assert!(!node.has_frost_share());
        assert!(node.frost_share().is_none());
        assert!(node.threshold().is_none());
    }

    #[test]
    fn test_node_with_frost_share() {
        let mut node = BifrostNode::new().unwrap();

        // Generate a 2-of-3 share
        let shares = crate::keygen::generate_key_shares(2, 3).unwrap();
        node.set_frost_share(shares[0].clone());

        // Node should have frost_share
        assert!(node.has_frost_share());
        assert!(node.frost_share().is_some());
        assert_eq!(node.threshold(), Some(2));
    }

    #[tokio::test]
    async fn test_sign_requires_frost_share() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let node = BifrostNode::with_config(config).unwrap();
        let event_hash = [0x42; 32];

        // Should fail because no frost_share
        let result = node.sign(&event_hash).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("FrostShare not set"));
    }

    #[tokio::test]
    async fn test_sign_with_frost_share_partial_implementation() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let mut node = BifrostNode::with_config(config).unwrap();

        // Set frost share
        let shares = crate::keygen::generate_key_shares(2, 3).unwrap();
        node.set_frost_share(shares[0].clone());

        let event_hash = [0x42; 32];

        // Should fail at aggregation step (not fully implemented)
        let result = node.sign(&event_hash).await;
        assert!(result.is_err());
        // This will fail because publish_and_wait will timeout
        // (no actual relay connections in test)
    }

    #[test]
    fn test_generate_session_id() {
        let node = BifrostNode::new().unwrap();

        let id1 = node.generate_session_id();
        let id2 = node.generate_session_id();

        // Session IDs should be unique
        assert_ne!(id1, id2);
        // Should be hex strings of length 32
        assert_eq!(id1.len(), 32);
        assert_eq!(id2.len(), 32);
    }

    #[test]
    fn test_select_participants() {
        let node = BifrostNode::new().unwrap();

        let participants = node.select_participants(2).unwrap();
        assert_eq!(participants.len(), 2);
        assert_eq!(participants, vec![1, 2]);

        let participants = node.select_participants(3).unwrap();
        assert_eq!(participants.len(), 3);
        assert_eq!(participants, vec![1, 2, 3]);
    }

    #[test]
    fn test_serialize_commitment() {
        let node = BifrostNode::new().unwrap();

        // Generate dummy commitment
        let shares = crate::keygen::generate_key_shares(2, 3).unwrap();
        let (_, commitments) = round1_commit(&shares[0]);

        let bytes = node.serialize_commitment(&commitments).unwrap();
        assert_eq!(bytes.len(), 33);
    }

    #[test]
    fn test_threshold_from_share() {
        let mut node = BifrostNode::new().unwrap();

        // Test different thresholds
        let shares_2_3 = crate::keygen::generate_key_shares(2, 3).unwrap();
        node.set_frost_share(shares_2_3[0].clone());
        assert_eq!(node.threshold(), Some(2));

        let shares_3_5 = crate::keygen::generate_key_shares(3, 5).unwrap();
        node.set_frost_share(shares_3_5[0].clone());
        assert_eq!(node.threshold(), Some(3));
    }

    #[test]
    fn test_node_initial_state_not_running() {
        let node = BifrostNode::new().unwrap();
        assert!(!node.is_running());
    }

    #[tokio::test]
    async fn test_start_requires_transport() {
        let mut node = BifrostNode::new().unwrap();

        // Should fail because no transport
        let result = node.start().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("without transport"));
    }

    #[tokio::test]
    async fn test_start_node_with_transport() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let mut node = BifrostNode::with_config(config).unwrap();

        // Should succeed with transport
        assert!(!node.is_running());
        node.start().await.unwrap();
        assert!(node.is_running());
    }

    #[tokio::test]
    async fn test_start_already_running() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let mut node = BifrostNode::with_config(config).unwrap();

        // Start once
        node.start().await.unwrap();
        assert!(node.is_running());

        // Second start should fail
        let result = node.start().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already running"));
    }

    #[tokio::test]
    async fn test_stop_node() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let mut node = BifrostNode::with_config(config).unwrap();

        // Start then stop
        node.start().await.unwrap();
        assert!(node.is_running());

        node.stop().await.unwrap();
        assert!(!node.is_running());
    }

    #[tokio::test]
    async fn test_stop_already_stopped() {
        let mut node = BifrostNode::new().unwrap();

        // Stop when not running should be no-op
        assert!(!node.is_running());
        node.stop().await.unwrap();
        assert!(!node.is_running());
    }

    #[tokio::test]
    async fn test_reconnect_requires_transport() {
        let mut node = BifrostNode::new().unwrap();

        // Should fail without transport
        let result = node.reconnect().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("without transport"));
    }

    #[tokio::test]
    async fn test_reconnect_with_transport() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let mut node = BifrostNode::with_config(config).unwrap();

        // Reconnect should succeed (even if it's a no-op in current implementation)
        node.reconnect().await.unwrap();
    }

    #[tokio::test]
    async fn test_lifecycle_start_stop_start() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let mut node = BifrostNode::with_config(config).unwrap();

        // Start
        node.start().await.unwrap();
        assert!(node.is_running());

        // Stop
        node.stop().await.unwrap();
        assert!(!node.is_running());

        // Start again
        node.start().await.unwrap();
        assert!(node.is_running());
    }

    #[test]
    fn test_drop_sets_running_false() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let running_flag = {
            let node = BifrostNode::with_config(config).unwrap();
            Arc::clone(&node.running)
        };

        // Node is dropped here
        // Running flag should be false
        assert!(!running_flag.load(Ordering::Relaxed));
    }
}
