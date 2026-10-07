use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF, TassadarArticleDemoFrontendParityReport,
};

pub const TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_demo_frontend_parity_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoFrontendParitySummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleDemoFrontendParityReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub compiled_demo_count: usize,
    pub green_demo_count: usize,
    pub refusal_probe_green_count: usize,
    pub source_compile_receipt_parity_green: bool,
    pub workload_identity_parity_green: bool,
    pub unsupported_variant_refusal_green: bool,
    pub demo_frontend_parity_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleDemoFrontendParitySummary {
    fn new(report: TassadarArticleDemoFrontendParityReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_demo_frontend_parity.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            compiled_demo_count: report.compiled_demo_count,
            green_demo_count: report.green_demo_count,
            refusal_probe_green_count: report.refusal_probe_green_count,
            source_compile_receipt_parity_green: report.source_compile_receipt_parity_green,
            workload_identity_parity_green: report.workload_identity_parity_green,
            unsupported_variant_refusal_green: report.unsupported_variant_refusal_green,
            demo_frontend_parity_green: report.demo_frontend_parity_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-178 article-demo frontend parity tranche. It keeps the Hungarian and Sudoku demo-source closure plus explicit unsupported-variant refusals operator-readable without pretending that later interpreter-breadth, benchmark-wide, or final article-equivalence tranches are already green.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article demo frontend parity summary now records tied_requirement_satisfied={}, green_demos={}/{}, refusal_probes_green={}, source_compile_receipt_parity_green={}, workload_identity_parity_green={}, unsupported_variant_refusal_green={}, demo_frontend_parity_green={}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.green_demo_count,
            summary.compiled_demo_count,
            summary.refusal_probe_green_count,
            summary.source_compile_receipt_parity_green,
            summary.workload_identity_parity_green,
            summary.unsupported_variant_refusal_green,
            summary.demo_frontend_parity_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_demo_frontend_parity_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleDemoFrontendParitySummaryError {
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

pub fn build_tassadar_article_demo_frontend_parity_summary() -> Result<
    TassadarArticleDemoFrontendParitySummary,
    TassadarArticleDemoFrontendParitySummaryError,
> {
    let report: TassadarArticleDemoFrontendParityReport = read_repo_json(
        TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF,
        "article_demo_frontend_parity_report",
    )?;
    Ok(TassadarArticleDemoFrontendParitySummary::new(report))
}

pub fn tassadar_article_demo_frontend_parity_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_demo_frontend_parity_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleDemoFrontendParitySummary,
    TassadarArticleDemoFrontendParitySummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleDemoFrontendParitySummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_demo_frontend_parity_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleDemoFrontendParitySummaryError::Write {
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
) -> Result<T, TassadarArticleDemoFrontendParitySummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleDemoFrontendParitySummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleDemoFrontendParitySummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_SUMMARY_REPORT_REF,
        TassadarArticleDemoFrontendParitySummary,
        build_tassadar_article_demo_frontend_parity_summary, read_repo_json,
        tassadar_article_demo_frontend_parity_summary_path,
        write_tassadar_article_demo_frontend_parity_summary,
    };

    #[test]
    fn article_demo_frontend_parity_summary_tracks_demo_closure_without_final_green() {
        let summary = build_tassadar_article_demo_frontend_parity_summary().expect("summary");

        assert_eq!(summary.tied_requirement_id, "TAS-178");
        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.compiled_demo_count, 2);
        assert_eq!(summary.green_demo_count, 2);
        assert_eq!(summary.refusal_probe_green_count, 2);
        assert!(summary.source_compile_receipt_parity_green);
        assert!(summary.workload_identity_parity_green);
        assert!(summary.unsupported_variant_refusal_green);
        assert!(summary.demo_frontend_parity_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn article_demo_frontend_parity_summary_matches_committed_truth() {
        let generated = build_tassadar_article_demo_frontend_parity_summary().expect("summary");
        let committed: TassadarArticleDemoFrontendParitySummary = read_repo_json(
            TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_SUMMARY_REPORT_REF,
            "summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_demo_frontend_parity_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_demo_frontend_parity_summary.json");
        let written = write_tassadar_article_demo_frontend_parity_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleDemoFrontendParitySummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_demo_frontend_parity_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_demo_frontend_parity_summary.json")
        );
    }
}
