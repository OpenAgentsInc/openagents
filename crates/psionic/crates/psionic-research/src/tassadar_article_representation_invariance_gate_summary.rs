use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleRepresentationInvarianceGateReport,
    TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_representation_invariance_gate_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRepresentationInvarianceGateSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub invariance_gate_report_ref: String,
    pub invariance_gate_report: TassadarArticleRepresentationInvarianceGateReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub case_count: usize,
    pub suppressed_case_count: usize,
    pub exact_trace_case_count: usize,
    pub canonicalized_trace_case_count: usize,
    pub output_stable_case_count: usize,
    pub tokenizer_roundtrip_exact_case_count: usize,
    pub model_binding_roundtrip_exact_case_count: usize,
    pub article_representation_invariance_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleRepresentationInvarianceGateSummary {
    fn new(invariance_gate_report: TassadarArticleRepresentationInvarianceGateReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_representation_invariance_gate.summary.v1"),
            invariance_gate_report_ref: String::from(
                TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
            ),
            tied_requirement_id: invariance_gate_report
                .acceptance_gate_tie
                .tied_requirement_id
                .clone(),
            tied_requirement_satisfied: invariance_gate_report
                .acceptance_gate_tie
                .tied_requirement_satisfied,
            acceptance_status: format!(
                "{:?}",
                invariance_gate_report.acceptance_gate_tie.acceptance_status
            )
            .to_lowercase(),
            case_count: invariance_gate_report
                .representation_equivalence_review
                .case_count,
            suppressed_case_count: invariance_gate_report
                .representation_equivalence_review
                .suppressed_case_count,
            exact_trace_case_count: invariance_gate_report
                .trace_stability_review
                .exact_trace_case_count,
            canonicalized_trace_case_count: invariance_gate_report
                .trace_stability_review
                .canonicalized_trace_case_count,
            output_stable_case_count: invariance_gate_report
                .representation_equivalence_review
                .output_stable_case_count,
            tokenizer_roundtrip_exact_case_count: invariance_gate_report
                .representation_equivalence_review
                .tokenizer_roundtrip_exact_case_count,
            model_binding_roundtrip_exact_case_count: invariance_gate_report
                .representation_equivalence_review
                .model_binding_roundtrip_exact_case_count,
            article_representation_invariance_green: invariance_gate_report
                .article_representation_invariance_green,
            article_equivalence_green: invariance_gate_report.article_equivalence_green,
            invariance_gate_report,
            claim_boundary: String::from(
                "this summary mirrors the owned-route prompt, tokenization, and representation invariance gate only. It keeps the exact-trace versus canonicalized-equivalence split operator-readable without widening the underlying article-equivalence claim boundary.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article representation invariance summary now records case_count={}, suppressed_case_count={}, exact_trace_case_count={}, canonicalized_trace_case_count={}, output_stable_case_count={}, tokenizer_roundtrip_exact_case_count={}, model_binding_roundtrip_exact_case_count={}, article_representation_invariance_green={}, and article_equivalence_green={}.",
            summary.case_count,
            summary.suppressed_case_count,
            summary.exact_trace_case_count,
            summary.canonicalized_trace_case_count,
            summary.output_stable_case_count,
            summary.tokenizer_roundtrip_exact_case_count,
            summary.model_binding_roundtrip_exact_case_count,
            summary.article_representation_invariance_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_representation_invariance_gate_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleRepresentationInvarianceGateSummaryError {
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

pub fn build_tassadar_article_representation_invariance_gate_summary() -> Result<
    TassadarArticleRepresentationInvarianceGateSummary,
    TassadarArticleRepresentationInvarianceGateSummaryError,
> {
    let report: TassadarArticleRepresentationInvarianceGateReport = read_repo_json(
        TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
        "article_representation_invariance_gate",
    )?;
    Ok(TassadarArticleRepresentationInvarianceGateSummary::new(
        report,
    ))
}

pub fn tassadar_article_representation_invariance_gate_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_representation_invariance_gate_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleRepresentationInvarianceGateSummary,
    TassadarArticleRepresentationInvarianceGateSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleRepresentationInvarianceGateSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_representation_invariance_gate_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleRepresentationInvarianceGateSummaryError::Write {
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
) -> Result<T, TassadarArticleRepresentationInvarianceGateSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleRepresentationInvarianceGateSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleRepresentationInvarianceGateSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_representation_invariance_gate_summary, read_repo_json,
        write_tassadar_article_representation_invariance_gate_summary,
        TassadarArticleRepresentationInvarianceGateSummary,
        TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_representation_invariance_summary_tracks_green_gate_without_final_green() {
        let summary = build_tassadar_article_representation_invariance_gate_summary()
            .expect("representation invariance summary");

        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.acceptance_status, "green");
        assert!(summary.case_count > 0);
        assert!(summary.suppressed_case_count > 0);
        assert!(summary.exact_trace_case_count > 0);
        assert!(summary.canonicalized_trace_case_count > 0);
        assert!(summary.article_representation_invariance_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn article_representation_invariance_summary_matches_committed_truth() {
        let generated = build_tassadar_article_representation_invariance_gate_summary()
            .expect("representation invariance summary");
        let committed: TassadarArticleRepresentationInvarianceGateSummary = read_repo_json(
            TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_SUMMARY_REPORT_REF,
            "article_representation_invariance_gate_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_representation_invariance_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_representation_invariance_gate_summary.json");
        let written = write_tassadar_article_representation_invariance_gate_summary(&output_path)
            .expect("written summary");

        assert_eq!(
            written,
            read_repo_json(
                TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_SUMMARY_REPORT_REF,
                "article_representation_invariance_gate_summary",
            )
            .expect("committed summary")
        );
        assert_eq!(
            output_path.file_name().and_then(|value| value.to_str()),
            Some("tassadar_article_representation_invariance_gate_summary.json")
        );
    }
}
