//! Nostr client for connecting to relays and subscribing to git events

use anyhow::Result;
use nostr::Event;
use nostr_client::{PoolConfig, RelayPool};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};

use crate::nostr::cache::EventCache;
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

    // NIP-34 extensions for agent workflows
    pub const ISSUE_CLAIM: u16 = 1634;
    pub const WORK_ASSIGNMENT: u16 = 1635;
    pub const BOUNTY_OFFER: u16 = 1636;
    pub const BOUNTY_CLAIM: u16 = 1637;
}

/// Nostr client for AgentGit
pub struct NostrClient {
    pool: Arc<RelayPool>,
    broadcaster: Arc<WsBroadcaster>,
    cache: Arc<Mutex<EventCache>>,
}

impl NostrClient {
    /// Create a new Nostr client with relay URLs
    pub fn new(_relay_urls: Vec<String>, broadcaster: Arc<WsBroadcaster>) -> Result<Self> {
        let config = PoolConfig::default();
        let pool = Arc::new(RelayPool::new(config));

        // Initialize cache in user's data directory
        let cache_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agentgit");
        let cache_path = cache_dir.join("events.db");
        let cache = Arc::new(Mutex::new(EventCache::new(cache_path)?));

        Ok(Self {
            pool,
            broadcaster,
            cache,
        })
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

        // Create filter for NIP-34 git events and trajectory events
        let filters = vec![json!({
            "kinds": [
                1,     // Text notes (for PR/issue review comments via NIP-22)
                1985,  // Labels (NIP-32, for agent reputation)
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
                kinds::ISSUE_CLAIM,
                kinds::WORK_ASSIGNMENT,
                kinds::BOUNTY_OFFER,
                kinds::BOUNTY_CLAIM,
                38030, // Trajectory Session
                38031, // Trajectory Event
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
        let cache = Arc::clone(&self.cache);
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                debug!("Received event: kind={} id={}", event.kind, event.id);

                // Cache the event
                if let Err(e) = cache.lock().await.insert_event(&event) {
                    error!("Failed to cache event: {}", e);
                }

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

    /// Get cached repositories (useful for offline viewing)
    #[allow(dead_code)]
    pub async fn get_cached_repositories(&self, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.get_repositories(limit)
    }

    /// Get cached issues (useful for offline viewing)
    #[allow(dead_code)]
    pub async fn get_cached_issues(&self, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.get_issues(limit)
    }

    /// Get cached patches (useful for offline viewing)
    #[allow(dead_code)]
    pub async fn get_cached_patches(&self, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.get_patches(limit)
    }

    /// Get cached pull requests (useful for offline viewing)
    #[allow(dead_code)]
    pub async fn get_cached_pull_requests(&self, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.get_pull_requests(limit)
    }

    /// Get a specific cached event by ID
    #[allow(dead_code)]
    pub async fn get_cached_event(&self, event_id: &str) -> Result<Option<Event>> {
        self.cache.lock().await.get_event(event_id)
    }

    /// Get a repository by its identifier (d tag)
    pub async fn get_repository_by_identifier(&self, identifier: &str) -> Result<Option<Event>> {
        self.cache.lock().await.get_repository_by_identifier(identifier)
    }

    /// Get issues for a specific repository by its address tag
    pub async fn get_issues_by_repo(&self, repo_address: &str, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.get_issues_by_repo(repo_address, limit)
    }

    /// Get patches for a specific repository by its address tag
    pub async fn get_patches_by_repo(&self, repo_address: &str, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.get_patches_by_repo(repo_address, limit)
    }

    /// Get pull requests for a specific repository by its address tag
    pub async fn get_pull_requests_by_repo(&self, repo_address: &str, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.get_pull_requests_by_repo(repo_address, limit)
    }

    /// Get cache statistics
    #[allow(dead_code)]
    pub async fn get_cache_stats(&self) -> Result<crate::nostr::cache::CacheStats> {
        self.cache.lock().await.get_stats()
    }

    /// Clear old cached events (events older than max_age_seconds)
    #[allow(dead_code)]
    pub async fn cleanup_cache(&self, max_age_seconds: i64) -> Result<usize> {
        self.cache.lock().await.delete_old_events(max_age_seconds)
    }

    /// Get claims for a specific issue
    pub async fn get_claims_for_issue(&self, issue_event_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_claims_for_issue(issue_event_id)
    }

    /// Get bounty offers for a specific issue
    pub async fn get_bounties_for_issue(&self, issue_event_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_bounties_for_issue(issue_event_id)
    }

    /// Get trajectory session by ID
    pub async fn get_trajectory_session(&self, session_id: &str) -> Result<Option<Event>> {
        self.cache.lock().await.get_trajectory_session(session_id)
    }

    /// Get trajectory events for a session
    pub async fn get_trajectory_events(&self, session_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_trajectory_events(session_id)
    }

    /// Get review comments for a PR or patch
    pub async fn get_reviews_for_pr(&self, pr_event_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_reviews_for_pr(pr_event_id)
    }

    /// Get status events for a PR or patch
    pub async fn get_status_events_for_pr(&self, pr_event_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_status_events_for_pr(pr_event_id)
    }

    /// Get all pull requests by a specific agent (pubkey)
    pub async fn get_pull_requests_by_agent(&self, agent_pubkey: &str, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.get_pull_requests_by_agent(agent_pubkey, limit)
    }

    /// Get all issues claimed by a specific agent (pubkey)
    pub async fn get_issue_claims_by_agent(&self, agent_pubkey: &str, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.get_issue_claims_by_agent(agent_pubkey, limit)
    }

    /// Get reputation labels for an agent (NIP-32, kind:1985)
    pub async fn get_reputation_labels_for_agent(&self, agent_pubkey: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_reputation_labels_for_agent(agent_pubkey)
    }
}
