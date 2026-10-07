use std::collections::{BTreeMap, BTreeSet};

use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    DatasetContractError, DatasetKey, DatasetManifest, DatasetPackingPlan, DatasetPackingPolicy,
    DatasetRecordEncoding, DatasetSequenceDescriptor, DatasetShardManifest,
    DatasetSplitDeclaration, DatasetSplitKind, TokenizerDigest,
};

/// Stable ABI version for Tassadar token-sequence dataset contracts.
pub const TASSADAR_SEQUENCE_DATASET_ABI_VERSION: &str = "psionic.tassadar.sequence_dataset.v1";
/// Stable ABI version for Tassadar benchmark-package-set contracts.
pub const TASSADAR_BENCHMARK_PACKAGE_SET_ABI_VERSION: &str =
    "psionic.tassadar.benchmark_package_set.v1";
/// Stable ABI version for module-scale Wasm workload-suite contracts.
pub const TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_ABI_VERSION: &str =
    "psionic.tassadar.module_scale_workload_suite.v1";
/// Stable ABI version for Tassadar numeric-opcode ladder contracts.
pub const TASSADAR_NUMERIC_OPCODE_LADDER_ABI_VERSION: &str =
    "psionic.tassadar.numeric_opcode_ladder.v1";
/// Stable ABI version for structured numeric encoding lane contracts.
pub const TASSADAR_STRUCTURED_NUMERIC_ENCODING_LANE_ABI_VERSION: &str =
    "psionic.tassadar.structured_numeric_encoding_lane.v1";
/// Stable ABI version for Tassadar trace-family-set contracts.
pub const TASSADAR_TRACE_FAMILY_SET_ABI_VERSION: &str = "psionic.tassadar.trace_family_set.v1";
/// Stable ABI version for CLRS-to-Wasm bridge contracts.
pub const TASSADAR_CLRS_WASM_BRIDGE_ABI_VERSION: &str = "psionic.tassadar.clrs_wasm_bridge.v1";
/// Stable ABI version for verifier-guided search trace-family contracts.
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_ABI_VERSION: &str =
    "psionic.tassadar.verifier_guided_search_trace_family.v1";
/// Stable public family reference for the verifier-guided search trace lane.
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REF: &str =
    "trace-family://openagents/tassadar/verifier_guided_search";
/// Shared version used by the seeded verifier-guided search lane.
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_VERSION: &str = "2026.03.18";
/// Canonical machine-readable report ref for the verifier-guided search lane.
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_REF: &str =
    "fixtures/tassadar/runs/tassadar_verifier_guided_search_trace_family_v1/search_trace_family_report.json";

/// Benchmark-family taxonomy for the public Tassadar package set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBenchmarkFamily {
    /// Exact arithmetic kernels and microprograms.
    Arithmetic,
    /// CLRS-adjacent benchmark subset currently seeded by shortest-path witnesses.
    ClrsSubset,
    /// Sudoku-class exact-search workloads.
    Sudoku,
    /// Hungarian or matching-class workloads.
    Hungarian,
    /// Explicit trace-length and horizon-stress workloads.
    TraceLengthStress,
}

/// Canonical benchmark-summary axes for Tassadar benchmark packages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBenchmarkAxis {
    /// Exactness or equivalence facts.
    Exactness,
    /// Length-generalization or horizon-scaling posture.
    LengthGeneralization,
    /// Whether the family is useful for planner or route selection rather than only systems work.
    PlannerUsefulness,
}

/// CLRS algorithm family carried by the public CLRS-to-Wasm bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarClrsAlgorithmFamily {
    /// Bounded shortest-path witnesses rooted in the current seeded CLRS lane.
    ShortestPath,
}

/// Trajectory family used by one CLRS-to-Wasm bridge case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarClrsTrajectoryFamily {
    /// Textbook-style sequential relaxation over one fixed witness graph.
    SequentialRelaxation,
    /// Wavefront-style aggregate relaxation over the same fixed witness graph.
    WavefrontRelaxation,
}

/// Length bucket surfaced by one CLRS-to-Wasm bridge export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarClrsLengthBucket {
    /// Tiny fixed witness graph.
    Tiny,
    /// Small fixed witness graph.
    Small,
}

/// Search workload family surfaced by the verifier-guided trace lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarVerifierGuidedSearchWorkloadFamily {
    /// Sudoku-class bounded backtracking search.
    SudokuBacktracking,
    /// Synthetic kernel-style bounded search.
    SearchKernel,
}

/// Event kind carried by the verifier-guided search trace lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarVerifierGuidedSearchEventKind {
    /// One explicit guess action.
    Guess,
    /// One verifier-accepted candidate or partial state.
    Verify,
    /// One contradiction certificate.
    Contradiction,
    /// One explicit backtrack action.
    Backtrack,
    /// One final committed solved state.
    Commit,
}

/// Module-scale deterministic Wasm workload family tracked by the public suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleScaleWorkloadFamily {
    /// Fixed-span copy and state-movement modules.
    Memcpy,
    /// Fixed-token parse and decode modules.
    Parsing,
    /// Fixed-span checksum and accumulation modules.
    Checksum,
    /// Multi-export dispatch or VM-style handler modules.
    VmStyle,
}

/// Expected status for one module-scale workload case in the public suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleScaleWorkloadStatus {
    /// The current lane should lower and execute the case exactly.
    LoweredExact,
    /// The current lane should refuse lowering explicitly.
    LoweringRefused,
    /// The current lane should refuse before lowering.
    CompileRefused,
}

/// Numeric-opcode family tracked by the public Tassadar widening ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNumericOpcodeFamily {
    /// Core i32 arithmetic already admitted by the bounded lane.
    I32CoreArithmetic,
    /// Full i32 comparison and equality family, including `eqz`.
    I32Comparisons,
    /// i32 bitwise and shift family.
    I32BitOps,
    /// i64 integer family that remains out of scope today.
    I64Integer,
    /// Floating-point family that remains out of scope today.
    FloatingPoint,
}

/// Current support posture for one numeric-opcode family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNumericOpcodeFamilyStatus {
    /// The family has exact bounded coverage today.
    Implemented,
    /// The family still refuses explicitly today.
    Refused,
}

/// Family-level contract row for the public numeric-opcode widening ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericOpcodeFamilyContract {
    /// Stable numeric family.
    pub family: TassadarNumericOpcodeFamily,
    /// Current support posture for the family.
    pub status: TassadarNumericOpcodeFamilyStatus,
    /// Human-readable family summary.
    pub summary: String,
    /// Explicit claim boundary for the family.
    pub claim_boundary: String,
    /// Opcodes implemented exactly for the family today.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_opcodes: Vec<String>,
    /// Typed refusal classes still expected for the family today.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusal_kinds: Vec<String>,
    /// Stable seeded case identifiers that anchor the family today.
    pub case_ids: Vec<String>,
}

impl TassadarNumericOpcodeFamilyContract {
    fn validate(&self) -> Result<(), TassadarNumericOpcodeLadderError> {
        if self.summary.trim().is_empty() {
            return Err(TassadarNumericOpcodeLadderError::MissingFamilySummary {
                family: self.family,
            });
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(TassadarNumericOpcodeLadderError::MissingClaimBoundary {
                family: self.family,
            });
        }
        if self.case_ids.is_empty() {
            return Err(TassadarNumericOpcodeLadderError::MissingCaseIds {
                family: self.family,
            });
        }

        match self.status {
            TassadarNumericOpcodeFamilyStatus::Implemented if self.supported_opcodes.is_empty() => {
                return Err(TassadarNumericOpcodeLadderError::MissingSupportedOpcodes {
                    family: self.family,
                });
            }
            TassadarNumericOpcodeFamilyStatus::Refused if self.refusal_kinds.is_empty() => {
                return Err(TassadarNumericOpcodeLadderError::MissingRefusalKinds {
                    family: self.family,
                });
            }
            _ => {}
        }

        let mut seen_supported_opcodes = BTreeSet::new();
        for opcode in &self.supported_opcodes {
            if opcode.trim().is_empty() {
                return Err(TassadarNumericOpcodeLadderError::MissingSupportedOpcode {
                    family: self.family,
                });
            }
            if !seen_supported_opcodes.insert(opcode.clone()) {
                return Err(TassadarNumericOpcodeLadderError::DuplicateSupportedOpcode {
                    family: self.family,
                    opcode: opcode.clone(),
                });
            }
        }

        let mut seen_refusal_kinds = BTreeSet::new();
        for refusal_kind in &self.refusal_kinds {
            if refusal_kind.trim().is_empty() {
                return Err(TassadarNumericOpcodeLadderError::MissingRefusalKind {
                    family: self.family,
                });
            }
            if !seen_refusal_kinds.insert(refusal_kind.clone()) {
                return Err(TassadarNumericOpcodeLadderError::DuplicateRefusalKind {
                    family: self.family,
                    refusal_kind: refusal_kind.clone(),
                });
            }
        }

        let mut seen_case_ids = BTreeSet::new();
        for case_id in &self.case_ids {
            if case_id.trim().is_empty() {
                return Err(TassadarNumericOpcodeLadderError::MissingCaseId {
                    family: self.family,
                });
            }
            if !seen_case_ids.insert(case_id.clone()) {
                return Err(TassadarNumericOpcodeLadderError::DuplicateCaseId {
                    family: self.family,
                    case_id: case_id.clone(),
                });
            }
        }

        Ok(())
    }
}

/// Public contract for the current numeric-opcode widening ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericOpcodeLadderContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable ladder reference.
    pub ladder_ref: String,
    /// Immutable ladder version.
    pub version: String,
    /// Family rows covered by the ladder.
    pub families: Vec<TassadarNumericOpcodeFamilyContract>,
}

impl TassadarNumericOpcodeLadderContract {
    /// Creates and validates a numeric-opcode ladder contract.
    pub fn new(
        ladder_ref: impl Into<String>,
        version: impl Into<String>,
        families: Vec<TassadarNumericOpcodeFamilyContract>,
    ) -> Result<Self, TassadarNumericOpcodeLadderError> {
        let contract = Self {
            abi_version: String::from(TASSADAR_NUMERIC_OPCODE_LADDER_ABI_VERSION),
            ladder_ref: ladder_ref.into(),
            version: version.into(),
            families,
        };
        contract.validate()?;
        Ok(contract)
    }

