//! Peer discovery and connection management

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

/// Peer status
///
/// # Examples
///
/// ```
/// use frostr::bifrost::PeerStatus;
///
/// let status = PeerStatus::Unknown;
/// assert_eq!(status, PeerStatus::Unknown);
///
/// // Typical status progression
/// let mut status = PeerStatus::Unknown;
/// status = PeerStatus::Online;  // After successful ping
/// assert_eq!(status, PeerStatus::Online);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    /// Peer is online and reachable
    Online,
    /// Peer is offline or unreachable
    Offline,
    /// Unknown status (not yet checked)
    Unknown,
}

/// Peer metadata
///
/// # Examples
///
/// ```
/// use frostr::bifrost::{PeerInfo, PeerStatus};
///
/// // Create new peer
/// let pubkey = [42u8; 32];
/// let mut peer = PeerInfo::new(pubkey);
/// assert_eq!(peer.status, PeerStatus::Unknown);
/// assert_eq!(peer.failed_attempts, 0);
///
/// // Mark peer online
/// peer.mark_online();
/// assert_eq!(peer.status, PeerStatus::Online);
/// assert_eq!(peer.failed_attempts, 0);
///
/// // Mark peer offline
/// peer.mark_offline();
/// assert_eq!(peer.status, PeerStatus::Offline);
/// assert_eq!(peer.failed_attempts, 1);
///
/// // Update latency measurement
/// peer.update_latency(250); // 250ms
/// assert_eq!(peer.latency(), Some(250));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer's Nostr public key (32 bytes x-only)
    pub pubkey: [u8; 32],
    /// Peer's relay list (from NIP-65)
    pub relays: Vec<String>,
    /// Last time peer was seen online
    pub last_seen: u64,
    /// Current peer status
    pub status: PeerStatus,
    /// Number of failed connection attempts
    pub failed_attempts: u32,
    /// Last measured round-trip latency in milliseconds (None if never measured)
    pub latency_ms: Option<u64>,
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(pubkey: [u8; 32]) -> Self {
        Self {
            pubkey,
            relays: Vec::new(),
            last_seen: 0,
            status: PeerStatus::Unknown,
            failed_attempts: 0,
            latency_ms: None,
        }
    }

    /// Mark peer as online
    pub fn mark_online(&mut self) {
        self.status = PeerStatus::Online;
        self.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.failed_attempts = 0;
    }

    /// Mark peer as offline
    pub fn mark_offline(&mut self) {
        self.status = PeerStatus::Offline;
        self.failed_attempts += 1;
    }

    /// Update peer's relay list
    pub fn update_relays(&mut self, relays: Vec<String>) {
        self.relays = relays;
    }

    /// Check if peer was seen recently (within timeout)
    pub fn is_recently_seen(&self, timeout_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.last_seen < timeout_secs
    }

    /// Update peer's latency measurement
    pub fn update_latency(&mut self, latency_ms: u64) {
        self.latency_ms = Some(latency_ms);
    }

    /// Get peer's current latency in milliseconds
    pub fn latency(&self) -> Option<u64> {
        self.latency_ms
    }
}

/// Peer manager for tracking and connecting to threshold peers
///
/// # Examples
///
/// ```
/// use frostr::bifrost::PeerManager;
///
/// // Create manager with 5-minute timeout
/// let mut manager = PeerManager::new(300);
///
/// // Add peers
/// let peer1 = [1u8; 32];
/// let peer2 = [2u8; 32];
/// manager.add_peer(peer1);
/// manager.add_peer(peer2);
///
/// // Get peer info
/// let info = manager.get_peer(&peer1).unwrap();
/// assert_eq!(info.pubkey, peer1);
///
/// // Mark peer online
/// manager.get_peer_mut(&peer1).unwrap().mark_online();
///
/// // Get all online peers
/// let online: Vec<_> = manager.online_peers().collect();
/// assert_eq!(online.len(), 1);
/// ```
pub struct PeerManager {
    /// Known peers by pubkey
    peers: HashMap<[u8; 32], PeerInfo>,
    /// Timeout for considering peer offline (seconds)
    peer_timeout: u64,
    /// Retry backoff configuration
    retry_config: RetryConfig,
}

