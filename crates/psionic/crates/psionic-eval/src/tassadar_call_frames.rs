use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{tassadar_call_frame_publication, TassadarCallFramePublication};
use psionic_runtime::{
    execute_tassadar_call_frame_program, tassadar_seeded_call_frame_direct_call_program,
    tassadar_seeded_call_frame_multi_function_program,
    tassadar_seeded_call_frame_recursion_program, tassadar_seeded_call_frame_recursive_sum_program,
    TassadarCallFrameError, TassadarCallFrameHaltReason, TassadarCallFrameProgram,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_CALL_FRAME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_call_frame_report.json";

/// Repo-facing status for one bounded call-frame case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCallFrameCaseStatus {
    Exact,
    Refused,
}

/// One repo-facing bounded call-frame case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameCaseReport {
    pub case_id: String,
    pub status: TassadarCallFrameCaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub halt_reason: Option<TassadarCallFrameHaltReason>,
    pub trace_step_count: usize,
    pub max_frame_depth: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_digest: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub frame_presence_counts: BTreeMap<String, usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

/// Committed report over the bounded call-frame lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication: TassadarCallFramePublication,
    pub cases: Vec<TassadarCallFrameCaseReport>,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl TassadarCallFrameReport {
    fn new(cases: Vec<TassadarCallFrameCaseReport>) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.call_frames.report.v1"),
            publication: tassadar_call_frame_publication(),
            cases,
            claim_boundary: String::from(
                "this report proves one bounded direct-call multi-function lane with explicit frame-stack traces, exact direct-call replay, conditional in-frame control, exact bounded recursion under an explicit depth cap, and typed recursion refusal at the cap; it does not claim call_indirect, imports, host calls, tail calls, or arbitrary Wasm closure",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(b"psionic_tassadar_call_frame_report|", &report);
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarCallFrameReportError {
    #[error(transparent)]
    Runtime(#[from] TassadarCallFrameError),
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write call-frame report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read committed call-frame report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode committed call-frame report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_call_frame_report(
) -> Result<TassadarCallFrameReport, TassadarCallFrameReportError> {
    Ok(TassadarCallFrameReport::new(vec![
        build_exact_case(
            "direct_call_parity",
            tassadar_seeded_call_frame_direct_call_program(),
        )?,
        build_exact_case(
            "multi_function_replay",
            tassadar_seeded_call_frame_multi_function_program(),
        )?,
        build_exact_case(
            "bounded_recursive_exact",
            tassadar_seeded_call_frame_recursive_sum_program(),
        )?,
        build_refusal_case(
            "bounded_recursion_refusal",
            tassadar_seeded_call_frame_recursion_program(),
        ),
    ]))
}

pub fn tassadar_call_frame_report_path() -> PathBuf {
    repo_root().join(TASSADAR_CALL_FRAME_REPORT_REF)
}

pub fn write_tassadar_call_frame_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCallFrameReport, TassadarCallFrameReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarCallFrameReportError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_tassadar_call_frame_report()?;
    let json = serde_json::to_string_pretty(&report)
        .expect("call-frame report serialization should succeed");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCallFrameReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_exact_case(
    case_id: &str,
    program: TassadarCallFrameProgram,
) -> Result<TassadarCallFrameCaseReport, TassadarCallFrameReportError> {
    let execution = execute_tassadar_call_frame_program(&program)?;
    let mut frame_presence_counts = BTreeMap::new();
    for step in &execution.steps {
        for frame in &step.frame_stack_after {
            *frame_presence_counts
                .entry(frame.function_name.clone())
                .or_insert(0) += 1;
        }
    }
    Ok(TassadarCallFrameCaseReport {
        case_id: String::from(case_id),
        status: TassadarCallFrameCaseStatus::Exact,
        program_id: Some(program.program_id),
        returned_value: execution.returned_value,
        halt_reason: Some(execution.halt_reason),
        trace_step_count: execution.steps.len(),
        max_frame_depth: execution
            .steps
            .iter()
            .map(|step| step.frame_depth_after)
            .max()
            .unwrap_or_default(),
        execution_digest: Some(execution.execution_digest()),
        frame_presence_counts,
        refusal_kind: None,
        refusal_detail: None,
    })
}

fn build_refusal_case(
    case_id: &str,
    program: TassadarCallFrameProgram,
) -> TassadarCallFrameCaseReport {
    match execute_tassadar_call_frame_program(&program) {
        Ok(execution) => TassadarCallFrameCaseReport {
            case_id: String::from(case_id),
            status: TassadarCallFrameCaseStatus::Exact,
            program_id: Some(program.program_id),
            returned_value: execution.returned_value,
            halt_reason: Some(execution.halt_reason),
            trace_step_count: execution.steps.len(),
            max_frame_depth: execution
                .steps
                .iter()
                .map(|step| step.frame_depth_after)
                .max()
                .unwrap_or_default(),
            execution_digest: Some(execution.execution_digest()),
            frame_presence_counts: BTreeMap::new(),
            refusal_kind: None,
            refusal_detail: None,
        },
        Err(error) => TassadarCallFrameCaseReport {
            case_id: String::from(case_id),
            status: TassadarCallFrameCaseStatus::Refused,
            program_id: Some(program.program_id),
            returned_value: None,
            halt_reason: None,
            trace_step_count: 0,
            max_frame_depth: 0,
            execution_digest: None,
            frame_presence_counts: BTreeMap::new(),
            refusal_kind: Some(match error {
                TassadarCallFrameError::RecursionDepthExceeded { .. } => {
                    String::from("recursion_depth_exceeded")
                }
                _ => String::from("runtime_refusal"),
            }),
            refusal_detail: Some(error.to_string()),
        },
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
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
        build_tassadar_call_frame_report, repo_root, write_tassadar_call_frame_report,
        TassadarCallFrameCaseStatus, TassadarCallFrameReport, TASSADAR_CALL_FRAME_REPORT_REF,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn call_frame_report_captures_exact_and_refused_cases() {
        let report = build_tassadar_call_frame_report().expect("report");
        assert_eq!(report.cases.len(), 4);
        assert!(report
            .cases
            .iter()
            .any(|case| case.status == TassadarCallFrameCaseStatus::Exact));
        let recursive = report
            .cases
            .iter()
            .find(|case| case.case_id == "bounded_recursive_exact")
            .expect("recursive exact case");
        assert_eq!(recursive.returned_value, Some(15));
        assert!(recursive.max_frame_depth >= 6);
        let refusal = report
            .cases
            .iter()
            .find(|case| case.case_id == "bounded_recursion_refusal")
            .expect("refusal case");
        assert_eq!(refusal.status, TassadarCallFrameCaseStatus::Refused);
        assert_eq!(
            refusal.refusal_kind.as_deref(),
            Some("recursion_depth_exceeded")
        );
    }

    #[test]
    fn call_frame_report_matches_committed_truth() {
        let generated = build_tassadar_call_frame_report().expect("report");
        let committed: TassadarCallFrameReport =
            read_repo_json(TASSADAR_CALL_FRAME_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_call_frame_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory.path().join("tassadar_call_frame_report.json");
        let written = write_tassadar_call_frame_report(&output_path).expect("write report");
        let persisted: TassadarCallFrameReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }
}
