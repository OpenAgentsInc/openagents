use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_REPORT_REF,
    TassadarRustOnlyArticleCloseoutAuditReport,
};

pub const TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_only_article_closeout_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustOnlyArticleCloseoutSummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub audit_report_ref: String,
    pub audit_report: TassadarRustOnlyArticleCloseoutAuditReport,
    pub green: bool,
    pub reproduced_claim: String,
    pub remaining_exclusions: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarRustOnlyArticleCloseoutSummaryReport {
    fn new(audit_report: TassadarRustOnlyArticleCloseoutAuditReport) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.rust_only_article_closeout.summary.v1"),
            audit_report_ref: String::from(TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_REPORT_REF),
            green: audit_report.green,
            reproduced_claim: audit_report.reproduced_claim.clone(),
            remaining_exclusions: audit_report.remaining_exclusions.clone(),
            audit_report,
            claim_boundary: String::from(
                "this summary mirrors the final Rust-only article closeout audit only. It does not add new evidence or widen the published claim boundary beyond the audit itself",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Rust-only article closeout summary now records green={} with remaining_exclusions={}.",
            report.green,
            report.remaining_exclusions.len(),
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_rust_only_article_closeout_summary_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarRustOnlyArticleCloseoutSummaryError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_rust_only_article_closeout_summary_report()
-> Result<TassadarRustOnlyArticleCloseoutSummaryReport, TassadarRustOnlyArticleCloseoutSummaryError>
{
    let audit_report: TassadarRustOnlyArticleCloseoutAuditReport = read_repo_json(
        TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_REPORT_REF,
        "rust_only_article_closeout_audit",
    )?;
    Ok(TassadarRustOnlyArticleCloseoutSummaryReport::new(
        audit_report,
    ))
}

pub fn tassadar_rust_only_article_closeout_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_rust_only_article_closeout_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarRustOnlyArticleCloseoutSummaryReport, TassadarRustOnlyArticleCloseoutSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRustOnlyArticleCloseoutSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_rust_only_article_closeout_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarRustOnlyArticleCloseoutSummaryError::Write {
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
        .expect("psionic-research should live under <repo>/crates/psionic-research")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarRustOnlyArticleCloseoutSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarRustOnlyArticleCloseoutSummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarRustOnlyArticleCloseoutSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_SUMMARY_REPORT_REF,
        TassadarRustOnlyArticleCloseoutSummaryReport,
        build_tassadar_rust_only_article_closeout_summary_report, read_repo_json,
        tassadar_rust_only_article_closeout_summary_report_path,
        write_tassadar_rust_only_article_closeout_summary_report,
    };

    #[test]
    fn rust_only_article_closeout_summary_is_green_when_audit_is_green() {
        let report = build_tassadar_rust_only_article_closeout_summary_report().expect("summary");

        assert!(report.green);
        assert!(report.audit_report.all_surfaces_green);
        assert!(!report.reproduced_claim.is_empty());
    }

    #[test]
    fn rust_only_article_closeout_summary_matches_committed_truth() {
        let generated =
            build_tassadar_rust_only_article_closeout_summary_report().expect("summary");
        let committed: TassadarRustOnlyArticleCloseoutSummaryReport = read_repo_json(
            TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_SUMMARY_REPORT_REF,
            "rust_only_article_closeout_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_rust_only_article_closeout_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_rust_only_article_closeout_summary.json");
        let written = write_tassadar_rust_only_article_closeout_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarRustOnlyArticleCloseoutSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_rust_only_article_closeout_summary_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_rust_only_article_closeout_summary.json")
        );
    }
}
