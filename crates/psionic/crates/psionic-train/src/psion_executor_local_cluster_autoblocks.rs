use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_4080_frequent_eval_attachment_packet, builtin_executor_baseline_truth_record,
    builtin_executor_local_cluster_dashboard_packet, builtin_executor_local_cluster_ledger,
    builtin_executor_local_cluster_roundtrip_packet,
    PsionExecutor4080FrequentEvalAttachmentError, PsionExecutorBaselineTruthError,
    PsionExecutorLocalClusterCandidateStatus, PsionExecutorLocalClusterDashboardError,
    PsionExecutorLocalClusterDashboardPacket, PsionExecutorLocalClusterLedgerError,
    PsionExecutorLocalClusterRoundtripError,
    PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH,
    PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH,
};

/// Stable schema version for the canonical local-cluster auto-block report.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_SCHEMA_VERSION: &str =
    "psion.executor.local_cluster_autoblocks.v1";
/// Canonical fixture path for the local-cluster auto-block report.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_local_cluster_autoblocks_v1.json";
/// Canonical doc path for the local-cluster auto-block report.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS.md";

const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH: &str = "docs/PSION_EXECUTOR_BASELINE_TRUTH.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP.md";
const PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT.md";

#[derive(Debug, Error)]
pub enum PsionExecutorLocalClusterAutoblocksError {
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
    Dashboard(#[from] PsionExecutorLocalClusterDashboardError),
    #[error(transparent)]
    Ledger(#[from] PsionExecutorLocalClusterLedgerError),
    #[error(transparent)]
    Roundtrip(#[from] PsionExecutorLocalClusterRoundtripError),
    #[error(transparent)]
    FrequentEval(#[from] PsionExecutor4080FrequentEvalAttachmentError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorLocalClusterAutoblockScope {
    PhaseExitAndPromotion,
    PromotionOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterAutoblockRow {
    pub block_id: String,
    pub scope: PsionExecutorLocalClusterAutoblockScope,
    pub status: String,
    pub owner_surface_ref: String,
    pub owner_surface_digest: String,
    pub detail: String,
    pub block_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterAutoblocksReport {
    pub schema_version: String,
    pub report_id: String,
    pub current_best_row_id: String,
    pub dashboard_ref: String,
    pub dashboard_digest: String,
    pub ledger_ref: String,
    pub ledger_digest: String,
    pub baseline_truth_ref: String,
    pub baseline_truth_digest: String,
    pub frequent_eval_ref: String,
    pub frequent_eval_digest: String,
    pub phase_exit_blocked: bool,
    pub promotion_blocked: bool,
    pub active_phase_exit_block_ids: Vec<String>,
    pub active_promotion_block_ids: Vec<String>,
    pub block_rows: Vec<PsionExecutorLocalClusterAutoblockRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub report_digest: String,
}

impl PsionExecutorLocalClusterAutoblockRow {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterAutoblocksError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_autoblocks.block_rows[].block_id",
                self.block_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.block_rows[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.block_rows[].owner_surface_ref",
                self.owner_surface_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.block_rows[].owner_surface_digest",
                self.owner_surface_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.block_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.block_rows[].block_digest",
                self.block_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_block_row_digest(self) != self.block_digest {
            return Err(PsionExecutorLocalClusterAutoblocksError::DigestMismatch {
                field: String::from(
                    "psion_executor_local_cluster_autoblocks.block_rows[].block_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterAutoblocksReport {
    pub fn validate(&self) -> Result<(), PsionExecutorLocalClusterAutoblocksError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_local_cluster_autoblocks.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_SCHEMA_VERSION {
            return Err(
                PsionExecutorLocalClusterAutoblocksError::SchemaVersionMismatch {
                    expected: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_local_cluster_autoblocks.report_id",
                self.report_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.current_best_row_id",
                self.current_best_row_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.dashboard_ref",
                self.dashboard_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.dashboard_digest",
                self.dashboard_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.ledger_ref",
                self.ledger_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.ledger_digest",
                self.ledger_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.baseline_truth_ref",
                self.baseline_truth_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.baseline_truth_digest",
                self.baseline_truth_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.frequent_eval_ref",
                self.frequent_eval_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.frequent_eval_digest",
                self.frequent_eval_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_local_cluster_autoblocks.report_digest",
                self.report_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.block_rows.is_empty() {
            return Err(PsionExecutorLocalClusterAutoblocksError::MissingField {
                field: String::from("psion_executor_local_cluster_autoblocks.block_rows"),
            });
        }
        for row in &self.block_rows {
            row.validate()?;
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorLocalClusterAutoblocksError::MissingField {
                field: String::from("psion_executor_local_cluster_autoblocks.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(
                support_ref.as_str(),
                "psion_executor_local_cluster_autoblocks.support_refs[]",
            )?;
        }
        if stable_report_digest(self) != self.report_digest {
            return Err(PsionExecutorLocalClusterAutoblocksError::DigestMismatch {
                field: String::from("psion_executor_local_cluster_autoblocks.report_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_local_cluster_autoblocks_report(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterAutoblocksReport, PsionExecutorLocalClusterAutoblocksError> {
    let dashboard = builtin_executor_local_cluster_dashboard_packet(workspace_root)?;
    let ledger = builtin_executor_local_cluster_ledger(workspace_root)?;
    let baseline_truth = builtin_executor_baseline_truth_record(workspace_root)?;
    let frequent_eval = builtin_executor_4080_frequent_eval_attachment_packet(workspace_root)?;
    let roundtrip = builtin_executor_local_cluster_roundtrip_packet(workspace_root)?;

    let current_best_row = ledger
        .rows_for_candidate_status(PsionExecutorLocalClusterCandidateStatus::CurrentBest)
        .into_iter()
        .next()
        .ok_or_else(|| PsionExecutorLocalClusterAutoblocksError::MissingField {
            field: String::from("psion_executor_local_cluster_autoblocks.current_best_row"),
        })?;

    let mut block_rows = vec![
        build_eval_autoblock_row(
            &frequent_eval.packet_digest,
            current_best_row.run_id.as_str(),
            &frequent_eval,
        )?,
        build_recovery_autoblock_row(&ledger.ledger_digest, current_best_row)?,
        build_export_autoblock_row(&roundtrip.packet_digest, &roundtrip, current_best_row)?,
        build_reference_linear_autoblock_row(
            &baseline_truth.record_digest,
            &dashboard,
            &baseline_truth,
        )?,
    ];

    for row in &mut block_rows {
        row.block_digest = stable_block_row_digest(row);
    }

    let active_phase_exit_block_ids = block_rows
        .iter()
        .filter(|row| row_blocks_phase_exit(row))
        .map(|row| row.block_id.clone())
        .collect::<Vec<_>>();
    let active_promotion_block_ids = block_rows
        .iter()
        .filter(|row| row_blocks_promotion(row))
        .map(|row| row.block_id.clone())
        .collect::<Vec<_>>();

    let mut report = PsionExecutorLocalClusterAutoblocksReport {
        schema_version: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_SCHEMA_VERSION),
        report_id: String::from("psion_executor_local_cluster_autoblocks_v1"),
        current_best_row_id: current_best_row.row_id.clone(),
        dashboard_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH),
        dashboard_digest: dashboard.dashboard_digest,
        ledger_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
        ledger_digest: ledger.ledger_digest,
        baseline_truth_ref: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
        baseline_truth_digest: baseline_truth.record_digest,
        frequent_eval_ref: String::from(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH),
        frequent_eval_digest: frequent_eval.packet_digest,
        phase_exit_blocked: !active_phase_exit_block_ids.is_empty(),
        promotion_blocked: !active_promotion_block_ids.is_empty(),
        active_phase_exit_block_ids,
        active_promotion_block_ids,
        block_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_DOC_PATH),
        ],
        summary: String::from(
            "The admitted executor lane now has one canonical auto-block report for local-cluster promotion and phase exits. Missing eval, recovery, export, and `reference_linear` anchor facts are now machine-readable block rows instead of narrative review notes, and promotion-only eval blockers stay distinct from cluster-closure facts.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_report_digest(&report);
    report.validate()?;
    Ok(report)
}

pub fn write_builtin_executor_local_cluster_autoblocks_report(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterAutoblocksReport, PsionExecutorLocalClusterAutoblocksError> {
    let report = builtin_executor_local_cluster_autoblocks_report(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH,
        &report,
    )?;
    Ok(report)
}

fn build_eval_autoblock_row(
    frequent_eval_digest: &str,
    run_id: &str,
    frequent_eval: &crate::PsionExecutor4080FrequentEvalAttachmentPacket,
) -> Result<PsionExecutorLocalClusterAutoblockRow, PsionExecutorLocalClusterAutoblocksError> {
    let status = if frequent_eval.missing_eval_blocks_promotion
        || frequent_eval.checkpoint_eval_row.promotion_blocked
    {
        String::from("blocked_missing_eval_fact")
    } else {
        String::from("green")
    };
    Ok(PsionExecutorLocalClusterAutoblockRow {
        block_id: String::from("missing_eval_fact_current_best"),
        scope: PsionExecutorLocalClusterAutoblockScope::PromotionOnly,
        status,
        owner_surface_ref: String::from(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH),
        owner_surface_digest: String::from(frequent_eval_digest),
        detail: format!(
            "The current-best row for run `{}` inherits the retained frequent-eval blocker posture. Missing or unscored frequent-pack coverage still leaves blocker ids [{}] active, so promotion stays blocked until those eval facts turn green even after cluster closure is otherwise complete.",
            run_id,
            frequent_eval
                .checkpoint_eval_row
                .promotion_blocker_ids
                .join(", ")
        ),
        block_digest: String::new(),
    })
}

fn build_recovery_autoblock_row(
    ledger_digest: &str,
    current_best_row: &crate::PsionExecutorLocalClusterLedgerRow,
) -> Result<PsionExecutorLocalClusterAutoblockRow, PsionExecutorLocalClusterAutoblocksError> {
    let recovery_is_green = matches!(
        current_best_row.recovery_status.as_str(),
        "green" | "not_required_same_node"
    );
    Ok(PsionExecutorLocalClusterAutoblockRow {
        block_id: String::from("missing_recovery_fact_current_best"),
        scope: PsionExecutorLocalClusterAutoblockScope::PhaseExitAndPromotion,
        status: if recovery_is_green {
            String::from("green")
        } else {
            String::from("blocked_missing_recovery_fact")
        },
        owner_surface_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
        owner_surface_digest: String::from(ledger_digest),
        detail: format!(
            "The current-best row `{}` keeps recovery status `{}`. Anything outside `green` or `not_required_same_node` blocks both phase exit and promotion automatically.",
            current_best_row.row_id, current_best_row.recovery_status
        ),
        block_digest: String::new(),
    })
}

fn build_export_autoblock_row(
    roundtrip_digest: &str,
    roundtrip: &crate::PsionExecutorLocalClusterRoundtripPacket,
    current_best_row: &crate::PsionExecutorLocalClusterLedgerRow,
) -> Result<PsionExecutorLocalClusterAutoblockRow, PsionExecutorLocalClusterAutoblocksError> {
    Ok(PsionExecutorLocalClusterAutoblockRow {
        block_id: String::from("missing_export_fact_current_best"),
        scope: PsionExecutorLocalClusterAutoblockScope::PhaseExitAndPromotion,
        status: if current_best_row.export_status == "green" {
            String::from("green")
        } else {
            String::from("blocked_missing_export_fact")
        },
        owner_surface_ref: String::from(crate::PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH),
        owner_surface_digest: String::from(roundtrip_digest),
        detail: format!(
            "The roundtrip closeout packet keeps the current-best export posture explicit as `{}` on row `{}`. Until that turns green, the local-cluster phase exit and promotion gates stay blocked instead of relying on review memory. Current cluster closure status is `{}`.",
            current_best_row.export_status,
            current_best_row.row_id,
            roundtrip.cluster_closure_status
        ),
        block_digest: String::new(),
    })
}

fn build_reference_linear_autoblock_row(
    baseline_truth_digest: &str,
    dashboard: &PsionExecutorLocalClusterDashboardPacket,
    baseline_truth: &crate::PsionExecutorBaselineTruthRecord,
) -> Result<PsionExecutorLocalClusterAutoblockRow, PsionExecutorLocalClusterAutoblocksError> {
    let anchor_present = !dashboard
        .baseline_card
        .reference_linear_truth_anchor
        .trim()
        .is_empty()
        && dashboard.baseline_truth_digest == baseline_truth.record_digest
        && baseline_truth
            .suite_truths
            .iter()
            .any(|suite| suite.aggregate_green);
    Ok(PsionExecutorLocalClusterAutoblockRow {
        block_id: String::from("missing_reference_linear_anchor"),
        scope: PsionExecutorLocalClusterAutoblockScope::PhaseExitAndPromotion,
        status: if anchor_present {
            String::from("green")
        } else {
            String::from("blocked_missing_reference_linear_anchor")
        },
        owner_surface_ref: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
        owner_surface_digest: String::from(baseline_truth_digest),
        detail: String::from(
            "The frozen baseline still keeps `reference_linear` explicit as the measured truth anchor for the admitted executor family. If that anchor disappears, both phase exit and promotion stay blocked automatically.",
        ),
        block_digest: String::new(),
    })
}

fn row_blocks_phase_exit(row: &PsionExecutorLocalClusterAutoblockRow) -> bool {
    row.status != "green"
        && matches!(row.scope, PsionExecutorLocalClusterAutoblockScope::PhaseExitAndPromotion)
}

fn row_blocks_promotion(row: &PsionExecutorLocalClusterAutoblockRow) -> bool {
    row.status != "green"
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutorLocalClusterAutoblocksError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutorLocalClusterAutoblocksError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorLocalClusterAutoblocksError> {
    let bytes = read_bytes(workspace_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionExecutorLocalClusterAutoblocksError::Parse {
            path: relative_path.to_string(),
            error,
        }
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorLocalClusterAutoblocksError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorLocalClusterAutoblocksError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorLocalClusterAutoblocksError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorLocalClusterAutoblocksError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorLocalClusterAutoblocksError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_block_row_digest(row: &PsionExecutorLocalClusterAutoblockRow) -> String {
    let mut clone = row.clone();
    clone.block_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("autoblock row serialization should succeed"),
    ))
}

fn stable_report_digest(report: &PsionExecutorLocalClusterAutoblocksReport) -> String {
    let mut clone = report.clone();
    clone.report_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("autoblock report serialization should succeed"),
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
    fn builtin_executor_local_cluster_autoblocks_report_is_valid(
    ) -> Result<(), PsionExecutorLocalClusterAutoblocksError> {
        let root = workspace_root();
        let report = builtin_executor_local_cluster_autoblocks_report(root.as_path())?;
        report.validate()?;
        Ok(())
    }

    #[test]
    fn executor_local_cluster_autoblocks_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorLocalClusterAutoblocksError> {
        let root = workspace_root();
        let expected: PsionExecutorLocalClusterAutoblocksReport = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_local_cluster_autoblocks_report(root.as_path())?;
        if actual != expected {
            return Err(PsionExecutorLocalClusterAutoblocksError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn executor_local_cluster_autoblocks_keep_phase_exit_green_but_promotion_eval_blocked(
    ) -> Result<(), PsionExecutorLocalClusterAutoblocksError> {
        let root = workspace_root();
        let report = builtin_executor_local_cluster_autoblocks_report(root.as_path())?;
        assert!(!report.phase_exit_blocked);
        assert!(report.promotion_blocked);
        assert!(report.active_phase_exit_block_ids.is_empty());
        assert!(report
            .active_promotion_block_ids
            .contains(&String::from("missing_eval_fact_current_best")));
        Ok(())
    }

    #[test]
    fn write_executor_local_cluster_autoblocks_persists_current_truth(
    ) -> Result<(), PsionExecutorLocalClusterAutoblocksError> {
        let root = workspace_root();
        let report = write_builtin_executor_local_cluster_autoblocks_report(root.as_path())?;
        let committed: PsionExecutorLocalClusterAutoblocksReport = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH,
        )?;
        assert_eq!(report, committed);
        Ok(())
    }
}
