use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    TASSADAR_INTERNAL_MODULE_LIBRARY_REPORT_REF, TassadarInternalModuleLibraryReport,
    TassadarInternalModuleLinkPosture,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_INTERNAL_MODULE_LIBRARY_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_module_library_summary.json";

/// Research summary for the internal module library lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleLibrarySummaryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Runtime report consumed by the summary.
    pub runtime_report: TassadarInternalModuleLibraryReport,
    /// Module ids reused by more than one consumer family.
    pub reused_module_ids: Vec<String>,
    /// Module ids carrying explicit rollback guardrails.
    pub rollback_guarded_module_ids: Vec<String>,
    /// Consumer families refused by the bounded library lane.
    pub refused_consumer_families: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Summary failure while building or writing the report.
#[derive(Debug, Error)]
pub enum TassadarInternalModuleLibrarySummaryError {
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

/// Builds the machine-readable internal module library summary report.
pub fn build_tassadar_internal_module_library_summary_report()
-> Result<TassadarInternalModuleLibrarySummaryReport, TassadarInternalModuleLibrarySummaryError> {
    let runtime_report: TassadarInternalModuleLibraryReport =
        read_repo_json(TASSADAR_INTERNAL_MODULE_LIBRARY_REPORT_REF)?;
    let reused_module_ids = runtime_report.cross_program_reused_module_ids.clone();
    let rollback_guarded_module_ids = runtime_report
        .active_modules
        .iter()
        .filter(|module| module.rollback_version.is_some())
        .map(|module| module.module_id.clone())
        .collect::<Vec<_>>();
    let refused_consumer_families = runtime_report
        .case_reports
        .iter()
        .filter(|case| case.link_posture == TassadarInternalModuleLinkPosture::Refused)
        .map(|case| case.consumer_program_family.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarInternalModuleLibrarySummaryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.internal_module_library.summary.v1"),
        runtime_report,
        reused_module_ids,
        rollback_guarded_module_ids,
        refused_consumer_families,
        claim_boundary: String::from(
            "this summary keeps internal module reuse, rollback guardrails, and refused consumer families explicit. It does not treat a benchmark-bound module library as unrestricted self-extension or broad install governance closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Internal module library summary now marks {} reused module families, {} rollback-guarded module families, and {} refused consumer families.",
        report.reused_module_ids.len(),
        report.rollback_guarded_module_ids.len(),
        report.refused_consumer_families.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_internal_module_library_summary_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed summary report.
#[must_use]
pub fn tassadar_internal_module_library_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_MODULE_LIBRARY_SUMMARY_REPORT_REF)
}

/// Writes the committed summary report.
pub fn write_tassadar_internal_module_library_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInternalModuleLibrarySummaryReport, TassadarInternalModuleLibrarySummaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalModuleLibrarySummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_internal_module_library_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalModuleLibrarySummaryError::Write {
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
        .expect("repo root should resolve from psionic-research crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarInternalModuleLibrarySummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarInternalModuleLibrarySummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInternalModuleLibrarySummaryError::Deserialize {
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
        TASSADAR_INTERNAL_MODULE_LIBRARY_SUMMARY_REPORT_REF,
        TassadarInternalModuleLibrarySummaryReport,
        build_tassadar_internal_module_library_summary_report, read_repo_json,
        tassadar_internal_module_library_summary_report_path,
        write_tassadar_internal_module_library_summary_report,
    };

    #[test]
    fn internal_module_library_summary_marks_reuse_rollback_and_refusal() {
        let report = build_tassadar_internal_module_library_summary_report().expect("summary");

        assert!(
            report
                .reused_module_ids
                .contains(&String::from("frontier_relax_core"))
        );
        assert!(
            report
                .rollback_guarded_module_ids
                .contains(&String::from("candidate_select_core"))
        );
        assert!(
            report
                .refused_consumer_families
                .contains(&String::from("sudoku_search"))
        );
    }

    #[test]
    fn internal_module_library_summary_matches_committed_truth() {
        let generated = build_tassadar_internal_module_library_summary_report().expect("summary");
        let committed: TassadarInternalModuleLibrarySummaryReport =
            read_repo_json(TASSADAR_INTERNAL_MODULE_LIBRARY_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_internal_module_library_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_internal_module_library_summary.json");
        let written =
            write_tassadar_internal_module_library_summary_report(&output_path).expect("write");
        let persisted: TassadarInternalModuleLibrarySummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_internal_module_library_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_internal_module_library_summary.json")
        );
    }
}
