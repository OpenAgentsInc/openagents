# Containers

Portable container abstraction for agents.

---

## Overview

Agents access isolated compute environments through the `/containers` mount. The runtime provides:

- **Provider abstraction** — Same interface for local Docker, cloud sandboxes, and decentralized DVMs
- **Policy enforcement** — Per-agent container restrictions and resource limits
- **Budget tracking** — Sats-denominated accounting (runtime's native money)
- **OpenAgents API auth** — Nostr identity or API keys for cloud providers
- **Streaming output** — Real-time stdout/stderr via `watch()` on session streams

This enables agents to spawn isolated execution environments without hardcoding providers.

---

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Budget units | **Sats** (with FX conversion) | Runtime's native money; consistent with `/compute` and `/wallet` |
| Execution model | **Always returns session_id** | Non-blocking; works on all backends including Cloudflare |
| Auth methods | **Nostr + API keys** | Nostr fits identity model, API keys for compatibility |
| Interface | **`/containers/new` → session_id** | Avoids blocking; agent polls status or watches output |
| Provider naming | **dvm** (not "swarm") | Consistent with NIP-90 terminology |

---

## Filesystem Layout

```
/containers/
├── providers/           # Available container providers
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
├── new                  # Write request → read session_id (always async)
├── policy               # Read/write: container policy JSON
├── usage                # Read: current tick/day usage (sats)
├── auth/               # OpenAgents API authentication
│   ├── status          # Read: auth state JSON
│   ├── token           # Write: set API token
│   ├── challenge       # Read: Nostr auth challenge, Write: signed response
│   └── credits         # Read: available credits balance
└── sessions/
    └── <session_id>/   # Individual container session
        ├── status      # Read: provisioning|cloning|running|complete|failed
        ├── result      # Read: final result (when complete)
        ├── output      # Watch: streaming stdout/stderr
        ├── exec        # Write command → read result (interactive only)
        ├── files/      # Read/write files in container
        │   └── <path>  # Access files inside container
        ├── usage       # Read: resource consumption
        └── ctl         # Write: "stop" to terminate
```

**Key principle: `/containers/new` always returns a session_id immediately.** The agent then reads `/containers/sessions/<id>/status` to poll or watches `/containers/sessions/<id>/output` for streaming. This avoids blocking and works across all backends.

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

    /// Maximum cost in sats the caller is willing to pay
    /// If omitted, uses policy default or rejects if policy.require_max_cost
    pub max_cost_sats: Option<u64>,

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

    /// Actual cost in sats (post-execution)
    pub cost_sats: u64,

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

    /// Maximum cost per tick in sats
    pub max_cost_sats_per_tick: Option<u64>,

    /// Maximum cost per day in sats
    pub max_cost_sats_per_day: Option<u64>,

    /// Default max_cost_sats if request doesn't specify
    pub default_max_cost_sats: Option<u64>,

    /// Require requests to specify max_cost_sats
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
}

