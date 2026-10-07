use psionic_ir::{
    TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID, TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
    tassadar_mixed_numeric_profile_ladder_contract,
};
use psionic_runtime::TassadarMixedNumericProgram;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Expected outcome for one mixed-numeric fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "expectation_kind", rename_all = "snake_case")]
pub enum TassadarMixedNumericExpectation {
    F32Bits { bits: u32 },
    I32 { value: i32 },
    BoundedApproximateF32Bits { bits: u32 },
    Refusal { reason_id: String, detail: String },
}

/// Seeded compiler-owned fixture for the mixed-numeric ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fixture_kind", rename_all = "snake_case")]
pub enum TassadarMixedNumericFixture {
    I32ToF32ExactRange {
        case_id: String,
        source_ref: String,
        input_i32: i32,
        expected: TassadarMixedNumericExpectation,
    },
    F32ToI32TruncChecked {
        case_id: String,
        source_ref: String,
        input_f32_bits: u32,
        expected: TassadarMixedNumericExpectation,
    },
    MixedI32F32ScaleAddExact {
        case_id: String,
        source_ref: String,
        input_i32: i32,
        scale_f32_bits: u32,
        bias_f32_bits: u32,
        expected: TassadarMixedNumericExpectation,
    },
    F64ToF32Bounded {
        case_id: String,
        source_ref: String,
        input_f64_bits: u64,
        expected: TassadarMixedNumericExpectation,
    },
}

impl TassadarMixedNumericFixture {
    #[must_use]
    pub fn case_id(&self) -> &str {
        match self {
            Self::I32ToF32ExactRange { case_id, .. }
            | Self::F32ToI32TruncChecked { case_id, .. }
            | Self::MixedI32F32ScaleAddExact { case_id, .. }
            | Self::F64ToF32Bounded { case_id, .. } => case_id,
        }
    }

    #[must_use]
    pub fn source_ref(&self) -> &str {
        match self {
            Self::I32ToF32ExactRange { source_ref, .. }
            | Self::F32ToI32TruncChecked { source_ref, .. }
            | Self::MixedI32F32ScaleAddExact { source_ref, .. }
            | Self::F64ToF32Bounded { source_ref, .. } => source_ref,
        }
    }

    #[must_use]
    pub fn expected(&self) -> &TassadarMixedNumericExpectation {
        match self {
            Self::I32ToF32ExactRange { expected, .. }
            | Self::F32ToI32TruncChecked { expected, .. }
            | Self::MixedI32F32ScaleAddExact { expected, .. }
            | Self::F64ToF32Bounded { expected, .. } => expected,
        }
    }
}

/// Lowered artifact for one mixed-numeric fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedNumericArtifact {
    pub case_id: String,
    pub source_ref: String,
    pub profile_id: String,
    pub program: TassadarMixedNumericProgram,
    pub claim_class: String,
    pub artifact_digest: String,
}

impl TassadarMixedNumericArtifact {
    fn new(
        fixture: &TassadarMixedNumericFixture,
        profile_id: &str,
        program: TassadarMixedNumericProgram,
    ) -> Self {
        let mut artifact = Self {
            case_id: String::from(fixture.case_id()),
            source_ref: String::from(fixture.source_ref()),
            profile_id: String::from(profile_id),
            program,
            claim_class: String::from("compiled_bounded_exactness"),
            artifact_digest: String::new(),
        };
        artifact.artifact_digest = stable_digest(b"psionic_tassadar_mixed_numeric_artifact|", &artifact);
        artifact
    }
}

