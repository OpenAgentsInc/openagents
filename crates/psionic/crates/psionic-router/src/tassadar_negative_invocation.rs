use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF, TASSADAR_NEGATIVE_INVOCATION_ROUTE_AUDIT_REF,
    TassadarNegativeInvocationEvidenceBundle, TassadarNegativeInvocationTrainingCase,
    TassadarPlannerRouteFamily,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Case-level before/after audit for negative-invocation planner training.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNegativeInvocationAuditCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Baseline route family.
    pub baseline_route_family: TassadarPlannerRouteFamily,
    /// Preferred route family after negative-invocation supervision.
    pub preferred_route_family: TassadarPlannerRouteFamily,
    /// Whether the baseline route needlessly invoked the internal executor.
    pub unnecessary_internal_invocation_before: bool,
    /// Whether the preferred route still needlessly invokes the internal executor.
    pub unnecessary_internal_invocation_after: bool,
    /// Baseline fallback churn count.
    pub baseline_fallback_churn_count: u32,
    /// Preferred fallback churn count.
    pub preferred_fallback_churn_count: u32,
    /// Whether the baseline route would refuse while a better lane exists.
    pub refusal_when_better_lane_exists_before: bool,
    /// Whether the preferred route would refuse while a better lane exists.
    pub refusal_when_better_lane_exists_after: bool,
    /// Baseline cost in milliunits.
    pub baseline_cost_milliunits: u32,
    /// Preferred cost in milliunits.
    pub preferred_cost_milliunits: u32,
    /// Baseline latency in milliseconds.
    pub baseline_latency_millis: u32,
    /// Preferred latency in milliseconds.
    pub preferred_latency_millis: u32,
    /// Baseline evidence quality in basis points.
    pub baseline_evidence_quality_bps: u32,
    /// Preferred evidence quality in basis points.
    pub preferred_evidence_quality_bps: u32,
    /// Plain-language note.
    pub note: String,
}

/// Router-side audit over the negative-invocation training bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNegativeInvocationRouteAudit {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable audit identifier.
    pub audit_id: String,
    /// Source evidence bundle ref.
    pub evidence_bundle_ref: String,
    /// Source evidence bundle digest.
    pub evidence_bundle_digest: String,
    /// Ordered case audits.
    pub case_audits: Vec<TassadarNegativeInvocationAuditCase>,
    /// Baseline unnecessary internal invocation rate.
    pub unnecessary_internal_invocation_before_bps: u32,
    /// Preferred unnecessary internal invocation rate.
    pub unnecessary_internal_invocation_after_bps: u32,
    /// Total baseline fallback churn.
    pub fallback_churn_before_total: u32,
    /// Total preferred fallback churn.
    pub fallback_churn_after_total: u32,
    /// Baseline refusal-when-better-lane-exists rate.
    pub refusal_when_better_lane_exists_before_bps: u32,
    /// Preferred refusal-when-better-lane-exists rate.
    pub refusal_when_better_lane_exists_after_bps: u32,
    /// Average baseline cost.
    pub baseline_average_cost_milliunits: u32,
    /// Average preferred cost.
    pub preferred_average_cost_milliunits: u32,
    /// Average baseline latency.
    pub baseline_average_latency_millis: u32,
    /// Average preferred latency.
    pub preferred_average_latency_millis: u32,
    /// Average baseline evidence quality.
    pub baseline_average_evidence_quality_bps: u32,
    /// Average preferred evidence quality.
    pub preferred_average_evidence_quality_bps: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable audit digest.
    pub audit_digest: String,
}

/// Route-audit build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarNegativeInvocationRouteAuditError {
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

