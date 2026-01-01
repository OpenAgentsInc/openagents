//! Container filesystem service and providers.

#![allow(missing_docs)]

use crate::budget::{BudgetError, BudgetPolicy, BudgetReservation, BudgetTracker};
use crate::compute::{ApiTokenProvider, Prefer};
#[cfg(not(target_arch = "wasm32"))]
use crate::dvm::{
    DvmFeedbackStatus, DvmTransport, RelayPoolTransport, msats_to_sats, parse_feedback_event,
};
use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Permissions,
    SeekFrom, Stat, WatchEvent, WatchHandle,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::fx::{FxRateCache, FxRateProvider, FxSource};
use crate::idempotency::{IdempotencyJournal, JournalError};
use crate::identity::{PublicKey, Signature, SigningService};
use crate::storage::AgentStorage;
use crate::types::{AgentId, Timestamp};
#[cfg(not(target_arch = "wasm32"))]
use crate::wallet::{block_on_wallet, WalletFxProvider, WalletService};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
use crate::wasm_http;
use bech32::{Bech32, Hrp};
#[cfg(not(target_arch = "wasm32"))]
use compute::domain::sandbox_run::{
    ResourceLimits as SandboxResourceLimits, SandboxRunRequest, SandboxRunResult,
};
#[cfg(not(target_arch = "wasm32"))]
use daytona::{
    CreateSandbox, DaytonaClient, DaytonaConfig, ExecuteRequest, GitCloneRequest,
    SandboxState as DaytonaSandboxState,
};
#[cfg(not(target_arch = "wasm32"))]
use nostr::nip90::KIND_JOB_SANDBOX_RUN;
#[cfg(not(target_arch = "wasm32"))]
use nostr::{
    DELETION_REQUEST_KIND, JobRequest, JobResult, JobStatus, KIND_JOB_FEEDBACK, UnsignedEvent,
    create_deletion_tags, get_event_hash,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::io::{Read, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, RwLock};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;
use urlencoding::{decode, encode};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
use wasm_bindgen_futures::spawn_local;

const IDEMPOTENCY_TTL: Duration = Duration::from_secs(3600);
const CHUNK_SIZE: u64 = 1_048_576;
const MAX_PATH_LEN: usize = 4096;
const AUTH_STATE_KEY: &str = "containers:auth:state";
const AUTH_TOKEN_KEY: &str = "containers:auth:token";
const AUTH_CHALLENGE_KEY: &str = "containers:auth:challenge";
const AUTH_CHALLENGE_TTL: Duration = Duration::from_secs(300);
const OPENAGENTS_API_URL_ENV: &str = "OPENAGENTS_API_URL";
const DAYTONA_API_URL_ENV: &str = "DAYTONA_API_URL";
const DAYTONA_BASE_URL_ENV: &str = "DAYTONA_BASE_URL";
const DAYTONA_API_KEY_ENV: &str = "DAYTONA_API_KEY";
const DAYTONA_ORG_ID_ENV: &str = "DAYTONA_ORG_ID";
const DAYTONA_TARGET_ENV: &str = "DAYTONA_TARGET";
const DAYTONA_SNAPSHOT_ENV: &str = "DAYTONA_SNAPSHOT";
const DAYTONA_DEFAULT_SNAPSHOT_ENV: &str = "DAYTONA_DEFAULT_SNAPSHOT";
const DAYTONA_AUTO_STOP_ENV: &str = "DAYTONA_AUTO_STOP_MINUTES";
const DAYTONA_AUTO_DELETE_ENV: &str = "DAYTONA_AUTO_DELETE_MINUTES";

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

/// Routes container requests to appropriate providers.
#[derive(Default)]
pub struct ContainerRouter {
    providers: Vec<Arc<dyn ContainerProvider>>,
}

impl ContainerRouter {
    /// Create a new router.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Arc<dyn ContainerProvider>) {
        self.providers.push(provider);
    }

    /// List providers.
    pub fn list_providers(&self) -> Vec<ContainerProviderInfo> {
        self.providers.iter().map(|p| p.info()).collect()
    }

    /// Get provider by id.
    pub fn provider_by_id(&self, id: &str) -> Option<Arc<dyn ContainerProvider>> {
        self.providers.iter().find(|p| p.id() == id).cloned()
    }

    /// Select provider for request based on policy.
    pub fn select(
        &self,
        request: &ContainerRequest,
        policy: &ContainerPolicy,
    ) -> Result<Arc<dyn ContainerProvider>, ContainerError> {
        let candidates: Vec<_> = self
            .providers
            .iter()
            .filter(|p| p.is_available())
            .filter(|p| self.image_available(p, &request.image))
            .filter(|p| {
                policy.allowed_providers.is_empty()
                    || policy.allowed_providers.contains(&p.id().to_string())
            })
            .filter(|p| self.within_limits(p, request))
            .cloned()
            .collect();

        if candidates.is_empty() {
            return Err(ContainerError::NoProviderAvailable(
                "no providers match policy".to_string(),
            ));
        }

        let selected = match policy.prefer {
            Prefer::Cost => candidates
                .into_iter()
                .min_by_key(|p| self.estimate_cost_usd(p, request)),
            Prefer::Latency => candidates
                .into_iter()
                .min_by_key(|p| p.info().latency.startup_ms),
            Prefer::Quality => candidates
                .into_iter()
                .max_by_key(|p| p.info().limits.max_memory_mb),
            Prefer::Balanced => candidates.into_iter().min_by_key(|p| {
                let cost = self.estimate_cost_usd(p, request);
                let latency = p.info().latency.startup_ms;
                cost.saturating_mul(latency)
            }),
        };

        selected.ok_or_else(|| {
            ContainerError::NoProviderAvailable("provider selection failed".to_string())
        })
    }

    fn image_available(
        &self,
        provider: &Arc<dyn ContainerProvider>,
        image: &Option<String>,
    ) -> bool {
        let Some(image) = image.as_ref() else {
            return true;
        };
        let info = provider.info();
        if info.available_images.is_empty() {
            return true;
        }
        info.available_images
            .iter()
            .any(|pattern| pattern_matches(pattern, image))
    }

    fn within_limits(
        &self,
        provider: &Arc<dyn ContainerProvider>,
        request: &ContainerRequest,
    ) -> bool {
        let info = provider.info();
        if request.repo.is_some() && !info.capabilities.git_clone {
            return false;
        }
        if matches!(request.kind, ContainerKind::Interactive) && !info.capabilities.interactive {
            return false;
        }
        if request.limits.allow_network && !info.limits.network_allowed {
            return false;
        }
        if request.limits.max_time_secs > info.limits.max_time_secs {
            return false;
        }
        if request.limits.max_memory_mb > info.limits.max_memory_mb {
            return false;
        }
        if request.limits.max_cpu_cores > info.limits.max_cpu_cores {
            return false;
        }
        if request.limits.max_disk_mb > info.limits.max_disk_mb {
            return false;
        }
        true
    }

    fn estimate_cost_usd(
        &self,
        provider: &Arc<dyn ContainerProvider>,
        request: &ContainerRequest,
    ) -> u64 {
        let info = provider.info();
        let pricing = match info.pricing {
            Some(pricing) => pricing,
            None => return request.max_cost_usd.unwrap_or(0),
        };
        let estimated_secs = request.limits.max_time_secs as u64;
        pricing
            .startup_usd
            .saturating_add(pricing.per_second_usd.saturating_mul(estimated_secs))
    }
}

/// OpenAgents API auth response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiAuthResponse {
    #[serde(flatten, default)]
    pub state: ApiAuthState,
    #[serde(default)]
    pub access_token: Option<String>,
}

/// OpenAgents API client interface for auth and container calls.
pub trait OpenAgentsApiClient: Send + Sync {
    fn authenticate_token(&self, token: &str) -> Result<ApiAuthResponse, ContainerError>;
    fn authenticate_nostr(
        &self,
        response: &NostrAuthResponse,
    ) -> Result<ApiAuthResponse, ContainerError>;
    fn provider_info(
        &self,
        provider_id: &str,
        token: Option<&str>,
    ) -> Result<ContainerProviderInfo, ContainerError>;
    fn submit_container(
        &self,
        provider_id: &str,
        request: &ContainerRequest,
        token: &str,
    ) -> Result<String, ContainerError>;
    fn session_state(&self, session_id: &str, token: &str) -> Result<SessionState, ContainerError>;
    fn submit_exec(
        &self,
        session_id: &str,
        command: &str,
        token: &str,
    ) -> Result<String, ContainerError>;
    fn exec_state(&self, exec_id: &str, token: &str) -> Result<ExecState, ContainerError>;
    fn poll_output(
        &self,
        session_id: &str,
        cursor: Option<&str>,
        token: &str,
    ) -> Result<(Option<OutputChunk>, Option<String>), ContainerError>;
    fn poll_exec_output(
        &self,
        exec_id: &str,
        cursor: Option<&str>,
        token: &str,
    ) -> Result<(Option<OutputChunk>, Option<String>), ContainerError>;
    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
        token: &str,
    ) -> Result<Vec<u8>, ContainerError>;
    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
        token: &str,
    ) -> Result<(), ContainerError>;
    fn stop(&self, session_id: &str, token: &str) -> Result<(), ContainerError>;
}

fn openagents_api_from_env() -> Option<Arc<dyn OpenAgentsApiClient>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return HttpOpenAgentsApiClient::from_env();
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// OpenAgents API auth + credits manager.
#[derive(Clone)]
pub struct OpenAgentsAuth {
    agent_id: AgentId,
    storage: Arc<dyn AgentStorage>,
    signer: Arc<dyn SigningService>,
    api: Option<Arc<dyn OpenAgentsApiClient>>,
    api_base_url: Option<String>,
    state: Arc<RwLock<ApiAuthState>>,
    token_cache: Arc<Mutex<Option<String>>>,
    challenge: Arc<RwLock<Option<NostrAuthChallenge>>>,
}

impl OpenAgentsAuth {
    pub fn new(
        agent_id: AgentId,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
        api: Option<Arc<dyn OpenAgentsApiClient>>,
    ) -> Self {
        let mut state = Self::load_state(&storage, &agent_id);
        let token = Self::load_token(&storage, &agent_id);
        state.token_set = token.is_some();
        if state.agent_pubkey.is_none() {
            if let Ok(npub) = Self::agent_npub_static(&signer, &agent_id) {
                state.agent_pubkey = Some(npub);
            }
        }
        let challenge = Self::load_challenge(&storage, &agent_id);
        Self {
            agent_id,
            storage,
            signer,
            api,
            api_base_url: None,
            state: Arc::new(RwLock::new(state)),
            token_cache: Arc::new(Mutex::new(token)),
            challenge: Arc::new(RwLock::new(challenge)),
        }
    }

    pub fn from_env(
        agent_id: AgentId,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        let api = openagents_api_from_env();
        Self::new(agent_id, storage, signer, api)
    }

    /// Create auth state with an explicit OpenAgents API base URL (browser usage).
    pub fn with_base_url(
        agent_id: AgentId,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
        base_url: impl Into<String>,
    ) -> Self {
        let mut auth = Self::new(agent_id, storage, signer, None);
        auth.api_base_url = Some(base_url.into());
        auth
    }

    pub fn status(&self) -> ApiAuthState {
        self.state.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn status_json(&self) -> FsResult<Vec<u8>> {
        serde_json::to_vec_pretty(&self.status()).map_err(|err| FsError::Other(err.to_string()))
    }

    pub fn credits_json(&self) -> FsResult<Vec<u8>> {
        let state = self.status();
        let json = serde_json::json!({ "credits_usd": state.credits_usd });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    pub fn challenge_json(&self) -> FsResult<Vec<u8>> {
        let challenge = self
            .issue_challenge()
            .map_err(|err| FsError::Other(err.to_string()))?;
        serde_json::to_vec_pretty(&challenge).map_err(|err| FsError::Other(err.to_string()))
    }

    pub fn token(&self) -> Option<String> {
        let mut cache = self.token_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(token) = cache.as_ref() {
            return Some(token.clone());
        }
        let token = Self::load_token(&self.storage, &self.agent_id);
        *cache = token.clone();
        token
    }

    pub fn set_token(&self, token: &str) -> Result<(), ContainerError> {
        Self::store_token(&self.storage, &self.agent_id, token)?;
        {
            let mut cache = self.token_cache.lock().unwrap_or_else(|e| e.into_inner());
            *cache = Some(token.to_string());
        }
        let mut state = self.status();
        state.method = Some(AuthMethod::ApiKey);
        state.token_set = true;
        state.authenticated = false;
        state.expires_at = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(api) = &self.api {
                if let Ok(response) = api.authenticate_token(token) {
                    state = self.apply_auth_response(response, AuthMethod::ApiKey, None)?;
                }
            }
        }
        #[cfg(all(feature = "browser", target_arch = "wasm32"))]
        {
            if let Some(base_url) = self.api_base_url.clone() {
                let auth = self.clone();
                let token = token.trim().to_string();
                spawn_local(async move {
                    auth.validate_token_async(base_url, token).await;
                });
            }
        }
        self.save_state(&state)?;
        Ok(())
    }

    pub fn issue_challenge(&self) -> Result<NostrAuthChallenge, ContainerError> {
        let mut guard = self.challenge.write().unwrap_or_else(|e| e.into_inner());
        if let Some(challenge) = guard.as_ref() {
            if challenge.expires_at.as_millis() > Timestamp::now().as_millis() {
                return Ok(challenge.clone());
            }
        }
        let challenge = NostrAuthChallenge {
            challenge: uuid::Uuid::new_v4().to_string(),
            expires_at: Timestamp::from_millis(
                Timestamp::now()
                    .as_millis()
                    .saturating_add(AUTH_CHALLENGE_TTL.as_millis() as u64),
            ),
            pubkey: self.agent_npub()?,
        };
        Self::store_challenge(&self.storage, &self.agent_id, &challenge)?;
        *guard = Some(challenge.clone());
        Ok(challenge)
    }

    pub fn submit_challenge(&self, response: NostrAuthResponse) -> Result<(), ContainerError> {
        let challenge = self
            .load_current_challenge()
            .ok_or_else(|| ContainerError::InvalidRequest("no auth challenge".to_string()))?;
        if challenge.challenge != response.challenge {
            return Err(ContainerError::InvalidRequest(
                "challenge mismatch".to_string(),
            ));
        }
        if challenge.expires_at.as_millis() <= Timestamp::now().as_millis() {
            return Err(ContainerError::InvalidRequest(
                "challenge expired".to_string(),
            ));
        }
        let pubkey = Self::parse_pubkey(&response.pubkey)?;
        let signature_bytes = hex::decode(&response.signature)
            .map_err(|_| ContainerError::InvalidRequest("invalid signature".to_string()))?;
        let signature = Signature::new(signature_bytes);
        if !self
            .signer
            .verify(&pubkey, response.challenge.as_bytes(), &signature)
        {
            return Err(ContainerError::InvalidRequest(
                "signature verification failed".to_string(),
            ));
        }
        let expected_pubkey = self
            .signer
            .pubkey(&self.agent_id)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if expected_pubkey.as_bytes() != pubkey.as_bytes() {
            return Err(ContainerError::InvalidRequest(
                "pubkey mismatch".to_string(),
            ));
        }

        let mut state = self.status();
        state.method = Some(AuthMethod::Nostr);
        state.agent_pubkey = Some(challenge.pubkey.clone());
        state.authenticated = true;
        state.expires_at = None;
        state.rate_limit = RateLimitStatus::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(api) = &self.api {
                if let Ok(response) = api.authenticate_nostr(&response) {
                    state = self.apply_auth_response(
                        response,
                        AuthMethod::Nostr,
                        Some(challenge.pubkey.clone()),
                    )?;
                }
            }
        }
        #[cfg(all(feature = "browser", target_arch = "wasm32"))]
        {
            if let Some(base_url) = self.api_base_url.clone() {
                let auth = self.clone();
                spawn_local(async move {
                    auth.validate_nostr_async(base_url, response).await;
                });
            }
        }
        self.save_state(&state)?;
        Self::clear_challenge(&self.storage, &self.agent_id)?;
        let mut guard = self.challenge.write().unwrap_or_else(|e| e.into_inner());
        *guard = None;
        Ok(())
    }

    fn apply_auth_response(
        &self,
        response: ApiAuthResponse,
        default_method: AuthMethod,
        default_pubkey: Option<String>,
    ) -> Result<ApiAuthState, ContainerError> {
        let mut state = response.state;
        if state.method.is_none() {
            state.method = Some(default_method);
        }
        if state.agent_pubkey.is_none() {
            state.agent_pubkey = default_pubkey;
        }
        if let Some(access_token) = response.access_token {
            Self::store_token(&self.storage, &self.agent_id, &access_token)?;
            let mut cache = self.token_cache.lock().unwrap_or_else(|e| e.into_inner());
            *cache = Some(access_token);
        }
        state.token_set = self.token().is_some();
        Ok(state)
    }

    #[cfg(all(feature = "browser", target_arch = "wasm32"))]
    async fn validate_token_async(&self, base_url: String, token: String) {
        let url = format!("{}/containers/auth/token", base_url.trim_end_matches('/'));
        let body = match serde_json::to_string(&serde_json::json!({ "token": token })) {
            Ok(body) => body,
            Err(_) => return,
        };
        let response = wasm_http::request_bytes("POST", &url, None, Some(body)).await;
        let Ok((status, bytes)) = response else {
            return;
        };
        if !(200..300).contains(&status) {
            return;
        }
        let Ok(payload) = serde_json::from_slice::<ApiAuthResponse>(&bytes) else {
            return;
        };
        let state = match self.apply_auth_response(payload, AuthMethod::ApiKey, None) {
            Ok(state) => state,
            Err(_) => return,
        };
        let _ = self.save_state(&state);
    }

    #[cfg(all(feature = "browser", target_arch = "wasm32"))]
    async fn validate_nostr_async(&self, base_url: String, response: NostrAuthResponse) {
        let url = format!("{}/containers/auth/nostr", base_url.trim_end_matches('/'));
        let body = match serde_json::to_string(&response) {
            Ok(body) => body,
            Err(_) => return,
        };
        let resp = wasm_http::request_bytes("POST", &url, None, Some(body)).await;
        let Ok((status, bytes)) = resp else {
            return;
        };
        if !(200..300).contains(&status) {
            return;
        }
        let Ok(payload) = serde_json::from_slice::<ApiAuthResponse>(&bytes) else {
            return;
        };
        let state = match self
            .apply_auth_response(payload, AuthMethod::Nostr, Some(response.pubkey.clone()))
        {
            Ok(state) => state,
            Err(_) => return,
        };
        let _ = self.save_state(&state);
    }

