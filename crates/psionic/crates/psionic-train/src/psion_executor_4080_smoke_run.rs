use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionExecutor4080FrequentEvalAttachmentPacket,
    PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_DOC_PATH,
    PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH,
};

/// Stable schema version for the admitted 4080 smoke-run packet.
pub const PSION_EXECUTOR_4080_SMOKE_RUN_SCHEMA_VERSION: &str = "psion.executor.4080_smoke_run.v1";
/// Canonical fixture path for the admitted 4080 smoke-run packet.
pub const PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_4080_smoke_run_v1.json";
/// Canonical doc path for the admitted 4080 smoke-run packet.
pub const PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH: &str = "docs/PSION_EXECUTOR_4080_SMOKE_RUN.md";

const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const LOCAL_TAILNET_CONTROL_PROFILE_ID: &str = "local_tailnet_cluster_control_plane";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const TAILRUN_HOME_SUMMARY_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/tailrun_admitted_home_run_summary.json";
const TAILNET_COORDINATOR_REPORT_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/coordinator_runtime_report.json";
const TAILNET_CONTRIBUTOR_REPORT_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/contributor_runtime_report.json";
const EXPECTED_RUN_ID: &str = "tailrun-home-admitted-20260328k";
const EXPECTED_LINUX_WORKER_ID: &str = "swarm-linux-4080-a";