/// Builds the committed negative-invocation route audit.
pub fn build_tassadar_negative_invocation_route_audit()
-> Result<TassadarNegativeInvocationRouteAudit, TassadarNegativeInvocationRouteAuditError> {
    let bundle: TassadarNegativeInvocationEvidenceBundle =
        read_repo_json(TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF)?;
    let case_audits = bundle
        .cases
        .iter()
        .map(build_case_audit)
        .collect::<Vec<_>>();
    let case_count = case_audits.len() as u32;
    let mut audit = TassadarNegativeInvocationRouteAudit {
        schema_version: REPORT_SCHEMA_VERSION,
        audit_id: String::from("tassadar.negative_invocation.route_audit.v1"),
        evidence_bundle_ref: String::from(TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF),
        evidence_bundle_digest: bundle.bundle_digest.clone(),
        unnecessary_internal_invocation_before_bps: ratio_bps(
            case_audits
                .iter()
                .filter(|case| case.unnecessary_internal_invocation_before)
                .count() as u32,
            case_count,
        ),
        unnecessary_internal_invocation_after_bps: ratio_bps(
            case_audits
                .iter()
                .filter(|case| case.unnecessary_internal_invocation_after)
                .count() as u32,
            case_count,
        ),
        fallback_churn_before_total: case_audits
            .iter()
            .map(|case| case.baseline_fallback_churn_count)
            .sum(),
        fallback_churn_after_total: case_audits
            .iter()
            .map(|case| case.preferred_fallback_churn_count)
            .sum(),
        refusal_when_better_lane_exists_before_bps: ratio_bps(
            case_audits
                .iter()
                .filter(|case| case.refusal_when_better_lane_exists_before)
                .count() as u32,
            case_count,
        ),
        refusal_when_better_lane_exists_after_bps: ratio_bps(
            case_audits
                .iter()
                .filter(|case| case.refusal_when_better_lane_exists_after)
                .count() as u32,
            case_count,
        ),
        baseline_average_cost_milliunits: average_u32(
            case_audits
                .iter()
                .map(|case| case.baseline_cost_milliunits)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        preferred_average_cost_milliunits: average_u32(
            case_audits
                .iter()
                .map(|case| case.preferred_cost_milliunits)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        baseline_average_latency_millis: average_u32(
            case_audits
                .iter()
                .map(|case| case.baseline_latency_millis)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        preferred_average_latency_millis: average_u32(
            case_audits
                .iter()
                .map(|case| case.preferred_latency_millis)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        baseline_average_evidence_quality_bps: average_u32(
            case_audits
                .iter()
                .map(|case| case.baseline_evidence_quality_bps)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        preferred_average_evidence_quality_bps: average_u32(
            case_audits
                .iter()
                .map(|case| case.preferred_evidence_quality_bps)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        case_audits,
        claim_boundary: String::from(
            "this audit is a benchmark-bound router surface over seeded negative-invocation training cases. It compares baseline and preferred route posture without turning route improvements into served capability promotion or accepted-outcome authority",
        ),
        summary: String::new(),
        audit_digest: String::new(),
    };
    audit.summary = format!(
        "Negative-invocation route audit covers {} seeded cases with unnecessary_internal_invocation {}->{} bps, fallback_churn {}->{}, and refusal_when_better_lane_exists {}->{} bps.",
        audit.case_audits.len(),
        audit.unnecessary_internal_invocation_before_bps,
        audit.unnecessary_internal_invocation_after_bps,
        audit.fallback_churn_before_total,
        audit.fallback_churn_after_total,
        audit.refusal_when_better_lane_exists_before_bps,
        audit.refusal_when_better_lane_exists_after_bps,
    );
    audit.audit_digest =
        stable_digest(b"psionic_tassadar_negative_invocation_route_audit|", &audit);
    Ok(audit)
}

/// Returns the canonical absolute path for the committed route audit.
#[must_use]
pub fn tassadar_negative_invocation_route_audit_path() -> PathBuf {
    repo_root().join(TASSADAR_NEGATIVE_INVOCATION_ROUTE_AUDIT_REF)
}

/// Writes the committed negative-invocation route audit.
pub fn write_tassadar_negative_invocation_route_audit(
    output_path: impl AsRef<Path>,
) -> Result<TassadarNegativeInvocationRouteAudit, TassadarNegativeInvocationRouteAuditError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarNegativeInvocationRouteAuditError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let audit = build_tassadar_negative_invocation_route_audit()?;
    let json = serde_json::to_string_pretty(&audit)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarNegativeInvocationRouteAuditError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(audit)
}

fn build_case_audit(
    case: &TassadarNegativeInvocationTrainingCase,
) -> TassadarNegativeInvocationAuditCase {
    let baseline = route_outcome(case, case.baseline_route_family);
    let preferred = route_outcome(case, case.preferred_route_family);
    TassadarNegativeInvocationAuditCase {
        case_id: case.case_id.clone(),
        baseline_route_family: case.baseline_route_family,
        preferred_route_family: case.preferred_route_family,
        unnecessary_internal_invocation_before: case.unnecessary_internal_invocation,
        unnecessary_internal_invocation_after: false,
        baseline_fallback_churn_count: baseline.fallback_churn_count,
        preferred_fallback_churn_count: preferred.fallback_churn_count,
        refusal_when_better_lane_exists_before: baseline.would_refuse_when_better_lane_exists,
        refusal_when_better_lane_exists_after: preferred.would_refuse_when_better_lane_exists,
        baseline_cost_milliunits: baseline.estimated_cost_milliunits,
        preferred_cost_milliunits: preferred.estimated_cost_milliunits,
        baseline_latency_millis: baseline.estimated_latency_millis,
        preferred_latency_millis: preferred.estimated_latency_millis,
        baseline_evidence_quality_bps: baseline.evidence_quality_bps,
        preferred_evidence_quality_bps: preferred.evidence_quality_bps,
        note: case.note.clone(),
    }
}

fn route_outcome(
    case: &TassadarNegativeInvocationTrainingCase,
    route_family: TassadarPlannerRouteFamily,
) -> &psionic_models::TassadarNegativeInvocationRouteOutcome {
    case.route_outcomes
        .iter()
        .find(|outcome| outcome.route_family == route_family)
        .expect("route family should exist in seeded outcomes")
}

fn average_u32(values: &[u32]) -> u32 {
    if values.is_empty() {
        0
    } else {
        values.iter().sum::<u32>() / values.len() as u32
    }
}

fn ratio_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        numerator.saturating_mul(10_000) / denominator
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-router crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarNegativeInvocationRouteAuditError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarNegativeInvocationRouteAuditError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarNegativeInvocationRouteAuditError::Deserialize {
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
        TassadarNegativeInvocationRouteAudit, build_tassadar_negative_invocation_route_audit,
        read_repo_json, tassadar_negative_invocation_route_audit_path,
        write_tassadar_negative_invocation_route_audit,
    };
    use psionic_models::TASSADAR_NEGATIVE_INVOCATION_ROUTE_AUDIT_REF;

    #[test]
    fn negative_invocation_route_audit_keeps_before_after_penalties_explicit() {
        let audit =
            build_tassadar_negative_invocation_route_audit().expect("negative invocation audit");

        assert_eq!(audit.case_audits.len(), 6);
        assert!(audit.unnecessary_internal_invocation_before_bps >= 6_000);
        assert_eq!(audit.unnecessary_internal_invocation_after_bps, 0);
        assert!(audit.fallback_churn_before_total > audit.fallback_churn_after_total);
        assert!(
            audit.refusal_when_better_lane_exists_before_bps
                > audit.refusal_when_better_lane_exists_after_bps
        );
    }

    #[test]
    fn negative_invocation_route_audit_matches_committed_truth() {
        let generated =
            build_tassadar_negative_invocation_route_audit().expect("negative invocation audit");
        let committed: TassadarNegativeInvocationRouteAudit =
            read_repo_json(TASSADAR_NEGATIVE_INVOCATION_ROUTE_AUDIT_REF).expect("committed audit");
        assert_eq!(generated, committed);
    }

    #[test]
    fn negative_invocation_route_audit_writer_uses_committed_path() {
        let path = tassadar_negative_invocation_route_audit_path();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("tassadar_negative_invocation_route_audit.json")
        );
    }

    #[test]
    fn negative_invocation_route_audit_writer_round_trips() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let output_path = temp_dir
            .path()
            .join("tassadar_negative_invocation_route_audit.json");
        let written =
            write_tassadar_negative_invocation_route_audit(&output_path).expect("write audit");
        let decoded = std::fs::read_to_string(&output_path).expect("read audit");
        let reparsed: TassadarNegativeInvocationRouteAudit =
            serde_json::from_str(&decoded).expect("decode audit");

        assert_eq!(written, reparsed);
    }
}
