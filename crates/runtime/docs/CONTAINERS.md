# Containers

Portable container abstraction for agents.

---

## Overview

Agents access isolated compute environments through the `/containers` mount. The runtime provides:

- **Provider abstraction** — Same interface for local Docker/Apple Container, cloud sandboxes, and decentralized DVMs
- **Policy enforcement** — Per-agent container restrictions and resource limits
- **Budget tracking** — USD-denominated accounting (settled in sats via Lightning for DVMs)
- **OpenAgents API auth** — Nostr identity or API keys for cloud providers
- **Streaming output** — Real-time stdout/stderr via `watch()` on session streams

This enables agents to spawn isolated execution environments without hardcoding providers.

---

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Budget units | **USD** (micro-USD for precision) | Universal pricing language; settled in sats for DVM payments |
| Execution model | **Always returns session_id** | Non-blocking; works on all backends including Cloudflare |
| Exec model | **Job-based (exec_id)** | Non-blocking; interactive commands also return immediately |
| Auth methods | **Nostr + API keys** | Nostr fits identity model, API keys for compatibility |
| Interface | **`/containers/new` → session_id** | Avoids blocking; agent polls status or watches output |
| Provider naming | **dvm** (not "swarm") | Consistent with NIP-90 terminology |
| File paths | **URL-encoded, no traversal** | Portable, secure, unambiguous |

---

## Filesystem Layout

```
/containers/
├── providers/           # Available container providers
│   ├── apple/          # Apple Container (macOS 26+)
│   ├── local/          # Docker on local machine
│   │   ├── info        # Read: provider info JSON
│   │   ├── images      # Read: available images
│   │   └── health      # Read: health status
│   ├── cloudflare/     # Cloudflare Containers
│   │   ├── info
│   │   ├── images
│   │   └── health
│   ├── daytona/        # Daytona SDK sandboxes
│   │   ├── info
│   │   ├── images
│   │   └── health
│   └── dvm/            # NIP-90 DVM network
│       ├── info
│       ├── handlers    # Available DVM handlers
│       └── health
├── new                  # Call (write+read same handle) → session_id (always async)
├── policy               # Read-only to agents (control plane updates)
├── usage                # Read: current tick/day usage (reserved/spent USD)
├── auth/               # OpenAgents API authentication
│   ├── status          # Read: auth state JSON (never exposes token)
│   ├── token           # Write-only: set API token (stored in secret store)
│   ├── challenge       # Read: Nostr auth challenge, Write: signed response
│   └── credits         # Read: available credits balance (micro-USD)
└── sessions/
    └── <session_id>/   # Individual container session
        ├── status      # Read: provisioning|cloning|running|complete|failed
        ├── result      # Read: final result (when complete)
        ├── output      # Watch: streaming stdout/stderr
        ├── exec/       # Interactive command execution (job-based)
        │   ├── new     # Call (write+read same handle) → exec_id
        │   └── <exec_id>/
        │       ├── status   # Read: pending|running|complete|failed
        │       ├── result   # Read: command result
        │       └── output   # Watch: streaming output
        ├── files/      # Read/write files in container
        │   └── <url-encoded-path>
        ├── usage       # Read: resource consumption + cost reconciliation
        └── ctl         # Write: "stop" to terminate
```

**Key principles:**

1. **`/containers/new` always returns a session_id immediately.** Use `env.call("/containers/new", ...)`
   or open a read/write handle so the response is read from the same handle. The agent then reads
   `/containers/sessions/<id>/status` to poll or watches `/containers/sessions/<id>/output` for streaming.

2. **`/containers/sessions/<id>/exec/new` always returns an exec_id immediately.** Use the same
   call pattern (single handle). Interactive commands are also non-blocking—poll status or watch output.

3. **File paths are URL-encoded.** The path `/containers/sessions/<id>/files/src%2Fmain.rs` accesses `src/main.rs` in the container.

**Apple Container provider:** Available on macOS 26+ when the runtime is built with the `apple-container` feature. Uses the `container` CLI (`container system status`) for availability checks.

---

## Core Types

### ContainerKind

```rust
/// Kind of container operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerKind {
    /// Execute commands in ephemeral container (destroyed after completion)
    Ephemeral,
    /// Long-running sandbox with shell access
    Interactive,
    /// Build/CI container with artifact extraction
    Build,
    /// Custom container type
    Custom(String),
}
```

### ResourceLimits

```rust
/// Resource limits for container execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum execution time in seconds (default: 300)
    #[serde(default = "default_max_time")]
    pub max_time_secs: u32,

    /// Maximum memory in MB (default: 1024)
    #[serde(default = "default_max_memory")]
    pub max_memory_mb: u32,

    /// Maximum disk usage in MB (default: 512)
    #[serde(default = "default_max_disk")]
    pub max_disk_mb: u32,

    /// Maximum CPU cores, 1.0 = one core (default: 1.0)
    #[serde(default = "default_max_cpu")]
    pub max_cpu_cores: f32,

    /// Whether network access is allowed (default: false)
    #[serde(default)]
    pub allow_network: bool,
}

impl ResourceLimits {
    /// Basic preset: 5 min, 1GB RAM, 512MB disk, 1 core, no network
    pub fn basic() -> Self {
        Self {
            max_time_secs: 300,
            max_memory_mb: 1024,
            max_disk_mb: 512,
            max_cpu_cores: 1.0,
            allow_network: false,
        }
    }

    /// Build preset: 10 min, 4GB RAM, 2GB disk, 2 cores, network enabled
    pub fn for_build() -> Self {
        Self {
            max_time_secs: 600,
            max_memory_mb: 4096,
            max_disk_mb: 2048,
            max_cpu_cores: 2.0,
            allow_network: true,
        }
    }
}
```

