use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{
    compile_tassadar_locality_preserving_scratchpad_cases, TassadarLocalityScratchpadCompileError,
};
use psionic_ir::TassadarLocalityScratchpadTraceFamily;
use psionic_models::{tassadar_locality_scratchpad_publication, TassadarLocalityScratchpadPublication};
use psionic_runtime::{
    build_tassadar_locality_scratchpad_replay_receipt, TassadarLocalityScratchpadReplayError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_LOCALITY_SCRATCHPAD_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json";
pub const TASSADAR_LOCALITY_SCRATCHPAD_SUITE_RUN_REF: &str =
    "fixtures/tassadar/runs/tassadar_locality_scratchpad_suite_v1/locality_scratchpad_suite.json";

const REPORT_SCHEMA_VERSION: u16 = 1;

/// One same-program baseline-vs-candidate case report in the locality scratchpad eval report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityScratchpadCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Trace family covered by the case.
    pub trace_family: TassadarLocalityScratchpadTraceFamily,
    /// Stable source trace reference.
    pub source_trace_ref: String,
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

/// Family-level aggregate summary in the locality scratchpad eval report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityScratchpadFamilySummary {
    /// Trace family covered by the summary.
    pub trace_family: TassadarLocalityScratchpadTraceFamily,
    /// Number of seeded cases.
    pub case_count: u32,
    /// Mean locality gain across the family.
    pub mean_locality_gain_bps: u32,
    /// Maximum scratchpad overhead across the family.
    pub max_scratchpad_overhead_bps: u32,
    /// Whether all cases kept replay exact.
    pub all_cases_replay_exact: bool,
}

/// Committed eval report over the locality-preserving scratchpad lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityScratchpadReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Canonical train-side suite run ref.
    pub suite_run_ref: String,
    /// Public publication for the lane.
    pub publication: TassadarLocalityScratchpadPublication,
    /// Ordered seeded-case results.
    pub case_reports: Vec<TassadarLocalityScratchpadCaseReport>,
    /// Family-level aggregate summaries.
    pub family_summaries: Vec<TassadarLocalityScratchpadFamilySummary>,
    /// Explicit claim boundary for the current lane.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Report build failures for the locality scratchpad lane.
#[derive(Debug, Error)]
pub enum TassadarLocalityScratchpadReportError {
    /// Compiler pass materialization failed.
    #[error(transparent)]
    Compiler(#[from] TassadarLocalityScratchpadCompileError),
    /// Runtime replay receipt construction failed.
    #[error(transparent)]
    Runtime(#[from] TassadarLocalityScratchpadReplayError),
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the committed report.
    #[error("failed to write locality scratchpad report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read a committed report.
    #[error("failed to read committed locality scratchpad report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode a committed report.
    #[error("failed to decode committed locality scratchpad report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed report for the locality-preserving scratchpad lane.
pub fn build_tassadar_locality_scratchpad_report(
) -> Result<TassadarLocalityScratchpadReport, TassadarLocalityScratchpadReportError> {
    let publication = tassadar_locality_scratchpad_publication();
    let compilations = compile_tassadar_locality_preserving_scratchpad_cases()?;
    let case_reports = compilations
        .iter()
        .map(|compilation| {
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
        })
        .collect::<Result<Vec<_>, TassadarLocalityScratchpadReportError>>()?;
    let family_summaries = build_family_summaries(case_reports.as_slice());
    let mut report = TassadarLocalityScratchpadReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.locality_scratchpad.report.v1"),
        suite_run_ref: String::from(TASSADAR_LOCALITY_SCRATCHPAD_SUITE_RUN_REF),
        publication,
        case_reports,
        family_summaries,
        claim_boundary: String::from(
            "this report compares same-program baseline-vs-scratchpad trace formatting on the seeded symbolic and module-trace-v2 families only; it proves replay-preserving locality and cost facts for the bounded pass, and does not imply semantic program rewrites, arbitrary long-horizon learned exactness, or served promotion",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(b"psionic_tassadar_locality_scratchpad_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_locality_scratchpad_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LOCALITY_SCRATCHPAD_REPORT_REF)
}

/// Writes the committed report for the locality-preserving scratchpad lane.
pub fn write_tassadar_locality_scratchpad_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLocalityScratchpadReport, TassadarLocalityScratchpadReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarLocalityScratchpadReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_locality_scratchpad_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarLocalityScratchpadReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_family_summaries(
    case_reports: &[TassadarLocalityScratchpadCaseReport],
) -> Vec<TassadarLocalityScratchpadFamilySummary> {
    let mut grouped =
        BTreeMap::<TassadarLocalityScratchpadTraceFamily, Vec<&TassadarLocalityScratchpadCaseReport>>::new();
    for case in case_reports {
        grouped.entry(case.trace_family).or_default().push(case);
    }
    let mut summaries = grouped
        .into_iter()
        .map(|(trace_family, cases)| TassadarLocalityScratchpadFamilySummary {
            trace_family,
            case_count: cases.len() as u32,
            mean_locality_gain_bps: if cases.is_empty() {
                0
            } else {
                cases
                    .iter()
                    .map(|case| u64::from(case.locality_gain_bps))
                    .sum::<u64>() as u32
                    / cases.len() as u32
            },
            max_scratchpad_overhead_bps: cases
                .iter()
                .map(|case| case.scratchpad_overhead_bps)
                .max()
                .unwrap_or(0),
            all_cases_replay_exact: cases.iter().all(|case| case.replay_exact),
        })
        .collect::<Vec<_>>();
    summaries.sort_by_key(|summary| summary.trace_family);
    summaries
}

fn locality_gain_bps(baseline: u32, candidate: u32) -> u32 {
    if baseline == 0 || candidate >= baseline {
        0
    } else {
        (((u64::from(baseline - candidate)) * 10_000) / u64::from(baseline)) as u32
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_locality_scratchpad_report, tassadar_locality_scratchpad_report_path,
        write_tassadar_locality_scratchpad_report,
    };

    #[test]
    fn locality_scratchpad_report_captures_symbolic_and_module_families() {
        let report = build_tassadar_locality_scratchpad_report().expect("report");
        assert_eq!(report.family_summaries.len(), 2);
        assert!(report
            .family_summaries
            .iter()
            .all(|summary| summary.all_cases_replay_exact));
        assert!(report
            .family_summaries
            .iter()
            .all(|summary| summary.mean_locality_gain_bps > 0));
    }

    #[test]
    fn locality_scratchpad_report_matches_committed_truth() {
        let generated = build_tassadar_locality_scratchpad_report().expect("report");
        let committed = std::fs::read_to_string(tassadar_locality_scratchpad_report_path())
            .expect("committed report");
        let committed_report = serde_json::from_str(&committed).expect("decode report");
        assert_eq!(generated, committed_report);
    }

    #[test]
    fn write_locality_scratchpad_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_locality_scratchpad_report.json");
        let generated = write_tassadar_locality_scratchpad_report(&output_path).expect("report");
        let written = std::fs::read_to_string(&output_path).expect("read written");
        let reparsed = serde_json::from_str(&written).expect("decode written report");
        assert_eq!(generated, reparsed);
        let _ = std::fs::remove_file(output_path);
    }
}
