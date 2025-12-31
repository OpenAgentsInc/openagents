# AI Compute

Portable compute abstraction for agents.

---

## Overview

Agents access AI compute through the `/compute` mount. The runtime provides:

- **Provider abstraction** — Same interface for local, cloud, and decentralized compute
- **Policy enforcement** — Per-agent model restrictions and cost limits
- **Budget tracking** — Micro-USD accounting per tick and per day
- **Idempotency** — Dedup via journal to prevent double-billing
- **Streaming** — Token-by-token output via watch handles

This enables agents to use any AI model without hardcoding providers.

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
│   └── swarm/          # NIP-90 DVM network
│       ├── info
│       ├── models
│       └── health
├── run                  # Write request → read response (router)
├── policy               # Read/write: compute policy JSON
├── usage                # Read: current tick/day usage
└── jobs/
    └── <job_id>/       # Individual job tracking
        ├── status      # Read: running|streaming|complete|failed
        ├── result      # Read: final result (when complete)
        └── stream      # Watch: streaming chunks
```

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
    pub idempotency_key: Option<String>,

    /// Maximum cost in micro-USD (budget check)
    pub max_cost_microusd: Option<u64>,
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

    /// Actual cost in micro-USD
    pub cost_microusd: u64,

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
    pub max_cost_microusd_per_tick: Option<u64>,

    /// Maximum cost per day in micro-USD
    pub max_cost_microusd_per_day: Option<u64>,

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

Mount table configuration:

```yaml
mounts:
  /compute:
    type: compute
    policy:
      allowed_providers: ["local", "cloudflare"]
      blocked_models: ["gpt-4"]  # Explicit blocklist
      prefer: cost
      max_cost_microusd_per_tick: 100000    # $0.10/tick
      max_cost_microusd_per_day: 10000000   # $10/day
      require_idempotency: true
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
    pub region: Option<String>,
    pub status: ProviderStatus,
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

/// Provider pricing defaults (micro-USD per 1k tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricing {
    pub input_per_1k_microusd: u64,
    pub output_per_1k_microusd: u64,
    pub minimum_microusd: u64,
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

    /// Provider info (models, pricing, etc.)
    fn info(&self) -> ProviderInfo;

    /// Check if provider is available
    fn is_available(&self) -> bool;

    /// Check if provider supports a model
    fn supports_model(&self, model: &str) -> bool;

    /// Estimate cost for a request (before execution)
    fn estimate_cost(&self, request: &ComputeRequest) -> Result<u64, ComputeError>;

    /// Execute a compute request
    fn execute(&self, request: ComputeRequest) -> Result<JobHandle, ComputeError>;

    /// Get result of a job by ID
    fn get_job(&self, job_id: &str) -> Option<JobState>;
}

/// Result of starting a compute job
pub enum JobHandle {
    /// Job completed synchronously
    Complete(ComputeResponse),
    /// Job is streaming
    Streaming {
        job_id: String,
        poll: Box<dyn StreamPoll>,
    },
    /// Job is async (check status later)
    Pending { job_id: String },
}

/// Job state for tracking
pub enum JobState {
    Running { started_at: Timestamp },
    Streaming { started_at: Timestamp, chunks_emitted: usize },
    Complete(ComputeResponse),
    Failed { error: String, at: Timestamp },
}
```

---

## StreamPoll Trait

Sync polling interface for streaming (WASM-compatible):

```rust
/// Sync polling interface for streams
pub trait StreamPoll: Send {
    /// Poll for next chunk. Returns None when complete.
    fn poll(&mut self) -> Result<Option<ComputeChunk>, ComputeError>;

    /// Check if stream is complete
    fn is_complete(&self) -> bool;

    /// Cancel the stream
    fn cancel(&mut self);

    /// Get accumulated response (for building final result)
    fn accumulated(&self) -> Option<&ComputeResponse>;
}
```

Implementation pattern:

```rust
/// Stream poll backed by mpsc channel
pub struct ChannelStreamPoll {
    job_id: String,
    rx: mpsc::Receiver<Result<ComputeChunk, ComputeError>>,
    complete: bool,
    accumulated_text: String,
    accumulated_usage: TokenUsage,
}

