use std::{
    env, fs,
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use psionic_backend_cuda::CudaBackend;
use psionic_core::{PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope};
use psionic_runtime::{
    ClusterCommunicationClass, ClusterTransportClass, DeviceDescriptor, HealthStatus,
    RuntimeHealth, TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind, TrainingDeviceMeshContext,
    TrainingElasticMembershipContext,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    device_matches_distributed_h100, inspect_local_distributed_8xh100_machine,
    ParameterGolfDistributed8xH100BringupReport, ParameterGolfDistributedLiveVisualizationWriter,
    ParameterGolfDistributedVisualizationError,
    PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_LOGS_DIR_ARTIFACT_REF,
    PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_RECEIPTS_DIR_ARTIFACT_REF,
    PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RECEIPT_ARTIFACT_REF,
};

const CHILD_ENV_VAR: &str = "PSIONIC_PARAMETER_GOLF_DISTRIBUTED_8XH100_BOOTSTRAP_CHILD";
const CHILD_RANK_ENV_VAR: &str = "PSIONIC_PARAMETER_GOLF_DISTRIBUTED_8XH100_BOOTSTRAP_RANK";
const CHILD_LOCAL_RANK_ENV_VAR: &str =
    "PSIONIC_PARAMETER_GOLF_DISTRIBUTED_8XH100_BOOTSTRAP_LOCAL_RANK";
const CHILD_WORLD_SIZE_ENV_VAR: &str =
    "PSIONIC_PARAMETER_GOLF_DISTRIBUTED_8XH100_BOOTSTRAP_WORLD_SIZE";
const CHILD_RECEIPT_PATH_ENV_VAR: &str =
    "PSIONIC_PARAMETER_GOLF_DISTRIBUTED_8XH100_BOOTSTRAP_RECEIPT_PATH";
const CHILD_LOG_PATH_ENV_VAR: &str = "PSIONIC_PARAMETER_GOLF_DISTRIBUTED_8XH100_BOOTSTRAP_LOG_PATH";

const CHALLENGE_WORLD_SIZE: usize = 8;
const BOOTSTRAP_MESH_ID: &str = "parameter_golf.distributed_8xh100.runtime_bootstrap";
const BOOTSTRAP_CLUSTER_STATE_DIGEST: &str = "parameter_golf_distributed_8xh100_cluster_state_v1";
const BOOTSTRAP_TOPOLOGY_DIGEST: &str = "parameter_golf_distributed_8xh100_topology_v1";
const BOOTSTRAP_NODE_ID_PREFIX: &str = "parameter-golf-rank";
const BOOTSTRAP_REQUESTED_BACKEND: ParameterGolfDistributed8xH100RuntimeRequestedBackend =
    ParameterGolfDistributed8xH100RuntimeRequestedBackend::Nccl;

/// Requested backend family preserved by the runtime bootstrap contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfDistributed8xH100RuntimeRequestedBackend {
    /// NCCL-class public posture for the `8xH100` lane.
    Nccl,
}

impl ParameterGolfDistributed8xH100RuntimeRequestedBackend {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nccl => "nccl",
        }
    }
}

/// Ordered member ledger preserved by the runtime bootstrap contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributed8xH100RuntimeBootstrapGroupMember {
    /// Stable synthetic node identifier for this rank.
    pub node_id: String,
    /// Global rank.
    pub rank: usize,
    /// Logical shard identifier.
    pub shard_id: usize,
    /// Plain-language device label.
    pub device_label: String,
}

/// Explicit mesh/bootstrap snapshot preserved by the runtime bootstrap contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributed8xH100RuntimeBootstrapGroupSnapshot {
    /// Stable digest over the explicit runtime bootstrap contract.
    pub group_id: String,
    /// Requested backend family for the public posture.
    pub requested_backend: ParameterGolfDistributed8xH100RuntimeRequestedBackend,
    /// Effective runtime backend carried by the mesh.
    pub effective_backend: String,
    /// Communication class carried by the mesh.
    pub communication_class: ClusterCommunicationClass,
    /// Transport class attributed to this single-pod bootstrap.
    pub transport: ClusterTransportClass,
    /// Explicit mesh context preserved by the runtime bootstrap.
    pub mesh: TrainingDeviceMeshContext,
    /// Stable synthetic local node identifier for this rank.
    pub local_node_id: String,
    /// Local rank inside the bootstrapped posture.
    pub rank: usize,
    /// Total size of the bootstrapped posture.
    pub size: usize,
    /// Ordered members that define the rank layout.
    pub members: Vec<ParameterGolfDistributed8xH100RuntimeBootstrapGroupMember>,
}

/// Final state for one real distributed runtime bootstrap attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfDistributed8xH100RuntimeBootstrapDisposition {
    /// All eight ranks bootstrapped one explicit public distributed group.
    Bootstrapped,
    /// At least one rank refused runtime prerequisites after the machine-level bring-up gate passed.
    RefusedRuntimePrerequisites,
    /// At least one child process failed before writing one explicit refusal receipt.
    ChildProcessFailed,
}

