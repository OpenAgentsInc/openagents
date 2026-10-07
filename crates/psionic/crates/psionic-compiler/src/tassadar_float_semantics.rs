use psionic_runtime::{
    TASSADAR_FLOAT_CANONICAL_NAN_BITS, TASSADAR_FLOAT_SEMANTICS_FAMILY_ID,
    TASSADAR_FLOAT_SEMANTICS_PROFILE_ID, TassadarFloatArithmeticOp,
    TassadarFloatComparisonOp, TassadarFloatSemanticsProgram,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Expected outcome for one bounded float-semantics fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "expectation_kind", rename_all = "snake_case")]
pub enum TassadarFloatSemanticsExpectation {
    F32Bits { bits: u32 },
    I32 { value: i32 },
    Refusal { regime_id: String, detail: String },
}

/// Seeded compiler-owned fixture for the bounded float-semantics lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fixture_kind", rename_all = "snake_case")]
pub enum TassadarFloatSemanticsFixture {
    Arithmetic {
        case_id: String,
        source_ref: String,
        op: TassadarFloatArithmeticOp,
        lhs_bits: u32,
        rhs_bits: u32,
        expected: TassadarFloatSemanticsExpectation,
        claim_boundary: String,
    },
    Comparison {
        case_id: String,
        source_ref: String,
        op: TassadarFloatComparisonOp,
        lhs_bits: u32,
        rhs_bits: u32,
        expected: TassadarFloatSemanticsExpectation,
        claim_boundary: String,
    },
    UnsupportedRegime {
        case_id: String,
        source_ref: String,
        regime_id: String,
        expected: TassadarFloatSemanticsExpectation,
        claim_boundary: String,
    },
}

impl TassadarFloatSemanticsFixture {
    #[must_use]
    pub fn case_id(&self) -> &str {
        match self {
            Self::Arithmetic { case_id, .. }
            | Self::Comparison { case_id, .. }
            | Self::UnsupportedRegime { case_id, .. } => case_id,
        }
    }

    #[must_use]
    pub fn source_ref(&self) -> &str {
        match self {
            Self::Arithmetic { source_ref, .. }
            | Self::Comparison { source_ref, .. }
            | Self::UnsupportedRegime { source_ref, .. } => source_ref,
        }
    }

    #[must_use]
    pub fn expected(&self) -> &TassadarFloatSemanticsExpectation {
        match self {
            Self::Arithmetic { expected, .. }
            | Self::Comparison { expected, .. }
            | Self::UnsupportedRegime { expected, .. } => expected,
        }
    }

    #[must_use]
    pub fn claim_boundary(&self) -> &str {
        match self {
            Self::Arithmetic { claim_boundary, .. }
            | Self::Comparison { claim_boundary, .. }
            | Self::UnsupportedRegime { claim_boundary, .. } => claim_boundary,
        }
    }
}

/// Lowered artifact for one bounded float-semantics fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatSemanticsArtifact {
    pub case_id: String,
    pub source_ref: String,
    pub family_id: String,
    pub profile_id: String,
    pub program: TassadarFloatSemanticsProgram,
    pub claim_class: String,
    pub artifact_digest: String,
}

impl TassadarFloatSemanticsArtifact {
    fn new(fixture: &TassadarFloatSemanticsFixture, program: TassadarFloatSemanticsProgram) -> Self {
        let mut artifact = Self {
            case_id: String::from(fixture.case_id()),
            source_ref: String::from(fixture.source_ref()),
            family_id: String::from(TASSADAR_FLOAT_SEMANTICS_FAMILY_ID),
            profile_id: String::from(TASSADAR_FLOAT_SEMANTICS_PROFILE_ID),
            program,
            claim_class: String::from("compiled_bounded_exactness"),
            artifact_digest: String::new(),
        };
        artifact.artifact_digest =
            stable_digest(b"psionic_tassadar_float_semantics_artifact|", &artifact);
        artifact
    }
}

/// Typed compiler-side lowering failure for the bounded float-semantics lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error_kind")]
pub enum TassadarFloatSemanticsLoweringError {
    #[error("float semantics fixture `{case_id}` declares unsupported regime `{regime_id}`")]
    UnsupportedRegime {
        case_id: String,
        regime_id: String,
        detail: String,
    },
}

