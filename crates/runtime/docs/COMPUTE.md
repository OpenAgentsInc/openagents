# AI Compute

Portable compute abstraction for agents.

---

## Overview

Agents access AI compute through the `/compute` mount. The runtime provides:

- **Provider abstraction** — Same interface for local, cloud, and decentralized compute
- **Policy enforcement** — Per-agent model restrictions and cost limits
- **Budget tracking** — USD-denominated accounting (settled in sats via Lightning)
- **Idempotency** — Dedup via scoped journal to prevent double-billing
- **Streaming** — Token-by-token output via `watch()` on job streams

This enables agents to use any AI model without hardcoding providers.

---

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Budget units | **USD** (micro-USD for precision) | Universal pricing language; settled in sats via Lightning |
| Execution model | **Always returns job_id** | No blocking; works on all backends including Cloudflare |
| Streaming | **Only via watch()** | Single streaming surface; no internal StreamPoll exposed |
| Interface | **`/compute/new` → job_id** | Avoids shared-file races on `/compute/run` |
| Provider naming | **`dvm`** (not "swarm") | Consistent with NIP-90 terminology |

---

## Filesystem Layout

```
/compute/
├── providers/           # Available compute providers
│   ├── local/          # Ollama, llama.cpp, Apple FM
│   │   ├── info        # Read: provider info JSON
│   │   ├── models      # Read: available models
│   │   └── health      # Read: health status
│   ├── cloudflare/     # Cloudflare Workers AI
│   │   ├── info
│   │   ├── models
│   │   └── health
│   └── dvm/            # NIP-90 DVM network
│       ├── info
│       ├── models
│       └── health
├── new                  # Write request → read job_id (always async)
├── policy               # Read/write: compute policy JSON
├── usage                # Read: current tick/day usage (USD)
└── jobs/
    └── <job_id>/       # Individual job tracking
        ├── status      # Read: pending|running|streaming|complete|failed
        ├── result      # Read: final result (when complete)
        └── stream      # Watch: streaming chunks
```

**Key principle: `/compute/new` always returns a job_id immediately.** The agent then reads `/compute/jobs/<id>/status` to poll or watches `/compute/jobs/<id>/stream` for streaming. This avoids blocking and shared-file races.

---

## Core Types

### ComputeKind

```rust
/// Kind of compute operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputeKind {
    /// Chat/conversational completion
    Chat,
    /// Raw text completion
    Complete,
    /// Generate embeddings
    Embeddings,
    /// Generate images
    ImageGenerate,
    /// Analyze images (vision)
    ImageAnalyze,
    /// Audio transcription
    Transcribe,
    /// Text-to-speech
    Speak,
    /// Custom operation type
    Custom(String),
}
```

### ComputeRequest

```rust
/// A compute request (provider-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRequest {
    /// Model identifier (e.g., "llama-70b", "@cf/meta/llama-2-7b-chat-fp16")
    pub model: String,

    /// Kind of compute operation
    pub kind: ComputeKind,

    /// Input data (schema depends on kind)
    /// For Chat: { "messages": [{"role": "user", "content": "..."}] }
    /// For Embeddings: { "text": "..." } or { "texts": ["...", "..."] }
    pub input: serde_json::Value,

    /// Request streaming response
    #[serde(default)]
    pub stream: bool,

    /// Timeout in milliseconds (default: 120000)
    pub timeout_ms: Option<u64>,

    /// Idempotency key for deduplication (REQUIRED for non-idempotent ops)
    /// Full key is scoped: {agent_id}:{provider_id}:{idempotency_key}
    pub idempotency_key: Option<String>,

    /// Maximum cost in micro-USD (1 micro-USD = $0.000001) the caller is willing to pay.
    /// Budget check uses this value directly—no estimation guessing.
    /// If omitted, uses policy default or rejects if policy.require_max_cost.
    pub max_cost_usd: Option<u64>,
}
```

### ComputeResponse

```rust
/// Compute response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResponse {
    /// Job identifier
    pub job_id: String,

    /// Output data (schema depends on kind)
    /// For Chat: { "text": "...", "finish_reason": "stop" }
    /// For Embeddings: { "embeddings": [[...], [...]] }
    pub output: serde_json::Value,

    /// Token usage (if applicable)
    pub usage: Option<TokenUsage>,

    /// Actual cost in micro-USD (post-execution)
    pub cost_usd: u64,

    /// Latency in milliseconds
    pub latency_ms: u64,

    /// Provider that handled the request
    pub provider_id: String,

    /// Model actually used (may differ from requested if aliased)
    pub model: String,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}
```

### ComputeChunk (Streaming)

```rust
/// A streaming chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeChunk {
    /// Job identifier
    pub job_id: String,

    /// Delta content
    /// For Chat: { "text": "token" }
    pub delta: serde_json::Value,

    /// Finish reason (only on final chunk)
    pub finish_reason: Option<String>,

    /// Accumulated usage (updated each chunk)
    pub usage: Option<TokenUsage>,
}
```

---

## ComputePolicy

Per-agent policy configured via mount table:

```rust
/// Provider selection preference
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Prefer {
    /// Minimize cost
    Cost,
    /// Minimize latency
    Latency,
    /// Maximize quality/reliability
    Quality,
    /// Balanced (default)
    #[default]
    Balanced,
}

/// Policy for compute operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputePolicy {
    /// Allowed provider IDs (empty = all)
    #[serde(default)]
    pub allowed_providers: Vec<String>,

    /// Allowed models (empty = all)
    #[serde(default)]
    pub allowed_models: Vec<String>,

    /// Blocked models (takes precedence over allowed)
    #[serde(default)]
    pub blocked_models: Vec<String>,

    /// Maximum cost per tick in micro-USD
    pub max_cost_usd_per_tick: Option<u64>,

    /// Maximum cost per day in micro-USD
    pub max_cost_usd_per_day: Option<u64>,

    /// Default max_cost_usd if request doesn't specify
    pub default_max_cost_usd: Option<u64>,

    /// Require requests to specify max_cost_usd
    #[serde(default)]
    pub require_max_cost: bool,

    /// Default timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub default_timeout_ms: u64,

    /// Selection preference
    #[serde(default)]
    pub prefer: Prefer,

    /// Fallback provider (if primary fails)
    pub fallback_provider: Option<String>,

    /// Require idempotency keys for all requests
    #[serde(default)]
    pub require_idempotency: bool,
}

fn default_timeout() -> u64 { 120_000 } // 2 minutes
```

### Budget Units: USD with Sats Settlement

All budget tracking uses **micro-USD** (1 micro-USD = $0.000001) as the canonical unit. This is the universal pricing language—providers price in USD, users think in USD, and budgets are set in USD.

**Settlement happens in sats via Lightning.** When paying DVM providers or reconciling costs, the runtime converts USD to sats using a configured FX source:

```rust
/// FX rate source for USD→sats conversion (for settlement)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxSource {
    /// Fixed rate (sats per USD)
    Fixed { sats_per_usd: u64 },
    /// From /wallet/fx endpoint (runtime-provided)
    Wallet,
    /// External oracle (URL returns JSON with sats_per_usd)
    Oracle { url: String },
}

/// Budget configuration for compute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeBudgetConfig {
    /// How to convert USD to sats for Lightning settlement
    pub fx_source: FxSource,
    /// Cache FX rate for this duration (seconds)
    #[serde(default = "default_fx_cache")]
    pub fx_cache_secs: u64,
}

fn default_fx_cache() -> u64 { 300 } // 5 minutes
```

**Why USD:**
- Providers price in USD (OpenAI, Cloudflare, Anthropic)
- Users set budgets in USD ("$10/day")
- Stable unit for cost comparison and limits
- No constant recalculation as BTC price moves

**Why sats for settlement:**
- DVM providers accept Lightning payments
- Permissionless, instant, global
- Runtime's `/wallet` already handles sats

Mount table configuration:

```yaml
mounts:
  /compute:
    type: compute
    access: budgeted  # REQUIRED: /compute MUST use AccessLevel::Budgeted
    budget:
      per_tick_usd: 100000      # $0.10/tick (in micro-USD)
      per_day_usd: 10000000     # $10/day (in micro-USD)
      approval_threshold_usd: 1000000  # $1 requires approval
    policy:
      allowed_providers: ["local", "cloudflare", "dvm"]
      blocked_models: ["gpt-4"]
      prefer: cost
      require_idempotency: true
      require_max_cost: true    # Reject requests without max_cost_usd
    settlement:
      fx_source: wallet         # Use /wallet/fx for USD→sats conversion
      fx_cache_secs: 300
```

---

## ComputeProvider Trait

Sync trait for FileService compatibility:

```rust
/// Information about a compute provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub models: Vec<ModelInfo>,
    pub capabilities: Vec<ComputeKind>,
    pub pricing: Option<ProviderPricing>,
    pub latency: ProviderLatency,
    pub region: Option<String>,
    pub status: ProviderStatus,
}

/// Latency metrics for provider selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderLatency {
    /// Expected time-to-first-token in milliseconds
    pub ttft_ms: u64,
    /// Expected tokens per second throughput
    pub tokens_per_sec: Option<u64>,
    /// Is this measured (true) or estimated (false)?
    pub measured: bool,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_length: Option<u32>,
    pub capabilities: Vec<ComputeKind>,
    pub pricing: Option<ModelPricing>,
}

/// Provider pricing defaults (micro-USD per 1k tokens, for FX conversion)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricing {
    pub input_per_1k_microusd: u64,
    pub output_per_1k_microusd: u64,
}

/// Per-model pricing overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_per_1k_microusd: u64,
    pub output_per_1k_microusd: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderStatus {
    Available,
    Degraded { reason: String },
    Unavailable { reason: String },
}

/// Compute provider trait (sync for FileService compatibility)
pub trait ComputeProvider: Send + Sync {
    /// Provider identifier
    fn id(&self) -> &str;

    /// Provider info (models, pricing, latency, etc.)
    fn info(&self) -> ProviderInfo;

    /// Check if provider is available
    fn is_available(&self) -> bool;

    /// Check if provider supports a model
    fn supports_model(&self, model: &str) -> bool;

    /// Submit a compute request. ALWAYS returns job_id immediately.
    /// No blocking—enqueues work and returns.
    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError>;

    /// Get current state of a job by ID
    fn get_job(&self, job_id: &str) -> Option<JobState>;

    /// Poll streaming job for next chunk (internal use by ComputeFs)
    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError>;

    /// Cancel a running job
    fn cancel(&self, job_id: &str) -> Result<(), ComputeError>;
}

/// Job state for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobState {
    /// Job submitted, waiting for execution
    Pending { submitted_at: Timestamp },
    /// Job is executing
    Running { started_at: Timestamp },
    /// Job is streaming (chunks available via poll_stream)
    Streaming { started_at: Timestamp, chunks_emitted: usize },
    /// Job completed successfully
    Complete(ComputeResponse),
    /// Job failed
    Failed { error: String, at: Timestamp },
}
```

**Key invariant: `submit()` never blocks.** It enqueues the request and returns a `job_id`. The caller polls via `get_job()` or watches `/compute/jobs/<id>/stream`. This is critical for Cloudflare DO (no blocking allowed) and consistent across all backends.

---

## ComputeRouter

Routes requests to appropriate providers:

```rust
/// Routes compute requests to appropriate providers
pub struct ComputeRouter {
    providers: Vec<Arc<dyn ComputeProvider>>,
}

impl ComputeRouter {
    /// Register a provider
    pub fn register(&mut self, provider: Arc<dyn ComputeProvider>) {
        self.providers.push(provider);
    }

    /// List all available providers
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        self.providers.iter()
            .filter(|p| p.is_available())
            .map(|p| p.info())
            .collect()
    }

    /// Select best provider for request based on policy
    pub fn select(
        &self,
        request: &ComputeRequest,
        policy: &ComputePolicy,
    ) -> Result<&dyn ComputeProvider, ComputeError> {
        let candidates: Vec<_> = self.providers.iter()
            .filter(|p| p.is_available())
            .filter(|p| p.supports_model(&request.model))
            .filter(|p| {
                policy.allowed_providers.is_empty()
                    || policy.allowed_providers.contains(&p.id().to_string())
            })
            .filter(|p| {
                policy.allowed_models.is_empty()
                    || policy.allowed_models.contains(&request.model)
            })
            .filter(|_| !policy.blocked_models.contains(&request.model))
            .collect();

        if candidates.is_empty() {
            return Err(ComputeError::NoProviderAvailable {
                model: request.model.clone(),
                reason: "No provider supports this model or all are filtered by policy".into(),
            });
        }

        // Sort by preference
        let selected = match policy.prefer {
            Prefer::Cost => {
                // Use caller's max_cost_usd as the cost ceiling for comparison
                // Providers that can't serve under that cost are filtered out
                let max_usd = request.max_cost_usd.unwrap_or(u64::MAX);
                candidates.into_iter()
                    .filter(|p| self.estimate_provider_cost_usd(p, request) <= max_usd)
                    .min_by_key(|p| self.estimate_provider_cost_usd(p, request))
            }
            Prefer::Latency => {
                // Use actual latency metrics from ProviderInfo
                candidates.into_iter()
                    .min_by_key(|p| p.info().latency.ttft_ms)
            }
            Prefer::Quality => {
                // Prefer measured latency (real data) over estimates
                candidates.into_iter()
                    .max_by_key(|p| (p.info().latency.measured, p.info().latency.tokens_per_sec.unwrap_or(0)))
            }
            Prefer::Balanced => {
                // Balance cost and latency (simple heuristic)
                candidates.into_iter()
                    .min_by_key(|p| {
                        let cost = self.estimate_provider_cost_usd(p, request);
                        let latency = p.info().latency.ttft_ms;
                        cost.saturating_mul(latency) // Simple cost×latency score
                    })
            }
        };

        selected
            .map(|p| p.as_ref())
            .ok_or(ComputeError::NoProviderAvailable {
                model: request.model.clone(),
                reason: "Selection failed".into(),
            })
    }

    /// Estimate cost in micro-USD for a provider
    fn estimate_provider_cost_usd(
        &self,
        provider: &Arc<dyn ComputeProvider>,
        request: &ComputeRequest,
    ) -> u64 {
        // If provider is free (local), return 0
        let info = provider.info();
        let pricing = match info.pricing {
            Some(p) => p,
            None => return 0,
        };

        // Use request's max_cost_usd if provided
        if let Some(max_usd) = request.max_cost_usd {
            return max_usd;
        }

        // Fallback: estimate from provider pricing (rough, assumes ~1k tokens)
        pricing.input_per_1k_microusd + pricing.output_per_1k_microusd
    }

    /// Submit request with routing (returns job_id immediately)
    pub fn submit(
        &self,
        request: ComputeRequest,
        policy: &ComputePolicy,
    ) -> Result<String, ComputeError> {
        let provider = self.select(&request, policy)?;
        provider.submit(request)
    }
}
```

