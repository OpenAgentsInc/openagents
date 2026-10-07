use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_router::{
    TASSADAR_EFFECT_ROUTE_POLICY_REPORT_REF, TassadarEffectRouteKind,
    build_tassadar_effect_route_policy_report,
};
use psionic_runtime::{
    TassadarEffectReceipt, TassadarEffectRefusalReason, TassadarEffectRequest,
    TassadarEffectTaxonomy, negotiate_tassadar_effect_request, tassadar_effect_taxonomy,
};
use psionic_sandbox::{
    TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF, TassadarSandboxEffectBoundary,
    tassadar_sandbox_effect_boundary,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_EFFECT_TAXONOMY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_effect_taxonomy_report.json";

/// One negotiated request or refusal over the widened effect taxonomy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectTaxonomyCaseReport {
    pub case_id: String,
    pub request: TassadarEffectRequest,
    pub expected_route_kind: TassadarEffectRouteKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<TassadarEffectReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarEffectRefusalReason>,
    pub note: String,
}

/// Committed eval report for the widened effect taxonomy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectTaxonomyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub effect_taxonomy: TassadarEffectTaxonomy,
    pub sandbox_effect_boundary: TassadarSandboxEffectBoundary,
    pub route_policy_ref: String,
    pub admitted_case_count: u32,
    pub refusal_case_count: u32,
    pub deterministic_internal_case_count: u32,
    pub host_state_case_count: u32,
    pub sandbox_delegation_case_count: u32,
    pub receipt_bound_input_case_count: u32,
    pub refused_side_effect_case_count: u32,
    pub case_reports: Vec<TassadarEffectTaxonomyCaseReport>,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarEffectTaxonomyReportError {
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

pub fn build_tassadar_effect_taxonomy_report()
-> Result<TassadarEffectTaxonomyReport, TassadarEffectTaxonomyReportError> {
    let effect_taxonomy = tassadar_effect_taxonomy();
    let sandbox_effect_boundary = tassadar_sandbox_effect_boundary();
    let route_policy = build_tassadar_effect_route_policy_report();
    let case_reports = seeded_case_reports(&effect_taxonomy, &route_policy)?;
    let admitted_case_count = case_reports
        .iter()
        .filter(|case| case.receipt.is_some())
        .count() as u32;
    let refusal_case_count = case_reports
        .iter()
        .filter(|case| case.refusal_reason.is_some())
        .count() as u32;
    let deterministic_internal_case_count = case_reports
        .iter()
        .filter(|case| case.expected_route_kind == TassadarEffectRouteKind::InternalExact)
        .count() as u32;
    let host_state_case_count = case_reports
        .iter()
        .filter(|case| case.expected_route_kind == TassadarEffectRouteKind::HostStateSnapshotBound)
        .count() as u32;
    let sandbox_delegation_case_count = case_reports
        .iter()
        .filter(|case| case.expected_route_kind == TassadarEffectRouteKind::SandboxDelegation)
        .count() as u32;
    let receipt_bound_input_case_count = case_reports
        .iter()
        .filter(|case| case.expected_route_kind == TassadarEffectRouteKind::ReceiptBoundInput)
        .count() as u32;
    let refused_side_effect_case_count = case_reports
        .iter()
        .filter(|case| case.expected_route_kind == TassadarEffectRouteKind::Refused)
        .count() as u32;
    let mut generated_from_refs = vec![
        String::from(TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF),
        String::from(TASSADAR_EFFECT_ROUTE_POLICY_REPORT_REF),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let mut report = TassadarEffectTaxonomyReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.effect_taxonomy.report.v1"),
        effect_taxonomy,
        sandbox_effect_boundary,
        route_policy_ref: String::from(TASSADAR_EFFECT_ROUTE_POLICY_REPORT_REF),
        admitted_case_count,
        refusal_case_count,
        deterministic_internal_case_count,
        host_state_case_count,
        sandbox_delegation_case_count,
        receipt_bound_input_case_count,
        refused_side_effect_case_count,
        case_reports,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report widens the former narrow import matrix into a typed effect taxonomy with explicit receipts and replay limits. It keeps deterministic internal stubs, host-backed state, sandbox delegation, receipt-bound nondeterministic inputs, and refused unsafe side effects distinct instead of flattening them into one implicit host-behavior bucket",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Effect taxonomy report covers {} negotiated cases with admitted={}, refused={}, and category coverage internal={}, host_state={}, sandbox_delegation={}, receipt_bound_input={}, refused_side_effect={}.",
        report.case_reports.len(),
        report.admitted_case_count,
        report.refusal_case_count,
        report.deterministic_internal_case_count,
        report.host_state_case_count,
        report.sandbox_delegation_case_count,
        report.receipt_bound_input_case_count,
        report.refused_side_effect_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_effect_taxonomy_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_effect_taxonomy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EFFECT_TAXONOMY_REPORT_REF)
}

pub fn write_tassadar_effect_taxonomy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarEffectTaxonomyReport, TassadarEffectTaxonomyReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarEffectTaxonomyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_effect_taxonomy_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarEffectTaxonomyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_effect_taxonomy_report(
    path: impl AsRef<Path>,
) -> Result<TassadarEffectTaxonomyReport, TassadarEffectTaxonomyReportError> {
    read_json(path)
}

fn seeded_case_reports(
    taxonomy: &TassadarEffectTaxonomy,
    route_policy: &psionic_router::TassadarEffectRoutePolicyReport,
) -> Result<Vec<TassadarEffectTaxonomyCaseReport>, TassadarEffectTaxonomyReportError> {
    let cases = vec![
        (
            String::from("internal_exact_clock_stub"),
            TassadarEffectRequest {
                request_id: String::from("req.effect.internal_exact_clock_stub"),
                effect_ref: String::from("env.clock_stub"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 12,
            },
            String::from("deterministic stub stays on the internal exact route"),
        ),
        (
            String::from("host_state_snapshot_bound_allowed"),
            TassadarEffectRequest {
                request_id: String::from("req.effect.host_state_snapshot_bound_allowed"),
                effect_ref: String::from("state.counter_slot_read"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: true,
                durable_state_receipt_present: true,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 2,
            },
            String::from(
                "durable host state stays admissible only with snapshot and state receipt evidence",
            ),
        ),
        (
            String::from("host_state_replay_limit_refusal"),
            TassadarEffectRequest {
                request_id: String::from("req.effect.host_state_replay_limit_refusal"),
                effect_ref: String::from("state.counter_slot_read"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: true,
                durable_state_receipt_present: true,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 4,
            },
            String::from("durable host state refuses once the explicit replay window is exceeded"),
        ),
        (
            String::from("sandbox_delegation_allowed"),
            TassadarEffectRequest {
                request_id: String::from("req.effect.sandbox_delegation_allowed"),
                effect_ref: String::from("sandbox.math_eval"),
                allow_external_delegation: true,
                policy_allows_delegation: true,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: true,
                challenge_receipt_present: true,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 1,
            },
            String::from(
                "sandbox delegation stays explicit and challengeable instead of being rebranded as internal compute",
            ),
        ),
        (
            String::from("sandbox_delegation_policy_denied"),
            TassadarEffectRequest {
                request_id: String::from("req.effect.sandbox_delegation_policy_denied"),
                effect_ref: String::from("sandbox.math_eval"),
                allow_external_delegation: true,
                policy_allows_delegation: false,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: true,
                challenge_receipt_present: true,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 1,
            },
            String::from("delegation still refuses when policy denies the sandbox path"),
        ),
        (
            String::from("receipt_bound_input_allowed"),
            TassadarEffectRequest {
                request_id: String::from("req.effect.receipt_bound_input_allowed"),
                effect_ref: String::from("input.relay_sample"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: true,
                replay_attempt: 1,
            },
            String::from(
                "bounded nondeterministic input stays admissible only with an observed-value receipt",
            ),
        ),
        (
            String::from("receipt_bound_input_missing_receipt"),
            TassadarEffectRequest {
                request_id: String::from("req.effect.receipt_bound_input_missing_receipt"),
                effect_ref: String::from("input.relay_sample"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 1,
            },
            String::from(
                "bounded nondeterministic input refuses when the receipt window is missing",
            ),
        ),
        (
            String::from("unsafe_side_effect_refusal"),
            TassadarEffectRequest {
                request_id: String::from("req.effect.unsafe_side_effect_refusal"),
                effect_ref: String::from("host.fs_write"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 0,
            },
            String::from(
                "unsafe side effects remain a typed refusal instead of widening into ambient host authority",
            ),
        ),
    ];
    cases
        .into_iter()
        .map(|(case_id, request, note)| {
            let expected_route_kind = route_policy
                .rows
                .iter()
                .find(|row| row.effect_ref == request.effect_ref)
                .map(|row| row.route_kind)
                .ok_or_else(|| {
                    TassadarEffectTaxonomyReportError::Json(serde_json::Error::io(
                        std::io::Error::other(format!(
                            "missing route-policy row for effect `{}`",
                            request.effect_ref
                        )),
                    ))
                })?;
            match negotiate_tassadar_effect_request(&request, taxonomy) {
                Ok(receipt) => Ok(TassadarEffectTaxonomyCaseReport {
                    case_id,
                    request,
                    expected_route_kind,
                    receipt: Some(receipt),
                    refusal_reason: None,
                    note,
                }),
                Err(refusal_reason) => Ok(TassadarEffectTaxonomyCaseReport {
                    case_id,
                    request,
                    expected_route_kind,
                    receipt: None,
                    refusal_reason: Some(refusal_reason),
                    note,
                }),
            }
        })
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarEffectTaxonomyReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarEffectTaxonomyReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarEffectTaxonomyReportError::Deserialize {
        path: path.display().to_string(),
        error,
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
    use psionic_router::TassadarEffectRouteKind;
    use psionic_runtime::TassadarEffectRefusalReason;

    use super::{
        build_tassadar_effect_taxonomy_report, load_tassadar_effect_taxonomy_report,
        tassadar_effect_taxonomy_report_path, write_tassadar_effect_taxonomy_report,
    };

    #[test]
    fn effect_taxonomy_report_covers_admitted_and_refused_paths() {
        let report = build_tassadar_effect_taxonomy_report().expect("report");
        assert_eq!(report.admitted_case_count, 4);
        assert_eq!(report.refusal_case_count, 4);
        assert_eq!(report.host_state_case_count, 2);
        assert_eq!(report.sandbox_delegation_case_count, 2);
        assert_eq!(report.receipt_bound_input_case_count, 2);
        assert!(report.case_reports.iter().any(|case| {
            case.case_id == "host_state_replay_limit_refusal"
                && case.expected_route_kind == TassadarEffectRouteKind::HostStateSnapshotBound
                && case.refusal_reason == Some(TassadarEffectRefusalReason::ReplayLimitExceeded)
        }));
        assert!(report.case_reports.iter().any(|case| {
            case.case_id == "receipt_bound_input_allowed"
                && case.receipt.is_some()
                && case.expected_route_kind == TassadarEffectRouteKind::ReceiptBoundInput
        }));
    }

    #[test]
    fn effect_taxonomy_report_matches_committed_truth() {
        let report = build_tassadar_effect_taxonomy_report().expect("report");
        let persisted =
            load_tassadar_effect_taxonomy_report(tassadar_effect_taxonomy_report_path())
                .expect("committed report");
        assert_eq!(persisted, report);
    }

    #[test]
    fn write_effect_taxonomy_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_effect_taxonomy_report.json");
        let report = write_tassadar_effect_taxonomy_report(&output_path).expect("report");
        let persisted = load_tassadar_effect_taxonomy_report(&output_path).expect("persisted");
        assert_eq!(persisted, report);
        std::fs::remove_file(output_path).expect("temp report should be removable");
    }
}
