use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleInterpreterBreadthEnvelopeReport,
    TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_interpreter_breadth_envelope_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthEnvelopeSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleInterpreterBreadthEnvelopeReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub current_floor_green_count: usize,
    pub declared_required_family_green_count: usize,
    pub research_only_family_green_count: usize,
    pub explicit_out_of_envelope_green_count: usize,
    pub envelope_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleInterpreterBreadthEnvelopeSummary {
    fn new(report: TassadarArticleInterpreterBreadthEnvelopeReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_interpreter_breadth_envelope.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            acceptance_status: format!("{:?}", report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            current_floor_green_count: report.current_floor_green_count,
            declared_required_family_green_count: report.declared_required_family_green_count,
            research_only_family_green_count: report.research_only_family_green_count,
            explicit_out_of_envelope_green_count: report.explicit_out_of_envelope_green_count,
            envelope_contract_green: report.envelope_contract_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-179 declared interpreter-breadth envelope. It keeps the in-envelope, research-only, and out-of-envelope family split operator-readable without pretending that the later breadth suite or final article-equivalence gate are already closed.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article interpreter breadth envelope summary now records tied_requirement_satisfied={}, current_floor_green={}, declared_required_family_green={}, research_only_family_green={}, explicit_out_of_envelope_green={}, envelope_contract_green={}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.current_floor_green_count,
            summary.declared_required_family_green_count,
            summary.research_only_family_green_count,
            summary.explicit_out_of_envelope_green_count,
            summary.envelope_contract_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_interpreter_breadth_envelope_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleInterpreterBreadthEnvelopeSummaryError {
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

pub fn build_tassadar_article_interpreter_breadth_envelope_summary() -> Result<
    TassadarArticleInterpreterBreadthEnvelopeSummary,
    TassadarArticleInterpreterBreadthEnvelopeSummaryError,
> {
    let report: TassadarArticleInterpreterBreadthEnvelopeReport = read_repo_json(
        TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF,
        "article_interpreter_breadth_envelope_report",
    )?;
    Ok(TassadarArticleInterpreterBreadthEnvelopeSummary::new(
        report,
    ))
}

pub fn tassadar_article_interpreter_breadth_envelope_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_interpreter_breadth_envelope_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleInterpreterBreadthEnvelopeSummary,
    TassadarArticleInterpreterBreadthEnvelopeSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleInterpreterBreadthEnvelopeSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_interpreter_breadth_envelope_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleInterpreterBreadthEnvelopeSummaryError::Write {
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
) -> Result<T, TassadarArticleInterpreterBreadthEnvelopeSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleInterpreterBreadthEnvelopeSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleInterpreterBreadthEnvelopeSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_interpreter_breadth_envelope_summary, read_repo_json,
        tassadar_article_interpreter_breadth_envelope_summary_path,
        write_tassadar_article_interpreter_breadth_envelope_summary,
        TassadarArticleInterpreterBreadthEnvelopeSummary,
        TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_interpreter_breadth_envelope_summary_tracks_declared_envelope_without_final_green() {
        let summary =
            build_tassadar_article_interpreter_breadth_envelope_summary().expect("summary");

        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.current_floor_green_count, 2);
        assert_eq!(summary.declared_required_family_green_count, 3);
        assert_eq!(summary.research_only_family_green_count, 1);
        assert_eq!(summary.explicit_out_of_envelope_green_count, 7);
        assert!(summary.envelope_contract_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn article_interpreter_breadth_envelope_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_interpreter_breadth_envelope_summary().expect("summary");
        let committed: TassadarArticleInterpreterBreadthEnvelopeSummary = read_repo_json(
            TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_SUMMARY_REPORT_REF,
            "article_interpreter_breadth_envelope_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_interpreter_breadth_envelope_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_interpreter_breadth_envelope_summary.json");
        let written = write_tassadar_article_interpreter_breadth_envelope_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleInterpreterBreadthEnvelopeSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_interpreter_breadth_envelope_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_interpreter_breadth_envelope_summary.json")
        );
    }
}
