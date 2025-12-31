//! Nostr client for connecting to relays and subscribing to git events

use anyhow::Result;
use nostr::Event;
use nostr_client::{PoolConfig, RelayPool};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use crate::nostr::cache::EventCache;
use crate::nostr::publish_result::{ErrorCategory, PublishResult, RelayFailure};
use crate::nostr::retry::{RetryConfig, retry_with_backoff};
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

/// Nostr client for GitAfter
pub struct NostrClient {
    pool: Arc<RelayPool>,
    broadcaster: Arc<WsBroadcaster>,
    cache: Arc<Mutex<EventCache>>,
    retry_config: RetryConfig,
    /// Event handler task handle for graceful shutdown
    event_handler: Mutex<Option<JoinHandle<()>>>,
}

impl NostrClient {
    /// Create a new Nostr client with relay URLs
    pub fn new(_relay_urls: Vec<String>, broadcaster: Arc<WsBroadcaster>) -> Result<Self> {
        let config = PoolConfig::default();
        let pool = Arc::new(RelayPool::new(config));

        // Initialize cache in user's data directory
        let cache_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("gitafter");
        let cache_path = cache_dir.join("events.db");
        let cache = Arc::new(Mutex::new(EventCache::new(cache_path)?));

        Ok(Self {
            pool,
            broadcaster,
            cache,
            retry_config: RetryConfig::default(),
            event_handler: Mutex::new(None),
        })
    }

