use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{
    tassadar_compiler_frozen_core_wasm_window, tassadar_frozen_core_wasm_binary_fixture_refs,
    tassadar_frozen_core_wasm_negative_validation_fixtures,
    tassadar_frozen_core_wasm_text_fixture_refs,
};
use psionic_ir::TassadarFrozenCoreWasmWindow;
use psionic_runtime::{
    TassadarFrozenCoreWasmValidationError, validate_tassadar_frozen_core_wasm_binary,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF, TASSADAR_WASM_CONFORMANCE_REPORT_REF,
    TassadarCompilePipelineMatrixReport, TassadarWasmConformanceReport,
};

const TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json";

/// Validation status for one frozen-window harness case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFrozenCoreWasmHarnessValidationStatus {
    /// The binary validated inside the frozen window.
    Admitted,
    /// The binary or text fixture refused as out-of-window.
    RefusedOutOfWindow,
}

/// One validation case inside the official frozen-window harness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFrozenCoreWasmHarnessValidationCase {
    /// Stable case id.
    pub case_id: String,
    /// Stable harness axis.
    pub harness_axis: String,
    /// Stable source ref.
    pub source_ref: String,
    /// Stable digest over the validated Wasm bytes.
    pub wasm_binary_digest: String,
    /// Admitted or refused status.
    pub status: TassadarFrozenCoreWasmHarnessValidationStatus,
    /// Refused proposal family when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_family_id: Option<String>,
    /// Optional validation detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Committed report declaring the frozen core-Wasm window and official harness packaging.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFrozenCoreWasmWindowReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report id.
    pub report_id: String,
    /// Shared frozen semantic window.
    pub frozen_window: TassadarFrozenCoreWasmWindow,
    /// Compile report ref reused as the official text/binary harness anchor.
    pub compile_pipeline_matrix_report_ref: String,
    /// Compile report id reused as the official text/binary harness anchor.
    pub compile_pipeline_matrix_report_id: String,
    /// Canonical text fixture refs in the harness.
    pub official_text_fixture_refs: Vec<String>,
    /// Canonical binary fixture refs in the harness.
    pub official_binary_fixture_refs: Vec<String>,
    /// Official validation cases for admitted and refused rows.
    pub validation_cases: Vec<TassadarFrozenCoreWasmHarnessValidationCase>,
    /// Conformance report ref reused as the execution harness anchor.
    pub execution_harness_report_ref: String,
    /// Conformance report id reused as the execution harness anchor.
    pub execution_harness_report_id: String,
    /// Reference authority id for the execution harness.
    pub execution_reference_authority_id: String,
    /// Case families exercised by the execution harness.
    pub execution_case_family_ids: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarFrozenCoreWasmWindowReport {
    fn new(
        frozen_window: TassadarFrozenCoreWasmWindow,
        compile_report: TassadarCompilePipelineMatrixReport,
        conformance_report: TassadarWasmConformanceReport,
        validation_cases: Vec<TassadarFrozenCoreWasmHarnessValidationCase>,
    ) -> Result<Self, TassadarFrozenCoreWasmWindowReportError> {
        let expected_text_fixture_refs = tassadar_frozen_core_wasm_text_fixture_refs();
        let expected_binary_fixture_refs = tassadar_frozen_core_wasm_binary_fixture_refs();
        let mut actual_text_fixture_refs = compile_report
            .cases
            .iter()
            .filter(|case| {
                matches!(
                    case.source_kind,
                    psionic_runtime::TassadarProgramSourceKind::WasmText
                )
            })
            .map(|case| case.source_ref.clone())
            .collect::<Vec<_>>();
        actual_text_fixture_refs.sort();
        actual_text_fixture_refs.dedup();
        if actual_text_fixture_refs != expected_text_fixture_refs {
            return Err(TassadarFrozenCoreWasmWindowReportError::TextHarnessDrift {
                expected: expected_text_fixture_refs,
                actual: actual_text_fixture_refs,
            });
        }
        let mut actual_binary_fixture_refs = compile_report
            .cases
            .iter()
            .filter_map(|case| case.wasm_binary_ref.clone())
            .collect::<Vec<_>>();
        actual_binary_fixture_refs.sort();
        actual_binary_fixture_refs.dedup();
        if actual_binary_fixture_refs != expected_binary_fixture_refs {
            return Err(
                TassadarFrozenCoreWasmWindowReportError::BinaryHarnessDrift {
                    expected: expected_binary_fixture_refs,
                    actual: actual_binary_fixture_refs,
                },
            );
        }
        let mut report = Self {
            schema_version: TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.frozen_core_wasm_window.report.v1"),
            frozen_window,
            compile_pipeline_matrix_report_ref: String::from(
                TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF,
            ),
            compile_pipeline_matrix_report_id: compile_report.report_id,
            official_text_fixture_refs: actual_text_fixture_refs,
            official_binary_fixture_refs: actual_binary_fixture_refs,
            validation_cases,
            execution_harness_report_ref: String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF),
            execution_harness_report_id: conformance_report.report_id,
            execution_reference_authority_id: conformance_report.reference_authority_id,
            execution_case_family_ids: conformance_report.case_family_ids,
            claim_boundary: String::from(
                "declares one frozen core-Wasm semantic window and official harness over text fixtures, binary fixtures, validator-backed refusal cases, and the existing differential execution report; it fixes the closure target and current out-of-window families, but it does not by itself claim full Wasm closure or arbitrary Wasm execution",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_frozen_core_wasm_window_report|", &report);
        Ok(report)
    }
}

/// Build failures for the frozen core-Wasm window report.
#[derive(Debug, Error)]
pub enum TassadarFrozenCoreWasmWindowReportError {
    /// One committed JSON artifact could not be read.
    #[error("failed to read committed JSON `{path}`: {error}")]
    ReadJson {
        /// Repo-relative path.
        path: String,
        /// IO or decode summary.
        error: String,
    },
    /// One binary fixture could not be read.
    #[error("failed to read Wasm fixture `{path}`: {error}")]
    ReadBinary {
        /// Repo-relative path.
        path: String,
        /// IO failure.
        error: std::io::Error,
    },
    /// The compile report drifted from the compiler-owned text harness.
    #[error("frozen core-Wasm text harness drifted: expected {expected:?}, actual {actual:?}")]
    TextHarnessDrift {
        /// Expected refs.
        expected: Vec<String>,
        /// Actual refs.
        actual: Vec<String>,
    },
    /// The compile report drifted from the compiler-owned binary harness.
    #[error("frozen core-Wasm binary harness drifted: expected {expected:?}, actual {actual:?}")]
    BinaryHarnessDrift {
        /// Expected refs.
        expected: Vec<String>,
        /// Actual refs.
        actual: Vec<String>,
    },
    /// One positive fixture failed validation.
    #[error("frozen core-Wasm positive fixture `{source_ref}` unexpectedly refused: {detail}")]
    PositiveFixtureRefused {
        /// Source ref.
        source_ref: String,
        /// Refusal detail.
        detail: String,
    },
    /// One negative fixture did not refuse with the expected proposal family.
    #[error(
        "frozen core-Wasm negative fixture `{source_ref}` drifted: expected `{expected}`, actual `{actual}`"
    )]
    NegativeFixtureDrift {
        /// Source ref.
        source_ref: String,
        /// Expected proposal family.
        expected: String,
        /// Actual validation outcome summary.
        actual: String,
    },
    /// Failed to create the report output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the report.
    #[error("failed to write frozen core-Wasm window report `{path}`: {error}")]
    Write {
        /// Output path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
}

