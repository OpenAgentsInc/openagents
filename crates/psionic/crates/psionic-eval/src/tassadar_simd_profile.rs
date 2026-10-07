use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TASSADAR_SIMD_PROFILE_ID, TASSADAR_SIMD_PROFILE_RUNTIME_REPORT_REF,
    TassadarSimdProfileRuntimeReport, build_tassadar_simd_profile_runtime_report,
};

/// Stable committed report ref for the bounded SIMD deterministic profile.
pub const TASSADAR_SIMD_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_simd_profile_report.json";

/// Eval-facing report for the bounded SIMD deterministic profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimdProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_report_ref: String,
    pub runtime_report: TassadarSimdProfileRuntimeReport,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub exact_backend_ids: Vec<String>,
    pub fallback_backend_ids: Vec<String>,
    pub refused_backend_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSimdProfileReportError {
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

pub fn build_tassadar_simd_profile_report(
) -> Result<TassadarSimdProfileReport, TassadarSimdProfileReportError> {
    let runtime_report = build_tassadar_simd_profile_runtime_report();
    let overall_green = runtime_report.exact_case_count == 1
        && runtime_report.fallback_case_count >= 1
        && runtime_report.refusal_case_count >= 1
        && runtime_report
            .exact_backend_ids
            .contains(&String::from("cpu_reference_current_host"))
        && runtime_report
            .fallback_backend_ids
            .contains(&String::from("metal_served"))
        && runtime_report
            .fallback_backend_ids
            .contains(&String::from("cuda_served"));
    let mut report = TassadarSimdProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.simd_profile.report.v1"),
        runtime_report_ref: String::from(TASSADAR_SIMD_PROFILE_RUNTIME_REPORT_REF),
        exact_backend_ids: runtime_report.exact_backend_ids.clone(),
        fallback_backend_ids: runtime_report.fallback_backend_ids.clone(),
        refused_backend_ids: runtime_report.refused_backend_ids.clone(),
        runtime_report,
        public_profile_allowed_profile_ids: vec![String::from(TASSADAR_SIMD_PROFILE_ID)],
        default_served_profile_allowed_profile_ids: Vec::new(),
        overall_green,
        claim_boundary: String::from(
            "this eval report covers one bounded SIMD deterministic profile with exact cpu-reference publication, explicit scalar-fallback accelerator rows, and typed refusal truth. It does not claim arbitrary SIMD closure, accelerator-invariant vector exactness, or a default served SIMD lane",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "SIMD profile report covers exact_backends={}, fallback_backends={}, refused_backends={}, overall_green={}.",
        report.exact_backend_ids.len(),
        report.fallback_backend_ids.len(),
        report.refused_backend_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_simd_profile_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_simd_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SIMD_PROFILE_REPORT_REF)
}

pub fn write_tassadar_simd_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSimdProfileReport, TassadarSimdProfileReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarSimdProfileReportError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_tassadar_simd_profile_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSimdProfileReportError::Write {
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
) -> Result<T, TassadarSimdProfileReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarSimdProfileReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarSimdProfileReportError::Deserialize {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_SIMD_PROFILE_REPORT_REF, build_tassadar_simd_profile_report, read_json,
        tassadar_simd_profile_report_path, write_tassadar_simd_profile_report,
    };
    use psionic_runtime::TASSADAR_SIMD_PROFILE_ID;
    use tempfile::tempdir;

    #[test]
    fn simd_profile_report_keeps_exact_fallback_and_refusal_rows_explicit() {
        let report = build_tassadar_simd_profile_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(
            report.public_profile_allowed_profile_ids,
            vec![String::from(TASSADAR_SIMD_PROFILE_ID)]
        );
        assert!(report.default_served_profile_allowed_profile_ids.is_empty());
        assert_eq!(report.exact_backend_ids, vec![String::from("cpu_reference_current_host")]);
        assert!(report.fallback_backend_ids.contains(&String::from("metal_served")));
        assert!(report.fallback_backend_ids.contains(&String::from("cuda_served")));
        assert!(report
            .refused_backend_ids
            .contains(&String::from("accelerator_specific_unbounded")));
    }

    #[test]
    fn simd_profile_report_matches_committed_truth() {
        let generated = build_tassadar_simd_profile_report().expect("report");
        let committed = read_json(TASSADAR_SIMD_PROFILE_REPORT_REF).expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_simd_profile_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("tassadar_simd_profile_report.json");
        let report = write_tassadar_simd_profile_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_simd_profile_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_simd_profile_report.json")
        );
    }
}
