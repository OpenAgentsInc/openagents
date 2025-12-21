//! Nostr client wrapper for wallet operations

use anyhow::{Context, Result};
use nostr::Event;
use nostr_client::{PublishConfirmation, RelayConnection};
use std::time::Duration;

/// Simple Nostr client for wallet operations
pub struct NostrClient {
    relay_urls: Vec<String>,
}

impl NostrClient {
    /// Create a new client with relay URLs
    pub fn new(relay_urls: Vec<String>) -> Self {
        Self { relay_urls }
    }

    /// Publish an event to all configured relays
    pub async fn publish_event(&self, event: &Event) -> Result<Vec<PublishResult>> {
        let mut results = Vec::new();

        for url in &self.relay_urls {
            let result = self.publish_to_relay(url, event).await;
            results.push(PublishResult {
                relay_url: url.clone(),
                result,
            });
        }

        Ok(results)
    }

    /// Publish event to a single relay
    async fn publish_to_relay(&self, url: &str, event: &Event) -> Result<PublishConfirmation> {
        let relay = RelayConnection::new(url)
            .with_context(|| format!("Failed to create relay connection to {}", url))?;

        relay
            .connect()
            .await
            .with_context(|| format!("Failed to connect to {}", url))?;

        let confirmation = relay
            .publish_event(event, Duration::from_secs(5))
            .await
            .with_context(|| format!("Failed to publish event to {}", url))?;

        relay.disconnect().await.ok(); // Best effort disconnect

        Ok(confirmation)
    }

    /// Fetch profile from relays
    #[allow(dead_code)]
    pub async fn fetch_profile(&self, pubkey: &str) -> Result<Option<Event>> {
        // Create filter for kind:0 (metadata) events
        let filter = serde_json::json!({
            "kinds": [0],
            "authors": [pubkey],
            "limit": 1
        });

        // Fetch events with filter
        let mut events = self.fetch_events(vec![filter]).await?;

        // Return the most recent profile event
        Ok(events.pop())
    }

    /// Fetch events from relays with custom filters
    pub async fn fetch_events(&self, filters: Vec<serde_json::Value>) -> Result<Vec<Event>> {
        use nostr_client::RelayMessage;

        if self.relay_urls.is_empty() {
            return Ok(Vec::new());
        }

        // For simplicity, just use the first relay
        // In a real implementation, we'd merge events from multiple relays
        let url = &self.relay_urls[0];

        let relay = RelayConnection::new(url)
            .with_context(|| format!("Failed to create relay connection to {}", url))?;

        relay
            .connect()
            .await
            .with_context(|| format!("Failed to connect to {}", url))?;

        relay.subscribe("fetch", &filters).await?;

        // Collect events
        let mut events = Vec::new();
        let timeout_duration = Duration::from_secs(5);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout_duration {
            match tokio::time::timeout(Duration::from_secs(1), relay.recv()).await {
                Ok(Ok(Some(msg))) => match msg {
                    RelayMessage::Event(_sub_id, event) => {
                        events.push(event);
                    }
                    RelayMessage::Eose(_sub_id) => {
                        // End of stored events, we can stop
                        break;
                    }
                    _ => {}
                },
                Ok(Ok(None)) => break, // Channel closed
                Ok(Err(_)) => break,   // Error receiving
                Err(_) => continue,    // Timeout, keep trying
            }
        }

        relay.disconnect().await.ok();

        // Sort by timestamp (newest first)
        events.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(events)
    }

    /// Fetch feed events from relays
    pub async fn fetch_feed(&self, limit: usize) -> Result<Vec<Event>> {
        use serde_json::json;

        let filters = vec![json!({
            "kinds": [1],
            "limit": limit
        })];

        self.fetch_events(filters).await
    }
}

/// Result of publishing to a relay
#[derive(Debug)]
pub struct PublishResult {
    pub relay_url: String,
    pub result: Result<PublishConfirmation>,
}

impl PublishResult {
    /// Check if the publish was successful
    pub fn is_success(&self) -> bool {
        match &self.result {
            Ok(confirmation) => confirmation.accepted,
            Err(_) => false,
        }
    }

    /// Get error message if failed
    pub fn error_message(&self) -> Option<String> {
        match &self.result {
            Err(e) => Some(e.to_string()),
            Ok(confirmation) if !confirmation.accepted => {
                if confirmation.message.is_empty() {
                    Some("Rejected by relay".to_string())
                } else {
                    Some(confirmation.message.clone())
                }
            }
            _ => None,
        }
    }
}
