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
    TassadarThreadsResearchProfileEvalReport, build_tassadar_threads_research_profile_report,
};

pub const TASSADAR_THREADS_RESEARCH_PROFILE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_threads_research_profile_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarThreadsResearchProfileSummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarThreadsResearchProfileEvalReport,
    pub fragile_scheduler_ids: Vec<String>,
    pub refused_scheduler_ids: Vec<String>,
    pub deterministic_scope_sentence: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarThreadsResearchProfileSummaryReport {
    fn new(eval_report: TassadarThreadsResearchProfileEvalReport) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.threads_research_profile.summary.v1"),
            fragile_scheduler_ids: vec![String::from("deterministic_round_robin_v1")],
            refused_scheduler_ids: eval_report.refused_scheduler_ids.clone(),
            deterministic_scope_sentence: String::from(
                "the current research surface is only a two-thread deterministic scheduler envelope with explicit replay order; host-nondeterministic scheduling and relaxed shared-memory ordering remain refused",
            ),
            eval_report,
            claim_boundary: String::from(
                "this summary keeps shared-memory and threads as a research-only architecture surface. It highlights the narrow deterministic scheduler scope and explicit refusals instead of widening concurrency into served or market posture",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Threads research summary now records overall_green={}, fragile_schedulers={}, refused_schedulers={}, and served_publication_allowed={}.",
            report.eval_report.overall_green,
            report.fragile_scheduler_ids.len(),
            report.refused_scheduler_ids.len(),
            report.eval_report.served_publication_allowed,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_threads_research_profile_summary_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarThreadsResearchProfileSummaryError {
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

pub fn build_tassadar_threads_research_profile_summary_report()
-> Result<TassadarThreadsResearchProfileSummaryReport, TassadarThreadsResearchProfileSummaryError> {
    Ok(TassadarThreadsResearchProfileSummaryReport::new(
        build_tassadar_threads_research_profile_report(),
    ))
}

#[must_use]
pub fn tassadar_threads_research_profile_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_THREADS_RESEARCH_PROFILE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_threads_research_profile_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarThreadsResearchProfileSummaryReport, TassadarThreadsResearchProfileSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarThreadsResearchProfileSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_threads_research_profile_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarThreadsResearchProfileSummaryError::Write {
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
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarThreadsResearchProfileSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarThreadsResearchProfileSummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarThreadsResearchProfileSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_THREADS_RESEARCH_PROFILE_SUMMARY_REPORT_REF,
        TassadarThreadsResearchProfileSummaryReport,
        build_tassadar_threads_research_profile_summary_report, read_repo_json,
        tassadar_threads_research_profile_summary_report_path,
        write_tassadar_threads_research_profile_summary_report,
    };

    #[test]
    fn threads_research_profile_summary_keeps_scope_and_refusals_explicit() {
        let report =
            build_tassadar_threads_research_profile_summary_report().expect("summary report");

        assert!(report.eval_report.overall_green);
        assert_eq!(
            report.fragile_scheduler_ids,
            vec![String::from("deterministic_round_robin_v1")]
        );
        assert!(
            report
                .refused_scheduler_ids
                .contains(&String::from("host_nondeterministic_runtime"))
        );
        assert!(!report.eval_report.served_publication_allowed);
    }

    #[test]
    fn threads_research_profile_summary_matches_committed_truth() {
        let generated =
            build_tassadar_threads_research_profile_summary_report().expect("summary report");
        let committed: TassadarThreadsResearchProfileSummaryReport =
            read_repo_json(TASSADAR_THREADS_RESEARCH_PROFILE_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_threads_research_profile_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_threads_research_profile_summary.json");
        let written = write_tassadar_threads_research_profile_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarThreadsResearchProfileSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_threads_research_profile_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_threads_research_profile_summary.json")
        );
    }
}