---

## ComputeFs FileService

The `/compute` FileService implementation:

```rust
/// Compute capability as a filesystem
pub struct ComputeFs {
    agent_id: String,  // For idempotency key scoping
    router: Arc<RwLock<ComputeRouter>>,
    policy: Arc<RwLock<ComputePolicy>>,
    usage: Arc<RwLock<ComputeUsageState>>,
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    journal: Arc<dyn IdempotencyJournal>,
}

/// Usage tracking (micro-USD)
pub struct ComputeUsageState {
    pub tick_start: Timestamp,
    pub day_start: Timestamp,
    pub spent_tick_usd: u64,
    pub spent_day_usd: u64,
}

impl FileService for ComputeFs {
    fn name(&self) -> &str { "compute" }

    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        match parts.as_slice() {
            // /compute/new — submit compute request, always returns job_id
            ["new"] if flags.write => {
                Ok(Box::new(ComputeNewHandle::new(
                    self.agent_id.clone(),
                    self.router.clone(),
                    self.policy.clone(),
                    self.usage.clone(),
                    self.jobs.clone(),
                    self.journal.clone(),
                )))
            }

            // /compute/policy — read/write policy
            ["policy"] => {
                if flags.write {
                    Ok(Box::new(PolicyWriteHandle::new(self.policy.clone())))
                } else {
                    let policy = self.policy.read();
                    Ok(Box::new(StringHandle::new(serde_json::to_string_pretty(&*policy)?)))
                }
            }

            // /compute/usage — current usage stats (micro-USD)
            ["usage"] => {
                let usage = self.usage.read();
                let policy = self.policy.read();
                let json = serde_json::json!({
                    "tick": {
                        "spent_usd": usage.spent_tick_usd,
                        "limit_usd": policy.max_cost_usd_per_tick,
                        "remaining_usd": policy.max_cost_usd_per_tick
                            .map(|l| l.saturating_sub(usage.spent_tick_usd)),
                    },
                    "day": {
                        "spent_usd": usage.spent_day_usd,
                        "limit_usd": policy.max_cost_usd_per_day,
                        "remaining_usd": policy.max_cost_usd_per_day
                            .map(|l| l.saturating_sub(usage.spent_day_usd)),
                    }
                });
                Ok(Box::new(StringHandle::new(json.to_string())))
            }

            // /compute/providers — list providers
            ["providers"] => {
                let router = self.router.read();
                let providers = router.list_providers();
                Ok(Box::new(StringHandle::new(serde_json::to_string_pretty(&providers)?)))
            }

            // /compute/providers/<id>/info
            ["providers", id, "info"] => {
                let router = self.router.read();
                let info = router.list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(Error::NotFound)?;
                Ok(Box::new(StringHandle::new(serde_json::to_string_pretty(&info)?)))
            }

            // /compute/providers/<id>/models
            ["providers", id, "models"] => {
                let router = self.router.read();
                let info = router.list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(Error::NotFound)?;
                Ok(Box::new(StringHandle::new(serde_json::to_string_pretty(&info.models)?)))
            }

            // /compute/jobs/<job_id>/status
            ["jobs", job_id, "status"] => {
                let jobs = self.jobs.read();
                let state = jobs.get(*job_id).ok_or(Error::NotFound)?;
                let status = match state {
                    JobState::Pending { .. } => "pending",
                    JobState::Running { .. } => "running",
                    JobState::Streaming { .. } => "streaming",
                    JobState::Complete(_) => "complete",
                    JobState::Failed { .. } => "failed",
                };
                Ok(Box::new(StringHandle::new(format!(r#"{{"status":"{}"}}"#, status))))
            }

            // /compute/jobs/<job_id>/result
            ["jobs", job_id, "result"] => {
                let jobs = self.jobs.read();
                match jobs.get(*job_id) {
                    Some(JobState::Complete(response)) => {
                        Ok(Box::new(StringHandle::new(serde_json::to_string_pretty(response)?)))
                    }
                    Some(JobState::Failed { error, .. }) => {
                        Err(Error::Failed(error.clone()))
                    }
                    _ => Err(Error::NotReady),
                }
            }

            _ => Err(Error::NotFound),
        }
    }

    fn watch(&self, path: &str) -> Result<Option<Box<dyn WatchHandle>>> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        // /compute/jobs/<job_id>/stream — watch for streaming chunks
        if let ["jobs", job_id, "stream"] = parts.as_slice() {
            return Ok(Some(Box::new(StreamWatchHandle::new(
                self.router.clone(),
                self.jobs.clone(),
                job_id.to_string(),
            ))));
        }

        Ok(None)
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>> {
        match path {
            "" | "/" => Ok(vec![
                DirEntry::dir("providers"),
                DirEntry::file("new"),
                DirEntry::file("policy"),
                DirEntry::file("usage"),
                DirEntry::dir("jobs"),
            ]),
            "/providers" => {
                let router = self.router.read();
                Ok(router.list_providers().iter()
                    .map(|p| DirEntry::dir(&p.id))
                    .collect())
            }
            "/jobs" => {
                let jobs = self.jobs.read();
                Ok(jobs.keys().map(|id| DirEntry::dir(id)).collect())
            }
            _ => Ok(vec![]),
        }
    }

    fn stat(&self, path: &str) -> Result<Stat> {
        // ... standard stat implementation
        todo!()
    }

    fn mkdir(&self, _path: &str) -> Result<()> { Err(Error::ReadOnly) }
    fn remove(&self, _path: &str) -> Result<()> { Err(Error::ReadOnly) }
    fn rename(&self, _from: &str, _to: &str) -> Result<()> { Err(Error::ReadOnly) }
}
```

