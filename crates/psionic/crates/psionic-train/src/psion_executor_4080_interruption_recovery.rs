use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionExecutor4080SmokeRunPacket, PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH,
    PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH,
};

/// Stable schema version for the admitted 4080 interruption-recovery packet.
pub const PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_SCHEMA_VERSION: &str =
    "psion.executor.4080_interruption_recovery.v1";
/// Canonical fixture path for the admitted 4080 interruption-recovery packet.
pub const PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_4080_interruption_recovery_v1.json";
/// Canonical doc path for the admitted 4080 interruption-recovery packet.
pub const PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY.md";

const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const LOCAL_TAILNET_CONTROL_PROFILE_ID: &str = "local_tailnet_cluster_control_plane";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const FAILURE_DRILL_REPORT_PATH: &str =
    "fixtures/swarm/reports/first_swarm_trusted_lan_failure_drills_v1.json";
const TAILNET_COORDINATOR_REPORT_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/coordinator_runtime_report.json";
const TAILNET_CONTRIBUTOR_REPORT_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/contributor_runtime_report.json";
const EXPECTED_RUN_ID: &str = "tailrun-home-admitted-20260328k";
const EXPECTED_RUN_FAMILY_ID: &str = "swarm.local.mlx_metal_plus_rtx4080.open_adapter.v1";
const EXPECTED_LINUX_WORKER_ID: &str = "swarm-linux-4080-a";
const EXPECTED_LINUX_ROLE_ID: &str = "swarm.linux.cuda.rtx4080.contributor";

#[derive(Clone, Debug, Deserialize)]
struct FailureDrillBundle {
    run_family_id: String,
    bundle_digest: String,
    drills: Vec<FailureDrill>,
}

