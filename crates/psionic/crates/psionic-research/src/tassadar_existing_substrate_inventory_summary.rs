use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarExistingSubstrateClassificationCount, TassadarExistingSubstrateInventoryReport,
    TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF,
};

pub const TASSADAR_EXISTING_SUBSTRATE_INVENTORY_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_existing_substrate_inventory_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExistingSubstrateInventorySummary {
    pub schema_version: u16,
    pub report_id: String,
    pub inventory_report_ref: String,
    pub inventory_report: TassadarExistingSubstrateInventoryReport,
    pub surface_count: usize,
    pub blocker_surface_count: usize,
    pub non_blocker_surface_count: usize,
    pub classification_counts: Vec<TassadarExistingSubstrateClassificationCount>,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub article_equivalence_green: bool,
    pub current_truth_boundary: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarExistingSubstrateInventorySummary {
    fn new(inventory_report: TassadarExistingSubstrateInventoryReport) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.existing_substrate_inventory.summary.v1"),
            inventory_report_ref: String::from(TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF),
            surface_count: inventory_report.surface_count,
            blocker_surface_count: inventory_report.blocker_surface_count,
            non_blocker_surface_count: inventory_report.non_blocker_surface_count,
            classification_counts: inventory_report.classification_counts.clone(),
            tied_requirement_id: inventory_report
                .acceptance_gate_tie
                .tied_requirement_id
                .clone(),
            tied_requirement_satisfied: inventory_report
                .acceptance_gate_tie
                .tied_requirement_satisfied,
            acceptance_status: format!("{:?}", inventory_report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            article_equivalence_green: inventory_report.article_equivalence_green,
            current_truth_boundary: inventory_report.current_truth_boundary.clone(),
            inventory_report,
            claim_boundary: String::from(
                "this summary mirrors the existing-substrate inventory only. It keeps the reusable-substrate boundary operator-readable, but it does not widen the current public article-equivalence claim boundary beyond the underlying inventory report",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Existing substrate inventory summary now records surface_count={}, blocker_surface_count={}, tied_requirement_satisfied={}, acceptance_status={}, and article_equivalence_green={}.",
            report.surface_count,
            report.blocker_surface_count,
            report.tied_requirement_satisfied,
            report.acceptance_status,
            report.article_equivalence_green,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_existing_substrate_inventory_summary|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarExistingSubstrateInventorySummaryError {
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

pub fn build_tassadar_existing_substrate_inventory_summary() -> Result<
    TassadarExistingSubstrateInventorySummary,
    TassadarExistingSubstrateInventorySummaryError,
> {
    let inventory_report: TassadarExistingSubstrateInventoryReport = read_repo_json(
        TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF,
        "existing_substrate_inventory",
    )?;
    Ok(TassadarExistingSubstrateInventorySummary::new(inventory_report))
}

pub fn tassadar_existing_substrate_inventory_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_EXISTING_SUBSTRATE_INVENTORY_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_existing_substrate_inventory_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarExistingSubstrateInventorySummary,
    TassadarExistingSubstrateInventorySummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarExistingSubstrateInventorySummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_existing_substrate_inventory_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarExistingSubstrateInventorySummaryError::Write {
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
) -> Result<T, TassadarExistingSubstrateInventorySummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarExistingSubstrateInventorySummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarExistingSubstrateInventorySummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_existing_substrate_inventory_summary, read_repo_json,
        tassadar_existing_substrate_inventory_summary_path,
        write_tassadar_existing_substrate_inventory_summary,
        TassadarExistingSubstrateInventorySummary,
        TASSADAR_EXISTING_SUBSTRATE_INVENTORY_SUMMARY_REPORT_REF,
    };

    #[test]
    fn existing_substrate_inventory_summary_tracks_gate_tie_without_final_green() {
        let report = build_tassadar_existing_substrate_inventory_summary().expect("summary");

        assert!(report.tied_requirement_satisfied);
        assert_eq!(report.acceptance_status, "green");
        assert!(report.article_equivalence_green);
        assert_eq!(report.surface_count, 11);
        assert_eq!(report.blocker_surface_count, 6);
    }

    #[test]
    fn existing_substrate_inventory_summary_matches_committed_truth() {
        let generated =
            build_tassadar_existing_substrate_inventory_summary().expect("summary");
        let committed: TassadarExistingSubstrateInventorySummary = read_repo_json(
            TASSADAR_EXISTING_SUBSTRATE_INVENTORY_SUMMARY_REPORT_REF,
            "existing_substrate_inventory_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_existing_substrate_inventory_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_existing_substrate_inventory_summary.json");
        let written = write_tassadar_existing_substrate_inventory_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarExistingSubstrateInventorySummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_existing_substrate_inventory_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_existing_substrate_inventory_summary.json")
        );
    }
}