/// Per-rank bootstrap receipt emitted by the shipped runtime payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributed8xH100RuntimeBootstrapRankReceipt {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable run identifier.
    pub run_id: String,
    /// Rank owned by this child.
    pub rank: usize,
    /// Local rank on the current pod.
    pub local_rank: usize,
    /// Declared world size.
    pub world_size: usize,
    /// Stable synthetic node id attributed to this rank inside the bootstrapped mesh.
    pub local_node_id: String,
    /// Exact `CUDA_VISIBLE_DEVICES` contract observed by this rank.
    pub cuda_visible_devices: String,
    /// Retained rank-local log path.
    pub log_path: String,
    /// Observed CUDA runtime health under the rank-local GPU visibility contract.
    pub observed_cuda_health: RuntimeHealth,
    /// Observed CUDA devices visible to this rank.
    pub observed_cuda_devices: Vec<DeviceDescriptor>,
    /// Number of visible devices that satisfy the exact H100 challenge matcher.
    pub matching_visible_h100_device_count: usize,
    /// Requested distributed backend family for the public group bootstrap.
    pub requested_backend: ParameterGolfDistributed8xH100RuntimeRequestedBackend,
    /// Whether runtime prerequisites were satisfied strongly enough to admit group bootstrap.
    pub runtime_prerequisites_satisfied: bool,
    /// Bootstrapped public distributed group snapshot when bootstrap succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distributed_group: Option<ParameterGolfDistributed8xH100RuntimeBootstrapGroupSnapshot>,
    /// Explicit refusal when runtime prerequisites or group bootstrap failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
    /// Honest claim boundary for the child receipt.
    pub claim_boundary: String,
    /// Stable digest over the child receipt payload.
    pub receipt_digest: String,
}

impl ParameterGolfDistributed8xH100RuntimeBootstrapRankReceipt {
    /// Returns a stable digest over the child receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_distributed_8xh100_runtime_bootstrap_rank_receipt|",
            &digestible,
        )
    }
}

/// Parent-observed outcome for one spawned rank process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributed8xH100RuntimeBootstrapRankLaunch {
    /// Rank that was launched.
    pub rank: usize,
    /// Local rank that was launched.
    pub local_rank: usize,
    /// Exact `CUDA_VISIBLE_DEVICES` assignment used for the child.
    pub cuda_visible_devices: String,
    /// Retained child receipt path.
    pub receipt_path: String,
    /// Retained child log path.
    pub log_path: String,
    /// Child exit code when one was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Machine-readable child receipt when one was preserved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<ParameterGolfDistributed8xH100RuntimeBootstrapRankReceipt>,
}

/// Aggregate runtime bootstrap receipt emitted by the shipped runtime payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributed8xH100RuntimeBootstrapReceipt {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable run identifier.
    pub run_id: String,
    /// Distributed backend family requested for the public bootstrap.
    pub requested_backend: ParameterGolfDistributed8xH100RuntimeRequestedBackend,
    /// Expected world size for the exact public posture.
    pub world_size: usize,
    /// Bring-up report path that gated this runtime bootstrap attempt.
    pub bringup_report_path: String,
    /// Bring-up report digest that gated this runtime bootstrap attempt.
    pub bringup_report_digest: String,
    /// Runtime payload path used for the child fanout.
    pub runtime_payload_path: String,
    /// Manifest path used for the child fanout.
    pub runtime_manifest_path: String,
    /// Ordered child launch outcomes.
    pub rank_launches: Vec<ParameterGolfDistributed8xH100RuntimeBootstrapRankLaunch>,
    /// Number of ranks that bootstrapped successfully.
    pub successful_rank_count: usize,
    /// Final disposition for the runtime bootstrap attempt.
    pub disposition: ParameterGolfDistributed8xH100RuntimeBootstrapDisposition,
    /// Primary refusal when bootstrap did not succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
    /// Honest drift notes for the current boundary.
    pub drift_notes: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the aggregate receipt payload.
    pub receipt_digest: String,
}

impl ParameterGolfDistributed8xH100RuntimeBootstrapReceipt {
    /// Returns a stable digest over the aggregate receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_distributed_8xh100_runtime_bootstrap_receipt|",
            &digestible,
        )
    }
}

