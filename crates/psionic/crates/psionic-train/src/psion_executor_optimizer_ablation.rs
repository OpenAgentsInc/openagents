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

pub const PSION_EXECUTOR_OPTIMIZER_ABLATION_SCHEMA_VERSION: &str =
    "psion.executor.optimizer_ablation.v1";
pub const PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_optimizer_ablation_v1.json";
pub const PSION_EXECUTOR_OPTIMIZER_ABLATION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_OPTIMIZER_ABLATION.md";

const PACKET_ID: &str = "psion_executor_optimizer_ablation_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const REVIEW_WINDOW_ID: &str = "2026-W15";
const PACK_ID: &str = "tassadar.eval.promotion.v0";
const BASELINE_OPTIMIZER_ID: &str = "adamw_beta2_0.95_weight_decay_0.10_eps_1e-08";
const CANDIDATE_OPTIMIZER_ID: &str = "adamw_beta2_0.98_weight_decay_0.10_eps_1e-08";
const PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const INITIAL_RUN_ID: &str = "tailrun-home-admitted-20260329a";
const REPEAT_RUN_ID: &str = "tailrun-home-admitted-20260329b";
const CANDIDATE_MODEL_ID: &str =
    "tassadar-article-transformer-trace-bound-trained-v0-optimizer-ablation-candidate-v1";

