//! Minimal DVM helpers built on relay pool transport.

use crate::error::{ClientError, Result};
use crate::pool::RelayPool;
use crate::relay::{PublishConfirmation, RelayMessage};
use nostr::Event;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Lightweight DVM helper for publishing NIP-90 job events.
pub struct DvmClient {
    pool: Arc<RelayPool>,
}

impl DvmClient {
    /// Create a new DVM client from a relay pool.
    pub fn new(pool: Arc<RelayPool>) -> Self {
        Self { pool }
    }

    /// Publish NIP-90 request event.
    pub async fn publish_job_request(&self, event: &Event) -> Result<Vec<PublishConfirmation>> {
        if !nostr::nip90::is_job_request_kind(event.kind) {
            return Err(ClientError::InvalidRequest(format!(
                "expected NIP-90 request kind, got {}",
                event.kind
            )));
        }
        self.pool.publish(event).await
    }

    /// Publish NIP-90 result event.
    pub async fn publish_job_result(&self, event: &Event) -> Result<Vec<PublishConfirmation>> {
        if !nostr::nip90::is_job_result_kind(event.kind) {
            return Err(ClientError::InvalidRequest(format!(
                "expected NIP-90 result kind, got {}",
                event.kind
            )));
        }
        self.pool.publish(event).await
    }

    /// Subscribe for result events referencing a specific request id.
    pub async fn subscribe_results_for_request(
        &self,
        subscription_id: impl Into<String>,
        request_event_id: &str,
    ) -> Result<()> {
        self.pool
            .subscribe_filters(
                subscription_id.into(),
                vec![serde_json::json!({
                    "kinds": [nostr::nip90::JOB_RESULT_KIND_MIN, nostr::nip90::JOB_RESULT_KIND_MAX],
                    "#e": [request_event_id]
                })],
            )
            .await
    }

    /// Wait for a result event that references `request_event_id` in its `e` tag.
    pub async fn await_result_for_request(
        &self,
        request_event_id: &str,
        timeout: Duration,
    ) -> Result<Event> {
        let subscription_id = format!("nip90-result-{}", request_event_id);
        self.subscribe_results_for_request(subscription_id.clone(), request_event_id)
            .await?;

        let deadline = Instant::now() + timeout;
        let poll_step = Duration::from_millis(150);
        loop {
            if Instant::now() >= deadline {
                let _ = self.pool.unsubscribe(subscription_id.as_str()).await;
                return Err(ClientError::Timeout(format!(
                    "timed out waiting for result event for request {}",
                    request_event_id
                )));
            }

            let relays = self.pool.relays().await;
            for relay in relays {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                let wait = poll_step.min(remaining);
                let recv = tokio::time::timeout(wait, relay.recv()).await;
                let message = match recv {
                    Ok(Ok(Some(message))) => message,
                    Ok(Ok(None)) | Ok(Err(_)) | Err(_) => continue,
                };

                if let RelayMessage::Event(_, event) = message
                    && nostr::nip90::is_job_result_kind(event.kind)
                    && event_references_request(&event, request_event_id)
                {
                    let _ = self.pool.unsubscribe(subscription_id.as_str()).await;
                    return Ok(event);
                }
            }

            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    }

    /// Publish a request and await the matching result event.
    pub async fn submit_job_request_and_await_result(
        &self,
        request_event: &Event,
        timeout: Duration,
    ) -> Result<Event> {
        self.publish_job_request(request_event).await?;
        self.await_result_for_request(request_event.id.as_str(), timeout)
            .await
    }
}

fn event_references_request(event: &Event, request_event_id: &str) -> bool {
    event.tags.iter().any(|tag| {
        tag.first().is_some_and(|value| value == "e")
            && tag.get(1).is_some_and(|value| value == request_event_id)
    })
}

#[cfg(test)]
mod tests {
    use super::event_references_request;
    use nostr::Event;

    fn sample_result_event(tags: Vec<Vec<&str>>) -> Event {
        Event {
            id: "result-id".to_string(),
            pubkey: "provider-pubkey".to_string(),
            created_at: 1_760_000_000,
            kind: 6050,
            tags: tags
                .into_iter()
                .map(|tag| tag.into_iter().map(ToString::to_string).collect())
                .collect(),
            content: "{\"result\":\"ok\"}".to_string(),
            sig: "00".repeat(64),
        }
    }

    #[test]
    fn references_request_when_e_tag_matches() {
        let event = sample_result_event(vec![vec!["e", "request-id"], vec!["p", "buyer"]]);
        assert!(event_references_request(&event, "request-id"));
    }

    #[test]
    fn ignores_non_matching_request_tags() {
        let event = sample_result_event(vec![vec!["e", "other-id"], vec!["p", "buyer"]]);
        assert!(!event_references_request(&event, "request-id"));
    }
}