impl StreamPoll for ChannelStreamPoll {
    fn poll(&mut self) -> Result<Option<ComputeChunk>, ComputeError> {
        if self.complete {
            return Ok(None);
        }

        match self.rx.try_recv() {
            Ok(Ok(chunk)) => {
                // Accumulate
                if let Some(text) = chunk.delta.get("text").and_then(|t| t.as_str()) {
                    self.accumulated_text.push_str(text);
                }
                if let Some(usage) = &chunk.usage {
                    self.accumulated_usage = usage.clone();
                }
                if chunk.finish_reason.is_some() {
                    self.complete = true;
                }
                Ok(Some(chunk))
            }
            Ok(Err(e)) => {
                self.complete = true;
                Err(e)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.complete = true;
                Ok(None)
            }
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn cancel(&mut self) {
        self.complete = true;
        // Drop receiver to signal cancellation
    }
}
```

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
            Prefer::Cost => candidates.into_iter()
                .min_by_key(|p| p.estimate_cost(request).unwrap_or(u64::MAX)),
            Prefer::Latency => candidates.into_iter()
                .min_by_key(|p| p.info().pricing.map(|pr| pr.minimum_microusd).unwrap_or(0)),
            Prefer::Quality | Prefer::Balanced => candidates.into_iter().next(),
        };

        selected
            .map(|p| p.as_ref())
            .ok_or(ComputeError::NoProviderAvailable {
                model: request.model.clone(),
                reason: "Selection failed".into(),
            })
    }

    /// Execute request with routing
    pub fn execute(
        &self,
        request: ComputeRequest,
        policy: &ComputePolicy,
    ) -> Result<JobHandle, ComputeError> {
        let provider = self.select(&request, policy)?;
        provider.execute(request)
    }
}
```

---

## ComputeFs FileService

The `/compute` FileService implementation:

```rust
/// Compute capability as a filesystem
pub struct ComputeFs {
    router: Arc<RwLock<ComputeRouter>>,
    policy: Arc<RwLock<ComputePolicy>>,
    usage: Arc<RwLock<ComputeUsageState>>,
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    journal: Arc<dyn IdempotencyJournal>,
    executor: Arc<ExecutorManager>,
}

/// Usage tracking
pub struct ComputeUsageState {
    pub tick_start: Timestamp,
    pub day_start: Timestamp,
    pub spent_tick_microusd: u64,
    pub spent_day_microusd: u64,
}

impl FileService for ComputeFs {
    fn name(&self) -> &str { "compute" }

    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        match parts.as_slice() {
            // /compute/run — execute compute request
            ["run"] if flags.write => {
                Ok(Box::new(ComputeRunHandle::new(
                    self.router.clone(),
                    self.policy.clone(),
                    self.usage.clone(),
                    self.jobs.clone(),
                    self.journal.clone(),
                    self.executor.clone(),
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

            // /compute/usage — current usage stats
            ["usage"] => {
                let usage = self.usage.read();
                let policy = self.policy.read();
                let json = serde_json::json!({
                    "tick": {
                        "spent_microusd": usage.spent_tick_microusd,
                        "limit_microusd": policy.max_cost_microusd_per_tick,
                        "remaining_microusd": policy.max_cost_microusd_per_tick
                            .map(|l| l.saturating_sub(usage.spent_tick_microusd)),
                    },
                    "day": {
                        "spent_microusd": usage.spent_day_microusd,
                        "limit_microusd": policy.max_cost_microusd_per_day,
                        "remaining_microusd": policy.max_cost_microusd_per_day
                            .map(|l| l.saturating_sub(usage.spent_day_microusd)),
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
                DirEntry::file("run"),
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

## ComputeRunHandle

Write-then-read pattern for `/compute/run`:

```rust
/// Handle for /compute/run
struct ComputeRunHandle {
    router: Arc<RwLock<ComputeRouter>>,
    policy: Arc<RwLock<ComputePolicy>>,
    usage: Arc<RwLock<ComputeUsageState>>,
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    journal: Arc<dyn IdempotencyJournal>,
    executor: Arc<ExecutorManager>,

    // State
    request_buf: Vec<u8>,
    response: Option<Vec<u8>>,
    position: u64,
}

impl FileHandle for ComputeRunHandle {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.request_buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // Execute on first read if response not yet computed
        if self.response.is_none() {
            self.execute_request()?;
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
        // Execute request on flush (for explicit trigger)
        if self.response.is_none() && !self.request_buf.is_empty() {
            self.execute_request()?;
        }
        Ok(())
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        // ... standard seek implementation
        todo!()
    }

    fn position(&self) -> u64 { self.position }
    fn close(&mut self) -> Result<()> { Ok(()) }
}

impl ComputeRunHandle {
    fn execute_request(&mut self) -> Result<()> {
        let request: ComputeRequest = serde_json::from_slice(&self.request_buf)
            .map_err(|e| Error::InvalidRequest(e.to_string()))?;

        // 1. Check idempotency journal
        if let Some(key) = &request.idempotency_key {
            if let Some(cached) = self.journal.get(key)? {
                self.response = Some(cached);
                return Ok(());
            }
        }

        let policy = self.policy.read();

        // 2. Check if idempotency is required
        if policy.require_idempotency && request.idempotency_key.is_none() {
            return Err(Error::IdempotencyRequired);
        }

        // 3. Select provider and estimate cost
        let router = self.router.read();
        let provider = router.select(&request, &policy)?;
        let estimated_cost = provider.estimate_cost(&request)?;

        // 4. Check budget
        let mut usage = self.usage.write();

        if let Some(tick_limit) = policy.max_cost_microusd_per_tick {
            if usage.spent_tick_microusd + estimated_cost > tick_limit {
                return Err(Error::BudgetExceeded {
                    limit: "tick",
                    spent: usage.spent_tick_microusd,
                    limit_value: tick_limit,
                    requested: estimated_cost,
                });
            }
        }

        if let Some(day_limit) = policy.max_cost_microusd_per_day {
            if usage.spent_day_microusd + estimated_cost > day_limit {
                return Err(Error::BudgetExceeded {
                    limit: "day",
                    spent: usage.spent_day_microusd,
                    limit_value: day_limit,
                    requested: estimated_cost,
                });
            }
        }

        // 5. Execute
        let job_handle = provider.execute(request.clone())?;

        let response_bytes = match job_handle {
            JobHandle::Complete(response) => {
                // Update usage
                usage.spent_tick_microusd += response.cost_microusd;
                usage.spent_day_microusd += response.cost_microusd;

                serde_json::to_vec_pretty(&response)?
            }
            JobHandle::Streaming { job_id, poll } => {
                // Store streaming job for watch
                self.jobs.write().insert(job_id.clone(), JobState::Streaming {
                    started_at: Timestamp::now(),
                    chunks_emitted: 0,
                });

                // Return job info (client should watch /compute/jobs/<id>/stream)
                serde_json::to_vec(&serde_json::json!({
                    "job_id": job_id,
                    "status": "streaming",
                    "stream_path": format!("/compute/jobs/{}/stream", job_id),
                }))?
            }
            JobHandle::Pending { job_id } => {
                self.jobs.write().insert(job_id.clone(), JobState::Running {
                    started_at: Timestamp::now(),
                });

                serde_json::to_vec(&serde_json::json!({
                    "job_id": job_id,
                    "status": "pending",
                    "status_path": format!("/compute/jobs/{}/status", job_id),
                }))?
            }
        };

        // 6. Store in idempotency journal
        if let Some(key) = &request.idempotency_key {
            self.journal.put(key, &response_bytes)?;
        }

        self.response = Some(response_bytes);
        Ok(())
    }
}
```

---

## Idempotency Journal

Prevents double-billing on retries:

```rust
/// Journal for idempotent effect tracking
pub trait IdempotencyJournal: Send + Sync {
    /// Get cached result by idempotency key
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, JournalError>;

    /// Store result with idempotency key
    fn put(&self, key: &str, value: &[u8]) -> Result<(), JournalError>;

    /// Check if key exists (without retrieving value)
    fn contains(&self, key: &str) -> Result<bool, JournalError>;

    /// Remove expired entries
    fn cleanup(&self, max_age: Duration) -> Result<usize, JournalError>;
}
```

### MemoryJournal

```rust
/// In-memory journal (for testing/local)
pub struct MemoryJournal {
    entries: RwLock<HashMap<String, (Vec<u8>, Instant)>>,
    max_entries: usize,
}

impl IdempotencyJournal for MemoryJournal {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, JournalError> {
        let entries = self.entries.read();
        Ok(entries.get(key).map(|(v, _)| v.clone()))
    }

    fn put(&self, key: &str, value: &[u8]) -> Result<(), JournalError> {
        let mut entries = self.entries.write();

        // Evict oldest if at capacity
        if entries.len() >= self.max_entries {
            if let Some(oldest) = entries.iter()
                .min_by_key(|(_, (_, t))| t)
                .map(|(k, _)| k.clone())
            {
                entries.remove(&oldest);
            }
        }

        entries.insert(key.to_string(), (value.to_vec(), Instant::now()));
        Ok(())
    }

    fn cleanup(&self, max_age: Duration) -> Result<usize, JournalError> {
        let mut entries = self.entries.write();
        let cutoff = Instant::now() - max_age;
        let before = entries.len();
        entries.retain(|_, (_, t)| *t > cutoff);
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
                created_at INTEGER NOT NULL
            )",
            [],
        )?;
        Ok(Self { conn })
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
}

impl LocalProvider {
    pub fn new(executor: Arc<ExecutorManager>) -> Self {
        let mut registry = BackendRegistry::new();

        // Auto-detect available backends
        executor.execute(async {
            registry.auto_detect().await;
        });

        Self {
            registry: Arc::new(registry),
            executor,
        }
    }
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
            pricing: Some(ProviderPricing {
                input_per_1k_microusd: 0,
                output_per_1k_microusd: 0,
                minimum_microusd: 0,
            }),
            region: Some("local".to_string()),
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        self.executor.execute(async {
            self.registry.has_available_backend().await
        }).unwrap_or(false)
    }