/// Builds the committed frozen core-Wasm window report.
pub fn build_tassadar_frozen_core_wasm_window_report()
-> Result<TassadarFrozenCoreWasmWindowReport, TassadarFrozenCoreWasmWindowReportError> {
    let compile_report = read_repo_json::<TassadarCompilePipelineMatrixReport>(
        TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF,
    )?;
    let conformance_report =
        read_repo_json::<TassadarWasmConformanceReport>(TASSADAR_WASM_CONFORMANCE_REPORT_REF)?;
    let mut validation_cases = Vec::new();
    for binary_ref in tassadar_frozen_core_wasm_binary_fixture_refs() {
        let bytes = fs::read(repo_root().join(&binary_ref)).map_err(|error| {
            TassadarFrozenCoreWasmWindowReportError::ReadBinary {
                path: binary_ref.clone(),
                error,
            }
        })?;
        if let Err(error) = validate_tassadar_frozen_core_wasm_binary(&bytes) {
            return Err(
                TassadarFrozenCoreWasmWindowReportError::PositiveFixtureRefused {
                    source_ref: binary_ref,
                    detail: error.to_string(),
                },
            );
        }
        validation_cases.push(TassadarFrozenCoreWasmHarnessValidationCase {
            case_id: format!(
                "admitted.{}",
                Path::new(&binary_ref)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("fixture")
            ),
            harness_axis: String::from("binary_validation"),
            source_ref: binary_ref,
            wasm_binary_digest: stable_bytes_digest(&bytes),
            status: TassadarFrozenCoreWasmHarnessValidationStatus::Admitted,
            proposal_family_id: None,
            detail: None,
        });
    }
    for fixture in tassadar_frozen_core_wasm_negative_validation_fixtures() {
        match validate_tassadar_frozen_core_wasm_binary(&fixture.wasm_binary) {
            Err(TassadarFrozenCoreWasmValidationError::ProposalFamilyUnsupported {
                proposal_family_id,
                detail,
                ..
            }) if proposal_family_id == fixture.proposal_family_id => {
                validation_cases.push(TassadarFrozenCoreWasmHarnessValidationCase {
                    case_id: fixture.case_id,
                    harness_axis: String::from("proposal_refusal"),
                    source_ref: fixture.source_ref,
                    wasm_binary_digest: stable_bytes_digest(&fixture.wasm_binary),
                    status: TassadarFrozenCoreWasmHarnessValidationStatus::RefusedOutOfWindow,
                    proposal_family_id: Some(proposal_family_id),
                    detail: Some(detail),
                });
            }
            Err(error) => {
                return Err(
                    TassadarFrozenCoreWasmWindowReportError::NegativeFixtureDrift {
                        source_ref: fixture.source_ref,
                        expected: fixture.proposal_family_id,
                        actual: error.to_string(),
                    },
                );
            }
            Ok(()) => {
                return Err(
                    TassadarFrozenCoreWasmWindowReportError::NegativeFixtureDrift {
                        source_ref: fixture.source_ref,
                        expected: fixture.proposal_family_id,
                        actual: String::from("validator admitted the out-of-window fixture"),
                    },
                );
            }
        }
    }
    validation_cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    TassadarFrozenCoreWasmWindowReport::new(
        tassadar_compiler_frozen_core_wasm_window(),
        compile_report,
        conformance_report,
        validation_cases,
    )
}

