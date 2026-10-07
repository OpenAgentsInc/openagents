use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF, TASSADAR_NEGATIVE_INVOCATION_REPORT_REF,
    TASSADAR_NEGATIVE_INVOCATION_ROUTE_AUDIT_REF, TassadarNegativeInvocationEvidenceBundle,
    tassadar_negative_invocation_publication,
};
use psionic_router::TassadarNegativeInvocationRouteAudit;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Eval-side summary report for negative-invocation planner training.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNegativeInvocationReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Source publication identifier.
    pub publication_id: String,
    /// Source evidence bundle ref.
    pub evidence_bundle_ref: String,
    /// Source route audit ref.
    pub route_audit_ref: String,
    /// Baseline unnecessary internal invocation rate.
    pub unnecessary_internal_invocation_before_bps: u32,
    /// Preferred unnecessary internal invocation rate.
    pub unnecessary_internal_invocation_after_bps: u32,
    /// Baseline fallback churn.
    pub fallback_churn_before_total: u32,
    /// Preferred fallback churn.
    pub fallback_churn_after_total: u32,
    /// Baseline average cost.
    pub baseline_average_cost_milliunits: u32,
    /// Preferred average cost.
    pub preferred_average_cost_milliunits: u32,
    /// Baseline average latency.
    pub baseline_average_latency_millis: u32,
    /// Preferred average latency.
    pub preferred_average_latency_millis: u32,
    /// Baseline average evidence quality.
    pub baseline_average_evidence_quality_bps: u32,
    /// Preferred average evidence quality.
    pub preferred_average_evidence_quality_bps: u32,
    /// Count of cases preferring language-only after supervision.
    pub preferred_language_only_case_count: u32,
    /// Count of cases preferring internal exact-compute after supervision.
    pub preferred_internal_exact_compute_case_count: u32,
    /// Count of cases preferring external-tool after supervision.
    pub preferred_external_tool_case_count: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// Report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarNegativeInvocationReportError {
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

/// Builds the committed negative-invocation report.
pub fn build_tassadar_negative_invocation_report()
-> Result<TassadarNegativeInvocationReport, TassadarNegativeInvocationReportError> {
    let publication = tassadar_negative_invocation_publication();
    let bundle: TassadarNegativeInvocationEvidenceBundle =
        read_repo_json(TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF)?;
    let audit: TassadarNegativeInvocationRouteAudit =
        read_repo_json(TASSADAR_NEGATIVE_INVOCATION_ROUTE_AUDIT_REF)?;
    let preferred_language_only_case_count = bundle
        .cases
        .iter()
        .filter(|case| {
            case.preferred_route_family == psionic_models::TassadarPlannerRouteFamily::LanguageOnly
        })
        .count() as u32;
    let preferred_internal_exact_compute_case_count = bundle
        .cases
        .iter()
        .filter(|case| {
            case.preferred_route_family
                == psionic_models::TassadarPlannerRouteFamily::InternalExactCompute
        })
        .count() as u32;
    let preferred_external_tool_case_count = bundle
        .cases
        .iter()
        .filter(|case| {
            case.preferred_route_family == psionic_models::TassadarPlannerRouteFamily::ExternalTool
        })
        .count() as u32;
    let mut report = TassadarNegativeInvocationReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.negative_invocation.report.v1"),
        publication_id: publication.publication_id,
        evidence_bundle_ref: String::from(TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF),
        route_audit_ref: String::from(TASSADAR_NEGATIVE_INVOCATION_ROUTE_AUDIT_REF),
        unnecessary_internal_invocation_before_bps: audit
            .unnecessary_internal_invocation_before_bps,
        unnecessary_internal_invocation_after_bps: audit.unnecessary_internal_invocation_after_bps,
        fallback_churn_before_total: audit.fallback_churn_before_total,
        fallback_churn_after_total: audit.fallback_churn_after_total,
        baseline_average_cost_milliunits: audit.baseline_average_cost_milliunits,
        preferred_average_cost_milliunits: audit.preferred_average_cost_milliunits,
        baseline_average_latency_millis: audit.baseline_average_latency_millis,
        preferred_average_latency_millis: audit.preferred_average_latency_millis,
        baseline_average_evidence_quality_bps: audit.baseline_average_evidence_quality_bps,
        preferred_average_evidence_quality_bps: audit.preferred_average_evidence_quality_bps,
        preferred_language_only_case_count,
        preferred_internal_exact_compute_case_count,
        preferred_external_tool_case_count,
        claim_boundary: String::from(
            "this report is a benchmark-bound research surface over seeded planner cases. It compares baseline and preferred routing on cost, latency, evidence quality, churn, and unnecessary invocation without promoting any lane or implying accepted-outcome closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Negative-invocation report covers {} seeded cases with unnecessary_internal_invocation {}->{} bps, cost {}->{}, latency {}->{}, and evidence_quality {}->{} bps.",
        bundle.cases.len(),
        report.unnecessary_internal_invocation_before_bps,
        report.unnecessary_internal_invocation_after_bps,
        report.baseline_average_cost_milliunits,
        report.preferred_average_cost_milliunits,
        report.baseline_average_latency_millis,
        report.preferred_average_latency_millis,
        report.baseline_average_evidence_quality_bps,
        report.preferred_average_evidence_quality_bps,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_negative_invocation_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed negative-invocation report.
#[must_use]
pub fn tassadar_negative_invocation_report_path() -> PathBuf {
    repo_root().join(TASSADAR_NEGATIVE_INVOCATION_REPORT_REF)
}

/// Writes the committed negative-invocation report.
pub fn write_tassadar_negative_invocation_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarNegativeInvocationReport, TassadarNegativeInvocationReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarNegativeInvocationReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_negative_invocation_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarNegativeInvocationReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarNegativeInvocationReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarNegativeInvocationReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarNegativeInvocationReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
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
        TassadarNegativeInvocationReport, build_tassadar_negative_invocation_report,
        read_repo_json, tassadar_negative_invocation_report_path,
        write_tassadar_negative_invocation_report,
    };
    use psionic_models::TASSADAR_NEGATIVE_INVOCATION_REPORT_REF;

    #[test]
    fn negative_invocation_report_keeps_cost_latency_quality_tradeoffs_explicit() {
        let report =
            build_tassadar_negative_invocation_report().expect("negative invocation report");

        assert_eq!(report.preferred_language_only_case_count, 2);
        assert_eq!(report.preferred_internal_exact_compute_case_count, 2);
        assert_eq!(report.preferred_external_tool_case_count, 2);
        assert!(report.baseline_average_cost_milliunits > report.preferred_average_cost_milliunits);
        assert!(report.baseline_average_latency_millis > report.preferred_average_latency_millis);
        assert!(
            report.preferred_average_evidence_quality_bps
                > report.baseline_average_evidence_quality_bps
        );
    }

    #[test]
    fn negative_invocation_report_matches_committed_truth() {
        let generated = build_tassadar_negative_invocation_report().expect("report");
        let committed: TassadarNegativeInvocationReport =
            read_repo_json(TASSADAR_NEGATIVE_INVOCATION_REPORT_REF).expect("committed");
        assert_eq!(generated, committed);
    }

    #[test]
    fn negative_invocation_report_writer_round_trips() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_negative_invocation_report.json");
        let written = write_tassadar_negative_invocation_report(&output_path).expect("write");
        let persisted: TassadarNegativeInvocationReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }

    #[test]
    fn negative_invocation_report_writer_uses_committed_path() {
        let path = tassadar_negative_invocation_report_path();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("tassadar_negative_invocation_report.json")
        );
    }
}
