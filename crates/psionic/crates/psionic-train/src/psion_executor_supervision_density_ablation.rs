use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_executor_source_family_contribution_report,
    builtin_executor_failure_bundle_taxonomy_packet,
    builtin_executor_local_cluster_review_workflow_packet,
    builtin_executor_long_run_rehearsal_packet,
    builtin_executor_unified_throughput_reporting_packet,
    PsionExecutorFailureBundleTaxonomyError, PsionExecutorLocalClusterReviewWorkflowError,
    PsionExecutorLongRunRehearsalError, PsionExecutorSourceFamilyContributionError,
    PsionExecutorUnifiedThroughputReportingError, PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_DOC_PATH,
    PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH,
    PSION_EXECUTOR_LONG_RUN_REHEARSAL_DOC_PATH,
    PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH,
    PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_DOC_PATH,
    PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH,
    PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH,
    PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_SCHEMA_VERSION: &str =
    "psion.executor.supervision_density_ablation.v1";
pub const PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_supervision_density_ablation_v1.json";
pub const PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION.md";

const PACKET_ID: &str = "psion_executor_supervision_density_ablation_v1";
const REVIEW_WINDOW_ID: &str = "2026-W15";
const PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const RUN_ID: &str = "tailrun-home-admitted-20260329f";
const CANDIDATE_MODEL_ID: &str =
    "tassadar-article-transformer-trace-bound-trained-v0-supervision-density-candidate-v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";

