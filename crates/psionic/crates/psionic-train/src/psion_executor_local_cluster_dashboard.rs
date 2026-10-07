use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_baseline_truth_record, builtin_executor_local_cluster_ledger,
    builtin_executor_local_cluster_run_registration_packet, PsionExecutorBaselineTruthError,
    PsionExecutorBaselineTruthRecord, PsionExecutorLocalClusterCandidateStatus,
    PsionExecutorLocalClusterLedger, PsionExecutorLocalClusterLedgerError,
    PsionExecutorLocalClusterLedgerRow, PsionExecutorLocalClusterRunRegistrationError,
    PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH, PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH,
};

/// Stable schema version for the canonical local-cluster dashboard packet.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_SCHEMA_VERSION: &str =
    "psion.executor.local_cluster_dashboard.v1";
/// Canonical fixture path for the local-cluster dashboard packet.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_local_cluster_dashboard_v1.json";
/// Canonical doc path for the local-cluster dashboard packet.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD.md";

const CURRENT_BEST_PANEL_ID: &str = "psion_executor_local_cluster_dashboard_current_best_v1";
const CANDIDATE_PANEL_ID: &str = "psion_executor_local_cluster_dashboard_candidate_v1";
const BASELINE_PANEL_ID: &str = "psion_executor_local_cluster_dashboard_baseline_v1";
const PROFILE_COMPARISON_ID: &str = "psion_executor_local_cluster_dashboard_profile_comparison_v1";
const LOCAL_MAC_MLX_PROFILE_ID: &str = "local_mac_mlx_aarch64";
const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH: &str = "docs/PSION_EXECUTOR_BASELINE_TRUTH.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER.md";
const PSION_EXECUTOR_MLX_DECISION_GRADE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MLX_DECISION_GRADE_RUN.md";
const PSION_EXECUTOR_4080_DECISION_GRADE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_DECISION_GRADE_RUN.md";
const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MAC_EXPORT_INSPECTION.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP.md";