    /// Validates the contract.
    pub fn validate(&self) -> Result<(), TassadarNumericOpcodeLadderError> {
        if self.abi_version != TASSADAR_NUMERIC_OPCODE_LADDER_ABI_VERSION {
            return Err(TassadarNumericOpcodeLadderError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.ladder_ref.trim().is_empty() {
            return Err(TassadarNumericOpcodeLadderError::MissingLadderRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarNumericOpcodeLadderError::MissingLadderVersion);
        }
        if self.families.is_empty() {
            return Err(TassadarNumericOpcodeLadderError::MissingFamilies);
        }

        let mut seen_families = BTreeSet::new();
        for family in &self.families {
            family.validate()?;
            if !seen_families.insert(family.family) {
                return Err(TassadarNumericOpcodeLadderError::DuplicateFamily {
                    family: family.family,
                });
            }
        }

        Ok(())
    }

    /// Returns a stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_tassadar_numeric_opcode_ladder_contract|", self)
    }
}

/// Returns the current public numeric-opcode widening ladder contract.
#[must_use]
pub fn tassadar_numeric_opcode_ladder_contract() -> TassadarNumericOpcodeLadderContract {
    TassadarNumericOpcodeLadderContract::new(
        "tassadar://numeric_opcode_ladder/structured_control_v1",
        "2026.03.17",
        vec![
            TassadarNumericOpcodeFamilyContract {
                family: TassadarNumericOpcodeFamily::I32CoreArithmetic,
                status: TassadarNumericOpcodeFamilyStatus::Implemented,
                summary: String::from(
                    "core i32 arithmetic remains exact in the bounded structured-control lane",
                ),
                claim_boundary: String::from(
                    "compiled_bounded_exactness over zero-parameter i32-only Wasm functions with empty block types",
                ),
                supported_opcodes: vec![
                    String::from("i32.add"),
                    String::from("i32.sub"),
                    String::from("i32.mul"),
                ],
                refusal_kinds: Vec::new(),
                case_ids: vec![String::from("i32_core_arithmetic_suite")],
            },
            TassadarNumericOpcodeFamilyContract {
                family: TassadarNumericOpcodeFamily::I32Comparisons,
                status: TassadarNumericOpcodeFamilyStatus::Implemented,
                summary: String::from(
                    "full i32 equality and comparison coverage now exists, including `eqz` and signed-vs-unsigned forms",
                ),
                claim_boundary: String::from(
                    "compiled_bounded_exactness for the current zero-parameter i32-only structured-control lowering lane",
                ),
                supported_opcodes: vec![
                    String::from("i32.eqz"),
                    String::from("i32.eq"),
                    String::from("i32.ne"),
                    String::from("i32.lt_s"),
                    String::from("i32.lt_u"),
                    String::from("i32.gt_s"),
                    String::from("i32.gt_u"),
                    String::from("i32.le_s"),
                    String::from("i32.le_u"),
                    String::from("i32.ge_s"),
                    String::from("i32.ge_u"),
                ],
                refusal_kinds: Vec::new(),
                case_ids: vec![String::from("i32_comparison_suite")],
            },
            TassadarNumericOpcodeFamilyContract {
                family: TassadarNumericOpcodeFamily::I32BitOps,
                status: TassadarNumericOpcodeFamilyStatus::Implemented,
                summary: String::from(
                    "i32 bitwise logic and shift operations now lower exactly in the bounded lane",
                ),
                claim_boundary: String::from(
                    "compiled_bounded_exactness for the current zero-parameter i32-only structured-control lowering lane",
                ),
                supported_opcodes: vec![
                    String::from("i32.and"),
                    String::from("i32.or"),
                    String::from("i32.xor"),
                    String::from("i32.shl"),
                    String::from("i32.shr_s"),
                    String::from("i32.shr_u"),
                ],
                refusal_kinds: Vec::new(),
                case_ids: vec![String::from("i32_bit_ops_suite")],
            },
            TassadarNumericOpcodeFamilyContract {
                family: TassadarNumericOpcodeFamily::I64Integer,
                status: TassadarNumericOpcodeFamilyStatus::Refused,
                summary: String::from(
                    "i64 result and instruction families remain outside the current i32-only lowering lane",
                ),
                claim_boundary: String::from(
                    "typed refusal only; this issue does not claim i64 closure or mixed-width execution",
                ),
                supported_opcodes: Vec::new(),
                refusal_kinds: vec![String::from("unsupported_instruction")],
                case_ids: vec![String::from("i64_refusal")],
            },
            TassadarNumericOpcodeFamilyContract {
                family: TassadarNumericOpcodeFamily::FloatingPoint,
                status: TassadarNumericOpcodeFamilyStatus::Refused,
                summary: String::from(
                    "floating-point lowering remains explicitly gated outside the current structured-control lane",
                ),
                claim_boundary: String::from(
                    "typed refusal only; no floating-point exactness or conformance claim is made here",
                ),
                supported_opcodes: Vec::new(),
                refusal_kinds: vec![String::from("unsupported_instruction")],
                case_ids: vec![String::from("float_refusal")],
            },
        ],
    )
    .expect("current Tassadar numeric-opcode ladder contract should stay valid")
}

/// Numeric-opcode-ladder validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarNumericOpcodeLadderError {
    /// Unsupported ABI version.
    #[error("unsupported Tassadar numeric-opcode ladder ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Missing ladder ref.
    #[error("Tassadar numeric-opcode ladder is missing `ladder_ref`")]
    MissingLadderRef,
    /// Missing ladder version.
    #[error("Tassadar numeric-opcode ladder is missing `version`")]
    MissingLadderVersion,
    /// No families declared.
    #[error("Tassadar numeric-opcode ladder must contain at least one family contract")]
    MissingFamilies,
    /// One family repeated.
    #[error("Tassadar numeric-opcode ladder repeated family `{family:?}`")]
    DuplicateFamily {
        /// Repeated family.
        family: TassadarNumericOpcodeFamily,
    },
    /// Family summary missing.
    #[error("Tassadar numeric-opcode family `{family:?}` is missing `summary`")]
    MissingFamilySummary {
        /// Family.
        family: TassadarNumericOpcodeFamily,
    },
    /// Family claim boundary missing.
    #[error("Tassadar numeric-opcode family `{family:?}` is missing `claim_boundary`")]
    MissingClaimBoundary {
        /// Family.
        family: TassadarNumericOpcodeFamily,
    },
    /// Implemented family lacks supported opcode list.
    #[error(
        "implemented Tassadar numeric-opcode family `{family:?}` must declare supported opcodes"
    )]
    MissingSupportedOpcodes {
        /// Family.
        family: TassadarNumericOpcodeFamily,
    },
    /// Refused family lacks refusal list.
    #[error("refused Tassadar numeric-opcode family `{family:?}` must declare refusal kinds")]
    MissingRefusalKinds {
        /// Family.
        family: TassadarNumericOpcodeFamily,
    },
    /// One supported opcode was empty.
    #[error("Tassadar numeric-opcode family `{family:?}` contains an empty supported opcode")]
    MissingSupportedOpcode {
        /// Family.
        family: TassadarNumericOpcodeFamily,
    },
    /// One refusal kind was empty.
    #[error("Tassadar numeric-opcode family `{family:?}` contains an empty refusal kind")]
    MissingRefusalKind {
        /// Family.
        family: TassadarNumericOpcodeFamily,
    },
    /// No case ids declared.
    #[error("Tassadar numeric-opcode family `{family:?}` must declare at least one case id")]
    MissingCaseIds {
        /// Family.
        family: TassadarNumericOpcodeFamily,
    },
    /// One case id was empty.
    #[error("Tassadar numeric-opcode family `{family:?}` contains an empty case id")]
    MissingCaseId {
        /// Family.
        family: TassadarNumericOpcodeFamily,
    },
    /// One supported opcode repeated.
    #[error("Tassadar numeric-opcode family `{family:?}` repeated supported opcode `{opcode}`")]
    DuplicateSupportedOpcode {
        /// Family.
        family: TassadarNumericOpcodeFamily,
        /// Repeated opcode.
        opcode: String,
    },
    /// One refusal kind repeated.
    #[error("Tassadar numeric-opcode family `{family:?}` repeated refusal kind `{refusal_kind}`")]
    DuplicateRefusalKind {
        /// Family.
        family: TassadarNumericOpcodeFamily,
        /// Repeated refusal kind.
        refusal_kind: String,
    },
    /// One case id repeated.
    #[error("Tassadar numeric-opcode family `{family:?}` repeated case id `{case_id}`")]
    DuplicateCaseId {
        /// Family.
        family: TassadarNumericOpcodeFamily,
        /// Repeated case id.
        case_id: String,
    },
}

/// Workload family summarized by the structured numeric encoding lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStructuredNumericEncodingWorkloadFamily {
    /// Arithmetic kernels whose generalization stress sits in immediate literals.
    ArithmeticImmediates,
    /// Address and offset stress workloads.
    AddressOffsetStress,
    /// Explicit address-family workloads.
    MemoryAddresses,
}

/// One seeded case in the structured numeric encoding lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredNumericEncodingCaseContract {
    /// Stable case identifier.
    pub case_id: String,
    /// Workload family carried by the case.
    pub workload_family: TassadarStructuredNumericEncodingWorkloadFamily,
    /// Stable legacy encoding identifier.
    pub legacy_encoding_id: String,
    /// Stable candidate encoding identifiers.
    pub candidate_encoding_ids: Vec<String>,
    /// Human-readable case summary.
    pub summary: String,
    /// Bounded numeric values visible during the training split.
    pub train_values: Vec<u32>,
    /// Held-out numeric values reserved for evaluation.
    pub held_out_values: Vec<u32>,
}

impl TassadarStructuredNumericEncodingCaseContract {
    fn validate(&self) -> Result<(), TassadarStructuredNumericEncodingLaneError> {
        if self.case_id.trim().is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingCaseId);
        }
        if self.legacy_encoding_id.trim().is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingLegacyEncodingId {
                case_id: self.case_id.clone(),
            });
        }
        if self.candidate_encoding_ids.is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingCandidateEncodingIds {
                case_id: self.case_id.clone(),
            });
        }
        if self.summary.trim().is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingCaseSummary {
                case_id: self.case_id.clone(),
            });
        }
        if self.train_values.is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingTrainValues {
                case_id: self.case_id.clone(),
            });
        }
        if self.held_out_values.is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingHeldOutValues {
                case_id: self.case_id.clone(),
            });
        }

        let mut seen_candidate_ids = BTreeSet::new();
        for candidate_encoding_id in &self.candidate_encoding_ids {
            if candidate_encoding_id.trim().is_empty() {
                return Err(TassadarStructuredNumericEncodingLaneError::MissingCandidateEncodingId {
                    case_id: self.case_id.clone(),
                });
            }
            if !seen_candidate_ids.insert(candidate_encoding_id.clone()) {
                return Err(
                    TassadarStructuredNumericEncodingLaneError::DuplicateCandidateEncodingId {
                        case_id: self.case_id.clone(),
                        encoding_id: candidate_encoding_id.clone(),
                    },
                );
            }
        }

        let train_values = self.train_values.iter().copied().collect::<BTreeSet<_>>();
        let held_out_values = self.held_out_values.iter().copied().collect::<BTreeSet<_>>();
        if !train_values.is_disjoint(&held_out_values) {
            return Err(TassadarStructuredNumericEncodingLaneError::TrainHeldOutOverlap {
                case_id: self.case_id.clone(),
            });
        }
        Ok(())
    }
}

/// Public contract for the structured numeric encoding lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredNumericEncodingLaneContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable lane reference.
    pub lane_ref: String,
    /// Immutable lane version.
    pub version: String,
    /// Workload families covered by the lane.
    pub workload_families: Vec<TassadarStructuredNumericEncodingWorkloadFamily>,
    /// Canonical evaluation axes emitted by the lane.
    pub evaluation_axes: Vec<String>,
    /// Seeded cases under the lane.
    pub cases: Vec<TassadarStructuredNumericEncodingCaseContract>,
    /// Canonical committed report ref for the lane.
    pub report_ref: String,
}

impl TassadarStructuredNumericEncodingLaneContract {
    /// Creates and validates the lane contract.
    pub fn new(
        lane_ref: impl Into<String>,
        version: impl Into<String>,
        workload_families: Vec<TassadarStructuredNumericEncodingWorkloadFamily>,
        evaluation_axes: Vec<String>,
        cases: Vec<TassadarStructuredNumericEncodingCaseContract>,
        report_ref: impl Into<String>,
    ) -> Result<Self, TassadarStructuredNumericEncodingLaneError> {
        let contract = Self {
            abi_version: String::from(TASSADAR_STRUCTURED_NUMERIC_ENCODING_LANE_ABI_VERSION),
            lane_ref: lane_ref.into(),
            version: version.into(),
            workload_families,
            evaluation_axes,
            cases,
            report_ref: report_ref.into(),
        };
        contract.validate()?;
        Ok(contract)
    }

    /// Validates the contract.
    pub fn validate(&self) -> Result<(), TassadarStructuredNumericEncodingLaneError> {
        if self.abi_version != TASSADAR_STRUCTURED_NUMERIC_ENCODING_LANE_ABI_VERSION {
            return Err(
                TassadarStructuredNumericEncodingLaneError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.lane_ref.trim().is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingLaneRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingLaneVersion);
        }
        if self.workload_families.is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingWorkloadFamilies);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingEvaluationAxes);
        }
        if self.cases.is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingCases);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarStructuredNumericEncodingLaneError::MissingReportRef);
        }

        let mut seen_workload_families = BTreeSet::new();
        for workload_family in &self.workload_families {
            if !seen_workload_families.insert(*workload_family) {
                return Err(
                    TassadarStructuredNumericEncodingLaneError::DuplicateWorkloadFamily {
                        workload_family: *workload_family,
                    },
                );
            }
        }
        let mut seen_axes = BTreeSet::new();
        for axis in &self.evaluation_axes {
            if axis.trim().is_empty() {
                return Err(TassadarStructuredNumericEncodingLaneError::MissingEvaluationAxis);
            }
            if !seen_axes.insert(axis.clone()) {
                return Err(
                    TassadarStructuredNumericEncodingLaneError::DuplicateEvaluationAxis {
                        axis: axis.clone(),
                    },
                );
            }
        }
        let supported_families = self.workload_families.iter().copied().collect::<BTreeSet<_>>();
        let mut seen_case_ids = BTreeSet::new();
        for case in &self.cases {
            case.validate()?;
            if !supported_families.contains(&case.workload_family) {
                return Err(
                    TassadarStructuredNumericEncodingLaneError::CaseWorkloadFamilyNotDeclared {
                        case_id: case.case_id.clone(),
                        workload_family: case.workload_family,
                    },
                );
            }
            if !seen_case_ids.insert(case.case_id.clone()) {
                return Err(TassadarStructuredNumericEncodingLaneError::DuplicateCaseId {
                    case_id: case.case_id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Returns a stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(
            b"psionic_tassadar_structured_numeric_encoding_lane_contract|",
            self,
        )
    }
}

/// Returns the current public structured numeric encoding lane contract.
#[must_use]
pub fn tassadar_structured_numeric_encoding_lane_contract(
) -> TassadarStructuredNumericEncodingLaneContract {
    TassadarStructuredNumericEncodingLaneContract::new(
        "tassadar://structured_numeric_encoding_lane/u8_v1",
        "2026.03.18",
        vec![
            TassadarStructuredNumericEncodingWorkloadFamily::ArithmeticImmediates,
            TassadarStructuredNumericEncodingWorkloadFamily::AddressOffsetStress,
            TassadarStructuredNumericEncodingWorkloadFamily::MemoryAddresses,
        ],
        vec![
            String::from("held_out_vocab_coverage_bps"),
            String::from("mean_tokens_per_value"),
            String::from("semantic_roundtrip_exact_bps"),
            String::from("representation_generalization_gain_bps"),
        ],
        vec![
            TassadarStructuredNumericEncodingCaseContract {
                case_id: String::from("arithmetic_immediates_u8"),
                workload_family:
                    TassadarStructuredNumericEncodingWorkloadFamily::ArithmeticImmediates,
                legacy_encoding_id: String::from("tassadar.numeric.immediate.legacy_u8.v1"),
                candidate_encoding_ids: vec![
                    String::from("tassadar.numeric.immediate.binary_u8.v1"),
                    String::from("tassadar.numeric.immediate.hex_u8.v1"),
                ],
                summary: String::from(
                    "bounded arithmetic-kernel immediates with held-out higher literals",
                ),
                train_values: (0_u32..=15).collect(),
                held_out_values: vec![16, 31, 42, 63, 127, 255],
            },
            TassadarStructuredNumericEncodingCaseContract {
                case_id: String::from("address_offsets_u8"),
                workload_family:
                    TassadarStructuredNumericEncodingWorkloadFamily::AddressOffsetStress,
                legacy_encoding_id: String::from("tassadar.numeric.offset.legacy_u8.v1"),
                candidate_encoding_ids: vec![
                    String::from("tassadar.numeric.offset.binary_u8.v1"),
                    String::from("tassadar.numeric.offset.hex_u8.v1"),
                ],
                summary: String::from(
                    "bounded offset families with held-out stride and span combinations",
                ),
                train_values: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 12, 16, 20],
                held_out_values: vec![24, 28, 36, 44, 52, 68, 84, 100],
            },
            TassadarStructuredNumericEncodingCaseContract {
                case_id: String::from("memory_addresses_u8"),
                workload_family:
                    TassadarStructuredNumericEncodingWorkloadFamily::MemoryAddresses,
                legacy_encoding_id: String::from("tassadar.numeric.address.legacy_u8.v1"),
                candidate_encoding_ids: vec![
                    String::from("tassadar.numeric.address.binary_u8.v1"),
                    String::from("tassadar.numeric.address.hex_u8.v1"),
                ],
                summary: String::from(
                    "bounded address families with held-out higher memory buckets",
                ),
                train_values: vec![0, 4, 8, 12, 16, 20, 24, 28, 32, 48, 64, 80],
                held_out_values: vec![96, 112, 128, 144, 160, 176, 192, 224, 240],
            },
        ],
        "fixtures/tassadar/reports/tassadar_numeric_encoding_report.json",
    )
    .expect("current structured numeric encoding lane contract should stay valid")
}

/// Structured numeric encoding lane validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarStructuredNumericEncodingLaneError {
    /// Unsupported ABI version.
    #[error("unsupported structured numeric encoding lane ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Missing lane ref.
    #[error("structured numeric encoding lane is missing `lane_ref`")]
    MissingLaneRef,
    /// Missing version.
    #[error("structured numeric encoding lane is missing `version`")]
    MissingLaneVersion,
    /// Missing workload families.
    #[error("structured numeric encoding lane must declare workload families")]
    MissingWorkloadFamilies,
    /// Missing evaluation axes.
    #[error("structured numeric encoding lane must declare evaluation axes")]
    MissingEvaluationAxes,
    /// One evaluation axis was empty.
    #[error("structured numeric encoding lane contains an empty evaluation axis")]
    MissingEvaluationAxis,
    /// Missing cases.
    #[error("structured numeric encoding lane must declare cases")]
    MissingCases,
    /// Missing report ref.
    #[error("structured numeric encoding lane is missing `report_ref`")]
    MissingReportRef,
    /// One workload family repeated.
    #[error("structured numeric encoding lane repeated workload family `{workload_family:?}`")]
    DuplicateWorkloadFamily {
        /// Repeated workload family.
        workload_family: TassadarStructuredNumericEncodingWorkloadFamily,
    },
    /// One evaluation axis repeated.
    #[error("structured numeric encoding lane repeated evaluation axis `{axis}`")]
    DuplicateEvaluationAxis {
        /// Repeated axis.
        axis: String,
    },
    /// Missing case id.
    #[error("structured numeric encoding lane contains a case without `case_id`")]
    MissingCaseId,
    /// Missing legacy encoding identifier.
    #[error("structured numeric encoding case `{case_id}` is missing `legacy_encoding_id`")]
    MissingLegacyEncodingId {
        /// Case identifier.
        case_id: String,
    },
    /// Missing candidate encodings.
    #[error("structured numeric encoding case `{case_id}` must declare candidate encodings")]
    MissingCandidateEncodingIds {
        /// Case identifier.
        case_id: String,
    },
    /// One candidate encoding identifier was empty.
    #[error("structured numeric encoding case `{case_id}` contains an empty candidate encoding id")]
    MissingCandidateEncodingId {
        /// Case identifier.
        case_id: String,
    },
    /// One candidate encoding identifier repeated.
    #[error("structured numeric encoding case `{case_id}` repeated candidate encoding `{encoding_id}`")]
    DuplicateCandidateEncodingId {
        /// Case identifier.
        case_id: String,
        /// Repeated encoding identifier.
        encoding_id: String,
    },
    /// Missing case summary.
    #[error("structured numeric encoding case `{case_id}` is missing `summary`")]
    MissingCaseSummary {
        /// Case identifier.
        case_id: String,
    },
    /// Missing training values.
    #[error("structured numeric encoding case `{case_id}` must declare train values")]
    MissingTrainValues {
        /// Case identifier.
        case_id: String,
    },
    /// Missing held-out values.
    #[error("structured numeric encoding case `{case_id}` must declare held-out values")]
    MissingHeldOutValues {
        /// Case identifier.
        case_id: String,
    },
    /// Train and held-out sets overlapped.
    #[error("structured numeric encoding case `{case_id}` overlaps train and held-out values")]
    TrainHeldOutOverlap {
        /// Case identifier.
        case_id: String,
    },
    /// One case referenced an undeclared workload family.
    #[error(
        "structured numeric encoding case `{case_id}` references undeclared workload family `{workload_family:?}`"
    )]
    CaseWorkloadFamilyNotDeclared {
        /// Case identifier.
        case_id: String,
        /// Undeclared workload family.
        workload_family: TassadarStructuredNumericEncodingWorkloadFamily,
    },
    /// One case id repeated.
    #[error("structured numeric encoding lane repeated case `{case_id}`")]
    DuplicateCaseId {
        /// Repeated case identifier.
        case_id: String,
    },
}