/// Failure while bootstrapping the distributed runtime entrypoint.
#[derive(Debug, Error)]
pub enum ParameterGolfDistributed8xH100RuntimeBootstrapError {
    #[error(
        "parameter golf distributed 8xH100 runtime bootstrap failed to read `{path}`: {error}"
    )]
    Read { path: String, error: std::io::Error },
    #[error(
        "parameter golf distributed 8xH100 runtime bootstrap failed to write `{path}`: {error}"
    )]
    Write { path: String, error: std::io::Error },
    #[error(
        "parameter golf distributed 8xH100 runtime bootstrap missing environment variable `{key}`"
    )]
    MissingEnv { key: &'static str },
    #[error(
        "parameter golf distributed 8xH100 runtime bootstrap invalid environment `{key}`=`{value}`"
    )]
    InvalidEnv { key: &'static str, value: String },
    #[error("parameter golf distributed 8xH100 runtime bootstrap child spawn failed for rank {rank}: {error}")]
    ChildSpawn { rank: usize, error: std::io::Error },
    #[error("parameter golf distributed 8xH100 runtime bootstrap child wait failed for rank {rank}: {error}")]
    ChildWait { rank: usize, error: std::io::Error },
    #[error("parameter golf distributed 8xH100 runtime bootstrap child receipt decode failed at `{path}`: {error}")]
    ChildDecode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Visualization(#[from] ParameterGolfDistributedVisualizationError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Returns whether the current process is one internal distributed-bootstrap child.
#[must_use]
pub fn parameter_golf_distributed_8xh100_runtime_bootstrap_child_enabled() -> bool {
    env::var_os(CHILD_ENV_VAR).is_some()
}

/// Derives the canonical aggregate bootstrap receipt path beside the bring-up report.
#[must_use]
pub fn parameter_golf_distributed_8xh100_runtime_bootstrap_receipt_path(
    root: &Path,
    bringup_report_path: &str,
) -> PathBuf {
    let resolved = root.join(bringup_report_path);
    match resolved.parent() {
        Some(parent) => parent.join("parameter_golf_distributed_8xh100_runtime_bootstrap.json"),
        None => root.join(PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RECEIPT_ARTIFACT_REF),
    }
}

/// Derives the canonical per-rank receipt directory beside the bring-up report.
#[must_use]
pub fn parameter_golf_distributed_8xh100_runtime_bootstrap_rank_receipts_dir(
    root: &Path,
    bringup_report_path: &str,
) -> PathBuf {
    let resolved = root.join(bringup_report_path);
    match resolved.parent() {
        Some(parent) => parent.join("runtime_bootstrap_receipts"),
        None => root.join(
            PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_RECEIPTS_DIR_ARTIFACT_REF,
        ),
    }
}

/// Derives the canonical per-rank log directory beside the bring-up report.
#[must_use]
pub fn parameter_golf_distributed_8xh100_runtime_bootstrap_rank_logs_dir(
    root: &Path,
    bringup_report_path: &str,
) -> PathBuf {
    let resolved = root.join(bringup_report_path);
    match resolved.parent() {
        Some(parent) => parent.join("runtime_bootstrap_logs"),
        None => root
            .join(PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_LOGS_DIR_ARTIFACT_REF),
    }
}

/// Executes one real multi-rank bootstrap attempt from the shipped runtime payload.
pub fn execute_parameter_golf_distributed_8xh100_runtime_bootstrap(
    root: &Path,
    manifest_path: &Path,
    run_id: &str,
    bringup_report_path: &Path,
    bringup_report: &ParameterGolfDistributed8xH100BringupReport,
    mut live_visualization_writer: Option<&mut ParameterGolfDistributedLiveVisualizationWriter>,
) -> Result<
    ParameterGolfDistributed8xH100RuntimeBootstrapReceipt,
    ParameterGolfDistributed8xH100RuntimeBootstrapError,
> {
    let receipt_path = parameter_golf_distributed_8xh100_runtime_bootstrap_receipt_path(
        root,
        &bringup_report_path
            .strip_prefix(root)
            .unwrap_or(bringup_report_path)
            .display()
            .to_string(),
    );
    let rank_receipts_dir = parameter_golf_distributed_8xh100_runtime_bootstrap_rank_receipts_dir(
        root,
        &bringup_report_path
            .strip_prefix(root)
            .unwrap_or(bringup_report_path)
            .display()
            .to_string(),
    );
    let rank_logs_dir = parameter_golf_distributed_8xh100_runtime_bootstrap_rank_logs_dir(
        root,
        &bringup_report_path
            .strip_prefix(root)
            .unwrap_or(bringup_report_path)
            .display()
            .to_string(),
    );
    fs::create_dir_all(&rank_receipts_dir).map_err(|error| {
        ParameterGolfDistributed8xH100RuntimeBootstrapError::Write {
            path: rank_receipts_dir.display().to_string(),
            error,
        }
    })?;
    fs::create_dir_all(&rank_logs_dir).map_err(|error| {
        ParameterGolfDistributed8xH100RuntimeBootstrapError::Write {
            path: rank_logs_dir.display().to_string(),
            error,
        }
    })?;
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfDistributed8xH100RuntimeBootstrapError::Write {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }

    let current_exe = env::current_exe().map_err(|error| {
        ParameterGolfDistributed8xH100RuntimeBootstrapError::Read {
            path: String::from("current_exe"),
            error,
        }
    })?;
    let runtime_payload_path = current_exe.display().to_string();
    let manifest_path = manifest_path
        .canonicalize()
        .unwrap_or_else(|_| manifest_path.to_path_buf());

    if let Some(writer) = live_visualization_writer.as_deref_mut() {
        writer.record_phase(
            "training",
            Some(String::from("runtime_bootstrap")),
            format!(
                "Launching {} distributed bootstrap ranks for the RunPod 8xH100 lane.",
                CHALLENGE_WORLD_SIZE
            ),
            vec![
                String::from("runtime_bootstrap"),
                String::from("rank_fanout"),
                String::from("distributed_runtime"),
            ],
            Some(1),
            true,
        )?;
    }

    let mut children = Vec::new();
    for rank in 0..CHALLENGE_WORLD_SIZE {
        let log_path = rank_logs_dir.join(format!("rank_{rank}.log"));
        let receipt_path = rank_receipts_dir.join(format!("rank_{rank}.json"));
        let stdout = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&log_path)
            .map_err(
                |error| ParameterGolfDistributed8xH100RuntimeBootstrapError::Write {
                    path: log_path.display().to_string(),
                    error,
                },
            )?;
        let stderr = stdout.try_clone().map_err(|error| {
            ParameterGolfDistributed8xH100RuntimeBootstrapError::Write {
                path: log_path.display().to_string(),
                error,
            }
        })?;
        let child =
            Command::new(&current_exe)
                .arg(&manifest_path)
                .current_dir(root)
                .env(
                    "PSIONIC_PARAMETER_GOLF_EXECUTION_MODE",
                    "distributed_8xh100_train",
                )
                .env(CHILD_ENV_VAR, "1")
                .env(CHILD_RANK_ENV_VAR, rank.to_string())
                .env(CHILD_LOCAL_RANK_ENV_VAR, rank.to_string())
                .env(CHILD_WORLD_SIZE_ENV_VAR, CHALLENGE_WORLD_SIZE.to_string())
                .env(CHILD_RECEIPT_PATH_ENV_VAR, &receipt_path)
                .env(CHILD_LOG_PATH_ENV_VAR, &log_path)
                .env("CUDA_VISIBLE_DEVICES", rank.to_string())
                .env("WORLD_SIZE", CHALLENGE_WORLD_SIZE.to_string())
                .env("PSIONIC_DISTRIBUTED_RANK", rank.to_string())
                .env("PSIONIC_DISTRIBUTED_LOCAL_RANK", rank.to_string())
                .env(
                    "PSIONIC_DISTRIBUTED_WORLD_SIZE",
                    CHALLENGE_WORLD_SIZE.to_string(),
                )
                .env("PSIONIC_DISTRIBUTED_NODE_ID", bootstrap_node_id(rank))
                .env(
                    "PSIONIC_DISTRIBUTED_CLUSTER_STATE_DIGEST",
                    BOOTSTRAP_CLUSTER_STATE_DIGEST,
                )
                .env(
                    "PSIONIC_DISTRIBUTED_TOPOLOGY_DIGEST",
                    BOOTSTRAP_TOPOLOGY_DIGEST,
                )
                .env("PSIONIC_DISTRIBUTED_MESH_ID", BOOTSTRAP_MESH_ID)
                .env("PSIONIC_DISTRIBUTED_MESH_REVISION", "1")
                .env("PSIONIC_DISTRIBUTED_EFFECTIVE_BACKEND", "cuda")
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr))
                .spawn()
                .map_err(|error| {
                    ParameterGolfDistributed8xH100RuntimeBootstrapError::ChildSpawn { rank, error }
                })?;
        children.push((rank, log_path, receipt_path, child));
    }

    let mut rank_launches = Vec::with_capacity(CHALLENGE_WORLD_SIZE);
    let mut completed_launch_count = 0_usize;
    for (rank, log_path, receipt_path, mut child) in children {
        let status = child.wait().map_err(|error| {
            ParameterGolfDistributed8xH100RuntimeBootstrapError::ChildWait { rank, error }
        })?;
        let receipt = if receipt_path.is_file() {
            let bytes = fs::read(&receipt_path).map_err(|error| {
                ParameterGolfDistributed8xH100RuntimeBootstrapError::Read {
                    path: receipt_path.display().to_string(),
                    error,
                }
            })?;
            Some(
                serde_json::from_slice::<ParameterGolfDistributed8xH100RuntimeBootstrapRankReceipt>(
                    &bytes,
                )
                .map_err(|error| ParameterGolfDistributed8xH100RuntimeBootstrapError::ChildDecode {
                    path: receipt_path.display().to_string(),
                    error,
                })?,
            )
        } else {
            None
        };
        rank_launches.push(ParameterGolfDistributed8xH100RuntimeBootstrapRankLaunch {
            rank,
            local_rank: rank,
            cuda_visible_devices: rank.to_string(),
            receipt_path: receipt_path.display().to_string(),
            log_path: log_path.display().to_string(),
            exit_code: status.code(),
            receipt,
        });
        completed_launch_count = completed_launch_count.saturating_add(1);
        if let Some(writer) = live_visualization_writer.as_deref_mut() {
            writer.record_phase(
                "training",
                Some(String::from("runtime_bootstrap")),
                format!(
                    "Distributed bootstrap ranks completed: {completed_launch_count}/{}.",
                    CHALLENGE_WORLD_SIZE
                ),
                vec![
                    String::from("runtime_bootstrap"),
                    String::from("rank_wait"),
                    String::from("distributed_runtime"),
                ],
                Some(1),
                false,
            )?;
        }
    }

    let successful_rank_count = rank_launches
        .iter()
        .filter(|launch| {
            launch.exit_code == Some(0)
                && launch
                    .receipt
                    .as_ref()
                    .is_some_and(|receipt| receipt.runtime_prerequisites_satisfied)
        })
        .count();

    let disposition = if successful_rank_count == CHALLENGE_WORLD_SIZE {
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::Bootstrapped
    } else if rank_launches.iter().all(|launch| launch.receipt.is_some()) {
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::RefusedRuntimePrerequisites
    } else {
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::ChildProcessFailed
    };

    let refusal = match disposition {
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::Bootstrapped => None,
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::RefusedRuntimePrerequisites => {
            let failing_ranks = rank_launches
                .iter()
                .filter(|launch| {
                    launch
                        .receipt
                        .as_ref()
                        .is_none_or(|receipt| !receipt.runtime_prerequisites_satisfied)
                })
                .map(|launch| launch.rank.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    format!(
                        "distributed 8xH100 runtime bootstrap admitted the machine-level bring-up gate but one or more ranks refused runtime prerequisites: failing_ranks=[{failing_ranks}]"
                    ),
                )
                .with_subject(String::from(
                    "parameter_golf_distributed_8xh100_runtime_bootstrap",
                )),
            )
        }
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::ChildProcessFailed => Some(
            PsionicRefusal::new(
                PsionicRefusalCode::TopologyMismatch,
                PsionicRefusalScope::Runtime,
                String::from(
                    "distributed 8xH100 runtime bootstrap could not collect one complete explicit receipt from every spawned rank child",
                ),
            )
            .with_subject(String::from(
                "parameter_golf_distributed_8xh100_runtime_bootstrap",
            )),
        ),
    };
    let mut drift_notes = vec![String::from(
        "This receipt proves the shipped runtime crossed from machine admission into one real multi-rank bootstrap attempt with explicit rank-to-GPU assignment. It does not claim that the later distributed train step or distributed validation have landed.",
    )];
    if matches!(
        disposition,
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::Bootstrapped
    ) {
        drift_notes.push(String::from(
            "The bootstrapped public distributed group still exposes reference-emulated collective helpers on the current Psionic surface; later train-step work must add execution-backed step observations above this bootstrap seam.",
        ));
    }
    let mut receipt = ParameterGolfDistributed8xH100RuntimeBootstrapReceipt {
        schema_version: 1,
        run_id: run_id.to_string(),
        requested_backend: BOOTSTRAP_REQUESTED_BACKEND,
        world_size: CHALLENGE_WORLD_SIZE,
        bringup_report_path: bringup_report_path.display().to_string(),
        bringup_report_digest: bringup_report.report_digest.clone(),
        runtime_payload_path,
        runtime_manifest_path: manifest_path.display().to_string(),
        rank_launches,
        successful_rank_count,
        disposition,
        refusal,
        drift_notes,
        claim_boundary: String::from(
            "This runtime bootstrap receipt proves the exported-folder distributed mode crossed into one explicit multi-rank Rust bootstrap attempt with per-rank GPU binding and distributed-group initialization. It does not claim that the repo already owns real 8xH100 train-step, validation, or artifact-export closure.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    fs::write(
        &receipt_path,
        format!("{}\n", serde_json::to_string_pretty(&receipt)?),
    )
    .map_err(
        |error| ParameterGolfDistributed8xH100RuntimeBootstrapError::Write {
            path: receipt_path.display().to_string(),
            error,
        },
    )?;
    if let Some(writer) = live_visualization_writer.as_deref_mut() {
        writer.record_event(
            if disposition == ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::Bootstrapped {
                crate::RemoteTrainingEventSeverity::Info
            } else {
                crate::RemoteTrainingEventSeverity::Warning
            },
            "runtime_bootstrap_result",
            format!(
                "Distributed bootstrap finished with disposition {:?} and successful_rank_count={}.",
                disposition, successful_rank_count
            ),
        )?;
    }
    Ok(receipt)
}