---

## ComputeNewHandle

Write-then-read pattern for `/compute/new`. Always returns job_id immediately (no blocking):

```rust
/// Handle for /compute/new
struct ComputeNewHandle {
    agent_id: String,
    router: Arc<RwLock<ComputeRouter>>,
    policy: Arc<RwLock<ComputePolicy>>,
    usage: Arc<RwLock<ComputeUsageState>>,
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    journal: Arc<dyn IdempotencyJournal>,

    // State
    request_buf: Vec<u8>,
    response: Option<Vec<u8>>,
    position: u64,
}

impl FileHandle for ComputeNewHandle {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.request_buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // Submit on first read if not yet done
        if self.response.is_none() {
            self.submit_request()?;
        }

        let response = self.response.as_ref().unwrap();
        let pos = self.position as usize;
        if pos >= response.len() {
            return Ok(0);
        }

        let len = std::cmp::min(buf.len(), response.len() - pos);
        buf[..len].copy_from_slice(&response[pos..pos + len]);
        self.position += len as u64;
        Ok(len)
    }

    fn flush(&mut self) -> Result<()> {
        if self.response.is_none() && !self.request_buf.is_empty() {
            self.submit_request()?;
        }
        Ok(())
    }

    fn position(&self) -> u64 { self.position }
    fn close(&mut self) -> Result<()> { Ok(()) }
}

impl ComputeNewHandle {
    fn submit_request(&mut self) -> Result<()> {
        let request: ComputeRequest = serde_json::from_slice(&self.request_buf)
            .map_err(|e| Error::InvalidRequest(e.to_string()))?;

        let policy = self.policy.read();

        // 1. Check if idempotency is required
        if policy.require_idempotency && request.idempotency_key.is_none() {
            return Err(Error::IdempotencyRequired);
        }

        // 2. Build scoped idempotency key: {agent_id}:{provider_id}:{user_key}
        //    (provider_id added after selection)
        let router = self.router.read();
        let provider = router.select(&request, &policy)?;
        let provider_id = provider.id();

        let scoped_key = request.idempotency_key.as_ref().map(|k| {
            format!("{}:{}:{}", self.agent_id, provider_id, k)
        });

        // 3. Check idempotency journal for cached job_id
        if let Some(key) = &scoped_key {
            if let Some(cached) = self.journal.get(key)? {
                self.response = Some(cached);
                return Ok(());
            }
        }

        // 4. Check max_cost_usd requirement
        let max_cost_usd = match (request.max_cost_usd, policy.default_max_cost_usd, policy.require_max_cost) {
            (Some(c), _, _) => c,
            (None, Some(d), _) => d,
            (None, None, true) => return Err(Error::MaxCostRequired),
            (None, None, false) => u64::MAX, // No limit
        };

        // 5. Check budget (uses max_cost_usd as the ceiling)
        let mut usage = self.usage.write();

        if let Some(tick_limit) = policy.max_cost_usd_per_tick {
            if usage.spent_tick_usd + max_cost_usd > tick_limit {
                return Err(Error::BudgetExceeded {
                    limit: "tick",
                    spent_usd: usage.spent_tick_usd,
                    limit_usd: tick_limit,
                    requested_usd: max_cost_usd,
                });
            }
        }

        if let Some(day_limit) = policy.max_cost_usd_per_day {
            if usage.spent_day_usd + max_cost_usd > day_limit {
                return Err(Error::BudgetExceeded {
                    limit: "day",
                    spent_usd: usage.spent_day_usd,
                    limit_usd: day_limit,
                    requested_usd: max_cost_usd,
                });
            }
        }

        // 6. Reserve budget (actual cost deducted on completion)
        usage.spent_tick_usd += max_cost_usd;
        usage.spent_day_usd += max_cost_usd;
        drop(usage); // Release lock before submit

        // 7. Submit to provider (never blocks)
        let job_id = provider.submit(request.clone())?;

        // 8. Track job
        self.jobs.write().insert(job_id.clone(), JobState::Pending {
            submitted_at: Timestamp::now(),
        });

        // 9. Build response
        let response_json = serde_json::json!({
            "job_id": job_id,
            "status": "pending",
            "status_path": format!("/compute/jobs/{}/status", job_id),
            "stream_path": format!("/compute/jobs/{}/stream", job_id),
            "result_path": format!("/compute/jobs/{}/result", job_id),
        });
        let response_bytes = serde_json::to_vec(&response_json)?;

        // 10. Store in idempotency journal (with TTL)
        if let Some(key) = &scoped_key {
            self.journal.put_with_ttl(key, &response_bytes, Duration::from_secs(3600))?;
        }

        self.response = Some(response_bytes);
        Ok(())
    }
}
```

**Key differences from old `/compute/run`:**
1. `submit()` never blocks — returns job_id immediately
2. Budget uses `max_cost_usd` directly (no estimation)
3. Idempotency keys are scoped: `{agent_id}:{provider_id}:{user_key}`
4. Response always includes paths to poll status/stream/result

---

## Idempotency Journal

Prevents double-billing on retries. Keys are **scoped** to prevent cross-agent/cross-provider collisions.

### Key Scoping

Idempotency keys provided by the caller are scoped before storage:

```
{agent_id}:{provider_id}:{user_key}
```

This ensures:
- Agent A's `req_123` doesn't collide with Agent B's `req_123`
- Same request to different providers gets different cache entries
- User-provided keys can be simple (e.g., `"my-request"`)

### TTL

Entries expire after a configurable TTL (default: 1 hour). This prevents:
- Unbounded journal growth
- Stale entries blocking legitimate retries after long delays

```rust
/// Journal for idempotent effect tracking
pub trait IdempotencyJournal: Send + Sync {
    /// Get cached result by scoped key
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, JournalError>;

    /// Store result with scoped key and TTL
    fn put_with_ttl(&self, key: &str, value: &[u8], ttl: Duration) -> Result<(), JournalError>;

    /// Check if key exists (without retrieving value)
    fn contains(&self, key: &str) -> Result<bool, JournalError>;

    /// Remove expired entries (called periodically)
    fn cleanup_expired(&self) -> Result<usize, JournalError>;
}

/// Journal entry with expiration
struct JournalEntry {
    value: Vec<u8>,
    created_at: Instant,
    expires_at: Instant,
}
```

### MemoryJournal

