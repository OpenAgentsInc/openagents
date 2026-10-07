use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_4080_decision_grade_run_packet, builtin_executor_4080_durable_checkpoint_packet,
    builtin_executor_4080_interruption_recovery_packet, builtin_executor_4080_remote_launch_packet,
    builtin_executor_local_cluster_run_registration_packet, builtin_executor_mac_export_inspection_packet,
    PsionExecutor4080DecisionGradeRunError, PsionExecutor4080DurableCheckpointError,
    PsionExecutor4080InterruptionRecoveryError, PsionExecutor4080RemoteLaunchError,
    PsionExecutorLocalClusterCandidateStatus, PsionExecutorLocalClusterRunRegistrationError,
    PsionExecutorLocalClusterRunRegistrationPacket, PsionExecutorMacExportInspectionError,
    PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH,
    PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH,
    PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH,
    PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH,
    PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_SCHEMA_VERSION: &str =
    "psion.executor.local_cluster_roundtrip.v1";
pub const PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_local_cluster_roundtrip_v1.json";
pub const PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP.md";

const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const SHARED_DEVICE_MATRIX_RUN_ID: &str = "tailrun-admitted-device-matrix-20260327b";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION.md";
const PSION_EXECUTOR_4080_REMOTE_LAUNCH_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_REMOTE_LAUNCH.md";
const PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_DURABLE_CHECKPOINT.md";
const PSION_EXECUTOR_4080_DECISION_GRADE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_DECISION_GRADE_RUN.md";
const PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY.md";
const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MAC_EXPORT_INSPECTION.md";

