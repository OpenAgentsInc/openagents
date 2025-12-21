//! Bifrost node implementation

use crate::bifrost::peer::PeerManager;
use crate::Result;

/// Bifrost node configuration
#[derive(Debug, Clone)]
pub struct BifrostConfig {
    /// Peer timeout in seconds
    pub peer_timeout: u64,
    /// Default relays for fallback
    pub default_relays: Vec<String>,
}

impl Default for BifrostConfig {
    fn default() -> Self {
        Self {
            peer_timeout: 300, // 5 minutes
            default_relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
            ],
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

    /// Sign an event hash using threshold shares
    pub async fn sign(&self, _event_hash: &[u8; 32]) -> Result<[u8; 64]> {
        todo!("Implement threshold signing")
    }

    /// Perform threshold ECDH with a peer
    pub async fn ecdh(&self, _peer_pubkey: &[u8; 32]) -> Result<[u8; 32]> {
        todo!("Implement threshold ECDH")
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
        };

        let node = BifrostNode::with_config(config).unwrap();

        assert_eq!(node.config.peer_timeout, 600);
        assert_eq!(node.config.default_relays.len(), 1);
    }
}