```rust
/// In-memory journal (for testing/local)
pub struct MemoryJournal {
    entries: RwLock<HashMap<String, JournalEntry>>,
    max_entries: usize,
    default_ttl: Duration,
}

impl IdempotencyJournal for MemoryJournal {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, JournalError> {
        let entries = self.entries.read();
        match entries.get(key) {
            Some(entry) if entry.expires_at > Instant::now() => {
                Ok(Some(entry.value.clone()))
            }
            _ => Ok(None), // Expired or not found
        }
    }

    fn put_with_ttl(&self, key: &str, value: &[u8], ttl: Duration) -> Result<(), JournalError> {
        let mut entries = self.entries.write();
        let now = Instant::now();

        // Evict expired entries first
        entries.retain(|_, e| e.expires_at > now);

        // Evict oldest if still at capacity
        if entries.len() >= self.max_entries {
            if let Some(oldest) = entries.iter()
                .min_by_key(|(_, e)| e.created_at)
                .map(|(k, _)| k.clone())
            {
                entries.remove(&oldest);
            }
        }

        entries.insert(key.to_string(), JournalEntry {
            value: value.to_vec(),
            created_at: now,
            expires_at: now + ttl,
        });
        Ok(())
    }

    fn cleanup_expired(&self) -> Result<usize, JournalError> {
        let mut entries = self.entries.write();
        let now = Instant::now();
        let before = entries.len();
        entries.retain(|_, e| e.expires_at > now);
        Ok(before - entries.len())
    }
}
```

### SqliteJournal

```rust
/// SQLite-backed journal (for persistent)
pub struct SqliteJournal {
    conn: Connection,
}

impl SqliteJournal {
    pub fn new(path: &str) -> Result<Self, JournalError> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS idempotency (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL
            )",
            [],
        )?;
        // Create index for cleanup
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_expires ON idempotency(expires_at)",
            [],
        )?;
        Ok(Self { conn })
    }
}

impl IdempotencyJournal for SqliteJournal {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, JournalError> {
        let now = Timestamp::now().as_secs();
        self.conn.query_row(
            "SELECT value FROM idempotency WHERE key = ? AND expires_at > ?",
            [key, &now.to_string()],
            |row| row.get(0),
        ).optional()
    }

    fn put_with_ttl(&self, key: &str, value: &[u8], ttl: Duration) -> Result<(), JournalError> {
        let now = Timestamp::now().as_secs();
        let expires = now + ttl.as_secs();
        self.conn.execute(
            "INSERT OR REPLACE INTO idempotency (key, value, created_at, expires_at) VALUES (?, ?, ?, ?)",
            params![key, value, now, expires],
        )?;
        Ok(())
    }

    fn cleanup_expired(&self) -> Result<usize, JournalError> {
        let now = Timestamp::now().as_secs();
        let deleted = self.conn.execute(
            "DELETE FROM idempotency WHERE expires_at <= ?",
            [now],
        )?;
        Ok(deleted)
    }
}
```

### DoJournal (Cloudflare)

```rust
/// Cloudflare DO storage journal
#[cfg(feature = "cloudflare")]
pub struct DoJournal {
    storage: Storage,
    prefix: String,
}

#[cfg(feature = "cloudflare")]
impl DoJournal {
    /// Store with TTL using DO's built-in expiration
    fn put_with_ttl(&self, key: &str, value: &[u8], ttl: Duration) -> Result<(), JournalError> {
        let prefixed = format!("{}:{}", self.prefix, key);
        // DO storage supports expiration natively
        self.storage.put(&prefixed, value)
            .expiration_ttl(ttl.as_secs() as u32)?;
        Ok(())
    }
}
```

---

## Backend Implementations

### Local Provider

Wraps existing `crates/compute` backends (Ollama, llama.cpp, Apple FM):

```rust
/// Local compute provider
pub struct LocalProvider {
    /// Backend registry from crates/compute
    registry: Arc<BackendRegistry>,
    /// Executor for async bridging
    executor: Arc<ExecutorManager>,
    /// Active jobs
    jobs: Arc<RwLock<HashMap<String, LocalJobState>>>,
}

struct LocalJobState {
    request: ComputeRequest,
    status: JobState,
    stream_rx: Option<mpsc::Receiver<ComputeChunk>>,
}

impl ComputeProvider for LocalProvider {
    fn id(&self) -> &str { "local" }

    fn info(&self) -> ProviderInfo {
        let models = self.executor.execute(async {
            self.registry.list_all_models().await
        }).unwrap_or_default();

        ProviderInfo {
            id: "local".to_string(),
            name: "Local (Ollama/llama.cpp/Apple FM)".to_string(),
            models: models.into_iter().map(|m| ModelInfo {
                id: m.id,
                name: m.name,
                context_length: Some(m.context_length as u32),
                capabilities: vec![ComputeKind::Chat, ComputeKind::Complete],
                pricing: None,  // Free
            }).collect(),
            capabilities: vec![ComputeKind::Chat, ComputeKind::Complete, ComputeKind::Embeddings],
            pricing: None,  // Free
            latency: ProviderLatency {
                ttft_ms: 50,       // Local is fast
                tokens_per_sec: Some(30),
                measured: false,   // Estimate until we measure
            },
            region: Some("local".to_string()),
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        self.executor.execute(async {
            self.registry.has_available_backend().await
        }).unwrap_or(false)
    }

    fn supports_model(&self, model: &str) -> bool {
        // Check registry for model availability
        self.executor.execute(async {
            self.registry.supports_model(model).await
        }).unwrap_or(false)
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        let job_id = generate_job_id();

        // Spawn async execution (doesn't block submit)
        let registry = self.registry.clone();
        let jobs = self.jobs.clone();
        let job_id_clone = job_id.clone();
        let request_clone = request.clone();

        self.executor.spawn(async move {
            let result = execute_local_job(&registry, &request_clone).await;

            let mut jobs = jobs.write();
            if let Some(job) = jobs.get_mut(&job_id_clone) {
                match result {
                    Ok(response) => job.status = JobState::Complete(response),
                    Err(e) => job.status = JobState::Failed {
                        error: e.to_string(),
                        at: Timestamp::now(),
                    },
                }
            }
        });

        // Track job immediately
        self.jobs.write().insert(job_id.clone(), LocalJobState {
            request,
            status: JobState::Pending { submitted_at: Timestamp::now() },
            stream_rx: None,
        });

        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        self.jobs.read().get(job_id).map(|j| j.status.clone())
    }

    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        let mut jobs = self.jobs.write();
        let job = jobs.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;

        if let Some(rx) = &mut job.stream_rx {
            match rx.try_recv() {
                Ok(chunk) => Ok(Some(chunk)),
                Err(mpsc::TryRecvError::Empty) => Ok(None),
                Err(mpsc::TryRecvError::Disconnected) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let mut jobs = self.jobs.write();
        if let Some(job) = jobs.get_mut(job_id) {
            job.status = JobState::Failed {
                error: "Cancelled".to_string(),
                at: Timestamp::now(),
            };
        }
        Ok(())
    }
}

async fn execute_local_job(
    registry: &BackendRegistry,
    request: &ComputeRequest,
) -> Result<ComputeResponse, ComputeError> {
    let start = Instant::now();
    let completion_req = CompletionRequest::new(
        &request.model,
        &extract_prompt(&request.input),
    );

    let response = registry.complete(&request.model, completion_req).await?;

    Ok(ComputeResponse {
        job_id: generate_job_id(),
        output: serde_json::json!({ "text": response.text }),
        usage: response.usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens as u64,
            output_tokens: u.completion_tokens as u64,
            total_tokens: u.total_tokens as u64,
        }),
        cost_usd: 0,  // Local is free
        latency_ms: start.elapsed().as_millis() as u64,
        provider_id: "local".to_string(),
        model: request.model.clone(),
    })
}
```

