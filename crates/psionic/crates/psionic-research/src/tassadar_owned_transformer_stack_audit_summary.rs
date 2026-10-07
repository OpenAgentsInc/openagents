use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarOwnedTransformerStackAuditReport, TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_REPORT_REF,
};

pub const TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_owned_transformer_stack_audit_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOwnedTransformerStackAuditSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub audit_report_ref: String,
    pub audit_report: TassadarOwnedTransformerStackAuditReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub owned_stack_backed_surface_count: usize,
    pub fixture_backed_surface_count: usize,
    pub research_only_surface_count: usize,
    pub substrate_only_surface_count: usize,
    pub remaining_blocker_count: usize,
    pub actual_owned_transformer_stack_exists: bool,
    pub owned_transformer_stack_audit_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarOwnedTransformerStackAuditSummary {
    fn new(audit_report: TassadarOwnedTransformerStackAuditReport) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.owned_transformer_stack_audit.summary.v1"),
            audit_report_ref: String::from(TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_REPORT_REF),
            tied_requirement_id: audit_report
                .acceptance_gate_tie
                .tied_requirement_id
                .clone(),
            tied_requirement_satisfied: audit_report
                .acceptance_gate_tie
                .tied_requirement_satisfied,
            acceptance_status: format!("{:?}", audit_report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            owned_stack_backed_surface_count: audit_report.owned_stack_backed_surface_count,
            fixture_backed_surface_count: audit_report.fixture_backed_surface_count,
            research_only_surface_count: audit_report.research_only_surface_count,
            substrate_only_surface_count: audit_report.substrate_only_surface_count,
            remaining_blocker_count: audit_report.remaining_blocker_count,
            actual_owned_transformer_stack_exists: audit_report.actual_owned_transformer_stack_exists,
            owned_transformer_stack_audit_green: audit_report.owned_transformer_stack_audit_green,
            article_equivalence_green: audit_report.article_equivalence_green,
            audit_report,
            claim_boundary: String::from(
                "this summary mirrors the owned Transformer stack audit only. It keeps the extracted-stack boundary operator-readable without widening the underlying article-equivalence claim boundary.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Owned Transformer stack audit summary now records owned_stack_backed_surface_count={}, fixture_backed_surface_count={}, research_only_surface_count={}, substrate_only_surface_count={}, remaining_blocker_count={}, actual_owned_transformer_stack_exists={}, owned_transformer_stack_audit_green={}, and article_equivalence_green={}.",
            report.owned_stack_backed_surface_count,
            report.fixture_backed_surface_count,
            report.research_only_surface_count,
            report.substrate_only_surface_count,
            report.remaining_blocker_count,
            report.actual_owned_transformer_stack_exists,
            report.owned_transformer_stack_audit_green,
            report.article_equivalence_green,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_owned_transformer_stack_audit_summary|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarOwnedTransformerStackAuditSummaryError {
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

pub fn build_tassadar_owned_transformer_stack_audit_summary(
) -> Result<TassadarOwnedTransformerStackAuditSummary, TassadarOwnedTransformerStackAuditSummaryError>
{
    let audit_report: TassadarOwnedTransformerStackAuditReport = read_repo_json(
        TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_REPORT_REF,
        "owned_transformer_stack_audit",
    )?;
    Ok(TassadarOwnedTransformerStackAuditSummary::new(audit_report))
}

pub fn tassadar_owned_transformer_stack_audit_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_owned_transformer_stack_audit_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarOwnedTransformerStackAuditSummary, TassadarOwnedTransformerStackAuditSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarOwnedTransformerStackAuditSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_owned_transformer_stack_audit_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarOwnedTransformerStackAuditSummaryError::Write {
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
) -> Result<T, TassadarOwnedTransformerStackAuditSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarOwnedTransformerStackAuditSummaryError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarOwnedTransformerStackAuditSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_owned_transformer_stack_audit_summary, read_repo_json,
        tassadar_owned_transformer_stack_audit_summary_path,
        write_tassadar_owned_transformer_stack_audit_summary,
        TassadarOwnedTransformerStackAuditSummary,
        TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_SUMMARY_REPORT_REF,
    };

    #[test]
    fn owned_transformer_stack_audit_summary_tracks_green_audit_without_final_green() {
        let report = build_tassadar_owned_transformer_stack_audit_summary().expect("summary");

        assert!(report.tied_requirement_satisfied);
        assert_eq!(report.acceptance_status, "green");
        assert_eq!(report.owned_stack_backed_surface_count, 4);
        assert_eq!(report.fixture_backed_surface_count, 1);
        assert_eq!(report.research_only_surface_count, 1);
        assert_eq!(report.substrate_only_surface_count, 3);
        assert_eq!(report.remaining_blocker_count, 7);
        assert!(report.actual_owned_transformer_stack_exists);
        assert!(report.owned_transformer_stack_audit_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn owned_transformer_stack_audit_summary_matches_committed_truth() {
        let generated = build_tassadar_owned_transformer_stack_audit_summary().expect("summary");
        let committed: TassadarOwnedTransformerStackAuditSummary = read_repo_json(
            TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_SUMMARY_REPORT_REF,
            "owned_transformer_stack_audit_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_owned_transformer_stack_audit_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_owned_transformer_stack_audit_summary.json");
        let written = write_tassadar_owned_transformer_stack_audit_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarOwnedTransformerStackAuditSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_owned_transformer_stack_audit_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_owned_transformer_stack_audit_summary.json")
        );
    }
}
