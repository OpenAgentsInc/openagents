use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_general_internal_compute_red_team_audit_report,
    TassadarGeneralInternalComputeRedTeamAuditReport,
    TassadarGeneralInternalComputeRedTeamAuditReportError,
};

pub const TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_general_internal_compute_red_team_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralInternalComputeRedTeamSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub audit_report: TassadarGeneralInternalComputeRedTeamAuditReport,
    pub allowed_statement: String,
    pub blocked_finding_ids: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarGeneralInternalComputeRedTeamSummaryError {
    #[error(transparent)]
    Audit(#[from] TassadarGeneralInternalComputeRedTeamAuditReportError),
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

pub fn build_tassadar_general_internal_compute_red_team_summary() -> Result<
    TassadarGeneralInternalComputeRedTeamSummary,
    TassadarGeneralInternalComputeRedTeamSummaryError,
> {
    let audit_report = build_tassadar_general_internal_compute_red_team_audit_report()?;
    let blocked_finding_ids = audit_report.blocked_finding_ids.clone();
    let explicit_non_implications = vec![
        String::from("arbitrary Wasm execution"),
        String::from("silent proposal-family inheritance"),
        String::from("public threads publication"),
        String::from("default served relaxed-SIMD"),
    ];
    let mut summary = TassadarGeneralInternalComputeRedTeamSummary {
        schema_version: 1,
        report_id: String::from("tassadar.general_internal_compute.red_team.summary.v1"),
        audit_report,
        allowed_statement: String::from(
            "Psionic/Tassadar can now say that the broader internal-compute lane has an explicit red-team boundary audit, and that the audited publication, route, and proposal-family leaks still fail closed under the current named-profile posture.",
        ),
        blocked_finding_ids,
        explicit_non_implications,
        claim_boundary: String::from(
            "this summary is disclosure-safe. It says the broader internal-compute lane has been red-teamed successfully without turning the successful audit itself into broader public capability promotion.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    summary.summary = format!(
        "General internal-compute red-team summary keeps blocked_findings={}, explicit_non_implications={}, publication_safe={}.",
        summary.blocked_finding_ids.len(),
        summary.explicit_non_implications.len(),
        summary.audit_report.publication_safe,
    );
    summary.report_digest = stable_digest(
        b"psionic_tassadar_general_internal_compute_red_team_summary|",
        &summary,
    );
    Ok(summary)
}

#[must_use]
pub fn tassadar_general_internal_compute_red_team_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_SUMMARY_REF)
}

pub fn write_tassadar_general_internal_compute_red_team_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarGeneralInternalComputeRedTeamSummary,
    TassadarGeneralInternalComputeRedTeamSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarGeneralInternalComputeRedTeamSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_general_internal_compute_red_team_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarGeneralInternalComputeRedTeamSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(summary)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root")
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
) -> Result<T, TassadarGeneralInternalComputeRedTeamSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarGeneralInternalComputeRedTeamSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarGeneralInternalComputeRedTeamSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_general_internal_compute_red_team_summary, read_repo_json,
        tassadar_general_internal_compute_red_team_summary_path,
        TassadarGeneralInternalComputeRedTeamSummary,
        TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_SUMMARY_REF,
    };

    #[test]
    fn red_team_summary_keeps_leaks_and_non_implications_explicit() {
        let summary = build_tassadar_general_internal_compute_red_team_summary().expect("summary");

        assert!(summary.audit_report.publication_safe);
        assert_eq!(summary.blocked_finding_ids.len(), 5);
        assert!(summary
            .explicit_non_implications
            .contains(&String::from("arbitrary Wasm execution")));
    }

    #[test]
    fn red_team_summary_matches_committed_truth() {
        let generated =
            build_tassadar_general_internal_compute_red_team_summary().expect("summary");
        let committed: TassadarGeneralInternalComputeRedTeamSummary =
            read_repo_json(TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_general_internal_compute_red_team_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_general_internal_compute_red_team_summary.json")
        );
    }
}