/// One benchmark package that participates in the public Tassadar package set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBenchmarkPackageBinding {
    /// Stable benchmark reference.
    pub benchmark_ref: String,
    /// Immutable benchmark package version.
    pub version: String,
    /// Environment package that owns benchmark execution.
    pub environment_ref: String,
    /// Bound dataset identity.
    pub dataset: DatasetKey,
    /// Optional dataset split.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
}

impl TassadarBenchmarkPackageBinding {
    fn validate(&self) -> Result<(), TassadarBenchmarkPackageSetError> {
        if self.benchmark_ref.trim().is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingBenchmarkRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingBenchmarkVersion {
                benchmark_ref: self.benchmark_ref.clone(),
            });
        }
        if self.environment_ref.trim().is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingEnvironmentRef {
                benchmark_ref: self.benchmark_ref.clone(),
            });
        }
        if self.dataset.dataset_ref.trim().is_empty() || self.dataset.version.trim().is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingDatasetBinding {
                benchmark_ref: self.benchmark_ref.clone(),
            });
        }
        Ok(())
    }
}

/// Family-level benchmark-package-set contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBenchmarkFamilyContract {
    /// Stable benchmark family.
    pub family: TassadarBenchmarkFamily,
    /// Human-readable family summary.
    pub summary: String,
    /// Benchmark packages that currently cover the family.
    pub benchmark_packages: Vec<TassadarBenchmarkPackageBinding>,
    /// Canonical reporting axes that the family supports.
    pub axis_coverage: Vec<TassadarBenchmarkAxis>,
    /// Stable benchmark case identifiers that seed the family today.
    pub case_ids: Vec<String>,
}

impl TassadarBenchmarkFamilyContract {
    fn validate(&self) -> Result<(), TassadarBenchmarkPackageSetError> {
        if self.summary.trim().is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingFamilySummary {
                family: self.family,
            });
        }
        if self.benchmark_packages.is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingBenchmarkPackages {
                family: self.family,
            });
        }
        if self.axis_coverage.is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingAxisCoverage {
                family: self.family,
            });
        }
        if self.case_ids.is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingCaseIds {
                family: self.family,
            });
        }

        let mut seen_axes = BTreeSet::new();
        for axis in &self.axis_coverage {
            if !seen_axes.insert(*axis) {
                return Err(TassadarBenchmarkPackageSetError::DuplicateAxis {
                    family: self.family,
                    axis: *axis,
                });
            }
        }

        let mut seen_packages = BTreeSet::new();
        for package in &self.benchmark_packages {
            package.validate()?;
            let package_key = format!("{}@{}", package.benchmark_ref, package.version);
            if !seen_packages.insert(package_key.clone()) {
                return Err(
                    TassadarBenchmarkPackageSetError::DuplicateBenchmarkPackage {
                        family: self.family,
                        package_key,
                    },
                );
            }
        }

        let mut seen_case_ids = BTreeSet::new();
        for case_id in &self.case_ids {
            if case_id.trim().is_empty() {
                return Err(TassadarBenchmarkPackageSetError::MissingCaseId {
                    family: self.family,
                });
            }
            if !seen_case_ids.insert(case_id.clone()) {
                return Err(TassadarBenchmarkPackageSetError::DuplicateCaseId {
                    family: self.family,
                    case_id: case_id.clone(),
                });
            }
        }

        Ok(())
    }
}

/// Public package-set contract for the current Tassadar benchmark families.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBenchmarkPackageSetContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable package-set reference.
    pub package_set_ref: String,
    /// Immutable package-set version.
    pub version: String,
    /// Family contracts covered by the package set.
    pub families: Vec<TassadarBenchmarkFamilyContract>,
}

impl TassadarBenchmarkPackageSetContract {
    /// Creates and validates a package-set contract.
    pub fn new(
        package_set_ref: impl Into<String>,
        version: impl Into<String>,
        families: Vec<TassadarBenchmarkFamilyContract>,
    ) -> Result<Self, TassadarBenchmarkPackageSetError> {
        let contract = Self {
            abi_version: String::from(TASSADAR_BENCHMARK_PACKAGE_SET_ABI_VERSION),
            package_set_ref: package_set_ref.into(),
            version: version.into(),
            families,
        };
        contract.validate()?;
        Ok(contract)
    }

    /// Validates the contract.
    pub fn validate(&self) -> Result<(), TassadarBenchmarkPackageSetError> {
        if self.abi_version != TASSADAR_BENCHMARK_PACKAGE_SET_ABI_VERSION {
            return Err(TassadarBenchmarkPackageSetError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.package_set_ref.trim().is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingPackageSetRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingPackageSetVersion);
        }
        if self.families.is_empty() {
            return Err(TassadarBenchmarkPackageSetError::MissingFamilies);
        }

        let mut seen_families = BTreeSet::new();
        for family in &self.families {
            family.validate()?;
            if !seen_families.insert(family.family) {
                return Err(TassadarBenchmarkPackageSetError::DuplicateFamily {
                    family: family.family,
                });
            }
        }

        Ok(())
    }

    /// Returns a stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_tassadar_benchmark_package_set_contract|", self)
    }
}

/// Benchmark-package-set validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarBenchmarkPackageSetError {
    /// Unsupported ABI version.
    #[error("unsupported Tassadar benchmark package set ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Missing package-set ref.
    #[error("Tassadar benchmark package set is missing `package_set_ref`")]
    MissingPackageSetRef,
    /// Missing package-set version.
    #[error("Tassadar benchmark package set is missing `version`")]
    MissingPackageSetVersion,
    /// No family contracts.
    #[error("Tassadar benchmark package set must contain at least one family contract")]
    MissingFamilies,
    /// Family repeated.
    #[error("Tassadar benchmark package set repeated family `{family:?}`")]
    DuplicateFamily {
        /// Repeated family.
        family: TassadarBenchmarkFamily,
    },
    /// Benchmark ref missing.
    #[error("Tassadar benchmark package binding is missing `benchmark_ref`")]
    MissingBenchmarkRef,
    /// Package version missing.
    #[error("Tassadar benchmark package `{benchmark_ref}` is missing `version`")]
    MissingBenchmarkVersion {
        /// Benchmark reference.
        benchmark_ref: String,
    },
    /// Environment ref missing.
    #[error("Tassadar benchmark package `{benchmark_ref}` is missing `environment_ref`")]
    MissingEnvironmentRef {
        /// Benchmark reference.
        benchmark_ref: String,
    },
    /// Dataset binding missing.
    #[error("Tassadar benchmark package `{benchmark_ref}` is missing a stable dataset binding")]
    MissingDatasetBinding {
        /// Benchmark reference.
        benchmark_ref: String,
    },
    /// Family summary missing.
    #[error("Tassadar benchmark family `{family:?}` is missing `summary`")]
    MissingFamilySummary {
        /// Benchmark family.
        family: TassadarBenchmarkFamily,
    },
    /// Family has no packages.
    #[error(
        "Tassadar benchmark family `{family:?}` must reference at least one benchmark package"
    )]
    MissingBenchmarkPackages {
        /// Benchmark family.
        family: TassadarBenchmarkFamily,
    },
    /// Family has no axes.
    #[error("Tassadar benchmark family `{family:?}` must declare axis coverage")]
    MissingAxisCoverage {
        /// Benchmark family.
        family: TassadarBenchmarkFamily,
    },
    /// Family has no cases.
    #[error("Tassadar benchmark family `{family:?}` must declare at least one case id")]
    MissingCaseIds {
        /// Benchmark family.
        family: TassadarBenchmarkFamily,
    },
    /// Empty case id.
    #[error("Tassadar benchmark family `{family:?}` contains an empty case id")]
    MissingCaseId {
        /// Benchmark family.
        family: TassadarBenchmarkFamily,
    },
    /// Repeated axis.
    #[error("Tassadar benchmark family `{family:?}` repeated axis `{axis:?}`")]
    DuplicateAxis {
        /// Benchmark family.
        family: TassadarBenchmarkFamily,
        /// Repeated axis.
        axis: TassadarBenchmarkAxis,
    },
    /// Repeated package.
    #[error("Tassadar benchmark family `{family:?}` repeated benchmark package `{package_key}`")]
    DuplicateBenchmarkPackage {
        /// Benchmark family.
        family: TassadarBenchmarkFamily,
        /// Repeated package key.
        package_key: String,
    },
    /// Repeated case id.
    #[error("Tassadar benchmark family `{family:?}` repeated case id `{case_id}`")]
    DuplicateCaseId {
        /// Benchmark family.
        family: TassadarBenchmarkFamily,
        /// Repeated case id.
        case_id: String,
    },
}

/// One deterministic Wasm source case in the public module-scale workload suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleScaleWorkloadCaseContract {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable workload family.
    pub family: TassadarModuleScaleWorkloadFamily,
    /// Human-readable case summary.
    pub summary: String,
    /// Repo-relative source ref.
    pub source_ref: String,
    /// Expected status under the current bounded lane.
    pub expected_status: TassadarModuleScaleWorkloadStatus,
}

impl TassadarModuleScaleWorkloadCaseContract {
    fn validate(&self) -> Result<(), TassadarModuleScaleWorkloadSuiteError> {
        if self.case_id.trim().is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingCaseId);
        }
        if self.summary.trim().is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingCaseSummary {
                case_id: self.case_id.clone(),
            });
        }
        if self.source_ref.trim().is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingSourceRef {
                case_id: self.case_id.clone(),
            });
        }
        Ok(())
    }
}

/// Public workload-package contract for deterministic module-scale Wasm programs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleScaleWorkloadSuiteContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable suite reference.
    pub suite_ref: String,
    /// Immutable suite version.
    pub version: String,
    /// Stable benchmark reference bound to the suite.
    pub benchmark_ref: String,
    /// Stable environment reference bound to the suite.
    pub environment_ref: String,
    /// Workload families covered by the suite.
    pub supported_families: Vec<TassadarModuleScaleWorkloadFamily>,
    /// Evaluation axes emitted by the suite report.
    pub evaluation_axes: Vec<String>,
    /// Ordered deterministic Wasm cases in the suite.
    pub cases: Vec<TassadarModuleScaleWorkloadCaseContract>,
    /// Canonical machine-readable report ref for the suite.
    pub report_ref: String,
}

