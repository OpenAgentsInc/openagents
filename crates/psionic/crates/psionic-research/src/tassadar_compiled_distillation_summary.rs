use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::TASSADAR_COMPILED_DISTILLATION_SUMMARY_REPORT_REF;
use psionic_eval::{
    build_tassadar_compiled_distillation_report, TassadarCompiledDistillationReport,
    TassadarCompiledDistillationReportError,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationSummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub distillation_report: TassadarCompiledDistillationReport,
    pub mixed_distillation_rescue_workload_families: Vec<String>,
    pub io_only_refusal_workload_families: Vec<String>,
    pub full_trace_dependency_workload_families: Vec<String>,
    pub positive_invariance_ablation_workload_families: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarCompiledDistillationSummaryReport {
    fn new(distillation_report: TassadarCompiledDistillationReport) -> Self {
        let mixed_distillation_rescue_workload_families = distillation_report
            .workload_summaries
            .iter()
            .filter(|summary| summary.mixed_distillation_gain_over_io_only_bps >= 1_000)
            .map(|summary| String::from(summary.workload_family.as_str()))
            .collect::<Vec<_>>();
        let io_only_refusal_workload_families = distillation_report
            .workload_summaries
            .iter()
            .filter(|summary| summary.io_only_refused)
            .map(|summary| String::from(summary.workload_family.as_str()))
            .collect::<Vec<_>>();
        let full_trace_dependency_workload_families = distillation_report
            .workload_summaries
            .iter()
            .filter(|summary| summary.full_trace_gap_bps >= 500)
            .map(|summary| String::from(summary.workload_family.as_str()))
            .collect::<Vec<_>>();
        let positive_invariance_ablation_workload_families = distillation_report
            .evidence_bundle
            .invariance_ablations
            .iter()
            .filter(|ablation| ablation.delta_bps > 0)
            .map(|ablation| String::from(ablation.workload_family.as_str()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.compiled_distillation.summary.v1"),
            distillation_report,
            mixed_distillation_rescue_workload_families,
            io_only_refusal_workload_families,
            full_trace_dependency_workload_families,
            positive_invariance_ablation_workload_families,
            claim_boundary: String::from(
                "this summary keeps compiled-to-learned distillation as a research-only training surface. Mixed-distillation rescue families, io-only refusal families, and full-trace dependency remain explicit and do not widen served learned capability",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Compiled distillation summary now marks {} mixed-distillation rescue families, {} io-only refusal families, {} full-trace dependency families, and {} positive invariance-ablation families.",
            report.mixed_distillation_rescue_workload_families.len(),
            report.io_only_refusal_workload_families.len(),
            report.full_trace_dependency_workload_families.len(),
            report.positive_invariance_ablation_workload_families.len(),
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_compiled_distillation_summary_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarCompiledDistillationSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarCompiledDistillationReportError),
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

pub fn build_tassadar_compiled_distillation_summary_report(
) -> Result<TassadarCompiledDistillationSummaryReport, TassadarCompiledDistillationSummaryError> {
    let distillation_report = build_tassadar_compiled_distillation_report()?;
    Ok(TassadarCompiledDistillationSummaryReport::new(
        distillation_report,
    ))
}

#[must_use]
pub fn tassadar_compiled_distillation_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_COMPILED_DISTILLATION_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_compiled_distillation_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCompiledDistillationSummaryReport, TassadarCompiledDistillationSummaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCompiledDistillationSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_compiled_distillation_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCompiledDistillationSummaryError::Write {
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
) -> Result<T, TassadarCompiledDistillationSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarCompiledDistillationSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCompiledDistillationSummaryError::Deserialize {
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
        build_tassadar_compiled_distillation_summary_report, read_repo_json,
        tassadar_compiled_distillation_summary_report_path,
        write_tassadar_compiled_distillation_summary_report,
        TassadarCompiledDistillationSummaryReport,
    };
    use psionic_data::TASSADAR_COMPILED_DISTILLATION_SUMMARY_REPORT_REF;

    #[test]
    fn compiled_distillation_summary_marks_rescues_refusals_and_dependencies() {
        let report = build_tassadar_compiled_distillation_summary_report().expect("summary");

        assert!(report
            .mixed_distillation_rescue_workload_families
            .contains(&String::from("sudoku_search")));
        assert!(report
            .io_only_refusal_workload_families
            .contains(&String::from("hungarian_matching")));
        assert!(report
            .full_trace_dependency_workload_families
            .contains(&String::from("sudoku_search")));
    }

    #[test]
    fn compiled_distillation_summary_matches_committed_truth() {
        let generated = build_tassadar_compiled_distillation_summary_report().expect("summary");
        let committed: TassadarCompiledDistillationSummaryReport =
            read_repo_json(TASSADAR_COMPILED_DISTILLATION_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_compiled_distillation_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_compiled_distillation_summary.json");
        let written = write_tassadar_compiled_distillation_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarCompiledDistillationSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_compiled_distillation_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_compiled_distillation_summary.json")
        );
    }
}
