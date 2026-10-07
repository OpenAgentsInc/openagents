use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ProviderSandboxEntrypointType, ProviderSandboxEnvironmentVar, ProviderSandboxExecutionControls,
    ProviderSandboxExecutionReceipt, ProviderSandboxExecutionResult, ProviderSandboxJobRequest,
    ProviderSandboxProfile, ProviderSandboxResourceRequest, execute_sandbox_job,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPoolSessionState {
    Ready,
    Acquired,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStageArtifactKind {
    CommandInput,
    ImageFrame,
    ContextArtifact,
    OutputArtifact,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxPoolSpec {
    pub pool_id: String,
    pub workspace_root: PathBuf,
    pub target_ready: u32,
    pub max_sessions: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxPoolSessionSnapshot {
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub state: SandboxPoolSessionState,
    pub reuse_count: u32,
    pub iteration_count: u32,
    pub last_ready_at_ms: i64,
    pub active_acquisition_id: Option<String>,
    pub staged_receipt_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxPoolSnapshot {
    pub pool_id: String,
    pub profile_id: String,
    pub target_ready: u32,
    pub max_sessions: u32,
    pub ready_sessions: u32,
    pub acquired_sessions: u32,
    pub sessions: Vec<SandboxPoolSessionSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxPoolWarmReceipt {
    pub pool_id: String,
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub created_new_session: bool,
    pub warm_latency_ms: u64,
    pub ready_session_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxPoolAcquisitionReceipt {
    pub pool_id: String,
    pub session_id: String,
    pub acquisition_id: String,
    pub reused_ready_session: bool,
    pub acquisition_latency_ms: u64,
    pub ready_sessions_before: u32,
    pub ready_sessions_after: u32,
    pub reuse_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxStagedArtifactReceipt {
    pub stage_id: String,
    pub pool_id: String,
    pub session_id: String,
    pub acquisition_id: String,
    pub stage_kind: SandboxStageArtifactKind,
    pub relative_path: String,
    pub sha256_digest: String,
    pub size_bytes: u64,
    pub observed_at_ms: i64,
    pub stream_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxLoopIterationRequest {
    pub entrypoint_type: ProviderSandboxEntrypointType,
    pub entrypoint: String,
    pub payload: Option<String>,
    pub arguments: Vec<String>,
    pub expected_outputs: Vec<String>,
    pub timeout_request_s: u64,
    pub network_request: String,
    pub filesystem_request: String,
    pub environment: Vec<ProviderSandboxEnvironmentVar>,
    pub resource_request: ProviderSandboxResourceRequest,
    pub payout_reference: Option<String>,
    pub verification_posture: Option<String>,
}

impl SandboxLoopIterationRequest {
    #[must_use]
    pub fn new(
        entrypoint_type: ProviderSandboxEntrypointType,
        entrypoint: impl Into<String>,
    ) -> Self {
        Self {
            entrypoint_type,
            entrypoint: entrypoint.into(),
            payload: None,
            arguments: Vec::new(),
            expected_outputs: Vec::new(),
            timeout_request_s: 5,
            network_request: String::from("host_inherit"),
            filesystem_request: String::from("host_inherit"),
            environment: Vec::new(),
            resource_request: ProviderSandboxResourceRequest::default(),
            payout_reference: None,
            verification_posture: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxLoopIterationReceipt {
    pub pool_id: String,
    pub session_id: String,
    pub acquisition_id: String,
    pub iteration_index: u32,
    pub reused_workspace: bool,
    pub staged_inputs: Vec<SandboxStagedArtifactReceipt>,
    pub output_artifacts: Vec<SandboxStagedArtifactReceipt>,
    pub execution_result: ProviderSandboxExecutionResult,
    pub iteration_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SandboxPoolError {
    DuplicatePool {
        pool_id: String,
    },
    UnknownPool {
        pool_id: String,
    },
    UnknownSession {
        pool_id: String,
        session_id: String,
    },
    InvalidState {
        pool_id: String,
        session_id: String,
        action: String,
        state: SandboxPoolSessionState,
    },
    AcquisitionMismatch {
        pool_id: String,
        session_id: String,
        expected_acquisition_id: String,
        actual_acquisition_id: String,
    },
    PoolExhausted {
        pool_id: String,
        max_sessions: u32,
    },
    InvalidRelativePath {
        path: String,
    },
    IoFailure {
        detail: String,
    },
}

impl std::fmt::Display for SandboxPoolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicatePool { pool_id } => {
                write!(formatter, "sandbox pool `{pool_id}` already exists")
            }
            Self::UnknownPool { pool_id } => {
                write!(formatter, "sandbox pool `{pool_id}` not found")
            }
            Self::UnknownSession {
                pool_id,
                session_id,
            } => write!(
                formatter,
                "sandbox session `{session_id}` not found in pool `{pool_id}`"
            ),
            Self::InvalidState {
                pool_id,
                session_id,
                action,
                state,
            } => write!(
                formatter,
                "sandbox session `{session_id}` in pool `{pool_id}` cannot perform `{action}` while in state `{state:?}`"
            ),
            Self::AcquisitionMismatch {
                pool_id,
                session_id,
                expected_acquisition_id,
                actual_acquisition_id,
            } => write!(
                formatter,
                "sandbox session `{session_id}` in pool `{pool_id}` expected acquisition `{expected_acquisition_id}` but received `{actual_acquisition_id}`"
            ),
            Self::PoolExhausted {
                pool_id,
                max_sessions,
            } => write!(
                formatter,
                "sandbox pool `{pool_id}` exhausted max_sessions={max_sessions}"
            ),
            Self::InvalidRelativePath { path } => {
                write!(
                    formatter,
                    "sandbox staged relative path `{path}` is invalid"
                )
            }
            Self::IoFailure { detail } => formatter.write_str(detail),
        }
    }
}

impl std::error::Error for SandboxPoolError {}

#[derive(Clone, Default)]
pub struct InMemorySandboxPoolService {
    pools: BTreeMap<String, ManagedSandboxPool>,
}

#[derive(Clone)]
struct ManagedSandboxPool {
    spec: SandboxPoolSpec,
    profile: ProviderSandboxProfile,
    next_session_index: u32,
    sessions: BTreeMap<String, ManagedSandboxSession>,
}

#[derive(Clone)]
struct ManagedSandboxSession {
    session_id: String,
    workspace_root: PathBuf,
    state: SandboxPoolSessionState,
    reuse_count: u32,
    iteration_count: u32,
    acquisition_counter: u32,
    active_acquisition_id: Option<String>,
    last_ready_at_ms: i64,
    staged_receipts: Vec<SandboxStagedArtifactReceipt>,
}

impl InMemorySandboxPoolService {
    pub fn create_pool(
        &mut self,
        spec: SandboxPoolSpec,
        profile: ProviderSandboxProfile,
    ) -> Result<SandboxPoolSnapshot, SandboxPoolError> {
        if self.pools.contains_key(spec.pool_id.as_str()) {
            return Err(SandboxPoolError::DuplicatePool {
                pool_id: spec.pool_id,
            });
        }
        fs::create_dir_all(spec.workspace_root.as_path()).map_err(|error| {
            SandboxPoolError::IoFailure {
                detail: format!(
                    "failed to create sandbox pool workspace `{}`: {error}",
                    spec.workspace_root.display()
                ),
            }
        })?;
        let pool = ManagedSandboxPool {
            spec: spec.clone(),
            profile,
            next_session_index: 0,
            sessions: BTreeMap::new(),
        };
        self.pools.insert(spec.pool_id.clone(), pool);
        self.snapshot(spec.pool_id.as_str())
    }

    pub fn warm_pool(
        &mut self,
        pool_id: &str,
    ) -> Result<Vec<SandboxPoolWarmReceipt>, SandboxPoolError> {
        let mut receipts = Vec::new();
        loop {
            let ready_sessions = self.snapshot(pool_id)?.ready_sessions;
            let (target_ready, max_sessions, total_sessions) = {
                let pool = self.pool(pool_id)?;
                (
                    pool.spec.target_ready,
                    pool.spec.max_sessions,
                    pool.sessions.len() as u32,
                )
            };
            if ready_sessions >= target_ready || total_sessions >= max_sessions {
                break;
            }
            receipts.push(self.warm_one_session(pool_id, true)?);
        }
        Ok(receipts)
    }

    pub fn acquire_session(
        &mut self,
        pool_id: &str,
    ) -> Result<SandboxPoolAcquisitionReceipt, SandboxPoolError> {
        let ready_before = self.snapshot(pool_id)?.ready_sessions;
        let start = Instant::now();
        let (session_id, reused_ready_session) = {
            let pool = self.pool(pool_id)?;
            if let Some(session) = pool
                .sessions
                .values()
                .find(|session| session.state == SandboxPoolSessionState::Ready)
            {
                (session.session_id.clone(), true)
            } else if pool.sessions.len() < pool.spec.max_sessions as usize {
                (self.warm_one_session(pool_id, false)?.session_id, false)
            } else {
                return Err(SandboxPoolError::PoolExhausted {
                    pool_id: String::from(pool_id),
                    max_sessions: pool.spec.max_sessions,
                });
            }
        };
        let pool = self.pool_mut(pool_id)?;
        let (resolved_session_id, acquisition_id, reuse_count) = {
            let session = pool.sessions.get_mut(session_id.as_str()).ok_or_else(|| {
                SandboxPoolError::UnknownSession {
                    pool_id: String::from(pool_id),
                    session_id: session_id.clone(),
                }
            })?;
            if session.state != SandboxPoolSessionState::Ready {
                return Err(SandboxPoolError::InvalidState {
                    pool_id: String::from(pool_id),
                    session_id,
                    action: String::from("acquire"),
                    state: session.state,
                });
            }
            session.state = SandboxPoolSessionState::Acquired;
            session.acquisition_counter = session.acquisition_counter.saturating_add(1);
            let acquisition_id =
                format!("{}-acq-{}", session.session_id, session.acquisition_counter);
            session.active_acquisition_id = Some(acquisition_id.clone());
            (
                session.session_id.clone(),
                acquisition_id,
                session.reuse_count,
            )
        };
        let ready_after = pool
            .sessions
            .values()
            .filter(|session| session.state == SandboxPoolSessionState::Ready)
            .count() as u32;
        Ok(SandboxPoolAcquisitionReceipt {
            pool_id: String::from(pool_id),
            session_id: resolved_session_id,
            acquisition_id,
            reused_ready_session,
            acquisition_latency_ms: start.elapsed().as_millis() as u64,
            ready_sessions_before: ready_before,
            ready_sessions_after: ready_after,
            reuse_count,
        })
    }

    pub fn stage_artifact(
        &mut self,
        pool_id: &str,
        session_id: &str,
        acquisition_id: &str,
        stage_kind: SandboxStageArtifactKind,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<SandboxStagedArtifactReceipt, SandboxPoolError> {
        let validated = validate_relative_path(relative_path)?;
        let pool = self.pool_mut(pool_id)?;
        let session =
            pool.sessions
                .get_mut(session_id)
                .ok_or_else(|| SandboxPoolError::UnknownSession {
                    pool_id: String::from(pool_id),
                    session_id: String::from(session_id),
                })?;
        if session.state != SandboxPoolSessionState::Acquired {
            return Err(SandboxPoolError::InvalidState {
                pool_id: String::from(pool_id),
                session_id: String::from(session_id),
                action: String::from("stage"),
                state: session.state,
            });
        }
        let expected_acquisition_id = session
            .active_acquisition_id
            .clone()
            .unwrap_or_else(|| String::from("none"));
        if expected_acquisition_id != acquisition_id {
            return Err(SandboxPoolError::AcquisitionMismatch {
                pool_id: String::from(pool_id),
                session_id: String::from(session_id),
                expected_acquisition_id,
                actual_acquisition_id: String::from(acquisition_id),
            });
        }

        let path = session.workspace_root.join(validated.as_path());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| SandboxPoolError::IoFailure {
                detail: format!(
                    "failed to create staged artifact directory `{}`: {error}",
                    parent.display()
                ),
            })?;
        }
        fs::write(path.as_path(), bytes).map_err(|error| SandboxPoolError::IoFailure {
            detail: format!(
                "failed to write staged artifact `{}`: {error}",
                path.display()
            ),
        })?;

        let receipt = staged_artifact_receipt(
            pool_id,
            session_id,
            acquisition_id,
            stage_kind,
            relative_path,
            bytes,
            session
                .staged_receipts
                .iter()
                .filter(|receipt| receipt.acquisition_id == acquisition_id)
                .count() as u32,
        );
        session.staged_receipts.push(receipt.clone());
        Ok(receipt)
    }

    pub fn run_iteration(
        &mut self,
        pool_id: &str,
        session_id: &str,
        acquisition_id: &str,
        request: SandboxLoopIterationRequest,
        controls: ProviderSandboxExecutionControls,
    ) -> Result<SandboxLoopIterationReceipt, SandboxPoolError> {
        let (
            profile,
            job_request,
            workspace_root,
            expected_outputs,
            staged_inputs,
            iteration_index,
            reuse_count,
        ) = {
            let pool = self.pool_mut(pool_id)?;
            let session = pool.sessions.get_mut(session_id).ok_or_else(|| {
                SandboxPoolError::UnknownSession {
                    pool_id: String::from(pool_id),
                    session_id: String::from(session_id),
                }
            })?;
            if session.state != SandboxPoolSessionState::Acquired {
                return Err(SandboxPoolError::InvalidState {
                    pool_id: String::from(pool_id),
                    session_id: String::from(session_id),
                    action: String::from("run_iteration"),
                    state: session.state,
                });
            }
            let expected_acquisition_id = session
                .active_acquisition_id
                .clone()
                .unwrap_or_else(|| String::from("none"));
            if expected_acquisition_id != acquisition_id {
                return Err(SandboxPoolError::AcquisitionMismatch {
                    pool_id: String::from(pool_id),
                    session_id: String::from(session_id),
                    expected_acquisition_id,
                    actual_acquisition_id: String::from(acquisition_id),
                });
            }
            let iteration_index = session.iteration_count.saturating_add(1);
            let staged_inputs = session
                .staged_receipts
                .iter()
                .filter(|receipt| receipt.acquisition_id == acquisition_id)
                .cloned()
                .collect::<Vec<_>>();
            let expected_outputs = request.expected_outputs.clone();
            let job_request = ProviderSandboxJobRequest {
                job_id: format!("{session_id}-iter-{iteration_index}"),
                provider_id: format!("sandbox-pool:{pool_id}"),
                compute_product_id: pool.profile.execution_class.product_id().to_string(),
                execution_class: pool.profile.execution_class,
                entrypoint_type: request.entrypoint_type,
                entrypoint: request.entrypoint,
                payload: request.payload,
                arguments: request.arguments,
                workspace_root: session.workspace_root.clone(),
                expected_outputs: expected_outputs.clone(),
                timeout_request_s: request.timeout_request_s,
                network_request: request.network_request,
                filesystem_request: request.filesystem_request,
                environment: request.environment,
                resource_request: request.resource_request,
                payout_reference: request.payout_reference,
                verification_posture: request.verification_posture,
            };
            (
                pool.profile.clone(),
                job_request,
                session.workspace_root.clone(),
                expected_outputs,
                staged_inputs,
                iteration_index,
                session.reuse_count,
            )
        };

        let execution_result = execute_sandbox_job(&profile, &job_request, &controls);
        let output_artifacts = expected_outputs
            .iter()
            .enumerate()
            .filter_map(|(index, relative_path)| {
                let path = workspace_root.join(relative_path);
                let bytes = fs::read(path.as_path()).ok()?;
                Some(staged_artifact_receipt(
                    pool_id,
                    session_id,
                    acquisition_id,
                    SandboxStageArtifactKind::OutputArtifact,
                    relative_path.as_str(),
                    bytes.as_slice(),
                    index as u32,
                ))
            })
            .collect::<Vec<_>>();
        let iteration_digest = stable_iteration_digest(
            pool_id,
            session_id,
            acquisition_id,
            iteration_index,
            staged_inputs.as_slice(),
            output_artifacts.as_slice(),
            &execution_result.receipt,
        );

        let pool = self.pool_mut(pool_id)?;
        let session =
            pool.sessions
                .get_mut(session_id)
                .ok_or_else(|| SandboxPoolError::UnknownSession {
                    pool_id: String::from(pool_id),
                    session_id: String::from(session_id),
                })?;
        session.iteration_count = iteration_index;
        session.reuse_count = session.reuse_count.saturating_add(1);
        session.state = SandboxPoolSessionState::Ready;
        session.active_acquisition_id = None;
        session.last_ready_at_ms = now_epoch_ms();
        session.staged_receipts.extend(output_artifacts.clone());

        Ok(SandboxLoopIterationReceipt {
            pool_id: String::from(pool_id),
            session_id: String::from(session_id),
            acquisition_id: String::from(acquisition_id),
            iteration_index,
            reused_workspace: reuse_count > 0,
            staged_inputs,
            output_artifacts,
            execution_result,
            iteration_digest,
        })
    }

    pub fn snapshot(&self, pool_id: &str) -> Result<SandboxPoolSnapshot, SandboxPoolError> {
        let pool = self.pool(pool_id)?;
        let ready_sessions = pool
            .sessions
            .values()
            .filter(|session| session.state == SandboxPoolSessionState::Ready)
            .count() as u32;
        let acquired_sessions = pool
            .sessions
            .values()
            .filter(|session| session.state == SandboxPoolSessionState::Acquired)
            .count() as u32;
        Ok(SandboxPoolSnapshot {
            pool_id: pool.spec.pool_id.clone(),
            profile_id: pool.profile.profile_id.clone(),
            target_ready: pool.spec.target_ready,
            max_sessions: pool.spec.max_sessions,
            ready_sessions,
            acquired_sessions,
            sessions: pool
                .sessions
                .values()
                .map(|session| SandboxPoolSessionSnapshot {
                    session_id: session.session_id.clone(),
                    workspace_root: session.workspace_root.clone(),
                    state: session.state,
                    reuse_count: session.reuse_count,
                    iteration_count: session.iteration_count,
                    last_ready_at_ms: session.last_ready_at_ms,
                    active_acquisition_id: session.active_acquisition_id.clone(),
                    staged_receipt_count: session.staged_receipts.len() as u32,
                })
                .collect(),
        })
    }

    fn warm_one_session(
        &mut self,
        pool_id: &str,
        created_new_session: bool,
    ) -> Result<SandboxPoolWarmReceipt, SandboxPoolError> {
        let start = Instant::now();
        let pool = self.pool_mut(pool_id)?;
        pool.next_session_index = pool.next_session_index.saturating_add(1);
        let session_id = format!("{}-session-{}", pool.spec.pool_id, pool.next_session_index);
        let workspace_root = pool.spec.workspace_root.join(session_id.as_str());
        fs::create_dir_all(workspace_root.join(".openagents-home")).map_err(|error| {
            SandboxPoolError::IoFailure {
                detail: format!(
                    "failed to create pool workspace home `{}`: {error}",
                    workspace_root.display()
                ),
            }
        })?;
        fs::create_dir_all(workspace_root.join(".openagents-tmp")).map_err(|error| {
            SandboxPoolError::IoFailure {
                detail: format!(
                    "failed to create pool workspace tmp `{}`: {error}",
                    workspace_root.display()
                ),
            }
        })?;
        let session = ManagedSandboxSession {
            session_id: session_id.clone(),
            workspace_root: workspace_root.clone(),
            state: SandboxPoolSessionState::Ready,
            reuse_count: 0,
            iteration_count: 0,
            acquisition_counter: 0,
            active_acquisition_id: None,
            last_ready_at_ms: now_epoch_ms(),
            staged_receipts: Vec::new(),
        };
        pool.sessions.insert(session_id.clone(), session);
        let ready_session_count = pool
            .sessions
            .values()
            .filter(|session| session.state == SandboxPoolSessionState::Ready)
            .count() as u32;
        Ok(SandboxPoolWarmReceipt {
            pool_id: String::from(pool_id),
            session_id,
            workspace_root,
            created_new_session,
            warm_latency_ms: start.elapsed().as_millis() as u64,
            ready_session_count,
        })
    }

    fn pool(&self, pool_id: &str) -> Result<&ManagedSandboxPool, SandboxPoolError> {
        self.pools
            .get(pool_id)
            .ok_or_else(|| SandboxPoolError::UnknownPool {
                pool_id: String::from(pool_id),
            })
    }

    fn pool_mut(&mut self, pool_id: &str) -> Result<&mut ManagedSandboxPool, SandboxPoolError> {
        self.pools
            .get_mut(pool_id)
            .ok_or_else(|| SandboxPoolError::UnknownPool {
                pool_id: String::from(pool_id),
            })
    }
}

fn validate_relative_path(relative_path: &str) -> Result<PathBuf, SandboxPoolError> {
    let path = Path::new(relative_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(SandboxPoolError::InvalidRelativePath {
            path: String::from(relative_path),
        });
    }
    Ok(path.to_path_buf())
}

fn staged_artifact_receipt(
    pool_id: &str,
    session_id: &str,
    acquisition_id: &str,
    stage_kind: SandboxStageArtifactKind,
    relative_path: &str,
    bytes: &[u8],
    stream_index: u32,
) -> SandboxStagedArtifactReceipt {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let sha256_digest = hex::encode(hasher.finalize());
    SandboxStagedArtifactReceipt {
        stage_id: stable_stage_id(
            pool_id,
            session_id,
            acquisition_id,
            relative_path,
            sha256_digest.as_str(),
            stream_index,
        ),
        pool_id: String::from(pool_id),
        session_id: String::from(session_id),
        acquisition_id: String::from(acquisition_id),
        stage_kind,
        relative_path: String::from(relative_path),
        sha256_digest,
        size_bytes: bytes.len() as u64,
        observed_at_ms: now_epoch_ms(),
        stream_index,
    }
}

fn stable_stage_id(
    pool_id: &str,
    session_id: &str,
    acquisition_id: &str,
    relative_path: &str,
    digest: &str,
    stream_index: u32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_sandbox_stage|");
    hasher.update(pool_id.as_bytes());
    hasher.update(b"|");
    hasher.update(session_id.as_bytes());
    hasher.update(b"|");
    hasher.update(acquisition_id.as_bytes());
    hasher.update(b"|");
    hasher.update(relative_path.as_bytes());
    hasher.update(b"|");
    hasher.update(digest.as_bytes());
    hasher.update(b"|");
    hasher.update(stream_index.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_iteration_digest(
    pool_id: &str,
    session_id: &str,
    acquisition_id: &str,
    iteration_index: u32,
    staged_inputs: &[SandboxStagedArtifactReceipt],
    output_artifacts: &[SandboxStagedArtifactReceipt],
    execution_receipt: &ProviderSandboxExecutionReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_sandbox_iteration|");
    hasher.update(pool_id.as_bytes());
    hasher.update(b"|");
    hasher.update(session_id.as_bytes());
    hasher.update(b"|");
    hasher.update(acquisition_id.as_bytes());
    hasher.update(b"|");
    hasher.update(iteration_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(execution_receipt.receipt_id.as_bytes());
    for input in staged_inputs {
        hasher.update(b"|input|");
        hasher.update(input.stage_id.as_bytes());
    }
    for output in output_artifacts {
        hasher.update(b"|output|");
        hasher.update(output.stage_id.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn now_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        InMemorySandboxPoolService, SandboxLoopIterationRequest, SandboxPoolSessionState,
        SandboxPoolSpec, SandboxStageArtifactKind,
    };
    use crate::{
        ProviderSandboxEntrypointType, ProviderSandboxExecutionClass,
        ProviderSandboxExecutionControls, ProviderSandboxProfile, ProviderSandboxRuntimeKind,
    };

    fn fake_binary(
        dir: &std::path::Path,
        name: &str,
        body: &str,
    ) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let path = dir.join(name);
        fs::write(&path, body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions)?;
        }
        Ok(path)
    }

    fn subprocess_profile(runtime_binary_path: &std::path::Path) -> ProviderSandboxProfile {
        ProviderSandboxProfile {
            profile_id: String::from("pool-test-profile"),
            profile_digest: String::from("sha256:pool-test"),
            execution_class: ProviderSandboxExecutionClass::PosixExec,
            runtime_family: String::from("bash"),
            runtime_version: String::from("test-runtime"),
            sandbox_engine: String::from("local_subprocess"),
            os_family: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_limit: 2,
            memory_limit_mb: 2048,
            disk_limit_mb: 4096,
            timeout_limit_s: 5,
            network_mode: String::from("host_inherit"),
            filesystem_mode: String::from("host_inherit"),
            workspace_mode: String::from("ephemeral"),
            artifact_output_mode: String::from("declared_paths_only"),
            secrets_mode: String::from("none"),
            allowed_binaries: vec![String::from("runtime")],
            toolchain_inventory: vec![String::from("runtime")],
            container_image: None,
            runtime_image_digest: None,
            accelerator_policy: None,
            runtime_kind: ProviderSandboxRuntimeKind::Posix,
            runtime_ready: true,
            runtime_binary_path: Some(runtime_binary_path.display().to_string()),
            capability_summary: String::from("test"),
        }
    }

    #[test]
    fn sandbox_pool_warms_and_reuses_the_same_workspace_across_iterations()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let mut service = InMemorySandboxPoolService::default();
        service.create_pool(
            SandboxPoolSpec {
                pool_id: String::from("weather"),
                workspace_root: temp.path().join("pool"),
                target_ready: 1,
                max_sessions: 1,
            },
            subprocess_profile(runtime.as_path()),
        )?;

        let warm = service.warm_pool("weather")?;
        assert_eq!(warm.len(), 1);
        let first_acquire = service.acquire_session("weather")?;
        assert_eq!(first_acquire.ready_sessions_before, 1);
        assert_eq!(first_acquire.ready_sessions_after, 0);

        service.stage_artifact(
            "weather",
            first_acquire.session_id.as_str(),
            first_acquire.acquisition_id.as_str(),
            SandboxStageArtifactKind::CommandInput,
            "scripts/job.sh",
            b"/bin/mkdir -p state outputs\ncount=0\nif [ -f state/counter.txt ]; then count=$(/bin/cat state/counter.txt); fi\ncount=$((count + 1))\nprintf '%s' \"$count\" > state/counter.txt\nprintf 'run-%s' \"$count\" > outputs/result.txt\n",
        )?;
        let image_stage = service.stage_artifact(
            "weather",
            first_acquire.session_id.as_str(),
            first_acquire.acquisition_id.as_str(),
            SandboxStageArtifactKind::ImageFrame,
            "frames/frame-1.bin",
            b"frame-one",
        )?;
        assert_eq!(image_stage.stage_kind, SandboxStageArtifactKind::ImageFrame);

        let mut request = SandboxLoopIterationRequest::new(
            ProviderSandboxEntrypointType::WorkspaceFile,
            "scripts/job.sh",
        );
        request.expected_outputs = vec![String::from("outputs/result.txt")];
        let first_iteration = service.run_iteration(
            "weather",
            first_acquire.session_id.as_str(),
            first_acquire.acquisition_id.as_str(),
            request.clone(),
            ProviderSandboxExecutionControls::default(),
        )?;
        assert_eq!(first_iteration.iteration_index, 1);
        assert!(!first_iteration.reused_workspace);
        assert_eq!(
            first_iteration.output_artifacts.len(),
            1,
            "unexpected first iteration receipt: {:?}",
            first_iteration.execution_result
        );

        let snapshot = service.snapshot("weather")?;
        assert_eq!(snapshot.ready_sessions, 1);
        assert_eq!(snapshot.sessions[0].state, SandboxPoolSessionState::Ready);
        let result_path = snapshot.sessions[0]
            .workspace_root
            .join("outputs/result.txt");
        assert_eq!(
            fs::read_to_string(result_path.as_path())?,
            String::from("run-1")
        );

        let second_acquire = service.acquire_session("weather")?;
        assert_eq!(second_acquire.session_id, first_acquire.session_id);
        service.stage_artifact(
            "weather",
            second_acquire.session_id.as_str(),
            second_acquire.acquisition_id.as_str(),
            SandboxStageArtifactKind::ContextArtifact,
            "context/input.json",
            br#"{"city":"Paris"}"#,
        )?;
        let second_iteration = service.run_iteration(
            "weather",
            second_acquire.session_id.as_str(),
            second_acquire.acquisition_id.as_str(),
            request,
            ProviderSandboxExecutionControls::default(),
        )?;
        assert_eq!(second_iteration.iteration_index, 2);
        assert!(second_iteration.reused_workspace);

        let snapshot = service.snapshot("weather")?;
        let result_path = snapshot.sessions[0]
            .workspace_root
            .join("outputs/result.txt");
        assert_eq!(
            fs::read_to_string(result_path.as_path())?,
            String::from("run-2")
        );
        Ok(())
    }

    #[test]
    fn sandbox_pool_on_demand_acquire_is_machine_legible() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempdir()?;
        let runtime = fake_binary(
            temp.path(),
            "runtime",
            "#!/bin/sh\nscript=\"$1\"\nshift\n/bin/sh \"$script\" \"$@\"\n",
        )?;
        let mut service = InMemorySandboxPoolService::default();
        service.create_pool(
            SandboxPoolSpec {
                pool_id: String::from("eval"),
                workspace_root: temp.path().join("pool"),
                target_ready: 0,
                max_sessions: 1,
            },
            subprocess_profile(runtime.as_path()),
        )?;

        let acquire = service.acquire_session("eval")?;
        assert_eq!(acquire.ready_sessions_before, 0);
        assert_eq!(acquire.ready_sessions_after, 0);
        assert_eq!(acquire.reuse_count, 0);

        service.stage_artifact(
            "eval",
            acquire.session_id.as_str(),
            acquire.acquisition_id.as_str(),
            SandboxStageArtifactKind::CommandInput,
            "scripts/job.sh",
            b"/bin/mkdir -p outputs\nprintf 'ok' > outputs/result.txt\n",
        )?;
        let mut request = SandboxLoopIterationRequest::new(
            ProviderSandboxEntrypointType::WorkspaceFile,
            "scripts/job.sh",
        );
        request.expected_outputs = vec![String::from("outputs/result.txt")];
        let receipt = service.run_iteration(
            "eval",
            acquire.session_id.as_str(),
            acquire.acquisition_id.as_str(),
            request,
            ProviderSandboxExecutionControls::default(),
        )?;
        assert_eq!(
            receipt.output_artifacts.len(),
            1,
            "unexpected pool iteration receipt: {:?}",
            receipt.execution_result
        );
        assert_eq!(
            receipt.output_artifacts[0].stage_kind,
            SandboxStageArtifactKind::OutputArtifact
        );
        assert!(!receipt.iteration_digest.is_empty());
        Ok(())
    }
}
