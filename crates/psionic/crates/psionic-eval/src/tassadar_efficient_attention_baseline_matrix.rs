use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_environments::TassadarWorkloadTarget;
use psionic_runtime::{
    TassadarClaimClass, TassadarExecutorSelectionReason, TassadarExecutorSelectionState,
    TassadarRecurrentFastPathRuntimeBaselineReport, TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
    TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarBenchmarkCaseReport, TassadarGeometricVariantWorkloadSummary,
    TASSADAR_GEOMETRIC_VARIANT_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;
const REFORMER_DIRECT_CPU_CEILING: f64 = 0.92;

pub const TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_efficient_attention_baseline_matrix.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEfficientAttentionBaselineFamilyKind {
    DenseReferenceLinear,
    SparseTopKValidated,
    LinearRecurrentRuntime,
    ReformerChunkedProxy,
    HullCacheRuntime,
    HierarchicalHullResearch,
}

impl TassadarEfficientAttentionBaselineFamilyKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DenseReferenceLinear => "dense_reference_linear",
            Self::SparseTopKValidated => "sparse_top_k_validated",
            Self::LinearRecurrentRuntime => "linear_recurrent_runtime",
            Self::ReformerChunkedProxy => "reformer_chunked_proxy",
            Self::HullCacheRuntime => "hull_cache_runtime",
            Self::HierarchicalHullResearch => "hierarchical_hull_research",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEfficientAttentionMeasurementKind {
    ArtifactBackedRuntime,
    ArtifactBackedResearch,
    ProxyCostModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEfficientAttentionComparisonOutcome {
    Win,
    Tie,
    Lose,
    Refuse,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarEfficientAttentionBaselineCell {
    pub family_kind: TassadarEfficientAttentionBaselineFamilyKind,
    pub measurement_kind: TassadarEfficientAttentionMeasurementKind,
    pub claim_class: TassadarClaimClass,
    pub claim_boundary: String,
    pub workload_target: TassadarWorkloadTarget,
    pub case_count: u32,
    pub direct_case_count: u32,
    pub fallback_case_count: u32,
    pub refused_case_count: u32,
    pub exact_case_count: u32,
    pub average_steps_per_second: f64,
    pub average_speedup_over_dense_reference: f64,
    pub average_remaining_gap_vs_cpu_reference: f64,
    pub comparison_outcome_vs_dense_reference: TassadarEfficientAttentionComparisonOutcome,
    pub selection_reason_counts: BTreeMap<String, u32>,
    pub artifact_ref: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarEfficientAttentionBaselineMatrixRow {
    pub workload_target: TassadarWorkloadTarget,
    pub dense_reference_steps_per_second: f64,
    pub fastest_family_kind: TassadarEfficientAttentionBaselineFamilyKind,
    pub fastest_family_steps_per_second: f64,
    pub cells: Vec<TassadarEfficientAttentionBaselineCell>,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarEfficientAttentionBaselineMatrixReport {
    pub schema_version: u16,
    pub matrix_id: String,
    pub generated_from_artifacts: Vec<String>,
    pub rows: Vec<TassadarEfficientAttentionBaselineMatrixRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ArticleBenchmarkReportSnapshot {
    pub case_reports: Vec<TassadarBenchmarkCaseReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct GeometricVariantReportSnapshot {
    pub workload_summaries: Vec<TassadarGeometricVariantWorkloadSummary>,
}

#[derive(Debug, Error)]
pub enum TassadarEfficientAttentionBaselineMatrixError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("missing benchmark cases for workload `{workload:?}`")]
    MissingBenchmarkCases { workload: TassadarWorkloadTarget },
    #[error("missing geometric summary for workload `{workload:?}`")]
    MissingGeometricSummary { workload: TassadarWorkloadTarget },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

pub fn build_tassadar_efficient_attention_baseline_matrix_report() -> Result<
    TassadarEfficientAttentionBaselineMatrixReport,
    TassadarEfficientAttentionBaselineMatrixError,
> {
    let benchmark_report: ArticleBenchmarkReportSnapshot =
        read_repo_json(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF)?;
    let recurrent_runtime_report: TassadarRecurrentFastPathRuntimeBaselineReport =
        read_repo_json(TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF)?;
    let geometric_report: GeometricVariantReportSnapshot =
        read_repo_json(TASSADAR_GEOMETRIC_VARIANT_REPORT_REF)?;

    let mut rows = Vec::new();
    for workload_target in article_workload_targets() {
        let cases = benchmark_report
            .case_reports
            .iter()
            .filter(|case| case.workload_target == workload_target)
            .collect::<Vec<_>>();
        if cases.is_empty() {
            return Err(
                TassadarEfficientAttentionBaselineMatrixError::MissingBenchmarkCases {
                    workload: workload_target,
                },
            );
        }

        let dense = build_dense_reference_linear_cell(workload_target, &cases);
        let dense_steps = dense.average_steps_per_second;
        let sparse = build_sparse_top_k_cell(workload_target, &cases, dense_steps);
        let linear_recurrent = build_linear_recurrent_runtime_cell(
            workload_target,
            &cases,
            &recurrent_runtime_report,
            dense_steps,
        )?;
        let reformer = build_reformer_chunked_proxy_cell(workload_target, &cases, dense_steps);
        let hull = build_hull_cache_runtime_cell(workload_target, &cases, dense_steps);
        let hierarchical_hull = build_hierarchical_hull_cell(
            workload_target,
            dense_steps,
            geometric_report.workload_summaries.as_slice(),
        )?;

        let cells = vec![
            dense,
            sparse,
            linear_recurrent,
            reformer,
            hull,
            hierarchical_hull,
        ];
        let fastest_cell = cells
            .iter()
            .filter(|cell| {
                cell.comparison_outcome_vs_dense_reference
                    != TassadarEfficientAttentionComparisonOutcome::Refuse
            })
            .max_by(|left, right| {
                left.average_steps_per_second
                    .partial_cmp(&right.average_steps_per_second)
                    .expect("finite matrix throughput")
            })
            .expect("dense baseline should always participate");
        let fallback_or_refusal_family_count = cells
            .iter()
            .filter(|cell| cell.fallback_case_count > 0 || cell.refused_case_count > 0)
            .count();

        rows.push(TassadarEfficientAttentionBaselineMatrixRow {
            workload_target,
            dense_reference_steps_per_second: dense_steps,
            fastest_family_kind: fastest_cell.family_kind,
            fastest_family_steps_per_second: fastest_cell.average_steps_per_second,
            summary: format!(
                "same-harness efficient-attention matrix row for {:?}: fastest family is `{}` at {:.3} steps/s vs dense {:.3}, with {} family rows carrying explicit fallback or refusal posture",
                workload_target,
                fastest_cell.family_kind.label(),
                fastest_cell.average_steps_per_second,
                dense_steps,
                fallback_or_refusal_family_count,
            ),
            cells,
        });
    }

    let hull_runtime_workload_wins = rows
        .iter()
        .filter(|row| {
            row.fastest_family_kind
                == TassadarEfficientAttentionBaselineFamilyKind::HullCacheRuntime
        })
        .count();
    let hierarchical_hull_workload_wins = rows
        .iter()
        .filter(|row| {
            row.fastest_family_kind
                == TassadarEfficientAttentionBaselineFamilyKind::HierarchicalHullResearch
        })
        .count();
    let recurrent_workload_wins = rows
        .iter()
        .filter(|row| {
            row.fastest_family_kind
                == TassadarEfficientAttentionBaselineFamilyKind::LinearRecurrentRuntime
        })
        .count();

    let mut report = TassadarEfficientAttentionBaselineMatrixReport {
        schema_version: REPORT_SCHEMA_VERSION,
        matrix_id: String::from("tassadar.efficient_attention_baseline_matrix.v0"),
        generated_from_artifacts: vec![
            String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
            String::from(TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF),
            String::from(TASSADAR_GEOMETRIC_VARIANT_REPORT_REF),
        ],
        rows,
        claim_boundary: String::from(
            "this matrix keeps current runtime dense, hull-cache, sparse-top-k, and recurrent-baseline execution evidence separate from research-only hierarchical-hull and Reformer-style proxy rows; the recurrent row is now artifact-backed runtime evidence, but it remains research-only and non-promoted",
        ),
        summary: format!(
            "Public efficient-attention baseline matrix now freezes {} article-class workload rows under one shared artifact contract: current promoted HullCache is fastest on {} workloads, the research hierarchical-hull candidate is fastest on {}, and the recurrent runtime baseline is fastest on {}; sparse, Reformer-style, hull-family, and recurrent rows now carry explicit win/tie/lose/refuse posture against the dense reference floor instead of comparing only to naive dense headlines.",
            article_workload_targets().len(),
            hull_runtime_workload_wins,
            hierarchical_hull_workload_wins,
            recurrent_workload_wins,
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_efficient_attention_baseline_matrix_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_efficient_attention_baseline_matrix_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF)
}

pub fn write_tassadar_efficient_attention_baseline_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarEfficientAttentionBaselineMatrixReport,
    TassadarEfficientAttentionBaselineMatrixError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarEfficientAttentionBaselineMatrixError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_efficient_attention_baseline_matrix_report()?;
    let bytes = serde_json::to_vec_pretty(&report).expect("matrix report should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarEfficientAttentionBaselineMatrixError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_dense_reference_linear_cell(
    workload_target: TassadarWorkloadTarget,
    cases: &[&TassadarBenchmarkCaseReport],
) -> TassadarEfficientAttentionBaselineCell {
    let case_count = cases.len() as u32;
    let average_steps_per_second = average(
        cases
            .iter()
            .map(|case| case.reference_linear_steps_per_second),
    );
    let average_remaining_gap_vs_cpu_reference = average(cases.iter().map(|case| {
        case.cpu_reference_steps_per_second / case.reference_linear_steps_per_second.max(1e-9)
    }));

    TassadarEfficientAttentionBaselineCell {
        family_kind: TassadarEfficientAttentionBaselineFamilyKind::DenseReferenceLinear,
        measurement_kind: TassadarEfficientAttentionMeasurementKind::ArtifactBackedRuntime,
        claim_class: TassadarClaimClass::CompiledArticleClass,
        claim_boundary: String::from(
            "current exact reference-linear dense floor on the committed article-class benchmark package",
        ),
        workload_target,
        case_count,
        direct_case_count: case_count,
        fallback_case_count: 0,
        refused_case_count: 0,
        exact_case_count: case_count,
        average_steps_per_second: round_metric(average_steps_per_second),
        average_speedup_over_dense_reference: 1.0,
        average_remaining_gap_vs_cpu_reference: round_metric(
            average_remaining_gap_vs_cpu_reference,
        ),
        comparison_outcome_vs_dense_reference: TassadarEfficientAttentionComparisonOutcome::Tie,
        selection_reason_counts: BTreeMap::new(),
        artifact_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        note: String::from(
            "exact reference-linear dense executor baseline reused as the same-harness floor for every other family row",
        ),
    }
}

fn build_sparse_top_k_cell(
    workload_target: TassadarWorkloadTarget,
    cases: &[&TassadarBenchmarkCaseReport],
    dense_steps: f64,
) -> TassadarEfficientAttentionBaselineCell {
    let case_count = cases.len() as u32;
    let direct_case_count = cases
        .iter()
        .filter(|case| case.sparse_top_k_selection_state == TassadarExecutorSelectionState::Direct)
        .count() as u32;
    let fallback_case_count = cases
        .iter()
        .filter(|case| {
            case.sparse_top_k_selection_state == TassadarExecutorSelectionState::Fallback
        })
        .count() as u32;
    let refused_case_count = cases
        .iter()
        .filter(|case| case.sparse_top_k_selection_state == TassadarExecutorSelectionState::Refused)
        .count() as u32;
    let average_steps_per_second =
        average(cases.iter().map(|case| case.sparse_top_k_steps_per_second));
    let average_remaining_gap_vs_cpu_reference = average(
        cases
            .iter()
            .map(|case| case.sparse_top_k_remaining_gap_vs_cpu_reference),
    );

    TassadarEfficientAttentionBaselineCell {
        family_kind: TassadarEfficientAttentionBaselineFamilyKind::SparseTopKValidated,
        measurement_kind: TassadarEfficientAttentionMeasurementKind::ArtifactBackedRuntime,
        claim_class: TassadarClaimClass::CompiledArticleClass,
        claim_boundary: String::from(
            "current validated sparse-top-k runtime posture on the shared article-class benchmark package; direct on bounded cases and explicit fallback where the current validation ceiling ends",
        ),
        workload_target,
        case_count,
        direct_case_count,
        fallback_case_count,
        refused_case_count,
        exact_case_count: case_count.saturating_sub(refused_case_count),
        average_steps_per_second: round_metric(average_steps_per_second),
        average_speedup_over_dense_reference: round_metric(
            average_steps_per_second / dense_steps.max(1e-9),
        ),
        average_remaining_gap_vs_cpu_reference: round_metric(
            average_remaining_gap_vs_cpu_reference,
        ),
        comparison_outcome_vs_dense_reference: comparison_outcome(
            average_steps_per_second,
            dense_steps,
            refused_case_count == case_count,
        ),
        selection_reason_counts: reason_counts(
            cases.iter()
                .filter_map(|case| selection_reason_key(case.sparse_top_k_selection_reason)),
        ),
        artifact_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        note: String::from(
            "validated runtime sparse-top-k baseline on the same article-class cases; explicit fallback remains part of the surface rather than being hidden behind aggregate speedups",
        ),
    }
}

fn build_linear_recurrent_runtime_cell(
    workload_target: TassadarWorkloadTarget,
    cases: &[&TassadarBenchmarkCaseReport],
    recurrent_runtime_report: &TassadarRecurrentFastPathRuntimeBaselineReport,
    dense_steps: f64,
) -> Result<TassadarEfficientAttentionBaselineCell, TassadarEfficientAttentionBaselineMatrixError> {
    let recurrent_cases = cases
        .iter()
        .map(|case| {
            recurrent_runtime_report
                .case_reports
                .iter()
                .find(|recurrent_case| recurrent_case.case_id == case.case_id)
                .ok_or(
                    TassadarEfficientAttentionBaselineMatrixError::MissingBenchmarkCases {
                        workload: workload_target,
                    },
                )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let case_count = recurrent_cases.len() as u32;
    let direct_case_count = recurrent_cases
        .iter()
        .filter(|case| case.selection_state == TassadarExecutorSelectionState::Direct)
        .count() as u32;
    let fallback_case_count = recurrent_cases
        .iter()
        .filter(|case| case.selection_state == TassadarExecutorSelectionState::Fallback)
        .count() as u32;
    let average_steps_per_second = average(
        recurrent_cases
            .iter()
            .map(|case| case.recurrent_steps_per_second),
    );
    let average_remaining_gap_vs_cpu_reference = average(
        recurrent_cases
            .iter()
            .map(|case| case.recurrent_remaining_gap_vs_cpu_reference),
    );

    Ok(TassadarEfficientAttentionBaselineCell {
        family_kind: TassadarEfficientAttentionBaselineFamilyKind::LinearRecurrentRuntime,
        measurement_kind: TassadarEfficientAttentionMeasurementKind::ArtifactBackedRuntime,
        claim_class: TassadarClaimClass::ResearchOnly,
        claim_boundary: recurrent_runtime_report.claim_boundary.clone(),
        workload_target,
        case_count,
        direct_case_count,
        fallback_case_count,
        refused_case_count: 0,
        exact_case_count: case_count,
        average_steps_per_second: round_metric(average_steps_per_second),
        average_speedup_over_dense_reference: round_metric(
            average_steps_per_second / dense_steps.max(1e-9),
        ),
        average_remaining_gap_vs_cpu_reference: round_metric(
            average_remaining_gap_vs_cpu_reference,
        ),
        comparison_outcome_vs_dense_reference: comparison_outcome(
            average_steps_per_second,
            dense_steps,
            false,
        ),
        selection_reason_counts: recurrent_cases
            .iter()
            .filter_map(|case| {
                case.selection_reason.map(|reason| {
                    serde_json::to_string(&reason)
                        .unwrap_or_else(|_| String::from("\"unknown_selection_reason\""))
                        .trim_matches('"')
                        .to_string()
                })
            })
            .fold(BTreeMap::new(), |mut counts, reason| {
                *counts.entry(reason).or_insert(0) += 1;
                counts
            }),
        artifact_ref: String::from(TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF),
        note: String::from(
            "research-only artifact-backed recurrent runtime baseline on the same article-class benchmark package; direct on the smaller direct profiles and explicit exact fallback on the larger article-sized profiles",
        ),
    })
}

fn build_reformer_chunked_proxy_cell(
    workload_target: TassadarWorkloadTarget,
    cases: &[&TassadarBenchmarkCaseReport],
    dense_steps: f64,
) -> TassadarEfficientAttentionBaselineCell {
    let case_count = cases.len() as u32;
    let mut direct_case_count = 0_u32;
    let mut fallback_case_count = 0_u32;
    let mut refused_case_count = 0_u32;
    let mut exact_case_count = 0_u32;
    let mut selection_reason_counts = BTreeMap::new();
    let mut realized_steps = Vec::new();
    let mut remaining_gaps = Vec::new();

    for case in cases {
        match reformer_proxy_policy(case.workload_target) {
            ReformerProxyPolicy::Direct => {
                direct_case_count = direct_case_count.saturating_add(1);
                exact_case_count = exact_case_count.saturating_add(1);
                let speedup = reformer_proxy_speedup(case.trace_steps);
                let steps = (case.reference_linear_steps_per_second * speedup)
                    .min(case.cpu_reference_steps_per_second * REFORMER_DIRECT_CPU_CEILING);
                realized_steps.push(steps);
                remaining_gaps.push(case.cpu_reference_steps_per_second / steps.max(1e-9));
            }
            ReformerProxyPolicy::Fallback(reason) => {
                fallback_case_count = fallback_case_count.saturating_add(1);
                exact_case_count = exact_case_count.saturating_add(1);
                realized_steps.push(case.reference_linear_steps_per_second);
                remaining_gaps.push(
                    case.cpu_reference_steps_per_second
                        / case.reference_linear_steps_per_second.max(1e-9),
                );
                *selection_reason_counts
                    .entry(String::from(reason))
                    .or_insert(0) += 1;
            }
            ReformerProxyPolicy::Refused(reason) => {
                refused_case_count = refused_case_count.saturating_add(1);
                *selection_reason_counts
                    .entry(String::from(reason))
                    .or_insert(0) += 1;
            }
        }
    }

    let average_steps_per_second = average(realized_steps.into_iter());
    let average_remaining_gap_vs_cpu_reference = average(remaining_gaps.into_iter());

    TassadarEfficientAttentionBaselineCell {
        family_kind: TassadarEfficientAttentionBaselineFamilyKind::ReformerChunkedProxy,
        measurement_kind: TassadarEfficientAttentionMeasurementKind::ProxyCostModel,
        claim_class: TassadarClaimClass::ResearchOnly,
        claim_boundary: String::from(
            "research-only proxy for a chunked / Reformer-style family under the same article-class harness; direct only on bounded chunk-local cases, explicit fallback on fragmented control-flow, and explicit refusal on search or long-loop revisitation",
        ),
        workload_target,
        case_count,
        direct_case_count,
        fallback_case_count,
        refused_case_count,
        exact_case_count,
        average_steps_per_second: round_metric(average_steps_per_second),
        average_speedup_over_dense_reference: round_metric(
            average_steps_per_second / dense_steps.max(1e-9),
        ),
        average_remaining_gap_vs_cpu_reference: round_metric(
            average_remaining_gap_vs_cpu_reference,
        ),
        comparison_outcome_vs_dense_reference: comparison_outcome(
            average_steps_per_second,
            dense_steps,
            refused_case_count == case_count,
        ),
        selection_reason_counts,
        artifact_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        note: String::from(
            "research-only chunked / Reformer-style proxy row on the same benchmark package, with explicit fallback or refusal instead of silently widening support",
        ),
    }
}

fn build_hull_cache_runtime_cell(
    workload_target: TassadarWorkloadTarget,
    cases: &[&TassadarBenchmarkCaseReport],
    dense_steps: f64,
) -> TassadarEfficientAttentionBaselineCell {
    let case_count = cases.len() as u32;
    let direct_case_count = cases
        .iter()
        .filter(|case| case.selection_state == TassadarExecutorSelectionState::Direct)
        .count() as u32;
    let fallback_case_count = cases
        .iter()
        .filter(|case| case.selection_state == TassadarExecutorSelectionState::Fallback)
        .count() as u32;
    let refused_case_count = cases
        .iter()
        .filter(|case| case.selection_state == TassadarExecutorSelectionState::Refused)
        .count() as u32;
    let average_steps_per_second =
        average(cases.iter().map(|case| case.hull_cache_steps_per_second));
    let average_remaining_gap_vs_cpu_reference = average(
        cases
            .iter()
            .map(|case| case.hull_cache_remaining_gap_vs_cpu_reference),
    );

    TassadarEfficientAttentionBaselineCell {
        family_kind: TassadarEfficientAttentionBaselineFamilyKind::HullCacheRuntime,
        measurement_kind: TassadarEfficientAttentionMeasurementKind::ArtifactBackedRuntime,
        claim_class: TassadarClaimClass::CompiledArticleClass,
        claim_boundary: String::from(
            "current promoted HullCache runtime posture on the shared article-class benchmark package, including explicit fallback truth outside the current validated direct subset",
        ),
        workload_target,
        case_count,
        direct_case_count,
        fallback_case_count,
        refused_case_count,
        exact_case_count: case_count.saturating_sub(refused_case_count),
        average_steps_per_second: round_metric(average_steps_per_second),
        average_speedup_over_dense_reference: round_metric(
            average_steps_per_second / dense_steps.max(1e-9),
        ),
        average_remaining_gap_vs_cpu_reference: round_metric(
            average_remaining_gap_vs_cpu_reference,
        ),
        comparison_outcome_vs_dense_reference: comparison_outcome(
            average_steps_per_second,
            dense_steps,
            refused_case_count == case_count,
        ),
        selection_reason_counts: reason_counts(
            cases.iter()
                .filter_map(|case| selection_reason_key(case.selection_reason)),
        ),
        artifact_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        note: String::from(
            "promoted HullCache runtime row on the same benchmark package, preserving explicit fallback posture where current control-flow limits still apply",
        ),
    }
}

fn build_hierarchical_hull_cell(
    workload_target: TassadarWorkloadTarget,
    dense_steps: f64,
    geometric_summaries: &[TassadarGeometricVariantWorkloadSummary],
) -> Result<TassadarEfficientAttentionBaselineCell, TassadarEfficientAttentionBaselineMatrixError> {
    let summary = geometric_summaries
        .iter()
        .find(|summary| {
            summary.workload_target == workload_target
                && summary.variant_id == "tassadar.geometric_variant.hierarchical_hull_candidate.v0"
        })
        .ok_or(
            TassadarEfficientAttentionBaselineMatrixError::MissingGeometricSummary {
                workload: workload_target,
            },
        )?;

    Ok(TassadarEfficientAttentionBaselineCell {
        family_kind: TassadarEfficientAttentionBaselineFamilyKind::HierarchicalHullResearch,
        measurement_kind: TassadarEfficientAttentionMeasurementKind::ArtifactBackedResearch,
        claim_class: summary.claim_class,
        claim_boundary: String::from(match summary.claim_boundary {
            crate::TassadarGeometricVariantClaimBoundary::RuntimeReady => "runtime_ready",
            crate::TassadarGeometricVariantClaimBoundary::ResearchOnly => "research_only",
        }),
        workload_target,
        case_count: summary.direct_case_count as u32
            + summary.fallback_case_count as u32
            + summary.refused_case_count as u32,
        direct_case_count: summary.direct_case_count as u32,
        fallback_case_count: summary.fallback_case_count as u32,
        refused_case_count: summary.refused_case_count as u32,
        exact_case_count: summary.exact_case_count as u32,
        average_steps_per_second: round_metric(summary.average_steps_per_second),
        average_speedup_over_dense_reference: round_metric(
            summary.average_steps_per_second / dense_steps.max(1e-9),
        ),
        average_remaining_gap_vs_cpu_reference: round_metric(
            summary.average_remaining_gap_vs_cpu_reference,
        ),
        comparison_outcome_vs_dense_reference: comparison_outcome(
            summary.average_steps_per_second,
            dense_steps,
            summary.refused_case_count
                == summary.direct_case_count
                    + summary.fallback_case_count
                    + summary.refused_case_count,
        ),
        selection_reason_counts: summary
            .selection_reason_counts
            .iter()
            .map(|(reason, count)| (reason.clone(), *count as u32))
            .collect(),
        artifact_ref: summary.artifact_ref.clone(),
        note: summary.note.clone(),
    })
}

fn reformer_proxy_speedup(trace_steps: u64) -> f64 {
    let log_gain = (trace_steps.max(2) as f64).log2() / 6.0;
    (1.0 + log_gain).clamp(1.15, 4.0)
}

fn comparison_outcome(
    average_steps_per_second: f64,
    dense_steps: f64,
    refused: bool,
) -> TassadarEfficientAttentionComparisonOutcome {
    if refused {
        return TassadarEfficientAttentionComparisonOutcome::Refuse;
    }
    let ratio = average_steps_per_second / dense_steps.max(1e-9);
    if ratio > 1.05 {
        TassadarEfficientAttentionComparisonOutcome::Win
    } else if ratio < 0.95 {
        TassadarEfficientAttentionComparisonOutcome::Lose
    } else {
        TassadarEfficientAttentionComparisonOutcome::Tie
    }
}

fn average(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn reason_counts(reasons: impl Iterator<Item = String>) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for reason in reasons {
        *counts.entry(reason).or_insert(0) += 1;
    }
    counts
}

fn selection_reason_key(reason: Option<TassadarExecutorSelectionReason>) -> Option<String> {
    reason.map(|reason| {
        serde_json::to_string(&reason)
            .expect("selection reason should serialize")
            .trim_matches('"')
            .to_string()
    })
}

fn article_workload_targets() -> [TassadarWorkloadTarget; 6] {
    [
        TassadarWorkloadTarget::MicroWasmKernel,
        TassadarWorkloadTarget::BranchHeavyKernel,
        TassadarWorkloadTarget::MemoryHeavyKernel,
        TassadarWorkloadTarget::LongLoopKernel,
        TassadarWorkloadTarget::SudokuClass,
        TassadarWorkloadTarget::HungarianMatching,
    ]
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn read_repo_json<T>(
    repo_relative_path: &str,
) -> Result<T, TassadarEfficientAttentionBaselineMatrixError>
where
    T: DeserializeOwned,
{
    let path = repo_root().join(repo_relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarEfficientAttentionBaselineMatrixError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarEfficientAttentionBaselineMatrixError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("efficient attention baseline matrix should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

enum ReformerProxyPolicy {
    Direct,
    Fallback(&'static str),
    Refused(&'static str),
}

fn reformer_proxy_policy(workload_target: TassadarWorkloadTarget) -> ReformerProxyPolicy {
    match workload_target {
        TassadarWorkloadTarget::MicroWasmKernel
        | TassadarWorkloadTarget::MemoryHeavyKernel
        | TassadarWorkloadTarget::HungarianMatching => ReformerProxyPolicy::Direct,
        TassadarWorkloadTarget::BranchHeavyKernel => {
            ReformerProxyPolicy::Fallback("chunk_boundary_fragmentation")
        }
        TassadarWorkloadTarget::LongLoopKernel => {
            ReformerProxyPolicy::Refused("loop_revisitation_outside_chunk_locality")
        }
        TassadarWorkloadTarget::SudokuClass => {
            ReformerProxyPolicy::Refused("search_backtracking_outside_chunk_locality")
        }
        TassadarWorkloadTarget::ArithmeticMicroprogram
        | TassadarWorkloadTarget::ClrsShortestPath
        | TassadarWorkloadTarget::MemoryLookupMicroprogram
        | TassadarWorkloadTarget::BranchControlFlowMicroprogram
        | TassadarWorkloadTarget::ModuleMemcpy
        | TassadarWorkloadTarget::ModuleParsing
        | TassadarWorkloadTarget::ModuleChecksum
        | TassadarWorkloadTarget::ModuleVmStyle => ReformerProxyPolicy::Direct,
    }
}

#[cfg(test)]
mod tests {
    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        build_tassadar_efficient_attention_baseline_matrix_report,
        tassadar_efficient_attention_baseline_matrix_report_path,
        write_tassadar_efficient_attention_baseline_matrix_report,
        TassadarEfficientAttentionBaselineFamilyKind,
        TassadarEfficientAttentionBaselineMatrixReport,
        TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF,
    };
    use psionic_environments::TassadarWorkloadTarget;

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    fn read_repo_json<T>(repo_relative_path: &str) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn efficient_attention_baseline_matrix_tracks_runtime_and_proxy_boundaries(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_efficient_attention_baseline_matrix_report()?;
        assert_eq!(report.rows.len(), 6);

        let micro = report
            .rows
            .iter()
            .find(|row| row.workload_target == TassadarWorkloadTarget::MicroWasmKernel)
            .expect("micro workload row");
        let hull = micro
            .cells
            .iter()
            .find(|cell| {
                cell.family_kind == TassadarEfficientAttentionBaselineFamilyKind::HullCacheRuntime
            })
            .expect("micro hull cell");
        assert!(hull.average_speedup_over_dense_reference > 1.0);

        let long_loop = report
            .rows
            .iter()
            .find(|row| row.workload_target == TassadarWorkloadTarget::LongLoopKernel)
            .expect("long loop workload row");
        let recurrent = long_loop
            .cells
            .iter()
            .find(|cell| {
                cell.family_kind
                    == TassadarEfficientAttentionBaselineFamilyKind::LinearRecurrentRuntime
            })
            .expect("linear recurrent runtime cell");
        assert!(recurrent.average_speedup_over_dense_reference > 1.0);
        assert!(recurrent.direct_case_count > 0);
        let reformer = long_loop
            .cells
            .iter()
            .find(|cell| {
                cell.family_kind
                    == TassadarEfficientAttentionBaselineFamilyKind::ReformerChunkedProxy
            })
            .expect("reformer proxy cell");
        assert_eq!(reformer.refused_case_count, reformer.case_count);

        let sudoku = report
            .rows
            .iter()
            .find(|row| row.workload_target == TassadarWorkloadTarget::SudokuClass)
            .expect("sudoku workload row");
        let hull = sudoku
            .cells
            .iter()
            .find(|cell| {
                cell.family_kind == TassadarEfficientAttentionBaselineFamilyKind::HullCacheRuntime
            })
            .expect("hull runtime cell");
        assert!(hull.fallback_case_count > 0);
        let recurrent = sudoku
            .cells
            .iter()
            .find(|cell| {
                cell.family_kind
                    == TassadarEfficientAttentionBaselineFamilyKind::LinearRecurrentRuntime
            })
            .expect("recurrent runtime cell");
        assert!(recurrent.fallback_case_count > 0);
        Ok(())
    }

    #[test]
    fn efficient_attention_baseline_matrix_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_efficient_attention_baseline_matrix_report()?;
        let persisted: TassadarEfficientAttentionBaselineMatrixReport =
            read_repo_json(TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn write_efficient_attention_baseline_matrix_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let report = write_tassadar_efficient_attention_baseline_matrix_report(
            temp_dir
                .path()
                .join("tassadar_efficient_attention_baseline_matrix.json"),
        )?;
        let persisted: TassadarEfficientAttentionBaselineMatrixReport =
            serde_json::from_slice(&std::fs::read(
                temp_dir
                    .path()
                    .join("tassadar_efficient_attention_baseline_matrix.json"),
            )?)?;
        assert_eq!(persisted, report);
        assert_eq!(
            tassadar_efficient_attention_baseline_matrix_report_path().strip_prefix(repo_root())?,
            std::path::Path::new(TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF)
        );
        Ok(())
    }
}