/// Returns the canonical absolute path for the committed frozen core-Wasm window report.
pub fn tassadar_frozen_core_wasm_window_report_path() -> PathBuf {
    repo_root().join(TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF)
}

/// Writes the committed frozen core-Wasm window report.
pub fn write_tassadar_frozen_core_wasm_window_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarFrozenCoreWasmWindowReport, TassadarFrozenCoreWasmWindowReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarFrozenCoreWasmWindowReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_frozen_core_wasm_window_report()?;
    let bytes = serde_json::to_vec_pretty(&report)
        .expect("frozen core-Wasm window report should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarFrozenCoreWasmWindowReportError::Write {
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
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    repo_relative_path: &str,
) -> Result<T, TassadarFrozenCoreWasmWindowReportError> {
    let path = repo_root().join(repo_relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarFrozenCoreWasmWindowReportError::ReadJson {
            path: repo_relative_path.to_string(),
            error: error.to_string(),
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarFrozenCoreWasmWindowReportError::ReadJson {
            path: repo_relative_path.to_string(),
            error: error.to_string(),
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF, TassadarFrozenCoreWasmWindowReport,
        build_tassadar_frozen_core_wasm_window_report,
        tassadar_frozen_core_wasm_window_report_path,
        write_tassadar_frozen_core_wasm_window_report,
    };

    #[test]
    fn frozen_core_wasm_window_report_imports_official_harness() {
        let report = build_tassadar_frozen_core_wasm_window_report()
            .expect("frozen core-Wasm window report should build");
        assert_eq!(
            report.frozen_window.window_id,
            "tassadar.frozen_core_wasm.window.v1"
        );
        assert_eq!(
            report.official_text_fixture_refs,
            vec![
                String::from("fixtures/tassadar/sources/tassadar_memory_lookup_kernel.wat"),
                String::from("fixtures/tassadar/sources/tassadar_multi_export_kernel.wat"),
                String::from("fixtures/tassadar/sources/tassadar_param_abi_kernel.wat"),
            ]
        );
        assert_eq!(report.validation_cases.len(), 5);
        assert!(
            report
                .validation_cases
                .iter()
                .any(|case| case.case_id == "proposal.float_value_type_refused"
                    && case.proposal_family_id.as_deref() == Some("floating_point"))
        );
        assert_eq!(
            report.execution_reference_authority_id,
            "wasmi.reference.v1"
        );
    }

    #[test]
    fn frozen_core_wasm_window_report_matches_committed_truth() {
        let report = build_tassadar_frozen_core_wasm_window_report()
            .expect("frozen core-Wasm window report should build");
        let path = tassadar_frozen_core_wasm_window_report_path();
        let bytes = std::fs::read(&path).expect("committed report should exist");
        let persisted: TassadarFrozenCoreWasmWindowReport =
            serde_json::from_slice(&bytes).expect("committed report should decode");
        assert_eq!(
            persisted,
            report,
            "run the example to refresh `{}`",
            path.display()
        );
    }

    #[test]
    fn write_frozen_core_wasm_window_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_frozen_core_wasm_window_report.json");
        let report = write_tassadar_frozen_core_wasm_window_report(&output_path)
            .expect("frozen core-Wasm window report should write");
        let bytes = std::fs::read(&output_path).expect("persisted report should exist");
        let persisted: TassadarFrozenCoreWasmWindowReport =
            serde_json::from_slice(&bytes).expect("persisted report should decode");
        assert_eq!(persisted, report);
        std::fs::remove_file(&output_path).expect("temp report should be removable");
        assert_eq!(
            PathBuf::from(TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF)
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_frozen_core_wasm_window_report.json")
        );
    }
}
