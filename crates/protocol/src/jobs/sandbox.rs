//! Sandbox run job type (`oa.sandbox_run.v1`).
//!
//! This job type is used for running commands in a sandboxed environment.
//! It supports Docker-based execution with configurable security policies.

use crate::provenance::Provenance;
use crate::verification::Verification;
use crate::version::SchemaVersion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{JobRequest, JobResponse};

/// Network policy for sandbox execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// No network access.
    #[default]
    None,
    /// Localhost only.
    Localhost,
    /// Full network access.
    Full,
}

/// Resource limits for sandbox execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Memory limit in MB.
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,

    /// CPU limit (number of cores).
    #[serde(default = "default_cpus")]
    pub cpus: f32,

    /// Timeout in seconds.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u32,

    /// Disk space limit in MB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_mb: Option<u32>,
}

fn default_memory_mb() -> u32 {
    512
}

fn default_cpus() -> f32 {
    1.0
}

fn default_timeout_secs() -> u32 {
    60
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            memory_mb: default_memory_mb(),
            cpus: default_cpus(),
            timeout_secs: default_timeout_secs(),
            disk_mb: None,
        }
    }
}

/// Sandbox configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Sandbox provider (e.g., "docker", "firecracker").
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Docker image digest (sha256:...) for reproducibility.
    pub image_digest: String,

    /// Network policy.
    #[serde(default)]
    pub network_policy: NetworkPolicy,

    /// Resource limits.
    #[serde(default)]
    pub resources: ResourceLimits,
}

fn default_provider() -> String {
    "docker".to_string()
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            image_digest: String::new(),
            network_policy: NetworkPolicy::default(),
            resources: ResourceLimits::default(),
        }
    }
}

/// A command to run in the sandbox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxCommand {
    /// The command to execute.
    pub cmd: String,

    /// Working directory (relative to repo root).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,

    /// Whether to continue on failure.
    #[serde(default)]
    pub continue_on_fail: bool,
}

impl SandboxCommand {
    /// Create a new sandbox command.
    pub fn new(cmd: impl Into<String>) -> Self {
        Self {
            cmd: cmd.into(),
            workdir: None,
            continue_on_fail: false,
        }
    }

    /// Set working directory.
    pub fn with_workdir(mut self, workdir: impl Into<String>) -> Self {
        self.workdir = Some(workdir.into());
        self
    }

    /// Continue on failure.
    pub fn continue_on_fail(mut self) -> Self {
        self.continue_on_fail = true;
        self
    }
}

/// Repository to mount in the sandbox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoMount {
    /// Repository URL or local path.
    pub source: String,

    /// Git ref (branch, tag, commit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,

    /// Mount path in sandbox.
    #[serde(default = "default_mount_path")]
    pub mount_path: String,
}

fn default_mount_path() -> String {
    "/workspace".to_string()
}

impl Default for RepoMount {
    fn default() -> Self {
        Self {
            source: String::new(),
            git_ref: None,
            mount_path: default_mount_path(),
        }
    }
}

/// Request for sandbox execution.
///
/// # Example
///
/// ```
/// use protocol::jobs::{SandboxRunRequest, sandbox::{SandboxConfig, SandboxCommand}};
///
/// let request = SandboxRunRequest {
///     sandbox: SandboxConfig {
///         image_digest: "sha256:abc123...".into(),
///         ..Default::default()
///     },
///     commands: vec![
///         SandboxCommand::new("cargo build"),
///         SandboxCommand::new("cargo test"),
///     ],
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxRunRequest {
    /// Sandbox configuration.
    pub sandbox: SandboxConfig,

    /// Repository to mount.
    #[serde(default)]
    pub repo: RepoMount,

    /// Commands to execute.
    pub commands: Vec<SandboxCommand>,

    /// Environment variables.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,

    /// Verification settings (objective for sandbox runs).
    #[serde(default = "default_verification")]
    pub verification: Verification,
}

fn default_verification() -> Verification {
    Verification::objective()
}

impl Default for SandboxRunRequest {
    fn default() -> Self {
        Self {
            sandbox: SandboxConfig::default(),
            repo: RepoMount::default(),
            commands: Vec::new(),
            env: HashMap::new(),
            verification: default_verification(),
        }
    }
}

impl JobRequest for SandboxRunRequest {
    const JOB_TYPE: &'static str = "oa.sandbox_run.v1";
    const SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0, 0);

    fn verification(&self) -> &Verification {
        &self.verification
    }
}

/// Result of a single command execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    /// The command that was run.
    pub cmd: String,

    /// Exit code.
    pub exit_code: i32,

    /// Duration in milliseconds.
    pub duration_ms: u64,

    /// SHA-256 of stdout.
    pub stdout_sha256: String,

    /// SHA-256 of stderr.
    pub stderr_sha256: String,

    /// Truncated stdout (for quick inspection).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_preview: Option<String>,

    /// Truncated stderr (for quick inspection).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_preview: Option<String>,
}