fn default_max_time_policy() -> u32 { 600 }    // 10 minutes
fn default_max_memory_policy() -> u32 { 4096 } // 4 GB
fn default_max_concurrent() -> u32 { 3 }
```

---

## ContainerProvider Trait

Sync trait for FileService compatibility:

```rust
/// Information about a container provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub available_images: Vec<String>,
    pub capabilities: Vec<String>,  // "git_clone", "file_access", "interactive", etc.
    pub pricing: Option<ProviderPricing>,
    pub latency: ProviderLatency,
    pub limits: ProviderLimits,
    pub status: ProviderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricing {
    /// Cost to start container (micro-USD, converted to sats)
    pub startup_microusd: u64,
    /// Cost per second of execution (micro-USD)
    pub per_second_microusd: u64,
    /// Cost per GB of network transfer (micro-USD)
    pub network_per_gb_microusd: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderLatency {
    /// Expected container startup time in milliseconds
    pub startup_ms: u64,
    /// Is this measured (true) or estimated (false)?
    pub measured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderLimits {
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

    /// Provider info (images, pricing, limits, etc.)
    fn info(&self) -> ProviderInfo;

    /// Check if provider is available
    fn is_available(&self) -> bool;

    /// Submit a container request. ALWAYS returns session_id immediately.
    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError>;

    /// Get current state of a session by ID
    fn get_session(&self, session_id: &str) -> Option<SessionState>;

    /// Execute command in running container (for interactive sessions)
    fn exec(&self, session_id: &str, command: &str) -> Result<CommandResult, ContainerError>;

    /// Read file from container
    fn read_file(&self, session_id: &str, path: &str) -> Result<Vec<u8>, ContainerError>;

    /// Write file to container
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
                .min_by_key(|p| self.estimate_cost_sats(p, request)),
            Prefer::Latency => candidates.into_iter()
                .min_by_key(|p| p.info().latency.startup_ms),
            Prefer::Quality => candidates.into_iter()
                .max_by_key(|p| p.info().limits.max_memory_mb),
            Prefer::Balanced => candidates.into_iter()
                .min_by_key(|p| {
                    let cost = self.estimate_cost_sats(p, request).unwrap_or(u64::MAX);
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

## OpenAgents API Authentication

Agents authenticate with the OpenAgents platform to use cloud container providers. Two methods are supported:

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
/// 2. Runtime validates token with OpenAgents API
/// 3. Auth state updated with credit balance

// Write token (only the token string)
env.write("/containers/auth/token", b"oa_live_abc123...")?;

// Read auth status
let status = env.read("/containers/auth/status")?;
```

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

    /// Whether API token is set
    pub token_set: bool,

    /// Token expiry timestamp
    pub expires_at: Option<Timestamp>,

    /// Credit balance in sats (from OpenAgents API)
    pub credits_sats: u64,

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

    fn check_credits(&self, estimated_cost_sats: u64) -> Result<(), ContainerError> {
        let auth = self.auth_state.read();

        if auth.credits_sats < estimated_cost_sats {
            return Err(ContainerError::InsufficientCredits {
                required_sats: estimated_cost_sats,
                available_sats: auth.credits_sats,
            });
        }

        Ok(())
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
    executor: Arc<ExecutorManager>,
}

impl ContainerProvider for LocalContainerProvider {
    fn id(&self) -> &str { "local" }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "local".to_string(),
            name: "Local Docker".to_string(),
            available_images: self.list_local_images(),
            capabilities: vec!["git_clone", "file_access", "interactive", "artifacts"],
            pricing: None,  // Free (user's compute)
            latency: ProviderLatency { startup_ms: 1000, measured: true },
            limits: ProviderLimits {
                max_memory_mb: 16384,  // Depends on host
                max_cpu_cores: 8.0,
                max_disk_mb: 102400,
                max_time_secs: 86400,  // 24 hours
                network_allowed: true,
            },
            status: ProviderStatus::Available,
        }
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        let session_id = generate_session_id();

        // Track session immediately
        self.sessions.write().insert(session_id.clone(), LocalSession {
            status: ContainerStatus::Provisioning,
            request: request.clone(),
            started_at: Timestamp::now(),
            container_id: None,
        });

        // Spawn container async
        let docker = self.docker.clone();
        let sessions = self.sessions.clone();
        let sid = session_id.clone();

        self.executor.spawn(async move {
            // 1. Pull image if needed
            // 2. Create container with resource limits (--memory, --cpus, etc.)
            // 3. Clone repo if specified
            // 4. Execute commands sequentially
            // 5. Collect output and artifacts
            // 6. Update session state
        });

        Ok(session_id)
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
}

impl ContainerProvider for CloudflareContainerProvider {
    fn id(&self) -> &str { "cloudflare" }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "cloudflare".to_string(),
            name: "Cloudflare Containers".to_string(),
            available_images: vec![
                "node:20".to_string(),
                "python:3.12".to_string(),
                "rust:1.75".to_string(),
            ],
            capabilities: vec!["git_clone", "file_access"],
            pricing: Some(ProviderPricing {
                startup_microusd: 1000,      // $0.001 per start
                per_second_microusd: 100,    // $0.0001/sec
                network_per_gb_microusd: 50000,
            }),
            latency: ProviderLatency { startup_ms: 3000, measured: true },
            limits: ProviderLimits {
                max_memory_mb: 4096,   // 4 GB
                max_cpu_cores: 2.0,
                max_disk_mb: 2048,
                max_time_secs: 900,    // 15 min max
                network_allowed: true,
            },
            status: ProviderStatus::Available,
        }
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        let session_id = generate_session_id();

        // Start container if not running
        if !self.container.running() {
            let mut opts = ContainerStartupOptions::new();
            opts.enable_internet(request.limits.allow_network);
            self.container.start(Some(opts))?;
        }

        // Forward request to container's HTTP API
        // Container runs autopilot-container which handles execution
        let fetcher = self.container.get_tcp_port(8080)?;
        // POST /api/start with request body

        Ok(session_id)
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
    executor: Arc<ExecutorManager>,
}

#[derive(Debug, Clone)]
pub struct DaytonaConfig {
    pub api_key: String,
    pub base_url: String,
    pub organization_id: Option<String>,
    pub default_snapshot: String,
}

impl ContainerProvider for DaytonaProvider {
    fn id(&self) -> &str { "daytona" }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "daytona".to_string(),
            name: "Daytona Cloud Sandbox".to_string(),
            available_images: vec!["daytona-sandbox".to_string()],
            capabilities: vec![
                "git_clone", "git_diff", "git_commit", "git_push",
                "file_access", "file_tree", "interactive", "lsp",
            ],
            pricing: Some(ProviderPricing {
                startup_microusd: 500,
                per_second_microusd: 50,
                network_per_gb_microusd: 40000,
            }),
            latency: ProviderLatency { startup_ms: 5000, measured: true },
            limits: ProviderLimits {
                max_memory_mb: 8192,
                max_cpu_cores: 4.0,
                max_disk_mb: 20480,
                max_time_secs: 3600,  // 1 hour
                network_allowed: true,
            },
            status: ProviderStatus::Available,
        }
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        let session_id = generate_session_id();

        let client = self.client.clone();
        let sandboxes = self.sandboxes.clone();
        let config = self.config.clone();
        let req = request.clone();

        self.executor.spawn(async move {
            // 1. Create sandbox via Daytona API
            let sandbox = client.create_sandbox(CreateSandboxRequest {
                snapshot: config.default_snapshot,
                auto_stop_interval: req.limits.max_time_secs,
                resources: DaytonaResources {
                    cpu: req.limits.max_cpu_cores,
                    memory_mb: req.limits.max_memory_mb,
                    disk_mb: req.limits.max_disk_mb,
                },
            }).await?;

            // 2. Clone repo if specified (native SDK support)
            if let Some(repo) = &req.repo {
                client.git_clone(&sandbox.id, &repo.url, &repo.git_ref).await?;
            }

            // 3. Execute commands
            for cmd in &req.commands {
                client.execute(&sandbox.id, cmd).await?;
            }

            // 4. Update session state
        });

        Ok(session_id)
    }

    // Daytona has native file operations
    fn read_file(&self, session_id: &str, path: &str) -> Result<Vec<u8>, ContainerError> {
        let sandboxes = self.sandboxes.read();
        let sandbox = sandboxes.get(session_id).ok_or(ContainerError::SessionNotFound)?;

        self.executor.execute(async {
            self.client.read_file(&sandbox.id, path).await
        })
    }

    fn write_file(&self, session_id: &str, path: &str, data: &[u8]) -> Result<(), ContainerError> {
        let sandboxes = self.sandboxes.read();
        let sandbox = sandboxes.get(session_id).ok_or(ContainerError::SessionNotFound)?;

        self.executor.execute(async {
            self.client.write_file(&sandbox.id, path, data).await
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
}

impl ContainerProvider for DvmContainerProvider {
    fn id(&self) -> &str { "dvm" }

    fn info(&self) -> ProviderInfo {
        // Query NIP-89 handler info from relays
        let handlers = self.executor.execute(async {
            self.query_sandbox_handlers().await
        }).unwrap_or_default();

        ProviderInfo {
            id: "dvm".to_string(),
            name: "NIP-90 DVM Network".to_string(),
            available_images: handlers.iter()
                .flat_map(|h| h.supported_images.clone())
                .collect(),
            capabilities: vec!["git_clone", "file_access"],
            pricing: None,  // Bid-based, varies by handler
            latency: ProviderLatency { startup_ms: 10000, measured: false },
            limits: ProviderLimits {
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

        // Create NIP-90 job request (kind 5100 for sandbox execution)
        let job_event = self.executor.execute(async {
            self.create_sandbox_job_request(&session_id, &request).await
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
            bid_sats: request.max_cost_sats.unwrap_or(10000),
        });

        // Start listening for responses
        self.start_response_listener(&session_id);

        Ok(session_id)
    }
}

impl DvmContainerProvider {
    async fn create_sandbox_job_request(
        &self,
        session_id: &str,
        request: &ContainerRequest,
    ) -> Result<NostrEvent, ContainerError> {
        // NIP-90 job request for sandbox execution
        // Using kind 5100 (custom sandbox kind)
        let content = serde_json::to_string(&SandboxJobInput {
            image: request.image.clone(),
            repo: request.repo.clone(),
            commands: request.commands.clone(),
            limits: request.limits.clone(),
        })?;

        let tags = vec![
            vec!["i", &content, "json"],
            vec!["param", "session_id", session_id],
            vec!["bid", &request.max_cost_sats.unwrap_or(10000).to_string()],
            vec!["expiration", &(Timestamp::now().as_secs() + 3600).to_string()],
        ];

        self.signer.sign_event(5100, content, tags)
    }
}
```

---

## Implementation per Backend

| Feature | Local (Docker) | Cloudflare | Daytona | DVM |
|---------|----------------|------------|---------|-----|
| **Provider** | `LocalContainerProvider` | `CloudflareContainerProvider` | `DaytonaProvider` | `DvmContainerProvider` |
| **Auth Required** | Optional (policy) | Required | Required | Required (Nostr) |
| **Pricing** | Free | Per-second + startup | Per-second | Bid-based |
| **Payment** | None | OpenAgents credits | OpenAgents credits | Lightning |
| **Max Duration** | 24 hours | 15 minutes | 1 hour | 30 minutes |
| **Max Memory** | Host-limited | 4 GB | 8 GB | Handler-dependent |
| **Network** | Configurable | Configurable | Always | Handler-dependent |
| **Git Operations** | Via commands | Via commands | Native SDK | Via commands |
| **File Access** | Via exec/mount | Via HTTP | Native SDK | Via job output |
| **Streaming** | Docker logs API | WebSocket | WebSocket | NIP-90 feedback |
| **Interactive** | TTY attach | Limited | Full shell | No |
| **Cold Start** | ~1s | ~3s | ~5s | ~10s (bid phase) |

---

## Budget Integration

ContainerFs integrates with runtime budget system:

```yaml
# Mount table configuration
mounts:
  /containers:
    type: containers
    access: budgeted  # REQUIRED for cloud providers
    budget:
      per_tick_sats: 5000       # ~$0.50 at $100k/BTC
      per_day_sats: 500000      # ~$50/day
      approval_threshold_sats: 25000
    policy:
      allowed_providers: ["local", "cloudflare", "daytona", "dvm"]
      allowed_images: ["node:*", "python:*", "rust:*"]
      blocked_images: ["*:latest"]  # Require pinned versions
      allow_network: true
      max_execution_time_secs: 600
      max_memory_mb: 4096
      max_concurrent: 3
      require_api_auth: true
      prefer: cost
    auth:
      # Option 1: API token from secrets
      token_source: /secrets/openagents_api_token
      # Option 2: Derive from agent identity (Nostr auth)
      use_agent_identity: true
```

The `AccessLevel::Budgeted` controls mount-level access, while `ContainerPolicy` provides container-specific limits.

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
    }
});

// Write request
env.write("/containers/new", &serde_json::to_vec(&request)?)?;

// Read session info
let session_info: serde_json::Value = serde_json::from_slice(&env.read("/containers/new")?)?;
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
println!("Output: {}", result.stdout);
```

### Build with Repository Clone

```rust
let request = serde_json::json!({
    "kind": "build",
    "image": "rust:1.75",
    "repo": {
        "url": "https://github.com/user/project.git",
        "git_ref": "main"
    },
    "commands": [
        "cargo build --release",
        "cargo test"
    ],
    "limits": ResourceLimits::for_build(),
    "max_cost_sats": 5000,
    "idempotency_key": "build_abc123"
});

env.write("/containers/new", &serde_json::to_vec(&request)?)?;
```

### Streaming Output

```rust
// Submit request
env.write("/containers/new", &serde_json::to_vec(&request)?)?;
let session_id = get_session_id(&env)?;

// Watch for output
if let Some(mut watch) = env.watch(&format!("/containers/sessions/{}/output", session_id))? {
    while let Some(event) = watch.next(Some(Duration::from_secs(30)))? {
        if let WatchEvent::Data(data) = event {
            let chunk: OutputChunk = serde_json::from_slice(&data)?;
            match chunk.stream {
                "stdout" => print!("{}", chunk.data),
                "stderr" => eprint!("{}", chunk.data),
                _ => {}
            }

            if chunk.done {
                break;
            }
        }
    }
}
```

### Interactive Session (Daytona)

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
    }
});

env.write("/containers/new", &serde_json::to_vec(&request)?)?;
let session_id = get_session_id(&env)?;

// Wait for running state
wait_for_status(&env, &session_id, "running")?;

// Execute commands interactively
env.write(
    &format!("/containers/sessions/{}/exec", session_id),
    b"cargo test --no-fail-fast"
)?;
let result = env.read(&format!("/containers/sessions/{}/exec", session_id))?;

// Read/write files
let content = env.read(&format!("/containers/sessions/{}/files/src/main.rs", session_id))?;
env.write(
    &format!("/containers/sessions/{}/files/src/main.rs", session_id),
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
    env.write("/identity/sign", challenge.challenge.as_bytes())?;
    let signature = env.read("/identity/sign")?;

    // Submit signed response
    let response = NostrAuthResponse {
        challenge: challenge.challenge,
        signature: hex::encode(&signature),
        pubkey: get_agent_pubkey(&env)?,
    };
    env.write("/containers/auth/challenge", &serde_json::to_vec(&response)?)?;
}

// Check credits
let credits = env.read("/containers/auth/credits")?;
println!("Available credits: {} sats", auth.credits_sats);
```

---

## Integration with Compute

Containers and AI compute (`/compute`) complement each other:

- `/compute` — AI inference (LLM calls, embeddings, image generation)
- `/containers` — Code execution (build, test, run, deploy)

Both use:
- Sats as canonical budget unit
- OpenAgents API auth for cloud providers
- Local (free) and cloud (paid) tiers
- Idempotency journal for deduplication
- Non-blocking execution model

A container can call `/compute` for AI assistance during execution, and vice versa (LLM can request container spawning for tool use).

The forthcoming OpenRouter-style inference gateway will provide unified LLM routing across providers, sharing the same auth and credit system documented here.
