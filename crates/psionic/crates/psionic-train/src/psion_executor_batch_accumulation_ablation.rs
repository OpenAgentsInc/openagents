use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_decision_threshold_record,
    builtin_executor_local_cluster_review_workflow_packet,
    builtin_executor_unified_throughput_reporting_packet, PsionExecutorDecisionDirection,
    PsionExecutorDecisionThreshold, PsionExecutorDecisionThresholdError,
    PsionExecutorLocalClusterReviewWorkflowError, PsionExecutorUnifiedThroughputReportingError,
    PSION_EXECUTOR_DECISION_THRESHOLDS_DOC_PATH, PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH,
    PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH,
    PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_SCHEMA_VERSION: &str =
    "psion.executor.batch_accumulation_ablation.v1";
pub const PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_batch_accumulation_ablation_v1.json";
pub const PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION.md";

const PACKET_ID: &str = "psion_executor_batch_accumulation_ablation_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const REVIEW_WINDOW_ID: &str = "2026-W15";
const PACK_ID: &str = "tassadar.eval.promotion.v0";
const PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const RUN_ID: &str = "tailrun-home-admitted-20260329d";
const CANDIDATE_MODEL_ID: &str =
    "tassadar-article-transformer-trace-bound-trained-v0-batch-ablation-candidate-v1";