### ContainerRequest

```rust
/// A container request (provider-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRequest {
    /// Kind of container operation
    pub kind: ContainerKind,

    /// Container image (e.g., "node:20", "python:3.12", "rust:1.75")
    pub image: Option<String>,

    /// Git repository to clone (optional)
    pub repo: Option<RepoConfig>,

    /// Commands to execute (in order)
    pub commands: Vec<String>,

    /// Working directory inside container
    pub workdir: Option<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Resource limits
    #[serde(default = "ResourceLimits::basic")]
    pub limits: ResourceLimits,

    /// Maximum cost in micro-USD the caller is willing to pay.
    /// Budget check reserves this amount; actual cost reconciled on completion.
    /// If omitted, uses policy default or rejects if policy.require_max_cost.
    pub max_cost_usd: Option<u64>,

    /// Idempotency key for deduplication
    /// Full key is scoped: {agent_id}:{provider_id}:{idempotency_key}
    pub idempotency_key: Option<String>,

    /// Timeout in milliseconds (default: 300000 = 5 min)
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    /// Git repository URL
    pub url: String,
    /// Branch, tag, or commit to checkout
    pub git_ref: String,
    /// Subdirectory to use as workdir (optional)
    pub subdir: Option<String>,
    /// Authentication for private repos (uses secret store path)
    pub auth: Option<RepoAuth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RepoAuth {
    /// Token-based authentication
    Token {
        /// Path to secret in agent's secret store (e.g., "/secrets/github_token")
        secret_path: String,
    },
    /// SSH key authentication
    Ssh {
        /// Path to SSH key in secret store
        secret_path: String,
    },
}
```

### ContainerResponse

```rust
/// Container response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerResponse {
    /// Session identifier
    pub session_id: String,

    /// Exit code (0 = success, None if still running)
    pub exit_code: Option<i32>,

    /// Combined stdout
    pub stdout: String,

    /// Combined stderr
    pub stderr: String,

    /// Per-command results (if multiple commands)
    pub command_results: Vec<CommandResult>,

    /// Artifact hashes (for build containers)
    pub artifacts: Vec<ArtifactInfo>,

    /// Resource usage
    pub usage: ContainerUsage,

    /// Actual cost in micro-USD (post-execution)
    /// This is the reconciled cost; reserved_usd - cost_usd is refunded to budget.
    pub cost_usd: u64,

    /// Reserved cost (from max_cost_usd at submission)
    pub reserved_usd: u64,

    /// Execution duration in milliseconds
    pub duration_ms: u64,

    /// Provider that handled the request
    pub provider_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerUsage {
    pub cpu_time_ms: u64,
    pub peak_memory_bytes: u64,
    pub disk_writes_bytes: u64,
    pub network_bytes: u64,
}
```

### ContainerStatus

```rust
/// Container session status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerStatus {
    /// Container is being provisioned
    Provisioning,
    /// Cloning repository
    Cloning,
    /// Container is running
    Running,
    /// Execution complete
    Complete,
    /// Execution failed
    Failed { error: String },
    /// Session expired or cleaned up
    Expired,
}
```

---

## ContainerPolicy

Per-agent policy configured via mount table:

```rust
/// Policy for container operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPolicy {
    /// Allowed provider IDs (empty = all available)
    #[serde(default)]
    pub allowed_providers: Vec<String>,

    /// Allowed images (glob patterns supported, e.g., "node:*", "python:3.*")
    #[serde(default)]
    pub allowed_images: Vec<String>,

    /// Blocked images (takes precedence over allowed)
    #[serde(default)]
    pub blocked_images: Vec<String>,

    /// Whether network access can be enabled
    #[serde(default = "default_true")]
    pub allow_network: bool,

    /// Maximum execution time allowed (seconds)
    #[serde(default = "default_max_time_policy")]
    pub max_execution_time_secs: u32,

    /// Maximum memory allowed (MB)
    #[serde(default = "default_max_memory_policy")]
    pub max_memory_mb: u32,

    /// Maximum concurrent containers per agent
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,

    /// Maximum cost per tick in micro-USD
    pub max_cost_usd_per_tick: Option<u64>,

    /// Maximum cost per day in micro-USD
    pub max_cost_usd_per_day: Option<u64>,

    /// Default max_cost_usd if request doesn't specify
    pub default_max_cost_usd: Option<u64>,

    /// Require requests to specify max_cost_usd
    #[serde(default)]
    pub require_max_cost: bool,

    /// Require OpenAgents API authentication for all providers
    #[serde(default)]
    pub require_api_auth: bool,

    /// Selection preference
    #[serde(default)]
    pub prefer: Prefer,

    /// Require idempotency keys for all requests
    #[serde(default)]
    pub require_idempotency: bool,

    /// Maximum file size for read/write operations (bytes)
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: u64,
}

fn default_max_time_policy() -> u32 { 600 }       // 10 minutes
fn default_max_memory_policy() -> u32 { 4096 }    // 4 GB
fn default_max_concurrent() -> u32 { 3 }
fn default_max_file_size() -> u64 { 10_485_760 }  // 10 MB
```