#[derive(Clone, Debug, Deserialize)]
struct FailureDrill {
    drill_id: String,
    drill_kind: String,
    validator_disposition: String,
    departure_reason: Option<String>,
    suspension_reason: Option<String>,
    expected_upload_manifest_digest: Option<String>,
    observed_upload_manifest_digest: Option<String>,
    stale_after_ms: Option<u64>,
    observed_worker_skew_ms: Option<u64>,
    disposition: String,
    aggregation_blocked: bool,
    replay_required: bool,
    operator_action: String,
    detail: String,
    drill_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CoordinatorRuntimeReport {
    run_id: String,
    run_family_id: String,
    window_plan: WindowPlan,
    heartbeat_receipts: Vec<HeartbeatReceipt>,
    submission_receipts: Vec<SubmissionReceipt>,
    validator_summary: ValidatorSummary,
    replay_receipt_digests: Vec<String>,
    report_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct WindowPlan {
    dataset_slices: Vec<DatasetSlice>,
    input_checkpoint_pointer: InputCheckpointPointer,
}

#[derive(Clone, Debug, Deserialize)]
struct DatasetSlice {
    dataset_id: String,
    split_name: String,
    slice_id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct InputCheckpointPointer {
    checkpoint_family: String,
    checkpoint: CheckpointReceipt,
    pointer_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CheckpointReceipt {
    checkpoint_ref: String,
    step: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct HeartbeatReceipt {
    worker_id: String,
    progress: Option<HeartbeatProgress>,
    receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct HeartbeatProgress {
    completed_steps: u64,
    processed_samples: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct SubmissionReceipt {
    worker_id: String,
    upload: UploadRecord,
    execution_summary: ExecutionSummary,
    receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct UploadRecord {
    upload_manifest_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ExecutionSummary {
    local_step_count: u64,
    sample_count: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct ValidatorSummary {
    summary_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ContributorRuntimeReport {
    run_id: String,
    run_family_id: String,
    node_id: String,
    local_contribution: LocalContribution,
    report_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct LocalContribution {
    session_id: String,
    role_id: String,
    contributor_receipt: ContributorReceipt,
    execution_summary: ExecutionSummary,
}

#[derive(Clone, Debug, Deserialize)]
struct ContributorReceipt {
    manifest: ContributorManifest,
    checkpoint_family: String,
    receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ContributorManifest {
    replay_policy_id: String,
    shared_replay_identity_digest: String,
}

/// One retained recovery-check row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080InterruptionRecoveryChecklistRow {
    /// Stable checklist id.
    pub checklist_id: String,
    /// Final status.
    pub status: String,
    /// Honest detail.
    pub detail: String,
}

/// Typed packet binding admitted recovery policy and restore evidence for the 4080 lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutor4080InterruptionRecoveryPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted 4080 worker profile id.
    pub worker_profile_id: String,
    /// Admitted Tailnet control-plane profile id.
    pub control_plane_profile_id: String,
    /// Prerequisite smoke packet reference.
    pub smoke_packet_ref: String,
    /// Stable SHA256 over the smoke packet bytes.
    pub smoke_packet_sha256: String,
    /// Failure-drill bundle reference.
    pub failure_drill_bundle_ref: String,
    /// Stable SHA256 over the failure-drill bundle bytes.
    pub failure_drill_bundle_sha256: String,
    /// In-band failure-drill bundle digest.
    pub failure_drill_bundle_digest: String,
    /// Retained coordinator report reference.
    pub coordinator_report_ref: String,
    /// Stable SHA256 over the retained coordinator report bytes.
    pub coordinator_report_sha256: String,
    /// Retained contributor report reference.
    pub contributor_report_ref: String,
    /// Stable SHA256 over the retained contributor report bytes.
    pub contributor_report_sha256: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable run family id.
    pub run_family_id: String,
    /// Linux worker id under review.
    pub worker_id: String,
    /// Linux worker session id under review.
    pub worker_session_id: String,
    /// Linux worker role id under review.
    pub worker_role_id: String,
    /// Dataset id replayed on interruption.
    pub dataset_id: String,
    /// Dataset split replayed on interruption.
    pub dataset_split_name: String,
    /// Linux worker slice id replayed on interruption.
    pub worker_slice_id: String,
    /// Replay policy id retained by the contributor receipt.
    pub replay_policy_id: String,
    /// Shared replay identity digest retained by the contributor receipt.
    pub shared_replay_identity_digest: String,
    /// Checkpoint family retained by the recovery packet.
    pub checkpoint_family: String,
    /// Checkpoint pointer digest retained by the recovery packet.
    pub checkpoint_pointer_digest: String,
    /// Checkpoint ref retained by the recovery packet.
    pub checkpoint_ref: String,
    /// Checkpoint step retained by the recovery packet.
    pub checkpoint_step: u64,
    /// Stale-worker drill digest.
    pub stale_worker_drill_digest: String,
    /// Stale-worker timeout in milliseconds.
    pub stale_worker_timeout_ms: u64,
    /// Stale-worker validator disposition.
    pub stale_worker_validator_disposition: String,
    /// Whether stale worker blocks aggregation.
    pub stale_worker_aggregation_blocked: bool,
    /// Whether stale worker requires replay.
    pub stale_worker_replay_required: bool,
    /// Contributor-loss drill digest.
    pub contributor_loss_drill_digest: String,
    /// Contributor-loss validator disposition.
    pub contributor_loss_validator_disposition: String,
    /// Upload-disagreement drill digest.
    pub upload_disagreement_drill_digest: String,
    /// Upload-disagreement validator disposition.
    pub upload_disagreement_validator_disposition: String,
    /// Expected upload manifest digest retained by the upload-disagreement drill.
    pub upload_disagreement_expected_manifest_digest: String,
    /// Observed drifted upload manifest digest retained by the upload-disagreement drill.
    pub upload_disagreement_observed_manifest_digest: String,
    /// Uneven-worker-speed drill digest.
    pub uneven_worker_speed_drill_digest: String,
    /// Uneven-worker-speed disposition.
    pub uneven_worker_speed_disposition: String,
    /// Observed skew in the uneven-worker-speed drill.
    pub uneven_worker_speed_observed_skew_ms: u64,
    /// Live upload manifest digest retained by the Linux submission receipt.
    pub live_linux_upload_manifest_digest: String,
    /// Maximum unreported steps between retained Linux progress checkpoints.
    pub max_unreported_progress_steps: u64,
    /// Maximum unreported samples between retained Linux progress checkpoints.
    pub max_unreported_progress_samples: u64,
    /// Maximum replay loss steps allowed by the single-window recovery policy.
    pub max_replay_loss_steps: u64,
    /// Maximum replay loss samples allowed by the single-window recovery policy.
    pub max_replay_loss_samples: u64,
    /// Linux progress heartbeat receipt digests kept as restore evidence.
    pub linux_progress_heartbeat_receipt_digests: Vec<String>,
    /// Linux submission receipt digest kept as restore evidence.
    pub linux_submission_receipt_digest: String,
    /// Linux contributor receipt digest kept as restore evidence.
    pub linux_contributor_receipt_digest: String,
    /// Validator summary digest kept as restore evidence.
    pub validator_summary_digest: String,
    /// Replay receipt digests kept as restore evidence.
    pub replay_receipt_digests: Vec<String>,
    /// Coordinator report digest kept as restore evidence.
    pub coordinator_report_digest: String,
    /// Contributor report digest kept as restore evidence.
    pub contributor_report_digest: String,
    /// Retained checklist rows.
    pub checklist_rows: Vec<PsionExecutor4080InterruptionRecoveryChecklistRow>,
    /// Support references.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutor4080InterruptionRecoveryPacket {
    /// Validate the retained interruption-recovery packet.
    pub fn validate(&self) -> Result<(), PsionExecutor4080InterruptionRecoveryError> {
        for (field, value) in [
            (
                "psion_executor_4080_interruption_recovery.schema_version",
                self.schema_version.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.worker_profile_id",
                self.worker_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.control_plane_profile_id",
                self.control_plane_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.smoke_packet_ref",
                self.smoke_packet_ref.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.smoke_packet_sha256",
                self.smoke_packet_sha256.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.failure_drill_bundle_ref",
                self.failure_drill_bundle_ref.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.failure_drill_bundle_sha256",
                self.failure_drill_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.failure_drill_bundle_digest",
                self.failure_drill_bundle_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.coordinator_report_ref",
                self.coordinator_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.coordinator_report_sha256",
                self.coordinator_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.contributor_report_ref",
                self.contributor_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.contributor_report_sha256",
                self.contributor_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.run_family_id",
                self.run_family_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.worker_id",
                self.worker_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.worker_session_id",
                self.worker_session_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.worker_role_id",
                self.worker_role_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.dataset_id",
                self.dataset_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.dataset_split_name",
                self.dataset_split_name.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.worker_slice_id",
                self.worker_slice_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.replay_policy_id",
                self.replay_policy_id.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.shared_replay_identity_digest",
                self.shared_replay_identity_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.checkpoint_family",
                self.checkpoint_family.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.checkpoint_pointer_digest",
                self.checkpoint_pointer_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.checkpoint_ref",
                self.checkpoint_ref.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.stale_worker_drill_digest",
                self.stale_worker_drill_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.stale_worker_validator_disposition",
                self.stale_worker_validator_disposition.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.contributor_loss_drill_digest",
                self.contributor_loss_drill_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.contributor_loss_validator_disposition",
                self.contributor_loss_validator_disposition.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.upload_disagreement_drill_digest",
                self.upload_disagreement_drill_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.upload_disagreement_validator_disposition",
                self.upload_disagreement_validator_disposition.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.upload_disagreement_expected_manifest_digest",
                self.upload_disagreement_expected_manifest_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.upload_disagreement_observed_manifest_digest",
                self.upload_disagreement_observed_manifest_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.uneven_worker_speed_drill_digest",
                self.uneven_worker_speed_drill_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.uneven_worker_speed_disposition",
                self.uneven_worker_speed_disposition.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.live_linux_upload_manifest_digest",
                self.live_linux_upload_manifest_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.linux_submission_receipt_digest",
                self.linux_submission_receipt_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.linux_contributor_receipt_digest",
                self.linux_contributor_receipt_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.validator_summary_digest",
                self.validator_summary_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.coordinator_report_digest",
                self.coordinator_report_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.contributor_report_digest",
                self.contributor_report_digest.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_4080_interruption_recovery.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.schema_version != PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_SCHEMA_VERSION {
            return Err(
                PsionExecutor4080InterruptionRecoveryError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        if self.worker_profile_id != LOCAL_4080_PROFILE_ID {
            return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_interruption_recovery.worker_profile_id",
                ),
                detail: String::from("worker profile id drifted"),
            });
        }
        if self.control_plane_profile_id != LOCAL_TAILNET_CONTROL_PROFILE_ID {
            return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_interruption_recovery.control_plane_profile_id",
                ),
                detail: String::from("control-plane profile id drifted"),
            });
        }
        if self.stale_worker_timeout_ms != 5000 {
            return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_interruption_recovery.stale_worker_timeout_ms",
                ),
                detail: String::from("stale-worker timeout must stay frozen at five seconds"),
            });
        }
        if !self.stale_worker_aggregation_blocked || !self.stale_worker_replay_required {
            return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_interruption_recovery.stale_worker_replay_required",
                ),
                detail: String::from(
                    "stale worker must keep aggregation blocked and replay required",
                ),
            });
        }
        if self.max_unreported_progress_steps > self.max_replay_loss_steps
            || self.max_unreported_progress_samples > self.max_replay_loss_samples
        {
            return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_interruption_recovery.max_unreported_progress_steps",
                ),
                detail: String::from("unreported progress cannot exceed replay-loss policy"),
            });
        }
        if self.linux_progress_heartbeat_receipt_digests.is_empty()
            || self.replay_receipt_digests.is_empty()
            || self.checklist_rows.len() != 3
            || self.support_refs.is_empty()
        {
            return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
                field: String::from("psion_executor_4080_interruption_recovery.evidence"),
                detail: String::from(
                    "recovery packet must keep heartbeat, replay, checklist, and support evidence explicit",
                ),
            });
        }
        if self.packet_digest != stable_interruption_recovery_packet_digest(self) {
            return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
                field: String::from("psion_executor_4080_interruption_recovery.packet_digest"),
                detail: String::from("packet digest drifted"),
            });
        }
        Ok(())
    }
}

