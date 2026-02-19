//! Outbox model for intelligent relay selection
//!
//! Implements NIP-65 outbox model for improved message delivery by publishing
//! to recipients' preferred relays based on their relay list metadata.
//!
//! The outbox model works as follows:
//! - When downloading events FROM a user, use their WRITE relays
//! - When downloading events ABOUT a user (mentions), use their READ relays
//! - When publishing an event, send to:
//!   - WRITE relays of the author
//!   - READ relays of each tagged user

use crate::error::{ClientError, Result};
use nostr::{Event, RELAY_LIST_METADATA_KIND, RelayListMetadata};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// Cache entry for relay list metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Relay list metadata
    metadata: RelayListMetadata,
    /// When this entry was cached
    cached_at: SystemTime,
}

/// Configuration for outbox model
#[derive(Debug, Clone)]
pub struct OutboxConfig {
    /// How long to cache relay list metadata
    pub cache_ttl: Duration,
    /// Whether to enable the outbox model
    pub enabled: bool,
    /// Fallback relays to use when user has no relay list
    pub fallback_relays: Vec<String>,
}

impl Default for OutboxConfig {
    fn default() -> Self {
        Self {
            cache_ttl: Duration::from_secs(3600), // 1 hour
            enabled: true,
            fallback_relays: vec![],
        }
    }
}

