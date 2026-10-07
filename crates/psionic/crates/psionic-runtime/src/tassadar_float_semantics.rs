use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_FLOAT_SEMANTICS_FAMILY_ID: &str = "tassadar.float_semantics.matrix.v1";
pub const TASSADAR_FLOAT_SEMANTICS_PROFILE_ID: &str = "tassadar.float_semantics.scalar_f32.v1";
pub const TASSADAR_FLOAT_CANONICAL_NAN_BITS: u32 = 0x7fc0_0000;

/// Declared NaN policy for the bounded scalar-f32 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFloatNaNPolicy {
    CanonicalQuietNan32,
}

impl TassadarFloatNaNPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CanonicalQuietNan32 => "canonical_quiet_nan32",
        }
    }
}

/// Declared rounding policy for the bounded scalar-f32 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFloatRoundingPolicy {
    NearestTiesToEven,
}

impl TassadarFloatRoundingPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NearestTiesToEven => "nearest_ties_to_even",
        }
    }
}

/// Declared comparison policy for the bounded scalar-f32 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFloatComparisonPolicy {
    OrderedWasmF32,
}

impl TassadarFloatComparisonPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrderedWasmF32 => "ordered_wasm_f32",
        }
    }
}

/// Supported arithmetic operation in the bounded scalar-f32 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFloatArithmeticOp {
    Add,
    Sub,
    Mul,
}

impl TassadarFloatArithmeticOp {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
        }
    }
}

/// Supported comparison operation in the bounded scalar-f32 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFloatComparisonOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl TassadarFloatComparisonOp {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Lt => "lt",
            Self::Le => "le",
            Self::Gt => "gt",
            Self::Ge => "ge",
        }
    }
}

/// Runtime-owned policy for the bounded scalar-f32 semantics lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatSemanticsPolicy {
    pub policy_id: String,
    pub family_id: String,
    pub profile_id: String,
    pub nan_policy_id: String,
    pub rounding_policy_id: String,
    pub comparison_policy_id: String,
    pub canonical_nan_bits_hex: String,
    pub supported_backend_family_ids: Vec<String>,
    pub refused_backend_family_ids: Vec<String>,
    pub supported_operation_ids: Vec<String>,
    pub refused_regime_ids: Vec<String>,
    pub claim_boundary: String,
    pub policy_digest: String,
}

impl TassadarFloatSemanticsPolicy {
    fn new() -> Self {
        let mut policy = Self {
            policy_id: String::from("tassadar.float_semantics.policy.v1"),
            family_id: String::from(TASSADAR_FLOAT_SEMANTICS_FAMILY_ID),
            profile_id: String::from(TASSADAR_FLOAT_SEMANTICS_PROFILE_ID),
            nan_policy_id: String::from(TassadarFloatNaNPolicy::CanonicalQuietNan32.as_str()),
            rounding_policy_id: String::from(
                TassadarFloatRoundingPolicy::NearestTiesToEven.as_str(),
            ),
            comparison_policy_id: String::from(
                TassadarFloatComparisonPolicy::OrderedWasmF32.as_str(),
            ),
            canonical_nan_bits_hex: format!("0x{TASSADAR_FLOAT_CANONICAL_NAN_BITS:08x}"),
            supported_backend_family_ids: vec![String::from("cpu_reference")],
            refused_backend_family_ids: vec![
                String::from("cuda_served"),
                String::from("metal_served"),
            ],
            supported_operation_ids: vec![
                String::from("f32.add"),
                String::from("f32.sub"),
                String::from("f32.mul"),
                String::from("f32.eq"),
                String::from("f32.ne"),
                String::from("f32.lt"),
                String::from("f32.le"),
                String::from("f32.gt"),
                String::from("f32.ge"),
            ],
            refused_regime_ids: vec![
                String::from("f64_scalar"),
                String::from("nan_payload_preservation"),
                String::from("non_cpu_backend_fast_math"),
            ],
            claim_boundary: String::from(
                "this policy declares one bounded scalar-f32 execution lane with canonical quiet-NaN normalization, nearest-ties-to-even arithmetic, and ordered Wasm-style comparisons. It does not claim f64 closure, NaN payload preservation, fast-math equivalence across non-CPU backends, or arbitrary Wasm float closure",
            ),
            policy_digest: String::new(),
        };
        policy.policy_digest = stable_digest(b"psionic_tassadar_float_semantics_policy|", &policy);
        policy
    }
}

