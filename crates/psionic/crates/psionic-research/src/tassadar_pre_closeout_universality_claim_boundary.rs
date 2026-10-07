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
    build_tassadar_pre_closeout_universality_audit_report,
    TassadarPreCloseoutUniversalityAuditReport, TassadarPreCloseoutUniversalityAuditReportError,
};

pub const TASSADAR_PRE_CLOSEOUT_UNIVERSALITY_CLAIM_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_pre_closeout_universality_claim_boundary_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreCloseoutUniversalityClaimBoundaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarPreCloseoutUniversalityAuditReport,
    pub allowed_statement: String,
    pub blocked_by: Vec<String>,
    pub current_true_scopes: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPreCloseoutUniversalityClaimBoundaryReportError {
    #[error(transparent)]
    Eval(#[from] TassadarPreCloseoutUniversalityAuditReportError),
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

pub fn build_tassadar_pre_closeout_universality_claim_boundary_report() -> Result<
    TassadarPreCloseoutUniversalityClaimBoundaryReport,
    TassadarPreCloseoutUniversalityClaimBoundaryReportError,
> {
    let eval_report = build_tassadar_pre_closeout_universality_audit_report()?;
    let mut report = TassadarPreCloseoutUniversalityClaimBoundaryReport {
        schema_version: 1,
        report_id: String::from("tassadar.pre_closeout_universality_claim_boundary.report.v1"),
        allowed_statement: String::from(
            "Psionic/Tassadar now has one pre-closeout universality boundary report that names the bounded broadness already landed and the exact terminal-contract artifacts still missing before the repo can speak in Turing-completeness language.",
        ),
        blocked_by: eval_report.remaining_terminal_contract_ids.clone(),
        current_true_scopes: eval_report.current_true_scopes.clone(),
        explicit_non_implications: eval_report.explicit_non_implications.clone(),
        eval_report,
        claim_boundary: String::from(
            "this summary is disclosure-safe. It keeps pre-closeout broadness, frozen-window Wasm, proposal-family widening, authority bridges, and the terminal universal-substrate contract as distinct claim classes.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Pre-closeout universality claim-boundary report keeps blocked_by={}, current_true_scopes={}, claim_status={:?}.",
        report.blocked_by.len(),
        report.current_true_scopes.len(),
        report.eval_report.claim_status,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_pre_closeout_universality_claim_boundary_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_pre_closeout_universality_claim_boundary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PRE_CLOSEOUT_UNIVERSALITY_CLAIM_BOUNDARY_REPORT_REF)
}

pub fn write_tassadar_pre_closeout_universality_claim_boundary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPreCloseoutUniversalityClaimBoundaryReport,
    TassadarPreCloseoutUniversalityClaimBoundaryReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPreCloseoutUniversalityClaimBoundaryReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_pre_closeout_universality_claim_boundary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPreCloseoutUniversalityClaimBoundaryReportError::Write {
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
) -> Result<T, TassadarPreCloseoutUniversalityClaimBoundaryReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPreCloseoutUniversalityClaimBoundaryReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPreCloseoutUniversalityClaimBoundaryReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_pre_closeout_universality_claim_boundary_report, read_repo_json,
        tassadar_pre_closeout_universality_claim_boundary_report_path,
        TassadarPreCloseoutUniversalityClaimBoundaryReport,
        TASSADAR_PRE_CLOSEOUT_UNIVERSALITY_CLAIM_BOUNDARY_REPORT_REF,
    };

    #[test]
    fn pre_closeout_claim_boundary_report_keeps_terminal_gap_explicit() {
        let report =
            build_tassadar_pre_closeout_universality_claim_boundary_report().expect("report");

        assert!(report
            .blocked_by
            .contains(&String::from("minimal_universal_substrate_gate")));
        assert!(report
            .explicit_non_implications
            .contains(&String::from("Turing-complete support")));
    }

    #[test]
    fn pre_closeout_claim_boundary_report_matches_committed_truth() {
        let generated =
            build_tassadar_pre_closeout_universality_claim_boundary_report().expect("report");
        let committed: TassadarPreCloseoutUniversalityClaimBoundaryReport =
            read_repo_json(TASSADAR_PRE_CLOSEOUT_UNIVERSALITY_CLAIM_BOUNDARY_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_pre_closeout_universality_claim_boundary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_pre_closeout_universality_claim_boundary_report.json")
        );
    }
}
