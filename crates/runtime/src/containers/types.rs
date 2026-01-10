/// Kind of container operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerKind {
    /// Execute commands in ephemeral container (destroyed after completion).
    Ephemeral,
    /// Long-running sandbox with shell access.
    Interactive,
    /// Build/CI container with artifact extraction.
    Build,
    /// Custom container type.
    Custom(String),
}

/// Resource limits for container execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum execution time in seconds (default: 300).
    #[serde(default = "default_max_time")]
    pub max_time_secs: u32,
    /// Maximum memory in MB (default: 1024).
    #[serde(default = "default_max_memory")]
    pub max_memory_mb: u32,
    /// Maximum disk usage in MB (default: 512).
    #[serde(default = "default_max_disk")]
    pub max_disk_mb: u32,
    /// Maximum CPU cores, 1.0 = one core (default: 1.0).
    #[serde(default = "default_max_cpu")]
    pub max_cpu_cores: f32,
    /// Whether network access is allowed (default: false).
    #[serde(default)]
    pub allow_network: bool,
}

impl ResourceLimits {
    /// Basic preset: 5 min, 1GB RAM, 512MB disk, 1 core, no network.
    pub fn basic() -> Self {
        Self {
            max_time_secs: 300,
            max_memory_mb: 1024,
            max_disk_mb: 512,
            max_cpu_cores: 1.0,
            allow_network: false,
        }
    }

    /// Build preset: 10 min, 4GB RAM, 2GB disk, 2 cores, network enabled.
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

fn default_max_time() -> u32 {
    300
}

fn default_max_memory() -> u32 {
    1024
}

fn default_max_disk() -> u32 {
    512
}

fn default_max_cpu() -> f32 {
    1.0
}

/// A container request (provider-agnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRequest {
    /// Kind of container operation.
    pub kind: ContainerKind,
    /// Container image (e.g., "node:20").
    pub image: Option<String>,
    /// Git repository to clone (optional).
    pub repo: Option<RepoConfig>,
    /// Commands to execute (in order).
    pub commands: Vec<String>,
    /// Working directory inside container.
    pub workdir: Option<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Resource limits.
    #[serde(default = "ResourceLimits::basic")]
    pub limits: ResourceLimits,
    /// Maximum cost in micro-USD the caller is willing to pay.
    pub max_cost_usd: Option<u64>,
    /// Idempotency key for deduplication.
    pub idempotency_key: Option<String>,
    /// Timeout in milliseconds (default: 300000 = 5 min).
    pub timeout_ms: Option<u64>,
}

/// Git repository configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    /// Git repository URL.
    pub url: String,
    /// Branch, tag, or commit to checkout.
    pub git_ref: String,
    /// Subdirectory to use as workdir (optional).
    pub subdir: Option<String>,
    /// Authentication for private repos (uses secret store path).
    pub auth: Option<RepoAuth>,
}

/// Repository authentication config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RepoAuth {
    /// Token-based authentication.
    Token {
        /// Path to secret in agent's secret store.
        secret_path: String,
    },
    /// SSH key authentication.
    Ssh {
        /// Path to SSH key in secret store.
        secret_path: String,
    },
}

/// Container response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerResponse {
    /// Session identifier.
    pub session_id: String,
    /// Exit code (0 = success, None if still running).
    pub exit_code: Option<i32>,
    /// Combined stdout.
    pub stdout: String,
    /// Combined stderr.
    pub stderr: String,
    /// Per-command results (if multiple commands).
    pub command_results: Vec<CommandResult>,
    /// Artifact hashes (for build containers).
    pub artifacts: Vec<ArtifactInfo>,
    /// Resource usage.
    pub usage: ContainerUsage,
    /// Actual cost in micro-USD (post-execution).
    pub cost_usd: u64,
    /// Reserved cost (from max_cost_usd at submission).
    pub reserved_usd: u64,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Provider that handled the request.
    pub provider_id: String,
}

/// Result of a single command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// Command string.
    pub command: String,
    /// Exit code.
    pub exit_code: i32,
    /// Stdout output.
    pub stdout: String,
    /// Stderr output.
    pub stderr: String,
    /// Command duration in milliseconds.
    pub duration_ms: u64,
}

