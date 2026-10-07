use std::{fs, path::Path};

use psionic_environments::TassadarWorkloadTarget;
use psionic_eval::{
    build_tassadar_efficient_attention_baseline_matrix_report,
    TassadarEfficientAttentionBaselineFamilyKind, TassadarEfficientAttentionComparisonOutcome,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_OUTPUT_DIR: &str =
    "fixtures/tassadar/reports";
pub const TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_REPORT_FILE: &str =
    "tassadar_efficient_attention_baseline_summary.json";
pub const TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_efficient_attention_baseline_summary.json";
pub const TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_efficient_attention_baseline_summary";
pub const TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_TEST_COMMAND: &str =
    "cargo test -p psionic-research efficient_attention_baseline_summary_matches_committed_truth -- --nocapture";

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEfficientAttentionFamilyOutcomeSummary {
    pub family_kind: TassadarEfficientAttentionBaselineFamilyKind,
    pub workload_win_count: u32,
    pub workload_tie_count: u32,
    pub workload_loss_count: u32,
    pub workload_refuse_count: u32,
    pub fastest_workload_targets: Vec<TassadarWorkloadTarget>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarEfficientAttentionWorkloadWinner {
    pub workload_target: TassadarWorkloadTarget,
    pub fastest_family_kind: TassadarEfficientAttentionBaselineFamilyKind,
    pub fastest_family_steps_per_second: f64,
    pub dense_reference_steps_per_second: f64,
    pub fastest_family_speedup_over_dense_reference: f64,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarEfficientAttentionBaselineSummaryReport {
    pub schema_version: u16,
    pub matrix_report_ref: String,
    pub matrix_report_digest: String,
    pub regeneration_commands: Vec<String>,
    pub family_summaries: Vec<TassadarEfficientAttentionFamilyOutcomeSummary>,
    pub workload_winners: Vec<TassadarEfficientAttentionWorkloadWinner>,
    pub claim_class: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarEfficientAttentionBaselineSummaryError {
    #[error(transparent)]
    Eval(#[from] psionic_eval::TassadarEfficientAttentionBaselineMatrixError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

pub fn build_tassadar_efficient_attention_baseline_summary_report() -> Result<
    TassadarEfficientAttentionBaselineSummaryReport,
    TassadarEfficientAttentionBaselineSummaryError,
> {
    let matrix = build_tassadar_efficient_attention_baseline_matrix_report()?;
    let family_order = [
        TassadarEfficientAttentionBaselineFamilyKind::DenseReferenceLinear,
        TassadarEfficientAttentionBaselineFamilyKind::SparseTopKValidated,
        TassadarEfficientAttentionBaselineFamilyKind::LinearRecurrentRuntime,
        TassadarEfficientAttentionBaselineFamilyKind::ReformerChunkedProxy,
        TassadarEfficientAttentionBaselineFamilyKind::HullCacheRuntime,
        TassadarEfficientAttentionBaselineFamilyKind::HierarchicalHullResearch,
    ];

    let mut family_summaries = Vec::new();
    for family_kind in family_order {
        let cells = matrix
            .rows
            .iter()
            .filter_map(|row| {
                row.cells
                    .iter()
                    .find(|cell| cell.family_kind == family_kind)
                    .map(|cell| (row, cell))
            })
            .collect::<Vec<_>>();
        let fastest_workload_targets = cells
            .iter()
            .filter(|(row, _)| row.fastest_family_kind == family_kind)
            .map(|(row, _)| row.workload_target)
            .collect::<Vec<_>>();
        family_summaries.push(TassadarEfficientAttentionFamilyOutcomeSummary {
            family_kind,
            workload_win_count: cells
                .iter()
                .filter(|(_, cell)| {
                    cell.comparison_outcome_vs_dense_reference
                        == TassadarEfficientAttentionComparisonOutcome::Win
                })
                .count() as u32,
            workload_tie_count: cells
                .iter()
                .filter(|(_, cell)| {
                    cell.comparison_outcome_vs_dense_reference
                        == TassadarEfficientAttentionComparisonOutcome::Tie
                })
                .count() as u32,
            workload_loss_count: cells
                .iter()
                .filter(|(_, cell)| {
                    cell.comparison_outcome_vs_dense_reference
                        == TassadarEfficientAttentionComparisonOutcome::Lose
                })
                .count() as u32,
            workload_refuse_count: cells
                .iter()
                .filter(|(_, cell)| {
                    cell.comparison_outcome_vs_dense_reference
                        == TassadarEfficientAttentionComparisonOutcome::Refuse
                })
                .count() as u32,
            fastest_workload_targets,
            note: family_summary_note(family_kind),
        });
    }

    let workload_winners = matrix
        .rows
        .iter()
        .map(|row| TassadarEfficientAttentionWorkloadWinner {
            workload_target: row.workload_target,
            fastest_family_kind: row.fastest_family_kind,
            fastest_family_steps_per_second: row.fastest_family_steps_per_second,
            dense_reference_steps_per_second: row.dense_reference_steps_per_second,
            fastest_family_speedup_over_dense_reference: round_metric(
                row.fastest_family_steps_per_second
                    / row.dense_reference_steps_per_second.max(1e-9),
            ),
            note: row.summary.clone(),
        })
        .collect::<Vec<_>>();

    let hull_runtime_fastest_count = workload_winners
        .iter()
        .filter(|winner| {
            winner.fastest_family_kind
                == TassadarEfficientAttentionBaselineFamilyKind::HullCacheRuntime
        })
        .count();
    let reformer_refusal_count = family_summaries
        .iter()
        .find(|summary| {
            summary.family_kind
                == TassadarEfficientAttentionBaselineFamilyKind::ReformerChunkedProxy
        })
        .map(|summary| summary.workload_refuse_count)
        .unwrap_or(0);
    let hierarchical_hull_fastest_count = workload_winners
        .iter()
        .filter(|winner| {
            winner.fastest_family_kind
                == TassadarEfficientAttentionBaselineFamilyKind::HierarchicalHullResearch
        })
        .count();

    let mut report = TassadarEfficientAttentionBaselineSummaryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        matrix_report_ref: String::from(
            psionic_eval::TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF,
        ),
        matrix_report_digest: matrix.report_digest.clone(),
        regeneration_commands: vec![
            String::from(TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_EXAMPLE_COMMAND),
            String::from(TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_TEST_COMMAND),
        ],
        family_summaries,
        workload_winners,
        claim_class: String::from("research_only"),
        claim_boundary: String::from(
            "this summary is a research-only aggregation over one same-harness efficient-attention matrix. Runtime dense, hull-cache, sparse, and recurrent rows stay artifact-backed, but the Reformer-style row remains a proxy baseline and the hierarchical-hull row remains unpromoted research evidence; none of that becomes served capability through this report alone",
        ),
        summary: format!(
            "Public efficient-attention summary now freezes {} same-harness workload winners from one shared matrix digest: promoted HullCache is still fastest on {} workloads, the research hierarchical-hull candidate is fastest on {}, and the Reformer-style proxy now carries explicit refuse posture on {} workloads while the recurrent row stays artifact-backed and research-only instead of dense-only hand-waving.",
            matrix.rows.len(),
            hull_runtime_fastest_count,
            hierarchical_hull_fastest_count,
            reformer_refusal_count,
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_efficient_attention_baseline_summary_report|",
        &report,
    );
    Ok(report)
}

pub fn run_tassadar_efficient_attention_baseline_summary_report(
    output_dir: &Path,
) -> Result<
    TassadarEfficientAttentionBaselineSummaryReport,
    TassadarEfficientAttentionBaselineSummaryError,
> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarEfficientAttentionBaselineSummaryError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_efficient_attention_baseline_summary_report()?;
    let report_path = output_dir.join(TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_REPORT_FILE);
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(&report_path, bytes).map_err(|error| {
        TassadarEfficientAttentionBaselineSummaryError::Write {
            path: report_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn family_summary_note(family_kind: TassadarEfficientAttentionBaselineFamilyKind) -> String {
    match family_kind {
        TassadarEfficientAttentionBaselineFamilyKind::DenseReferenceLinear => String::from(
            "exact dense floor reused for every same-harness comparison row",
        ),
        TassadarEfficientAttentionBaselineFamilyKind::SparseTopKValidated => String::from(
            "validated runtime sparse-top-k row with explicit fallback posture where the current ceiling ends",
        ),
        TassadarEfficientAttentionBaselineFamilyKind::LinearRecurrentRuntime => String::from(
            "research-only artifact-backed recurrent runtime row used to keep specialized claims honest against a real state-carry baseline rather than only naive dense replay",
        ),
        TassadarEfficientAttentionBaselineFamilyKind::ReformerChunkedProxy => String::from(
            "research-only chunked / Reformer-style proxy row with explicit fallback and refusal posture on locality-breaking workloads",
        ),
        TassadarEfficientAttentionBaselineFamilyKind::HullCacheRuntime => String::from(
            "promoted HullCache runtime row under the shared article-class benchmark contract",
        ),
        TassadarEfficientAttentionBaselineFamilyKind::HierarchicalHullResearch => String::from(
            "research-only hierarchical-hull row that widens direct exact coverage but remains unpromoted",
        ),
    }
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("efficient attention baseline summary should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use psionic_eval::TassadarEfficientAttentionBaselineFamilyKind;
    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        build_tassadar_efficient_attention_baseline_summary_report,
        run_tassadar_efficient_attention_baseline_summary_report,
        TassadarEfficientAttentionBaselineSummaryReport,
        TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_OUTPUT_DIR,
        TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_REPORT_REF,
    };
    use psionic_environments::TassadarWorkloadTarget;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
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
    fn efficient_attention_baseline_summary_highlights_winners_and_refusals(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_efficient_attention_baseline_summary_report()?;
        assert_eq!(report.workload_winners.len(), 6);
        assert!(report
            .workload_winners
            .iter()
            .any(|winner| winner.workload_target == TassadarWorkloadTarget::MicroWasmKernel));
        let reformer = report
            .family_summaries
            .iter()
            .find(|summary| {
                summary.family_kind
                    == TassadarEfficientAttentionBaselineFamilyKind::ReformerChunkedProxy
            })
            .expect("reformer summary");
        assert!(reformer.workload_refuse_count >= 2);
        Ok(())
    }

    #[test]
    fn efficient_attention_baseline_summary_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_efficient_attention_baseline_summary_report()?;
        let persisted: TassadarEfficientAttentionBaselineSummaryReport =
            read_repo_json(TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_REPORT_REF)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn efficient_attention_baseline_summary_writes_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_dir = tempdir()?;
        let report = run_tassadar_efficient_attention_baseline_summary_report(output_dir.path())?;
        let persisted: TassadarEfficientAttentionBaselineSummaryReport =
            serde_json::from_slice(&std::fs::read(
                output_dir
                    .path()
                    .join("tassadar_efficient_attention_baseline_summary.json"),
            )?)?;
        assert_eq!(persisted, report);
        assert_eq!(
            std::path::Path::new(TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_OUTPUT_DIR),
            std::path::Path::new("fixtures/tassadar/reports")
        );
        Ok(())
    }
}