---

## ContainerProvider Trait

Sync trait for FileService compatibility. Note the **distinct type name** to avoid collision with compute's `ProviderInfo`.

```rust
/// Information about a container provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerProviderInfo {
    pub id: String,
    pub name: String,
    pub available_images: Vec<String>,
    /// Capability flags for programmatic checking
    pub capabilities: ContainerCapabilities,
    pub pricing: Option<ContainerPricing>,
    pub latency: ContainerLatency,
    pub limits: ContainerLimits,
    pub status: ProviderStatus,
}

/// Capability flags for container providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerCapabilities {
    /// Can clone git repositories
    pub git_clone: bool,
    /// Can read/write files in container
    pub file_access: bool,
    /// Supports interactive shell (exec)
    pub interactive: bool,
    /// Can extract build artifacts
    pub artifacts: bool,
    /// Supports output streaming
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPricing {
    /// Cost to start container (micro-USD)
    pub startup_usd: u64,
    /// Cost per second of execution (micro-USD)
    pub per_second_usd: u64,
    /// Cost per GB of network transfer (micro-USD)
    pub network_per_gb_usd: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLatency {
    /// Expected container startup time in milliseconds
    pub startup_ms: u64,
    /// Is this measured (true) or estimated (false)?
    pub measured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLimits {
    pub max_memory_mb: u32,
    pub max_cpu_cores: f32,
    pub max_disk_mb: u32,
    pub max_time_secs: u32,
    pub network_allowed: bool,
}

/// Container provider trait (sync for FileService compatibility)
pub trait ContainerProvider: Send + Sync {
    /// Provider identifier
    fn id(&self) -> &str;

    /// Provider info (images, pricing, limits, capabilities)
    fn info(&self) -> ContainerProviderInfo;

    /// Check if provider is available
    fn is_available(&self) -> bool;

    /// Submit a container request. ALWAYS returns session_id immediately.
    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError>;

    /// Get current state of a session by ID
    fn get_session(&self, session_id: &str) -> Option<SessionState>;

    /// Submit command for execution. Returns exec_id immediately (non-blocking).
    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError>;

    /// Get state of an exec job
    fn get_exec(&self, exec_id: &str) -> Option<ExecState>;

    /// Poll exec output stream
    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError>;

    /// Cancel an exec job
    fn cancel_exec(&self, exec_id: &str) -> Result<(), ContainerError>;

    /// Read file from container (may return NotSupported error)
    fn read_file(&self, session_id: &str, path: &str, offset: u64, len: u64) -> Result<Vec<u8>, ContainerError>;

    /// Write file to container (may return NotSupported error)
    fn write_file(&self, session_id: &str, path: &str, data: &[u8]) -> Result<(), ContainerError>;

    /// Stop a running container
    fn stop(&self, session_id: &str) -> Result<(), ContainerError>;

    /// Poll for output stream (internal use by ContainerFs)
    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError>;
}

/// Session state for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionState {
    /// Session created, container being provisioned
    Provisioning { started_at: Timestamp },
    /// Cloning repository
    Cloning { started_at: Timestamp, repo_url: String },
    /// Container is running
    Running { started_at: Timestamp, commands_completed: usize },
    /// Session completed successfully
    Complete(ContainerResponse),
    /// Session failed
    Failed { error: String, at: Timestamp },
    /// Session expired (cleaned up)
    Expired { at: Timestamp },
}

/// Exec job state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecState {
    Pending { submitted_at: Timestamp },
    Running { started_at: Timestamp },
    Complete(CommandResult),
    Failed { error: String, at: Timestamp },
}
```

---

## File Access Semantics

### Path Encoding

File paths in `/containers/sessions/<id>/files/<path>` use URL encoding:

- `src/main.rs` → `src%2Fmain.rs`
- `path with spaces/file.txt` → `path%20with%20spaces%2Ffile.txt`
- Percent-encode: `/`, ` `, `%`, and non-ASCII characters

**Security rules:**
- `..` traversal is **rejected** (returns `Error::InvalidPath`)
- Absolute paths (starting with `/`) are **rejected**
- Paths must be relative to container workdir
- Maximum path length: 4096 bytes

### Chunking for Large Files

For files larger than `max_file_size_bytes`, use chunk-based access:

```
/containers/sessions/<id>/files/<path>?offset=<bytes>&len=<bytes>
```

Or via filesystem path segments:

```
/containers/sessions/<id>/files/<path>/chunks/<chunk_number>
```

Each chunk is up to 1MB. The `/files/<path>/meta` file returns size and chunk count.

### Unsupported Providers

Not all providers support file access. Check `capabilities.file_access` before use:

```rust
// Check provider capabilities
let info: ContainerProviderInfo = serde_json::from_slice(
    &env.read("/containers/providers/dvm/info")?
)?;

if !info.capabilities.file_access {
    // DVM doesn't support arbitrary file read/write
    return Err(ContainerError::NotSupported {
        capability: "file_access",
        provider: "dvm",
    });
}
```

Providers that don't support a capability return `Error::NotSupported`:

```rust
#[derive(Debug)]
pub enum ContainerError {
    // ... other variants
    NotSupported {
        capability: String,
        provider: String,
    },
}
```

---

## Budget Integration

### USD as Base Denomination

All budget tracking uses **micro-USD** (1 micro-USD = $0.000001) as the canonical unit. This matches `/compute`.

**Settlement happens in sats for DVMs.** When paying DVM providers via Lightning, the runtime converts USD to sats using a configured FX source:

```rust
/// FX rate source for USD→sats conversion (for DVM settlement)
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
```

### Reserve Max, Reconcile Actual

The budget system uses a **reserve/reconcile** model:

1. **At submission:** Reserve `max_cost_usd` from budget
2. **During execution:** Cost accrues (but doesn't exceed reserved)
3. **At completion:** Reconcile `actual_cost_usd`, refund difference to budget

**Invariant: Actual cost must never exceed reserved max_cost_usd.** If a provider would exceed it, the runtime must stop the session or require approval.

### Usage Tracking

The `/containers/usage` file shows reserved vs spent:

```json
{
  "tick": {
    "reserved_usd": 500000,
    "spent_usd": 123456,
    "limit_usd": 1000000,
    "remaining_usd": 500000
  },
  "day": {
    "reserved_usd": 2500000,
    "spent_usd": 1234567,
    "limit_usd": 10000000,
    "remaining_usd": 7500000
  }
}
```

- `reserved_usd`: Sum of `max_cost_usd` for active sessions
- `spent_usd`: Sum of `cost_usd` for completed sessions
- `remaining_usd`: `limit_usd - reserved_usd - spent_usd`

### Mount Configuration

```yaml
mounts:
  /containers:
    type: containers
    access: budgeted  # REQUIRED for cloud providers
    budget:
      per_tick_usd: 500000        # $0.50/tick (in micro-USD)
      per_day_usd: 50000000       # $50/day (in micro-USD)
      approval_threshold_usd: 5000000  # $5 requires approval
    policy:
      allowed_providers: ["apple", "local", "cloudflare", "daytona", "dvm"]
      allowed_images: ["node:*", "python:*", "rust:*"]
      blocked_images: ["*:latest"]  # Require pinned versions
      allow_network: true
      max_execution_time_secs: 600
      max_memory_mb: 4096
      max_concurrent: 3
      require_api_auth: true
      prefer: cost
      require_max_cost: true
    settlement:
      fx_source: wallet  # For DVM Lightning payments
      fx_cache_secs: 300
    auth:
      # Secrets stored in agent's secret store, not state
      token_source: /secrets/openagents_api_token
```

---

## OpenAgents API Authentication

Agents authenticate with the OpenAgents platform to use cloud container providers. Two methods are supported.

### Nostr Identity Authentication

The agent proves identity by signing a challenge with its keypair:

```rust
/// Nostr authentication flow
///
/// 1. Agent reads challenge from /containers/auth/challenge
/// 2. Agent signs challenge with its keypair (via /identity/sign)
/// 3. Agent writes signed response to /containers/auth/challenge
/// 4. Runtime validates signature and fetches credits from OpenAgents API
/// 5. Auth state updated in /containers/auth/status

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrAuthChallenge {
    /// Random challenge string
    pub challenge: String,
    /// Challenge expiry
    pub expires_at: Timestamp,
    /// Expected pubkey (agent's npub)
    pub pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrAuthResponse {
    /// The challenge that was signed
    pub challenge: String,
    /// Schnorr signature over challenge
    pub signature: String,
    /// Agent's pubkey
    pub pubkey: String,
}
```

### API Key Authentication

Traditional API token for compatibility:

```rust
/// API key authentication
///
/// 1. Agent writes API token to /containers/auth/token
/// 2. Token is stored in SECRET STORE (not agent SQLite state)
/// 3. Runtime validates token with OpenAgents API
/// 4. Auth state updated (token_set: true, but token never readable)

// Write token (stored in secret store, not state)
env.write("/containers/auth/token", b"oa_live_abc123...")?;

// Read auth status (token is NEVER returned, only token_set flag)
let status = env.read("/containers/auth/status")?;
```

**Important:** Writing to `/containers/auth/token` stores the token in the agent's **secret store**, not in regular state. Reading back returns only `token_set: true`, never the actual token.

### Auth State

```rust
/// OpenAgents API authentication state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiAuthState {
    /// Whether agent is authenticated
    pub authenticated: bool,

    /// Authentication method used
    pub method: Option<AuthMethod>,

    /// Agent's npub (if Nostr auth)
    pub agent_pubkey: Option<String>,

    /// Whether API token is set (token itself is never exposed)
    pub token_set: bool,

    /// Token expiry timestamp
    pub expires_at: Option<Timestamp>,

    /// Credit balance in micro-USD (from OpenAgents API)
    pub credits_usd: u64,

    /// Rate limit status
    pub rate_limit: RateLimitStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    Nostr,
    ApiKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    /// Requests remaining this window
    pub remaining: u32,
    /// Window reset timestamp
    pub resets_at: Timestamp,
    /// Per-minute limit (user)
    pub user_limit_per_minute: u32,
    /// Per-minute limit (IP)
    pub ip_limit_per_minute: u32,
}
```

### Auth Enforcement

```rust
impl ContainerFs {
    fn check_auth(&self, provider_id: &str) -> Result<(), ContainerError> {
        let policy = self.policy.read();
        let auth = self.auth_state.read();

        // Local provider may not require auth
        if provider_id == "local" && !policy.require_api_auth {
            return Ok(());
        }

        // Cloud providers always require auth
        if !auth.authenticated {
            return Err(ContainerError::AuthRequired {
                provider: provider_id.to_string(),
                message: "OpenAgents API authentication required for cloud containers".into(),
            });
        }

        // Check rate limits
        if auth.rate_limit.remaining == 0 {
            return Err(ContainerError::RateLimited {
                resets_at: auth.rate_limit.resets_at,
            });
        }

        Ok(())
    }

    fn check_credits(&self, estimated_cost_usd: u64) -> Result<(), ContainerError> {
        let auth = self.auth_state.read();

        if auth.credits_usd < estimated_cost_usd {
            return Err(ContainerError::InsufficientCredits {
                required_usd: estimated_cost_usd,
                available_usd: auth.credits_usd,
            });
        }

        Ok(())
    }
}
```

---

## ContainerRouter

Routes requests to appropriate providers:

```rust
/// Routes container requests to appropriate providers
pub struct ContainerRouter {
    providers: Vec<Arc<dyn ContainerProvider>>,
    fx_converter: Arc<dyn FxConverter>,
}

impl ContainerRouter {
    /// Select best provider for request based on policy
    pub fn select(
        &self,
        request: &ContainerRequest,
        policy: &ContainerPolicy,
    ) -> Result<&dyn ContainerProvider, ContainerError> {
        let candidates: Vec<_> = self.providers.iter()
            .filter(|p| p.is_available())
            .filter(|p| self.image_available(p, &request.image))
            .filter(|p| {
                policy.allowed_providers.is_empty()
                    || policy.allowed_providers.contains(&p.id().to_string())
            })
            .filter(|p| self.within_limits(p, &request.limits))
            .collect();

        if candidates.is_empty() {
            return Err(ContainerError::NoProviderAvailable {
                image: request.image.clone(),
                reason: "No provider matches requirements".into(),
            });
        }

        // Sort by preference
        match policy.prefer {
            Prefer::Cost => candidates.into_iter()
                .min_by_key(|p| self.estimate_cost_usd(p, request)),
            Prefer::Latency => candidates.into_iter()
                .min_by_key(|p| p.info().latency.startup_ms),
            Prefer::Quality => candidates.into_iter()
                .max_by_key(|p| p.info().limits.max_memory_mb),
            Prefer::Balanced => candidates.into_iter()
                .min_by_key(|p| {
                    let cost = self.estimate_cost_usd(p, request).unwrap_or(u64::MAX);
                    let latency = p.info().latency.startup_ms;
                    cost.saturating_mul(latency)
                }),
        }
        .map(|p| p.as_ref())
        .ok_or(ContainerError::NoProviderAvailable {
            image: request.image.clone(),
            reason: "Selection failed".into(),
        })
    }
}
```

---

## Backend Implementations

### Local Provider (Docker)

```rust
/// Local Docker container provider
pub struct LocalContainerProvider {
    docker: Arc<Docker>,
    sessions: Arc<RwLock<HashMap<String, LocalSession>>>,
    execs: Arc<RwLock<HashMap<String, ExecState>>>,
    executor: Arc<ExecutorManager>,
}

impl ContainerProvider for LocalContainerProvider {
    fn id(&self) -> &str { "local" }

    fn info(&self) -> ContainerProviderInfo {
        ContainerProviderInfo {
            id: "local".to_string(),
            name: "Local Docker".to_string(),
            available_images: self.list_local_images(),
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: true,
                interactive: true,
                artifacts: true,
                streaming: true,
            },
            pricing: None,  // Free (user's compute)
            latency: ContainerLatency { startup_ms: 1000, measured: true },
            limits: ContainerLimits {
                max_memory_mb: 16384,  // Depends on host
                max_cpu_cores: 8.0,
                max_disk_mb: 102400,
                max_time_secs: 86400,  // 24 hours
                network_allowed: true,
            },
            status: ProviderStatus::Available,
        }
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let exec_id = generate_exec_id();
        let sessions = self.sessions.read();
        let session = sessions.get(session_id).ok_or(ContainerError::SessionNotFound)?;

        // Track exec immediately
        self.execs.write().insert(exec_id.clone(), ExecState::Pending {
            submitted_at: Timestamp::now(),
        });

        // Spawn async execution
        let docker = self.docker.clone();
        let container_id = session.container_id.clone();
        let execs = self.execs.clone();
        let eid = exec_id.clone();
        let cmd = command.to_string();

        self.executor.spawn(async move {
            let result = docker.exec(&container_id, &cmd).await;
            let mut execs = execs.write();
            if let Some(exec) = execs.get_mut(&eid) {
                match result {
                    Ok(r) => *exec = ExecState::Complete(r),
                    Err(e) => *exec = ExecState::Failed {
                        error: e.to_string(),
                        at: Timestamp::now(),
                    },
                }
            }
        });

        Ok(exec_id)  // Return immediately
    }

    fn read_file(&self, session_id: &str, path: &str, offset: u64, len: u64) -> Result<Vec<u8>, ContainerError> {
        let sessions = self.sessions.read();
        let session = sessions.get(session_id).ok_or(ContainerError::SessionNotFound)?;

        // Validate path (no traversal)
        if path.contains("..") || path.starts_with('/') {
            return Err(ContainerError::InvalidPath);
        }

        self.executor.execute(async {
            let data = self.docker.read_file(&session.container_id, path).await?;
            let end = std::cmp::min(offset as usize + len as usize, data.len());
            Ok(data[offset as usize..end].to_vec())
        })
    }
}
```

### Cloudflare Containers Provider

```rust
/// Cloudflare Container provider (via Durable Object + Container API)
#[cfg(feature = "cloudflare")]
pub struct CloudflareContainerProvider {
    container: Container,
    sessions: Arc<RwLock<HashMap<String, CfSession>>>,
    execs: Arc<RwLock<HashMap<String, ExecState>>>,
}

impl ContainerProvider for CloudflareContainerProvider {
    fn id(&self) -> &str { "cloudflare" }

    fn info(&self) -> ContainerProviderInfo {
        ContainerProviderInfo {
            id: "cloudflare".to_string(),
            name: "Cloudflare Containers".to_string(),
            available_images: vec!["node:20".into(), "python:3.12".into(), "rust:1.75".into()],
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: true,
                interactive: false,  // Limited exec support
                artifacts: false,
                streaming: true,
            },
            pricing: Some(ContainerPricing {
                startup_usd: 1000,         // $0.001 per start
                per_second_usd: 100,       // $0.0001/sec
                network_per_gb_usd: 50000, // $0.05/GB
            }),
            latency: ContainerLatency { startup_ms: 3000, measured: true },
            limits: ContainerLimits {
                max_memory_mb: 4096,
                max_cpu_cores: 2.0,
                max_disk_mb: 2048,
                max_time_secs: 900,  // 15 min max
                network_allowed: true,
            },
            status: ProviderStatus::Available,
        }
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        // Cloudflare exec is job-based via HTTP to container
        let exec_id = generate_exec_id();

        // Forward to container's HTTP API (non-blocking)
        let fetcher = self.container.get_tcp_port(8080)?;
        // POST /api/exec with { session_id, command, exec_id }

        self.execs.write().insert(exec_id.clone(), ExecState::Pending {
            submitted_at: Timestamp::now(),
        });

        Ok(exec_id)
    }
}
```

### Daytona SDK Provider

```rust
/// Daytona SDK container provider
pub struct DaytonaProvider {
    client: Arc<DaytonaClient>,
    config: DaytonaConfig,
    sandboxes: Arc<RwLock<HashMap<String, DaytonaSandbox>>>,
    execs: Arc<RwLock<HashMap<String, ExecState>>>,
    executor: Arc<ExecutorManager>,
}

impl ContainerProvider for DaytonaProvider {
    fn id(&self) -> &str { "daytona" }

    fn info(&self) -> ContainerProviderInfo {
        ContainerProviderInfo {
            id: "daytona".to_string(),
            name: "Daytona Cloud Sandbox".to_string(),
            available_images: vec!["daytona-sandbox".to_string()],
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: true,
                interactive: true,
                artifacts: true,
                streaming: true,
            },
            pricing: Some(ContainerPricing {
                startup_usd: 500,
                per_second_usd: 50,
                network_per_gb_usd: 40000,
            }),
            latency: ContainerLatency { startup_ms: 5000, measured: true },
            limits: ContainerLimits {
                max_memory_mb: 8192,
                max_cpu_cores: 4.0,
                max_disk_mb: 20480,
                max_time_secs: 3600,  // 1 hour
                network_allowed: true,
            },
            status: ProviderStatus::Available,
        }
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let exec_id = generate_exec_id();
        let sandboxes = self.sandboxes.read();
        let sandbox = sandboxes.get(session_id).ok_or(ContainerError::SessionNotFound)?;

        // Daytona exec is async via SDK
        let client = self.client.clone();
        let sandbox_id = sandbox.id.clone();
        let execs = self.execs.clone();
        let eid = exec_id.clone();
        let cmd = command.to_string();

        self.executor.spawn(async move {
            let result = client.execute(&sandbox_id, &cmd).await;
            let mut execs = execs.write();
            if let Some(exec) = execs.get_mut(&eid) {
                match result {
                    Ok(r) => *exec = ExecState::Complete(r),
                    Err(e) => *exec = ExecState::Failed {
                        error: e.to_string(),
                        at: Timestamp::now(),
                    },
                }
            }
        });

        Ok(exec_id)
    }

    // Daytona has native file operations
    fn read_file(&self, session_id: &str, path: &str, offset: u64, len: u64) -> Result<Vec<u8>, ContainerError> {
        if path.contains("..") || path.starts_with('/') {
            return Err(ContainerError::InvalidPath);
        }

        let sandboxes = self.sandboxes.read();
        let sandbox = sandboxes.get(session_id).ok_or(ContainerError::SessionNotFound)?;

        self.executor.execute(async {
            let data = self.client.read_file(&sandbox.id, path).await?;
            let end = std::cmp::min(offset as usize + len as usize, data.len());
            Ok(data[offset as usize..end].to_vec())
        })
    }
}
```

### NIP-90 DVM Provider

```rust
/// NIP-90 DVM provider for decentralized container execution
pub struct DvmContainerProvider {
    relays: Vec<String>,
    nostr_client: Arc<NostrClient>,
    signer: Arc<dyn SigningService>,
    active_jobs: Arc<RwLock<HashMap<String, DvmJobState>>>,
    executor: Arc<ExecutorManager>,
    fx_converter: Arc<dyn FxConverter>,  // USD↔sats conversion
}

impl ContainerProvider for DvmContainerProvider {
    fn id(&self) -> &str { "dvm" }

    fn info(&self) -> ContainerProviderInfo {
        ContainerProviderInfo {
            id: "dvm".to_string(),
            name: "NIP-90 DVM Network".to_string(),
            available_images: self.query_handler_images(),
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: false,  // DVMs typically don't support arbitrary file access
                interactive: false,
                artifacts: false,
                streaming: true,  // Via NIP-90 feedback events
            },
            pricing: None,  // Bid-based, varies by handler
            latency: ContainerLatency { startup_ms: 10000, measured: false },
            limits: ContainerLimits {
                max_memory_mb: 8192,   // Handler-dependent
                max_cpu_cores: 4.0,
                max_disk_mb: 10240,
                max_time_secs: 1800,   // 30 min typical
                network_allowed: true,
            },
            status: ProviderStatus::Available,
        }
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        let session_id = generate_session_id();
        let max_cost_usd = request.max_cost_usd.unwrap_or(100_000); // Default $0.10

        // Convert USD to sats for DVM bid
        let max_cost_sats = self.fx_converter.usd_to_sats(max_cost_usd);

        // Create NIP-90 job request
        let job_event = self.executor.execute(async {
            self.create_sandbox_job_request(&session_id, &request, max_cost_sats).await
        })?;

        // Publish to relays
        let event_id = self.executor.execute(async {
            self.nostr_client.publish(job_event).await
        })?;

        // Track job
        self.active_jobs.write().insert(session_id.clone(), DvmJobState {
            request_event_id: event_id,
            status: DvmJobStatus::Submitted,
            result: None,
            provider_pubkey: None,
            bid_usd: max_cost_usd,
            bid_sats: max_cost_sats,
        });

        self.start_response_listener(&session_id);
        Ok(session_id)
    }

    fn read_file(&self, _session_id: &str, _path: &str, _offset: u64, _len: u64) -> Result<Vec<u8>, ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "file_access".to_string(),
            provider: "dvm".to_string(),
        })
    }

    fn submit_exec(&self, _session_id: &str, _command: &str) -> Result<String, ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "interactive".to_string(),
            provider: "dvm".to_string(),
        })
    }
}
```

---

## Implementation per Backend

| Feature | Local (Docker) | Cloudflare | Daytona | DVM |
|---------|----------------|------------|---------|-----|
| **Provider** | `LocalContainerProvider` | `CloudflareContainerProvider` | `DaytonaProvider` | `DvmContainerProvider` |
| **Auth Required** | Optional (policy) | Required | Required | Required (Nostr) |
| **Pricing** | Free | Per-second (micro-USD) | Per-second (micro-USD) | Bid-based (sats) |
| **Payment** | None | OpenAgents credits | OpenAgents credits | Lightning |
| **Max Duration** | 24 hours | 15 minutes | 1 hour | 30 minutes |
| **Max Memory** | Host-limited | 4 GB | 8 GB | Handler-dependent |
| **Network** | Configurable | Configurable | Always | Handler-dependent |
| **Git Operations** | Via commands | Via commands | Native SDK | Via commands |
| **File Access** | Yes | Yes | Yes (native) | **No** |
| **Interactive (exec)** | Yes | Limited | Yes | **No** |
| **Streaming** | Docker logs API | WebSocket | WebSocket | NIP-90 feedback |
| **Cold Start** | ~1s | ~3s | ~5s | ~10s (bid phase) |

---

## Usage Examples

### Simple Command Execution

```rust
// Agent code
let request = serde_json::json!({
    "kind": "ephemeral",
    "image": "node:20",
    "commands": ["node --version", "npm --version"],
    "limits": {
        "max_time_secs": 60,
        "max_memory_mb": 512
    },
    "max_cost_usd": 100000,  // $0.10 budget ceiling (micro-USD)
    "idempotency_key": "test_node_123"
});

// Submit request (returns immediately)
let session_info: serde_json::Value =
    serde_json::from_slice(&env.call("/containers/new", &serde_json::to_vec(&request)?)?)?;
let session_id = session_info["session_id"].as_str().unwrap();

// Poll for completion
loop {
    let status = env.read(&format!("/containers/sessions/{}/status", session_id))?;
    let status: serde_json::Value = serde_json::from_slice(&status)?;

    if status["status"] == "complete" {
        break;
    }
    std::thread::sleep(Duration::from_secs(1));
}

// Read result
let result = env.read(&format!("/containers/sessions/{}/result", session_id))?;
let result: ContainerResponse = serde_json::from_slice(&result)?;
println!("Exit code: {:?}", result.exit_code);
println!("Cost: ${:.6} (reserved: ${:.6})",
    result.cost_usd as f64 / 1_000_000.0,
    result.reserved_usd as f64 / 1_000_000.0);
```

### Build with Repository Clone (Private Repo)

```rust
let request = serde_json::json!({
    "kind": "build",
    "image": "rust:1.75",
    "repo": {
        "url": "https://github.com/user/private-project.git",
        "git_ref": "main",
        "auth": {
            "type": "token",
            "secret_path": "/secrets/github_token"  // Token from secret store
        }
    },
    "commands": [
        "cargo build --release",
        "cargo test"
    ],
    "limits": ResourceLimits::for_build(),
    "max_cost_usd": 5000000,  // $5 budget (micro-USD)
    "idempotency_key": "build_abc123"
});

let _session_info: serde_json::Value =
    serde_json::from_slice(&env.call("/containers/new", &serde_json::to_vec(&request)?)?)?;
```

### Interactive Session with Job-Based Exec

```rust
// Create interactive session
let request = serde_json::json!({
    "kind": "interactive",
    "repo": {
        "url": "https://github.com/user/project.git",
        "git_ref": "main"
    },
    "limits": {
        "max_time_secs": 3600,  // 1 hour
        "allow_network": true
    },
    "max_cost_usd": 10000000  // $10
});

let session_info: serde_json::Value =
    serde_json::from_slice(&env.call("/containers/new", &serde_json::to_vec(&request)?)?)?;
let session_id = session_info["session_id"].as_str().unwrap().to_string();

// Wait for running state
wait_for_status(&env, &session_id, "running")?;

// Execute command (non-blocking, returns exec_id)
let exec_info: serde_json::Value = serde_json::from_slice(&env.call(
    &format!("/containers/sessions/{}/exec/new", session_id),
    b"cargo test --no-fail-fast",
)?)?;
let exec_id = exec_info["exec_id"].as_str().unwrap();

// Watch exec output
if let Some(mut watch) = env.watch(&format!(
    "/containers/sessions/{}/exec/{}/output", session_id, exec_id
))? {
    while let Some(event) = watch.next(Some(Duration::from_secs(30)))? {
        if let WatchEvent::Data(data) = event {
            let chunk: OutputChunk = serde_json::from_slice(&data)?;
            print!("{}", chunk.data);
            if chunk.done { break; }
        }
    }
}

// Read/write files (URL-encoded paths)
let content = env.read(&format!(
    "/containers/sessions/{}/files/src%2Fmain.rs", session_id
))?;
env.write(
    &format!("/containers/sessions/{}/files/src%2Fmain.rs", session_id),
    &modified_content
)?;

// Stop when done
env.write(&format!("/containers/sessions/{}/ctl", session_id), b"stop")?;
```

### Check Auth and Credits

```rust
// Check auth status
let auth_status = env.read("/containers/auth/status")?;
let auth: ApiAuthState = serde_json::from_slice(&auth_status)?;

if !auth.authenticated {
    // Authenticate with Nostr (agent signs challenge)
    let challenge = env.read("/containers/auth/challenge")?;
    let challenge: NostrAuthChallenge = serde_json::from_slice(&challenge)?;

    // Sign challenge using agent's identity
    let signature = env.call("/identity/sign", challenge.challenge.as_bytes())?;

    // Submit signed response
    let response = NostrAuthResponse {
        challenge: challenge.challenge,
        signature: hex::encode(&signature),
        pubkey: get_agent_pubkey(&env)?,
    };
    env.write("/containers/auth/challenge", &serde_json::to_vec(&response)?)?;
}

// Check credits (micro-USD)
println!("Available credits: ${:.2}", auth.credits_usd as f64 / 1_000_000.0);
```

### Check Provider Capabilities

```rust
// Check if provider supports file access before using it
let info_bytes = env.read("/containers/providers/dvm/info")?;
let info: ContainerProviderInfo = serde_json::from_slice(&info_bytes)?;

if !info.capabilities.file_access {
    println!("DVM does not support file access, using output only");
}

if !info.capabilities.interactive {
    println!("DVM does not support interactive exec");
}
```

---

## Integration with Compute

Containers and AI compute (`/compute`) complement each other:

- `/compute` — AI inference (LLM calls, embeddings, image generation)
- `/containers` — Code execution (build, test, run, deploy)

Both use:
- **Micro-USD** as canonical budget unit
- **OpenAgents API auth** for cloud providers
- **Local (free) and cloud (paid) tiers**
- **Idempotency journal** for deduplication
- **Non-blocking execution model** (job IDs returned immediately)
- **Reserve/reconcile budget model**

A container can call `/compute` for AI assistance during execution, and vice versa (LLM can request container spawning for tool use).

The forthcoming OpenRouter-style inference gateway will provide unified LLM routing across providers, sharing the same auth and credit system documented here.