    fn estimate_cost(&self, _request: &ComputeRequest) -> Result<u64, ComputeError> {
        Ok(0)  // Local is free
    }

    fn execute(&self, request: ComputeRequest) -> Result<JobHandle, ComputeError> {
        let completion_req = CompletionRequest::new(
            &request.model,
            &extract_prompt(&request.input),
        );

        if request.stream {
            let rx = self.executor.execute(async {
                self.registry.complete_stream(&request.model, completion_req).await
            })?;

            let job_id = generate_job_id();
            Ok(JobHandle::Streaming {
                job_id: job_id.clone(),
                poll: Box::new(ChannelStreamPoll::new(job_id, rx)),
            })
        } else {
            let start = Instant::now();
            let response = self.executor.execute(async {
                self.registry.complete(&request.model, completion_req).await
            })?;

            Ok(JobHandle::Complete(ComputeResponse {
                job_id: generate_job_id(),
                output: serde_json::json!({ "text": response.text }),
                usage: response.usage.map(|u| TokenUsage {
                    input_tokens: u.prompt_tokens as u64,
                    output_tokens: u.completion_tokens as u64,
                    total_tokens: u.total_tokens as u64,
                }),
                cost_microusd: 0,
                latency_ms: start.elapsed().as_millis() as u64,
                provider_id: "local".to_string(),
                model: request.model,
            }))
        }
    }
}
```

### Cloudflare Workers AI Provider

```rust
/// Cloudflare Workers AI provider
#[cfg(feature = "cloudflare")]
pub struct CloudflareProvider {
    ai: worker::Ai,
}

