//! Minimal DVM helpers built on relay pool transport.

use crate::error::{ClientError, Result};
use crate::pool::RelayPool;
use crate::relay::PublishConfirmation;
use nostr::Event;
use std::sync::Arc;

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
}
