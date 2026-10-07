use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_effective_unbounded_compute_claim_report,
    build_tassadar_full_core_wasm_public_acceptance_gate_report,
    build_tassadar_relaxed_simd_research_ladder_report,
    build_tassadar_shared_state_concurrency_verdict_report,
    TassadarEffectiveUnboundedComputeClaimReportError,
    TassadarFullCoreWasmPublicAcceptanceGateReportError,
    TassadarRelaxedSimdResearchLadderReportError,
    TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_REPORT_REF,
    TASSADAR_FULL_CORE_WASM_PUBLIC_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_RELAXED_SIMD_RESEARCH_LADDER_REPORT_REF,
    TASSADAR_SHARED_STATE_CONCURRENCY_VERDICT_REPORT_REF,
};
use psionic_router::{
    build_tassadar_general_internal_compute_red_team_route_exercises_report,
    TassadarGeneralInternalComputeRedTeamRouteExercisesReport,
    TassadarGeneralInternalComputeRedTeamRouteExercisesReportError,
    TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_ROUTE_EXERCISES_REPORT_REF,
};

pub const TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_general_internal_compute_red_team_audit_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeneralInternalComputeRedTeamFindingStatus {
    BlockedAsExpected,
    UnexpectedPass,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralInternalComputeRedTeamFindingRow {
    pub finding_id: String,
    pub target_surface: String,
    pub finding_status: TassadarGeneralInternalComputeRedTeamFindingStatus,
    pub source_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralInternalComputeRedTeamAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub route_exercises_report_ref: String,
    pub route_exercises_report: TassadarGeneralInternalComputeRedTeamRouteExercisesReport,
    pub finding_rows: Vec<TassadarGeneralInternalComputeRedTeamFindingRow>,
    pub blocked_finding_ids: Vec<String>,
    pub failed_finding_ids: Vec<String>,
    pub overall_green: bool,
    pub publication_safe: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarGeneralInternalComputeRedTeamAuditReportError {
    #[error(transparent)]
    EffectiveUnbounded(#[from] TassadarEffectiveUnboundedComputeClaimReportError),
    #[error(transparent)]
    FullCoreWasm(#[from] TassadarFullCoreWasmPublicAcceptanceGateReportError),
    #[error(transparent)]
    RelaxedSimd(#[from] TassadarRelaxedSimdResearchLadderReportError),
    #[error(transparent)]
    RouteExercises(#[from] TassadarGeneralInternalComputeRedTeamRouteExercisesReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_general_internal_compute_red_team_audit_report() -> Result<
    TassadarGeneralInternalComputeRedTeamAuditReport,
    TassadarGeneralInternalComputeRedTeamAuditReportError,
> {
    let effective_unbounded = build_tassadar_effective_unbounded_compute_claim_report()?;
    let full_core_wasm = build_tassadar_full_core_wasm_public_acceptance_gate_report()?;
    let relaxed_simd = build_tassadar_relaxed_simd_research_ladder_report()?;
    let shared_state = build_tassadar_shared_state_concurrency_verdict_report();
    let route_exercises =
        build_tassadar_general_internal_compute_red_team_route_exercises_report()?;

    let finding_rows = vec![
        finding(
            "arbitrary_wasm_claim_stays_blocked",
            "claim_checker",
            effective_unbounded
                .out_of_scope_claims
                .contains(&String::from("arbitrary Wasm execution")),
            &[TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_REPORT_REF],
            "the effective-unbounded claim checker still names arbitrary Wasm execution as an explicit non-implication",
        ),
        finding(
            "full_core_wasm_publication_stays_suppressed",
            "public_acceptance_gate",
            !full_core_wasm.served_publication_allowed,
            &[TASSADAR_FULL_CORE_WASM_PUBLIC_ACCEPTANCE_GATE_REPORT_REF],
            "the frozen core-Wasm lane still refuses silent public publication while its gate remains suppressed",
        ),
        finding(
            "relaxed_simd_stays_non_promoted",
            "proposal_profile_ladder",
            !relaxed_simd.public_promotion_allowed && !relaxed_simd.default_served_profile_allowed,
            &[TASSADAR_RELAXED_SIMD_RESEARCH_LADDER_REPORT_REF],
            "relaxed-SIMD remains a research-only ladder and cannot inherit deterministic SIMD publication",
        ),
        finding(
            "shared_state_threads_stay_publicly_suppressed",
            "shared_state_verdicts",
            shared_state.public_profile_allowed_profile_ids.is_empty()
                && shared_state
                    .operator_profile_allowed_profile_ids
                    .contains(&String::from(
                        "tassadar.research_profile.threads_deterministic_scheduler.v1",
                    )),
            &[TASSADAR_SHARED_STATE_CONCURRENCY_VERDICT_REPORT_REF],
            "threads remains operator-visible but publicly suppressed, so concurrency research does not widen public posture",
        ),
        finding(
            "route_exercises_found_no_leak",
            "router",
            route_exercises.overall_green,
            &[TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_ROUTE_EXERCISES_REPORT_REF],
            "the router route-exercise matrix found no case where candidate-only, operator-only, or research-only profiles routed more broadly than declared",
        ),
    ];

    let blocked_finding_ids = finding_rows
        .iter()
        .filter(|finding| {
            finding.finding_status
                == TassadarGeneralInternalComputeRedTeamFindingStatus::BlockedAsExpected
        })
        .map(|finding| finding.finding_id.clone())
        .collect::<Vec<_>>();
    let failed_finding_ids = finding_rows
        .iter()
        .filter(|finding| {
            finding.finding_status
                == TassadarGeneralInternalComputeRedTeamFindingStatus::UnexpectedPass
        })
        .map(|finding| finding.finding_id.clone())
        .collect::<Vec<_>>();

    let mut report = TassadarGeneralInternalComputeRedTeamAuditReport {
        schema_version: 1,
        report_id: String::from("tassadar.general_internal_compute.red_team_audit.report.v1"),
        route_exercises_report_ref: String::from(
            TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_ROUTE_EXERCISES_REPORT_REF,
        ),
        route_exercises_report: route_exercises,
        finding_rows,
        blocked_finding_ids,
        failed_finding_ids,
        overall_green: false,
        publication_safe: false,
        claim_boundary: String::from(
            "this red-team audit is a disclosure-safe boundary check for the broader internal-compute lane. It proves that unsupported publication, implicit proposal inheritance, and silent route widening still fail closed instead of leaking into public claim posture.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.failed_finding_ids.is_empty()
        && report.blocked_finding_ids.len() == 5
        && report.route_exercises_report.overall_green;
    report.publication_safe = report.overall_green;
    report.summary = format!(
        "General internal-compute red-team audit keeps blocked_findings={}, failed_findings={}, route_cases={}, publication_safe={}.",
        report.blocked_finding_ids.len(),
        report.failed_finding_ids.len(),
        report.route_exercises_report.blocked_case_ids.len(),
        report.publication_safe,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_general_internal_compute_red_team_audit_report|",
        &report,
    );
    Ok(report)
}

fn finding(
    finding_id: &str,
    target_surface: &str,
    blocked_as_expected: bool,
    source_refs: &[&str],
    note: &str,
) -> TassadarGeneralInternalComputeRedTeamFindingRow {
    TassadarGeneralInternalComputeRedTeamFindingRow {
        finding_id: String::from(finding_id),
        target_surface: String::from(target_surface),
        finding_status: if blocked_as_expected {
            TassadarGeneralInternalComputeRedTeamFindingStatus::BlockedAsExpected
        } else {
            TassadarGeneralInternalComputeRedTeamFindingStatus::UnexpectedPass
        },
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

#[must_use]
pub fn tassadar_general_internal_compute_red_team_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_AUDIT_REPORT_REF)
}

pub fn write_tassadar_general_internal_compute_red_team_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarGeneralInternalComputeRedTeamAuditReport,
    TassadarGeneralInternalComputeRedTeamAuditReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarGeneralInternalComputeRedTeamAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_general_internal_compute_red_team_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarGeneralInternalComputeRedTeamAuditReportError::Write {
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
) -> Result<T, TassadarGeneralInternalComputeRedTeamAuditReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarGeneralInternalComputeRedTeamAuditReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarGeneralInternalComputeRedTeamAuditReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_general_internal_compute_red_team_audit_report, read_json,
        tassadar_general_internal_compute_red_team_audit_report_path,
        TassadarGeneralInternalComputeRedTeamAuditReport,
        TassadarGeneralInternalComputeRedTeamFindingStatus,
    };

    #[test]
    fn red_team_audit_keeps_claim_leaks_blocked() {
        let report =
            build_tassadar_general_internal_compute_red_team_audit_report().expect("report");

        assert!(report.overall_green);
        assert!(report.publication_safe);
        assert_eq!(report.blocked_finding_ids.len(), 5);
        assert!(report.failed_finding_ids.is_empty());
        assert!(report.finding_rows.iter().all(|finding| {
            finding.finding_status
                == TassadarGeneralInternalComputeRedTeamFindingStatus::BlockedAsExpected
        }));
    }

    #[test]
    fn red_team_audit_matches_committed_truth() {
        let generated =
            build_tassadar_general_internal_compute_red_team_audit_report().expect("report");
        let committed: TassadarGeneralInternalComputeRedTeamAuditReport =
            read_json(tassadar_general_internal_compute_red_team_audit_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_general_internal_compute_red_team_audit_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_general_internal_compute_red_team_audit_report.json")
        );
    }
}
