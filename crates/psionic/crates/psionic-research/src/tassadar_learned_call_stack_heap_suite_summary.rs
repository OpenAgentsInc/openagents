use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_SUMMARY_REF, TassadarCallStackHeapGeneralizationSplit,
    TassadarCallStackHeapWorkloadFamily,
};
use psionic_eval::{
    TassadarLearnedCallStackHeapSuiteReport, TassadarLearnedCallStackHeapSuiteReportError,
    build_tassadar_learned_call_stack_heap_suite_report,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedCallStackHeapSuiteSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarLearnedCallStackHeapSuiteReport,
    pub strong_in_family_workloads: Vec<TassadarCallStackHeapWorkloadFamily>,
    pub held_out_recoverable_workloads: Vec<TassadarCallStackHeapWorkloadFamily>,
    pub call_stack_dominant_breaks: Vec<TassadarCallStackHeapWorkloadFamily>,
    pub heap_dominant_breaks: Vec<TassadarCallStackHeapWorkloadFamily>,
    pub refusal_sensitive_workloads: Vec<TassadarCallStackHeapWorkloadFamily>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarLearnedCallStackHeapSuiteSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarLearnedCallStackHeapSuiteReportError),
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

pub fn build_tassadar_learned_call_stack_heap_suite_summary()
-> Result<TassadarLearnedCallStackHeapSuiteSummary, TassadarLearnedCallStackHeapSuiteSummaryError> {
    let eval_report = build_tassadar_learned_call_stack_heap_suite_report()?;
    let strong_in_family_workloads = eval_report
        .workload_summaries
        .iter()
        .filter(|summary| summary.split == TassadarCallStackHeapGeneralizationSplit::InFamily)
        .filter(|summary| summary.structured_later_window_exactness_bps >= 8_000)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let held_out_recoverable_workloads = eval_report
        .workload_summaries
        .iter()
        .filter(|summary| summary.split == TassadarCallStackHeapGeneralizationSplit::HeldOutFamily)
        .filter(|summary| summary.structured_later_window_exactness_bps >= 6_500)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let call_stack_dominant_breaks = eval_report
        .workload_summaries
        .iter()
        .filter(|summary| {
            matches!(
                summary.workload_family,
                TassadarCallStackHeapWorkloadFamily::RecursiveEvaluator
                    | TassadarCallStackHeapWorkloadFamily::ParserFrameMachine
                    | TassadarCallStackHeapWorkloadFamily::HeldOutContinuationMachine
            )
        })
        .filter(|summary| summary.structured_gain_vs_baseline_bps >= 1_000)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let heap_dominant_breaks = eval_report
        .workload_summaries
        .iter()
        .filter(|summary| {
            matches!(
                summary.workload_family,
                TassadarCallStackHeapWorkloadFamily::BumpAllocatorHeap
                    | TassadarCallStackHeapWorkloadFamily::FreeListAllocatorHeap
                    | TassadarCallStackHeapWorkloadFamily::ResumableProcessHeap
                    | TassadarCallStackHeapWorkloadFamily::HeldOutAllocatorScheduler
            )
        })
        .filter(|summary| summary.structured_gain_vs_baseline_bps >= 1_200)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let refusal_sensitive_workloads = eval_report
        .workload_summaries
        .iter()
        .filter(|summary| summary.refusal_gain_vs_baseline_bps >= 700)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let mut summary = TassadarLearnedCallStackHeapSuiteSummary {
        schema_version: 1,
        report_id: String::from("tassadar.learned_call_stack_heap_suite.summary.v1"),
        eval_report,
        strong_in_family_workloads,
        held_out_recoverable_workloads,
        call_stack_dominant_breaks,
        heap_dominant_breaks,
        refusal_sensitive_workloads,
        claim_boundary: String::from(
            "this summary is a research interpretation over the committed learned call-stack/heap suite. It keeps in-family strength, held-out recovery, and break classes explicit instead of promoting the learned lane into broad practical internal computation",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    summary.summary = format!(
        "Learned call-stack/heap summary marks {} strong in-family workloads, {} held-out recoverable workloads, {} call-stack dominant breaks, and {} heap dominant breaks.",
        summary.strong_in_family_workloads.len(),
        summary.held_out_recoverable_workloads.len(),
        summary.call_stack_dominant_breaks.len(),
        summary.heap_dominant_breaks.len(),
    );
    summary.report_digest = stable_digest(
        b"psionic_tassadar_learned_call_stack_heap_suite_summary|",
        &summary,
    );
    Ok(summary)
}

#[must_use]
pub fn tassadar_learned_call_stack_heap_suite_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_SUMMARY_REF)
}

pub fn write_tassadar_learned_call_stack_heap_suite_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLearnedCallStackHeapSuiteSummary, TassadarLearnedCallStackHeapSuiteSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarLearnedCallStackHeapSuiteSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_learned_call_stack_heap_suite_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarLearnedCallStackHeapSuiteSummaryError::Write {
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
) -> Result<T, TassadarLearnedCallStackHeapSuiteSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarLearnedCallStackHeapSuiteSummaryError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarLearnedCallStackHeapSuiteSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarLearnedCallStackHeapSuiteSummary,
        build_tassadar_learned_call_stack_heap_suite_summary, read_repo_json,
        tassadar_learned_call_stack_heap_suite_summary_path,
        write_tassadar_learned_call_stack_heap_suite_summary,
    };
    use psionic_data::TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_SUMMARY_REF;
    use psionic_data::TassadarCallStackHeapWorkloadFamily;

    #[test]
    fn learned_call_stack_heap_suite_summary_marks_held_out_recovery_and_break_classes() {
        let summary = build_tassadar_learned_call_stack_heap_suite_summary().expect("summary");

        assert!(
            summary
                .held_out_recoverable_workloads
                .contains(&TassadarCallStackHeapWorkloadFamily::HeldOutContinuationMachine)
        );
        assert!(
            summary
                .call_stack_dominant_breaks
                .contains(&TassadarCallStackHeapWorkloadFamily::ParserFrameMachine)
        );
        assert!(
            summary
                .heap_dominant_breaks
                .contains(&TassadarCallStackHeapWorkloadFamily::HeldOutAllocatorScheduler)
        );
    }

    #[test]
    fn learned_call_stack_heap_suite_summary_matches_committed_truth() {
        let generated = build_tassadar_learned_call_stack_heap_suite_summary().expect("summary");
        let committed: TassadarLearnedCallStackHeapSuiteSummary =
            read_repo_json(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_SUMMARY_REF)
                .expect("committed summary");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_learned_call_stack_heap_suite_summary_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_learned_call_stack_heap_suite_summary.json");
        let summary = write_tassadar_learned_call_stack_heap_suite_summary(&output_path)
            .expect("write summary");
        let written: TassadarLearnedCallStackHeapSuiteSummary =
            serde_json::from_str(&std::fs::read_to_string(&output_path).expect("written file"))
                .expect("parse");

        assert_eq!(summary, written);
        assert_eq!(
            tassadar_learned_call_stack_heap_suite_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_learned_call_stack_heap_suite_summary.json")
        );
    }
}