/// Returns the canonical seeded fixtures for the bounded float-semantics lane.
#[must_use]
pub fn tassadar_seeded_float_semantics_fixtures() -> Vec<TassadarFloatSemanticsFixture> {
    let source_ref = "synthetic://tassadar/float_semantics_matrix/scalar_f32_v1";
    let claim_boundary = "bounded scalar-f32 float semantics only";
    vec![
        TassadarFloatSemanticsFixture::Arithmetic {
            case_id: String::from("f32_add_finite_exact"),
            source_ref: String::from(source_ref),
            op: TassadarFloatArithmeticOp::Add,
            lhs_bits: 1.5f32.to_bits(),
            rhs_bits: 2.25f32.to_bits(),
            expected: TassadarFloatSemanticsExpectation::F32Bits {
                bits: 3.75f32.to_bits(),
            },
            claim_boundary: String::from(claim_boundary),
        },
        TassadarFloatSemanticsFixture::Arithmetic {
            case_id: String::from("f32_sub_finite_exact"),
            source_ref: String::from(source_ref),
            op: TassadarFloatArithmeticOp::Sub,
            lhs_bits: 10.5f32.to_bits(),
            rhs_bits: 0.25f32.to_bits(),
            expected: TassadarFloatSemanticsExpectation::F32Bits {
                bits: 10.25f32.to_bits(),
            },
            claim_boundary: String::from(claim_boundary),
        },
        TassadarFloatSemanticsFixture::Arithmetic {
            case_id: String::from("f32_mul_finite_exact"),
            source_ref: String::from(source_ref),
            op: TassadarFloatArithmeticOp::Mul,
            lhs_bits: (-3.0f32).to_bits(),
            rhs_bits: 2.0f32.to_bits(),
            expected: TassadarFloatSemanticsExpectation::F32Bits {
                bits: (-6.0f32).to_bits(),
            },
            claim_boundary: String::from(claim_boundary),
        },
        TassadarFloatSemanticsFixture::Arithmetic {
            case_id: String::from("f32_add_nan_canonicalized_exact"),
            source_ref: String::from(source_ref),
            op: TassadarFloatArithmeticOp::Add,
            lhs_bits: 0x7fa1_2345,
            rhs_bits: 1.0f32.to_bits(),
            expected: TassadarFloatSemanticsExpectation::F32Bits {
                bits: TASSADAR_FLOAT_CANONICAL_NAN_BITS,
            },
            claim_boundary: String::from(claim_boundary),
        },
        TassadarFloatSemanticsFixture::Comparison {
            case_id: String::from("f32_lt_finite_exact"),
            source_ref: String::from(source_ref),
            op: TassadarFloatComparisonOp::Lt,
            lhs_bits: 1.0f32.to_bits(),
            rhs_bits: 2.0f32.to_bits(),
            expected: TassadarFloatSemanticsExpectation::I32 { value: 1 },
            claim_boundary: String::from(claim_boundary),
        },
        TassadarFloatSemanticsFixture::Comparison {
            case_id: String::from("f32_eq_signed_zero_exact"),
            source_ref: String::from(source_ref),
            op: TassadarFloatComparisonOp::Eq,
            lhs_bits: (-0.0f32).to_bits(),
            rhs_bits: 0.0f32.to_bits(),
            expected: TassadarFloatSemanticsExpectation::I32 { value: 1 },
            claim_boundary: String::from(claim_boundary),
        },
        TassadarFloatSemanticsFixture::Comparison {
            case_id: String::from("f32_eq_nan_false_exact"),
            source_ref: String::from(source_ref),
            op: TassadarFloatComparisonOp::Eq,
            lhs_bits: 0x7fa1_2345,
            rhs_bits: 1.0f32.to_bits(),
            expected: TassadarFloatSemanticsExpectation::I32 { value: 0 },
            claim_boundary: String::from(claim_boundary),
        },
        TassadarFloatSemanticsFixture::Comparison {
            case_id: String::from("f32_ne_nan_true_exact"),
            source_ref: String::from(source_ref),
            op: TassadarFloatComparisonOp::Ne,
            lhs_bits: 0x7fa1_2345,
            rhs_bits: 1.0f32.to_bits(),
            expected: TassadarFloatSemanticsExpectation::I32 { value: 1 },
            claim_boundary: String::from(claim_boundary),
        },
        TassadarFloatSemanticsFixture::UnsupportedRegime {
            case_id: String::from("f64_scalar_refusal"),
            source_ref: String::from(source_ref),
            regime_id: String::from("f64_scalar"),
            expected: TassadarFloatSemanticsExpectation::Refusal {
                regime_id: String::from("f64_scalar"),
                detail: String::from(
                    "the bounded float-semantics lane is scalar-f32 only and refuses f64 execution",
                ),
            },
            claim_boundary: String::from(claim_boundary),
        },
        TassadarFloatSemanticsFixture::UnsupportedRegime {
            case_id: String::from("nan_payload_preservation_refusal"),
            source_ref: String::from(source_ref),
            regime_id: String::from("nan_payload_preservation"),
            expected: TassadarFloatSemanticsExpectation::Refusal {
                regime_id: String::from("nan_payload_preservation"),
                detail: String::from(
                    "the bounded float-semantics lane canonicalizes NaNs and refuses payload-preservation claims",
                ),
            },
            claim_boundary: String::from(claim_boundary),
        },
    ]
}