#[derive(Debug, Error)]
pub enum PsionExecutorOptimizerAblationError {
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
pub struct PsionExecutorOptimizerAblationMetricRow {
    pub threshold_id: String,
    pub suite_id: String,
    pub metric_id: String,
    pub direction: String,
    pub baseline_value: f64,
    pub candidate_value: f64,
    pub delta_value: f64,
    pub minimum_meaningful_delta: f64,
    pub outside_noise_band: bool,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorOptimizerRepeatConfirmation {
    pub initial_run_id: String,
    pub repeat_run_id: String,
    pub confirmed_threshold_ids: Vec<String>,
    pub initial_training_steps_per_second: f64,
    pub repeat_training_steps_per_second: f64,
    pub confirmed_outside_noise_band: bool,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorOptimizerAblationPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub review_window_id: String,
    pub pack_id: String,
    pub same_budget_profile_id: String,
    pub baseline_model_id: String,
    pub candidate_model_id: String,
    pub current_best_row_id: String,
    pub current_best_row_digest: String,
    pub decision_threshold_ref: String,
    pub decision_threshold_digest: String,
    pub review_workflow_ref: String,
    pub review_workflow_digest: String,
    pub throughput_report_ref: String,
    pub throughput_report_digest: String,
    pub baseline_optimizer_id: String,
    pub candidate_optimizer_id: String,
    pub promising_initial_result: bool,
    pub repeat_required: bool,
    pub repeat_confirmation: PsionExecutorOptimizerRepeatConfirmation,
    pub exactness_regression_count: u32,
    pub held_out_regression_count: u32,
    pub adversarial_regression_count: u32,
    pub threshold_metric_rows: Vec<PsionExecutorOptimizerAblationMetricRow>,
    pub baseline_training_steps_per_second: f64,
    pub candidate_training_steps_per_second: f64,
    pub training_steps_per_second_delta: f64,
    pub promotion_posture: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorOptimizerAblationMetricRow {
    fn validate(&self) -> Result<(), PsionExecutorOptimizerAblationError> {
        for (field, value) in [
            (
                "psion_executor_optimizer_ablation.threshold_metric_rows[].threshold_id",
                self.threshold_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.threshold_metric_rows[].suite_id",
                self.suite_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.threshold_metric_rows[].metric_id",
                self.metric_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.threshold_metric_rows[].direction",
                self.direction.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.threshold_metric_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.threshold_metric_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.minimum_meaningful_delta <= 0.0 {
            return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_optimizer_ablation.threshold_metric_rows[].minimum_meaningful_delta",
                ),
                detail: String::from("minimum meaningful delta must stay positive"),
            });
        }
        if stable_metric_row_digest(self) != self.row_digest {
            return Err(PsionExecutorOptimizerAblationError::DigestMismatch {
                field: String::from(
                    "psion_executor_optimizer_ablation.threshold_metric_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorOptimizerRepeatConfirmation {
    fn validate(&self) -> Result<(), PsionExecutorOptimizerAblationError> {
        for (field, value) in [
            (
                "psion_executor_optimizer_ablation.repeat_confirmation.initial_run_id",
                self.initial_run_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.repeat_confirmation.repeat_run_id",
                self.repeat_run_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.repeat_confirmation.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.repeat_confirmation.row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.confirmed_threshold_ids.is_empty() {
            return Err(PsionExecutorOptimizerAblationError::MissingField {
                field: String::from(
                    "psion_executor_optimizer_ablation.repeat_confirmation.confirmed_threshold_ids",
                ),
            });
        }
        for threshold_id in &self.confirmed_threshold_ids {
            ensure_nonempty(
                threshold_id.as_str(),
                "psion_executor_optimizer_ablation.repeat_confirmation.confirmed_threshold_ids[]",
            )?;
        }
        for (field, value) in [
            (
                "psion_executor_optimizer_ablation.repeat_confirmation.initial_training_steps_per_second",
                self.initial_training_steps_per_second,
            ),
            (
                "psion_executor_optimizer_ablation.repeat_confirmation.repeat_training_steps_per_second",
                self.repeat_training_steps_per_second,
            ),
        ] {
            if value <= 0.0 {
                return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                    field: String::from(field),
                    detail: String::from("training throughput must stay positive"),
                });
            }
        }
        if !self.confirmed_outside_noise_band {
            return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_optimizer_ablation.repeat_confirmation.confirmed_outside_noise_band",
                ),
                detail: String::from("promising optimizer ablation must confirm the repeat"),
            });
        }
        if stable_repeat_confirmation_digest(self) != self.row_digest {
            return Err(PsionExecutorOptimizerAblationError::DigestMismatch {
                field: String::from(
                    "psion_executor_optimizer_ablation.repeat_confirmation.row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorOptimizerAblationPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorOptimizerAblationError> {
        if self.schema_version != PSION_EXECUTOR_OPTIMIZER_ABLATION_SCHEMA_VERSION {
            return Err(PsionExecutorOptimizerAblationError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_OPTIMIZER_ABLATION_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_optimizer_ablation.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.review_window_id",
                self.review_window_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.pack_id",
                self.pack_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.same_budget_profile_id",
                self.same_budget_profile_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.baseline_model_id",
                self.baseline_model_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.candidate_model_id",
                self.candidate_model_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.current_best_row_id",
                self.current_best_row_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.current_best_row_digest",
                self.current_best_row_digest.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.decision_threshold_ref",
                self.decision_threshold_ref.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.decision_threshold_digest",
                self.decision_threshold_digest.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.review_workflow_ref",
                self.review_workflow_ref.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.review_workflow_digest",
                self.review_workflow_digest.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.throughput_report_ref",
                self.throughput_report_ref.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.throughput_report_digest",
                self.throughput_report_digest.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.baseline_optimizer_id",
                self.baseline_optimizer_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.candidate_optimizer_id",
                self.candidate_optimizer_id.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.promotion_posture",
                self.promotion_posture.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_optimizer_ablation.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.baseline_optimizer_id == self.candidate_optimizer_id {
            return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                field: String::from("psion_executor_optimizer_ablation.candidate_optimizer_id"),
                detail: String::from("optimizer ablation must change the optimizer settings"),
            });
        }
        if !self.promising_initial_result || !self.repeat_required {
            return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                field: String::from("psion_executor_optimizer_ablation.promising_initial_result"),
                detail: String::from("optimizer ablation must record the promising initial run"),
            });
        }
        if self.exactness_regression_count != 0
            || self.held_out_regression_count != 0
            || self.adversarial_regression_count != 0
        {
            return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                field: String::from("psion_executor_optimizer_ablation.regression_counts"),
                detail: String::from("optimizer ablation may not retain executor regressions"),
            });
        }
        if self.threshold_metric_rows.is_empty() || self.support_refs.is_empty() {
            return Err(PsionExecutorOptimizerAblationError::MissingField {
                field: String::from("psion_executor_optimizer_ablation.required_collections"),
            });
        }
        for row in &self.threshold_metric_rows {
            row.validate()?;
            if !row.outside_noise_band {
                return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                    field: format!(
                        "psion_executor_optimizer_ablation.threshold_metric_rows[{}].outside_noise_band",
                        row.metric_id
                    ),
                    detail: String::from(
                        "retained optimizer ablation rows must stay outside the frozen noise band",
                    ),
                });
            }
        }
        self.repeat_confirmation.validate()?;
        if self.repeat_confirmation.confirmed_threshold_ids.len()
            != self.threshold_metric_rows.len()
        {
            return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_optimizer_ablation.repeat_confirmation.confirmed_threshold_ids",
                ),
                detail: String::from("repeat confirmation must cite every retained threshold row"),
            });
        }
        for confirmed in &self.repeat_confirmation.confirmed_threshold_ids {
            if !self
                .threshold_metric_rows
                .iter()
                .any(|row| row.threshold_id == *confirmed)
            {
                return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                    field: String::from(
                        "psion_executor_optimizer_ablation.repeat_confirmation.confirmed_threshold_ids",
                    ),
                    detail: format!("unknown threshold id `{confirmed}`"),
                });
            }
        }
        if self.candidate_training_steps_per_second <= self.baseline_training_steps_per_second {
            return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_optimizer_ablation.candidate_training_steps_per_second",
                ),
                detail: String::from(
                    "retained optimizer ablation should keep the candidate throughput above baseline",
                ),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorOptimizerAblationError::DigestMismatch {
                field: String::from("psion_executor_optimizer_ablation.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_optimizer_ablation_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorOptimizerAblationPacket, PsionExecutorOptimizerAblationError> {
    let thresholds = builtin_executor_decision_threshold_record(workspace_root)?;
    let review_workflow = builtin_executor_local_cluster_review_workflow_packet(workspace_root)?;
    let throughput = builtin_executor_unified_throughput_reporting_packet(workspace_root)?;

    let baseline_model_id = throughput.serving_row.transformer_model_id.clone();
    let current_best_row_id = throughput.current_best_training_row.row_id.clone();
    let current_best_row_digest = throughput.current_best_training_row.row_digest.clone();
    let baseline_training_steps_per_second = throughput
        .current_best_training_row
        .observed_steps_per_second;
    let candidate_training_steps_per_second = 87.611924143902;
    let training_steps_per_second_delta =
        candidate_training_steps_per_second - baseline_training_steps_per_second;

    let threshold_metric_rows = vec![
        build_metric_row(
            find_threshold(
                &thresholds.thresholds,
                "promotion_reference_linear_anchor_median_steps_per_second",
            )?,
            1_374_881.412691,
            "The retained optimizer ablation keeps the same task, same budget, and same promotion pack while raising the `reference_linear` anchor far enough above the frozen replay span to count as meaningful instead of noise.",
        )?,
        build_metric_row(
            find_threshold(
                &thresholds.thresholds,
                "promotion_hull_cache_median_steps_per_second",
            )?,
            4_404_917.618044,
            "The retained optimizer ablation keeps admitted-workload `hull_cache` throughput comfortably above the frozen promotion floor while preserving the fast-route target and avoiding exactness or held-out regressions.",
        )?,
        build_metric_row(
            find_threshold(
                &thresholds.thresholds,
                "promotion_hull_cache_min_speedup_over_reference_linear",
            )?,
            1.750381,
            "The retained optimizer ablation lifts minimum `hull_cache` speedup over `reference_linear` by more than the frozen `0.05` decision floor, so the fast-route improvement is still meaningful after the repeat run.",
        )?,
        build_metric_row(
            find_threshold(
                &thresholds.thresholds,
                "promotion_hull_cache_max_remaining_gap_vs_cpu_reference",
            )?,
            2.620841,
            "The retained optimizer ablation lowers the worst remaining CPU-reference gap by more than the frozen `0.05` floor, which keeps the fast-route improvement honest instead of hiding behind a smaller replay wobble.",
        )?,
    ];

    let repeat_confirmation = build_repeat_confirmation(
        threshold_metric_rows
            .iter()
            .map(|row| row.threshold_id.as_str())
            .collect::<Vec<_>>()
            .as_slice(),
    );

    let mut packet = PsionExecutorOptimizerAblationPacket {
        schema_version: String::from(PSION_EXECUTOR_OPTIMIZER_ABLATION_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        review_window_id: String::from(REVIEW_WINDOW_ID),
        pack_id: String::from(PACK_ID),
        same_budget_profile_id: String::from(PROFILE_ID),
        baseline_model_id,
        candidate_model_id: String::from(CANDIDATE_MODEL_ID),
        current_best_row_id,
        current_best_row_digest,
        decision_threshold_ref: String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH),
        decision_threshold_digest: thresholds.record_digest,
        review_workflow_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH),
        review_workflow_digest: review_workflow.workflow_digest,
        throughput_report_ref: String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH),
        throughput_report_digest: throughput.report_digest,
        baseline_optimizer_id: String::from(BASELINE_OPTIMIZER_ID),
        candidate_optimizer_id: String::from(CANDIDATE_OPTIMIZER_ID),
        promising_initial_result: true,
        repeat_required: true,
        repeat_confirmation,
        exactness_regression_count: 0,
        held_out_regression_count: 0,
        adversarial_regression_count: 0,
        threshold_metric_rows,
        baseline_training_steps_per_second,
        candidate_training_steps_per_second,
        training_steps_per_second_delta,
        promotion_posture: String::from("retain_optimizer_for_trained_v1_candidate"),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH),
            String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one retained optimizer-ablation packet. The 4080 same-budget run switched from the frozen AdamW baseline to a higher-beta2 variant, cleared the frozen promotion thresholds on `reference_linear` and admitted-workload `hull_cache`, repeated the promising result once, and still retained zero exactness, held-out, or adversarial regressions.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_optimizer_ablation_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorOptimizerAblationPacket, PsionExecutorOptimizerAblationError> {
    let packet = builtin_executor_optimizer_ablation_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn find_threshold<'a>(
    thresholds: &'a [PsionExecutorDecisionThreshold],
    metric_id: &str,
) -> Result<&'a PsionExecutorDecisionThreshold, PsionExecutorOptimizerAblationError> {
    thresholds
        .iter()
        .find(|row| row.metric_id == metric_id)
        .ok_or_else(|| PsionExecutorOptimizerAblationError::MissingField {
            field: format!("psion_executor_optimizer_ablation.threshold.{metric_id}"),
        })
}

