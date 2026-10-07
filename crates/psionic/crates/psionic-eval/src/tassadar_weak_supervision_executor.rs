use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    TassadarWeakSupervisionEvidenceBundle, TassadarWeakSupervisionRegime,
    TassadarWeakSupervisionWorkloadFamily, TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF,
    TASSADAR_WEAK_SUPERVISION_REPORT_REF,
};
use psionic_models::tassadar_weak_supervision_publication;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Regime-level summary in the weak-supervision eval report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWeakSupervisionRegimeSummary {
    /// Compared supervision regime.
    pub supervision_regime: TassadarWeakSupervisionRegime,
    /// Mean later-window exactness.
    pub mean_later_window_exactness_bps: u32,
    /// Mean final-output exactness.
    pub mean_final_output_exactness_bps: u32,
    /// Mean refusal calibration.
    pub mean_refusal_calibration_bps: u32,
    /// Mean count of under-supervised failures on the seeded workload slice.
    pub mean_under_supervised_failure_count: u32,
    /// Dominant failure modes on the seeded workloads.
    pub dominant_failure_modes: Vec<String>,
}

/// Workload-level summary in the weak-supervision eval report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWeakSupervisionWorkloadSummary {
    /// Compared workload family.
    pub workload_family: TassadarWeakSupervisionWorkloadFamily,
    /// Best regime on the seeded comparison.
    pub best_regime: TassadarWeakSupervisionRegime,
    /// Later-window gap between mixed and full trace.
    pub mixed_gap_vs_full_trace_bps: i32,
    /// Mixed-regime refusal calibration used by viability thresholds.
    pub mixed_refusal_calibration_bps: u32,
    /// Later-window gap between io-only and full trace.
    pub io_only_gap_vs_full_trace_bps: i32,
    /// Io-only later-window exactness used by fragility thresholds.
    pub io_only_later_window_exactness_bps: u32,
    /// Refusal-calibration gap between io-only and full trace.
    pub io_only_refusal_gap_vs_full_trace_bps: i32,
    /// Plain-language note.
    pub note: String,
}

/// Eval-side report for the weak-supervision executor family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWeakSupervisionExecutorReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Model publication digest consumed by the report.
    pub publication_digest: String,
    /// Source contract ref.
    pub contract_ref: String,
    /// Source contract digest.
    pub contract_digest: String,
    /// Evidence bundle ref.
    pub evidence_bundle_ref: String,
    /// Evidence bundle digest.
    pub evidence_bundle_digest: String,
    /// Regime summaries.
    pub regime_summaries: Vec<TassadarWeakSupervisionRegimeSummary>,
    /// Workload summaries.
    pub workload_summaries: Vec<TassadarWeakSupervisionWorkloadSummary>,
    /// Workloads where mixed supervision recovers most of full-trace behavior.
    pub mixed_viable_workloads: Vec<TassadarWeakSupervisionWorkloadFamily>,
    /// Workloads where io-only remains fragile.
    pub io_only_fragile_workloads: Vec<TassadarWeakSupervisionWorkloadFamily>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Eval failure while building or writing the weak-supervision report.