    pub fn check_auth(
        &self,
        provider_id: &str,
        policy: &ContainerPolicy,
        requires_auth: bool,
    ) -> Result<(), ContainerError> {
        let state = self.status();
        if provider_id == "local" {
            return Ok(());
        }
        if !policy.require_api_auth && !requires_auth {
            return Ok(());
        }
        if !state.authenticated {
            return Err(ContainerError::AuthRequired {
                provider: provider_id.to_string(),
                message: "OpenAgents API authentication required".to_string(),
            });
        }
        if provider_id != "local" && !state.token_set {
            return Err(ContainerError::AuthRequired {
                provider: provider_id.to_string(),
                message: "OpenAgents API token required".to_string(),
            });
        }
        if state.rate_limit.is_limited() && state.rate_limit.remaining == 0 {
            return Err(ContainerError::RateLimited {
                resets_at: state.rate_limit.resets_at,
            });
        }
        Ok(())
    }

    pub fn check_credits(&self, estimated_cost_usd: u64) -> Result<(), ContainerError> {
        let state = self.status();
        if state.credits_usd < estimated_cost_usd {
            return Err(ContainerError::InsufficientCredits {
                required_usd: estimated_cost_usd,
                available_usd: state.credits_usd,
            });
        }
        Ok(())
    }

    pub fn reserve_credits(&self, amount: u64) -> Result<u64, ContainerError> {
        if amount == 0 {
            return Ok(0);
        }
        let mut state = self.status();
        if state.credits_usd < amount {
            return Err(ContainerError::InsufficientCredits {
                required_usd: amount,
                available_usd: state.credits_usd,
            });
        }
        state.credits_usd = state.credits_usd.saturating_sub(amount);
        self.save_state(&state)?;
        Ok(amount)
    }

    pub fn release_credits(&self, amount: u64) -> Result<(), ContainerError> {
        if amount == 0 {
            return Ok(());
        }
        let mut state = self.status();
        state.credits_usd = state.credits_usd.saturating_add(amount);
        self.save_state(&state)?;
        Ok(())
    }

    pub fn reconcile_credits(&self, reserved: u64, actual: u64) -> Result<(), ContainerError> {
        if reserved == 0 {
            return Ok(());
        }
        let mut state = self.status();
        if actual <= reserved {
            state.credits_usd = state.credits_usd.saturating_add(reserved - actual);
        } else {
            let extra = actual - reserved;
            state.credits_usd = state.credits_usd.saturating_sub(extra);
        }
        self.save_state(&state)?;
        Ok(())
    }

    fn agent_npub(&self) -> Result<String, ContainerError> {
        Self::agent_npub_static(&self.signer, &self.agent_id)
    }

