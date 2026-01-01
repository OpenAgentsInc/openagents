//! Daytona API model types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =========================================================================
// Sandbox Types
// =========================================================================

/// Sandbox state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxState {
    Pending,
    Creating,
    Building,
    Pulling,
    Initializing,
    Started,
    Stopping,
    Stopped,
    Archiving,
    Archived,
    Destroying,
    Destroyed,
    Error,
    BuildFailed,
    Unknown,
}

/// Sandbox desired state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxDesiredState {
    Started,
    Stopped,
    Archived,
    Destroyed,
}

/// Backup state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupState {
    None,
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// Build information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dockerfile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub devcontainer_file_path: Option<String>,
}

/// Create build information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBuildInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dockerfile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub devcontainer_file_path: Option<String>,
}

/// Sandbox volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxVolume {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mount_path: Option<String>,
}

/// Sandbox information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sandbox {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<SandboxState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_stop_interval: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_archive_interval: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_info: Option<BuildInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<SandboxVolume>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_state: Option<BackupState>,
}

/// Request to create a sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSandbox {
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_stop_interval: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_archive_interval: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_vars: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_info: Option<CreateBuildInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<SandboxVolume>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

impl CreateSandbox {
    /// Create a new sandbox request with an image.
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
            target: None,
            snapshot: None,
            cpu: None,
            gpu: None,
            memory: None,
            disk: None,
            auto_stop_interval: None,
            auto_archive_interval: None,
            labels: None,
            public: None,
            env_vars: None,
            build_info: None,
            volumes: None,
            user: None,
        }
    }

    /// Set the target region.
    pub fn target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Set the CPU count.
    pub fn cpu(mut self, cpu: i32) -> Self {
        self.cpu = Some(cpu);
        self
    }

    /// Set the memory in GB.
    pub fn memory(mut self, memory: i32) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Set the disk size in GB.
    pub fn disk(mut self, disk: i32) -> Self {
        self.disk = Some(disk);
        self
    }

    /// Set the auto-stop interval in minutes.
    pub fn auto_stop(mut self, minutes: i32) -> Self {
        self.auto_stop_interval = Some(minutes);
        self
    }

    /// Set labels.
    pub fn labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = Some(labels);
        self
    }

    /// Set environment variables.
    pub fn env_vars(mut self, env_vars: HashMap<String, String>) -> Self {
        self.env_vars = Some(env_vars);
        self
    }

    /// Set environment variables (alias for env_vars).
    pub fn env(mut self, env_vars: HashMap<String, String>) -> Self {
        self.env_vars = Some(env_vars);
        self
    }

    /// Set the auto-stop interval in minutes (alias for auto_stop).
    pub fn auto_stop_interval(mut self, minutes: i32) -> Self {
        self.auto_stop_interval = Some(minutes);
        self
    }

    /// Set the auto-delete interval in minutes.
    pub fn auto_delete_interval(mut self, minutes: i32) -> Self {
        // Note: Daytona API may not support this directly, map to auto_archive
        self.auto_archive_interval = Some(minutes);
        self
    }
}

/// Sandbox labels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxLabels {
    pub labels: HashMap<String, String>,
}

// =========================================================================
// Process/Session Types
// =========================================================================

/// Execute request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteRequest {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_vars: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i32>,
}

impl ExecuteRequest {
    /// Create a new execute request.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            cwd: None,
            env_vars: None,
            timeout: None,
        }
    }

    /// Set working directory.
    pub fn cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set timeout in seconds (accepts i32).
    pub fn timeout(mut self, timeout: i32) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set timeout in seconds (accepts f64, truncates to i32).
    pub fn timeout_secs(mut self, timeout: f64) -> Self {
        self.timeout = Some(timeout as i32);
        self
    }
}

/// Execute response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteResponse {
    pub code: i32,
    pub result: String,
}

impl ExecuteResponse {
    /// Get exit code (alias for code field).
    pub fn exit_code(&self) -> i32 {
        self.code
    }
}

/// Session information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<String>>,
}

/// Create session request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Session execute request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExecuteRequest {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub async_exec: Option<bool>,
}

/// Session execute response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExecuteResponse {
    pub cmd_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

// =========================================================================
// File Types
// =========================================================================

/// File information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub name: String,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_time: Option<String>,
}

/// Match result from find-in-files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub file: String,
    pub line: i32,
    pub content: String,
}

/// Search files response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilesResponse {
    pub files: Vec<String>,
}

/// Replace request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceRequest {
    pub files: Vec<String>,
    pub pattern: String,
    pub new_value: String,
}

/// Replace result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceResult {
    pub file: String,
    pub replacements: i32,
}

/// Port preview URL response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortPreviewUrl {
    pub url: String,
}

// =========================================================================
// Git Types
// =========================================================================

/// File status in git.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatus {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staging: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
}

/// Git status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileStatus>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ahead: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<i32>,
}

/// Git clone request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCloneRequest {
    pub url: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl GitCloneRequest {
    /// Create a new git clone request.
    pub fn new(url: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            path: path.into(),
            branch: None,
            commit_id: None,
            username: None,
            password: None,
        }
    }

    /// Set the branch to clone.
    pub fn branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Set credentials.
    pub fn credentials(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Set the commit ID to checkout.
    pub fn commit_id(mut self, commit_id: impl Into<String>) -> Self {
        self.commit_id = Some(commit_id.into());
        self
    }
}

/// Git add request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitAddRequest {
    pub path: String,
    pub files: Vec<String>,
}

/// Git commit request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitRequest {
    pub path: String,
    pub message: String,
    pub author: String,
    pub email: String,
}

/// Git commit response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitResponse {
    pub sha: String,
}

/// Git commit info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitInfo {
    pub sha: String,
    pub author: String,
    pub email: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// Git branch request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchRequest {
    pub path: String,
    pub name: String,
}

/// Git delete branch request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDeleteBranchRequest {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

/// Git checkout request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCheckoutRequest {
    pub path: String,
    pub name: String,
}

/// Git repo request (for pull/push).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepoRequest {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// List branch response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListBranchResponse {
    pub branches: Vec<String>,
}
