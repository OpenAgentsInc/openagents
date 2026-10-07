use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

use psionic_data::{
    tassadar_verifier_guided_search_trace_family_contract,
    TassadarVerifierGuidedSearchTraceFamilyContract,
    TassadarVerifierGuidedSearchTraceFamilyError,
    TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_REF,
};
use psionic_runtime::{
    tassadar_verifier_guided_search_trace_artifacts, TassadarVerifierGuidedSearchEventKind,
    TassadarVerifierGuidedSearchTraceArtifact, TassadarVerifierGuidedSearchTraceError,
    TassadarVerifierGuidedSearchWorkloadFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json";

static VERIFIER_GUIDED_SEARCH_REPORT_LOCK: LazyLock<Mutex<()>> =
    LazyLock::new(|| Mutex::new(()));

/// Case-level metrics surfaced by the verifier-guided search evaluation report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchEvaluationCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Search workload family.
    pub workload_family: TassadarVerifierGuidedSearchWorkloadFamily,
    /// Stable search trace identifier.
    pub trace_id: String,
    /// Stable runtime trace digest.
    pub trace_digest: String,
    /// Explicit guess count.
    pub guess_count: u32,
    /// Explicit backtrack count.
    pub backtrack_count: u32,
    /// Explicit contradiction count.
    pub contradiction_count: u32,
    /// Verifier-certificate accuracy in basis points.
    pub verifier_certificate_accuracy_bps: u32,
    /// Backtrack exactness in basis points.
    pub backtrack_exactness_bps: u32,
    /// Recovery quality in basis points.
    pub recovery_quality_bps: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// Family-level summary in the verifier-guided search evaluation report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchEvaluationFamilySummary {
    /// Search workload family.
    pub workload_family: TassadarVerifierGuidedSearchWorkloadFamily,
    /// Number of seeded cases in the family.
    pub case_count: u32,
    /// Mean guess count rounded down.
    pub mean_guess_count: u32,
    /// Mean backtrack count rounded down.
    pub mean_backtrack_count: u32,
    /// Mean verifier-certificate accuracy in basis points.
    pub verifier_certificate_accuracy_bps: u32,
    /// Mean backtrack exactness in basis points.
    pub backtrack_exactness_bps: u32,
    /// Mean recovery quality in basis points.
    pub recovery_quality_bps: u32,
}