#[derive(Debug, Error)]
pub enum PsionExecutorLocalClusterDashboardError {
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
    BaselineTruth(#[from] PsionExecutorBaselineTruthError),
    #[error(transparent)]
    Registration(#[from] PsionExecutorLocalClusterRunRegistrationError),
    #[error(transparent)]
    Ledger(#[from] PsionExecutorLocalClusterLedgerError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterDashboardBaselineCard {
    pub panel_id: String,
    pub model_id: String,
    pub pack_ids: Vec<String>,
    pub total_suite_count: u64,
    pub green_suite_count: u64,
    pub manual_review_suite_count: u64,
    pub baseline_truth_digest: String,
    pub reference_linear_truth_anchor: String,
    pub hull_cache_target_posture: String,
    pub detail: String,
    pub panel_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterDashboardMetricSummary {
    pub completed_steps: u64,
    pub final_mean_loss: f64,
    pub retained_failure_count: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterDashboardThroughputSummary {
    pub observed_steps_per_second: f64,
    pub observed_samples_per_second: f64,
    pub observed_source_tokens_per_second: f64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterDashboardRunCard {
    pub panel_id: String,
    pub row_id: String,
    pub registration_id: String,
    pub run_id: String,
    pub profile_id: String,
    pub candidate_status: String,
    pub model_id: String,
    pub eval_pack_ids: Vec<String>,
    pub checkpoint_family: String,
    pub metrics: PsionExecutorLocalClusterDashboardMetricSummary,
    pub throughput: PsionExecutorLocalClusterDashboardThroughputSummary,
    pub budget_burn_ratio: f64,
    pub export_status: String,
    pub recovery_status: String,
    pub detail: String,
    pub panel_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterDashboardProfileColumn {
    pub profile_id: String,
    pub logical_role: String,
    pub run_id: String,
    pub candidate_status: String,
    pub final_mean_loss: f64,
    pub observed_steps_per_second: f64,
    pub budget_burn_ratio: f64,
    pub export_status: String,
    pub recovery_status: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterDashboardProfileComparison {
    pub comparison_id: String,
    pub shared_eval_pack_ids: Vec<String>,
    pub shared_run_search_key: String,
    pub candidate_to_current_best_steps_ratio: f64,
    pub columns: Vec<PsionExecutorLocalClusterDashboardProfileColumn>,
    pub detail: String,
    pub comparison_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterDashboardPacket {
    pub schema_version: String,
    pub dashboard_id: String,
    pub registration_packet_ref: String,
    pub registration_packet_digest: String,
    pub ledger_ref: String,
    pub ledger_digest: String,
    pub baseline_truth_ref: String,
    pub baseline_truth_digest: String,
    pub baseline_card: PsionExecutorLocalClusterDashboardBaselineCard,
    pub current_best_card: PsionExecutorLocalClusterDashboardRunCard,
    pub candidate_card: PsionExecutorLocalClusterDashboardRunCard,
    pub profile_comparison: PsionExecutorLocalClusterDashboardProfileComparison,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub dashboard_digest: String,
}

impl PsionExecutorLocalClusterDashboardBaselineCard {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_dashboard.baseline_card.panel_id",
                self.panel_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.baseline_card.model_id",
                self.model_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.baseline_card.baseline_truth_digest",
                self.baseline_truth_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.baseline_card.reference_linear_truth_anchor",
                self.reference_linear_truth_anchor.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.baseline_card.hull_cache_target_posture",
                self.hull_cache_target_posture.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.baseline_card.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.baseline_card.panel_digest",
                self.panel_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.pack_ids.is_empty() {
            return Err(PsionExecutorLocalClusterDashboardError::MissingField {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.baseline_card.pack_ids",
                ),
            });
        }
        for pack_id in &self.pack_ids {
            ensure_nonempty(
                pack_id.as_str(),
                "psion_executor_local_cluster_dashboard.baseline_card.pack_ids[]",
            )?;
        }
        if self.total_suite_count == 0 {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.baseline_card.total_suite_count",
                ),
                detail: String::from("total suite count must stay positive"),
            });
        }
        if self.green_suite_count > self.total_suite_count
            || self.manual_review_suite_count > self.total_suite_count
        {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.baseline_card.green_suite_count",
                ),
                detail: String::from("suite counts must stay bounded by total suite count"),
            });
        }
        if stable_baseline_card_digest(self) != self.panel_digest {
            return Err(PsionExecutorLocalClusterDashboardError::DigestMismatch {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.baseline_card.panel_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterDashboardMetricSummary {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        if self.completed_steps == 0 {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.run_card.metrics.completed_steps",
                ),
                detail: String::from("completed steps must stay positive"),
            });
        }
        if self.retained_failure_count == 0 {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.run_card.metrics.retained_failure_count",
                ),
                detail: String::from("retained failure count must stay positive"),
            });
        }
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_local_cluster_dashboard.run_card.metrics.detail",
        )
    }
}

impl PsionExecutorLocalClusterDashboardThroughputSummary {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_dashboard.run_card.throughput.observed_steps_per_second",
                self.observed_steps_per_second,
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.throughput.observed_samples_per_second",
                self.observed_samples_per_second,
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.throughput.observed_source_tokens_per_second",
                self.observed_source_tokens_per_second,
            ),
        ] {
            if value <= 0.0 {
                return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                    field: String::from(field),
                    detail: String::from("throughput values must stay positive"),
                });
            }
        }
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_local_cluster_dashboard.run_card.throughput.detail",
        )
    }
}

