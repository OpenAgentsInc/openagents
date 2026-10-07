use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarCanonicalTransformerStackBoundaryReport,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_canonical_transformer_stack_boundary_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCanonicalTransformerStackBoundarySummary {
    pub schema_version: u16,
    pub report_id: String,
    pub boundary_report_ref: String,
    pub boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub boundary_doc_ref: String,
    pub interface_count: usize,
    pub dependency_check_count: usize,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub boundary_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarCanonicalTransformerStackBoundarySummary {
    fn new(boundary_report: TassadarCanonicalTransformerStackBoundaryReport) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.canonical_transformer_stack_boundary.summary.v1"),
            boundary_report_ref: String::from(TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF),
            boundary_doc_ref: boundary_report.boundary_doc_ref.clone(),
            interface_count: boundary_report.interface_rows.len(),
            dependency_check_count: boundary_report.dependency_checks.len(),
            tied_requirement_id: boundary_report
                .acceptance_gate_tie
                .tied_requirement_id
                .clone(),
            tied_requirement_satisfied: boundary_report
                .acceptance_gate_tie
                .tied_requirement_satisfied,
            acceptance_status: format!("{:?}", boundary_report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            boundary_contract_green: boundary_report.boundary_contract_green,
            article_equivalence_green: boundary_report.article_equivalence_green,
            boundary_report,
            claim_boundary: String::from(
                "this summary mirrors the canonical Transformer stack boundary only. It keeps the crate/interface split operator-readable, but it does not widen the current public article-equivalence claim boundary beyond the underlying boundary report",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Canonical Transformer stack boundary summary now records interface_count={}, dependency_check_count={}, tied_requirement_satisfied={}, boundary_contract_green={}, and article_equivalence_green={}.",
            report.interface_count,
            report.dependency_check_count,
            report.tied_requirement_satisfied,
            report.boundary_contract_green,
            report.article_equivalence_green,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_canonical_transformer_stack_boundary_summary|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarCanonicalTransformerStackBoundarySummaryError {
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

pub fn build_tassadar_canonical_transformer_stack_boundary_summary() -> Result<
    TassadarCanonicalTransformerStackBoundarySummary,
    TassadarCanonicalTransformerStackBoundarySummaryError,
> {
    let boundary_report: TassadarCanonicalTransformerStackBoundaryReport = read_repo_json(
        TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        "canonical_transformer_stack_boundary",
    )?;
    Ok(TassadarCanonicalTransformerStackBoundarySummary::new(
        boundary_report,
    ))
}

pub fn tassadar_canonical_transformer_stack_boundary_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_canonical_transformer_stack_boundary_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarCanonicalTransformerStackBoundarySummary,
    TassadarCanonicalTransformerStackBoundarySummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCanonicalTransformerStackBoundarySummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_canonical_transformer_stack_boundary_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCanonicalTransformerStackBoundarySummaryError::Write {
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
) -> Result<T, TassadarCanonicalTransformerStackBoundarySummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarCanonicalTransformerStackBoundarySummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCanonicalTransformerStackBoundarySummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_canonical_transformer_stack_boundary_summary, read_repo_json,
        tassadar_canonical_transformer_stack_boundary_summary_path,
        write_tassadar_canonical_transformer_stack_boundary_summary,
        TassadarCanonicalTransformerStackBoundarySummary,
        TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_SUMMARY_REPORT_REF,
    };

    #[test]
    fn canonical_transformer_stack_boundary_summary_tracks_gate_tie_without_final_green() {
        let report =
            build_tassadar_canonical_transformer_stack_boundary_summary().expect("summary");

        assert!(report.tied_requirement_satisfied);
        assert_eq!(report.acceptance_status, "green");
        assert!(report.boundary_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.interface_count, 5);
        assert_eq!(report.dependency_check_count, 6);
    }

    #[test]
    fn canonical_transformer_stack_boundary_summary_matches_committed_truth() {
        let generated =
            build_tassadar_canonical_transformer_stack_boundary_summary().expect("summary");
        let committed: TassadarCanonicalTransformerStackBoundarySummary = read_repo_json(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_SUMMARY_REPORT_REF,
            "canonical_transformer_stack_boundary_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_canonical_transformer_stack_boundary_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_canonical_transformer_stack_boundary_summary.json");
        let written = write_tassadar_canonical_transformer_stack_boundary_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarCanonicalTransformerStackBoundarySummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_canonical_transformer_stack_boundary_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_canonical_transformer_stack_boundary_summary.json")
        );
    }
}
