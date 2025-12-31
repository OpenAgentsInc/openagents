//! Compute filesystem service and providers.

use crate::budget::{BudgetError, BudgetPolicy, BudgetReservation, BudgetTracker};
use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Permissions,
    SeekFrom, Stat, WatchEvent, WatchHandle,
};
use crate::idempotency::{IdempotencyJournal, JournalError};
use crate::types::{AgentId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock as TokioRwLock};

use ::compute as compute_provider;
use compute_provider::backends::{
    BackendRegistry, CompletionRequest, CompletionResponse, UsageInfo as BackendUsageInfo,
};
use compute_provider::backends::{BackendError, InferenceBackend};

const IDEMPOTENCY_TTL: Duration = Duration::from_secs(3600);

/// Kind of compute operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeKind {
    /// Chat/conversational completion.
    Chat,
    /// Raw text completion.
    Complete,
    /// Generate embeddings.
    Embeddings,
    /// Generate images.
    ImageGenerate,
    /// Analyze images (vision).
    ImageAnalyze,
    /// Audio transcription.
    Transcribe,
    /// Text-to-speech.
    Speak,
    /// Custom operation type.
    Custom(String),
}

/// A compute request (provider-agnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRequest {
    /// Model identifier.
    pub model: String,
    /// Kind of compute operation.
    pub kind: ComputeKind,
    /// Input data (schema depends on kind).
    pub input: serde_json::Value,
    /// Request streaming response.
    #[serde(default)]
    pub stream: bool,
    /// Timeout in milliseconds (default: 120000).
    pub timeout_ms: Option<u64>,
    /// Idempotency key for deduplication.
    pub idempotency_key: Option<String>,
    /// Maximum cost in micro-USD.
    pub max_cost_usd: Option<u64>,
}

/// Compute response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResponse {
    /// Job identifier.
    pub job_id: String,
    /// Output data (schema depends on kind).
    pub output: serde_json::Value,
    /// Token usage (if applicable).
    pub usage: Option<TokenUsage>,
    /// Actual cost in micro-USD (post-execution).
    pub cost_usd: u64,
    /// Latency in milliseconds.
    pub latency_ms: u64,
    /// Provider that handled the request.
    pub provider_id: String,
    /// Model actually used.
    pub model: String,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input token count.
    pub input_tokens: u64,
    /// Output token count.
    pub output_tokens: u64,
    /// Total token count.
    pub total_tokens: u64,
}

/// A streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeChunk {
    /// Job identifier.
    pub job_id: String,
    /// Delta content.
    pub delta: serde_json::Value,
    /// Finish reason (only on final chunk).
    pub finish_reason: Option<String>,
    /// Accumulated usage (updated each chunk).
    pub usage: Option<TokenUsage>,
}

/// Provider selection preference.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Prefer {
    /// Minimize cost.
    Cost,
    /// Minimize latency.
    Latency,
    /// Maximize quality/reliability.
    Quality,
    /// Balanced (default).
    #[default]
    Balanced,
}

/// Policy for compute operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputePolicy {
    /// Allowed provider IDs (empty = all).
    #[serde(default)]
    pub allowed_providers: Vec<String>,
    /// Allowed models (empty = all).
    #[serde(default)]
    pub allowed_models: Vec<String>,
    /// Blocked models (takes precedence over allowed).
    #[serde(default)]
    pub blocked_models: Vec<String>,
    /// Maximum cost per tick in micro-USD.
    pub max_cost_usd_per_tick: Option<u64>,
    /// Maximum cost per day in micro-USD.
    pub max_cost_usd_per_day: Option<u64>,
    /// Default max_cost_usd if request doesn't specify.
    pub default_max_cost_usd: Option<u64>,
    /// Require requests to specify max_cost_usd.
    #[serde(default)]
    pub require_max_cost: bool,
    /// Default timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub default_timeout_ms: u64,
    /// Selection preference.
    #[serde(default)]
    pub prefer: Prefer,
    /// Fallback provider (if primary fails).
    pub fallback_provider: Option<String>,
    /// Require idempotency keys for all requests.
    #[serde(default)]
    pub require_idempotency: bool,
}

impl Default for ComputePolicy {
    fn default() -> Self {
        Self {
            allowed_providers: Vec::new(),
            allowed_models: Vec::new(),
            blocked_models: Vec::new(),
            max_cost_usd_per_tick: None,
            max_cost_usd_per_day: None,
            default_max_cost_usd: None,
            require_max_cost: false,
            default_timeout_ms: default_timeout_ms(),
            prefer: Prefer::Balanced,
            fallback_provider: None,
            require_idempotency: false,
        }
    }
}

