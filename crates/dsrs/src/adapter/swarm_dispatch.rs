//! Swarm Job Dispatcher.
//!
//! Dispatches jobs to the OpenAgents swarm via Nostr NIP-90.

use crate::trace::nostr_bridge::NostrBridge;
use anyhow::{Context, Result};
use nostr::Keypair;
use protocol::jobs::{
    chunk_analysis::{ChunkAnalysisRequest, ChunkAnalysisResponse, CodeChunk, OutputConstraints},
    rerank::{RerankCandidate, RerankRequest, RerankResponse},
    sandbox::{SandboxConfig, SandboxRunRequest, SandboxRunResponse},
    JobRequest,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for swarm dispatch.
#[derive(Debug, Clone)]
pub struct SwarmDispatchConfig {
    /// Nostr relay URLs.
    pub relays: Vec<String>,

    /// Default budget per job in millisatoshis.
    pub default_budget_msats: u64,

    /// Timeout for job completion.
    pub timeout: Duration,

    /// Whether to wait for job confirmation.
    pub wait_for_ok: bool,
}

impl Default for SwarmDispatchConfig {
    fn default() -> Self {
        Self {
            relays: vec!["wss://nexus.openagents.com".to_string()],
            default_budget_msats: 1000,
            timeout: Duration::from_secs(60),
            wait_for_ok: true,
        }
    }
}

/// Swarm job dispatcher for NIP-90 DVM jobs.
pub struct SwarmDispatcher {
    /// Nostr bridge for signing and publishing.
    bridge: NostrBridge,

    /// Configuration.
    config: SwarmDispatchConfig,
}

/// Result of a dispatched job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult<T> {
    /// The job result.
    pub result: T,

    /// Event ID of the job request.
    pub request_event_id: String,

    /// Event ID of the job result.
    pub result_event_id: Option<String>,

    /// Cost in millisatoshis.
    pub cost_msats: u64,

    /// Duration in milliseconds.
    pub duration_ms: u64,
}

impl SwarmDispatcher {
    /// Create a new swarm dispatcher with random keys.
    pub fn generate() -> Self {
        Self {
            bridge: NostrBridge::generate(),
            config: SwarmDispatchConfig::default(),
        }
    }

    /// Create a new swarm dispatcher with existing keypair.
    pub fn new(keypair: Keypair) -> Self {
        Self {
            bridge: NostrBridge::new(keypair),
            config: SwarmDispatchConfig::default(),
        }
    }

    /// Set configuration.
    pub fn with_config(mut self, config: SwarmDispatchConfig) -> Self {
        self.config = config;
        self
    }

    /// Set relay URLs.
    pub fn with_relays(mut self, relays: Vec<String>) -> Self {
        self.config.relays = relays;
        self
    }

    /// Set default budget.
    pub fn with_budget(mut self, budget_msats: u64) -> Self {
        self.config.default_budget_msats = budget_msats;
        self
    }

    /// Set timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Dispatch a chunk analysis job to the swarm.
    pub async fn dispatch_chunk_analysis(
        &self,
        task: impl Into<String>,
        user_task: Option<String>,
        chunk: CodeChunk,
    ) -> Result<DispatchResult<ChunkAnalysisResponse>> {
        let request = ChunkAnalysisRequest {
            task: task.into(),
            user_task,
            chunk,
            output_constraints: OutputConstraints::default(),
            verification: protocol::verification::Verification::subjective_with_judge(2),
        };

        self.dispatch_job(request).await
    }

