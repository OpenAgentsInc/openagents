//! Script-based skill execution with sandboxing
//!
//! Provides sandboxed execution for skills that include scripts/ directories.

use crate::dspy_security::{
    classify_filesystem_permission, classify_resource_limits, classify_safe_path,
    classify_skill_security,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during script execution
#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Script not found: {0}")]
    ScriptNotFound(String),

    #[error("Execution timeout after {0}ms")]
    Timeout(u64),

    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid script path: {0}")]
    InvalidScriptPath(String),

    #[error("Unsupported script type: {0}")]
    UnsupportedScriptType(String),

    #[error("Approval required: {0}")]
    ApprovalRequired(String),
}

/// Filesystem access control for sandboxed execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[derive(Default)]
pub enum FilesystemAccess {
    /// No filesystem access allowed
    #[default]
    None,

    /// Read-only access to specified paths
    ReadOnly {
        /// Paths that can be read
        paths: Vec<PathBuf>,
    },

    /// Read-write access to specified paths
    ReadWrite {
        /// Paths that can be read and written
        paths: Vec<PathBuf>,
    },
}

impl FilesystemAccess {
    /// Create read-only access to specific paths
    pub fn read_only(paths: Vec<PathBuf>) -> Self {
        Self::ReadOnly { paths }
    }

    /// Create read-write access to specific paths
    pub fn read_write(paths: Vec<PathBuf>) -> Self {
        Self::ReadWrite { paths }
    }

    /// Check if this configuration allows reading from a path
    pub fn can_read(&self, _path: &PathBuf) -> bool {
        match self {
            Self::None => false,
            Self::ReadOnly { .. } | Self::ReadWrite { .. } => true,
        }
    }

    /// Check if this configuration allows writing to a path
    pub fn can_write(&self, _path: &PathBuf) -> bool {
        matches!(self, Self::ReadWrite { .. })
    }
}

/// Sandbox configuration for script execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Allow network access
    #[serde(default)]
    pub allow_network: bool,

    /// Filesystem access permissions
    #[serde(default)]
    pub allow_filesystem: FilesystemAccess,

    /// Maximum memory in megabytes
    #[serde(default = "default_max_memory")]
    pub max_memory_mb: u32,

    /// Maximum CPU time in seconds
    #[serde(default = "default_max_cpu")]
    pub max_cpu_seconds: u32,

    /// Environment variables to pass to the script
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}

/// Human approval context for high-risk executions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApprovalContext {
    /// Whether approval was granted
    #[serde(default)]
    pub approved: bool,

    /// Approver identity (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approver: Option<String>,

    /// Notes about the approval
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl ApprovalContext {
    /// Create an approval context marked as approved
    pub fn approved(approver: Option<String>) -> Self {
        Self {
            approved: true,
            approver,
            notes: None,
        }
    }
}

fn default_max_memory() -> u32 {
    512 // 512 MB default
}

fn default_max_cpu() -> u32 {
    30 // 30 seconds default
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            allow_network: false,
            allow_filesystem: FilesystemAccess::None,
            max_memory_mb: default_max_memory(),
            max_cpu_seconds: default_max_cpu(),
            env_vars: HashMap::new(),
        }
    }
}

impl SandboxConfig {
    /// Create a new sandbox configuration with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable network access
    pub fn with_network(mut self, allow: bool) -> Self {
        self.allow_network = allow;
        self
    }

    /// Set filesystem access
    pub fn with_filesystem(mut self, access: FilesystemAccess) -> Self {
        self.allow_filesystem = access;
        self
    }

    /// Set maximum memory limit
    pub fn with_max_memory_mb(mut self, mb: u32) -> Self {
        self.max_memory_mb = mb;
        self
    }

    /// Set maximum CPU time
    pub fn with_max_cpu_seconds(mut self, seconds: u32) -> Self {
        self.max_cpu_seconds = seconds;
        self
    }

    /// Add an environment variable
    pub fn with_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Describe filesystem access as paths + operation
    pub fn filesystem_request(&self) -> Option<(Vec<PathBuf>, &'static str)> {
        match &self.allow_filesystem {
            FilesystemAccess::None => None,
            FilesystemAccess::ReadOnly { paths } => Some((paths.clone(), "Read")),
            FilesystemAccess::ReadWrite { paths } => Some((paths.clone(), "Write")),
        }
    }