/// Returns the canonical float-semantics policy for the bounded scalar-f32 lane.
#[must_use]
pub fn tassadar_float_semantics_policy() -> TassadarFloatSemanticsPolicy {
    TassadarFloatSemanticsPolicy::new()
}

/// One runtime-executable bounded scalar-f32 program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "program_kind", rename_all = "snake_case")]
pub enum TassadarFloatSemanticsProgram {
    Arithmetic {
        program_id: String,
        family_id: String,
        profile_id: String,
        op: TassadarFloatArithmeticOp,
        lhs_bits: u32,
        rhs_bits: u32,
        claim_boundary: String,
    },
    Comparison {
        program_id: String,
        family_id: String,
        profile_id: String,
        op: TassadarFloatComparisonOp,
        lhs_bits: u32,
        rhs_bits: u32,
        claim_boundary: String,
    },
}

impl TassadarFloatSemanticsProgram {
    #[must_use]
    pub fn program_id(&self) -> &str {
        match self {
            Self::Arithmetic { program_id, .. } | Self::Comparison { program_id, .. } => program_id,
        }
    }

    #[must_use]
    pub fn family_id(&self) -> &str {
        match self {
            Self::Arithmetic { family_id, .. } | Self::Comparison { family_id, .. } => family_id,
        }
    }

    #[must_use]
    pub fn profile_id(&self) -> &str {
        match self {
            Self::Arithmetic { profile_id, .. } | Self::Comparison { profile_id, .. } => {
                profile_id
            }
        }
    }
}

/// Runtime result for one bounded scalar-f32 program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result_kind", rename_all = "snake_case")]
pub enum TassadarFloatSemanticsResult {
    F32Bits { bits: u32 },
    I32 { value: i32 },
}

/// Runtime-owned execution receipt for one bounded scalar-f32 program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatSemanticsExecution {
    pub program_id: String,
    pub family_id: String,
    pub profile_id: String,
    pub policy_id: String,
    pub result: TassadarFloatSemanticsResult,
    pub detail: String,
}

/// Executes one bounded scalar-f32 program under the declared float policy.
#[must_use]
pub fn execute_tassadar_float_semantics_program(
    program: &TassadarFloatSemanticsProgram,
) -> TassadarFloatSemanticsExecution {
    let policy = tassadar_float_semantics_policy();
    let result = match program {
        TassadarFloatSemanticsProgram::Arithmetic {
            op,
            lhs_bits,
            rhs_bits,
            ..
        } => TassadarFloatSemanticsResult::F32Bits {
            bits: execute_tassadar_f32_arithmetic(*op, *lhs_bits, *rhs_bits),
        },
        TassadarFloatSemanticsProgram::Comparison {
            op,
            lhs_bits,
            rhs_bits,
            ..
        } => TassadarFloatSemanticsResult::I32 {
            value: execute_tassadar_f32_comparison(*op, *lhs_bits, *rhs_bits),
        },
    };
    TassadarFloatSemanticsExecution {
        program_id: String::from(program.program_id()),
        family_id: String::from(program.family_id()),
        profile_id: String::from(program.profile_id()),
        policy_id: policy.policy_id,
        detail: String::from(
            "bounded scalar-f32 execution stays tied to canonical quiet-NaN normalization and ordered Wasm-style comparisons",
        ),
        result,
    }
}

/// Canonicalizes one `f32` bit pattern under the declared NaN policy.
#[must_use]
pub fn canonicalize_tassadar_f32_bits(bits: u32) -> u32 {
    if f32::from_bits(bits).is_nan() {
        TASSADAR_FLOAT_CANONICAL_NAN_BITS
    } else {
        bits
    }
}