/// Committed evaluation report over the verifier-guided search trace lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchEvaluationReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Underlying public family contract.
    pub family_contract: TassadarVerifierGuidedSearchTraceFamilyContract,
    /// Canonical repo-relative train-side run ref.
    pub trace_family_run_ref: String,
    /// Runtime traces carried by the lane.
    pub runtime_traces: Vec<TassadarVerifierGuidedSearchTraceArtifact>,
    /// Case-level metrics surfaced by the lane.
    pub case_reports: Vec<TassadarVerifierGuidedSearchEvaluationCaseReport>,
    /// Family-level aggregate summaries.
    pub family_summaries: Vec<TassadarVerifierGuidedSearchEvaluationFamilySummary>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// Verifier-guided search report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarVerifierGuidedSearchEvaluationReportError {
    /// Family contract validation failed.
    #[error(transparent)]
    Contract(#[from] TassadarVerifierGuidedSearchTraceFamilyError),
    /// Runtime trace generation or validation failed.
    #[error(transparent)]
    Runtime(#[from] TassadarVerifierGuidedSearchTraceError),
    /// Filesystem read/write failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed verifier-guided search evaluation report.
pub fn build_tassadar_verifier_guided_search_evaluation_report(
) -> Result<TassadarVerifierGuidedSearchEvaluationReport, TassadarVerifierGuidedSearchEvaluationReportError>
{
    let _guard = VERIFIER_GUIDED_SEARCH_REPORT_LOCK
        .lock()
        .expect("verifier-guided search report lock should not be poisoned");
    build_tassadar_verifier_guided_search_evaluation_report_impl()
}

fn build_tassadar_verifier_guided_search_evaluation_report_impl(
) -> Result<TassadarVerifierGuidedSearchEvaluationReport, TassadarVerifierGuidedSearchEvaluationReportError>
{
    let family_contract = tassadar_verifier_guided_search_trace_family_contract();
    family_contract.validate()?;
    let runtime_traces = tassadar_verifier_guided_search_trace_artifacts()?;
    let case_reports = runtime_traces
        .iter()
        .map(build_case_report)
        .collect::<Vec<_>>();
    let family_summaries = build_family_summaries(case_reports.as_slice());

    let mut report = TassadarVerifierGuidedSearchEvaluationReport {
        schema_version: TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.verifier_guided_search.report.v1"),
        family_contract,
        trace_family_run_ref: String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_REF),
        runtime_traces,
        case_reports,
        family_summaries,
        claim_boundary: String::from(
            "this report summarizes the research-only verifier-guided search trace lane with explicit guess, verifier, contradiction, and backtrack events over one real Sudoku-v0 case and one bounded search kernel. It does not imply compiled correctness, general solver closure, or served promotion",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_verifier_guided_search_evaluation_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed verifier-guided search report.
pub fn tassadar_verifier_guided_search_evaluation_report_path() -> PathBuf {
    repo_root().join(TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF)
}

/// Writes the committed verifier-guided search evaluation report.
pub fn write_tassadar_verifier_guided_search_evaluation_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarVerifierGuidedSearchEvaluationReport, TassadarVerifierGuidedSearchEvaluationReportError>
{
    let _guard = VERIFIER_GUIDED_SEARCH_REPORT_LOCK
        .lock()
        .expect("verifier-guided search report lock should not be poisoned");
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_verifier_guided_search_evaluation_report_impl()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

fn build_case_report(
    trace: &TassadarVerifierGuidedSearchTraceArtifact,
) -> TassadarVerifierGuidedSearchEvaluationCaseReport {
    let contradiction_count = trace
        .events
        .iter()
        .filter(|event| event.event_kind == TassadarVerifierGuidedSearchEventKind::Contradiction)
        .count() as u32;
    let verifier_certificate_accuracy_bps = if trace.events.iter().all(|event| match event.event_kind
    {
        TassadarVerifierGuidedSearchEventKind::Verify => event
            .certificate
            .as_ref()
            .is_some_and(|certificate| !certificate.contradiction_detected),
        TassadarVerifierGuidedSearchEventKind::Contradiction => event
            .certificate
            .as_ref()
            .is_some_and(|certificate| certificate.contradiction_detected),
        TassadarVerifierGuidedSearchEventKind::Commit => event.certificate.is_some(),
        TassadarVerifierGuidedSearchEventKind::Guess
        | TassadarVerifierGuidedSearchEventKind::Backtrack => true,
    }) {
        10_000
    } else {
        0
    };
    let backtrack_exactness_bps = if trace.events.windows(2).all(|window| {
        let previous = &window[0];
        let current = &window[1];
        if current.event_kind != TassadarVerifierGuidedSearchEventKind::Backtrack {
            return true;
        }
        previous.event_kind == TassadarVerifierGuidedSearchEventKind::Contradiction
            && current
                .backtrack_to_depth
                .is_some_and(|depth| depth < previous.depth)
            && current.depth
                == current
                    .backtrack_to_depth
                    .expect("backtrack event should carry target depth")
    }) {
        10_000
    } else {
        0
    };
    let recovery_quality_bps = if trace
        .events
        .iter()
        .any(|event| event.event_kind == TassadarVerifierGuidedSearchEventKind::Commit)
    {
        10_000
    } else {
        0
    };

    TassadarVerifierGuidedSearchEvaluationCaseReport {
        case_id: trace.case_id.clone(),
        workload_family: trace.workload_family,
        trace_id: trace.trace_id.clone(),
        trace_digest: trace.trace_digest.clone(),
        guess_count: trace.guess_count(),
        backtrack_count: trace.backtrack_count(),
        contradiction_count,
        verifier_certificate_accuracy_bps,
        backtrack_exactness_bps,
        recovery_quality_bps,
        claim_boundary: trace.claim_boundary.clone(),
    }
}

fn build_family_summaries(
    case_reports: &[TassadarVerifierGuidedSearchEvaluationCaseReport],
) -> Vec<TassadarVerifierGuidedSearchEvaluationFamilySummary> {
    let mut grouped =
        BTreeMap::<TassadarVerifierGuidedSearchWorkloadFamily, Vec<&TassadarVerifierGuidedSearchEvaluationCaseReport>>::new();
    for case in case_reports {
        grouped.entry(case.workload_family).or_default().push(case);
    }
    let mut summaries = grouped
        .into_iter()
        .map(|(workload_family, cases)| {
            let case_count = cases.len() as u32;
            let guess_sum = cases.iter().map(|case| case.guess_count).sum::<u32>();
            let backtrack_sum = cases.iter().map(|case| case.backtrack_count).sum::<u32>();
            let verifier_sum = cases
                .iter()
                .map(|case| case.verifier_certificate_accuracy_bps)
                .sum::<u32>();
            let backtrack_exactness_sum = cases
                .iter()
                .map(|case| case.backtrack_exactness_bps)
                .sum::<u32>();
            let recovery_sum = cases
                .iter()
                .map(|case| case.recovery_quality_bps)
                .sum::<u32>();
            TassadarVerifierGuidedSearchEvaluationFamilySummary {
                workload_family,
                case_count,
                mean_guess_count: guess_sum / case_count.max(1),
                mean_backtrack_count: backtrack_sum / case_count.max(1),
                verifier_certificate_accuracy_bps: verifier_sum / case_count.max(1),
                backtrack_exactness_bps: backtrack_exactness_sum / case_count.max(1),
                recovery_quality_bps: recovery_sum / case_count.max(1),
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by_key(|summary| summary.workload_family);
    summaries
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value)
        .expect("verifier-guided search evaluation report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        build_tassadar_verifier_guided_search_evaluation_report,
        tassadar_verifier_guided_search_evaluation_report_path,
        write_tassadar_verifier_guided_search_evaluation_report,
    };

    #[test]
    fn verifier_guided_search_evaluation_report_keeps_search_families_separate() {
        let report = build_tassadar_verifier_guided_search_evaluation_report()
            .expect("search evaluation report should build");
        assert_eq!(report.family_summaries.len(), 2);
        assert!(report
            .family_summaries
            .iter()
            .all(|summary| summary.verifier_certificate_accuracy_bps == 10_000));
    }

    #[test]
    fn verifier_guided_search_evaluation_report_matches_committed_truth() {
        let report = build_tassadar_verifier_guided_search_evaluation_report()
            .expect("search evaluation report should build");
        let committed = fs::read_to_string(tassadar_verifier_guided_search_evaluation_report_path())
            .expect("committed search evaluation report should exist");
        let committed_report =
            serde_json::from_str(&committed).expect("committed search evaluation report should parse");
        assert_eq!(report, committed_report);
    }

    #[test]
    fn write_verifier_guided_search_evaluation_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_verifier_guided_search_report.json");
        let report = write_tassadar_verifier_guided_search_evaluation_report(&output_path)
            .expect("writing search evaluation report should succeed");
        let written = fs::read_to_string(&output_path).expect("written report should exist");
        let reparsed =
            serde_json::from_str(&written).expect("written search evaluation report should parse");
        assert_eq!(report, reparsed);
        let _ = fs::remove_file(output_path);
    }
}
