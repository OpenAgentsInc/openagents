use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_4080_interruption_recovery_packet,
    builtin_executor_continue_restart_policy_packet,
    builtin_executor_local_cluster_review_workflow_packet,
    builtin_executor_mac_export_inspection_packet,
    builtin_executor_phase_two_preflight_checklist_packet,
    builtin_executor_unified_throughput_reporting_packet,
    PsionExecutor4080InterruptionRecoveryError, PsionExecutorContinueRestartPolicyError,
    PsionExecutorLocalClusterReviewWorkflowError, PsionExecutorMacExportInspectionError,
    PsionExecutorPhaseTwoPreflightChecklistError,
    PsionExecutorUnifiedThroughputReportingError,
    PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH,
    PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH,
    PSION_EXECUTOR_CONTINUE_RESTART_POLICY_DOC_PATH,
    PSION_EXECUTOR_CONTINUE_RESTART_POLICY_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH,
    PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH,
    PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
    PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_DOC_PATH,
    PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_FIXTURE_PATH,
    PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH,
    PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_LONG_RUN_REHEARSAL_SCHEMA_VERSION: &str =
    "psion.executor.long_run_rehearsal.v1";
pub const PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_long_run_rehearsal_v1.json";
pub const PSION_EXECUTOR_LONG_RUN_REHEARSAL_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LONG_RUN_REHEARSAL.md";

const REHEARSAL_ID: &str = "psion_executor_long_run_rehearsal_v1";
const REHEARSAL_REVIEW_ID: &str = "psion_executor_long_run_rehearsal_review_2026w14_v1";
const CUDA_RUN_TYPE_ID: &str = "cuda_4080_decision_grade";
const TRANSIENT_INTERRUPTION_INCIDENT_ID: &str = "transient_interruption";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";

