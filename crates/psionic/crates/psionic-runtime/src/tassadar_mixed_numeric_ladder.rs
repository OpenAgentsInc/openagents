use psionic_ir::TassadarMixedNumericSupportPosture;
use serde::{Deserialize, Serialize};

use crate::canonicalize_tassadar_f32_bits;

pub const TASSADAR_EXACT_I32_TO_F32_LIMIT: i32 = 16_777_216;

/// One runtime-executable mixed-numeric program in the staged ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "program_kind", rename_all = "snake_case")]
pub enum TassadarMixedNumericProgram {
    I32ToF32ExactRange {
        program_id: String,
        profile_id: String,
        input_i32: i32,
    },
    F32ToI32TruncChecked {
        program_id: String,
        profile_id: String,
        input_f32_bits: u32,
    },
    MixedI32F32ScaleAddExact {
        program_id: String,
        profile_id: String,
        input_i32: i32,
        scale_f32_bits: u32,
        bias_f32_bits: u32,
    },
    F64ToF32Bounded {
        program_id: String,
        profile_id: String,
        input_f64_bits: u64,
    },
}

/// Runtime result for one mixed-numeric program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result_kind", rename_all = "snake_case")]
pub enum TassadarMixedNumericResult {
    I32 { value: i32 },
    F32Bits { bits: u32 },
    ApproximateF32Bits { bits: u32, detail: String },
}

/// Runtime-owned execution receipt for one mixed-numeric program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedNumericExecution {
    pub program_id: String,
    pub profile_id: String,
    pub support_posture: TassadarMixedNumericSupportPosture,
    pub result: TassadarMixedNumericResult,
}

/// Runtime refusal for one mixed-numeric program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error_kind", rename_all = "snake_case")]
pub enum TassadarMixedNumericError {
    NonExactI32ToF32 {
        input_i32: i32,
        reason_id: String,
    },
    InvalidF32ToI32 {
        input_f32_bits: u32,
        reason_id: String,
    },
    InvalidF64ToF32 {
        input_f64_bits: u64,
        reason_id: String,
    },
}

