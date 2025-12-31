//! Bifrost node implementation

use crate::Result;
use crate::bifrost::aggregator::EcdhAggregator;
use crate::bifrost::peer::{PeerManager, PeerStatus};
use crate::bifrost::serialization::{
    deserialize_commitments, serialize_commitments, serialize_sig_share,
};
use crate::bifrost::transport::{NostrTransport, TransportConfig};
use crate::bifrost::{
    BifrostMessage, CommitmentRequest, CommitmentResponse, EcdhRequest, PartialSignature,
    ParticipantCommitment, Ping, SigningPackageMessage,
};
use crate::ecdh::create_ecdh_share;
use crate::keygen::FrostShare;
use crate::signing::{round1_commit, round2_sign};
use frost_secp256k1::{Identifier, SigningPackage, round1::SigningNonces};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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
            sign_timeout_ms: 30000,    // 30 seconds
            ecdh_timeout_ms: 10000,    // 10 seconds
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
///
/// # Examples
///
/// ```
/// use frostr::bifrost::BifrostConfig;
///
/// // Use default configuration
/// let config = BifrostConfig::default();
/// assert_eq!(config.peer_timeout, 300); // 5 minutes
///
/// // Custom configuration
/// let config = BifrostConfig {
///     peer_timeout: 600,
///     default_relays: vec![
///         "wss://relay.damus.io".to_string(),
///         "wss://nos.lol".to_string(),
///     ],
///     secret_key: Some([42u8; 32]),
///     peer_pubkeys: vec![[1u8; 32], [2u8; 32]],
///     ..Default::default()
/// };
/// ```
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
///
/// # Examples
///
/// ```no_run
/// use frostr::bifrost::{BifrostNode, BifrostConfig};
///
/// # async fn example() -> anyhow::Result<()> {
/// // Create node with default configuration
/// let mut node = BifrostNode::new()?;
///
/// // Add peers
/// let peer1_pubkey = [1u8; 32];
/// let peer2_pubkey = [2u8; 32];
/// node.add_peer(peer1_pubkey);
/// node.add_peer(peer2_pubkey);
///
/// // Start the node
/// node.start().await?;
///
/// // Perform threshold signing
/// let event_hash = [0u8; 32];
/// let signature = node.sign(&event_hash).await?;
/// println!("Threshold signature created");
///
/// // Perform threshold ECDH
/// let target_pubkey = [3u8; 32];
/// let shared_secret = node.ecdh(&target_pubkey).await?;
/// println!("Shared secret computed");
///
/// // Stop the node
/// node.stop().await?;
/// # Ok(())
/// # }
/// ```
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
    /// Pending nonces for two-phase signing protocol
    /// Maps session_id -> SigningNonces (stored after Round 1 until Round 2)
    pending_nonces: Arc<Mutex<HashMap<String, SigningNonces>>>,
}

