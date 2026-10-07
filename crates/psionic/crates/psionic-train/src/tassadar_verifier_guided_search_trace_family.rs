use std::{fs, path::Path};

use psionic_data::{
    tassadar_verifier_guided_search_trace_family_contract,
    TassadarVerifierGuidedSearchTraceFamilyContract, TassadarVerifierGuidedSearchTraceFamilyError,
};
use psionic_runtime::{
    tassadar_verifier_guided_search_trace_artifacts, TassadarVerifierGuidedSearchEventKind,
    TassadarVerifierGuidedSearchTraceArtifact, TassadarVerifierGuidedSearchTraceError,
    TassadarVerifierGuidedSearchWorkloadFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable public family reference for the verifier-guided search trace lane.
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REF: &str =
    "trace-family://openagents/tassadar/verifier_guided_search";
/// Shared version used by the seeded verifier-guided search lane.
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_VERSION: &str = "2026.03.18";
/// Canonical output root for the verifier-guided search trace family run.
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_verifier_guided_search_trace_family_v1";
/// Canonical machine-readable report file for the verifier-guided search lane.
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_FILE: &str =
    "search_trace_family_report.json";
/// Canonical repo-relative report ref for the verifier-guided search lane.
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_REF: &str =
    "fixtures/tassadar/runs/tassadar_verifier_guided_search_trace_family_v1/search_trace_family_report.json";

/// One machine-readable seeded case report in the verifier-guided search lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchTraceCaseReport {
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

/// Top-level report for the seeded verifier-guided search trace lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchTraceFamilyReport {
    /// Shared family version.
    pub version: String,
    /// Underlying public family contract.
    pub family_contract: TassadarVerifierGuidedSearchTraceFamilyContract,
    /// Runtime artifacts surfaced by the lane.
    pub runtime_traces: Vec<TassadarVerifierGuidedSearchTraceArtifact>,
    /// Case-level training and replay metrics.
    pub case_reports: Vec<TassadarVerifierGuidedSearchTraceCaseReport>,
    /// Plain-language summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// Errors while materializing the verifier-guided search trace lane.
#[derive(Debug, Error)]
pub enum TassadarVerifierGuidedSearchTraceFamilyReportError {
    /// Family contract validation failed.
    #[error(transparent)]
    Contract(#[from] TassadarVerifierGuidedSearchTraceFamilyError),
    /// Runtime trace generation or validation failed.
    #[error(transparent)]
    Runtime(#[from] TassadarVerifierGuidedSearchTraceError),
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the report.
    #[error("failed to write verifier-guided search trace-family report `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the public verifier-guided search trace-family contract.
pub fn build_tassadar_verifier_guided_search_trace_family_contract(
) -> Result<TassadarVerifierGuidedSearchTraceFamilyContract, TassadarVerifierGuidedSearchTraceFamilyError>
{
    let contract = tassadar_verifier_guided_search_trace_family_contract();
    debug_assert_eq!(
        contract.family_ref,
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REF
    );
    debug_assert_eq!(
        contract.version,
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_VERSION
    );
    debug_assert_eq!(
        contract.report_ref,
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_REF
    );
    Ok(contract)
}

/// Executes the seeded verifier-guided search trace-family lane and writes the report.
pub fn execute_tassadar_verifier_guided_search_trace_family(
    output_dir: &Path,
) -> Result<TassadarVerifierGuidedSearchTraceFamilyReport, TassadarVerifierGuidedSearchTraceFamilyReportError>
{
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarVerifierGuidedSearchTraceFamilyReportError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let family_contract = build_tassadar_verifier_guided_search_trace_family_contract()?;
    let runtime_traces = tassadar_verifier_guided_search_trace_artifacts()?;
    let case_reports = runtime_traces
        .iter()
        .map(build_case_report)
        .collect::<Vec<_>>();
    let mut report = TassadarVerifierGuidedSearchTraceFamilyReport {
        version: String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_VERSION),
        family_contract,
        runtime_traces,
        case_reports,
        summary: String::from(
            "Verifier-guided search trace family now freezes one real Sudoku-v0 backtracking trace and one bounded search-kernel trace with explicit guess, verifier, contradiction, and backtrack events. The lane stays research-only and separates verifier-guided search evidence from the deterministic compiled executor trace",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_verifier_guided_search_trace_family_report|",
        &report,
    );

    let output_path = output_dir.join(TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_FILE);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarVerifierGuidedSearchTraceFamilyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_case_report(
    trace: &TassadarVerifierGuidedSearchTraceArtifact,
) -> TassadarVerifierGuidedSearchTraceCaseReport {
    let contradiction_count = trace
        .events
        .iter()
        .filter(|event| event.event_kind == TassadarVerifierGuidedSearchEventKind::Contradiction)
        .count() as u32;
    let verifier_certificate_accuracy_bps = if trace.events.iter().all(|event| {
        match event.event_kind {
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
        }
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

    TassadarVerifierGuidedSearchTraceCaseReport {
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

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("verifier-guided search trace-family value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::{
        build_tassadar_verifier_guided_search_trace_family_contract,
        execute_tassadar_verifier_guided_search_trace_family,
        TassadarVerifierGuidedSearchTraceFamilyReport,
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_OUTPUT_DIR,
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_REF,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    #[test]
    fn verifier_guided_search_trace_family_contract_is_machine_legible() {
        let contract = build_tassadar_verifier_guided_search_trace_family_contract()
            .expect("search trace-family contract should build");
        assert_eq!(contract.cases.len(), 2);
        assert_eq!(contract.event_kinds.len(), 5);
        assert!(!contract.stable_digest().is_empty());
    }

    #[test]
    fn verifier_guided_search_trace_family_report_preserves_recovery_metrics() {
        let output_dir = tempdir().expect("temp dir");
        let report = execute_tassadar_verifier_guided_search_trace_family(output_dir.path())
            .expect("search trace-family report should build");
        assert_eq!(report.case_reports.len(), 2);
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.verifier_certificate_accuracy_bps == 10_000));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.recovery_quality_bps == 10_000));
        assert!(TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_OUTPUT_DIR.contains("search_trace_family"));
    }

    #[test]
    fn verifier_guided_search_trace_family_report_matches_committed_truth() {
        let output_dir = tempdir().expect("temp dir");
        let report = execute_tassadar_verifier_guided_search_trace_family(output_dir.path())
            .expect("search trace-family report should build");
        let committed = fs::read_to_string(
            repo_root().join(TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_REF),
        )
        .expect("committed search trace-family report should exist");
        let committed_report: TassadarVerifierGuidedSearchTraceFamilyReport =
            serde_json::from_str(&committed)
                .expect("committed search trace-family report should parse");
        assert_eq!(report, committed_report);
    }
}
