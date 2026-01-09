//! Pylon Sandbox Provider for CPU-intensive DSPy operations.
//!
//! This module provides sandboxed execution for:
//! - Running tests
//! - Building projects
//! - Linting code
//! - Other deterministic, verifiable operations
//!
//! Separate from the Pylon LM provider, as sandbox jobs have different
//! resource profiles and verification characteristics.

use anyhow::Result;
use nostr::{Keypair, generate_secret_key, get_public_key};
use protocol::jobs::{SandboxRunRequest, SandboxRunResponse};
use protocol::jobs::sandbox::{
    CommandResult, EnvInfo, NetworkPolicy, RepoMount, ResourceLimits, SandboxCommand,
    SandboxConfig, SandboxStatus,
};
use protocol::provenance::Provenance;
use serde::{Deserialize, Serialize};

/// Resource profile for sandbox execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SandboxProfile {
    /// Small: 1 vCPU, 1GB RAM, 5GB disk
    Small,
    /// Medium: 2 vCPU, 4GB RAM, 8GB disk
    Medium,
    /// Large: 4 vCPU, 8GB RAM, 10GB disk
    Large,
}

impl SandboxProfile {
    /// Get resource limits for this profile.
    pub fn limits(&self) -> ResourceLimits {
        match self {
            Self::Small => ResourceLimits {
                memory_mb: 1024,
                cpus: 1.0,
                timeout_secs: 60,
                disk_mb: Some(5 * 1024),
            },
            Self::Medium => ResourceLimits {
                memory_mb: 4096,
                cpus: 2.0,
                timeout_secs: 120,
                disk_mb: Some(8 * 1024),
            },
            Self::Large => ResourceLimits {
                memory_mb: 8192,
                cpus: 4.0,
                timeout_secs: 300,
                disk_mb: Some(10 * 1024),
            },
        }
    }
}

impl Default for SandboxProfile {
    fn default() -> Self {
        Self::Medium
    }
}

/// Configuration for the Pylon sandbox provider.
#[derive(Debug, Clone)]
pub struct PylonSandboxConfig {
    /// Relay URL for NIP-90 job submission.
    pub relay_url: String,
    /// Default resource profile.
    pub profile: SandboxProfile,
    /// Default Docker image digest.
    pub default_image: String,
    /// Default network policy.
    pub network_policy: NetworkPolicy,
    /// Timeout for waiting on job results (milliseconds).
    pub result_timeout_ms: u64,
}

impl Default for PylonSandboxConfig {
    fn default() -> Self {
        Self {
            relay_url: "wss://nexus.openagents.com".to_string(),
            profile: SandboxProfile::Medium,
            default_image: "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            network_policy: NetworkPolicy::None,
            result_timeout_ms: 300_000, // 5 minutes
        }
    }
}

/// Pylon sandbox provider for executing commands in sandboxed environments.
///
/// This provider submits NIP-90 kind:5050 jobs for sandbox execution
/// and waits for kind:6050 results.
///
/// # Example
///
/// ```ignore
/// use dsrs::adapter::PylonSandboxProvider;
/// use dsrs::adapter::pylon_sandbox::SandboxProfile;
///
/// let provider = PylonSandboxProvider::generate()
///     .with_profile(SandboxProfile::Large)
///     .with_image("sha256:abc123...");
///
/// let result = provider.run_commands(vec![
///     "cargo build --release",
///     "cargo test",
/// ]).await?;
/// ```
pub struct PylonSandboxProvider {
    /// Nostr keypair for signing.
    keypair: Keypair,
    /// Configuration.
    config: PylonSandboxConfig,
}

impl PylonSandboxProvider {
    /// Create a new sandbox provider with a keypair.
    pub fn new(keypair: Keypair) -> Self {
        Self {
            keypair,
            config: PylonSandboxConfig::default(),
        }
    }

    /// Create a new sandbox provider with a random keypair.
    pub fn generate() -> Self {
        let secret_key = generate_secret_key();
        let keypair = Keypair {
            private_key: secret_key,
            public_key: get_public_key(&secret_key).expect("valid secret key"),
        };
        Self::new(keypair)
    }