impl PsionExecutorLocalClusterDashboardRunCard {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_dashboard.run_card.panel_id",
                self.panel_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.row_id",
                self.row_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.registration_id",
                self.registration_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.profile_id",
                self.profile_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.candidate_status",
                self.candidate_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.model_id",
                self.model_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.checkpoint_family",
                self.checkpoint_family.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.export_status",
                self.export_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.recovery_status",
                self.recovery_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.run_card.panel_digest",
                self.panel_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.eval_pack_ids.is_empty() {
            return Err(PsionExecutorLocalClusterDashboardError::MissingField {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.run_card.eval_pack_ids",
                ),
            });
        }
        for eval_pack_id in &self.eval_pack_ids {
            ensure_nonempty(
                eval_pack_id.as_str(),
                "psion_executor_local_cluster_dashboard.run_card.eval_pack_ids[]",
            )?;
        }
        if self.budget_burn_ratio <= 0.0 || self.budget_burn_ratio > 1.0 {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.run_card.budget_burn_ratio",
                ),
                detail: String::from("budget burn ratio must stay in (0, 1]"),
            });
        }
        self.metrics.validate()?;
        self.throughput.validate()?;
        if stable_run_card_digest(self) != self.panel_digest {
            return Err(PsionExecutorLocalClusterDashboardError::DigestMismatch {
                field: String::from("psion_executor_local_cluster_dashboard.run_card.panel_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterDashboardProfileColumn {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.columns[].profile_id",
                self.profile_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.columns[].logical_role",
                self.logical_role.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.columns[].run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.columns[].candidate_status",
                self.candidate_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.columns[].export_status",
                self.export_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.columns[].recovery_status",
                self.recovery_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.columns[].detail",
                self.detail.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.observed_steps_per_second <= 0.0 || self.budget_burn_ratio <= 0.0 {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.profile_comparison.columns[].observed_steps_per_second",
                ),
                detail: String::from("column metrics must stay positive"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterDashboardProfileComparison {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.comparison_id",
                self.comparison_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.shared_run_search_key",
                self.shared_run_search_key.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.profile_comparison.comparison_digest",
                self.comparison_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.shared_eval_pack_ids.is_empty() {
            return Err(PsionExecutorLocalClusterDashboardError::MissingField {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.profile_comparison.shared_eval_pack_ids",
                ),
            });
        }
        for eval_pack_id in &self.shared_eval_pack_ids {
            ensure_nonempty(
                eval_pack_id.as_str(),
                "psion_executor_local_cluster_dashboard.profile_comparison.shared_eval_pack_ids[]",
            )?;
        }
        if self.columns.len() != 2 {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.profile_comparison.columns",
                ),
                detail: String::from("profile comparison must keep exactly two columns"),
            });
        }
        if self.candidate_to_current_best_steps_ratio <= 0.0 {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.profile_comparison.candidate_to_current_best_steps_ratio",
                ),
                detail: String::from("candidate-to-current-best ratio must stay positive"),
            });
        }
        for column in &self.columns {
            column.validate()?;
        }
        if stable_profile_comparison_digest(self) != self.comparison_digest {
            return Err(PsionExecutorLocalClusterDashboardError::DigestMismatch {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.profile_comparison.comparison_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterDashboardPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_local_cluster_dashboard.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_SCHEMA_VERSION {
            return Err(
                PsionExecutorLocalClusterDashboardError::SchemaVersionMismatch {
                    expected: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_local_cluster_dashboard.dashboard_id",
                self.dashboard_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.registration_packet_ref",
                self.registration_packet_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.registration_packet_digest",
                self.registration_packet_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.ledger_ref",
                self.ledger_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.ledger_digest",
                self.ledger_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.baseline_truth_ref",
                self.baseline_truth_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.baseline_truth_digest",
                self.baseline_truth_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_local_cluster_dashboard.dashboard_digest",
                self.dashboard_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.current_best_card.candidate_status != "current_best" {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.current_best_card.candidate_status",
                ),
                detail: String::from("current-best card must stay current_best"),
            });
        }
        if self.candidate_card.candidate_status != "candidate" {
            return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_dashboard.candidate_card.candidate_status",
                ),
                detail: String::from("candidate card must stay candidate"),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorLocalClusterDashboardError::MissingField {
                field: String::from("psion_executor_local_cluster_dashboard.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(
                support_ref.as_str(),
                "psion_executor_local_cluster_dashboard.support_refs[]",
            )?;
        }
        self.baseline_card.validate()?;
        self.current_best_card.validate()?;
        self.candidate_card.validate()?;
        self.profile_comparison.validate()?;
        if stable_dashboard_digest(self) != self.dashboard_digest {
            return Err(PsionExecutorLocalClusterDashboardError::DigestMismatch {
                field: String::from("psion_executor_local_cluster_dashboard.dashboard_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_local_cluster_dashboard_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterDashboardPacket, PsionExecutorLocalClusterDashboardError> {
    let registration = builtin_executor_local_cluster_run_registration_packet(workspace_root)?;
    let ledger = builtin_executor_local_cluster_ledger(workspace_root)?;
    let baseline_truth = builtin_executor_baseline_truth_record(workspace_root)?;

    let current_best_row = find_single_row(
        &ledger,
        PsionExecutorLocalClusterCandidateStatus::CurrentBest,
        "current_best",
    )?;
    let candidate_row = find_single_row(
        &ledger,
        PsionExecutorLocalClusterCandidateStatus::Candidate,
        "candidate",
    )?;

    let baseline_card = build_baseline_card(&baseline_truth);
    let current_best_card = build_run_card(current_best_row, CURRENT_BEST_PANEL_ID);
    let candidate_card = build_run_card(candidate_row, CANDIDATE_PANEL_ID);
    let profile_comparison = build_profile_comparison(current_best_row, candidate_row)?;

    let mut packet = PsionExecutorLocalClusterDashboardPacket {
        schema_version: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_SCHEMA_VERSION),
        dashboard_id: String::from("psion_executor_local_cluster_dashboard_v1"),
        registration_packet_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH),
        registration_packet_digest: registration.packet_digest,
        ledger_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
        ledger_digest: ledger.ledger_digest,
        baseline_truth_ref: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
        baseline_truth_digest: baseline_truth.record_digest,
        baseline_card,
        current_best_card,
        candidate_card,
        profile_comparison,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_DECISION_GRADE_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH),
        ],
        summary: String::from(
            "The admitted executor lane now has one canonical dashboard packet that keeps the frozen baseline, the retained current-best row, and the retained candidate row visible together. Metrics, throughput, recovery, export, and budget burn now project from the same retained ledger instead of separate packet prose, and the current-best 4080 row now shows the retained roundtrip as export-green.",
        ),
        dashboard_digest: String::new(),
    };
    packet.dashboard_digest = stable_dashboard_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_local_cluster_dashboard_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterDashboardPacket, PsionExecutorLocalClusterDashboardError> {
    let packet = builtin_executor_local_cluster_dashboard_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_baseline_card(
    baseline_truth: &PsionExecutorBaselineTruthRecord,
) -> PsionExecutorLocalClusterDashboardBaselineCard {
    let pack_ids: Vec<String> = baseline_truth
        .suite_truths
        .iter()
        .map(|suite| suite.pack_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut card = PsionExecutorLocalClusterDashboardBaselineCard {
        panel_id: String::from(BASELINE_PANEL_ID),
        model_id: baseline_truth.model_id.clone(),
        pack_ids,
        total_suite_count: baseline_truth.suite_truths.len() as u64,
        green_suite_count: baseline_truth
            .suite_truths
            .iter()
            .filter(|suite| suite.aggregate_green)
            .count() as u64,
        manual_review_suite_count: baseline_truth
            .suite_truths
            .iter()
            .filter(|suite| suite.manual_review_required)
            .count() as u64,
        baseline_truth_digest: baseline_truth.record_digest.clone(),
        reference_linear_truth_anchor: String::from(
            "`reference_linear` remains the measured baseline truth anchor for the current `trained-v0` executor baseline and stays visible in the dashboard instead of being flattened into a fast-route claim.",
        ),
        hull_cache_target_posture: String::from(
            "`hull_cache` remains the admitted fast-route target on the executor family, but it stays subordinate to baseline truth and fallback posture outside the admitted workload envelope.",
        ),
        detail: String::from(
            "The baseline panel keeps the frozen frequent and promotion pack truth visible beside local run cards so weekly review can compare candidate evidence against the current `trained-v0` floor without leaving the local-cluster dashboard surface.",
        ),
        panel_digest: String::new(),
    };
    card.panel_digest = stable_baseline_card_digest(&card);
    card
}

fn build_run_card(
    row: &PsionExecutorLocalClusterLedgerRow,
    panel_id: &str,
) -> PsionExecutorLocalClusterDashboardRunCard {
    let mut card = PsionExecutorLocalClusterDashboardRunCard {
        panel_id: String::from(panel_id),
        row_id: row.row_id.clone(),
        registration_id: row.registration_id.clone(),
        run_id: row.run_id.clone(),
        profile_id: row.admitted_profile_id.clone(),
        candidate_status: candidate_status_key(&row.candidate_status),
        model_id: row.model_id.clone(),
        eval_pack_ids: row.eval_pack_ids.clone(),
        checkpoint_family: row.checkpoint_lineage.checkpoint_family.clone(),
        metrics: PsionExecutorLocalClusterDashboardMetricSummary {
            completed_steps: row.metric_posture.completed_steps,
            final_mean_loss: row.metric_posture.final_mean_loss,
            retained_failure_count: row.failure_facts.len() as u64,
            detail: format!(
                "The `{}` panel keeps the retained completed-step count, final loss, and failure fact count from the cumulative ledger row instead of recomputing a second metric view.",
                row.admitted_profile_id
            ),
        },
        throughput: PsionExecutorLocalClusterDashboardThroughputSummary {
            observed_steps_per_second: row.metric_posture.observed_steps_per_second,
            observed_samples_per_second: row.metric_posture.observed_samples_per_second,
            observed_source_tokens_per_second: row.metric_posture.observed_source_tokens_per_second,
            detail: format!(
                "Throughput on `{}` stays projected directly from the ledger metric posture so the dashboard side-by-side view never drifts from the retained row facts.",
                row.admitted_profile_id
            ),
        },
        budget_burn_ratio: row.cost_posture.budget_burn_ratio,
        export_status: row.export_status.clone(),
        recovery_status: row.recovery_status.clone(),
        detail: format!(
            "The `{}` dashboard card keeps metrics, throughput, recovery, export status, and budget burn together for weekly local-cluster review.",
            row.admitted_profile_id
        ),
        panel_digest: String::new(),
    };
    card.panel_digest = stable_run_card_digest(&card);
    card
}

fn build_profile_comparison(
    current_best_row: &PsionExecutorLocalClusterLedgerRow,
    candidate_row: &PsionExecutorLocalClusterLedgerRow,
) -> Result<
    PsionExecutorLocalClusterDashboardProfileComparison,
    PsionExecutorLocalClusterDashboardError,
> {
    let shared_eval_pack_ids: Vec<String> = current_best_row
        .eval_pack_ids
        .iter()
        .filter(|pack_id| candidate_row.eval_pack_ids.contains(pack_id))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if shared_eval_pack_ids.is_empty() {
        return Err(PsionExecutorLocalClusterDashboardError::MissingField {
            field: String::from(
                "psion_executor_local_cluster_dashboard.profile_comparison.shared_eval_pack_ids",
            ),
        });
    }
    let shared_run_search_key = current_best_row
        .search_run_ids
        .iter()
        .find(|run_id| candidate_row.search_run_ids.contains(run_id))
        .cloned()
        .ok_or_else(|| PsionExecutorLocalClusterDashboardError::MissingField {
            field: String::from(
                "psion_executor_local_cluster_dashboard.profile_comparison.shared_run_search_key",
            ),
        })?;
    let current_best_column = PsionExecutorLocalClusterDashboardProfileColumn {
        profile_id: current_best_row.admitted_profile_id.clone(),
        logical_role: String::from("current_best"),
        run_id: current_best_row.run_id.clone(),
        candidate_status: candidate_status_key(&current_best_row.candidate_status),
        final_mean_loss: current_best_row.metric_posture.final_mean_loss,
        observed_steps_per_second: current_best_row.metric_posture.observed_steps_per_second,
        budget_burn_ratio: current_best_row.cost_posture.budget_burn_ratio,
        export_status: current_best_row.export_status.clone(),
        recovery_status: current_best_row.recovery_status.clone(),
        detail: String::from(
            "The current-best column keeps the retained winner visible inside the shared local-cluster review surface without hiding which admitted profile currently owns the stronger row.",
        ),
    };
    let candidate_column = PsionExecutorLocalClusterDashboardProfileColumn {
        profile_id: candidate_row.admitted_profile_id.clone(),
        logical_role: String::from("candidate"),
        run_id: candidate_row.run_id.clone(),
        candidate_status: candidate_status_key(&candidate_row.candidate_status),
        final_mean_loss: candidate_row.metric_posture.final_mean_loss,
        observed_steps_per_second: candidate_row.metric_posture.observed_steps_per_second,
        budget_burn_ratio: candidate_row.cost_posture.budget_burn_ratio,
        export_status: candidate_row.export_status.clone(),
        recovery_status: candidate_row.recovery_status.clone(),
        detail: String::from(
            "The candidate column keeps the retained challenger visible inside the same side-by-side review surface while preserving its explicit export and recovery posture.",
        ),
    };
    let mut comparison = PsionExecutorLocalClusterDashboardProfileComparison {
        comparison_id: String::from(PROFILE_COMPARISON_ID),
        shared_eval_pack_ids,
        shared_run_search_key,
        candidate_to_current_best_steps_ratio: stable_ratio(
            candidate_row.metric_posture.observed_steps_per_second,
            current_best_row.metric_posture.observed_steps_per_second,
        ),
        columns: vec![current_best_column, candidate_column],
        detail: String::from(
            "The profile comparison keeps the admitted MLX and 4080 rows side-by-side under one shared device-matrix search key so review can compare throughput, export, recovery, and budget burn without reconstructing cross-packet context by hand.",
        ),
        comparison_digest: String::new(),
    };
    comparison.comparison_digest = stable_profile_comparison_digest(&comparison);
    Ok(comparison)
}

fn find_single_row<'a>(
    ledger: &'a PsionExecutorLocalClusterLedger,
    candidate_status: PsionExecutorLocalClusterCandidateStatus,
    label: &str,
) -> Result<&'a PsionExecutorLocalClusterLedgerRow, PsionExecutorLocalClusterDashboardError> {
    let rows = ledger.rows_for_candidate_status(candidate_status);
    if rows.len() != 1 {
        return Err(PsionExecutorLocalClusterDashboardError::InvalidValue {
            field: format!("psion_executor_local_cluster_dashboard.rows[{label}]"),
            detail: format!(
                "expected exactly one `{label}` row but found {}",
                rows.len()
            ),
        });
    }
    Ok(rows[0])
}

fn candidate_status_key(status: &PsionExecutorLocalClusterCandidateStatus) -> String {
    match status {
        PsionExecutorLocalClusterCandidateStatus::CurrentBest => String::from("current_best"),
        PsionExecutorLocalClusterCandidateStatus::Candidate => String::from("candidate"),
    }
}

fn stable_ratio(numerator: f64, denominator: f64) -> f64 {
    let raw = numerator / denominator;
    (raw * 1_000_000_000_000_000.0).round() / 1_000_000_000_000_000.0
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutorLocalClusterDashboardError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutorLocalClusterDashboardError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorLocalClusterDashboardError> {
    let bytes = read_bytes(workspace_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorLocalClusterDashboardError::Parse {
        path: relative_path.to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorLocalClusterDashboardError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorLocalClusterDashboardError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorLocalClusterDashboardError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorLocalClusterDashboardError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorLocalClusterDashboardError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_baseline_card_digest(card: &PsionExecutorLocalClusterDashboardBaselineCard) -> String {
    let mut clone = card.clone();
    clone.panel_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("baseline card serialization should succeed"),
    ))
}

fn stable_run_card_digest(card: &PsionExecutorLocalClusterDashboardRunCard) -> String {
    let mut clone = card.clone();
    clone.panel_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("run card serialization should succeed"),
    ))
}

fn stable_profile_comparison_digest(
    comparison: &PsionExecutorLocalClusterDashboardProfileComparison,
) -> String {
    let mut clone = comparison.clone();
    clone.comparison_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("profile comparison serialization should succeed"),
    ))
}

fn stable_dashboard_digest(packet: &PsionExecutorLocalClusterDashboardPacket) -> String {
    let mut clone = packet.clone();
    clone.dashboard_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("dashboard serialization should succeed"),
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
    fn builtin_executor_local_cluster_dashboard_is_valid(
    ) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        let root = workspace_root();
        let packet = builtin_executor_local_cluster_dashboard_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_local_cluster_dashboard_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        let root = workspace_root();
        let expected: PsionExecutorLocalClusterDashboardPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_local_cluster_dashboard_packet(root.as_path())?;
        if actual != expected {
            return Err(PsionExecutorLocalClusterDashboardError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn executor_local_cluster_dashboard_keeps_side_by_side_profiles(
    ) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        let root = workspace_root();
        let packet = builtin_executor_local_cluster_dashboard_packet(root.as_path())?;
        assert_eq!(packet.profile_comparison.columns.len(), 2);
        assert_eq!(
            packet.profile_comparison.columns[0].profile_id,
            LOCAL_4080_PROFILE_ID
        );
        assert_eq!(
            packet.profile_comparison.columns[1].profile_id,
            LOCAL_MAC_MLX_PROFILE_ID
        );
        assert_eq!(
            packet.current_best_card.export_status,
            "green"
        );
        assert_eq!(packet.candidate_card.export_status, "green");
        Ok(())
    }

    #[test]
    fn write_executor_local_cluster_dashboard_persists_current_truth(
    ) -> Result<(), PsionExecutorLocalClusterDashboardError> {
        let root = workspace_root();
        let packet = write_builtin_executor_local_cluster_dashboard_packet(root.as_path())?;
        let committed: PsionExecutorLocalClusterDashboardPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH,
        )?;
        assert_eq!(packet, committed);
        Ok(())
    }
}