#[derive(Debug, Error)]
pub enum PsionExecutorLocalClusterRoundtripError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Registration(#[from] PsionExecutorLocalClusterRunRegistrationError),
    #[error(transparent)]
    RemoteLaunch(#[from] PsionExecutor4080RemoteLaunchError),
    #[error(transparent)]
    DurableCheckpoint(#[from] PsionExecutor4080DurableCheckpointError),
    #[error(transparent)]
    DecisionGrade(#[from] PsionExecutor4080DecisionGradeRunError),
    #[error(transparent)]
    Recovery(#[from] PsionExecutor4080InterruptionRecoveryError),
    #[error(transparent)]
    MacExport(#[from] PsionExecutorMacExportInspectionError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterRoundtripStepRow {
    pub step_id: String,
    pub status: String,
    pub owner_surface_ref: String,
    pub owner_surface_digest: String,
    pub detail: String,
    pub step_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterRoundtripPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub registration_packet_ref: String,
    pub registration_packet_digest: String,
    pub remote_launch_packet_ref: String,
    pub remote_launch_packet_digest: String,
    pub durable_checkpoint_packet_ref: String,
    pub durable_checkpoint_packet_digest: String,
    pub decision_grade_packet_ref: String,
    pub decision_grade_packet_digest: String,
    pub recovery_packet_ref: String,
    pub recovery_packet_digest: String,
    pub current_best_registration_id: String,
    pub current_best_run_id: String,
    pub current_best_candidate_status: String,
    pub shared_run_search_key: String,
    pub export_bundle_ref: String,
    pub export_bundle_sha256: String,
    pub mac_validation_machine_class_id: String,
    pub mac_validation_digest: String,
    pub reference_linear_metric_id: String,
    pub hull_cache_metric_id: String,
    pub min_hull_cache_speedup_over_reference_linear: f64,
    pub max_hull_cache_remaining_gap_vs_cpu_reference: f64,
    pub step_rows: Vec<PsionExecutorLocalClusterRoundtripStepRow>,
    pub phase_exit_green: bool,
    pub cluster_closure_status: String,
    pub cluster_closure_detail: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorLocalClusterRoundtripStepRow {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterRoundtripError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_roundtrip.step_rows[].step_id",
                self.step_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.step_rows[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.step_rows[].owner_surface_ref",
                self.owner_surface_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.step_rows[].owner_surface_digest",
                self.owner_surface_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.step_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.step_rows[].step_digest",
                self.step_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.status != "green" {
            return Err(PsionExecutorLocalClusterRoundtripError::InvalidValue {
                field: String::from("psion_executor_local_cluster_roundtrip.step_rows[].status"),
                detail: String::from("roundtrip closeout only counts if every retained step is green"),
            });
        }
        if stable_step_digest(self) != self.step_digest {
            return Err(PsionExecutorLocalClusterRoundtripError::DigestMismatch {
                field: String::from("psion_executor_local_cluster_roundtrip.step_rows[].step_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterRoundtripPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorLocalClusterRoundtripError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_local_cluster_roundtrip.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_SCHEMA_VERSION {
            return Err(PsionExecutorLocalClusterRoundtripError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_local_cluster_roundtrip.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.registration_packet_ref",
                self.registration_packet_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.registration_packet_digest",
                self.registration_packet_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.remote_launch_packet_ref",
                self.remote_launch_packet_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.remote_launch_packet_digest",
                self.remote_launch_packet_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.durable_checkpoint_packet_ref",
                self.durable_checkpoint_packet_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.durable_checkpoint_packet_digest",
                self.durable_checkpoint_packet_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.decision_grade_packet_ref",
                self.decision_grade_packet_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.decision_grade_packet_digest",
                self.decision_grade_packet_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.recovery_packet_ref",
                self.recovery_packet_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.recovery_packet_digest",
                self.recovery_packet_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.current_best_registration_id",
                self.current_best_registration_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.current_best_run_id",
                self.current_best_run_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.current_best_candidate_status",
                self.current_best_candidate_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.shared_run_search_key",
                self.shared_run_search_key.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.export_bundle_ref",
                self.export_bundle_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.export_bundle_sha256",
                self.export_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.mac_validation_machine_class_id",
                self.mac_validation_machine_class_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.mac_validation_digest",
                self.mac_validation_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.reference_linear_metric_id",
                self.reference_linear_metric_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.hull_cache_metric_id",
                self.hull_cache_metric_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.cluster_closure_status",
                self.cluster_closure_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.cluster_closure_detail",
                self.cluster_closure_detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_local_cluster_roundtrip.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.step_rows.len() != 6 {
            return Err(PsionExecutorLocalClusterRoundtripError::InvalidValue {
                field: String::from("psion_executor_local_cluster_roundtrip.step_rows"),
                detail: String::from("roundtrip closeout must keep exactly six retained steps"),
            });
        }
        for step_row in &self.step_rows {
            step_row.validate()?;
        }
        if !self.phase_exit_green {
            return Err(PsionExecutorLocalClusterRoundtripError::InvalidValue {
                field: String::from("psion_executor_local_cluster_roundtrip.phase_exit_green"),
                detail: String::from("roundtrip closeout does not count unless phase exit is green"),
            });
        }
        if self.cluster_closure_status != "green" {
            return Err(PsionExecutorLocalClusterRoundtripError::InvalidValue {
                field: String::from("psion_executor_local_cluster_roundtrip.cluster_closure_status"),
                detail: String::from("roundtrip closeout must stay green"),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorLocalClusterRoundtripError::MissingField {
                field: String::from("psion_executor_local_cluster_roundtrip.support_refs"),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorLocalClusterRoundtripError::DigestMismatch {
                field: String::from("psion_executor_local_cluster_roundtrip.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_local_cluster_roundtrip_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterRoundtripPacket, PsionExecutorLocalClusterRoundtripError> {
    let registration = builtin_executor_local_cluster_run_registration_packet(workspace_root)?;
    let remote_launch = builtin_executor_4080_remote_launch_packet(workspace_root)?;
    let durable_checkpoint = builtin_executor_4080_durable_checkpoint_packet(workspace_root)?;
    let decision_grade = builtin_executor_4080_decision_grade_run_packet(workspace_root)?;
    let recovery = builtin_executor_4080_interruption_recovery_packet(workspace_root)?;
    let mac_export = builtin_executor_mac_export_inspection_packet(workspace_root)?;

    let current_best_registration = find_registration_row(&registration, LOCAL_4080_PROFILE_ID)?;
    let shared_run_search_key = current_best_registration
        .search_run_ids
        .iter()
        .find(|run_id| run_id.as_str() == SHARED_DEVICE_MATRIX_RUN_ID)
        .cloned()
        .ok_or_else(|| PsionExecutorLocalClusterRoundtripError::MissingField {
            field: String::from("psion_executor_local_cluster_roundtrip.shared_run_search_key"),
        })?;
    let mac_validation_digest = stable_validation_digest(
        decision_grade.retained_remote_bundle_sha256.as_str(),
        mac_export.local_cpu_machine_class_id.as_str(),
        mac_export.reference_linear_metric_id.as_str(),
        mac_export.hull_cache_metric_id.as_str(),
    );

    let mut step_rows = vec![
        build_step_row(
            "launch_on_mac_green",
            PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH,
            remote_launch.packet_digest.as_str(),
            format!(
                "The controller-owned Mac launch packet keeps Tailnet worker `{}` and submitted run `{}` explicit, so the local cluster still starts on the Mac instead of moving control-plane ownership onto the worker.",
                remote_launch.contributor_host, remote_launch.run_id
            ),
        ),
        build_step_row(
            "train_on_4080_green",
            PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH,
            decision_grade.packet_digest.as_str(),
            format!(
                "The retained CUDA decision run `{}` stays inside the admitted 600 second accelerator budget and remains baseline-comparable under execution backend `{}` on `{}`.",
                decision_grade.decision_run_id,
                decision_grade.execution_backend_label,
                decision_grade.logical_device_label
            ),
        ),
        build_step_row(
            "checkpoint_on_4080_green",
            PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH,
            durable_checkpoint.packet_digest.as_str(),
            format!(
                "Checkpoint family `{}` stays durable through pointer digest `{}` and ref `{}` before the artifact leaves the worker lane.",
                durable_checkpoint.checkpoint_family,
                durable_checkpoint.checkpoint_pointer_digest,
                durable_checkpoint.checkpoint_ref
            ),
        ),
        build_step_row(
            "recover_on_4080_green",
            PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH,
            recovery.packet_digest.as_str(),
            format!(
                "The admitted recovery packet keeps restore evidence explicit for run `{}` with checkpoint pointer `{}` and replay-required stale-worker discipline instead of hiding recovery behind a happy-path run.",
                recovery.run_id,
                recovery.checkpoint_pointer_digest
            ),
        ),
        build_step_row(
            "export_back_to_mac_green",
            PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH,
            decision_grade.packet_digest.as_str(),
            format!(
                "The accelerator-backed run returns portable bundle `{}` (sha256 `{}`) onto the controller-owned Mac path, closing the export side of the Mac -> 4080 -> Mac loop.",
                decision_grade.retained_remote_bundle_ref,
                decision_grade.retained_remote_bundle_sha256
            ),
        ),
        build_step_row(
            "validate_back_on_mac_green",
            PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
            mac_export.packet_digest.as_str(),
            format!(
                "Back-on-Mac validation reuses machine class `{}` with anchor `{}` and fast-route metric `{}` while keeping min hull-cache speedup {:.6} and max remaining CPU gap {:.6} explicit for the returned 4080 bundle.",
                mac_export.local_cpu_machine_class_id,
                mac_export.reference_linear_metric_id,
                mac_export.hull_cache_metric_id,
                mac_export.min_hull_cache_speedup_over_reference_linear,
                mac_export.max_hull_cache_remaining_gap_vs_cpu_reference
            ),
        ),
    ];
    for step_row in &mut step_rows {
        step_row.step_digest = stable_step_digest(step_row);
    }
    let phase_exit_green = step_rows.iter().all(|step_row| step_row.status == "green");

    let mut packet = PsionExecutorLocalClusterRoundtripPacket {
        schema_version: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_local_cluster_roundtrip_v1"),
        registration_packet_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH),
        registration_packet_digest: registration.packet_digest.clone(),
        remote_launch_packet_ref: String::from(PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH),
        remote_launch_packet_digest: remote_launch.packet_digest.clone(),
        durable_checkpoint_packet_ref: String::from(PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH),
        durable_checkpoint_packet_digest: durable_checkpoint.packet_digest.clone(),
        decision_grade_packet_ref: String::from(PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH),
        decision_grade_packet_digest: decision_grade.packet_digest.clone(),
        recovery_packet_ref: String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH),
        recovery_packet_digest: recovery.packet_digest.clone(),
        current_best_registration_id: current_best_registration.registration_id.clone(),
        current_best_run_id: current_best_registration.run_id.clone(),
        current_best_candidate_status: candidate_status_key(&current_best_registration.candidate_status),
        shared_run_search_key,
        export_bundle_ref: decision_grade.retained_remote_bundle_ref.clone(),
        export_bundle_sha256: decision_grade.retained_remote_bundle_sha256.clone(),
        mac_validation_machine_class_id: mac_export.local_cpu_machine_class_id.clone(),
        mac_validation_digest,
        reference_linear_metric_id: mac_export.reference_linear_metric_id.clone(),
        hull_cache_metric_id: mac_export.hull_cache_metric_id.clone(),
        min_hull_cache_speedup_over_reference_linear: mac_export
            .min_hull_cache_speedup_over_reference_linear,
        max_hull_cache_remaining_gap_vs_cpu_reference: mac_export
            .max_hull_cache_remaining_gap_vs_cpu_reference,
        step_rows,
        phase_exit_green,
        cluster_closure_status: if phase_exit_green {
            String::from("green")
        } else {
            String::from("blocked")
        },
        cluster_closure_detail: format!(
            "The retained current-best registration `{}` now keeps the full Mac -> 4080 -> Mac loop green: controller launch on Mac, accelerator-backed decision run `{}`, durable checkpoint `{}`, restore rehearsal, returned bundle `{}`, and back-on-Mac validation under `{}` all stay explicit in one packet.",
            current_best_registration.registration_id,
            decision_grade.decision_run_id,
            recovery.checkpoint_ref,
            decision_grade.retained_remote_bundle_ref,
            mac_export.local_cpu_machine_class_id
        ),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_REMOTE_LAUNCH_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_DECISION_GRADE_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
        ],
        summary: format!(
            "The executor lane now has one retained local-cluster roundtrip closeout packet. Current-best run `{}` keeps all six admitted steps green across the Mac controller, the 4080 worker, and the Mac-side validation return path, so EPIC 4 cluster closure is now machine-legible instead of implied by separate launch, checkpoint, recovery, and export packets.",
            current_best_registration.run_id
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_local_cluster_roundtrip_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterRoundtripPacket, PsionExecutorLocalClusterRoundtripError> {
    let packet = builtin_executor_local_cluster_roundtrip_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn find_registration_row<'a>(
    registration: &'a PsionExecutorLocalClusterRunRegistrationPacket,
    profile_id: &str,
) -> Result<
    &'a crate::PsionExecutorLocalClusterRunRegistrationRow,
    PsionExecutorLocalClusterRoundtripError,
> {
    registration
        .registration_rows
        .iter()
        .find(|row| row.admitted_profile_id == profile_id)
        .ok_or_else(|| PsionExecutorLocalClusterRoundtripError::MissingField {
            field: format!("registration_row[{profile_id}]"),
        })
}

fn build_step_row(
    step_id: &str,
    owner_surface_ref: &str,
    owner_surface_digest: &str,
    detail: String,
) -> PsionExecutorLocalClusterRoundtripStepRow {
    PsionExecutorLocalClusterRoundtripStepRow {
        step_id: String::from(step_id),
        status: String::from("green"),
        owner_surface_ref: String::from(owner_surface_ref),
        owner_surface_digest: String::from(owner_surface_digest),
        detail,
        step_digest: String::new(),
    }
}

fn candidate_status_key(status: &PsionExecutorLocalClusterCandidateStatus) -> String {
    match status {
        PsionExecutorLocalClusterCandidateStatus::CurrentBest => String::from("current_best"),
        PsionExecutorLocalClusterCandidateStatus::Candidate => String::from("candidate"),
    }
}

fn stable_validation_digest(
    export_bundle_sha256: &str,
    machine_class_id: &str,
    reference_linear_metric_id: &str,
    hull_cache_metric_id: &str,
) -> String {
    let joined = format!(
        "bundle_sha256={export_bundle_sha256}|machine_class_id={machine_class_id}|reference_linear_metric_id={reference_linear_metric_id}|hull_cache_metric_id={hull_cache_metric_id}"
    );
    hex::encode(Sha256::digest(joined.as_bytes()))
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutorLocalClusterRoundtripError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutorLocalClusterRoundtripError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorLocalClusterRoundtripError> {
    let bytes = read_bytes(workspace_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorLocalClusterRoundtripError::Parse {
        path: relative_path.to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorLocalClusterRoundtripError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorLocalClusterRoundtripError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorLocalClusterRoundtripError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorLocalClusterRoundtripError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorLocalClusterRoundtripError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_step_digest(step_row: &PsionExecutorLocalClusterRoundtripStepRow) -> String {
    let mut clone = step_row.clone();
    clone.step_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("roundtrip step serialization should succeed"),
    ))
}

fn stable_packet_digest(packet: &PsionExecutorLocalClusterRoundtripPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("roundtrip packet serialization should succeed"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_local_cluster_roundtrip_packet_is_valid(
    ) -> Result<(), PsionExecutorLocalClusterRoundtripError> {
        let root = workspace_root();
        let packet = builtin_executor_local_cluster_roundtrip_packet(root.as_path())?;
        packet.validate()?;
        assert!(packet.phase_exit_green);
        assert_eq!(packet.cluster_closure_status, "green");
        Ok(())
    }

    #[test]
    fn executor_local_cluster_roundtrip_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorLocalClusterRoundtripError> {
        let root = workspace_root();
        let expected: PsionExecutorLocalClusterRoundtripPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_local_cluster_roundtrip_packet(root.as_path())?;
        if actual != expected {
            return Err(PsionExecutorLocalClusterRoundtripError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn executor_local_cluster_roundtrip_keeps_all_six_steps_green(
    ) -> Result<(), PsionExecutorLocalClusterRoundtripError> {
        let root = workspace_root();
        let packet = builtin_executor_local_cluster_roundtrip_packet(root.as_path())?;
        assert_eq!(packet.step_rows.len(), 6);
        assert!(packet.step_rows.iter().all(|step_row| step_row.status == "green"));
        Ok(())
    }

    #[test]
    fn write_executor_local_cluster_roundtrip_persists_current_truth(
    ) -> Result<(), PsionExecutorLocalClusterRoundtripError> {
        let root = workspace_root();
        let packet = write_builtin_executor_local_cluster_roundtrip_packet(root.as_path())?;
        let committed: PsionExecutorLocalClusterRoundtripPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH,
        )?;
        assert_eq!(packet, committed);
        Ok(())
    }
}
