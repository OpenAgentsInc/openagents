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
    TassadarTuringCompletenessCloseoutAuditReport,
    TassadarTuringCompletenessCloseoutAuditReportError,
    build_tassadar_turing_completeness_closeout_audit_report,
};

pub const TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_turing_completeness_closeout_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTuringCompletenessCloseoutSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarTuringCompletenessCloseoutAuditReport,
    pub allowed_statement: String,
    pub explicit_scope: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub refusal_boundary_ids: Vec<String>,
    pub served_blocked_by: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarTuringCompletenessCloseoutSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarTuringCompletenessCloseoutAuditReportError),
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

pub fn build_tassadar_turing_completeness_closeout_summary()
-> Result<TassadarTuringCompletenessCloseoutSummary, TassadarTuringCompletenessCloseoutSummaryError>
{
    let eval_report = build_tassadar_turing_completeness_closeout_audit_report()?;
    let served_blocked_by = eval_report
        .universality_verdict_split_report
        .verdict_rows
        .iter()
        .find(|row| row.verdict_level == psionic_eval::TassadarUniversalityVerdictLevel::Served)
        .map(|row| row.blocked_by.clone())
        .unwrap_or_default();

    let mut summary = TassadarTuringCompletenessCloseoutSummary {
        schema_version: 1,
        report_id: String::from("tassadar.turing_completeness_closeout.summary.v1"),
        allowed_statement: eval_report.allowed_statement.clone(),
        explicit_scope: eval_report.explicit_scope.clone(),
        portability_envelope_ids: eval_report.portability_envelope_ids.clone(),
        refusal_boundary_ids: eval_report.refusal_boundary_ids.clone(),
        served_blocked_by,
        explicit_non_implications: eval_report.explicit_non_implications.clone(),
        eval_report,
        claim_boundary: String::from(
            "this summary is the disclosure-safe terminal closeout statement for Turing completeness inside standalone psionic. It keeps theory/operator support, portability envelopes, refusal boundaries, and served suppression explicit.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    summary.summary = format!(
        "Turing-completeness closeout summary keeps portability_envelopes={}, refusal_boundaries={}, served_blocked_by={}, claim_status={:?}.",
        summary.portability_envelope_ids.len(),
        summary.refusal_boundary_ids.len(),
        summary.served_blocked_by.len(),
        summary.eval_report.claim_status,
    );
    summary.report_digest = stable_digest(
        b"psionic_tassadar_turing_completeness_closeout_summary|",
        &summary,
    );
    Ok(summary)
}

#[must_use]
pub fn tassadar_turing_completeness_closeout_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF)
}

pub fn write_tassadar_turing_completeness_closeout_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarTuringCompletenessCloseoutSummary, TassadarTuringCompletenessCloseoutSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarTuringCompletenessCloseoutSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_turing_completeness_closeout_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarTuringCompletenessCloseoutSummaryError::Write {
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
        .expect("repo root should resolve from psionic-research crate dir")
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
) -> Result<T, TassadarTuringCompletenessCloseoutSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarTuringCompletenessCloseoutSummaryError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarTuringCompletenessCloseoutSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF,
        TassadarTuringCompletenessCloseoutSummary,
        build_tassadar_turing_completeness_closeout_summary, read_repo_json,
        tassadar_turing_completeness_closeout_summary_path,
    };

    #[test]
    fn turing_completeness_closeout_summary_keeps_served_blockers_explicit() {
        let summary = build_tassadar_turing_completeness_closeout_summary().expect("summary");

        assert!(
            summary
                .served_blocked_by
                .contains(&String::from("named_served_universal_profile_missing"))
        );
        assert!(
            summary
                .explicit_non_implications
                .contains(&String::from("broad served internal compute"))
        );
    }

    #[test]
    fn turing_completeness_closeout_summary_matches_committed_truth() {
        let generated = build_tassadar_turing_completeness_closeout_summary().expect("summary");
        let committed: TassadarTuringCompletenessCloseoutSummary =
            read_repo_json(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_turing_completeness_closeout_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_turing_completeness_closeout_summary.json")
        );
    }
}
