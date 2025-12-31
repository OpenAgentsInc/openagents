//! Compute filesystem service and providers.

use crate::budget::{BudgetError, BudgetPolicy, BudgetReservation, BudgetTracker};
#[cfg(not(target_arch = "wasm32"))]
use crate::dvm::{msats_to_sats, parse_feedback_event, DvmFeedbackStatus, DvmTransport, RelayPoolTransport};
#[cfg(not(target_arch = "wasm32"))]
use crate::fx::{FxRateCache, FxRateProvider, FxSource};
use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Permissions,
    SeekFrom, Stat, WatchEvent, WatchHandle,
};
use crate::idempotency::{IdempotencyJournal, JournalError};
use crate::identity::SigningService;
use crate::types::{AgentId, Timestamp};
#[cfg(not(target_arch = "wasm32"))]
use crate::wallet::{WalletFxProvider, WalletService};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
use crate::wasm_http;
#[cfg(not(target_arch = "wasm32"))]
use nostr::{
    create_deletion_tags, get_event_hash, get_result_kind, HandlerInfo, HandlerType, JobInput,
    JobRequest, JobResult, JobStatus, UnsignedEvent, DELETION_REQUEST_KIND, KIND_HANDLER_INFO,
    KIND_JOB_FEEDBACK, KIND_JOB_IMAGE_GENERATION, KIND_JOB_SPEECH_TO_TEXT, KIND_JOB_TEXT_GENERATION,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::{mpsc, RwLock as TokioRwLock};
#[cfg(all(
    target_arch = "wasm32",
    any(feature = "cloudflare", feature = "browser")
))]
use wasm_bindgen_futures::spawn_local;
#[cfg(feature = "cloudflare")]
use worker::Ai;

#[cfg(not(target_arch = "wasm32"))]
use ::compute as compute_provider;
#[cfg(not(target_arch = "wasm32"))]
use compute_provider::backends::{
    BackendRegistry, CompletionRequest, CompletionResponse, UsageInfo as BackendUsageInfo,
};
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

/// Source for API tokens used by remote compute providers.
pub trait ApiTokenProvider: Send + Sync {
    /// Return the current API token, if available.
    fn api_token(&self) -> Option<String>;
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
        let info = provider.info();
        if let Some(pricing) = info
            .models
            .iter()
            .find(|model| model.id == request.model)
            .and_then(|model| model.pricing.clone())
        {
            return pricing.input_per_1k_microusd.saturating_add(pricing.output_per_1k_microusd);
        }
        if let Some(pricing) = info.pricing {
            return pricing
                .input_per_1k_microusd
                .saturating_add(pricing.output_per_1k_microusd);
        }

        request.max_cost_usd.unwrap_or(0)
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
    emitted_chunk: bool,
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
            emitted_chunk: false,
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
                    self.emitted_chunk = true;
                    if let Some(state) = provider.get_job(&self.job_id) {
                        self.reconcile(&state)?;
                    }
                    let payload = serde_json::to_vec(&chunk)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    return Ok(Some(WatchEvent::Data(payload)));
                }
                Ok(None) => {
                    if let Some(state) = provider.get_job(&self.job_id) {
                        match &state {
                            JobState::Complete(response) => {
                                self.reconcile(&state)?;
                                if !self.emitted_chunk {
                                    self.emitted_chunk = true;
                                    let chunk = ComputeChunk {
                                        job_id: self.job_id.clone(),
                                        delta: response.output.clone(),
                                        finish_reason: Some("complete".to_string()),
                                        usage: response.usage.clone(),
                                    };
                                    let payload = serde_json::to_vec(&chunk)
                                        .map_err(|err| FsError::Other(err.to_string()))?;
                                    return Ok(Some(WatchEvent::Data(payload)));
                                }
                                return Ok(None);
                            }
                            JobState::Failed { .. } => {
                                self.reconcile(&state)?;
                                return Ok(None);
                            }
                            _ => {}
                        }
                    }
                }
                Err(err) => return Err(FsError::Other(err.to_string())),
            }

            if !wait_for_stream(deadline)? {
                return Ok(None);
            }
        }
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

fn wait_for_stream(deadline: Option<Instant>) -> FsResult<bool> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = deadline;
        return Ok(false);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                return Ok(false);
            }
        }
        std::thread::sleep(Duration::from_millis(25));
        Ok(true)
    }
}

