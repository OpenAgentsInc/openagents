//! FRLM integration for Pylon Desktop
//!
//! This module provides adapters to connect the FRLM Conductor
//! with Pylon's Nostr and FM runtimes, as well as trace event
//! processing for UI state updates.

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::mpsc;

use std::time::Duration;

use frlm::conductor::{FrlmConductor, LocalExecutor, SubQuerySubmitter};
use frlm::error::Result as FrlmResult;
use frlm::policy::{FrlmPolicy, Quorum};
use frlm::trace::TraceEvent;
use frlm::types::{FrlmProgram, SubQuery};

use crate::nostr_runtime::{BatchJobRequest, NostrCommand, NostrRuntime};
use crate::state::{ExecutionVenue, FmVizState, FrlmRunState, SubQueryDisplayStatus};

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

/// FRLM Integration for Pylon - handles conductor lifecycle and state updates
pub struct FrlmIntegration {
    /// The trace event receiver (from conductor)
    trace_rx: Option<std::sync::mpsc::Receiver<TraceEvent>>,
    /// Current conductor (if running)
    conductor: Option<FrlmConductor>,
    /// Policy for new runs
    policy: FrlmPolicy,
    /// Manager for submitter/executor
    manager: Option<FrlmManager>,
}

impl FrlmIntegration {
    /// Create a new FRLM integration
    pub fn new() -> Self {
        Self {
            trace_rx: None,
            conductor: None,
            policy: FrlmPolicy::default()
                .with_budget_sats(10000)
                .with_timeout(Duration::from_secs(30))
                .with_quorum(Quorum::Fraction(0.8)),
                // Local fallback is enabled by default
            manager: None,
        }
    }

    /// Initialize with Nostr runtime and FM bridge URL
    pub fn init(&mut self, nostr_runtime: &NostrRuntime, fm_bridge_url: Option<&str>) {
        self.manager = Some(FrlmManager::new(nostr_runtime, fm_bridge_url));
    }

    /// Set the policy for new FRLM runs
    pub fn set_policy(&mut self, policy: FrlmPolicy) {
        self.policy = policy;
    }

    /// Start a new FRLM run
    pub fn start_run(&mut self, program: FrlmProgram, state: &mut FmVizState) {
        // Create conductor with policy
        let mut conductor = FrlmConductor::new(self.policy.clone());

        // Take the trace receiver from the conductor
        self.trace_rx = conductor.take_trace_receiver();

        // Initialize UI state
        let run_state = FrlmRunState::new(
            program.run_id.clone(),
            program.query.clone(),
            program.fragments.len(),
            self.policy.budget.limit_sats,
        );
        state.frlm_active_run = Some(run_state);
        state.frlm_subquery_status.clear();

        self.conductor = Some(conductor);
    }

    /// Check if a run is active
    pub fn is_running(&self) -> bool {
        self.conductor.is_some()
    }

    /// Get the conductor (if running)
    pub fn conductor(&self) -> Option<&FrlmConductor> {
        self.conductor.as_ref()
    }

    /// Get mutable conductor (if running)
    pub fn conductor_mut(&mut self) -> Option<&mut FrlmConductor> {
        self.conductor.as_mut()
    }

    /// Poll trace events and update UI state. Returns true if any events were processed.
    pub fn poll(&mut self, state: &mut FmVizState) -> bool {
        let mut processed = false;

        // Process all available trace events
        if let Some(ref trace_rx) = self.trace_rx {
            while let Ok(event) = trace_rx.try_recv() {
                processed = true;
                Self::update_state_from_trace(event, state);
            }
        }

        processed
    }

