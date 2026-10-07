use std::{fs, path::Path};

use psionic_compiler::{
    compile_tassadar_locality_preserving_scratchpad_cases, TassadarLocalityScratchpadCompilation,
    TassadarLocalityScratchpadCompileError,
};
use psionic_ir::TassadarLocalityScratchpadTraceFamily;
use psionic_models::{tassadar_locality_scratchpad_publication, TassadarLocalityScratchpadPublication};
use psionic_runtime::{
    build_tassadar_locality_scratchpad_replay_receipt, TassadarLocalityScratchpadReplayError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical output root for the locality-preserving scratchpad suite run.
pub const TASSADAR_LOCALITY_SCRATCHPAD_SUITE_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_locality_scratchpad_suite_v1";
/// Canonical machine-readable report file for the locality-preserving scratchpad suite.
pub const TASSADAR_LOCALITY_SCRATCHPAD_SUITE_REPORT_FILE: &str =
    "locality_scratchpad_suite.json";
/// Canonical repo-relative report ref for the locality-preserving scratchpad suite.
pub const TASSADAR_LOCALITY_SCRATCHPAD_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/runs/tassadar_locality_scratchpad_suite_v1/locality_scratchpad_suite.json";

/// One same-program baseline-vs-candidate case report in the locality scratchpad suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityScratchpadCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Trace family covered by the case.
    pub trace_family: TassadarLocalityScratchpadTraceFamily,
    /// Stable source trace reference.
    pub source_trace_ref: String,
    /// Stable source trace digest.
    pub source_trace_digest: String,
    /// Stable pass identifier.
    pub pass_id: String,
    /// Baseline max useful lookback.
    pub baseline_max_useful_lookback: u32,
    /// Candidate max useful lookback.
    pub candidate_max_useful_lookback: u32,
    /// Lookback reduction in basis points against the baseline.
    pub locality_gain_bps: u32,
    /// Baseline token count.
    pub baseline_token_count: u32,
    /// Candidate token count.
    pub candidate_token_count: u32,
    /// Scratchpad overhead in basis points.
    pub scratchpad_overhead_bps: u32,
    /// Whether replay stayed exact.
    pub replay_exact: bool,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// Top-level suite report for the locality-preserving scratchpad lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityScratchpadSuiteReport {
    /// Public publication for the lane.
    pub publication: TassadarLocalityScratchpadPublication,
    /// Ordered same-program case reports.
    pub case_reports: Vec<TassadarLocalityScratchpadCaseReport>,
    /// Plain-language summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// Errors while materializing the locality-preserving scratchpad suite.
#[derive(Debug, Error)]
pub enum TassadarLocalityScratchpadSuiteError {
    /// Compiler pass materialization failed.
    #[error(transparent)]
    Compiler(#[from] TassadarLocalityScratchpadCompileError),
    /// Runtime replay receipt construction failed.
    #[error(transparent)]
    Runtime(#[from] TassadarLocalityScratchpadReplayError),
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the report.
    #[error("failed to write locality scratchpad suite report `{path}`: {error}")]
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

/// Executes the seeded locality-preserving scratchpad suite and writes the report.
pub fn execute_tassadar_locality_scratchpad_suite(
    output_dir: &Path,
) -> Result<TassadarLocalityScratchpadSuiteReport, TassadarLocalityScratchpadSuiteError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarLocalityScratchpadSuiteError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let publication = tassadar_locality_scratchpad_publication();
    let compilations = compile_tassadar_locality_preserving_scratchpad_cases()?;
    let case_reports = compilations
        .iter()
        .map(build_case_report)
        .collect::<Result<Vec<_>, _>>()?;
    let symbolic_case_count = case_reports
        .iter()
        .filter(|case| case.trace_family == TassadarLocalityScratchpadTraceFamily::SymbolicStraightLine)
        .count();
    let module_case_count = case_reports
        .iter()
        .filter(|case| case.trace_family == TassadarLocalityScratchpadTraceFamily::ModuleTraceV2)
        .count();
    let mean_locality_gain_bps = if case_reports.is_empty() {
        0
    } else {
        case_reports
            .iter()
            .map(|case| u64::from(case.locality_gain_bps))
            .sum::<u64>() as u32
            / case_reports.len() as u32
    };
    let mut report = TassadarLocalityScratchpadSuiteReport {
        publication,
        case_reports,
        summary: format!(
            "Locality-preserving scratchpad suite now freezes {} symbolic and {} module-trace same-program comparisons with mean lookback reduction={}bps while keeping replay exact and scratchpad overhead explicit.",
            symbolic_case_count,
            module_case_count,
            mean_locality_gain_bps,
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_locality_scratchpad_suite_report|",
        &report,
    );

    let output_path = output_dir.join(TASSADAR_LOCALITY_SCRATCHPAD_SUITE_REPORT_FILE);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarLocalityScratchpadSuiteError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_case_report(
    compilation: &TassadarLocalityScratchpadCompilation,
) -> Result<TassadarLocalityScratchpadCaseReport, TassadarLocalityScratchpadSuiteError> {
    let receipt = build_tassadar_locality_scratchpad_replay_receipt(
        format!("tassadar.locality_scratchpad.receipt.{}", compilation.case_id),
        compilation.case_id.clone(),
        compilation.trace_family,
        compilation.source_trace_ref.clone(),
        compilation.source_trace_digest.clone(),
        compilation.pass.pass_id.clone(),
        &compilation.baseline_sequence,
        &compilation.candidate_sequence,
        compilation.pass.claim_boundary.clone(),
    )?;
    Ok(TassadarLocalityScratchpadCaseReport {
        case_id: compilation.case_id.clone(),
        trace_family: compilation.trace_family,
        source_trace_ref: compilation.source_trace_ref.clone(),
        source_trace_digest: compilation.source_trace_digest.clone(),
        pass_id: compilation.pass.pass_id.clone(),
        baseline_max_useful_lookback: receipt.baseline_max_useful_lookback,
        candidate_max_useful_lookback: receipt.candidate_max_useful_lookback,
        locality_gain_bps: locality_gain_bps(
            receipt.baseline_max_useful_lookback,
            receipt.candidate_max_useful_lookback,
        ),
        baseline_token_count: receipt.baseline_token_count,
        candidate_token_count: receipt.candidate_token_count,
        scratchpad_overhead_bps: receipt.scratchpad_overhead_bps,
        replay_exact: receipt.replay_exact,
        claim_boundary: receipt.claim_boundary,
    })
}

fn locality_gain_bps(baseline: u32, candidate: u32) -> u32 {
    if baseline == 0 || candidate >= baseline {
        0
    } else {
        (((u64::from(baseline - candidate)) * 10_000) / u64::from(baseline)) as u32
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("locality scratchpad suite report should serialize");
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
        execute_tassadar_locality_scratchpad_suite, TassadarLocalityScratchpadSuiteReport,
        TASSADAR_LOCALITY_SCRATCHPAD_SUITE_OUTPUT_DIR,
        TASSADAR_LOCALITY_SCRATCHPAD_SUITE_REPORT_REF,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    #[test]
    fn locality_scratchpad_suite_reduces_lookback_without_changing_truth() {
        let output_dir = tempdir().expect("temp dir");
        let report = execute_tassadar_locality_scratchpad_suite(output_dir.path())
            .expect("suite report should build");
        assert!(report.case_reports.iter().all(|case| case.replay_exact));
        assert!(report.case_reports.iter().all(|case| case.locality_gain_bps > 0));
        assert!(TASSADAR_LOCALITY_SCRATCHPAD_SUITE_OUTPUT_DIR.contains("locality_scratchpad"));
    }

    #[test]
    fn locality_scratchpad_suite_matches_committed_truth() {
        let output_dir = tempdir().expect("temp dir");
        let report = execute_tassadar_locality_scratchpad_suite(output_dir.path())
            .expect("suite report should build");
        let committed = fs::read_to_string(
            repo_root().join(TASSADAR_LOCALITY_SCRATCHPAD_SUITE_REPORT_REF),
        )
        .expect("committed locality scratchpad suite should exist");
        let committed_report: TassadarLocalityScratchpadSuiteReport =
            serde_json::from_str(&committed)
                .expect("committed locality scratchpad suite should parse");
        assert_eq!(report, committed_report);
    }
}