fn default_timeout_ms() -> u64 {
    120_000
}

/// Information about a compute provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Provider identifier.
    pub id: String,
    /// Provider display name.
    pub name: String,
    /// Available models.
    pub models: Vec<ModelInfo>,
    /// Supported compute kinds.
    pub capabilities: Vec<ComputeKind>,
    /// Pricing metadata, if known.
    pub pricing: Option<ProviderPricing>,
    /// Latency metadata.
    pub latency: ProviderLatency,
    /// Region label, if known.
    pub region: Option<String>,
    /// Availability status.
    pub status: ProviderStatus,
}

/// Latency metrics for provider selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderLatency {
    /// Time to first token in milliseconds.
    pub ttft_ms: u64,
    /// Tokens per second throughput.
    pub tokens_per_sec: Option<u64>,
    /// Whether this is measured (true) or estimated (false).
    pub measured: bool,
}

/// Model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier.
    pub id: String,
    /// Model display name.
    pub name: String,
    /// Context length in tokens.
    pub context_length: Option<u32>,
    /// Supported compute kinds.
    pub capabilities: Vec<ComputeKind>,
    /// Pricing override, if known.
    pub pricing: Option<ModelPricing>,
}

/// Provider pricing defaults (micro-USD per 1k tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricing {
    /// Input cost per 1k tokens (micro-USD).
    pub input_per_1k_microusd: u64,
    /// Output cost per 1k tokens (micro-USD).
    pub output_per_1k_microusd: u64,
}

/// Per-model pricing overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Input cost per 1k tokens (micro-USD).
    pub input_per_1k_microusd: u64,
    /// Output cost per 1k tokens (micro-USD).
    pub output_per_1k_microusd: u64,
}

/// Provider availability status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderStatus {
    /// Provider is available.
    Available,
    /// Provider is available but degraded.
    Degraded {
        /// Reason for degraded status.
        reason: String,
    },
    /// Provider is unavailable.
    Unavailable {
        /// Reason for unavailable status.
        reason: String,
    },
}

/// Compute errors.
#[derive(Debug, thiserror::Error)]
pub enum ComputeError {
    /// Request parsing or validation failed.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// The compute kind is unsupported by the provider.
    #[error("unsupported compute kind: {0}")]
    UnsupportedKind(String),
    /// No providers match the request or policy.
    #[error("no provider available for model {model}: {reason}")]
    NoProviderAvailable {
        /// Requested model id.
        model: String,
        /// Selection failure reason.
        reason: String,
    },
    /// Provider returned an error.
    #[error("provider error: {0}")]
    ProviderError(String),
    /// Job id was not found.
    #[error("job not found")]
    JobNotFound,
    /// Budget limits were exceeded.
    #[error("budget exceeded")]
    BudgetExceeded,
    /// Idempotency key required but missing.
    #[error("idempotency key required")]
    IdempotencyRequired,
    /// max_cost_usd required but missing.
    #[error("max_cost_usd required")]
    MaxCostRequired,
    /// Job result is not ready.
    #[error("job not ready")]
    NotReady,
    /// Idempotency journal error.
    #[error("journal error: {0}")]
    Journal(String),
}

impl From<BudgetError> for ComputeError {
    fn from(err: BudgetError) -> Self {
        match err {
            BudgetError::Exceeded => ComputeError::BudgetExceeded,
            BudgetError::ActualExceedsReservation => {
                ComputeError::ProviderError("actual cost exceeded reservation".to_string())
            }
        }
    }
}

impl From<JournalError> for ComputeError {
    fn from(err: JournalError) -> Self {
        ComputeError::Journal(err.to_string())
    }
}

impl From<BackendError> for ComputeError {
    fn from(err: BackendError) -> Self {
        ComputeError::ProviderError(err.to_string())
    }
}

/// Job state for tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobState {
    /// Job submitted, waiting for execution.
    Pending {
        /// Submission timestamp.
        submitted_at: Timestamp,
    },
    /// Job is executing.
    Running {
        /// Start timestamp.
        started_at: Timestamp,
    },
    /// Job is streaming (chunks available via poll_stream).
    Streaming {
        /// Start timestamp.
        started_at: Timestamp,
        /// Number of chunks emitted.
        chunks_emitted: usize,
    },
    /// Job completed successfully.
    Complete(ComputeResponse),
    /// Job failed.
    Failed {
        /// Error message.
        error: String,
        /// Failure timestamp.
        at: Timestamp,
    },
}