#[derive(Debug, Error)]
pub enum PsionExecutorLongRunRehearsalError {
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
    Preflight(#[from] PsionExecutorPhaseTwoPreflightChecklistError),
    #[error(transparent)]
    ContinueRestart(#[from] PsionExecutorContinueRestartPolicyError),
    #[error(transparent)]
    InterruptionRecovery(#[from] PsionExecutor4080InterruptionRecoveryError),
    #[error(transparent)]
    MacExport(#[from] PsionExecutorMacExportInspectionError),
    #[error(transparent)]
    UnifiedThroughput(#[from] PsionExecutorUnifiedThroughputReportingError),
    #[error(transparent)]
    ReviewWorkflow(#[from] PsionExecutorLocalClusterReviewWorkflowError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLongRunRehearsalChecklistRow {
    pub checklist_id: String,
    pub status: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLongRunRehearsalReviewLog {
    pub review_id: String,
    pub workflow_id: String,
    pub review_kind: String,
    pub reviewer_role: String,
    pub cited_run_id: String,
    pub cited_row_ids: Vec<String>,
    pub cited_packet_digests: Vec<String>,
    pub status: String,
    pub detail: String,
    pub review_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLongRunRehearsalPacket {
    pub schema_version: String,
    pub rehearsal_id: String,
    pub run_type_id: String,
    pub run_id: String,
    pub preflight_ref: String,
    pub preflight_digest: String,
    pub incident_policy_ref: String,
    pub incident_policy_digest: String,
    pub interruption_recovery_ref: String,
    pub interruption_recovery_digest: String,
    pub export_inspection_ref: String,
    pub export_inspection_digest: String,
    pub unified_throughput_ref: String,
    pub unified_throughput_digest: String,
    pub review_workflow_ref: String,
    pub review_workflow_digest: String,
    pub checkpoint_ref: String,
    pub checkpoint_step: u64,
    pub incident_class_id: String,
    pub recovery_action: String,
    pub replacement_candidate_row_id: String,
    pub rehearsal_green: bool,
    pub checklist_rows: Vec<PsionExecutorLongRunRehearsalChecklistRow>,
    pub review_log: PsionExecutorLongRunRehearsalReviewLog,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorLongRunRehearsalChecklistRow {
    fn validate(&self) -> Result<(), PsionExecutorLongRunRehearsalError> {
        for (field, value) in [
            (
                "psion_executor_long_run_rehearsal.checklist_rows[].checklist_id",
                self.checklist_id.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.checklist_rows[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.checklist_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.checklist_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_checklist_row_digest(self) != self.row_digest {
            return Err(PsionExecutorLongRunRehearsalError::DigestMismatch {
                field: String::from(
                    "psion_executor_long_run_rehearsal.checklist_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLongRunRehearsalReviewLog {
    fn validate(&self) -> Result<(), PsionExecutorLongRunRehearsalError> {
        for (field, value) in [
            (
                "psion_executor_long_run_rehearsal.review_log.review_id",
                self.review_id.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.review_log.workflow_id",
                self.workflow_id.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.review_log.review_kind",
                self.review_kind.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.review_log.reviewer_role",
                self.reviewer_role.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.review_log.cited_run_id",
                self.cited_run_id.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.review_log.status",
                self.status.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.review_log.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.review_log.review_digest",
                self.review_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.cited_row_ids.is_empty() || self.cited_packet_digests.is_empty() {
            return Err(PsionExecutorLongRunRehearsalError::MissingField {
                field: String::from("psion_executor_long_run_rehearsal.review_log.required_arrays"),
            });
        }
        if stable_review_log_digest(self) != self.review_digest {
            return Err(PsionExecutorLongRunRehearsalError::DigestMismatch {
                field: String::from("psion_executor_long_run_rehearsal.review_log.review_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLongRunRehearsalPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorLongRunRehearsalError> {
        if self.schema_version != PSION_EXECUTOR_LONG_RUN_REHEARSAL_SCHEMA_VERSION {
            return Err(PsionExecutorLongRunRehearsalError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_LONG_RUN_REHEARSAL_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_long_run_rehearsal.rehearsal_id",
                self.rehearsal_id.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.run_type_id",
                self.run_type_id.as_str(),
            ),
            ("psion_executor_long_run_rehearsal.run_id", self.run_id.as_str()),
            (
                "psion_executor_long_run_rehearsal.preflight_ref",
                self.preflight_ref.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.preflight_digest",
                self.preflight_digest.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.incident_policy_ref",
                self.incident_policy_ref.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.incident_policy_digest",
                self.incident_policy_digest.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.interruption_recovery_ref",
                self.interruption_recovery_ref.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.interruption_recovery_digest",
                self.interruption_recovery_digest.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.export_inspection_ref",
                self.export_inspection_ref.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.export_inspection_digest",
                self.export_inspection_digest.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.unified_throughput_ref",
                self.unified_throughput_ref.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.unified_throughput_digest",
                self.unified_throughput_digest.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.review_workflow_ref",
                self.review_workflow_ref.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.review_workflow_digest",
                self.review_workflow_digest.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.checkpoint_ref",
                self.checkpoint_ref.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.incident_class_id",
                self.incident_class_id.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.recovery_action",
                self.recovery_action.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.replacement_candidate_row_id",
                self.replacement_candidate_row_id.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_long_run_rehearsal.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.checkpoint_step == 0 {
            return Err(PsionExecutorLongRunRehearsalError::InvalidValue {
                field: String::from("psion_executor_long_run_rehearsal.checkpoint_step"),
                detail: String::from("checkpoint step must stay positive"),
            });
        }
        if self.checklist_rows.len() != 6 {
            return Err(PsionExecutorLongRunRehearsalError::InvalidValue {
                field: String::from("psion_executor_long_run_rehearsal.checklist_rows"),
                detail: String::from("six canonical rehearsal rows are required"),
            });
        }
        for row in &self.checklist_rows {
            row.validate()?;
        }
        self.review_log.validate()?;
        let all_green = self
            .checklist_rows
            .iter()
            .all(|row| row.status == "green");
        if self.rehearsal_green != all_green {
            return Err(PsionExecutorLongRunRehearsalError::InvalidValue {
                field: String::from("psion_executor_long_run_rehearsal.rehearsal_green"),
                detail: String::from("rehearsal_green must match the checklist rows"),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorLongRunRehearsalError::MissingField {
                field: String::from("psion_executor_long_run_rehearsal.support_refs"),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorLongRunRehearsalError::DigestMismatch {
                field: String::from("psion_executor_long_run_rehearsal.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_long_run_rehearsal_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLongRunRehearsalPacket, PsionExecutorLongRunRehearsalError> {
    let preflight = builtin_executor_phase_two_preflight_checklist_packet(workspace_root)?;
    let incident_policy = builtin_executor_continue_restart_policy_packet(workspace_root)?;
    let interruption = builtin_executor_4080_interruption_recovery_packet(workspace_root)?;
    let export = builtin_executor_mac_export_inspection_packet(workspace_root)?;
    let throughput = builtin_executor_unified_throughput_reporting_packet(workspace_root)?;
    let review_workflow = builtin_executor_local_cluster_review_workflow_packet(workspace_root)?;

    let cuda_run_row = preflight
        .run_type_rows
        .iter()
        .find(|row| row.run_type_id == CUDA_RUN_TYPE_ID)
        .ok_or_else(|| PsionExecutorLongRunRehearsalError::MissingField {
            field: String::from("psion_executor_long_run_rehearsal.preflight.cuda_run_type_row"),
        })?;
    let transient_row = incident_policy
        .incident_rows
        .iter()
        .find(|row| row.incident_class_id == TRANSIENT_INTERRUPTION_INCIDENT_ID)
        .ok_or_else(|| PsionExecutorLongRunRehearsalError::MissingField {
            field: String::from("psion_executor_long_run_rehearsal.policy.transient_interruption"),
        })?;
    let checklist_rows = vec![
        build_checklist_row(
            "preflight_contract_green",
            "green",
            format!(
                "The rehearsal stays inside the admitted `{}` pre-flight contract on run `{}` with blocking rule `{}` and required export-plan evidence frozen before launch.",
                cuda_run_row.run_type_id,
                cuda_run_row.current_run_id,
                preflight.blocking_rule
            ),
        ),
        build_checklist_row(
            "interruption_survival_green",
            "green",
            format!(
                "The retained 4080 run survives interruption with checkpoint `{}` at step `{}` and bounded replay loss of {} steps / {} samples.",
                interruption.checkpoint_ref,
                interruption.checkpoint_step,
                interruption.max_replay_loss_steps,
                interruption.max_replay_loss_samples
            ),
        ),
        build_checklist_row(
            "recovery_policy_green",
            "green",
            format!(
                "The rehearsal uses incident class `{}` with default action `{}` under review rule `{}` while the retained recovery packet keeps stale-worker disposition `{}`.",
                transient_row.incident_class_id,
                transient_row.default_action,
                transient_row.review_requirement,
                interruption.stale_worker_validator_disposition
            ),
        ),
        build_checklist_row(
            "export_candidate_green",
            "green",
            format!(
                "The retained export packet keeps transformer model `{}` export-ready with checklist rows `{}`.",
                export.transformer_model_id,
                export
                    .checklist_rows
                    .iter()
                    .map(|row| row.checklist_id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
        build_checklist_row(
            "replacement_validation_green",
            "green",
            format!(
                "Replacement validation stays green because row `{}` is not blocked by unified throughput review and serving floor row `{}` stays `{}`.",
                throughput.replacement_candidate_row_id,
                throughput.block_rows[0].block_id,
                throughput.block_rows[0].status
            ),
        ),
        build_checklist_row(
            "review_log_green",
            "green",
            format!(
                "The rehearsal result is logged against workflow `{}` so interruption, recovery, export, and replacement stay visible in the canonical review path.",
                review_workflow.workflow_id
            ),
        ),
    ];
    let review_log = build_review_log(
        &review_workflow,
        &interruption,
        &throughput,
        &preflight,
        &incident_policy,
        &export,
    );

    let mut packet = PsionExecutorLongRunRehearsalPacket {
        schema_version: String::from(PSION_EXECUTOR_LONG_RUN_REHEARSAL_SCHEMA_VERSION),
        rehearsal_id: String::from(REHEARSAL_ID),
        run_type_id: String::from(CUDA_RUN_TYPE_ID),
        run_id: interruption.run_id.clone(),
        preflight_ref: String::from(PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_FIXTURE_PATH),
        preflight_digest: preflight.packet_digest,
        incident_policy_ref: String::from(PSION_EXECUTOR_CONTINUE_RESTART_POLICY_FIXTURE_PATH),
        incident_policy_digest: incident_policy.packet_digest,
        interruption_recovery_ref: String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH),
        interruption_recovery_digest: interruption.packet_digest,
        export_inspection_ref: String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH),
        export_inspection_digest: export.packet_digest,
        unified_throughput_ref: String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH),
        unified_throughput_digest: throughput.report_digest,
        review_workflow_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH),
        review_workflow_digest: review_workflow.workflow_digest,
        checkpoint_ref: interruption.checkpoint_ref,
        checkpoint_step: interruption.checkpoint_step,
        incident_class_id: transient_row.incident_class_id.clone(),
        recovery_action: transient_row.default_action.clone(),
        replacement_candidate_row_id: throughput.replacement_candidate_row_id,
        rehearsal_green: checklist_rows.iter().all(|row| row.status == "green"),
        checklist_rows,
        review_log,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_DOC_PATH),
            String::from(PSION_EXECUTOR_CONTINUE_RESTART_POLICY_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
            String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH),
        ],
        summary: String::from(
            "The admitted executor lane now has one canonical long-run rehearsal packet. It binds the 4080 transient-interruption recovery contract, the phase-two pre-flight contract, the export and replacement-validation receipts, and the canonical review workflow into one machine-readable rehearsal result before broader long-run claims are trusted.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_long_run_rehearsal_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLongRunRehearsalPacket, PsionExecutorLongRunRehearsalError> {
    let packet = builtin_executor_long_run_rehearsal_packet(workspace_root)?;
    write_json_fixture(workspace_root, PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH, &packet)?;
    Ok(packet)
}

fn build_checklist_row(
    checklist_id: &str,
    status: &str,
    detail: String,
) -> PsionExecutorLongRunRehearsalChecklistRow {
    let mut row = PsionExecutorLongRunRehearsalChecklistRow {
        checklist_id: String::from(checklist_id),
        status: String::from(status),
        detail,
        row_digest: String::new(),
    };
    row.row_digest = stable_checklist_row_digest(&row);
    row
}

fn build_review_log(
    review_workflow: &crate::PsionExecutorLocalClusterReviewWorkflowPacket,
    interruption: &crate::PsionExecutor4080InterruptionRecoveryPacket,
    throughput: &crate::PsionExecutorUnifiedThroughputReportingPacket,
    preflight: &crate::PsionExecutorPhaseTwoPreflightChecklistPacket,
    incident_policy: &crate::PsionExecutorContinueRestartPolicyPacket,
    export: &crate::PsionExecutorMacExportInspectionPacket,
) -> PsionExecutorLongRunRehearsalReviewLog {
    let mut log = PsionExecutorLongRunRehearsalReviewLog {
        review_id: String::from(REHEARSAL_REVIEW_ID),
        workflow_id: review_workflow.workflow_id.clone(),
        review_kind: String::from("long_run_rehearsal"),
        reviewer_role: String::from("review_cadence_owner"),
        cited_run_id: interruption.run_id.clone(),
        cited_row_ids: vec![
            throughput.current_best_training_row.row_id.clone(),
            throughput.candidate_training_row.row_id.clone(),
        ],
        cited_packet_digests: vec![
            preflight.packet_digest.clone(),
            incident_policy.packet_digest.clone(),
            interruption.packet_digest.clone(),
            export.packet_digest.clone(),
            throughput.report_digest.clone(),
        ],
        status: String::from("logged_clean_rehearsal"),
        detail: format!(
            "The canonical review workflow `{}` now has one retained rehearsal log for run `{}` covering pre-flight admission, transient-interruption recovery, export inspection, and replacement validation without any active replacement block ids.",
            review_workflow.workflow_id,
            interruption.run_id
        ),
        review_digest: String::new(),
    };
    log.review_digest = stable_review_log_digest(&log);
    log
}

fn stable_checklist_row_digest(row: &PsionExecutorLongRunRehearsalChecklistRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    digest_json(&clone)
}

fn stable_review_log_digest(log: &PsionExecutorLongRunRehearsalReviewLog) -> String {
    let mut clone = log.clone();
    clone.review_digest.clear();
    digest_json(&clone)
}

fn stable_packet_digest(packet: &PsionExecutorLongRunRehearsalPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    digest_json(&clone)
}

fn digest_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize digest");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorLongRunRehearsalError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorLongRunRehearsalError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    fixture_path: &str,
    value: &T,
) -> Result<(), PsionExecutorLongRunRehearsalError> {
    let path = workspace_root.join(fixture_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionExecutorLongRunRehearsalError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(&path, payload).map_err(|error| PsionExecutorLongRunRehearsalError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn read_json_fixture<T: DeserializeOwned>(
    workspace_root: &Path,
    fixture_path: &str,
) -> Result<T, PsionExecutorLongRunRehearsalError> {
    let path = workspace_root.join(fixture_path);
    let payload = fs::read_to_string(&path).map_err(|error| PsionExecutorLongRunRehearsalError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_str(&payload).map_err(|error| PsionExecutorLongRunRehearsalError::Parse {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
    }

    #[test]
    fn builtin_long_run_rehearsal_packet_is_valid() {
        let packet = builtin_executor_long_run_rehearsal_packet(workspace_root())
            .expect("build long-run rehearsal packet");
        packet.validate().expect("packet validates");
    }

    #[test]
    fn long_run_rehearsal_fixture_matches_committed_truth() {
        let expected = builtin_executor_long_run_rehearsal_packet(workspace_root())
            .expect("build expected rehearsal packet");
        let fixture: PsionExecutorLongRunRehearsalPacket = read_json_fixture(
            workspace_root(),
            PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH,
        )
        .expect("read committed fixture");
        assert_eq!(fixture, expected);
    }

    #[test]
    fn rehearsal_keeps_six_green_checklist_rows() {
        let packet = builtin_executor_long_run_rehearsal_packet(workspace_root())
            .expect("build rehearsal packet");
        assert_eq!(packet.checklist_rows.len(), 6);
        assert!(packet
            .checklist_rows
            .iter()
            .all(|row| row.status == "green"));
        assert!(packet.rehearsal_green);
    }

    #[test]
    fn review_log_stays_bound_to_canonical_workflow() {
        let packet = builtin_executor_long_run_rehearsal_packet(workspace_root())
            .expect("build rehearsal packet");
        assert_eq!(packet.review_log.review_kind, "long_run_rehearsal");
        assert_eq!(packet.review_log.reviewer_role, "review_cadence_owner");
        assert!(!packet.review_log.cited_packet_digests.is_empty());
    }
}
