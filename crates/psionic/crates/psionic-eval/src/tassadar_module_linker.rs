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
    TassadarModuleLinkRuntimePosture, build_tassadar_module_link_runtime_report,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_MODULE_LINK_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_link_eval_report.json";

/// Eval summary over the bounded module-link runtime report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleLinkEvalReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Number of exact link cases.
    pub exact_case_count: u32,
    /// Number of rollback link cases.
    pub rollback_case_count: u32,
    /// Number of refused link cases.
    pub refused_case_count: u32,
    /// Total dependency edges preserved across exact and rollback cases.
    pub dependency_edge_count: u32,
    /// Number of exact or rolled-back cases that preserved output parity.
    pub exact_output_parity_case_count: u32,
    /// Number of exact or rolled-back cases that preserved trace parity.
    pub exact_trace_parity_case_count: u32,
    /// Stable benchmark refs anchoring the report.
    pub benchmark_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Eval failure while persisting or validating the report.
#[derive(Debug, Error)]
pub enum TassadarModuleLinkEvalReportError {
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

/// Builds the machine-legible eval summary over bounded module-link runtime truth.
#[must_use]
pub fn build_tassadar_module_link_eval_report() -> TassadarModuleLinkEvalReport {
    let runtime_report = build_tassadar_module_link_runtime_report();
    let mut benchmark_refs = runtime_report
        .case_reports
        .iter()
        .flat_map(|case| case.benchmark_refs.iter().cloned())
        .collect::<Vec<_>>();
    benchmark_refs.sort();
    benchmark_refs.dedup();
    let exact_output_parity_case_count = runtime_report
        .case_reports
        .iter()
        .filter(|case| {
            case.posture != TassadarModuleLinkRuntimePosture::Refused
                && case.exact_outputs_preserved
        })
        .count() as u32;
    let exact_trace_parity_case_count = runtime_report
        .case_reports
        .iter()
        .filter(|case| {
            case.posture != TassadarModuleLinkRuntimePosture::Refused && case.exact_trace_match
        })
        .count() as u32;
    let mut report = TassadarModuleLinkEvalReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.module_link_eval.report.v1"),
        exact_case_count: runtime_report.exact_case_count,
        rollback_case_count: runtime_report.rollback_case_count,
        refused_case_count: runtime_report.refused_case_count,
        dependency_edge_count: runtime_report.dependency_edge_count,
        exact_output_parity_case_count,
        exact_trace_parity_case_count,
        benchmark_refs,
        claim_boundary: String::from(
            "this eval summary freezes bounded module-link exact, rollback, and refusal posture over the current internal module lane. It does not widen module composition into arbitrary install or unrestricted software growth claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Module-link eval report now freezes {} exact cases, {} rollback cases, {} refused cases, {} preserved dependency edges, {} output-parity cases, and {} trace-parity cases.",
        report.exact_case_count,
        report.rollback_case_count,
        report.refused_case_count,
        report.dependency_edge_count,
        report.exact_output_parity_case_count,
        report.exact_trace_parity_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_module_link_eval_report|", &report);
    report
}

/// Returns the canonical absolute path for the committed eval report.
#[must_use]
pub fn tassadar_module_link_eval_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_LINK_EVAL_REPORT_REF)
}

/// Writes the committed eval report.
pub fn write_tassadar_module_link_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleLinkEvalReport, TassadarModuleLinkEvalReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleLinkEvalReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_link_eval_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleLinkEvalReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_module_link_eval_report(
    path: impl AsRef<Path>,
) -> Result<TassadarModuleLinkEvalReport, TassadarModuleLinkEvalReportError> {
    read_json(path)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarModuleLinkEvalReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarModuleLinkEvalReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarModuleLinkEvalReportError::Deserialize {
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
    use super::{
        build_tassadar_module_link_eval_report, load_tassadar_module_link_eval_report,
        tassadar_module_link_eval_report_path,
    };

    #[test]
    fn module_link_eval_report_tracks_parity_and_refusal_counts() {
        let report = build_tassadar_module_link_eval_report();

        assert_eq!(report.exact_case_count, 1);
        assert_eq!(report.rollback_case_count, 1);
        assert_eq!(report.refused_case_count, 1);
        assert_eq!(report.dependency_edge_count, 1);
        assert_eq!(report.exact_output_parity_case_count, 2);
        assert_eq!(report.exact_trace_parity_case_count, 2);
    }

    #[test]
    fn module_link_eval_report_matches_committed_truth() {
        let expected = build_tassadar_module_link_eval_report();
        let committed =
            load_tassadar_module_link_eval_report(tassadar_module_link_eval_report_path())
                .expect("committed report");

        assert_eq!(committed, expected);
    }
}