    /// Create a new Nostr client with custom retry configuration
    #[allow(dead_code)]
    pub fn with_retry_config(
        _relay_urls: Vec<String>,
        broadcaster: Arc<WsBroadcaster>,
        retry_config: RetryConfig,
    ) -> Result<Self> {
        let config = PoolConfig::default();
        let pool = Arc::new(RelayPool::new(config));

        let cache_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("gitafter");
        let cache_path = cache_dir.join("events.db");
        let cache = Arc::new(Mutex::new(EventCache::new(cache_path)?));

        Ok(Self {
            pool,
            broadcaster,
            cache,
            retry_config,
            event_handler: Mutex::new(None),
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
                39230, // Trajectory Session (NIP-SA)
                39231, // Trajectory Event (NIP-SA)
            ],
            "limit": 100
        })];

        // Subscribe and get event receiver
        let mut event_rx = self.pool.subscribe("gitafter-main", &filters).await?;

        info!("Successfully subscribed to git events");

        // Spawn task to handle incoming events and store the handle
        let broadcaster = Arc::clone(&self.broadcaster);
        let cache = Arc::clone(&self.cache);
        let handle = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                debug!("Received event: kind={} id={}", event.kind, event.id);

                // Cache the event
                if let Err(e) = cache.lock().await.insert_event(&event) {
                    error!("Failed to cache event: {}", e);
                }

                // Create notifications for relevant events
                if let Err(e) = Self::handle_notification_triggers(&cache, &event).await {
                    error!("Failed to create notification: {}", e);
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

        // Store the task handle
        *self.event_handler.lock().await = Some(handle);

        Ok(())
    }

    /// Subscribe to a specific repository's events
    #[allow(dead_code)]
    pub async fn subscribe_to_repository(
        &self,
        repo_address: &str,
    ) -> Result<mpsc::Receiver<Event>> {
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

    /// Shutdown the event handler task gracefully
    #[allow(dead_code)]
    pub async fn shutdown(&self) {
        if let Some(handle) = self.event_handler.lock().await.take() {
            handle.abort();
            let _ = handle.await;
        }
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
        self.cache
            .lock()
            .await
            .get_repository_by_identifier(identifier)
    }

    /// Get notifications for a user
    pub async fn get_notifications(
        &self,
        user_pubkey: &str,
        limit: usize,
    ) -> Result<Vec<crate::nostr::cache::Notification>> {
        self.cache
            .lock()
            .await
            .get_notifications(user_pubkey, limit)
    }

    /// Get unread notification count for a user
    pub async fn get_unread_count(&self, user_pubkey: &str) -> Result<usize> {
        self.cache.lock().await.get_unread_count(user_pubkey)
    }

    /// Mark a notification as read
    pub async fn mark_notification_read(&self, notification_id: &str) -> Result<()> {
        self.cache
            .lock()
            .await
            .mark_notification_read(notification_id)
    }

    /// Mark all notifications as read for a user
    pub async fn mark_all_notifications_read(&self, user_pubkey: &str) -> Result<usize> {
        self.cache
            .lock()
            .await
            .mark_all_notifications_read(user_pubkey)
    }

    /// Get issues for a specific repository by its address tag
    pub async fn get_issues_by_repo(&self, repo_address: &str, limit: usize) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .get_issues_by_repo(repo_address, limit)
    }

    /// Get patches for a specific repository by its address tag
    pub async fn get_patches_by_repo(
        &self,
        repo_address: &str,
        limit: usize,
    ) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .get_patches_by_repo(repo_address, limit)
    }

    /// Get pull requests for a specific repository by its address tag
    pub async fn get_pull_requests_by_repo(
        &self,
        repo_address: &str,
        limit: usize,
    ) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .get_pull_requests_by_repo(repo_address, limit)
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

    /// Get repository state (kind:30618) for a repository
    pub async fn get_repository_state(&self, repo_identifier: &str) -> Result<Option<Event>> {
        self.cache
            .lock()
            .await
            .get_repository_state(repo_identifier)
    }

    /// Get claims for a specific issue
    pub async fn get_claims_for_issue(&self, issue_event_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_claims_for_issue(issue_event_id)
    }

    /// Get bounty offers for a specific issue
    pub async fn get_bounties_for_issue(&self, issue_event_id: &str) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .get_bounties_for_issue(issue_event_id)
    }

    /// Get bounty offers for a specific PR
    pub async fn get_bounties_for_pr(&self, pr_event_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_bounties_for_issue(pr_event_id)
    }

    /// Get bounties for a specific stack layer
    pub async fn get_bounties_for_layer(&self, stack_id: &str, layer: u32) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .get_bounties_for_layer(stack_id, layer)
    }

    /// Get all bounties for a stack (all layers)
    pub async fn get_bounties_for_stack(&self, stack_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_bounties_for_stack(stack_id)
    }

    /// Get the latest status for a PR (returns kind number: 1630=Open, 1631=Merged, 1632=Closed, 1633=Draft)
    pub async fn get_pr_status(&self, pr_event_id: &str) -> Result<u16> {
        let status_events = self
            .cache
            .lock()
            .await
            .get_status_events_for_pr(pr_event_id)?;

        // Return the most recent status event's kind, or 1630 (Open) if no status found
        Ok(status_events.first().map(|e| e.kind).unwrap_or(1630))
    }

    /// Get PR updates for a pull request
    pub async fn get_pr_updates(&self, pr_event_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_pr_updates(pr_event_id)
    }

    /// Get comments for a specific issue (NIP-22)
    pub async fn get_comments_for_issue(&self, issue_event_id: &str) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .get_comments_for_issue(issue_event_id)
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
        self.cache
            .lock()
            .await
            .get_status_events_for_pr(pr_event_id)
    }

    /// Get all pull requests by a specific agent (pubkey)
    pub async fn get_pull_requests_by_agent(
        &self,
        agent_pubkey: &str,
        limit: usize,
    ) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .get_pull_requests_by_agent(agent_pubkey, limit)
    }

    /// Get all issues claimed by a specific agent (pubkey)
    pub async fn get_issue_claims_by_agent(
        &self,
        agent_pubkey: &str,
        limit: usize,
    ) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .get_issue_claims_by_agent(agent_pubkey, limit)
    }

    /// Get reputation labels for an agent (NIP-32, kind:1985)
    pub async fn get_reputation_labels_for_agent(&self, agent_pubkey: &str) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .get_reputation_labels_for_agent(agent_pubkey)
    }

    /// Search repositories by query string (NIP-50)
    pub async fn search_repositories(&self, query: &str, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.search_repositories(query, limit)
    }

    /// Search issues by query string (NIP-50)
    pub async fn search_issues(&self, query: &str, limit: usize) -> Result<Vec<Event>> {
        self.cache.lock().await.search_issues(query, limit)
    }

    /// Watch a repository
    pub async fn watch_repository(&self, repo_identifier: &str, repo_address: &str) -> Result<()> {
        self.cache
            .lock()
            .await
            .watch_repository(repo_identifier, repo_address)
    }

    /// Unwatch a repository
    pub async fn unwatch_repository(&self, repo_identifier: &str) -> Result<()> {
        self.cache.lock().await.unwatch_repository(repo_identifier)
    }

    /// Check if a repository is watched
    pub async fn is_repository_watched(&self, repo_identifier: &str) -> Result<bool> {
        self.cache
            .lock()
            .await
            .is_repository_watched(repo_identifier)
    }

    /// Get all watched repositories
    pub async fn get_watched_repositories(&self) -> Result<Vec<String>> {
        self.cache.lock().await.get_watched_repositories()
    }

    /// Get all pull requests in a stack by stack ID
    #[allow(dead_code)]
    pub async fn get_pull_requests_by_stack(&self, stack_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_pull_requests_by_stack(stack_id)
    }

    /// Get the dependency PR for a given PR
    #[allow(dead_code)]
    pub async fn get_dependency_pr(&self, pr_event: &Event) -> Result<Option<Event>> {
        self.cache.lock().await.get_dependency_pr(pr_event)
    }

    /// Check if a PR's dependencies are satisfied (mergeable)
    #[allow(dead_code)]
    pub async fn is_pr_mergeable(&self, pr_event: &Event) -> Result<bool> {
        self.cache.lock().await.is_pr_mergeable(pr_event)
    }

    /// Get PRs that depend on the given PR (later layers in a stack)
    #[allow(dead_code)]
    pub async fn get_dependent_prs(&self, pr_id: &str) -> Result<Vec<Event>> {
        self.cache.lock().await.get_dependent_prs(pr_id)
    }

    /// Publish a signed event to all connected relays with retry and error handling
    #[allow(dead_code)]
    pub async fn publish_event(&self, event: Event) -> Result<PublishResult> {
        info!("Publishing event: kind={} id={}", event.kind, event.id);
        let event_id = event.id.clone();

        // Store in local cache first (best effort)
        if let Err(e) = self.cache.lock().await.insert_event(&event) {
            error!("Failed to cache event: {}", e);
        }

        // Publish to relay pool with retry
        let pool = Arc::clone(&self.pool);
        let event_clone = event.clone();

        let publish_result = retry_with_backoff(&self.retry_config, "relay publish", || {
            let pool = Arc::clone(&pool);
            let event = event_clone.clone();
            async move {
                // Add timeout to prevent indefinite hanging
                tokio::time::timeout(std::time::Duration::from_secs(10), pool.publish(&event))
                    .await
                    .map_err(|_| anyhow::anyhow!("Publish timeout"))
                    .and_then(|r| r.map_err(|e| anyhow::anyhow!("Publish failed: {}", e)))
            }
        })
        .await;

        // Build detailed result
        let result = match publish_result {
            Ok(confirmations) => {
                let confirmation_count = confirmations.len();
                debug!("Received {} publish confirmations", confirmation_count);

                // Broadcast to UI for real-time updates
                if let Ok(json) = serde_json::to_string(&event) {
                    self.broadcaster.broadcast(&format!(
                        r#"<div class="event event-published" data-kind="{}">{}</div>"#,
                        event.kind, json
                    ));
                }

                info!(
                    "Successfully published event: {} to {} relays",
                    event_id, confirmation_count
                );

                PublishResult::success(event_id, confirmation_count, confirmation_count)
            }
            Err(e) => {
                error!("Failed to publish event {}: {}", event_id, e);

                // Create detailed failure result
                let error_msg = e.to_string();
                let category = ErrorCategory::from_error_message(&error_msg);

                let failure = RelayFailure {
                    relay_url: "pool".to_string(),
                    error: error_msg.clone(),
                    category,
                };

                // Broadcast failure to UI
                self.broadcaster.broadcast(&format!(
                    r#"<div class="event event-failed" data-kind="{}" data-error="{}">Failed to publish event</div>"#,
                    event.kind,
                    html_escape::encode_text(&error_msg)
                ));

                PublishResult::failure(event_id, 0, 1, vec![failure])
            }
        };

        Ok(result)
    }

    /// Publish a signed event without retry (for time-sensitive operations)
    #[allow(dead_code)]
    pub async fn publish_event_no_retry(&self, event: Event) -> Result<PublishResult> {
        info!(
            "Publishing event (no retry): kind={} id={}",
            event.kind, event.id
        );
        let event_id = event.id.clone();

        // Store in local cache first
        if let Err(e) = self.cache.lock().await.insert_event(&event) {
            error!("Failed to cache event: {}", e);
        }

        // Publish with timeout
        let publish_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), self.pool.publish(&event))
                .await;

        let result = match publish_result {
            Ok(Ok(confirmations)) => {
                let count = confirmations.len();
                info!("Published event {} to {} relays", event_id, count);

                PublishResult::success(event_id, count, count)
            }
            Ok(Err(e)) => {
                let error_msg = e.to_string();
                let failure = RelayFailure {
                    relay_url: "pool".to_string(),
                    error: error_msg.clone(),
                    category: ErrorCategory::from_error_message(&error_msg),
                };
                PublishResult::failure(event_id, 0, 1, vec![failure])
            }
            Err(_) => {
                let failure = RelayFailure {
                    relay_url: "pool".to_string(),
                    error: "Publish timeout (5s)".to_string(),
                    category: ErrorCategory::Timeout,
                };
                PublishResult::failure(event_id, 0, 1, vec![failure])
            }
        };

        Ok(result)
    }

    /// Handle notification triggers for incoming events
    async fn handle_notification_triggers(
        cache: &Arc<Mutex<EventCache>>,
        event: &Event,
    ) -> Result<()> {
        let cache_lock = cache.lock().await;

        match event.kind {
            // Review comment on PR (kind:1 with 'e' tag referencing PR)
            1 => {
                // Find PR being commented on via 'e' tag
                for tag in &event.tags {
                    if tag.len() >= 2 && tag[0] == "e" {
                        let pr_event_id = &tag[1];

                        // Get the PR event to find its author
                        if let Ok(Some(pr_event)) = cache_lock.get_event(pr_event_id) {
                            if pr_event.kind == kinds::PULL_REQUEST {
                                // Don't notify if user is commenting on their own PR
                                if pr_event.pubkey != event.pubkey {
                                    let title = "New review on your PR".to_string();
                                    let preview = if event.content.len() > 100 {
                                        Some(format!("{}...", &event.content[..100]))
                                    } else {
                                        Some(event.content.clone())
                                    };

                                    cache_lock.create_notification(
                                        &pr_event.pubkey,
                                        &event.id,
                                        event.kind,
                                        "pr_review",
                                        &title,
                                        preview.as_deref(),
                                    )?;
                                }
                                break;
                            }
                        }
                    }
                }
            }
            // Status change events (approve, merge, close, draft)
            kinds::STATUS_OPEN
            | kinds::STATUS_APPLIED
            | kinds::STATUS_CLOSED
            | kinds::STATUS_DRAFT => {
                for tag in &event.tags {
                    if tag.len() >= 2 && tag[0] == "e" {
                        let pr_event_id = &tag[1];

                        if let Ok(Some(pr_event)) = cache_lock.get_event(pr_event_id) {
                            if pr_event.kind == kinds::PULL_REQUEST
                                && pr_event.pubkey != event.pubkey
                            {
                                let status_name = match event.kind {
                                    kinds::STATUS_OPEN => "opened",
                                    kinds::STATUS_APPLIED => "merged",
                                    kinds::STATUS_CLOSED => "closed",
                                    kinds::STATUS_DRAFT => "marked as draft",
                                    _ => "updated",
                                };

                                let title = format!("Your PR was {}", status_name);

                                cache_lock.create_notification(
                                    &pr_event.pubkey,
                                    &event.id,
                                    event.kind,
                                    "pr_status",
                                    &title,
                                    None,
                                )?;
                                break;
                            }
                        }
                    }
                }
            }
            // Issue claim (kind:1634)
            kinds::ISSUE_CLAIM => {
                for tag in &event.tags {
                    if tag.len() >= 2 && tag[0] == "e" {
                        let issue_event_id = &tag[1];

                        if let Ok(Some(issue_event)) = cache_lock.get_event(issue_event_id) {
                            if issue_event.kind == kinds::ISSUE
                                && issue_event.pubkey != event.pubkey
                            {
                                let title = "Someone claimed your issue".to_string();

                                cache_lock.create_notification(
                                    &issue_event.pubkey,
                                    &event.id,
                                    event.kind,
                                    "issue_claim",
                                    &title,
                                    None,
                                )?;
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}