impl TassadarModuleScaleWorkloadSuiteContract {
    /// Creates and validates a module-scale workload suite contract.
    pub fn new(
        suite_ref: impl Into<String>,
        version: impl Into<String>,
        benchmark_ref: impl Into<String>,
        environment_ref: impl Into<String>,
        supported_families: Vec<TassadarModuleScaleWorkloadFamily>,
        evaluation_axes: Vec<String>,
        cases: Vec<TassadarModuleScaleWorkloadCaseContract>,
        report_ref: impl Into<String>,
    ) -> Result<Self, TassadarModuleScaleWorkloadSuiteError> {
        let contract = Self {
            abi_version: String::from(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_ABI_VERSION),
            suite_ref: suite_ref.into(),
            version: version.into(),
            benchmark_ref: benchmark_ref.into(),
            environment_ref: environment_ref.into(),
            supported_families,
            evaluation_axes,
            cases,
            report_ref: report_ref.into(),
        };
        contract.validate()?;
        Ok(contract)
    }

    /// Validates the suite contract.
    pub fn validate(&self) -> Result<(), TassadarModuleScaleWorkloadSuiteError> {
        if self.abi_version != TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_ABI_VERSION {
            return Err(
                TassadarModuleScaleWorkloadSuiteError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.suite_ref.trim().is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingSuiteRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingSuiteVersion);
        }
        if self.benchmark_ref.trim().is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingBenchmarkRef);
        }
        if self.environment_ref.trim().is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingEnvironmentRef);
        }
        if self.supported_families.is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingFamilies);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingEvaluationAxes);
        }
        if self.cases.is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingCases);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarModuleScaleWorkloadSuiteError::MissingReportRef);
        }

        let mut seen_families = BTreeSet::new();
        for family in &self.supported_families {
            if !seen_families.insert(*family) {
                return Err(TassadarModuleScaleWorkloadSuiteError::DuplicateFamily {
                    family: *family,
                });
            }
        }

        let mut seen_axes = BTreeSet::new();
        for axis in &self.evaluation_axes {
            if axis.trim().is_empty() {
                return Err(TassadarModuleScaleWorkloadSuiteError::MissingEvaluationAxis);
            }
            if !seen_axes.insert(axis.clone()) {
                return Err(
                    TassadarModuleScaleWorkloadSuiteError::DuplicateEvaluationAxis {
                        axis: axis.clone(),
                    },
                );
            }
        }

        let mut seen_case_ids = BTreeSet::new();
        for case in &self.cases {
            case.validate()?;
            if !seen_families.contains(&case.family) {
                return Err(
                    TassadarModuleScaleWorkloadSuiteError::CaseFamilyNotDeclared {
                        case_id: case.case_id.clone(),
                        family: case.family,
                    },
                );
            }
            if !seen_case_ids.insert(case.case_id.clone()) {
                return Err(TassadarModuleScaleWorkloadSuiteError::DuplicateCaseId {
                    case_id: case.case_id.clone(),
                });
            }
        }

        Ok(())
    }

    /// Returns a stable digest over the suite contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(
            b"psionic_tassadar_module_scale_workload_suite_contract|",
            self,
        )
    }
}

/// Module-scale workload-suite validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarModuleScaleWorkloadSuiteError {
    /// Unsupported ABI version.
    #[error("unsupported module-scale workload suite ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Missing suite ref.
    #[error("module-scale workload suite is missing `suite_ref`")]
    MissingSuiteRef,
    /// Missing suite version.
    #[error("module-scale workload suite is missing `version`")]
    MissingSuiteVersion,
    /// Missing benchmark ref.
    #[error("module-scale workload suite is missing `benchmark_ref`")]
    MissingBenchmarkRef,
    /// Missing environment ref.
    #[error("module-scale workload suite is missing `environment_ref`")]
    MissingEnvironmentRef,
    /// Missing families.
    #[error("module-scale workload suite must declare at least one workload family")]
    MissingFamilies,
    /// Duplicate family.
    #[error("module-scale workload suite repeated family `{family:?}`")]
    DuplicateFamily {
        /// Repeated family.
        family: TassadarModuleScaleWorkloadFamily,
    },
    /// Missing evaluation axes.
    #[error("module-scale workload suite must declare evaluation axes")]
    MissingEvaluationAxes,
    /// One evaluation axis was empty.
    #[error("module-scale workload suite includes an empty evaluation axis")]
    MissingEvaluationAxis,
    /// One evaluation axis repeated.
    #[error("module-scale workload suite repeated evaluation axis `{axis}`")]
    DuplicateEvaluationAxis {
        /// Repeated axis.
        axis: String,
    },
    /// Missing cases.
    #[error("module-scale workload suite must declare at least one case")]
    MissingCases,
    /// Missing report ref.
    #[error("module-scale workload suite is missing `report_ref`")]
    MissingReportRef,
    /// Missing case id.
    #[error("module-scale workload suite contains a case without `case_id`")]
    MissingCaseId,
    /// Missing case summary.
    #[error("module-scale workload case `{case_id}` is missing `summary`")]
    MissingCaseSummary {
        /// Case identifier.
        case_id: String,
    },
    /// Missing source ref.
    #[error("module-scale workload case `{case_id}` is missing `source_ref`")]
    MissingSourceRef {
        /// Case identifier.
        case_id: String,
    },
    /// Repeated case id.
    #[error("module-scale workload suite repeated case `{case_id}`")]
    DuplicateCaseId {
        /// Repeated case identifier.
        case_id: String,
    },
    /// Case family was not declared in the suite header.
    #[error("module-scale workload case `{case_id}` references undeclared family `{family:?}`")]
    CaseFamilyNotDeclared {
        /// Case identifier.
        case_id: String,
        /// Undeclared family.
        family: TassadarModuleScaleWorkloadFamily,
    },
}

/// One length-bucket export carried by a CLRS-to-Wasm bridge case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarClrsWasmBridgeExportContract {
    /// Stable length bucket.
    pub length_bucket: TassadarClrsLengthBucket,
    /// Export symbol implementing the bucket.
    pub export_name: String,
}

impl TassadarClrsWasmBridgeExportContract {
    fn validate(&self, case_id: &str) -> Result<(), TassadarClrsWasmBridgeError> {
        if self.export_name.trim().is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingExportName {
                case_id: String::from(case_id),
                length_bucket: self.length_bucket,
            });
        }
        Ok(())
    }
}

/// One public CLRS-to-Wasm bridge case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarClrsWasmBridgeCaseContract {
    /// Stable case identifier.
    pub case_id: String,
    /// CLRS algorithm family.
    pub algorithm_family: TassadarClrsAlgorithmFamily,
    /// Trajectory family represented by the module.
    pub trajectory_family: TassadarClrsTrajectoryFamily,
    /// Human-readable case summary.
    pub summary: String,
    /// Repo-relative source ref for the module.
    pub source_ref: String,
    /// Length-bucket exports implemented by the module.
    pub export_bindings: Vec<TassadarClrsWasmBridgeExportContract>,
}

impl TassadarClrsWasmBridgeCaseContract {
    fn validate(
        &self,
        supported_algorithms: &BTreeSet<TassadarClrsAlgorithmFamily>,
        supported_trajectories: &BTreeSet<TassadarClrsTrajectoryFamily>,
        supported_length_buckets: &BTreeSet<TassadarClrsLengthBucket>,
    ) -> Result<(), TassadarClrsWasmBridgeError> {
        if self.case_id.trim().is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingCaseId);
        }
        if self.summary.trim().is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingCaseSummary {
                case_id: self.case_id.clone(),
            });
        }
        if self.source_ref.trim().is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingCaseSourceRef {
                case_id: self.case_id.clone(),
            });
        }
        if !supported_algorithms.contains(&self.algorithm_family) {
            return Err(TassadarClrsWasmBridgeError::CaseAlgorithmNotDeclared {
                case_id: self.case_id.clone(),
                algorithm_family: self.algorithm_family,
            });
        }
        if !supported_trajectories.contains(&self.trajectory_family) {
            return Err(TassadarClrsWasmBridgeError::CaseTrajectoryNotDeclared {
                case_id: self.case_id.clone(),
                trajectory_family: self.trajectory_family,
            });
        }
        if self.export_bindings.is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingCaseExports {
                case_id: self.case_id.clone(),
            });
        }

        let mut seen_length_buckets = BTreeSet::new();
        for export in &self.export_bindings {
            if !supported_length_buckets.contains(&export.length_bucket) {
                return Err(TassadarClrsWasmBridgeError::ExportLengthBucketNotDeclared {
                    case_id: self.case_id.clone(),
                    length_bucket: export.length_bucket,
                });
            }
            export.validate(self.case_id.as_str())?;
            if !seen_length_buckets.insert(export.length_bucket) {
                return Err(
                    TassadarClrsWasmBridgeError::DuplicateCaseExportLengthBucket {
                        case_id: self.case_id.clone(),
                        length_bucket: export.length_bucket,
                    },
                );
            }
        }

        Ok(())
    }
}

/// Public contract for the literature-facing CLRS-to-Wasm bridge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarClrsWasmBridgeContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable bridge reference.
    pub bridge_ref: String,
    /// Immutable bridge version.
    pub version: String,
    /// Stable benchmark reference bound to the bridge.
    pub benchmark_ref: String,
    /// Stable benchmark environment reference bound to the bridge.
    pub environment_ref: String,
    /// CLRS algorithm families currently represented.
    pub supported_algorithms: Vec<TassadarClrsAlgorithmFamily>,
    /// Trajectory families currently represented.
    pub trajectory_families: Vec<TassadarClrsTrajectoryFamily>,
    /// Length buckets currently represented.
    pub length_buckets: Vec<TassadarClrsLengthBucket>,
    /// Evaluation axes published by the committed report.
    pub evaluation_axes: Vec<String>,
    /// Explicit bridge cases.
    pub cases: Vec<TassadarClrsWasmBridgeCaseContract>,
    /// Canonical machine-readable report ref for the bridge.
    pub report_ref: String,
}

impl TassadarClrsWasmBridgeContract {
    /// Creates and validates a CLRS-to-Wasm bridge contract.
    pub fn new(
        bridge_ref: impl Into<String>,
        version: impl Into<String>,
        benchmark_ref: impl Into<String>,
        environment_ref: impl Into<String>,
        supported_algorithms: Vec<TassadarClrsAlgorithmFamily>,
        trajectory_families: Vec<TassadarClrsTrajectoryFamily>,
        length_buckets: Vec<TassadarClrsLengthBucket>,
        evaluation_axes: Vec<String>,
        cases: Vec<TassadarClrsWasmBridgeCaseContract>,
        report_ref: impl Into<String>,
    ) -> Result<Self, TassadarClrsWasmBridgeError> {
        let contract = Self {
            abi_version: String::from(TASSADAR_CLRS_WASM_BRIDGE_ABI_VERSION),
            bridge_ref: bridge_ref.into(),
            version: version.into(),
            benchmark_ref: benchmark_ref.into(),
            environment_ref: environment_ref.into(),
            supported_algorithms,
            trajectory_families,
            length_buckets,
            evaluation_axes,
            cases,
            report_ref: report_ref.into(),
        };
        contract.validate()?;
        Ok(contract)
    }

    /// Validates the bridge contract.
    pub fn validate(&self) -> Result<(), TassadarClrsWasmBridgeError> {
        if self.abi_version != TASSADAR_CLRS_WASM_BRIDGE_ABI_VERSION {
            return Err(TassadarClrsWasmBridgeError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.bridge_ref.trim().is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingBridgeRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingBridgeVersion);
        }
        if self.benchmark_ref.trim().is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingBenchmarkRef);
        }
        if self.environment_ref.trim().is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingEnvironmentRef);
        }
        if self.supported_algorithms.is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingAlgorithms);
        }
        if self.trajectory_families.is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingTrajectoryFamilies);
        }
        if self.length_buckets.is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingLengthBuckets);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingEvaluationAxes);
        }
        if self.cases.is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingCases);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarClrsWasmBridgeError::MissingReportRef);
        }

        let mut seen_algorithms = BTreeSet::new();
        for algorithm in &self.supported_algorithms {
            if !seen_algorithms.insert(*algorithm) {
                return Err(TassadarClrsWasmBridgeError::DuplicateAlgorithm {
                    algorithm_family: *algorithm,
                });
            }
        }
        let mut seen_trajectories = BTreeSet::new();
        for trajectory in &self.trajectory_families {
            if !seen_trajectories.insert(*trajectory) {
                return Err(TassadarClrsWasmBridgeError::DuplicateTrajectoryFamily {
                    trajectory_family: *trajectory,
                });
            }
        }
        let mut seen_length_buckets = BTreeSet::new();
        for length_bucket in &self.length_buckets {
            if !seen_length_buckets.insert(*length_bucket) {
                return Err(TassadarClrsWasmBridgeError::DuplicateLengthBucket {
                    length_bucket: *length_bucket,
                });
            }
        }
        let mut seen_axes = BTreeSet::new();
        for axis in &self.evaluation_axes {
            if axis.trim().is_empty() {
                return Err(TassadarClrsWasmBridgeError::MissingEvaluationAxis);
            }
            if !seen_axes.insert(axis.clone()) {
                return Err(TassadarClrsWasmBridgeError::DuplicateEvaluationAxis {
                    axis: axis.clone(),
                });
            }
        }

        let supported_algorithms = self.supported_algorithms.iter().copied().collect();
        let supported_trajectories = self.trajectory_families.iter().copied().collect();
        let supported_length_buckets = self.length_buckets.iter().copied().collect();
        let mut seen_case_ids = BTreeSet::new();
        let mut seen_algorithm_trajectories = BTreeSet::new();
        for case in &self.cases {
            case.validate(
                &supported_algorithms,
                &supported_trajectories,
                &supported_length_buckets,
            )?;
            if !seen_case_ids.insert(case.case_id.clone()) {
                return Err(TassadarClrsWasmBridgeError::DuplicateCaseId {
                    case_id: case.case_id.clone(),
                });
            }
            if !seen_algorithm_trajectories.insert((case.algorithm_family, case.trajectory_family))
            {
                return Err(TassadarClrsWasmBridgeError::DuplicateCaseTrajectory {
                    algorithm_family: case.algorithm_family,
                    trajectory_family: case.trajectory_family,
                });
            }
        }

        Ok(())
    }

    /// Returns a stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_tassadar_clrs_wasm_bridge_contract|", self)
    }
}

