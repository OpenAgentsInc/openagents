use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::ProviderSandboxEnvironmentVar;

pub const PODMAN_LEGAL_INPUT_PATH: &str = "/workspace/inputs";
pub const PODMAN_LEGAL_SCRATCH_PATH: &str = "/workspace/scratch";
pub const PODMAN_LEGAL_OUTPUT_PATH: &str = "/workspace/output";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodmanNetworkPolicy {
    #[default]
    Disabled,
    Bridge,
    Host,
}

impl PodmanNetworkPolicy {
    const fn podman_arg(self) -> &'static str {
        match self {
            Self::Disabled => "none",
            Self::Bridge => "bridge",
            Self::Host => "host",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodmanMountAccess {
    #[default]
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PodmanSandboxMount {
    pub host_path: PathBuf,
    pub container_path: String,
    pub access: PodmanMountAccess,
}

impl PodmanSandboxMount {
    pub fn read_only(host_path: impl Into<PathBuf>, container_path: impl Into<String>) -> Self {
        Self {
            host_path: host_path.into(),
            container_path: container_path.into(),
            access: PodmanMountAccess::ReadOnly,
        }
    }

    pub fn read_write(host_path: impl Into<PathBuf>, container_path: impl Into<String>) -> Self {
        Self {
            host_path: host_path.into(),
            container_path: container_path.into(),
            access: PodmanMountAccess::ReadWrite,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PodmanSandboxConfig {
    pub image: String,
    pub container_name: Option<String>,
    pub workdir: String,
    pub mounts: Vec<PodmanSandboxMount>,
    pub environment: Vec<ProviderSandboxEnvironmentVar>,
    pub cpu_limit: Option<u32>,
    pub memory_limit_mb: Option<u64>,
    pub pid_limit: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub network_policy: PodmanNetworkPolicy,
    pub allowed_host_roots: Vec<PathBuf>,
    pub read_only_root_filesystem: bool,
}

impl Default for PodmanSandboxConfig {
    fn default() -> Self {
        Self {
            image: String::new(),
            container_name: None,
            workdir: "/workspace".to_string(),
            mounts: Vec::new(),
            environment: Vec::new(),
            cpu_limit: None,
            memory_limit_mb: None,
            pid_limit: Some(256),
            timeout_ms: Some(120_000),
            network_policy: PodmanNetworkPolicy::Disabled,
            allowed_host_roots: Vec::new(),
            read_only_root_filesystem: true,
        }
    }
}

impl PodmanSandboxConfig {
    pub fn legal_benchmark(
        image: impl Into<String>,
        input_root: impl Into<PathBuf>,
        scratch_root: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        let input_root = input_root.into();
        let scratch_root = scratch_root.into();
        let output_root = output_root.into();
        Self {
            image: image.into(),
            mounts: vec![
                PodmanSandboxMount::read_only(input_root.clone(), PODMAN_LEGAL_INPUT_PATH),
                PodmanSandboxMount::read_write(scratch_root.clone(), PODMAN_LEGAL_SCRATCH_PATH),
                PodmanSandboxMount::read_write(output_root.clone(), PODMAN_LEGAL_OUTPUT_PATH),
            ],
            allowed_host_roots: vec![input_root, scratch_root, output_root],
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PodmanSandboxResourceLimits {
    pub cpu_limit: Option<u32>,
    pub memory_limit_mb: Option<u64>,
    pub pid_limit: Option<u32>,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PodmanSandboxCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub image: String,
    pub network_policy: PodmanNetworkPolicy,
    pub mounts: Vec<PodmanSandboxMount>,
    pub resource_limits: PodmanSandboxResourceLimits,
}

impl PodmanSandboxCommand {
    pub fn timeout(&self) -> Option<Duration> {
        self.resource_limits.timeout_ms.map(Duration::from_millis)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodmanSandboxCommandState {
    Succeeded,
    Failed,
    TimedOut,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PodmanSandboxCommandReceipt {
    pub backend: String,
    pub command_digest: String,
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
    pub wall_time_ms: u64,
    pub final_state: PodmanSandboxCommandState,
    pub exit_code: Option<i32>,
    pub exit_signal: Option<i32>,
    pub timed_out: bool,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub image: String,
    pub network_policy: PodmanNetworkPolicy,
    pub resource_limits: PodmanSandboxResourceLimits,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PodmanSandboxCommandOutput {
    pub receipt: PodmanSandboxCommandReceipt,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub trait SandboxCommandBackend {
    type Config;
    type Command;
    type Output;
    type Error: std::error::Error;

    fn backend_id(&self) -> &'static str;

    fn build_sandbox_command(
        &self,
        config: &Self::Config,
        command: &[String],
    ) -> Result<Self::Command, Self::Error>;

    fn run_sandbox_command(
        &self,
        config: &Self::Config,
        command: &[String],
    ) -> Result<Self::Output, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PodmanSandboxBackend {
    podman_binary: PathBuf,
}

impl Default for PodmanSandboxBackend {
    fn default() -> Self {
        Self {
            podman_binary: PathBuf::from("podman"),
        }
    }
}

impl PodmanSandboxBackend {
    pub fn new(podman_binary: impl Into<PathBuf>) -> Self {
        Self {
            podman_binary: podman_binary.into(),
        }
    }

    pub fn podman_binary(&self) -> &Path {
        self.podman_binary.as_path()
    }
}

impl SandboxCommandBackend for PodmanSandboxBackend {
    type Command = PodmanSandboxCommand;
    type Config = PodmanSandboxConfig;
    type Error = PodmanSandboxError;
    type Output = PodmanSandboxCommandOutput;

    fn backend_id(&self) -> &'static str {
        "podman.local.v1"
    }

    fn build_sandbox_command(
        &self,
        config: &PodmanSandboxConfig,
        command: &[String],
    ) -> Result<PodmanSandboxCommand, PodmanSandboxError> {
        let validated = validate_config(config, command)?;
        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--pull=never".to_string(),
            "--cap-drop".to_string(),
            "all".to_string(),
            "--security-opt".to_string(),
            "no-new-privileges".to_string(),
            "--network".to_string(),
            config.network_policy.podman_arg().to_string(),
            "--workdir".to_string(),
            config.workdir.clone(),
        ];
        if config.read_only_root_filesystem {
            args.push("--read-only".to_string());
        }
        if let Some(container_name) = config.container_name.as_deref() {
            args.push("--name".to_string());
            args.push(container_name.to_string());
        }
        if let Some(cpu_limit) = config.cpu_limit {
            args.push("--cpus".to_string());
            args.push(cpu_limit.to_string());
        }
        if let Some(memory_limit_mb) = config.memory_limit_mb {
            args.push("--memory".to_string());
            args.push(format!("{memory_limit_mb}m"));
        }
        if let Some(pid_limit) = config.pid_limit {
            args.push("--pids-limit".to_string());
            args.push(pid_limit.to_string());
        }
        for mount in &validated.mounts {
            args.push("--mount".to_string());
            args.push(mount.mount_arg());
        }
        for env in &config.environment {
            args.push("--env".to_string());
            args.push(format!("{}={}", env.key, env.value));
        }
        args.push(config.image.clone());
        args.extend(command.iter().cloned());

        Ok(PodmanSandboxCommand {
            program: self.podman_binary.clone(),
            args,
            image: config.image.clone(),
            network_policy: config.network_policy,
            mounts: validated
                .mounts
                .into_iter()
                .map(ValidatedPodmanMount::into_mount)
                .collect(),
            resource_limits: PodmanSandboxResourceLimits {
                cpu_limit: config.cpu_limit,
                memory_limit_mb: config.memory_limit_mb,
                pid_limit: config.pid_limit,
                timeout_ms: config.timeout_ms,
            },
        })
    }

    fn run_sandbox_command(
        &self,
        config: &PodmanSandboxConfig,
        command: &[String],
    ) -> Result<PodmanSandboxCommandOutput, PodmanSandboxError> {
        let prepared = self.build_sandbox_command(config, command)?;
        run_prepared_sandbox_command(self.backend_id(), &prepared)
    }
}

#[derive(Debug, Error)]
pub enum PodmanSandboxError {
    #[error("podman sandbox image cannot be empty")]
    EmptyImage,
    #[error("podman sandbox command cannot be empty")]
    EmptyCommand,
    #[error("podman sandbox workdir `{path}` is invalid: {detail}")]
    InvalidWorkdir { path: String, detail: String },
    #[error("podman sandbox container path `{path}` is invalid: {detail}")]
    InvalidContainerPath { path: String, detail: String },
    #[error("podman sandbox environment key `{key}` is invalid")]
    InvalidEnvironmentKey { key: String },
    #[error("podman sandbox host path `{path}` contains an unsupported mount separator")]
    HostPathContainsMountSeparator { path: PathBuf },
    #[error("podman sandbox host path `{path}` does not exist")]
    HostPathMissing { path: PathBuf },
    #[error("podman sandbox failed to canonicalize host path `{path}`: {message}")]
    HostPathCanonicalize { path: PathBuf, message: String },
    #[error("podman sandbox failed to canonicalize allowed root `{path}`: {message}")]
    AllowedRootCanonicalize { path: PathBuf, message: String },
    #[error("podman sandbox host path `{path}` is outside allowed roots")]
    HostPathOutsideAllowedRoots {
        path: PathBuf,
        allowed_roots: Vec<PathBuf>,
    },
    #[error("podman sandbox container path `{path}` is mounted more than once")]
    DuplicateContainerPath { path: String },
    #[error("podman sandbox failed to spawn command: {message}")]
    Spawn { message: String },
    #[error("podman sandbox failed while polling command: {message}")]
    Poll { message: String },
    #[error("podman sandbox failed while collecting command output: {message}")]
    CollectOutput { message: String },
}

struct ValidatedPodmanConfig {
    mounts: Vec<ValidatedPodmanMount>,
}

struct ValidatedPodmanMount {
    host_path: PathBuf,
    container_path: String,
    access: PodmanMountAccess,
}

impl ValidatedPodmanMount {
    fn mount_arg(&self) -> String {
        let mut mount = format!(
            "type=bind,source={},target={}",
            self.host_path.display(),
            self.container_path
        );
        if self.access == PodmanMountAccess::ReadOnly {
            mount.push_str(",readonly");
        }
        mount
    }

    fn into_mount(self) -> PodmanSandboxMount {
        PodmanSandboxMount {
            host_path: self.host_path,
            container_path: self.container_path,
            access: self.access,
        }
    }
}

fn validate_config(
    config: &PodmanSandboxConfig,
    command: &[String],
) -> Result<ValidatedPodmanConfig, PodmanSandboxError> {
    if config.image.trim().is_empty() {
        return Err(PodmanSandboxError::EmptyImage);
    }
    if command.is_empty() || command.iter().any(|part| part.trim().is_empty()) {
        return Err(PodmanSandboxError::EmptyCommand);
    }
    validate_absolute_container_path(config.workdir.as_str()).map_err(|detail| {
        PodmanSandboxError::InvalidWorkdir {
            path: config.workdir.clone(),
            detail,
        }
    })?;
    for env in &config.environment {
        if env.key.trim().is_empty() || env.key.contains('=') || env.key.contains('\0') {
            return Err(PodmanSandboxError::InvalidEnvironmentKey {
                key: env.key.clone(),
            });
        }
    }

    let allowed_roots = canonical_allowed_roots(config.allowed_host_roots.as_slice())?;
    let mut container_paths = BTreeSet::new();
    let mut mounts = Vec::with_capacity(config.mounts.len());
    for mount in &config.mounts {
        validate_absolute_container_path(mount.container_path.as_str()).map_err(|detail| {
            PodmanSandboxError::InvalidContainerPath {
                path: mount.container_path.clone(),
                detail,
            }
        })?;
        if !container_paths.insert(mount.container_path.clone()) {
            return Err(PodmanSandboxError::DuplicateContainerPath {
                path: mount.container_path.clone(),
            });
        }
        let canonical_host_path =
            canonical_mount_path(mount.host_path.as_path(), allowed_roots.as_slice())?;
        mounts.push(ValidatedPodmanMount {
            host_path: canonical_host_path,
            container_path: mount.container_path.clone(),
            access: mount.access,
        });
    }

    Ok(ValidatedPodmanConfig { mounts })
}

fn canonical_allowed_roots(roots: &[PathBuf]) -> Result<Vec<PathBuf>, PodmanSandboxError> {
    let mut canonical_roots = Vec::with_capacity(roots.len());
    for root in roots {
        canonical_roots.push(root.canonicalize().map_err(|error| {
            PodmanSandboxError::AllowedRootCanonicalize {
                path: root.clone(),
                message: error.to_string(),
            }
        })?);
    }
    Ok(canonical_roots)
}

fn canonical_mount_path(
    path: &Path,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf, PodmanSandboxError> {
    let path_string = path.to_string_lossy();
    if path_string.contains(',') || path_string.contains('\0') {
        return Err(PodmanSandboxError::HostPathContainsMountSeparator {
            path: path.to_path_buf(),
        });
    }
    if !path.exists() {
        return Err(PodmanSandboxError::HostPathMissing {
            path: path.to_path_buf(),
        });
    }
    let canonical =
        path.canonicalize()
            .map_err(|error| PodmanSandboxError::HostPathCanonicalize {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    if !allowed_roots.is_empty()
        && !allowed_roots
            .iter()
            .any(|allowed_root| canonical.starts_with(allowed_root))
    {
        return Err(PodmanSandboxError::HostPathOutsideAllowedRoots {
            path: canonical,
            allowed_roots: allowed_roots.to_vec(),
        });
    }
    Ok(canonical)
}

fn validate_absolute_container_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("path cannot be empty".to_string());
    }
    let parsed = Path::new(path);
    if !parsed.is_absolute() {
        return Err("path must be absolute".to_string());
    }
    if parsed == Path::new("/") {
        return Err("path cannot be the container root".to_string());
    }
    for component in parsed.components() {
        match component {
            Component::RootDir | Component::Normal(_) => {}
            Component::CurDir => {
                return Err("path cannot contain current-dir components".to_string());
            }
            Component::ParentDir => {
                return Err("path cannot contain parent-dir components".to_string());
            }
            Component::Prefix(_) => {
                return Err("path cannot contain host path prefixes".to_string());
            }
        }
    }
    if path.contains(',') || path.contains('\0') {
        return Err("path cannot contain mount separators".to_string());
    }
    Ok(())
}

pub fn run_prepared_sandbox_command(
    backend_id: &str,
    command: &PodmanSandboxCommand,
) -> Result<PodmanSandboxCommandOutput, PodmanSandboxError> {
    let started_at_ms = now_epoch_ms();
    let started = Instant::now();
    let mut child = Command::new(command.program.as_path())
        .args(command.args.as_slice())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|error| PodmanSandboxError::Spawn {
            message: error.to_string(),
        })?;

    let timeout = command.timeout();
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if timeout.is_some_and(|duration| started.elapsed() >= duration) {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return Err(PodmanSandboxError::Poll {
                    message: error.to_string(),
                });
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|error| PodmanSandboxError::CollectOutput {
            message: error.to_string(),
        })?;
    let ended_at_ms = now_epoch_ms();
    let final_state = if timed_out {
        PodmanSandboxCommandState::TimedOut
    } else if output.status.success() {
        PodmanSandboxCommandState::Succeeded
    } else {
        PodmanSandboxCommandState::Failed
    };
    let receipt = PodmanSandboxCommandReceipt {
        backend: backend_id.to_string(),
        command_digest: command_digest(command),
        started_at_ms,
        ended_at_ms,
        wall_time_ms: ended_at_ms.saturating_sub(started_at_ms).unsigned_abs(),
        final_state,
        exit_code: output.status.code(),
        exit_signal: exit_signal(&output.status),
        timed_out,
        stdout_digest: sha256_prefixed(output.stdout.as_slice()),
        stderr_digest: sha256_prefixed(output.stderr.as_slice()),
        stdout_bytes: u64::try_from(output.stdout.len()).unwrap_or(u64::MAX),
        stderr_bytes: u64::try_from(output.stderr.len()).unwrap_or(u64::MAX),
        image: command.image.clone(),
        network_policy: command.network_policy,
        resource_limits: command.resource_limits.clone(),
    };

    Ok(PodmanSandboxCommandOutput {
        receipt,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn command_digest(command: &PodmanSandboxCommand) -> String {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(command.program.to_string_lossy().as_bytes());
    for arg in &command.args {
        encoded.extend_from_slice(b"\0");
        encoded.extend_from_slice(arg.as_bytes());
    }
    sha256_prefixed(encoded.as_slice())
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn now_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(unix)]
fn exit_signal(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn backend() -> PodmanSandboxBackend {
        PodmanSandboxBackend::new("podman")
    }

    fn command_parts() -> Vec<String> {
        vec![
            "/bin/sh".to_string(),
            "-lc".to_string(),
            "cp /workspace/inputs/source.txt /workspace/output/source.txt".to_string(),
        ]
    }

    #[test]
    fn legal_benchmark_config_mounts_inputs_ro_and_outputs_rw()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let inputs = temp.path().join("inputs");
        let scratch = temp.path().join("scratch");
        let output = temp.path().join("output");
        fs::create_dir_all(inputs.as_path())?;
        fs::create_dir_all(scratch.as_path())?;
        fs::create_dir_all(output.as_path())?;

        let config = PodmanSandboxConfig::legal_benchmark(
            "localhost/openagents/legal-benchmark:latest",
            inputs.as_path(),
            scratch.as_path(),
            output.as_path(),
        );
        let prepared = backend().build_sandbox_command(&config, command_parts().as_slice())?;

        assert_eq!(prepared.network_policy, PodmanNetworkPolicy::Disabled);
        assert!(
            prepared
                .args
                .windows(2)
                .any(|window| window == ["--network", "none"])
        );
        assert!(
            prepared.args.iter().any(|arg| {
                arg.contains("target=/workspace/inputs") && arg.contains("readonly")
            })
        );
        assert!(
            prepared.args.iter().any(|arg| {
                arg.contains("target=/workspace/scratch") && !arg.contains("readonly")
            })
        );
        assert!(
            prepared.args.iter().any(|arg| {
                arg.contains("target=/workspace/output") && !arg.contains("readonly")
            })
        );
        Ok(())
    }

    #[test]
    fn traversal_container_mount_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let config = PodmanSandboxConfig {
            image: "localhost/openagents/legal-benchmark:latest".to_string(),
            mounts: vec![PodmanSandboxMount::read_only(
                temp.path(),
                "/workspace/../escape",
            )],
            allowed_host_roots: vec![temp.path().to_path_buf()],
            ..PodmanSandboxConfig::default()
        };

        let error = backend()
            .build_sandbox_command(&config, command_parts().as_slice())
            .err();
        assert!(matches!(
            error,
            Some(PodmanSandboxError::InvalidContainerPath { .. })
        ));
        Ok(())
    }

    #[test]
    fn duplicate_container_mount_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let input = temp.path().join("input");
        let output = temp.path().join("output");
        fs::create_dir_all(input.as_path())?;
        fs::create_dir_all(output.as_path())?;
        let config = PodmanSandboxConfig {
            image: "localhost/openagents/legal-benchmark:latest".to_string(),
            mounts: vec![
                PodmanSandboxMount::read_only(input.as_path(), "/workspace/inputs"),
                PodmanSandboxMount::read_write(output.as_path(), "/workspace/inputs"),
            ],
            allowed_host_roots: vec![temp.path().to_path_buf()],
            ..PodmanSandboxConfig::default()
        };

        let error = backend()
            .build_sandbox_command(&config, command_parts().as_slice())
            .err();
        assert!(matches!(
            error,
            Some(PodmanSandboxError::DuplicateContainerPath { .. })
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected_before_execution() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let safe_root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let outside_file = outside.path().join("outside.txt");
        fs::write(outside_file.as_path(), b"outside")?;
        let escaped_link = safe_root.path().join("escaped.txt");
        symlink(outside_file.as_path(), escaped_link.as_path())?;

        let config = PodmanSandboxConfig {
            image: "localhost/openagents/legal-benchmark:latest".to_string(),
            mounts: vec![PodmanSandboxMount::read_only(
                escaped_link.as_path(),
                "/workspace/inputs/escaped.txt",
            )],
            allowed_host_roots: vec![safe_root.path().to_path_buf()],
            ..PodmanSandboxConfig::default()
        };

        let error = backend()
            .build_sandbox_command(&config, command_parts().as_slice())
            .err();
        assert!(matches!(
            error,
            Some(PodmanSandboxError::HostPathOutsideAllowedRoots { .. })
        ));
        Ok(())
    }

    #[test]
    fn fixture_config_parses_with_disabled_network() -> Result<(), Box<dyn std::error::Error>> {
        let config: PodmanSandboxConfig = serde_json::from_str(include_str!(
            "../../../fixtures/legal_benchmark/podman_sandbox_config.json"
        ))?;
        assert_eq!(config.network_policy, PodmanNetworkPolicy::Disabled);
        assert_eq!(config.mounts[0].access, PodmanMountAccess::ReadOnly);
        assert_eq!(config.mounts[1].access, PodmanMountAccess::ReadWrite);
        assert_eq!(config.mounts[2].access, PodmanMountAccess::ReadWrite);
        Ok(())
    }

    #[test]
    fn prepared_command_runner_captures_output_and_exit() -> Result<(), Box<dyn std::error::Error>>
    {
        let command = PodmanSandboxCommand {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_string(), "printf ok".to_string()],
            image: "host-test".to_string(),
            network_policy: PodmanNetworkPolicy::Disabled,
            mounts: Vec::new(),
            resource_limits: PodmanSandboxResourceLimits {
                cpu_limit: None,
                memory_limit_mb: None,
                pid_limit: None,
                timeout_ms: Some(5_000),
            },
        };

        let output = run_prepared_sandbox_command("host-test.v1", &command)?;
        assert_eq!(
            output.receipt.final_state,
            PodmanSandboxCommandState::Succeeded
        );
        assert_eq!(output.receipt.exit_code, Some(0));
        assert_eq!(output.stdout, b"ok");
        Ok(())
    }

    #[test]
    fn prepared_command_runner_turns_timeout_into_receipt() -> Result<(), Box<dyn std::error::Error>>
    {
        let command = PodmanSandboxCommand {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_string(), "sleep 2".to_string()],
            image: "host-test".to_string(),
            network_policy: PodmanNetworkPolicy::Disabled,
            mounts: Vec::new(),
            resource_limits: PodmanSandboxResourceLimits {
                cpu_limit: None,
                memory_limit_mb: None,
                pid_limit: None,
                timeout_ms: Some(20),
            },
        };

        let output = run_prepared_sandbox_command("host-test.v1", &command)?;
        assert_eq!(
            output.receipt.final_state,
            PodmanSandboxCommandState::TimedOut
        );
        assert!(output.receipt.timed_out);
        Ok(())
    }
}