/// Artifact metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    /// Artifact path.
    pub path: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// SHA-256 hash.
    pub sha256: String,
}

/// Container usage stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerUsage {
    /// CPU time in milliseconds.
    pub cpu_time_ms: u64,
    /// Peak memory in bytes.
    pub peak_memory_bytes: u64,
    /// Disk writes in bytes.
    pub disk_writes_bytes: u64,
    /// Network bytes transferred.
    pub network_bytes: u64,
}

impl ContainerUsage {
    /// Zeroed usage.
    pub fn zero() -> Self {
        Self {
            cpu_time_ms: 0,
            peak_memory_bytes: 0,
            disk_writes_bytes: 0,
            network_bytes: 0,
        }
    }
}

/// Container session status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerStatus {
    /// Container is being provisioned.
    Provisioning,
    /// Cloning repository.
    Cloning,
    /// Container is running.
    Running,
    /// Execution complete.
    Complete,
    /// Execution failed.
    Failed,
    /// Session expired or cleaned up.
    Expired,
}

/// Nostr authentication challenge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrAuthChallenge {
    /// Random challenge string.
    pub challenge: String,
    /// Challenge expiry.
    pub expires_at: Timestamp,
    /// Expected pubkey (agent's npub).
    pub pubkey: String,
}

/// Nostr authentication response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrAuthResponse {
    /// The challenge that was signed.
    pub challenge: String,
    /// Schnorr signature over challenge (hex).
    pub signature: String,
    /// Agent's pubkey (npub or hex).
    pub pubkey: String,
}

/// OpenAgents API authentication state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiAuthState {
    /// Whether agent is authenticated.
    pub authenticated: bool,
    /// Authentication method used.
    pub method: Option<AuthMethod>,
    /// Agent's npub (if Nostr auth).
    pub agent_pubkey: Option<String>,
    /// Whether API token is set (token itself is never exposed).
    pub token_set: bool,
    /// Token expiry timestamp.
    pub expires_at: Option<Timestamp>,
    /// Credit balance in micro-USD.
    pub credits_usd: u64,
    /// Rate limit status.
    pub rate_limit: RateLimitStatus,
}

impl Default for ApiAuthState {
    fn default() -> Self {
        Self {
            authenticated: false,
            method: None,
            agent_pubkey: None,
            token_set: false,
            expires_at: None,
            credits_usd: 0,
            rate_limit: RateLimitStatus::default(),
        }
    }
}

/// Authentication method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    Nostr,
    ApiKey,
}

/// Rate limit status for OpenAgents API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitStatus {
    /// Requests remaining this window.
    pub remaining: u32,
    /// Window reset timestamp.
    pub resets_at: Timestamp,
    /// Per-minute limit (user).
    pub user_limit_per_minute: u32,
    /// Per-minute limit (IP).
    pub ip_limit_per_minute: u32,
}

impl RateLimitStatus {
    fn is_limited(&self) -> bool {
        self.user_limit_per_minute > 0 || self.ip_limit_per_minute > 0
    }
}

impl Default for RateLimitStatus {
    fn default() -> Self {
        Self {
            remaining: 0,
            resets_at: Timestamp::from_millis(0),
            user_limit_per_minute: 0,
            ip_limit_per_minute: 0,
        }
    }
}

/// Policy for container operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPolicy {
    /// Allowed provider IDs (empty = all available).
    #[serde(default)]
    pub allowed_providers: Vec<String>,
    /// Allowed images (glob patterns supported, e.g., "node:*").
    #[serde(default)]
    pub allowed_images: Vec<String>,
    /// Blocked images (takes precedence over allowed).
    #[serde(default)]
    pub blocked_images: Vec<String>,
    /// Whether network access can be enabled.
    #[serde(default = "default_true")]
    pub allow_network: bool,
    /// Maximum execution time allowed (seconds).
    #[serde(default = "default_max_time_policy")]
    pub max_execution_time_secs: u32,
    /// Maximum memory allowed (MB).
    #[serde(default = "default_max_memory_policy")]
    pub max_memory_mb: u32,
    /// Maximum concurrent containers per agent.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
    /// Maximum cost per tick in micro-USD.
    pub max_cost_usd_per_tick: Option<u64>,
    /// Maximum cost per day in micro-USD.
    pub max_cost_usd_per_day: Option<u64>,
    /// Default max_cost_usd if request doesn't specify.
    pub default_max_cost_usd: Option<u64>,
    /// Require requests to specify max_cost_usd.
    #[serde(default)]
    pub require_max_cost: bool,
    /// Require OpenAgents API authentication for all providers.
    #[serde(default)]
    pub require_api_auth: bool,
    /// Selection preference.
    #[serde(default)]
    pub prefer: Prefer,
    /// Require idempotency keys for all requests.
    #[serde(default)]
    pub require_idempotency: bool,
    /// Maximum file size for read/write operations (bytes).
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: u64,
}

