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

