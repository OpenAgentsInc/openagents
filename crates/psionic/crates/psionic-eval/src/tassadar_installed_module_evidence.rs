use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TassadarInstalledModuleEvidenceStatus, build_tassadar_installed_module_evidence_bundle,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_INSTALLED_MODULE_EVIDENCE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_installed_module_evidence_report.json";

/// Eval summary over the installed-module evidence bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledModuleEvidenceReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Number of complete evidence records.
    pub complete_record_count: u32,
    /// Number of missing-evidence refusals.
    pub missing_evidence_refusal_count: u32,
    /// Number of stale-evidence refusals.
    pub stale_evidence_refusal_count: u32,
    /// Number of records with explicit revocation hooks.
    pub revocation_ready_record_count: u32,
    /// Number of reinstall parity groups confirmed across multiple installs.
    pub reinstall_parity_group_count: u32,
    /// Number of records with audit or decompilation artifacts present.
    pub audit_receipt_ready_record_count: u32,
    /// Stable bundle ref consumed by the report.
    pub bundle_ref: String,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Eval failure while persisting or validating the report.
#[derive(Debug, Error)]
pub enum TassadarInstalledModuleEvidenceReportError {
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
    #[error(transparent)]
    Bundle(#[from] psionic_runtime::TassadarInstalledModuleEvidenceBundleError),
}

/// Builds the machine-legible installed-module evidence summary.
pub fn build_tassadar_installed_module_evidence_report()
-> Result<TassadarInstalledModuleEvidenceReport, TassadarInstalledModuleEvidenceReportError> {
    let bundle = build_tassadar_installed_module_evidence_bundle()?;
    let parity_groups = bundle
        .records
        .iter()
        .filter_map(|record| {
            record
                .reinstall_parity_digest
                .as_ref()
                .map(|digest| ((record.module_ref.as_str(), digest.as_str()), record.status))
        })
        .fold(
            BTreeMap::<(&str, &str), u32>::new(),
            |mut groups, (key, status)| {
                if status == TassadarInstalledModuleEvidenceStatus::Complete {
                    *groups.entry(key).or_insert(0) += 1;
                }
                groups
            },
        );
    let reinstall_parity_group_count =
        parity_groups.values().filter(|count| **count >= 2).count() as u32;
    let mut report = TassadarInstalledModuleEvidenceReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.installed_module_evidence.report.v1"),
        complete_record_count: bundle
            .records
            .iter()
            .filter(|record| record.status == TassadarInstalledModuleEvidenceStatus::Complete)
            .count() as u32,
        missing_evidence_refusal_count: bundle
            .records
            .iter()
            .filter(|record| {
                record.status == TassadarInstalledModuleEvidenceStatus::RefusedMissingEvidence
            })
            .count() as u32,
        stale_evidence_refusal_count: bundle
            .records
            .iter()
            .filter(|record| {
                record.status == TassadarInstalledModuleEvidenceStatus::RefusedStaleEvidence
            })
            .count() as u32,
        revocation_ready_record_count: bundle
            .records
            .iter()
            .filter(|record| !record.revocation_hooks.is_empty())
            .count() as u32,
        reinstall_parity_group_count,
        audit_receipt_ready_record_count: bundle
            .records
            .iter()
            .filter(|record| !record.audit_artifact_refs.is_empty())
            .count() as u32,
        bundle_ref: String::from(
            "fixtures/tassadar/runs/tassadar_installed_module_evidence_v1/installed_module_evidence_bundle.json",
        ),
        claim_boundary: String::from(
            "this eval report freezes missing-evidence refusal, stale-evidence refusal, revocation-hook readiness, and reinstall-parity truth for the bounded installed-module lane. It does not let installed evidence bundles imply economic authority or unrestricted module promotion",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Installed-module evidence report now freezes {} complete records, {} missing-evidence refusals, {} stale-evidence refusals, {} revocation-ready records, and {} reinstall parity groups.",
        report.complete_record_count,
        report.missing_evidence_refusal_count,
        report.stale_evidence_refusal_count,
        report.revocation_ready_record_count,
        report.reinstall_parity_group_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_installed_module_evidence_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed eval report.
#[must_use]
pub fn tassadar_installed_module_evidence_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INSTALLED_MODULE_EVIDENCE_REPORT_REF)
}

/// Writes the committed eval report.
pub fn write_tassadar_installed_module_evidence_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInstalledModuleEvidenceReport, TassadarInstalledModuleEvidenceReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInstalledModuleEvidenceReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_installed_module_evidence_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInstalledModuleEvidenceReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_installed_module_evidence_report(
    path: impl AsRef<Path>,
) -> Result<TassadarInstalledModuleEvidenceReport, TassadarInstalledModuleEvidenceReportError> {
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
) -> Result<T, TassadarInstalledModuleEvidenceReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarInstalledModuleEvidenceReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInstalledModuleEvidenceReportError::Deserialize {
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
        build_tassadar_installed_module_evidence_report,
        load_tassadar_installed_module_evidence_report,
        tassadar_installed_module_evidence_report_path,
    };

    #[test]
    fn installed_module_evidence_report_tracks_missing_stale_and_parity_cases() {
        let report = build_tassadar_installed_module_evidence_report().expect("report");

        assert_eq!(report.complete_record_count, 3);
        assert_eq!(report.missing_evidence_refusal_count, 1);
        assert_eq!(report.stale_evidence_refusal_count, 1);
        assert_eq!(report.revocation_ready_record_count, 5);
        assert_eq!(report.reinstall_parity_group_count, 1);
        assert_eq!(report.audit_receipt_ready_record_count, 4);
    }

    #[test]
    fn installed_module_evidence_report_matches_committed_truth() {
        let expected = build_tassadar_installed_module_evidence_report().expect("report");
        let committed = load_tassadar_installed_module_evidence_report(
            tassadar_installed_module_evidence_report_path(),
        )
        .expect("committed report");

        assert_eq!(committed, expected);
    }
}
