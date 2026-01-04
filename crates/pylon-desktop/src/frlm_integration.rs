//! FRLM integration for Pylon Desktop
//!
//! This module provides adapters to connect the FRLM Conductor
//! with Pylon's Nostr and FM runtimes.

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::mpsc;

use frlm::conductor::{LocalExecutor, SubQuerySubmitter};
use frlm::error::Result as FrlmResult;
use frlm::types::SubQuery;

use crate::nostr_runtime::{BatchJobRequest, NostrCommand, NostrRuntime};

/// Adapter that implements SubQuerySubmitter using Pylon's NostrRuntime.
///
/// This allows the FRLM Conductor to submit batch jobs to the Nostr network.
pub struct NostrSubmitter {
    command_tx: mpsc::Sender<NostrCommand>,
    relay_connected: bool,
}

impl NostrSubmitter {
    /// Create a new NostrSubmitter from a NostrRuntime.
    pub fn new(runtime: &NostrRuntime) -> Self {
        Self {
            command_tx: runtime.command_sender(),
            relay_connected: true, // Assume connected; will be updated
        }
    }

    /// Update connection status.
    pub fn set_connected(&mut self, connected: bool) {
        self.relay_connected = connected;
    }
}

#[async_trait]
impl SubQuerySubmitter for NostrSubmitter {
    async fn submit_batch(&self, queries: Vec<SubQuery>) -> FrlmResult<Vec<(String, String)>> {
        // Convert SubQuery to BatchJobRequest
        let jobs: Vec<BatchJobRequest> = queries
            .iter()
            .map(|q| BatchJobRequest {
                id: q.id.clone(),
                prompt: q.prompt.clone(),
                model: q.model.clone(),
                max_tokens: q.max_tokens,
            })
            .collect();

        // Send command to publish batch
        let _ = self.command_tx.try_send(NostrCommand::PublishJobBatch { jobs: jobs.clone() });

        // Return local_id -> local_id mappings (actual job_id comes async via events)
        // The real job_id mapping will be received via JobBatchPublished event
        Ok(queries.iter().map(|q| (q.id.clone(), q.id.clone())).collect())
    }

    async fn is_available(&self) -> bool {
        self.relay_connected
    }
}

/// Adapter that implements LocalExecutor using Pylon's FM Bridge.
///
/// This allows the FRLM Conductor to fall back to local FM inference.
pub struct FmLocalExecutor {
    fm_bridge_url: String,
}

impl FmLocalExecutor {
    /// Create a new FmLocalExecutor with the FM Bridge URL.
    pub fn new(bridge_url: &str) -> Self {
        Self {
            fm_bridge_url: bridge_url.to_string(),
        }
    }
}

#[async_trait]
impl LocalExecutor for FmLocalExecutor {
    async fn execute(&self, query: &str) -> FrlmResult<String> {
        // Use fm-bridge to execute locally
        // For now, we use a simple HTTP call to the bridge
        let url = format!("http://{}/generate", self.fm_bridge_url);

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(&serde_json::json!({
                "prompt": query,
                "max_tokens": 1000
            }))
            .send()
            .await
            .map_err(|e| frlm::error::FrlmError::Internal(e.to_string()))?;

        if !response.status().is_success() {
            return Err(frlm::error::FrlmError::Internal(format!(
                "FM Bridge returned status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| frlm::error::FrlmError::Internal(e.to_string()))?;

        Ok(result["text"].as_str().unwrap_or("").to_string())
    }
}

/// FRLM Manager for coordinating FRLM runs in Pylon.
pub struct FrlmManager {
    submitter: Arc<NostrSubmitter>,
    local_executor: Option<Arc<FmLocalExecutor>>,
}

impl FrlmManager {
    /// Create a new FrlmManager.
    pub fn new(nostr_runtime: &NostrRuntime, fm_bridge_url: Option<&str>) -> Self {
        let submitter = Arc::new(NostrSubmitter::new(nostr_runtime));
        let local_executor = fm_bridge_url.map(|url| Arc::new(FmLocalExecutor::new(url)));

        Self {
            submitter,
            local_executor,
        }
    }

    /// Get the submitter for passing to FrlmConductor.
    pub fn submitter(&self) -> &Arc<NostrSubmitter> {
        &self.submitter
    }

    /// Get the local executor for passing to FrlmConductor.
    pub fn local_executor(&self) -> Option<&Arc<FmLocalExecutor>> {
        self.local_executor.as_ref()
    }
}
