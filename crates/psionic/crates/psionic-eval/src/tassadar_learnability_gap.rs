use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_LEARNABILITY_GAP_EVIDENCE_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_learnability_gap_v1/learnability_gap_evidence_bundle.json";
pub const TASSADAR_LEARNABILITY_GAP_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_learnability_gap_report.json";
const REPORT_SCHEMA_VERSION: u16 = 1;

/// Stable workload family audited by the learnability-gap bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnabilityGapWorkloadFamily {
    KernelArithmetic,
    SudokuSearch,
    HungarianMatching,
    ClrsWasmShortestPath,
}

/// Plausible learnability failure class surfaced by the bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnabilityGapClass {
    PositionalSchemeMiss,
    SupervisionMiss,
    TraceFormatMiss,
    ArchitectureCapacityLimit,
}

/// Persisted train-side case record consumed by eval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnabilityGapEvidenceCase {
    pub case_id: String,
    pub workload_family: TassadarLearnabilityGapWorkloadFamily,
    pub baseline_metric_bps: u32,
    pub improved_metric_bps: u32,
    pub primary_gap: TassadarLearnabilityGapClass,
    pub evidence_refs: Vec<String>,
    pub detail: String,
}

/// Persisted train-side evidence bundle consumed by eval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnabilityGapEvidenceBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub cases: Vec<TassadarLearnabilityGapEvidenceCase>,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

/// One eval-side learnability-gap case report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnabilityGapCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Workload family being audited.
    pub workload_family: TassadarLearnabilityGapWorkloadFamily,
    /// Primary gap class for the case.
    pub primary_gap: TassadarLearnabilityGapClass,
    /// Improved minus baseline metric delta.
    pub gap_delta_bps: i32,
    /// Whether the delta is large enough to treat as material.
    pub materially_actionable: bool,
    /// Whether the case points to an explicit refusal-boundary or narrowing story.
    pub refusal_visible: bool,
    /// Stable evidence refs anchoring the case.
    pub evidence_refs: Vec<String>,
    /// Plain-language detail.
    pub detail: String,
}

/// Aggregate summary for one primary gap class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnabilityGapSummaryRow {
    /// Primary gap class.
    pub gap_class: TassadarLearnabilityGapClass,
    /// Number of cases assigned to the gap class.
    pub case_count: u32,
    /// Mean improved-vs-baseline delta.
    pub mean_gap_delta_bps: i32,
    /// Whether every case in the class is materially actionable.
    pub all_materially_actionable: bool,
}