impl Default for ContainerPolicy {
    fn default() -> Self {
        Self {
            allowed_providers: Vec::new(),
            allowed_images: Vec::new(),
            blocked_images: Vec::new(),
            allow_network: true,
            max_execution_time_secs: default_max_time_policy(),
            max_memory_mb: default_max_memory_policy(),
            max_concurrent: default_max_concurrent(),
            max_cost_usd_per_tick: None,
            max_cost_usd_per_day: None,
            default_max_cost_usd: None,
            require_max_cost: false,
            require_api_auth: false,
            prefer: Prefer::Balanced,
            require_idempotency: false,
            max_file_size_bytes: default_max_file_size(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_max_time_policy() -> u32 {
    600
}

fn default_max_memory_policy() -> u32 {
    4096
}

fn default_max_concurrent() -> u32 {
    3
}

fn default_max_file_size() -> u64 {
    10_485_760
}

fn default_timeout_ms() -> u64 {
    300_000
}

/// Information about a container provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerProviderInfo {
    /// Provider identifier.
    pub id: String,
    /// Provider display name.
    pub name: String,
    /// Available images.
    pub available_images: Vec<String>,
    /// Capability flags for programmatic checking.
    pub capabilities: ContainerCapabilities,
    /// Pricing metadata.
    pub pricing: Option<ContainerPricing>,
    /// Latency metadata.
    pub latency: ContainerLatency,
    /// Provider limits.
    pub limits: ContainerLimits,
    /// Provider status.
    pub status: ProviderStatus,
}

/// Capability flags for container providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerCapabilities {
    /// Can clone git repositories.
    pub git_clone: bool,
    /// Can read/write files in container.
    pub file_access: bool,
    /// Supports interactive shell (exec).
    pub interactive: bool,
    /// Can extract build artifacts.
    pub artifacts: bool,
    /// Supports output streaming.
    pub streaming: bool,
}

/// Pricing metadata for container providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPricing {
    /// Cost to start container (micro-USD).
    pub startup_usd: u64,
    /// Cost per second of execution (micro-USD).
    pub per_second_usd: u64,
    /// Cost per GB of network transfer (micro-USD).
    pub network_per_gb_usd: u64,
}

/// Container latency metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLatency {
    /// Expected container startup time in milliseconds.
    pub startup_ms: u64,
    /// Whether this is measured (true) or estimated (false).
    pub measured: bool,
}

/// Container limits reported by providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLimits {
    /// Max memory in MB.
    pub max_memory_mb: u32,
    /// Max CPU cores.
    pub max_cpu_cores: f32,
    /// Max disk in MB.
    pub max_disk_mb: u32,
    /// Max execution time in seconds.
    pub max_time_secs: u32,
    /// Whether network is allowed.
    pub network_allowed: bool,
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