/// Outbox model relay selector
///
/// Caches user relay lists and provides methods for intelligent relay selection
pub struct OutboxModel {
    /// Configuration
    config: OutboxConfig,
    /// Cache of relay lists by pubkey
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl OutboxModel {
    /// Create a new outbox model with default config
    pub fn new() -> Self {
        Self::with_config(OutboxConfig::default())
    }

    /// Create a new outbox model with custom config
    pub fn with_config(config: OutboxConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update relay list metadata for a user
    ///
    /// This should be called whenever a kind 10002 event is received
    pub fn update_relay_list(&self, pubkey: &str, event: &Event) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        if event.kind != RELAY_LIST_METADATA_KIND {
            return Err(ClientError::InvalidEvent(format!(
                "expected kind {}, got {}",
                RELAY_LIST_METADATA_KIND, event.kind
            )));
        }

        let metadata = RelayListMetadata::from_event(event)
            .map_err(|e| ClientError::InvalidEvent(e.to_string()))?;

        let entry = CacheEntry {
            metadata,
            cached_at: SystemTime::now(),
        };

        let mut cache = self
            .cache
            .write()
            .map_err(|_| ClientError::Internal("Cache lock poisoned".to_string()))?;
        cache.insert(pubkey.to_string(), entry);

        Ok(())
    }

    /// Get write relays for a user (where to download their events from)
    pub fn get_write_relays(&self, pubkey: &str) -> Vec<String> {
        if !self.config.enabled {
            return self.config.fallback_relays.clone();
        }

        let cache = match self.cache.read() {
            Ok(c) => c,
            Err(_) => return self.config.fallback_relays.clone(),
        };

        if let Some(entry) = cache.get(pubkey)
            && let Ok(elapsed) = entry.cached_at.elapsed()
            && elapsed < self.config.cache_ttl
        {
            let relays = entry.metadata.write_relays();
            if !relays.is_empty() {
                return relays;
            }
        }

        // Return fallback relays if no cached data or cache expired
        self.config.fallback_relays.clone()
    }

    /// Get read relays for a user (where to publish mentions of them)
    pub fn get_read_relays(&self, pubkey: &str) -> Vec<String> {
        if !self.config.enabled {
            return self.config.fallback_relays.clone();
        }

        let cache = match self.cache.read() {
            Ok(c) => c,
            Err(_) => return self.config.fallback_relays.clone(),
        };

        if let Some(entry) = cache.get(pubkey)
            && let Ok(elapsed) = entry.cached_at.elapsed()
            && elapsed < self.config.cache_ttl
        {
            let relays = entry.metadata.read_relays();
            if !relays.is_empty() {
                return relays;
            }
        }

        // Return fallback relays if no cached data or cache expired
        self.config.fallback_relays.clone()
    }

    /// Get relays to publish an event to, based on the outbox model
    ///
    /// Returns:
    /// - Author's WRITE relays
    /// - READ relays of all tagged users (p-tags)
    pub fn get_publish_relays(&self, event: &Event) -> Vec<String> {
        if !self.config.enabled {
            return self.config.fallback_relays.clone();
        }

        let mut relays = Vec::new();

        // Add author's write relays
        let author_relays = self.get_write_relays(&event.pubkey);
        relays.extend(author_relays);

        // Add read relays of all tagged users
        for tag in &event.tags {
            if tag.len() >= 2 && tag[0] == "p" {
                let tagged_pubkey = &tag[1];
                let tagged_relays = self.get_read_relays(tagged_pubkey);
                relays.extend(tagged_relays);
            }
        }

        // Deduplicate relays
        relays.sort();
        relays.dedup();

        // If no relays found, use fallback
        if relays.is_empty() {
            relays = self.config.fallback_relays.clone();
        }

        relays
    }

    /// Check if we have cached relay list for a user
    pub fn has_relay_list(&self, pubkey: &str) -> bool {
        let cache = match self.cache.read() {
            Ok(c) => c,
            Err(_) => return false,
        };

        if let Some(entry) = cache.get(pubkey)
            && let Ok(elapsed) = entry.cached_at.elapsed()
        {
            return elapsed < self.config.cache_ttl;
        }

        false
    }

    /// Clear the relay list cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Remove a specific user's relay list from cache
    pub fn remove_from_cache(&self, pubkey: &str) {
        if let Ok(mut cache) = self.cache.write() {
            cache.remove(pubkey);
        }
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = match self.cache.read() {
            Ok(c) => c,
            Err(_) => return (0, 0),
        };
        let total = cache.len();

        let valid = cache
            .values()
            .filter(|entry| {
                entry
                    .cached_at
                    .elapsed()
                    .map(|elapsed| elapsed < self.config.cache_ttl)
                    .unwrap_or(false)
            })
            .count();

        (valid, total)
    }
}

impl Default for OutboxModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(pubkey: &str, kind: u16, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test_id".to_string(),
            pubkey: pubkey.to_string(),
            created_at: 1707409439,
            kind,
            tags,
            content: "".to_string(),
            sig: "test_sig".to_string(),
        }
    }

    fn create_relay_list_event(pubkey: &str) -> Event {
        create_test_event(
            pubkey,
            RELAY_LIST_METADATA_KIND,
            vec![
                vec![
                    "r".to_string(),
                    "wss://write1.com".to_string(),
                    "write".to_string(),
                ],
                vec![
                    "r".to_string(),
                    "wss://read1.com".to_string(),
                    "read".to_string(),
                ],
                vec!["r".to_string(), "wss://both.com".to_string()],
            ],
        )
    }

    #[test]
    fn test_outbox_model_update_relay_list() {
        let outbox = OutboxModel::new();
        let event = create_relay_list_event("pubkey1");

        outbox.update_relay_list("pubkey1", &event).unwrap();
        assert!(outbox.has_relay_list("pubkey1"));
    }

    #[test]
    fn test_outbox_model_get_write_relays() {
        let outbox = OutboxModel::new();
        let event = create_relay_list_event("pubkey1");

        outbox.update_relay_list("pubkey1", &event).unwrap();

        let write_relays = outbox.get_write_relays("pubkey1");
        assert_eq!(write_relays.len(), 2); // write1 and both
        assert!(write_relays.contains(&"wss://write1.com".to_string()));
        assert!(write_relays.contains(&"wss://both.com".to_string()));
    }

    #[test]
    fn test_outbox_model_get_read_relays() {
        let outbox = OutboxModel::new();
        let event = create_relay_list_event("pubkey1");

        outbox.update_relay_list("pubkey1", &event).unwrap();

        let read_relays = outbox.get_read_relays("pubkey1");
        assert_eq!(read_relays.len(), 2); // read1 and both
        assert!(read_relays.contains(&"wss://read1.com".to_string()));
        assert!(read_relays.contains(&"wss://both.com".to_string()));
    }

    #[test]
    fn test_outbox_model_get_publish_relays() {
        let outbox = OutboxModel::new();

        // Setup relay lists for author and tagged user
        let author_event = create_relay_list_event("author");
        let tagged_event = create_relay_list_event("tagged");

        outbox.update_relay_list("author", &author_event).unwrap();
        outbox.update_relay_list("tagged", &tagged_event).unwrap();

        // Create an event with a p-tag
        let event = create_test_event(
            "author",
            1,
            vec![vec!["p".to_string(), "tagged".to_string()]],
        );

        let publish_relays = outbox.get_publish_relays(&event);

        // Should contain author's write relays and tagged user's read relays
        assert!(publish_relays.contains(&"wss://write1.com".to_string())); // author write
        assert!(publish_relays.contains(&"wss://both.com".to_string())); // both
        assert!(publish_relays.contains(&"wss://read1.com".to_string())); // tagged read
    }

    #[test]
    fn test_outbox_model_fallback_relays() {
        let config = OutboxConfig {
            cache_ttl: Duration::from_secs(3600),
            enabled: true,
            fallback_relays: vec!["wss://fallback.com".to_string()],
        };
        let outbox = OutboxModel::with_config(config);

        // Get relays for unknown user should return fallback
        let relays = outbox.get_write_relays("unknown");
        assert_eq!(relays.len(), 1);
        assert_eq!(relays[0], "wss://fallback.com");
    }

    #[test]
    fn test_outbox_model_clear_cache() {
        let outbox = OutboxModel::new();
        let event = create_relay_list_event("pubkey1");

        outbox.update_relay_list("pubkey1", &event).unwrap();
        assert!(outbox.has_relay_list("pubkey1"));

        outbox.clear_cache();
        assert!(!outbox.has_relay_list("pubkey1"));
    }

    #[test]
    fn test_outbox_model_remove_from_cache() {
        let outbox = OutboxModel::new();
        let event1 = create_relay_list_event("pubkey1");
        let event2 = create_relay_list_event("pubkey2");

        outbox.update_relay_list("pubkey1", &event1).unwrap();
        outbox.update_relay_list("pubkey2", &event2).unwrap();

        outbox.remove_from_cache("pubkey1");
        assert!(!outbox.has_relay_list("pubkey1"));
        assert!(outbox.has_relay_list("pubkey2"));
    }

    #[test]
    fn test_outbox_model_cache_stats() {
        let outbox = OutboxModel::new();
        let event = create_relay_list_event("pubkey1");

        outbox.update_relay_list("pubkey1", &event).unwrap();

        let (valid, total) = outbox.cache_stats();
        assert_eq!(valid, 1);
        assert_eq!(total, 1);
    }

    #[test]
    fn test_outbox_model_disabled() {
        let config = OutboxConfig {
            cache_ttl: Duration::from_secs(3600),
            enabled: false,
            fallback_relays: vec!["wss://fallback.com".to_string()],
        };
        let outbox = OutboxModel::with_config(config);
        let event = create_relay_list_event("pubkey1");

        outbox.update_relay_list("pubkey1", &event).unwrap();

        // When disabled, should always return fallback relays
        let relays = outbox.get_write_relays("pubkey1");
        assert_eq!(relays.len(), 1);
        assert_eq!(relays[0], "wss://fallback.com");
    }

    #[test]
    fn test_outbox_model_deduplicates_relays() {
        let outbox = OutboxModel::new();

        // Create relay list with overlapping relays
        let author_event = create_test_event(
            "author",
            RELAY_LIST_METADATA_KIND,
            vec![vec!["r".to_string(), "wss://shared.com".to_string()]],
        );

        let tagged_event = create_test_event(
            "tagged",
            RELAY_LIST_METADATA_KIND,
            vec![vec!["r".to_string(), "wss://shared.com".to_string()]],
        );

        outbox.update_relay_list("author", &author_event).unwrap();
        outbox.update_relay_list("tagged", &tagged_event).unwrap();

        let event = create_test_event(
            "author",
            1,
            vec![vec!["p".to_string(), "tagged".to_string()]],
        );

        let publish_relays = outbox.get_publish_relays(&event);

        // Should deduplicate the shared relay
        assert_eq!(
            publish_relays
                .iter()
                .filter(|r| *r == "wss://shared.com")
                .count(),
            1
        );
    }
}