/// Executes one bounded arithmetic operation and canonicalizes NaN results.
#[must_use]
pub fn execute_tassadar_f32_arithmetic(
    op: TassadarFloatArithmeticOp,
    lhs_bits: u32,
    rhs_bits: u32,
) -> u32 {
    let lhs = f32::from_bits(lhs_bits);
    let rhs = f32::from_bits(rhs_bits);
    let result = match op {
        TassadarFloatArithmeticOp::Add => lhs + rhs,
        TassadarFloatArithmeticOp::Sub => lhs - rhs,
        TassadarFloatArithmeticOp::Mul => lhs * rhs,
    };
    canonicalize_tassadar_f32_bits(result.to_bits())
}

/// Executes one bounded ordered comparison under the declared policy.
#[must_use]
pub fn execute_tassadar_f32_comparison(
    op: TassadarFloatComparisonOp,
    lhs_bits: u32,
    rhs_bits: u32,
) -> i32 {
    let lhs = f32::from_bits(lhs_bits);
    let rhs = f32::from_bits(rhs_bits);
    let result = match op {
        TassadarFloatComparisonOp::Eq => lhs == rhs,
        TassadarFloatComparisonOp::Ne => lhs != rhs,
        TassadarFloatComparisonOp::Lt => lhs < rhs,
        TassadarFloatComparisonOp::Le => lhs <= rhs,
        TassadarFloatComparisonOp::Gt => lhs > rhs,
        TassadarFloatComparisonOp::Ge => lhs >= rhs,
    };
    i32::from(result)
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
        TASSADAR_FLOAT_CANONICAL_NAN_BITS, TassadarFloatArithmeticOp,
        TassadarFloatComparisonOp, TassadarFloatSemanticsProgram,
        canonicalize_tassadar_f32_bits, execute_tassadar_f32_comparison,
        execute_tassadar_float_semantics_program, tassadar_float_semantics_policy,
    };

    #[test]
    fn float_semantics_policy_is_machine_legible() {
        let policy = tassadar_float_semantics_policy();

        assert_eq!(policy.profile_id, "tassadar.float_semantics.scalar_f32.v1");
        assert_eq!(policy.nan_policy_id, "canonical_quiet_nan32");
        assert!(policy
            .supported_operation_ids
            .contains(&String::from("f32.add")));
    }

    #[test]
    fn canonicalize_f32_bits_collapses_nan_payloads() {
        assert_eq!(
            canonicalize_tassadar_f32_bits(0x7fa1_2345),
            TASSADAR_FLOAT_CANONICAL_NAN_BITS
        );
    }

    #[test]
    fn float_comparison_policy_matches_ordered_wasm_semantics() {
        assert_eq!(
            execute_tassadar_f32_comparison(
                TassadarFloatComparisonOp::Eq,
                (-0.0f32).to_bits(),
                0.0f32.to_bits(),
            ),
            1
        );
        assert_eq!(
            execute_tassadar_f32_comparison(
                TassadarFloatComparisonOp::Lt,
                0x7fa1_2345,
                1.0f32.to_bits(),
            ),
            0
        );
        assert_eq!(
            execute_tassadar_f32_comparison(
                TassadarFloatComparisonOp::Ne,
                0x7fa1_2345,
                1.0f32.to_bits(),
            ),
            1
        );
    }

    #[test]
    fn arithmetic_program_canonicalizes_nan_results() {
        let execution = execute_tassadar_float_semantics_program(
            &TassadarFloatSemanticsProgram::Arithmetic {
                program_id: String::from("float.nan.add"),
                family_id: String::from("tassadar.float_semantics.matrix.v1"),
                profile_id: String::from("tassadar.float_semantics.scalar_f32.v1"),
                op: TassadarFloatArithmeticOp::Add,
                lhs_bits: 0x7fa1_2345,
                rhs_bits: 1.0f32.to_bits(),
                claim_boundary: String::from("test"),
            },
        );

        assert_eq!(
            execution.result,
            super::TassadarFloatSemanticsResult::F32Bits {
                bits: TASSADAR_FLOAT_CANONICAL_NAN_BITS,
            }
        );
    }
}
