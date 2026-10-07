use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleTraceVocabularyBindingReport,
    TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_trace_vocabulary_binding_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTraceVocabularyBindingSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub binding_report_ref: String,
    pub binding_report: TassadarArticleTraceVocabularyBindingReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub channel_binding_row_count: usize,
    pub source_vocab_compatible: bool,
    pub target_vocab_compatible: bool,
    pub prompt_trace_boundary_supported: bool,
    pub halt_boundary_supported: bool,
    pub all_required_channels_bound: bool,
    pub roundtrip_exact: bool,
    pub article_trace_vocabulary_binding_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleTraceVocabularyBindingSummary {
    fn new(binding_report: TassadarArticleTraceVocabularyBindingReport) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_trace_vocabulary_binding.summary.v1"),
            binding_report_ref: String::from(TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF),
            tied_requirement_id: binding_report
                .acceptance_gate_tie
                .tied_requirement_id
                .clone(),
            tied_requirement_satisfied: binding_report
                .acceptance_gate_tie
                .tied_requirement_satisfied,
            acceptance_status: format!("{:?}", binding_report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            channel_binding_row_count: binding_report.trace_domain_binding.channel_binding_rows.len(),
            source_vocab_compatible: binding_report.trace_domain_binding.source_vocab_compatible,
            target_vocab_compatible: binding_report.trace_domain_binding.target_vocab_compatible,
            prompt_trace_boundary_supported: binding_report
                .trace_domain_binding
                .prompt_trace_boundary_supported,
            halt_boundary_supported: binding_report.trace_domain_binding.halt_boundary_supported,
            all_required_channels_bound: binding_report
                .trace_domain_binding
                .all_required_channels_bound,
            roundtrip_exact: binding_report.roundtrip.roundtrip_exact,
            article_trace_vocabulary_binding_green: binding_report
                .article_trace_vocabulary_binding_green,
            article_equivalence_green: binding_report.article_equivalence_green,
            binding_report,
            claim_boundary: String::from(
                "this summary mirrors the owned-route article trace vocabulary binding only. It keeps the tokenizer/schema/channel closure operator-readable without widening the underlying article-equivalence claim boundary.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Article trace vocabulary binding summary now records channel_binding_row_count={}, source_vocab_compatible={}, target_vocab_compatible={}, prompt_trace_boundary_supported={}, halt_boundary_supported={}, all_required_channels_bound={}, roundtrip_exact={}, article_trace_vocabulary_binding_green={}, and article_equivalence_green={}.",
            report.channel_binding_row_count,
            report.source_vocab_compatible,
            report.target_vocab_compatible,
            report.prompt_trace_boundary_supported,
            report.halt_boundary_supported,
            report.all_required_channels_bound,
            report.roundtrip_exact,
            report.article_trace_vocabulary_binding_green,
            report.article_equivalence_green,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_article_trace_vocabulary_binding_summary|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleTraceVocabularyBindingSummaryError {
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

pub fn build_tassadar_article_trace_vocabulary_binding_summary() -> Result<
    TassadarArticleTraceVocabularyBindingSummary,
    TassadarArticleTraceVocabularyBindingSummaryError,
> {
    let binding_report: TassadarArticleTraceVocabularyBindingReport = read_repo_json(
        TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF,
        "article_trace_vocabulary_binding",
    )?;
    Ok(TassadarArticleTraceVocabularyBindingSummary::new(
        binding_report,
    ))
}

pub fn tassadar_article_trace_vocabulary_binding_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_trace_vocabulary_binding_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTraceVocabularyBindingSummary,
    TassadarArticleTraceVocabularyBindingSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTraceVocabularyBindingSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_trace_vocabulary_binding_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTraceVocabularyBindingSummaryError::Write {
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
) -> Result<T, TassadarArticleTraceVocabularyBindingSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleTraceVocabularyBindingSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTraceVocabularyBindingSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_trace_vocabulary_binding_summary, read_repo_json,
        write_tassadar_article_trace_vocabulary_binding_summary,
        TassadarArticleTraceVocabularyBindingSummary,
        TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_trace_vocabulary_binding_summary_tracks_green_binding_without_final_green() {
        let report =
            build_tassadar_article_trace_vocabulary_binding_summary().expect("binding summary");

        assert!(report.tied_requirement_satisfied);
        assert_eq!(report.acceptance_status, "green");
        assert_eq!(report.channel_binding_row_count, 14);
        assert!(report.source_vocab_compatible);
        assert!(report.target_vocab_compatible);
        assert!(report.prompt_trace_boundary_supported);
        assert!(report.halt_boundary_supported);
        assert!(report.all_required_channels_bound);
        assert!(report.roundtrip_exact);
        assert!(report.article_trace_vocabulary_binding_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_trace_vocabulary_binding_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_trace_vocabulary_binding_summary().expect("binding summary");
        let committed: TassadarArticleTraceVocabularyBindingSummary = read_repo_json(
            TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_SUMMARY_REPORT_REF,
            "article_trace_vocabulary_binding_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_trace_vocabulary_binding_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_trace_vocabulary_binding_summary.json");
        let written = write_tassadar_article_trace_vocabulary_binding_summary(&output_path)
            .expect("written summary");

        assert_eq!(
            written,
            read_repo_json(
                TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_SUMMARY_REPORT_REF,
                "article_trace_vocabulary_binding_summary",
            )
            .expect("committed summary")
        );
        assert_eq!(
            output_path.file_name().and_then(|value| value.to_str()),
            Some("tassadar_article_trace_vocabulary_binding_summary.json")
        );
        assert_eq!(
            TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_SUMMARY_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_article_trace_vocabulary_binding_summary.json"
        );
    }
}