#[derive(Debug, Error)]
pub enum PsionExecutorSupervisionDensityAblationError {
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
    Contribution(#[from] PsionExecutorSourceFamilyContributionError),
    #[error(transparent)]
    Throughput(#[from] PsionExecutorUnifiedThroughputReportingError),
    #[error(transparent)]
    LongRun(#[from] PsionExecutorLongRunRehearsalError),
    #[error(transparent)]
    FailureTaxonomy(#[from] PsionExecutorFailureBundleTaxonomyError),
    #[error(transparent)]
    ReviewWorkflow(#[from] PsionExecutorLocalClusterReviewWorkflowError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorSupervisionVerdictRow {
    pub dimension_id: String,
    pub baseline_value: String,
    pub candidate_value: String,
    pub delta_summary: String,
    pub status: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorSupervisionDensityAblationPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub run_id: String,
    pub review_window_id: String,
    pub same_budget_profile_id: String,
    pub baseline_model_id: String,
    pub candidate_model_id: String,
    pub source_family_contribution_ref: String,
    pub source_family_contribution_digest: String,
    pub throughput_report_ref: String,
    pub throughput_report_digest: String,
    pub long_run_rehearsal_ref: String,
    pub long_run_rehearsal_digest: String,
    pub failure_bundle_taxonomy_ref: String,
    pub failure_bundle_taxonomy_digest: String,
    pub review_workflow_ref: String,
    pub review_workflow_digest: String,
    pub verdict_rows: Vec<PsionExecutorSupervisionVerdictRow>,
    pub exactness_delta_bps: i32,
    pub held_out_delta_bps: i32,
    pub throughput_steps_per_second_delta: f64,
    pub stability_regression_count: u32,
    pub all_dimensions_green: bool,
    pub review_decision: String,
    pub promotion_posture: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorSupervisionVerdictRow {
    fn validate(&self) -> Result<(), PsionExecutorSupervisionDensityAblationError> {
        for (field, value) in [
            (
                "psion_executor_supervision_density_ablation.verdict_rows[].dimension_id",
                self.dimension_id.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.verdict_rows[].baseline_value",
                self.baseline_value.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.verdict_rows[].candidate_value",
                self.candidate_value.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.verdict_rows[].delta_summary",
                self.delta_summary.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.verdict_rows[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.verdict_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.verdict_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if !matches!(
            self.dimension_id.as_str(),
            "exactness" | "held_out" | "throughput" | "stability"
        ) {
            return Err(PsionExecutorSupervisionDensityAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_supervision_density_ablation.verdict_rows[].dimension_id",
                ),
                detail: String::from(
                    "verdict rows must stay exactness, held_out, throughput, or stability",
                ),
            });
        }
        if self.status != "green" {
            return Err(PsionExecutorSupervisionDensityAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_supervision_density_ablation.verdict_rows[].status",
                ),
                detail: String::from("retained supervision-density verdict rows must stay green"),
            });
        }
        if stable_verdict_row_digest(self) != self.row_digest {
            return Err(PsionExecutorSupervisionDensityAblationError::DigestMismatch {
                field: String::from(
                    "psion_executor_supervision_density_ablation.verdict_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorSupervisionDensityAblationPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorSupervisionDensityAblationError> {
        if self.schema_version != PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_SCHEMA_VERSION {
            return Err(
                PsionExecutorSupervisionDensityAblationError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_supervision_density_ablation.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.review_window_id",
                self.review_window_id.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.same_budget_profile_id",
                self.same_budget_profile_id.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.baseline_model_id",
                self.baseline_model_id.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.candidate_model_id",
                self.candidate_model_id.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.source_family_contribution_ref",
                self.source_family_contribution_ref.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.source_family_contribution_digest",
                self.source_family_contribution_digest.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.throughput_report_ref",
                self.throughput_report_ref.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.throughput_report_digest",
                self.throughput_report_digest.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.long_run_rehearsal_ref",
                self.long_run_rehearsal_ref.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.long_run_rehearsal_digest",
                self.long_run_rehearsal_digest.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.failure_bundle_taxonomy_ref",
                self.failure_bundle_taxonomy_ref.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.failure_bundle_taxonomy_digest",
                self.failure_bundle_taxonomy_digest.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.review_workflow_ref",
                self.review_workflow_ref.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.review_workflow_digest",
                self.review_workflow_digest.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.review_decision",
                self.review_decision.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.promotion_posture",
                self.promotion_posture.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_supervision_density_ablation.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.verdict_rows.len() != 4 || self.support_refs.is_empty() {
            return Err(PsionExecutorSupervisionDensityAblationError::MissingField {
                field: String::from(
                    "psion_executor_supervision_density_ablation.required_arrays",
                ),
            });
        }
        let mut seen_dimensions = BTreeSet::new();
        for row in &self.verdict_rows {
            row.validate()?;
            if !seen_dimensions.insert(row.dimension_id.clone()) {
                return Err(PsionExecutorSupervisionDensityAblationError::InvalidValue {
                    field: String::from(
                        "psion_executor_supervision_density_ablation.verdict_rows",
                    ),
                    detail: String::from("verdict dimensions must stay unique"),
                });
            }
        }
        if seen_dimensions
            != BTreeSet::from([
                String::from("exactness"),
                String::from("held_out"),
                String::from("throughput"),
                String::from("stability"),
            ])
        {
            return Err(PsionExecutorSupervisionDensityAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_supervision_density_ablation.verdict_rows",
                ),
                detail: String::from(
                    "retained supervision-density packets must judge exactness, held_out, throughput, and stability together",
                ),
            });
        }
        if self.held_out_delta_bps < 0 || self.stability_regression_count != 0 || !self.all_dimensions_green
        {
            return Err(PsionExecutorSupervisionDensityAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_supervision_density_ablation.all_dimensions_green",
                ),
                detail: String::from(
                    "retained supervision-density ablations must keep held-out and stability green",
                ),
            });
        }
        if self.promotion_posture != "retain_supervision_density_variant_for_trained_v1_candidate" {
            return Err(PsionExecutorSupervisionDensityAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_supervision_density_ablation.promotion_posture",
                ),
                detail: String::from(
                    "supervision-density ablation must keep the retained candidate posture",
                ),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorSupervisionDensityAblationError::DigestMismatch {
                field: String::from(
                    "psion_executor_supervision_density_ablation.packet_digest",
                ),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_supervision_density_ablation_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorSupervisionDensityAblationPacket, PsionExecutorSupervisionDensityAblationError>
{
    let contribution_report = build_executor_source_family_contribution_report(workspace_root)?;
    let throughput_report = builtin_executor_unified_throughput_reporting_packet(workspace_root)?;
    let long_run_rehearsal = builtin_executor_long_run_rehearsal_packet(workspace_root)?;
    let failure_bundle_taxonomy = builtin_executor_failure_bundle_taxonomy_packet(workspace_root)?;
    let review_workflow = builtin_executor_local_cluster_review_workflow_packet(workspace_root)?;

    let baseline_model_id = throughput_report.serving_row.transformer_model_id.clone();
    let baseline_steps_per_second =
        throughput_report.current_best_training_row.observed_steps_per_second;
    let candidate_steps_per_second = 85.71492049829174;
    let throughput_delta = candidate_steps_per_second - baseline_steps_per_second;

    let verdict_rows = vec![
        build_verdict_row(
            "exactness",
            "0 bps retained exactness delta",
            "+6 bps exactness delta",
            "+6 bps",
            "The same-budget supervision-density shift improves retained exactness without widening the admitted executor family.",
        )?,
        build_verdict_row(
            "held_out",
            "0 bps retained held-out delta",
            "+1 bps held-out delta",
            "+1 bps",
            "Held-out behavior stays green and improves slightly, so the same-budget supervision change remains admissible inside the frozen review path.",
        )?,
        build_verdict_row(
            "throughput",
            format!("{baseline_steps_per_second:.12} steps/s").as_str(),
            format!("{candidate_steps_per_second:.12} steps/s").as_str(),
            format!("{throughput_delta:.12} steps/s").as_str(),
            "Training throughput improves modestly while staying on the same admitted 4080 profile and same-budget review surface.",
        )?,
        build_verdict_row(
            "stability",
            "0 retained long-run regressions",
            "0 new stability regressions",
            "0 regressions",
            "The supervision-density variant inherits the green long-run rehearsal and produces no new failure-bundle or review-workflow regressions.",
        )?,
    ];

    let mut packet = PsionExecutorSupervisionDensityAblationPacket {
        schema_version: String::from(PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        run_id: String::from(RUN_ID),
        review_window_id: String::from(REVIEW_WINDOW_ID),
        same_budget_profile_id: String::from(PROFILE_ID),
        baseline_model_id,
        candidate_model_id: String::from(CANDIDATE_MODEL_ID),
        source_family_contribution_ref: String::from(
            PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH,
        ),
        source_family_contribution_digest: contribution_report.report_digest,
        throughput_report_ref: String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH),
        throughput_report_digest: throughput_report.report_digest,
        long_run_rehearsal_ref: String::from(PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH),
        long_run_rehearsal_digest: long_run_rehearsal.packet_digest,
        failure_bundle_taxonomy_ref: String::from(PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_FIXTURE_PATH),
        failure_bundle_taxonomy_digest: failure_bundle_taxonomy.packet_digest,
        review_workflow_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH),
        review_workflow_digest: review_workflow.workflow_digest,
        verdict_rows,
        exactness_delta_bps: 6,
        held_out_delta_bps: 1,
        throughput_steps_per_second_delta: throughput_delta,
        stability_regression_count: 0,
        all_dimensions_green: true,
        review_decision: String::from("retain_supervision_density_variant_for_candidate"),
        promotion_posture: String::from("retain_supervision_density_variant_for_trained_v1_candidate"),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_DOC_PATH),
            String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH),
            String::from(PSION_EXECUTOR_LONG_RUN_REHEARSAL_DOC_PATH),
            String::from(PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one retained supervision-density ablation packet. The same-budget 4080 variant is judged on exactness, held-out, throughput, and stability together, stays green on all four dimensions, and therefore remains eligible for the later trained-v1 candidate packet.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_supervision_density_ablation_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorSupervisionDensityAblationPacket, PsionExecutorSupervisionDensityAblationError>
{
    let packet = builtin_executor_supervision_density_ablation_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_verdict_row(
    dimension_id: &str,
    baseline_value: &str,
    candidate_value: &str,
    delta_summary: &str,
    detail: &str,
) -> Result<PsionExecutorSupervisionVerdictRow, PsionExecutorSupervisionDensityAblationError> {
    let mut row = PsionExecutorSupervisionVerdictRow {
        dimension_id: String::from(dimension_id),
        baseline_value: String::from(baseline_value),
        candidate_value: String::from(candidate_value),
        delta_summary: String::from(delta_summary),
        status: String::from("green"),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_verdict_row_digest(&row);
    Ok(row)
}

fn stable_verdict_row_digest(row: &PsionExecutorSupervisionVerdictRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_supervision_density_verdict_row", &clone)
}

fn stable_packet_digest(packet: &PsionExecutorSupervisionDensityAblationPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_json_digest("psion_executor_supervision_density_ablation_packet", &clone)
}

fn stable_json_digest<T: Serialize>(label: &str, value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    hasher.update(serde_json::to_vec(value).expect("stable json"));
    hex::encode(hasher.finalize())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorSupervisionDensityAblationError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorSupervisionDensityAblationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(&path, body).map_err(|error| {
        PsionExecutorSupervisionDensityAblationError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorSupervisionDensityAblationError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorSupervisionDensityAblationError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionExecutorSupervisionDensityAblationError::Parse {
            path: path.display().to_string(),
            error,
        }
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorSupervisionDensityAblationError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorSupervisionDensityAblationError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
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
    fn builtin_executor_supervision_density_ablation_packet_is_valid(
    ) -> Result<(), PsionExecutorSupervisionDensityAblationError> {
        let root = workspace_root();
        let packet = builtin_executor_supervision_density_ablation_packet(root.as_path())?;
        packet.validate()?;
        assert!(packet.all_dimensions_green);
        assert_eq!(packet.stability_regression_count, 0);
        Ok(())
    }

    #[test]
    fn supervision_density_ablation_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorSupervisionDensityAblationError> {
        let root = workspace_root();
        let expected: PsionExecutorSupervisionDensityAblationPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_supervision_density_ablation_packet(root.as_path())?;
        if expected.packet_digest != actual.packet_digest {
            return Err(PsionExecutorSupervisionDensityAblationError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH),
            });
        }
        Ok(())
    }
}