/// Compute provider trait (sync for FileService compatibility).
pub trait ComputeProvider: Send + Sync {
    /// Provider identifier.
    fn id(&self) -> &str;
    /// Provider info (models, pricing, latency, etc.).
    fn info(&self) -> ProviderInfo;
    /// Check if provider is available.
    fn is_available(&self) -> bool;
    /// Check if provider supports a model.
    fn supports_model(&self, model: &str) -> bool;
    /// Submit a compute request. ALWAYS returns job_id immediately.
    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError>;
    /// Get current state of a job by ID.
    fn get_job(&self, job_id: &str) -> Option<JobState>;
    /// Poll streaming job for next chunk (internal use by ComputeFs).
    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError>;
    /// Cancel a running job.
    fn cancel(&self, job_id: &str) -> Result<(), ComputeError>;
}

/// Routes compute requests to appropriate providers.
#[derive(Default)]
pub struct ComputeRouter {
    providers: Vec<Arc<dyn ComputeProvider>>,
}

impl ComputeRouter {
    /// Create a new router.
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Arc<dyn ComputeProvider>) {
        self.providers.push(provider);
    }

    /// List providers.
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        self.providers.iter().map(|p| p.info()).collect()
    }

    /// Get provider by id.
    pub fn provider_by_id(&self, id: &str) -> Option<Arc<dyn ComputeProvider>> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .cloned()
    }

    /// Select best provider for request based on policy.
    pub fn select(
        &self,
        request: &ComputeRequest,
        policy: &ComputePolicy,
    ) -> Result<Arc<dyn ComputeProvider>, ComputeError> {
        let candidates: Vec<_> = self
            .providers
            .iter()
            .filter(|p| p.is_available())
            .filter(|p| p.supports_model(&request.model))
            .filter(|p| {
                policy.allowed_providers.is_empty()
                    || policy.allowed_providers.contains(&p.id().to_string())
            })
            .filter(|_| {
                policy.allowed_models.is_empty()
                    || policy.allowed_models.contains(&request.model)
            })
            .filter(|_| !policy.blocked_models.contains(&request.model))
            .cloned()
            .collect();

        if candidates.is_empty() {
            return Err(ComputeError::NoProviderAvailable {
                model: request.model.clone(),
                reason: "no provider matches policy filters".to_string(),
            });
        }

        let selected = match policy.prefer {
            Prefer::Cost => candidates
                .into_iter()
                .min_by_key(|p| self.estimate_provider_cost_usd(p, request)),
            Prefer::Latency => candidates
                .into_iter()
                .min_by_key(|p| p.info().latency.ttft_ms),
            Prefer::Quality => candidates.into_iter().max_by_key(|p| {
                let info = p.info();
                (info.latency.measured, info.latency.tokens_per_sec.unwrap_or(0))
            }),
            Prefer::Balanced => candidates.into_iter().min_by_key(|p| {
                let cost = self.estimate_provider_cost_usd(p, request);
                let latency = p.info().latency.ttft_ms;
                cost.saturating_mul(latency)
            }),
        };

        selected.ok_or_else(|| ComputeError::NoProviderAvailable {
            model: request.model.clone(),
            reason: "selection failed".to_string(),
        })
    }

    fn estimate_provider_cost_usd(
        &self,
        provider: &Arc<dyn ComputeProvider>,
        request: &ComputeRequest,
    ) -> u64 {
        if let Some(max_usd) = request.max_cost_usd {
            return max_usd;
        }

        let info = provider.info();
        let pricing = match info.pricing {
            Some(p) => p,
            None => return 0,
        };

        pricing.input_per_1k_microusd + pricing.output_per_1k_microusd
    }
}

/// Job record tracked by ComputeFs for budgeting.
#[derive(Clone)]
struct JobRecord {
    provider_id: String,
    reservation: BudgetReservation,
    reconciled: bool,
}

/// Compute capability as a filesystem.
pub struct ComputeFs {
    agent_id: AgentId,
    router: Arc<RwLock<ComputeRouter>>,
    policy: Arc<RwLock<ComputePolicy>>,
    budget: Arc<Mutex<BudgetTracker>>,
    journal: Arc<dyn IdempotencyJournal>,
    jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
}

