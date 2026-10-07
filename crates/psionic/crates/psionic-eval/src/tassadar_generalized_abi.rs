use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{
    TassadarGeneralizedAbiLoweringError, lower_tassadar_generalized_abi_fixture,
};
use psionic_ir::TassadarGeneralizedAbiFixture;
use psionic_models::{
    TASSADAR_GENERALIZED_ABI_FAMILY_REPORT_REF, TassadarGeneralizedAbiPublication,
    tassadar_generalized_abi_publication,
};
use psionic_runtime::{
    TassadarGeneralizedAbiError, TassadarGeneralizedAbiInvocation,
    TassadarGeneralizedAbiRegionObservation, execute_tassadar_generalized_abi_program,
    tassadar_generalized_abi_dual_heap_dot_invocation,
    tassadar_generalized_abi_dual_heap_dot_out_of_range_invocation,
    tassadar_generalized_abi_i64_status_output_invocation,
    tassadar_generalized_abi_i64_status_output_unaligned_invocation,
    tassadar_generalized_abi_pair_add_i64_invocation, tassadar_generalized_abi_pair_add_invocation,
    tassadar_generalized_abi_pair_sum_and_diff_i32_invocation,
    tassadar_generalized_abi_status_output_aliasing_invocation,
    tassadar_generalized_abi_status_output_invocation,
    tassadar_generalized_abi_status_output_short_invocation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Repo-facing status for one generalized ABI case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeneralizedAbiCaseStatus {
    Exact,
    Refused,
}

/// One repo-facing generalized ABI case result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralizedAbiCaseReport {
    pub case_id: String,
    pub source_case_id: String,
    pub source_ref: String,
    pub export_name: String,
    pub program_shape_id: String,
    pub runtime_support_ids: Vec<String>,
    pub status: TassadarGeneralizedAbiCaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_i64: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub returned_values: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_step_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    pub invocation_args: Vec<i64>,
    pub heap_byte_len: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_regions: Vec<TassadarGeneralizedAbiRegionObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

/// Committed report over the generalized ABI family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralizedAbiFamilyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication: TassadarGeneralizedAbiPublication,
    pub generated_from_refs: Vec<String>,
    pub exact_case_count: u16,
    pub refusal_case_count: u16,
    pub cases: Vec<TassadarGeneralizedAbiCaseReport>,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl TassadarGeneralizedAbiFamilyReport {
    fn new(cases: Vec<TassadarGeneralizedAbiCaseReport>, generated_from_refs: Vec<String>) -> Self {
        let exact_case_count = cases
            .iter()
            .filter(|case| case.status == TassadarGeneralizedAbiCaseStatus::Exact)
            .count() as u16;
        let refusal_case_count = cases
            .iter()
            .filter(|case| case.status == TassadarGeneralizedAbiCaseStatus::Refused)
            .count() as u16;
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.generalized_abi.family_report.v1"),
            publication: tassadar_generalized_abi_publication(),
            generated_from_refs,
            exact_case_count,
            refusal_case_count,
            cases,
            claim_boundary: String::from(
                "this report widens the bounded ABI story to a reusable generalized family over multi-param scalar entrypoints, exact i64 scalar entrypoints, homogeneous two-value i32 returns, multiple pointer-length inputs, caller-owned result-code-plus-output-buffer shapes, 8-byte caller-owned buffer layouts, and bounded multi-export program shapes. It keeps floating-point params, mixed-width multi-result returns, host-handle callbacks, callee-allocated returned buffers, malformed or aliased output buffers, and arbitrary runtime support explicit as refusals",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_generalized_abi_family_report|", &report);
        report
    }
}

/// Failures while building or persisting the generalized ABI family report.
#[derive(Debug, Error)]
pub enum TassadarGeneralizedAbiFamilyReportError {
    #[error(transparent)]
    Lowering(#[from] TassadarGeneralizedAbiLoweringError),
    #[error(transparent)]
    Runtime(#[from] TassadarGeneralizedAbiError),
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write generalized ABI family report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

/// Builds the committed generalized ABI family report.
pub fn build_tassadar_generalized_abi_family_report()
-> Result<TassadarGeneralizedAbiFamilyReport, TassadarGeneralizedAbiFamilyReportError> {
    let pair_fixture = TassadarGeneralizedAbiFixture::pair_add_i32();
    let pair_i64_fixture = TassadarGeneralizedAbiFixture::pair_add_i64();
    let dot_fixture = TassadarGeneralizedAbiFixture::dual_heap_dot_i32();
    let output_fixture = TassadarGeneralizedAbiFixture::sum_and_max_status_output();
    let pair_multi_fixture = TassadarGeneralizedAbiFixture::pair_sum_and_diff_i32();
    let i64_output_fixture = TassadarGeneralizedAbiFixture::sum_and_max_i64_status_output();
    let pair_sum_fixture = TassadarGeneralizedAbiFixture::multi_export_pair_sum();
    let local_double_fixture = TassadarGeneralizedAbiFixture::multi_export_local_double();
    let float_fixture = TassadarGeneralizedAbiFixture::unsupported_float_param();
    let multi_result_fixture = TassadarGeneralizedAbiFixture::unsupported_multi_result();
    let host_handle_fixture = TassadarGeneralizedAbiFixture::unsupported_host_handle();
    let returned_buffer_fixture = TassadarGeneralizedAbiFixture::unsupported_returned_buffer();

    let mut generated_from_refs = vec![
        String::from("fixtures/tassadar/reports/tassadar_rust_source_canon_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_article_abi_closure_report.json"),
        String::from(TASSADAR_GENERALIZED_ABI_FAMILY_REPORT_REF),
    ];
    for fixture in [
        &pair_fixture,
        &pair_i64_fixture,
        &dot_fixture,
        &output_fixture,
        &pair_multi_fixture,
        &i64_output_fixture,
        &pair_sum_fixture,
        &local_double_fixture,
        &float_fixture,
        &multi_result_fixture,
        &host_handle_fixture,
        &returned_buffer_fixture,
    ] {
        generated_from_refs.push(fixture.source_ref.clone());
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();

    Ok(TassadarGeneralizedAbiFamilyReport::new(
        vec![
            build_exact_case(
                "multi_param_i32_exact",
                &pair_fixture,
                tassadar_generalized_abi_pair_add_invocation(),
                Some(42),
                None,
                Vec::new(),
                Vec::new(),
            )?,
            build_exact_case(
                "multi_param_i64_exact",
                &pair_i64_fixture,
                tassadar_generalized_abi_pair_add_i64_invocation(),
                None,
                Some(42),
                Vec::new(),
                Vec::new(),
            )?,
            build_exact_case(
                "multiple_pointer_length_inputs_exact",
                &dot_fixture,
                tassadar_generalized_abi_dual_heap_dot_invocation(),
                Some(32),
                None,
                Vec::new(),
                Vec::new(),
            )?,
            build_runtime_refusal_case(
                "multiple_pointer_length_inputs_out_of_range_refusal",
                &dot_fixture,
                tassadar_generalized_abi_dual_heap_dot_out_of_range_invocation(),
            )?,
            build_exact_case(
                "result_code_plus_output_buffer_exact",
                &output_fixture,
                tassadar_generalized_abi_status_output_invocation(),
                Some(0),
                None,
                Vec::new(),
                vec![vec![19, 9]],
            )?,
            build_runtime_refusal_case(
                "result_code_plus_output_buffer_short_output_refusal",
                &output_fixture,
                tassadar_generalized_abi_status_output_short_invocation(),
            )?,
            build_runtime_refusal_case(
                "result_code_plus_output_buffer_alias_refusal",
                &output_fixture,
                tassadar_generalized_abi_status_output_aliasing_invocation(),
            )?,
            build_exact_case(
                "homogeneous_multi_value_return_exact",
                &pair_multi_fixture,
                tassadar_generalized_abi_pair_sum_and_diff_i32_invocation(),
                None,
                None,
                vec![42, -2],
                Vec::new(),
            )?,
            build_exact_case(
                "result_code_plus_output_buffer_i64_exact",
                &i64_output_fixture,
                tassadar_generalized_abi_i64_status_output_invocation(),
                Some(0),
                None,
                Vec::new(),
                vec![vec![19, 9]],
            )?,
            build_runtime_refusal_case(
                "result_code_plus_output_buffer_i64_unaligned_refusal",
                &i64_output_fixture,
                tassadar_generalized_abi_i64_status_output_unaligned_invocation(),
            )?,
            build_exact_case(
                "bounded_multi_export_pair_sum_exact",
                &pair_sum_fixture,
                TassadarGeneralizedAbiInvocation::new(Vec::new()),
                Some(5),
                None,
                Vec::new(),
                Vec::new(),
            )?,
            build_exact_case(
                "bounded_multi_export_local_double_exact",
                &local_double_fixture,
                TassadarGeneralizedAbiInvocation::new(Vec::new()),
                Some(14),
                None,
                Vec::new(),
                Vec::new(),
            )?,
            build_lowering_refusal_case("floating_point_param_refusal", &float_fixture),
            build_lowering_refusal_case("mixed_multi_result_return_refusal", &multi_result_fixture),
            build_lowering_refusal_case("host_handle_param_refusal", &host_handle_fixture),
            build_lowering_refusal_case(
                "callee_allocated_returned_buffer_refusal",
                &returned_buffer_fixture,
            ),
        ],
        generated_from_refs,
    ))
}

/// Returns the canonical absolute path for the generalized ABI family report.
#[must_use]
pub fn tassadar_generalized_abi_family_report_path() -> PathBuf {
    repo_root().join(TASSADAR_GENERALIZED_ABI_FAMILY_REPORT_REF)
}

/// Writes the generalized ABI family report.
pub fn write_tassadar_generalized_abi_family_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarGeneralizedAbiFamilyReport, TassadarGeneralizedAbiFamilyReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarGeneralizedAbiFamilyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_generalized_abi_family_report()?;
    let json =
        serde_json::to_string_pretty(&report).expect("generalized ABI family report serializes");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarGeneralizedAbiFamilyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_exact_case(
    case_id: &str,
    fixture: &TassadarGeneralizedAbiFixture,
    invocation: TassadarGeneralizedAbiInvocation,
    expected_returned_value: Option<i32>,
    expected_returned_i64: Option<i64>,
    expected_returned_values: Vec<i64>,
    expected_output_words: Vec<Vec<i64>>,
) -> Result<TassadarGeneralizedAbiCaseReport, TassadarGeneralizedAbiFamilyReportError> {
    let artifact = lower_tassadar_generalized_abi_fixture(fixture)?;
    let execution = execute_tassadar_generalized_abi_program(&artifact.program, &invocation)?;
    assert_eq!(
        execution.returned_value, expected_returned_value,
        "generalized ABI case `{case_id}` should match its CPU reference expectation"
    );
    assert_eq!(
        execution.returned_i64, expected_returned_i64,
        "generalized ABI case `{case_id}` should match its i64 return expectation"
    );
    assert_eq!(
        execution.returned_values, expected_returned_values,
        "generalized ABI case `{case_id}` should match its multi-value return expectation"
    );
    if !expected_output_words.is_empty() {
        let actual = execution
            .output_regions
            .iter()
            .map(|region| region.words.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            actual, expected_output_words,
            "generalized ABI case `{case_id}` should match its output-buffer expectation"
        );
    }
    let execution_digest = execution.execution_digest();
    Ok(TassadarGeneralizedAbiCaseReport {
        case_id: String::from(case_id),
        source_case_id: fixture.source_case_id.clone(),
        source_ref: fixture.source_ref.clone(),
        export_name: fixture.export_name.clone(),
        program_shape_id: fixture.program_shape_id.clone(),
        runtime_support_ids: fixture.runtime_support_ids.clone(),
        status: TassadarGeneralizedAbiCaseStatus::Exact,
        returned_value: execution.returned_value,
        returned_i64: execution.returned_i64,
        returned_values: execution.returned_values,
        execution_digest: Some(execution_digest),
        trace_step_count: Some(execution.steps.len()),
        artifact_digest: Some(artifact.artifact_digest),
        invocation_args: invocation.args,
        heap_byte_len: invocation.heap_bytes.len(),
        output_regions: execution.output_regions,
        refusal_kind: None,
        refusal_detail: None,
    })
}

fn build_runtime_refusal_case(
    case_id: &str,
    fixture: &TassadarGeneralizedAbiFixture,
    invocation: TassadarGeneralizedAbiInvocation,
) -> Result<TassadarGeneralizedAbiCaseReport, TassadarGeneralizedAbiFamilyReportError> {
    let artifact = lower_tassadar_generalized_abi_fixture(fixture)?;
    match execute_tassadar_generalized_abi_program(&artifact.program, &invocation) {
        Ok(execution) => {
            let execution_digest = execution.execution_digest();
            Ok(TassadarGeneralizedAbiCaseReport {
                case_id: String::from(case_id),
                source_case_id: fixture.source_case_id.clone(),
                source_ref: fixture.source_ref.clone(),
                export_name: fixture.export_name.clone(),
                program_shape_id: fixture.program_shape_id.clone(),
                runtime_support_ids: fixture.runtime_support_ids.clone(),
                status: TassadarGeneralizedAbiCaseStatus::Exact,
                returned_value: execution.returned_value,
                returned_i64: execution.returned_i64,
                returned_values: execution.returned_values,
                execution_digest: Some(execution_digest),
                trace_step_count: Some(execution.steps.len()),
                artifact_digest: Some(artifact.artifact_digest),
                invocation_args: invocation.args,
                heap_byte_len: invocation.heap_bytes.len(),
                output_regions: execution.output_regions,
                refusal_kind: None,
                refusal_detail: None,
            })
        }
        Err(error) => Ok(TassadarGeneralizedAbiCaseReport {
            case_id: String::from(case_id),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            status: TassadarGeneralizedAbiCaseStatus::Refused,
            returned_value: None,
            returned_i64: None,
            returned_values: Vec::new(),
            execution_digest: None,
            trace_step_count: None,
            artifact_digest: Some(artifact.artifact_digest),
            invocation_args: invocation.args,
            heap_byte_len: invocation.heap_bytes.len(),
            output_regions: Vec::new(),
            refusal_kind: Some(runtime_refusal_kind(&error)),
            refusal_detail: Some(error.to_string()),
        }),
    }
}

fn build_lowering_refusal_case(
    case_id: &str,
    fixture: &TassadarGeneralizedAbiFixture,
) -> TassadarGeneralizedAbiCaseReport {
    match lower_tassadar_generalized_abi_fixture(fixture) {
        Ok(artifact) => TassadarGeneralizedAbiCaseReport {
            case_id: String::from(case_id),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            status: TassadarGeneralizedAbiCaseStatus::Exact,
            returned_value: None,
            returned_i64: None,
            returned_values: Vec::new(),
            execution_digest: None,
            trace_step_count: None,
            artifact_digest: Some(artifact.artifact_digest),
            invocation_args: Vec::new(),
            heap_byte_len: 0,
            output_regions: Vec::new(),
            refusal_kind: None,
            refusal_detail: None,
        },
        Err(error) => TassadarGeneralizedAbiCaseReport {
            case_id: String::from(case_id),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            status: TassadarGeneralizedAbiCaseStatus::Refused,
            returned_value: None,
            returned_i64: None,
            returned_values: Vec::new(),
            execution_digest: None,
            trace_step_count: None,
            artifact_digest: None,
            invocation_args: Vec::new(),
            heap_byte_len: 0,
            output_regions: Vec::new(),
            refusal_kind: Some(lowering_refusal_kind(&error)),
            refusal_detail: Some(error.to_string()),
        },
    }
}

fn lowering_refusal_kind(error: &TassadarGeneralizedAbiLoweringError) -> String {
    match error {
        TassadarGeneralizedAbiLoweringError::UnsupportedParamKind { .. } => {
            String::from("unsupported_param_kind")
        }
        TassadarGeneralizedAbiLoweringError::UnsupportedResultKind { .. } => {
            String::from("unsupported_result_kind")
        }
        TassadarGeneralizedAbiLoweringError::UnsupportedResultKinds { .. } => {
            String::from("unsupported_result_kinds")
        }
        TassadarGeneralizedAbiLoweringError::InvalidLoweredProgram { .. } => {
            String::from("invalid_lowered_program")
        }
    }
}

fn runtime_refusal_kind(error: &TassadarGeneralizedAbiError) -> String {
    match error {
        TassadarGeneralizedAbiError::MemoryRegionOutOfRange { .. } => {
            String::from("memory_region_out_of_range")
        }
        TassadarGeneralizedAbiError::MemoryRegionTooShort { .. } => {
            String::from("memory_region_too_short")
        }
        TassadarGeneralizedAbiError::AliasedMemoryRegions { .. } => {
            String::from("aliased_memory_regions")
        }
        TassadarGeneralizedAbiError::MissingHeapBytes => String::from("missing_heap_bytes"),
        TassadarGeneralizedAbiError::InvocationArgCountMismatch { .. } => {
            String::from("invocation_arg_count_mismatch")
        }
        TassadarGeneralizedAbiError::UnalignedPointer { .. } => String::from("unaligned_pointer"),
        TassadarGeneralizedAbiError::MemoryRegionIndexOutOfRange { .. } => {
            String::from("memory_region_index_out_of_range")
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
        TassadarGeneralizedAbiCaseStatus, TassadarGeneralizedAbiFamilyReport,
        build_tassadar_generalized_abi_family_report, tassadar_generalized_abi_family_report_path,
        write_tassadar_generalized_abi_family_report,
    };

    #[test]
    fn generalized_abi_family_report_is_machine_legible() {
        let report = build_tassadar_generalized_abi_family_report().expect("report");

        assert_eq!(
            report.publication.family_id,
            "tassadar.rust_generalized_abi.v1"
        );
        assert_eq!(report.exact_case_count, 8);
        assert!(report.cases.iter().any(|case| {
            case.case_id == "result_code_plus_output_buffer_exact"
                && case.status == TassadarGeneralizedAbiCaseStatus::Exact
                && case
                    .output_regions
                    .first()
                    .map(|region| region.words.as_slice())
                    == Some([19, 9].as_slice())
        }));
        assert!(report.cases.iter().any(|case| {
            case.case_id == "multi_param_i64_exact"
                && case.returned_i64 == Some(42)
                && case.status == TassadarGeneralizedAbiCaseStatus::Exact
        }));
        assert!(report.cases.iter().any(|case| {
            case.case_id == "host_handle_param_refusal"
                && case.status == TassadarGeneralizedAbiCaseStatus::Refused
        }));
    }

    #[test]
    fn generalized_abi_family_report_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_generalized_abi_family_report()?;
        let bytes = std::fs::read(tassadar_generalized_abi_family_report_path())?;
        let committed: TassadarGeneralizedAbiFamilyReport = serde_json::from_slice(&bytes)?;
        assert_eq!(report, committed);
        Ok(())
    }

    #[test]
    fn write_generalized_abi_family_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let report_path = temp_dir
            .path()
            .join("tassadar_generalized_abi_family_report.json");
        let report = write_tassadar_generalized_abi_family_report(&report_path)?;
        let persisted: TassadarGeneralizedAbiFamilyReport =
            serde_json::from_slice(&std::fs::read(&report_path)?)?;
        assert_eq!(report, persisted);
        Ok(())
    }
}