    fn agent_npub_static(
        signer: &Arc<dyn SigningService>,
        agent_id: &AgentId,
    ) -> Result<String, ContainerError> {
        let pubkey = signer
            .pubkey(agent_id)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let bytes = pubkey.as_bytes();
        if bytes.len() != 32 {
            return Err(ContainerError::InvalidRequest(
                "nostr pubkey must be 32 bytes".to_string(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        public_key_to_npub(&arr)
    }

    fn parse_pubkey(value: &str) -> Result<PublicKey, ContainerError> {
        if value.starts_with("npub1") {
            let bytes = npub_to_public_key(value)?;
            return Ok(PublicKey::new(bytes.to_vec()));
        }
        let bytes = hex::decode(value)
            .map_err(|_| ContainerError::InvalidRequest("invalid pubkey".to_string()))?;
        if bytes.len() != 32 {
            return Err(ContainerError::InvalidRequest(
                "nostr pubkey must be 32 bytes".to_string(),
            ));
        }
        Ok(PublicKey::new(bytes))
    }

    fn load_state(storage: &Arc<dyn AgentStorage>, agent_id: &AgentId) -> ApiAuthState {
        let data = futures::executor::block_on(storage.get(agent_id, AUTH_STATE_KEY))
            .ok()
            .flatten();
        data.and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save_state(&self, state: &ApiAuthState) -> Result<(), ContainerError> {
        let data = serde_json::to_vec_pretty(state)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        futures::executor::block_on(self.storage.set(&self.agent_id, AUTH_STATE_KEY, &data))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let mut guard = self.state.write().unwrap_or_else(|e| e.into_inner());
        *guard = state.clone();
        Ok(())
    }

    fn load_token(storage: &Arc<dyn AgentStorage>, agent_id: &AgentId) -> Option<String> {
        let data = futures::executor::block_on(storage.get(agent_id, AUTH_TOKEN_KEY))
            .ok()
            .flatten()?;
        String::from_utf8(data).ok()
    }

    fn store_token(
        storage: &Arc<dyn AgentStorage>,
        agent_id: &AgentId,
        token: &str,
    ) -> Result<(), ContainerError> {
        futures::executor::block_on(storage.set(agent_id, AUTH_TOKEN_KEY, token.as_bytes()))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn load_challenge(
        storage: &Arc<dyn AgentStorage>,
        agent_id: &AgentId,
    ) -> Option<NostrAuthChallenge> {
        let data = futures::executor::block_on(storage.get(agent_id, AUTH_CHALLENGE_KEY))
            .ok()
            .flatten()?;
        serde_json::from_slice(&data).ok()
    }

    fn store_challenge(
        storage: &Arc<dyn AgentStorage>,
        agent_id: &AgentId,
        challenge: &NostrAuthChallenge,
    ) -> Result<(), ContainerError> {
        let data = serde_json::to_vec(challenge)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        futures::executor::block_on(storage.set(agent_id, AUTH_CHALLENGE_KEY, &data))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn clear_challenge(
        storage: &Arc<dyn AgentStorage>,
        agent_id: &AgentId,
    ) -> Result<(), ContainerError> {
        futures::executor::block_on(storage.delete(agent_id, AUTH_CHALLENGE_KEY))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn load_current_challenge(&self) -> Option<NostrAuthChallenge> {
        let guard = self.challenge.read().unwrap_or_else(|e| e.into_inner());
        if guard.as_ref().is_some() {
            return guard.clone();
        }
        drop(guard);
        let challenge = Self::load_challenge(&self.storage, &self.agent_id);
        let mut guard = self.challenge.write().unwrap_or_else(|e| e.into_inner());
        *guard = challenge.clone();
        challenge
    }
}

fn npub_to_public_key(value: &str) -> Result<[u8; 32], ContainerError> {
    let (hrp, data) = bech32::decode(value)
        .map_err(|_| ContainerError::InvalidRequest("invalid npub".to_string()))?;
    if hrp.as_str() != "npub" {
        return Err(ContainerError::InvalidRequest(
            "invalid npub prefix".to_string(),
        ));
    }
    if data.len() != 32 {
        return Err(ContainerError::InvalidRequest(
            "nostr pubkey must be 32 bytes".to_string(),
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data);
    Ok(out)
}

fn public_key_to_npub(public_key: &[u8; 32]) -> Result<String, ContainerError> {
    let hrp = Hrp::parse("npub").map_err(|err| ContainerError::ProviderError(err.to_string()))?;
    bech32::encode::<Bech32>(hrp, public_key)
        .map_err(|err| ContainerError::ProviderError(err.to_string()))
}

impl ApiTokenProvider for OpenAgentsAuth {
    fn api_token(&self) -> Option<String> {
        self.token()
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct HttpOpenAgentsApiClient {
    base_url: String,
    client: reqwest::Client,
}

#[cfg(not(target_arch = "wasm32"))]
impl HttpOpenAgentsApiClient {
    fn from_env() -> Option<Arc<dyn OpenAgentsApiClient>> {
        let base_url = std::env::var(OPENAGENTS_API_URL_ENV).ok()?;
        if base_url.trim().is_empty() {
            return None;
        }
        Self::new(base_url)
            .ok()
            .map(|client| Arc::new(client) as Arc<dyn OpenAgentsApiClient>)
    }

    fn new(base_url: impl Into<String>) -> Result<Self, ContainerError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        Ok(Self {
            base_url: base_url.into(),
            client,
        })
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn build_request(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let mut builder = self.client.request(method, self.url(path));
        if let Some(token) = token {
            builder = builder.bearer_auth(token);
        }
        builder
    }

    fn execute<F, T>(&self, fut: F) -> Result<T, ContainerError>
    where
        F: std::future::Future<Output = Result<T, ContainerError>>,
    {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            return tokio::task::block_in_place(|| handle.block_on(fut));
        }
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        runtime.block_on(fut)
    }

    fn send_request(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<(reqwest::StatusCode, Vec<u8>), ContainerError> {
        self.execute(async {
            let response = builder
                .send()
                .await
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            let status = response.status();
            let bytes = response
                .bytes()
                .await
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?
                .to_vec();
            Ok((status, bytes))
        })
    }

    fn request_json<R: DeserializeOwned>(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<R, ContainerError> {
        let (status, bytes) = self.send_request(builder)?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        serde_json::from_slice(&bytes).map_err(|err| ContainerError::ProviderError(err.to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl OpenAgentsApiClient for HttpOpenAgentsApiClient {
    fn authenticate_token(&self, token: &str) -> Result<ApiAuthResponse, ContainerError> {
        let body = serde_json::json!({ "token": token });
        let builder = self
            .build_request(reqwest::Method::POST, "containers/auth/token", None)
            .json(&body);
        self.request_json(builder)
    }

    fn authenticate_nostr(
        &self,
        response: &NostrAuthResponse,
    ) -> Result<ApiAuthResponse, ContainerError> {
        let builder = self
            .build_request(reqwest::Method::POST, "containers/auth/nostr", None)
            .json(response);
        self.request_json(builder)
    }

    fn provider_info(
        &self,
        provider_id: &str,
        token: Option<&str>,
    ) -> Result<ContainerProviderInfo, ContainerError> {
        let path = format!("containers/providers/{}/info", provider_id);
        let builder = self.build_request(reqwest::Method::GET, &path, token);
        self.request_json(builder)
    }

    fn submit_container(
        &self,
        provider_id: &str,
        request: &ContainerRequest,
        token: &str,
    ) -> Result<String, ContainerError> {
        #[derive(Deserialize)]
        struct SessionResponse {
            session_id: String,
        }
        let path = format!("containers/providers/{}/sessions", provider_id);
        let body = serde_json::to_value(request)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let builder = self
            .build_request(reqwest::Method::POST, &path, Some(token))
            .json(&body);
        let response: SessionResponse = self.request_json(builder)?;
        Ok(response.session_id)
    }

    fn session_state(&self, session_id: &str, token: &str) -> Result<SessionState, ContainerError> {
        let path = format!("containers/sessions/{}", session_id);
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(ContainerError::SessionNotFound);
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        serde_json::from_slice(&bytes).map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn submit_exec(
        &self,
        session_id: &str,
        command: &str,
        token: &str,
    ) -> Result<String, ContainerError> {
        #[derive(Deserialize)]
        struct ExecResponse {
            exec_id: String,
        }
        let path = format!("containers/sessions/{}/exec", session_id);
        let body = serde_json::json!({ "command": command });
        let builder = self
            .build_request(reqwest::Method::POST, &path, Some(token))
            .json(&body);
        let response: ExecResponse = self.request_json(builder)?;
        Ok(response.exec_id)
    }

    fn exec_state(&self, exec_id: &str, token: &str) -> Result<ExecState, ContainerError> {
        let path = format!("containers/exec/{}", exec_id);
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(ContainerError::ExecNotFound);
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        serde_json::from_slice(&bytes).map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn poll_output(
        &self,
        session_id: &str,
        cursor: Option<&str>,
        token: &str,
    ) -> Result<(Option<OutputChunk>, Option<String>), ContainerError> {
        #[derive(Deserialize)]
        struct OutputResponse {
            chunk: Option<OutputChunk>,
            cursor: Option<String>,
        }
        let mut path = format!("containers/sessions/{}/output", session_id);
        if let Some(cursor) = cursor {
            path = format!("{}?cursor={}", path, cursor);
        }
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NO_CONTENT || bytes.is_empty() {
            return Ok((None, cursor.map(|c| c.to_string())));
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        if let Ok(payload) = serde_json::from_slice::<OutputResponse>(&bytes) {
            return Ok((payload.chunk, payload.cursor));
        }
        let chunk: OutputChunk = serde_json::from_slice(&bytes)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        Ok((Some(chunk), None))
    }

    fn poll_exec_output(
        &self,
        exec_id: &str,
        cursor: Option<&str>,
        token: &str,
    ) -> Result<(Option<OutputChunk>, Option<String>), ContainerError> {
        #[derive(Deserialize)]
        struct OutputResponse {
            chunk: Option<OutputChunk>,
            cursor: Option<String>,
        }
        let mut path = format!("containers/exec/{}/output", exec_id);
        if let Some(cursor) = cursor {
            path = format!("{}?cursor={}", path, cursor);
        }
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NO_CONTENT || bytes.is_empty() {
            return Ok((None, cursor.map(|c| c.to_string())));
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        if let Ok(payload) = serde_json::from_slice::<OutputResponse>(&bytes) {
            return Ok((payload.chunk, payload.cursor));
        }
        let chunk: OutputChunk = serde_json::from_slice(&bytes)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        Ok((Some(chunk), None))
    }

    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
        token: &str,
    ) -> Result<Vec<u8>, ContainerError> {
        let encoded = encode(path);
        let path = format!(
            "containers/sessions/{}/files/{}?offset={}&len={}",
            session_id, encoded, offset, len
        );
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(ContainerError::SessionNotFound);
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        Ok(bytes)
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
        token: &str,
    ) -> Result<(), ContainerError> {
        let encoded = encode(path);
        let path = format!(
            "containers/sessions/{}/files/{}?offset={}",
            session_id, encoded, offset
        );
        let builder = self
            .build_request(reqwest::Method::PUT, &path, Some(token))
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(data.to_vec());
        let (status, bytes) = self.send_request(builder)?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        Ok(())
    }

    fn stop(&self, session_id: &str, token: &str) -> Result<(), ContainerError> {
        let path = format!("containers/sessions/{}/stop", session_id);
        let builder = self.build_request(reqwest::Method::POST, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct SessionRecord {
    provider_id: String,
    reservation: BudgetReservation,
    reconciled: bool,
    credits_reserved: u64,
}

#[derive(Clone)]
struct ExecRecord {
    provider_id: String,
    session_id: String,
}

/// Container filesystem service.
pub struct ContainerFs {
    agent_id: AgentId,
    router: Arc<RwLock<ContainerRouter>>,
    policy: Arc<RwLock<ContainerPolicy>>,
    auth: Arc<OpenAgentsAuth>,
    budget: Arc<Mutex<BudgetTracker>>,
    journal: Arc<dyn IdempotencyJournal>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    execs: Arc<RwLock<HashMap<String, ExecRecord>>>,
}

impl ContainerFs {
    /// Create a new container filesystem.
    pub fn new(
        agent_id: AgentId,
        router: ContainerRouter,
        policy: ContainerPolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        let auth = Arc::new(OpenAgentsAuth::from_env(agent_id.clone(), storage, signer));
        Self::with_auth(agent_id, router, policy, budget_policy, journal, auth)
    }

    /// Create a container filesystem with local + OpenAgents providers from env.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_default_providers(
        agent_id: AgentId,
        policy: ContainerPolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        let api = openagents_api_from_env();
        let auth = Arc::new(OpenAgentsAuth::new(
            agent_id.clone(),
            storage,
            signer,
            api.clone(),
        ));
        let mut router = ContainerRouter::new();
        #[cfg(all(target_os = "macos", feature = "apple-container"))]
        router.register(Arc::new(AppleContainerProvider::new()));
        router.register(Arc::new(LocalContainerProvider::new()));
        let mut has_daytona = false;
        if let Ok(Some(provider)) = DaytonaContainerProvider::from_env() {
            router.register(Arc::new(provider));
            has_daytona = true;
        }
        if let Some(api) = api {
            router.register(Arc::new(OpenAgentsContainerProvider::cloudflare(
                api.clone(),
                auth.clone(),
            )));
            if !has_daytona {
                router.register(Arc::new(OpenAgentsContainerProvider::daytona(
                    api,
                    auth.clone(),
                )));
            }
        }
        Self::with_auth(agent_id, router, policy, budget_policy, journal, auth)
    }

    /// Create a container filesystem with a preconfigured auth manager.
    pub fn with_auth(
        agent_id: AgentId,
        router: ContainerRouter,
        policy: ContainerPolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        auth: Arc<OpenAgentsAuth>,
    ) -> Self {
        Self {
            agent_id,
            router: Arc::new(RwLock::new(router)),
            policy: Arc::new(RwLock::new(policy)),
            auth,
            budget: Arc::new(Mutex::new(BudgetTracker::new(budget_policy))),
            journal,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
        }
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

    fn reconcile_session(&self, session_id: &str, state: &SessionState) -> FsResult<()> {
        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        let record = match sessions.get_mut(session_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }
        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            SessionState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
                if record.credits_reserved > 0 {
                    self.auth
                        .reconcile_credits(record.credits_reserved, response.cost_usd)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                }
            }
            SessionState::Failed { .. } | SessionState::Expired { .. } => {
                tracker.release(record.reservation);
                if record.credits_reserved > 0 {
                    self.auth
                        .release_credits(record.credits_reserved)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                }
            }
            _ => return Ok(()),
        }
        record.reconciled = true;
        Ok(())
    }

    fn session_provider(&self, session_id: &str) -> FsResult<Arc<dyn ContainerProvider>> {
        let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
        let record = sessions.get(session_id).ok_or(FsError::NotFound)?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        router
            .provider_by_id(&record.provider_id)
            .ok_or(FsError::NotFound)
    }

    fn exec_provider(&self, exec_id: &str) -> FsResult<(Arc<dyn ContainerProvider>, ExecRecord)> {
        let execs = self.execs.read().unwrap_or_else(|e| e.into_inner());
        let record = execs.get(exec_id).ok_or(FsError::NotFound)?.clone();
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .provider_by_id(&record.provider_id)
            .ok_or(FsError::NotFound)?;
        Ok((provider, record))
    }
}

impl FileService for ContainerFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        match parts.as_slice() {
            ["new"] if flags.write => Ok(Box::new(ContainerNewHandle::new(
                self.agent_id.clone(),
                self.router.clone(),
                self.policy.clone(),
                self.auth.clone(),
                self.budget.clone(),
                self.sessions.clone(),
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
            ["auth", "status"] => Ok(Box::new(BytesHandle::new(self.auth.status_json()?))),
            ["auth", "credits"] => Ok(Box::new(BytesHandle::new(self.auth.credits_json()?))),
            ["auth", "token"] => {
                if flags.write {
                    Ok(Box::new(AuthTokenHandle::new(self.auth.clone())))
                } else {
                    Err(FsError::PermissionDenied)
                }
            }
            ["auth", "challenge"] => {
                if flags.write {
                    Ok(Box::new(AuthChallengeWriteHandle::new(self.auth.clone())))
                } else {
                    Ok(Box::new(BytesHandle::new(self.auth.challenge_json()?)))
                }
            }
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
            ["providers", id, "images"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::to_vec_pretty(&info.available_images)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "health"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::json!({
                    "status": match info.status {
                        ProviderStatus::Available => "available",
                        ProviderStatus::Degraded { .. } => "degraded",
                        ProviderStatus::Unavailable { .. } => "unavailable",
                    }
                });
                let bytes =
                    serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "status"] => {
                let provider = self.session_provider(session_id)?;
                let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
                self.reconcile_session(session_id, &state)?;
                let (status, error) = match &state {
                    SessionState::Provisioning { .. } => ("provisioning", None),
                    SessionState::Cloning { .. } => ("cloning", None),
                    SessionState::Running { .. } => ("running", None),
                    SessionState::Complete(_) => ("complete", None),
                    SessionState::Failed { error, .. } => ("failed", Some(error.clone())),
                    SessionState::Expired { .. } => ("expired", None),
                };
                let json = serde_json::json!({
                    "status": status,
                    "error": error,
                });
                let bytes =
                    serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "result"] => {
                let provider = self.session_provider(session_id)?;
                let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
                self.reconcile_session(session_id, &state)?;
                match state {
                    SessionState::Complete(response) => {
                        let json = serde_json::to_vec_pretty(&response)
                            .map_err(|err| FsError::Other(err.to_string()))?;
                        Ok(Box::new(BytesHandle::new(json)))
                    }
                    SessionState::Failed { error, .. } => Err(FsError::Other(error)),
                    _ => Err(FsError::Other("not ready".to_string())),
                }
            }
            ["sessions", session_id, "usage"] => {
                let provider = self.session_provider(session_id)?;
                let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
                self.reconcile_session(session_id, &state)?;
                let response = match state {
                    SessionState::Complete(response) => serde_json::json!({
                        "usage": response.usage,
                        "cost_usd": response.cost_usd,
                        "reserved_usd": response.reserved_usd,
                        "duration_ms": response.duration_ms,
                    }),
                    _ => serde_json::json!({
                        "usage": ContainerUsage::zero(),
                        "cost_usd": 0,
                        "reserved_usd": 0,
                        "duration_ms": 0,
                    }),
                };
                let bytes =
                    serde_json::to_vec(&response).map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "ctl"] if flags.write => Ok(Box::new(CtlHandle::new(
                session_id.to_string(),
                self.router.clone(),
                self.sessions.clone(),
            ))),
            ["sessions", session_id, "exec", "new"] if flags.write => {
                Ok(Box::new(ExecNewHandle::new(
                    session_id.to_string(),
                    self.router.clone(),
                    self.execs.clone(),
                )))
            }
            ["sessions", session_id, "exec", exec_id, "status"] => {
                let (provider, record) = self.exec_provider(exec_id)?;
                if record.session_id != *session_id {
                    return Err(FsError::NotFound);
                }
                let state = provider.get_exec(exec_id).ok_or(FsError::NotFound)?;
                let (status, error) = match &state {
                    ExecState::Pending { .. } => ("pending", None),
                    ExecState::Running { .. } => ("running", None),
                    ExecState::Complete(_) => ("complete", None),
                    ExecState::Failed { error, .. } => ("failed", Some(error.clone())),
                };
                let json = serde_json::json!({
                    "status": status,
                    "error": error,
                });
                let bytes =
                    serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "exec", exec_id, "result"] => {
                let (provider, record) = self.exec_provider(exec_id)?;
                if record.session_id != *session_id {
                    return Err(FsError::NotFound);
                }
                let state = provider.get_exec(exec_id).ok_or(FsError::NotFound)?;
                match state {
                    ExecState::Complete(result) => {
                        let json = serde_json::to_vec_pretty(&result)
                            .map_err(|err| FsError::Other(err.to_string()))?;
                        Ok(Box::new(BytesHandle::new(json)))
                    }
                    ExecState::Failed { error, .. } => Err(FsError::Other(error)),
                    _ => Err(FsError::Other("not ready".to_string())),
                }
            }
            ["sessions", session_id, "files", encoded] => {
                let policy = self.policy.read().unwrap_or_else(|e| e.into_inner());
                let file_path = decode_path(encoded)?;
                validate_relative_path(&file_path)?;
                let provider = self.session_provider(session_id)?;
                if flags.write {
                    Ok(Box::new(FileWriteHandle::new(
                        provider,
                        session_id.to_string(),
                        file_path,
                        0,
                        policy.max_file_size_bytes,
                        false,
                    )))
                } else {
                    let data = provider
                        .read_file(
                            session_id,
                            &file_path,
                            0,
                            policy.max_file_size_bytes.saturating_add(1),
                        )
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    if data.len() as u64 > policy.max_file_size_bytes {
                        return Err(FsError::Other("file too large".to_string()));
                    }
                    Ok(Box::new(BytesHandle::new(data)))
                }
            }
            ["sessions", session_id, "files", encoded, "chunks", chunk] => {
                let policy = self.policy.read().unwrap_or_else(|e| e.into_inner());
                let file_path = decode_path(encoded)?;
                validate_relative_path(&file_path)?;
                let chunk_index: u64 = chunk.parse().map_err(|_| FsError::InvalidPath)?;
                let offset = chunk_index.saturating_mul(CHUNK_SIZE);
                let provider = self.session_provider(session_id)?;
                if flags.write {
                    let max_chunk = policy.max_file_size_bytes.min(CHUNK_SIZE);
                    Ok(Box::new(FileWriteHandle::new(
                        provider,
                        session_id.to_string(),
                        file_path,
                        offset,
                        max_chunk,
                        true,
                    )))
                } else {
                    let len = CHUNK_SIZE.min(policy.max_file_size_bytes);
                    let data = provider
                        .read_file(session_id, &file_path, offset, len)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    Ok(Box::new(BytesHandle::new(data)))
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
                DirEntry::dir("auth"),
                DirEntry::dir("sessions"),
            ]),
            "auth" => Ok(vec![
                DirEntry::file("status", 0),
                DirEntry::file("token", 0),
                DirEntry::file("challenge", 0),
                DirEntry::file("credits", 0),
            ]),
            "providers" => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                Ok(router
                    .list_providers()
                    .iter()
                    .map(|p| DirEntry::dir(&p.id))
                    .collect())
            }
            "sessions" => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                Ok(sessions.keys().map(|id| DirEntry::dir(id)).collect())
            }
            _ => Ok(Vec::new()),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        match parts.as_slice() {
            [] => Ok(Stat::dir()),
            ["providers"] | ["sessions"] | ["auth"] => Ok(Stat::dir()),
            ["new"] | ["policy"] => Ok(Stat {
                size: 0,
                is_dir: false,
                created: None,
                modified: None,
                permissions: Permissions::read_write(),
            }),
            ["usage"] => Ok(Stat::file(0)),
            ["auth", "status"] | ["auth", "credits"] => Ok(Stat::file(0)),
            ["auth", "challenge"] => Ok(Stat {
                size: 0,
                is_dir: false,
                created: None,
                modified: None,
                permissions: Permissions::read_write(),
            }),
            ["auth", "token"] => Ok(Stat {
                size: 0,
                is_dir: false,
                created: None,
                modified: None,
                permissions: Permissions {
                    read: false,
                    write: true,
                    execute: false,
                },
            }),
            ["providers", id] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["providers", id, "info"]
            | ["providers", id, "images"]
            | ["providers", id, "health"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "status"]
            | ["sessions", session_id, "result"]
            | ["sessions", session_id, "output"]
            | ["sessions", session_id, "usage"]
            | ["sessions", session_id, "ctl"] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "exec"] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "exec", "new"] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat {
                        size: 0,
                        is_dir: false,
                        created: None,
                        modified: None,
                        permissions: Permissions::read_write(),
                    })
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "exec", exec_id] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                let execs = self.execs.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) && execs.contains_key(*exec_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "exec", exec_id, "status"]
            | ["sessions", session_id, "exec", exec_id, "result"]
            | ["sessions", session_id, "exec", exec_id, "output"] => {
                let execs = self.execs.read().unwrap_or_else(|e| e.into_inner());
                if execs
                    .get(*exec_id)
                    .map(|record| record.session_id == *session_id)
                    .unwrap_or(false)
                {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "files", ..] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
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
        if let ["sessions", session_id, "output"] = parts.as_slice() {
            let provider = self.session_provider(session_id)?;
            return Ok(Some(Box::new(SessionWatchHandle::new(
                session_id.to_string(),
                provider,
                self.sessions.clone(),
                self.budget.clone(),
                self.auth.clone(),
            ))));
        }
        if let ["sessions", session_id, "exec", exec_id, "output"] = parts.as_slice() {
            let (provider, record) = self.exec_provider(exec_id)?;
            if record.session_id != *session_id {
                return Err(FsError::NotFound);
            }
            return Ok(Some(Box::new(ExecWatchHandle::new(
                exec_id.to_string(),
                provider,
            ))));
        }
        Ok(None)
    }

    fn name(&self) -> &str {
        "containers"
    }
}

struct ContainerNewHandle {
    agent_id: AgentId,
    router: Arc<RwLock<ContainerRouter>>,
    policy: Arc<RwLock<ContainerPolicy>>,
    auth: Arc<OpenAgentsAuth>,
    budget: Arc<Mutex<BudgetTracker>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    journal: Arc<dyn IdempotencyJournal>,
    request_buf: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ContainerNewHandle {
    fn new(
        agent_id: AgentId,
        router: Arc<RwLock<ContainerRouter>>,
        policy: Arc<RwLock<ContainerPolicy>>,
        auth: Arc<OpenAgentsAuth>,
        budget: Arc<Mutex<BudgetTracker>>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        journal: Arc<dyn IdempotencyJournal>,
    ) -> Self {
        Self {
            agent_id,
            router,
            policy,
            auth,
            budget,
            sessions,
            journal,
            request_buf: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn submit_request(&mut self) -> FsResult<()> {
        let mut request: ContainerRequest = serde_json::from_slice(&self.request_buf)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let policy = self
            .policy
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if policy.require_idempotency && request.idempotency_key.is_none() {
            return Err(FsError::Other(
                ContainerError::IdempotencyRequired.to_string(),
            ));
        }

        if request.timeout_ms.is_none() {
            request.timeout_ms = Some(default_timeout_ms());
        }

        if request.commands.is_empty() && !matches!(request.kind, ContainerKind::Interactive) {
            return Err(FsError::Other("commands required".to_string()));
        }

        validate_image(&policy, &request.image)?;
        validate_limits(&policy, &request.limits)?;

        if policy.max_concurrent > 0 {
            let active = count_active_sessions(&self.router, &self.sessions);
            if active as u32 >= policy.max_concurrent {
                return Err(FsError::Other(
                    "max concurrent containers reached".to_string(),
                ));
            }
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
                return Err(FsError::Other(ContainerError::MaxCostRequired.to_string()));
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

        let requires_auth = provider.requires_openagents_auth();
        self.auth
            .check_auth(&provider_id, &policy, requires_auth)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let requires_credits = requires_auth;

        let scoped_key = request
            .idempotency_key
            .as_ref()
            .map(|key| format!("{}:{}:{}", self.agent_id.as_str(), provider_id, key));

        if let Some(key) = scoped_key.as_ref() {
            if let Some(cached) = self
                .journal
                .get(key)
                .map_err(|err| FsError::Other(err.to_string()))?
            {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&cached) {
                    if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
                        self.sessions
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .entry(session_id.to_string())
                            .or_insert(SessionRecord {
                                provider_id: provider_id.clone(),
                                reservation: BudgetReservation { amount_usd: 0 },
                                reconciled: true,
                                credits_reserved: 0,
                            });
                    }
                }
                self.response = Some(cached);
                return Ok(());
            }
        }

        if requires_credits {
            self.auth
                .check_credits(max_cost_usd)
                .map_err(|err| FsError::Other(err.to_string()))?;
        }

        let reservation = {
            let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
            let reservation = tracker
                .reserve(max_cost_usd)
                .map_err(|_| FsError::BudgetExceeded)?;
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

        let credits_reserved = if requires_credits {
            match self.auth.reserve_credits(max_cost_usd) {
                Ok(reserved) => reserved,
                Err(err) => {
                    let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                    tracker.release(reservation);
                    return Err(FsError::Other(err.to_string()));
                }
            }
        } else {
            0
        };

        let session_id = match provider.submit(request.clone()) {
            Ok(session_id) => session_id,
            Err(err) => {
                let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                tracker.release(reservation);
                if credits_reserved > 0 {
                    let _ = self.auth.release_credits(credits_reserved);
                }
                return Err(FsError::Other(err.to_string()));
            }
        };

        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.clone(),
                SessionRecord {
                    provider_id,
                    reservation,
                    reconciled: false,
                    credits_reserved,
                },
            );

        let response_json = serde_json::json!({
            "session_id": session_id,
            "status": "provisioning",
            "status_path": format!("/containers/sessions/{}/status", session_id),
            "output_path": format!("/containers/sessions/{}/output", session_id),
            "result_path": format!("/containers/sessions/{}/result", session_id),
            "exec_path": format!("/containers/sessions/{}/exec", session_id),
            "files_path": format!("/containers/sessions/{}/files", session_id),
        });
        let response_bytes =
            serde_json::to_vec(&response_json).map_err(|err| FsError::Other(err.to_string()))?;

        if let Some(key) = scoped_key.as_ref() {
            self.journal
                .put_with_ttl(key, &response_bytes, IDEMPOTENCY_TTL)
                .map_err(|err| FsError::Other(err.to_string()))?;
        }

        self.response = Some(response_bytes);
        Ok(())
    }
}

impl FileHandle for ContainerNewHandle {
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
    policy: Arc<RwLock<ContainerPolicy>>,
    buffer: Vec<u8>,
}

impl PolicyWriteHandle {
    fn new(policy: Arc<RwLock<ContainerPolicy>>) -> Self {
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
        let policy: ContainerPolicy =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        let mut guard = self.policy.write().unwrap_or_else(|e| e.into_inner());
        *guard = policy;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct AuthTokenHandle {
    auth: Arc<OpenAgentsAuth>,
    buffer: Vec<u8>,
}

impl AuthTokenHandle {
    fn new(auth: Arc<OpenAgentsAuth>) -> Self {
        Self {
            auth,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for AuthTokenHandle {
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
        let token = String::from_utf8(self.buffer.clone())
            .map_err(|_| FsError::Other("invalid token utf-8".to_string()))?;
        if token.trim().is_empty() {
            return Err(FsError::Other("token required".to_string()));
        }
        self.auth
            .set_token(token.trim())
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct AuthChallengeWriteHandle {
    auth: Arc<OpenAgentsAuth>,
    buffer: Vec<u8>,
}

impl AuthChallengeWriteHandle {
    fn new(auth: Arc<OpenAgentsAuth>) -> Self {
        Self {
            auth,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for AuthChallengeWriteHandle {
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
        let response: NostrAuthResponse =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        self.auth
            .submit_challenge(response)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ExecNewHandle {
    session_id: String,
    router: Arc<RwLock<ContainerRouter>>,
    execs: Arc<RwLock<HashMap<String, ExecRecord>>>,
    buffer: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ExecNewHandle {
    fn new(
        session_id: String,
        router: Arc<RwLock<ContainerRouter>>,
        execs: Arc<RwLock<HashMap<String, ExecRecord>>>,
    ) -> Self {
        Self {
            session_id,
            router,
            execs,
            buffer: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn submit(&mut self) -> FsResult<()> {
        let command = String::from_utf8(self.buffer.clone())
            .map_err(|_| FsError::Other("invalid utf-8 command".to_string()))?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        let exec_id = provider
            .submit_exec(&self.session_id, command.trim())
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                exec_id.clone(),
                ExecRecord {
                    provider_id: provider.id().to_string(),
                    session_id: self.session_id.clone(),
                },
            );
        let response_json = serde_json::json!({
            "exec_id": exec_id,
            "status": "pending",
            "status_path": format!("/containers/sessions/{}/exec/{}/status", self.session_id, exec_id),
            "output_path": format!("/containers/sessions/{}/exec/{}/output", self.session_id, exec_id),
            "result_path": format!("/containers/sessions/{}/exec/{}/result", self.session_id, exec_id),
        });
        let response_bytes =
            serde_json::to_vec(&response_json).map_err(|err| FsError::Other(err.to_string()))?;
        self.response = Some(response_bytes);
        Ok(())
    }
}

impl FileHandle for ExecNewHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.response.is_none() {
            self.submit()?;
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
        self.buffer.extend_from_slice(buf);
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
        if self.response.is_none() && !self.buffer.is_empty() {
            self.submit()?;
        }
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct CtlHandle {
    session_id: String,
    router: Arc<RwLock<ContainerRouter>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    buffer: Vec<u8>,
}

impl CtlHandle {
    fn new(
        session_id: String,
        router: Arc<RwLock<ContainerRouter>>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    ) -> Self {
        Self {
            session_id,
            router,
            sessions,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for CtlHandle {
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
        let command = String::from_utf8_lossy(&self.buffer);
        if command.trim() != "stop" {
            return Err(FsError::Other("unsupported ctl command".to_string()));
        }
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        provider
            .stop(&self.session_id)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.session_id);
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct SessionWatchHandle {
    session_id: String,
    provider: Arc<dyn ContainerProvider>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    budget: Arc<Mutex<BudgetTracker>>,
    auth: Arc<OpenAgentsAuth>,
}

impl SessionWatchHandle {
    fn new(
        session_id: String,
        provider: Arc<dyn ContainerProvider>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        budget: Arc<Mutex<BudgetTracker>>,
        auth: Arc<OpenAgentsAuth>,
    ) -> Self {
        Self {
            session_id,
            provider,
            sessions,
            budget,
            auth,
        }
    }

    fn reconcile(&self, state: &SessionState) -> FsResult<()> {
        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        let record = match sessions.get_mut(&self.session_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }
        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            SessionState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
                if record.credits_reserved > 0 {
                    self.auth
                        .reconcile_credits(record.credits_reserved, response.cost_usd)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                }
            }
            SessionState::Failed { .. } | SessionState::Expired { .. } => {
                tracker.release(record.reservation);
                if record.credits_reserved > 0 {
                    self.auth
                        .release_credits(record.credits_reserved)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                }
            }
            _ => return Ok(()),
        }
        record.reconciled = true;
        Ok(())
    }
}

impl WatchHandle for SessionWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            match self.provider.poll_output(&self.session_id) {
                Ok(Some(chunk)) => {
                    if let Some(state) = self.provider.get_session(&self.session_id) {
                        self.reconcile(&state)?;
                    }
                    let payload = serde_json::to_vec(&chunk)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    return Ok(Some(WatchEvent::Data(payload)));
                }
                Ok(None) => {
                    if let Some(state) = self.provider.get_session(&self.session_id) {
                        if matches!(
                            state,
                            SessionState::Complete(_)
                                | SessionState::Failed { .. }
                                | SessionState::Expired { .. }
                        ) {
                            self.reconcile(&state)?;
                            return Ok(None);
                        }
                    }
                }
                Err(err) => return Err(FsError::Other(err.to_string())),
            }
            if !wait_for_output(deadline)? {
                return Ok(None);
            }
        }
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

struct ExecWatchHandle {
    exec_id: String,
    provider: Arc<dyn ContainerProvider>,
}

impl ExecWatchHandle {
    fn new(exec_id: String, provider: Arc<dyn ContainerProvider>) -> Self {
        Self { exec_id, provider }
    }
}

impl WatchHandle for ExecWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            match self.provider.poll_exec_output(&self.exec_id) {
                Ok(Some(chunk)) => {
                    let payload = serde_json::to_vec(&chunk)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    return Ok(Some(WatchEvent::Data(payload)));
                }
                Ok(None) => {
                    if let Some(state) = self.provider.get_exec(&self.exec_id) {
                        if matches!(state, ExecState::Complete(_) | ExecState::Failed { .. }) {
                            return Ok(None);
                        }
                    }
                }
                Err(err) => return Err(FsError::Other(err.to_string())),
            }
            if !wait_for_output(deadline)? {
                return Ok(None);
            }
        }
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

struct FileWriteHandle {
    provider: Arc<dyn ContainerProvider>,
    session_id: String,
    path: String,
    offset: u64,
    buffer: Vec<u8>,
    max_size: u64,
    is_chunk: bool,
}

fn wait_for_output(deadline: Option<Instant>) -> FsResult<bool> {
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
        thread::sleep(Duration::from_millis(25));
        Ok(true)
    }
}

impl FileWriteHandle {
    fn new(
        provider: Arc<dyn ContainerProvider>,
        session_id: String,
        path: String,
        offset: u64,
        max_size: u64,
        is_chunk: bool,
    ) -> Self {
        Self {
            provider,
            session_id,
            path,
            offset,
            buffer: Vec::new(),
            max_size,
            is_chunk,
        }
    }
}

impl FileHandle for FileWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        if (self.buffer.len() + buf.len()) as u64 > self.max_size {
            return Err(FsError::Other("file write exceeds max size".to_string()));
        }
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
        if self.is_chunk && self.buffer.len() as u64 > CHUNK_SIZE {
            return Err(FsError::Other("chunk exceeds max chunk size".to_string()));
        }
        self.provider
            .write_file(&self.session_id, &self.path, self.offset, &self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

fn decode_path(encoded: &str) -> FsResult<String> {
    if encoded.len() > MAX_PATH_LEN {
        return Err(FsError::InvalidPath);
    }
    decode(encoded)
        .map(|value| value.into_owned())
        .map_err(|_| FsError::InvalidPath)
}

fn validate_relative_path(path: &str) -> FsResult<()> {
    if path.is_empty() || path.len() > MAX_PATH_LEN {
        return Err(FsError::InvalidPath);
    }
    if path.starts_with('/') {
        return Err(FsError::InvalidPath);
    }
    if path.contains('\\') {
        return Err(FsError::InvalidPath);
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(FsError::InvalidPath);
        }
    }
    Ok(())
}

fn validate_image(policy: &ContainerPolicy, image: &Option<String>) -> FsResult<()> {
    let Some(image) = image.as_ref() else {
        return Ok(());
    };
    if policy
        .blocked_images
        .iter()
        .any(|pattern| pattern_matches(pattern, image))
    {
        return Err(FsError::Other("image blocked by policy".to_string()));
    }
    if !policy.allowed_images.is_empty()
        && !policy
            .allowed_images
            .iter()
            .any(|pattern| pattern_matches(pattern, image))
    {
        return Err(FsError::Other("image not allowed by policy".to_string()));
    }
    Ok(())
}

fn validate_limits(policy: &ContainerPolicy, limits: &ResourceLimits) -> FsResult<()> {
    if !policy.allow_network && limits.allow_network {
        return Err(FsError::Other("network access not allowed".to_string()));
    }
    if limits.max_time_secs > policy.max_execution_time_secs {
        return Err(FsError::Other("max execution time exceeded".to_string()));
    }
    if limits.max_memory_mb > policy.max_memory_mb {
        return Err(FsError::Other("max memory exceeded".to_string()));
    }
    Ok(())
}

fn count_active_sessions(
    router: &Arc<RwLock<ContainerRouter>>,
    sessions: &Arc<RwLock<HashMap<String, SessionRecord>>>,
) -> usize {
    let sessions = sessions.read().unwrap_or_else(|e| e.into_inner());
    let router = router.read().unwrap_or_else(|e| e.into_inner());
    sessions
        .iter()
        .filter(|(session_id, record)| {
            if let Some(provider) = router.provider_by_id(&record.provider_id) {
                match provider.get_session(session_id) {
                    Some(SessionState::Complete(_))
                    | Some(SessionState::Failed { .. })
                    | Some(SessionState::Expired { .. }) => false,
                    Some(_) => true,
                    None => true,
                }
            } else {
                true
            }
        })
        .count()
}

fn pattern_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut pos = 0usize;
    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if let Some(found) = value[pos..].find(part) {
            pos += found + part.len();
        } else {
            return false;
        }
        if idx == 0 && !pattern.starts_with('*') && !value.starts_with(part) {
            return false;
        }
    }
    if !pattern.ends_with('*') {
        if let Some(last) = parts.last() {
            if !value.ends_with(last) {
                return false;
            }
        }
    }
    true
}

fn unavailable_provider_info(id: &str, name: &str, reason: String) -> ContainerProviderInfo {
    ContainerProviderInfo {
        id: id.to_string(),
        name: name.to_string(),
        available_images: Vec::new(),
        capabilities: ContainerCapabilities {
            git_clone: false,
            file_access: false,
            interactive: false,
            artifacts: false,
            streaming: false,
        },
        pricing: None,
        latency: ContainerLatency {
            startup_ms: 0,
            measured: false,
        },
        limits: ContainerLimits {
            max_memory_mb: 0,
            max_cpu_cores: 0.0,
            max_disk_mb: 0,
            max_time_secs: 0,
            network_allowed: false,
        },
        status: ProviderStatus::Unavailable { reason },
    }
}

#[cfg(not(target_arch = "wasm32"))]
const DVM_QUOTE_WINDOW: Duration = Duration::from_secs(5);

/// NIP-90 DVM container provider.
#[cfg(not(target_arch = "wasm32"))]
pub struct DvmContainerProvider {
    agent_id: AgentId,
    transport: Arc<dyn DvmTransport>,
    signer: Arc<dyn SigningService>,
    wallet: Option<Arc<dyn WalletService>>,
    fx: Arc<FxRateCache>,
    executor: AsyncExecutor,
    sessions: Arc<RwLock<HashMap<String, DvmContainerSession>>>,
    execs: Arc<RwLock<HashMap<String, ExecState>>>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct DvmContainerQuote {
    provider_pubkey: String,
    price_sats: u64,
    price_usd: u64,
    event_id: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
#[allow(dead_code)]
enum DvmContainerLifecycle {
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
#[allow(dead_code)]
struct DvmContainerSession {
    session_id: String,
    request_event_id: String,
    request: ContainerRequest,
    submitted_at: Timestamp,
    lifecycle: DvmContainerLifecycle,
    quotes: Vec<DvmContainerQuote>,
    accepted_quote: Option<DvmContainerQuote>,
    result: Option<ContainerResponse>,
    output: VecDeque<OutputChunk>,
    payment_made: bool,
    paid_amount_sats: Option<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DvmContainerProvider {
    /// Create a new DVM container provider.
    pub fn new(
        agent_id: AgentId,
        relays: Vec<String>,
        signer: Arc<dyn SigningService>,
        wallet: Option<Arc<dyn WalletService>>,
        fx_source: FxSource,
        fx_cache_secs: u64,
    ) -> Result<Self, ContainerError> {
        let transport = Arc::new(RelayPoolTransport::new(relays));
        Self::with_transport(
            agent_id,
            transport,
            signer,
            wallet,
            fx_source,
            fx_cache_secs,
        )
    }

    /// Create a DVM provider with custom transport (tests).
    pub(crate) fn with_transport(
        agent_id: AgentId,
        transport: Arc<dyn DvmTransport>,
        signer: Arc<dyn SigningService>,
        wallet: Option<Arc<dyn WalletService>>,
        fx_source: FxSource,
        fx_cache_secs: u64,
    ) -> Result<Self, ContainerError> {
        let executor = AsyncExecutor::new()?;
        let runtime = executor.runtime();
        executor
            .block_on(transport.connect())
            .map_err(ContainerError::ProviderError)?;
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
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn build_job_request(&self, request: &ContainerRequest) -> Result<JobRequest, ContainerError> {
        let repo = request
            .repo
            .as_ref()
            .ok_or_else(|| ContainerError::InvalidRequest("repo required for dvm".to_string()))?;

        let mut sandbox = SandboxRunRequest::new(repo.url.clone(), repo.git_ref.clone());
        for command in &request.commands {
            sandbox = sandbox.add_command(command.clone());
        }

        let workdir = join_workdir(&repo.subdir, &request.workdir);
        if let Some(workdir) = workdir {
            sandbox = sandbox.with_workdir(workdir);
        }

        for (key, value) in &request.env {
            sandbox = sandbox.add_env(key.clone(), value.clone());
        }

        let limits = SandboxResourceLimits {
            max_time_secs: request.limits.max_time_secs,
            max_memory_mb: request.limits.max_memory_mb,
            max_disk_mb: request.limits.max_disk_mb,
            max_cpu_cores: request.limits.max_cpu_cores,
            allow_network: request.limits.allow_network,
        };
        sandbox = sandbox.with_limits(limits);

        let mut job = sandbox
            .to_job_request()
            .map_err(|err| ContainerError::InvalidRequest(err.to_string()))?;
        for relay in self.transport.relays() {
            job = job.add_relay(relay);
        }

        let max_cost_usd = request.max_cost_usd.unwrap_or(100_000);
        let max_cost_sats = self
            .fx
            .usd_to_sats(max_cost_usd)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let bid_msats = u128::from(max_cost_sats) * 1000;
        let bid_msats = u64::try_from(bid_msats)
            .map_err(|_| ContainerError::ProviderError("bid overflow".to_string()))?;
        job = job.with_bid(bid_msats);
        Ok(job)
    }

    fn sign_event(
        &self,
        kind: u16,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> Result<nostr::Event, ContainerError> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let pubkey = self
            .signer
            .pubkey(&self.agent_id)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let pubkey_hex = pubkey.to_hex();
        let unsigned = UnsignedEvent {
            pubkey: pubkey_hex.clone(),
            created_at,
            kind,
            tags,
            content,
        };
        let id = get_event_hash(&unsigned)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let id_bytes =
            hex::decode(&id).map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let sig = self
            .signer
            .sign(&self.agent_id, &id_bytes)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

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

    fn spawn_quote_manager(&self, session_id: String) {
        let sessions = self.sessions.clone();
        let transport = self.transport.clone();
        let signer = self.signer.clone();
        let agent_id = self.agent_id.clone();
        let executor = self.executor.clone();

        executor.spawn(async move {
            tokio::time::sleep(DVM_QUOTE_WINDOW).await;

            let (request_event_id, quote) = {
                let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                let session = match guard.get_mut(&session_id) {
                    Some(session) => session,
                    None => return,
                };
                if !matches!(
                    session.lifecycle,
                    DvmContainerLifecycle::AwaitingQuotes { .. }
                ) {
                    return;
                }
                let best = match session
                    .quotes
                    .iter()
                    .min_by_key(|quote| quote.price_usd)
                    .cloned()
                {
                    Some(best) => best,
                    None => {
                        session.lifecycle = DvmContainerLifecycle::Failed {
                            error: "no quotes received".to_string(),
                            at: Timestamp::now(),
                        };
                        return;
                    }
                };
                session.accepted_quote = Some(best.clone());
                session.lifecycle = DvmContainerLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: best.provider_pubkey.clone(),
                };
                (session.request_event_id.clone(), best)
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

    fn subscribe_session_events(
        &self,
        session_id: String,
        request_event_id: String,
    ) -> Result<(), ContainerError> {
        let result_kind = KIND_JOB_SANDBOX_RUN + 1000;
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
        let subscription_id = format!("dvm-session-{}", request_event_id);
        let mut rx = self
            .executor
            .block_on(self.transport.subscribe(&subscription_id, &filters))
            .map_err(ContainerError::ProviderError)?;

        let sessions = self.sessions.clone();
        let fx = self.fx.clone();
        let wallet = self.wallet.clone();

        self.executor.spawn(async move {
            while let Some(event) = rx.recv().await {
                if event.kind == result_kind {
                    handle_dvm_container_result(&session_id, &event, &sessions, &fx, &wallet);
                } else if event.kind == KIND_JOB_FEEDBACK {
                    if let Some(feedback) = parse_feedback_event(&event) {
                        handle_dvm_container_feedback(
                            &session_id,
                            feedback,
                            &sessions,
                            &fx,
                            &wallet,
                        );
                    }
                }
            }
        });

        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ContainerProvider for DvmContainerProvider {
    fn id(&self) -> &str {
        "dvm"
    }

    fn info(&self) -> ContainerProviderInfo {
        ContainerProviderInfo {
            id: "dvm".to_string(),
            name: "NIP-90 DVM Network".to_string(),
            available_images: Vec::new(),
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: false,
                interactive: false,
                artifacts: false,
                streaming: true,
            },
            pricing: None,
            latency: ContainerLatency {
                startup_ms: 10_000,
                measured: false,
            },
            limits: ContainerLimits {
                max_memory_mb: 8192,
                max_cpu_cores: 4.0,
                max_disk_mb: 10_240,
                max_time_secs: 1800,
                network_allowed: true,
            },
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

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        if self.wallet.is_none() {
            return Err(ContainerError::ProviderError(
                "wallet not configured".to_string(),
            ));
        }
        let job_request = self.build_job_request(&request)?;
        let event = self.sign_event(
            job_request.kind,
            job_request.to_tags(),
            job_request.content.clone(),
        )?;
        let event_id = event.id.clone();

        self.executor
            .block_on(self.transport.publish(event))
            .map_err(ContainerError::ProviderError)?;

        let session_id = uuid::Uuid::new_v4().to_string();
        let now = Timestamp::now();
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.clone(),
                DvmContainerSession {
                    session_id: session_id.clone(),
                    request_event_id: event_id.clone(),
                    request: request.clone(),
                    submitted_at: now,
                    lifecycle: DvmContainerLifecycle::AwaitingQuotes {
                        since: now,
                        timeout_at: Timestamp::from_millis(
                            now.as_millis() + DVM_QUOTE_WINDOW.as_millis() as u64,
                        ),
                    },
                    quotes: Vec::new(),
                    accepted_quote: None,
                    result: None,
                    output: VecDeque::new(),
                    payment_made: false,
                    paid_amount_sats: None,
                },
            );

        self.subscribe_session_events(session_id.clone(), event_id)?;
        self.spawn_quote_manager(session_id.clone());
        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        let guard = self.sessions.read().unwrap_or_else(|e| e.into_inner());
        let session = guard.get(session_id)?;
        Some(match &session.lifecycle {
            DvmContainerLifecycle::AwaitingQuotes { .. } => SessionState::Provisioning {
                started_at: session.submitted_at,
            },
            DvmContainerLifecycle::Processing { accepted_at, .. } => SessionState::Running {
                started_at: *accepted_at,
                commands_completed: 0,
            },
            DvmContainerLifecycle::PendingSettlement { .. } => SessionState::Running {
                started_at: session.submitted_at,
                commands_completed: 0,
            },
            DvmContainerLifecycle::Settled { .. } => session
                .result
                .clone()
                .map(SessionState::Complete)
                .unwrap_or(SessionState::Running {
                    started_at: session.submitted_at,
                    commands_completed: 0,
                }),
            DvmContainerLifecycle::Failed { error, at } => SessionState::Failed {
                error: error.clone(),
                at: *at,
            },
        })
    }

    fn submit_exec(&self, _session_id: &str, _command: &str) -> Result<String, ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "interactive".to_string(),
            provider: "dvm".to_string(),
        })
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned()
    }

    fn poll_exec_output(&self, _exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "interactive".to_string(),
            provider: "dvm".to_string(),
        })
    }

    fn cancel_exec(&self, _exec_id: &str) -> Result<(), ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "interactive".to_string(),
            provider: "dvm".to_string(),
        })
    }

    fn read_file(
        &self,
        _session_id: &str,
        _path: &str,
        _offset: u64,
        _len: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "file_access".to_string(),
            provider: "dvm".to_string(),
        })
    }

    fn write_file(
        &self,
        _session_id: &str,
        _path: &str,
        _offset: u64,
        _data: &[u8],
    ) -> Result<(), ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "file_access".to_string(),
            provider: "dvm".to_string(),
        })
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let request_event_id = {
            let mut guard = self.sessions.write().unwrap_or_else(|e| e.into_inner());
            let session = guard
                .get_mut(session_id)
                .ok_or(ContainerError::SessionNotFound)?;
            session.lifecycle = DvmContainerLifecycle::Failed {
                error: "cancelled".to_string(),
                at: Timestamp::now(),
            };
            session.request_event_id.clone()
        };

        let tags = create_deletion_tags(&[request_event_id.as_str()], Some(KIND_JOB_SANDBOX_RUN));
        let event = self.sign_event(DELETION_REQUEST_KIND, tags, String::new())?;
        self.executor
            .block_on(self.transport.publish(event))
            .map_err(ContainerError::ProviderError)?;
        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let mut guard = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        let session = guard
            .get_mut(session_id)
            .ok_or(ContainerError::SessionNotFound)?;
        Ok(session.output.pop_front())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_dvm_container_feedback(
    session_id: &str,
    feedback: crate::dvm::DvmFeedback,
    sessions: &Arc<RwLock<HashMap<String, DvmContainerSession>>>,
    fx: &Arc<FxRateCache>,
    wallet: &Option<Arc<dyn WalletService>>,
) {
    let mut payment_request = None;

    {
        let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
        let Some(session) = guard.get_mut(session_id) else {
            return;
        };
        if matches!(
            session.lifecycle,
            DvmContainerLifecycle::Failed { .. } | DvmContainerLifecycle::Settled { .. }
        ) {
            return;
        }

        match feedback.status {
            DvmFeedbackStatus::Quote => {
                if let Some(amount_msats) = feedback.amount_msats {
                    let price_sats = msats_to_sats(amount_msats);
                    let price_usd = match fx.sats_to_usd(price_sats) {
                        Ok(price_usd) => price_usd,
                        Err(err) => {
                            session.lifecycle = DvmContainerLifecycle::Failed {
                                error: err.to_string(),
                                at: Timestamp::now(),
                            };
                            return;
                        }
                    };
                    let quote = DvmContainerQuote {
                        provider_pubkey: feedback.provider_pubkey.clone(),
                        price_sats,
                        price_usd,
                        event_id: feedback.event_id.clone(),
                    };
                    if let Some(existing) = session
                        .quotes
                        .iter_mut()
                        .find(|q| q.provider_pubkey == quote.provider_pubkey)
                    {
                        if quote.price_usd < existing.price_usd {
                            *existing = quote;
                        }
                    } else {
                        session.quotes.push(quote);
                    }
                }
            }
            DvmFeedbackStatus::Job(JobStatus::Partial) => {
                session.output.push_back(OutputChunk {
                    session_id: session_id.to_string(),
                    exec_id: None,
                    stream: OutputStream::Stdout,
                    data: feedback.content,
                });
            }
            DvmFeedbackStatus::Job(JobStatus::Processing) => {
                session.lifecycle = DvmContainerLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: feedback.provider_pubkey.clone(),
                };
            }
            DvmFeedbackStatus::Job(JobStatus::PaymentRequired) => {
                if session.payment_made {
                    return;
                }
                let invoice = feedback.bolt11.clone().or_else(|| {
                    let trimmed = feedback.content.trim();
                    if trimmed.starts_with("ln") {
                        Some(trimmed.to_string())
                    } else {
                        None
                    }
                });
                if let Some(invoice) = invoice {
                    payment_request =
                        Some((invoice, feedback.amount_msats, feedback.provider_pubkey));
                } else {
                    session.lifecycle = DvmContainerLifecycle::Failed {
                        error: "payment required but invoice missing".to_string(),
                        at: Timestamp::now(),
                    };
                }
            }
            DvmFeedbackStatus::Job(JobStatus::Error) => {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: feedback
                        .status_extra
                        .unwrap_or_else(|| "provider error".to_string()),
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
        let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
        if let Some(session) = guard.get_mut(session_id) {
            session.lifecycle = DvmContainerLifecycle::Failed {
                error: "wallet not configured".to_string(),
                at: Timestamp::now(),
            };
        }
        return;
    };
    let amount_sats = amount_msats.map(msats_to_sats);
    let wallet = Arc::clone(wallet);
    let payment = block_on_wallet(async move { wallet.pay_invoice(&invoice, amount_sats).await });
    match payment {
        Ok(payment) => {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.payment_made = true;
                session.paid_amount_sats = Some(payment.amount_sats);
                session.lifecycle = DvmContainerLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: provider_pubkey,
                };
            }
        }
        Err(err) => {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_dvm_container_result(
    session_id: &str,
    event: &nostr::Event,
    sessions: &Arc<RwLock<HashMap<String, DvmContainerSession>>>,
    fx: &Arc<FxRateCache>,
    wallet: &Option<Arc<dyn WalletService>>,
) {
    let result_event = match JobResult::from_event(event) {
        Ok(result) => result,
        Err(_) => return,
    };
    let invoice = result_event.bolt11.clone();
    let amount_sats = result_event.amount.map(msats_to_sats);

    let (response, already_paid) = {
        let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
        let Some(session) = guard.get_mut(session_id) else {
            return;
        };
        if matches!(session.lifecycle, DvmContainerLifecycle::Failed { .. }) {
            return;
        }
        let run = match SandboxRunResult::from_job_result(&result_event) {
            Ok(run) => run,
            Err(err) => {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
                return;
            }
        };

        let cost_sats = amount_sats
            .or(session.paid_amount_sats)
            .or_else(|| {
                session
                    .accepted_quote
                    .as_ref()
                    .map(|quote| quote.price_sats)
            })
            .unwrap_or(0);
        let cost_usd = match fx.sats_to_usd(cost_sats) {
            Ok(cost_usd) => cost_usd,
            Err(err) => {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
                return;
            }
        };

        let duration_ms = Timestamp::now()
            .as_millis()
            .saturating_sub(session.submitted_at.as_millis()) as u64;
        let command_results = run
            .command_results
            .into_iter()
            .map(|cmd| CommandResult {
                command: cmd.command,
                exit_code: cmd.exit_code,
                stdout: cmd.stdout,
                stderr: cmd.stderr,
                duration_ms: cmd.duration_ms,
            })
            .collect::<Vec<_>>();
        let artifacts = run
            .artifacts
            .into_iter()
            .map(|artifact| ArtifactInfo {
                path: artifact.path,
                size_bytes: artifact.size,
                sha256: artifact.sha256,
            })
            .collect::<Vec<_>>();
        let usage = ContainerUsage {
            cpu_time_ms: run.usage.cpu_time_ms,
            peak_memory_bytes: run.usage.peak_memory_bytes,
            disk_writes_bytes: run.usage.disk_writes_bytes,
            network_bytes: run.usage.network_bytes,
        };

        let response = ContainerResponse {
            session_id: session_id.to_string(),
            exit_code: Some(run.exit_code),
            stdout: run.stdout,
            stderr: run.stderr,
            command_results,
            artifacts,
            usage,
            cost_usd,
            reserved_usd: session.request.max_cost_usd.unwrap_or(0),
            duration_ms,
            provider_id: "dvm".to_string(),
        };
        session.result = Some(response.clone());
        if invoice.is_some() {
            session.lifecycle = DvmContainerLifecycle::PendingSettlement {
                result_at: Timestamp::now(),
                invoice: invoice.clone(),
            };
        } else {
            session.lifecycle = DvmContainerLifecycle::Settled {
                settled_at: Timestamp::now(),
            };
        }
        (response, session.payment_made)
    };

    if invoice.is_none() || already_paid {
        if already_paid {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.lifecycle = DvmContainerLifecycle::Settled {
                    settled_at: Timestamp::now(),
                };
            }
        }
        return;
    }

    let Some(wallet) = wallet.as_ref() else {
        let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
        if let Some(session) = guard.get_mut(session_id) {
            session.lifecycle = DvmContainerLifecycle::Failed {
                error: "wallet not configured".to_string(),
                at: Timestamp::now(),
            };
        }
        return;
    };
    let invoice = invoice.unwrap();
    let wallet = Arc::clone(wallet);
    let payment = block_on_wallet(async move { wallet.pay_invoice(&invoice, amount_sats).await });
    match payment {
        Ok(payment) => {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.payment_made = true;
                session.paid_amount_sats = Some(payment.amount_sats);
                session.result = Some(response);
                session.lifecycle = DvmContainerLifecycle::Settled {
                    settled_at: Timestamp::now(),
                };
            }
        }
        Err(err) => {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn join_workdir(repo_subdir: &Option<String>, workdir: &Option<String>) -> Option<String> {
    match (repo_subdir.as_ref(), workdir.as_ref()) {
        (Some(base), Some(extra)) => Some(format!(
            "{}/{}",
            base.trim_end_matches('/'),
            extra.trim_start_matches('/')
        )),
        (Some(base), None) => Some(base.clone()),
        (None, Some(extra)) => Some(extra.clone()),
        (None, None) => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct AsyncExecutor {
    runtime: Arc<tokio::runtime::Runtime>,
}

#[cfg(not(target_arch = "wasm32"))]
impl AsyncExecutor {
    fn new() -> Result<Self, ContainerError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
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

/// OpenAgents API-backed container provider (cloudflare/daytona).
pub struct OpenAgentsContainerProvider {
    provider_id: String,
    name: String,
    api: Arc<dyn OpenAgentsApiClient>,
    auth: Arc<OpenAgentsAuth>,
    session_cursors: Arc<Mutex<HashMap<String, String>>>,
    exec_cursors: Arc<Mutex<HashMap<String, String>>>,
}

impl OpenAgentsContainerProvider {
    pub fn new(
        provider_id: impl Into<String>,
        name: impl Into<String>,
        api: Arc<dyn OpenAgentsApiClient>,
        auth: Arc<OpenAgentsAuth>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            name: name.into(),
            api,
            auth,
            session_cursors: Arc::new(Mutex::new(HashMap::new())),
            exec_cursors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn cloudflare(api: Arc<dyn OpenAgentsApiClient>, auth: Arc<OpenAgentsAuth>) -> Self {
        Self::new("cloudflare", "Cloudflare Containers", api, auth)
    }

    pub fn daytona(api: Arc<dyn OpenAgentsApiClient>, auth: Arc<OpenAgentsAuth>) -> Self {
        Self::new("daytona", "Daytona Cloud Sandbox", api, auth)
    }

    fn require_token(&self) -> Result<String, ContainerError> {
        self.auth.token().ok_or(ContainerError::AuthRequired {
            provider: self.provider_id.clone(),
            message: "OpenAgents API token required".to_string(),
        })
    }
}

impl ContainerProvider for OpenAgentsContainerProvider {
    fn id(&self) -> &str {
        &self.provider_id
    }

    fn info(&self) -> ContainerProviderInfo {
        let token = self.auth.token();
        match self.api.provider_info(&self.provider_id, token.as_deref()) {
            Ok(info) => info,
            Err(err) => unavailable_provider_info(
                &self.provider_id,
                &self.name,
                format!("OpenAgents API error: {}", err),
            ),
        }
    }

    fn is_available(&self) -> bool {
        matches!(
            self.info().status,
            ProviderStatus::Available | ProviderStatus::Degraded { .. }
        )
    }

    fn requires_openagents_auth(&self) -> bool {
        true
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        let token = self.require_token()?;
        self.api
            .submit_container(&self.provider_id, &request, &token)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        let token = self.require_token().ok()?;
        match self.api.session_state(session_id, &token) {
            Ok(state) => Some(state),
            Err(ContainerError::SessionNotFound) => None,
            Err(err) => Some(SessionState::Failed {
                error: err.to_string(),
                at: Timestamp::now(),
            }),
        }
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let token = self.require_token()?;
        self.api.submit_exec(session_id, command, &token)
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        let token = self.require_token().ok()?;
        match self.api.exec_state(exec_id, &token) {
            Ok(state) => Some(state),
            Err(ContainerError::ExecNotFound) => None,
            Err(err) => Some(ExecState::Failed {
                error: err.to_string(),
                at: Timestamp::now(),
            }),
        }
    }

    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let token = self.require_token()?;
        let cursor = {
            let guard = self.exec_cursors.lock().unwrap_or_else(|e| e.into_inner());
            guard.get(exec_id).cloned()
        };
        let (chunk, next) = self
            .api
            .poll_exec_output(exec_id, cursor.as_deref(), &token)?;
        if let Some(next) = next {
            let mut guard = self.exec_cursors.lock().unwrap_or_else(|e| e.into_inner());
            guard.insert(exec_id.to_string(), next);
        }
        Ok(chunk)
    }

    fn cancel_exec(&self, _exec_id: &str) -> Result<(), ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "cancel_exec".to_string(),
            provider: self.provider_id.clone(),
        })
    }

    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        let token = self.require_token()?;
        self.api.read_file(session_id, path, offset, len, &token)
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ContainerError> {
        let token = self.require_token()?;
        self.api.write_file(session_id, path, offset, data, &token)
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let token = self.require_token()?;
        self.api.stop(session_id, &token)
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let token = self.require_token()?;
        let cursor = {
            let guard = self
                .session_cursors
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard.get(session_id).cloned()
        };
        let (chunk, next) = self
            .api
            .poll_output(session_id, cursor.as_deref(), &token)?;
        if let Some(next) = next {
            let mut guard = self
                .session_cursors
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard.insert(session_id.to_string(), next);
        }
        Ok(chunk)
    }
}

#[cfg(not(target_arch = "wasm32"))]
const DAYTONA_DEFAULT_SNAPSHOT: &str = "daytonaio/sandbox:latest";
#[cfg(not(target_arch = "wasm32"))]
const OUTPUT_CHUNK_SIZE: usize = 64 * 1024;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct DaytonaProviderConfig {
    base_url: String,
    snapshot: Option<String>,
    target: Option<String>,
    organization_id: Option<String>,
    auto_stop_minutes: Option<i32>,
    auto_delete_minutes: Option<i32>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DaytonaProviderConfig {
    fn from_env() -> (Self, Option<String>) {
        let base_url = std::env::var(DAYTONA_API_URL_ENV)
            .or_else(|_| std::env::var(DAYTONA_BASE_URL_ENV))
            .unwrap_or_else(|_| "https://api.daytona.io".to_string());
        let api_key = std::env::var(DAYTONA_API_KEY_ENV).ok();
        let snapshot = std::env::var(DAYTONA_SNAPSHOT_ENV)
            .or_else(|_| std::env::var(DAYTONA_DEFAULT_SNAPSHOT_ENV))
            .ok();
        let target = std::env::var(DAYTONA_TARGET_ENV).ok();
        let organization_id = std::env::var(DAYTONA_ORG_ID_ENV).ok();
        let auto_stop_minutes = parse_env_i32(DAYTONA_AUTO_STOP_ENV);
        let auto_delete_minutes = parse_env_i32(DAYTONA_AUTO_DELETE_ENV);
        (
            Self {
                base_url,
                snapshot,
                target,
                organization_id,
                auto_stop_minutes,
                auto_delete_minutes,
            },
            api_key,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_env_i32(name: &str) -> Option<i32> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_daytona_snapshot(request: &ContainerRequest, config: &DaytonaProviderConfig) -> String {
    request
        .image
        .clone()
        .or_else(|| config.snapshot.clone())
        .unwrap_or_else(|| DAYTONA_DEFAULT_SNAPSHOT.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn wrap_shell_command(command: &str) -> String {
    let shell_metacharacters = ['|', '>', '<', '&', ';', '$', '`', '(', ')', '{', '}', '\n'];
    if command.contains(shell_metacharacters.as_ref()) {
        let escaped = command.replace('\'', "'\"'\"'");
        format!("sh -lc '{}'", escaped)
    } else {
        command.to_string()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn is_daytona_snapshot_resource_error(err: &daytona::DaytonaError) -> bool {
    matches!(
        err,
        daytona::DaytonaError::ApiError { message, .. }
            if message.contains("Cannot specify Sandbox resources")
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn is_daytona_snapshot_not_found(err: &daytona::DaytonaError) -> bool {
    matches!(
        err,
        daytona::DaytonaError::ApiError { message, .. }
            if message.contains("Snapshot") && message.contains("not found")
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn join_daytona_path(base: &str, path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn push_output_chunks(
    queue: &mut VecDeque<OutputChunk>,
    session_id: &str,
    exec_id: Option<&str>,
    stream: OutputStream,
    data: &str,
) {
    if data.is_empty() {
        return;
    }
    let exec_id = exec_id.map(|id| id.to_string());
    for chunk in data.as_bytes().chunks(OUTPUT_CHUNK_SIZE) {
        let text = String::from_utf8_lossy(chunk).to_string();
        queue.push_back(OutputChunk {
            session_id: session_id.to_string(),
            exec_id: exec_id.clone(),
            stream: stream.clone(),
            data: text,
        });
    }
}

/// Daytona SDK-backed container provider.
#[cfg(not(target_arch = "wasm32"))]
pub struct DaytonaContainerProvider {
    client: Arc<DaytonaClient>,
    config: DaytonaProviderConfig,
    executor: AsyncExecutor,
    sessions: Arc<RwLock<HashMap<String, Arc<DaytonaSession>>>>,
    execs: Arc<RwLock<HashMap<String, Arc<DaytonaExec>>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DaytonaContainerProvider {
    pub fn from_env() -> Result<Option<Self>, ContainerError> {
        let (config, api_key) = DaytonaProviderConfig::from_env();
        let Some(api_key) = api_key else {
            return Ok(None);
        };
        let mut daytona_config = DaytonaConfig::with_api_key(api_key).base_url(&config.base_url);
        if let Some(org_id) = config.organization_id.clone() {
            daytona_config = daytona_config.organization_id(org_id);
        }
        let client = DaytonaClient::new(daytona_config)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        Self::new(client, config).map(Some)
    }

    fn new(client: DaytonaClient, config: DaytonaProviderConfig) -> Result<Self, ContainerError> {
        let executor = AsyncExecutor::new()?;
        Ok(Self {
            client: Arc::new(client),
            config,
            executor,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn session(&self, session_id: &str) -> Result<Arc<DaytonaSession>, ContainerError> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .cloned()
            .ok_or(ContainerError::SessionNotFound)
    }

    fn exec(&self, exec_id: &str) -> Result<Arc<DaytonaExec>, ContainerError> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned()
            .ok_or(ContainerError::ExecNotFound)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ContainerProvider for DaytonaContainerProvider {
    fn id(&self) -> &str {
        "daytona"
    }

    fn info(&self) -> ContainerProviderInfo {
        ContainerProviderInfo {
            id: "daytona".to_string(),
            name: "Daytona Cloud Sandbox".to_string(),
            available_images: Vec::new(),
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: true,
                interactive: true,
                artifacts: false,
                streaming: true,
            },
            pricing: None,
            latency: ContainerLatency {
                startup_ms: 5000,
                measured: false,
            },
            limits: ContainerLimits {
                max_memory_mb: 8192,
                max_cpu_cores: 4.0,
                max_disk_mb: 20_480,
                max_time_secs: 3600,
                network_allowed: true,
            },
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        true
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        if request
            .repo
            .as_ref()
            .and_then(|repo| repo.auth.as_ref())
            .is_some()
        {
            return Err(ContainerError::NotSupported {
                capability: "repo_auth".to_string(),
                provider: "daytona".to_string(),
            });
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let session = Arc::new(DaytonaSession::new(session_id.clone(), request));
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone(), session.clone());

        let client = self.client.clone();
        let config = self.config.clone();
        self.executor.spawn(async move {
            if let Err(err) = run_daytona_session(client, session.clone(), config).await {
                session.fail(&err.to_string());
            }
        });

        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .map(|session| {
                session
                    .state
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
            })
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let session = self.session(session_id)?;
        let sandbox_id = session.sandbox_id()?;
        let exec_id = uuid::Uuid::new_v4().to_string();
        let exec_id_clone = exec_id.clone();
        let exec = Arc::new(DaytonaExec::new());
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(exec_id.clone(), exec.clone());

        let client = self.client.clone();
        let execs = self.execs.clone();
        let command_string = command.to_string();
        let session_clone = session.clone();
        self.executor.spawn(async move {
            exec.running();
            let start = Instant::now();
            let wrapped = wrap_shell_command(&command_string);
            let mut request = ExecuteRequest::new(wrapped)
                .timeout(session_clone.request.limits.max_time_secs as i32);
            if let Some(workdir) = session_clone.workdir() {
                request = request.cwd(workdir);
            }

            match client.execute_command(&sandbox_id, &request).await {
                Ok(response) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    let result = CommandResult {
                        command: command_string.clone(),
                        exit_code: response.exit_code(),
                        stdout: response.result.clone(),
                        stderr: String::new(),
                        duration_ms,
                    };
                    exec.complete(result.clone());
                    exec.push_output(
                        &session_clone.session_id,
                        Some(&exec_id_clone),
                        OutputStream::Stdout,
                        &response.result,
                    );
                    session_clone.push_output(
                        Some(&exec_id_clone),
                        OutputStream::Stdout,
                        &response.result,
                    );
                }
                Err(err) => {
                    let message = err.to_string();
                    exec.fail(&message);
                    exec.push_output(
                        &session_clone.session_id,
                        Some(&exec_id_clone),
                        OutputStream::Stderr,
                        &message,
                    );
                    session_clone.push_output(Some(&exec_id_clone), OutputStream::Stderr, &message);
                }
            }

            execs
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&exec_id_clone);
        });

        Ok(exec_id)
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .map(|exec| exec.state.read().unwrap_or_else(|e| e.into_inner()).clone())
    }

    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let exec = self.exec(exec_id)?;
        Ok(exec.pop_output())
    }

    fn cancel_exec(&self, exec_id: &str) -> Result<(), ContainerError> {
        let exec = self.exec(exec_id)?;
        exec.fail("cancelled");
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(exec_id);
        Ok(())
    }

    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        let session = self.session(session_id)?;
        let sandbox_id = session.sandbox_id()?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let full_path = session.resolve_path(path);
        let data = self
            .executor
            .block_on(self.client.download_file(&sandbox_id, &full_path))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let start = offset as usize;
        if start >= data.len() {
            return Ok(Vec::new());
        }
        let end = (offset + len).min(data.len() as u64) as usize;
        Ok(data[start..end].to_vec())
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let sandbox_id = session.sandbox_id()?;
        let full_path = session.resolve_path(path);
        let payload = if offset == 0 {
            data.to_vec()
        } else {
            let mut existing = self
                .executor
                .block_on(self.client.download_file(&sandbox_id, &full_path))
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            let start = offset as usize;
            let end = start.saturating_add(data.len());
            if existing.len() < start {
                existing.resize(start, 0);
            }
            if existing.len() < end {
                existing.resize(end, 0);
            }
            existing[start..end].copy_from_slice(data);
            existing
        };
        self.executor
            .block_on(self.client.upload_file(&sandbox_id, &full_path, &payload))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let sandbox_id = session.sandbox_id()?;
        let _ = self
            .executor
            .block_on(self.client.stop_sandbox(&sandbox_id));
        let _ = self
            .executor
            .block_on(self.client.delete_sandbox(&sandbox_id, false));
        session.expire();
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session_id);
        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let session = self.session(session_id)?;
        Ok(session.pop_output())
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct DaytonaSession {
    session_id: String,
    request: ContainerRequest,
    state: RwLock<SessionState>,
    output: Mutex<VecDeque<OutputChunk>>,
    sandbox_id: Mutex<Option<String>>,
    workdir: Mutex<Option<String>>,
    start: Instant,
}

#[cfg(not(target_arch = "wasm32"))]
impl DaytonaSession {
    fn new(session_id: String, request: ContainerRequest) -> Self {
        Self {
            session_id: session_id.clone(),
            state: RwLock::new(SessionState::Provisioning {
                started_at: Timestamp::now(),
            }),
            output: Mutex::new(VecDeque::new()),
            request,
            sandbox_id: Mutex::new(None),
            workdir: Mutex::new(None),
            start: Instant::now(),
        }
    }

    fn sandbox_id(&self) -> Result<String, ContainerError> {
        self.sandbox_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or(ContainerError::NotReady)
    }

    fn set_sandbox_id(&self, id: String) {
        *self.sandbox_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
    }

    fn set_workdir(&self, workdir: Option<String>) {
        *self.workdir.lock().unwrap_or_else(|e| e.into_inner()) = workdir;
    }

    fn workdir(&self) -> Option<String> {
        self.workdir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn resolve_path(&self, path: &str) -> String {
        match self.workdir() {
            Some(base) => join_daytona_path(&base, path),
            None => path.to_string(),
        }
    }

    fn push_output(&self, exec_id: Option<&str>, stream: OutputStream, data: &str) {
        let mut guard = self.output.lock().unwrap_or_else(|e| e.into_inner());
        push_output_chunks(&mut guard, &self.session_id, exec_id, stream, data);
    }

    fn pop_output(&self) -> Option<OutputChunk> {
        self.output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
    }

    fn set_state(&self, state: SessionState) {
        let mut guard = self.state.write().unwrap_or_else(|e| e.into_inner());
        *guard = state;
    }

    fn fail(&self, message: &str) {
        self.set_state(SessionState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        });
    }

    fn expire(&self) {
        self.set_state(SessionState::Expired {
            at: Timestamp::now(),
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct DaytonaExec {
    state: RwLock<ExecState>,
    output: Mutex<VecDeque<OutputChunk>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DaytonaExec {
    fn new() -> Self {
        Self {
            state: RwLock::new(ExecState::Pending {
                submitted_at: Timestamp::now(),
            }),
            output: Mutex::new(VecDeque::new()),
        }
    }

    fn running(&self) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Running {
            started_at: Timestamp::now(),
        };
    }

    fn complete(&self, result: CommandResult) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Complete(result);
    }

    fn fail(&self, message: &str) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        };
    }

    fn push_output(
        &self,
        session_id: &str,
        exec_id: Option<&str>,
        stream: OutputStream,
        data: &str,
    ) {
        let mut guard = self.output.lock().unwrap_or_else(|e| e.into_inner());
        push_output_chunks(&mut guard, session_id, exec_id, stream, data);
    }

    fn pop_output(&self) -> Option<OutputChunk> {
        self.output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_daytona_session(
    client: Arc<DaytonaClient>,
    session: Arc<DaytonaSession>,
    config: DaytonaProviderConfig,
) -> Result<(), ContainerError> {
    let request = session.request.clone();
    let snapshot = resolve_daytona_snapshot(&request, &config);

    let mut create = CreateSandbox::new(snapshot.clone());
    if let Some(target) = config.target.as_ref() {
        create = create.target(target.clone());
    }
    if !request.env.is_empty() {
        create = create.env(request.env.clone());
    }
    if let Some(minutes) = config.auto_stop_minutes {
        create = create.auto_stop_interval(minutes);
    }
    if let Some(minutes) = config.auto_delete_minutes {
        create = create.auto_delete_interval(minutes);
    }

    let mut labels = HashMap::new();
    labels.insert(
        "openagents_session_id".to_string(),
        session.session_id.clone(),
    );
    create = create.labels(labels);

    let base_create = create.clone();
    let cpu = request.limits.max_cpu_cores.ceil().max(1.0) as i32;
    let memory_gb = ((request.limits.max_memory_mb as f64) / 1024.0).ceil() as i32;
    let disk_gb = ((request.limits.max_disk_mb as f64) / 1024.0).ceil() as i32;
    if cpu > 0 {
        create = create.cpu(cpu);
    }
    if memory_gb > 0 {
        create = create.memory(memory_gb);
    }
    if disk_gb > 0 {
        create = create.disk(disk_gb);
    }

    let sandbox = match client.create_sandbox(&create).await {
        Ok(sandbox) => sandbox,
        Err(err) if is_daytona_snapshot_resource_error(&err) => {
            match client.create_sandbox(&base_create).await {
                Ok(sandbox) => sandbox,
                Err(err) if is_daytona_snapshot_not_found(&err) => {
                    return Err(ContainerError::ProviderError(
                        "Daytona snapshot not found. Set DAYTONA_SNAPSHOT to a snapshot you can access."
                            .to_string(),
                    ));
                }
                Err(err) => return Err(ContainerError::ProviderError(err.to_string())),
            }
        }
        Err(err) if is_daytona_snapshot_not_found(&err) => {
            return Err(ContainerError::ProviderError(
                "Daytona snapshot not found. Set DAYTONA_SNAPSHOT to a snapshot you can access."
                    .to_string(),
            ));
        }
        Err(err) => return Err(ContainerError::ProviderError(err.to_string())),
    };
    session.set_sandbox_id(sandbox.id.clone());

    client
        .start_sandbox(&sandbox.id)
        .await
        .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

    let timeout_ms = request.timeout_ms.unwrap_or_else(default_timeout_ms);
    client
        .wait_for_state(
            &sandbox.id,
            DaytonaSandboxState::Started,
            Duration::from_millis(timeout_ms),
        )
        .await
        .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

    session.set_state(SessionState::Running {
        started_at: Timestamp::now(),
        commands_completed: 0,
    });

    let mut workdir = request.workdir.clone();
    if let Some(repo) = request.repo.clone() {
        session.set_state(SessionState::Cloning {
            started_at: Timestamp::now(),
            repo_url: repo.url.clone(),
        });

        let project_dir = client
            .get_project_dir(&sandbox.id)
            .await
            .unwrap_or_else(|_| "/workspace".to_string());
        let mut clone_request = GitCloneRequest::new(repo.url.clone(), project_dir.clone());
        if repo.git_ref.len() >= 40 && repo.git_ref.chars().all(|c| c.is_ascii_hexdigit()) {
            clone_request = clone_request.commit_id(repo.git_ref.clone());
        } else {
            clone_request = clone_request.branch(repo.git_ref.clone());
        }
        client
            .git_clone(&sandbox.id, &clone_request)
            .await
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

        let suffix = join_workdir(&repo.subdir, &request.workdir);
        workdir = Some(match suffix {
            Some(path) => join_daytona_path(&project_dir, &path),
            None => project_dir,
        });

        session.set_state(SessionState::Running {
            started_at: Timestamp::now(),
            commands_completed: 0,
        });
    } else if workdir.is_none() {
        if let Ok(project_dir) = client.get_project_dir(&sandbox.id).await {
            workdir = Some(project_dir);
        }
    }

    session.set_workdir(workdir.clone());

    let mut command_results = Vec::new();
    let mut combined_exit = 0;
    for (idx, command) in request.commands.iter().enumerate() {
        let start = Instant::now();
        let wrapped = wrap_shell_command(command);
        let mut exec_request =
            ExecuteRequest::new(wrapped).timeout(request.limits.max_time_secs as i32);
        if let Some(dir) = workdir.clone() {
            exec_request = exec_request.cwd(dir);
        }

        let response = client
            .execute_command(&sandbox.id, &exec_request)
            .await
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let result = CommandResult {
            command: command.clone(),
            exit_code: response.exit_code(),
            stdout: response.result.clone(),
            stderr: String::new(),
            duration_ms,
        };
        session.push_output(None, OutputStream::Stdout, &response.result);
        command_results.push(result.clone());

        if response.exit_code() != 0 {
            session.fail("command failed");
            return Ok(());
        }

        combined_exit = response.exit_code();
        session.set_state(SessionState::Running {
            started_at: Timestamp::now(),
            commands_completed: idx + 1,
        });
    }

    if matches!(request.kind, ContainerKind::Interactive) {
        return Ok(());
    }

    let stdout = command_results
        .iter()
        .map(|r| r.stdout.clone())
        .collect::<Vec<_>>()
        .join("");
    let stderr = command_results
        .iter()
        .map(|r| r.stderr.clone())
        .collect::<Vec<_>>()
        .join("");
    let response = ContainerResponse {
        session_id: session.session_id.clone(),
        exit_code: Some(combined_exit),
        stdout,
        stderr,
        command_results,
        artifacts: Vec::new(),
        usage: ContainerUsage::zero(),
        cost_usd: request.max_cost_usd.unwrap_or(0),
        reserved_usd: request.max_cost_usd.unwrap_or(0),
        duration_ms: session.start.elapsed().as_millis() as u64,
        provider_id: "daytona".to_string(),
    };
    session.set_state(SessionState::Complete(response));
    Ok(())
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
#[derive(Default)]
struct RemoteSessionState {
    remote_id: Option<String>,
    cursor: Option<String>,
    queue: VecDeque<OutputChunk>,
    refreshing: bool,
    streaming: bool,
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
#[derive(Default)]
struct RemoteExecState {
    remote_id: Option<String>,
    cursor: Option<String>,
    queue: VecDeque<OutputChunk>,
    session_id: String,
    refreshing: bool,
    streaming: bool,
}

/// OpenAgents API-backed container provider for browser targets.
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub struct WasmOpenAgentsContainerProvider {
    provider_id: String,
    name: String,
    base_url: String,
    auth: Arc<OpenAgentsAuth>,
    info: Arc<RwLock<ContainerProviderInfo>>,
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
    execs: Arc<RwLock<HashMap<String, ExecState>>>,
    remote_sessions: Arc<Mutex<HashMap<String, RemoteSessionState>>>,
    remote_execs: Arc<Mutex<HashMap<String, RemoteExecState>>>,
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
impl WasmOpenAgentsContainerProvider {
    /// Create a new OpenAgents API-backed provider (browser).
    pub fn new(
        provider_id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        auth: Arc<OpenAgentsAuth>,
    ) -> Self {
        let provider_id = provider_id.into();
        let name = name.into();
        let info = ContainerProviderInfo {
            id: provider_id.clone(),
            name: name.clone(),
            available_images: Vec::new(),
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: false,
                interactive: false,
                artifacts: false,
                streaming: true,
            },
            pricing: None,
            latency: ContainerLatency {
                startup_ms: 0,
                measured: false,
            },
            limits: ContainerLimits {
                max_memory_mb: 8192,
                max_cpu_cores: 4.0,
                max_disk_mb: 10240,
                max_time_secs: 3600,
                network_allowed: true,
            },
            status: ProviderStatus::Degraded {
                reason: "loading provider info".to_string(),
            },
        };
        let provider = Self {
            provider_id,
            name,
            base_url: base_url.into(),
            auth,
            info: Arc::new(RwLock::new(info)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
            remote_sessions: Arc::new(Mutex::new(HashMap::new())),
            remote_execs: Arc::new(Mutex::new(HashMap::new())),
        };
        provider.spawn_info_refresh();
        provider
    }

    pub fn cloudflare(base_url: impl Into<String>, auth: Arc<OpenAgentsAuth>) -> Self {
        Self::new("cloudflare", "Cloudflare Containers", base_url, auth)
    }

    pub fn daytona(base_url: impl Into<String>, auth: Arc<OpenAgentsAuth>) -> Self {
        Self::new("daytona", "Daytona Cloud Sandbox", base_url, auth)
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn require_token(&self) -> Result<String, ContainerError> {
        self.auth.token().ok_or(ContainerError::AuthRequired {
            provider: self.provider_id.clone(),
            message: "OpenAgents API token required".to_string(),
        })
    }

    fn spawn_info_refresh(&self) {
        let info = Arc::clone(&self.info);
        let provider_id = self.provider_id.clone();
        let name = self.name.clone();
        let url = self.url(&format!("containers/providers/{}/info", provider_id));
        let auth = Arc::clone(&self.auth);
        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let updated = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    serde_json::from_slice::<ContainerProviderInfo>(&bytes).unwrap_or_else(|err| {
                        unavailable_provider_info(
                            &provider_id,
                            &name,
                            format!("invalid provider info: {}", err),
                        )
                    })
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    unavailable_provider_info(
                        &provider_id,
                        &name,
                        format!("openagents api {}: {}", status, body),
                    )
                }
                Err(err) => unavailable_provider_info(&provider_id, &name, err),
            };
            let mut guard = info.write().unwrap_or_else(|e| e.into_inner());
            *guard = updated;
        });
    }

    fn spawn_session_refresh(&self, session_id: &str) {
        let (remote_id, url, auth, sessions, remote_sessions, session_id) = {
            let mut guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(session_id) {
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
            let url = self.url(&format!("containers/sessions/{}", remote_id));
            (
                remote_id,
                url,
                Arc::clone(&self.auth),
                Arc::clone(&self.sessions),
                Arc::clone(&self.remote_sessions),
                session_id.to_string(),
            )
        };

        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let next_state = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    serde_json::from_slice::<SessionState>(&bytes)
                        .map_err(|err| format!("invalid session state: {}", err))
                }
                Ok((404, _)) => Err("session not found".to_string()),
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    Err(format!("openagents api {}: {}", status, body))
                }
                Err(err) => Err(err),
            };

            match next_state {
                Ok(state) => {
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(session_id.clone(), state);
                }
                Err(err) => {
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        session_id.clone(),
                        SessionState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }

            let mut guard = remote_sessions.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&session_id) {
                state.refreshing = false;
                state.remote_id = Some(remote_id);
            }
        });
    }

    fn spawn_session_output_poll(&self, session_id: &str) {
        let (url, auth, sessions, remote_sessions, session_id, cursor) = {
            let mut guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(session_id) {
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
                    "containers/sessions/{}/output?cursor={}",
                    remote_id, cursor
                )),
                None => self.url(&format!("containers/sessions/{}/output", remote_id)),
            };
            (
                url,
                Arc::clone(&self.auth),
                Arc::clone(&self.sessions),
                Arc::clone(&self.remote_sessions),
                session_id.to_string(),
                cursor,
            )
        };

        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let mut next_chunk: Option<OutputChunk> = None;
            let mut next_cursor = cursor.clone();
            let mut error: Option<String> = None;

            match response {
                Ok((status, bytes)) if status == 204 || bytes.is_empty() => {}
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    #[derive(Deserialize)]
                    struct OutputResponse {
                        chunk: Option<OutputChunk>,
                        cursor: Option<String>,
                    }
                    if let Ok(payload) = serde_json::from_slice::<OutputResponse>(&bytes) {
                        next_chunk = payload.chunk;
                        next_cursor = payload.cursor.or(next_cursor);
                    } else if let Ok(chunk) = serde_json::from_slice::<OutputChunk>(&bytes) {
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
                let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                guard.insert(
                    session_id.clone(),
                    SessionState::Failed {
                        error: err,
                        at: Timestamp::now(),
                    },
                );
            } else if let Some(chunk) = next_chunk {
                {
                    let mut guard = remote_sessions.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(state) = guard.get_mut(&session_id) {
                        state.queue.push_back(chunk);
                        state.cursor = next_cursor.clone();
                    }
                }
            }

            let mut guard = remote_sessions.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&session_id) {
                state.streaming = false;
                state.cursor = next_cursor;
            }
        });
    }

    fn spawn_exec_refresh(&self, exec_id: &str) {
        let (remote_id, url, auth, execs, remote_execs, exec_id) = {
            let mut guard = self.remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(exec_id) {
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
            let url = self.url(&format!("containers/exec/{}", remote_id));
            (
                remote_id,
                url,
                Arc::clone(&self.auth),
                Arc::clone(&self.execs),
                Arc::clone(&self.remote_execs),
                exec_id.to_string(),
            )
        };

        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let next_state = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    serde_json::from_slice::<ExecState>(&bytes)
                        .map_err(|err| format!("invalid exec state: {}", err))
                }
                Ok((404, _)) => Err("exec not found".to_string()),
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    Err(format!("openagents api {}: {}", status, body))
                }
                Err(err) => Err(err),
            };

            match next_state {
                Ok(state) => {
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(exec_id.clone(), state);
                }
                Err(err) => {
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        exec_id.clone(),
                        ExecState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }

            let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&exec_id) {
                state.refreshing = false;
                state.remote_id = Some(remote_id);
            }
        });
    }

    fn spawn_exec_output_poll(&self, exec_id: &str) {
        let (url, auth, remote_execs, exec_id, cursor) = {
            let mut guard = self.remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(exec_id) {
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
                    "containers/exec/{}/output?cursor={}",
                    remote_id, cursor
                )),
                None => self.url(&format!("containers/exec/{}/output", remote_id)),
            };
            (
                url,
                Arc::clone(&self.auth),
                Arc::clone(&self.remote_execs),
                exec_id.to_string(),
                cursor,
            )
        };

        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let mut next_chunk: Option<OutputChunk> = None;
            let mut next_cursor = cursor.clone();
            let mut error: Option<String> = None;

            match response {
                Ok((status, bytes)) if status == 204 || bytes.is_empty() => {}
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    #[derive(Deserialize)]
                    struct OutputResponse {
                        chunk: Option<OutputChunk>,
                        cursor: Option<String>,
                    }
                    if let Ok(payload) = serde_json::from_slice::<OutputResponse>(&bytes) {
                        next_chunk = payload.chunk;
                        next_cursor = payload.cursor.or(next_cursor);
                    } else if let Ok(chunk) = serde_json::from_slice::<OutputChunk>(&bytes) {
                        next_chunk = Some(chunk);
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    error = Some(format!("openagents api {}: {}", status, body));
                }
                Err(err) => error = Some(err),
            }

            if let Some(chunk) = next_chunk {
                let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(state) = guard.get_mut(&exec_id) {
                    state.queue.push_back(chunk);
                    state.cursor = next_cursor.clone();
                }
            }

            if error.is_some() {
                let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(state) = guard.get_mut(&exec_id) {
                    state.queue.push_back(OutputChunk {
                        session_id: exec_id.clone(),
                        exec_id: Some(exec_id.clone()),
                        stream: OutputStream::Stderr,
                        data: error.unwrap_or_else(|| "exec output error".to_string()),
                    });
                }
            }

            let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&exec_id) {
                state.streaming = false;
                state.cursor = next_cursor;
            }
        });
    }
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
impl ContainerProvider for WasmOpenAgentsContainerProvider {
    fn id(&self) -> &str {
        &self.provider_id
    }

    fn info(&self) -> ContainerProviderInfo {
        self.info.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn is_available(&self) -> bool {
        let info = self.info.read().unwrap_or_else(|e| e.into_inner());
        matches!(
            info.status,
            ProviderStatus::Available | ProviderStatus::Degraded { .. }
        )
    }

    fn requires_openagents_auth(&self) -> bool {
        true
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        let token = self.require_token()?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let started_at = Timestamp::now();
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.clone(),
                SessionState::Provisioning { started_at },
            );
        self.remote_sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone(), RemoteSessionState::default());

        let url = self.url(&format!(
            "containers/providers/{}/sessions",
            self.provider_id
        ));
        let sessions = Arc::clone(&self.sessions);
        let remote_sessions = Arc::clone(&self.remote_sessions);
        let session_id_clone = session_id.clone();

        spawn_local(async move {
            let body = match serde_json::to_string(&request) {
                Ok(body) => body,
                Err(err) => {
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        session_id_clone.clone(),
                        SessionState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        },
                    );
                    return;
                }
            };
            let response = wasm_http::request_bytes("POST", &url, Some(&token), Some(body)).await;
            #[derive(Deserialize)]
            struct SessionResponse {
                session_id: String,
            }
            match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    match serde_json::from_slice::<SessionResponse>(&bytes) {
                        Ok(payload) => {
                            let mut guard =
                                remote_sessions.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(state) = guard.get_mut(&session_id_clone) {
                                state.remote_id = Some(payload.session_id);
                            }
                        }
                        Err(err) => {
                            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                            guard.insert(
                                session_id_clone.clone(),
                                SessionState::Failed {
                                    error: format!("invalid response: {}", err),
                                    at: Timestamp::now(),
                                },
                            );
                        }
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        session_id_clone.clone(),
                        SessionState::Failed {
                            error: format!("openagents api {}: {}", status, body),
                            at: Timestamp::now(),
                        },
                    );
                }
                Err(err) => {
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        session_id_clone.clone(),
                        SessionState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }
        });

        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        let state = self
            .sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .cloned();
        if let Some(state) = state.as_ref() {
            if !matches!(
                state,
                SessionState::Complete(_)
                    | SessionState::Failed { .. }
                    | SessionState::Expired { .. }
            ) {
                self.spawn_session_refresh(session_id);
            }
        }
        state
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let token = self.require_token()?;
        let remote_id = {
            let guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard
                .get(session_id)
                .and_then(|state| state.remote_id.clone())
        }
        .ok_or_else(|| ContainerError::InvalidRequest("session not ready".to_string()))?;

        let exec_id = uuid::Uuid::new_v4().to_string();
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                exec_id.clone(),
                ExecState::Pending {
                    submitted_at: Timestamp::now(),
                },
            );
        self.remote_execs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                exec_id.clone(),
                RemoteExecState {
                    session_id: session_id.to_string(),
                    ..RemoteExecState::default()
                },
            );

        let url = self.url(&format!("containers/sessions/{}/exec", remote_id));
        let execs = Arc::clone(&self.execs);
        let remote_execs = Arc::clone(&self.remote_execs);
        let exec_id_clone = exec_id.clone();
        let command = command.to_string();

        spawn_local(async move {
            let body = serde_json::json!({ "command": command });
            let body = match serde_json::to_string(&body) {
                Ok(body) => body,
                Err(err) => {
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        exec_id_clone.clone(),
                        ExecState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        },
                    );
                    return;
                }
            };
            let response = wasm_http::request_bytes("POST", &url, Some(&token), Some(body)).await;
            #[derive(Deserialize)]
            struct ExecResponse {
                exec_id: String,
            }
            match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    match serde_json::from_slice::<ExecResponse>(&bytes) {
                        Ok(payload) => {
                            let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(state) = guard.get_mut(&exec_id_clone) {
                                state.remote_id = Some(payload.exec_id);
                            }
                            let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                            guard.insert(
                                exec_id_clone.clone(),
                                ExecState::Running {
                                    started_at: Timestamp::now(),
                                },
                            );
                        }
                        Err(err) => {
                            let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                            guard.insert(
                                exec_id_clone.clone(),
                                ExecState::Failed {
                                    error: format!("invalid response: {}", err),
                                    at: Timestamp::now(),
                                },
                            );
                        }
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        exec_id_clone.clone(),
                        ExecState::Failed {
                            error: format!("openagents api {}: {}", status, body),
                            at: Timestamp::now(),
                        },
                    );
                }
                Err(err) => {
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        exec_id_clone.clone(),
                        ExecState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }
        });

        Ok(exec_id)
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        let state = self
            .execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned();
        if let Some(state) = state.as_ref() {
            if !matches!(state, ExecState::Complete(_) | ExecState::Failed { .. }) {
                self.spawn_exec_refresh(exec_id);
            }
        }
        state
    }

    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let mut chunk = None;
        {
            let mut guard = self.remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(exec_id) {
                chunk = state.queue.pop_front();
            }
        }
        if chunk.is_some() {
            return Ok(chunk);
        }
        self.spawn_exec_output_poll(exec_id);
        Ok(None)
    }

    fn cancel_exec(&self, _exec_id: &str) -> Result<(), ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "cancel_exec".to_string(),
            provider: self.provider_id.clone(),
        })
    }

    fn read_file(
        &self,
        _session_id: &str,
        _path: &str,
        _offset: u64,
        _len: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "file_access".to_string(),
            provider: self.provider_id.clone(),
        })
    }

    fn write_file(
        &self,
        _session_id: &str,
        _path: &str,
        _offset: u64,
        _data: &[u8],
    ) -> Result<(), ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "file_access".to_string(),
            provider: self.provider_id.clone(),
        })
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let remote_id = {
            let guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard
                .get(session_id)
                .and_then(|state| state.remote_id.clone())
        }
        .ok_or_else(|| ContainerError::SessionNotFound)?;

        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        sessions.insert(
            session_id.to_string(),
            SessionState::Failed {
                error: "cancelled".to_string(),
                at: Timestamp::now(),
            },
        );

        let url = self.url(&format!("containers/sessions/{}/stop", remote_id));
        let auth = Arc::clone(&self.auth);
        spawn_local(async move {
            if let Some(token) = auth.token() {
                let _ = wasm_http::request_bytes("POST", &url, Some(&token), None).await;
            }
        });
        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let mut chunk = None;
        {
            let mut guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(session_id) {
                chunk = state.queue.pop_front();
            }
        }
        if chunk.is_some() {
            return Ok(chunk);
        }
        self.spawn_session_output_poll(session_id);
        Ok(None)
    }
}

/// Apple Container provider (macOS 26+).
#[cfg(all(target_os = "macos", feature = "apple-container"))]
pub struct AppleContainerProvider {
    sessions: Arc<RwLock<HashMap<String, Arc<AppleSession>>>>,
    execs: Arc<RwLock<HashMap<String, Arc<AppleExec>>>>,
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
impl AppleContainerProvider {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn ensure_available() -> Result<(), ContainerError> {
        Self::availability_status().map_err(ContainerError::Unavailable)
    }

    fn availability_status() -> Result<(), String> {
        if !Self::macos_supported() {
            return Err("macOS 26+ required for Apple Container".to_string());
        }
        Self::container_status()
    }

    fn macos_supported() -> bool {
        let output = Command::new("sw_vers").arg("-productVersion").output();
        let output = match output {
            Ok(output) if output.status.success() => output,
            _ => return false,
        };
        let version = String::from_utf8_lossy(&output.stdout);
        let mut parts = version.trim().split('.');
        let major = Self::parse_version_part(parts.next().unwrap_or(""));
        major.map_or(false, |major| major >= 26)
    }

    fn parse_version_part(value: &str) -> Option<u32> {
        let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse::<u32>().ok()
    }

    fn container_status() -> Result<(), String> {
        let output = Command::new("container")
            .args(["system", "status"])
            .output()
            .map_err(|err| {
                if err.kind() == std::io::ErrorKind::NotFound {
                    "container CLI not available".to_string()
                } else {
                    err.to_string()
                }
            })?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Err(if !stderr.is_empty() { stderr } else { stdout })
        }
    }

    fn container_images() -> Result<Vec<String>, ContainerError> {
        let output = Command::new("container")
            .args(["image", "list", "--quiet"])
            .output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if !output.status.success() {
            return Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let images = text
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        Ok(images)
    }

    fn session(&self, session_id: &str) -> Result<Arc<AppleSession>, ContainerError> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .cloned()
            .ok_or(ContainerError::SessionNotFound)
    }

    fn exec(&self, exec_id: &str) -> Result<Arc<AppleExec>, ContainerError> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned()
            .ok_or(ContainerError::ExecNotFound)
    }
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
impl Default for AppleContainerProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
impl ContainerProvider for AppleContainerProvider {
    fn id(&self) -> &str {
        "apple"
    }

    fn info(&self) -> ContainerProviderInfo {
        let available = Self::availability_status();
        let images = if available.is_ok() {
            Self::container_images().unwrap_or_default()
        } else {
            Vec::new()
        };
        ContainerProviderInfo {
            id: "apple".to_string(),
            name: "Apple Container".to_string(),
            available_images: images,
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: true,
                interactive: true,
                artifacts: false,
                streaming: true,
            },
            pricing: None,
            latency: ContainerLatency {
                startup_ms: 0,
                measured: false,
            },
            limits: ContainerLimits {
                max_memory_mb: 16384,
                max_cpu_cores: 8.0,
                max_disk_mb: 10240,
                max_time_secs: 3600,
                network_allowed: true,
            },
            status: match available {
                Ok(_) => ProviderStatus::Available,
                Err(reason) => ProviderStatus::Unavailable { reason },
            },
        }
    }

    fn is_available(&self) -> bool {
        Self::availability_status().is_ok()
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        Self::ensure_available()?;
        let image = request
            .image
            .clone()
            .ok_or_else(|| ContainerError::InvalidRequest("image required".to_string()))?;

        if request
            .repo
            .as_ref()
            .and_then(|r| r.auth.as_ref())
            .is_some()
        {
            return Err(ContainerError::NotSupported {
                capability: "repo_auth".to_string(),
                provider: "apple".to_string(),
            });
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let session_id_clone = session_id.clone();
        let (output_tx, output_rx) = mpsc::channel(256);
        let session = Arc::new(AppleSession::new(
            session_id.clone(),
            request.clone(),
            output_tx,
            output_rx,
        ));

        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone(), session.clone());

        let provider_sessions = self.sessions.clone();
        thread::spawn(move || {
            if let Err(err) = session.run(image) {
                session.fail(&err.to_string());
                provider_sessions
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&session_id_clone);
            }
        });

        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .map(|session| {
                session
                    .state
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
            })
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        let exec_id = uuid::Uuid::new_v4().to_string();
        let exec_id_clone = exec_id.clone();
        let (output_tx, output_rx) = mpsc::channel(128);
        let exec = Arc::new(AppleExec::new(
            exec_id.clone(),
            session_id.to_string(),
            output_tx,
            output_rx,
        ));
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(exec_id.clone(), exec.clone());

        let provider_execs = self.execs.clone();
        let session_clone = session.clone();
        let command_string = command.to_string();
        thread::spawn(move || {
            exec.running();
            let result = run_apple_exec_command(
                &container_id,
                &session_clone.session_id,
                &command_string,
                session_clone.request.workdir.clone(),
                session_clone.request.env.clone(),
                session_clone.request.limits.max_time_secs,
                &exec.output_tx,
                Some(exec_id_clone.clone()),
                &session_clone.output_tx,
            );
            match result {
                Ok(command_result) => exec.complete(command_result),
                Err(err) => exec.fail(&err.to_string()),
            }
            provider_execs
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&exec_id_clone);
        });

        Ok(exec_id)
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .map(|exec| exec.state.read().unwrap_or_else(|e| e.into_inner()).clone())
    }

    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let exec = self.exec(exec_id)?;
        let mut rx = exec.output_rx.lock().unwrap_or_else(|e| e.into_inner());
        match rx.try_recv() {
            Ok(chunk) => Ok(Some(chunk)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }

    fn cancel_exec(&self, exec_id: &str) -> Result<(), ContainerError> {
        let exec = self.exec(exec_id)?;
        exec.fail("cancelled");
        Ok(())
    }

    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let escaped = shell_escape(path);
        let script = format!(
            "dd if={} bs=1 skip={} count={} 2>/dev/null",
            escaped, offset, len
        );
        let output = Command::new("container")
            .args(["exec", &container_id, "sh", "-lc", &script])
            .output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        let escaped = shell_escape(path);
        if offset == 0 {
            let truncate = format!("truncate -s 0 {} 2>/dev/null || : ", escaped);
            let _ = Command::new("container")
                .args(["exec", &container_id, "sh", "-lc", &truncate])
                .output();
        }
        let script = format!(
            "dd of={} bs=1 seek={} conv=notrunc 2>/dev/null",
            escaped, offset
        );
        let mut child = Command::new("container")
            .args(["exec", "-i", &container_id, "sh", "-lc", &script])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(data)
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        let _ = Command::new("container")
            .args(["stop", &container_id])
            .output();
        let _ = Command::new("container")
            .args(["delete", "--force", &container_id])
            .output();
        session.expire();
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session_id);
        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let session = self.session(session_id)?;
        let mut rx = session.output_rx.lock().unwrap_or_else(|e| e.into_inner());
        match rx.try_recv() {
            Ok(chunk) => Ok(Some(chunk)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
struct AppleSession {
    session_id: String,
    state: RwLock<SessionState>,
    output_tx: mpsc::Sender<OutputChunk>,
    output_rx: Mutex<mpsc::Receiver<OutputChunk>>,
    request: ContainerRequest,
    container_id: Mutex<Option<String>>,
    start: Instant,
    repo_dir: Mutex<Option<tempfile::TempDir>>,
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
impl AppleSession {
    fn new(
        session_id: String,
        request: ContainerRequest,
        output_tx: mpsc::Sender<OutputChunk>,
        output_rx: mpsc::Receiver<OutputChunk>,
    ) -> Self {
        Self {
            session_id: session_id.clone(),
            state: RwLock::new(SessionState::Provisioning {
                started_at: Timestamp::now(),
            }),
            output_tx,
            output_rx: Mutex::new(output_rx),
            request,
            container_id: Mutex::new(None),
            start: Instant::now(),
            repo_dir: Mutex::new(None),
        }
    }

    fn container_id(&self) -> Result<String, ContainerError> {
        self.container_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or(ContainerError::SessionNotFound)
    }

    fn set_container_id(&self, id: String) {
        *self.container_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
    }

    fn run(&self, image: String) -> Result<(), ContainerError> {
        let mut workdir = self.request.workdir.clone();
        if let Some(repo) = self.request.repo.clone() {
            let repo_dir = tempfile::TempDir::new()
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            {
                let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
                *state = SessionState::Cloning {
                    started_at: Timestamp::now(),
                    repo_url: repo.url.clone(),
                };
            }
            let status = Command::new("git")
                .args([
                    "clone",
                    "--depth",
                    "1",
                    "--branch",
                    &repo.git_ref,
                    &repo.url,
                    repo_dir.path().to_str().unwrap_or_default(),
                ])
                .status()
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            if !status.success() {
                return Err(ContainerError::ProviderError(
                    "git clone failed".to_string(),
                ));
            }
            let mut repo_guard = self.repo_dir.lock().unwrap_or_else(|e| e.into_inner());
            *repo_guard = Some(repo_dir);
            let base = "/workspace".to_string();
            workdir = Some(match repo.subdir {
                Some(subdir) => format!("{}/{}", base, subdir.trim_matches('/')),
                None => base,
            });
        }

        let mut cmd = Command::new("container");
        cmd.arg("run").arg("-d");
        cmd.args(["--name", &self.session_id]);
        cmd.arg("--memory")
            .arg(format!("{}M", self.request.limits.max_memory_mb));
        cmd.arg("--cpus")
            .arg(format!("{}", self.request.limits.max_cpu_cores));
        if !self.request.limits.allow_network {
            cmd.args(["--network", "none"]);
        }
        for (key, value) in &self.request.env {
            cmd.arg("-e").arg(format!("{}={}", key, value));
        }
        if let Some(ref dir) = workdir {
            cmd.args(["-w", dir]);
        }
        if let Some(repo_dir) = self
            .repo_dir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            let mount = format!("{}:/workspace", repo_dir.path().display());
            cmd.arg("-v").arg(mount);
        }
        cmd.arg(&image);
        cmd.args(["sh", "-lc", "sleep infinity"]);
        let output = cmd
            .output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if !output.status.success() {
            return Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let mut container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if container_id.is_empty() {
            container_id = self.session_id.clone();
        }
        self.set_container_id(container_id.clone());

        {
            let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
            *state = SessionState::Running {
                started_at: Timestamp::now(),
                commands_completed: 0,
            };
        }

        let mut command_results = Vec::new();
        let mut combined_exit = 0;
        for (idx, command) in self.request.commands.iter().enumerate() {
            let result = run_apple_exec_command(
                &container_id,
                &self.session_id,
                command,
                workdir.clone(),
                self.request.env.clone(),
                self.request.limits.max_time_secs,
                &self.output_tx,
                None,
                &self.output_tx,
            )?;
            if result.exit_code != 0 {
                command_results.push(result);
                let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
                *state = SessionState::Failed {
                    error: "command failed".to_string(),
                    at: Timestamp::now(),
                };
                return Ok(());
            }
            let exit_code = result.exit_code;
            command_results.push(result);
            combined_exit = exit_code;
            let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
            *state = SessionState::Running {
                started_at: Timestamp::now(),
                commands_completed: idx + 1,
            };
        }

        if matches!(self.request.kind, ContainerKind::Interactive) {
            return Ok(());
        }

        let stdout = command_results
            .iter()
            .map(|r| r.stdout.clone())
            .collect::<Vec<_>>()
            .join("");
        let stderr = command_results
            .iter()
            .map(|r| r.stderr.clone())
            .collect::<Vec<_>>()
            .join("");
        let response = ContainerResponse {
            session_id: self.session_id.clone(),
            exit_code: Some(combined_exit),
            stdout,
            stderr,
            command_results,
            artifacts: Vec::new(),
            usage: ContainerUsage::zero(),
            cost_usd: 0,
            reserved_usd: self.request.max_cost_usd.unwrap_or(0),
            duration_ms: self.start.elapsed().as_millis() as u64,
            provider_id: "apple".to_string(),
        };
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = SessionState::Complete(response);
        Ok(())
    }

    fn fail(&self, message: &str) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = SessionState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        };
    }

    fn expire(&self) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = SessionState::Expired {
            at: Timestamp::now(),
        };
    }
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
struct AppleExec {
    state: RwLock<ExecState>,
    output_tx: mpsc::Sender<OutputChunk>,
    output_rx: Mutex<mpsc::Receiver<OutputChunk>>,
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
impl AppleExec {
    fn new(
        _exec_id: String,
        _session_id: String,
        output_tx: mpsc::Sender<OutputChunk>,
        output_rx: mpsc::Receiver<OutputChunk>,
    ) -> Self {
        Self {
            state: RwLock::new(ExecState::Pending {
                submitted_at: Timestamp::now(),
            }),
            output_tx,
            output_rx: Mutex::new(output_rx),
        }
    }

    fn complete(&self, result: CommandResult) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Complete(result);
    }

    fn running(&self) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Running {
            started_at: Timestamp::now(),
        };
    }

    fn fail(&self, message: &str) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        };
    }
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
fn run_apple_exec_command(
    container_id: &str,
    session_id: &str,
    command: &str,
    workdir: Option<String>,
    env: HashMap<String, String>,
    max_time_secs: u32,
    exec_tx: &mpsc::Sender<OutputChunk>,
    exec_id: Option<String>,
    session_tx: &mpsc::Sender<OutputChunk>,
) -> Result<CommandResult, ContainerError> {
    let mut cmd = Command::new("container");
    cmd.arg("exec").arg("-i");
    if let Some(ref dir) = workdir {
        cmd.args(["-w", dir]);
    }
    for (key, value) in env {
        cmd.arg("-e").arg(format!("{}={}", key, value));
    }
    cmd.arg(container_id);
    cmd.args(["sh", "-lc", command]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ContainerError::ProviderError("missing stdout pipe".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ContainerError::ProviderError("missing stderr pipe".to_string()))?;

    let exec_id_clone = exec_id.clone();
    let session_id = session_id.to_string();
    let out_tx = exec_tx.clone();
    let session_tx_clone = session_tx.clone();
    let stdout_handle = spawn_reader(
        stdout,
        OutputStream::Stdout,
        session_id.clone(),
        exec_id_clone.clone(),
        out_tx,
        session_tx_clone,
    );
    let err_tx = exec_tx.clone();
    let session_tx_clone = session_tx.clone();
    let stderr_handle = spawn_reader(
        stderr,
        OutputStream::Stderr,
        session_id.clone(),
        exec_id_clone,
        err_tx,
        session_tx_clone,
    );

    let start = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?
        {
            let stdout_text = stdout_handle.join().unwrap_or_else(|_| String::new());
            let stderr_text = stderr_handle.join().unwrap_or_else(|_| String::new());
            let exit_code = status.code().unwrap_or(-1);
            return Ok(CommandResult {
                command: command.to_string(),
                exit_code,
                stdout: stdout_text,
                stderr: stderr_text,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }
        if start.elapsed().as_secs() > max_time_secs as u64 {
            let _ = child.kill();
            let stdout_text = stdout_handle.join().unwrap_or_else(|_| String::new());
            let stderr_text = stderr_handle.join().unwrap_or_else(|_| String::new());
            return Err(ContainerError::ProviderError(format!(
                "command timeout after {}s (stdout={} stderr={})",
                max_time_secs, stdout_text, stderr_text
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Local container provider using the Docker CLI.
#[cfg(not(target_arch = "wasm32"))]
pub struct LocalContainerProvider {
    sessions: Arc<RwLock<HashMap<String, Arc<LocalSession>>>>,
    execs: Arc<RwLock<HashMap<String, Arc<LocalExec>>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalContainerProvider {
    /// Create a new local provider.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn docker_available() -> bool {
        Command::new("docker")
            .arg("version")
            .arg("--format")
            .arg("{{.Server.Version}}")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    fn docker_images() -> Result<Vec<String>, ContainerError> {
        let output = Command::new("docker")
            .args(["images", "--format", "{{.Repository}}:{{.Tag}}"])
            .output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if !output.status.success() {
            return Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let images = text
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .filter(|line| !line.contains("<none>"))
            .collect();
        Ok(images)
    }

    fn ensure_available() -> Result<(), ContainerError> {
        if Self::docker_available() {
            Ok(())
        } else {
            Err(ContainerError::Unavailable(
                "docker not available".to_string(),
            ))
        }
    }

    fn session(&self, session_id: &str) -> Result<Arc<LocalSession>, ContainerError> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .cloned()
            .ok_or(ContainerError::SessionNotFound)
    }

    fn exec(&self, exec_id: &str) -> Result<Arc<LocalExec>, ContainerError> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned()
            .ok_or(ContainerError::ExecNotFound)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for LocalContainerProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ContainerProvider for LocalContainerProvider {
    fn id(&self) -> &str {
        "local"
    }

    fn info(&self) -> ContainerProviderInfo {
        let available = Self::docker_available();
        let images = if available {
            Self::docker_images().unwrap_or_default()
        } else {
            Vec::new()
        };
        ContainerProviderInfo {
            id: "local".to_string(),
            name: "Local (Docker)".to_string(),
            available_images: images,
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: true,
                interactive: true,
                artifacts: false,
                streaming: true,
            },
            pricing: None,
            latency: ContainerLatency {
                startup_ms: 0,
                measured: false,
            },
            limits: ContainerLimits {
                max_memory_mb: 16384,
                max_cpu_cores: 8.0,
                max_disk_mb: 10240,
                max_time_secs: 3600,
                network_allowed: true,
            },
            status: if available {
                ProviderStatus::Available
            } else {
                ProviderStatus::Unavailable {
                    reason: "docker not available".to_string(),
                }
            },
        }
    }

    fn is_available(&self) -> bool {
        Self::docker_available()
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        Self::ensure_available()?;
        let image = request
            .image
            .clone()
            .ok_or_else(|| ContainerError::InvalidRequest("image required".to_string()))?;

        if request
            .repo
            .as_ref()
            .and_then(|r| r.auth.as_ref())
            .is_some()
        {
            return Err(ContainerError::NotSupported {
                capability: "repo_auth".to_string(),
                provider: "local".to_string(),
            });
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let session_id_clone = session_id.clone();
        let (output_tx, output_rx) = mpsc::channel(256);
        let session = Arc::new(LocalSession::new(
            session_id.clone(),
            request.clone(),
            output_tx,
            output_rx,
        ));

        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone(), session.clone());

        let provider_sessions = self.sessions.clone();
        thread::spawn(move || {
            if let Err(err) = session.run(image) {
                session.fail(&err.to_string());
                provider_sessions
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&session_id_clone);
            }
        });

        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .map(|session| {
                session
                    .state
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
            })
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        let exec_id = uuid::Uuid::new_v4().to_string();
        let exec_id_clone = exec_id.clone();
        let (output_tx, output_rx) = mpsc::channel(128);
        let exec = Arc::new(LocalExec::new(
            exec_id.clone(),
            session_id.to_string(),
            output_tx,
            output_rx,
        ));
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(exec_id.clone(), exec.clone());

        let provider_execs = self.execs.clone();
        let session_clone = session.clone();
        let command_string = command.to_string();
        thread::spawn(move || {
            exec.running();
            let result = run_exec_command(
                &container_id,
                &session_clone.session_id,
                &command_string,
                session_clone.request.workdir.clone(),
                session_clone.request.env.clone(),
                session_clone.request.limits.max_time_secs,
                &exec.output_tx,
                Some(exec_id_clone.clone()),
                &session_clone.output_tx,
            );
            match result {
                Ok(command_result) => exec.complete(command_result),
                Err(err) => exec.fail(&err.to_string()),
            }
            provider_execs
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&exec_id_clone);
        });

        Ok(exec_id)
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .map(|exec| exec.state.read().unwrap_or_else(|e| e.into_inner()).clone())
    }

    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let exec = self.exec(exec_id)?;
        let mut rx = exec.output_rx.lock().unwrap_or_else(|e| e.into_inner());
        match rx.try_recv() {
            Ok(chunk) => Ok(Some(chunk)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }

    fn cancel_exec(&self, exec_id: &str) -> Result<(), ContainerError> {
        let exec = self.exec(exec_id)?;
        exec.fail("cancelled");
        Ok(())
    }

    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let escaped = shell_escape(path);
        let script = format!(
            "dd if={} bs=1 skip={} count={} 2>/dev/null",
            escaped, offset, len
        );
        let output = Command::new("docker")
            .args(["exec", &container_id, "sh", "-lc", &script])
            .output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        let escaped = shell_escape(path);
        if offset == 0 {
            let truncate = format!("truncate -s 0 {} 2>/dev/null || : ", escaped);
            let _ = Command::new("docker")
                .args(["exec", &container_id, "sh", "-lc", &truncate])
                .output();
        }
        let script = format!(
            "dd of={} bs=1 seek={} conv=notrunc 2>/dev/null",
            escaped, offset
        );
        let mut child = Command::new("docker")
            .args(["exec", "-i", &container_id, "sh", "-lc", &script])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(data)
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        let _ = Command::new("docker")
            .args(["stop", &container_id])
            .output();
        let _ = Command::new("docker").args(["rm", &container_id]).output();
        session.expire();
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session_id);
        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let session = self.session(session_id)?;
        let mut rx = session.output_rx.lock().unwrap_or_else(|e| e.into_inner());
        match rx.try_recv() {
            Ok(chunk) => Ok(Some(chunk)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct LocalSession {
    session_id: String,
    state: RwLock<SessionState>,
    output_tx: mpsc::Sender<OutputChunk>,
    output_rx: Mutex<mpsc::Receiver<OutputChunk>>,
    request: ContainerRequest,
    container_id: Mutex<Option<String>>,
    start: Instant,
    repo_dir: Mutex<Option<tempfile::TempDir>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalSession {
    fn new(
        session_id: String,
        request: ContainerRequest,
        output_tx: mpsc::Sender<OutputChunk>,
        output_rx: mpsc::Receiver<OutputChunk>,
    ) -> Self {
        Self {
            session_id: session_id.clone(),
            state: RwLock::new(SessionState::Provisioning {
                started_at: Timestamp::now(),
            }),
            output_tx,
            output_rx: Mutex::new(output_rx),
            request,
            container_id: Mutex::new(None),
            start: Instant::now(),
            repo_dir: Mutex::new(None),
        }
    }

    fn container_id(&self) -> Result<String, ContainerError> {
        self.container_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or(ContainerError::SessionNotFound)
    }

    fn set_container_id(&self, id: String) {
        *self.container_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
    }

    fn run(&self, image: String) -> Result<(), ContainerError> {
        let mut workdir = self.request.workdir.clone();
        if let Some(repo) = self.request.repo.clone() {
            let repo_dir = tempfile::TempDir::new()
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            {
                let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
                *state = SessionState::Cloning {
                    started_at: Timestamp::now(),
                    repo_url: repo.url.clone(),
                };
            }
            let status = Command::new("git")
                .args([
                    "clone",
                    "--depth",
                    "1",
                    "--branch",
                    &repo.git_ref,
                    &repo.url,
                    repo_dir.path().to_str().unwrap_or_default(),
                ])
                .status()
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            if !status.success() {
                return Err(ContainerError::ProviderError(
                    "git clone failed".to_string(),
                ));
            }
            let mut repo_guard = self.repo_dir.lock().unwrap_or_else(|e| e.into_inner());
            *repo_guard = Some(repo_dir);
            let base = "/workspace".to_string();
            workdir = Some(match repo.subdir {
                Some(subdir) => format!("{}/{}", base, subdir.trim_matches('/')),
                None => base,
            });
        }

        let mut cmd = Command::new("docker");
        cmd.arg("run").arg("-d");
        cmd.args(["--name", &self.session_id]);
        cmd.arg(format!("--memory={}m", self.request.limits.max_memory_mb));
        cmd.arg(format!("--cpus={}", self.request.limits.max_cpu_cores));
        if !self.request.limits.allow_network {
            cmd.args(["--network", "none"]);
        }
        for (key, value) in &self.request.env {
            cmd.arg("-e").arg(format!("{}={}", key, value));
        }
        if let Some(ref dir) = workdir {
            cmd.args(["-w", dir]);
        }
        if let Some(repo_dir) = self
            .repo_dir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            let mount = format!("{}:/workspace", repo_dir.path().display());
            cmd.arg("-v").arg(mount);
        }
        cmd.arg(&image);
        cmd.args(["sh", "-lc", "sleep infinity"]);
        let output = cmd
            .output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if !output.status.success() {
            return Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        self.set_container_id(container_id.clone());

        {
            let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
            *state = SessionState::Running {
                started_at: Timestamp::now(),
                commands_completed: 0,
            };
        }

        let mut command_results = Vec::new();
        let mut combined_exit = 0;
        for (idx, command) in self.request.commands.iter().enumerate() {
            let result = run_exec_command(
                &container_id,
                &self.session_id,
                command,
                workdir.clone(),
                self.request.env.clone(),
                self.request.limits.max_time_secs,
                &self.output_tx,
                None,
                &self.output_tx,
            )?;
            if result.exit_code != 0 {
                command_results.push(result);
                let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
                *state = SessionState::Failed {
                    error: "command failed".to_string(),
                    at: Timestamp::now(),
                };
                return Ok(());
            }
            let exit_code = result.exit_code;
            command_results.push(result);
            combined_exit = exit_code;
            let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
            *state = SessionState::Running {
                started_at: Timestamp::now(),
                commands_completed: idx + 1,
            };
        }

        if matches!(self.request.kind, ContainerKind::Interactive) {
            return Ok(());
        }

        let stdout = command_results
            .iter()
            .map(|r| r.stdout.clone())
            .collect::<Vec<_>>()
            .join("");
        let stderr = command_results
            .iter()
            .map(|r| r.stderr.clone())
            .collect::<Vec<_>>()
            .join("");
        let response = ContainerResponse {
            session_id: self.session_id.clone(),
            exit_code: Some(combined_exit),
            stdout,
            stderr,
            command_results,
            artifacts: Vec::new(),
            usage: ContainerUsage::zero(),
            cost_usd: 0,
            reserved_usd: self.request.max_cost_usd.unwrap_or(0),
            duration_ms: self.start.elapsed().as_millis() as u64,
            provider_id: "local".to_string(),
        };
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = SessionState::Complete(response);
        Ok(())
    }

    fn fail(&self, message: &str) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = SessionState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        };
    }

    fn expire(&self) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = SessionState::Expired {
            at: Timestamp::now(),
        };
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct LocalExec {
    state: RwLock<ExecState>,
    output_tx: mpsc::Sender<OutputChunk>,
    output_rx: Mutex<mpsc::Receiver<OutputChunk>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalExec {
    fn new(
        _exec_id: String,
        _session_id: String,
        output_tx: mpsc::Sender<OutputChunk>,
        output_rx: mpsc::Receiver<OutputChunk>,
    ) -> Self {
        Self {
            state: RwLock::new(ExecState::Pending {
                submitted_at: Timestamp::now(),
            }),
            output_tx,
            output_rx: Mutex::new(output_rx),
        }
    }

    fn complete(&self, result: CommandResult) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Complete(result);
    }

    fn running(&self) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Running {
            started_at: Timestamp::now(),
        };
    }

    fn fail(&self, message: &str) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        };
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_exec_command(
    container_id: &str,
    session_id: &str,
    command: &str,
    workdir: Option<String>,
    env: HashMap<String, String>,
    max_time_secs: u32,
    exec_tx: &mpsc::Sender<OutputChunk>,
    exec_id: Option<String>,
    session_tx: &mpsc::Sender<OutputChunk>,
) -> Result<CommandResult, ContainerError> {
    let mut cmd = Command::new("docker");
    cmd.arg("exec").arg("-i");
    if let Some(ref dir) = workdir {
        cmd.args(["-w", dir]);
    }
    for (key, value) in env {
        cmd.arg("-e").arg(format!("{}={}", key, value));
    }
    cmd.arg(container_id);
    cmd.args(["sh", "-lc", command]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ContainerError::ProviderError("missing stdout pipe".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ContainerError::ProviderError("missing stderr pipe".to_string()))?;

    let exec_id_clone = exec_id.clone();
    let session_id = session_id.to_string();
    let out_tx = exec_tx.clone();
    let session_tx_clone = session_tx.clone();
    let stdout_handle = spawn_reader(
        stdout,
        OutputStream::Stdout,
        session_id.clone(),
        exec_id_clone.clone(),
        out_tx,
        session_tx_clone,
    );
    let err_tx = exec_tx.clone();
    let session_tx_clone = session_tx.clone();
    let stderr_handle = spawn_reader(
        stderr,
        OutputStream::Stderr,
        session_id.clone(),
        exec_id_clone,
        err_tx,
        session_tx_clone,
    );

    let start = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?
        {
            let stdout_text = stdout_handle.join().unwrap_or_else(|_| String::new());
            let stderr_text = stderr_handle.join().unwrap_or_else(|_| String::new());
            let exit_code = status.code().unwrap_or(-1);
            return Ok(CommandResult {
                command: command.to_string(),
                exit_code,
                stdout: stdout_text,
                stderr: stderr_text,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }
        if start.elapsed().as_secs() > max_time_secs as u64 {
            let _ = child.kill();
            let stdout_text = stdout_handle.join().unwrap_or_else(|_| String::new());
            let stderr_text = stderr_handle.join().unwrap_or_else(|_| String::new());
            return Err(ContainerError::ProviderError(format!(
                "command timeout after {}s (stdout={} stderr={})",
                max_time_secs, stdout_text, stderr_text
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_reader<R: Read + Send + 'static>(
    mut reader: R,
    stream: OutputStream,
    session_id: String,
    exec_id: Option<String>,
    exec_tx: mpsc::Sender<OutputChunk>,
    session_tx: mpsc::Sender<OutputChunk>,
) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut output = String::new();
        loop {
            let read = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            let chunk = String::from_utf8_lossy(&buf[..read]).to_string();
            output.push_str(&chunk);
            let payload = OutputChunk {
                session_id: session_id.clone(),
                exec_id: exec_id.clone(),
                stream: stream.clone(),
                data: chunk,
            };
            let _ = exec_tx.send(payload.clone());
            let _ = session_tx.send(payload);
        }
        output
    })
}

fn shell_escape(value: &str) -> String {
    let mut escaped = String::new();
    escaped.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            escaped.push_str("'\\''");
        } else {
            escaped.push(ch);
        }
    }
    escaped.push('\'');
    escaped
}
