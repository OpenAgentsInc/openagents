use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarModuleInstallEvalOutcome, TassadarModuleInstallationStagingReport,
    TASSADAR_MODULE_INSTALLATION_STAGING_REPORT_REF,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_MODULE_INSTALLATION_STAGING_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_installation_staging_summary.json";

/// Research summary for the staged-install lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleInstallationStagingSummaryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Eval report consumed by the summary.
    pub staging_report: TassadarModuleInstallationStagingReport,
    /// Module ids with at least one activated install.
    pub activated_module_ids: Vec<String>,
    /// Module ids currently in or passing through challenge gates.
    pub challenge_gated_module_ids: Vec<String>,
    /// Module ids rolled back under explicit lineage.
    pub rolled_back_module_ids: Vec<String>,
    /// Module ids refused by install policy.
    pub refused_module_ids: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the summary.
    pub report_digest: String,
}

/// Summary failure while building or writing the report.
#[derive(Debug, Error)]
pub enum TassadarModuleInstallationStagingSummaryError {
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

/// Builds the machine-readable staged-install summary.
pub fn build_tassadar_module_installation_staging_summary_report(
) -> Result<
    TassadarModuleInstallationStagingSummaryReport,
    TassadarModuleInstallationStagingSummaryError,
> {
    let staging_report: TassadarModuleInstallationStagingReport =
        read_repo_json(TASSADAR_MODULE_INSTALLATION_STAGING_REPORT_REF)?;
    let activated_module_ids = staging_report
        .case_reports
        .iter()
        .filter(|case| case.outcome == TassadarModuleInstallEvalOutcome::Activated)
        .map(|case| case.module_id.clone())
        .collect::<Vec<_>>();
    let challenge_gated_module_ids = staging_report
        .case_reports
        .iter()
        .filter(|case| {
            matches!(
                case.outcome,
                TassadarModuleInstallEvalOutcome::ChallengeWindow
                    | TassadarModuleInstallEvalOutcome::RolledBack
            )
        })
        .map(|case| case.module_id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let rolled_back_module_ids = staging_report
        .case_reports
        .iter()
        .filter(|case| case.outcome == TassadarModuleInstallEvalOutcome::RolledBack)
        .map(|case| case.module_id.clone())
        .collect::<Vec<_>>();
    let refused_module_ids = staging_report
        .case_reports
        .iter()
        .filter(|case| case.outcome == TassadarModuleInstallEvalOutcome::Refused)
        .map(|case| case.module_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarModuleInstallationStagingSummaryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.module_installation_staging.summary.v1"),
        staging_report,
        activated_module_ids,
        challenge_gated_module_ids,
        rolled_back_module_ids,
        refused_module_ids,
        claim_boundary: String::from(
            "this summary keeps bounded installation, challenge windows, rollback lineage, and policy refusal explicit. It does not treat staged install receipts as evidence for unrestricted self-extension",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Module-install staging summary now marks {} activated modules, {} challenge-gated modules, {} rolled-back modules, and {} refused modules.",
        report.activated_module_ids.len(),
        report.challenge_gated_module_ids.len(),
        report.rolled_back_module_ids.len(),
        report.refused_module_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_module_installation_staging_summary_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed summary.
#[must_use]
pub fn tassadar_module_installation_staging_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_INSTALLATION_STAGING_SUMMARY_REPORT_REF)
}

/// Writes the committed staged-install summary.
pub fn write_tassadar_module_installation_staging_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarModuleInstallationStagingSummaryReport,
    TassadarModuleInstallationStagingSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleInstallationStagingSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_installation_staging_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleInstallationStagingSummaryError::Write {
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
) -> Result<T, TassadarModuleInstallationStagingSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarModuleInstallationStagingSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarModuleInstallationStagingSummaryError::Deserialize {
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
        build_tassadar_module_installation_staging_summary_report, read_repo_json,
        tassadar_module_installation_staging_summary_report_path,
        write_tassadar_module_installation_staging_summary_report,
        TassadarModuleInstallationStagingSummaryReport,
        TASSADAR_MODULE_INSTALLATION_STAGING_SUMMARY_REPORT_REF,
    };

    #[test]
    fn module_installation_staging_summary_marks_activated_challenged_and_refused_modules() {
        let report = build_tassadar_module_installation_staging_summary_report().expect("summary");

        assert!(report
            .activated_module_ids
            .contains(&String::from("frontier_relax_core")));
        assert!(report
            .challenge_gated_module_ids
            .contains(&String::from("candidate_select_core")));
        assert!(report
            .rolled_back_module_ids
            .contains(&String::from("candidate_select_core")));
        assert!(report
            .refused_module_ids
            .contains(&String::from("branch_prune_core")));
    }

    #[test]
    fn module_installation_staging_summary_matches_committed_truth() {
        let generated =
            build_tassadar_module_installation_staging_summary_report().expect("summary");
        let committed: TassadarModuleInstallationStagingSummaryReport =
            read_repo_json(TASSADAR_MODULE_INSTALLATION_STAGING_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_module_installation_staging_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_module_installation_staging_summary.json");
        let written =
            write_tassadar_module_installation_staging_summary_report(&output_path)
                .expect("write");
        let persisted: TassadarModuleInstallationStagingSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_module_installation_staging_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_module_installation_staging_summary.json")
        );
    }
}
