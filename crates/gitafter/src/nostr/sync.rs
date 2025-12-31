//! Background sync worker for proactive event cache updates
//!
//! NOTE: This is a foundational module that provides the structure for background
//! event syncing. Full implementation requires integration with NostrClient relay
//! subscription APIs which are being developed.

use anyhow::Result;
use std::time::Duration;
use tracing::info;

/// Configuration for background sync
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Interval between sync cycles
    pub sync_interval: Duration,
    /// Maximum events per batch insertion
    pub max_batch_size: usize,
    /// Relay URLs to sync from
    pub relay_urls: Vec<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            sync_interval: Duration::from_secs(30),
            max_batch_size: 100,
            relay_urls: vec![
                "wss://relay.nostr.bg".to_string(),
                "wss://nos.lol".to_string(),
            ],
        }
    }
}

/// Background sync worker
///
/// This worker will proactively sync events from watched repositories
/// when the NostrClient subscription APIs are fully integrated.
pub struct SyncWorker {
    config: SyncConfig,
}

impl SyncWorker {
    /// Create a new sync worker with the given config
    pub fn new(config: SyncConfig) -> Self {
        info!(
            "Created sync worker with {} relays",
            config.relay_urls.len()
        );
        Self { config }
    }

    /// Get the sync configuration
    pub fn config(&self) -> &SyncConfig {
        &self.config
    }

    /// Placeholder for future start method
    ///
    /// Will spawn background task that:
    /// - Queries watched_repos from EventCache
    /// - Subscribes to NIP-34 events for each repo
    /// - Batches events and inserts via insert_events_batch()
    /// - Tracks last_sync timestamps per repo
    pub async fn start(&self) -> Result<()> {
        info!("Sync worker ready (waiting for NostrClient subscription API integration)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_default() {
        let config = SyncConfig::default();
        assert_eq!(config.sync_interval, Duration::from_secs(30));
        assert_eq!(config.max_batch_size, 100);
        assert_eq!(config.relay_urls.len(), 2);
    }

    #[test]
    fn test_sync_worker_creation() {
        let config = SyncConfig::default();
        let worker = SyncWorker::new(config);
        assert_eq!(worker.config().max_batch_size, 100);
    }
}