/// CLRS-to-Wasm bridge validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarClrsWasmBridgeError {
    /// Unsupported ABI version.
    #[error("unsupported Tassadar CLRS-to-Wasm bridge ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Missing bridge ref.
    #[error("Tassadar CLRS-to-Wasm bridge is missing `bridge_ref`")]
    MissingBridgeRef,
    /// Missing bridge version.
    #[error("Tassadar CLRS-to-Wasm bridge is missing `version`")]
    MissingBridgeVersion,
    /// Missing benchmark ref.
    #[error("Tassadar CLRS-to-Wasm bridge is missing `benchmark_ref`")]
    MissingBenchmarkRef,
    /// Missing environment ref.
    #[error("Tassadar CLRS-to-Wasm bridge is missing `environment_ref`")]
    MissingEnvironmentRef,
    /// Missing algorithms.
    #[error("Tassadar CLRS-to-Wasm bridge must declare at least one algorithm family")]
    MissingAlgorithms,
    /// Missing trajectory families.
    #[error("Tassadar CLRS-to-Wasm bridge must declare at least one trajectory family")]
    MissingTrajectoryFamilies,
    /// Missing length buckets.
    #[error("Tassadar CLRS-to-Wasm bridge must declare at least one length bucket")]
    MissingLengthBuckets,
    /// Missing evaluation axes.
    #[error("Tassadar CLRS-to-Wasm bridge must declare evaluation axes")]
    MissingEvaluationAxes,
    /// One evaluation axis was empty.
    #[error("Tassadar CLRS-to-Wasm bridge contains an empty evaluation axis")]
    MissingEvaluationAxis,
    /// Missing cases.
    #[error("Tassadar CLRS-to-Wasm bridge must declare at least one case")]
    MissingCases,
    /// Missing report ref.
    #[error("Tassadar CLRS-to-Wasm bridge is missing `report_ref`")]
    MissingReportRef,
    /// One algorithm repeated.
    #[error("Tassadar CLRS-to-Wasm bridge repeated algorithm family `{algorithm_family:?}`")]
    DuplicateAlgorithm {
        /// Repeated algorithm family.
        algorithm_family: TassadarClrsAlgorithmFamily,
    },
    /// One trajectory family repeated.
    #[error("Tassadar CLRS-to-Wasm bridge repeated trajectory family `{trajectory_family:?}`")]
    DuplicateTrajectoryFamily {
        /// Repeated trajectory family.
        trajectory_family: TassadarClrsTrajectoryFamily,
    },
    /// One length bucket repeated.
    #[error("Tassadar CLRS-to-Wasm bridge repeated length bucket `{length_bucket:?}`")]
    DuplicateLengthBucket {
        /// Repeated length bucket.
        length_bucket: TassadarClrsLengthBucket,
    },
    /// One evaluation axis repeated.
    #[error("Tassadar CLRS-to-Wasm bridge repeated evaluation axis `{axis}`")]
    DuplicateEvaluationAxis {
        /// Repeated evaluation axis.
        axis: String,
    },
    /// Missing case identifier.
    #[error("Tassadar CLRS-to-Wasm bridge case is missing `case_id`")]
    MissingCaseId,
    /// Missing case summary.
    #[error("Tassadar CLRS-to-Wasm bridge case `{case_id}` is missing `summary`")]
    MissingCaseSummary {
        /// Case identifier.
        case_id: String,
    },
    /// Missing case source ref.
    #[error("Tassadar CLRS-to-Wasm bridge case `{case_id}` is missing `source_ref`")]
    MissingCaseSourceRef {
        /// Case identifier.
        case_id: String,
    },
    /// Missing case exports.
    #[error("Tassadar CLRS-to-Wasm bridge case `{case_id}` must declare exports")]
    MissingCaseExports {
        /// Case identifier.
        case_id: String,
    },
    /// One case algorithm was not declared globally.
    #[error(
        "Tassadar CLRS-to-Wasm bridge case `{case_id}` uses undeclared algorithm family `{algorithm_family:?}`"
    )]
    CaseAlgorithmNotDeclared {
        /// Case identifier.
        case_id: String,
        /// Undeclared algorithm family.
        algorithm_family: TassadarClrsAlgorithmFamily,
    },
    /// One case trajectory was not declared globally.
    #[error(
        "Tassadar CLRS-to-Wasm bridge case `{case_id}` uses undeclared trajectory family `{trajectory_family:?}`"
    )]
    CaseTrajectoryNotDeclared {
        /// Case identifier.
        case_id: String,
        /// Undeclared trajectory family.
        trajectory_family: TassadarClrsTrajectoryFamily,
    },
    /// One export length bucket was not declared globally.
    #[error(
        "Tassadar CLRS-to-Wasm bridge case `{case_id}` uses undeclared length bucket `{length_bucket:?}`"
    )]
    ExportLengthBucketNotDeclared {
        /// Case identifier.
        case_id: String,
        /// Undeclared length bucket.
        length_bucket: TassadarClrsLengthBucket,
    },
    /// One export name was missing.
    #[error(
        "Tassadar CLRS-to-Wasm bridge case `{case_id}` is missing export name for length bucket `{length_bucket:?}`"
    )]
    MissingExportName {
        /// Case identifier.
        case_id: String,
        /// Length bucket.
        length_bucket: TassadarClrsLengthBucket,
    },
    /// One case identifier repeated.
    #[error("Tassadar CLRS-to-Wasm bridge repeated case `{case_id}`")]
    DuplicateCaseId {
        /// Repeated case identifier.
        case_id: String,
    },
    /// One algorithm/trajectory pair repeated.
    #[error(
        "Tassadar CLRS-to-Wasm bridge repeated case trajectory `{algorithm_family:?}` / `{trajectory_family:?}`"
    )]
    DuplicateCaseTrajectory {
        /// Algorithm family.
        algorithm_family: TassadarClrsAlgorithmFamily,
        /// Trajectory family.
        trajectory_family: TassadarClrsTrajectoryFamily,
    },
    /// One case repeated a length bucket.
    #[error(
        "Tassadar CLRS-to-Wasm bridge case `{case_id}` repeated length bucket `{length_bucket:?}`"
    )]
    DuplicateCaseExportLengthBucket {
        /// Case identifier.
        case_id: String,
        /// Repeated length bucket.
        length_bucket: TassadarClrsLengthBucket,
    },
}

/// One seeded case in the verifier-guided search trace family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchTraceCaseContract {
    /// Stable case identifier.
    pub case_id: String,
    /// Search workload family.
    pub workload_family: TassadarVerifierGuidedSearchWorkloadFamily,
    /// Human-readable case summary.
    pub summary: String,
    /// Explicit maximum guess count admitted by the seeded case.
    pub max_guess_budget: u32,
    /// Explicit maximum backtrack count admitted by the seeded case.
    pub max_backtrack_budget: u32,
}

impl TassadarVerifierGuidedSearchTraceCaseContract {
    fn validate(
        &self,
        supported_workload_families: &BTreeSet<TassadarVerifierGuidedSearchWorkloadFamily>,
    ) -> Result<(), TassadarVerifierGuidedSearchTraceFamilyError> {
        if self.case_id.trim().is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingCaseId);
        }
        if self.summary.trim().is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingCaseSummary {
                case_id: self.case_id.clone(),
            });
        }
        if !supported_workload_families.contains(&self.workload_family) {
            return Err(
                TassadarVerifierGuidedSearchTraceFamilyError::CaseWorkloadFamilyNotDeclared {
                    case_id: self.case_id.clone(),
                    workload_family: self.workload_family,
                },
            );
        }
        if self.max_guess_budget == 0 {
            return Err(
                TassadarVerifierGuidedSearchTraceFamilyError::InvalidCaseGuessBudget {
                    case_id: self.case_id.clone(),
                },
            );
        }
        if self.max_backtrack_budget == 0 {
            return Err(
                TassadarVerifierGuidedSearchTraceFamilyError::InvalidCaseBacktrackBudget {
                    case_id: self.case_id.clone(),
                },
            );
        }
        Ok(())
    }
}

/// Public contract for the verifier-guided search trace family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchTraceFamilyContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable family reference.
    pub family_ref: String,
    /// Immutable family version.
    pub version: String,
    /// Supported search workload families.
    pub supported_workload_families: Vec<TassadarVerifierGuidedSearchWorkloadFamily>,
    /// Supported event kinds in the trace lane.
    pub event_kinds: Vec<TassadarVerifierGuidedSearchEventKind>,
    /// Evaluation axes surfaced by the committed reports.
    pub evaluation_axes: Vec<String>,
    /// Seeded benchmark-bound cases.
    pub cases: Vec<TassadarVerifierGuidedSearchTraceCaseContract>,
    /// Canonical machine-readable report ref for the lane.
    pub report_ref: String,
}

impl TassadarVerifierGuidedSearchTraceFamilyContract {
    /// Creates and validates the search trace-family contract.
    pub fn new(
        family_ref: impl Into<String>,
        version: impl Into<String>,
        supported_workload_families: Vec<TassadarVerifierGuidedSearchWorkloadFamily>,
        event_kinds: Vec<TassadarVerifierGuidedSearchEventKind>,
        evaluation_axes: Vec<String>,
        cases: Vec<TassadarVerifierGuidedSearchTraceCaseContract>,
        report_ref: impl Into<String>,
    ) -> Result<Self, TassadarVerifierGuidedSearchTraceFamilyError> {
        let contract = Self {
            abi_version: String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_ABI_VERSION),
            family_ref: family_ref.into(),
            version: version.into(),
            supported_workload_families,
            event_kinds,
            evaluation_axes,
            cases,
            report_ref: report_ref.into(),
        };
        contract.validate()?;
        Ok(contract)
    }

    /// Validates the contract.
    pub fn validate(&self) -> Result<(), TassadarVerifierGuidedSearchTraceFamilyError> {
        if self.abi_version != TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_ABI_VERSION {
            return Err(
                TassadarVerifierGuidedSearchTraceFamilyError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.family_ref.trim().is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingFamilyRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingVersion);
        }
        if self.supported_workload_families.is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingWorkloadFamilies);
        }
        if self.event_kinds.is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingEventKinds);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingEvaluationAxes);
        }
        if self.cases.is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingCases);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingReportRef);
        }

        let mut seen_workload_families = BTreeSet::new();
        for workload_family in &self.supported_workload_families {
            if !seen_workload_families.insert(*workload_family) {
                return Err(
                    TassadarVerifierGuidedSearchTraceFamilyError::DuplicateWorkloadFamily {
                        workload_family: *workload_family,
                    },
                );
            }
        }
        let mut seen_event_kinds = BTreeSet::new();
        for event_kind in &self.event_kinds {
            if !seen_event_kinds.insert(*event_kind) {
                return Err(TassadarVerifierGuidedSearchTraceFamilyError::DuplicateEventKind {
                    event_kind: *event_kind,
                });
            }
        }
        let mut seen_axes = BTreeSet::new();
        for axis in &self.evaluation_axes {
            if axis.trim().is_empty() {
                return Err(TassadarVerifierGuidedSearchTraceFamilyError::MissingEvaluationAxis);
            }
            if !seen_axes.insert(axis.clone()) {
                return Err(
                    TassadarVerifierGuidedSearchTraceFamilyError::DuplicateEvaluationAxis {
                        axis: axis.clone(),
                    },
                );
            }
        }

        let supported_workload_families = self.supported_workload_families.iter().copied().collect();
        let mut seen_case_ids = BTreeSet::new();
        for case in &self.cases {
            case.validate(&supported_workload_families)?;
            if !seen_case_ids.insert(case.case_id.clone()) {
                return Err(TassadarVerifierGuidedSearchTraceFamilyError::DuplicateCaseId {
                    case_id: case.case_id.clone(),
                });
            }
        }

        Ok(())
    }

    /// Returns a stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(
            b"psionic_tassadar_verifier_guided_search_trace_family_contract|",
            self,
        )
    }
}

/// Returns the current public verifier-guided search trace-family contract.
#[must_use]
pub fn tassadar_verifier_guided_search_trace_family_contract(
) -> TassadarVerifierGuidedSearchTraceFamilyContract {
    TassadarVerifierGuidedSearchTraceFamilyContract::new(
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REF,
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_VERSION,
        vec![
            TassadarVerifierGuidedSearchWorkloadFamily::SudokuBacktracking,
            TassadarVerifierGuidedSearchWorkloadFamily::SearchKernel,
        ],
        vec![
            TassadarVerifierGuidedSearchEventKind::Guess,
            TassadarVerifierGuidedSearchEventKind::Verify,
            TassadarVerifierGuidedSearchEventKind::Contradiction,
            TassadarVerifierGuidedSearchEventKind::Backtrack,
            TassadarVerifierGuidedSearchEventKind::Commit,
        ],
        vec![
            String::from("backtrack_exactness_bps"),
            String::from("verifier_certificate_accuracy_bps"),
            String::from("guess_count"),
            String::from("recovery_quality_bps"),
        ],
        vec![
            TassadarVerifierGuidedSearchTraceCaseContract {
                case_id: String::from("sudoku_v0_train_a"),
                workload_family: TassadarVerifierGuidedSearchWorkloadFamily::SudokuBacktracking,
                summary: String::from(
                    "real Sudoku-v0 search case with one contradiction certificate and one bounded backtrack",
                ),
                max_guess_budget: 3,
                max_backtrack_budget: 2,
            },
            TassadarVerifierGuidedSearchTraceCaseContract {
                case_id: String::from("search_kernel_two_branch_recovery"),
                workload_family: TassadarVerifierGuidedSearchWorkloadFamily::SearchKernel,
                summary: String::from(
                    "synthetic two-branch recovery kernel with one dead-end contradiction and one successful fallback branch",
                ),
                max_guess_budget: 2,
                max_backtrack_budget: 1,
            },
        ],
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_REPORT_REF,
    )
    .expect("current verifier-guided search trace-family contract should stay valid")
}