/// Container errors.
#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    /// Request parsing or validation failed.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// Provider is unavailable.
    #[error("provider unavailable: {0}")]
    Unavailable(String),
    /// Authentication required for provider.
    #[error("auth required: {provider} ({message})")]
    AuthRequired {
        /// Provider id.
        provider: String,
        /// Message describing requirement.
        message: String,
    },
    /// Rate limit reached.
    #[error("rate limited until {resets_at}")]
    RateLimited {
        /// Rate limit reset timestamp.
        resets_at: Timestamp,
    },
    /// Insufficient credits for request.
    #[error("insufficient credits (required {required_usd}, available {available_usd})")]
    InsufficientCredits {
        required_usd: u64,
        available_usd: u64,
    },
    /// Unsupported capability.
    #[error("not supported: {capability} ({provider})")]
    NotSupported {
        /// Capability that is missing.
        capability: String,
        /// Provider id.
        provider: String,
    },
    /// No provider matches the request or policy.
    #[error("no provider available: {0}")]
    NoProviderAvailable(String),
    /// Session not found.
    #[error("session not found")]
    SessionNotFound,
    /// Exec job not found.
    #[error("exec not found")]
    ExecNotFound,
    /// Provider returned an error.
    #[error("provider error: {0}")]
    ProviderError(String),
    /// Budget limits were exceeded.
    #[error("budget exceeded")]
    BudgetExceeded,
    /// Idempotency key required but missing.
    #[error("idempotency key required")]
    IdempotencyRequired,
    /// max_cost_usd required but missing.
    #[error("max_cost_usd required")]
    MaxCostRequired,
    /// Session result not ready.
    #[error("not ready")]
    NotReady,
    /// Journal error.
    #[error("journal error: {0}")]
    Journal(String),
}

impl From<BudgetError> for ContainerError {
    fn from(err: BudgetError) -> Self {
        match err {
            BudgetError::Exceeded => ContainerError::BudgetExceeded,
            BudgetError::ActualExceedsReservation => {
                ContainerError::ProviderError("actual cost exceeded reservation".to_string())
            }
        }
    }
}

impl From<JournalError> for ContainerError {
    fn from(err: JournalError) -> Self {
        ContainerError::Journal(err.to_string())
    }
}

/// Session state for tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionState {
    /// Session created, container being provisioned.
    Provisioning {
        /// Start timestamp.
        started_at: Timestamp,
    },
    /// Cloning repository.
    Cloning {
        /// Start timestamp.
        started_at: Timestamp,
        /// Repository URL.
        repo_url: String,
    },
    /// Container is running.
    Running {
        /// Start timestamp.
        started_at: Timestamp,
        /// Commands completed.
        commands_completed: usize,
    },
    /// Session completed successfully.
    Complete(ContainerResponse),
    /// Session failed.
    Failed {
        /// Error message.
        error: String,
        /// Failure timestamp.
        at: Timestamp,
    },
    /// Session expired (cleaned up).
    Expired {
        /// Expiration timestamp.
        at: Timestamp,
    },
}

/// Exec job state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecState {
    /// Exec job pending.
    Pending {
        /// Submission timestamp.
        submitted_at: Timestamp,
    },
    /// Exec job running.
    Running {
        /// Start timestamp.
        started_at: Timestamp,
    },
    /// Exec job completed successfully.
    Complete(CommandResult),
    /// Exec job failed.
    Failed {
        /// Error message.
        error: String,
        /// Failure timestamp.
        at: Timestamp,
    },
}

/// Output stream kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// Streaming output chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputChunk {
    /// Session identifier.
    pub session_id: String,
    /// Exec id (if from exec).
    pub exec_id: Option<String>,
    /// Output stream kind.
    pub stream: OutputStream,
    /// Output payload.
    pub data: String,
}

/// Container provider trait (sync for FileService compatibility).
pub trait ContainerProvider: Send + Sync {
    /// Provider identifier.
    fn id(&self) -> &str;
    /// Provider info (images, pricing, limits, capabilities).
    fn info(&self) -> ContainerProviderInfo;
    /// Check if provider is available.
    fn is_available(&self) -> bool;
    /// Whether the provider requires OpenAgents API auth/credits.
    fn requires_openagents_auth(&self) -> bool {
        false
    }
    /// Submit a container request. ALWAYS returns session_id immediately.
    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError>;
    /// Get current state of a session by ID.
    fn get_session(&self, session_id: &str) -> Option<SessionState>;
    /// Submit command for execution. Returns exec_id immediately.
    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError>;
    /// Get state of an exec job.
    fn get_exec(&self, exec_id: &str) -> Option<ExecState>;
    /// Poll exec output stream.
    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError>;
    /// Cancel an exec job.
    fn cancel_exec(&self, exec_id: &str) -> Result<(), ContainerError>;
    /// Read file from container.
    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, ContainerError>;
    /// Write file to container.
    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ContainerError>;
    /// Stop a running container.
    fn stop(&self, session_id: &str) -> Result<(), ContainerError>;
    /// Poll for output stream (internal use by ContainerFs).
    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError>;
}