/// Environment information from the sandbox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvInfo {
    /// Image digest used.
    pub image_digest: String,

    /// Actual hostname in container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    /// System info (uname, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_info: Option<String>,
}

/// An artifact produced by the sandbox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    /// Path in sandbox.
    pub path: String,

    /// SHA-256 of artifact content.
    pub sha256: String,

    /// Size in bytes.
    pub size_bytes: u64,

    /// MIME type if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Overall status of sandbox execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStatus {
    /// All commands succeeded.
    Success,
    /// One or more commands failed.
    Failed,
    /// Execution timed out.
    Timeout,
    /// Execution was cancelled.
    Cancelled,
    /// Internal error.
    Error,
}

/// Response from sandbox execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxRunResponse {
    /// Environment information.
    pub env_info: EnvInfo,

    /// Results from each command.
    pub runs: Vec<CommandResult>,

    /// Artifacts produced.
    #[serde(default)]
    pub artifacts: Vec<Artifact>,

    /// Overall status.
    pub status: SandboxStatus,

    /// Error message if status is Error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Provenance information.
    pub provenance: Provenance,
}

impl JobResponse for SandboxRunResponse {
    type Request = SandboxRunRequest;

    fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobEnvelope;
    use crate::verification::VerificationMode;

    #[test]
    fn test_sandbox_request_hash() {
        let request = SandboxRunRequest {
            sandbox: SandboxConfig {
                image_digest: "sha256:abc123".into(),
                ..Default::default()
            },
            commands: vec![SandboxCommand::new("echo hello")],
            ..Default::default()
        };

        let hash1 = request.compute_hash().unwrap();
        let hash2 = request.compute_hash().unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_job_type_constant() {
        assert_eq!(SandboxRunRequest::JOB_TYPE, "oa.sandbox_run.v1");
        assert_eq!(SandboxRunRequest::SCHEMA_VERSION, SchemaVersion::new(1, 0, 0));
    }

    #[test]
    fn test_default_verification_is_objective() {
        let request = SandboxRunRequest::default();
        assert_eq!(request.verification.mode, VerificationMode::Objective);
        assert_eq!(request.verification.redundancy, 1);
    }

    #[test]
    fn test_request_serde() {
        let mut env = HashMap::new();
        env.insert("RUST_LOG".into(), "debug".into());

        let request = SandboxRunRequest {
            sandbox: SandboxConfig {
                provider: "docker".into(),
                image_digest: "sha256:abc123".into(),
                network_policy: NetworkPolicy::Localhost,
                resources: ResourceLimits {
                    memory_mb: 1024,
                    cpus: 2.0,
                    timeout_secs: 120,
                    disk_mb: Some(500),
                },
            },
            repo: RepoMount {
                source: "https://github.com/example/repo".into(),
                git_ref: Some("main".into()),
                mount_path: "/workspace".into(),
            },
            commands: vec![
                SandboxCommand::new("cargo build").with_workdir("/workspace"),
                SandboxCommand::new("cargo test").continue_on_fail(),
            ],
            env,
            verification: Verification::objective(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: SandboxRunRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, parsed);
    }

    #[test]
    fn test_response_serde() {
        let response = SandboxRunResponse {
            env_info: EnvInfo {
                image_digest: "sha256:abc123".into(),
                hostname: Some("sandbox-123".into()),
                system_info: None,
            },
            runs: vec![CommandResult {
                cmd: "cargo build".into(),
                exit_code: 0,
                duration_ms: 5000,
                stdout_sha256: "sha256:stdout".into(),
                stderr_sha256: "sha256:stderr".into(),
                stdout_preview: Some("Compiling...".into()),
                stderr_preview: None,
            }],
            artifacts: vec![Artifact {
                path: "/workspace/target/release/app".into(),
                sha256: "sha256:binary".into(),
                size_bytes: 1024000,
                mime_type: Some("application/octet-stream".into()),
            }],
            status: SandboxStatus::Success,
            error: None,
            provenance: Provenance::new("sandbox-executor"),
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: SandboxRunResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, parsed);
    }

    #[test]
    fn test_envelope_integration() {
        let request = SandboxRunRequest {
            sandbox: SandboxConfig {
                image_digest: "sha256:test".into(),
                ..Default::default()
            },
            commands: vec![SandboxCommand::new("ls")],
            ..Default::default()
        };

        let envelope = JobEnvelope::from_request(request);
        assert_eq!(envelope.job_type, "oa.sandbox_run.v1");
    }

    #[test]
    fn test_network_policy_serde() {
        let config = SandboxConfig {
            image_digest: "sha256:test".into(),
            network_policy: NetworkPolicy::Full,
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"network_policy\":\"full\""));
    }
}
