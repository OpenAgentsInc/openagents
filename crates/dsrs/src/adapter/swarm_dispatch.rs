//! Swarm Job Dispatcher.
//!
//! Dispatches jobs to the OpenAgents swarm via Nostr NIP-90.
//!
//! This module connects to the relay network via DvmClient to submit jobs
//! and receive results from distributed providers.
//!
//! ## Privacy Support
//!
//! SwarmDispatcher supports privacy policies for controlling what jobs can be
//! dispatched and how content is redacted before sending to providers. Use
//! `with_privacy_policy()` to configure.

use crate::privacy::{PolicyViolation, PrivacyPolicy};
use crate::trace::nostr_bridge::NostrBridge;
use anyhow::{Context, Result};
use nostr::{JobInput, JobRequest as NostrJobRequest, Keypair};
use nostr_client::dvm::DvmClient;
use protocol::jobs::{
    chunk_analysis::{ChunkAnalysisRequest, ChunkAnalysisResponse, CodeChunk, OutputConstraints},
    rerank::{RerankCandidate, RerankRequest, RerankResponse},
    sandbox::{SandboxConfig, SandboxRunRequest, SandboxRunResponse},
    JobRequest,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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

    /// DVM client for actual job submission.
    dvm_client: Option<Arc<DvmClient>>,

    /// Configuration.
    config: SwarmDispatchConfig,

    /// Optional privacy policy for controlling job dispatch.
    privacy_policy: Option<PrivacyPolicy>,
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
    /// Create a new swarm dispatcher with random keys (offline mode - no DvmClient).
    pub fn generate() -> Self {
        Self {
            bridge: NostrBridge::generate(),
            dvm_client: None,
            config: SwarmDispatchConfig::default(),
            privacy_policy: None,
        }
    }

    /// Create a new swarm dispatcher with existing keypair (offline mode).
    pub fn new(keypair: Keypair) -> Self {
        Self {
            bridge: NostrBridge::new(keypair),
            dvm_client: None,
            config: SwarmDispatchConfig::default(),
            privacy_policy: None,
        }
    }

    /// Create a new swarm dispatcher with private key (online mode - connected to relays).
    ///
    /// This variant creates a DvmClient for actual job submission to the swarm.
    pub fn with_private_key(private_key: [u8; 32]) -> Result<Self> {
        let dvm_client = DvmClient::new(private_key)
            .map_err(|e| anyhow::anyhow!("Failed to create DVM client: {}", e))?;

        // Derive keypair for NostrBridge from the private key
        let public_key = nostr::get_public_key(&private_key)
            .map_err(|e| anyhow::anyhow!("Failed to derive public key: {:?}", e))?;
        let keypair = Keypair {
            private_key,
            public_key,
        };

        Ok(Self {
            bridge: NostrBridge::new(keypair),
            dvm_client: Some(Arc::new(dvm_client)),
            config: SwarmDispatchConfig::default(),
            privacy_policy: None,
        })
    }

    /// Create from BIP-39 mnemonic (online mode).
    pub fn from_mnemonic(mnemonic: &str) -> Result<Self> {
        let private_key = derive_private_key_from_mnemonic(mnemonic)?;
        Self::with_private_key(private_key)
    }

    /// Check if dispatcher is connected (has DvmClient).
    pub fn is_connected(&self) -> bool {
        self.dvm_client.is_some()
    }

    /// Set privacy policy for controlling job dispatch.
    ///
    /// When a privacy policy is set:
    /// - Job types are checked against the allowlist
    /// - Content is validated against size limits and path restrictions
    /// - Job requests may be blocked if they violate the policy
    pub fn with_privacy_policy(mut self, policy: PrivacyPolicy) -> Self {
        self.privacy_policy = Some(policy);
        self
    }

    /// Get the current privacy policy (if any).
    pub fn privacy_policy(&self) -> Option<&PrivacyPolicy> {
        self.privacy_policy.as_ref()
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
    ///
    /// If DvmClient is available, submits the job to the swarm and waits for result.
    /// Otherwise, returns an error indicating offline mode.
    ///
    /// ## Privacy Policy
    ///
    /// If a privacy policy is configured, the following checks are performed:
    /// - Job type must be in the allowlist (if non-empty)
    /// - Content must pass validation (size limits, path restrictions)
    /// - Trusted provider tags are added to the job request
    async fn dispatch_job<Req, Resp>(&self, request: Req) -> Result<DispatchResult<Resp>>
    where
        Req: JobRequest + Serialize,
        Resp: for<'de> Deserialize<'de>,
    {
        let start = std::time::Instant::now();

        // Check privacy policy constraints
        if let Some(policy) = &self.privacy_policy {
            // Check if job type is allowed
            if !policy.is_job_allowed(Req::JOB_TYPE) {
                return Err(anyhow::anyhow!(
                    "{}",
                    PolicyViolation::JobTypeNotAllowed(Req::JOB_TYPE.to_string())
                ));
            }
        }

        // Compute job hash for tracking
        let job_hash = request.compute_hash().context("Failed to compute job hash")?;

        // Get NIP-90 kind for this job type
        let kind = self.get_job_kind::<Req>();

        // Serialize request content
        let content = serde_json::to_string(&request).context("Failed to serialize request")?;

        // Validate content against privacy policy
        if let Some(policy) = &self.privacy_policy {
            policy.validate_content(&content)?;
        }

        // Check if we have a DVM client for actual submission
        let dvm_client = match &self.dvm_client {
            Some(client) => client,
            None => {
                // Offline mode - create event but don't submit
                let tags = vec![
                    vec!["job_type".to_string(), Req::JOB_TYPE.to_string()],
                    vec!["job_hash".to_string(), job_hash.clone()],
                    vec![
                        "bid".to_string(),
                        self.config.default_budget_msats.to_string(),
                    ],
                ];
                let event = self.bridge.create_event(kind, &content, tags)?;

                return Err(anyhow::anyhow!(
                    "Swarm dispatch in offline mode. \
                     Job {} prepared with hash {} but not submitted. \
                     Event ID: {}. Use with_private_key() or from_mnemonic() for online mode.",
                    Req::JOB_TYPE,
                    job_hash,
                    event.id
                ));
            }
        };

        // Build NIP-90 job request
        let mut nostr_request = NostrJobRequest::new(kind)
            .map_err(|e| anyhow::anyhow!("Failed to create job request: {}", e))?;

        // Add the serialized request as input
        nostr_request = nostr_request.add_input(JobInput::text(&content));
        nostr_request = nostr_request.with_bid(self.config.default_budget_msats);

        // Add job type and hash as parameters
        nostr_request = nostr_request.add_param("job_type", Req::JOB_TYPE);
        nostr_request = nostr_request.add_param("job_hash", &job_hash);

        // Add relays
        for relay in &self.config.relays {
            if !relay.is_empty() {
                nostr_request = nostr_request.add_relay(relay);
            }
        }

        // Add trusted provider hints if privacy policy specifies them
        // This allows the relay to prioritize routing to specific providers
        if let Some(policy) = &self.privacy_policy {
            for provider in &policy.trusted_providers {
                // Add provider pubkey as a parameter hint
                // NIP-90: Providers can filter jobs by these hints
                nostr_request = nostr_request.add_param("preferred_provider", provider);
            }
        }

        // Submit job to relays
        let relay_refs: Vec<&str> = self.config.relays.iter().map(|s| s.as_str()).collect();
        let submission = dvm_client
            .submit_job(nostr_request, &relay_refs)
            .await
            .map_err(|e| anyhow::anyhow!("Job submission failed: {}", e))?;

        let request_event_id = submission.event_id.clone();

        // Await result with timeout
        let result = dvm_client
            .await_result(&request_event_id, self.config.timeout)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Job {} timed out or failed after {:?}: {}",
                    Req::JOB_TYPE,
                    self.config.timeout,
                    e
                )
            })?;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Parse the result content back into the response type
        let response: Resp = serde_json::from_str(&result.content).context(format!(
            "Failed to parse job result for {}: {}",
            Req::JOB_TYPE,
            &result.content[..100.min(result.content.len())]
        ))?;

        Ok(DispatchResult {
            result: response,
            request_event_id,
            result_event_id: None, // JobResult doesn't track its own event ID
            cost_msats: result.amount.unwrap_or(0),
            duration_ms,
        })
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
    private_key: Option<[u8; 32]>,
    config: SwarmDispatchConfig,
}