/// Local provider backed by the compute registry.
#[cfg(not(target_arch = "wasm32"))]
pub struct LocalProvider {
    registry: Arc<BackendRegistry>,
    executor: Executor,
    jobs: Arc<RwLock<HashMap<String, LocalJobState>>>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LocalJobState {
    status: JobState,
    stream_rx: Option<Mutex<mpsc::Receiver<ComputeChunk>>>,
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

/// Cloudflare Workers AI provider.
#[cfg(feature = "cloudflare")]
pub struct CloudflareProvider {
    ai: Arc<Ai>,
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    last_latency_ms: Arc<Mutex<Option<u64>>>,
}

#[cfg(feature = "cloudflare")]
impl CloudflareProvider {
    /// Create a provider from a Workers AI binding.
    pub fn new(ai: Ai) -> Self {
        Self {
            ai: Arc::new(ai),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            last_latency_ms: Arc::new(Mutex::new(None)),
        }
    }
}

#[cfg(feature = "cloudflare")]
impl ComputeProvider for CloudflareProvider {
    fn id(&self) -> &str {
        "cloudflare"
    }

    fn info(&self) -> ProviderInfo {
        let latency = {
            let guard = self.last_latency_ms.lock().unwrap_or_else(|e| e.into_inner());
            ProviderLatency {
                ttft_ms: guard.unwrap_or(0),
                tokens_per_sec: None,
                measured: guard.is_some(),
            }
        };
        ProviderInfo {
            id: "cloudflare".to_string(),
            name: "Cloudflare Workers AI".to_string(),
            models: Vec::new(),
            capabilities: Vec::new(),
            pricing: None,
            latency,
            region: None,
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        true
    }

    fn supports_model(&self, model: &str) -> bool {
        model.starts_with("@cf/")
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let job_id_clone = job_id.clone();
        let ai = Arc::clone(&self.ai);
        let jobs = Arc::clone(&self.jobs);
        let last_latency_ms = Arc::clone(&self.last_latency_ms);
        let request_clone = request.clone();

        self.jobs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                job_id.clone(),
                JobState::Pending {
                    submitted_at: Timestamp::now(),
                },
            );

        spawn_local(async move {
            {
                let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                if let Some(job) = jobs.get_mut(&job_id_clone) {
                    *job = JobState::Running {
                        started_at: Timestamp::now(),
                    };
                }
            }

            let start = Instant::now();
            let output: Result<serde_json::Value, worker::Error> =
                ai.run(&request_clone.model, request_clone.input.clone()).await;
            let latency_ms = start.elapsed().as_millis() as u64;

            {
                let mut guard = last_latency_ms.lock().unwrap_or_else(|e| e.into_inner());
                *guard = Some(latency_ms);
            }

            let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
            match output {
                Ok(output) => {
                    let cost_usd = request_clone
                        .max_cost_usd
                        .unwrap_or(0); // usage not available; treat reservation as spend
                    let response = ComputeResponse {
                        job_id: job_id_clone.clone(),
                        output,
                        usage: None,
                        cost_usd,
                        latency_ms,
                        provider_id: "cloudflare".to_string(),
                        model: request_clone.model.clone(),
                    };
                    jobs.insert(job_id_clone.clone(), JobState::Complete(response));
                }
                Err(err) => {
                    jobs.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        },
                    );
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
            .cloned()
    }

    fn poll_stream(&self, _job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        Ok(None)
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let job = jobs.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
        *job = JobState::Failed {
            error: "cancelled".to_string(),
            at: Timestamp::now(),
        };
        Ok(())
    }
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
#[derive(Default)]
struct RemoteJobState {
    remote_id: Option<String>,
    cursor: Option<String>,
    queue: VecDeque<ComputeChunk>,
    refreshing: bool,
    streaming: bool,
}

/// OpenAgents API-backed compute provider for browser targets.
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub struct OpenAgentsComputeProvider {
    base_url: String,
    provider_id: String,
    token_provider: Arc<dyn ApiTokenProvider>,
    info: Arc<RwLock<ProviderInfo>>,
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    remote: Arc<Mutex<HashMap<String, RemoteJobState>>>,
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
impl OpenAgentsComputeProvider {
    /// Create a new OpenAgents API compute provider.
    pub fn new(
        base_url: impl Into<String>,
        provider_id: impl Into<String>,
        token_provider: Arc<dyn ApiTokenProvider>,
    ) -> Self {
        let provider_id = provider_id.into();
        let info = ProviderInfo {
            id: provider_id.clone(),
            name: format!("OpenAgents ({})", provider_id),
            models: Vec::new(),
            capabilities: Vec::new(),
            pricing: None,
            latency: ProviderLatency {
                ttft_ms: 0,
                tokens_per_sec: None,
                measured: false,
            },
            region: Some("openagents".to_string()),
            status: ProviderStatus::Degraded {
                reason: "loading provider info".to_string(),
            },
        };
        let provider = Self {
            base_url: base_url.into(),
            provider_id,
            token_provider,
            info: Arc::new(RwLock::new(info)),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            remote: Arc::new(Mutex::new(HashMap::new())),
        };
        provider.spawn_info_refresh();
        provider
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn spawn_info_refresh(&self) {
        let info = Arc::clone(&self.info);
        let provider_id = self.provider_id.clone();
        let url = self.url(&format!("compute/providers/{}/info", provider_id));
        let token_provider = Arc::clone(&self.token_provider);
        spawn_local(async move {
            let token = token_provider.api_token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let updated = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    match serde_json::from_slice::<ProviderInfo>(&bytes) {
                        Ok(mut info) => {
                            if info.id.is_empty() {
                                info.id = provider_id.clone();
                            }
                            if info.name.is_empty() {
                                info.name = format!("OpenAgents ({})", provider_id);
                            }
                            info
                        }
                        Err(err) => ProviderInfo {
                            id: provider_id.clone(),
                            name: format!("OpenAgents ({})", provider_id),
                            models: Vec::new(),
                            capabilities: Vec::new(),
                            pricing: None,
                            latency: ProviderLatency {
                                ttft_ms: 0,
                                tokens_per_sec: None,
                                measured: false,
                            },
                            region: Some("openagents".to_string()),
                            status: ProviderStatus::Unavailable {
                                reason: format!("invalid provider info: {}", err),
                            },
                        },
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    ProviderInfo {
                        id: provider_id.clone(),
                        name: format!("OpenAgents ({})", provider_id),
                        models: Vec::new(),
                        capabilities: Vec::new(),
                        pricing: None,
                        latency: ProviderLatency {
                            ttft_ms: 0,
                            tokens_per_sec: None,
                            measured: false,
                        },
                        region: Some("openagents".to_string()),
                        status: ProviderStatus::Unavailable {
                            reason: format!("openagents api {}: {}", status, body),
                        },
                    }
                }
                Err(err) => ProviderInfo {
                    id: provider_id.clone(),
                    name: format!("OpenAgents ({})", provider_id),
                    models: Vec::new(),
                    capabilities: Vec::new(),
                    pricing: None,
                    latency: ProviderLatency {
                        ttft_ms: 0,
                        tokens_per_sec: None,
                        measured: false,
                    },
                    region: Some("openagents".to_string()),
                    status: ProviderStatus::Unavailable {
                        reason: err,
                    },
                },
            };
            let mut guard = info.write().unwrap_or_else(|e| e.into_inner());
            *guard = updated;
        });
    }

    fn spawn_refresh(&self, job_id: &str) {
        let (remote_id, url, token_provider, jobs, remote, job_id) = {
            let mut guard = self.remote.lock().unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(job_id) {
                Some(state) => state,
                None => return,
            };
            if state.refreshing {
                return;
            }
            let remote_id = match state.remote_id.clone() {
                Some(id) => id,
                None => return,
            };
            state.refreshing = true;
            let url = self.url(&format!("compute/jobs/{}", remote_id));
            (
                remote_id,
                url,
                Arc::clone(&self.token_provider),
                Arc::clone(&self.jobs),
                Arc::clone(&self.remote),
                job_id.to_string(),
            )
        };

        spawn_local(async move {
            let token = token_provider.api_token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let next_state = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    serde_json::from_slice::<JobState>(&bytes)
                        .map_err(|err| format!("invalid job state: {}", err))
                }
                Ok((404, _)) => Err("job not found".to_string()),
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    Err(format!("openagents api {}: {}", status, body))
                }
                Err(err) => Err(err),
            };

            match next_state {
                Ok(state) => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(job_id.clone(), state);
                }
                Err(err) => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id.clone(),
                        JobState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }

            let mut guard = remote.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&job_id) {
                state.refreshing = false;
                state.remote_id = Some(remote_id);
            }
        });
    }

    fn spawn_stream_poll(&self, job_id: &str) {
        let (url, token_provider, jobs, remote, job_id, cursor) = {
            let mut guard = self.remote.lock().unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(job_id) {
                Some(state) => state,
                None => return,
            };
            if state.streaming {
                return;
            }
            let remote_id = match state.remote_id.clone() {
                Some(id) => id,
                None => return,
            };
            state.streaming = true;
            let cursor = state.cursor.clone();
            let url = match cursor.as_ref() {
                Some(cursor) => self.url(&format!(
                    "compute/jobs/{}/stream?cursor={}",
                    remote_id, cursor
                )),
                None => self.url(&format!("compute/jobs/{}/stream", remote_id)),
            };
            (
                url,
                Arc::clone(&self.token_provider),
                Arc::clone(&self.jobs),
                Arc::clone(&self.remote),
                job_id.to_string(),
                cursor,
            )
        };

        spawn_local(async move {
            let token = token_provider.api_token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let mut next_chunk: Option<ComputeChunk> = None;
            let mut next_cursor = cursor.clone();
            let mut error: Option<String> = None;

            match response {
                Ok((status, bytes)) if status == 204 || bytes.is_empty() => {}
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    #[derive(Deserialize)]
                    struct StreamResponse {
                        chunk: Option<ComputeChunk>,
                        cursor: Option<String>,
                    }

                    if let Ok(payload) = serde_json::from_slice::<StreamResponse>(&bytes) {
                        next_chunk = payload.chunk;
                        next_cursor = payload.cursor.or(next_cursor);
                    } else if let Ok(chunk) = serde_json::from_slice::<ComputeChunk>(&bytes) {
                        next_chunk = Some(chunk);
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    error = Some(format!("openagents api {}: {}", status, body));
                }
                Err(err) => error = Some(err),
            }

            if let Some(err) = error {
                let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                guard.insert(
                    job_id.clone(),
                    JobState::Failed {
                        error: err,
                        at: Timestamp::now(),
                    },
                );
            } else if let Some(chunk) = next_chunk {
                {
                    let mut guard = remote.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(state) = guard.get_mut(&job_id) {
                        state.queue.push_back(chunk);
                        state.cursor = next_cursor.clone();
                    }
                }
                let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                let updated = match guard.get(&job_id) {
                    Some(JobState::Streaming {
                        started_at,
                        chunks_emitted,
                    }) => JobState::Streaming {
                        started_at: *started_at,
                        chunks_emitted: chunks_emitted.saturating_add(1),
                    },
                    Some(JobState::Running { started_at }) => JobState::Streaming {
                        started_at: *started_at,
                        chunks_emitted: 1,
                    },
                    Some(JobState::Pending { .. }) | None => JobState::Streaming {
                        started_at: Timestamp::now(),
                        chunks_emitted: 1,
                    },
                    Some(JobState::Complete(response)) => JobState::Complete(response.clone()),
                    Some(JobState::Failed { error, at }) => JobState::Failed {
                        error: error.clone(),
                        at: *at,
                    },
                };
                guard.insert(job_id.clone(), updated);
            }

            let mut guard = remote.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&job_id) {
                state.streaming = false;
                state.cursor = next_cursor;
            }
        });
    }
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
impl ComputeProvider for OpenAgentsComputeProvider {
    fn id(&self) -> &str {
        &self.provider_id
    }