/// Errors emitted by the retained 4080 interruption-recovery packet.
#[derive(Debug, Error)]
pub enum PsionExecutor4080InterruptionRecoveryError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, got `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("failed to read `{path}`: {error}")]
    Read {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        #[source]
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to encode interruption-recovery packet: {0}")]
    Encode(#[from] serde_json::Error),
}

/// Build the retained 4080 interruption-recovery packet.
pub fn builtin_executor_4080_interruption_recovery_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080InterruptionRecoveryPacket, PsionExecutor4080InterruptionRecoveryError>
{
    let smoke_packet_path = workspace_root.join(PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH);
    let smoke_packet_bytes = fs::read(&smoke_packet_path).map_err(|error| {
        PsionExecutor4080InterruptionRecoveryError::Read {
            path: smoke_packet_path.display().to_string(),
            error,
        }
    })?;
    let smoke_packet: PsionExecutor4080SmokeRunPacket =
        serde_json::from_slice(&smoke_packet_bytes).map_err(|error| {
            PsionExecutor4080InterruptionRecoveryError::Decode {
                path: smoke_packet_path.display().to_string(),
                error,
            }
        })?;
    let failure_drill_path = workspace_root.join(FAILURE_DRILL_REPORT_PATH);
    let failure_drill_bytes = fs::read(&failure_drill_path).map_err(|error| {
        PsionExecutor4080InterruptionRecoveryError::Read {
            path: failure_drill_path.display().to_string(),
            error,
        }
    })?;
    let failure_drills: FailureDrillBundle =
        serde_json::from_slice(&failure_drill_bytes).map_err(|error| {
            PsionExecutor4080InterruptionRecoveryError::Decode {
                path: failure_drill_path.display().to_string(),
                error,
            }
        })?;
    let coordinator_report_path = workspace_root.join(TAILNET_COORDINATOR_REPORT_PATH);
    let coordinator_report_bytes =
        fs::read(&coordinator_report_path).map_err(|error| {
            PsionExecutor4080InterruptionRecoveryError::Read {
                path: coordinator_report_path.display().to_string(),
                error,
            }
        })?;
    let coordinator_report: CoordinatorRuntimeReport =
        serde_json::from_slice(&coordinator_report_bytes).map_err(|error| {
            PsionExecutor4080InterruptionRecoveryError::Decode {
                path: coordinator_report_path.display().to_string(),
                error,
            }
        })?;
    let contributor_report_path = workspace_root.join(TAILNET_CONTRIBUTOR_REPORT_PATH);
    let contributor_report_bytes =
        fs::read(&contributor_report_path).map_err(|error| {
            PsionExecutor4080InterruptionRecoveryError::Read {
                path: contributor_report_path.display().to_string(),
                error,
            }
        })?;
    let contributor_report: ContributorRuntimeReport =
        serde_json::from_slice(&contributor_report_bytes).map_err(|error| {
            PsionExecutor4080InterruptionRecoveryError::Decode {
                path: contributor_report_path.display().to_string(),
                error,
            }
        })?;

    if smoke_packet.run_id != EXPECTED_RUN_ID
        || coordinator_report.run_id != EXPECTED_RUN_ID
        || contributor_report.run_id != EXPECTED_RUN_ID
    {
        return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
            field: String::from("psion_executor_4080_interruption_recovery.run_id"),
            detail: String::from("smoke packet and retained runtime reports must stay aligned"),
        });
    }
    if smoke_packet.run_family_id != EXPECTED_RUN_FAMILY_ID
        || failure_drills.run_family_id != EXPECTED_RUN_FAMILY_ID
        || coordinator_report.run_family_id != EXPECTED_RUN_FAMILY_ID
        || contributor_report.run_family_id != EXPECTED_RUN_FAMILY_ID
    {
        return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
            field: String::from("psion_executor_4080_interruption_recovery.run_family_id"),
            detail: String::from("retained recovery sources drifted off the admitted run family"),
        });
    }
    if contributor_report.node_id != EXPECTED_LINUX_WORKER_ID
        || contributor_report.local_contribution.role_id != EXPECTED_LINUX_ROLE_ID
    {
        return Err(PsionExecutor4080InterruptionRecoveryError::InvalidValue {
            field: String::from("psion_executor_4080_interruption_recovery.worker_id"),
            detail: String::from("contributor report no longer matches the admitted Linux worker"),
        });
    }

    let stale_worker_drill = failure_drills
        .drills
        .iter()
        .find(|drill| drill.drill_kind == "stale_worker")
        .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
            field: String::from(
                "psion_executor_4080_interruption_recovery.stale_worker_drill",
            ),
        })?;
    let contributor_loss_drill = failure_drills
        .drills
        .iter()
        .find(|drill| drill.drill_kind == "contributor_loss")
        .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
            field: String::from(
                "psion_executor_4080_interruption_recovery.contributor_loss_drill",
            ),
        })?;
    let upload_disagreement_drill = failure_drills
        .drills
        .iter()
        .find(|drill| drill.drill_kind == "upload_disagreement")
        .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
            field: String::from(
                "psion_executor_4080_interruption_recovery.upload_disagreement_drill",
            ),
        })?;
    let uneven_worker_speed_drill = failure_drills
        .drills
        .iter()
        .find(|drill| drill.drill_kind == "uneven_worker_speed")
        .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
            field: String::from(
                "psion_executor_4080_interruption_recovery.uneven_worker_speed_drill",
            ),
        })?;

    let worker_slice = coordinator_report
        .window_plan
        .dataset_slices
        .iter()
        .find(|slice| slice.slice_id.contains("swarm_linux_cuda_rtx4080_contributor"))
        .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
            field: String::from("psion_executor_4080_interruption_recovery.worker_slice_id"),
        })?;

    let linux_progress_heartbeats = coordinator_report
        .heartbeat_receipts
        .iter()
        .filter(|row| row.worker_id == EXPECTED_LINUX_WORKER_ID)
        .filter_map(|row| {
            row.progress.as_ref().map(|progress| {
                (
                    row.receipt_digest.clone(),
                    progress.completed_steps,
                    progress.processed_samples,
                )
            })
        })
        .collect::<Vec<_>>();
    if linux_progress_heartbeats.is_empty() {
        return Err(PsionExecutor4080InterruptionRecoveryError::MissingField {
            field: String::from(
                "psion_executor_4080_interruption_recovery.linux_progress_heartbeat_receipt_digests",
            ),
        });
    }
    let mut previous_steps = 0;
    let mut previous_samples = 0;
    let mut max_unreported_progress_steps = 0;
    let mut max_unreported_progress_samples = 0;
    let mut linux_progress_heartbeat_receipt_digests = Vec::new();
    for (digest, steps, samples) in &linux_progress_heartbeats {
        max_unreported_progress_steps =
            max_unreported_progress_steps.max(steps.saturating_sub(previous_steps));
        max_unreported_progress_samples =
            max_unreported_progress_samples.max(samples.saturating_sub(previous_samples));
        previous_steps = *steps;
        previous_samples = *samples;
        linux_progress_heartbeat_receipt_digests.push(digest.clone());
    }

    let linux_submission = coordinator_report
        .submission_receipts
        .iter()
        .find(|row| row.worker_id == EXPECTED_LINUX_WORKER_ID)
        .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
            field: String::from(
                "psion_executor_4080_interruption_recovery.linux_submission_receipt_digest",
            ),
        })?;
    max_unreported_progress_steps = max_unreported_progress_steps.max(
        linux_submission
            .execution_summary
            .local_step_count
            .saturating_sub(previous_steps),
    );
    max_unreported_progress_samples = max_unreported_progress_samples.max(
        linux_submission
            .execution_summary
            .sample_count
            .saturating_sub(previous_samples),
    );

    let max_replay_loss_steps = contributor_report.local_contribution.execution_summary.local_step_count;
    let max_replay_loss_samples = contributor_report.local_contribution.execution_summary.sample_count;
    let replay_receipt_digests = coordinator_report.replay_receipt_digests.clone();

    let checklist_rows = vec![
        PsionExecutor4080InterruptionRecoveryChecklistRow {
            checklist_id: String::from("stale_worker_sla_green"),
            status: String::from("green"),
            detail: format!(
                "The admitted stale-worker drill freezes a {} ms timeout with validator disposition `{}` and replay-required aggregation block truth instead of letting worker loss disappear into a partial merge.",
                stale_worker_drill
                    .stale_after_ms
                    .expect("stale timeout must exist"),
                stale_worker_drill.validator_disposition,
            ),
        },
        PsionExecutor4080InterruptionRecoveryChecklistRow {
            checklist_id: String::from("lost_work_policy_green"),
            status: String::from("green"),
            detail: format!(
                "The retained Linux worker stays on replay policy `{}` for one admitted slice `{}`. Worst-case recovery loss is bounded to one contribution window ({} steps / {} samples), while unreported progress between heartbeat checkpoints stays bounded to {} steps / {} samples.",
                contributor_report.local_contribution.contributor_receipt.manifest.replay_policy_id,
                worker_slice.slice_id,
                max_replay_loss_steps,
                max_replay_loss_samples,
                max_unreported_progress_steps,
                max_unreported_progress_samples,
            ),
        },
        PsionExecutor4080InterruptionRecoveryChecklistRow {
            checklist_id: String::from("restore_evidence_green"),
            status: String::from("green"),
            detail: format!(
                "Restore evidence now stays explicit through checkpoint pointer `{}`, Linux progress heartbeat receipts {:?}, submission receipt `{}`, contributor receipt `{}`, and replay receipt digests {:?}.",
                smoke_packet.checkpoint_pointer_digest,
                linux_progress_heartbeat_receipt_digests,
                linux_submission.receipt_digest,
                contributor_report.local_contribution.contributor_receipt.receipt_digest,
                coordinator_report.replay_receipt_digests,
            ),
        },
    ];

    let mut packet = PsionExecutor4080InterruptionRecoveryPacket {
        schema_version: String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_4080_interruption_recovery_v1"),
        worker_profile_id: String::from(LOCAL_4080_PROFILE_ID),
        control_plane_profile_id: String::from(LOCAL_TAILNET_CONTROL_PROFILE_ID),
        smoke_packet_ref: String::from(PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH),
        smoke_packet_sha256: hex::encode(Sha256::digest(&smoke_packet_bytes)),
        failure_drill_bundle_ref: String::from(FAILURE_DRILL_REPORT_PATH),
        failure_drill_bundle_sha256: hex::encode(Sha256::digest(&failure_drill_bytes)),
        failure_drill_bundle_digest: failure_drills.bundle_digest,
        coordinator_report_ref: String::from(TAILNET_COORDINATOR_REPORT_PATH),
        coordinator_report_sha256: hex::encode(Sha256::digest(&coordinator_report_bytes)),
        contributor_report_ref: String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
        contributor_report_sha256: hex::encode(Sha256::digest(&contributor_report_bytes)),
        run_id: String::from(EXPECTED_RUN_ID),
        run_family_id: String::from(EXPECTED_RUN_FAMILY_ID),
        worker_id: String::from(EXPECTED_LINUX_WORKER_ID),
        worker_session_id: contributor_report.local_contribution.session_id,
        worker_role_id: contributor_report.local_contribution.role_id,
        dataset_id: worker_slice.dataset_id.clone(),
        dataset_split_name: worker_slice.split_name.clone(),
        worker_slice_id: worker_slice.slice_id.clone(),
        replay_policy_id: contributor_report
            .local_contribution
            .contributor_receipt
            .manifest
            .replay_policy_id
            .clone(),
        shared_replay_identity_digest: contributor_report
            .local_contribution
            .contributor_receipt
            .manifest
            .shared_replay_identity_digest
            .clone(),
        checkpoint_family: smoke_packet.checkpoint_family.clone(),
        checkpoint_pointer_digest: smoke_packet.checkpoint_pointer_digest.clone(),
        checkpoint_ref: smoke_packet.checkpoint_ref.clone(),
        checkpoint_step: smoke_packet.executed_steps,
        stale_worker_drill_digest: stale_worker_drill.drill_digest.clone(),
        stale_worker_timeout_ms: stale_worker_drill
            .stale_after_ms
            .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
                field: String::from(
                    "psion_executor_4080_interruption_recovery.stale_worker_timeout_ms",
                ),
            })?,
        stale_worker_validator_disposition: stale_worker_drill.validator_disposition.clone(),
        stale_worker_aggregation_blocked: stale_worker_drill.aggregation_blocked,
        stale_worker_replay_required: stale_worker_drill.replay_required,
        contributor_loss_drill_digest: contributor_loss_drill.drill_digest.clone(),
        contributor_loss_validator_disposition: contributor_loss_drill
            .validator_disposition
            .clone(),
        upload_disagreement_drill_digest: upload_disagreement_drill.drill_digest.clone(),
        upload_disagreement_validator_disposition: upload_disagreement_drill
            .validator_disposition
            .clone(),
        upload_disagreement_expected_manifest_digest: upload_disagreement_drill
            .expected_upload_manifest_digest
            .clone()
            .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
                field: String::from(
                    "psion_executor_4080_interruption_recovery.upload_disagreement_expected_manifest_digest",
                ),
            })?,
        upload_disagreement_observed_manifest_digest: upload_disagreement_drill
            .observed_upload_manifest_digest
            .clone()
            .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
                field: String::from(
                    "psion_executor_4080_interruption_recovery.upload_disagreement_observed_manifest_digest",
                ),
            })?,
        uneven_worker_speed_drill_digest: uneven_worker_speed_drill.drill_digest.clone(),
        uneven_worker_speed_disposition: uneven_worker_speed_drill.disposition.clone(),
        uneven_worker_speed_observed_skew_ms: uneven_worker_speed_drill
            .observed_worker_skew_ms
            .ok_or_else(|| PsionExecutor4080InterruptionRecoveryError::MissingField {
                field: String::from(
                    "psion_executor_4080_interruption_recovery.uneven_worker_speed_observed_skew_ms",
                ),
            })?,
        live_linux_upload_manifest_digest: linux_submission.upload.upload_manifest_digest.clone(),
        max_unreported_progress_steps,
        max_unreported_progress_samples,
        max_replay_loss_steps,
        max_replay_loss_samples,
        linux_progress_heartbeat_receipt_digests,
        linux_submission_receipt_digest: linux_submission.receipt_digest.clone(),
        linux_contributor_receipt_digest: contributor_report
            .local_contribution
            .contributor_receipt
            .receipt_digest
            .clone(),
        validator_summary_digest: coordinator_report.validator_summary.summary_digest,
        replay_receipt_digests: replay_receipt_digests.clone(),
        coordinator_report_digest: coordinator_report.report_digest,
        contributor_report_digest: contributor_report.report_digest,
        checklist_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH),
            String::from(FAILURE_DRILL_REPORT_PATH),
            String::from(TAILNET_COORDINATOR_REPORT_PATH),
            String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
        ],
        summary: format!(
            "The admitted 4080 lane now has one interruption-recovery packet tied to smoke run `{}`. It freezes the stale-worker timeout at {} ms, keeps replay-required and aggregation-blocked truth explicit for worker loss, bounds lost work to one admitted Linux slice `{}` ({} steps / {} samples), and records restore evidence through checkpoint pointer `{}`, Linux heartbeat receipts, submission receipt `{}`, and replay receipt digests {:?}.",
            EXPECTED_RUN_ID,
            stale_worker_drill.stale_after_ms.unwrap_or_default(),
            worker_slice.slice_id,
            max_replay_loss_steps,
            max_replay_loss_samples,
            smoke_packet.checkpoint_pointer_digest,
            linux_submission.receipt_digest,
            replay_receipt_digests,
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_interruption_recovery_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the retained 4080 interruption-recovery packet.
pub fn write_builtin_executor_4080_interruption_recovery_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080InterruptionRecoveryPacket, PsionExecutor4080InterruptionRecoveryError>
{
    let packet = builtin_executor_4080_interruption_recovery_packet(workspace_root)?;
    let path = workspace_root.join(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutor4080InterruptionRecoveryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&packet)?).map_err(|error| {
        PsionExecutor4080InterruptionRecoveryError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

fn stable_interruption_recovery_packet_digest(
    packet: &PsionExecutor4080InterruptionRecoveryPacket,
) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_4080_interruption_recovery|", &canonical)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutor4080InterruptionRecoveryError> {
    if value.trim().is_empty() {
        return Err(PsionExecutor4080InterruptionRecoveryError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_executor_4080_interruption_recovery_packet_is_valid() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = builtin_executor_4080_interruption_recovery_packet(workspace_root.as_path())
            .expect("build 4080 interruption recovery packet");
        packet
            .validate()
            .expect("validate 4080 interruption recovery packet");
        assert_eq!(packet.run_id, EXPECTED_RUN_ID);
        assert_eq!(packet.worker_id, EXPECTED_LINUX_WORKER_ID);
        assert_eq!(packet.stale_worker_timeout_ms, 5000);
        assert_eq!(packet.max_replay_loss_steps, 12);
    }

    #[test]
    fn executor_4080_interruption_recovery_fixture_matches_committed_truth() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let generated = builtin_executor_4080_interruption_recovery_packet(workspace_root.as_path())
            .expect("build 4080 interruption recovery packet");
        let fixture_path =
            workspace_root.join(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH);
        let fixture_bytes = fs::read(&fixture_path).expect("read 4080 interruption recovery fixture");
        let committed: PsionExecutor4080InterruptionRecoveryPacket =
            serde_json::from_slice(&fixture_bytes).expect("decode 4080 interruption recovery fixture");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_executor_4080_interruption_recovery_packet_persists_current_truth() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet =
            write_builtin_executor_4080_interruption_recovery_packet(workspace_root.as_path())
                .expect("write 4080 interruption recovery packet");
        packet
            .validate()
            .expect("validate written 4080 interruption recovery packet");
    }
}
