use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_MODULE_INSTALLATION_STAGING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_installation_staging_report.json";

/// Bounded install scope used by the staged-install report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleInstallEvalScope {
    SessionMount,
    WorkerMount,
}

/// Final staged-install outcome recorded by the eval report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleInstallEvalOutcome {
    Activated,
    ChallengeWindow,
    RolledBack,
    Refused,
}

/// One staged-install drill case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleInstallationCaseReport {
    /// Stable install identifier.
    pub install_id: String,
    /// Stable module identifier.
    pub module_id: String,
    /// Requested version.
    pub requested_version: String,
    /// Resolved version after challenge or rollback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_version: Option<String>,
    /// Install scope.
    pub scope: TassadarModuleInstallEvalScope,
    /// Final install outcome.
    pub outcome: TassadarModuleInstallEvalOutcome,
    /// Whether a challenge ticket was attached or required.
    pub challenge_ticket_present: bool,
    /// Whether rollback lineage was attached.
    pub rollback_receipt_present: bool,
    /// Stable benchmark refs anchoring the case.
    pub benchmark_refs: Vec<String>,
    /// Plain-language case note.
    pub note: String,
}

/// Machine-readable staged-install drill report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleInstallationStagingReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Number of activated installs.
    pub activated_case_count: u32,
    /// Number of challenge-window installs.
    pub challenge_window_case_count: u32,
    /// Number of rollback drills.
    pub rolled_back_case_count: u32,
    /// Number of refused installs.
    pub refused_case_count: u32,
    /// Per-case staged-install reports.
    pub case_reports: Vec<TassadarModuleInstallationCaseReport>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Eval failure while building or writing the report.
#[derive(Debug, Error)]
pub enum TassadarModuleInstallationStagingReportError {
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

/// Builds the machine-readable staged-install drill report.
#[must_use]
pub fn build_tassadar_module_installation_staging_report() -> TassadarModuleInstallationStagingReport
{
    let case_reports = vec![
        case_report(
            "install.frontier_relax_core.session.v1",
            "frontier_relax_core",
            "1.0.0",
            Some("1.0.0"),
            TassadarModuleInstallEvalScope::SessionMount,
            TassadarModuleInstallEvalOutcome::Activated,
            false,
            false,
            &["fixtures/tassadar/reports/tassadar_internal_module_library_report.json"],
            "frontier_relax_core activated on a bounded session mount after compatibility and benchmark checks passed",
        ),
        case_report(
            "install.candidate_select_core.challenge.v1",
            "candidate_select_core",
            "1.2.0",
            None,
            TassadarModuleInstallEvalScope::WorkerMount,
            TassadarModuleInstallEvalOutcome::ChallengeWindow,
            true,
            false,
            &["fixtures/tassadar/reports/tassadar_internal_module_library_report.json"],
            "candidate_select_core entered a worker-mount challenge window before activation",
        ),
        case_report(
            "install.candidate_select_core.rollback.v1",
            "candidate_select_core",
            "1.2.0",
            Some("1.1.0"),
            TassadarModuleInstallEvalScope::WorkerMount,
            TassadarModuleInstallEvalOutcome::RolledBack,
            true,
            true,
            &["fixtures/tassadar/reports/tassadar_internal_module_library_report.json"],
            "candidate_select_core rolled back to 1.1.0 after the challenge drill failed",
        ),
        case_report(
            "install.branch_prune_core.refused.v1",
            "branch_prune_core",
            "0.1.0",
            None,
            TassadarModuleInstallEvalScope::SessionMount,
            TassadarModuleInstallEvalOutcome::Refused,
            false,
            false,
            &[],
            "branch_prune_core was refused because it lacked trusted-class and benchmark evidence",
        ),
    ];
    let mut report = TassadarModuleInstallationStagingReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.module_installation_staging.report.v1"),
        activated_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarModuleInstallEvalOutcome::Activated)
            .count() as u32,
        challenge_window_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarModuleInstallEvalOutcome::ChallengeWindow)
            .count() as u32,
        rolled_back_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarModuleInstallEvalOutcome::RolledBack)
            .count() as u32,
        refused_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarModuleInstallEvalOutcome::Refused)
            .count() as u32,
        case_reports,
        claim_boundary: String::from(
            "this report freezes bounded staged-install drills with explicit challenge windows, rollback lineage, and refusal paths. It does not treat controlled installation as evidence of unrestricted self-modification",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Module-install staging report now freezes {} drill cases with {} activated installs, {} challenge-window installs, {} rollback drills, and {} refused installs.",
        report.case_reports.len(),
        report.activated_case_count,
        report.challenge_window_case_count,
        report.rolled_back_case_count,
        report.refused_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_module_installation_staging_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_module_installation_staging_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_INSTALLATION_STAGING_REPORT_REF)
}

/// Writes the committed staged-install drill report.
pub fn write_tassadar_module_installation_staging_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleInstallationStagingReport, TassadarModuleInstallationStagingReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleInstallationStagingReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_installation_staging_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleInstallationStagingReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn case_report(
    install_id: &str,
    module_id: &str,
    requested_version: &str,
    resolved_version: Option<&str>,
    scope: TassadarModuleInstallEvalScope,
    outcome: TassadarModuleInstallEvalOutcome,
    challenge_ticket_present: bool,
    rollback_receipt_present: bool,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarModuleInstallationCaseReport {
    TassadarModuleInstallationCaseReport {
        install_id: String::from(install_id),
        module_id: String::from(module_id),
        requested_version: String::from(requested_version),
        resolved_version: resolved_version.map(String::from),
        scope,
        outcome,
        challenge_ticket_present,
        rollback_receipt_present,
        benchmark_refs: benchmark_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        note: String::from(note),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarModuleInstallationStagingReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarModuleInstallationStagingReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarModuleInstallationStagingReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_module_installation_staging_report, read_repo_json,
        tassadar_module_installation_staging_report_path,
        write_tassadar_module_installation_staging_report,
        TassadarModuleInstallationStagingReport,
        TASSADAR_MODULE_INSTALLATION_STAGING_REPORT_REF,
    };

    #[test]
    fn module_installation_staging_report_keeps_challenge_rollback_and_refusal_explicit() {
        let report = build_tassadar_module_installation_staging_report();

        assert_eq!(report.challenge_window_case_count, 1);
        assert_eq!(report.rolled_back_case_count, 1);
        assert_eq!(report.refused_case_count, 1);
        assert!(report.case_reports.iter().any(|case| {
            case.module_id == "candidate_select_core" && case.rollback_receipt_present
        }));
    }

    #[test]
    fn module_installation_staging_report_matches_committed_truth() {
        let generated = build_tassadar_module_installation_staging_report();
        let committed: TassadarModuleInstallationStagingReport =
            read_repo_json(TASSADAR_MODULE_INSTALLATION_STAGING_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_module_installation_staging_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_module_installation_staging_report.json");
        let written =
            write_tassadar_module_installation_staging_report(&output_path).expect("write");
        let persisted: TassadarModuleInstallationStagingReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_module_installation_staging_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_module_installation_staging_report.json")
        );
    }
}
