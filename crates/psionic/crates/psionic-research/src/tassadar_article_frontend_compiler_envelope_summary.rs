use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF,
    TassadarArticleFrontendCompilerEnvelopeReport,
};

pub const TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_frontend_compiler_envelope_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCompilerEnvelopeSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleFrontendCompilerEnvelopeReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub admitted_case_green_count: usize,
    pub refusal_probe_green_count: usize,
    pub toolchain_identity_green: bool,
    pub refusal_taxonomy_green: bool,
    pub envelope_manifest_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleFrontendCompilerEnvelopeSummary {
    fn new(report: TassadarArticleFrontendCompilerEnvelopeReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_frontend_compiler_envelope.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            admitted_case_green_count: report.admitted_case_green_count,
            refusal_probe_green_count: report.refusal_probe_green_count,
            toolchain_identity_green: report.toolchain_identity_green,
            refusal_taxonomy_green: report.refusal_taxonomy_green,
            envelope_manifest_green: report.envelope_manifest_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-176 declared frontend/compiler envelope tranche. It keeps the bounded admitted Rust source set, refusal taxonomy, and toolchain checks operator-readable without pretending that the later corpus, demo, or final article-equivalence tranches are already closed.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article frontend/compiler envelope summary now records tied_requirement_satisfied={}, admitted_cases_green={}, refusal_probes_green={}, toolchain_identity_green={}, refusal_taxonomy_green={}, envelope_manifest_green={}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.admitted_case_green_count,
            summary.refusal_probe_green_count,
            summary.toolchain_identity_green,
            summary.refusal_taxonomy_green,
            summary.envelope_manifest_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_frontend_compiler_envelope_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleFrontendCompilerEnvelopeSummaryError {
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

pub fn build_tassadar_article_frontend_compiler_envelope_summary() -> Result<
    TassadarArticleFrontendCompilerEnvelopeSummary,
    TassadarArticleFrontendCompilerEnvelopeSummaryError,
> {
    let report: TassadarArticleFrontendCompilerEnvelopeReport = read_repo_json(
        TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF,
        "article_frontend_compiler_envelope_report",
    )?;
    Ok(TassadarArticleFrontendCompilerEnvelopeSummary::new(report))
}

pub fn tassadar_article_frontend_compiler_envelope_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_frontend_compiler_envelope_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFrontendCompilerEnvelopeSummary,
    TassadarArticleFrontendCompilerEnvelopeSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFrontendCompilerEnvelopeSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_frontend_compiler_envelope_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFrontendCompilerEnvelopeSummaryError::Write {
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
) -> Result<T, TassadarArticleFrontendCompilerEnvelopeSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFrontendCompilerEnvelopeSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFrontendCompilerEnvelopeSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_SUMMARY_REPORT_REF,
        TassadarArticleFrontendCompilerEnvelopeSummary,
        build_tassadar_article_frontend_compiler_envelope_summary, read_repo_json,
        tassadar_article_frontend_compiler_envelope_summary_path,
        write_tassadar_article_frontend_compiler_envelope_summary,
    };

    #[test]
    fn frontend_compiler_envelope_summary_tracks_bounded_green_without_final_green() {
        let summary = build_tassadar_article_frontend_compiler_envelope_summary().expect("summary");

        assert_eq!(summary.tied_requirement_id, "TAS-176");
        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.admitted_case_green_count, 8);
        assert_eq!(summary.refusal_probe_green_count, 6);
        assert!(summary.toolchain_identity_green);
        assert!(summary.refusal_taxonomy_green);
        assert!(summary.envelope_manifest_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn frontend_compiler_envelope_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_frontend_compiler_envelope_summary().expect("summary");
        let committed: TassadarArticleFrontendCompilerEnvelopeSummary = read_repo_json(
            TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_SUMMARY_REPORT_REF,
            "summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_frontend_compiler_envelope_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_frontend_compiler_envelope_summary.json");
        let written = write_tassadar_article_frontend_compiler_envelope_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleFrontendCompilerEnvelopeSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_frontend_compiler_envelope_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_frontend_compiler_envelope_summary.json")
        );
    }
}