impl BifrostNode {
    /// Create a new Bifrost node with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(BifrostConfig::default())
    }

    /// Create a new Bifrost node with custom configuration
    pub fn with_config(config: BifrostConfig) -> Result<Self> {
        let mut peer_manager = PeerManager::new(config.peer_timeout);
        for pubkey in &config.peer_pubkeys {
            peer_manager.add_peer(*pubkey);
        }

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
            pending_nonces: Arc::new(Mutex::new(HashMap::new())),
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
    /// Initializes the node and connects to Nostr relays:
    /// - Connects to all configured relays via transport
    /// - Starts background subscription for incoming messages
    /// - Marks node as running
    ///
    /// Returns an error if:
    /// - Node is already running
    /// - Transport is not configured
    /// - Relay connection fails
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running() {
            return Err(crate::Error::Protocol("Node is already running".into()));
        }

        let transport = self.transport.as_mut().ok_or_else(|| {
            crate::Error::Protocol(
                "Cannot start node without transport. Configure secret_key in BifrostConfig."
                    .into(),
            )
        })?;

        // Connect to relays and start subscription
        transport.connect().await?;

        // Mark as running
        self.running.store(true, Ordering::Relaxed);

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
                "Cannot reconnect without transport configured".into(),
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
                "NostrTransport not initialized. Provide secret_key in BifrostConfig.".into(),
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
        let responses = transport.publish_and_wait(&message, 1).await?;

        // Check if we got a pong response and calculate latency
        let recv_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for response in responses {
            if let BifrostMessage::Pong(pong) = response
                && pong.session_id == session_id
            {
                // Calculate round-trip latency
                let latency_ms = recv_time.saturating_sub(timestamp);

                // Mark peer as responsive with latency
                self.peer_manager
                    .mark_peer_responsive(pubkey, Some(latency_ms));
                return Ok(true);
            }
        }

        // No valid pong received
        self.peer_manager.mark_peer_unresponsive(pubkey);
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

    fn is_retryable_error(&self, error: &crate::Error) -> bool {
        matches!(error, crate::Error::Timeout | crate::Error::Transport(_))
    }

    async fn retry_with_backoff<F, Fut, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut(u32) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let max_attempts = self.config.retries.max_retries.saturating_add(1).max(1);
        let mut last_err = None;

        for attempt in 0..max_attempts {
            match operation(attempt).await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    if !self.is_retryable_error(&err) || attempt + 1 >= max_attempts {
                        return Err(err);
                    }
                    last_err = Some(err);
                    tokio::time::sleep(self.calculate_retry_delay(attempt)).await;
                }
            }
        }

        Err(last_err.unwrap_or(crate::Error::Timeout))
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
    /// This method coordinates a threshold signing operation using the two-round
    /// FROST RFC 9591 compliant protocol:
    ///
    /// **Round 1 - Commitment Collection:**
    /// - Coordinator sends CommitmentRequest to all k participants
    /// - Each participant generates nonces, stores them, responds with CommitmentResponse
    /// - Coordinator collects all k commitments
    ///
    /// **Round 2 - Signature Generation:**
    /// - Coordinator sends SigningPackage with ALL k commitments to participants
    /// - Each participant computes their partial signature using the complete package
    /// - Coordinator aggregates all partial signatures into final signature
    ///
    /// Requires:
    /// - NostrTransport must be initialized (secret_key in config)
    /// - FrostShare must be set (call set_frost_share() first)
    pub async fn sign(&self, event_hash: &[u8; 32]) -> Result<[u8; 64]> {
        // Check preconditions
        let transport = self.transport.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "NostrTransport not initialized. Provide secret_key in BifrostConfig.".into(),
            )
        })?;

        let frost_share = self.frost_share.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "FrostShare not set. Call set_frost_share() before signing.".into(),
            )
        })?;

        self.sign_two_phase(event_hash, transport, frost_share)
            .await
    }

    /// Internal implementation of two-round FROST signing protocol
    async fn sign_two_phase(
        &self,
        event_hash: &[u8; 32],
        transport: &NostrTransport,
        frost_share: &FrostShare,
    ) -> Result<[u8; 64]> {
        let threshold = frost_share.threshold as usize;
        let initiator_id = frost_share.participant_id;
        self.retry_with_backoff(|attempt| async move {
            let participants =
                self.select_participants(threshold, initiator_id, attempt as usize)?;
            self.sign_two_phase_once(event_hash, transport, frost_share, participants)
                .await
        })
        .await
    }

    async fn sign_two_phase_once(
        &self,
        event_hash: &[u8; 32],
        transport: &NostrTransport,
        frost_share: &FrostShare,
        participants: Vec<u8>,
    ) -> Result<[u8; 64]> {
        let initiator_id = frost_share.participant_id;
        let required_responses = participants.len().saturating_sub(1);
        let participant_set: HashSet<u8> = participants.iter().copied().collect();
        let session_id = self.generate_session_id();

        // ========== ROUND 1: Collect commitments ==========

        // Generate our own commitment
        let (nonces, our_commitments) = round1_commit(frost_share);
        let our_commitment_bytes = serialize_commitments(&our_commitments);

        let commitment_responses = if required_responses > 0 {
            let commitment_request = CommitmentRequest {
                event_hash: *event_hash,
                session_id: session_id.clone(),
                participants: participants.clone(),
                initiator_id,
            };

            let request_message = BifrostMessage::CommitmentRequest(commitment_request);
            let raw_responses = transport
                .publish_and_wait(&request_message, required_responses)
                .await?;

            let mut seen = HashSet::new();
            let mut filtered = Vec::new();
            for response in raw_responses {
                if let BifrostMessage::CommitmentResponse(cr) = response
                    && cr.participant_id != initiator_id
                    && participant_set.contains(&cr.participant_id)
                    && seen.insert(cr.participant_id)
                {
                    filtered.push(cr);
                }
            }

            if filtered.len() < required_responses {
                return Err(crate::Error::Timeout);
            }

            filtered
        } else {
            Vec::new()
        };

        // Collect all commitments (ours + peers)
        let mut all_commitments: Vec<ParticipantCommitment> = vec![ParticipantCommitment {
            participant_id: initiator_id,
            commitment: our_commitment_bytes,
        }];

        for response in &commitment_responses {
            all_commitments.push(ParticipantCommitment {
                participant_id: response.participant_id,
                commitment: response.nonce_commitment,
            });
        }

        // ========== ROUND 2: Distribute SigningPackage and collect signatures ==========

        // Build signing package with ALL commitments
        let mut signing_commitments = BTreeMap::new();
        for pc in &all_commitments {
            let id = Identifier::try_from(pc.participant_id as u16)
                .map_err(|e| crate::Error::Protocol(format!("Invalid participant ID: {:?}", e)))?;
            let commitment = deserialize_commitments(&pc.commitment)?;
            signing_commitments.insert(id, commitment);
        }
        let signing_package = SigningPackage::new(signing_commitments, event_hash);

        let signature_responses = if required_responses > 0 {
            let package_message = SigningPackageMessage {
                event_hash: *event_hash,
                session_id: session_id.clone(),
                commitments: all_commitments.clone(),
                participants: participants.clone(),
            };

            let package_msg = BifrostMessage::SigningPackage(package_message);
            let raw_responses = transport
                .publish_and_wait(&package_msg, required_responses)
                .await?;

            let mut seen = HashSet::new();
            let mut filtered = Vec::new();
            for response in raw_responses {
                if let BifrostMessage::PartialSignature(ps) = response
                    && ps.participant_id != initiator_id
                    && participant_set.contains(&ps.participant_id)
                    && seen.insert(ps.participant_id)
                {
                    filtered.push(ps);
                }
            }

            if filtered.len() < required_responses {
                return Err(crate::Error::Timeout);
            }

            filtered
        } else {
            Vec::new()
        };

        // ========== AGGREGATION ==========

        // Generate our own partial signature
        let our_sig_share = round2_sign(frost_share, &nonces, &signing_package)?;
        let our_partial_sig_bytes = serialize_sig_share(&our_sig_share);

        // Collect all signature shares
        let mut signature_shares = BTreeMap::new();

        // Add our signature
        let our_id = Identifier::try_from(initiator_id as u16)
            .map_err(|e| crate::Error::Protocol(format!("Invalid initiator ID: {:?}", e)))?;
        let our_sig = crate::bifrost::serialization::deserialize_sig_share(&our_partial_sig_bytes)?;
        signature_shares.insert(our_id, our_sig);

        // Add peer signatures
        for response in signature_responses {
            let id = Identifier::try_from(response.participant_id as u16)
                .map_err(|e| crate::Error::Protocol(format!("Invalid participant ID: {:?}", e)))?;
            let sig = crate::bifrost::serialization::deserialize_sig_share(&response.partial_sig)?;
            signature_shares.insert(id, sig);
        }

        // Aggregate signatures using frost-secp256k1
        let final_signature =
            crate::signing::aggregate_signatures(&signing_package, &signature_shares, frost_share)?;

        // Convert to 64-byte BIP-340 format
        let sig_bytes = final_signature.serialize().map_err(|e| {
            crate::Error::Encoding(format!("Failed to serialize signature: {:?}", e))
        })?;

        if sig_bytes.len() != 65 {
            return Err(crate::Error::Encoding(format!(
                "Unexpected signature length: expected 65, got {}",
                sig_bytes.len()
            )));
        }

        let mut result = [0u8; 64];
        result[..32].copy_from_slice(&sig_bytes[1..33]); // R.x (skip compression prefix)
        result[32..].copy_from_slice(&sig_bytes[33..65]); // s scalar

        Ok(result)
    }

    /// Generate a unique session ID
    fn generate_session_id(&self) -> String {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        format!("{:032x}", u128::from_be_bytes(bytes))
    }

    /// Select participants for signing or ECDH
    fn select_participants(
        &self,
        threshold: usize,
        initiator_id: u8,
        rotation: usize,
    ) -> Result<Vec<u8>> {
        if threshold == 0 {
            return Err(crate::Error::Protocol(
                "Threshold must be at least 1".into(),
            ));
        }

        let needed = threshold.saturating_sub(1);
        if needed == 0 {
            return Ok(vec![initiator_id]);
        }

        let mut online = Vec::new();
        let mut unknown = Vec::new();

        for (idx, pubkey) in self.config.peer_pubkeys.iter().enumerate() {
            let participant_id = (idx + 1) as u8;
            if participant_id == initiator_id {
                continue;
            }

            let peer = self.peer_manager.get_peer(pubkey);
            match peer {
                Some(peer) => {
                    if peer.status == PeerStatus::Offline {
                        continue;
                    }
                    if peer.status == PeerStatus::Online
                        && peer.is_recently_seen(self.config.peer_timeout)
                    {
                        online.push(participant_id);
                    } else {
                        unknown.push(participant_id);
                    }
                }
                None => unknown.push(participant_id),
            }
        }

        let mut selected = Vec::with_capacity(threshold);
        selected.push(initiator_id);

        let rotate = |candidates: &mut Vec<u8>| {
            if !candidates.is_empty() {
                let offset = rotation % candidates.len();
                candidates.rotate_left(offset);
            }
        };

        if online.len() >= needed {
            rotate(&mut online);
            selected.extend(online.into_iter().take(needed));
        } else {
            selected.extend(online);
            let remaining = needed.saturating_sub(selected.len().saturating_sub(1));
            rotate(&mut unknown);
            selected.extend(unknown.into_iter().take(remaining));
        }

        if selected.len() < threshold {
            return Err(crate::Error::Protocol(format!(
                "Not enough available participants: need {}, have {}",
                threshold,
                selected.len()
            )));
        }

        Ok(selected)
    }

    /// Handle Round 1 of two-phase signing: CommitmentRequest
    ///
    /// This is called when a coordinator requests commitments from participants.
    /// We generate nonces, store them keyed by session_id, and return our commitment.
    /// The nonces will be used in Round 2 when we receive the SigningPackage.
    pub fn handle_commitment_request(
        &self,
        request: &CommitmentRequest,
    ) -> Result<CommitmentResponse> {
        let frost_share = self.frost_share.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "FrostShare not set. Call set_frost_share() before handling commitment requests."
                    .into(),
            )
        })?;

        // Generate nonces and commitment
        let (nonces, our_commitment) = round1_commit(frost_share);
        let our_commitment_bytes = serialize_commitments(&our_commitment);

        // Store nonces for Round 2 (keyed by session_id)
        {
            let mut pending = self.pending_nonces.lock().map_err(|e| {
                crate::Error::Protocol(format!("Failed to lock pending_nonces: {}", e))
            })?;
            pending.insert(request.session_id.clone(), nonces);
        }

        Ok(CommitmentResponse {
            session_id: request.session_id.clone(),
            participant_id: frost_share.participant_id,
            nonce_commitment: our_commitment_bytes,
        })
    }

    /// Handle Round 2 of two-phase signing: SigningPackage
    ///
    /// This is called when a coordinator sends the full SigningPackage with ALL commitments.
    /// We retrieve our stored nonces from Round 1, build the SigningPackage, sign, and return.
    pub fn handle_signing_package(
        &self,
        package: &SigningPackageMessage,
    ) -> Result<PartialSignature> {
        let frost_share = self.frost_share.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "FrostShare not set. Call set_frost_share() before handling signing packages."
                    .into(),
            )
        })?;

        // Retrieve our stored nonces from Round 1
        let nonces = {
            let mut pending = self.pending_nonces.lock().map_err(|e| {
                crate::Error::Protocol(format!("Failed to lock pending_nonces: {}", e))
            })?;
            pending.remove(&package.session_id).ok_or_else(|| {
                crate::Error::Protocol(format!(
                    "No pending nonces for session {}. Did Round 1 happen?",
                    package.session_id
                ))
            })?
        };

        // Build SigningPackage from ALL commitments
        let mut signing_commitments = BTreeMap::new();
        for pc in &package.commitments {
            let id = Identifier::try_from(pc.participant_id as u16)
                .map_err(|e| crate::Error::Protocol(format!("Invalid participant ID: {:?}", e)))?;
            let commitment = deserialize_commitments(&pc.commitment)?;
            signing_commitments.insert(id, commitment);
        }
        let signing_package = SigningPackage::new(signing_commitments, &package.event_hash);

        // Generate our partial signature
        let sig_share = round2_sign(frost_share, &nonces, &signing_package)?;
        let sig_share_bytes = serialize_sig_share(&sig_share);

        Ok(PartialSignature {
            session_id: package.session_id.clone(),
            participant_id: frost_share.participant_id,
            partial_sig: sig_share_bytes,
        })
    }

    /// Handle an incoming BifrostMessage
    ///
    /// This method routes incoming messages to the appropriate handler:
    /// - CommitmentRequest: Generate nonces and return CommitmentResponse (Round 1)
    /// - SigningPackage: Generate partial signature and return PartialSignature (Round 2)
    /// - EcdhRequest: Generate partial ECDH and return EcdhResponse
    /// - Ping: Automatically handled by transport with Pong
    ///
    /// Returns an optional response message to send back.
    pub fn handle_message(&self, message: &BifrostMessage) -> Result<Option<BifrostMessage>> {
        match message {
            // Two-phase FROST signing protocol (RFC 9591)
            BifrostMessage::CommitmentRequest(request) => {
                let response = self.handle_commitment_request(request)?;
                Ok(Some(BifrostMessage::CommitmentResponse(response)))
            }
            BifrostMessage::SigningPackage(package) => {
                let response = self.handle_signing_package(package)?;
                Ok(Some(BifrostMessage::PartialSignature(response)))
            }
            // ECDH protocol
            BifrostMessage::EcdhRequest(request) => {
                let response = self.handle_ecdh_request(request)?;
                Ok(Some(BifrostMessage::EcdhResponse(response)))
            }
            // Utility
            BifrostMessage::Ping(_) => {
                // Ping/Pong is handled automatically by transport
                Ok(None)
            }
            _ => {
                // Other messages (responses, results, errors) don't require action
                Ok(None)
            }
        }
    }

    /// Perform threshold ECDH with a peer
    ///
    /// This method coordinates a threshold ECDH operation:
    /// 1. Computes our own partial ECDH share
    /// 2. Broadcasts EcdhRequest to threshold peers
    /// 3. Collects k-1 EcdhResponse messages (we are one of the k participants)
    /// 4. Aggregates partial ECDH results into shared secret
    ///
    /// Requires:
    /// - NostrTransport must be initialized (secret_key in config)
    /// - FrostShare must be set (call set_frost_share() first)
    ///
    /// # Returns
    /// 32-byte shared secret compatible with NIP-44 encryption
    pub async fn ecdh(&self, peer_pubkey: &[u8; 32]) -> Result<[u8; 32]> {
        // Check preconditions
        let transport = self.transport.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "NostrTransport not initialized. Provide secret_key in BifrostConfig.".into(),
            )
        })?;

        let frost_share = self.frost_share.as_ref().ok_or_else(|| {
            crate::Error::Protocol("FrostShare not set. Call set_frost_share() before ECDH.".into())
        })?;

        // Get threshold requirement
        let threshold = frost_share.threshold as usize;
        let initiator_id = frost_share.participant_id;

        self.retry_with_backoff(|attempt| async move {
            let participants =
                self.select_participants(threshold, initiator_id, attempt as usize)?;
            self.ecdh_with_participants(peer_pubkey, transport, frost_share, participants)
                .await
        })
        .await
    }

    async fn ecdh_with_participants(
        &self,
        peer_pubkey: &[u8; 32],
        transport: &NostrTransport,
        frost_share: &FrostShare,
        participants: Vec<u8>,
    ) -> Result<[u8; 32]> {
        let threshold = frost_share.threshold as usize;
        let initiator_id = frost_share.participant_id;
        let required_responses = participants.len().saturating_sub(1);
        let participant_set: HashSet<u8> = participants.iter().copied().collect();

        // Step 1: Compute member list for Lagrange coefficients
        let members: Vec<u16> = participants.iter().map(|&p| p as u16).collect();

        // Step 2: Compute our own partial ECDH share
        let our_ecdh_share = create_ecdh_share(frost_share, &members, peer_pubkey)?;

        // Step 3: Create and broadcast EcdhRequest
        let session_id = self.generate_session_id();

        let request = EcdhRequest {
            target_pubkey: *peer_pubkey,
            session_id: session_id.clone(),
            participants: participants.clone(),
        };

        let message = BifrostMessage::EcdhRequest(request);

        // Step 4: Broadcast and wait for (k-1) responses (we are one of the k participants)
        let responses = if required_responses > 0 {
            transport
                .publish_and_wait(&message, required_responses)
                .await?
        } else {
            Vec::new()
        };

        // Step 5: Collect and aggregate responses
        let mut aggregator = EcdhAggregator::new(threshold, session_id);

        // Add our own share first
        aggregator.add_response(frost_share.participant_id, our_ecdh_share.partial_point)?;

        if required_responses > 0 {
            let mut seen = HashSet::new();
            let mut filtered = Vec::new();
            for response in responses {
                if let BifrostMessage::EcdhResponse(ecdh_response) = response
                    && ecdh_response.participant_id != initiator_id
                    && participant_set.contains(&ecdh_response.participant_id)
                    && seen.insert(ecdh_response.participant_id)
                {
                    filtered.push(ecdh_response);
                }
            }

            if filtered.len() < required_responses {
                return Err(crate::Error::Timeout);
            }

            // Add responses from peers
            for response in filtered {
                aggregator.add_response(response.participant_id, response.partial_ecdh)?;
            }
        }

        // Step 6: Aggregate to get shared secret
        let shared_secret = aggregator.aggregate()?;

        Ok(shared_secret)
    }

    /// Run the responder loop to handle incoming requests
    ///
    /// This method continuously receives incoming Bifrost messages and handles them:
    /// - SignRequest → generates SignResponse
    /// - EcdhRequest → generates EcdhResponse
    ///
    /// Call this in a background task for nodes that should respond to other coordinators.
    ///
    /// # Example
    /// ```ignore
    /// // Spawn responder in background
    /// let node = Arc::new(node);
    /// let node_clone = node.clone();
    /// tokio::spawn(async move {
    ///     node_clone.run_responder().await.ok();
    /// });
    /// ```
    pub async fn run_responder(&self) -> Result<()> {
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| crate::Error::Protocol("Transport not initialized".into()))?;

        while self.is_running() {
            // Receive incoming message with timeout
            match tokio::time::timeout(tokio::time::Duration::from_secs(1), transport.receive())
                .await
            {
                Ok(Ok(message)) => {
                    // Handle the message and get optional response
                    if let Ok(Some(response)) = self.handle_message(&message) {
                        // Broadcast response back
                        if let Err(e) = transport.broadcast(&response).await {
                            eprintln!("Failed to broadcast response: {}", e);
                        }
                    }
                }
                Ok(Err(e)) => {
                    // Receive error - channel might be closed
                    if self.is_running() {
                        eprintln!("Responder receive error: {}", e);
                    }
                }
                Err(_) => {
                    // Timeout - continue loop to check running flag
                }
            }
        }

        Ok(())
    }

    /// Handle an incoming EcdhRequest when this node is a responder
    ///
    /// This method is called when another node initiates ECDH and we need to participate:
    /// 1. Compute the member list from participants
    /// 2. Generate our partial ECDH share using Lagrange interpolation
    /// 3. Return an EcdhResponse with our partial point
    pub fn handle_ecdh_request(
        &self,
        request: &crate::bifrost::EcdhRequest,
    ) -> Result<crate::bifrost::EcdhResponse> {
        // Need our frost share to compute ECDH
        let frost_share = self.frost_share.as_ref().ok_or_else(|| {
            crate::Error::Protocol(
                "FrostShare not set. Call set_frost_share() before handling ECDH requests.".into(),
            )
        })?;

        // Convert participants to member indices for Lagrange coefficients
        let members: Vec<u16> = request.participants.iter().map(|&p| p as u16).collect();

        // Compute our partial ECDH share
        let ecdh_share = create_ecdh_share(frost_share, &members, &request.target_pubkey)?;

        // Build and return EcdhResponse
        Ok(crate::bifrost::EcdhResponse {
            session_id: request.session_id.clone(),
            participant_id: frost_share.participant_id,
            partial_ecdh: ecdh_share.partial_point,
        })
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

    #[tokio::test]
    async fn test_retry_with_backoff_succeeds_after_timeouts() {
        let mut config = BifrostConfig::default();
        config.retries.max_retries = 2;
        config.retries.initial_delay_ms = 1;
        config.retries.max_delay_ms = 1;

        let node = BifrostNode::with_config(config).unwrap();
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result = node
            .retry_with_backoff(|_| {
                let attempts = Arc::clone(&attempts_clone);
                async move {
                    let current = attempts.fetch_add(1, Ordering::SeqCst);
                    if current < 2 {
                        Err(crate::Error::Timeout)
                    } else {
                        Ok(current)
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), 2);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("NostrTransport not initialized")
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("FrostShare not set")
        );
    }

    #[tokio::test]
    async fn test_ecdh_requires_transport() {
        let node = BifrostNode::new().unwrap();
        let peer_pubkey = [0x42; 32];

        // Should fail because no transport
        let result = node.ecdh(&peer_pubkey).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("NostrTransport not initialized")
        );
    }

    #[tokio::test]
    async fn test_ecdh_requires_frost_share() {
        let mut config = BifrostConfig::default();
        config.secret_key = Some([0x42; 32]);

        let node = BifrostNode::with_config(config).unwrap();
        let peer_pubkey = [0x42; 32];

        // Should fail because FrostShare not set
        let result = node.ecdh(&peer_pubkey).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("FrostShare not set")
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("FrostShare not set")
        );
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
        // This will fail because no relay connections are configured in test.
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
    fn test_select_participants_skips_offline_peers() {
        let mut config = BifrostConfig::default();
        config.peer_pubkeys = vec![[0x01; 32], [0x02; 32], [0x03; 32]];
        let mut node = BifrostNode::with_config(config).unwrap();

        node.peer_manager_mut().mark_peer_unresponsive(&[0x02; 32]);
        node.peer_manager_mut()
            .mark_peer_responsive(&[0x03; 32], None);

        let participants = node.select_participants(2, 1, 0).unwrap();
        assert_eq!(participants.len(), 2);
        assert_eq!(participants, vec![1, 3]);
    }

    #[test]
    fn test_select_participants_rotation_changes_selection() {
        let mut config = BifrostConfig::default();
        config.peer_pubkeys = vec![[0x01; 32], [0x02; 32], [0x03; 32], [0x04; 32]];
        let node = BifrostNode::with_config(config).unwrap();

        let participants_a = node.select_participants(2, 1, 0).unwrap();
        let participants_b = node.select_participants(2, 1, 1).unwrap();

        assert_ne!(participants_a, participants_b);
    }

    #[test]
    fn test_serialize_commitment() {
        // Generate dummy commitment
        let shares = crate::keygen::generate_key_shares(2, 3).unwrap();
        let (_, commitments) = round1_commit(&shares[0]);

        // Use the serialization module directly
        let bytes = serialize_commitments(&commitments);
        assert_eq!(bytes.len(), 66); // 66 bytes: hiding (33) + binding (33)
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("without transport")
        );
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
        assert!(result.unwrap_err().to_string().contains("already running"));
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("without transport")
        );
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

    #[test]
    fn test_handle_commitment_request_requires_frost_share() {
        let node = BifrostNode::new().unwrap();

        let request = CommitmentRequest {
            event_hash: [0x42; 32],
            session_id: "test-session".to_string(),
            participants: vec![1, 2],
            initiator_id: 1,
        };

        // Should fail because no frost share
        let result = node.handle_commitment_request(&request);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("FrostShare not set")
        );
    }

    #[test]
    fn test_handle_message_routes_commitment_correctly() {
        let mut node = BifrostNode::new().unwrap();

        // Set up frost share
        let shares = crate::keygen::generate_key_shares(2, 3).unwrap();
        node.set_frost_share(shares[1].clone()); // Use share 2 as responder

        let commitment_request = CommitmentRequest {
            event_hash: [0x42; 32],
            session_id: "test-session".to_string(),
            participants: vec![1, 2],
            initiator_id: 1, // Initiated by participant 1
        };

        let message = BifrostMessage::CommitmentRequest(commitment_request);

        // handle_message should return a CommitmentResponse
        let result = node.handle_message(&message);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());

        if let Some(BifrostMessage::CommitmentResponse(resp)) = response {
            assert_eq!(resp.session_id, "test-session");
            assert_eq!(resp.participant_id, shares[1].participant_id);
            assert_eq!(resp.nonce_commitment.len(), 66);
        } else {
            panic!("Expected CommitmentResponse");
        }
    }

    #[test]
    fn test_handle_ecdh_request_requires_frost_share() {
        let node = BifrostNode::new().unwrap();

        let request = crate::bifrost::EcdhRequest {
            target_pubkey: [0x42; 32],
            session_id: "ecdh-session".to_string(),
            participants: vec![1, 2],
        };

        // Should fail because no frost share
        let result = node.handle_ecdh_request(&request);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("FrostShare not set")
        );
    }

    #[test]
    fn test_handle_ecdh_request_produces_valid_response() {
        let mut node = BifrostNode::new().unwrap();

        // Set up frost share
        let shares = crate::keygen::generate_key_shares(2, 3).unwrap();
        node.set_frost_share(shares[1].clone()); // Use share 2 as responder

        // Create a valid peer pubkey (generator point x-coordinate works)
        let peer_pubkey = [
            0x79, 0xBE, 0x66, 0x7E, 0xF9, 0xDC, 0xBB, 0xAC, 0x55, 0xA0, 0x62, 0x95, 0xCE, 0x87,
            0x0B, 0x07, 0x02, 0x9B, 0xFC, 0xDB, 0x2D, 0xCE, 0x28, 0xD9, 0x59, 0xF2, 0x81, 0x5B,
            0x16, 0xF8, 0x17, 0x98,
        ];

        let request = crate::bifrost::EcdhRequest {
            target_pubkey: peer_pubkey,
            session_id: "ecdh-session-123".to_string(),
            participants: vec![1, 2],
        };

        // Should produce valid response
        let result = node.handle_ecdh_request(&request);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.session_id, "ecdh-session-123");
        assert_eq!(response.participant_id, shares[1].participant_id);
        assert_eq!(response.partial_ecdh.len(), 33); // Compressed point
        // First byte should be 0x02 or 0x03 (compressed point prefix)
        assert!(response.partial_ecdh[0] == 0x02 || response.partial_ecdh[0] == 0x03);
    }

    #[test]
    fn test_handle_message_routes_ecdh_correctly() {
        let mut node = BifrostNode::new().unwrap();

        // Set up frost share
        let shares = crate::keygen::generate_key_shares(2, 3).unwrap();
        node.set_frost_share(shares[1].clone()); // Use share 2 as responder

        // Create a valid peer pubkey
        let peer_pubkey = [
            0x79, 0xBE, 0x66, 0x7E, 0xF9, 0xDC, 0xBB, 0xAC, 0x55, 0xA0, 0x62, 0x95, 0xCE, 0x87,
            0x0B, 0x07, 0x02, 0x9B, 0xFC, 0xDB, 0x2D, 0xCE, 0x28, 0xD9, 0x59, 0xF2, 0x81, 0x5B,
            0x16, 0xF8, 0x17, 0x98,
        ];

        let ecdh_request = crate::bifrost::EcdhRequest {
            target_pubkey: peer_pubkey,
            session_id: "ecdh-session".to_string(),
            participants: vec![1, 2],
        };

        let message = BifrostMessage::EcdhRequest(ecdh_request);

        // handle_message should return an EcdhResponse
        let result = node.handle_message(&message);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());

        if let Some(BifrostMessage::EcdhResponse(resp)) = response {
            assert_eq!(resp.session_id, "ecdh-session");
            assert_eq!(resp.participant_id, shares[1].participant_id);
            assert_eq!(resp.partial_ecdh.len(), 33);
        } else {
            panic!("Expected EcdhResponse");
        }
    }
}