impl ComputeFs {
    /// Create a new compute filesystem.
    pub fn new(
        agent_id: AgentId,
        router: ComputeRouter,
        policy: ComputePolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
    ) -> Self {
        Self {
            agent_id,
            router: Arc::new(RwLock::new(router)),
            policy: Arc::new(RwLock::new(policy)),
            budget: Arc::new(Mutex::new(BudgetTracker::new(budget_policy))),
            journal,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn job_provider(&self, job_id: &str) -> FsResult<(Arc<dyn ComputeProvider>, String)> {
        let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
        let record = jobs.get(job_id).ok_or(FsError::NotFound)?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .provider_by_id(&record.provider_id)
            .ok_or(FsError::NotFound)?;
        Ok((provider, record.provider_id.clone()))
    }

    fn reconcile_job(&self, job_id: &str, state: &JobState) -> FsResult<()> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let record = match jobs.get_mut(job_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }

        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            JobState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
            }
            JobState::Failed { .. } => {
                tracker.release(record.reservation);
            }
            _ => return Ok(()),
        }

        record.reconciled = true;
        Ok(())
    }

    fn usage_json(&self) -> FsResult<Vec<u8>> {
        let tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        let policy = tracker.policy().clone();
        let state = tracker.state().clone();
        let json = serde_json::json!({
            "tick": {
                "reserved_usd": state.reserved_tick_usd,
                "spent_usd": state.spent_tick_usd,
                "limit_usd": policy.per_tick_usd,
                "remaining_usd": state.remaining_tick(&policy),
            },
            "day": {
                "reserved_usd": state.reserved_day_usd,
                "spent_usd": state.spent_day_usd,
                "limit_usd": policy.per_day_usd,
                "remaining_usd": state.remaining_day(&policy),
            }
        });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }
}

