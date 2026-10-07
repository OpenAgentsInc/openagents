use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{TassadarArticleAbiLoweringError, lower_tassadar_article_abi_fixture};
use psionic_ir::{TassadarArticleAbiFixture, tassadar_article_abi_fixture_suite};
use psionic_models::{
    TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF, TassadarArticleAbiPublication,
    tassadar_article_abi_publication,
};
use psionic_runtime::{
    TassadarArticleAbiError, TassadarArticleAbiInvocation, execute_tassadar_article_abi_program,
    tassadar_article_abi_heap_sum_invocation, tassadar_article_abi_heap_sum_offset_invocation,
    tassadar_article_abi_heap_sum_out_of_range_invocation, tassadar_article_abi_scalar_invocation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Repo-facing status for one bounded Rust-only article ABI case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleAbiCaseStatus {
    Exact,
    Refused,
}

/// One repo-facing bounded Rust-only article ABI case result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiCaseReport {
    pub case_id: String,
    pub source_case_id: String,
    pub source_ref: String,
    pub export_name: String,
    pub status: TassadarArticleAbiCaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_step_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    pub invocation_args: Vec<i32>,
    pub heap_byte_len: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

/// Committed report over the bounded Rust-only article ABI closure lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiClosureReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication: TassadarArticleAbiPublication,
    pub generated_from_refs: Vec<String>,
    pub cases: Vec<TassadarArticleAbiCaseReport>,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl TassadarArticleAbiClosureReport {
    fn new(cases: Vec<TassadarArticleAbiCaseReport>, generated_from_refs: Vec<String>) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_abi.closure_report.v1"),
            publication: tassadar_article_abi_publication(),
            generated_from_refs,
            cases,
            claim_boundary: String::from(
                "this report closes the bounded Rust-only article ABI gap only for the committed direct scalar i32 and pointer-plus-length i32 heap-input fixtures. It keeps floating-point params, multi-result returns, host-handle imports, and arbitrary Wasm ABI closure explicit as refusals",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_article_abi_closure_report|", &report);
        report
    }
}

/// Failures while building or persisting the bounded Rust-only article ABI closure report.
#[derive(Debug, Error)]
pub enum TassadarArticleAbiClosureReportError {
    #[error(transparent)]
    Lowering(#[from] TassadarArticleAbiLoweringError),
    #[error(transparent)]
    Runtime(#[from] TassadarArticleAbiError),
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write article ABI closure report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read committed article ABI closure report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode committed article ABI closure report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

/// Builds the committed bounded Rust-only article ABI closure report.
pub fn build_tassadar_article_abi_closure_report()
-> Result<TassadarArticleAbiClosureReport, TassadarArticleAbiClosureReportError> {
    let scalar_fixture = TassadarArticleAbiFixture::scalar_add_one();
    let heap_fixture = TassadarArticleAbiFixture::heap_sum_i32();
    let float_fixture = TassadarArticleAbiFixture::unsupported_float_param();
    let multi_result_fixture = TassadarArticleAbiFixture::unsupported_multi_result();

    let scalar_case = build_exact_case(
        "direct_scalar_i32_param",
        &scalar_fixture,
        tassadar_article_abi_scalar_invocation(),
        42,
    )?;
    let heap_case = build_exact_case(
        "pointer_length_heap_input",
        &heap_fixture,
        tassadar_article_abi_heap_sum_invocation(),
        20,
    )?;
    let heap_offset_case = build_exact_case(
        "pointer_length_heap_input_with_offset",
        &heap_fixture,
        tassadar_article_abi_heap_sum_offset_invocation(),
        24,
    )?;
    let runtime_refusal_case = build_runtime_refusal_case(
        "heap_input_out_of_range_refusal",
        &heap_fixture,
        tassadar_article_abi_heap_sum_out_of_range_invocation(),
    )?;
    let float_refusal_case =
        build_lowering_refusal_case("floating_point_param_refusal", &float_fixture);
    let multi_result_refusal_case =
        build_lowering_refusal_case("multi_result_return_refusal", &multi_result_fixture);

    let mut generated_from_refs = vec![
        String::from("fixtures/tassadar/reports/tassadar_rust_source_canon_report.json"),
        String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF),
    ];
    generated_from_refs.extend(
        tassadar_article_abi_fixture_suite()
            .into_iter()
            .map(|fixture| fixture.source_ref),
    );
    generated_from_refs.sort();
    generated_from_refs.dedup();

    Ok(TassadarArticleAbiClosureReport::new(
        vec![
            scalar_case,
            heap_case,
            heap_offset_case,
            runtime_refusal_case,
            float_refusal_case,
            multi_result_refusal_case,
        ],
        generated_from_refs,
    ))
}

/// Returns the canonical absolute path for the committed bounded Rust-only article ABI closure report.
#[must_use]
pub fn tassadar_article_abi_closure_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF)
}

/// Writes the committed bounded Rust-only article ABI closure report.
pub fn write_tassadar_article_abi_closure_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleAbiClosureReport, TassadarArticleAbiClosureReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleAbiClosureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_abi_closure_report()?;
    let json =
        serde_json::to_string_pretty(&report).expect("article ABI closure report serializes");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleAbiClosureReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_exact_case(
    case_id: &str,
    fixture: &TassadarArticleAbiFixture,
    invocation: TassadarArticleAbiInvocation,
    expected_returned_value: i32,
) -> Result<TassadarArticleAbiCaseReport, TassadarArticleAbiClosureReportError> {
    let artifact = lower_tassadar_article_abi_fixture(fixture)?;
    let execution = execute_tassadar_article_abi_program(&artifact.program, &invocation)?;
    assert_eq!(
        execution.returned_value,
        Some(expected_returned_value),
        "bounded article ABI case `{case_id}` should match its CPU reference expectation"
    );
    Ok(TassadarArticleAbiCaseReport {
        case_id: String::from(case_id),
        source_case_id: fixture.source_case_id.clone(),
        source_ref: fixture.source_ref.clone(),
        export_name: fixture.export_name.clone(),
        status: TassadarArticleAbiCaseStatus::Exact,
        returned_value: execution.returned_value,
        execution_digest: Some(execution.execution_digest()),
        trace_step_count: Some(execution.steps.len()),
        artifact_digest: Some(artifact.artifact_digest),
        invocation_args: invocation.args,
        heap_byte_len: invocation.heap_bytes.len(),
        refusal_kind: None,
        refusal_detail: None,
    })
}

fn build_runtime_refusal_case(
    case_id: &str,
    fixture: &TassadarArticleAbiFixture,
    invocation: TassadarArticleAbiInvocation,
) -> Result<TassadarArticleAbiCaseReport, TassadarArticleAbiClosureReportError> {
    let artifact = lower_tassadar_article_abi_fixture(fixture)?;
    match execute_tassadar_article_abi_program(&artifact.program, &invocation) {
        Ok(execution) => Ok(TassadarArticleAbiCaseReport {
            case_id: String::from(case_id),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            status: TassadarArticleAbiCaseStatus::Exact,
            returned_value: execution.returned_value,
            execution_digest: Some(execution.execution_digest()),
            trace_step_count: Some(execution.steps.len()),
            artifact_digest: Some(artifact.artifact_digest),
            invocation_args: invocation.args,
            heap_byte_len: invocation.heap_bytes.len(),
            refusal_kind: None,
            refusal_detail: None,
        }),
        Err(error) => Ok(TassadarArticleAbiCaseReport {
            case_id: String::from(case_id),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            status: TassadarArticleAbiCaseStatus::Refused,
            returned_value: None,
            execution_digest: None,
            trace_step_count: None,
            artifact_digest: Some(artifact.artifact_digest),
            invocation_args: invocation.args,
            heap_byte_len: invocation.heap_bytes.len(),
            refusal_kind: Some(runtime_refusal_kind(&error)),
            refusal_detail: Some(error.to_string()),
        }),
    }
}

fn build_lowering_refusal_case(
    case_id: &str,
    fixture: &TassadarArticleAbiFixture,
) -> TassadarArticleAbiCaseReport {
    match lower_tassadar_article_abi_fixture(fixture) {
        Ok(artifact) => TassadarArticleAbiCaseReport {
            case_id: String::from(case_id),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            status: TassadarArticleAbiCaseStatus::Exact,
            returned_value: None,
            execution_digest: None,
            trace_step_count: None,
            artifact_digest: Some(artifact.artifact_digest),
            invocation_args: Vec::new(),
            heap_byte_len: 0,
            refusal_kind: None,
            refusal_detail: None,
        },
        Err(error) => TassadarArticleAbiCaseReport {
            case_id: String::from(case_id),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            status: TassadarArticleAbiCaseStatus::Refused,
            returned_value: None,
            execution_digest: None,
            trace_step_count: None,
            artifact_digest: None,
            invocation_args: Vec::new(),
            heap_byte_len: 0,
            refusal_kind: Some(lowering_refusal_kind(&error)),
            refusal_detail: Some(error.to_string()),
        },
    }
}

fn lowering_refusal_kind(error: &TassadarArticleAbiLoweringError) -> String {
    match error {
        TassadarArticleAbiLoweringError::UnsupportedParamKind { .. } => {
            String::from("unsupported_param_kind")
        }
        TassadarArticleAbiLoweringError::UnsupportedResultKind { .. } => {
            String::from("unsupported_result_kind")
        }
        TassadarArticleAbiLoweringError::UnsupportedHeapLayout { .. } => {
            String::from("unsupported_heap_layout")
        }
        TassadarArticleAbiLoweringError::InvalidLoweredProgram { .. } => {
            String::from("invalid_lowered_program")
        }
    }
}

fn runtime_refusal_kind(error: &TassadarArticleAbiError) -> String {
    match error {
        TassadarArticleAbiError::HeapInputOutOfRange { .. } => {
            String::from("heap_input_out_of_range")
        }
        TassadarArticleAbiError::MissingHeapBytes => String::from("missing_heap_bytes"),
        TassadarArticleAbiError::InvocationArgCountMismatch { .. } => {
            String::from("invocation_arg_count_mismatch")
        }
        _ => String::from("runtime_refusal"),
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
        TassadarArticleAbiCaseStatus, TassadarArticleAbiClosureReport,
        build_tassadar_article_abi_closure_report, tassadar_article_abi_closure_report_path,
        write_tassadar_article_abi_closure_report,
    };

    #[test]
    fn article_abi_closure_report_is_machine_legible() {
        let report = build_tassadar_article_abi_closure_report().expect("report");

        assert_eq!(report.publication.family_id, "tassadar.rust_article_abi.v1");
        assert!(report.cases.iter().any(|case| {
            case.case_id == "direct_scalar_i32_param"
                && case.status == TassadarArticleAbiCaseStatus::Exact
                && case.returned_value == Some(42)
        }));
        assert!(report.cases.iter().any(|case| {
            case.case_id == "floating_point_param_refusal"
                && case.status == TassadarArticleAbiCaseStatus::Refused
        }));
    }

    #[test]
    fn article_abi_closure_report_matches_committed_truth() -> Result<(), Box<dyn std::error::Error>>
    {
        let report = build_tassadar_article_abi_closure_report()?;
        let bytes = std::fs::read(tassadar_article_abi_closure_report_path())?;
        let committed: TassadarArticleAbiClosureReport = serde_json::from_slice(&bytes)?;
        assert_eq!(report, committed);
        Ok(())
    }

    #[test]
    fn write_article_abi_closure_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let report_path = temp_dir
            .path()
            .join("tassadar_article_abi_closure_report.json");
        let report = write_tassadar_article_abi_closure_report(&report_path)?;
        let persisted: TassadarArticleAbiClosureReport =
            serde_json::from_slice(&std::fs::read(&report_path)?)?;
        assert_eq!(report, persisted);
        Ok(())
    }
}
