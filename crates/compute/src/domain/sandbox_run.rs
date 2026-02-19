//! SandboxRun job types for NIP-90 compute marketplace
//!
//! SandboxRun jobs execute commands in isolated containers for a repository.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use nostr::nip90::{JobInput, JobRequest, JobResult, KIND_JOB_SANDBOX_RUN, Nip90Error};

/// Resource limits for sandbox execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceLimits {
    /// Maximum execution time in seconds
    pub max_time_secs: u32,
    /// Maximum memory in MB
    pub max_memory_mb: u32,
    /// Maximum disk usage in MB
    pub max_disk_mb: u32,
    /// Maximum CPU cores (1.0 = one core)
    pub max_cpu_cores: f32,
    /// Whether network access is allowed
    pub allow_network: bool,
}

impl ResourceLimits {
    /// Default limits for basic sandbox runs
    pub fn default_basic() -> Self {
        Self {
            max_time_secs: 300,   // 5 minutes
            max_memory_mb: 1024,  // 1 GB
            max_disk_mb: 512,     // 512 MB
            max_cpu_cores: 1.0,   // 1 core
            allow_network: false, // No network by default
        }
    }

    /// Limits for build/test operations
    pub fn for_build() -> Self {
        Self {
            max_time_secs: 600,  // 10 minutes
            max_memory_mb: 4096, // 4 GB
            max_disk_mb: 2048,   // 2 GB
            max_cpu_cores: 2.0,  // 2 cores
            allow_network: true, // Need network for package managers
        }
    }
}

/// A sandbox run request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRunRequest {
    /// Repository URL (git clone URL)
    pub repo: String,
    /// Git reference (branch, tag, or commit SHA)
    pub git_ref: String,
    /// Commands to execute in order
    pub commands: Vec<String>,
    /// Working directory relative to repo root
    pub workdir: Option<String>,
    /// Environment variables to set
    pub env: HashMap<String, String>,
    /// Resource limits
    pub limits: ResourceLimits,
}

impl SandboxRunRequest {
    /// Create a new sandbox run request
    pub fn new(repo: impl Into<String>, git_ref: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            git_ref: git_ref.into(),
            commands: Vec::new(),
            workdir: None,
            env: HashMap::new(),
            limits: ResourceLimits::default_basic(),
        }
    }

    /// Add a command to execute
    pub fn add_command(mut self, cmd: impl Into<String>) -> Self {
        self.commands.push(cmd.into());
        self
    }

    /// Set the working directory
    pub fn with_workdir(mut self, dir: impl Into<String>) -> Self {
        self.workdir = Some(dir.into());
        self
    }

    /// Add an environment variable
    pub fn add_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set resource limits
    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Convert to NIP-90 JobRequest
    pub fn to_job_request(&self) -> Result<JobRequest, Nip90Error> {
        let mut request = JobRequest::new(KIND_JOB_SANDBOX_RUN)?
            .add_input(JobInput::url(&self.repo).with_marker("repo"))
            .add_param("git_ref", &self.git_ref);

        // Add commands as params
        for (i, cmd) in self.commands.iter().enumerate() {
            request = request.add_param(format!("cmd_{}", i), cmd);
        }

        // Add workdir if set
        if let Some(ref workdir) = self.workdir {
            request = request.add_param("workdir", workdir);
        }

        // Add env vars
        for (key, value) in &self.env {
            request = request.add_param(format!("env_{}", key), value);
        }

        // Add limits
        request = request
            .add_param("max_time_secs", self.limits.max_time_secs.to_string())
            .add_param("max_memory_mb", self.limits.max_memory_mb.to_string())
            .add_param("max_disk_mb", self.limits.max_disk_mb.to_string())
            .add_param("max_cpu_cores", self.limits.max_cpu_cores.to_string())
            .add_param("allow_network", self.limits.allow_network.to_string());

        Ok(request)
    }

    /// Parse from NIP-90 JobRequest
    pub fn from_job_request(request: &JobRequest) -> Result<Self, Nip90Error> {
        if request.kind != KIND_JOB_SANDBOX_RUN {
            return Err(Nip90Error::InvalidKind(request.kind, "5930".to_string()));
        }

        let mut repo = String::new();
        let mut git_ref = String::new();
        let mut commands = Vec::new();
        let mut workdir = None;
        let mut env = HashMap::new();
        let mut limits = ResourceLimits::default_basic();

        // Extract repo from inputs
        for input in &request.inputs {
            if input.marker.as_deref() == Some("repo") {
                repo = input.data.clone();
            }
        }

        // Extract params
        for param in &request.params {
            match param.key.as_str() {
                "git_ref" => git_ref = param.value.clone(),
                "workdir" => workdir = Some(param.value.clone()),
                "max_time_secs" => {
                    if let Ok(v) = param.value.parse() {
                        limits.max_time_secs = v;
                    }
                }
                "max_memory_mb" => {
                    if let Ok(v) = param.value.parse() {
                        limits.max_memory_mb = v;
                    }
                }
                "max_disk_mb" => {
                    if let Ok(v) = param.value.parse() {
                        limits.max_disk_mb = v;
                    }
                }
                "max_cpu_cores" => {
                    if let Ok(v) = param.value.parse() {
                        limits.max_cpu_cores = v;
                    }
                }
                "allow_network" => {
                    limits.allow_network = param.value == "true";
                }
                key if key.starts_with("cmd_") => {
                    if let Ok(idx) = key[4..].parse::<usize>() {
                        while commands.len() <= idx {
                            commands.push(String::new());
                        }
                        commands[idx] = param.value.clone();
                    }
                }
                key if key.starts_with("env_") => {
                    env.insert(key[4..].to_string(), param.value.clone());
                }
                _ => {}
            }
        }

        Ok(Self {
            repo,
            git_ref,
            commands,
            workdir,
            env,
            limits,
        })
    }
}

