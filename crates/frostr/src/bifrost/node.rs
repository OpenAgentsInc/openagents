//! Bifrost node implementation

use crate::bifrost::peer::PeerManager;
use crate::Result;

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
        }
    }
}

/// Bifrost node for coordinating threshold operations
pub struct BifrostNode {
    /// Configuration
    config: BifrostConfig,
    /// Peer manager for tracking threshold peers
    peer_manager: PeerManager,
}

impl BifrostNode {
    /// Create a new Bifrost node with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(BifrostConfig::default())
    }

    /// Create a new Bifrost node with custom configuration
    pub fn with_config(config: BifrostConfig) -> Result<Self> {
        let peer_manager = PeerManager::new(config.peer_timeout);

        Ok(Self {
            config,
            peer_manager,
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

    /// Ping a peer to check connectivity
    pub async fn ping(&mut self, pubkey: &[u8; 32]) -> Result<bool> {
        self.peer_manager.ping(pubkey).await
    }

    /// Perform health check on all peers
    pub async fn health_check(&mut self) -> Result<()> {
        self.peer_manager.health_check().await
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
    /// 1. Broadcasts SignRequest to threshold peers
    /// 2. Collects k-of-n SignResponse messages
    /// 3. Aggregates partial signatures into final signature
    /// 4. Broadcasts SignResult to all participants
    ///
    /// Note: This is a coordinator-side stub. Full implementation requires:
    /// - NostrTransport integration for message publishing
    /// - Access to local FrostShare for aggregation
    /// - Session management for correlating requests/responses
    pub async fn sign(&self, _event_hash: &[u8; 32]) -> Result<[u8; 64]> {
        Err(crate::Error::Protocol(
            "Sign operation requires NostrTransport integration and FrostShare. \
             This will be implemented when BifrostNode is extended with transport \
             and local share management.".into()
        ))
    }

    /// Perform threshold ECDH with a peer
    ///
    /// This method coordinates a threshold ECDH operation:
    /// 1. Broadcasts EcdhRequest to threshold peers
    /// 2. Collects k-of-n EcdhResponse messages
    /// 3. Aggregates partial ECDH results into shared secret
    ///
    /// Note: This is currently not implemented because threshold ECDH
    /// requires multiplicative secret sharing which FROST shares don't
    /// directly support. See crate::ecdh module documentation for details.
    pub async fn ecdh(&self, _peer_pubkey: &[u8; 32]) -> Result<[u8; 32]> {
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
}