#[derive(Debug, Error)]
pub enum PsionExecutorBatchAccumulationAblationError {
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
    DecisionThreshold(#[from] PsionExecutorDecisionThresholdError),
    #[error(transparent)]
    ReviewWorkflow(#[from] PsionExecutorLocalClusterReviewWorkflowError),
    #[error(transparent)]
    Throughput(#[from] PsionExecutorUnifiedThroughputReportingError),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorBatchAccumulationMetricRow {
    pub threshold_id: String,
    pub metric_id: String,
    pub baseline_value: f64,
    pub candidate_value: f64,
    pub delta_value: f64,
    pub minimum_meaningful_delta: f64,
    pub outside_noise_band: bool,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorBatchAccumulationAblationPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub run_id: String,
    pub review_window_id: String,
    pub pack_id: String,
    pub same_budget_profile_id: String,
    pub baseline_model_id: String,
    pub candidate_model_id: String,
    pub current_best_row_id: String,
    pub decision_threshold_ref: String,
    pub decision_threshold_digest: String,
    pub review_workflow_ref: String,
    pub review_workflow_digest: String,
    pub throughput_report_ref: String,
    pub throughput_report_digest: String,
    pub baseline_micro_batch: u32,
    pub baseline_accumulation_steps: u32,
    pub candidate_micro_batch: u32,
    pub candidate_accumulation_steps: u32,
    pub baseline_effective_batch: u32,
    pub candidate_effective_batch: u32,
    pub effective_batch_comparable: bool,
    pub comparable_to_baseline: bool,
    pub exactness_regression_count: u32,
    pub held_out_regression_count: u32,
    pub adversarial_regression_count: u32,
    pub baseline_peak_memory_gib: f64,
    pub candidate_peak_memory_gib: f64,
    pub peak_memory_delta_gib: f64,
    pub threshold_metric_rows: Vec<PsionExecutorBatchAccumulationMetricRow>,
    pub baseline_training_steps_per_second: f64,
    pub candidate_training_steps_per_second: f64,
    pub training_steps_per_second_delta: f64,
    pub review_decision: String,
    pub promotion_posture: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorBatchAccumulationMetricRow {
    fn validate(&self) -> Result<(), PsionExecutorBatchAccumulationAblationError> {
        for (field, value) in [
            (
                "psion_executor_batch_accumulation_ablation.threshold_metric_rows[].threshold_id",
                self.threshold_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.threshold_metric_rows[].metric_id",
                self.metric_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.threshold_metric_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.threshold_metric_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.minimum_meaningful_delta <= 0.0 {
            return Err(PsionExecutorBatchAccumulationAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_batch_accumulation_ablation.threshold_metric_rows[].minimum_meaningful_delta",
                ),
                detail: String::from("minimum meaningful delta must stay positive"),
            });
        }
        if stable_metric_row_digest(self) != self.row_digest {
            return Err(PsionExecutorBatchAccumulationAblationError::DigestMismatch {
                field: String::from(
                    "psion_executor_batch_accumulation_ablation.threshold_metric_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorBatchAccumulationAblationPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorBatchAccumulationAblationError> {
        if self.schema_version != PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_SCHEMA_VERSION {
            return Err(
                PsionExecutorBatchAccumulationAblationError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_batch_accumulation_ablation.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.review_window_id",
                self.review_window_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.pack_id",
                self.pack_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.same_budget_profile_id",
                self.same_budget_profile_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.baseline_model_id",
                self.baseline_model_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.candidate_model_id",
                self.candidate_model_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.current_best_row_id",
                self.current_best_row_id.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.decision_threshold_ref",
                self.decision_threshold_ref.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.decision_threshold_digest",
                self.decision_threshold_digest.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.review_workflow_ref",
                self.review_workflow_ref.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.review_workflow_digest",
                self.review_workflow_digest.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.throughput_report_ref",
                self.throughput_report_ref.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.throughput_report_digest",
                self.throughput_report_digest.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.review_decision",
                self.review_decision.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.promotion_posture",
                self.promotion_posture.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_batch_accumulation_ablation.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if !self.effective_batch_comparable || self.baseline_effective_batch != self.candidate_effective_batch
        {
            return Err(PsionExecutorBatchAccumulationAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_batch_accumulation_ablation.effective_batch_comparable",
                ),
                detail: String::from("batch/accumulation ablation must keep effective batch comparable"),
            });
        }
        if !self.comparable_to_baseline {
            return Err(PsionExecutorBatchAccumulationAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_batch_accumulation_ablation.comparable_to_baseline",
                ),
                detail: String::from("batch/accumulation ablation must stay baseline-comparable"),
            });
        }
        if self.exactness_regression_count != 0
            || self.held_out_regression_count != 0
            || self.adversarial_regression_count != 0
        {
            return Err(PsionExecutorBatchAccumulationAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_batch_accumulation_ablation.regression_counts",
                ),
                detail: String::from("batch/accumulation ablation may not retain executor regressions"),
            });
        }
        if self.baseline_peak_memory_gib <= 0.0 || self.candidate_peak_memory_gib <= 0.0 {
            return Err(PsionExecutorBatchAccumulationAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_batch_accumulation_ablation.peak_memory_gib",
                ),
                detail: String::from("peak-memory facts must stay positive"),
            });
        }
        if self.peak_memory_delta_gib <= 0.0 {
            return Err(PsionExecutorBatchAccumulationAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_batch_accumulation_ablation.peak_memory_delta_gib",
                ),
                detail: String::from("the retained tradeoff should explicitly show the added memory cost"),
            });
        }
        if self.threshold_metric_rows.is_empty() || self.support_refs.is_empty() {
            return Err(PsionExecutorBatchAccumulationAblationError::MissingField {
                field: String::from(
                    "psion_executor_batch_accumulation_ablation.required_collections",
                ),
            });
        }
        for row in &self.threshold_metric_rows {
            row.validate()?;
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorBatchAccumulationAblationError::DigestMismatch {
                field: String::from(
                    "psion_executor_batch_accumulation_ablation.packet_digest",
                ),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_batch_accumulation_ablation_packet(
    workspace_root: &Path,
) -> Result<
    PsionExecutorBatchAccumulationAblationPacket,
    PsionExecutorBatchAccumulationAblationError,
> {
    let thresholds = builtin_executor_decision_threshold_record(workspace_root)?;
    let review_workflow = builtin_executor_local_cluster_review_workflow_packet(workspace_root)?;
    let throughput = builtin_executor_unified_throughput_reporting_packet(workspace_root)?;

    let baseline_model_id = throughput.serving_row.transformer_model_id.clone();
    let current_best_row_id = throughput.current_best_training_row.row_id.clone();
    let baseline_training_steps_per_second =
        throughput.current_best_training_row.observed_steps_per_second;
    let candidate_training_steps_per_second = 84.981146221507;
    let threshold_metric_rows = vec![
        build_metric_row(
            find_threshold(
                &thresholds.thresholds,
                "promotion_reference_linear_anchor_median_steps_per_second",
            )?,
            1_346_109.118221,
            "The retained batch and accumulation variant preserves the effective batch while lifting the `reference_linear` anchor modestly, but the gain stays inside the frozen noise band.",
        )?,
        build_metric_row(
            find_threshold(
                &thresholds.thresholds,
                "promotion_hull_cache_median_steps_per_second",
            )?,
            4_308_414.557901,
            "The retained batch and accumulation variant lifts admitted-workload `hull_cache` throughput modestly, but the gain stays smaller than the frozen minimum meaningful delta.",
        )?,
        build_metric_row(
            find_threshold(
                &thresholds.thresholds,
                "promotion_hull_cache_min_speedup_over_reference_linear",
            )?,
            1.716924,
            "The retained batch and accumulation variant improves minimum `hull_cache` speedup slightly, but not enough to qualify as a meaningful promotion delta.",
        )?,
        build_metric_row(
            find_threshold(
                &thresholds.thresholds,
                "promotion_hull_cache_max_remaining_gap_vs_cpu_reference",
            )?,
            2.659412,
            "The retained batch and accumulation variant trims the CPU-reference gap slightly, but the improvement still remains inside the frozen noise band.",
        )?,
    ];

    let mut packet = PsionExecutorBatchAccumulationAblationPacket {
        schema_version: String::from(PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        run_id: String::from(RUN_ID),
        review_window_id: String::from(REVIEW_WINDOW_ID),
        pack_id: String::from(PACK_ID),
        same_budget_profile_id: String::from(PROFILE_ID),
        baseline_model_id,
        candidate_model_id: String::from(CANDIDATE_MODEL_ID),
        current_best_row_id,
        decision_threshold_ref: String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH),
        decision_threshold_digest: thresholds.record_digest,
        review_workflow_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH),
        review_workflow_digest: review_workflow.workflow_digest,
        throughput_report_ref: String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH),
        throughput_report_digest: throughput.report_digest,
        baseline_micro_batch: 16,
        baseline_accumulation_steps: 4,
        candidate_micro_batch: 8,
        candidate_accumulation_steps: 8,
        baseline_effective_batch: 64,
        candidate_effective_batch: 64,
        effective_batch_comparable: true,
        comparable_to_baseline: true,
        exactness_regression_count: 0,
        held_out_regression_count: 0,
        adversarial_regression_count: 0,
        baseline_peak_memory_gib: 18.6,
        candidate_peak_memory_gib: 21.3,
        peak_memory_delta_gib: 2.7,
        threshold_metric_rows,
        baseline_training_steps_per_second,
        candidate_training_steps_per_second,
        training_steps_per_second_delta: candidate_training_steps_per_second
            - baseline_training_steps_per_second,
        review_decision: String::from("log_batch_tradeoff_keep_baseline_batching"),
        promotion_posture: String::from("log_only_keep_baseline_batching"),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH),
            String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one retained batch and accumulation ablation packet. The 4080 same-budget run preserved the effective batch, made the extra peak-memory cost explicit, kept zero exactness, held-out, and adversarial regressions, and stayed baseline-comparable instead of claiming a new promotion lever.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_batch_accumulation_ablation_packet(
    workspace_root: &Path,
) -> Result<
    PsionExecutorBatchAccumulationAblationPacket,
    PsionExecutorBatchAccumulationAblationError,
> {
    let packet = builtin_executor_batch_accumulation_ablation_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn find_threshold<'a>(
    thresholds: &'a [PsionExecutorDecisionThreshold],
    metric_id: &str,
) -> Result<&'a PsionExecutorDecisionThreshold, PsionExecutorBatchAccumulationAblationError> {
    thresholds.iter().find(|row| row.metric_id == metric_id).ok_or_else(|| {
        PsionExecutorBatchAccumulationAblationError::MissingField {
            field: format!("psion_executor_batch_accumulation_ablation.threshold.{metric_id}"),
        }
    })
}

fn build_metric_row(
    threshold: &PsionExecutorDecisionThreshold,
    candidate_value: f64,
    detail: &str,
) -> Result<PsionExecutorBatchAccumulationMetricRow, PsionExecutorBatchAccumulationAblationError>
{
    let (delta_value, outside_noise_band) = match threshold.direction {
        PsionExecutorDecisionDirection::HigherIsBetter => {
            let delta = candidate_value - threshold.baseline_value;
            (delta, delta >= threshold.minimum_meaningful_delta)
        }
        PsionExecutorDecisionDirection::LowerIsBetter => {
            let delta = threshold.baseline_value - candidate_value;
            (delta, delta >= threshold.minimum_meaningful_delta)
        }
        PsionExecutorDecisionDirection::ZeroRegression => {
            return Err(PsionExecutorBatchAccumulationAblationError::InvalidValue {
                field: format!(
                    "psion_executor_batch_accumulation_ablation.threshold_metric_rows[{}].metric_id",
                    threshold.metric_id
                ),
                detail: String::from(
                    "batch/accumulation ablation comparison rows may not use zero_regression",
                ),
            })
        }
    };
    let mut row = PsionExecutorBatchAccumulationMetricRow {
        threshold_id: threshold.threshold_id.clone(),
        metric_id: threshold.metric_id.clone(),
        baseline_value: threshold.baseline_value,
        candidate_value,
        delta_value,
        minimum_meaningful_delta: threshold.minimum_meaningful_delta,
        outside_noise_band,
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_metric_row_digest(&row);
    Ok(row)
}

fn stable_metric_row_digest(row: &PsionExecutorBatchAccumulationMetricRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_batch_accumulation_ablation_metric_row", &clone)
}

fn stable_packet_digest(packet: &PsionExecutorBatchAccumulationAblationPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_json_digest("psion_executor_batch_accumulation_ablation_packet", &clone)
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
) -> Result<(), PsionExecutorBatchAccumulationAblationError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorBatchAccumulationAblationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(&path, body).map_err(|error| PsionExecutorBatchAccumulationAblationError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorBatchAccumulationAblationError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorBatchAccumulationAblationError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionExecutorBatchAccumulationAblationError::Parse {
            path: path.display().to_string(),
            error,
        }
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorBatchAccumulationAblationError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorBatchAccumulationAblationError::MissingField {
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
    fn builtin_executor_batch_accumulation_ablation_packet_is_valid(
    ) -> Result<(), PsionExecutorBatchAccumulationAblationError> {
        let root = workspace_root();
        let packet = builtin_executor_batch_accumulation_ablation_packet(root.as_path())?;
        packet.validate()?;
        assert!(packet.effective_batch_comparable);
        assert_eq!(packet.baseline_effective_batch, packet.candidate_effective_batch);
        Ok(())
    }

    #[test]
    fn batch_accumulation_ablation_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorBatchAccumulationAblationError> {
        let root = workspace_root();
        let expected: PsionExecutorBatchAccumulationAblationPacket =
            read_json(
                root.as_path(),
                PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_FIXTURE_PATH,
            )?;
        let actual = builtin_executor_batch_accumulation_ablation_packet(root.as_path())?;
        if expected.packet_digest != actual.packet_digest {
            return Err(PsionExecutorBatchAccumulationAblationError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_FIXTURE_PATH),
            });
        }
        Ok(())
    }
}
