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
    TassadarSharedStateConcurrencyVerdictReport, TassadarSharedStateConcurrencyVerdictReportError,
    build_tassadar_shared_state_concurrency_verdict_report,
};

pub const TASSADAR_SHARED_STATE_CONCURRENCY_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_shared_state_concurrency_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedStateConcurrencySummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarSharedStateConcurrencyVerdictReport,
    pub operator_green_class_ids: Vec<String>,
    pub public_suppressed_profile_ids: Vec<String>,
    pub refused_class_ids: Vec<String>,
    pub allowed_statement: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSharedStateConcurrencySummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarSharedStateConcurrencyVerdictReportError),
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

pub fn build_tassadar_shared_state_concurrency_summary_report()
-> Result<TassadarSharedStateConcurrencySummaryReport, TassadarSharedStateConcurrencySummaryError> {
    let eval_report = build_tassadar_shared_state_concurrency_verdict_report();
    let mut report = TassadarSharedStateConcurrencySummaryReport {
        schema_version: 1,
        report_id: String::from("tassadar.shared_state_concurrency.summary.v1"),
        operator_green_class_ids: eval_report.operator_green_class_ids.clone(),
        public_suppressed_profile_ids: eval_report.operator_profile_allowed_profile_ids.clone(),
        refused_class_ids: eval_report.refused_class_ids.clone(),
        allowed_statement: String::from(
            "Psionic/Tassadar has one operator-truth shared-state concurrency lane for explicit single-host deterministic scheduler classes, but public shared-state publication remains suppressed and broader concurrency classes stay refused.",
        ),
        eval_report,
        claim_boundary: String::from(
            "this summary keeps shared-state concurrency verdicts disclosure-safe. It highlights the narrow operator-green classes and the still-refused families instead of widening concurrency into public served or market posture",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Shared-state concurrency summary keeps operator_green_classes={}, refused_classes={}, public_profiles={}, overall_green={}.",
        report.operator_green_class_ids.len(),
        report.refused_class_ids.len(),
        report.public_suppressed_profile_ids.len(),
        report.eval_report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_shared_state_concurrency_summary_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_shared_state_concurrency_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SHARED_STATE_CONCURRENCY_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_shared_state_concurrency_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSharedStateConcurrencySummaryReport, TassadarSharedStateConcurrencySummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSharedStateConcurrencySummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_shared_state_concurrency_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSharedStateConcurrencySummaryError::Write {
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

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarSharedStateConcurrencySummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarSharedStateConcurrencySummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSharedStateConcurrencySummaryError::Deserialize {
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
        TASSADAR_SHARED_STATE_CONCURRENCY_SUMMARY_REPORT_REF,
        TassadarSharedStateConcurrencySummaryReport,
        build_tassadar_shared_state_concurrency_summary_report, read_repo_json,
        tassadar_shared_state_concurrency_summary_report_path,
        write_tassadar_shared_state_concurrency_summary_report,
    };

    #[test]
    fn shared_state_concurrency_summary_keeps_operator_scope_and_refusals_explicit() {
        let report = build_tassadar_shared_state_concurrency_summary_report().expect("summary");

        assert!(report.eval_report.overall_green);
        assert_eq!(report.operator_green_class_ids.len(), 2);
        assert_eq!(report.refused_class_ids.len(), 3);
        assert_eq!(report.public_suppressed_profile_ids.len(), 1);
    }

    #[test]
    fn shared_state_concurrency_summary_matches_committed_truth() {
        let generated = build_tassadar_shared_state_concurrency_summary_report().expect("summary");
        let committed: TassadarSharedStateConcurrencySummaryReport =
            read_repo_json(TASSADAR_SHARED_STATE_CONCURRENCY_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_shared_state_concurrency_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_shared_state_concurrency_summary.json");
        let written = write_tassadar_shared_state_concurrency_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarSharedStateConcurrencySummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_shared_state_concurrency_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_shared_state_concurrency_summary.json")
        );
    }
}