/// Executes one mixed-numeric program under the staged ladder.
pub fn execute_tassadar_mixed_numeric_program(
    program: &TassadarMixedNumericProgram,
) -> Result<TassadarMixedNumericExecution, TassadarMixedNumericError> {
    match program {
        TassadarMixedNumericProgram::I32ToF32ExactRange {
            program_id,
            profile_id,
            input_i32,
        } => {
            if input_i32.unsigned_abs() > TASSADAR_EXACT_I32_TO_F32_LIMIT as u32 {
                return Err(TassadarMixedNumericError::NonExactI32ToF32 {
                    input_i32: *input_i32,
                    reason_id: String::from("i32_to_f32_non_exact"),
                });
            }
            Ok(TassadarMixedNumericExecution {
                program_id: program_id.clone(),
                profile_id: profile_id.clone(),
                support_posture: TassadarMixedNumericSupportPosture::Exact,
                result: TassadarMixedNumericResult::F32Bits {
                    bits: (*input_i32 as f32).to_bits(),
                },
            })
        }
        TassadarMixedNumericProgram::F32ToI32TruncChecked {
            program_id,
            profile_id,
            input_f32_bits,
        } => {
            let value = f32::from_bits(*input_f32_bits);
            if value.is_nan() {
                return Err(TassadarMixedNumericError::InvalidF32ToI32 {
                    input_f32_bits: *input_f32_bits,
                    reason_id: String::from("f32_to_i32_invalid_nan"),
                });
            }
            if !value.is_finite() || value < i32::MIN as f32 || value > i32::MAX as f32 {
                return Err(TassadarMixedNumericError::InvalidF32ToI32 {
                    input_f32_bits: *input_f32_bits,
                    reason_id: String::from("f32_to_i32_invalid_out_of_range"),
                });
            }
            Ok(TassadarMixedNumericExecution {
                program_id: program_id.clone(),
                profile_id: profile_id.clone(),
                support_posture: TassadarMixedNumericSupportPosture::Exact,
                result: TassadarMixedNumericResult::I32 {
                    value: value.trunc() as i32,
                },
            })
        }
        TassadarMixedNumericProgram::MixedI32F32ScaleAddExact {
            program_id,
            profile_id,
            input_i32,
            scale_f32_bits,
            bias_f32_bits,
        } => {
            if input_i32.unsigned_abs() > TASSADAR_EXACT_I32_TO_F32_LIMIT as u32 {
                return Err(TassadarMixedNumericError::NonExactI32ToF32 {
                    input_i32: *input_i32,
                    reason_id: String::from("i32_to_f32_non_exact"),
                });
            }
            let value = *input_i32 as f32;
            let scaled = value * f32::from_bits(*scale_f32_bits) + f32::from_bits(*bias_f32_bits);
            Ok(TassadarMixedNumericExecution {
                program_id: program_id.clone(),
                profile_id: profile_id.clone(),
                support_posture: TassadarMixedNumericSupportPosture::Exact,
                result: TassadarMixedNumericResult::F32Bits {
                    bits: canonicalize_tassadar_f32_bits(scaled.to_bits()),
                },
            })
        }
        TassadarMixedNumericProgram::F64ToF32Bounded {
            program_id,
            profile_id,
            input_f64_bits,
        } => {
            let value = f64::from_bits(*input_f64_bits);
            if value.is_nan() {
                return Err(TassadarMixedNumericError::InvalidF64ToF32 {
                    input_f64_bits: *input_f64_bits,
                    reason_id: String::from("f64_nan_invalid"),
                });
            }
            if !value.is_finite() || value > f32::MAX as f64 || value < f32::MIN as f64 {
                return Err(TassadarMixedNumericError::InvalidF64ToF32 {
                    input_f64_bits: *input_f64_bits,
                    reason_id: String::from("f64_out_of_range"),
                });
            }
            let narrowed = value as f32;
            let posture = if f64::from(narrowed) == value {
                TassadarMixedNumericSupportPosture::Exact
            } else {
                TassadarMixedNumericSupportPosture::BoundedApproximate
            };
            let result = if posture == TassadarMixedNumericSupportPosture::Exact {
                TassadarMixedNumericResult::F32Bits {
                    bits: narrowed.to_bits(),
                }
            } else {
                TassadarMixedNumericResult::ApproximateF32Bits {
                    bits: narrowed.to_bits(),
                    detail: String::from(
                        "bounded f64 conversion narrowed into f32 with an explicit roundtrip-loss envelope",
                    ),
                }
            };
            Ok(TassadarMixedNumericExecution {
                program_id: program_id.clone(),
                profile_id: profile_id.clone(),
                support_posture: posture,
                result,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        execute_tassadar_mixed_numeric_program, TassadarMixedNumericError,
        TassadarMixedNumericProgram, TassadarMixedNumericResult,
    };
    use psionic_ir::{
        TassadarMixedNumericSupportPosture, TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID,
        TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
    };

    #[test]
    fn mixed_numeric_runtime_supports_exact_i32_f32_conversions() {
        let execution = execute_tassadar_mixed_numeric_program(
            &TassadarMixedNumericProgram::I32ToF32ExactRange {
                program_id: String::from("test.i32_to_f32"),
                profile_id: String::from(TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID),
                input_i32: 1024,
            },
        )
        .expect("execution");

        assert_eq!(
            execution.result,
            TassadarMixedNumericResult::F32Bits {
                bits: 1024.0f32.to_bits(),
            }
        );
    }

    #[test]
    fn mixed_numeric_runtime_keeps_bounded_f64_approximate_explicit() {
        let execution = execute_tassadar_mixed_numeric_program(
            &TassadarMixedNumericProgram::F64ToF32Bounded {
                program_id: String::from("test.f64_to_f32"),
                profile_id: String::from(TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID),
                input_f64_bits: 0.1f64.to_bits(),
            },
        )
        .expect("execution");

        assert_eq!(
            execution.support_posture,
            TassadarMixedNumericSupportPosture::BoundedApproximate
        );
    }

    #[test]
    fn mixed_numeric_runtime_refuses_invalid_conversions() {
        let err = execute_tassadar_mixed_numeric_program(
            &TassadarMixedNumericProgram::F32ToI32TruncChecked {
                program_id: String::from("test.f32_nan"),
                profile_id: String::from(TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID),
                input_f32_bits: f32::NAN.to_bits(),
            },
        )
        .expect_err("nan should refuse");

        assert!(matches!(
            err,
            TassadarMixedNumericError::InvalidF32ToI32 { .. }
        ));
    }
}