    /// Validate the sandbox configuration
    pub fn validate(&self) -> Result<(), ExecutionError> {
        if self.max_memory_mb == 0 {
            return Err(ExecutionError::ResourceLimitExceeded(
                "max_memory_mb must be greater than 0".to_string(),
            ));
        }

        if self.max_cpu_seconds == 0 {
            return Err(ExecutionError::ResourceLimitExceeded(
                "max_cpu_seconds must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Script execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptExecution {
    /// Skill identifier
    pub skill_id: String,

    /// Path to the script relative to skill directory (e.g., "scripts/main.py")
    pub script_path: String,

    /// Sandbox configuration
    pub sandbox: SandboxConfig,

    /// Execution timeout in milliseconds
    pub timeout_ms: u64,

    /// Input data passed to the script (as JSON)
    #[serde(default)]
    pub input: serde_json::Value,

    /// Optional skill manifest payload for security review
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_manifest: Option<serde_json::Value>,

    /// Optional approval context for high-risk executions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<ApprovalContext>,
}

impl ScriptExecution {
    /// Create a new script execution request
    pub fn new(
        skill_id: impl Into<String>,
        script_path: impl Into<String>,
        sandbox: SandboxConfig,
        timeout_ms: u64,
    ) -> Self {
        Self {
            skill_id: skill_id.into(),
            script_path: script_path.into(),
            sandbox,
            timeout_ms,
            input: serde_json::Value::Null,
            skill_manifest: None,
            approval: None,
        }
    }

    /// Set the input data
    pub fn with_input(mut self, input: serde_json::Value) -> Self {
        self.input = input;
        self
    }

    /// Attach a skill manifest for security review
    pub fn with_manifest(mut self, manifest: serde_json::Value) -> Self {
        self.skill_manifest = Some(manifest);
        self
    }

    /// Attach an approval context for high-risk executions
    pub fn with_approval(mut self, approval: ApprovalContext) -> Self {
        self.approval = Some(approval);
        self
    }

    /// Get the timeout as a Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    fn approval_granted(&self) -> bool {
        self.approval.as_ref().map(|a| a.approved).unwrap_or(false)
    }

    /// Validate the execution request
    pub fn validate(&self) -> Result<(), ExecutionError> {
        // Validate sandbox config
        self.sandbox.validate()?;

        // Validate script path
        if self.script_path.is_empty() {
            return Err(ExecutionError::InvalidScriptPath(
                "script_path cannot be empty".to_string(),
            ));
        }

        if let Some(decision) = classify_safe_path(&self.script_path) {
            tracing::info!(
                skill_id = %self.skill_id,
                script_path = %self.script_path,
                safe = decision.safe,
                reason = %decision.reason,
                "Skill path safety check"
            );

            if !decision.safe {
                return Err(ExecutionError::InvalidScriptPath(decision.reason));
            }
        } else {
            // Fallback heuristics if DSPy unavailable
            if self.script_path.contains("..") {
                return Err(ExecutionError::InvalidScriptPath(
                    "script_path cannot contain '..'".to_string(),
                ));
            }

            if !self.script_path.starts_with("scripts/") {
                return Err(ExecutionError::InvalidScriptPath(
                    "script_path must start with 'scripts/'".to_string(),
                ));
            }
        }

        // Validate filesystem permissions
        if let Some((paths, operation)) = self.sandbox.filesystem_request() {
            let requested_paths = serde_json::to_string(&paths).unwrap_or_default();
            if let Some(decision) = classify_filesystem_permission(
                &self.skill_id,
                &requested_paths,
                operation,
            ) {
                tracing::info!(
                    skill_id = %self.skill_id,
                    operation = %operation,
                    allowed = decision.allowed,
                    reasoning = %decision.reasoning,
                    "Filesystem permission review"
                );

                if !decision.allowed {
                    return Err(ExecutionError::SandboxViolation(decision.reasoning));
                }
            }
        }

        // Validate resource limits
        let requested_limits = serde_json::json!({
            "max_memory_mb": self.sandbox.max_memory_mb,
            "max_cpu_seconds": self.sandbox.max_cpu_seconds,
            "allow_network": self.sandbox.allow_network,
        })
        .to_string();

        if let Some(decision) = classify_resource_limits(&self.skill_id, &requested_limits) {
            tracing::info!(
                skill_id = %self.skill_id,
                approved_limits = %decision.approved_limits,
                adjustments = %decision.adjustments,
                "Resource limit review"
            );

            if let Some(enforced) = ApprovedLimits::from_json(&decision.approved_limits) {
                if enforced.exceeds_request(&self.sandbox) {
                    return Err(ExecutionError::ResourceLimitExceeded(format!(
                        "requested limits exceed approved: {}",
                        decision.approved_limits
                    )));
                }
            }
        }

        // Skill security classification
        let manifest_payload = self
            .skill_manifest
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
            .unwrap_or_else(|| {
                serde_json::json!({
                    "skill_id": self.skill_id,
                    "script_path": self.script_path,
                })
                .to_string()
            });

        let requested_permissions = serde_json::json!({
            "allow_network": self.sandbox.allow_network,
            "filesystem": self.sandbox.allow_filesystem,
            "max_memory_mb": self.sandbox.max_memory_mb,
            "max_cpu_seconds": self.sandbox.max_cpu_seconds,
        })
        .to_string();

        let input_keys: Vec<String> = self
            .input
            .as_object()
            .map(|obj| obj.keys().cloned().collect())
            .unwrap_or_default();

        let execution_context = serde_json::json!({
            "script_path": self.script_path,
            "timeout_ms": self.timeout_ms,
            "input_keys": input_keys,
        })
        .to_string();

        if let Some(decision) =
            classify_skill_security(&manifest_payload, &requested_permissions, &execution_context)
        {
            let requires_approval =
                decision.requires_approval || decision.risk_level.requires_approval();

            tracing::info!(
                skill_id = %self.skill_id,
                risk_level = ?decision.risk_level,
                requires_approval = requires_approval,
                concerns = ?decision.concerns,
                recommended_sandbox = %decision.recommended_sandbox,
                "Skill security review"
            );

            if requires_approval && !self.approval_granted() {
                return Err(ExecutionError::ApprovalRequired(format!(
                    "risk level {:?} requires approval",
                    decision.risk_level
                )));
            }
        }

        // Validate timeout
        if self.timeout_ms == 0 {
            return Err(ExecutionError::Timeout(0));
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct ApprovedLimits {
    max_memory_mb: Option<u32>,
    max_cpu_seconds: Option<u32>,
}

impl ApprovedLimits {
    fn from_json(raw: &str) -> Option<Self> {
        serde_json::from_str(raw).ok()
    }

    fn exceeds_request(&self, sandbox: &SandboxConfig) -> bool {
        if let Some(max_memory_mb) = self.max_memory_mb {
            if sandbox.max_memory_mb > max_memory_mb {
                return true;
            }
        }
        if let Some(max_cpu_seconds) = self.max_cpu_seconds {
            if sandbox.max_cpu_seconds > max_cpu_seconds {
                return true;
            }
        }
        false
    }
}

/// Result of script execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether the script executed successfully (exit code 0)
    pub success: bool,

    /// Standard output from the script
    pub stdout: String,

    /// Standard error from the script
    pub stderr: String,

    /// Exit code from the script
    pub exit_code: i32,

    /// Execution duration in milliseconds
    pub duration_ms: u64,

    /// Whether the execution was terminated due to timeout
    #[serde(default)]
    pub timed_out: bool,

    /// Resource usage statistics (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_usage: Option<ResourceUsage>,
}

impl ExecutionResult {
    /// Create a new execution result
    pub fn new(
        success: bool,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
        exit_code: i32,
        duration_ms: u64,
    ) -> Self {
        Self {
            success,
            stdout: stdout.into(),
            stderr: stderr.into(),
            exit_code,
            duration_ms,
            timed_out: false,
            resource_usage: None,
        }
    }

    /// Create a timeout result
    pub fn timeout(duration_ms: u64) -> Self {
        Self {
            success: false,
            stdout: String::new(),
            stderr: "Execution timed out".to_string(),
            exit_code: -1,
            duration_ms,
            timed_out: true,
            resource_usage: None,
        }
    }

    /// Mark this result as a timeout
    pub fn with_timeout(mut self) -> Self {
        self.timed_out = true;
        self
    }

    /// Add resource usage statistics
    pub fn with_resource_usage(mut self, usage: ResourceUsage) -> Self {
        self.resource_usage = Some(usage);
        self
    }
}

/// Resource usage statistics from script execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Peak memory usage in megabytes
    pub peak_memory_mb: u32,

    /// CPU time used in seconds
    pub cpu_seconds: f64,

    /// Number of network requests made (if network access enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_requests: Option<u32>,

    /// Number of file reads
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_reads: Option<u32>,

    /// Number of file writes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_writes: Option<u32>,
}

impl ResourceUsage {
    /// Create new resource usage stats
    pub fn new(peak_memory_mb: u32, cpu_seconds: f64) -> Self {
        Self {
            peak_memory_mb,
            cpu_seconds,
            network_requests: None,
            file_reads: None,
            file_writes: None,
        }
    }

    /// Check if any limits were exceeded
    pub fn exceeds_limits(&self, config: &SandboxConfig) -> bool {
        self.peak_memory_mb > config.max_memory_mb
            || self.cpu_seconds > config.max_cpu_seconds as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_access_none() {
        let access = FilesystemAccess::None;
        let path = PathBuf::from("/tmp/test.txt");

        assert!(!access.can_read(&path));
        assert!(!access.can_write(&path));
    }

    #[test]
    fn test_filesystem_access_read_only() {
        let paths = vec![PathBuf::from("/tmp")];
        let access = FilesystemAccess::read_only(paths);
        let path = PathBuf::from("/tmp/test.txt");

        assert!(access.can_read(&path));
        assert!(!access.can_write(&path));
    }

    #[test]
    fn test_filesystem_access_read_write() {
        let paths = vec![PathBuf::from("/tmp")];
        let access = FilesystemAccess::read_write(paths);
        let path = PathBuf::from("/tmp/test.txt");

        assert!(access.can_read(&path));
        assert!(access.can_write(&path));
    }

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();

        assert!(!config.allow_network);
        assert_eq!(config.allow_filesystem, FilesystemAccess::None);
        assert_eq!(config.max_memory_mb, 512);
        assert_eq!(config.max_cpu_seconds, 30);
        assert!(config.env_vars.is_empty());
    }

    #[test]
    fn test_sandbox_config_builder() {
        let config = SandboxConfig::new()
            .with_network(true)
            .with_max_memory_mb(1024)
            .with_max_cpu_seconds(60)
            .with_env_var("API_KEY", "secret");

        assert!(config.allow_network);
        assert_eq!(config.max_memory_mb, 1024);
        assert_eq!(config.max_cpu_seconds, 60);
        assert_eq!(config.env_vars.get("API_KEY"), Some(&"secret".to_string()));
    }

    #[test]
    fn test_sandbox_config_validation() {
        let valid = SandboxConfig::default();
        assert!(valid.validate().is_ok());

        let invalid_memory = SandboxConfig {
            max_memory_mb: 0,
            ..Default::default()
        };
        assert!(invalid_memory.validate().is_err());

        let invalid_cpu = SandboxConfig {
            max_cpu_seconds: 0,
            ..Default::default()
        };
        assert!(invalid_cpu.validate().is_err());
    }

    #[test]
    fn test_script_execution_validation() {
        let valid = ScriptExecution::new(
            "my-skill",
            "scripts/main.py",
            SandboxConfig::default(),
            5000,
        );
        assert!(valid.validate().is_ok());

        // Invalid: empty script path
        let invalid_empty = ScriptExecution::new("my-skill", "", SandboxConfig::default(), 5000);
        assert!(invalid_empty.validate().is_err());

        // Invalid: path traversal
        let invalid_traversal = ScriptExecution::new(
            "my-skill",
            "scripts/../etc/passwd",
            SandboxConfig::default(),
            5000,
        );
        assert!(invalid_traversal.validate().is_err());

        // Invalid: not in scripts/ directory
        let invalid_dir =
            ScriptExecution::new("my-skill", "main.py", SandboxConfig::default(), 5000);
        assert!(invalid_dir.validate().is_err());

        // Invalid: zero timeout
        let invalid_timeout =
            ScriptExecution::new("my-skill", "scripts/main.py", SandboxConfig::default(), 0);
        assert!(invalid_timeout.validate().is_err());
    }

    #[test]
    fn test_script_execution_with_input() {
        let input = serde_json::json!({
            "param1": "value1",
            "param2": 42
        });

        let exec = ScriptExecution::new(
            "my-skill",
            "scripts/process.py",
            SandboxConfig::default(),
            10000,
        )
        .with_input(input.clone());

        assert_eq!(exec.input, input);
    }

    #[test]
    fn test_script_execution_timeout() {
        let exec = ScriptExecution::new(
            "my-skill",
            "scripts/main.py",
            SandboxConfig::default(),
            5000,
        );

        assert_eq!(exec.timeout(), Duration::from_millis(5000));
    }

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult::new(true, "Hello, world!", "", 0, 1234);

        assert!(result.success);
        assert_eq!(result.stdout, "Hello, world!");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.duration_ms, 1234);
        assert!(!result.timed_out);
    }

    #[test]
    fn test_execution_result_failure() {
        let result = ExecutionResult::new(false, "", "Error: file not found", 1, 567);

        assert!(!result.success);
        assert_eq!(result.stderr, "Error: file not found");
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_execution_result_timeout() {
        let result = ExecutionResult::timeout(5000);

        assert!(!result.success);
        assert!(result.timed_out);
        assert_eq!(result.exit_code, -1);
        assert_eq!(result.duration_ms, 5000);
    }

    #[test]
    fn test_resource_usage() {
        let usage = ResourceUsage::new(256, 15.5);
        assert_eq!(usage.peak_memory_mb, 256);
        assert_eq!(usage.cpu_seconds, 15.5);

        let config = SandboxConfig::default(); // 512 MB, 30s
        assert!(!usage.exceeds_limits(&config));

        let high_usage = ResourceUsage::new(600, 10.0);
        assert!(high_usage.exceeds_limits(&config));

        let high_cpu = ResourceUsage::new(200, 35.0);
        assert!(high_cpu.exceeds_limits(&config));
    }

    #[test]
    fn test_execution_result_with_resource_usage() {
        let usage = ResourceUsage::new(128, 5.5);
        let result = ExecutionResult::new(true, "output", "", 0, 5500).with_resource_usage(usage);

        assert!(result.resource_usage.is_some());
        assert_eq!(result.resource_usage.as_ref().unwrap().peak_memory_mb, 128);
    }

    #[test]
    fn test_filesystem_access_serde() {
        let access = FilesystemAccess::read_only(vec![PathBuf::from("/tmp")]);
        let json = serde_json::to_string(&access).unwrap();
        let deserialized: FilesystemAccess = serde_json::from_str(&json).unwrap();
        assert_eq!(access, deserialized);
    }

    #[test]
    fn test_sandbox_config_serde() {
        let config = SandboxConfig::new()
            .with_network(true)
            .with_max_memory_mb(1024);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SandboxConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.allow_network, deserialized.allow_network);
        assert_eq!(config.max_memory_mb, deserialized.max_memory_mb);
    }

    #[test]
    fn test_script_execution_serde() {
        let exec = ScriptExecution::new(
            "my-skill",
            "scripts/main.py",
            SandboxConfig::default(),
            10000,
        );

        let json = serde_json::to_string(&exec).unwrap();
        let deserialized: ScriptExecution = serde_json::from_str(&json).unwrap();

        assert_eq!(exec.skill_id, deserialized.skill_id);
        assert_eq!(exec.script_path, deserialized.script_path);
        assert_eq!(exec.timeout_ms, deserialized.timeout_ms);
    }

    #[test]
    fn test_execution_result_serde() {
        let result = ExecutionResult::new(true, "output", "error", 0, 1000);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ExecutionResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.success, deserialized.success);
        assert_eq!(result.stdout, deserialized.stdout);
        assert_eq!(result.exit_code, deserialized.exit_code);
    }
}