/// Search trace-family validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarVerifierGuidedSearchTraceFamilyError {
    /// Unsupported ABI version.
    #[error("unsupported verifier-guided search trace-family ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Missing family ref.
    #[error("verifier-guided search trace family is missing `family_ref`")]
    MissingFamilyRef,
    /// Missing version.
    #[error("verifier-guided search trace family is missing `version`")]
    MissingVersion,
    /// Missing workload families.
    #[error("verifier-guided search trace family must declare workload families")]
    MissingWorkloadFamilies,
    /// Missing event kinds.
    #[error("verifier-guided search trace family must declare event kinds")]
    MissingEventKinds,
    /// Missing evaluation axes.
    #[error("verifier-guided search trace family must declare evaluation axes")]
    MissingEvaluationAxes,
    /// One evaluation axis was empty.
    #[error("verifier-guided search trace family contains an empty evaluation axis")]
    MissingEvaluationAxis,
    /// Missing cases.
    #[error("verifier-guided search trace family must declare cases")]
    MissingCases,
    /// Missing report ref.
    #[error("verifier-guided search trace family is missing `report_ref`")]
    MissingReportRef,
    /// One workload family repeated.
    #[error("verifier-guided search trace family repeated workload family `{workload_family:?}`")]
    DuplicateWorkloadFamily {
        /// Repeated workload family.
        workload_family: TassadarVerifierGuidedSearchWorkloadFamily,
    },
    /// One event kind repeated.
    #[error("verifier-guided search trace family repeated event kind `{event_kind:?}`")]
    DuplicateEventKind {
        /// Repeated event kind.
        event_kind: TassadarVerifierGuidedSearchEventKind,
    },
    /// One evaluation axis repeated.
    #[error("verifier-guided search trace family repeated evaluation axis `{axis}`")]
    DuplicateEvaluationAxis {
        /// Repeated axis.
        axis: String,
    },
    /// Missing case id.
    #[error("verifier-guided search trace family contains a case without `case_id`")]
    MissingCaseId,
    /// Missing case summary.
    #[error("verifier-guided search trace case `{case_id}` is missing `summary`")]
    MissingCaseSummary {
        /// Case identifier.
        case_id: String,
    },
    /// One case referenced an undeclared workload family.
    #[error(
        "verifier-guided search trace case `{case_id}` references undeclared workload family `{workload_family:?}`"
    )]
    CaseWorkloadFamilyNotDeclared {
        /// Case identifier.
        case_id: String,
        /// Undeclared workload family.
        workload_family: TassadarVerifierGuidedSearchWorkloadFamily,
    },
    /// One case declared an invalid guess budget.
    #[error("verifier-guided search trace case `{case_id}` has invalid `max_guess_budget=0`")]
    InvalidCaseGuessBudget {
        /// Case identifier.
        case_id: String,
    },
    /// One case declared an invalid backtrack budget.
    #[error(
        "verifier-guided search trace case `{case_id}` has invalid `max_backtrack_budget=0`"
    )]
    InvalidCaseBacktrackBudget {
        /// Case identifier.
        case_id: String,
    },
    /// One case id repeated.
    #[error("verifier-guided search trace family repeated case `{case_id}`")]
    DuplicateCaseId {
        /// Repeated case identifier.
        case_id: String,
    },
}

/// Topology classification for one public Tassadar trace family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTraceFamilyTopology {
    /// Canonical CPU-style append-only trace authority.
    SequentialCpuReference,
    /// Parallel or wavefront-style target family.
    ParallelWavefront,
    /// Parallel frontier-style target family.
    ParallelFrontier,
}

/// Honest reconstruction scope admitted by one public Tassadar trace family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTraceFamilyAuthorityScope {
    /// Full CPU-trace reconstruction is preserved.
    FullCpuTrace,
    /// Only final outputs are preserved exactly.
    FinalOutputsOnly,
}

/// One workload currently bound to one public Tassadar trace family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceFamilyWorkloadBinding {
    /// Stable workload or dataset reference.
    pub workload_ref: String,
    /// Honest claim boundary currently attached to this workload/family pairing.
    pub claim_boundary: String,
}

impl TassadarTraceFamilyWorkloadBinding {
    fn validate(&self, family_label: &str) -> Result<(), TassadarTraceFamilySetError> {
        if self.workload_ref.trim().is_empty() {
            return Err(TassadarTraceFamilySetError::MissingWorkloadRef {
                family_label: family_label.to_string(),
            });
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(TassadarTraceFamilySetError::MissingWorkloadClaimBoundary {
                family_label: family_label.to_string(),
                workload_ref: self.workload_ref.clone(),
            });
        }
        Ok(())
    }
}

/// Public contract for one comparable Tassadar trace family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceFamilyContract {
    /// Stable trace-family label.
    pub family_label: String,
    /// High-level topology for the family.
    pub topology: TassadarTraceFamilyTopology,
    /// Human-readable family summary.
    pub summary: String,
    /// Optional dataset suffix used when this family materializes alternate targets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_suffix: Option<String>,
    /// Honest reconstruction scope for the family.
    pub authority_scope: TassadarTraceFamilyAuthorityScope,
    /// Workloads currently covered by this family.
    pub workloads: Vec<TassadarTraceFamilyWorkloadBinding>,
}

impl TassadarTraceFamilyContract {
    fn validate(&self) -> Result<(), TassadarTraceFamilySetError> {
        if self.family_label.trim().is_empty() {
            return Err(TassadarTraceFamilySetError::MissingFamilyLabel);
        }
        if self.summary.trim().is_empty() {
            return Err(TassadarTraceFamilySetError::MissingFamilySummary {
                family_label: self.family_label.clone(),
            });
        }
        if self
            .dataset_suffix
            .as_ref()
            .is_some_and(|suffix| suffix.trim().is_empty())
        {
            return Err(TassadarTraceFamilySetError::EmptyDatasetSuffix {
                family_label: self.family_label.clone(),
            });
        }
        if self.workloads.is_empty() {
            return Err(TassadarTraceFamilySetError::MissingWorkloads {
                family_label: self.family_label.clone(),
            });
        }

        let mut seen_workloads = BTreeSet::new();
        for workload in &self.workloads {
            workload.validate(self.family_label.as_str())?;
            if !seen_workloads.insert(workload.workload_ref.clone()) {
                return Err(TassadarTraceFamilySetError::DuplicateWorkloadRef {
                    family_label: self.family_label.clone(),
                    workload_ref: workload.workload_ref.clone(),
                });
            }
        }

        Ok(())
    }
}

/// Public package-set contract for comparable Tassadar trace families.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceFamilySetContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable trace-family-set reference.
    pub trace_family_set_ref: String,
    /// Immutable set version.
    pub version: String,
    /// Public trace families in the set.
    pub families: Vec<TassadarTraceFamilyContract>,
}

impl TassadarTraceFamilySetContract {
    /// Creates and validates a comparable trace-family-set contract.
    pub fn new(
        trace_family_set_ref: impl Into<String>,
        version: impl Into<String>,
        families: Vec<TassadarTraceFamilyContract>,
    ) -> Result<Self, TassadarTraceFamilySetError> {
        let contract = Self {
            abi_version: String::from(TASSADAR_TRACE_FAMILY_SET_ABI_VERSION),
            trace_family_set_ref: trace_family_set_ref.into(),
            version: version.into(),
            families,
        };
        contract.validate()?;
        Ok(contract)
    }

    /// Validates the contract.
    pub fn validate(&self) -> Result<(), TassadarTraceFamilySetError> {
        if self.abi_version != TASSADAR_TRACE_FAMILY_SET_ABI_VERSION {
            return Err(TassadarTraceFamilySetError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.trace_family_set_ref.trim().is_empty() {
            return Err(TassadarTraceFamilySetError::MissingTraceFamilySetRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarTraceFamilySetError::MissingTraceFamilySetVersion);
        }
        if self.families.is_empty() {
            return Err(TassadarTraceFamilySetError::MissingFamilies);
        }

        let mut seen_family_labels = BTreeSet::new();
        for family in &self.families {
            family.validate()?;
            if !seen_family_labels.insert(family.family_label.clone()) {
                return Err(TassadarTraceFamilySetError::DuplicateFamilyLabel {
                    family_label: family.family_label.clone(),
                });
            }
        }

        Ok(())
    }

    /// Returns a stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_tassadar_trace_family_set_contract|", self)
    }
}

/// Trace-family-set validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarTraceFamilySetError {
    /// Unsupported ABI version.
    #[error("unsupported Tassadar trace family set ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Missing trace-family-set ref.
    #[error("Tassadar trace family set is missing `trace_family_set_ref`")]
    MissingTraceFamilySetRef,
    /// Missing version.
    #[error("Tassadar trace family set is missing `version`")]
    MissingTraceFamilySetVersion,
    /// No families were declared.
    #[error("Tassadar trace family set must contain at least one family contract")]
    MissingFamilies,
    /// Missing family label.
    #[error("Tassadar trace family contract is missing `family_label`")]
    MissingFamilyLabel,
    /// Missing family summary.
    #[error("Tassadar trace family `{family_label}` is missing `summary`")]
    MissingFamilySummary {
        /// Family label.
        family_label: String,
    },
    /// Empty dataset suffix.
    #[error("Tassadar trace family `{family_label}` contains an empty `dataset_suffix`")]
    EmptyDatasetSuffix {
        /// Family label.
        family_label: String,
    },
    /// Family has no workloads.
    #[error("Tassadar trace family `{family_label}` must declare at least one workload")]
    MissingWorkloads {
        /// Family label.
        family_label: String,
    },
    /// One workload ref was missing.
    #[error("Tassadar trace family `{family_label}` contains an empty `workload_ref`")]
    MissingWorkloadRef {
        /// Family label.
        family_label: String,
    },
    /// One workload claim boundary was missing.
    #[error(
        "Tassadar trace family `{family_label}` is missing `claim_boundary` for workload `{workload_ref}`"
    )]
    MissingWorkloadClaimBoundary {
        /// Family label.
        family_label: String,
        /// Workload reference.
        workload_ref: String,
    },
    /// One family label repeated.
    #[error("Tassadar trace family set repeated family `{family_label}`")]
    DuplicateFamilyLabel {
        /// Repeated family label.
        family_label: String,
    },
    /// One workload ref repeated inside a family.
    #[error("Tassadar trace family `{family_label}` repeated workload `{workload_ref}`")]
    DuplicateWorkloadRef {
        /// Family label.
        family_label: String,
        /// Repeated workload reference.
        workload_ref: String,
    },
}

/// Split identity used by the canonical Sudoku-v0 token-sequence dataset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSequenceSplit {
    /// Main training split.
    Train,
    /// Held-out validation split.
    Validation,
    /// Final test split.
    Test,
}

impl TassadarSequenceSplit {
    /// Returns the stable split name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Validation => "validation",
            Self::Test => "test",
        }
    }

    fn dataset_kind(self) -> DatasetSplitKind {
        match self {
            Self::Train => DatasetSplitKind::Train,
            Self::Validation => DatasetSplitKind::Validation,
            Self::Test => DatasetSplitKind::Test,
        }
    }
}

/// Per-example lineage and curriculum metadata carried alongside one tokenized sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSequenceExampleMetadata {
    /// Stable corpus case identifier.
    pub case_id: String,
    /// Stable puzzle identity derived from the raw puzzle cells.
    pub puzzle_digest: String,
    /// Stable validated-program identifier.
    pub program_id: String,
    /// Stable validated-program digest.
    pub program_digest: String,
    /// Stable program-artifact digest for CPU-reference truth.
    pub program_artifact_digest: String,
    /// Stable append-only trace digest.
    pub trace_digest: String,
    /// Stable behavior digest over the full execution.
    pub behavior_digest: String,
    /// Stable split assignment.
    pub split: TassadarSequenceSplit,
    /// Number of given Sudoku clues in the source puzzle.
    pub given_count: u32,
    /// Tokens in the program prompt prefix.
    pub prompt_token_count: u32,
    /// Tokens in the predicted trace suffix.
    pub target_token_count: u32,
    /// Total tokens in the full sequence.
    pub total_token_count: u32,
    /// Exact CPU-reference trace step count.
    pub trace_step_count: u32,
    /// Count of taken backward branches in the reference trace.
    pub backward_branch_count: u32,
    /// Maximum stack depth observed across the reference trace.
    pub max_stack_depth: u32,
}

/// One fully tokenized program-plus-trace example.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSequenceExample {
    /// Stable sequence identifier.
    pub sequence_id: String,
    /// Stable ordered token ids in little-endian `u32` space.
    pub token_ids: Vec<u32>,
    /// Typed lineage and curriculum metadata.
    pub metadata: TassadarSequenceExampleMetadata,
}

impl TassadarSequenceExample {
    fn validate(&self) -> Result<(), TassadarSequenceDatasetError> {
        if self.sequence_id.trim().is_empty() {
            return Err(TassadarSequenceDatasetError::MissingSequenceId);
        }
        if self.token_ids.is_empty() {
            return Err(TassadarSequenceDatasetError::SequenceHasNoTokens {
                sequence_id: self.sequence_id.clone(),
            });
        }
        if self.metadata.total_token_count != self.token_ids.len() as u32 {
            return Err(TassadarSequenceDatasetError::TokenCountMismatch {
                sequence_id: self.sequence_id.clone(),
                declared: self.metadata.total_token_count,
                actual: self.token_ids.len() as u32,
            });
        }
        if self.metadata.prompt_token_count + self.metadata.target_token_count
            != self.metadata.total_token_count
        {
            return Err(TassadarSequenceDatasetError::PromptTargetBoundaryMismatch {
                sequence_id: self.sequence_id.clone(),
                prompt_tokens: self.metadata.prompt_token_count,
                target_tokens: self.metadata.target_token_count,
                total_tokens: self.metadata.total_token_count,
            });
        }
        if self.metadata.case_id.trim().is_empty() {
            return Err(TassadarSequenceDatasetError::MissingCaseId {
                sequence_id: self.sequence_id.clone(),
            });
        }
        if self.metadata.program_digest.trim().is_empty()
            || self.metadata.trace_digest.trim().is_empty()
            || self.metadata.behavior_digest.trim().is_empty()
            || self.metadata.program_artifact_digest.trim().is_empty()
        {
            return Err(TassadarSequenceDatasetError::MissingLineageDigest {
                sequence_id: self.sequence_id.clone(),
            });
        }
        Ok(())
    }
}

/// Full versioned dataset contract for Tassadar token sequences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSequenceDatasetContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Canonical dataset manifest.
    pub manifest: DatasetManifest,
    /// Stable digest over the vocabulary/tokenizer contract used to produce the token ids.
    pub vocabulary_digest: String,
    /// All tokenized examples across splits.
    pub examples: Vec<TassadarSequenceExample>,
}

