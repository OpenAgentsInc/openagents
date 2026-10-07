use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{
    TassadarFloatSemanticsExpectation, TassadarFloatSemanticsFixture,
    TassadarFloatSemanticsLoweringError, lower_tassadar_float_semantics_fixture,
    tassadar_seeded_float_semantics_fixtures,
};
use psionic_runtime::{
    TassadarFloatSemanticsExecution, TassadarFloatSemanticsPolicy,
    TassadarFloatSemanticsResult, execute_tassadar_float_semantics_program,
    tassadar_float_semantics_policy,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_FLOAT_SEMANTICS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_float_semantics_comparison_matrix_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFloatSemanticsCaseStatus {
    Exact,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatSemanticsCaseReport {
    pub case_id: String,
    pub source_ref: String,
    pub family_id: String,
    pub profile_id: String,
    pub status: TassadarFloatSemanticsCaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_f32_bits_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_f32_bits_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_i32: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_i32: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_regime_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatSemanticsComparisonMatrixReport {
    pub schema_version: u16,
    pub report_id: String,
    pub policy: TassadarFloatSemanticsPolicy,
    pub generated_from_refs: Vec<String>,
    pub exact_case_count: u16,
    pub refusal_case_count: u16,
    pub cases: Vec<TassadarFloatSemanticsCaseReport>,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl TassadarFloatSemanticsComparisonMatrixReport {
    fn new(cases: Vec<TassadarFloatSemanticsCaseReport>, generated_from_refs: Vec<String>) -> Self {
        let exact_case_count = cases
            .iter()
            .filter(|case| case.status == TassadarFloatSemanticsCaseStatus::Exact)
            .count() as u16;
        let refusal_case_count = cases
            .iter()
            .filter(|case| case.status == TassadarFloatSemanticsCaseStatus::Refused)
            .count() as u16;
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.float_semantics.comparison_matrix.report.v1"),
            policy: tassadar_float_semantics_policy(),
            generated_from_refs,
            exact_case_count,
            refusal_case_count,
            cases,
            claim_boundary: String::from(
                "this report proves one bounded scalar-f32 semantics lane only: canonical quiet-NaN normalization, nearest-ties-to-even arithmetic, ordered Wasm-style comparisons, and explicit refusal on f64, NaN-payload-preservation, and non-CPU fast-math regimes. It does not claim arbitrary Wasm float closure, f64 execution, or backend-invariant served publication",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_float_semantics_comparison_matrix_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarFloatSemanticsReportError {
    #[error("case `{case_id}` lowered unexpectedly for a refused regime")]
    UnexpectedLoweredRefusal { case_id: String },
    #[error("case `{case_id}` expected a lowering refusal but lowered successfully")]
    ExpectedRefusal { case_id: String },
    #[error("case `{case_id}` float result mismatch: expected `{expected}`, observed `{observed}`")]
    FloatMismatch {
        case_id: String,
        expected: String,
        observed: String,
    },
    #[error("case `{case_id}` comparison result mismatch: expected `{expected}`, observed `{observed}`")]
    ComparisonMismatch {
        case_id: String,
        expected: i32,
        observed: i32,
    },
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write float-semantics report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read committed float-semantics report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode committed float-semantics report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_float_semantics_comparison_matrix_report(
) -> Result<TassadarFloatSemanticsComparisonMatrixReport, TassadarFloatSemanticsReportError> {
    let fixtures = tassadar_seeded_float_semantics_fixtures();
    let mut generated_from_refs = vec![String::from(TASSADAR_FLOAT_SEMANTICS_REPORT_REF)];
    let mut cases = Vec::with_capacity(fixtures.len());
    for fixture in fixtures {
        generated_from_refs.push(String::from(fixture.source_ref()));
        cases.push(build_case_report(&fixture)?);
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();
    Ok(TassadarFloatSemanticsComparisonMatrixReport::new(
        cases,
        generated_from_refs,
    ))
}

pub fn tassadar_float_semantics_report_path() -> PathBuf {
    repo_root().join(TASSADAR_FLOAT_SEMANTICS_REPORT_REF)
}

pub fn write_tassadar_float_semantics_comparison_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarFloatSemanticsComparisonMatrixReport, TassadarFloatSemanticsReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarFloatSemanticsReportError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_tassadar_float_semantics_comparison_matrix_report()?;
    let json = serde_json::to_string_pretty(&report).expect("report should serialize");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarFloatSemanticsReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_case_report(
    fixture: &TassadarFloatSemanticsFixture,
) -> Result<TassadarFloatSemanticsCaseReport, TassadarFloatSemanticsReportError> {
    match lower_tassadar_float_semantics_fixture(fixture) {
        Ok(artifact) => {
            let execution = execute_tassadar_float_semantics_program(&artifact.program);
            build_exact_case_report(fixture, &execution)
        }
        Err(TassadarFloatSemanticsLoweringError::UnsupportedRegime {
            case_id,
            regime_id,
            detail,
        }) => match fixture.expected() {
            TassadarFloatSemanticsExpectation::Refusal { .. } => Ok(TassadarFloatSemanticsCaseReport {
                case_id,
                source_ref: String::from(fixture.source_ref()),
                family_id: String::from("tassadar.float_semantics.matrix.v1"),
                profile_id: String::from("tassadar.float_semantics.scalar_f32.v1"),
                status: TassadarFloatSemanticsCaseStatus::Refused,
                operation_id: None,
                observed_f32_bits_hex: None,
                expected_f32_bits_hex: None,
                observed_i32: None,
                expected_i32: None,
                refusal_regime_id: Some(regime_id),
                refusal_detail: Some(detail),
            }),
            _ => Err(TassadarFloatSemanticsReportError::UnexpectedLoweredRefusal { case_id }),
        },
    }
}

fn build_exact_case_report(
    fixture: &TassadarFloatSemanticsFixture,
    execution: &TassadarFloatSemanticsExecution,
) -> Result<TassadarFloatSemanticsCaseReport, TassadarFloatSemanticsReportError> {
    let operation_id = Some(match fixture {
        TassadarFloatSemanticsFixture::Arithmetic { op, .. } => format!("f32.{}", op.as_str()),
        TassadarFloatSemanticsFixture::Comparison { op, .. } => format!("f32.{}", op.as_str()),
        TassadarFloatSemanticsFixture::UnsupportedRegime { .. } => {
            return Err(TassadarFloatSemanticsReportError::ExpectedRefusal {
                case_id: String::from(fixture.case_id()),
            });
        }
    });
    match (fixture.expected(), &execution.result) {
        (
            TassadarFloatSemanticsExpectation::F32Bits { bits: expected_bits },
            TassadarFloatSemanticsResult::F32Bits { bits: observed_bits },
        ) => {
            if expected_bits != observed_bits {
                return Err(TassadarFloatSemanticsReportError::FloatMismatch {
                    case_id: String::from(fixture.case_id()),
                    expected: format!("0x{expected_bits:08x}"),
                    observed: format!("0x{observed_bits:08x}"),
                });
            }
            Ok(TassadarFloatSemanticsCaseReport {
                case_id: String::from(fixture.case_id()),
                source_ref: String::from(fixture.source_ref()),
                family_id: execution.family_id.clone(),
                profile_id: execution.profile_id.clone(),
                status: TassadarFloatSemanticsCaseStatus::Exact,
                operation_id,
                observed_f32_bits_hex: Some(format!("0x{observed_bits:08x}")),
                expected_f32_bits_hex: Some(format!("0x{expected_bits:08x}")),
                observed_i32: None,
                expected_i32: None,
                refusal_regime_id: None,
                refusal_detail: None,
            })
        }
        (
            TassadarFloatSemanticsExpectation::I32 { value: expected_value },
            TassadarFloatSemanticsResult::I32 { value: observed_value },
        ) => {
            if expected_value != observed_value {
                return Err(TassadarFloatSemanticsReportError::ComparisonMismatch {
                    case_id: String::from(fixture.case_id()),
                    expected: *expected_value,
                    observed: *observed_value,
                });
            }
            Ok(TassadarFloatSemanticsCaseReport {
                case_id: String::from(fixture.case_id()),
                source_ref: String::from(fixture.source_ref()),
                family_id: execution.family_id.clone(),
                profile_id: execution.profile_id.clone(),
                status: TassadarFloatSemanticsCaseStatus::Exact,
                operation_id,
                observed_f32_bits_hex: None,
                expected_f32_bits_hex: None,
                observed_i32: Some(*observed_value),
                expected_i32: Some(*expected_value),
                refusal_regime_id: None,
                refusal_detail: None,
            })
        }
        (TassadarFloatSemanticsExpectation::Refusal { .. }, _) => {
            Err(TassadarFloatSemanticsReportError::ExpectedRefusal {
                case_id: String::from(fixture.case_id()),
            })
        }
        _ => Err(TassadarFloatSemanticsReportError::ExpectedRefusal {
            case_id: String::from(fixture.case_id()),
        }),
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
        TASSADAR_FLOAT_SEMANTICS_REPORT_REF, TassadarFloatSemanticsCaseStatus,
        TassadarFloatSemanticsComparisonMatrixReport,
        build_tassadar_float_semantics_comparison_matrix_report, repo_root,
        write_tassadar_float_semantics_comparison_matrix_report,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn float_semantics_matrix_keeps_refusals_explicit() {
        let report = build_tassadar_float_semantics_comparison_matrix_report().expect("report");

        assert_eq!(report.exact_case_count, 8);
        assert_eq!(report.refusal_case_count, 2);
        assert!(report
            .cases
            .iter()
            .any(|case| case.status == TassadarFloatSemanticsCaseStatus::Refused
                && case.refusal_regime_id.as_deref() == Some("f64_scalar")));
    }

    #[test]
    fn float_semantics_matrix_matches_committed_truth() -> Result<(), Box<dyn std::error::Error>>
    {
        let committed: TassadarFloatSemanticsComparisonMatrixReport =
            read_repo_json(TASSADAR_FLOAT_SEMANTICS_REPORT_REF)?;
        let current = build_tassadar_float_semantics_comparison_matrix_report()?;

        assert_eq!(current, committed);
        Ok(())
    }

    #[test]
    fn write_float_semantics_matrix_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_path = repo_root().join(TASSADAR_FLOAT_SEMANTICS_REPORT_REF);
        let written = write_tassadar_float_semantics_comparison_matrix_report(&output_path)?;
        let reread: TassadarFloatSemanticsComparisonMatrixReport =
            read_repo_json(TASSADAR_FLOAT_SEMANTICS_REPORT_REF)?;

        assert_eq!(written, reread);
        Ok(())
    }
}
