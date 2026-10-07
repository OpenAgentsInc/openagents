use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_baseline_truth_record, builtin_executor_canonical_mixture_packet,
    builtin_executor_local_cluster_ledger, PsionExecutorBaselineTruthError,
    PsionExecutorCanonicalMixtureError, PsionExecutorLocalClusterLedger,
    PsionExecutorLocalClusterLedgerError, PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH,
    PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH, PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH,
    PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH, PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
};

pub const PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_SCHEMA_VERSION: &str =
    "psion.executor.source_family_contribution.v1";
pub const PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_source_family_contribution_v1.json";
pub const PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION.md";

const REPORT_ID: &str = "psion_executor_source_family_contribution_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const MLX_PROFILE_ID: &str = "local_mac_mlx_aarch64";
const CUDA_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";

#[derive(Debug, Error)]
pub enum PsionExecutorSourceFamilyContributionError {
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
    Mixture(#[from] PsionExecutorCanonicalMixtureError),
    #[error(transparent)]
    BaselineTruth(#[from] PsionExecutorBaselineTruthError),
    #[error(transparent)]
    Ledger(#[from] PsionExecutorLocalClusterLedgerError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorContributionSliceDelta {
    pub slice_id: String,
    pub delta_bps: i32,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorSourceFamilyContributionRow {
    pub source_family_id: String,
    pub initial_weight_bps: u32,
    pub primary_eval_slice_ids: Vec<String>,
    pub exactness_delta_bps: i32,
    pub held_out_slice_deltas: Vec<PsionExecutorContributionSliceDelta>,
    pub adversarial_slice_deltas: Vec<PsionExecutorContributionSliceDelta>,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorThroughputRegressionRow {
    pub metric_id: String,
    pub baseline_profile_id: String,
    pub candidate_profile_id: String,
    pub baseline_value: f64,
    pub candidate_value: f64,
    pub delta_value: f64,
    pub regression: bool,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorStabilityRegressionRow {
    pub fact_id: String,
    pub candidate_profile_id: String,
    pub regression_class: String,
    pub status: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorSourceFamilyContributionReport {
    pub schema_version: String,
    pub report_id: String,
    pub mixture_ref: String,
    pub mixture_digest: String,
    pub baseline_truth_ref: String,
    pub baseline_truth_digest: String,
    pub local_cluster_ledger_ref: String,
    pub local_cluster_ledger_digest: String,
    pub baseline_row_id: String,
    pub baseline_row_digest: String,
    pub candidate_row_id: String,
    pub candidate_row_digest: String,
    pub source_family_rows: Vec<PsionExecutorSourceFamilyContributionRow>,
    pub throughput_regressions: Vec<PsionExecutorThroughputRegressionRow>,
    pub stability_regressions: Vec<PsionExecutorStabilityRegressionRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub report_digest: String,
}

impl PsionExecutorContributionSliceDelta {
    fn validate(&self, field: &str) -> Result<(), PsionExecutorSourceFamilyContributionError> {
        for (field, value) in [
            (format!("{field}.slice_id"), self.slice_id.as_str()),
            (format!("{field}.detail"), self.detail.as_str()),
            (format!("{field}.row_digest"), self.row_digest.as_str()),
        ] {
            ensure_nonempty(value, field.as_str())?;
        }
        if stable_slice_delta_digest(self) != self.row_digest {
            return Err(PsionExecutorSourceFamilyContributionError::DigestMismatch {
                field: format!("{field}.row_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorSourceFamilyContributionRow {
    fn validate(&self) -> Result<(), PsionExecutorSourceFamilyContributionError> {
        for (field, value) in [
            (
                "psion_executor_source_family_contribution.source_family_rows[].source_family_id",
                self.source_family_id.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.source_family_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.source_family_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.initial_weight_bps == 0 || self.primary_eval_slice_ids.is_empty() {
            return Err(PsionExecutorSourceFamilyContributionError::MissingField {
                field: String::from(
                    "psion_executor_source_family_contribution.source_family_rows[].required_fields",
                ),
            });
        }
        for slice in &self.primary_eval_slice_ids {
            ensure_nonempty(
                slice.as_str(),
                "psion_executor_source_family_contribution.source_family_rows[].primary_eval_slice_ids[]",
            )?;
        }
        for (index, row) in self.held_out_slice_deltas.iter().enumerate() {
            row.validate(
                format!(
                    "psion_executor_source_family_contribution.source_family_rows[].held_out_slice_deltas[{index}]"
                )
                .as_str(),
            )?;
        }
        for (index, row) in self.adversarial_slice_deltas.iter().enumerate() {
            row.validate(
                format!(
                    "psion_executor_source_family_contribution.source_family_rows[].adversarial_slice_deltas[{index}]"
                )
                .as_str(),
            )?;
        }
        if stable_source_family_contribution_row_digest(self) != self.row_digest {
            return Err(PsionExecutorSourceFamilyContributionError::DigestMismatch {
                field: String::from(
                    "psion_executor_source_family_contribution.source_family_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorThroughputRegressionRow {
    fn validate(&self) -> Result<(), PsionExecutorSourceFamilyContributionError> {
        for (field, value) in [
            (
                "psion_executor_source_family_contribution.throughput_regressions[].metric_id",
                self.metric_id.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.throughput_regressions[].baseline_profile_id",
                self.baseline_profile_id.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.throughput_regressions[].candidate_profile_id",
                self.candidate_profile_id.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.throughput_regressions[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.throughput_regressions[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.baseline_value <= 0.0 || self.candidate_value <= 0.0 {
            return Err(PsionExecutorSourceFamilyContributionError::InvalidValue {
                field: String::from(
                    "psion_executor_source_family_contribution.throughput_regressions[].values",
                ),
                detail: String::from("throughput values must stay positive"),
            });
        }
        if stable_throughput_regression_digest(self) != self.row_digest {
            return Err(PsionExecutorSourceFamilyContributionError::DigestMismatch {
                field: String::from(
                    "psion_executor_source_family_contribution.throughput_regressions[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorStabilityRegressionRow {
    fn validate(&self) -> Result<(), PsionExecutorSourceFamilyContributionError> {
        for (field, value) in [
            (
                "psion_executor_source_family_contribution.stability_regressions[].fact_id",
                self.fact_id.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.stability_regressions[].candidate_profile_id",
                self.candidate_profile_id.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.stability_regressions[].regression_class",
                self.regression_class.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.stability_regressions[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.stability_regressions[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.stability_regressions[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_stability_regression_digest(self) != self.row_digest {
            return Err(PsionExecutorSourceFamilyContributionError::DigestMismatch {
                field: String::from(
                    "psion_executor_source_family_contribution.stability_regressions[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorSourceFamilyContributionReport {
    pub fn validate(&self) -> Result<(), PsionExecutorSourceFamilyContributionError> {
        if self.schema_version != PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_SCHEMA_VERSION {
            return Err(PsionExecutorSourceFamilyContributionError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_source_family_contribution.report_id",
                self.report_id.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.mixture_ref",
                self.mixture_ref.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.mixture_digest",
                self.mixture_digest.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.baseline_truth_ref",
                self.baseline_truth_ref.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.baseline_truth_digest",
                self.baseline_truth_digest.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.local_cluster_ledger_ref",
                self.local_cluster_ledger_ref.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.local_cluster_ledger_digest",
                self.local_cluster_ledger_digest.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.baseline_row_id",
                self.baseline_row_id.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.baseline_row_digest",
                self.baseline_row_digest.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.candidate_row_id",
                self.candidate_row_id.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.candidate_row_digest",
                self.candidate_row_digest.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_source_family_contribution.report_digest",
                self.report_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.source_family_rows.is_empty()
            || self.throughput_regressions.is_empty()
            || self.stability_regressions.is_empty()
            || self.support_refs.is_empty()
        {
            return Err(PsionExecutorSourceFamilyContributionError::MissingField {
                field: String::from("psion_executor_source_family_contribution.required_arrays"),
            });
        }
        let mut seen_family_ids = BTreeSet::new();
        let mut total_weight_bps = 0u32;
        for row in &self.source_family_rows {
            row.validate()?;
            if !seen_family_ids.insert(row.source_family_id.as_str()) {
                return Err(PsionExecutorSourceFamilyContributionError::InvalidValue {
                    field: String::from(
                        "psion_executor_source_family_contribution.source_family_rows[].source_family_id",
                    ),
                    detail: format!("duplicate source-family id `{}`", row.source_family_id),
                });
            }
            total_weight_bps = total_weight_bps.saturating_add(row.initial_weight_bps);
        }
        if total_weight_bps != 10_000 {
            return Err(PsionExecutorSourceFamilyContributionError::InvalidValue {
                field: String::from(
                    "psion_executor_source_family_contribution.source_family_rows[].initial_weight_bps",
                ),
                detail: format!("expected total family weight to stay at 10000 bps, found {total_weight_bps}"),
            });
        }
        for row in &self.throughput_regressions {
            row.validate()?;
        }
        for row in &self.stability_regressions {
            row.validate()?;
        }
        if stable_report_digest(self) != self.report_digest {
            return Err(PsionExecutorSourceFamilyContributionError::DigestMismatch {
                field: String::from("psion_executor_source_family_contribution.report_digest"),
            });
        }
        Ok(())
    }
}

pub fn build_executor_source_family_contribution_report(
    workspace_root: impl AsRef<Path>,
) -> Result<PsionExecutorSourceFamilyContributionReport, PsionExecutorSourceFamilyContributionError>
{
    let root = workspace_root.as_ref();
    let mixture = builtin_executor_canonical_mixture_packet(root)?;
    let baseline_truth = builtin_executor_baseline_truth_record(root)?;
    let ledger = builtin_executor_local_cluster_ledger(root)?;
    let baseline_row = find_ledger_row(&ledger, MLX_PROFILE_ID)?;
    let candidate_row = find_ledger_row(&ledger, CUDA_PROFILE_ID)?;

    let source_family_rows = mixture
        .source_families
        .iter()
        .map(|family| build_source_family_row(family.source_family_id.as_str(), family.initial_weight_bps))
        .collect::<Result<Vec<_>, _>>()?;

    let throughput_regressions = vec![
        throughput_regression_row(
            "observed_steps_per_second",
            baseline_row.admitted_profile_id.as_str(),
            candidate_row.admitted_profile_id.as_str(),
            baseline_row.metric_posture.observed_steps_per_second,
            candidate_row.metric_posture.observed_steps_per_second,
            "The current-best 4080 row is still slower than the retained MLX row on raw step throughput, so mixture review must keep accelerator throughput regressions visible instead of hiding them behind source-family prose.",
        ),
        throughput_regression_row(
            "observed_samples_per_second",
            baseline_row.admitted_profile_id.as_str(),
            candidate_row.admitted_profile_id.as_str(),
            baseline_row.metric_posture.observed_samples_per_second,
            candidate_row.metric_posture.observed_samples_per_second,
            "Sample throughput is still below the retained MLX row on the admitted matrix, so weekly mixture review can see the accelerator deficit directly.",
        ),
        throughput_regression_row(
            "observed_source_tokens_per_second",
            baseline_row.admitted_profile_id.as_str(),
            candidate_row.admitted_profile_id.as_str(),
            baseline_row.metric_posture.observed_source_tokens_per_second,
            candidate_row.metric_posture.observed_source_tokens_per_second,
            "Source-token throughput remains below the retained MLX row, which keeps throughput regressions explicit even while exactness and held-out deltas stay flat.",
        ),
    ];

    let stability_regressions = candidate_row
        .failure_facts
        .iter()
        .filter(|fact| fact.status != "green")
        .map(|fact| stability_regression_row(
            fact.fact_id.as_str(),
            candidate_row.admitted_profile_id.as_str(),
            fact.status.as_str(),
            regression_class_for_fact(fact.fact_id.as_str()),
            fact.detail.as_str(),
        ))
        .collect::<Vec<_>>();

    let mut report = PsionExecutorSourceFamilyContributionReport {
        schema_version: String::from(PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_SCHEMA_VERSION),
        report_id: String::from(REPORT_ID),
        mixture_ref: String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH),
        mixture_digest: mixture.packet_digest.clone(),
        baseline_truth_ref: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
        baseline_truth_digest: baseline_truth.record_digest.clone(),
        local_cluster_ledger_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
        local_cluster_ledger_digest: ledger.ledger_digest.clone(),
        baseline_row_id: baseline_row.row_id.clone(),
        baseline_row_digest: baseline_row.row_digest.clone(),
        candidate_row_id: candidate_row.row_id.clone(),
        candidate_row_digest: candidate_row.row_digest.clone(),
        source_family_rows,
        throughput_regressions,
        stability_regressions,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH),
        ],
        summary: format!(
            "The executor lane now has one canonical source-family contribution report. It keeps all six source-family weights explicit against baseline-truth slice deltas (exactness, held-out, adversarial all remain flat at zero on the retained baseline), while the run-level section keeps {} throughput regressions and {} stability regressions visible on the current-best 4080 row instead of letting mixture review hide infrastructure debt.",
            3,
            candidate_row.failure_facts.iter().filter(|fact| fact.status != "green").count(),
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_report_digest(&report);
    report.validate()?;
    Ok(report)
}

pub fn write_executor_source_family_contribution_report(
    workspace_root: impl AsRef<Path>,
) -> Result<PsionExecutorSourceFamilyContributionReport, PsionExecutorSourceFamilyContributionError>
{
    let root = workspace_root.as_ref();
    let report = build_executor_source_family_contribution_report(root)?;
    write_json_fixture(root, PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH, &report)?;
    Ok(report)
}

fn build_source_family_row(
    source_family_id: &str,
    initial_weight_bps: u32,
) -> Result<PsionExecutorSourceFamilyContributionRow, PsionExecutorSourceFamilyContributionError> {
    let (primary_eval_slice_ids, held_out_slice_deltas, adversarial_slice_deltas, detail) =
        match source_family_id {
            "executor.boundary_prefix_traces" => (
                vec![
                    String::from("frequent_exactness_cases_v0"),
                    String::from("frequent_held_out_exclusions_v0"),
                ],
                vec![slice_delta(
                    "frequent_held_out_exclusions_v0",
                    0,
                    "Boundary-prefix traces keep the held-out exclusion boundary flat on the retained baseline packet.",
                )],
                Vec::new(),
                "Boundary-prefix traces remain a zero-delta family on the retained baseline: frequent exactness and held-out exclusion slices stay green and unchanged while later mixture runs are still pending.".to_string(),
            ),
            "executor.article_route_direct_traces" => (
                vec![
                    String::from("frequent_exactness_cases_v0"),
                    String::from("promotion_exactness_suite_v0"),
                    String::from("promotion_held_out_suite_v0"),
                ],
                vec![
                    slice_delta(
                        "frequent_held_out_exclusions_v0",
                        0,
                        "Article-route direct traces do not move the retained frequent held-out exclusions on the current baseline.",
                    ),
                    slice_delta(
                        "promotion_held_out_suite_v0",
                        0,
                        "Article-route direct traces keep the retained promotion held-out suite flat on the current baseline packet.",
                    ),
                ],
                Vec::new(),
                "Article-route direct traces keep both frequent and promotion exactness slices flat on the retained baseline packet; no held-out movement is credited yet because no new mixture candidate has run.".to_string(),
            ),
            "executor.long_loop_kernel_traces" => (
                vec![
                    String::from("promotion_exactness_suite_v0"),
                    String::from("promotion_held_out_suite_v0"),
                    String::from("promotion_adversarial_suite_v0"),
                ],
                vec![slice_delta(
                    "promotion_held_out_suite_v0",
                    0,
                    "Long-loop kernel traces keep the retained promotion held-out suite flat at baseline.",
                )],
                vec![slice_delta(
                    "promotion_adversarial_suite_v0",
                    0,
                    "Long-loop kernel traces keep the adversarial promotion slice flat on the retained baseline packet.",
                )],
                "Long-loop kernel traces are the heaviest promotion-stage family, but the retained packet still records zero exactness, held-out, and adversarial movement until a new mixture candidate exists.".to_string(),
            ),
            "executor.sudoku_v0_traces" => (
                vec![String::from("promotion_exactness_suite_v0")],
                Vec::new(),
                Vec::new(),
                "Sudoku traces remain a pure exactness-support family in the retained report and currently show zero delta against the frozen promotion exactness suite.".to_string(),
            ),
            "executor.hungarian_matching_traces" => (
                vec![String::from("promotion_exactness_suite_v0")],
                Vec::new(),
                Vec::new(),
                "Hungarian-matching traces remain a pure exactness-support family in the retained report and currently show zero delta against the frozen promotion exactness suite.".to_string(),
            ),
            "executor.refusal_negative_traces" => (
                vec![
                    String::from("frequent_held_out_exclusions_v0"),
                    String::from("promotion_held_out_suite_v0"),
                    String::from("promotion_adversarial_suite_v0"),
                ],
                vec![
                    slice_delta(
                        "frequent_held_out_exclusions_v0",
                        0,
                        "Refusal-negative traces keep the retained held-out exclusion boundary flat on the baseline packet.",
                    ),
                    slice_delta(
                        "promotion_held_out_suite_v0",
                        0,
                        "Refusal-negative traces keep the promotion held-out suite flat on the baseline packet.",
                    ),
                ],
                vec![slice_delta(
                    "promotion_adversarial_suite_v0",
                    0,
                    "Refusal-negative traces keep the adversarial promotion slice flat on the baseline packet.",
                )],
                "Refusal-negative traces still contribute only boundary integrity on the retained packet: held-out and adversarial slices remain flat until a fresh mixture candidate is compared.".to_string(),
            ),
            _ => {
                return Err(PsionExecutorSourceFamilyContributionError::InvalidValue {
                    field: String::from(
                        "psion_executor_source_family_contribution.source_family_rows[].source_family_id",
                    ),
                    detail: format!("unsupported source-family id `{source_family_id}`"),
                });
            }
        };
    let exactness_delta_bps = 0;
    let mut row = PsionExecutorSourceFamilyContributionRow {
        source_family_id: String::from(source_family_id),
        initial_weight_bps,
        primary_eval_slice_ids,
        exactness_delta_bps,
        held_out_slice_deltas,
        adversarial_slice_deltas,
        detail,
        row_digest: String::new(),
    };
    row.row_digest = stable_source_family_contribution_row_digest(&row);
    Ok(row)
}

fn throughput_regression_row(
    metric_id: &str,
    baseline_profile_id: &str,
    candidate_profile_id: &str,
    baseline_value: f64,
    candidate_value: f64,
    detail: &str,
) -> PsionExecutorThroughputRegressionRow {
    let mut row = PsionExecutorThroughputRegressionRow {
        metric_id: String::from(metric_id),
        baseline_profile_id: String::from(baseline_profile_id),
        candidate_profile_id: String::from(candidate_profile_id),
        baseline_value,
        candidate_value,
        delta_value: candidate_value - baseline_value,
        regression: candidate_value < baseline_value,
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_throughput_regression_digest(&row);
    row
}

fn stability_regression_row(
    fact_id: &str,
    candidate_profile_id: &str,
    status: &str,
    regression_class: &str,
    detail: &str,
) -> PsionExecutorStabilityRegressionRow {
    let mut row = PsionExecutorStabilityRegressionRow {
        fact_id: String::from(fact_id),
        candidate_profile_id: String::from(candidate_profile_id),
        regression_class: String::from(regression_class),
        status: String::from(status),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_stability_regression_digest(&row);
    row
}

fn slice_delta(slice_id: &str, delta_bps: i32, detail: &str) -> PsionExecutorContributionSliceDelta {
    let mut row = PsionExecutorContributionSliceDelta {
        slice_id: String::from(slice_id),
        delta_bps,
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_slice_delta_digest(&row);
    row
}

fn regression_class_for_fact(fact_id: &str) -> &'static str {
    match fact_id {
        "unsupported_precision_publish_refusal" => "publication_refusal",
        "stale_worker_replay_required" => "recovery_gate",
        "upload_disagreement_rejected" => "lineage_rejection",
        "uneven_worker_speed_wait_then_replay" => "worker_skew",
        _ => "stability_regression",
    }
}

fn find_ledger_row<'a>(
    ledger: &'a PsionExecutorLocalClusterLedger,
    admitted_profile_id: &str,
) -> Result<&'a crate::PsionExecutorLocalClusterLedgerRow, PsionExecutorSourceFamilyContributionError>
{
    ledger
        .rows
        .iter()
        .find(|row| row.admitted_profile_id == admitted_profile_id)
        .ok_or_else(|| PsionExecutorSourceFamilyContributionError::MissingField {
            field: format!("psion_executor_source_family_contribution.ledger_row[{admitted_profile_id}]"),
        })
}

fn stable_slice_delta_digest(row: &PsionExecutorContributionSliceDelta) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_source_family_contribution_slice_delta", &clone)
}

fn stable_source_family_contribution_row_digest(
    row: &PsionExecutorSourceFamilyContributionRow,
) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_source_family_contribution_row", &clone)
}

fn stable_throughput_regression_digest(row: &PsionExecutorThroughputRegressionRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_source_family_contribution_throughput", &clone)
}

fn stable_stability_regression_digest(row: &PsionExecutorStabilityRegressionRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_source_family_contribution_stability", &clone)
}

fn stable_report_digest(report: &PsionExecutorSourceFamilyContributionReport) -> String {
    let mut clone = report.clone();
    clone.report_digest.clear();
    stable_json_digest("psion_executor_source_family_contribution_report", &clone)
}

fn stable_json_digest<T: Serialize>(label: &str, value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    hasher.update(serde_json::to_vec(value).expect("stable json"));
    hex::encode(hasher.finalize())
}

fn read_fixture<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorSourceFamilyContributionError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorSourceFamilyContributionError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorSourceFamilyContributionError::Parse {
        path: path.display().to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorSourceFamilyContributionError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorSourceFamilyContributionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(&path, body).map_err(|error| PsionExecutorSourceFamilyContributionError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorSourceFamilyContributionError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorSourceFamilyContributionError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn built_source_family_contribution_report_is_valid()
    -> Result<(), PsionExecutorSourceFamilyContributionError> {
        let report = build_executor_source_family_contribution_report(repo_root())?;
        report.validate()?;
        assert_eq!(report.source_family_rows.len(), 6);
        assert_eq!(report.throughput_regressions.len(), 3);
        assert_eq!(report.stability_regressions.len(), 4);
        assert!(report
            .throughput_regressions
            .iter()
            .all(|row| row.regression && row.delta_value < 0.0));
        Ok(())
    }

    #[test]
    fn builtin_source_family_contribution_fixture_matches_generator()
    -> Result<(), PsionExecutorSourceFamilyContributionError> {
        let root = repo_root();
        let expected = build_executor_source_family_contribution_report(&root)?;
        let actual: PsionExecutorSourceFamilyContributionReport =
            read_fixture(&root, PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH)?;
        if expected != actual {
            return Err(PsionExecutorSourceFamilyContributionError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn source_family_contribution_weights_remain_canonical()
    -> Result<(), PsionExecutorSourceFamilyContributionError> {
        let report = build_executor_source_family_contribution_report(repo_root())?;
        let total_weight = report
            .source_family_rows
            .iter()
            .map(|row| row.initial_weight_bps)
            .sum::<u32>();
        assert_eq!(total_weight, 10_000);
        assert_eq!(report.baseline_row_id, "psion_executor_local_cluster_ledger_row_mlx_v1");
        assert_eq!(report.candidate_row_id, "psion_executor_local_cluster_ledger_row_4080_v1");
        Ok(())
    }
}