/// Executes one child rank for the real distributed runtime bootstrap attempt.
pub fn execute_parameter_golf_distributed_8xh100_runtime_bootstrap_child(
    output_run_id: &str,
) -> Result<
    ParameterGolfDistributed8xH100RuntimeBootstrapRankReceipt,
    ParameterGolfDistributed8xH100RuntimeBootstrapError,
> {
    let rank = parse_env_usize(CHILD_RANK_ENV_VAR)?;
    let local_rank = parse_env_usize(CHILD_LOCAL_RANK_ENV_VAR)?;
    let world_size = parse_env_usize(CHILD_WORLD_SIZE_ENV_VAR)?;
    let receipt_path = PathBuf::from(required_env(CHILD_RECEIPT_PATH_ENV_VAR)?);
    let log_path = required_env(CHILD_LOG_PATH_ENV_VAR)?;
    let cuda_visible_devices = env::var("CUDA_VISIBLE_DEVICES").unwrap_or_default();
    let backend = CudaBackend::new();
    let (observed_cuda_health, observed_cuda_devices) = match backend.discovery_report() {
        Ok(report) => (report.health, report.devices),
        Err(error) => (
            RuntimeHealth {
                status: HealthStatus::Offline,
                message: error.to_string(),
            },
            Vec::new(),
        ),
    };
    let (observed_cuda_health, observed_cuda_devices) = apply_cuda_visibility_contract(
        observed_cuda_health,
        observed_cuda_devices,
        &cuda_visible_devices,
    )?;
    let thresholds = inspect_local_distributed_8xh100_machine().thresholds;
    let matching_visible_h100_device_count = observed_cuda_devices
        .iter()
        .filter(|device| device_matches_distributed_h100(device, &thresholds))
        .count();

    let local_node_id = bootstrap_node_id(rank);
    let (runtime_prerequisites_satisfied, distributed_group, refusal) = if world_size
        != CHALLENGE_WORLD_SIZE
    {
        (
                false,
                None,
                Some(
                    PsionicRefusal::new(
                        PsionicRefusalCode::UnsupportedBackendCapability,
                        PsionicRefusalScope::Runtime,
                        format!(
                            "distributed 8xH100 runtime bootstrap requires world_size={} but rank {rank} observed world_size={world_size}",
                            CHALLENGE_WORLD_SIZE
                        ),
                    )
                    .with_subject(String::from(
                        "parameter_golf_distributed_8xh100_runtime_bootstrap_rank",
                    )),
                ),
            )
    } else if local_rank != rank {
        (
                false,
                None,
                Some(
                    PsionicRefusal::new(
                        PsionicRefusalCode::TopologyMismatch,
                        PsionicRefusalScope::Runtime,
                        format!(
                            "distributed 8xH100 runtime bootstrap requires local_rank == rank on the single-pod lane, found rank={rank} local_rank={local_rank}"
                        ),
                    )
                    .with_subject(String::from(
                        "parameter_golf_distributed_8xh100_runtime_bootstrap_rank",
                    )),
                ),
            )
    } else if cuda_visible_devices.trim().is_empty() {
        (
                false,
                None,
                Some(
                    PsionicRefusal::new(
                        PsionicRefusalCode::UnsupportedBackendCapability,
                        PsionicRefusalScope::Runtime,
                        String::from(
                            "distributed 8xH100 runtime bootstrap requires explicit CUDA_VISIBLE_DEVICES assignment for each rank",
                        ),
                    )
                    .with_subject(String::from(
                        "parameter_golf_distributed_8xh100_runtime_bootstrap_rank",
                    )),
                ),
            )
    } else if observed_cuda_health.status == HealthStatus::Offline {
        (
                false,
                None,
                Some(
                    PsionicRefusal::new(
                        PsionicRefusalCode::UnsupportedBackendCapability,
                        PsionicRefusalScope::Runtime,
                        format!(
                            "distributed 8xH100 runtime bootstrap rank {rank} could not discover CUDA devices: {}",
                            observed_cuda_health.message
                        ),
                    )
                    .with_subject(String::from(
                        "parameter_golf_distributed_8xh100_runtime_bootstrap_rank",
                    )),
                ),
            )
    } else if matching_visible_h100_device_count != 1 {
        (
                false,
                None,
                Some(
                    PsionicRefusal::new(
                        PsionicRefusalCode::UnsupportedBackendCapability,
                        PsionicRefusalScope::Runtime,
                        format!(
                            "distributed 8xH100 runtime bootstrap rank {rank} requires exactly one visible H100 after CUDA_VISIBLE_DEVICES binding but found {matching_visible_h100_device_count} matching device(s)"
                        ),
                    )
                    .with_subject(String::from(
                        "parameter_golf_distributed_8xh100_runtime_bootstrap_rank",
                    )),
                ),
            )
    } else {
        (true, Some(build_bootstrap_group_snapshot(local_rank)), None)
    };

    let mut receipt = ParameterGolfDistributed8xH100RuntimeBootstrapRankReceipt {
        schema_version: 1,
        run_id: output_run_id.to_string(),
        rank,
        local_rank,
        world_size,
        local_node_id,
        cuda_visible_devices,
        log_path,
        observed_cuda_health,
        observed_cuda_devices,
        matching_visible_h100_device_count,
        requested_backend: BOOTSTRAP_REQUESTED_BACKEND,
        runtime_prerequisites_satisfied,
        distributed_group,
        refusal,
        claim_boundary: String::from(
            "This child receipt proves one rank-local GPU binding and public distributed-group bootstrap attempt inside the shipped distributed runtime. It does not claim later train-step or validation execution.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfDistributed8xH100RuntimeBootstrapError::Write {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(
        &receipt_path,
        format!("{}\n", serde_json::to_string_pretty(&receipt)?),
    )
    .map_err(
        |error| ParameterGolfDistributed8xH100RuntimeBootstrapError::Write {
            path: receipt_path.display().to_string(),
            error,
        },
    )?;
    Ok(receipt)
}

fn parse_env_usize(
    key: &'static str,
) -> Result<usize, ParameterGolfDistributed8xH100RuntimeBootstrapError> {
    let value = required_env(key)?;
    value
        .parse::<usize>()
        .map_err(|_| ParameterGolfDistributed8xH100RuntimeBootstrapError::InvalidEnv { key, value })
}

fn apply_cuda_visibility_contract(
    observed_cuda_health: RuntimeHealth,
    observed_cuda_devices: Vec<DeviceDescriptor>,
    cuda_visible_devices: &str,
) -> Result<
    (RuntimeHealth, Vec<DeviceDescriptor>),
    ParameterGolfDistributed8xH100RuntimeBootstrapError,
> {
    let visible_ordinals = parse_cuda_visible_device_ordinals(cuda_visible_devices)?;
    if visible_ordinals.is_empty() || observed_cuda_devices.len() <= visible_ordinals.len() {
        return Ok((observed_cuda_health, observed_cuda_devices));
    }
    let raw_discovered_device_count = observed_cuda_devices.len();
    let observed_cuda_devices = observed_cuda_devices
        .into_iter()
        .filter(|device| visible_ordinals.contains(&usize::from(device.device.ordinal())))
        .collect::<Vec<_>>();
    let observed_cuda_health = if observed_cuda_health.status == HealthStatus::Offline {
        observed_cuda_health
    } else {
        RuntimeHealth {
            status: observed_cuda_health.status,
            message: format!(
                "cuda visibility contract retained {} device(s) from {} discovered device(s)",
                observed_cuda_devices.len(),
                raw_discovered_device_count
            ),
        }
    };
    Ok((observed_cuda_health, observed_cuda_devices))
}

fn parse_cuda_visible_device_ordinals(
    cuda_visible_devices: &str,
) -> Result<Vec<usize>, ParameterGolfDistributed8xH100RuntimeBootstrapError> {
    cuda_visible_devices
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value.parse::<usize>().map_err(|_| {
                ParameterGolfDistributed8xH100RuntimeBootstrapError::InvalidEnv {
                    key: "CUDA_VISIBLE_DEVICES",
                    value: cuda_visible_devices.to_string(),
                }
            })
        })
        .collect()
}