/// Lowers one seeded float-semantics fixture into a runtime program.
pub fn lower_tassadar_float_semantics_fixture(
    fixture: &TassadarFloatSemanticsFixture,
) -> Result<TassadarFloatSemanticsArtifact, TassadarFloatSemanticsLoweringError> {
    let program = match fixture {
        TassadarFloatSemanticsFixture::Arithmetic {
            case_id,
            op,
            lhs_bits,
            rhs_bits,
            claim_boundary,
            ..
        } => TassadarFloatSemanticsProgram::Arithmetic {
            program_id: format!("tassadar.float_semantics.program.{case_id}"),
            family_id: String::from(TASSADAR_FLOAT_SEMANTICS_FAMILY_ID),
            profile_id: String::from(TASSADAR_FLOAT_SEMANTICS_PROFILE_ID),
            op: *op,
            lhs_bits: *lhs_bits,
            rhs_bits: *rhs_bits,
            claim_boundary: claim_boundary.clone(),
        },
        TassadarFloatSemanticsFixture::Comparison {
            case_id,
            op,
            lhs_bits,
            rhs_bits,
            claim_boundary,
            ..
        } => TassadarFloatSemanticsProgram::Comparison {
            program_id: format!("tassadar.float_semantics.program.{case_id}"),
            family_id: String::from(TASSADAR_FLOAT_SEMANTICS_FAMILY_ID),
            profile_id: String::from(TASSADAR_FLOAT_SEMANTICS_PROFILE_ID),
            op: *op,
            lhs_bits: *lhs_bits,
            rhs_bits: *rhs_bits,
            claim_boundary: claim_boundary.clone(),
        },
        TassadarFloatSemanticsFixture::UnsupportedRegime {
            case_id,
            regime_id,
            expected,
            ..
        } => {
            let detail = match expected {
                TassadarFloatSemanticsExpectation::Refusal { detail, .. } => detail.clone(),
                _ => format!("unsupported float regime `{regime_id}`"),
            };
            return Err(TassadarFloatSemanticsLoweringError::UnsupportedRegime {
                case_id: case_id.clone(),
                regime_id: regime_id.clone(),
                detail,
            });
        }
    };
    Ok(TassadarFloatSemanticsArtifact::new(fixture, program))
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
        TassadarFloatSemanticsExpectation, TassadarFloatSemanticsFixture,
        TassadarFloatSemanticsLoweringError, lower_tassadar_float_semantics_fixture,
        tassadar_seeded_float_semantics_fixtures,
    };

    #[test]
    fn float_semantics_fixture_set_is_machine_legible() {
        let fixtures = tassadar_seeded_float_semantics_fixtures();

        assert!(fixtures.iter().any(|fixture| {
            matches!(
                fixture,
                TassadarFloatSemanticsFixture::Arithmetic {
                    case_id,
                    expected: TassadarFloatSemanticsExpectation::F32Bits { .. },
                    ..
                } if case_id == "f32_add_nan_canonicalized_exact"
            )
        }));
        assert!(fixtures.iter().any(|fixture| {
            matches!(
                fixture,
                TassadarFloatSemanticsFixture::UnsupportedRegime { case_id, .. }
                    if case_id == "f64_scalar_refusal"
            )
        }));
    }

    #[test]
    fn float_semantics_lowering_preserves_supported_cases() {
        let fixture = tassadar_seeded_float_semantics_fixtures()
            .into_iter()
            .find(|fixture| fixture.case_id() == "f32_lt_finite_exact")
            .expect("fixture");
        let artifact = lower_tassadar_float_semantics_fixture(&fixture).expect("artifact");

        assert_eq!(artifact.family_id, "tassadar.float_semantics.matrix.v1");
        assert_eq!(artifact.profile_id, "tassadar.float_semantics.scalar_f32.v1");
    }

    #[test]
    fn float_semantics_lowering_refuses_unsupported_regimes() {
        let fixture = tassadar_seeded_float_semantics_fixtures()
            .into_iter()
            .find(|fixture| fixture.case_id() == "f64_scalar_refusal")
            .expect("fixture");
        let err =
            lower_tassadar_float_semantics_fixture(&fixture).expect_err("unsupported regime");

        assert!(matches!(
            err,
            TassadarFloatSemanticsLoweringError::UnsupportedRegime { .. }
        ));
    }
}