    /// Set the configuration.
    pub fn with_config(mut self, config: PylonSandboxConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the resource profile.
    pub fn with_profile(mut self, profile: SandboxProfile) -> Self {
        self.config.profile = profile;
        self
    }

    /// Set the Docker image digest.
    pub fn with_image(mut self, image: impl Into<String>) -> Self {
        self.config.default_image = image.into();
        self
    }

    /// Set the network policy.
    pub fn with_network_policy(mut self, policy: NetworkPolicy) -> Self {
        self.config.network_policy = policy;
        self
    }

    /// Set the relay URL.
    pub fn with_relay(mut self, url: impl Into<String>) -> Self {
        self.config.relay_url = url.into();
        self
    }

    /// Create a sandbox run request.
    pub fn create_request(&self, commands: Vec<String>) -> SandboxRunRequest {
        SandboxRunRequest {
            sandbox: SandboxConfig {
                provider: "docker".to_string(),
                image_digest: self.config.default_image.clone(),
                network_policy: self.config.network_policy,
                resources: self.config.profile.limits(),
            },
            repo: RepoMount::default(),
            commands: commands
                .into_iter()
                .map(|cmd| SandboxCommand::new(cmd))
                .collect(),
            env: std::collections::HashMap::new(),
            verification: protocol::verification::Verification::objective(),
        }
    }

    /// Create a request with a specific repo to mount.
    pub fn create_request_with_repo(
        &self,
        commands: Vec<String>,
        repo_source: impl Into<String>,
        git_ref: Option<String>,
    ) -> SandboxRunRequest {
        let mut request = self.create_request(commands);
        request.repo = RepoMount {
            source: repo_source.into(),
            git_ref,
            mount_path: "/workspace".to_string(),
        };
        request
    }

    /// Run commands in the sandbox (local execution for testing).
    ///
    /// In production, this would submit a NIP-90 job to the relay.
    /// This implementation provides a local mock for testing.
    pub async fn run(&self, request: SandboxRunRequest) -> Result<SandboxRunResponse> {
        // For now, return a mock response
        // TODO: Implement actual NIP-90 job submission
        let response = SandboxRunResponse {
            env_info: EnvInfo {
                image_digest: request.sandbox.image_digest.clone(),
                hostname: Some("sandbox-mock".to_string()),
                system_info: None,
            },
            runs: request
                .commands
                .iter()
                .map(|cmd| CommandResult {
                    cmd: cmd.cmd.clone(),
                    exit_code: 0,
                    duration_ms: 100,
                    stdout_sha256: "mock_stdout_hash".to_string(),
                    stderr_sha256: "mock_stderr_hash".to_string(),
                    stdout_preview: Some("Mock output".to_string()),
                    stderr_preview: None,
                })
                .collect(),
            artifacts: Vec::new(),
            status: SandboxStatus::Success,
            error: None,
            provenance: Provenance::new("sandbox-executor")
                .with_provider(&self.keypair.public_key_hex()),
        };

        Ok(response)
    }

    /// Run a single command.
    pub async fn run_command(&self, command: impl Into<String>) -> Result<SandboxRunResponse> {
        let request = self.create_request(vec![command.into()]);
        self.run(request).await
    }

    /// Run multiple commands.
    pub async fn run_commands(&self, commands: Vec<impl Into<String>>) -> Result<SandboxRunResponse> {
        let request = self.create_request(commands.into_iter().map(|c| c.into()).collect());
        self.run(request).await
    }

    /// Check if all commands succeeded.
    pub fn all_succeeded(response: &SandboxRunResponse) -> bool {
        response.status == SandboxStatus::Success
            && response.runs.iter().all(|r| r.exit_code == 0)
    }

    /// Get the public key of this provider as hex string.
    pub fn public_key_hex(&self) -> String {
        self.keypair.public_key_hex()
    }

    /// Get the public key bytes.
    pub fn public_key(&self) -> [u8; 32] {
        self.keypair.public_key
    }
}

/// Result of sandbox execution with additional metadata.
#[derive(Debug, Clone)]
pub struct SandboxResult {
    /// The response from the sandbox.
    pub response: SandboxRunResponse,
    /// Whether all commands succeeded.
    pub success: bool,
    /// Total duration across all commands (ms).
    pub total_duration_ms: u64,
    /// Total tokens if any LM was used.
    pub tokens_used: u64,
}

impl SandboxResult {
    /// Create from a response.
    pub fn from_response(response: SandboxRunResponse) -> Self {
        let success = PylonSandboxProvider::all_succeeded(&response);
        let total_duration_ms = response.runs.iter().map(|r| r.duration_ms).sum();

        Self {
            response,
            success,
            total_duration_ms,
            tokens_used: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_profile_limits() {
        let small = SandboxProfile::Small.limits();
        assert_eq!(small.memory_mb, 1024);
        assert_eq!(small.cpus, 1.0);

        let large = SandboxProfile::Large.limits();
        assert_eq!(large.memory_mb, 8192);
        assert_eq!(large.cpus, 4.0);
    }

    #[test]
    fn test_provider_creation() {
        let provider = PylonSandboxProvider::generate()
            .with_profile(SandboxProfile::Large)
            .with_network_policy(NetworkPolicy::Localhost);

        assert_eq!(provider.config.profile, SandboxProfile::Large);
        assert_eq!(provider.config.network_policy, NetworkPolicy::Localhost);
    }

    #[test]
    fn test_create_request() {
        let provider = PylonSandboxProvider::generate()
            .with_profile(SandboxProfile::Small)
            .with_image("sha256:test123");

        let request = provider.create_request(vec![
            "cargo build".to_string(),
            "cargo test".to_string(),
        ]);

        assert_eq!(request.commands.len(), 2);
        assert_eq!(request.sandbox.image_digest, "sha256:test123");
        assert_eq!(request.sandbox.resources.memory_mb, 1024);
    }

    #[test]
    fn test_create_request_with_repo() {
        let provider = PylonSandboxProvider::generate();

        let request = provider.create_request_with_repo(
            vec!["cargo test".to_string()],
            "https://github.com/example/repo",
            Some("main".to_string()),
        );

        assert_eq!(request.repo.source, "https://github.com/example/repo");
        assert_eq!(request.repo.git_ref, Some("main".to_string()));
    }

    #[tokio::test]
    async fn test_run_mock() {
        let provider = PylonSandboxProvider::generate();

        let response = provider.run_command("echo hello").await.unwrap();

        assert_eq!(response.status, SandboxStatus::Success);
        assert_eq!(response.runs.len(), 1);
        assert_eq!(response.runs[0].exit_code, 0);
    }

    #[tokio::test]
    async fn test_run_multiple_commands() {
        let provider = PylonSandboxProvider::generate();

        let response = provider
            .run_commands(vec!["cmd1", "cmd2", "cmd3"])
            .await
            .unwrap();

        assert_eq!(response.runs.len(), 3);
        assert!(PylonSandboxProvider::all_succeeded(&response));
    }

    #[test]
    fn test_sandbox_result() {
        let response = SandboxRunResponse {
            env_info: EnvInfo {
                image_digest: "test".to_string(),
                hostname: None,
                system_info: None,
            },
            runs: vec![
                CommandResult {
                    cmd: "cmd1".to_string(),
                    exit_code: 0,
                    duration_ms: 100,
                    stdout_sha256: "".to_string(),
                    stderr_sha256: "".to_string(),
                    stdout_preview: None,
                    stderr_preview: None,
                },
                CommandResult {
                    cmd: "cmd2".to_string(),
                    exit_code: 0,
                    duration_ms: 200,
                    stdout_sha256: "".to_string(),
                    stderr_sha256: "".to_string(),
                    stdout_preview: None,
                    stderr_preview: None,
                },
            ],
            artifacts: Vec::new(),
            status: SandboxStatus::Success,
            error: None,
            provenance: Provenance::new("test"),
        };

        let result = SandboxResult::from_response(response);

        assert!(result.success);
        assert_eq!(result.total_duration_ms, 300);
    }
}