fn required_env(
    key: &'static str,
) -> Result<String, ParameterGolfDistributed8xH100RuntimeBootstrapError> {
    env::var(key)
        .map_err(|_| ParameterGolfDistributed8xH100RuntimeBootstrapError::MissingEnv { key })
}

fn build_bootstrap_group_snapshot(
    local_rank: usize,
) -> ParameterGolfDistributed8xH100RuntimeBootstrapGroupSnapshot {
    let member_node_ids = (0..CHALLENGE_WORLD_SIZE)
        .map(bootstrap_node_id)
        .collect::<Vec<_>>();
    let membership = TrainingElasticMembershipContext::new(
        1,
        BOOTSTRAP_CLUSTER_STATE_DIGEST,
        BOOTSTRAP_TOPOLOGY_DIGEST,
        member_node_ids.clone(),
    );
    let mesh = TrainingDeviceMeshContext::new(
        BOOTSTRAP_MESH_ID,
        1,
        "cuda",
        ClusterCommunicationClass::TensorCollectiveMesh,
        membership,
        member_node_ids.clone(),
    )
    .with_axes(vec![TrainingDeviceMeshAxis::new(
        "dp",
        TrainingDeviceMeshAxisKind::DataParallel,
        CHALLENGE_WORLD_SIZE,
    )
    .with_collective_group_size(CHALLENGE_WORLD_SIZE)
    .with_detail("parameter golf public distributed data-parallel axis")]);
    let members = member_node_ids
        .iter()
        .enumerate()
        .map(
            |(rank, node_id)| ParameterGolfDistributed8xH100RuntimeBootstrapGroupMember {
                node_id: node_id.clone(),
                rank,
                shard_id: rank,
                device_label: format!("cuda:{rank}"),
            },
        )
        .collect::<Vec<_>>();
    let local_node_id = bootstrap_node_id(local_rank);
    let group_id = stable_digest(
        b"psionic_parameter_golf_distributed_8xh100_runtime_group_snapshot|",
        &(
            BOOTSTRAP_REQUESTED_BACKEND,
            &mesh,
            ClusterTransportClass::TrustedLanStream,
            &members,
        ),
    );
    ParameterGolfDistributed8xH100RuntimeBootstrapGroupSnapshot {
        group_id,
        requested_backend: BOOTSTRAP_REQUESTED_BACKEND,
        effective_backend: String::from("cuda"),
        communication_class: ClusterCommunicationClass::TensorCollectiveMesh,
        transport: ClusterTransportClass::TrustedLanStream,
        mesh,
        local_node_id,
        rank: local_rank,
        size: CHALLENGE_WORLD_SIZE,
        members,
    }
}

