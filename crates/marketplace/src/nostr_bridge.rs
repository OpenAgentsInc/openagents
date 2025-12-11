//! Bridge between marketplace UI and nostr-chat state.
//!
//! Provides:
//! - Job submission to DVMs
//! - Event polling for UI updates
//! - Active job tracking

use crate::types::DVMListing;
use nostr_chat::{ChatEvent, ChatState, DvmJob};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Bridge between marketplace UI and nostr-chat state.
pub struct NostrBridge {
    chat_state: Arc<ChatState>,
    events_rx: broadcast::Receiver<ChatEvent>,
}

impl NostrBridge {
    /// Create a new bridge from a ChatState.
    pub fn new(chat_state: Arc<ChatState>) -> Self {
        let events_rx = chat_state.subscribe();
        Self {
            chat_state,
            events_rx,
        }
    }

    /// Submit a job to a DVM.
    ///
    /// # Arguments
    /// * `dvm` - The DVM listing to submit the job to
    /// * `input` - The input data for the job
    ///
    /// # Returns
    /// The event ID of the published job request, or an error message
    pub async fn submit_job_to_dvm(
        &self,
        dvm: &DVMListing,
        input: String,
    ) -> Result<String, String> {
        self.chat_state
            .submit_job(
                dvm.kind,
                input,
                vec![],
                Some(vec![dvm.provider_pubkey.clone()]),
                None, // No bid for now
            )
            .await
            .map_err(|e| e.to_string())
    }

    /// Submit a job with parameters.
    pub async fn submit_job_with_params(
        &self,
        dvm: &DVMListing,
        input: String,
        params: Vec<(String, String)>,
        max_bid_msats: Option<u64>,
    ) -> Result<String, String> {
        self.chat_state
            .submit_job(
                dvm.kind,
                input,
                params,
                Some(vec![dvm.provider_pubkey.clone()]),
                max_bid_msats,
            )
            .await
            .map_err(|e| e.to_string())
    }

    /// Get all active jobs.
    pub async fn active_jobs(&self) -> Vec<DvmJob> {
        self.chat_state.jobs().await
    }

    /// Get a specific job by ID.
    pub async fn job(&self, id: &str) -> Option<DvmJob> {
        self.chat_state.job(id).await
    }

    /// Poll for new events (non-blocking).
    ///
    /// Returns all events received since the last poll.
    /// Use this in a gpui timer or frame callback to update UI.
    pub fn poll_events(&mut self) -> Vec<ChatEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events_rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Check if connected to relays.
    pub async fn is_connected(&self) -> bool {
        self.chat_state.connected_count().await > 0
    }

    /// Get the number of connected relays.
    pub async fn connected_relay_count(&self) -> usize {
        self.chat_state.connected_count().await
    }

    /// Subscribe to DVM results (call after connecting).
    pub async fn subscribe_to_results(&self) -> Result<(), String> {
        self.chat_state
            .subscribe_to_dvm_results()
            .await
            .map_err(|e| e.to_string())
    }
}
