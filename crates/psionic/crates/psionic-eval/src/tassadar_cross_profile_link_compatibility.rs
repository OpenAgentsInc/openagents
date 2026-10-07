use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_compiler::{
    TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_REF,
    TassadarCrossProfileLinkCompatibilityReport,
    build_tassadar_cross_profile_link_compatibility_report,
};
use psionic_router::{
    TASSADAR_CROSS_PROFILE_LINK_ROUTE_POLICY_REPORT_REF, TassadarCrossProfileLinkRoutePolicyReport,
    build_tassadar_cross_profile_link_route_policy_report,
};
use psionic_runtime::{
    TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_BUNDLE_FILE,
    TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_RUN_ROOT_REF,
    TassadarCrossProfileLinkCompatibilityRuntimeBundle,
    build_tassadar_cross_profile_link_compatibility_runtime_bundle,
};

pub const TASSADAR_CROSS_PROFILE_LINK_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_cross_profile_link_eval_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCrossProfileLinkEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub compiler_report_ref: String,
    pub compiler_report: TassadarCrossProfileLinkCompatibilityReport,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarCrossProfileLinkCompatibilityRuntimeBundle,
    pub route_policy_report_ref: String,
    pub route_policy_report: TassadarCrossProfileLinkRoutePolicyReport,
    pub routeable_case_ids: Vec<String>,
    pub downgraded_case_ids: Vec<String>,
    pub refused_case_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub overall_green: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarCrossProfileLinkEvalReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_cross_profile_link_eval_report()
-> Result<TassadarCrossProfileLinkEvalReport, TassadarCrossProfileLinkEvalReportError> {
    let compiler_report = build_tassadar_cross_profile_link_compatibility_report();
    let runtime_bundle = build_tassadar_cross_profile_link_compatibility_runtime_bundle();
    let route_policy_report = build_tassadar_cross_profile_link_route_policy_report();
    let routeable_case_ids = route_policy_report.routeable_case_ids.clone();
    let downgraded_case_ids = route_policy_report.downgraded_case_ids.clone();
    let refused_case_ids = route_policy_report.refused_case_ids.clone();
    let mut report = TassadarCrossProfileLinkEvalReport {
        schema_version: 1,
        report_id: String::from("tassadar.cross_profile_link.eval_report.v1"),
        compiler_report_ref: String::from(TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_REF),
        compiler_report,
        runtime_bundle_ref: format!(
            "{}/{}",
            TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_RUN_ROOT_REF,
            TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_BUNDLE_FILE
        ),
        runtime_bundle,
        route_policy_report_ref: String::from(TASSADAR_CROSS_PROFILE_LINK_ROUTE_POLICY_REPORT_REF),
        route_policy_report,
        routeable_case_ids,
        downgraded_case_ids,
        refused_case_ids,
        served_publication_allowed: false,
        overall_green: false,
        generated_from_refs: vec![
            String::from(TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_REF),
            format!(
                "{}/{}",
                TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_RUN_ROOT_REF,
                TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_BUNDLE_FILE
            ),
            String::from(TASSADAR_CROSS_PROFILE_LINK_ROUTE_POLICY_REPORT_REF),
        ],
        claim_boundary: String::from(
            "this eval report covers one bounded cross-profile linking lane with explicit exact routing, downgrade planning, and typed refusal. It does not imply arbitrary profile flattening or broader served publication; served_publication_allowed remains false here by design",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.compiler_report.exact_case_count == 1
        && report.compiler_report.downgraded_case_count == 1
        && report.compiler_report.refused_case_count == 2
        && report.runtime_bundle.exact_case_count == 1
        && report.runtime_bundle.downgraded_case_count == 1
        && report.runtime_bundle.refused_case_count == 2
        && report.route_policy_report.overall_green
        && !report.served_publication_allowed;
    report.summary = format!(
        "Cross-profile link eval report freezes routeable_cases={}, downgraded_cases={}, refused_cases={}, served_publication_allowed={}, overall_green={}.",
        report.routeable_case_ids.len(),
        report.downgraded_case_ids.len(),
        report.refused_case_ids.len(),
        report.served_publication_allowed,
        report.overall_green,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_cross_profile_link_eval_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_cross_profile_link_eval_report_path() -> PathBuf {
    repo_root().join(TASSADAR_CROSS_PROFILE_LINK_EVAL_REPORT_REF)
}

pub fn write_tassadar_cross_profile_link_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCrossProfileLinkEvalReport, TassadarCrossProfileLinkEvalReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCrossProfileLinkEvalReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_cross_profile_link_eval_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCrossProfileLinkEvalReportError::Write {
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
) -> Result<T, TassadarCrossProfileLinkEvalReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarCrossProfileLinkEvalReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCrossProfileLinkEvalReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_cross_profile_link_eval_report, read_json,
        tassadar_cross_profile_link_eval_report_path,
        write_tassadar_cross_profile_link_eval_report,
    };

    #[test]
    fn cross_profile_link_eval_report_keeps_routeability_downgrade_and_non_publication_explicit() {
        let report = build_tassadar_cross_profile_link_eval_report().expect("report");

        assert_eq!(report.routeable_case_ids.len(), 2);
        assert_eq!(report.downgraded_case_ids.len(), 1);
        assert_eq!(report.refused_case_ids.len(), 2);
        assert!(!report.served_publication_allowed);
        assert!(report.overall_green);
    }

    #[test]
    fn cross_profile_link_eval_report_matches_committed_truth() {
        let generated = build_tassadar_cross_profile_link_eval_report().expect("report");
        let committed =
            read_json(tassadar_cross_profile_link_eval_report_path()).expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_cross_profile_link_eval_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_cross_profile_link_eval_report.json");
        let report =
            write_tassadar_cross_profile_link_eval_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
    }
}