impl FileService for ComputeFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        match parts.as_slice() {
            ["new"] if flags.write => Ok(Box::new(ComputeNewHandle::new(
                self.agent_id.clone(),
                self.router.clone(),
                self.policy.clone(),
                self.budget.clone(),
                self.jobs.clone(),
                self.journal.clone(),
            ))),
            ["policy"] => {
                if flags.write {
                    Ok(Box::new(PolicyWriteHandle::new(self.policy.clone())))
                } else {
                    let policy = self.policy.read().unwrap_or_else(|e| e.into_inner());
                    let json = serde_json::to_vec_pretty(&*policy)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    Ok(Box::new(BytesHandle::new(json)))
                }
            }
            ["usage"] => Ok(Box::new(BytesHandle::new(self.usage_json()?))),
            ["providers"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let providers = router.list_providers();
                let json = serde_json::to_vec_pretty(&providers)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "info"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::to_vec_pretty(&info)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "models"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::to_vec_pretty(&info.models)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["jobs", job_id, "status"] => {
                let (provider, _) = self.job_provider(job_id)?;
                let state = provider.get_job(job_id).ok_or(FsError::NotFound)?;
                self.reconcile_job(job_id, &state)?;
                let status = match state {
                    JobState::Pending { .. } => "pending",
                    JobState::Running { .. } => "running",
                    JobState::Streaming { .. } => "streaming",
                    JobState::Complete(_) => "complete",
                    JobState::Failed { .. } => "failed",
                };
                let json = format!(r#"{{"status":"{}"}}"#, status);
                Ok(Box::new(BytesHandle::new(json.into_bytes())))
            }
            ["jobs", job_id, "result"] => {
                let (provider, _) = self.job_provider(job_id)?;
                let state = provider.get_job(job_id).ok_or(FsError::NotFound)?;
                self.reconcile_job(job_id, &state)?;
                match state {
                    JobState::Complete(response) => {
                        let json = serde_json::to_vec_pretty(&response)
                            .map_err(|err| FsError::Other(err.to_string()))?;
                        Ok(Box::new(BytesHandle::new(json)))
                    }
                    JobState::Failed { error, .. } => Err(FsError::Other(error)),
                    _ => Err(FsError::Other("not ready".to_string())),
                }
            }
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        let path = path.trim_matches('/');
        match path {
            "" => Ok(vec![
                DirEntry::dir("providers"),
                DirEntry::file("new", 0),
                DirEntry::file("policy", 0),
                DirEntry::file("usage", 0),
                DirEntry::dir("jobs"),
            ]),
            "providers" => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                Ok(router
                    .list_providers()
                    .iter()
                    .map(|p| DirEntry::dir(&p.id))
                    .collect())
            }
            "jobs" => {
                let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
                Ok(jobs.keys().map(|id| DirEntry::dir(id)).collect())
            }
            _ => Ok(vec![]),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        match parts.as_slice() {
            [] => Ok(Stat::dir()),
            ["providers"] | ["jobs"] => Ok(Stat::dir()),
            ["new"] | ["policy"] => Ok(Stat {
                size: 0,
                is_dir: false,
                created: None,
                modified: None,
                permissions: Permissions::read_write(),
            }),
            ["usage"] => Ok(Stat::file(0)),
            ["providers", id] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["providers", id, "info"] | ["providers", id, "models"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["jobs", job_id] => {
                let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
                if jobs.contains_key(*job_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["jobs", job_id, "status"]
            | ["jobs", job_id, "result"]
            | ["jobs", job_id, "stream"] => {
                let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
                if jobs.contains_key(*job_id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            _ => Err(FsError::NotFound),
        }
    }

    fn mkdir(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn remove(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn rename(&self, _from: &str, _to: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn watch(&self, path: &str) -> FsResult<Option<Box<dyn WatchHandle>>> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if let ["jobs", job_id, "stream"] = parts.as_slice() {
            let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
            let record = jobs.get(*job_id).ok_or(FsError::NotFound)?;
            return Ok(Some(Box::new(StreamWatchHandle::new(
                job_id.to_string(),
                record.provider_id.clone(),
                self.router.clone(),
                self.jobs.clone(),
                self.budget.clone(),
            ))));
        }
        Ok(None)
    }

    fn name(&self) -> &str {
        "compute"
    }
}

struct ComputeNewHandle {
    agent_id: AgentId,
    router: Arc<RwLock<ComputeRouter>>,
    policy: Arc<RwLock<ComputePolicy>>,
    budget: Arc<Mutex<BudgetTracker>>,
    jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
    journal: Arc<dyn IdempotencyJournal>,
    request_buf: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ComputeNewHandle {
    fn new(
        agent_id: AgentId,
        router: Arc<RwLock<ComputeRouter>>,
        policy: Arc<RwLock<ComputePolicy>>,
        budget: Arc<Mutex<BudgetTracker>>,
        jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
        journal: Arc<dyn IdempotencyJournal>,
    ) -> Self {
        Self {
            agent_id,
            router,
            policy,
            budget,
            jobs,
            journal,
            request_buf: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn submit_request(&mut self) -> FsResult<()> {
        let mut request: ComputeRequest = serde_json::from_slice(&self.request_buf)
            .map_err(|err| FsError::Other(err.to_string()))?;

        let policy = self
            .policy
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if policy.require_idempotency && request.idempotency_key.is_none() {
            return Err(FsError::Other(ComputeError::IdempotencyRequired.to_string()));
        }

        if request.timeout_ms.is_none() {
            request.timeout_ms = Some(policy.default_timeout_ms);
        }

        let max_cost_usd = match (
            request.max_cost_usd,
            policy.default_max_cost_usd,
            policy.require_max_cost,
        ) {
            (Some(cost), _, _) => cost,
            (None, Some(default_cost), _) => {
                request.max_cost_usd = Some(default_cost);
                default_cost
            }
            (None, None, true) => {
                return Err(FsError::Other(ComputeError::MaxCostRequired.to_string()));
            }
            (None, None, false) => {
                let tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                let budget_policy = tracker.policy();
                if budget_policy.per_tick_usd > 0 {
                    budget_policy.per_tick_usd
                } else {
                    budget_policy.per_day_usd
                }
            }
        };

        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .select(&request, &policy)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let provider_id = provider.id().to_string();

        let scoped_key = request.idempotency_key.as_ref().map(|key| {
            format!("{}:{}:{}", self.agent_id.as_str(), provider_id, key)
        });

        if let Some(key) = scoped_key.as_ref() {
            if let Some(cached) = self
                .journal
                .get(key)
                .map_err(|err| FsError::Other(err.to_string()))?
            {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&cached) {
                    if let Some(job_id) = value.get("job_id").and_then(|v| v.as_str()) {
                        self.jobs
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .entry(job_id.to_string())
                            .or_insert(JobRecord {
                                provider_id: provider_id.clone(),
                                reservation: BudgetReservation { amount_usd: 0 },
                                reconciled: true,
                            });
                    }
                }
                self.response = Some(cached);
                return Ok(());
            }
        }

        let reservation = {
            let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
            let reservation = tracker.reserve(max_cost_usd).map_err(|_| FsError::BudgetExceeded)?;
            let state = tracker.state().clone();
            if let Some(limit) = policy.max_cost_usd_per_tick {
                if state.reserved_tick_usd + state.spent_tick_usd > limit {
                    tracker.release(reservation);
                    return Err(FsError::BudgetExceeded);
                }
            }
            if let Some(limit) = policy.max_cost_usd_per_day {
                if state.reserved_day_usd + state.spent_day_usd > limit {
                    tracker.release(reservation);
                    return Err(FsError::BudgetExceeded);
                }
            }
            reservation
        };

        let job_id = match provider.submit(request.clone()) {
            Ok(job_id) => job_id,
            Err(err) => {
                let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                tracker.release(reservation);
                return Err(FsError::Other(err.to_string()));
            }
        };

        self.jobs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                job_id.clone(),
                JobRecord {
                    provider_id,
                    reservation,
                    reconciled: false,
                },
            );

        let response_json = serde_json::json!({
            "job_id": job_id,
            "status": "pending",
            "status_path": format!("/compute/jobs/{}/status", job_id),
            "stream_path": format!("/compute/jobs/{}/stream", job_id),
            "result_path": format!("/compute/jobs/{}/result", job_id),
        });
        let response_bytes = serde_json::to_vec(&response_json)
            .map_err(|err| FsError::Other(err.to_string()))?;

        if let Some(key) = scoped_key.as_ref() {
            self.journal
                .put_with_ttl(key, &response_bytes, IDEMPOTENCY_TTL)
                .map_err(|err| FsError::Other(err.to_string()))?;
        }

        self.response = Some(response_bytes);
        Ok(())
    }
}

impl FileHandle for ComputeNewHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.response.is_none() {
            self.submit_request()?;
        }
        let response = self.response.as_ref().unwrap();
        if self.position >= response.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), response.len() - self.position);
        buf[..len].copy_from_slice(&response[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.request_buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        if self.response.is_none() {
            return Err(FsError::InvalidPath);
        }
        let response = self.response.as_ref().unwrap();
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => response.len() as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if new_pos < 0 {
            return Err(FsError::InvalidPath);
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.response.is_none() && !self.request_buf.is_empty() {
            self.submit_request()?;
        }
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct PolicyWriteHandle {
    policy: Arc<RwLock<ComputePolicy>>,
    buffer: Vec<u8>,
}

impl PolicyWriteHandle {
    fn new(policy: Arc<RwLock<ComputePolicy>>) -> Self {
        Self {
            policy,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for PolicyWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let policy: ComputePolicy = serde_json::from_slice(&self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let mut guard = self.policy.write().unwrap_or_else(|e| e.into_inner());
        *guard = policy;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct StreamWatchHandle {
    job_id: String,
    provider_id: String,
    router: Arc<RwLock<ComputeRouter>>,
    jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
    budget: Arc<Mutex<BudgetTracker>>,
}

impl StreamWatchHandle {
    fn new(
        job_id: String,
        provider_id: String,
        router: Arc<RwLock<ComputeRouter>>,
        jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
        budget: Arc<Mutex<BudgetTracker>>,
    ) -> Self {
        Self {
            job_id,
            provider_id,
            router,
            jobs,
            budget,
        }
    }

    fn reconcile(&self, state: &JobState) -> FsResult<()> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let record = match jobs.get_mut(&self.job_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }
        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            JobState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
            }
            JobState::Failed { .. } => {
                tracker.release(record.reservation);
            }
            _ => return Ok(()),
        }
        record.reconciled = true;
        Ok(())
    }
}

impl WatchHandle for StreamWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            let provider = {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                router
                    .provider_by_id(&self.provider_id)
                    .ok_or(FsError::NotFound)?
            };
            match provider.poll_stream(&self.job_id) {
                Ok(Some(chunk)) => {
                    if let Some(state) = provider.get_job(&self.job_id) {
                        self.reconcile(&state)?;
                    }
                    let payload = serde_json::to_vec(&chunk)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    return Ok(Some(WatchEvent::Data(payload)));
                }
                Ok(None) => {
                    if let Some(state) = provider.get_job(&self.job_id) {
                        if matches!(state, JobState::Complete(_) | JobState::Failed { .. }) {
                            self.reconcile(&state)?;
                            return Ok(None);
                        }
                    }
                }
                Err(err) => return Err(FsError::Other(err.to_string())),
            }

            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    return Ok(None);
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

/// Local provider backed by the compute registry.
pub struct LocalProvider {
    registry: Arc<BackendRegistry>,
    executor: Executor,
    jobs: Arc<RwLock<HashMap<String, LocalJobState>>>,
}

struct LocalJobState {
    status: JobState,
    stream_rx: Option<Mutex<mpsc::Receiver<ComputeChunk>>>,
}

impl LocalProvider {
    /// Detect available local backends.
    pub fn detect() -> Result<Self, ComputeError> {
        let executor = Executor::new()?;
        let registry = executor
            .block_on(async { BackendRegistry::detect().await });
        Ok(Self {
            registry: Arc::new(registry),
            executor,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn backend_for_model(
        registry: &BackendRegistry,
        model: &str,
    ) -> Option<Arc<TokioRwLock<dyn InferenceBackend>>> {
        let models = registry.list_all_models().await;
        let backend_id = models
            .into_iter()
            .find(|(_, info)| info.id == model)
            .map(|(id, _)| id);
        backend_id.and_then(|id| registry.get(&id))
    }

    fn map_usage(usage: Option<BackendUsageInfo>) -> Option<TokenUsage> {
        usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens as u64,
            output_tokens: u.completion_tokens as u64,
            total_tokens: u.total_tokens as u64,
        })
    }

    fn completion_request(request: &ComputeRequest) -> Result<CompletionRequest, ComputeError> {
        let prompt = match request.kind {
            ComputeKind::Complete => parse_prompt(&request.input)
                .ok_or_else(|| ComputeError::InvalidRequest("missing prompt".to_string()))?,
            ComputeKind::Chat => parse_messages(&request.input)
                .ok_or_else(|| ComputeError::InvalidRequest("missing messages".to_string()))?,
            _ => {
                return Err(ComputeError::UnsupportedKind(format!(
                    "{:?}",
                    request.kind
                )))
            }
        };

        let mut completion = CompletionRequest::new(request.model.clone(), prompt);
        completion.stream = request.stream;

        if let Some(obj) = request.input.as_object() {
            if let Some(max_tokens) = obj.get("max_tokens").and_then(|v| v.as_u64()) {
                completion.max_tokens = Some(max_tokens as usize);
            }
            if let Some(temp) = obj.get("temperature").and_then(|v| v.as_f64()) {
                completion.temperature = Some(temp as f32);
            }
            if let Some(top_p) = obj.get("top_p").and_then(|v| v.as_f64()) {
                completion.top_p = Some(top_p as f32);
            }
            if let Some(stop) = obj.get("stop").and_then(|v| v.as_array()) {
                let stops: Vec<String> = stop
                    .iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect();
                if !stops.is_empty() {
                    completion.stop = Some(stops);
                }
            }
        }

        Ok(completion)
    }

    fn job_response(
        job_id: &str,
        provider_id: &str,
        response: CompletionResponse,
        latency_ms: u64,
    ) -> ComputeResponse {
        let output = serde_json::json!({
            "text": response.text,
            "finish_reason": response.finish_reason,
        });
        ComputeResponse {
            job_id: job_id.to_string(),
            output,
            usage: Self::map_usage(response.usage),
            cost_usd: 0,
            latency_ms,
            provider_id: provider_id.to_string(),
            model: response.model,
        }
    }
}

impl ComputeProvider for LocalProvider {
    fn id(&self) -> &str {
        "local"
    }

    fn info(&self) -> ProviderInfo {
        let models = self
            .executor
            .block_on(async { self.registry.list_all_models().await });
        let has_backends = self.registry.has_backends();
        ProviderInfo {
            id: "local".to_string(),
            name: "Local".to_string(),
            models: models
                .into_iter()
                .map(|(_, model)| ModelInfo {
                    id: model.id,
                    name: model.name,
                    context_length: Some(model.context_length as u32),
                    capabilities: vec![ComputeKind::Chat, ComputeKind::Complete],
                    pricing: None,
                })
                .collect(),
            capabilities: vec![
                ComputeKind::Chat,
                ComputeKind::Complete,
                ComputeKind::Embeddings,
            ],
            pricing: None,
            latency: ProviderLatency {
                ttft_ms: 0,
                tokens_per_sec: None,
                measured: false,
            },
            region: Some("local".to_string()),
            status: if has_backends {
                ProviderStatus::Available
            } else {
                ProviderStatus::Unavailable {
                    reason: "no local backend detected".to_string(),
                }
            },
        }
    }

    fn is_available(&self) -> bool {
        self.registry.has_backends()
    }

    fn supports_model(&self, model: &str) -> bool {
        self.executor.block_on(async {
            let models = self.registry.list_all_models().await;
            models.iter().any(|(_, info)| info.id == model)
        })
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let stream = request.stream;
        let provider_id = self.id().to_string();
        let start = Instant::now();
        let jobs = self.jobs.clone();
        let registry = self.registry.clone();
        let executor = self.executor.clone();
        let request_clone = request.clone();
        let job_id_clone = job_id.clone();

        let (stream_tx, stream_rx) = if stream {
            let (tx, rx) = mpsc::channel(64);
            (Some(tx), Some(Mutex::new(rx)))
        } else {
            (None, None)
        };

        self.jobs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                job_id.clone(),
                LocalJobState {
                    status: JobState::Pending {
                        submitted_at: Timestamp::now(),
                    },
                    stream_rx,
                },
            );

        executor.spawn(async move {
            let backend = LocalProvider::backend_for_model(&registry, &request_clone.model).await;
            let backend = match backend {
                Some(backend) => backend,
                None => {
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Failed {
                            error: "model not available".to_string(),
                            at: Timestamp::now(),
                        };
                    }
                    return;
                }
            };

            let completion_request = match LocalProvider::completion_request(&request_clone) {
                Ok(req) => req,
                Err(err) => {
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        };
                    }
                    return;
                }
            };

            if stream {
                {
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Streaming {
                            started_at: Timestamp::now(),
                            chunks_emitted: 0,
                        };
                    }
                }
                let rx = {
                    let backend = backend.read().await;
                    match backend.complete_stream(completion_request).await {
                        Ok(rx) => rx,
                        Err(err) => {
                            let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                            if let Some(job) = jobs.get_mut(&job_id_clone) {
                                job.status = JobState::Failed {
                                    error: err.to_string(),
                                    at: Timestamp::now(),
                                };
                            }
                            return;
                        }
                    }
                };

                let mut output_text = String::new();
                let mut finish_reason: Option<String> = None;
                let mut stream = rx;
                while let Some(chunk) = stream.recv().await {
                    match chunk {
                        Ok(chunk) => {
                            output_text.push_str(&chunk.delta);
                            finish_reason = chunk.finish_reason.clone();
                            if let Some(tx) = stream_tx.as_ref() {
                                let compute_chunk = ComputeChunk {
                                    job_id: job_id_clone.clone(),
                                    delta: serde_json::json!({ "text": chunk.delta }),
                                    finish_reason: chunk.finish_reason.clone(),
                                    usage: None,
                                };
                                let _ = tx.send(compute_chunk).await;
                            }
                            let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                            if let Some(job) = jobs.get_mut(&job_id_clone) {
                                if let JobState::Streaming { started_at, chunks_emitted } =
                                    job.status.clone()
                                {
                                    job.status = JobState::Streaming {
                                        started_at,
                                        chunks_emitted: chunks_emitted.saturating_add(1),
                                    };
                                }
                            }
                        }
                        Err(err) => {
                            let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                            if let Some(job) = jobs.get_mut(&job_id_clone) {
                                job.status = JobState::Failed {
                                    error: err.to_string(),
                                    at: Timestamp::now(),
                                };
                            }
                            return;
                        }
                    }
                }

                let response = ComputeResponse {
                    job_id: job_id_clone.clone(),
                    output: serde_json::json!({
                        "text": output_text,
                        "finish_reason": finish_reason,
                    }),
                    usage: None,
                    cost_usd: 0,
                    latency_ms: start.elapsed().as_millis() as u64,
                    provider_id: provider_id.clone(),
                    model: request_clone.model.clone(),
                };
                let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                if let Some(job) = jobs.get_mut(&job_id_clone) {
                    job.status = JobState::Complete(response);
                }
                return;
            }

            let completion = {
                let backend = backend.read().await;
                backend.complete(completion_request).await
            };
            match completion {
                Ok(response) => {
                    let response = LocalProvider::job_response(
                        &job_id_clone,
                        &provider_id,
                        response,
                        start.elapsed().as_millis() as u64,
                    );
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Complete(response);
                    }
                }
                Err(err) => {
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        };
                    }
                }
            }
        });

        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        self.jobs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(job_id)
            .map(|job| job.status.clone())
    }

    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let job = jobs.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
        let rx = match job.stream_rx.as_mut() {
            Some(rx) => rx,
            None => return Ok(None),
        };
        let mut rx = rx.lock().unwrap_or_else(|e| e.into_inner());
        match rx.try_recv() {
            Ok(chunk) => Ok(Some(chunk)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let job = jobs.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
        job.status = JobState::Failed {
            error: "cancelled".to_string(),
            at: Timestamp::now(),
        };
        Ok(())
    }
}

#[derive(Clone)]
struct Executor {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl Executor {
    fn new() -> Result<Self, ComputeError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| ComputeError::ProviderError(err.to_string()))?;
        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.runtime.block_on(fut)
    }

    fn spawn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(fut);
    }
}

fn parse_prompt(input: &serde_json::Value) -> Option<String> {
    match input {
        serde_json::Value::String(prompt) => Some(prompt.clone()),
        serde_json::Value::Object(map) => map.get("prompt").and_then(|v| v.as_str()).map(|s| s.to_string()),
        _ => None,
    }
}

fn parse_messages(input: &serde_json::Value) -> Option<String> {
    let messages = input.get("messages")?.as_array()?;
    let mut prompt = String::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("user");
        let content = message
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        prompt.push_str(role);
        prompt.push_str(": ");
        prompt.push_str(content);
        prompt.push('\n');
    }
    Some(prompt)
}
