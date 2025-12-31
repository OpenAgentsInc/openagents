//! Container filesystem service and providers.

use crate::budget::{BudgetError, BudgetPolicy, BudgetReservation, BudgetTracker};
use crate::compute::Prefer;
use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Permissions,
    SeekFrom, Stat, WatchEvent, WatchHandle,
};
use crate::idempotency::{IdempotencyJournal, JournalError};
use crate::types::{AgentId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use urlencoding::decode;

const IDEMPOTENCY_TTL: Duration = Duration::from_secs(3600);
const CHUNK_SIZE: u64 = 1_048_576;
const MAX_PATH_LEN: usize = 4096;

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
        Self { providers: Vec::new() }
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
            Prefer::Latency => candidates.into_iter().min_by_key(|p| p.info().latency.startup_ms),
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

    fn image_available(&self, provider: &Arc<dyn ContainerProvider>, image: &Option<String>) -> bool {
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

    fn within_limits(&self, provider: &Arc<dyn ContainerProvider>, request: &ContainerRequest) -> bool {
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
        if let Some(max_usd) = request.max_cost_usd {
            return max_usd;
        }
        let info = provider.info();
        let pricing = match info.pricing {
            Some(pricing) => pricing,
            None => return 0,
        };
        let estimated_secs = request.limits.max_time_secs as u64;
        pricing
            .startup_usd
            .saturating_add(pricing.per_second_usd.saturating_mul(estimated_secs))
    }
}

#[derive(Clone)]
struct SessionRecord {
    provider_id: String,
    reservation: BudgetReservation,
    reconciled: bool,
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
    ) -> Self {
        Self {
            agent_id,
            router: Arc::new(RwLock::new(router)),
            policy: Arc::new(RwLock::new(policy)),
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
            }
            SessionState::Failed { .. } | SessionState::Expired { .. } => {
                tracker.release(record.reservation);
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
                let bytes = serde_json::to_vec(&json)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "status"] => {
                let provider = self.session_provider(session_id)?;
                let state = provider
                    .get_session(session_id)
                    .ok_or(FsError::NotFound)?;
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
                let bytes = serde_json::to_vec(&json)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "result"] => {
                let provider = self.session_provider(session_id)?;
                let state = provider
                    .get_session(session_id)
                    .ok_or(FsError::NotFound)?;
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
                let state = provider
                    .get_session(session_id)
                    .ok_or(FsError::NotFound)?;
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
                let bytes = serde_json::to_vec(&response)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "ctl"] if flags.write => Ok(Box::new(CtlHandle::new(
                session_id.to_string(),
                self.router.clone(),
                self.sessions.clone(),
            ))),
            ["sessions", session_id, "exec", "new"] if flags.write => Ok(Box::new(
                ExecNewHandle::new(
                    session_id.to_string(),
                    self.router.clone(),
                    self.execs.clone(),
                ),
            )),
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
                let bytes = serde_json::to_vec(&json)
                    .map_err(|err| FsError::Other(err.to_string()))?;
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
                let chunk_index: u64 = chunk
                    .parse()
                    .map_err(|_| FsError::InvalidPath)?;
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
                DirEntry::dir("sessions"),
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
            ["providers"] | ["sessions"] => Ok(Stat::dir()),
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
            ["providers", id, "info"] | ["providers", id, "images"] | ["providers", id, "health"] => {
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
        budget: Arc<Mutex<BudgetTracker>>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        journal: Arc<dyn IdempotencyJournal>,
    ) -> Self {
        Self {
            agent_id,
            router,
            policy,
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
            return Err(FsError::Other(ContainerError::IdempotencyRequired.to_string()));
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
                return Err(FsError::Other("max concurrent containers reached".to_string()));
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
                    if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
                        self.sessions
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .entry(session_id.to_string())
                            .or_insert(SessionRecord {
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

        let session_id = match provider.submit(request.clone()) {
            Ok(session_id) => session_id,
            Err(err) => {
                let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                tracker.release(reservation);
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
        let policy: ContainerPolicy = serde_json::from_slice(&self.buffer)
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
        let response_bytes = serde_json::to_vec(&response_json)
            .map_err(|err| FsError::Other(err.to_string()))?;
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
}

impl SessionWatchHandle {
    fn new(
        session_id: String,
        provider: Arc<dyn ContainerProvider>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        budget: Arc<Mutex<BudgetTracker>>,
    ) -> Self {
        Self {
            session_id,
            provider,
            sessions,
            budget,
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
            }
            SessionState::Failed { .. } | SessionState::Expired { .. } => {
                tracker.release(record.reservation);
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
                            SessionState::Complete(_) | SessionState::Failed { .. } | SessionState::Expired { .. }
                        ) {
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
            thread::sleep(Duration::from_millis(25));
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
            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    return Ok(None);
                }
            }
            thread::sleep(Duration::from_millis(25));
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

/// Local container provider using the Docker CLI.
pub struct LocalContainerProvider {
    sessions: Arc<RwLock<HashMap<String, Arc<LocalSession>>>>,
    execs: Arc<RwLock<HashMap<String, Arc<LocalExec>>>>,
}

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
            Err(ContainerError::Unavailable("docker not available".to_string()))
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

impl Default for LocalContainerProvider {
    fn default() -> Self {
        Self::new()
    }
}

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

        if request.repo.as_ref().and_then(|r| r.auth.as_ref()).is_some() {
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
            .map(|session| session.state.read().unwrap_or_else(|e| e.into_inner()).clone())
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
                return Err(ContainerError::ProviderError("git clone failed".to_string()));
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

struct LocalExec {
    state: RwLock<ExecState>,
    output_tx: mpsc::Sender<OutputChunk>,
    output_rx: Mutex<mpsc::Receiver<OutputChunk>>,
}

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

    let stdout = child.stdout.take().ok_or_else(|| {
        ContainerError::ProviderError("missing stdout pipe".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        ContainerError::ProviderError("missing stderr pipe".to_string())
    })?;

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
        if let Some(status) = child.try_wait().map_err(|err| {
            ContainerError::ProviderError(err.to_string())
        })? {
            let stdout_text = stdout_handle
                .join()
                .unwrap_or_else(|_| String::new());
            let stderr_text = stderr_handle
                .join()
                .unwrap_or_else(|_| String::new());
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
            let stdout_text = stdout_handle
                .join()
                .unwrap_or_else(|_| String::new());
            let stderr_text = stderr_handle
                .join()
                .unwrap_or_else(|_| String::new());
            return Err(ContainerError::ProviderError(format!(
                "command timeout after {}s (stdout={} stderr={})",
                max_time_secs, stdout_text, stderr_text
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

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