    fn info(&self) -> ProviderInfo {
        self.info.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn is_available(&self) -> bool {
        let info = self.info.read().unwrap_or_else(|e| e.into_inner());
        matches!(
            info.status,
            ProviderStatus::Available | ProviderStatus::Degraded { .. }
        )
    }

    fn supports_model(&self, model: &str) -> bool {
        let info = self.info.read().unwrap_or_else(|e| e.into_inner());
        if info.models.is_empty() {
            return true;
        }
        info.models.iter().any(|entry| entry.id == model)
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let started_at = Timestamp::now();
        self.jobs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                job_id.clone(),
                JobState::Pending {
                    submitted_at: started_at,
                },
            );
        self.remote
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(job_id.clone(), RemoteJobState::default());

        let jobs = Arc::clone(&self.jobs);
        let remote = Arc::clone(&self.remote);
        let token_provider = Arc::clone(&self.token_provider);
        let url = self.url(&format!("compute/providers/{}/jobs", self.provider_id));
        let job_id_clone = job_id.clone();
        let request_clone = request.clone();

        spawn_local(async move {
            let token = match token_provider.api_token() {
                Some(token) => token,
                None => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: "OpenAgents API token required".to_string(),
                            at: Timestamp::now(),
                        },
                    );
                    return;
                }
            };
            let body = match serde_json::to_string(&request_clone) {
                Ok(body) => body,
                Err(err) => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        },
                    );
                    return;
                }
            };
            let response = wasm_http::request_bytes("POST", &url, Some(&token), Some(body)).await;
            #[derive(Deserialize)]
            struct JobResponse {
                job_id: String,
            }
            match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    match serde_json::from_slice::<JobResponse>(&bytes) {
                        Ok(payload) => {
                            let mut remote_guard =
                                remote.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(state) = remote_guard.get_mut(&job_id_clone) {
                                state.remote_id = Some(payload.job_id);
                            }
                            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                            let state = if request_clone.stream {
                                JobState::Streaming {
                                    started_at,
                                    chunks_emitted: 0,
                                }
                            } else {
                                JobState::Running { started_at }
                            };
                            guard.insert(job_id_clone.clone(), state);
                        }
                        Err(err) => {
                            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                            guard.insert(
                                job_id_clone.clone(),
                                JobState::Failed {
                                    error: format!("invalid response: {}", err),
                                    at: Timestamp::now(),
                                },
                            );
                        }
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: format!("openagents api {}: {}", status, body),
                            at: Timestamp::now(),
                        },
                    );
                }
                Err(err) => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }
        });

        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        let state = self
            .jobs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(job_id)
            .cloned();
        if let Some(state) = state.as_ref() {
            if !matches!(state, JobState::Complete(_) | JobState::Failed { .. }) {
                self.spawn_refresh(job_id);
            }
        }
        state
    }

    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        let mut chunk = None;
        {
            let mut guard = self.remote.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(job_id) {
                chunk = state.queue.pop_front();
            }
        }
        if chunk.is_some() {
            return Ok(chunk);
        }
        self.spawn_stream_poll(job_id);
        Ok(None)
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let remote_id = {
            let mut guard = self.remote.lock().unwrap_or_else(|e| e.into_inner());
            let state = guard.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
            state.remote_id.clone()
        };
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        jobs.insert(
            job_id.to_string(),
            JobState::Failed {
                error: "cancelled".to_string(),
                at: Timestamp::now(),
            },
        );

        if let Some(remote_id) = remote_id {
            let url = self.url(&format!("compute/jobs/{}/cancel", remote_id));
            let token_provider = Arc::clone(&self.token_provider);
            spawn_local(async move {
                if let Some(token) = token_provider.api_token() {
                    let _ = wasm_http::request_bytes("POST", &url, Some(&token), None).await;
                }
            });
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
const DVM_QUOTE_WINDOW: Duration = Duration::from_secs(5);

/// NIP-90 DVM provider for decentralized compute.
#[cfg(not(target_arch = "wasm32"))]
pub struct DvmProvider {
    agent_id: AgentId,
    transport: Arc<dyn DvmTransport>,
    signer: Arc<dyn SigningService>,
    wallet: Option<Arc<dyn WalletService>>,
    fx: Arc<FxRateCache>,
    executor: Executor,
    jobs: Arc<RwLock<HashMap<String, DvmJobState>>>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct DvmQuote {
    provider_pubkey: String,
    price_sats: u64,
    price_usd: u64,
    event_id: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
enum DvmLifecycle {
    AwaitingQuotes {
        since: Timestamp,
        timeout_at: Timestamp,
    },
    Processing {
        accepted_at: Timestamp,
        provider: String,
    },
    PendingSettlement {
        result_at: Timestamp,
        invoice: Option<String>,
    },
    Settled {
        settled_at: Timestamp,
    },
    Failed {
        error: String,
        at: Timestamp,
    },
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct DvmJobState {
    job_id: String,
    request_event_id: String,
    request_kind: u16,
    request: ComputeRequest,
    submitted_at: Timestamp,
    lifecycle: DvmLifecycle,
    quotes: Vec<DvmQuote>,
    accepted_quote: Option<DvmQuote>,
    result: Option<ComputeResponse>,
    partials: VecDeque<ComputeChunk>,
    payment_made: bool,
    paid_amount_sats: Option<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DvmProvider {
    /// Create a new DVM provider using Nostr relays.
    pub fn new(
        agent_id: AgentId,
        relays: Vec<String>,
        signer: Arc<dyn SigningService>,
        wallet: Option<Arc<dyn WalletService>>,
        fx_source: FxSource,
        fx_cache_secs: u64,
    ) -> Result<Self, ComputeError> {
        let transport = Arc::new(RelayPoolTransport::new(relays));
        Self::with_transport(agent_id, transport, signer, wallet, fx_source, fx_cache_secs)
    }

    /// Create a DVM provider with a custom transport (tests).
    pub(crate) fn with_transport(
        agent_id: AgentId,
        transport: Arc<dyn DvmTransport>,
        signer: Arc<dyn SigningService>,
        wallet: Option<Arc<dyn WalletService>>,
        fx_source: FxSource,
        fx_cache_secs: u64,
    ) -> Result<Self, ComputeError> {
        let executor = Executor::new()?;
        let runtime = executor.runtime();
        executor
            .block_on(transport.connect())
            .map_err(ComputeError::ProviderError)?;
        let wallet_fx = wallet.as_ref().map(|wallet| {
            Arc::new(WalletFxProvider::new(wallet.clone())) as Arc<dyn FxRateProvider>
        });
        let fx = Arc::new(FxRateCache::new(
            fx_source,
            fx_cache_secs,
            wallet_fx,
            runtime,
        ));
        Ok(Self {
            agent_id,
            transport,
            signer,
            wallet,
            fx,
            executor,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn map_kind(kind: &ComputeKind) -> Result<u16, ComputeError> {
        match kind {
            ComputeKind::Chat | ComputeKind::Complete => Ok(KIND_JOB_TEXT_GENERATION),
            ComputeKind::ImageGenerate => Ok(KIND_JOB_IMAGE_GENERATION),
            ComputeKind::Transcribe => Ok(KIND_JOB_SPEECH_TO_TEXT),
            other => Err(ComputeError::UnsupportedKind(format!("{:?}", other))),
        }
    }

    fn build_job_request(
        &self,
        request: &ComputeRequest,
        kind: u16,
    ) -> Result<JobRequest, ComputeError> {
        let prompt = match request.kind {
            ComputeKind::Chat => parse_messages(&request.input),
            ComputeKind::Complete | ComputeKind::ImageGenerate | ComputeKind::Transcribe => {
                parse_prompt(&request.input)
            }
            _ => None,
        }
        .ok_or_else(|| ComputeError::InvalidRequest("missing prompt".to_string()))?;

        let mut job = JobRequest::new(kind)
            .map_err(|err| ComputeError::InvalidRequest(err.to_string()))?
            .add_input(JobInput::text(prompt))
            .add_param("model", request.model.clone());

        if let Some(obj) = request.input.as_object() {
            if let Some(max_tokens) = obj.get("max_tokens").and_then(|v| v.as_u64()) {
                job = job.add_param("max_tokens", max_tokens.to_string());
            }
            if let Some(temp) = obj.get("temperature").and_then(|v| v.as_f64()) {
                job = job.add_param("temperature", temp.to_string());
            }
            if let Some(top_p) = obj.get("top_p").and_then(|v| v.as_f64()) {
                job = job.add_param("top_p", top_p.to_string());
            }
            if let Some(stop) = obj.get("stop").and_then(|v| v.as_array()) {
                for (idx, item) in stop.iter().enumerate() {
                    if let Some(value) = item.as_str() {
                        job = job.add_param(format!("stop_{}", idx), value.to_string());
                    }
                }
            }
        }

        for relay in self.transport.relays() {
            job = job.add_relay(relay);
        }

        let max_cost_usd = request.max_cost_usd.unwrap_or(100_000);
        let max_cost_sats = self
            .fx
            .usd_to_sats(max_cost_usd)
            .map_err(|err| ComputeError::ProviderError(err.to_string()))?;
        let bid_msats = u128::from(max_cost_sats) * 1000;
        let bid_msats = u64::try_from(bid_msats)
            .map_err(|_| ComputeError::ProviderError("bid overflow".to_string()))?;
        job = job.with_bid(bid_msats);

        Ok(job)
    }

    fn sign_event(
        &self,
        kind: u16,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> Result<nostr::Event, ComputeError> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let pubkey = self
            .signer
            .pubkey(&self.agent_id)
            .map_err(|err| ComputeError::ProviderError(err.to_string()))?;
        let pubkey_hex = pubkey.to_hex();
        let unsigned = UnsignedEvent {
            pubkey: pubkey_hex.clone(),
            created_at,
            kind,
            tags,
            content,
        };
        let id = get_event_hash(&unsigned).map_err(|err| ComputeError::ProviderError(err.to_string()))?;
        let id_bytes = hex::decode(&id).map_err(|err| ComputeError::ProviderError(err.to_string()))?;
        let sig = self
            .signer
            .sign(&self.agent_id, &id_bytes)
            .map_err(|err| ComputeError::ProviderError(err.to_string()))?;

        Ok(nostr::Event {
            id,
            pubkey: pubkey_hex,
            created_at,
            kind,
            tags: unsigned.tags,
            content: unsigned.content,
            sig: sig.to_hex(),
        })
    }

    fn spawn_quote_manager(&self, job_id: String) {
        let jobs = self.jobs.clone();
        let transport = self.transport.clone();
        let signer = self.signer.clone();
        let agent_id = self.agent_id.clone();
        let executor = self.executor.clone();

        executor.spawn(async move {
            tokio::time::sleep(DVM_QUOTE_WINDOW).await;

            let (request_event_id, quote) = {
                let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                let job = match guard.get_mut(&job_id) {
                    Some(job) => job,
                    None => return,
                };
                if !matches!(job.lifecycle, DvmLifecycle::AwaitingQuotes { .. }) {
                    return;
                }
                let best = match job
                    .quotes
                    .iter()
                    .min_by_key(|quote| quote.price_usd)
                    .cloned()
                {
                    Some(best) => best,
                    None => {
                        job.lifecycle = DvmLifecycle::Failed {
                            error: "no quotes received".to_string(),
                            at: Timestamp::now(),
                        };
                        return;
                    }
                };
                job.accepted_quote = Some(best.clone());
                job.lifecycle = DvmLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: best.provider_pubkey.clone(),
                };
                (job.request_event_id.clone(), best)
            };

            let tags = vec![
                vec!["e".to_string(), request_event_id],
                vec!["e".to_string(), quote.event_id.clone()],
                vec!["p".to_string(), quote.provider_pubkey.clone()],
                vec!["status".to_string(), "processing".to_string()],
            ];

            let created_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let pubkey = match signer.pubkey(&agent_id) {
                Ok(pubkey) => pubkey,
                Err(_) => return,
            };
            let pubkey_hex = pubkey.to_hex();
            let unsigned = UnsignedEvent {
                pubkey: pubkey_hex.clone(),
                created_at,
                kind: KIND_JOB_FEEDBACK,
                tags,
                content: String::new(),
            };
            let id = match get_event_hash(&unsigned) {
                Ok(id) => id,
                Err(_) => return,
            };
            let id_bytes = match hex::decode(&id) {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let sig = match signer.sign(&agent_id, &id_bytes) {
                Ok(sig) => sig,
                Err(_) => return,
            };
            let event = nostr::Event {
                id,
                pubkey: pubkey_hex,
                created_at,
                kind: KIND_JOB_FEEDBACK,
                tags: unsigned.tags,
                content: unsigned.content,
                sig: sig.to_hex(),
            };
            let _ = transport.publish(event).await;
        });
    }

    fn subscribe_job_events(
        &self,
        job_id: String,
        request_event_id: String,
        request_kind: u16,
    ) -> Result<(), ComputeError> {
        let result_kind = get_result_kind(request_kind)
            .ok_or_else(|| ComputeError::ProviderError("invalid job kind".to_string()))?;
        let filters = vec![
            serde_json::json!({
                "kinds": [result_kind],
                "#e": [request_event_id],
            }),
            serde_json::json!({
                "kinds": [KIND_JOB_FEEDBACK],
                "#e": [request_event_id],
            }),
        ];
        let subscription_id = format!("dvm-job-{}", request_event_id);
        let mut rx = self
            .executor
            .block_on(self.transport.subscribe(&subscription_id, &filters))
            .map_err(ComputeError::ProviderError)?;

        let jobs = self.jobs.clone();
        let fx = self.fx.clone();
        let wallet = self.wallet.clone();
        let request_model = {
            let guard = jobs.read().unwrap_or_else(|e| e.into_inner());
            guard
                .get(&job_id)
                .map(|job| job.request.model.clone())
                .unwrap_or_default()
        };

        self.executor.spawn(async move {
            while let Some(event) = rx.recv().await {
                if event.kind == result_kind {
                    handle_dvm_result(
                        &job_id,
                        &request_model,
                        &event,
                        &jobs,
                        &fx,
                        &wallet,
                    );
                } else if event.kind == KIND_JOB_FEEDBACK {
                    if let Some(feedback) = parse_feedback_event(&event) {
                        handle_dvm_feedback(&job_id, feedback, &jobs, &fx, &wallet);
                    }
                }
            }
        });

        Ok(())
    }

    fn query_handlers(&self) -> Result<Vec<HandlerInfo>, ComputeError> {
        let filters = vec![serde_json::json!({
            "kinds": [KIND_HANDLER_INFO],
            "limit": 100
        })];
        let events = self
            .executor
            .block_on(self.transport.query(&filters, Duration::from_secs(2)))
            .map_err(ComputeError::ProviderError)?;
        let mut handlers = Vec::new();
        for event in events {
            if let Ok(handler) = HandlerInfo::from_event(&event) {
                if handler.handler_type == HandlerType::ComputeProvider {
                    handlers.push(handler);
                }
            }
        }
        Ok(handlers)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ComputeProvider for DvmProvider {
    fn id(&self) -> &str {
        "dvm"
    }

    fn info(&self) -> ProviderInfo {
        let handlers = self.query_handlers().unwrap_or_default();
        let mut models = Vec::new();
        for handler in handlers {
            for (key, value) in handler.custom_tags {
                if key == "model" {
                    models.push(ModelInfo {
                        id: value.clone(),
                        name: value,
                        context_length: None,
                        capabilities: vec![ComputeKind::Chat, ComputeKind::Complete],
                        pricing: None,
                    });
                }
            }
        }
        ProviderInfo {
            id: "dvm".to_string(),
            name: "NIP-90 DVM Network".to_string(),
            models,
            capabilities: vec![
                ComputeKind::Chat,
                ComputeKind::Complete,
                ComputeKind::ImageGenerate,
                ComputeKind::Transcribe,
            ],
            pricing: None,
            latency: ProviderLatency {
                ttft_ms: 2000,
                tokens_per_sec: None,
                measured: false,
            },
            region: None,
            status: if self.wallet.is_none() {
                ProviderStatus::Unavailable {
                    reason: "wallet not configured".to_string(),
                }
            } else if self.transport.relays().is_empty() {
                ProviderStatus::Unavailable {
                    reason: "no relays configured".to_string(),
                }
            } else {
                ProviderStatus::Available
            },
        }
    }

    fn is_available(&self) -> bool {
        self.wallet.is_some() && !self.transport.relays().is_empty()
    }

    fn supports_model(&self, model: &str) -> bool {
        self.query_handlers()
            .map(|handlers| {
                handlers.iter().any(|handler| {
                    handler
                        .custom_tags
                        .iter()
                        .any(|(key, value)| key == "model" && value == model)
                })
            })
            .unwrap_or(false)
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        if self.wallet.is_none() {
            return Err(ComputeError::ProviderError(
                "wallet not configured".to_string(),
            ));
        }

        let kind = Self::map_kind(&request.kind)?;
        let job_request = self.build_job_request(&request, kind)?;
        let event = self.sign_event(
            job_request.kind,
            job_request.to_tags(),
            job_request.content.clone(),
        )?;
        let event_id = event.id.clone();

        self.executor
            .block_on(self.transport.publish(event))
            .map_err(ComputeError::ProviderError)?;

        let job_id = uuid::Uuid::new_v4().to_string();
        let now = Timestamp::now();
        self.jobs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                job_id.clone(),
                DvmJobState {
                    job_id: job_id.clone(),
                    request_event_id: event_id.clone(),
                    request_kind: kind,
                    request: request.clone(),
                    submitted_at: now,
                    lifecycle: DvmLifecycle::AwaitingQuotes {
                        since: now,
                        timeout_at: Timestamp::from_millis(
                            now.as_millis() + DVM_QUOTE_WINDOW.as_millis() as u64,
                        ),
                    },
                    quotes: Vec::new(),
                    accepted_quote: None,
                    result: None,
                    partials: VecDeque::new(),
                    payment_made: false,
                    paid_amount_sats: None,
                },
            );

        self.subscribe_job_events(job_id.clone(), event_id, kind)?;
        self.spawn_quote_manager(job_id.clone());
        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        let guard = self.jobs.read().unwrap_or_else(|e| e.into_inner());
        let job = guard.get(job_id)?;
        Some(match &job.lifecycle {
            DvmLifecycle::AwaitingQuotes { .. } => JobState::Pending {
                submitted_at: job.submitted_at,
            },
            DvmLifecycle::Processing { accepted_at, .. } => JobState::Running {
                started_at: *accepted_at,
            },
            DvmLifecycle::PendingSettlement { .. } => JobState::Running {
                started_at: job.submitted_at,
            },
            DvmLifecycle::Settled { .. } => job
                .result
                .clone()
                .map(JobState::Complete)
                .unwrap_or(JobState::Running {
                    started_at: job.submitted_at,
                }),
            DvmLifecycle::Failed { error, at } => JobState::Failed {
                error: error.clone(),
                at: *at,
            },
        })
    }

    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        let mut guard = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let job = guard.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
        Ok(job.partials.pop_front())
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let (request_event_id, request_kind) = {
            let mut guard = self.jobs.write().unwrap_or_else(|e| e.into_inner());
            let job = guard.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
            job.lifecycle = DvmLifecycle::Failed {
                error: "cancelled".to_string(),
                at: Timestamp::now(),
            };
            (job.request_event_id.clone(), job.request_kind)
        };

        let tags = create_deletion_tags(&[request_event_id.as_str()], Some(request_kind));
        let event = self.sign_event(DELETION_REQUEST_KIND, tags, String::new())?;
        self.executor
            .block_on(self.transport.publish(event))
            .map_err(ComputeError::ProviderError)?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_dvm_feedback(
    job_id: &str,
    feedback: crate::dvm::DvmFeedback,
    jobs: &Arc<RwLock<HashMap<String, DvmJobState>>>,
    fx: &Arc<FxRateCache>,
    wallet: &Option<Arc<dyn WalletService>>,
) {
    let mut payment_request = None;

    {
        let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
        let Some(job) = guard.get_mut(job_id) else {
            return;
        };
        if matches!(job.lifecycle, DvmLifecycle::Failed { .. } | DvmLifecycle::Settled { .. }) {
            return;
        }

        match feedback.status {
            DvmFeedbackStatus::Quote => {
                if let Some(amount_msats) = feedback.amount_msats {
                    let price_sats = msats_to_sats(amount_msats);
                    let price_usd = match fx.sats_to_usd(price_sats) {
                        Ok(price_usd) => price_usd,
                        Err(err) => {
                            job.lifecycle = DvmLifecycle::Failed {
                                error: err.to_string(),
                                at: Timestamp::now(),
                            };
                            return;
                        }
                    };
                    let quote = DvmQuote {
                        provider_pubkey: feedback.provider_pubkey.clone(),
                        price_sats,
                        price_usd,
                        event_id: feedback.event_id.clone(),
                    };
                    if let Some(existing) = job
                        .quotes
                        .iter_mut()
                        .find(|q| q.provider_pubkey == quote.provider_pubkey)
                    {
                        if quote.price_usd < existing.price_usd {
                            *existing = quote;
                        }
                    } else {
                        job.quotes.push(quote);
                    }
                }
            }
            DvmFeedbackStatus::Job(JobStatus::Partial) => {
                let chunk = ComputeChunk {
                    job_id: job_id.to_string(),
                    delta: serde_json::json!({ "text": feedback.content }),
                    finish_reason: None,
                    usage: None,
                };
                job.partials.push_back(chunk);
            }
            DvmFeedbackStatus::Job(JobStatus::Processing) => {
                job.lifecycle = DvmLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: feedback.provider_pubkey.clone(),
                };
            }
            DvmFeedbackStatus::Job(JobStatus::PaymentRequired) => {
                if job.payment_made {
                    return;
                }
                let invoice = feedback
                    .bolt11
                    .clone()
                    .or_else(|| {
                        let trimmed = feedback.content.trim();
                        if trimmed.starts_with("ln") {
                            Some(trimmed.to_string())
                        } else {
                            None
                        }
                    });
                if let Some(invoice) = invoice {
                    payment_request = Some((invoice, feedback.amount_msats, feedback.provider_pubkey));
                } else {
                    job.lifecycle = DvmLifecycle::Failed {
                        error: "payment required but invoice missing".to_string(),
                        at: Timestamp::now(),
                    };
                }
            }
            DvmFeedbackStatus::Job(JobStatus::Error) => {
                job.lifecycle = DvmLifecycle::Failed {
                    error: feedback.status_extra.unwrap_or_else(|| "provider error".to_string()),
                    at: Timestamp::now(),
                };
            }
            _ => {}
        }
    }

    let Some((invoice, amount_msats, provider_pubkey)) = payment_request else {
        return;
    };
    let Some(wallet) = wallet.as_ref() else {
        let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = guard.get_mut(job_id) {
            job.lifecycle = DvmLifecycle::Failed {
                error: "wallet not configured".to_string(),
                at: Timestamp::now(),
            };
        }
        return;
    };
    let amount_sats = amount_msats.map(msats_to_sats);
    let payment = tokio::task::block_in_place(|| wallet.pay_invoice(&invoice, amount_sats));
    match payment {
        Ok(payment) => {
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.payment_made = true;
                job.paid_amount_sats = Some(payment.amount_sats);
                job.lifecycle = DvmLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: provider_pubkey,
                };
            }
        }
        Err(err) => {
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.lifecycle = DvmLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_dvm_result(
    job_id: &str,
    model: &str,
    event: &nostr::Event,
    jobs: &Arc<RwLock<HashMap<String, DvmJobState>>>,
    fx: &Arc<FxRateCache>,
    wallet: &Option<Arc<dyn WalletService>>,
) {
    let result = match JobResult::from_event(event) {
        Ok(result) => result,
        Err(_) => return,
    };
    let invoice = result.bolt11.clone();
    let amount_sats = result.amount.map(msats_to_sats);

    let (response, already_paid) = {
        let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
        let Some(job) = guard.get_mut(job_id) else {
            return;
        };
        if matches!(job.lifecycle, DvmLifecycle::Failed { .. }) {
            return;
        }
        let cost_sats = amount_sats
            .or(job.paid_amount_sats)
            .or_else(|| job.accepted_quote.as_ref().map(|quote| quote.price_sats))
            .unwrap_or(0);
        let cost_usd = match fx.sats_to_usd(cost_sats) {
            Ok(cost_usd) => cost_usd,
            Err(err) => {
                job.lifecycle = DvmLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
                return;
            }
        };
        let output = serde_json::from_str(&result.content)
            .unwrap_or_else(|_| serde_json::json!({ "text": result.content }));
        let latency_ms = Timestamp::now()
            .as_millis()
            .saturating_sub(job.submitted_at.as_millis()) as u64;
        let response = ComputeResponse {
            job_id: job_id.to_string(),
            output,
            usage: None,
            cost_usd,
            latency_ms,
            provider_id: "dvm".to_string(),
            model: model.to_string(),
        };
        job.result = Some(response.clone());
        if invoice.is_some() {
            job.lifecycle = DvmLifecycle::PendingSettlement {
                result_at: Timestamp::now(),
                invoice: invoice.clone(),
            };
        } else {
            job.lifecycle = DvmLifecycle::Settled {
                settled_at: Timestamp::now(),
            };
        }
        (response, job.payment_made)
    };

    if invoice.is_none() || already_paid {
        if already_paid {
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.lifecycle = DvmLifecycle::Settled {
                    settled_at: Timestamp::now(),
                };
            }
        }
        return;
    }

    let Some(wallet) = wallet.as_ref() else {
        let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = guard.get_mut(job_id) {
            job.lifecycle = DvmLifecycle::Failed {
                error: "wallet not configured".to_string(),
                at: Timestamp::now(),
            };
        }
        return;
    };
    let invoice = invoice.unwrap();
    let payment = tokio::task::block_in_place(|| wallet.pay_invoice(&invoice, amount_sats));
    match payment {
        Ok(payment) => {
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.payment_made = true;
                job.paid_amount_sats = Some(payment.amount_sats);
                job.result = Some(response);
                job.lifecycle = DvmLifecycle::Settled {
                    settled_at: Timestamp::now(),
                };
            }
        }
        Err(err) => {
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.lifecycle = DvmLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
            }
        }
    }
}

#[derive(Clone)]
#[cfg(not(target_arch = "wasm32"))]
struct Executor {
    runtime: Arc<tokio::runtime::Runtime>,
}

#[cfg(not(target_arch = "wasm32"))]
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

    fn runtime(&self) -> Arc<tokio::runtime::Runtime> {
        self.runtime.clone()
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