#[derive(Debug, Error)]
pub enum TassadarWeakSupervisionExecutorReportError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed weak-supervision report.
pub fn build_tassadar_weak_supervision_executor_report(
) -> Result<TassadarWeakSupervisionExecutorReport, TassadarWeakSupervisionExecutorReportError> {
    let publication = tassadar_weak_supervision_publication();
    let evidence_bundle: TassadarWeakSupervisionEvidenceBundle =
        read_repo_json(TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF)?;
    let regime_summaries = build_regime_summaries(evidence_bundle.case_reports.as_slice());
    let workload_summaries = build_workload_summaries(evidence_bundle.case_reports.as_slice());
    let mixed_viable_workloads = workload_summaries
        .iter()
        .filter(|summary| summary.mixed_gap_vs_full_trace_bps >= -800)
        .filter(|summary| summary.mixed_refusal_calibration_bps >= 8_000)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let io_only_fragile_workloads = workload_summaries
        .iter()
        .filter(|summary| summary.io_only_later_window_exactness_bps < 5_000)
        .chain(
            workload_summaries
                .iter()
                .filter(|summary| summary.io_only_refusal_gap_vs_full_trace_bps < -2_000),
        )
        .map(|summary| summary.workload_family)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut report = TassadarWeakSupervisionExecutorReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.weak_supervision.report.v1"),
        publication_digest: publication.publication_digest.clone(),
        contract_ref: evidence_bundle.contract.contract_ref.clone(),
        contract_digest: evidence_bundle.contract.contract_digest.clone(),
        evidence_bundle_ref: String::from(TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF),
        evidence_bundle_digest: evidence_bundle.report_digest.clone(),
        regime_summaries,
        workload_summaries,
        mixed_viable_workloads,
        io_only_fragile_workloads,
        claim_boundary: String::from(
            "this report compares full-trace, mixed-weak, and io-only supervision on the seeded module-scale workloads. It keeps later-window exactness, refusal calibration, and under-supervised failure modes explicit instead of treating weaker supervision as broad learned closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Weak-supervision report now compares {} workloads across {} regimes, with {} mixed-viable workloads and {} io-only fragile workloads.",
        report.workload_summaries.len(),
        report.regime_summaries.len(),
        report.mixed_viable_workloads.len(),
        report.io_only_fragile_workloads.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_weak_supervision_executor_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_weak_supervision_executor_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WEAK_SUPERVISION_REPORT_REF)
}

/// Writes the committed weak-supervision report.
pub fn write_tassadar_weak_supervision_executor_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWeakSupervisionExecutorReport, TassadarWeakSupervisionExecutorReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWeakSupervisionExecutorReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_weak_supervision_executor_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarWeakSupervisionExecutorReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_regime_summaries(
    cases: &[psionic_data::TassadarWeakSupervisionEvidenceCase],
) -> Vec<TassadarWeakSupervisionRegimeSummary> {
    let mut grouped = BTreeMap::<
        TassadarWeakSupervisionRegime,
        Vec<&psionic_data::TassadarWeakSupervisionEvidenceCase>,
    >::new();
    for case in cases {
        grouped
            .entry(case.supervision_regime)
            .or_default()
            .push(case);
    }
    grouped
        .into_iter()
        .map(
            |(supervision_regime, cases)| TassadarWeakSupervisionRegimeSummary {
                supervision_regime,
                mean_later_window_exactness_bps: mean(
                    cases.iter().map(|case| case.later_window_exactness_bps),
                ),
                mean_final_output_exactness_bps: mean(
                    cases.iter().map(|case| case.final_output_exactness_bps),
                ),
                mean_refusal_calibration_bps: mean(
                    cases.iter().map(|case| case.refusal_calibration_bps),
                ),
                mean_under_supervised_failure_count: mean(
                    cases.iter().map(|case| case.under_supervised_failure_count),
                ),
                dominant_failure_modes: cases
                    .iter()
                    .map(|case| case.dominant_failure_mode.clone())
                    .collect(),
            },
        )
        .collect()
}