    /// Update UI state from a trace event
    fn update_state_from_trace(event: TraceEvent, state: &mut FmVizState) {
        match event {
            TraceEvent::RunInit { run_id, program, fragment_count, .. } => {
                // Update run info if already initialized
                if let Some(ref mut run) = state.frlm_active_run {
                    run.run_id = run_id;
                    run.program = program;
                    run.fragment_count = fragment_count;
                }
            }

            TraceEvent::EnvLoadFragment { .. } => {
                // Fragment loaded - tracked implicitly
            }

            TraceEvent::EnvSelectFragments { .. } => {
                // Fragments selected - tracked implicitly
            }

            TraceEvent::SubQuerySubmit { query_id, prompt_preview, fragment_id, .. } => {
                // Sub-query submitted
                state.frlm_subquery_status.insert(
                    query_id,
                    SubQueryDisplayStatus::Submitted {
                        job_id: fragment_id.unwrap_or(prompt_preview),
                    },
                );
                if let Some(ref mut run) = state.frlm_active_run {
                    run.pending_queries += 1;
                }
            }

            TraceEvent::SubQueryExecute { query_id, provider_id, venue, .. } => {
                // Sub-query being executed by provider
                state.frlm_subquery_status.insert(
                    query_id,
                    SubQueryDisplayStatus::Executing { provider_id: provider_id.clone() },
                );

                // Update topology with venue
                let execution_venue = match venue {
                    frlm::types::Venue::Local => ExecutionVenue::Local,
                    frlm::types::Venue::Swarm => ExecutionVenue::Swarm,
                    frlm::types::Venue::Codex => ExecutionVenue::Codex,
                    frlm::types::Venue::Datacenter => ExecutionVenue::Datacenter,
                    frlm::types::Venue::Unknown => ExecutionVenue::Unknown,
                };
                state.venue_topology.record_execution(execution_venue, Some(&provider_id));
            }

            TraceEvent::SubQueryReturn { query_id, duration_ms, cost_sats, .. } => {
                // Sub-query completed
                state.frlm_subquery_status.insert(
                    query_id,
                    SubQueryDisplayStatus::Complete { duration_ms },
                );
                if let Some(ref mut run) = state.frlm_active_run {
                    run.pending_queries = run.pending_queries.saturating_sub(1);
                    run.completed_queries += 1;
                    run.budget_used_sats += cost_sats;
                    run.budget_remaining_sats = run.budget_remaining_sats.saturating_sub(cost_sats);
                }
            }

            TraceEvent::SubQueryTimeout { query_id, .. } => {
                state.frlm_subquery_status.insert(query_id, SubQueryDisplayStatus::Timeout);
                if let Some(ref mut run) = state.frlm_active_run {
                    run.pending_queries = run.pending_queries.saturating_sub(1);
                }
            }

            TraceEvent::VerifyRedundant { .. } => {
                // Verification complete - could display in UI
            }

            TraceEvent::VerifyObjective { .. } => {
                // Objective verification complete
            }

            TraceEvent::BudgetReserve { amount_sats, .. } => {
                if let Some(ref mut run) = state.frlm_active_run {
                    run.budget_remaining_sats = run.budget_remaining_sats.saturating_sub(amount_sats);
                }
            }

            TraceEvent::BudgetSettle { refund_sats, .. } => {
                if let Some(ref mut run) = state.frlm_active_run {
                    run.budget_remaining_sats += refund_sats;
                }
            }

            TraceEvent::Aggregate { .. } => {
                // Aggregation happened - results combined
            }

            TraceEvent::FallbackLocal { .. } => {
                // Fell back to local - could show indicator in UI
            }

            TraceEvent::RunDone { total_cost_sats, .. } => {
                // Run complete
                state.frlm_runs_completed += 1;
                state.frlm_total_cost_sats += total_cost_sats;
            }
        }
    }

    /// Finish the current run and clean up
    pub fn finish_run(&mut self) {
        self.conductor = None;
        self.trace_rx = None;
    }

    /// Clear all FRLM state
    pub fn clear(&mut self, state: &mut FmVizState) {
        self.conductor = None;
        self.trace_rx = None;
        state.frlm_active_run = None;
        state.frlm_subquery_status.clear();
    }
}

impl Default for FrlmIntegration {
    fn default() -> Self {
        Self::new()
    }
}