/// Result of a single command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// The command that was executed
    pub command: String,
    /// Exit code (0 = success)
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

/// Resource usage during sandbox execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceUsage {
    /// CPU time used in milliseconds
    pub cpu_time_ms: u64,
    /// Peak memory usage in bytes
    pub peak_memory_bytes: u64,
    /// Disk writes in bytes
    pub disk_writes_bytes: u64,
    /// Network bytes transferred (if allowed)
    pub network_bytes: u64,
}

/// Artifact hash for outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactHash {
    /// Path relative to repo root
    pub path: String,
    /// SHA256 hash of the file
    pub sha256: String,
    /// File size in bytes
    pub size: u64,
}

/// Result of a sandbox run job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRunResult {
    /// Overall exit code (0 if all commands succeeded)
    pub exit_code: i32,
    /// Combined stdout from all commands
    pub stdout: String,
    /// Combined stderr from all commands
    pub stderr: String,
    /// Results for each command
    pub command_results: Vec<CommandResult>,
    /// Hashes of output artifacts
    pub artifacts: Vec<ArtifactHash>,
    /// Resource usage summary
    pub usage: ResourceUsage,
}

impl SandboxRunResult {
    /// Create a new result with exit code
    pub fn new(exit_code: i32) -> Self {
        Self {
            exit_code,
            stdout: String::new(),
            stderr: String::new(),
            command_results: Vec::new(),
            artifacts: Vec::new(),
            usage: ResourceUsage::default(),
        }
    }

    /// Check if the sandbox run was successful
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }

    /// Add a command result
    pub fn add_command_result(mut self, result: CommandResult) -> Self {
        self.command_results.push(result);
        self
    }

    /// Add an artifact
    pub fn add_artifact(mut self, artifact: ArtifactHash) -> Self {
        self.artifacts.push(artifact);
        self
    }

    /// Set resource usage
    pub fn with_usage(mut self, usage: ResourceUsage) -> Self {
        self.usage = usage;
        self
    }

    /// Convert to NIP-90 JobResult
    pub fn to_job_result(
        &self,
        request_id: &str,
        customer_pubkey: &str,
        amount: Option<u64>,
    ) -> Result<JobResult, Nip90Error> {
        let content =
            serde_json::to_string(self).map_err(|e| Nip90Error::Serialization(e.to_string()))?;

        let mut result =
            JobResult::new(KIND_JOB_SANDBOX_RUN, request_id, customer_pubkey, content)?;

        if let Some(amt) = amount {
            result = result.with_amount(amt, None);
        }

        Ok(result)
    }

    /// Parse from NIP-90 JobResult content
    pub fn from_job_result(result: &JobResult) -> Result<Self, Nip90Error> {
        serde_json::from_str(&result.content).map_err(|e| Nip90Error::Serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default_basic();
        assert_eq!(limits.max_time_secs, 300);
        assert_eq!(limits.max_memory_mb, 1024);
        assert!(!limits.allow_network);
    }

    #[test]
    fn test_resource_limits_for_build() {
        let limits = ResourceLimits::for_build();
        assert_eq!(limits.max_time_secs, 600);
        assert!(limits.allow_network);
    }

    #[test]
    fn test_sandbox_run_request_new() {
        let request = SandboxRunRequest::new("https://github.com/owner/repo.git", "main")
            .add_command("cargo build")
            .add_command("cargo test")
            .with_workdir("crates/mylib")
            .add_env("RUST_LOG", "debug");

        assert_eq!(request.repo, "https://github.com/owner/repo.git");
        assert_eq!(request.git_ref, "main");
        assert_eq!(request.commands.len(), 2);
        assert_eq!(request.workdir, Some("crates/mylib".to_string()));
        assert_eq!(request.env.get("RUST_LOG"), Some(&"debug".to_string()));
    }

    #[test]
    fn test_sandbox_run_request_to_job_request() {
        let request = SandboxRunRequest::new("https://github.com/owner/repo.git", "main")
            .add_command("cargo build");

        let job = request.to_job_request().unwrap();
        assert_eq!(job.kind, KIND_JOB_SANDBOX_RUN);
        assert!(!job.inputs.is_empty());
    }

    #[test]
    fn test_command_result() {
        let result = CommandResult {
            command: "cargo test".to_string(),
            exit_code: 0,
            stdout: "test result: ok".to_string(),
            stderr: String::new(),
            duration_ms: 5000,
        };

        assert_eq!(result.exit_code, 0);
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn test_sandbox_run_result_success() {
        let result = SandboxRunResult::new(0);
        assert!(result.is_success());

        let result = SandboxRunResult::new(1);
        assert!(!result.is_success());
    }

    #[test]
    fn test_sandbox_run_result_serialization() {
        let result = SandboxRunResult::new(0).add_command_result(CommandResult {
            command: "echo hello".to_string(),
            exit_code: 0,
            stdout: "hello\n".to_string(),
            stderr: String::new(),
            duration_ms: 10,
        });

        let json = serde_json::to_string(&result).unwrap();
        let parsed: SandboxRunResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.exit_code, result.exit_code);
        assert_eq!(parsed.command_results.len(), 1);
    }
}
