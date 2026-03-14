use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ProviderSandboxExecutionClass, ProviderSandboxProfile};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSandboxEntrypointType {
    InlinePayload,
    WorkspaceFile,
    Command,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxEnvironmentVar {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxResourceRequest {
    pub cpu_limit: Option<u32>,
    pub memory_limit_mb: Option<u64>,
    pub disk_limit_mb: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxExecutionControls {
    pub kill_after_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSandboxExecutionState {
    Rejected,
    Accepted,
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Killed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSandboxTerminationReason {
    CleanExit,
    NonZeroExit,
    PolicyRejected,
    Timeout,
    Killed,
    RuntimeFailure,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxStateTransition {
    pub state: ProviderSandboxExecutionState,
    pub observed_at_ms: i64,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxArtifactDigest {
    pub relative_path: String,
    pub sha256_digest: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxResourceUsageSummary {
    pub wall_time_ms: u64,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub artifact_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxDeliveryEvidence {
    pub evidence_id: String,
    pub profile_id: String,
    pub profile_digest: String,
    pub runtime_environment_digest: String,
    pub job_input_digest: String,
    pub entrypoint_digest: String,
    pub start_time_ms: i64,
    pub end_time_ms: i64,
    pub exit_code: Option<i32>,
    pub termination_reason: ProviderSandboxTerminationReason,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub artifact_digests: Vec<ProviderSandboxArtifactDigest>,
    pub resource_usage: ProviderSandboxResourceUsageSummary,
    pub payout_reference: Option<String>,
    pub verification_posture: Option<String>,
    pub state_trace: Vec<ProviderSandboxStateTransition>,
    pub policy_detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxExecutionReceipt {
    pub receipt_id: String,
    pub receipt_type: String,
    pub job_id: String,
    pub provider_id: String,
    pub compute_product_id: String,
    pub final_state: ProviderSandboxExecutionState,
    pub evidence: ProviderSandboxDeliveryEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxExecutionResult {
    pub receipt: ProviderSandboxExecutionReceipt,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxJobRequest {
    pub job_id: String,
    pub provider_id: String,
    pub compute_product_id: String,
    pub execution_class: ProviderSandboxExecutionClass,
    pub entrypoint_type: ProviderSandboxEntrypointType,
    pub entrypoint: String,
    pub payload: Option<String>,
    pub arguments: Vec<String>,
    pub workspace_root: PathBuf,
    pub expected_outputs: Vec<String>,
    pub timeout_request_s: u64,
    pub network_request: String,
    pub filesystem_request: String,
    pub environment: Vec<ProviderSandboxEnvironmentVar>,
    pub resource_request: ProviderSandboxResourceRequest,
    pub payout_reference: Option<String>,
    pub verification_posture: Option<String>,
}

pub fn execute_sandbox_job(
    profile: &ProviderSandboxProfile,
    request: &ProviderSandboxJobRequest,
    controls: &ProviderSandboxExecutionControls,
) -> ProviderSandboxExecutionResult {
    if let Err(detail) = validate_request(profile, request) {
        return rejected_result(profile, request, detail);
    }

    if !profile.runtime_ready {
        return rejected_result(profile, request, "sandbox runtime is not ready".to_string());
    }

    let Some(runtime_binary_path) = profile.runtime_binary_path.as_deref() else {
        return rejected_result(
            profile,
            request,
            "sandbox runtime binary path is not available".to_string(),
        );
    };

    let workspace_root = match canonical_workspace_root(request.workspace_root.as_path()) {
        Ok(root) => root,
        Err(detail) => return rejected_result(profile, request, detail),
    };
    let expected_outputs = match resolve_expected_outputs(
        workspace_root.as_path(),
        request.expected_outputs.as_slice(),
    ) {
        Ok(outputs) => outputs,
        Err(detail) => return rejected_result(profile, request, detail),
    };

    let prepared = match prepare_job(
        profile,
        request,
        workspace_root.as_path(),
        runtime_binary_path,
    ) {
        Ok(prepared) => prepared,
        Err(detail) => return rejected_result(profile, request, detail),
    };

    let start_time_ms = now_epoch_ms();
    let start = Instant::now();
    let mut state_trace = vec![
        state_transition(ProviderSandboxExecutionState::Accepted, None),
        state_transition(ProviderSandboxExecutionState::Running, None),
    ];
    let mut command = prepared.command;
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .current_dir(workspace_root.as_path())
        .env_clear();
    for env in &request.environment {
        command.env(env.key.as_str(), env.value.as_str());
    }
    command.env("HOME", workspace_root.join(".openagents-home"));
    command.env("TMPDIR", workspace_root.join(".openagents-tmp"));

    let spawn_result = command.spawn();
    let mut child = match spawn_result {
        Ok(child) => child,
        Err(error) => {
            return runtime_failure_result(
                profile,
                request,
                prepared.job_input_digest.as_str(),
                prepared.entrypoint_digest.as_str(),
                start_time_ms,
                state_trace,
                format!("failed to spawn sandbox process: {error}"),
            );
        }
    };

    let timeout = Duration::from_secs(request.timeout_request_s);
    let kill_after = controls.kill_after_ms.map(Duration::from_millis);
    let mut exit_code = None;
    let termination_reason;
    let final_state;
    let mut policy_detail = None;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                if status.success() {
                    termination_reason = ProviderSandboxTerminationReason::CleanExit;
                    final_state = ProviderSandboxExecutionState::Succeeded;
                } else {
                    termination_reason = ProviderSandboxTerminationReason::NonZeroExit;
                    final_state = ProviderSandboxExecutionState::Failed;
                    policy_detail =
                        Some(format!("sandbox process exited with {:?}", status.code()));
                }
                break;
            }
            Ok(None) => {
                if kill_after.is_some_and(|duration| start.elapsed() >= duration) {
                    let _ = child.kill();
                    termination_reason = ProviderSandboxTerminationReason::Killed;
                    final_state = ProviderSandboxExecutionState::Killed;
                    policy_detail = Some("sandbox execution killed by control flag".to_string());
                    break;
                }
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    termination_reason = ProviderSandboxTerminationReason::Timeout;
                    final_state = ProviderSandboxExecutionState::TimedOut;
                    policy_detail = Some("sandbox execution exceeded timeout".to_string());
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return runtime_failure_result(
                    profile,
                    request,
                    prepared.job_input_digest.as_str(),
                    prepared.entrypoint_digest.as_str(),
                    start_time_ms,
                    state_trace,
                    format!("failed while polling sandbox process: {error}"),
                );
            }
        }
    }

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(error) => {
            return runtime_failure_result(
                profile,
                request,
                prepared.job_input_digest.as_str(),
                prepared.entrypoint_digest.as_str(),
                start_time_ms,
                state_trace,
                format!("failed to collect sandbox process output: {error}"),
            );
        }
    };
    let end_time_ms = now_epoch_ms();
    state_trace.push(state_transition(final_state, policy_detail.clone()));

    let artifact_digests = match collect_artifacts(expected_outputs.as_slice()) {
        Ok(artifacts) => artifacts,
        Err(detail) => {
            return runtime_failure_result(
                profile,
                request,
                prepared.job_input_digest.as_str(),
                prepared.entrypoint_digest.as_str(),
                start_time_ms,
                state_trace,
                detail,
            );
        }
    };

    let mut adjusted_state = final_state;
    let mut adjusted_reason = termination_reason;
    if adjusted_state == ProviderSandboxExecutionState::Succeeded
        && !expected_outputs.is_empty()
        && artifact_digests.len() != expected_outputs.len()
    {
        adjusted_state = ProviderSandboxExecutionState::Failed;
        adjusted_reason = ProviderSandboxTerminationReason::RuntimeFailure;
        policy_detail = Some("missing declared sandbox artifact".to_string());
    }

    build_result(
        profile,
        request,
        CompletedSandboxExecution {
            job_input_digest: prepared.job_input_digest,
            entrypoint_digest: prepared.entrypoint_digest,
            start_time_ms,
            end_time_ms,
            exit_code,
            termination_reason: adjusted_reason,
            final_state: adjusted_state,
            artifact_digests,
            stdout: output.stdout,
            stderr: output.stderr,
            state_trace,
            policy_detail,
        },
    )
}

#[derive(Debug)]
struct PreparedSandboxCommand {
    command: Command,
    job_input_digest: String,
    entrypoint_digest: String,
}

fn validate_request(
    profile: &ProviderSandboxProfile,
    request: &ProviderSandboxJobRequest,
) -> Result<(), String> {
    if profile.execution_class != request.execution_class {
        return Err("requested execution class does not match sandbox profile".to_string());
    }
    if request.compute_product_id != request.execution_class.product_id() {
        return Err("compute product id does not match execution class".to_string());
    }
    if request.timeout_request_s == 0 {
        return Err("sandbox timeout_request_s must be greater than zero".to_string());
    }
    if request.timeout_request_s > profile.timeout_limit_s {
        return Err("sandbox timeout request exceeds declared profile limit".to_string());
    }
    if request
        .resource_request
        .cpu_limit
        .is_some_and(|cpu_limit| cpu_limit > profile.cpu_limit)
    {
        return Err("sandbox cpu limit exceeds declared profile".to_string());
    }
    if request
        .resource_request
        .memory_limit_mb
        .is_some_and(|memory_limit_mb| memory_limit_mb > profile.memory_limit_mb)
    {
        return Err("sandbox memory limit exceeds declared profile".to_string());
    }
    if request
        .resource_request
        .disk_limit_mb
        .is_some_and(|disk_limit_mb| disk_limit_mb > profile.disk_limit_mb)
    {
        return Err("sandbox disk limit exceeds declared profile".to_string());
    }
    if request.network_request.trim() != profile.network_mode.trim() {
        return Err("sandbox network request does not match declared profile".to_string());
    }
    if request.filesystem_request.trim() != profile.filesystem_mode.trim() {
        return Err("sandbox filesystem request does not match declared profile".to_string());
    }
    if profile.secrets_mode.trim() == "none" && !request.environment.is_empty() {
        return Err("sandbox profile forbids injected secrets or environment".to_string());
    }
    if request
        .expected_outputs
        .iter()
        .any(|path| path.trim().is_empty() || Path::new(path).is_absolute() || path.contains(".."))
    {
        return Err("sandbox expected outputs must be relative workspace paths".to_string());
    }
    Ok(())
}

fn canonical_workspace_root(workspace_root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(workspace_root)
        .map_err(|error| format!("failed to create sandbox workspace root: {error}"))?;
    workspace_root
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize sandbox workspace root: {error}"))
}

fn resolve_expected_outputs(
    workspace_root: &Path,
    expected_outputs: &[String],
) -> Result<Vec<(String, PathBuf)>, String> {
    let mut resolved = Vec::new();
    for relative_path in expected_outputs {
        let path = workspace_root.join(relative_path);
        let parent = path.parent().unwrap_or(workspace_root);
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create sandbox artifact parent: {error}"))?;
        let canonical_parent = parent
            .canonicalize()
            .map_err(|error| format!("failed to canonicalize sandbox artifact parent: {error}"))?;
        if !canonical_parent.starts_with(workspace_root) {
            return Err("sandbox expected output escapes workspace root".to_string());
        }
        resolved.push((relative_path.clone(), path));
    }
    Ok(resolved)
}

fn prepare_job(
    profile: &ProviderSandboxProfile,
    request: &ProviderSandboxJobRequest,
    workspace_root: &Path,
    runtime_binary_path: &str,
) -> Result<PreparedSandboxCommand, String> {
    if uses_container_adapter(profile) {
        prepare_container_command(profile, request, workspace_root, runtime_binary_path)
    } else {
        prepare_local_subprocess_command(profile, request, workspace_root, runtime_binary_path)
    }
}

fn uses_container_adapter(profile: &ProviderSandboxProfile) -> bool {
    matches!(
        profile.execution_class,
        ProviderSandboxExecutionClass::ContainerExec
    ) || profile.container_image.is_some()
        || matches!(
            profile.sandbox_engine.as_str(),
            "docker" | "podman" | "container_runtime"
        )
}

fn prepare_local_subprocess_command(
    profile: &ProviderSandboxProfile,
    request: &ProviderSandboxJobRequest,
    workspace_root: &Path,
    runtime_binary_path: &str,
) -> Result<PreparedSandboxCommand, String> {
    if request.network_request != "host_inherit" {
        return Err(
            "local sandbox subprocess adapter only supports host_inherit networking".to_string(),
        );
    }
    if request.filesystem_request != "host_inherit" {
        return Err(
            "local sandbox subprocess adapter only supports host_inherit filesystem access"
                .to_string(),
        );
    }

    let entrypoint = prepare_entrypoint(profile, request, workspace_root)?;
    let mut command = Command::new(runtime_binary_path);
    match request.execution_class {
        ProviderSandboxExecutionClass::PythonExec
        | ProviderSandboxExecutionClass::NodeExec
        | ProviderSandboxExecutionClass::PosixExec => {
            command.arg(entrypoint.execution_target.as_path());
        }
        ProviderSandboxExecutionClass::ContainerExec => {
            return Err("container execution must use the container adapter".to_string());
        }
    }
    for argument in &request.arguments {
        command.arg(argument);
    }

    Ok(PreparedSandboxCommand {
        command,
        job_input_digest: job_input_digest(request),
        entrypoint_digest: entrypoint.entrypoint_digest,
    })
}

fn prepare_container_command(
    profile: &ProviderSandboxProfile,
    request: &ProviderSandboxJobRequest,
    workspace_root: &Path,
    runtime_binary_path: &str,
) -> Result<PreparedSandboxCommand, String> {
    if request.filesystem_request != "workspace_only" {
        return Err(
            "container sandbox adapter requires workspace_only filesystem access".to_string(),
        );
    }
    let Some(container_image) = profile.container_image.as_deref() else {
        return Err("container sandbox profile is missing container_image".to_string());
    };
    let entrypoint = prepare_entrypoint(profile, request, workspace_root)?;
    let mount_arg = format!("type=bind,src={},dst=/workspace", workspace_root.display());

    let mut command = Command::new(runtime_binary_path);
    command
        .arg("run")
        .arg("--rm")
        .arg("--workdir")
        .arg("/workspace")
        .arg("--mount")
        .arg(mount_arg);
    match request.network_request.as_str() {
        "none" => {
            command.arg("--network").arg("none");
        }
        "host_inherit" => {}
        _ => {
            return Err(
                "container sandbox adapter only supports none or host_inherit networking"
                    .to_string(),
            );
        }
    }
    if profile.cpu_limit > 0 {
        command.arg("--cpus").arg(profile.cpu_limit.to_string());
    }
    if profile.memory_limit_mb > 0 {
        command
            .arg("--memory")
            .arg(format!("{}m", profile.memory_limit_mb));
    }
    command.arg(container_image);
    command.arg(format!(
        "/workspace/{}",
        entrypoint
            .relative_execution_target
            .as_deref()
            .unwrap_or_default()
    ));
    for argument in &request.arguments {
        command.arg(argument);
    }

    Ok(PreparedSandboxCommand {
        command,
        job_input_digest: job_input_digest(request),
        entrypoint_digest: entrypoint.entrypoint_digest,
    })
}

struct PreparedEntrypoint {
    execution_target: PathBuf,
    relative_execution_target: Option<String>,
    entrypoint_digest: String,
}

fn prepare_entrypoint(
    profile: &ProviderSandboxProfile,
    request: &ProviderSandboxJobRequest,
    workspace_root: &Path,
) -> Result<PreparedEntrypoint, String> {
    match request.entrypoint_type {
        ProviderSandboxEntrypointType::InlinePayload => {
            let Some(payload) = request.payload.as_deref() else {
                return Err("sandbox inline payload entrypoint requires payload".to_string());
            };
            let sandbox_dir = workspace_root.join(".openagents-sandbox");
            fs::create_dir_all(&sandbox_dir)
                .map_err(|error| format!("failed to create sandbox staging dir: {error}"))?;
            let relative_name = format!(
                "{}{}",
                sanitize_identifier(request.job_id.as_str()),
                script_extension(profile.execution_class)
            );
            let entrypoint_path = sandbox_dir.join(relative_name.as_str());
            fs::write(entrypoint_path.as_path(), payload)
                .map_err(|error| format!("failed to stage sandbox payload: {error}"))?;
            Ok(PreparedEntrypoint {
                execution_target: entrypoint_path,
                relative_execution_target: Some(format!(".openagents-sandbox/{relative_name}")),
                entrypoint_digest: sha256_prefixed(payload.as_bytes()),
            })
        }
        ProviderSandboxEntrypointType::WorkspaceFile => {
            if request.entrypoint.trim().is_empty() {
                return Err("sandbox workspace file entrypoint cannot be empty".to_string());
            }
            let entrypoint_path = workspace_root.join(request.entrypoint.as_str());
            let canonical_entrypoint = entrypoint_path.canonicalize().map_err(|error| {
                format!("failed to resolve sandbox workspace entrypoint: {error}")
            })?;
            if !canonical_entrypoint.starts_with(workspace_root) {
                return Err("sandbox entrypoint escapes workspace root".to_string());
            }
            let bytes = fs::read(canonical_entrypoint.as_path())
                .map_err(|error| format!("failed to read sandbox workspace entrypoint: {error}"))?;
            Ok(PreparedEntrypoint {
                execution_target: canonical_entrypoint,
                relative_execution_target: Some(request.entrypoint.clone()),
                entrypoint_digest: sha256_prefixed(bytes.as_slice()),
            })
        }
        ProviderSandboxEntrypointType::Command => {
            if uses_command_string(request.execution_class) {
                Ok(PreparedEntrypoint {
                    execution_target: PathBuf::from(request.entrypoint.as_str()),
                    relative_execution_target: Some(request.entrypoint.clone()),
                    entrypoint_digest: sha256_prefixed(request.entrypoint.as_bytes()),
                })
            } else {
                Err(
                    "sandbox command entrypoints are only supported for container execution"
                        .to_string(),
                )
            }
        }
    }
}

fn uses_command_string(execution_class: ProviderSandboxExecutionClass) -> bool {
    matches!(
        execution_class,
        ProviderSandboxExecutionClass::ContainerExec
    )
}

fn script_extension(execution_class: ProviderSandboxExecutionClass) -> &'static str {
    match execution_class {
        ProviderSandboxExecutionClass::PythonExec => ".py",
        ProviderSandboxExecutionClass::NodeExec => ".js",
        ProviderSandboxExecutionClass::PosixExec => ".sh",
        ProviderSandboxExecutionClass::ContainerExec => ".sh",
    }
}

fn collect_artifacts(
    expected_outputs: &[(String, PathBuf)],
) -> Result<Vec<ProviderSandboxArtifactDigest>, String> {
    let mut artifacts = Vec::new();
    for (relative_path, path) in expected_outputs {
        if !path.exists() {
            continue;
        }
        let bytes = fs::read(path).map_err(|error| {
            format!(
                "failed to read sandbox artifact {}: {error}",
                path.display()
            )
        })?;
        artifacts.push(ProviderSandboxArtifactDigest {
            relative_path: relative_path.clone(),
            sha256_digest: sha256_prefixed(bytes.as_slice()),
            size_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(artifacts)
}

struct CompletedSandboxExecution {
    job_input_digest: String,
    entrypoint_digest: String,
    start_time_ms: i64,
    end_time_ms: i64,
    exit_code: Option<i32>,
    termination_reason: ProviderSandboxTerminationReason,
    final_state: ProviderSandboxExecutionState,
    artifact_digests: Vec<ProviderSandboxArtifactDigest>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    state_trace: Vec<ProviderSandboxStateTransition>,
    policy_detail: Option<String>,
}

fn build_result(
    profile: &ProviderSandboxProfile,
    request: &ProviderSandboxJobRequest,
    completed: CompletedSandboxExecution,
) -> ProviderSandboxExecutionResult {
    let resource_usage = ProviderSandboxResourceUsageSummary {
        wall_time_ms: completed
            .end_time_ms
            .saturating_sub(completed.start_time_ms)
            .unsigned_abs(),
        stdout_bytes: u64::try_from(completed.stdout.len()).unwrap_or(u64::MAX),
        stderr_bytes: u64::try_from(completed.stderr.len()).unwrap_or(u64::MAX),
        artifact_bytes: completed
            .artifact_digests
            .iter()
            .map(|artifact| artifact.size_bytes)
            .sum(),
    };
    let evidence = ProviderSandboxDeliveryEvidence {
        evidence_id: deterministic_id(
            "sandbox-evidence",
            &[
                request.job_id.as_str(),
                profile.profile_digest.as_str(),
                completed.job_input_digest.as_str(),
                completed.entrypoint_digest.as_str(),
                final_state_label(completed.final_state),
            ],
        ),
        profile_id: profile.profile_id.clone(),
        profile_digest: profile.profile_digest.clone(),
        runtime_environment_digest: runtime_environment_digest(profile),
        job_input_digest: completed.job_input_digest.clone(),
        entrypoint_digest: completed.entrypoint_digest.clone(),
        start_time_ms: completed.start_time_ms,
        end_time_ms: completed.end_time_ms,
        exit_code: completed.exit_code,
        termination_reason: completed.termination_reason,
        stdout_digest: sha256_prefixed(completed.stdout.as_slice()),
        stderr_digest: sha256_prefixed(completed.stderr.as_slice()),
        artifact_digests: completed.artifact_digests,
        resource_usage,
        payout_reference: request.payout_reference.clone(),
        verification_posture: request.verification_posture.clone(),
        state_trace: completed.state_trace,
        policy_detail: completed.policy_detail,
    };
    let receipt = ProviderSandboxExecutionReceipt {
        receipt_id: deterministic_id(
            "sandbox-receipt",
            &[
                request.provider_id.as_str(),
                request.job_id.as_str(),
                evidence.evidence_id.as_str(),
            ],
        ),
        receipt_type: receipt_type_for_state(completed.final_state).to_string(),
        job_id: request.job_id.clone(),
        provider_id: request.provider_id.clone(),
        compute_product_id: request.compute_product_id.clone(),
        final_state: completed.final_state,
        evidence,
    };
    ProviderSandboxExecutionResult {
        receipt,
        stdout: completed.stdout,
        stderr: completed.stderr,
    }
}

fn rejected_result(
    profile: &ProviderSandboxProfile,
    request: &ProviderSandboxJobRequest,
    detail: String,
) -> ProviderSandboxExecutionResult {
    let now = now_epoch_ms();
    build_result(
        profile,
        request,
        CompletedSandboxExecution {
            job_input_digest: job_input_digest(request),
            entrypoint_digest: sha256_prefixed(request.entrypoint.as_bytes()),
            start_time_ms: now,
            end_time_ms: now,
            exit_code: None,
            termination_reason: ProviderSandboxTerminationReason::PolicyRejected,
            final_state: ProviderSandboxExecutionState::Rejected,
            artifact_digests: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            state_trace: vec![state_transition(
                ProviderSandboxExecutionState::Rejected,
                Some(detail.clone()),
            )],
            policy_detail: Some(detail),
        },
    )
}

fn runtime_failure_result(
    profile: &ProviderSandboxProfile,
    request: &ProviderSandboxJobRequest,
    job_input_digest: &str,
    entrypoint_digest: &str,
    start_time_ms: i64,
    mut state_trace: Vec<ProviderSandboxStateTransition>,
    detail: String,
) -> ProviderSandboxExecutionResult {
    let end_time_ms = now_epoch_ms();
    state_trace.push(state_transition(
        ProviderSandboxExecutionState::Failed,
        Some(detail.clone()),
    ));
    build_result(
        profile,
        request,
        CompletedSandboxExecution {
            job_input_digest: job_input_digest.to_string(),
            entrypoint_digest: entrypoint_digest.to_string(),
            start_time_ms,
            end_time_ms,
            exit_code: None,
            termination_reason: ProviderSandboxTerminationReason::RuntimeFailure,
            final_state: ProviderSandboxExecutionState::Failed,
            artifact_digests: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            state_trace,
            policy_detail: Some(detail),
        },
    )
}

fn state_transition(
    state: ProviderSandboxExecutionState,
    detail: Option<String>,
) -> ProviderSandboxStateTransition {
    ProviderSandboxStateTransition {
        state,
        observed_at_ms: now_epoch_ms(),
        detail,
    }
}

fn runtime_environment_digest(profile: &ProviderSandboxProfile) -> String {
    let mut payload = format!(
        "{}:{}:{}:{}:{}",
        profile.sandbox_engine,
        profile.runtime_family,
        profile.runtime_version,
        profile.profile_digest,
        profile.runtime_binary_path.as_deref().unwrap_or("unknown")
    );
    if let Some(container_image) = profile.container_image.as_deref() {
        payload.push(':');
        payload.push_str(container_image);
    }
    if let Some(runtime_image_digest) = profile.runtime_image_digest.as_deref() {
        payload.push(':');
        payload.push_str(runtime_image_digest);
    }
    sha256_prefixed(payload.as_bytes())
}

fn job_input_digest(request: &ProviderSandboxJobRequest) -> String {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(request.job_id.as_bytes());
    encoded.extend_from_slice(request.compute_product_id.as_bytes());
    encoded.extend_from_slice(request.entrypoint.as_bytes());
    if let Some(payload) = request.payload.as_deref() {
        encoded.extend_from_slice(payload.as_bytes());
    }
    for argument in &request.arguments {
        encoded.extend_from_slice(argument.as_bytes());
    }
    for output in &request.expected_outputs {
        encoded.extend_from_slice(output.as_bytes());
    }
    sha256_prefixed(encoded.as_slice())
}

fn receipt_type_for_state(state: ProviderSandboxExecutionState) -> &'static str {
    match state {
        ProviderSandboxExecutionState::Rejected => "compute.sandbox.execution.rejected.v1",
        ProviderSandboxExecutionState::Accepted | ProviderSandboxExecutionState::Running => {
            "compute.sandbox.execution.running.v1"
        }
        ProviderSandboxExecutionState::Succeeded => "compute.sandbox.execution.succeeded.v1",
        ProviderSandboxExecutionState::Failed => "compute.sandbox.execution.failed.v1",
        ProviderSandboxExecutionState::TimedOut => "compute.sandbox.execution.timed_out.v1",
        ProviderSandboxExecutionState::Killed => "compute.sandbox.execution.killed.v1",
    }
}

fn final_state_label(state: ProviderSandboxExecutionState) -> &'static str {
    match state {
        ProviderSandboxExecutionState::Rejected => "rejected",
        ProviderSandboxExecutionState::Accepted => "accepted",
        ProviderSandboxExecutionState::Running => "running",
        ProviderSandboxExecutionState::Succeeded => "succeeded",
        ProviderSandboxExecutionState::Failed => "failed",
        ProviderSandboxExecutionState::TimedOut => "timed_out",
        ProviderSandboxExecutionState::Killed => "killed",
    }
}

fn deterministic_id(prefix: &str, parts: &[&str]) -> String {
    let joined = parts.join(":");
    let digest = sha256_prefixed(joined.as_bytes());
    format!("{prefix}-{}", digest.trim_start_matches("sha256:"))
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn now_epoch_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderSandboxEntrypointType, ProviderSandboxExecutionControls,
        ProviderSandboxExecutionState, ProviderSandboxJobRequest, ProviderSandboxResourceRequest,
        ProviderSandboxTerminationReason, execute_sandbox_job,
    };
    use crate::{
        ProviderSandboxExecutionClass, ProviderSandboxProfile, ProviderSandboxRuntimeKind,
    };

    fn ensure(condition: bool, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if condition {
            Ok(())
        } else {
            Err(std::io::Error::other(message.to_string()).into())
        }
    }

    fn fake_binary(
        dir: &std::path::Path,
        name: &str,
        body: &str,
    ) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let path = dir.join(name);
        std::fs::write(&path, body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&path, permissions)?;
        }
        Ok(path)
    }

    fn subprocess_profile(
        runtime_binary_path: &std::path::Path,
        execution_class: ProviderSandboxExecutionClass,
    ) -> ProviderSandboxProfile {
        ProviderSandboxProfile {
            profile_id: format!("{:?}-profile", execution_class).to_lowercase(),
            profile_digest: "sha256:profile".to_string(),
            execution_class,
            runtime_family: match execution_class {
                ProviderSandboxExecutionClass::PythonExec => "python3",
                ProviderSandboxExecutionClass::NodeExec => "node",
                ProviderSandboxExecutionClass::PosixExec => "bash",
                ProviderSandboxExecutionClass::ContainerExec => "docker",
            }
            .to_string(),
            runtime_version: "test-runtime".to_string(),
            sandbox_engine: "local_subprocess".to_string(),
            os_family: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_limit: 2,
            memory_limit_mb: 2048,
            disk_limit_mb: 4096,
            timeout_limit_s: 5,
            network_mode: "host_inherit".to_string(),
            filesystem_mode: "host_inherit".to_string(),
            workspace_mode: "ephemeral".to_string(),
            artifact_output_mode: "declared_paths_only".to_string(),
            secrets_mode: "none".to_string(),
            allowed_binaries: vec!["runtime".to_string()],
            toolchain_inventory: vec!["runtime".to_string()],
            container_image: None,
            runtime_image_digest: None,
            accelerator_policy: None,
            runtime_kind: ProviderSandboxRuntimeKind::Python,
            runtime_ready: true,
            runtime_binary_path: Some(runtime_binary_path.display().to_string()),
            capability_summary: "test".to_string(),
        }
    }

    fn container_profile(runtime_binary_path: &std::path::Path) -> ProviderSandboxProfile {
        ProviderSandboxProfile {
            profile_id: "container-profile".to_string(),
            profile_digest: "sha256:container-profile".to_string(),
            execution_class: ProviderSandboxExecutionClass::ContainerExec,
            runtime_family: "docker".to_string(),
            runtime_version: "test-runtime".to_string(),
            sandbox_engine: "docker".to_string(),
            os_family: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_limit: 2,
            memory_limit_mb: 2048,
            disk_limit_mb: 4096,
            timeout_limit_s: 5,
            network_mode: "none".to_string(),
            filesystem_mode: "workspace_only".to_string(),
            workspace_mode: "ephemeral".to_string(),
            artifact_output_mode: "declared_paths_only".to_string(),
            secrets_mode: "none".to_string(),
            allowed_binaries: vec!["docker".to_string()],
            toolchain_inventory: vec!["docker".to_string()],
            container_image: Some("alpine:latest".to_string()),
            runtime_image_digest: None,
            accelerator_policy: None,
            runtime_kind: ProviderSandboxRuntimeKind::Container,
            runtime_ready: true,
            runtime_binary_path: Some(runtime_binary_path.display().to_string()),
            capability_summary: "test".to_string(),
        }
    }

    fn request(
        workspace_root: &std::path::Path,
        execution_class: ProviderSandboxExecutionClass,
    ) -> ProviderSandboxJobRequest {
        ProviderSandboxJobRequest {
            job_id: "job-1".to_string(),
            provider_id: "npub1provider".to_string(),
            compute_product_id: execution_class.product_id().to_string(),
            execution_class,
            entrypoint_type: ProviderSandboxEntrypointType::InlinePayload,
            entrypoint: "inline".to_string(),
            payload: Some("printf 'hello' > result.txt\n".to_string()),
            arguments: Vec::new(),
            workspace_root: workspace_root.to_path_buf(),
            expected_outputs: vec!["result.txt".to_string()],
            timeout_request_s: 2,
            network_request: "host_inherit".to_string(),
            filesystem_request: "host_inherit".to_string(),
            environment: Vec::new(),
            resource_request: ProviderSandboxResourceRequest::default(),
            payout_reference: Some("payment-1".to_string()),
            verification_posture: Some("hash_only".to_string()),
        }
    }

    #[test]
    fn local_subprocess_success_emits_receipt_and_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let profile =
            subprocess_profile(runtime.as_path(), ProviderSandboxExecutionClass::PythonExec);
        let request = request(&workspace, ProviderSandboxExecutionClass::PythonExec);

        let result = execute_sandbox_job(
            &profile,
            &request,
            &ProviderSandboxExecutionControls::default(),
        );

        ensure(
            result.receipt.final_state == ProviderSandboxExecutionState::Succeeded,
            "sandbox subprocess job should succeed",
        )?;
        ensure(
            result.receipt.evidence.termination_reason
                == ProviderSandboxTerminationReason::CleanExit,
            "sandbox subprocess should report clean exit",
        )?;
        ensure(
            result.receipt.evidence.artifact_digests.len() == 1,
            "sandbox subprocess should emit one artifact digest",
        )?;
        ensure(
            result.receipt.evidence.payout_reference.as_deref() == Some("payment-1"),
            "sandbox subprocess should retain payout reference",
        )?;
        Ok(())
    }

    #[test]
    fn node_runner_executes_inline_payload_and_emits_artifact()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let profile = subprocess_profile(runtime.as_path(), ProviderSandboxExecutionClass::NodeExec);
        let request = request(&workspace, ProviderSandboxExecutionClass::NodeExec);

        let result = execute_sandbox_job(
            &profile,
            &request,
            &ProviderSandboxExecutionControls::default(),
        );

        ensure(
            result.receipt.final_state == ProviderSandboxExecutionState::Succeeded,
            "sandbox node job should succeed",
        )?;
        ensure(
            result.receipt.evidence.artifact_digests.len() == 1,
            "sandbox node job should emit one artifact digest",
        )?;
        ensure(
            result.receipt.evidence.profile_id.contains("nodeexec"),
            "sandbox node job should retain node profile identity",
        )?;
        Ok(())
    }

    #[test]
    fn container_adapter_passes_workspace_and_network_policy()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "docker",
            "#!/bin/sh\nnetwork_ok=0\nworkspace=\"\"\nprev=\"\"\nfor arg in \"$@\"; do\n  if [ \"$prev\" = \"network\" ] && [ \"$arg\" = \"none\" ]; then network_ok=1; fi\n  if [ \"$arg\" = \"--network\" ]; then prev=\"network\"; else prev=\"\"; fi\n  case \"$arg\" in\n    type=bind,src=*,dst=/workspace)\n      workspace=\"${arg#type=bind,src=}\"\n      workspace=\"${workspace%,dst=/workspace}\"\n      ;;\n  esac\ndone\n[ \"$network_ok\" -eq 1 ] || exit 19\n[ -n \"$workspace\" ] || exit 20\nprintf 'container ok' > \"$workspace/result.txt\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let profile = container_profile(runtime.as_path());
        let mut request = request(&workspace, ProviderSandboxExecutionClass::ContainerExec);
        request.network_request = "none".to_string();
        request.filesystem_request = "workspace_only".to_string();

        let result = execute_sandbox_job(
            &profile,
            &request,
            &ProviderSandboxExecutionControls::default(),
        );

        ensure(
            result.receipt.final_state == ProviderSandboxExecutionState::Succeeded,
            "sandbox container job should succeed",
        )?;
        ensure(
            result.receipt.evidence.artifact_digests.len() == 1,
            "sandbox container job should emit one artifact digest",
        )?;
        Ok(())
    }

    #[test]
    fn mismatched_execution_class_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let profile =
            subprocess_profile(runtime.as_path(), ProviderSandboxExecutionClass::PythonExec);
        let request = request(&workspace, ProviderSandboxExecutionClass::NodeExec);

        let result = execute_sandbox_job(
            &profile,
            &request,
            &ProviderSandboxExecutionControls::default(),
        );

        ensure(
            result.receipt.final_state == ProviderSandboxExecutionState::Rejected,
            "sandbox class mismatch should be rejected",
        )?;
        ensure(
            result.receipt.evidence.termination_reason
                == ProviderSandboxTerminationReason::PolicyRejected,
            "sandbox class mismatch should retain policy rejection",
        )?;
        ensure(
            result
                .receipt
                .evidence
                .policy_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("execution class")),
            "sandbox class mismatch should preserve an explicit refusal detail",
        )?;
        Ok(())
    }

    #[test]
    fn policy_rejection_is_receipted() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let profile =
            subprocess_profile(runtime.as_path(), ProviderSandboxExecutionClass::PythonExec);
        let mut request = request(&workspace, ProviderSandboxExecutionClass::PythonExec);
        request
            .environment
            .push(super::ProviderSandboxEnvironmentVar {
                key: "SECRET_TOKEN".to_string(),
                value: "shh".to_string(),
            });

        let result = execute_sandbox_job(
            &profile,
            &request,
            &ProviderSandboxExecutionControls::default(),
        );

        ensure(
            result.receipt.final_state == ProviderSandboxExecutionState::Rejected,
            "sandbox request with secrets should be rejected",
        )?;
        ensure(
            result.receipt.evidence.termination_reason
                == ProviderSandboxTerminationReason::PolicyRejected,
            "sandbox rejection should be receipted as policy rejected",
        )?;
        Ok(())
    }

    #[test]
    fn timeout_is_receipted() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let profile =
            subprocess_profile(runtime.as_path(), ProviderSandboxExecutionClass::PosixExec);
        let mut request = request(&workspace, ProviderSandboxExecutionClass::PosixExec);
        request.payload = Some("sleep 3\n".to_string());
        request.expected_outputs.clear();
        request.timeout_request_s = 1;

        let result = execute_sandbox_job(
            &profile,
            &request,
            &ProviderSandboxExecutionControls::default(),
        );

        ensure(
            result.receipt.final_state == ProviderSandboxExecutionState::TimedOut,
            "sandbox timeout should be explicit",
        )?;
        ensure(
            result.receipt.evidence.termination_reason == ProviderSandboxTerminationReason::Timeout,
            "sandbox timeout should carry timeout termination reason",
        )?;
        Ok(())
    }

    #[test]
    fn kill_is_receipted() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let profile =
            subprocess_profile(runtime.as_path(), ProviderSandboxExecutionClass::PosixExec);
        let mut request = request(&workspace, ProviderSandboxExecutionClass::PosixExec);
        request.payload = Some("sleep 3\n".to_string());
        request.expected_outputs.clear();

        let result = execute_sandbox_job(
            &profile,
            &request,
            &ProviderSandboxExecutionControls {
                kill_after_ms: Some(100),
            },
        );

        ensure(
            result.receipt.final_state == ProviderSandboxExecutionState::Killed,
            "sandbox kill should be explicit",
        )?;
        ensure(
            result.receipt.evidence.termination_reason == ProviderSandboxTerminationReason::Killed,
            "sandbox kill should carry killed termination reason",
        )?;
        Ok(())
    }

    #[test]
    fn failure_is_receipted() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let profile =
            subprocess_profile(runtime.as_path(), ProviderSandboxExecutionClass::PosixExec);
        let mut request = request(&workspace, ProviderSandboxExecutionClass::PosixExec);
        request.payload = Some("exit 7\n".to_string());
        request.expected_outputs.clear();

        let result = execute_sandbox_job(
            &profile,
            &request,
            &ProviderSandboxExecutionControls::default(),
        );

        ensure(
            result.receipt.final_state == ProviderSandboxExecutionState::Failed,
            "sandbox non-zero exit should be a failure",
        )?;
        ensure(
            result.receipt.evidence.termination_reason
                == ProviderSandboxTerminationReason::NonZeroExit,
            "sandbox failure should retain non-zero exit termination reason",
        )?;
        ensure(
            result.receipt.evidence.exit_code == Some(7),
            "sandbox failure should record exit code",
        )?;
        Ok(())
    }
}