fn bootstrap_node_id(rank: usize) -> String {
    format!("{BOOTSTRAP_NODE_ID_PREFIX}-{rank}")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_cuda_visibility_contract, bootstrap_node_id, build_bootstrap_group_snapshot,
        parameter_golf_distributed_8xh100_runtime_bootstrap_rank_logs_dir,
        parameter_golf_distributed_8xh100_runtime_bootstrap_rank_receipts_dir,
        parameter_golf_distributed_8xh100_runtime_bootstrap_receipt_path,
        ParameterGolfDistributed8xH100RuntimeRequestedBackend, CHALLENGE_WORLD_SIZE,
    };
    use crate::{
        PARAMETER_GOLF_DISTRIBUTED_8XH100_BRINGUP_REPORT_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_LOGS_DIR_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_RECEIPTS_DIR_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RECEIPT_ARTIFACT_REF,
    };
    use psionic_core::{DType, Device, DeviceKind};
    use psionic_runtime::ClusterCommunicationClass;
    use psionic_runtime::{
        DeviceDescriptor, HealthStatus, NvidiaDeviceMetadata, NvidiaRecoveryAction,
        NvidiaRecoveryProfile, NvidiaRiskLevel, NvidiaRiskProfile, NvidiaTopologyInfo,
        RuntimeHealth,
    };

    fn sample_h100_device(ordinal: usize) -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(
                DeviceKind::Cuda,
                ordinal as u16,
                Some(format!("cuda:{ordinal}")),
            ),
            device_name: Some(String::from("NVIDIA H100 80GB HBM3")),
            supported_dtypes: vec![DType::F32, DType::BF16],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(80 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: Some(NvidiaDeviceMetadata {
                topology: NvidiaTopologyInfo {
                    architecture: Some(String::from("hopper")),
                    compute_capability: Some(String::from("9.0")),
                    pci_bdf: Some(format!("00000000:{:02x}:00.0", ordinal + 1)),
                    sm_count: Some(132),
                    vram_bytes: Some(80 * 1024 * 1024 * 1024),
                    mig_profile: None,
                },
                risk: NvidiaRiskProfile {
                    level: NvidiaRiskLevel::Standard,
                    display_attached: Some(false),
                    mig_partitioned: false,
                    warnings: Vec::new(),
                },
                recovery: NvidiaRecoveryProfile {
                    supports_gpu_reset: Some(true),
                    expected_actions: vec![
                        NvidiaRecoveryAction::ProcessRestart,
                        NvidiaRecoveryAction::GpuReset,
                        NvidiaRecoveryAction::RebootHost,
                    ],
                },
            }),
        }
    }

    #[test]
    fn distributed_runtime_bootstrap_paths_follow_submission_contract() {
        let root = std::path::Path::new("/tmp/exported");
        let receipt_path = parameter_golf_distributed_8xh100_runtime_bootstrap_receipt_path(
            root,
            PARAMETER_GOLF_DISTRIBUTED_8XH100_BRINGUP_REPORT_ARTIFACT_REF,
        );
        assert_eq!(
            receipt_path,
            root.join(PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RECEIPT_ARTIFACT_REF)
        );
        let receipt_dir = parameter_golf_distributed_8xh100_runtime_bootstrap_rank_receipts_dir(
            root,
            PARAMETER_GOLF_DISTRIBUTED_8XH100_BRINGUP_REPORT_ARTIFACT_REF,
        );
        assert_eq!(
            receipt_dir,
            root.join(
                PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_RECEIPTS_DIR_ARTIFACT_REF
            )
        );
        let log_dir = parameter_golf_distributed_8xh100_runtime_bootstrap_rank_logs_dir(
            root,
            PARAMETER_GOLF_DISTRIBUTED_8XH100_BRINGUP_REPORT_ARTIFACT_REF,
        );
        assert_eq!(
            log_dir,
            root.join(
                PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_LOGS_DIR_ARTIFACT_REF
            )
        );
    }

    #[test]
    fn distributed_runtime_bootstrap_payload_matches_public_world_size() {
        let bootstrap = build_bootstrap_group_snapshot(3);
        assert_eq!(bootstrap.local_node_id, bootstrap_node_id(3));
        assert_eq!(bootstrap.members.len(), CHALLENGE_WORLD_SIZE);
        assert_eq!(bootstrap.mesh.member_node_ids.len(), CHALLENGE_WORLD_SIZE);
        assert_eq!(bootstrap.mesh.axes.len(), 1);
        assert_eq!(bootstrap.mesh.axes[0].axis_id, "dp");
    }

    #[test]
    fn distributed_runtime_bootstrap_payload_preserves_public_nccl_contract() {
        let group = build_bootstrap_group_snapshot(2);
        assert_eq!(group.rank, 2);
        assert_eq!(group.size, CHALLENGE_WORLD_SIZE);
        assert_eq!(
            group.requested_backend,
            ParameterGolfDistributed8xH100RuntimeRequestedBackend::Nccl
        );
        assert_eq!(
            group.communication_class,
            ClusterCommunicationClass::TensorCollectiveMesh
        );
        assert_eq!(group.mesh.member_node_ids[2], bootstrap_node_id(2));
    }

    #[test]
    fn distributed_runtime_bootstrap_group_id_is_shared_across_ranks() {
        let rank_zero = build_bootstrap_group_snapshot(0);
        let rank_seven = build_bootstrap_group_snapshot(7);
        assert_eq!(rank_zero.group_id, rank_seven.group_id);
        assert_ne!(rank_zero.local_node_id, rank_seven.local_node_id);
        assert_eq!(rank_zero.mesh, rank_seven.mesh);
        assert_eq!(rank_zero.members, rank_seven.members);
    }

    #[test]
    fn distributed_runtime_bootstrap_filters_discovery_through_cuda_visible_devices() {
        let devices = (0..CHALLENGE_WORLD_SIZE)
            .map(sample_h100_device)
            .collect::<Vec<_>>();
        let (health, devices) = apply_cuda_visibility_contract(
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("cuda ready on 8 NVIDIA device(s)"),
            },
            devices,
            "3",
        )
        .expect("visibility contract should parse");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device.ordinal(), 3);
        assert_eq!(health.status, HealthStatus::Ready);
        assert!(health
            .message
            .contains("retained 1 device(s) from 8 discovered device(s)"));
    }
}