/// Committed eval report for learnability-gap analysis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnabilityGapReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Train-side evidence bundle.
    pub evidence_bundle: TassadarLearnabilityGapEvidenceBundle,
    /// Ordered eval-side case reports.
    pub case_reports: Vec<TassadarLearnabilityGapCaseReport>,
    /// Aggregate summary rows.
    pub summary_rows: Vec<TassadarLearnabilityGapSummaryRow>,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Learnability-gap eval report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarLearnabilityGapReportError {
    /// Failed to read the persisted train bundle.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode the persisted train bundle.
    #[error("failed to decode learnability-gap evidence bundle from `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write learnability-gap report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed learnability-gap eval report.
pub fn build_tassadar_learnability_gap_report(
) -> Result<TassadarLearnabilityGapReport, TassadarLearnabilityGapReportError> {
    let evidence_bundle: TassadarLearnabilityGapEvidenceBundle =
        read_repo_json(TASSADAR_LEARNABILITY_GAP_EVIDENCE_BUNDLE_REF)?;
    let case_reports = evidence_bundle
        .cases
        .iter()
        .map(build_case_report)
        .collect::<Vec<_>>();
    let summary_rows = build_summary_rows(case_reports.as_slice());
    let mut report = TassadarLearnabilityGapReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.learnability_gap.report.v1"),
        evidence_bundle,
        case_reports,
        summary_rows,
        claim_boundary: String::from(
            "this report classifies bounded learnability gaps across seeded kernel, Sudoku, Hungarian, and CLRS-to-Wasm families only; it records plausible failure classes and actionability from existing evidence surfaces without promoting any learned executor family or claiming that the underlying gap is closed",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(b"psionic_tassadar_learnability_gap_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_learnability_gap_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LEARNABILITY_GAP_REPORT_REF)
}

/// Writes the committed learnability-gap eval report.
pub fn write_tassadar_learnability_gap_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLearnabilityGapReport, TassadarLearnabilityGapReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarLearnabilityGapReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_learnability_gap_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarLearnabilityGapReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_case_report(
    case: &TassadarLearnabilityGapEvidenceCase,
) -> TassadarLearnabilityGapCaseReport {
    let gap_delta_bps = case.improved_metric_bps as i32 - case.baseline_metric_bps as i32;
    TassadarLearnabilityGapCaseReport {
        case_id: case.case_id.clone(),
        workload_family: case.workload_family,
        primary_gap: case.primary_gap,
        gap_delta_bps,
        materially_actionable: gap_delta_bps.abs() >= 500,
        refusal_visible: case
            .evidence_refs
            .iter()
            .any(|reference: &String| reference.contains("report")),
        evidence_refs: case.evidence_refs.clone(),
        detail: case.detail.clone(),
    }
}

fn build_summary_rows(
    case_reports: &[TassadarLearnabilityGapCaseReport],
) -> Vec<TassadarLearnabilityGapSummaryRow> {
    let mut grouped =
        BTreeMap::<TassadarLearnabilityGapClass, Vec<&TassadarLearnabilityGapCaseReport>>::new();
    for report in case_reports {
        grouped.entry(report.primary_gap).or_default().push(report);
    }
    let mut rows = grouped
        .into_iter()
        .map(|(gap_class, reports)| TassadarLearnabilityGapSummaryRow {
            gap_class,
            case_count: reports.len() as u32,
            mean_gap_delta_bps: if reports.is_empty() {
                0
            } else {
                reports
                    .iter()
                    .map(|report| i64::from(report.gap_delta_bps))
                    .sum::<i64>() as i32
                    / reports.len() as i32
            },
            all_materially_actionable: reports.iter().all(|report| report.materially_actionable),
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.gap_class);
    rows
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    repo_relative_path: &str,
) -> Result<T, TassadarLearnabilityGapReportError> {
    let path = repo_root().join(repo_relative_path);
    let bytes = std::fs::read(&path).map_err(|error| TassadarLearnabilityGapReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarLearnabilityGapReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

#[cfg(test)]
mod tests {
    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        build_tassadar_learnability_gap_report, tassadar_learnability_gap_report_path,
        write_tassadar_learnability_gap_report, TassadarLearnabilityGapClass,
        TassadarLearnabilityGapReport, TASSADAR_LEARNABILITY_GAP_REPORT_REF,
    };

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
        let bytes = std::fs::read(&path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn learnability_gap_report_surfaces_material_gap_classes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_learnability_gap_report()?;
        assert_eq!(report.case_reports.len(), 4);
        assert!(report
            .summary_rows
            .iter()
            .any(|row| row.gap_class == TassadarLearnabilityGapClass::SupervisionMiss));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.materially_actionable && case.refusal_visible));
        Ok(())
    }

    #[test]
    fn learnability_gap_report_matches_committed_truth() -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_learnability_gap_report()?;
        let committed: TassadarLearnabilityGapReport =
            read_repo_json(TASSADAR_LEARNABILITY_GAP_REPORT_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_learnability_gap_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let output_path = temp_dir
            .path()
            .join("tassadar_learnability_gap_report.json");
        let written = write_tassadar_learnability_gap_report(&output_path)?;
        let bytes = std::fs::read(&output_path)?;
        let roundtrip: TassadarLearnabilityGapReport = serde_json::from_slice(&bytes)?;
        assert_eq!(written, roundtrip);
        assert_eq!(
            tassadar_learnability_gap_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_learnability_gap_report.json")
        );
        Ok(())
    }
}
