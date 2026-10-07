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
    TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF,
    TassadarInternalComputePackageManagerReport,
    build_tassadar_internal_compute_package_manager_report,
};
use psionic_router::{
    TASSADAR_INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF,
    TassadarInternalComputePackageRoutePolicyReport,
    build_tassadar_internal_compute_package_route_policy_report,
};

pub const TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_compute_package_manager_eval_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputePackageManagerEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub compiler_report_ref: String,
    pub compiler_report: TassadarInternalComputePackageManagerReport,
    pub route_policy_report_ref: String,
    pub route_policy_report: TassadarInternalComputePackageRoutePolicyReport,
    pub public_package_ids: Vec<String>,
    pub default_served_package_ids: Vec<String>,
    pub routeable_package_ids: Vec<String>,
    pub refused_package_case_ids: Vec<String>,
    pub benchmark_ref_count: u32,
    pub served_publication_allowed: bool,
    pub overall_green: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarInternalComputePackageManagerEvalReportError {
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

pub fn build_tassadar_internal_compute_package_manager_eval_report(
) -> Result<TassadarInternalComputePackageManagerEvalReport, TassadarInternalComputePackageManagerEvalReportError>
{
    let compiler_report = build_tassadar_internal_compute_package_manager_report();
    let route_policy_report = build_tassadar_internal_compute_package_route_policy_report();
    let benchmark_ref_count = compiler_report
        .package_entries
        .iter()
        .flat_map(|entry| entry.benchmark_refs.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .len() as u32;
    let generated_from_refs = vec![
        String::from(TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF),
        String::from(TASSADAR_INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF),
    ];
    let mut report = TassadarInternalComputePackageManagerEvalReport {
        schema_version: 1,
        report_id: String::from("tassadar.internal_compute_package_manager.eval_report.v1"),
        compiler_report_ref: String::from(TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF),
        compiler_report,
        route_policy_report_ref: String::from(
            TASSADAR_INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF,
        ),
        route_policy_report,
        public_package_ids: Vec::new(),
        default_served_package_ids: Vec::new(),
        routeable_package_ids: Vec::new(),
        refused_package_case_ids: Vec::new(),
        benchmark_ref_count,
        served_publication_allowed: true,
        overall_green: false,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded internal-compute package manager lane with named public packages, explicit route policy, and zero default-served packages. It does not claim arbitrary package discovery, arbitrary dependency solving, or generic broad internal compute publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.public_package_ids = report.route_policy_report.named_public_package_ids.clone();
    report.default_served_package_ids = report.route_policy_report.default_served_package_ids.clone();
    report.routeable_package_ids = report.route_policy_report.routeable_package_ids.clone();
    report.refused_package_case_ids = report.route_policy_report.refused_package_case_ids.clone();
    report.overall_green = report.compiler_report.exact_case_count == 3
        && report.compiler_report.refusal_case_count == 3
        && report.route_policy_report.overall_green
        && report.default_served_package_ids.is_empty();
    report.summary = format!(
        "Internal compute package manager eval report freezes public_packages={}, routeable_packages={}, refused_cases={}, benchmark_ref_count={}, served_publication_allowed={}, overall_green={}.",
        report.public_package_ids.len(),
        report.routeable_package_ids.len(),
        report.refused_package_case_ids.len(),
        report.benchmark_ref_count,
        report.served_publication_allowed,
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_internal_compute_package_manager_eval_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_internal_compute_package_manager_eval_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_EVAL_REPORT_REF)
}

pub fn write_tassadar_internal_compute_package_manager_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInternalComputePackageManagerEvalReport, TassadarInternalComputePackageManagerEvalReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalComputePackageManagerEvalReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_internal_compute_package_manager_eval_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalComputePackageManagerEvalReportError::Write {
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
) -> Result<T, TassadarInternalComputePackageManagerEvalReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarInternalComputePackageManagerEvalReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInternalComputePackageManagerEvalReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_internal_compute_package_manager_eval_report, read_json,
        tassadar_internal_compute_package_manager_eval_report_path,
        write_tassadar_internal_compute_package_manager_eval_report,
    };

    #[test]
    fn internal_compute_package_manager_eval_report_keeps_named_public_and_default_served_split_explicit()
    {
        let report = build_tassadar_internal_compute_package_manager_eval_report()
            .expect("report");

        assert_eq!(report.public_package_ids.len(), 3);
        assert!(report.default_served_package_ids.is_empty());
        assert_eq!(report.routeable_package_ids.len(), 3);
        assert_eq!(report.refused_package_case_ids.len(), 3);
        assert!(report.served_publication_allowed);
        assert!(report.overall_green);
    }

    #[test]
    fn internal_compute_package_manager_eval_report_matches_committed_truth() {
        let generated = build_tassadar_internal_compute_package_manager_eval_report()
            .expect("report");
        let committed = read_json(tassadar_internal_compute_package_manager_eval_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_internal_compute_package_manager_eval_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_internal_compute_package_manager_eval_report.json");
        let report = write_tassadar_internal_compute_package_manager_eval_report(&output_path)
            .expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
    }
}