#[cfg(feature = "cloudflare")]
impl CloudflareProvider {
    pub fn new(ai: worker::Ai) -> Self {
        Self { ai }
    }

    fn model_pricing(model: &str) -> ModelPricing {
        // Cloudflare Workers AI pricing (as of 2025)
        // https://developers.cloudflare.com/workers-ai/platform/pricing/
        match model {
            m if m.contains("llama") => ModelPricing {
                input_per_1k_microusd: 10,   // $0.00001/token
                output_per_1k_microusd: 10,
            },
            m if m.contains("mistral") => ModelPricing {
                input_per_1k_microusd: 10,
                output_per_1k_microusd: 10,
            },
            _ => ModelPricing {
                input_per_1k_microusd: 50,   // Default/unknown
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
                ModelInfo {
                    id: "@cf/mistral/mistral-7b-instruct-v0.1".to_string(),
                    name: "Mistral 7B".to_string(),
                    context_length: Some(8192),
                    capabilities: vec![ComputeKind::Chat],
                    pricing: Some(Self::model_pricing("mistral")),
                },
                // ... more models
            ],
            capabilities: vec![
                ComputeKind::Chat,
                ComputeKind::Embeddings,
                ComputeKind::ImageGenerate,
                ComputeKind::Transcribe,
            ],
            pricing: Some(ProviderPricing {
                input_per_1k_microusd: 10,
                output_per_1k_microusd: 10,
                minimum_microusd: 0,
            }),
            region: Some("edge".to_string()),
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool { true }

    fn supports_model(&self, model: &str) -> bool {
        model.starts_with("@cf/")
    }

    fn estimate_cost(&self, request: &ComputeRequest) -> Result<u64, ComputeError> {
        let pricing = Self::model_pricing(&request.model);
        // Rough estimate: assume ~100 input tokens, ~500 output tokens
        let estimated_input = 100u64;
        let estimated_output = 500u64;
        Ok(
            (estimated_input * pricing.input_per_1k_microusd / 1000) +
            (estimated_output * pricing.output_per_1k_microusd / 1000)
        )
    }

    fn execute(&self, request: ComputeRequest) -> Result<JobHandle, ComputeError> {
        let start = Instant::now();

        // Workers AI doesn't expose streaming in the same way
        let output: serde_json::Value = self.ai.run(&request.model, request.input.clone())
            .map_err(|e| ComputeError::ProviderError(e.to_string()))?;

        Ok(JobHandle::Complete(ComputeResponse {
            job_id: generate_job_id(),
            output,
            usage: None,  // Workers AI doesn't expose token counts
            cost_microusd: self.estimate_cost(&request)?,
            latency_ms: start.elapsed().as_millis() as u64,
            provider_id: "cloudflare".to_string(),
            model: request.model,
        }))
    }
}
```

### NIP-90 DVM Provider

```rust
/// NIP-90 DVM provider for decentralized compute
pub struct DvmProvider {
    relays: Vec<String>,
    nostr_client: Arc<NostrClient>,
    executor: Arc<ExecutorManager>,
    active_jobs: Arc<RwLock<HashMap<String, DvmJobState>>>,
    signer: Arc<dyn SigningService>,
}

struct DvmJobState {
    request_event_id: String,
    status: DvmJobStatus,
    result: Option<ComputeResponse>,
    provider_pubkey: Option<String>,
}

enum DvmJobStatus {
    Submitted,
    Accepted { by: String },
    Processing,
    Complete,
    Failed { reason: String },
}

impl ComputeProvider for DvmProvider {
    fn id(&self) -> &str { "swarm" }

    fn info(&self) -> ProviderInfo {
        // Query NIP-89 handler info from relays
        let handlers = self.executor.execute(async {
            self.query_dvm_handlers().await
        }).unwrap_or_default();

        ProviderInfo {
            id: "swarm".to_string(),
            name: "NIP-90 DVM Swarm".to_string(),
            models: handlers.into_iter()
                .flat_map(|h| h.models)
                .collect(),
            capabilities: vec![ComputeKind::Chat, ComputeKind::Complete],
            pricing: None,  // Varies by DVM provider
            region: None,   // Decentralized
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        !self.relays.is_empty()
    }

    fn estimate_cost(&self, request: &ComputeRequest) -> Result<u64, ComputeError> {
        // Query DVMs for quotes or use default estimate
        // NIP-90 DVMs bid on jobs, so cost is determined at execution
        Ok(request.max_cost_microusd.unwrap_or(100_000))  // Default $0.10
    }

    fn execute(&self, request: ComputeRequest) -> Result<JobHandle, ComputeError> {
        // Create NIP-90 job request (kind 5050 for text generation)
        let job_id = generate_job_id();

        let job_event = self.executor.execute(async {
            self.create_job_request(&job_id, &request).await
        })?;

        // Publish to relays
        let event_id = self.executor.execute(async {
            self.nostr_client.publish(job_event).await
        })?;

        // Track job
        self.active_jobs.write().insert(job_id.clone(), DvmJobState {
            request_event_id: event_id,
            status: DvmJobStatus::Submitted,
            result: None,
            provider_pubkey: None,
        });

        // Return pending handle - client polls for result
        Ok(JobHandle::Pending { job_id })
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        let jobs = self.active_jobs.read();
        jobs.get(job_id).map(|state| match &state.status {
            DvmJobStatus::Submitted | DvmJobStatus::Accepted { .. } | DvmJobStatus::Processing => {
                JobState::Running { started_at: Timestamp::now() }
            }
            DvmJobStatus::Complete => {
                state.result.clone().map(JobState::Complete)
                    .unwrap_or(JobState::Running { started_at: Timestamp::now() })
            }
            DvmJobStatus::Failed { reason } => {
                JobState::Failed { error: reason.clone(), at: Timestamp::now() }
            }
        })
    }
}

impl DvmProvider {
    async fn create_job_request(
        &self,
        job_id: &str,
        request: &ComputeRequest,
    ) -> Result<NostrEvent, ComputeError> {
        // NIP-90 job request structure
        // Kind 5050 = text generation
        let content = serde_json::to_string(&request.input)?;

        let tags = vec![
            vec!["i".to_string(), content, "text".to_string()],
            vec!["param".to_string(), "model".to_string(), request.model.clone()],
            vec!["bid".to_string(), request.max_cost_microusd
                .map(|c| (c / 100).to_string())  // Convert to sats
                .unwrap_or("1000".to_string())],
        ];

        // Sign with agent's key
        let event = self.signer.sign_event(5050, content, tags)?;
        Ok(event)
    }

    async fn query_dvm_handlers(&self) -> Result<Vec<DvmHandler>, ComputeError> {
        // Query NIP-89 handler info (kind 31990)
        let filter = Filter::new()
            .kind(31990)
            .custom_tag("k", vec!["5050"]);  // Text generation handlers

        let events = self.nostr_client.query(filter).await?;

        Ok(events.into_iter()
            .filter_map(|e| DvmHandler::from_event(&e))
            .collect())
    }
}
```

---

## Implementation per Backend

| Feature | Local | Cloudflare | DVM (Swarm) |
|---------|-------|------------|-------------|
| Provider | `LocalProvider` | `CloudflareProvider` | `DvmProvider` |
| Execution | `BackendRegistry` | `worker::Ai` | NIP-90 events |
| Streaming | `mpsc::Receiver` | Not supported | Kind 7000 events |
| Cost | Free (local compute) | Per-token pricing | Bid-based |
| Models | Auto-detected | Fixed catalog | Handler-advertised |
| Latency | Lowest | Edge (10-50ms) | Variable (network) |
| Availability | Requires local server | Always available | Requires relays |
| Payment | None | Workers billing | Lightning (sats) |

---

## Budget Integration

ComputeFs integrates with runtime budget system:

```rust
// Mount with budget policy
namespace.mount("/compute", compute_fs, AccessLevel::Budgeted(BudgetPolicy {
    per_tick_sats: 100,        // ~$0.10 at $100k/BTC
    per_day_sats: 10000,       // ~$10/day
    approval_threshold_sats: 1000,
    approvers: vec![owner_pubkey],
}));
```

The `AccessLevel::Budgeted` controls mount-level access, while `ComputePolicy` provides compute-specific limits (models, providers, micro-USD costs).

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
    "idempotency_key": "req_abc123"
});

// Write request
env.write("/compute/run", &serde_json::to_vec(&request)?)?;

// Read response
let response_bytes = env.read("/compute/run")?;
let response: ComputeResponse = serde_json::from_slice(&response_bytes)?;

println!("Response: {}", response.output["text"]);
println!("Cost: {} micro-USD", response.cost_microusd);
```

### Streaming

```rust
// Request with streaming
let request = serde_json::json!({
    "model": "llama-70b",
    "kind": "chat",
    "input": {"messages": [{"role": "user", "content": "Write a story"}]},
    "stream": true,
    "idempotency_key": "stream_xyz"
});

env.write("/compute/run", &serde_json::to_vec(&request)?)?;

// Get job info
let job_info: serde_json::Value = serde_json::from_slice(&env.read("/compute/run")?)?;
let job_id = job_info["job_id"].as_str().unwrap();

// Watch for chunks
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

println!("Tick spent: {} / {} micro-USD",
    usage["tick"]["spent_microusd"],
    usage["tick"]["limit_microusd"]);
```

### List Providers

```rust
// List available providers
let providers_bytes = env.read("/compute/providers")?;
let providers: Vec<ProviderInfo> = serde_json::from_slice(&providers_bytes)?;

for p in providers {
    println!("{}: {} models available", p.name, p.models.len());
}
```