#[derive(Clone, Debug, Deserialize)]
struct TailrunAdmittedHomeRunSummary {
    run_id: String,
    run_family_id: String,
    result_classification: String,
    accepted_contributions: u64,
    replay_checked_contributions: u64,
    submission_receipt_count: u64,
    publish_disposition: String,
    promotion_disposition: String,
    per_device_contributions: Vec<PerDeviceContribution>,
    merged_artifact: MergedArtifactSummary,
    summary_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PerDeviceContribution {
    node_id: String,
    execution_backend_label: String,
    endpoint: String,
    observed_wallclock_ms: u64,
    local_execution_wallclock_ms: u64,
    executed_steps: u64,
    batch_count: u64,
    sample_count: u64,
    payload_bytes: u64,
    final_mean_loss: f64,
    contributor_receipt_digest: String,
    estimated_steps_per_second: f64,
    estimated_samples_per_second: f64,
    contribution_share: f64,
}

#[derive(Clone, Debug, Deserialize)]
struct MergedArtifactSummary {
    merge_strategy: String,
    merged_lora_rank: u64,
    canonical_profile_mean_loss: f64,
    deterministic_probe_top_token_id: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct CoordinatorRuntimeReport {
    membership_receipt: MembershipReceipt,
    window_plan: WindowPlan,
}

#[derive(Clone, Debug, Deserialize)]
struct MembershipReceipt {
    contributor_statuses: Vec<ContributorStatus>,
}

#[derive(Clone, Debug, Deserialize)]
struct ContributorStatus {
    node_id: String,
    free_memory_bytes: u64,
    accelerator_count: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct WindowPlan {
    dataset_slices: Vec<DatasetSlice>,
}

#[derive(Clone, Debug, Deserialize)]
struct DatasetSlice {
    dataset_id: String,
    split_name: String,
    slice_id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ContributorRuntimeReport {
    local_contribution: LocalContribution,
}

#[derive(Clone, Debug, Deserialize)]
struct LocalContribution {
    contributor_receipt: ContributorReceipt,
}

#[derive(Clone, Debug, Deserialize)]
struct ContributorReceipt {
    unsupported_precision_refusal: String,
}

/// One retained checklist row for the 4080 smoke packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080SmokeChecklistRow {
    /// Stable checklist id.
    pub checklist_id: String,
    /// Final status.
    pub status: String,
    /// Honest detail.
    pub detail: String,
}

/// Typed packet binding the first admitted 4080 smoke run into the executor roadmap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutor4080SmokeRunPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted 4080 worker profile id.
    pub worker_profile_id: String,
    /// Admitted Tailnet control-plane profile id.
    pub control_plane_profile_id: String,
    /// Prerequisite frequent-eval attachment packet reference.
    pub eval_attachment_packet_ref: String,
    /// Stable SHA256 over the prerequisite frequent-eval attachment packet bytes.
    pub eval_attachment_packet_sha256: String,
    /// Retained run summary reference.
    pub retained_run_summary_ref: String,
    /// Stable SHA256 over the retained run summary bytes.
    pub retained_run_summary_sha256: String,
    /// Stable in-band run summary digest.
    pub retained_run_summary_digest: String,
    /// Retained coordinator report reference.
    pub coordinator_report_ref: String,
    /// Stable SHA256 over the retained coordinator report bytes.
    pub coordinator_report_sha256: String,
    /// Retained contributor report reference.
    pub contributor_report_ref: String,
    /// Stable SHA256 over the retained contributor report bytes.
    pub contributor_report_sha256: String,
    /// Stable smoke objective id.
    pub smoke_objective_id: String,
    /// Honest smoke objective kind.
    pub smoke_objective_kind: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable run family id.
    pub run_family_id: String,
    /// Dataset id used by the retained smoke objective.
    pub objective_dataset_id: String,
    /// Dataset split name used by the retained smoke objective.
    pub objective_split_name: String,
    /// Dataset slice ids retained by the smoke objective.
    pub objective_slice_ids: Vec<String>,
    /// Stable result classification.
    pub result_classification: String,
    /// Linux worker id.
    pub linux_worker_id: String,
    /// Linux worker backend label.
    pub execution_backend_label: String,
    /// Linux worker endpoint.
    pub worker_endpoint: String,
    /// Linux worker free memory bytes retained at admission.
    pub free_memory_bytes: u64,
    /// Linux worker accelerator count retained at admission.
    pub accelerator_count: u64,
    /// Linux worker observed wallclock.
    pub observed_wallclock_ms: u64,
    /// Linux worker local execution wallclock.
    pub local_execution_wallclock_ms: u64,
    /// Linux worker executed steps.
    pub executed_steps: u64,
    /// Linux worker batch count.
    pub batch_count: u64,
    /// Linux worker sample count.
    pub sample_count: u64,
    /// Linux worker payload bytes.
    pub payload_bytes: u64,
    /// Linux worker final mean loss.
    pub final_mean_loss: f64,
    /// Linux worker estimated steps per second.
    pub estimated_steps_per_second: f64,
    /// Linux worker estimated samples per second.
    pub estimated_samples_per_second: f64,
    /// Linux worker contribution share.
    pub contribution_share: f64,
    /// Linux contributor receipt digest.
    pub contributor_receipt_digest: String,
    /// Checkpoint family tied to the smoke run.
    pub checkpoint_family: String,
    /// Checkpoint pointer digest tied to the smoke run.
    pub checkpoint_pointer_digest: String,
    /// Checkpoint ref tied to the smoke run.
    pub checkpoint_ref: String,
    /// Eval ledger row id tied to the smoke run.
    pub eval_ledger_row_id: String,
    /// Eval ledger row digest tied to the smoke run.
    pub eval_ledger_row_digest: String,
    /// Operator-review suite status.
    pub operator_review_suite_status: String,
    /// Whether missing or unscored frequent-pack coverage blocks promotion.
    pub eval_missing_blocks_promotion: bool,
    /// Publish disposition retained by the run.
    pub publish_disposition: String,
    /// Promotion disposition retained by the run.
    pub promotion_disposition: String,
    /// Explicit unsupported-precision refusal retained by the contributor receipt.
    pub unsupported_precision_refusal: String,
    /// Accepted contribution count.
    pub accepted_contributions: u64,
    /// Replay-checked contribution count.
    pub replay_checked_contributions: u64,
    /// Submission receipt count.
    pub submission_receipt_count: u64,
    /// Merge strategy retained by the run summary.
    pub merge_strategy: String,
    /// Merged LoRA rank retained by the run summary.
    pub merged_lora_rank: u64,
    /// Canonical profile mean loss retained by the merged artifact summary.
    pub canonical_profile_mean_loss: f64,
    /// Deterministic probe top token id retained by the merged artifact summary.
    pub deterministic_probe_top_token_id: u64,
    /// Retained checklist rows.
    pub checklist_rows: Vec<PsionExecutor4080SmokeChecklistRow>,
    /// Support references.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutor4080SmokeRunPacket {
    /// Validate the retained 4080 smoke-run packet.
    pub fn validate(&self) -> Result<(), PsionExecutor4080SmokeRunError> {
        for (field, value) in [
            (
                "psion_executor_4080_smoke_run.schema_version",
                self.schema_version.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.worker_profile_id",
                self.worker_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.control_plane_profile_id",
                self.control_plane_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.eval_attachment_packet_ref",
                self.eval_attachment_packet_ref.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.eval_attachment_packet_sha256",
                self.eval_attachment_packet_sha256.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.retained_run_summary_ref",
                self.retained_run_summary_ref.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.retained_run_summary_sha256",
                self.retained_run_summary_sha256.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.retained_run_summary_digest",
                self.retained_run_summary_digest.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.coordinator_report_ref",
                self.coordinator_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.coordinator_report_sha256",
                self.coordinator_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.contributor_report_ref",
                self.contributor_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.contributor_report_sha256",
                self.contributor_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.smoke_objective_id",
                self.smoke_objective_id.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.smoke_objective_kind",
                self.smoke_objective_kind.as_str(),
            ),
            ("psion_executor_4080_smoke_run.run_id", self.run_id.as_str()),
            (
                "psion_executor_4080_smoke_run.run_family_id",
                self.run_family_id.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.objective_dataset_id",
                self.objective_dataset_id.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.objective_split_name",
                self.objective_split_name.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.result_classification",
                self.result_classification.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.linux_worker_id",
                self.linux_worker_id.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.execution_backend_label",
                self.execution_backend_label.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.worker_endpoint",
                self.worker_endpoint.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.contributor_receipt_digest",
                self.contributor_receipt_digest.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.checkpoint_family",
                self.checkpoint_family.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.checkpoint_pointer_digest",
                self.checkpoint_pointer_digest.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.checkpoint_ref",
                self.checkpoint_ref.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.eval_ledger_row_id",
                self.eval_ledger_row_id.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.eval_ledger_row_digest",
                self.eval_ledger_row_digest.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.operator_review_suite_status",
                self.operator_review_suite_status.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.publish_disposition",
                self.publish_disposition.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.promotion_disposition",
                self.promotion_disposition.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.unsupported_precision_refusal",
                self.unsupported_precision_refusal.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.merge_strategy",
                self.merge_strategy.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_4080_smoke_run.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.schema_version != PSION_EXECUTOR_4080_SMOKE_RUN_SCHEMA_VERSION {
            return Err(PsionExecutor4080SmokeRunError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_4080_SMOKE_RUN_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        if self.worker_profile_id != LOCAL_4080_PROFILE_ID {
            return Err(PsionExecutor4080SmokeRunError::InvalidValue {
                field: String::from("psion_executor_4080_smoke_run.worker_profile_id"),
                detail: String::from("worker profile id drifted"),
            });
        }
        if self.control_plane_profile_id != LOCAL_TAILNET_CONTROL_PROFILE_ID {
            return Err(PsionExecutor4080SmokeRunError::InvalidValue {
                field: String::from("psion_executor_4080_smoke_run.control_plane_profile_id"),
                detail: String::from("control-plane profile id drifted"),
            });
        }
        if self.checklist_rows.len() != 5 {
            return Err(PsionExecutor4080SmokeRunError::InvalidValue {
                field: String::from("psion_executor_4080_smoke_run.checklist_rows"),
                detail: String::from("smoke packet must keep exactly five checklist rows"),
            });
        }
        if self.objective_slice_ids.is_empty() {
            return Err(PsionExecutor4080SmokeRunError::MissingField {
                field: String::from("psion_executor_4080_smoke_run.objective_slice_ids"),
            });
        }
        if !self.eval_missing_blocks_promotion {
            return Err(PsionExecutor4080SmokeRunError::InvalidValue {
                field: String::from("psion_executor_4080_smoke_run.eval_missing_blocks_promotion"),
                detail: String::from(
                    "missing or unscored frequent eval must remain a hard blocker",
                ),
            });
        }
        if self.operator_review_suite_status != "green" {
            return Err(PsionExecutor4080SmokeRunError::InvalidValue {
                field: String::from("psion_executor_4080_smoke_run.operator_review_suite_status"),
                detail: String::from(
                    "operator-review suite must stay green on the retained smoke run",
                ),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutor4080SmokeRunError::MissingField {
                field: String::from("psion_executor_4080_smoke_run.support_refs"),
            });
        }
        if self.packet_digest != stable_4080_smoke_run_packet_digest(self) {
            return Err(PsionExecutor4080SmokeRunError::InvalidValue {
                field: String::from("psion_executor_4080_smoke_run.packet_digest"),
                detail: String::from("packet digest drifted"),
            });
        }
        Ok(())
    }
}

/// Errors emitted by the retained 4080 smoke packet.
#[derive(Debug, Error)]
pub enum PsionExecutor4080SmokeRunError {
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
    #[error("failed to encode smoke packet: {0}")]
    Encode(#[from] serde_json::Error),
}

/// Build the retained 4080 smoke-run packet.
pub fn builtin_executor_4080_smoke_run_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080SmokeRunPacket, PsionExecutor4080SmokeRunError> {
    let eval_attachment_path =
        workspace_root.join(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH);
    let eval_attachment_bytes =
        fs::read(&eval_attachment_path).map_err(|error| PsionExecutor4080SmokeRunError::Read {
            path: eval_attachment_path.display().to_string(),
            error,
        })?;
    let eval_attachment: PsionExecutor4080FrequentEvalAttachmentPacket =
        serde_json::from_slice(&eval_attachment_bytes).map_err(|error| {
            PsionExecutor4080SmokeRunError::Decode {
                path: eval_attachment_path.display().to_string(),
                error,
            }
        })?;
    let summary_path = workspace_root.join(TAILRUN_HOME_SUMMARY_PATH);
    let summary_bytes =
        fs::read(&summary_path).map_err(|error| PsionExecutor4080SmokeRunError::Read {
            path: summary_path.display().to_string(),
            error,
        })?;
    let summary: TailrunAdmittedHomeRunSummary =
        serde_json::from_slice(&summary_bytes).map_err(|error| {
            PsionExecutor4080SmokeRunError::Decode {
                path: summary_path.display().to_string(),
                error,
            }
        })?;
    let coordinator_report_path = workspace_root.join(TAILNET_COORDINATOR_REPORT_PATH);
    let coordinator_report_bytes = fs::read(&coordinator_report_path).map_err(|error| {
        PsionExecutor4080SmokeRunError::Read {
            path: coordinator_report_path.display().to_string(),
            error,
        }
    })?;
    let coordinator_report: CoordinatorRuntimeReport =
        serde_json::from_slice(&coordinator_report_bytes).map_err(|error| {
            PsionExecutor4080SmokeRunError::Decode {
                path: coordinator_report_path.display().to_string(),
                error,
            }
        })?;
    let contributor_report_path = workspace_root.join(TAILNET_CONTRIBUTOR_REPORT_PATH);
    let contributor_report_bytes = fs::read(&contributor_report_path).map_err(|error| {
        PsionExecutor4080SmokeRunError::Read {
            path: contributor_report_path.display().to_string(),
            error,
        }
    })?;
    let contributor_report: ContributorRuntimeReport =
        serde_json::from_slice(&contributor_report_bytes).map_err(|error| {
            PsionExecutor4080SmokeRunError::Decode {
                path: contributor_report_path.display().to_string(),
                error,
            }
        })?;

    if summary.run_id != EXPECTED_RUN_ID || eval_attachment.run_id != EXPECTED_RUN_ID {
        return Err(PsionExecutor4080SmokeRunError::InvalidValue {
            field: String::from("psion_executor_4080_smoke_run.run_id"),
            detail: String::from(
                "retained run summary and frequent-eval attachment must stay aligned on the admitted rerun id",
            ),
        });
    }

    let linux_contribution = summary
        .per_device_contributions
        .iter()
        .find(|row| row.node_id == EXPECTED_LINUX_WORKER_ID)
        .ok_or_else(|| PsionExecutor4080SmokeRunError::InvalidValue {
            field: String::from("psion_executor_4080_smoke_run.linux_worker_id"),
            detail: String::from("retained run summary must keep one Linux 4080 contribution row"),
        })?;
    let linux_status = coordinator_report
        .membership_receipt
        .contributor_statuses
        .iter()
        .find(|row| row.node_id == EXPECTED_LINUX_WORKER_ID)
        .ok_or_else(|| PsionExecutor4080SmokeRunError::InvalidValue {
            field: String::from("psion_executor_4080_smoke_run.free_memory_bytes"),
            detail: String::from("coordinator report must keep one Linux 4080 membership row"),
        })?;

    let objective_dataset_id = coordinator_report
        .window_plan
        .dataset_slices
        .first()
        .ok_or_else(|| PsionExecutor4080SmokeRunError::MissingField {
            field: String::from("psion_executor_4080_smoke_run.objective_dataset_id"),
        })?
        .dataset_id
        .clone();
    let objective_split_name = coordinator_report.window_plan.dataset_slices[0]
        .split_name
        .clone();
    let objective_slice_ids = coordinator_report
        .window_plan
        .dataset_slices
        .iter()
        .map(|row| row.slice_id.clone())
        .collect::<Vec<_>>();

    if !eval_attachment.checkpoint_eval_row.promotion_blocked {
        return Err(PsionExecutor4080SmokeRunError::InvalidValue {
            field: String::from("psion_executor_4080_smoke_run.eval_missing_blocks_promotion"),
            detail: String::from(
                "frequent-eval attachment must keep the retained smoke run non-promotable while suites remain unscored",
            ),
        });
    }

    let operator_review_suite_status = eval_attachment
        .checkpoint_eval_row
        .suite_results
        .iter()
        .find(|row| row.suite_id == "frequent_operator_review_cases_v0")
        .map(|row| row.status.clone())
        .ok_or_else(|| PsionExecutor4080SmokeRunError::InvalidValue {
            field: String::from("psion_executor_4080_smoke_run.operator_review_suite_status"),
            detail: String::from(
                "frequent-eval attachment must keep one operator-review suite result",
            ),
        })?;

    let checklist_rows = vec![
        PsionExecutor4080SmokeChecklistRow {
            checklist_id: String::from("checkpoint_written_green"),
            status: String::from("green"),
            detail: format!(
                "The retained smoke run now stays tied to checkpoint family `{}` at pointer digest `{}` instead of relying on an implicit scratch path.",
                eval_attachment.checkpoint_eval_row.checkpoint_family,
                eval_attachment.checkpoint_eval_row.checkpoint_pointer_digest,
            ),
        },
        PsionExecutor4080SmokeChecklistRow {
            checklist_id: String::from("throughput_recorded_green"),
            status: String::from("green"),
            detail: format!(
                "The Linux 4080 contribution row records observed wallclock {} ms, local execution {} ms, {} steps, and {:.1} estimated steps/s.",
                linux_contribution.observed_wallclock_ms,
                linux_contribution.local_execution_wallclock_ms,
                linux_contribution.executed_steps,
                linux_contribution.estimated_steps_per_second,
            ),
        },
        PsionExecutor4080SmokeChecklistRow {
            checklist_id: String::from("memory_fact_green"),
            status: String::from("green"),
            detail: format!(
                "The coordinator membership receipt records {} free bytes and {} accelerator for the admitted Linux 4080 worker before the smoke run counts.",
                linux_status.free_memory_bytes,
                linux_status.accelerator_count,
            ),
        },
        PsionExecutor4080SmokeChecklistRow {
            checklist_id: String::from("eval_attachment_green"),
            status: String::from("green"),
            detail: format!(
                "The smoke run now cites frequent-eval ledger row `{}` with operator-review suite status `{}` and keeps missing or unscored suites as hard blockers instead of silent gaps.",
                eval_attachment.checkpoint_eval_row.ledger_row_id,
                operator_review_suite_status,
            ),
        },
        PsionExecutor4080SmokeChecklistRow {
            checklist_id: String::from("failure_facts_green"),
            status: String::from("green"),
            detail: format!(
                "The retained smoke run keeps unsupported-precision refusal `{}` plus publish disposition `{}` and promotion disposition `{}` explicit.",
                contributor_report
                    .local_contribution
                    .contributor_receipt
                    .unsupported_precision_refusal,
                summary.publish_disposition,
                summary.promotion_disposition,
            ),
        },
    ];

    let mut packet = PsionExecutor4080SmokeRunPacket {
        schema_version: String::from(PSION_EXECUTOR_4080_SMOKE_RUN_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_4080_smoke_run_v1"),
        worker_profile_id: String::from(LOCAL_4080_PROFILE_ID),
        control_plane_profile_id: String::from(LOCAL_TAILNET_CONTROL_PROFILE_ID),
        eval_attachment_packet_ref: String::from(
            PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH,
        ),
        eval_attachment_packet_sha256: hex::encode(Sha256::digest(&eval_attachment_bytes)),
        retained_run_summary_ref: String::from(TAILRUN_HOME_SUMMARY_PATH),
        retained_run_summary_sha256: hex::encode(Sha256::digest(&summary_bytes)),
        retained_run_summary_digest: summary.summary_sha256,
        coordinator_report_ref: String::from(TAILNET_COORDINATOR_REPORT_PATH),
        coordinator_report_sha256: hex::encode(Sha256::digest(&coordinator_report_bytes)),
        contributor_report_ref: String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
        contributor_report_sha256: hex::encode(Sha256::digest(&contributor_report_bytes)),
        smoke_objective_id: String::from("psion.executor.4080_tailnet_smoke_objective.v1"),
        smoke_objective_kind: String::from("executor_lane_local_cluster_admission_surrogate"),
        run_id: summary.run_id,
        run_family_id: summary.run_family_id,
        objective_dataset_id,
        objective_split_name,
        objective_slice_ids,
        result_classification: summary.result_classification,
        linux_worker_id: String::from(EXPECTED_LINUX_WORKER_ID),
        execution_backend_label: linux_contribution.execution_backend_label.clone(),
        worker_endpoint: linux_contribution.endpoint.clone(),
        free_memory_bytes: linux_status.free_memory_bytes,
        accelerator_count: linux_status.accelerator_count,
        observed_wallclock_ms: linux_contribution.observed_wallclock_ms,
        local_execution_wallclock_ms: linux_contribution.local_execution_wallclock_ms,
        executed_steps: linux_contribution.executed_steps,
        batch_count: linux_contribution.batch_count,
        sample_count: linux_contribution.sample_count,
        payload_bytes: linux_contribution.payload_bytes,
        final_mean_loss: linux_contribution.final_mean_loss,
        estimated_steps_per_second: linux_contribution.estimated_steps_per_second,
        estimated_samples_per_second: linux_contribution.estimated_samples_per_second,
        contribution_share: linux_contribution.contribution_share,
        contributor_receipt_digest: linux_contribution.contributor_receipt_digest.clone(),
        checkpoint_family: eval_attachment.checkpoint_eval_row.checkpoint_family.clone(),
        checkpoint_pointer_digest: eval_attachment
            .checkpoint_eval_row
            .checkpoint_pointer_digest
            .clone(),
        checkpoint_ref: eval_attachment.checkpoint_eval_row.checkpoint_ref.clone(),
        eval_ledger_row_id: eval_attachment.checkpoint_eval_row.ledger_row_id.clone(),
        eval_ledger_row_digest: eval_attachment.checkpoint_eval_row.ledger_row_digest.clone(),
        operator_review_suite_status,
        eval_missing_blocks_promotion: eval_attachment.missing_eval_blocks_promotion,
        publish_disposition: summary.publish_disposition,
        promotion_disposition: summary.promotion_disposition,
        unsupported_precision_refusal: contributor_report
            .local_contribution
            .contributor_receipt
            .unsupported_precision_refusal,
        accepted_contributions: summary.accepted_contributions,
        replay_checked_contributions: summary.replay_checked_contributions,
        submission_receipt_count: summary.submission_receipt_count,
        merge_strategy: summary.merged_artifact.merge_strategy,
        merged_lora_rank: summary.merged_artifact.merged_lora_rank,
        canonical_profile_mean_loss: summary.merged_artifact.canonical_profile_mean_loss,
        deterministic_probe_top_token_id: summary
            .merged_artifact
            .deterministic_probe_top_token_id,
        checklist_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_DOC_PATH),
            String::from(TAILRUN_HOME_SUMMARY_PATH),
            String::from(TAILNET_COORDINATOR_REPORT_PATH),
            String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
        ],
        summary: format!(
            "The admitted 4080 lane now has one real smoke-run packet tied to retained run `{}`. The smoke objective remains an executor-lane local-cluster admission surrogate on dataset `{}` through the Linux 4080 contribution row (steps={} observed_wallclock_ms={} estimated_steps_per_second={:.1}), while checkpoint pointer `{}` and frequent-eval ledger row `{}` keep checkpoint, eval, and blocker facts explicit instead of relying on launch-only infrastructure claims.",
            EXPECTED_RUN_ID,
            coordinator_report.window_plan.dataset_slices[0].dataset_id,
            linux_contribution.executed_steps,
            linux_contribution.observed_wallclock_ms,
            linux_contribution.estimated_steps_per_second,
            eval_attachment.checkpoint_eval_row.checkpoint_pointer_digest,
            eval_attachment.checkpoint_eval_row.ledger_row_id,
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_4080_smoke_run_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the retained 4080 smoke-run packet.
pub fn write_builtin_executor_4080_smoke_run_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080SmokeRunPacket, PsionExecutor4080SmokeRunError> {
    let packet = builtin_executor_4080_smoke_run_packet(workspace_root)?;
    let path = workspace_root.join(PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionExecutor4080SmokeRunError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&packet)?).map_err(|error| {
        PsionExecutor4080SmokeRunError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

fn stable_4080_smoke_run_packet_digest(packet: &PsionExecutor4080SmokeRunPacket) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_4080_smoke_run|", &canonical)
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

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutor4080SmokeRunError> {
    if value.trim().is_empty() {
        return Err(PsionExecutor4080SmokeRunError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_executor_4080_smoke_run_packet_is_valid() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = builtin_executor_4080_smoke_run_packet(workspace_root.as_path())
            .expect("build 4080 smoke packet");
        packet.validate().expect("validate 4080 smoke packet");
        assert_eq!(packet.run_id, EXPECTED_RUN_ID);
        assert_eq!(packet.linux_worker_id, EXPECTED_LINUX_WORKER_ID);
        assert_eq!(packet.executed_steps, 12);
        assert_eq!(packet.operator_review_suite_status, "green");
    }

    #[test]
    fn executor_4080_smoke_run_fixture_matches_committed_truth() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let generated = builtin_executor_4080_smoke_run_packet(workspace_root.as_path())
            .expect("build 4080 smoke packet");
        let fixture_path = workspace_root.join(PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH);
        let fixture_bytes = fs::read(&fixture_path).expect("read 4080 smoke fixture");
        let committed: PsionExecutor4080SmokeRunPacket =
            serde_json::from_slice(&fixture_bytes).expect("decode 4080 smoke fixture");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_executor_4080_smoke_run_packet_persists_current_truth() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = write_builtin_executor_4080_smoke_run_packet(workspace_root.as_path())
            .expect("write 4080 smoke packet");
        packet
            .validate()
            .expect("validate written 4080 smoke packet");
    }
}
