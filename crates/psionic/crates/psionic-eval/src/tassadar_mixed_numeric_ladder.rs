use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{
    TassadarMixedNumericExpectation, TassadarMixedNumericFixture,
    lower_tassadar_mixed_numeric_fixture, tassadar_seeded_mixed_numeric_fixtures,
};
use psionic_ir::{
    TassadarMixedNumericProfileLadderContract, TassadarMixedNumericSupportPosture,
    tassadar_mixed_numeric_profile_ladder_contract,
};
use psionic_runtime::{
    TassadarMixedNumericError, TassadarMixedNumericExecution, TassadarMixedNumericResult,
    execute_tassadar_mixed_numeric_program,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_mixed_numeric_profile_ladder_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMixedNumericCaseStatus {
    Exact,
    BoundedApproximate,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedNumericCaseReport {
    pub case_id: String,
    pub source_ref: String,
    pub profile_id: String,
    pub status: TassadarMixedNumericCaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_f32_bits_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_f32_bits_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_i32: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_i32: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedNumericProfileCoverage {
    pub profile_id: String,
    pub support_posture: TassadarMixedNumericSupportPosture,
    pub exact_case_ids: Vec<String>,
    pub bounded_approximate_case_ids: Vec<String>,
    pub refusal_case_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedNumericProfileLadderReport {
    pub schema_version: u16,
    pub report_id: String,
    pub ladder_contract: TassadarMixedNumericProfileLadderContract,
    pub exact_case_count: u16,
    pub bounded_approximate_case_count: u16,
    pub refusal_case_count: u16,
    pub cases: Vec<TassadarMixedNumericCaseReport>,
    pub profile_coverage: Vec<TassadarMixedNumericProfileCoverage>,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl TassadarMixedNumericProfileLadderReport {
    fn new(cases: Vec<TassadarMixedNumericCaseReport>) -> Self {
        let ladder_contract = tassadar_mixed_numeric_profile_ladder_contract();
        let exact_case_count = cases
            .iter()
            .filter(|case| case.status == TassadarMixedNumericCaseStatus::Exact)
            .count() as u16;
        let bounded_approximate_case_count = cases
            .iter()
            .filter(|case| case.status == TassadarMixedNumericCaseStatus::BoundedApproximate)
            .count() as u16;
        let refusal_case_count = cases
            .iter()
            .filter(|case| case.status == TassadarMixedNumericCaseStatus::Refused)
            .count() as u16;
        let profile_coverage = ladder_contract
            .profiles
            .iter()
            .map(|profile| TassadarMixedNumericProfileCoverage {
                profile_id: profile.profile_id.clone(),
                support_posture: profile.support_posture,
                exact_case_ids: cases
                    .iter()
                    .filter(|case| {
                        case.profile_id == profile.profile_id
                            && case.status == TassadarMixedNumericCaseStatus::Exact
                    })
                    .map(|case| case.case_id.clone())
                    .collect(),
                bounded_approximate_case_ids: cases
                    .iter()
                    .filter(|case| {
                        case.profile_id == profile.profile_id
                            && case.status == TassadarMixedNumericCaseStatus::BoundedApproximate
                    })
                    .map(|case| case.case_id.clone())
                    .collect(),
                refusal_case_ids: cases
                    .iter()
                    .filter(|case| {
                        case.profile_id == profile.profile_id
                            && case.status == TassadarMixedNumericCaseStatus::Refused
                    })
                    .map(|case| case.case_id.clone())
                    .collect(),
            })
            .collect::<Vec<_>>();
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.mixed_numeric_profile_ladder.report.v1"),
            ladder_contract,
            exact_case_count,
            bounded_approximate_case_count,
            refusal_case_count,
            cases,
            profile_coverage,
            claim_boundary: String::from(
                "this report stages numeric widening into exact scalar-f32, exact mixed i32/f32, and bounded-approximate f64-to-f32 cases while keeping malformed conversions and out-of-profile inputs on typed refusal paths. It does not claim arbitrary Wasm numeric closure or full f64 exactness",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_mixed_numeric_profile_ladder_report|", &report);
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarMixedNumericProfileLadderReportError {
    #[error("case `{case_id}` float result mismatch: expected `{expected}`, observed `{observed}`")]
    FloatMismatch {
        case_id: String,
        expected: String,
        observed: String,
    },
    #[error("case `{case_id}` integer result mismatch: expected `{expected}`, observed `{observed}`")]
    IntegerMismatch {
        case_id: String,
        expected: i32,
        observed: i32,
    },
    #[error("case `{case_id}` expected refusal `{expected}` but observed `{observed}`")]
    RefusalMismatch {
        case_id: String,
        expected: String,
        observed: String,
    },
    #[error("case `{case_id}` expected a refusal but execution succeeded")]
    ExpectedRefusal { case_id: String },
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write mixed-numeric ladder report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

pub fn build_tassadar_mixed_numeric_profile_ladder_report(
) -> Result<TassadarMixedNumericProfileLadderReport, TassadarMixedNumericProfileLadderReportError>
{
    let mut cases = Vec::new();
    for fixture in tassadar_seeded_mixed_numeric_fixtures() {
        let artifact = lower_tassadar_mixed_numeric_fixture(&fixture);
        let case = match execute_tassadar_mixed_numeric_program(&artifact.program) {
            Ok(execution) => build_success_case(&fixture, &execution)?,
            Err(error) => build_refusal_case(&fixture, error)?,
        };
        cases.push(case);
    }
    Ok(TassadarMixedNumericProfileLadderReport::new(cases))
}

pub fn tassadar_mixed_numeric_profile_ladder_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF)
}

pub fn write_tassadar_mixed_numeric_profile_ladder_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarMixedNumericProfileLadderReport, TassadarMixedNumericProfileLadderReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarMixedNumericProfileLadderReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_mixed_numeric_profile_ladder_report()?;
    let json = serde_json::to_string_pretty(&report).expect("report should serialize");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarMixedNumericProfileLadderReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_success_case(
    fixture: &TassadarMixedNumericFixture,
    execution: &TassadarMixedNumericExecution,
) -> Result<TassadarMixedNumericCaseReport, TassadarMixedNumericProfileLadderReportError> {
    match (fixture.expected(), &execution.result, execution.support_posture) {
        (
            TassadarMixedNumericExpectation::F32Bits { bits: expected_bits },
            TassadarMixedNumericResult::F32Bits { bits: observed_bits },
            TassadarMixedNumericSupportPosture::Exact,
        ) => {
            if expected_bits != observed_bits {
                return Err(TassadarMixedNumericProfileLadderReportError::FloatMismatch {
                    case_id: String::from(fixture.case_id()),
                    expected: format!("0x{expected_bits:08x}"),
                    observed: format!("0x{observed_bits:08x}"),
                });
            }
            Ok(base_case(fixture, execution.profile_id.clone(), TassadarMixedNumericCaseStatus::Exact)
                .with_f32(*expected_bits, *observed_bits))
        }
        (
            TassadarMixedNumericExpectation::I32 { value: expected_value },
            TassadarMixedNumericResult::I32 { value: observed_value },
            TassadarMixedNumericSupportPosture::Exact,
        ) => {
            if expected_value != observed_value {
                return Err(TassadarMixedNumericProfileLadderReportError::IntegerMismatch {
                    case_id: String::from(fixture.case_id()),
                    expected: *expected_value,
                    observed: *observed_value,
                });
            }
            Ok(base_case(
                fixture,
                execution.profile_id.clone(),
                TassadarMixedNumericCaseStatus::Exact,
            )
            .with_i32(*expected_value, *observed_value))
        }
        (
            TassadarMixedNumericExpectation::BoundedApproximateF32Bits { bits: expected_bits },
            TassadarMixedNumericResult::ApproximateF32Bits {
                bits: observed_bits, ..
            },
            TassadarMixedNumericSupportPosture::BoundedApproximate,
        ) => {
            if expected_bits != observed_bits {
                return Err(TassadarMixedNumericProfileLadderReportError::FloatMismatch {
                    case_id: String::from(fixture.case_id()),
                    expected: format!("0x{expected_bits:08x}"),
                    observed: format!("0x{observed_bits:08x}"),
                });
            }
            Ok(base_case(
                fixture,
                execution.profile_id.clone(),
                TassadarMixedNumericCaseStatus::BoundedApproximate,
            )
            .with_f32(*expected_bits, *observed_bits))
        }
        (TassadarMixedNumericExpectation::Refusal { .. }, _, _) => {
            Err(TassadarMixedNumericProfileLadderReportError::ExpectedRefusal {
                case_id: String::from(fixture.case_id()),
            })
        }
        _ => Err(TassadarMixedNumericProfileLadderReportError::ExpectedRefusal {
            case_id: String::from(fixture.case_id()),
        }),
    }
}

fn build_refusal_case(
    fixture: &TassadarMixedNumericFixture,
    error: TassadarMixedNumericError,
) -> Result<TassadarMixedNumericCaseReport, TassadarMixedNumericProfileLadderReportError> {
    let observed_reason = match &error {
        TassadarMixedNumericError::NonExactI32ToF32 { reason_id, .. }
        | TassadarMixedNumericError::InvalidF32ToI32 { reason_id, .. }
        | TassadarMixedNumericError::InvalidF64ToF32 { reason_id, .. } => reason_id.clone(),
    };
    match fixture.expected() {
        TassadarMixedNumericExpectation::Refusal {
            reason_id: expected_reason,
            detail,
        } => {
            if expected_reason != &observed_reason {
                return Err(TassadarMixedNumericProfileLadderReportError::RefusalMismatch {
                    case_id: String::from(fixture.case_id()),
                    expected: expected_reason.clone(),
                    observed: observed_reason,
                });
            }
            Ok(TassadarMixedNumericCaseReport {
                case_id: String::from(fixture.case_id()),
                source_ref: String::from(fixture.source_ref()),
                profile_id: profile_id_for_fixture(fixture),
                status: TassadarMixedNumericCaseStatus::Refused,
                observed_f32_bits_hex: None,
                expected_f32_bits_hex: None,
                observed_i32: None,
                expected_i32: None,
                refusal_reason_id: Some(expected_reason.clone()),
                refusal_detail: Some(detail.clone()),
            })
        }
        _ => Err(TassadarMixedNumericProfileLadderReportError::ExpectedRefusal {
            case_id: String::from(fixture.case_id()),
        }),
    }
}

#[derive(Clone)]
struct CaseBuilder {
    case_id: String,
    source_ref: String,
    profile_id: String,
    status: TassadarMixedNumericCaseStatus,
    observed_f32_bits_hex: Option<String>,
    expected_f32_bits_hex: Option<String>,
    observed_i32: Option<i32>,
    expected_i32: Option<i32>,
}

impl CaseBuilder {
    fn with_f32(mut self, expected_bits: u32, observed_bits: u32) -> TassadarMixedNumericCaseReport {
        self.expected_f32_bits_hex = Some(format!("0x{expected_bits:08x}"));
        self.observed_f32_bits_hex = Some(format!("0x{observed_bits:08x}"));
        self.finish()
    }

    fn with_i32(mut self, expected_value: i32, observed_value: i32) -> TassadarMixedNumericCaseReport {
        self.expected_i32 = Some(expected_value);
        self.observed_i32 = Some(observed_value);
        self.finish()
    }

    fn finish(self) -> TassadarMixedNumericCaseReport {
        TassadarMixedNumericCaseReport {
            case_id: self.case_id,
            source_ref: self.source_ref,
            profile_id: self.profile_id,
            status: self.status,
            observed_f32_bits_hex: self.observed_f32_bits_hex,
            expected_f32_bits_hex: self.expected_f32_bits_hex,
            observed_i32: self.observed_i32,
            expected_i32: self.expected_i32,
            refusal_reason_id: None,
            refusal_detail: None,
        }
    }
}

fn base_case(
    fixture: &TassadarMixedNumericFixture,
    profile_id: String,
    status: TassadarMixedNumericCaseStatus,
) -> CaseBuilder {
    CaseBuilder {
        case_id: String::from(fixture.case_id()),
        source_ref: String::from(fixture.source_ref()),
        profile_id,
        status,
        observed_f32_bits_hex: None,
        expected_f32_bits_hex: None,
        observed_i32: None,
        expected_i32: None,
    }
}

fn profile_id_for_fixture(fixture: &TassadarMixedNumericFixture) -> String {
    match fixture {
        TassadarMixedNumericFixture::F64ToF32Bounded { .. } => {
            String::from("tassadar.numeric_profile.bounded_f64_conversion.v1")
        }
        _ => String::from("tassadar.numeric_profile.mixed_i32_f32.v1"),
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
        TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF, TassadarMixedNumericCaseStatus,
        TassadarMixedNumericProfileLadderReport, build_tassadar_mixed_numeric_profile_ladder_report,
        repo_root, write_tassadar_mixed_numeric_profile_ladder_report,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn mixed_numeric_profile_ladder_keeps_exact_and_approximate_separate() {
        let report = build_tassadar_mixed_numeric_profile_ladder_report().expect("report");

        assert_eq!(report.exact_case_count, 3);
        assert_eq!(report.bounded_approximate_case_count, 1);
        assert_eq!(report.refusal_case_count, 3);
        assert!(report
            .cases
            .iter()
            .any(|case| case.status == TassadarMixedNumericCaseStatus::BoundedApproximate));
    }

    #[test]
    fn mixed_numeric_profile_ladder_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed: TassadarMixedNumericProfileLadderReport =
            read_repo_json(TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF)?;
        let current = build_tassadar_mixed_numeric_profile_ladder_report()?;

        assert_eq!(current, committed);
        Ok(())
    }

    #[test]
    fn write_mixed_numeric_profile_ladder_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_path = repo_root().join(TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF);
        let written = write_tassadar_mixed_numeric_profile_ladder_report(&output_path)?;
        let reread: TassadarMixedNumericProfileLadderReport =
            read_repo_json(TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF)?;

        assert_eq!(written, reread);
        Ok(())
    }
}