impl TassadarSequenceDatasetContract {
    /// Builds the canonical dataset manifest and split shards from tokenized examples.
    pub fn from_examples(
        key: DatasetKey,
        display_name: impl Into<String>,
        tokenizer: TokenizerDigest,
        vocabulary_digest: impl Into<String>,
        examples: Vec<TassadarSequenceExample>,
    ) -> Result<Self, TassadarSequenceDatasetError> {
        let vocabulary_digest = vocabulary_digest.into();
        if vocabulary_digest.trim().is_empty() {
            return Err(TassadarSequenceDatasetError::MissingVocabularyDigest);
        }
        if examples.is_empty() {
            return Err(TassadarSequenceDatasetError::DatasetHasNoExamples);
        }

        let mut split_examples =
            BTreeMap::<TassadarSequenceSplit, Vec<TassadarSequenceExample>>::new();
        for example in examples.iter().cloned() {
            split_examples
                .entry(example.metadata.split)
                .or_default()
                .push(example);
        }

        let max_tokens = examples
            .iter()
            .map(|example| example.token_ids.len() as u32)
            .max()
            .unwrap_or(1);

        let mut split_declarations = Vec::new();
        let mut manifest = DatasetManifest::new(
            key.clone(),
            display_name,
            DatasetRecordEncoding::TokenIdsLeU32,
            tokenizer,
        )
        .with_context_window_tokens(max_tokens.max(1));

        for split in [
            TassadarSequenceSplit::Train,
            TassadarSequenceSplit::Validation,
            TassadarSequenceSplit::Test,
        ] {
            let Some(split_examples) = split_examples.get(&split) else {
                continue;
            };
            let split_name = split.as_str();
            let shard_key = format!("{split_name}-000");
            let payload = serialize_split_payload(split_examples.as_slice());
            let datastream_manifest = DatastreamManifest::from_bytes(
                format!("dataset://{}/{}", key.storage_key(), split_name),
                DatastreamSubjectKind::TokenizedCorpus,
                payload.as_slice(),
                payload.len().max(1),
                DatastreamEncoding::TokenIdsLeU32,
            )
            .with_dataset_binding(key.datastream_binding(split_name, shard_key.clone()))
            .with_provenance_digest(stable_digest(
                b"psionic_tassadar_sequence_split_payload|",
                &split_examples
                    .iter()
                    .map(|example| example.sequence_id.as_str())
                    .collect::<Vec<_>>(),
            ));
            let token_count = split_examples
                .iter()
                .map(|example| example.token_ids.len() as u64)
                .sum::<u64>();
            let min_tokens = split_examples
                .iter()
                .map(|example| example.token_ids.len() as u32)
                .min()
                .unwrap_or(1);
            let max_tokens = split_examples
                .iter()
                .map(|example| example.token_ids.len() as u32)
                .max()
                .unwrap_or(1);
            let shard = DatasetShardManifest::new(
                &key,
                split_name,
                shard_key,
                datastream_manifest.manifest_ref(),
                split_examples.len() as u64,
                token_count,
                min_tokens,
                max_tokens,
            )?;
            let declaration =
                DatasetSplitDeclaration::new(&key, split_name, split.dataset_kind(), vec![shard])?;
            split_declarations.push(declaration);

            manifest.metadata.insert(
                format!("tassadar.{}.sequence_ids", split_name),
                json!(split_examples
                    .iter()
                    .map(|example| example.sequence_id.clone())
                    .collect::<Vec<_>>()),
            );
        }

        manifest = manifest.with_splits(split_declarations);
        manifest.metadata.insert(
            String::from("tassadar.sequence_dataset_abi_version"),
            Value::String(String::from(TASSADAR_SEQUENCE_DATASET_ABI_VERSION)),
        );
        manifest.metadata.insert(
            String::from("tassadar.vocabulary_digest"),
            Value::String(vocabulary_digest.clone()),
        );
        manifest.metadata.insert(
            String::from("tassadar.example_count"),
            json!(examples.len()),
        );

        let contract = Self {
            abi_version: String::from(TASSADAR_SEQUENCE_DATASET_ABI_VERSION),
            manifest,
            vocabulary_digest,
            examples,
        };
        contract.validate()?;
        Ok(contract)
    }

    /// Returns the stable dataset storage key.
    #[must_use]
    pub fn storage_key(&self) -> String {
        self.manifest.storage_key()
    }

    /// Returns all examples for one split in stable dataset order.
    #[must_use]
    pub fn split_examples(&self, split: TassadarSequenceSplit) -> Vec<&TassadarSequenceExample> {
        self.examples
            .iter()
            .filter(|example| example.metadata.split == split)
            .collect()
    }

    /// Returns sequence descriptors for one split that can be fed into generic packing contracts.
    pub fn sequence_descriptors(
        &self,
        split: TassadarSequenceSplit,
    ) -> Vec<DatasetSequenceDescriptor> {
        let shard_key = format!("{}-000", split.as_str());
        self.split_examples(split)
            .into_iter()
            .enumerate()
            .map(|(index, example)| {
                DatasetSequenceDescriptor::new(
                    example.sequence_id.clone(),
                    shard_key.clone(),
                    index as u64,
                    example.token_ids.len() as u32,
                )
            })
            .collect()
    }

    /// Builds a generic packing plan for one split.
    pub fn packing_plan(
        &self,
        split: TassadarSequenceSplit,
        policy: &DatasetPackingPolicy,
    ) -> Result<DatasetPackingPlan, TassadarSequenceDatasetError> {
        let descriptors = self.sequence_descriptors(split);
        if descriptors.is_empty() {
            return Err(TassadarSequenceDatasetError::UnknownSplit {
                split_name: split.as_str().to_string(),
            });
        }
        Ok(policy.plan(descriptors.as_slice())?)
    }

    /// Validates the dataset contract and manifest coherence.
    pub fn validate(&self) -> Result<(), TassadarSequenceDatasetError> {
        if self.abi_version != TASSADAR_SEQUENCE_DATASET_ABI_VERSION {
            return Err(TassadarSequenceDatasetError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.vocabulary_digest.trim().is_empty() {
            return Err(TassadarSequenceDatasetError::MissingVocabularyDigest);
        }
        self.manifest.validate()?;
        if self.examples.is_empty() {
            return Err(TassadarSequenceDatasetError::DatasetHasNoExamples);
        }

        let mut sequence_ids = BTreeSet::new();
        let mut expected_split_counts = BTreeMap::new();
        let mut expected_split_tokens = BTreeMap::new();
        for example in &self.examples {
            example.validate()?;
            if !sequence_ids.insert(example.sequence_id.clone()) {
                return Err(TassadarSequenceDatasetError::DuplicateSequenceId {
                    sequence_id: example.sequence_id.clone(),
                });
            }
            *expected_split_counts
                .entry(example.metadata.split.as_str().to_string())
                .or_insert(0_u64) += 1;
            *expected_split_tokens
                .entry(example.metadata.split.as_str().to_string())
                .or_insert(0_u64) += example.token_ids.len() as u64;
        }

        for split in &self.manifest.splits {
            let expected_count = expected_split_counts
                .get(split.split_name.as_str())
                .copied()
                .unwrap_or(0);
            let expected_tokens = expected_split_tokens
                .get(split.split_name.as_str())
                .copied()
                .unwrap_or(0);
            if split.sequence_count != expected_count {
                return Err(TassadarSequenceDatasetError::SplitSequenceCountMismatch {
                    split_name: split.split_name.clone(),
                    declared: split.sequence_count,
                    actual: expected_count,
                });
            }
            if split.token_count != expected_tokens {
                return Err(TassadarSequenceDatasetError::SplitTokenCountMismatch {
                    split_name: split.split_name.clone(),
                    declared: split.token_count,
                    actual: expected_tokens,
                });
            }
        }

        Ok(())
    }

    /// Returns a stable digest over the dataset contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_tassadar_sequence_dataset_contract|", self)
    }
}

/// Tassadar sequence dataset validation or packing failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarSequenceDatasetError {
    /// Unsupported ABI version.
    #[error("unsupported Tassadar sequence dataset ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Missing vocabulary digest.
    #[error("Tassadar sequence dataset is missing `vocabulary_digest`")]
    MissingVocabularyDigest,
    /// Missing sequence identifier.
    #[error("Tassadar sequence example is missing `sequence_id`")]
    MissingSequenceId,
    /// Empty dataset.
    #[error("Tassadar sequence dataset must contain at least one example")]
    DatasetHasNoExamples,
    /// Empty token sequence.
    #[error("Tassadar sequence `{sequence_id}` has no tokens")]
    SequenceHasNoTokens {
        /// Sequence identifier.
        sequence_id: String,
    },
    /// Duplicate sequence identifier.
    #[error("Tassadar sequence dataset repeated `sequence_id` `{sequence_id}`")]
    DuplicateSequenceId {
        /// Repeated sequence identifier.
        sequence_id: String,
    },
    /// Missing case identifier in metadata.
    #[error("Tassadar sequence `{sequence_id}` is missing `metadata.case_id`")]
    MissingCaseId {
        /// Sequence identifier.
        sequence_id: String,
    },
    /// One lineage digest was missing.
    #[error("Tassadar sequence `{sequence_id}` is missing one or more lineage digests")]
    MissingLineageDigest {
        /// Sequence identifier.
        sequence_id: String,
    },
    /// Total token count drifted from the visible token ids.
    #[error(
        "Tassadar sequence `{sequence_id}` declared total_token_count={declared} but carried {actual} tokens"
    )]
    TokenCountMismatch {
        /// Sequence identifier.
        sequence_id: String,
        /// Declared token count.
        declared: u32,
        /// Actual token count.
        actual: u32,
    },
    /// Prompt/target boundaries drifted from the total length.
    #[error(
        "Tassadar sequence `{sequence_id}` prompt/target boundary mismatch: prompt={prompt_tokens}, target={target_tokens}, total={total_tokens}"
    )]
    PromptTargetBoundaryMismatch {
        /// Sequence identifier.
        sequence_id: String,
        /// Prompt token count.
        prompt_tokens: u32,
        /// Target token count.
        target_tokens: u32,
        /// Total token count.
        total_tokens: u32,
    },
    /// Split requested for packing was not present.
    #[error("unknown Tassadar sequence split `{split_name}`")]
    UnknownSplit {
        /// Requested split name.
        split_name: String,
    },
    /// Split count drifted from the manifest.
    #[error(
        "Tassadar sequence split `{split_name}` declared sequence_count={declared} but examples derive {actual}"
    )]
    SplitSequenceCountMismatch {
        /// Split name.
        split_name: String,
        /// Manifest-declared count.
        declared: u64,
        /// Example-derived count.
        actual: u64,
    },
    /// Split token count drifted from the manifest.
    #[error(
        "Tassadar sequence split `{split_name}` declared token_count={declared} but examples derive {actual}"
    )]
    SplitTokenCountMismatch {
        /// Split name.
        split_name: String,
        /// Manifest-declared count.
        declared: u64,
        /// Example-derived count.
        actual: u64,
    },
    /// Generic dataset contract failure.
    #[error(transparent)]
    Dataset(#[from] DatasetContractError),
}

