use psionic_ir::{tassadar_symbolic_program_examples, TassadarSymbolicProgram};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_DECOMPILABLE_EXECUTOR_PUBLICATION_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_DECOMPILATION_FIDELITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_decompilation_fidelity_report.json";
pub const TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_decompilable_executor_artifacts_report.json";
pub const TASSADAR_DECOMPILABLE_EXECUTOR_CLAIM_CLASS: &str = "research_only_architecture";

/// Public repo status for the decompilable learned executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDecompilableExecutorPublicationStatus {
    /// The lane is present as an early research-only publication.
    ImplementedEarly,
}

/// One constrained family intentionally shaped for readable decompilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDecompilationFamily {
    /// Straight-line arithmetic and finite-state kernels with readable operator lattices.
    SymbolicOperatorLattice,
    /// Memory and stack-update kernels with explicit state-delta sketches.
    StateDeltaSketch,
}

impl TassadarDecompilationFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SymbolicOperatorLattice => "symbolic_operator_lattice",
            Self::StateDeltaSketch => "state_delta_sketch",
        }
    }
}

/// Stability classification for decompiled artifacts across retrains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDecompilationStabilityClass {
    /// All retrains decompile into the same readable form.
    StableExactForm,
    /// Retrains vary in naming or formatting but preserve the same readable structure.
    StableEquivalentForms,
}

/// Stable discretization path used by one retrain artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDecompilationDiscretizationKind {
    /// Direct thresholding into one readable symbolic program.
    DirectSymbolicThreshold,
    /// Operator-state deltas discretized into one readable symbolic program.
    StateDeltaProjection,
}

/// One seeded retrain artifact intended to remain decompilable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilationRetrainArtifact {
    /// Stable retrain run identifier.
    pub retrain_run_id: String,
    /// Stable seed identifier for the retrain.
    pub retrain_seed: u16,
    /// Stable checkpoint ref for the retrain artifact.
    pub checkpoint_ref: String,
    /// Discretization family used to produce the readable program.
    pub discretization_kind: TassadarDecompilationDiscretizationKind,
    /// Readable decompiled symbolic program.
    pub decompiled_program: TassadarSymbolicProgram,
}

/// One public case carried by the decompilable learned executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilableExecutorCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Seeded symbolic-reference case identifier.
    pub source_case_id: String,
    /// Constrained learned family the case belongs to.
    pub family: TassadarDecompilationFamily,
    /// Plain-language case summary.
    pub summary: String,
    /// Stable reference program identifier.
    pub reference_program_id: String,
    /// Stable reference program digest.
    pub reference_program_digest: String,
    /// Stable benchmark refs anchoring the case.
    pub benchmark_refs: Vec<String>,
    /// Ordered seeded retrain artifacts.
    pub retrains: Vec<TassadarDecompilationRetrainArtifact>,
}

