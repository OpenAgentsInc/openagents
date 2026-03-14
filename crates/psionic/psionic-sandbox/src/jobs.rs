use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ProviderSandboxExecutionControls, ProviderSandboxExecutionReceipt,
    ProviderSandboxExecutionResult, ProviderSandboxExecutionState, ProviderSandboxJobRequest,
    ProviderSandboxProfile, execute_sandbox_job,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSandboxBackgroundJobState {
    Created,
    Staged,
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Killed,
    Rejected,
}

impl ProviderSandboxBackgroundJobState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::TimedOut | Self::Killed | Self::Rejected
        )
    }

    const fn from_execution_state(state: ProviderSandboxExecutionState) -> Self {
        match state {
            ProviderSandboxExecutionState::Rejected => Self::Rejected,
            ProviderSandboxExecutionState::Accepted | ProviderSandboxExecutionState::Running => {
                Self::Running
            }
            ProviderSandboxExecutionState::Succeeded => Self::Succeeded,
            ProviderSandboxExecutionState::Failed => Self::Failed,
            ProviderSandboxExecutionState::TimedOut => Self::TimedOut,
            ProviderSandboxExecutionState::Killed => Self::Killed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxBackgroundJobEvent {
    pub state: ProviderSandboxBackgroundJobState,
    pub observed_at_ms: i64,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSandboxFileTransferKind {
    Upload,
    WorkspaceDownload,
    ArtifactDownload,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxFileTransferReceipt {
    pub transfer_id: String,
    pub job_id: String,
    pub profile_digest: String,
    pub relative_path: String,
    pub transfer_kind: ProviderSandboxFileTransferKind,
    pub sha256_digest: String,
    pub size_bytes: u64,
    pub observed_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxDownloadedFile {
    pub receipt: ProviderSandboxFileTransferReceipt,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxBackgroundJobSnapshot {
    pub job_id: String,
    pub provider_id: String,
    pub compute_product_id: String,
    pub profile_id: String,
    pub profile_digest: String,
    pub workspace_root: PathBuf,
    pub state: ProviderSandboxBackgroundJobState,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub uploads: Vec<ProviderSandboxFileTransferReceipt>,
    pub downloads: Vec<ProviderSandboxFileTransferReceipt>,
    pub lifecycle_events: Vec<ProviderSandboxBackgroundJobEvent>,
    pub terminal_receipt: Option<ProviderSandboxExecutionReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderSandboxJobServiceError {
    DuplicateJob {
        job_id: String,
    },
    JobNotFound {
        job_id: String,
    },
    InvalidState {
        job_id: String,
        action: String,
        state: ProviderSandboxBackgroundJobState,
    },
    InvalidRelativePath {
        path: String,
    },
    ArtifactUnavailable {
        job_id: String,
        relative_path: String,
    },
    IoFailure {
        detail: String,
    },
}

impl std::fmt::Display for ProviderSandboxJobServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateJob { job_id } => write!(f, "sandbox job `{job_id}` already exists"),
            Self::JobNotFound { job_id } => write!(f, "sandbox job `{job_id}` was not found"),
            Self::InvalidState {
                job_id,
                action,
                state,
            } => write!(
                f,
                "sandbox job `{job_id}` cannot perform `{action}` while in state `{state:?}`"
            ),
            Self::InvalidRelativePath { path } => {
                write!(f, "sandbox relative path `{path}` is invalid")
            }
            Self::ArtifactUnavailable {
                job_id,
                relative_path,
            } => write!(
                f,
                "sandbox artifact `{relative_path}` is not available for job `{job_id}`"
            ),
            Self::IoFailure { detail } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for ProviderSandboxJobServiceError {}

#[derive(Clone, Default)]
pub struct InMemorySandboxJobService {
    inner: Arc<InMemorySandboxJobServiceInner>,
}

#[derive(Default)]
struct InMemorySandboxJobServiceInner {
    jobs: Mutex<BTreeMap<String, ManagedSandboxJob>>,
    wake: Condvar,
}

struct ManagedSandboxJob {
    profile: ProviderSandboxProfile,
    request: ProviderSandboxJobRequest,
    controls: ProviderSandboxExecutionControls,
    state: ProviderSandboxBackgroundJobState,
    created_at_ms: i64,
    updated_at_ms: i64,
    uploads: Vec<ProviderSandboxFileTransferReceipt>,
    downloads: Vec<ProviderSandboxFileTransferReceipt>,
    lifecycle_events: Vec<ProviderSandboxBackgroundJobEvent>,
    terminal_result: Option<ProviderSandboxExecutionResult>,
}

impl InMemorySandboxJobService {
    pub fn create_job(
        &self,
        profile: ProviderSandboxProfile,
        request: ProviderSandboxJobRequest,
        controls: ProviderSandboxExecutionControls,
    ) -> Result<ProviderSandboxBackgroundJobSnapshot, ProviderSandboxJobServiceError> {
        fs::create_dir_all(request.workspace_root.as_path()).map_err(|error| {
            ProviderSandboxJobServiceError::IoFailure {
                detail: format!(
                    "failed to create sandbox workspace `{}`: {error}",
                    request.workspace_root.display()
                ),
            }
        })?;

        let now = now_epoch_ms();
        let snapshot = {
            let mut jobs = self
                .inner
                .jobs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if jobs.contains_key(request.job_id.as_str()) {
                return Err(ProviderSandboxJobServiceError::DuplicateJob {
                    job_id: request.job_id,
                });
            }
            let job = ManagedSandboxJob {
                profile,
                request,
                controls,
                state: ProviderSandboxBackgroundJobState::Created,
                created_at_ms: now,
                updated_at_ms: now,
                uploads: Vec::new(),
                downloads: Vec::new(),
                lifecycle_events: vec![background_event(
                    ProviderSandboxBackgroundJobState::Created,
                    None,
                )],
                terminal_result: None,
            };
            let snapshot = job.snapshot();
            jobs.insert(snapshot.job_id.clone(), job);
            snapshot
        };
        self.inner.wake.notify_all();
        Ok(snapshot)
    }

    pub fn upload_file(
        &self,
        job_id: &str,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<ProviderSandboxFileTransferReceipt, ProviderSandboxJobServiceError> {
        let validated = validate_relative_path(relative_path)?;
        let mut jobs = self
            .inner
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let job =
            jobs.get_mut(job_id)
                .ok_or_else(|| ProviderSandboxJobServiceError::JobNotFound {
                    job_id: job_id.to_string(),
                })?;
        if !matches!(
            job.state,
            ProviderSandboxBackgroundJobState::Created | ProviderSandboxBackgroundJobState::Staged
        ) {
            return Err(ProviderSandboxJobServiceError::InvalidState {
                job_id: job_id.to_string(),
                action: "upload".to_string(),
                state: job.state,
            });
        }

        let path = job.request.workspace_root.join(validated.as_path());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ProviderSandboxJobServiceError::IoFailure {
                    detail: format!(
                        "failed to create parent directories for sandbox upload `{}`: {error}",
                        path.display()
                    ),
                }
            })?;
        }
        fs::write(path.as_path(), bytes).map_err(|error| {
            ProviderSandboxJobServiceError::IoFailure {
                detail: format!(
                    "failed to write sandbox upload `{}`: {error}",
                    path.display()
                ),
            }
        })?;
        let receipt = transfer_receipt(
            job.request.job_id.as_str(),
            job.profile.profile_digest.as_str(),
            relative_path,
            ProviderSandboxFileTransferKind::Upload,
            bytes,
        );
        job.uploads.push(receipt.clone());
        job.state = ProviderSandboxBackgroundJobState::Staged;
        job.updated_at_ms = now_epoch_ms();
        job.lifecycle_events.push(background_event(
            ProviderSandboxBackgroundJobState::Staged,
            Some(format!("uploaded `{relative_path}`")),
        ));
        self.inner.wake.notify_all();
        Ok(receipt)
    }

    pub fn start_job(
        &self,
        job_id: &str,
    ) -> Result<ProviderSandboxBackgroundJobSnapshot, ProviderSandboxJobServiceError> {
        let (profile, request, controls) = {
            let mut jobs = self
                .inner
                .jobs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let job = jobs.get_mut(job_id).ok_or_else(|| {
                ProviderSandboxJobServiceError::JobNotFound {
                    job_id: job_id.to_string(),
                }
            })?;
            if !matches!(
                job.state,
                ProviderSandboxBackgroundJobState::Created
                    | ProviderSandboxBackgroundJobState::Staged
            ) {
                return Err(ProviderSandboxJobServiceError::InvalidState {
                    job_id: job_id.to_string(),
                    action: "start".to_string(),
                    state: job.state,
                });
            }
            job.state = ProviderSandboxBackgroundJobState::Running;
            job.updated_at_ms = now_epoch_ms();
            job.lifecycle_events.push(background_event(
                ProviderSandboxBackgroundJobState::Running,
                None,
            ));
            (
                job.profile.clone(),
                job.request.clone(),
                job.controls.clone(),
            )
        };

        let background_job_id = job_id.to_string();
        let inner = Arc::clone(&self.inner);
        thread::spawn(move || {
            let result = execute_sandbox_job(&profile, &request, &controls);
            let mut jobs = inner
                .jobs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(job) = jobs.get_mut(background_job_id.as_str()) {
                let final_state = ProviderSandboxBackgroundJobState::from_execution_state(
                    result.receipt.final_state,
                );
                let detail = result.receipt.evidence.policy_detail.clone();
                job.state = final_state;
                job.updated_at_ms = now_epoch_ms();
                job.lifecycle_events
                    .push(background_event(final_state, detail));
                job.terminal_result = Some(result);
            }
            inner.wake.notify_all();
        });

        self.poll_job(job_id)
    }

    pub fn poll_job(
        &self,
        job_id: &str,
    ) -> Result<ProviderSandboxBackgroundJobSnapshot, ProviderSandboxJobServiceError> {
        let jobs = self
            .inner
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let job = jobs
            .get(job_id)
            .ok_or_else(|| ProviderSandboxJobServiceError::JobNotFound {
                job_id: job_id.to_string(),
            })?;
        Ok(job.snapshot())
    }

    pub fn list_jobs(&self) -> Vec<ProviderSandboxBackgroundJobSnapshot> {
        let jobs = self
            .inner
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        jobs.values().map(ManagedSandboxJob::snapshot).collect()
    }

    pub fn wait_for_job(
        &self,
        job_id: &str,
        timeout: Duration,
    ) -> Result<ProviderSandboxBackgroundJobSnapshot, ProviderSandboxJobServiceError> {
        let start = std::time::Instant::now();
        let mut jobs = self
            .inner
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            let job =
                jobs.get(job_id)
                    .ok_or_else(|| ProviderSandboxJobServiceError::JobNotFound {
                        job_id: job_id.to_string(),
                    })?;
            if job.state.is_terminal() {
                return Ok(job.snapshot());
            }
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Ok(job.snapshot());
            }
            let remaining = timeout.saturating_sub(elapsed);
            let (guard, _) = self
                .inner
                .wake
                .wait_timeout(jobs, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            jobs = guard;
        }
    }

    pub fn download_workspace_file(
        &self,
        job_id: &str,
        relative_path: &str,
    ) -> Result<ProviderSandboxDownloadedFile, ProviderSandboxJobServiceError> {
        self.download_file(
            job_id,
            relative_path,
            ProviderSandboxFileTransferKind::WorkspaceDownload,
        )
    }

    pub fn download_artifact(
        &self,
        job_id: &str,
        relative_path: &str,
    ) -> Result<ProviderSandboxDownloadedFile, ProviderSandboxJobServiceError> {
        let mut jobs = self
            .inner
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let job =
            jobs.get_mut(job_id)
                .ok_or_else(|| ProviderSandboxJobServiceError::JobNotFound {
                    job_id: job_id.to_string(),
                })?;
        let Some(result) = job.terminal_result.as_ref() else {
            return Err(ProviderSandboxJobServiceError::InvalidState {
                job_id: job_id.to_string(),
                action: "download_artifact".to_string(),
                state: job.state,
            });
        };
        if !result
            .receipt
            .evidence
            .artifact_digests
            .iter()
            .any(|artifact| artifact.relative_path == relative_path)
        {
            return Err(ProviderSandboxJobServiceError::ArtifactUnavailable {
                job_id: job_id.to_string(),
                relative_path: relative_path.to_string(),
            });
        }
        download_file_from_job(
            job,
            relative_path,
            ProviderSandboxFileTransferKind::ArtifactDownload,
        )
    }

    fn download_file(
        &self,
        job_id: &str,
        relative_path: &str,
        transfer_kind: ProviderSandboxFileTransferKind,
    ) -> Result<ProviderSandboxDownloadedFile, ProviderSandboxJobServiceError> {
        let mut jobs = self
            .inner
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let job =
            jobs.get_mut(job_id)
                .ok_or_else(|| ProviderSandboxJobServiceError::JobNotFound {
                    job_id: job_id.to_string(),
                })?;
        download_file_from_job(job, relative_path, transfer_kind)
    }
}

impl ManagedSandboxJob {
    fn snapshot(&self) -> ProviderSandboxBackgroundJobSnapshot {
        ProviderSandboxBackgroundJobSnapshot {
            job_id: self.request.job_id.clone(),
            provider_id: self.request.provider_id.clone(),
            compute_product_id: self.request.compute_product_id.clone(),
            profile_id: self.profile.profile_id.clone(),
            profile_digest: self.profile.profile_digest.clone(),
            workspace_root: self.request.workspace_root.clone(),
            state: self.state,
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
            uploads: self.uploads.clone(),
            downloads: self.downloads.clone(),
            lifecycle_events: self.lifecycle_events.clone(),
            terminal_receipt: self
                .terminal_result
                .as_ref()
                .map(|result| result.receipt.clone()),
        }
    }
}

fn download_file_from_job(
    job: &mut ManagedSandboxJob,
    relative_path: &str,
    transfer_kind: ProviderSandboxFileTransferKind,
) -> Result<ProviderSandboxDownloadedFile, ProviderSandboxJobServiceError> {
    let validated = validate_relative_path(relative_path)?;
    let path = job.request.workspace_root.join(validated.as_path());
    let bytes =
        fs::read(path.as_path()).map_err(|error| ProviderSandboxJobServiceError::IoFailure {
            detail: format!("failed to read sandbox file `{}`: {error}", path.display()),
        })?;
    let receipt = transfer_receipt(
        job.request.job_id.as_str(),
        job.profile.profile_digest.as_str(),
        relative_path,
        transfer_kind,
        bytes.as_slice(),
    );
    job.downloads.push(receipt.clone());
    job.updated_at_ms = now_epoch_ms();
    Ok(ProviderSandboxDownloadedFile { receipt, bytes })
}

fn validate_relative_path(path: &str) -> Result<PathBuf, ProviderSandboxJobServiceError> {
    let candidate = Path::new(path);
    if path.trim().is_empty()
        || candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(ProviderSandboxJobServiceError::InvalidRelativePath {
            path: path.to_string(),
        });
    }
    Ok(candidate.to_path_buf())
}

fn background_event(
    state: ProviderSandboxBackgroundJobState,
    detail: Option<String>,
) -> ProviderSandboxBackgroundJobEvent {
    ProviderSandboxBackgroundJobEvent {
        state,
        observed_at_ms: now_epoch_ms(),
        detail,
    }
}

fn transfer_receipt(
    job_id: &str,
    profile_digest: &str,
    relative_path: &str,
    transfer_kind: ProviderSandboxFileTransferKind,
    bytes: &[u8],
) -> ProviderSandboxFileTransferReceipt {
    let sha256_digest = sha256_prefixed(bytes);
    ProviderSandboxFileTransferReceipt {
        transfer_id: deterministic_id(
            "sandbox-transfer",
            &[
                job_id,
                profile_digest,
                relative_path,
                transfer_kind_label(transfer_kind),
                sha256_digest.as_str(),
            ],
        ),
        job_id: job_id.to_string(),
        profile_digest: profile_digest.to_string(),
        relative_path: relative_path.to_string(),
        transfer_kind,
        sha256_digest,
        size_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        observed_at_ms: now_epoch_ms(),
    }
}

const fn transfer_kind_label(kind: ProviderSandboxFileTransferKind) -> &'static str {
    match kind {
        ProviderSandboxFileTransferKind::Upload => "upload",
        ProviderSandboxFileTransferKind::WorkspaceDownload => "workspace_download",
        ProviderSandboxFileTransferKind::ArtifactDownload => "artifact_download",
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

fn now_epoch_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        InMemorySandboxJobService, ProviderSandboxBackgroundJobState,
        ProviderSandboxFileTransferKind, ProviderSandboxJobServiceError,
    };
    use crate::{
        ProviderSandboxEntrypointType, ProviderSandboxExecutionClass,
        ProviderSandboxExecutionControls, ProviderSandboxJobRequest, ProviderSandboxProfile,
        ProviderSandboxResourceRequest, ProviderSandboxRuntimeKind,
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

    fn workspace_request(workspace_root: &std::path::Path) -> ProviderSandboxJobRequest {
        ProviderSandboxJobRequest {
            job_id: "job-1".to_string(),
            provider_id: "npub1provider".to_string(),
            compute_product_id: ProviderSandboxExecutionClass::PythonExec
                .product_id()
                .to_string(),
            execution_class: ProviderSandboxExecutionClass::PythonExec,
            entrypoint_type: ProviderSandboxEntrypointType::WorkspaceFile,
            entrypoint: "scripts/job.py".to_string(),
            payload: None,
            arguments: Vec::new(),
            workspace_root: workspace_root.to_path_buf(),
            expected_outputs: vec!["outputs/result.txt".to_string()],
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
    fn background_job_lifecycle_supports_upload_poll_wait_and_artifact_download()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        let service = InMemorySandboxJobService::default();
        let profile =
            subprocess_profile(runtime.as_path(), ProviderSandboxExecutionClass::PythonExec);
        let request = workspace_request(&workspace);

        let created = service.create_job(
            profile,
            request,
            ProviderSandboxExecutionControls::default(),
        )?;
        ensure(
            created.state == ProviderSandboxBackgroundJobState::Created,
            "sandbox background job should start in created state",
        )?;

        let upload = service.upload_file(
            "job-1",
            "scripts/job.py",
            b"mkdir -p outputs\nprintf 'artifact ok' > outputs/result.txt\n",
        )?;
        ensure(
            upload.transfer_kind == ProviderSandboxFileTransferKind::Upload,
            "sandbox upload should be classified as an upload transfer",
        )?;

        let staged = service.start_job("job-1")?;
        ensure(
            staged.state == ProviderSandboxBackgroundJobState::Running,
            "sandbox background job should enter running state after start",
        )?;

        let completed = service.wait_for_job("job-1", Duration::from_secs(5))?;
        ensure(
            completed.state == ProviderSandboxBackgroundJobState::Succeeded,
            &format!(
                "sandbox background job should complete successfully, got state {:?} and receipt {:?}",
                completed.state, completed.terminal_receipt
            ),
        )?;
        ensure(
            completed.terminal_receipt.is_some(),
            "sandbox background job should retain terminal receipt",
        )?;

        let script = service.download_workspace_file("job-1", "scripts/job.py")?;
        ensure(
            script.receipt.transfer_kind == ProviderSandboxFileTransferKind::WorkspaceDownload,
            "sandbox script download should be a workspace download",
        )?;
        let artifact = service.download_artifact("job-1", "outputs/result.txt")?;
        ensure(
            artifact.receipt.transfer_kind == ProviderSandboxFileTransferKind::ArtifactDownload,
            "sandbox artifact download should be an artifact download",
        )?;
        ensure(
            artifact.bytes == b"artifact ok",
            "sandbox artifact download should return artifact bytes",
        )?;

        let snapshot = service.poll_job("job-1")?;
        ensure(
            snapshot.downloads.len() == 2,
            "sandbox background job should retain download receipts",
        )?;
        Ok(())
    }

    #[test]
    fn artifact_download_is_refused_until_job_finishes() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let workspace = temp.path().join("workspace");
        let service = InMemorySandboxJobService::default();
        let profile =
            subprocess_profile(runtime.as_path(), ProviderSandboxExecutionClass::PythonExec);
        let request = workspace_request(&workspace);

        service.create_job(
            profile,
            request,
            ProviderSandboxExecutionControls::default(),
        )?;
        service.upload_file(
            "job-1",
            "scripts/job.py",
            b"sleep 1\nmkdir -p outputs\nprintf 'artifact ok' > outputs/result.txt\n",
        )?;
        service.start_job("job-1")?;

        match service.download_artifact("job-1", "outputs/result.txt") {
            Err(ProviderSandboxJobServiceError::InvalidState { .. }) => Ok(()),
            other => Err(std::io::Error::other(format!(
                "expected invalid-state error before sandbox job finished, got {other:?}"
            ))
            .into()),
        }
    }
}