fn serialize_split_payload(examples: &[TassadarSequenceExample]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for example in examples {
        bytes.extend_from_slice(&(example.token_ids.len() as u32).to_le_bytes());
        for token in &example.token_ids {
            bytes.extend_from_slice(&token.to_le_bytes());
        }
    }
    bytes
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar sequence dataset value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{
        tassadar_numeric_opcode_ladder_contract, tassadar_verifier_guided_search_trace_family_contract,
        tassadar_structured_numeric_encoding_lane_contract,
        TassadarBenchmarkAxis, TassadarBenchmarkFamily, TassadarBenchmarkFamilyContract,
        TassadarBenchmarkPackageBinding, TassadarBenchmarkPackageSetContract,
        TassadarClrsAlgorithmFamily, TassadarClrsLengthBucket, TassadarClrsTrajectoryFamily,
        TassadarClrsWasmBridgeCaseContract, TassadarClrsWasmBridgeContract,
        TassadarClrsWasmBridgeExportContract, TassadarModuleScaleWorkloadCaseContract,
        TassadarModuleScaleWorkloadFamily, TassadarModuleScaleWorkloadStatus,
        TassadarModuleScaleWorkloadSuiteContract, TassadarNumericOpcodeFamily,
        TassadarNumericOpcodeFamilyStatus, TassadarNumericOpcodeLadderContract,
        TassadarSequenceDatasetContract, TassadarSequenceExample,
        TassadarSequenceExampleMetadata, TassadarSequenceSplit,
        TassadarStructuredNumericEncodingLaneContract,
        TassadarStructuredNumericEncodingWorkloadFamily, TassadarTraceFamilyAuthorityScope,
        TassadarTraceFamilyContract, TassadarTraceFamilySetContract,
        TassadarTraceFamilyTopology, TassadarTraceFamilyWorkloadBinding,
        TASSADAR_BENCHMARK_PACKAGE_SET_ABI_VERSION,
        TASSADAR_CLRS_WASM_BRIDGE_ABI_VERSION, TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_ABI_VERSION,
        TASSADAR_NUMERIC_OPCODE_LADDER_ABI_VERSION, TASSADAR_SEQUENCE_DATASET_ABI_VERSION,
        TASSADAR_STRUCTURED_NUMERIC_ENCODING_LANE_ABI_VERSION,
        TASSADAR_TRACE_FAMILY_SET_ABI_VERSION,
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_ABI_VERSION,
    };
    use crate::{DatasetKey, TokenizerDigest, TokenizerFamily};

    fn sample_example(split: TassadarSequenceSplit, suffix: &str) -> TassadarSequenceExample {
        TassadarSequenceExample {
            sequence_id: format!("seq-{suffix}"),
            token_ids: vec![1, 2, 3, 4],
            metadata: TassadarSequenceExampleMetadata {
                case_id: format!("case-{suffix}"),
                puzzle_digest: format!("puzzle-{suffix}"),
                program_id: format!("program-{suffix}"),
                program_digest: format!("program-digest-{suffix}"),
                program_artifact_digest: format!("artifact-digest-{suffix}"),
                trace_digest: format!("trace-digest-{suffix}"),
                behavior_digest: format!("behavior-digest-{suffix}"),
                split,
                given_count: 4,
                prompt_token_count: 2,
                target_token_count: 2,
                total_token_count: 4,
                trace_step_count: 1,
                backward_branch_count: 0,
                max_stack_depth: 1,
            },
        }
    }

    #[test]
    fn sequence_dataset_builds_manifest_and_split_contracts() {
        let dataset = TassadarSequenceDatasetContract::from_examples(
            DatasetKey::new("oa.tassadar.sudoku_v0.sequence", "train-v0"),
            "Tassadar Sudoku-v0 Sequence Dataset",
            TokenizerDigest::new(TokenizerFamily::Custom, "tokenizer-digest", 320),
            "vocab-digest",
            vec![
                sample_example(TassadarSequenceSplit::Train, "a"),
                sample_example(TassadarSequenceSplit::Validation, "b"),
                sample_example(TassadarSequenceSplit::Test, "c"),
            ],
        )
        .expect("dataset should build");

        assert_eq!(dataset.abi_version, TASSADAR_SEQUENCE_DATASET_ABI_VERSION);
        assert_eq!(
            dataset.manifest.record_encoding,
            crate::DatasetRecordEncoding::TokenIdsLeU32
        );
        assert_eq!(dataset.manifest.splits.len(), 3);
        assert_eq!(
            dataset.split_examples(TassadarSequenceSplit::Train).len(),
            1
        );
        assert_eq!(
            dataset
                .manifest
                .metadata
                .get("tassadar.vocabulary_digest")
                .expect("vocabulary digest metadata"),
            &Value::String(String::from("vocab-digest"))
        );
        assert!(!dataset.stable_digest().is_empty());
    }

    #[test]
    fn benchmark_package_set_contract_is_machine_legible() {
        let contract = TassadarBenchmarkPackageSetContract::new(
            "benchmark-set://openagents/tassadar/public",
            "2026.03.17",
            vec![
                TassadarBenchmarkFamilyContract {
                    family: TassadarBenchmarkFamily::Arithmetic,
                    summary: String::from("exact arithmetic kernels and validation microprograms"),
                    benchmark_packages: vec![
                        TassadarBenchmarkPackageBinding {
                            benchmark_ref: String::from(
                                "benchmark://openagents/tassadar/reference_fixture/validation_corpus",
                            ),
                            version: String::from("2026.03.17"),
                            environment_ref: String::from("env.openagents.tassadar.benchmark"),
                            dataset: DatasetKey::new(
                                "dataset://openagents/tassadar/validation_corpus",
                                "2026.03.17",
                            ),
                            split: Some(String::from("benchmark")),
                        },
                        TassadarBenchmarkPackageBinding {
                            benchmark_ref: String::from(
                                "benchmark://openagents/tassadar/compiled_kernel_suite/reference_fixture",
                            ),
                            version: String::from("v0"),
                            environment_ref: String::from(
                                "env.openagents.tassadar.compiled_kernel_suite.benchmark",
                            ),
                            dataset: DatasetKey::new(
                                "dataset://openagents/tassadar/compiled_kernel_suite",
                                "v0",
                            ),
                            split: Some(String::from("benchmark")),
                        },
                    ],
                    axis_coverage: vec![
                        TassadarBenchmarkAxis::Exactness,
                        TassadarBenchmarkAxis::LengthGeneralization,
                        TassadarBenchmarkAxis::PlannerUsefulness,
                    ],
                    case_ids: vec![String::from("locals_add")],
                },
                TassadarBenchmarkFamilyContract {
                    family: TassadarBenchmarkFamily::ClrsSubset,
                    summary: String::from("bounded CLRS-adjacent shortest-path witness"),
                    benchmark_packages: vec![TassadarBenchmarkPackageBinding {
                        benchmark_ref: String::from(
                            "benchmark://openagents/tassadar/reference_fixture/validation_corpus",
                        ),
                        version: String::from("2026.03.17"),
                        environment_ref: String::from("env.openagents.tassadar.benchmark"),
                        dataset: DatasetKey::new(
                            "dataset://openagents/tassadar/validation_corpus",
                            "2026.03.17",
                        ),
                        split: Some(String::from("benchmark")),
                    }],
                    axis_coverage: vec![
                        TassadarBenchmarkAxis::Exactness,
                        TassadarBenchmarkAxis::LengthGeneralization,
                        TassadarBenchmarkAxis::PlannerUsefulness,
                    ],
                    case_ids: vec![String::from("shortest_path_two_route")],
                },
            ],
        )
        .expect("package set contract should build");

        assert_eq!(
            contract.abi_version,
            TASSADAR_BENCHMARK_PACKAGE_SET_ABI_VERSION
        );
        assert_eq!(contract.families.len(), 2);
        assert!(!contract.stable_digest().is_empty());
    }

    #[test]
    fn clrs_wasm_bridge_contract_is_machine_legible() {
        let contract = TassadarClrsWasmBridgeContract::new(
            "benchmark-bridge://openagents/tassadar/clrs_wasm",
            "2026.03.18",
            "benchmark://openagents/tassadar/clrs_wasm_bridge/reference_fixture",
            "env.openagents.tassadar.clrs_wasm_bridge.benchmark",
            vec![TassadarClrsAlgorithmFamily::ShortestPath],
            vec![
                TassadarClrsTrajectoryFamily::SequentialRelaxation,
                TassadarClrsTrajectoryFamily::WavefrontRelaxation,
            ],
            vec![
                TassadarClrsLengthBucket::Tiny,
                TassadarClrsLengthBucket::Small,
            ],
            vec![
                String::from("exactness_bps"),
                String::from("module_trace_steps"),
                String::from("cpu_reference_cost_units"),
                String::from("trajectory_step_delta"),
            ],
            vec![
                TassadarClrsWasmBridgeCaseContract {
                    case_id: String::from("clrs_shortest_path_sequential"),
                    algorithm_family: TassadarClrsAlgorithmFamily::ShortestPath,
                    trajectory_family: TassadarClrsTrajectoryFamily::SequentialRelaxation,
                    summary: String::from(
                        "fixed shortest-path witness compiled as sequential relaxation",
                    ),
                    source_ref: String::from(
                        "fixtures/tassadar/sources/tassadar_clrs_shortest_path_sequential.wat",
                    ),
                    export_bindings: vec![
                        TassadarClrsWasmBridgeExportContract {
                            length_bucket: TassadarClrsLengthBucket::Tiny,
                            export_name: String::from("distance_tiny"),
                        },
                        TassadarClrsWasmBridgeExportContract {
                            length_bucket: TassadarClrsLengthBucket::Small,
                            export_name: String::from("distance_small"),
                        },
                    ],
                },
                TassadarClrsWasmBridgeCaseContract {
                    case_id: String::from("clrs_shortest_path_wavefront"),
                    algorithm_family: TassadarClrsAlgorithmFamily::ShortestPath,
                    trajectory_family: TassadarClrsTrajectoryFamily::WavefrontRelaxation,
                    summary: String::from(
                        "fixed shortest-path witness compiled as wavefront relaxation",
                    ),
                    source_ref: String::from(
                        "fixtures/tassadar/sources/tassadar_clrs_shortest_path_wavefront.wat",
                    ),
                    export_bindings: vec![
                        TassadarClrsWasmBridgeExportContract {
                            length_bucket: TassadarClrsLengthBucket::Tiny,
                            export_name: String::from("distance_tiny"),
                        },
                        TassadarClrsWasmBridgeExportContract {
                            length_bucket: TassadarClrsLengthBucket::Small,
                            export_name: String::from("distance_small"),
                        },
                    ],
                },
            ],
            "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
        )
        .expect("bridge contract should build");

        assert_eq!(contract.abi_version, TASSADAR_CLRS_WASM_BRIDGE_ABI_VERSION);
        assert_eq!(contract.cases.len(), 2);
        assert!(!contract.stable_digest().is_empty());
    }

    #[test]
    fn verifier_guided_search_trace_family_contract_is_machine_legible() {
        let contract = tassadar_verifier_guided_search_trace_family_contract();
        assert_eq!(
            contract.abi_version,
            TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_ABI_VERSION
        );
        assert_eq!(contract.supported_workload_families.len(), 2);
        assert_eq!(contract.event_kinds.len(), 5);
        assert_eq!(contract.cases.len(), 2);
        assert!(!contract.stable_digest().is_empty());
    }

    #[test]
    fn structured_numeric_encoding_lane_contract_is_machine_legible() {
        let contract = tassadar_structured_numeric_encoding_lane_contract();
        assert_eq!(
            contract.abi_version,
            TASSADAR_STRUCTURED_NUMERIC_ENCODING_LANE_ABI_VERSION
        );
        assert_eq!(contract.workload_families.len(), 3);
        assert_eq!(contract.evaluation_axes.len(), 4);
        assert_eq!(contract.cases.len(), 3);
        assert_eq!(
            contract.cases[0].workload_family,
            TassadarStructuredNumericEncodingWorkloadFamily::ArithmeticImmediates
        );
        assert_eq!(
            contract.cases[1].workload_family,
            TassadarStructuredNumericEncodingWorkloadFamily::AddressOffsetStress
        );
        assert_eq!(
            contract.cases[2].workload_family,
            TassadarStructuredNumericEncodingWorkloadFamily::MemoryAddresses
        );
        let reparsed = TassadarStructuredNumericEncodingLaneContract::new(
            contract.lane_ref.clone(),
            contract.version.clone(),
            contract.workload_families.clone(),
            contract.evaluation_axes.clone(),
            contract.cases.clone(),
            contract.report_ref.clone(),
        )
        .expect("lane contract should rebuild");
        assert_eq!(reparsed, contract);
        assert!(!contract.stable_digest().is_empty());
    }

    #[test]
    fn module_scale_workload_suite_contract_is_machine_legible() {
        let contract = TassadarModuleScaleWorkloadSuiteContract::new(
            "benchmark-suite://openagents/tassadar/module_scale",
            "2026.03.17",
            "benchmark://openagents/tassadar/module_scale/reference_fixture",
            "env.openagents.tassadar.module_scale.benchmark",
            vec![
                TassadarModuleScaleWorkloadFamily::Memcpy,
                TassadarModuleScaleWorkloadFamily::Parsing,
                TassadarModuleScaleWorkloadFamily::Checksum,
                TassadarModuleScaleWorkloadFamily::VmStyle,
            ],
            vec![
                String::from("exactness_bps"),
                String::from("total_trace_steps"),
                String::from("cpu_reference_cost_units"),
                String::from("refusal_kind"),
            ],
            vec![
                TassadarModuleScaleWorkloadCaseContract {
                    case_id: String::from("memcpy_fixed_span_exact"),
                    family: TassadarModuleScaleWorkloadFamily::Memcpy,
                    summary: String::from("fixed-span copy kernel lowered exactly"),
                    source_ref: String::from(
                        "fixtures/tassadar/sources/tassadar_module_memcpy_suite.wat",
                    ),
                    expected_status: TassadarModuleScaleWorkloadStatus::LoweredExact,
                },
                TassadarModuleScaleWorkloadCaseContract {
                    case_id: String::from("vm_style_param_refusal"),
                    family: TassadarModuleScaleWorkloadFamily::VmStyle,
                    summary: String::from("vm-style param dispatch still refuses explicitly"),
                    source_ref: String::from(
                        "fixtures/tassadar/sources/tassadar_module_vm_style_param_refusal.wat",
                    ),
                    expected_status: TassadarModuleScaleWorkloadStatus::LoweringRefused,
                },
            ],
            "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
        )
        .expect("suite contract should build");

        assert_eq!(
            contract.abi_version,
            TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_ABI_VERSION
        );
        assert_eq!(contract.supported_families.len(), 4);
        assert_eq!(contract.cases.len(), 2);
        assert!(!contract.stable_digest().is_empty());
    }

    #[test]
    fn numeric_opcode_ladder_contract_is_machine_legible() {
        let contract = tassadar_numeric_opcode_ladder_contract();
        assert_eq!(
            contract.abi_version,
            TASSADAR_NUMERIC_OPCODE_LADDER_ABI_VERSION
        );
        assert_eq!(contract.families.len(), 5);
        assert_eq!(
            contract.families[1].family,
            TassadarNumericOpcodeFamily::I32Comparisons
        );
        assert_eq!(
            contract.families[3].status,
            TassadarNumericOpcodeFamilyStatus::Refused
        );
        assert!(!contract.stable_digest().is_empty());
    }

    #[test]
    fn numeric_opcode_ladder_contract_rejects_duplicate_family() {
        let error = TassadarNumericOpcodeLadderContract::new(
            "tassadar://numeric_opcode_ladder/test",
            "v1",
            vec![
                tassadar_numeric_opcode_ladder_contract().families[0].clone(),
                tassadar_numeric_opcode_ladder_contract().families[0].clone(),
            ],
        )
        .expect_err("duplicate family should fail");
        assert!(format!("{error}").contains("repeated family"));
    }

    #[test]
    fn trace_family_set_contract_is_machine_legible() {
        let contract = TassadarTraceFamilySetContract::new(
            "trace-family-set://openagents/tassadar/sequence_variants",
            "trace-family-v1",
            vec![
                TassadarTraceFamilyContract {
                    family_label: String::from("sequential_cpu_reference"),
                    topology: TassadarTraceFamilyTopology::SequentialCpuReference,
                    summary: String::from(
                        "canonical CPU-style append-only authority trace for seeded Tassadar sequence workloads",
                    ),
                    dataset_suffix: None,
                    authority_scope: TassadarTraceFamilyAuthorityScope::FullCpuTrace,
                    workloads: vec![
                        TassadarTraceFamilyWorkloadBinding {
                            workload_ref: String::from("oa.tassadar.sudoku_v0.sequence"),
                            claim_boundary: String::from("learned_bounded"),
                        },
                        TassadarTraceFamilyWorkloadBinding {
                            workload_ref: String::from("oa.tassadar.sudoku_9x9.sequence"),
                            claim_boundary: String::from("learned_bounded"),
                        },
                    ],
                },
                TassadarTraceFamilyContract {
                    family_label: String::from("sudoku_diagonal_wavefront"),
                    topology: TassadarTraceFamilyTopology::ParallelWavefront,
                    summary: String::from(
                        "research-only anti-diagonal Sudoku wavefront target family",
                    ),
                    dataset_suffix: Some(String::from("sudoku_diagonal_wavefront")),
                    authority_scope: TassadarTraceFamilyAuthorityScope::FinalOutputsOnly,
                    workloads: vec![
                        TassadarTraceFamilyWorkloadBinding {
                            workload_ref: String::from("oa.tassadar.sudoku_v0.sequence"),
                            claim_boundary: String::from("research_only"),
                        },
                        TassadarTraceFamilyWorkloadBinding {
                            workload_ref: String::from("oa.tassadar.sudoku_9x9.sequence"),
                            claim_boundary: String::from("research_only"),
                        },
                    ],
                },
            ],
        )
        .expect("trace family set should build");

        assert_eq!(contract.abi_version, TASSADAR_TRACE_FAMILY_SET_ABI_VERSION);
        assert_eq!(contract.families.len(), 2);
        assert!(!contract.stable_digest().is_empty());
    }
}
