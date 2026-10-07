use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
    TassadarRustOnlyArticleAcceptanceGateV2Report,
};

pub const TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_only_article_acceptance_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustOnlyArticleAcceptanceSummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub gate_report_ref: String,
    pub gate_report: TassadarRustOnlyArticleAcceptanceGateV2Report,
    pub failed_prerequisite_ids: Vec<String>,
    pub green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarRustOnlyArticleAcceptanceSummaryReport {
    fn new(gate_report: TassadarRustOnlyArticleAcceptanceGateV2Report) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.rust_only_article_acceptance.summary.v1"),
            gate_report_ref: String::from(TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF),
            failed_prerequisite_ids: gate_report.failed_prerequisite_ids.clone(),
            green: gate_report.green,
            gate_report,
            claim_boundary: String::from(
                "this summary mirrors the Rust-only article acceptance gate v2 only. It is an operator-facing summary of the same prerequisite set, not an expansion of the claim boundary",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Rust-only article acceptance summary now records green={} with failed_prerequisites={}.",
            report.green,
            report.failed_prerequisite_ids.len(),
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_rust_only_article_acceptance_summary_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarRustOnlyArticleAcceptanceSummaryError {
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

pub fn build_tassadar_rust_only_article_acceptance_summary_report(
) -> Result<TassadarRustOnlyArticleAcceptanceSummaryReport, TassadarRustOnlyArticleAcceptanceSummaryError>
{
    let gate_report: TassadarRustOnlyArticleAcceptanceGateV2Report = read_repo_json(
        TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
        "rust_only_article_acceptance_gate_v2",
    )?;
    Ok(TassadarRustOnlyArticleAcceptanceSummaryReport::new(gate_report))
}

pub fn tassadar_rust_only_article_acceptance_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_rust_only_article_acceptance_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarRustOnlyArticleAcceptanceSummaryReport, TassadarRustOnlyArticleAcceptanceSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRustOnlyArticleAcceptanceSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_rust_only_article_acceptance_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarRustOnlyArticleAcceptanceSummaryError::Write {
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
) -> Result<T, TassadarRustOnlyArticleAcceptanceSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarRustOnlyArticleAcceptanceSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarRustOnlyArticleAcceptanceSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_SUMMARY_REPORT_REF,
        TassadarRustOnlyArticleAcceptanceSummaryReport,
        build_tassadar_rust_only_article_acceptance_summary_report, read_repo_json,
        tassadar_rust_only_article_acceptance_summary_report_path,
        write_tassadar_rust_only_article_acceptance_summary_report,
    };

    #[test]
    fn rust_only_article_acceptance_summary_is_green_when_gate_is_green() {
        let report = build_tassadar_rust_only_article_acceptance_summary_report().expect("summary");

        assert!(report.green);
        assert!(report.failed_prerequisite_ids.is_empty());
    }

    #[test]
    fn rust_only_article_acceptance_summary_matches_committed_truth() {
        let generated =
            build_tassadar_rust_only_article_acceptance_summary_report().expect("summary");
        let committed: TassadarRustOnlyArticleAcceptanceSummaryReport = read_repo_json(
            TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_SUMMARY_REPORT_REF,
            "rust_only_article_acceptance_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_rust_only_article_acceptance_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_rust_only_article_acceptance_summary.json");
        let written = write_tassadar_rust_only_article_acceptance_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarRustOnlyArticleAcceptanceSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_rust_only_article_acceptance_summary_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_rust_only_article_acceptance_summary.json")
        );
    }
}