    /// Dispatch multiple chunk analysis jobs in parallel.
    pub async fn dispatch_chunk_analysis_batch(
        &self,
        task: impl Into<String>,
        user_task: Option<String>,
        chunks: Vec<CodeChunk>,
    ) -> Result<Vec<DispatchResult<ChunkAnalysisResponse>>> {
        use futures::future::join_all;

        let task_str = task.into();
        let futures: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                let task = task_str.clone();
                let user_task = user_task.clone();
                async move { self.dispatch_chunk_analysis(task, user_task, chunk).await }
            })
            .collect();

        let results = join_all(futures).await;

        // Collect successful results
        results.into_iter().collect()
    }

    /// Dispatch a rerank job to the swarm.
    pub async fn dispatch_rerank(
        &self,
        user_task: impl Into<String>,
        candidates: Vec<RerankCandidate>,
        k: usize,
    ) -> Result<DispatchResult<RerankResponse>> {
        let request = RerankRequest {
            user_task: user_task.into(),
            candidates,
            k,
            ranking_rubric: None,
            verification: protocol::verification::Verification::subjective_with_judge(2),
        };

        self.dispatch_job(request).await
    }

    /// Dispatch a sandbox run job to the swarm.
    pub async fn dispatch_sandbox_run(
        &self,
        config: SandboxConfig,
        commands: Vec<String>,
    ) -> Result<DispatchResult<SandboxRunResponse>> {
        use protocol::jobs::sandbox::{RepoMount, SandboxCommand};

        let request = SandboxRunRequest {
            sandbox: config,
            repo: RepoMount::default(),
            commands: commands
                .into_iter()
                .map(SandboxCommand::new)
                .collect(),
            env: std::collections::HashMap::new(),
            verification: protocol::verification::Verification::objective(),
        };

        self.dispatch_job(request).await
    }

    /// Generic job dispatch.
    async fn dispatch_job<Req, Resp>(&self, request: Req) -> Result<DispatchResult<Resp>>
    where
        Req: JobRequest + Serialize,
        Resp: for<'de> Deserialize<'de>,
    {
        let start = std::time::Instant::now();

        // Compute job hash
        let job_hash = request.compute_hash().context("Failed to compute job hash")?;

        // Create NIP-90 job request event
        // Kind 5xxx for job requests (specific kind depends on job type)
        let kind = self.get_job_kind::<Req>();

        // Serialize request
        let content = serde_json::to_string(&request).context("Failed to serialize request")?;

        // Create tags
        let tags = vec![
            vec!["job_type".to_string(), Req::JOB_TYPE.to_string()],
            vec!["job_hash".to_string(), job_hash.clone()],
            vec![
                "bid".to_string(),
                self.config.default_budget_msats.to_string(),
            ],
        ];

        // Create and sign event
        let event = self.bridge.create_event(kind, &content, tags)?;
        let event_id = event.id.clone();

        // In a real implementation, we would:
        // 1. Publish to relays
        // 2. Subscribe for result events (kind 6xxx)
        // 3. Wait for result or timeout
        // 4. Parse and return result

        // For now, return a placeholder
        // Real implementation would use nostr-client to interact with relays
        let duration_ms = start.elapsed().as_millis() as u64;

        // Placeholder: In real impl, we'd wait for the actual response
        Err(anyhow::anyhow!(
            "Swarm dispatch not yet connected to relay network. \
             Job {} prepared with hash {} but not submitted. \
             Event ID: {}",
            Req::JOB_TYPE,
            job_hash,
            event_id
        ))
    }

    /// Get NIP-90 kind for job type.
    fn get_job_kind<Req: JobRequest>(&self) -> u16 {
        // NIP-90 kinds: 5000-5999 for requests, 6000-6999 for results
        match Req::JOB_TYPE {
            "oa.code_chunk_analysis.v1" => 5100,
            "oa.retrieval_rerank.v1" => 5101,
            "oa.sandbox_run.v1" => 5102,
            _ => 5000, // Generic
        }
    }

    /// Get the public key of this dispatcher.
    pub fn public_key(&self) -> String {
        self.bridge.public_key_hex()
    }
}

/// Builder for creating dispatcher with custom configuration.
pub struct SwarmDispatcherBuilder {
    keypair: Option<Keypair>,
    config: SwarmDispatchConfig,
}

impl SwarmDispatcherBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            keypair: None,
            config: SwarmDispatchConfig::default(),
        }
    }

    /// Set keypair.
    pub fn keypair(mut self, keypair: Keypair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    /// Set relays.
    pub fn relays(mut self, relays: Vec<String>) -> Self {
        self.config.relays = relays;
        self
    }

    /// Set budget.
    pub fn budget(mut self, msats: u64) -> Self {
        self.config.default_budget_msats = msats;
        self
    }

    /// Set timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Build the dispatcher.
    pub fn build(self) -> SwarmDispatcher {
        let dispatcher = match self.keypair {
            Some(kp) => SwarmDispatcher::new(kp),
            None => SwarmDispatcher::generate(),
        };
        dispatcher.with_config(self.config)
    }
}

impl Default for SwarmDispatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatcher_creation() {
        let dispatcher = SwarmDispatcher::generate();
        assert!(!dispatcher.public_key().is_empty());
    }

    #[test]
    fn test_config_defaults() {
        let config = SwarmDispatchConfig::default();
        assert_eq!(config.default_budget_msats, 1000);
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert!(config.relays.contains(&"wss://nexus.openagents.com".to_string()));
    }

    #[test]
    fn test_builder() {
        let dispatcher = SwarmDispatcherBuilder::new()
            .relays(vec!["wss://test.relay".to_string()])
            .budget(5000)
            .timeout(Duration::from_secs(120))
            .build();

        assert_eq!(dispatcher.config.default_budget_msats, 5000);
        assert_eq!(dispatcher.config.timeout, Duration::from_secs(120));
    }
}
