use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_broad_family_specialization_report, TassadarBroadFamilySpecializationReport,
    TassadarBroadFamilySpecializationReportError,
};

pub const TASSADAR_BROAD_FAMILY_SPECIALIZATION_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_family_specialization_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadFamilySpecializationSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarBroadFamilySpecializationReport,
    pub promotion_ready_family_ids: Vec<String>,
    pub benchmark_only_family_ids: Vec<String>,
    pub refused_family_ids: Vec<String>,
    pub blocked_gate_reasons: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarBroadFamilySpecializationSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarBroadFamilySpecializationReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_broad_family_specialization_summary(
) -> Result<TassadarBroadFamilySpecializationSummary, TassadarBroadFamilySpecializationSummaryError>
{
    let eval_report = build_tassadar_broad_family_specialization_report();
    let promotion_ready_family_ids = eval_report.safety_gate_green_family_ids.clone();
    let benchmark_only_family_ids = eval_report.unstable_family_ids.clone();
    let refused_family_ids = eval_report.non_decompilable_family_ids.clone();
    let mut blocked_gate_reasons = vec![
        String::from("structure varies across retrains on search_frontier_bundle"),
        String::from("portability envelope remains too narrow on linked_worker_bundle"),
        String::from("effectful_resume_bundle is not decompilable enough to stay challengeable"),
    ];
    blocked_gate_reasons.sort();
    let mut summary = TassadarBroadFamilySpecializationSummary {
        schema_version: 1,
        report_id: String::from("tassadar.broad_family_specialization.summary.v1"),
        eval_report,
        promotion_ready_family_ids,
        benchmark_only_family_ids,
        refused_family_ids,
        blocked_gate_reasons,
        claim_boundary: String::from(
            "this summary interprets broad-family specialization as a research-only promotion-discipline surface. It keeps promotion-ready, benchmark-only, and refused families explicit instead of widening served posture or broad internal-compute claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    summary.summary = format!(
        "Broad-family specialization summary marks promotion_ready={}, benchmark_only={}, refused={}, blocked_gate_reasons={}.",
        summary.promotion_ready_family_ids.len(),
        summary.benchmark_only_family_ids.len(),
        summary.refused_family_ids.len(),
        summary.blocked_gate_reasons.len(),
    );
    summary.report_digest = stable_digest(
        b"psionic_tassadar_broad_family_specialization_summary|",
        &summary,
    );
    Ok(summary)
}

#[must_use]
pub fn tassadar_broad_family_specialization_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_FAMILY_SPECIALIZATION_SUMMARY_REF)
}

pub fn write_tassadar_broad_family_specialization_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarBroadFamilySpecializationSummary, TassadarBroadFamilySpecializationSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadFamilySpecializationSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_broad_family_specialization_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarBroadFamilySpecializationSummaryError::Write {
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
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarBroadFamilySpecializationSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarBroadFamilySpecializationSummaryError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadFamilySpecializationSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_broad_family_specialization_summary, read_repo_json,
        tassadar_broad_family_specialization_summary_path,
        TassadarBroadFamilySpecializationSummary, TASSADAR_BROAD_FAMILY_SPECIALIZATION_SUMMARY_REF,
    };

    #[test]
    fn broad_family_specialization_summary_marks_ready_benchmark_only_and_refused_families() {
        let summary = build_tassadar_broad_family_specialization_summary().expect("summary");

        assert_eq!(summary.promotion_ready_family_ids.len(), 1);
        assert_eq!(summary.benchmark_only_family_ids.len(), 2);
        assert_eq!(summary.refused_family_ids.len(), 1);
        assert_eq!(summary.blocked_gate_reasons.len(), 3);
    }

    #[test]
    fn broad_family_specialization_summary_matches_committed_truth() {
        let generated = build_tassadar_broad_family_specialization_summary().expect("summary");
        let committed: TassadarBroadFamilySpecializationSummary =
            read_repo_json(TASSADAR_BROAD_FAMILY_SPECIALIZATION_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn broad_family_specialization_summary_path_is_stable() {
        assert_eq!(
            tassadar_broad_family_specialization_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_broad_family_specialization_summary.json")
        );
    }
}