/// Public model-facing publication for the decompilable learned executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilableExecutorPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value.
    pub status: TassadarDecompilableExecutorPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable candidate descriptor for the lane.
    pub model: ModelDescriptor,
    /// Baseline family refs this lane compares against.
    pub baseline_family_refs: Vec<String>,
    /// Ordered repo-facing seeded cases.
    pub cases: Vec<TassadarDecompilableExecutorCase>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarDecompilableExecutorPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_DECOMPILABLE_EXECUTOR_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.decompilable_executor.publication.v1"),
            status: TassadarDecompilableExecutorPublicationStatus::ImplementedEarly,
            claim_class: String::from(TASSADAR_DECOMPILABLE_EXECUTOR_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-decompilable-executor-candidate-v0",
                "tassadar_decompilable_executor",
                "v0",
            ),
            baseline_family_refs: vec![
                String::from("model-family://openagents/tassadar/module_state_executor"),
                String::from("model-family://openagents/tassadar/shared_depth_executor"),
                String::from("artifact-suite://openagents/tassadar/symbolic_program_artifact_suite"),
            ],
            cases: tassadar_decompilable_executor_cases(),
            validation_refs: vec![
                String::from(TASSADAR_DECOMPILATION_FIDELITY_REPORT_REF),
                String::from(TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the lane is intentionally constrained to decompilable learned artifacts over seeded bounded symbolic kernels; arbitrary Wasm, arbitrary module-scale learned execution, and served promotion remain out of scope",
                ),
                String::from(
                    "readable decompilation is subordinate to benchmark lineage, compiled-reference comparison, and retrain stability, and is not proof of broad learned exactness or deployment readiness",
                ),
                String::from(
                    "the seeded retrains are machine-legible research artifacts only; they do not widen provider or served capability on their own",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_decompilable_executor_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public cases for the decompilable learned executor lane.
#[must_use]
pub fn tassadar_decompilable_executor_cases() -> Vec<TassadarDecompilableExecutorCase> {
    tassadar_symbolic_program_examples()
        .into_iter()
        .filter_map(|example| {
            let source_case_id = example.case_id.clone();
            let family = match source_case_id.as_str() {
                "addition_pair" | "parity_two_bits" | "finite_state_counter" => {
                    TassadarDecompilationFamily::SymbolicOperatorLattice
                }
                "memory_accumulator" | "stack_machine_add_step" => {
                    TassadarDecompilationFamily::StateDeltaSketch
                }
                _ => return None,
            };
            Some(TassadarDecompilableExecutorCase {
                case_id: format!("{}_decompilation_lane", source_case_id),
                source_case_id: source_case_id.clone(),
                family,
                summary: match family {
                    TassadarDecompilationFamily::SymbolicOperatorLattice => String::from(
                        "bounded learned executor artifacts decompile into one readable operator-lattice symbolic program on the seeded symbolic lane",
                    ),
                    TassadarDecompilationFamily::StateDeltaSketch => String::from(
                        "bounded learned executor artifacts decompile into one readable state-delta symbolic sketch on the seeded memory or stack-update lane",
                    ),
                },
                reference_program_id: example.program.program_id.clone(),
                reference_program_digest: stable_digest(
                    b"psionic_tassadar_decompilable_executor_reference_program|",
                    &example.program,
                ),
                benchmark_refs: vec![
                    String::from("fixtures/tassadar/reports/tassadar_symbolic_program_artifact_suite.json"),
                    String::from("fixtures/tassadar/reports/tassadar_module_state_architecture_report.json"),
                ],
                retrains: decompilation_retrains(source_case_id.as_str()),
            })
        })
        .collect()
}

/// Returns the canonical public publication for the decompilable learned executor lane.
#[must_use]
pub fn tassadar_decompilable_executor_publication() -> TassadarDecompilableExecutorPublication {
    TassadarDecompilableExecutorPublication::new()
}

fn decompilation_retrains(case_id: &str) -> Vec<TassadarDecompilationRetrainArtifact> {
    match case_id {
        "addition_pair" => vec![
            retrain(
                "addition_pair",
                7,
                TassadarDecompilationDiscretizationKind::DirectSymbolicThreshold,
                ADDITION_RETRAIN_A,
            ),
            retrain(
                "addition_pair",
                13,
                TassadarDecompilationDiscretizationKind::DirectSymbolicThreshold,
                ADDITION_RETRAIN_B,
            ),
            retrain(
                "addition_pair",
                29,
                TassadarDecompilationDiscretizationKind::DirectSymbolicThreshold,
                ADDITION_RETRAIN_C,
            ),
        ],
        "parity_two_bits" => vec![
            retrain(
                "parity_two_bits",
                5,
                TassadarDecompilationDiscretizationKind::DirectSymbolicThreshold,
                PARITY_RETRAIN_A,
            ),
            retrain(
                "parity_two_bits",
                17,
                TassadarDecompilationDiscretizationKind::DirectSymbolicThreshold,
                PARITY_RETRAIN_B,
            ),
            retrain(
                "parity_two_bits",
                31,
                TassadarDecompilationDiscretizationKind::DirectSymbolicThreshold,
                PARITY_RETRAIN_C,
            ),
        ],
        "memory_accumulator" => vec![
            retrain(
                "memory_accumulator",
                11,
                TassadarDecompilationDiscretizationKind::StateDeltaProjection,
                MEMORY_RETRAIN_A,
            ),
            retrain(
                "memory_accumulator",
                19,
                TassadarDecompilationDiscretizationKind::StateDeltaProjection,
                MEMORY_RETRAIN_B,
            ),
            retrain(
                "memory_accumulator",
                37,
                TassadarDecompilationDiscretizationKind::StateDeltaProjection,
                MEMORY_RETRAIN_C,
            ),
        ],
        "finite_state_counter" => vec![
            retrain(
                "finite_state_counter",
                3,
                TassadarDecompilationDiscretizationKind::DirectSymbolicThreshold,
                FINITE_STATE_RETRAIN_A,
            ),
            retrain(
                "finite_state_counter",
                23,
                TassadarDecompilationDiscretizationKind::DirectSymbolicThreshold,
                FINITE_STATE_RETRAIN_B,
            ),
            retrain(
                "finite_state_counter",
                41,
                TassadarDecompilationDiscretizationKind::DirectSymbolicThreshold,
                FINITE_STATE_RETRAIN_C,
            ),
        ],
        "stack_machine_add_step" => vec![
            retrain(
                "stack_machine_add_step",
                2,
                TassadarDecompilationDiscretizationKind::StateDeltaProjection,
                STACK_RETRAIN_A,
            ),
            retrain(
                "stack_machine_add_step",
                43,
                TassadarDecompilationDiscretizationKind::StateDeltaProjection,
                STACK_RETRAIN_B,
            ),
            retrain(
                "stack_machine_add_step",
                59,
                TassadarDecompilationDiscretizationKind::StateDeltaProjection,
                STACK_RETRAIN_C,
            ),
        ],
        _ => Vec::new(),
    }
}

fn retrain(
    source_case_id: &str,
    retrain_seed: u16,
    discretization_kind: TassadarDecompilationDiscretizationKind,
    source: &str,
) -> TassadarDecompilationRetrainArtifact {
    TassadarDecompilationRetrainArtifact {
        retrain_run_id: format!("{}.retrain.{retrain_seed}", source_case_id),
        retrain_seed,
        checkpoint_ref: format!(
            "fixtures/tassadar/runs/tassadar_decompilable_executor_v1/{source_case_id}/checkpoint_seed_{retrain_seed}.json"
        ),
        discretization_kind,
        decompiled_program: TassadarSymbolicProgram::parse(source)
            .expect("seeded decompiled program should parse"),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

const ADDITION_RETRAIN_A: &str = r#"
program tassadar.decompiled.addition_pair.retrain_a.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 2
input lhs = slot(0)
input rhs = slot(1)
let sum = add(lhs, rhs)
output sum
"#;

const ADDITION_RETRAIN_B: &str = r#"
program tassadar.decompiled.addition_pair.retrain_b.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 2
input left_value = slot(0)
input right_value = slot(1)
let merged = add(left_value, right_value)
output merged
"#;

const ADDITION_RETRAIN_C: &str = r#"
program tassadar.decompiled.addition_pair.retrain_c.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 2
input x = slot(0)
input y = slot(1)
let x_plus_y = add(x, y)
output x_plus_y
"#;

const PARITY_RETRAIN_A: &str = r#"
program tassadar.decompiled.parity_two_bits.retrain_a.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 2
input bit0 = slot(0)
input bit1 = slot(1)
let sum = add(bit0, bit1)
let lt_two = lt(sum, const(2))
let lt_one = lt(sum, const(1))
let parity = sub(lt_two, lt_one)
output parity
"#;

const PARITY_RETRAIN_B: &str = r#"
program tassadar.decompiled.parity_two_bits.retrain_b.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 2
input a = slot(0)
input b = slot(1)
let total = add(a, b)
let below_two = lt(total, const(2))
let below_one = lt(total, const(1))
let odd_bit = sub(below_two, below_one)
output odd_bit
"#;

const PARITY_RETRAIN_C: &str = r#"
program tassadar.decompiled.parity_two_bits.retrain_c.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 2
input lhs_bit = slot(0)
input rhs_bit = slot(1)
let pair_sum = add(lhs_bit, rhs_bit)
let lt2 = lt(pair_sum, const(2))
let lt1 = lt(pair_sum, const(1))
let parity_flag = sub(lt2, lt1)
output parity_flag
"#;

const MEMORY_RETRAIN_A: &str = r#"
program tassadar.decompiled.memory_accumulator.retrain_a.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 3
input value = slot(0)
init slot(2) = 10
let total = add(value, slot(2))
store slot(2) = total
output total
"#;

const MEMORY_RETRAIN_B: &str = r#"
program tassadar.decompiled.memory_accumulator.retrain_b.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 3
input incoming = slot(0)
init slot(2) = 10
let updated_total = add(incoming, slot(2))
store slot(2) = updated_total
output updated_total
"#;

const MEMORY_RETRAIN_C: &str = r#"
program tassadar.decompiled.memory_accumulator.retrain_c.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 3
input payload = slot(0)
init slot(2) = 10
let new_accumulator = add(payload, slot(2))
store slot(2) = new_accumulator
output new_accumulator
"#;

const FINITE_STATE_RETRAIN_A: &str = r#"
program tassadar.decompiled.finite_state_counter.retrain_a.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 1
input state = slot(0)
let can_advance = lt(state, const(2))
let next_state = add(state, can_advance)
store slot(0) = next_state
output next_state
"#;

const FINITE_STATE_RETRAIN_B: &str = r#"
program tassadar.decompiled.finite_state_counter.retrain_b.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 1
input current_state = slot(0)
let advance_flag = lt(current_state, const(2))
let successor_state = add(current_state, advance_flag)
store slot(0) = successor_state
output successor_state
"#;

const FINITE_STATE_RETRAIN_C: &str = r#"
program tassadar.decompiled.finite_state_counter.retrain_c.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 1
input s = slot(0)
let gate = lt(s, const(2))
let next = add(s, gate)
store slot(0) = next
output next
"#;

const STACK_RETRAIN_A: &str = r#"
program tassadar.decompiled.stack_machine_add_step.retrain_a.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 3
input stack_top = slot(0)
input stack_next = slot(1)
input stack_pointer = slot(2)
let summed = add(stack_top, stack_next)
let next_stack_pointer = sub(stack_pointer, const(1))
store slot(0) = summed
store slot(1) = const(0)
store slot(2) = next_stack_pointer
output summed
"#;

const STACK_RETRAIN_B: &str = r#"
program tassadar.decompiled.stack_machine_add_step.retrain_b.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 3
input top = slot(0)
input below_top = slot(1)
input pointer = slot(2)
let collapsed = add(top, below_top)
let decremented_pointer = sub(pointer, const(1))
store slot(0) = collapsed
store slot(1) = const(0)
store slot(2) = decremented_pointer
output collapsed
"#;

const STACK_RETRAIN_C: &str = r#"
program tassadar.decompiled.stack_machine_add_step.retrain_c.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 3
input lhs_stack = slot(0)
input rhs_stack = slot(1)
input sp = slot(2)
let merged_value = add(lhs_stack, rhs_stack)
let updated_sp = sub(sp, const(1))
store slot(0) = merged_value
store slot(1) = const(0)
store slot(2) = updated_sp
output merged_value
"#;

#[cfg(test)]
mod tests {
    use super::{
        tassadar_decompilable_executor_publication, TassadarDecompilableExecutorPublicationStatus,
        TassadarDecompilationFamily, TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_REPORT_REF,
        TASSADAR_DECOMPILATION_FIDELITY_REPORT_REF,
    };

    #[test]
    fn decompilable_executor_publication_is_machine_legible() {
        let publication = tassadar_decompilable_executor_publication();
        assert_eq!(
            publication.status,
            TassadarDecompilableExecutorPublicationStatus::ImplementedEarly
        );
        assert_eq!(publication.model.family, "tassadar_decompilable_executor");
        assert_eq!(publication.cases.len(), 5);
        assert_eq!(
            publication.validation_refs,
            vec![
                String::from(TASSADAR_DECOMPILATION_FIDELITY_REPORT_REF),
                String::from(TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_REPORT_REF),
            ]
        );
        assert!(!publication.publication_digest.is_empty());
    }

    #[test]
    fn decompilable_executor_publication_carries_operator_and_state_delta_families() {
        let publication = tassadar_decompilable_executor_publication();
        let operator_count = publication
            .cases
            .iter()
            .filter(|case| case.family == TassadarDecompilationFamily::SymbolicOperatorLattice)
            .count();
        let state_delta_count = publication
            .cases
            .iter()
            .filter(|case| case.family == TassadarDecompilationFamily::StateDeltaSketch)
            .count();
        assert_eq!(operator_count, 3);
        assert_eq!(state_delta_count, 2);
        assert!(publication
            .cases
            .iter()
            .all(|case| case.retrains.len() == 3 && !case.reference_program_digest.is_empty()));
    }
}