/// Returns the canonical seeded mixed-numeric fixtures.
#[must_use]
pub fn tassadar_seeded_mixed_numeric_fixtures() -> Vec<TassadarMixedNumericFixture> {
    let source_ref = "synthetic://tassadar/mixed_numeric_profile_ladder/v1";
    vec![
        TassadarMixedNumericFixture::I32ToF32ExactRange {
            case_id: String::from("i32_to_f32_exact_range"),
            source_ref: String::from(source_ref),
            input_i32: 1024,
            expected: TassadarMixedNumericExpectation::F32Bits {
                bits: 1024.0f32.to_bits(),
            },
        },
        TassadarMixedNumericFixture::I32ToF32ExactRange {
            case_id: String::from("i32_to_f32_nonexact_refusal"),
            source_ref: String::from(source_ref),
            input_i32: 16_777_217,
            expected: TassadarMixedNumericExpectation::Refusal {
                reason_id: String::from("i32_to_f32_non_exact"),
                detail: String::from(
                    "mixed i32/f32 exactness refuses i32 values outside the exact f32 range",
                ),
            },
        },
        TassadarMixedNumericFixture::F32ToI32TruncChecked {
            case_id: String::from("f32_to_i32_trunc_exact"),
            source_ref: String::from(source_ref),
            input_f32_bits: 42.75f32.to_bits(),
            expected: TassadarMixedNumericExpectation::I32 { value: 42 },
        },
        TassadarMixedNumericFixture::F32ToI32TruncChecked {
            case_id: String::from("f32_to_i32_nan_refusal"),
            source_ref: String::from(source_ref),
            input_f32_bits: f32::NAN.to_bits(),
            expected: TassadarMixedNumericExpectation::Refusal {
                reason_id: String::from("f32_to_i32_invalid_nan"),
                detail: String::from("checked f32-to-i32 truncation refuses NaN inputs"),
            },
        },
        TassadarMixedNumericFixture::MixedI32F32ScaleAddExact {
            case_id: String::from("mixed_i32_f32_scale_add_exact"),
            source_ref: String::from(source_ref),
            input_i32: 4,
            scale_f32_bits: 0.5f32.to_bits(),
            bias_f32_bits: 1.0f32.to_bits(),
            expected: TassadarMixedNumericExpectation::F32Bits {
                bits: 3.0f32.to_bits(),
            },
        },
        TassadarMixedNumericFixture::F64ToF32Bounded {
            case_id: String::from("f64_to_f32_bounded_approximate"),
            source_ref: String::from(source_ref),
            input_f64_bits: 0.1f64.to_bits(),
            expected: TassadarMixedNumericExpectation::BoundedApproximateF32Bits {
                bits: (0.1f64 as f32).to_bits(),
            },
        },
        TassadarMixedNumericFixture::F64ToF32Bounded {
            case_id: String::from("f64_to_f32_out_of_range_refusal"),
            source_ref: String::from(source_ref),
            input_f64_bits: (f32::MAX as f64 * 2.0).to_bits(),
            expected: TassadarMixedNumericExpectation::Refusal {
                reason_id: String::from("f64_out_of_range"),
                detail: String::from("bounded f64 conversion refuses values outside the f32 range"),
            },
        },
    ]
}

/// Lowers one mixed-numeric fixture into a runtime program.
#[must_use]
pub fn lower_tassadar_mixed_numeric_fixture(
    fixture: &TassadarMixedNumericFixture,
) -> TassadarMixedNumericArtifact {
    let _contract = tassadar_mixed_numeric_profile_ladder_contract();
    match fixture {
        TassadarMixedNumericFixture::I32ToF32ExactRange {
            case_id,
            input_i32,
            ..
        } => TassadarMixedNumericArtifact::new(
            fixture,
            TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
            TassadarMixedNumericProgram::I32ToF32ExactRange {
                program_id: format!("tassadar.mixed_numeric.program.{case_id}"),
                profile_id: String::from(TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID),
                input_i32: *input_i32,
            },
        ),
        TassadarMixedNumericFixture::F32ToI32TruncChecked {
            case_id,
            input_f32_bits,
            ..
        } => TassadarMixedNumericArtifact::new(
            fixture,
            TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
            TassadarMixedNumericProgram::F32ToI32TruncChecked {
                program_id: format!("tassadar.mixed_numeric.program.{case_id}"),
                profile_id: String::from(TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID),
                input_f32_bits: *input_f32_bits,
            },
        ),
        TassadarMixedNumericFixture::MixedI32F32ScaleAddExact {
            case_id,
            input_i32,
            scale_f32_bits,
            bias_f32_bits,
            ..
        } => TassadarMixedNumericArtifact::new(
            fixture,
            TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
            TassadarMixedNumericProgram::MixedI32F32ScaleAddExact {
                program_id: format!("tassadar.mixed_numeric.program.{case_id}"),
                profile_id: String::from(TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID),
                input_i32: *input_i32,
                scale_f32_bits: *scale_f32_bits,
                bias_f32_bits: *bias_f32_bits,
            },
        ),
        TassadarMixedNumericFixture::F64ToF32Bounded {
            case_id,
            input_f64_bits,
            ..
        } => TassadarMixedNumericArtifact::new(
            fixture,
            TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID,
            TassadarMixedNumericProgram::F64ToF32Bounded {
                program_id: format!("tassadar.mixed_numeric.program.{case_id}"),
                profile_id: String::from(TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID),
                input_f64_bits: *input_f64_bits,
            },
        ),
    }
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
        lower_tassadar_mixed_numeric_fixture, tassadar_seeded_mixed_numeric_fixtures,
    };
    use psionic_ir::{
        TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID, TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
    };

    #[test]
    fn mixed_numeric_fixture_suite_is_machine_legible() {
        let fixtures = tassadar_seeded_mixed_numeric_fixtures();
        assert_eq!(fixtures.len(), 7);
    }

    #[test]
    fn mixed_numeric_lowering_preserves_profile_ids() {
        let fixtures = tassadar_seeded_mixed_numeric_fixtures();
        let exact = fixtures
            .iter()
            .find(|fixture| fixture.case_id() == "mixed_i32_f32_scale_add_exact")
            .expect("fixture");
        let exact_artifact = lower_tassadar_mixed_numeric_fixture(exact);
        assert_eq!(exact_artifact.profile_id, TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID);

        let approximate = fixtures
            .iter()
            .find(|fixture| fixture.case_id() == "f64_to_f32_bounded_approximate")
            .expect("fixture");
        let approximate_artifact = lower_tassadar_mixed_numeric_fixture(approximate);
        assert_eq!(
            approximate_artifact.profile_id,
            TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID
        );
    }
}