### Cloudflare Workers AI Provider

```rust
/// Cloudflare Workers AI provider
#[cfg(feature = "cloudflare")]
pub struct CloudflareProvider {
    ai: worker::Ai,
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
}

#[cfg(feature = "cloudflare")]
impl CloudflareProvider {
    pub fn new(ai: worker::Ai) -> Self {
        Self {
            ai,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn model_pricing(model: &str) -> ModelPricing {
        // Cloudflare Workers AI pricing (as of 2025)
        match model {
            m if m.contains("llama") => ModelPricing {
                input_per_1k_microusd: 10,
                output_per_1k_microusd: 10,
            },
            m if m.contains("mistral") => ModelPricing {
                input_per_1k_microusd: 10,
                output_per_1k_microusd: 10,
            },
            _ => ModelPricing {
                input_per_1k_microusd: 50,
                output_per_1k_microusd: 50,
            },
        }
    }
}

#[cfg(feature = "cloudflare")]
impl ComputeProvider for CloudflareProvider {
    fn id(&self) -> &str { "cloudflare" }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "cloudflare".to_string(),
            name: "Cloudflare Workers AI".to_string(),
            models: vec![
                ModelInfo {
                    id: "@cf/meta/llama-2-7b-chat-fp16".to_string(),
                    name: "Llama 2 7B".to_string(),
                    context_length: Some(4096),
                    capabilities: vec![ComputeKind::Chat],
                    pricing: Some(Self::model_pricing("llama")),
                },
                // ... more models
            ],
            capabilities: vec![
                ComputeKind::Chat,
                ComputeKind::Embeddings,
                ComputeKind::ImageGenerate,
            ],
            pricing: Some(ProviderPricing {
                input_per_1k_microusd: 10,
                output_per_1k_microusd: 10,
            }),
            latency: ProviderLatency {
                ttft_ms: 30,        // Edge is fast
                tokens_per_sec: Some(50),
                measured: true,     // Based on CF docs
            },
            region: Some("edge".to_string()),
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool { true }

    fn supports_model(&self, model: &str) -> bool {
        model.starts_with("@cf/")
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        let job_id = generate_job_id();
        let start = Instant::now();

        // Workers AI is sync within DO context—execute immediately
        // (DO guarantees no concurrent execution, so this is safe)
        let output: serde_json::Value = self.ai.run(&request.model, request.input.clone())
            .map_err(|e| ComputeError::ProviderError(e.to_string()))?;

        let response = ComputeResponse {
            job_id: job_id.clone(),
            output,
            usage: None,
            cost_usd: 0, // Tracked at ComputeFs level via pricing
            latency_ms: start.elapsed().as_millis() as u64,
            provider_id: "cloudflare".to_string(),
            model: request.model,
        };

        self.jobs.write().insert(job_id.clone(), JobState::Complete(response));
        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        self.jobs.read().get(job_id).cloned()
    }

    fn poll_stream(&self, _job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        Ok(None) // Cloudflare Workers AI doesn't support streaming
    }

    fn cancel(&self, _job_id: &str) -> Result<(), ComputeError> {
        Ok(()) // No-op for sync execution
    }
}
```

### NIP-90 DVM Provider

Decentralized compute via Nostr DVMs. Uses explicit **quote/accept/settle** lifecycle:

```rust
/// NIP-90 DVM provider for decentralized compute
pub struct DvmProvider {
    relays: Vec<String>,
    nostr_client: Arc<NostrClient>,
    executor: Arc<ExecutorManager>,
    active_jobs: Arc<RwLock<HashMap<String, DvmJobState>>>,
    signer: Arc<dyn SigningService>,
    fx_converter: Arc<dyn FxConverter>,  // For USD↔sats conversion
}

/// FX conversion for DVM payments
pub trait FxConverter: Send + Sync {
    fn usd_to_sats(&self, micro_usd: u64) -> u64;
    fn sats_to_usd(&self, sats: u64) -> u64;
}

/// DVM job state machine
struct DvmJobState {
    /// Our internal job ID
    job_id: String,
    /// Nostr event ID of our request
    request_event_id: String,
    /// Original request
    request: ComputeRequest,
    /// Current lifecycle state
    lifecycle: DvmLifecycle,
    /// Quotes received from DVMs
    quotes: Vec<DvmQuote>,
    /// Accepted quote (if any)
    accepted_quote: Option<DvmQuote>,
    /// Final result
    result: Option<ComputeResponse>,
}

/// DVM lifecycle states (quote/accept/settle pattern)
#[derive(Debug, Clone)]
enum DvmLifecycle {
    /// Request published, waiting for quotes (kind 7000 with status=quote)
    AwaitingQuotes { since: Timestamp, timeout_at: Timestamp },
    /// Quote accepted, waiting for result (kind 7000 with status=processing)
    Processing { accepted_at: Timestamp, provider: String },
    /// Result received, pending payment (kind 6xxx)
    PendingSettlement { result_at: Timestamp, invoice: Option<String> },
    /// Fully settled (payment confirmed)
    Settled { settled_at: Timestamp },
    /// Failed at any stage
    Failed { error: String, at: Timestamp },
}

/// A quote from a DVM
#[derive(Debug, Clone)]
struct DvmQuote {
    /// DVM's pubkey
    provider_pubkey: String,
    /// Quoted price in micro-USD (converted from sats via FX)
    price_usd: u64,
    /// Quoted price in sats (for Lightning payment)
    price_sats: u64,
    /// Quote event ID
    event_id: String,
    /// Estimated completion time (seconds)
    eta_secs: Option<u64>,
}

impl ComputeProvider for DvmProvider {
    fn id(&self) -> &str { "dvm" }

    fn info(&self) -> ProviderInfo {
        let handlers = self.executor.execute(async {
            self.query_dvm_handlers().await
        }).unwrap_or_default();

        ProviderInfo {
            id: "dvm".to_string(),
            name: "NIP-90 DVM Network".to_string(),
            models: handlers.into_iter()
                .flat_map(|h| h.models)
                .collect(),
            capabilities: vec![ComputeKind::Chat, ComputeKind::Complete],
            pricing: None,  // Varies by DVM
            latency: ProviderLatency {
                ttft_ms: 2000,      // Network latency + quote collection
                tokens_per_sec: None,
                measured: false,
            },
            region: None,   // Decentralized
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        !self.relays.is_empty()
    }

    fn supports_model(&self, model: &str) -> bool {
        // DVMs advertise via NIP-89; check handlers
        self.executor.execute(async {
            self.query_dvm_handlers().await
                .map(|h| h.iter().any(|dvm| dvm.models.iter().any(|m| m.id == model)))
                .unwrap_or(false)
        }).unwrap_or(false)
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        let job_id = generate_job_id();
        let max_cost_usd = request.max_cost_usd.unwrap_or(100_000); // Default $0.10

        // Publish NIP-90 job request (kind 5xxx based on ComputeKind)
        let kind = match request.kind {
            ComputeKind::Chat | ComputeKind::Complete => 5050,
            ComputeKind::ImageGenerate => 5100,
            ComputeKind::Transcribe => 5250,
            _ => 5050,
        };

        let request_event = self.executor.execute(async {
            self.create_job_request(kind, &request, max_cost_usd).await
        })?;

        let event_id = self.executor.execute(async {
            self.nostr_client.publish(request_event).await
        })?;

        // Track job in AwaitingQuotes state
        let now = Timestamp::now();
        self.active_jobs.write().insert(job_id.clone(), DvmJobState {
            job_id: job_id.clone(),
            request_event_id: event_id,
            request,
            lifecycle: DvmLifecycle::AwaitingQuotes {
                since: now,
                timeout_at: now + Duration::from_secs(30), // 30s quote window
            },
            quotes: vec![],
            accepted_quote: None,
            result: None,
        });

        // Start background task to collect quotes and manage lifecycle
        self.spawn_lifecycle_manager(job_id.clone());

        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        self.active_jobs.read().get(job_id).map(|state| {
            match &state.lifecycle {
                DvmLifecycle::AwaitingQuotes { .. } => {
                    JobState::Pending { submitted_at: Timestamp::now() }
                }
                DvmLifecycle::Processing { accepted_at, .. } => {
                    JobState::Running { started_at: *accepted_at }
                }
                DvmLifecycle::PendingSettlement { .. } | DvmLifecycle::Settled { .. } => {
                    state.result.clone()
                        .map(JobState::Complete)
                        .unwrap_or(JobState::Running { started_at: Timestamp::now() })
                }
                DvmLifecycle::Failed { error, at } => {
                    JobState::Failed { error: error.clone(), at: *at }
                }
            }
        })
    }

    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        // DVM streaming uses kind 7000 with status=partial
        // Check for new partial results
        let jobs = self.active_jobs.read();
        let _job = jobs.get(job_id).ok_or(ComputeError::JobNotFound)?;
        // TODO: poll for kind 7000 partial events
        Ok(None)
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let mut jobs = self.active_jobs.write();
        if let Some(job) = jobs.get_mut(job_id) {
            job.lifecycle = DvmLifecycle::Failed {
                error: "Cancelled by user".to_string(),
                at: Timestamp::now(),
            };
            // TODO: publish cancellation event
        }
        Ok(())
    }
}

impl DvmProvider {
    /// Create NIP-90 job request event
    async fn create_job_request(
        &self,
        kind: u16,
        request: &ComputeRequest,
        max_cost_usd: u64,
    ) -> Result<NostrEvent, ComputeError> {
        let content = serde_json::to_string(&request.input)?;

        // Convert USD to sats for the bid (DVMs price in sats)
        let max_cost_sats = self.fx_converter.usd_to_sats(max_cost_usd);

        let tags = vec![
            vec!["i".to_string(), content.clone(), "text".to_string()],
            vec!["param".to_string(), "model".to_string(), request.model.clone()],
            // Bid is in sats (DVMs respond with quotes ≤ this)
            vec!["bid".to_string(), max_cost_sats.to_string()],
        ];

        let event = self.signer.sign_event(kind, content, tags)?;
        Ok(event)
    }

    /// Background task that manages the quote/accept/settle lifecycle
    fn spawn_lifecycle_manager(&self, job_id: String) {
        let jobs = self.active_jobs.clone();
        let nostr = self.nostr_client.clone();
        let signer = self.signer.clone();

        self.executor.spawn(async move {
            // Phase 1: Collect quotes (wait for kind 7000 with status=quote)
            tokio::time::sleep(Duration::from_secs(5)).await; // Quote collection window

            let mut jobs_guard = jobs.write();
            let job = match jobs_guard.get_mut(&job_id) {
                Some(j) => j,
                None => return,
            };

            if job.quotes.is_empty() {
                job.lifecycle = DvmLifecycle::Failed {
                    error: "No quotes received".to_string(),
                    at: Timestamp::now(),
                };
                return;
            }

            // Phase 2: Accept cheapest quote (by USD price)
            let best = job.quotes.iter()
                .min_by_key(|q| q.price_usd)
                .cloned()
                .unwrap();

            // Publish acceptance event
            if let Err(e) = Self::publish_acceptance(&nostr, &signer, &best, &job.request_event_id).await {
                job.lifecycle = DvmLifecycle::Failed {
                    error: format!("Failed to accept quote: {}", e),
                    at: Timestamp::now(),
                };
                return;
            }

            job.accepted_quote = Some(best.clone());
            job.lifecycle = DvmLifecycle::Processing {
                accepted_at: Timestamp::now(),
                provider: best.provider_pubkey.clone(),
            };
            drop(jobs_guard);

            // Phase 3: Wait for result (kind 6xxx)
            // This would poll for result events in a real implementation
            // For now, we wait for the result event handler to update state

            // Phase 4: Settlement (pay invoice if required)
            // DVMs may require payment before releasing result
        });
    }

    async fn publish_acceptance(
        nostr: &NostrClient,
        signer: &Arc<dyn SigningService>,
        quote: &DvmQuote,
        request_event_id: &str,
    ) -> Result<(), ComputeError> {
        // Publish a kind 7000 event referencing the quote to accept it
        let tags = vec![
            vec!["e".to_string(), request_event_id.to_string()],
            vec!["e".to_string(), quote.event_id.clone()],
            vec!["p".to_string(), quote.provider_pubkey.clone()],
        ];
        let event = signer.sign_event(7000, "".to_string(), tags)?;
        nostr.publish(event).await?;
        Ok(())
    }

    async fn query_dvm_handlers(&self) -> Result<Vec<DvmHandler>, ComputeError> {
        let filter = Filter::new()
            .kind(31990)
            .custom_tag("k", vec!["5050"]);

        let events = self.nostr_client.query(filter).await?;
        Ok(events.into_iter()
            .filter_map(|e| DvmHandler::from_event(&e))
            .collect())
    }
}
```

**DVM Lifecycle Summary:**

