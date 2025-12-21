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
    pub async fn fetch_profile(&self, _pubkey: &str) -> Result<Option<Event>> {
        // TODO: Implement profile fetching with filters
        // This will require subscribing to kind:0 events for the pubkey
        Ok(None)
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