impl SwarmDispatcherBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            keypair: None,
            private_key: None,
            config: SwarmDispatchConfig::default(),
        }
    }

    /// Set keypair (offline mode).
    pub fn keypair(mut self, keypair: Keypair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    /// Set private key (online mode - enables actual job submission).
    pub fn private_key(mut self, key: [u8; 32]) -> Self {
        self.private_key = Some(key);
        self
    }

    /// Set private key from mnemonic (online mode).
    pub fn mnemonic(mut self, mnemonic: &str) -> Result<Self> {
        let key = derive_private_key_from_mnemonic(mnemonic)?;
        self.private_key = Some(key);
        Ok(self)
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

    /// Build the dispatcher (offline mode).
    pub fn build(self) -> SwarmDispatcher {
        let dispatcher = match self.keypair {
            Some(kp) => SwarmDispatcher::new(kp),
            None => SwarmDispatcher::generate(),
        };
        dispatcher.with_config(self.config)
    }

    /// Build the dispatcher with DvmClient (online mode).
    ///
    /// Requires private_key to be set via `private_key()` or `mnemonic()`.
    pub fn build_connected(self) -> Result<SwarmDispatcher> {
        let private_key = self
            .private_key
            .ok_or_else(|| anyhow::anyhow!("Private key required for connected mode"))?;

        let mut dispatcher = SwarmDispatcher::with_private_key(private_key)?;
        dispatcher.config = self.config;
        Ok(dispatcher)
    }
}

impl Default for SwarmDispatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Derive private key from BIP-39 mnemonic
fn derive_private_key_from_mnemonic(mnemonic: &str) -> Result<[u8; 32]> {
    use bip39::Mnemonic;

    let mnemonic =
        Mnemonic::parse(mnemonic).map_err(|e| anyhow::anyhow!("Invalid mnemonic: {}", e))?;

    let seed = mnemonic.to_seed("");
    // Use first 32 bytes of seed as private key (simplified derivation)
    let mut key = [0u8; 32];
    key.copy_from_slice(&seed[0..32]);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatcher_creation() {
        let dispatcher = SwarmDispatcher::generate();
        assert!(!dispatcher.public_key().is_empty());
        assert!(!dispatcher.is_connected()); // Offline mode
    }

    #[test]
    fn test_dispatcher_with_private_key() {
        let mut key = [0u8; 32];
        key[31] = 1; // Valid non-zero key
        let dispatcher = SwarmDispatcher::with_private_key(key).unwrap();
        assert!(dispatcher.is_connected()); // Online mode
    }

    #[test]
    fn test_config_defaults() {
        let config = SwarmDispatchConfig::default();
        assert_eq!(config.default_budget_msats, 1000);
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert!(config
            .relays
            .contains(&"wss://nexus.openagents.com".to_string()));
    }

    #[test]
    fn test_builder_offline() {
        let dispatcher = SwarmDispatcherBuilder::new()
            .relays(vec!["wss://test.relay".to_string()])
            .budget(5000)
            .timeout(Duration::from_secs(120))
            .build();

        assert_eq!(dispatcher.config.default_budget_msats, 5000);
        assert_eq!(dispatcher.config.timeout, Duration::from_secs(120));
        assert!(!dispatcher.is_connected());
    }

    #[test]
    fn test_builder_connected() {
        let mut key = [0u8; 32];
        key[31] = 1;
        let dispatcher = SwarmDispatcherBuilder::new()
            .private_key(key)
            .relays(vec!["wss://test.relay".to_string()])
            .budget(5000)
            .build_connected()
            .unwrap();

        assert_eq!(dispatcher.config.default_budget_msats, 5000);
        assert!(dispatcher.is_connected());
    }

    #[test]
    fn test_builder_connected_requires_key() {
        let result = SwarmDispatcherBuilder::new()
            .relays(vec!["wss://test.relay".to_string()])
            .build_connected();

        assert!(result.is_err());
    }
}
