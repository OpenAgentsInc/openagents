//! Nostr client for connecting to relays and subscribing to git events

use anyhow::Result;
use nostr::Event;
use nostr_client::{PoolConfig, RelayPool};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::ws::WsBroadcaster;

/// NIP-34 event kinds for git operations
pub mod kinds {
    pub const REPOSITORY_ANNOUNCEMENT: u16 = 30617;
    pub const REPOSITORY_STATE: u16 = 30618;
    pub const PATCH: u16 = 1617;
    pub const PULL_REQUEST: u16 = 1618;
    pub const PR_UPDATE: u16 = 1619;
    pub const ISSUE: u16 = 1621;
    pub const STATUS_OPEN: u16 = 1630;
    pub const STATUS_APPLIED: u16 = 1631;
    pub const STATUS_CLOSED: u16 = 1632;
    pub const STATUS_DRAFT: u16 = 1633;
}

/// Nostr client for AgentGit
pub struct NostrClient {
    pool: Arc<RelayPool>,
    broadcaster: Arc<WsBroadcaster>,
}

impl NostrClient {
    /// Create a new Nostr client with relay URLs
    pub fn new(_relay_urls: Vec<String>, broadcaster: Arc<WsBroadcaster>) -> Self {
        let config = PoolConfig::default();
        let pool = Arc::new(RelayPool::new(config));

        Self { pool, broadcaster }
    }

    /// Connect to all configured relays
    pub async fn connect(&self, relay_urls: Vec<String>) -> Result<()> {
        info!("Connecting to {} relays...", relay_urls.len());

        // Add relays to pool
        for url in &relay_urls {
            if let Err(e) = self.pool.add_relay(url).await {
                error!("Failed to add relay {}: {}", url, e);
            }
        }

        // Connect to all relays
        self.pool.connect_all().await?;

        info!("Successfully connected to relays");
        Ok(())
    }

    /// Subscribe to NIP-34 git events
    pub async fn subscribe_to_git_events(&self) -> Result<()> {
        info!("Subscribing to NIP-34 git events...");

        // Create filter for NIP-34 git events
        let filters = vec![json!({
            "kinds": [
                kinds::REPOSITORY_ANNOUNCEMENT,
                kinds::REPOSITORY_STATE,
                kinds::PATCH,
                kinds::PULL_REQUEST,
                kinds::PR_UPDATE,
                kinds::ISSUE,
                kinds::STATUS_OPEN,
                kinds::STATUS_APPLIED,
                kinds::STATUS_CLOSED,
                kinds::STATUS_DRAFT,
            ],
            "limit": 100
        })];

        // Subscribe and get event receiver
        let mut event_rx = self
            .pool
            .subscribe("agentgit-main", &filters)
            .await?;

        info!("Successfully subscribed to git events");

        // Spawn task to handle incoming events
        let broadcaster = Arc::clone(&self.broadcaster);
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                debug!("Received event: kind={} id={}", event.kind, event.id);

                // Convert event to JSON and broadcast to WebSocket clients
                match serde_json::to_string(&event) {
                    Ok(json) => {
                        broadcaster.broadcast(&format!(
                            r#"<div class="event" data-kind="{}">{}</div>"#,
                            event.kind, json
                        ));
                    }
                    Err(e) => {
                        error!("Failed to serialize event: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Subscribe to a specific repository's events
    #[allow(dead_code)]
    pub async fn subscribe_to_repository(
        &self,
        repo_address: &str,
    ) -> Result<mpsc::UnboundedReceiver<Event>> {
        let filters = vec![json!({
            "kinds": [
                kinds::REPOSITORY_STATE,
                kinds::PATCH,
                kinds::PULL_REQUEST,
                kinds::PR_UPDATE,
                kinds::ISSUE,
            ],
            "#a": [repo_address],
            "limit": 100
        })];

        let event_rx = self
            .pool
            .subscribe(&format!("repo-{}", repo_address), &filters)
            .await?;

        Ok(event_rx)
    }

    /// Disconnect from all relays
    #[allow(dead_code)]
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting from relays...");
        self.pool.disconnect_all().await?;
        Ok(())
    }
}