| Phase | Event Kind | Description |
|-------|------------|-------------|
| Request | 5xxx | Agent publishes job request with `bid` tag |
| Quote | 7000 (status=quote) | DVMs respond with price quotes ≤ bid |
| Accept | 7000 (referencing quote) | Agent accepts a quote |
| Processing | 7000 (status=processing) | DVM acknowledges and starts work |
| Result | 6xxx | DVM publishes result (may include invoice) |
| Payment | Lightning | Agent pays invoice if required |
| Settled | — | Job complete, budget updated |

---

## Implementation per Backend

| Feature | Local | Cloudflare | DVM |
|---------|-------|------------|-----|
| Provider | `LocalProvider` | `CloudflareProvider` | `DvmProvider` |
| Execution | `BackendRegistry` | `worker::Ai` | NIP-90 events |
| Streaming | `mpsc::Receiver` | Not supported | Kind 7000 (status=partial) |
| Pricing | Free ($0) | Per-token (micro-USD) | Bid-based (sats→USD) |
| Models | Auto-detected | Fixed catalog | NIP-89 handlers |
| Latency (TTFT) | ~50ms | ~30ms | ~2000ms (includes quotes) |
| Availability | Requires local server | Always available | Requires relays |
| Payment | None | Workers billing | Lightning (sats) |
| Lifecycle | Simple | Simple | Quote/Accept/Settle |

---

## Budget Integration

**REQUIRED: `/compute` MUST be mounted with `AccessLevel::Budgeted`.**

This is enforced at the mount table level. Attempting to mount `/compute` with any other access level is an error.

```rust
// Mount with budget policy (amounts in micro-USD)
namespace.mount("/compute", compute_fs, AccessLevel::Budgeted(BudgetPolicy {
    per_tick_usd: 100_000,         // $0.10 per tick
    per_day_usd: 10_000_000,       // $10 per day
    approval_threshold_usd: 1_000_000, // $1 requires human approval
    approvers: vec![owner_pubkey],
}));
```

**Two-layer enforcement:**

1. **`AccessLevel::Budgeted`** — Mount-level enforcement (same as `/wallet/pay`)
2. **`ComputePolicy`** — Compute-specific limits (models, providers)

Both must pass for a request to succeed. The budget reservation happens at submit time using `max_cost_usd` from the request; actual cost is reconciled when the job completes.

---

## Usage Examples

### Simple Completion

```rust
// Agent code
let request = serde_json::json!({
    "model": "llama-70b",
    "kind": "chat",
    "input": {
        "messages": [{"role": "user", "content": "Hello!"}]
    },
    "max_cost_usd": 100_000,      // $0.10 budget ceiling (micro-USD)
    "idempotency_key": "req_abc123"
});

// Submit request (returns immediately with job_id)
env.write("/compute/new", &serde_json::to_vec(&request)?)?;
let job_info: serde_json::Value = serde_json::from_slice(&env.read("/compute/new")?)?;
let job_id = job_info["job_id"].as_str().unwrap();

// Poll for completion
loop {
    let status_bytes = env.read(&format!("/compute/jobs/{}/status", job_id))?;
    let status: serde_json::Value = serde_json::from_slice(&status_bytes)?;

    if status["status"] == "complete" {
        let result_bytes = env.read(&format!("/compute/jobs/{}/result", job_id))?;
        let response: ComputeResponse = serde_json::from_slice(&result_bytes)?;
        println!("Response: {}", response.output["text"]);
        println!("Cost: ${:.6}", response.cost_usd as f64 / 1_000_000.0);
        break;
    } else if status["status"] == "failed" {
        return Err("Job failed");
    }

    std::thread::sleep(Duration::from_millis(100));
}
```

### Streaming

```rust
// Request with streaming
let request = serde_json::json!({
    "model": "llama-70b",
    "kind": "chat",
    "input": {"messages": [{"role": "user", "content": "Write a story"}]},
    "stream": true,
    "max_cost_usd": 500_000,  // $0.50
    "idempotency_key": "stream_xyz"
});

// Submit
env.write("/compute/new", &serde_json::to_vec(&request)?)?;
let job_info: serde_json::Value = serde_json::from_slice(&env.read("/compute/new")?)?;
let job_id = job_info["job_id"].as_str().unwrap();

// Watch for chunks (only way to stream)
if let Some(mut watch) = env.watch(&format!("/compute/jobs/{}/stream", job_id))? {
    while let Some(event) = watch.next(Some(Duration::from_secs(30)))? {
        if let WatchEvent::Data(data) = event {
            let chunk: ComputeChunk = serde_json::from_slice(&data)?;
            print!("{}", chunk.delta["text"].as_str().unwrap_or(""));

            if chunk.finish_reason.is_some() {
                break;
            }
        }
    }
}
```

### Check Usage

```rust
// Check remaining budget
let usage_bytes = env.read("/compute/usage")?;
let usage: serde_json::Value = serde_json::from_slice(&usage_bytes)?;

// Values are in micro-USD
let tick_spent = usage["tick"]["spent_usd"].as_u64().unwrap_or(0);
let tick_limit = usage["tick"]["limit_usd"].as_u64().unwrap_or(0);
let day_spent = usage["day"]["spent_usd"].as_u64().unwrap_or(0);
let day_limit = usage["day"]["limit_usd"].as_u64().unwrap_or(0);

println!("Tick spent: ${:.4} / ${:.4}",
    tick_spent as f64 / 1_000_000.0,
    tick_limit as f64 / 1_000_000.0);

println!("Day spent: ${:.2} / ${:.2}",
    day_spent as f64 / 1_000_000.0,
    day_limit as f64 / 1_000_000.0);
```

### List Providers

```rust
// List available providers
let providers_bytes = env.read("/compute/providers")?;
let providers: Vec<ProviderInfo> = serde_json::from_slice(&providers_bytes)?;

for p in providers {
    println!("{} ({}): {} models, TTFT={}ms",
        p.name,
        p.id,
        p.models.len(),
        p.latency.ttft_ms);
}
```

### DVM-Specific (with Quote Selection)

```rust
// For DVM, you can observe the quote/accept lifecycle
let request = serde_json::json!({
    "model": "gpt-4-turbo",
    "kind": "chat",
    "input": {"messages": [{"role": "user", "content": "Analyze this code..."}]},
    "max_cost_usd": 500_000,  // $0.50 budget ceiling (micro-USD, converted to sats for DVM bid)
    "idempotency_key": "dvm_analysis_123"
});

env.write("/compute/new", &serde_json::to_vec(&request)?)?;
let job_info: serde_json::Value = serde_json::from_slice(&env.read("/compute/new")?)?;
let job_id = job_info["job_id"].as_str().unwrap();

// For DVM jobs, status progresses through:
// pending -> running (after quote accepted) -> complete (after result + settlement)
// The lifecycle is managed internally by DvmProvider
```