/// Retry configuration for peer connections
///
/// # Examples
///
/// ```
/// use frostr::bifrost::PeerRetryConfig;
///
/// // Use default configuration
/// let config = PeerRetryConfig::default();
/// assert_eq!(config.initial_delay, 1);  // 1 second
/// assert_eq!(config.max_delay, 300);    // 5 minutes
/// assert_eq!(config.multiplier, 2.0);
///
/// // Calculate exponential backoff delays
/// let delay1 = config.calculate_delay(0);  // Attempt 1: 1s
/// let delay2 = config.calculate_delay(1);  // Attempt 2: 2s
/// let delay3 = config.calculate_delay(2);  // Attempt 3: 4s
/// assert_eq!(delay1.as_secs(), 1);
/// assert_eq!(delay2.as_secs(), 2);
/// assert_eq!(delay3.as_secs(), 4);
///
/// // Delay is capped at max_delay
/// let delay_large = config.calculate_delay(100);
/// assert_eq!(delay_large.as_secs(), 300);  // Capped at 5 minutes
/// ```
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Initial retry delay (seconds)
    pub initial_delay: u64,
    /// Maximum retry delay (seconds)
    pub max_delay: u64,
    /// Backoff multiplier
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            initial_delay: 1,
            max_delay: 300, // 5 minutes
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Calculate retry delay using exponential backoff
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay = self.initial_delay as f64 * self.multiplier.powi(attempt as i32);
        let delay_secs = delay.min(self.max_delay as f64) as u64;
        Duration::from_secs(delay_secs)
    }
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new(peer_timeout: u64) -> Self {
        Self {
            peers: HashMap::new(),
            peer_timeout,
            retry_config: RetryConfig::default(),
        }
    }

    /// Add a peer to the manager
    pub fn add_peer(&mut self, pubkey: [u8; 32]) -> &mut PeerInfo {
        self.peers
            .entry(pubkey)
            .or_insert_with(|| PeerInfo::new(pubkey))
    }

    /// Get peer info
    pub fn get_peer(&self, pubkey: &[u8; 32]) -> Option<&PeerInfo> {
        self.peers.get(pubkey)
    }

    /// Get mutable peer info
    pub fn get_peer_mut(&mut self, pubkey: &[u8; 32]) -> Option<&mut PeerInfo> {
        self.peers.get_mut(pubkey)
    }

    /// Get all peers
    pub fn peers(&self) -> impl Iterator<Item = &PeerInfo> {
        self.peers.values()
    }

    /// Get all online peers
    pub fn online_peers(&self) -> impl Iterator<Item = &PeerInfo> {
        self.peers
            .values()
            .filter(|p| p.status == PeerStatus::Online && p.is_recently_seen(self.peer_timeout))
    }

    /// Find common relays between two peers
    pub fn common_relays(&self, peer1: &[u8; 32], peer2: &[u8; 32]) -> Vec<String> {
        let p1 = match self.peers.get(peer1) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let p2 = match self.peers.get(peer2) {
            Some(p) => p,
            None => return Vec::new(),
        };

        p1.relays
            .iter()
            .filter(|r| p2.relays.contains(r))
            .cloned()
            .collect()
    }

    /// Find common relays among multiple peers
    pub fn common_relays_multi(&self, peers: &[[u8; 32]]) -> Vec<String> {
        if peers.is_empty() {
            return Vec::new();
        }

        // Start with first peer's relays
        let first_peer = match self.peers.get(&peers[0]) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let mut common = first_peer.relays.clone();

        // Intersect with remaining peers
        for pubkey in &peers[1..] {
            if let Some(peer) = self.peers.get(pubkey) {
                common.retain(|r| peer.relays.contains(r));
            } else {
                return Vec::new();
            }
        }

        common
    }

    /// Select optimal relays for communicating with a peer
    pub fn select_relays(&self, peer: &[u8; 32], fallback: &[String]) -> Vec<String> {
        if let Some(info) = self.peers.get(peer)
            && !info.relays.is_empty()
        {
            return info.relays.clone();
        }

        // Fallback to default relays
        fallback.to_vec()
    }

    /// Calculate retry delay for a peer
    pub fn retry_delay(&self, peer: &[u8; 32]) -> Option<Duration> {
        self.peers
            .get(peer)
            .map(|p| self.retry_config.calculate_delay(p.failed_attempts))
    }

    /// Wait for retry delay before next attempt
    pub async fn wait_retry(&self, peer: &[u8; 32]) -> Result<()> {
        if let Some(delay) = self.retry_delay(peer) {
            sleep(delay).await;
            Ok(())
        } else {
            Err(Error::Protocol("peer not found".to_string()))
        }
    }

    /// Update peer relay list from NIP-65 event
    pub fn update_peer_relays(&mut self, pubkey: &[u8; 32], relays: Vec<String>) {
        if let Some(peer) = self.peers.get_mut(pubkey) {
            peer.update_relays(relays);
        } else {
            let mut peer = PeerInfo::new(*pubkey);
            peer.update_relays(relays);
            self.peers.insert(*pubkey, peer);
        }
    }

    /// Mark peer as responding to ping with latency measurement
    pub fn mark_peer_responsive(&mut self, pubkey: &[u8; 32], latency_ms: Option<u64>) {
        if let Some(peer) = self.peers.get_mut(pubkey) {
            peer.mark_online();
            if let Some(latency) = latency_ms {
                peer.update_latency(latency);
            }
        }
    }

    /// Mark peer as unresponsive/offline
    pub fn mark_peer_unresponsive(&mut self, pubkey: &[u8; 32]) {
        if let Some(peer) = self.peers.get_mut(pubkey) {
            peer.mark_offline();
        }
    }

    /// Get peer latency in milliseconds
    pub fn get_peer_latency(&self, pubkey: &[u8; 32]) -> Option<u64> {
        self.peers.get(pubkey).and_then(|p| p.latency())
    }

    /// Health check for all peers
    ///
    /// Checks the health of all tracked peers based on their last_seen timestamp
    /// and current status. Marks peers as offline if they haven't been seen
    /// within the timeout period.
    ///
    /// Returns the number of unhealthy peers detected.
    pub async fn health_check(&mut self) -> Result<usize> {
        let mut unhealthy_count = 0;

        for peer in self.peers.values_mut() {
            // Check if peer hasn't been seen recently
            if !peer.is_recently_seen(self.peer_timeout) {
                // Mark as offline if it was online or unknown
                if peer.status != PeerStatus::Offline {
                    peer.mark_offline();
                    unhealthy_count += 1;
                }
            }
        }

        Ok(unhealthy_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_info_new() {
        let pubkey = [0x42; 32];
        let peer = PeerInfo::new(pubkey);

        assert_eq!(peer.pubkey, pubkey);
        assert_eq!(peer.relays.len(), 0);
        assert_eq!(peer.status, PeerStatus::Unknown);
        assert_eq!(peer.failed_attempts, 0);
    }

    #[test]
    fn test_peer_mark_online() {
        let pubkey = [0x42; 32];
        let mut peer = PeerInfo::new(pubkey);

        peer.mark_online();

        assert_eq!(peer.status, PeerStatus::Online);
        assert_eq!(peer.failed_attempts, 0);
        assert!(peer.last_seen > 0);
    }

    #[test]
    fn test_peer_mark_offline() {
        let pubkey = [0x42; 32];
        let mut peer = PeerInfo::new(pubkey);

        peer.mark_offline();

        assert_eq!(peer.status, PeerStatus::Offline);
        assert_eq!(peer.failed_attempts, 1);

        peer.mark_offline();
        assert_eq!(peer.failed_attempts, 2);
    }

    #[test]
    fn test_mark_peer_unresponsive() {
        let mut manager = PeerManager::new(300);
        let pubkey = [0x42; 32];

        manager.add_peer(pubkey);
        manager.mark_peer_unresponsive(&pubkey);

        let peer = manager.get_peer(&pubkey).unwrap();
        assert_eq!(peer.status, PeerStatus::Offline);
        assert_eq!(peer.failed_attempts, 1);
    }

    #[test]
    fn test_peer_recently_seen() {
        let pubkey = [0x42; 32];
        let mut peer = PeerInfo::new(pubkey);

        peer.mark_online();

        assert!(peer.is_recently_seen(60)); // 60 seconds timeout
    }

    #[test]
    fn test_peer_manager_add() {
        let mut manager = PeerManager::new(300);
        let pubkey = [0x42; 32];

        manager.add_peer(pubkey);

        assert!(manager.get_peer(&pubkey).is_some());
    }

    #[test]
    fn test_common_relays() {
        let mut manager = PeerManager::new(300);

        let peer1 = [0x01; 32];
        let peer2 = [0x02; 32];

        manager.add_peer(peer1).update_relays(vec![
            "wss://relay1.com".to_string(),
            "wss://relay2.com".to_string(),
            "wss://relay3.com".to_string(),
        ]);

        manager.add_peer(peer2).update_relays(vec![
            "wss://relay2.com".to_string(),
            "wss://relay3.com".to_string(),
            "wss://relay4.com".to_string(),
        ]);

        let common = manager.common_relays(&peer1, &peer2);

        assert_eq!(common.len(), 2);
        assert!(common.contains(&"wss://relay2.com".to_string()));
        assert!(common.contains(&"wss://relay3.com".to_string()));
    }

    #[test]
    fn test_common_relays_multi() {
        let mut manager = PeerManager::new(300);

        let peer1 = [0x01; 32];
        let peer2 = [0x02; 32];
        let peer3 = [0x03; 32];

        manager.add_peer(peer1).update_relays(vec![
            "wss://relay1.com".to_string(),
            "wss://relay2.com".to_string(),
        ]);

        manager.add_peer(peer2).update_relays(vec![
            "wss://relay2.com".to_string(),
            "wss://relay3.com".to_string(),
        ]);

        manager.add_peer(peer3).update_relays(vec![
            "wss://relay2.com".to_string(),
            "wss://relay4.com".to_string(),
        ]);

        let common = manager.common_relays_multi(&[peer1, peer2, peer3]);

        assert_eq!(common.len(), 1);
        assert_eq!(common[0], "wss://relay2.com");
    }

    #[test]
    fn test_retry_delay_calculation() {
        let config = RetryConfig::default();

        // First attempt: 1 second
        assert_eq!(config.calculate_delay(0), Duration::from_secs(1));

        // Second attempt: 2 seconds
        assert_eq!(config.calculate_delay(1), Duration::from_secs(2));

        // Third attempt: 4 seconds
        assert_eq!(config.calculate_delay(2), Duration::from_secs(4));

        // Many attempts: capped at max_delay
        assert_eq!(config.calculate_delay(10), Duration::from_secs(300));
    }

    #[test]
    fn test_select_relays_with_peer_relays() {
        let mut manager = PeerManager::new(300);
        let pubkey = [0x42; 32];

        manager
            .add_peer(pubkey)
            .update_relays(vec!["wss://peer-relay.com".to_string()]);

        let fallback = vec!["wss://fallback.com".to_string()];
        let selected = manager.select_relays(&pubkey, &fallback);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0], "wss://peer-relay.com");
    }

    #[test]
    fn test_select_relays_fallback() {
        let manager = PeerManager::new(300);
        let pubkey = [0x42; 32];

        let fallback = vec!["wss://fallback.com".to_string()];
        let selected = manager.select_relays(&pubkey, &fallback);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0], "wss://fallback.com");
    }

    #[test]
    fn test_online_peers_filter() {
        let mut manager = PeerManager::new(300);

        let peer1 = [0x01; 32];
        let peer2 = [0x02; 32];
        let peer3 = [0x03; 32];

        manager.add_peer(peer1).mark_online();
        manager.add_peer(peer2).mark_offline();
        manager.add_peer(peer3).mark_online();

        let online: Vec<_> = manager.online_peers().collect();

        assert_eq!(online.len(), 2);
        assert!(online.iter().any(|p| p.pubkey == peer1));
        assert!(online.iter().any(|p| p.pubkey == peer3));
    }
}
