use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_hard_sudoku_benchmark_closure_report,
    build_tassadar_article_hungarian_demo_parity_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleHardSudokuBenchmarkClosureReport,
    TassadarArticleHardSudokuBenchmarkClosureReportError, TassadarArticleHungarianDemoParityReport,
    TassadarArticleHungarianDemoParityReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_CLOSURE_REPORT_REF,
    TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_demo_benchmark_equivalence_gate_report.json";
pub const TASSADAR_ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-demo-benchmark-equivalence-gate.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-182";
const HUNGARIAN_REQUIREMENT_ID: &str = "TAS-180";
const HARD_SUDOKU_REQUIREMENT_ID: &str = "TAS-181";
const EXPECTED_HUNGARIAN_CASE_ID: &str = "hungarian_10x10_test_a";
const EXPECTED_HARD_SUDOKU_CASE_ID: &str = "sudoku_9x9_test_a";
const EXPECTED_NAMED_ARTO_CASE_ID: &str = "sudoku_9x9_arto_inkala";
const OWNED_ROUTE_BOUNDARY_REFS: [&str; 2] = [
    "crates/psionic-transformer/Cargo.toml",
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoBenchmarkEquivalenceGateAcceptanceTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoBenchmarkEquivalenceHungarianReview {
    pub report_ref: String,
    pub report_id: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub canonical_case_id: String,
    pub session_fast_route_green: bool,
    pub hybrid_fast_route_green: bool,
    pub throughput_floor_green: bool,
    pub no_tool_proof_green: bool,
    pub binding_green: bool,
    pub hungarian_demo_parity_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoBenchmarkEquivalenceBenchmarkReview {
    pub report_ref: String,
    pub report_id: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub named_case_id: String,
    pub declared_case_ids: Vec<String>,
    pub named_arto_parity_green: bool,
    pub benchmark_wide_sudoku_parity_green: bool,
    pub session_fast_route_green: bool,
    pub hybrid_fast_route_green: bool,
    pub runtime_suite_green: bool,
    pub binding_green: bool,
    pub hard_sudoku_benchmark_closure_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoBenchmarkEquivalenceBindingReview {
    pub owned_route_boundary_refs: Vec<String>,
    pub owned_route_boundary_refs_exist: bool,
    pub hungarian_requirement_alignment_green: bool,
    pub hard_sudoku_requirement_alignment_green: bool,
    pub hungarian_case_alignment_green: bool,
    pub named_arto_case_alignment_green: bool,
    pub benchmark_suite_alignment_green: bool,
    pub upstream_binding_green: bool,
    pub binding_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoBenchmarkEquivalenceGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleDemoBenchmarkEquivalenceGateAcceptanceTie,
    pub hungarian_review: TassadarArticleDemoBenchmarkEquivalenceHungarianReview,
    pub benchmark_review: TassadarArticleDemoBenchmarkEquivalenceBenchmarkReview,
    pub binding_review: TassadarArticleDemoBenchmarkEquivalenceBindingReview,
    pub hungarian_demo_parity_green: bool,
    pub named_arto_parity_green: bool,
    pub benchmark_wide_sudoku_parity_green: bool,
    pub article_demo_benchmark_equivalence_gate_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleDemoBenchmarkEquivalenceGateReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Hungarian(#[from] TassadarArticleHungarianDemoParityReportError),
    #[error(transparent)]
    HardSudoku(#[from] TassadarArticleHardSudokuBenchmarkClosureReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_demo_benchmark_equivalence_gate_report() -> Result<
    TassadarArticleDemoBenchmarkEquivalenceGateReport,
    TassadarArticleDemoBenchmarkEquivalenceGateReportError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let hungarian_report = build_tassadar_article_hungarian_demo_parity_report()?;
    let benchmark_report = build_tassadar_article_hard_sudoku_benchmark_closure_report()?;
    Ok(build_report_from_inputs(
        acceptance_gate,
        hungarian_report,
        benchmark_report,
    ))
}

fn build_report_from_inputs(
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    hungarian_report: TassadarArticleHungarianDemoParityReport,
    benchmark_report: TassadarArticleHardSudokuBenchmarkClosureReport,
) -> TassadarArticleDemoBenchmarkEquivalenceGateReport {
    let acceptance_gate_tie = TassadarArticleDemoBenchmarkEquivalenceGateAcceptanceTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    };
    let hungarian_review = TassadarArticleDemoBenchmarkEquivalenceHungarianReview {
        report_ref: String::from(TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_REPORT_REF),
        report_id: hungarian_report.report_id.clone(),
        tied_requirement_id: hungarian_report
            .acceptance_gate_tie
            .tied_requirement_id
            .clone(),
        tied_requirement_satisfied: hungarian_report
            .acceptance_gate_tie
            .tied_requirement_satisfied,
        canonical_case_id: hungarian_report.frontend_review.canonical_case_id.clone(),
        session_fast_route_green: hungarian_report
            .fast_route_session_review
            .fast_route_direct_green,
        hybrid_fast_route_green: hungarian_report
            .fast_route_hybrid_review
            .fast_route_direct_green,
        throughput_floor_green: hungarian_report.throughput_review.declared_floor_green,
        no_tool_proof_green: hungarian_report.no_tool_proof_review.no_tool_proof_green,
        binding_green: hungarian_report.binding_review.binding_green,
        hungarian_demo_parity_green: hungarian_report.hungarian_demo_parity_green,
        detail: format!(
            "Hungarian demo report `{}` keeps tied_requirement_id={}, canonical_case_id={}, session_fast_route_green={}, hybrid_fast_route_green={}, throughput_floor_green={}, no_tool_proof_green={}, binding_green={}, and hungarian_demo_parity_green={}.",
            hungarian_report.report_id,
            hungarian_report.acceptance_gate_tie.tied_requirement_id,
            hungarian_report.frontend_review.canonical_case_id,
            hungarian_report.fast_route_session_review.fast_route_direct_green,
            hungarian_report.fast_route_hybrid_review.fast_route_direct_green,
            hungarian_report.throughput_review.declared_floor_green,
            hungarian_report.no_tool_proof_review.no_tool_proof_green,
            hungarian_report.binding_review.binding_green,
            hungarian_report.hungarian_demo_parity_green,
        ),
    };
    let benchmark_review = TassadarArticleDemoBenchmarkEquivalenceBenchmarkReview {
        report_ref: String::from(TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_CLOSURE_REPORT_REF),
        report_id: benchmark_report.report_id.clone(),
        tied_requirement_id: benchmark_report
            .acceptance_gate_tie
            .tied_requirement_id
            .clone(),
        tied_requirement_satisfied: benchmark_report
            .acceptance_gate_tie
            .tied_requirement_satisfied,
        named_case_id: benchmark_report.manifest_review.named_case_id.clone(),
        declared_case_ids: benchmark_report.manifest_review.declared_case_ids.clone(),
        named_arto_parity_green: benchmark_report.named_arto_green,
        benchmark_wide_sudoku_parity_green: benchmark_report.hard_sudoku_suite_green,
        session_fast_route_green: benchmark_report.fast_route_session_review.suite_green,
        hybrid_fast_route_green: benchmark_report.fast_route_hybrid_review.suite_green,
        runtime_suite_green: benchmark_report.runtime_review.suite_green,
        binding_green: benchmark_report.binding_review.binding_green,
        hard_sudoku_benchmark_closure_green: benchmark_report
            .hard_sudoku_benchmark_closure_green,
        detail: format!(
            "Hard-Sudoku benchmark report `{}` keeps tied_requirement_id={}, named_case_id={}, declared_case_count={}, named_arto_parity_green={}, benchmark_wide_sudoku_parity_green={}, session_fast_route_green={}, hybrid_fast_route_green={}, runtime_suite_green={}, binding_green={}, and hard_sudoku_benchmark_closure_green={}.",
            benchmark_report.report_id,
            benchmark_report.acceptance_gate_tie.tied_requirement_id,
            benchmark_report.manifest_review.named_case_id,
            benchmark_report.manifest_review.declared_case_ids.len(),
            benchmark_report.named_arto_green,
            benchmark_report.hard_sudoku_suite_green,
            benchmark_report.fast_route_session_review.suite_green,
            benchmark_report.fast_route_hybrid_review.suite_green,
            benchmark_report.runtime_review.suite_green,
            benchmark_report.binding_review.binding_green,
            benchmark_report.hard_sudoku_benchmark_closure_green,
        ),
    };
    let binding_review = build_binding_review(&hungarian_report, &benchmark_report);
    let hungarian_demo_parity_green = hungarian_report.hungarian_demo_parity_green;
    let named_arto_parity_green = benchmark_report.named_arto_green;
    let benchmark_wide_sudoku_parity_green = benchmark_report.hard_sudoku_suite_green;
    let article_demo_benchmark_equivalence_gate_green = acceptance_gate_tie
        .tied_requirement_satisfied
        && hungarian_demo_parity_green
        && named_arto_parity_green
        && benchmark_wide_sudoku_parity_green
        && binding_review.binding_green;

    let mut report = TassadarArticleDemoBenchmarkEquivalenceGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_demo_benchmark_equivalence_gate.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_CHECKER_REF,
        ),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        hungarian_review,
        benchmark_review,
        binding_review,
        hungarian_demo_parity_green,
        named_arto_parity_green,
        benchmark_wide_sudoku_parity_green,
        article_demo_benchmark_equivalence_gate_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && article_demo_benchmark_equivalence_gate_green,
        claim_boundary: String::from(
            "this report closes TAS-182 only. It freezes one unified article demo-and-benchmark gate on top of the canonical owned `psionic-transformer` route boundary by requiring the Hungarian demo parity row, the named Arto parity row, and the declared hard-Sudoku benchmark-suite row to stay green together. It does not imply single-run no-spill closure, clean-room weight causality, route minimality, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article demo-and-benchmark equivalence gate now records tied_requirement_satisfied={}, hungarian_demo_parity_green={}, named_arto_parity_green={}, benchmark_wide_sudoku_parity_green={}, binding_green={}, gate_green={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.hungarian_demo_parity_green,
        report.named_arto_parity_green,
        report.benchmark_wide_sudoku_parity_green,
        report.binding_review.binding_green,
        report.article_demo_benchmark_equivalence_gate_green,
        report.acceptance_gate_tie.blocked_issue_ids.first(),
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_demo_benchmark_equivalence_gate_report|",
        &report,
    );
    report
}

fn build_binding_review(
    hungarian_report: &TassadarArticleHungarianDemoParityReport,
    benchmark_report: &TassadarArticleHardSudokuBenchmarkClosureReport,
) -> TassadarArticleDemoBenchmarkEquivalenceBindingReview {
    let owned_route_boundary_refs = OWNED_ROUTE_BOUNDARY_REFS
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    let owned_route_boundary_refs_exist = owned_route_boundary_refs
        .iter()
        .all(|path| repo_root().join(path).is_file());
    let hungarian_requirement_alignment_green =
        hungarian_report.acceptance_gate_tie.tied_requirement_id == HUNGARIAN_REQUIREMENT_ID
            && hungarian_report
                .acceptance_gate_tie
                .tied_requirement_satisfied;
    let hard_sudoku_requirement_alignment_green =
        benchmark_report.acceptance_gate_tie.tied_requirement_id == HARD_SUDOKU_REQUIREMENT_ID
            && benchmark_report
                .acceptance_gate_tie
                .tied_requirement_satisfied;
    let hungarian_case_alignment_green =
        hungarian_report.frontend_review.canonical_case_id == EXPECTED_HUNGARIAN_CASE_ID;
    let named_arto_case_alignment_green =
        benchmark_report.manifest_review.named_case_id == EXPECTED_NAMED_ARTO_CASE_ID;
    let observed_benchmark_case_ids = benchmark_report
        .manifest_review
        .declared_case_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let expected_benchmark_case_ids = BTreeSet::from([
        String::from(EXPECTED_HARD_SUDOKU_CASE_ID),
        String::from(EXPECTED_NAMED_ARTO_CASE_ID),
    ]);
    let benchmark_suite_alignment_green =
        observed_benchmark_case_ids == expected_benchmark_case_ids;
    let upstream_binding_green = hungarian_report.binding_review.binding_green
        && benchmark_report.binding_review.binding_green;
    let binding_green = owned_route_boundary_refs_exist
        && hungarian_requirement_alignment_green
        && hard_sudoku_requirement_alignment_green
        && hungarian_case_alignment_green
        && named_arto_case_alignment_green
        && benchmark_suite_alignment_green
        && upstream_binding_green;

    TassadarArticleDemoBenchmarkEquivalenceBindingReview {
        owned_route_boundary_refs,
        owned_route_boundary_refs_exist,
        hungarian_requirement_alignment_green,
        hard_sudoku_requirement_alignment_green,
        hungarian_case_alignment_green,
        named_arto_case_alignment_green,
        benchmark_suite_alignment_green,
        upstream_binding_green,
        binding_green,
        detail: format!(
            "Binding review keeps owned_route_boundary_refs_exist={}, hungarian_requirement_alignment_green={}, hard_sudoku_requirement_alignment_green={}, hungarian_case_alignment_green={}, named_arto_case_alignment_green={}, benchmark_suite_alignment_green={}, and upstream_binding_green={}.",
            owned_route_boundary_refs_exist,
            hungarian_requirement_alignment_green,
            hard_sudoku_requirement_alignment_green,
            hungarian_case_alignment_green,
            named_arto_case_alignment_green,
            benchmark_suite_alignment_green,
            upstream_binding_green,
        ),
    }
}

#[must_use]
pub fn tassadar_article_demo_benchmark_equivalence_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF)
}

pub fn write_tassadar_article_demo_benchmark_equivalence_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleDemoBenchmarkEquivalenceGateReport,
    TassadarArticleDemoBenchmarkEquivalenceGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleDemoBenchmarkEquivalenceGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_demo_benchmark_equivalence_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleDemoBenchmarkEquivalenceGateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarArticleDemoBenchmarkEquivalenceGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarArticleDemoBenchmarkEquivalenceGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleDemoBenchmarkEquivalenceGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_report_from_inputs, build_tassadar_article_demo_benchmark_equivalence_gate_report,
        read_json, tassadar_article_demo_benchmark_equivalence_gate_report_path,
        write_tassadar_article_demo_benchmark_equivalence_gate_report,
        TassadarArticleDemoBenchmarkEquivalenceGateReport, EXPECTED_HARD_SUDOKU_CASE_ID,
        EXPECTED_HUNGARIAN_CASE_ID, EXPECTED_NAMED_ARTO_CASE_ID,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_article_hard_sudoku_benchmark_closure_report,
        build_tassadar_article_hungarian_demo_parity_report,
    };

    #[test]
    fn article_demo_benchmark_equivalence_gate_tracks_green_joined_surface() {
        let report =
            build_tassadar_article_demo_benchmark_equivalence_gate_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(
            report.hungarian_review.tied_requirement_id,
            String::from("TAS-180")
        );
        assert_eq!(
            report.benchmark_review.tied_requirement_id,
            String::from("TAS-181")
        );
        assert_eq!(
            report.hungarian_review.canonical_case_id,
            String::from(EXPECTED_HUNGARIAN_CASE_ID)
        );
        assert_eq!(
            report.benchmark_review.named_case_id,
            String::from(EXPECTED_NAMED_ARTO_CASE_ID)
        );
        assert!(report
            .benchmark_review
            .declared_case_ids
            .contains(&String::from(EXPECTED_HARD_SUDOKU_CASE_ID)));
        assert!(report.hungarian_demo_parity_green);
        assert!(report.named_arto_parity_green);
        assert!(report.benchmark_wide_sudoku_parity_green);
        assert!(report.binding_review.binding_green);
        assert!(report.article_demo_benchmark_equivalence_gate_green);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_demo_benchmark_equivalence_gate_requires_hungarian_demo_row() {
        let acceptance_gate =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate");
        let mut hungarian_report =
            build_tassadar_article_hungarian_demo_parity_report().expect("hungarian report");
        let benchmark_report = build_tassadar_article_hard_sudoku_benchmark_closure_report()
            .expect("benchmark report");
        hungarian_report.frontend_review.canonical_case_id = String::from("missing_hungarian_row");
        hungarian_report.hungarian_demo_parity_green = false;

        let report = build_report_from_inputs(acceptance_gate, hungarian_report, benchmark_report);

        assert!(!report.binding_review.hungarian_case_alignment_green);
        assert!(!report.hungarian_demo_parity_green);
        assert!(!report.article_demo_benchmark_equivalence_gate_green);
    }

    #[test]
    fn article_demo_benchmark_equivalence_gate_requires_named_arto_row() {
        let acceptance_gate =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate");
        let hungarian_report =
            build_tassadar_article_hungarian_demo_parity_report().expect("hungarian report");
        let mut benchmark_report = build_tassadar_article_hard_sudoku_benchmark_closure_report()
            .expect("benchmark report");
        benchmark_report.manifest_review.named_case_id = String::from("missing_named_arto_row");
        benchmark_report.named_arto_green = false;

        let report = build_report_from_inputs(acceptance_gate, hungarian_report, benchmark_report);

        assert!(!report.binding_review.named_arto_case_alignment_green);
        assert!(!report.named_arto_parity_green);
        assert!(!report.article_demo_benchmark_equivalence_gate_green);
    }

    #[test]
    fn article_demo_benchmark_equivalence_gate_requires_declared_benchmark_suite() {
        let acceptance_gate =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate");
        let hungarian_report =
            build_tassadar_article_hungarian_demo_parity_report().expect("hungarian report");
        let mut benchmark_report = build_tassadar_article_hard_sudoku_benchmark_closure_report()
            .expect("benchmark report");
        benchmark_report.manifest_review.declared_case_ids =
            vec![String::from(EXPECTED_NAMED_ARTO_CASE_ID)];
        benchmark_report.hard_sudoku_suite_green = false;

        let report = build_report_from_inputs(acceptance_gate, hungarian_report, benchmark_report);

        assert!(!report.binding_review.benchmark_suite_alignment_green);
        assert!(!report.benchmark_wide_sudoku_parity_green);
        assert!(!report.article_demo_benchmark_equivalence_gate_green);
    }

    #[test]
    fn article_demo_benchmark_equivalence_gate_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated =
            build_tassadar_article_demo_benchmark_equivalence_gate_report().expect("report");
        let committed: TassadarArticleDemoBenchmarkEquivalenceGateReport =
            read_json(tassadar_article_demo_benchmark_equivalence_gate_report_path())?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_demo_benchmark_equivalence_gate_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_demo_benchmark_equivalence_gate_report.json");
        let written = write_tassadar_article_demo_benchmark_equivalence_gate_report(&output_path)?;
        let persisted: TassadarArticleDemoBenchmarkEquivalenceGateReport =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_demo_benchmark_equivalence_gate_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_demo_benchmark_equivalence_gate_report.json")
        );
        Ok(())
    }
}