fn build_workload_summaries(
    cases: &[psionic_data::TassadarWeakSupervisionEvidenceCase],
) -> Vec<TassadarWeakSupervisionWorkloadSummary> {
    let mut grouped = BTreeMap::<
        TassadarWeakSupervisionWorkloadFamily,
        Vec<&psionic_data::TassadarWeakSupervisionEvidenceCase>,
    >::new();
    for case in cases {
        grouped.entry(case.workload_family).or_default().push(case);
    }
    grouped
        .into_iter()
        .map(|(workload_family, cases)| {
            let full = case_for_regime(cases.as_slice(), TassadarWeakSupervisionRegime::FullTrace);
            let mixed = case_for_regime(cases.as_slice(), TassadarWeakSupervisionRegime::MixedWeak);
            let io_only = case_for_regime(cases.as_slice(), TassadarWeakSupervisionRegime::IoOnly);
            let best_regime = cases
                .iter()
                .max_by_key(|case| case.later_window_exactness_bps)
                .map(|case| case.supervision_regime)
                .expect("workload group should not be empty");
            TassadarWeakSupervisionWorkloadSummary {
                workload_family,
                best_regime,
                mixed_gap_vs_full_trace_bps: mixed.later_window_exactness_bps as i32
                    - full.later_window_exactness_bps as i32,
                mixed_refusal_calibration_bps: mixed.refusal_calibration_bps,
                io_only_gap_vs_full_trace_bps: io_only.later_window_exactness_bps as i32
                    - full.later_window_exactness_bps as i32,
                io_only_later_window_exactness_bps: io_only.later_window_exactness_bps,
                io_only_refusal_gap_vs_full_trace_bps: io_only.refusal_calibration_bps as i32
                    - full.refusal_calibration_bps as i32,
                note: format!(
                    "{} currently prefers {} under the seeded weak-supervision comparison.",
                    workload_family.as_str(),
                    best_regime.as_str()
                ),
            }
        })
        .collect()
}

fn case_for_regime<'a>(
    cases: &[&'a psionic_data::TassadarWeakSupervisionEvidenceCase],
    regime: TassadarWeakSupervisionRegime,
) -> &'a psionic_data::TassadarWeakSupervisionEvidenceCase {
    cases
        .iter()
        .copied()
        .find(|case| case.supervision_regime == regime)
        .expect("every workload should surface every regime")
}

fn mean(values: impl IntoIterator<Item = u32>) -> u32 {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    values.iter().map(|value| u64::from(*value)).sum::<u64>() as u32 / values.len() as u32
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarWeakSupervisionExecutorReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarWeakSupervisionExecutorReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarWeakSupervisionExecutorReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_weak_supervision_executor_report, read_repo_json,
        tassadar_weak_supervision_executor_report_path,
        write_tassadar_weak_supervision_executor_report, TassadarWeakSupervisionExecutorReport,
    };
    use psionic_data::{
        TassadarWeakSupervisionRegime, TassadarWeakSupervisionWorkloadFamily,
        TASSADAR_WEAK_SUPERVISION_REPORT_REF,
    };

    #[test]
    fn weak_supervision_report_marks_mixed_viability_and_io_only_fragility() {
        let report =
            build_tassadar_weak_supervision_executor_report().expect("weak-supervision report");

        assert!(report
            .mixed_viable_workloads
            .contains(&TassadarWeakSupervisionWorkloadFamily::ModuleTraceV2));
        assert!(report
            .io_only_fragile_workloads
            .contains(&TassadarWeakSupervisionWorkloadFamily::ModuleStateControl));
        let full_summary = report
            .regime_summaries
            .iter()
            .find(|summary| summary.supervision_regime == TassadarWeakSupervisionRegime::FullTrace)
            .expect("full summary");
        assert!(full_summary.mean_later_window_exactness_bps > 8_000);
        assert!(full_summary.mean_under_supervised_failure_count <= 2);
        assert!(report
            .io_only_fragile_workloads
            .contains(&TassadarWeakSupervisionWorkloadFamily::VerifierSearchKernel));
    }

    #[test]
    fn weak_supervision_report_matches_committed_truth() {
        let generated =
            build_tassadar_weak_supervision_executor_report().expect("weak-supervision report");
        let committed: TassadarWeakSupervisionExecutorReport =
            read_repo_json(TASSADAR_WEAK_SUPERVISION_REPORT_REF)
                .expect("committed weak-supervision report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_weak_supervision_report_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_weak_supervision_executor_report.json");
        let report = write_tassadar_weak_supervision_executor_report(&output_path)
            .expect("write weak-supervision report");
        let written = std::fs::read_to_string(&output_path).expect("written report");
        let reparsed: TassadarWeakSupervisionExecutorReport =
            serde_json::from_str(&written).expect("written report should parse");

        assert_eq!(report, reparsed);
        assert_eq!(
            tassadar_weak_supervision_executor_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_weak_supervision_executor_report.json")
        );
    }
}