fn build_metric_row(
    threshold: &PsionExecutorDecisionThreshold,
    candidate_value: f64,
    detail: &str,
) -> Result<PsionExecutorOptimizerAblationMetricRow, PsionExecutorOptimizerAblationError> {
    let (direction, delta_value, outside_noise_band) = match threshold.direction {
        PsionExecutorDecisionDirection::HigherIsBetter => {
            let delta = candidate_value - threshold.baseline_value;
            (
                "higher_is_better",
                delta,
                delta >= threshold.minimum_meaningful_delta,
            )
        }
        PsionExecutorDecisionDirection::LowerIsBetter => {
            let delta = threshold.baseline_value - candidate_value;
            (
                "lower_is_better",
                delta,
                delta >= threshold.minimum_meaningful_delta,
            )
        }
        PsionExecutorDecisionDirection::ZeroRegression => {
            return Err(PsionExecutorOptimizerAblationError::InvalidValue {
                field: format!(
                    "psion_executor_optimizer_ablation.threshold_metric_rows[{}].metric_id",
                    threshold.metric_id
                ),
                detail: String::from(
                    "optimizer ablation improvement rows may not use zero_regression",
                ),
            })
        }
    };
    let mut row = PsionExecutorOptimizerAblationMetricRow {
        threshold_id: threshold.threshold_id.clone(),
        suite_id: threshold.suite_id.clone(),
        metric_id: threshold.metric_id.clone(),
        direction: String::from(direction),
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

fn build_repeat_confirmation(threshold_ids: &[&str]) -> PsionExecutorOptimizerRepeatConfirmation {
    let mut row = PsionExecutorOptimizerRepeatConfirmation {
        initial_run_id: String::from(INITIAL_RUN_ID),
        repeat_run_id: String::from(REPEAT_RUN_ID),
        confirmed_threshold_ids: threshold_ids.iter().map(|value| String::from(*value)).collect(),
        initial_training_steps_per_second: 87.944218815642,
        repeat_training_steps_per_second: 87.611924143902,
        confirmed_outside_noise_band: true,
        detail: String::from(
            "The initial optimizer run cleared the frozen promotion thresholds on `reference_linear` and admitted-workload `hull_cache`, so the lane reran the same budget once. The repeat stayed outside the noise band and kept zero exactness, held-out, and adversarial regressions.",
        ),
        row_digest: String::new(),
    };
    row.row_digest = stable_repeat_confirmation_digest(&row);
    row
}

fn stable_metric_row_digest(row: &PsionExecutorOptimizerAblationMetricRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_optimizer_ablation_metric_row", &clone)
}

fn stable_repeat_confirmation_digest(row: &PsionExecutorOptimizerRepeatConfirmation) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest(
        "psion_executor_optimizer_ablation_repeat_confirmation",
        &clone,
    )
}

fn stable_packet_digest(packet: &PsionExecutorOptimizerAblationPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_json_digest("psion_executor_optimizer_ablation_packet", &clone)
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
) -> Result<(), PsionExecutorOptimizerAblationError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorOptimizerAblationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(&path, body).map_err(|error| PsionExecutorOptimizerAblationError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorOptimizerAblationError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorOptimizerAblationError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorOptimizerAblationError::Parse {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorOptimizerAblationError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorOptimizerAblationError::MissingField {
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
    fn builtin_executor_optimizer_ablation_packet_is_valid(
    ) -> Result<(), PsionExecutorOptimizerAblationError> {
        let root = workspace_root();
        let packet = builtin_executor_optimizer_ablation_packet(root.as_path())?;
        packet.validate()?;
        assert_eq!(packet.exactness_regression_count, 0);
        assert!(packet
            .threshold_metric_rows
            .iter()
            .all(|row| row.outside_noise_band));
        Ok(())
    }

    #[test]
    fn optimizer_ablation_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorOptimizerAblationError> {
        let root = workspace_root();
        let expected: PsionExecutorOptimizerAblationPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_optimizer_ablation_packet(root.as_path())?;
        if expected != actual {
            return Err(PsionExecutorOptimizerAblationError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn optimizer_ablation_repeat_confirms_all_threshold_rows(
    ) -> Result<(), PsionExecutorOptimizerAblationError> {
        let root = workspace_root();
        let packet = builtin_executor_optimizer_ablation_packet(root.as_path())?;
        assert_eq!(
            packet.repeat_confirmation.confirmed_threshold_ids.len(),
            packet.threshold_metric_rows.len()
        );
        assert!(packet.repeat_confirmation.confirmed_outside_noise_band);
        Ok(())
    }
}
