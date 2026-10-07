use std::collections::BTreeSet;

use psionic_runtime::TassadarWasmProfileId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF;

pub const TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_compute_profile_ladder_report.json";

const TASSADAR_RUST_SOURCE_CANON_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_source_canon_report.json";
const TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_article_profile_completeness_report.json";
const TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_abi_closure_report.json";
const TASSADAR_GENERALIZED_ABI_FAMILY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_generalized_abi_family_report.json";
const TASSADAR_EFFECT_TAXONOMY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_effect_taxonomy_report.json";
const TASSADAR_RESUMABLE_MULTI_SLICE_PROMOTION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_resumable_multi_slice_promotion_report.json";
const TASSADAR_EFFECT_SAFE_RESUME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_effect_safe_resume_report.json";
const TASSADAR_LINKED_PROGRAM_BUNDLE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_linked_program_bundle_runtime_report.json";
const TASSADAR_LINKED_PROGRAM_BUNDLE_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_linked_program_bundle_eval_report.json";
const TASSADAR_LINKED_PROGRAM_BUNDLE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_linked_program_bundle_summary.json";
const TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_hungarian_10x10_article_reproducer_report.json";
const TASSADAR_SUDOKU_9X9_ARTICLE_REPRODUCER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_sudoku_9x9_article_reproducer_report.json";
const TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_runtime_closeout_report.json";
const TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_direct_model_weight_execution_proof_report.json";
const TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_cpu_reproducibility_report.json";
const TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_only_article_acceptance_gate_v2.json";
const TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_only_article_closeout_audit_report.json";
const TASSADAR_BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_route_policy_report.json";

/// Stable named post-article internal-compute profile identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComputeProfileId {
    ArticleCloseoutV1,
    GeneralizedAbiV1,
    WiderNumericDataLayoutV1,
    RuntimeSupportSubsetV1,
    DeterministicImportSubsetV1,
    ResumableMultiSliceV1,
    PortableBroadFamilyV1,
    PublicBroadFamilyV1,
}

impl TassadarInternalComputeProfileId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArticleCloseoutV1 => "tassadar.internal_compute.article_closeout.v1",
            Self::GeneralizedAbiV1 => "tassadar.internal_compute.generalized_abi.v1",
            Self::WiderNumericDataLayoutV1 => {
                "tassadar.internal_compute.wider_numeric_data_layout.v1"
            }
            Self::RuntimeSupportSubsetV1 => "tassadar.internal_compute.runtime_support_subset.v1",
            Self::DeterministicImportSubsetV1 => {
                "tassadar.internal_compute.deterministic_import_subset.v1"
            }
            Self::ResumableMultiSliceV1 => "tassadar.internal_compute.resumable_multi_slice.v1",
            Self::PortableBroadFamilyV1 => "tassadar.internal_compute.portable_broad_family.v1",
            Self::PublicBroadFamilyV1 => "tassadar.internal_compute.public_broad_family.v1",
        }
    }
}

/// Publication status for one named profile in the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComputeProfileStatus {
    Implemented,
    Planned,
}

/// Exactness posture declared for one named profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComputeExactnessPosture {
    ExactTraceAndOutput,
    ExactRouteBounded,
    Planned,
}

/// Import posture declared for one named profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComputeImportPosture {
    NoImportsOnly,
    DeterministicStubImportsOnly,
    Planned,
}

/// Portability posture declared for one named profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComputePortabilityPosture {
    DeclaredCpuMatrix,
    Planned,
}

/// Typed refusal class that a named profile must preserve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComputeRefusalClass {
    ArbitraryWasmUnsupported,
    BroadHostImportUnsupported,
    BroadAbiUnsupported,
    WiderNumericDataLayoutUnsupported,
    RuntimeSupportSubsetUnsupported,
    ResumableMultiSliceUnsupported,
    UnsupportedMachineClass,
    NonCpuBackendUnsupported,
}

/// One named internal-compute profile in the public ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputeProfileSpec {
    pub profile_id: String,
    pub status: TassadarInternalComputeProfileStatus,
    pub admitted_wasm_profile_ids: Vec<String>,
    pub admitted_opcode_family_ids: Vec<String>,
    pub admitted_abi_shape_ids: Vec<String>,
    pub admitted_numeric_family_ids: Vec<String>,
    pub admitted_memory_semantic_ids: Vec<String>,
    pub admitted_runtime_support_ids: Vec<String>,
    pub import_posture: TassadarInternalComputeImportPosture,
    pub exactness_posture: TassadarInternalComputeExactnessPosture,
    pub portability_posture: TassadarInternalComputePortabilityPosture,
    pub supported_machine_class_ids: Vec<String>,
    pub refusal_classes: Vec<TassadarInternalComputeRefusalClass>,
    pub required_evidence_refs: Vec<String>,
    pub detail: String,
}

impl TassadarInternalComputeProfileSpec {
    fn new(
        profile_id: TassadarInternalComputeProfileId,
        status: TassadarInternalComputeProfileStatus,
        mut admitted_wasm_profile_ids: Vec<String>,
        mut admitted_opcode_family_ids: Vec<String>,
        mut admitted_abi_shape_ids: Vec<String>,
        mut admitted_numeric_family_ids: Vec<String>,
        mut admitted_memory_semantic_ids: Vec<String>,
        mut admitted_runtime_support_ids: Vec<String>,
        import_posture: TassadarInternalComputeImportPosture,
        exactness_posture: TassadarInternalComputeExactnessPosture,
        portability_posture: TassadarInternalComputePortabilityPosture,
        mut supported_machine_class_ids: Vec<String>,
        mut refusal_classes: Vec<TassadarInternalComputeRefusalClass>,
        mut required_evidence_refs: Vec<String>,
        detail: impl Into<String>,
    ) -> Self {
        admitted_wasm_profile_ids.sort();
        admitted_wasm_profile_ids.dedup();
        admitted_opcode_family_ids.sort();
        admitted_opcode_family_ids.dedup();
        admitted_abi_shape_ids.sort();
        admitted_abi_shape_ids.dedup();
        admitted_numeric_family_ids.sort();
        admitted_numeric_family_ids.dedup();
        admitted_memory_semantic_ids.sort();
        admitted_memory_semantic_ids.dedup();
        admitted_runtime_support_ids.sort();
        admitted_runtime_support_ids.dedup();
        supported_machine_class_ids.sort();
        supported_machine_class_ids.dedup();
        refusal_classes.sort();
        refusal_classes.dedup();
        required_evidence_refs.sort();
        required_evidence_refs.dedup();
        Self {
            profile_id: String::from(profile_id.as_str()),
            status,
            admitted_wasm_profile_ids,
            admitted_opcode_family_ids,
            admitted_abi_shape_ids,
            admitted_numeric_family_ids,
            admitted_memory_semantic_ids,
            admitted_runtime_support_ids,
            import_posture,
            exactness_posture,
            portability_posture,
            supported_machine_class_ids,
            refusal_classes,
            required_evidence_refs,
            detail: detail.into(),
        }
    }
}

/// Public machine-readable profile ladder for post-article internal-compute claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputeProfileLadderPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub report_ref: String,
    pub profiles: Vec<TassadarInternalComputeProfileSpec>,
    pub claim_boundary: String,
    pub publication_digest: String,
}

impl TassadarInternalComputeProfileLadderPublication {
    fn new() -> Self {
        let current_evidence_refs = current_article_closeout_required_evidence_refs();
        let current_supported_machine_class_ids = vec![
            String::from("host_cpu_aarch64"),
            String::from("host_cpu_x86_64"),
        ];
        let current_refusal_classes = vec![
            TassadarInternalComputeRefusalClass::ArbitraryWasmUnsupported,
            TassadarInternalComputeRefusalClass::BroadHostImportUnsupported,
            TassadarInternalComputeRefusalClass::BroadAbiUnsupported,
            TassadarInternalComputeRefusalClass::UnsupportedMachineClass,
            TassadarInternalComputeRefusalClass::NonCpuBackendUnsupported,
        ];
        let mut profiles = vec![
            TassadarInternalComputeProfileSpec::new(
                TassadarInternalComputeProfileId::ArticleCloseoutV1,
                TassadarInternalComputeProfileStatus::Implemented,
                vec![
                    String::from(TassadarWasmProfileId::ArticleI32ComputeV1.as_str()),
                    String::from(TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str()),
                    String::from(TassadarWasmProfileId::Sudoku9x9SearchV1.as_str()),
                    String::from("tassadar.wasm.article_runtime_closeout.v1"),
                ],
                vec![
                    String::from("core_i32_arithmetic"),
                    String::from("structured_loops_and_bounded_backtracking"),
                    String::from("byte_addressed_linear_memory_v2"),
                    String::from("mutable_i32_globals_funcref_indirect_calls"),
                ],
                vec![
                    String::from("zero_param_exports"),
                    String::from("pointer_length_heap_inputs"),
                    String::from("single_i32_return"),
                ],
                vec![String::from("i32_integer_family")],
                vec![
                    String::from("byte_addressed_linear_memory_v2"),
                    String::from("data_segments_globals_exports"),
                ],
                vec![
                    String::from("route_bound_direct_execution_proof"),
                    String::from("one_command_operator_reproduction"),
                    String::from("declared_cpu_matrix"),
                ],
                TassadarInternalComputeImportPosture::NoImportsOnly,
                TassadarInternalComputeExactnessPosture::ExactRouteBounded,
                TassadarInternalComputePortabilityPosture::DeclaredCpuMatrix,
                current_supported_machine_class_ids.clone(),
                current_refusal_classes.clone(),
                current_evidence_refs,
                "the current honest internal-compute claim is the fully closed Rust-only article path only. It is benchmarked, route-bound, and CPU-matrix-bounded, and it must not be widened into arbitrary Rust/Wasm support",
            ),
            TassadarInternalComputeProfileSpec::new(
                TassadarInternalComputeProfileId::GeneralizedAbiV1,
                TassadarInternalComputeProfileStatus::Implemented,
                vec![
                    String::from(TassadarWasmProfileId::ArticleI32ComputeV1.as_str()),
                    String::from("tassadar.wasm.generalized_abi.v1"),
                ],
                vec![
                    String::from("article_control_and_memory_core"),
                    String::from("multi_export_program_shape"),
                    String::from("caller_owned_output_buffers"),
                ],
                vec![
                    String::from("multi_param_i32_entrypoints"),
                    String::from("multiple_pointer_length_inputs"),
                    String::from("heap_output_return_contracts"),
                    String::from("result_code_plus_output_buffer_shapes"),
                    String::from("bounded_multi_export_program_shapes"),
                ],
                vec![String::from("i32_integer_family")],
                vec![
                    String::from("byte_addressed_linear_memory_v2"),
                    String::from("caller_owned_output_regions"),
                ],
                vec![
                    String::from("panic_abort_loop_only"),
                    String::from("caller_owned_output_buffers"),
                    String::from("local_frame_helpers"),
                ],
                TassadarInternalComputeImportPosture::NoImportsOnly,
                TassadarInternalComputeExactnessPosture::ExactRouteBounded,
                TassadarInternalComputePortabilityPosture::Planned,
                current_supported_machine_class_ids.clone(),
                vec![
                    TassadarInternalComputeRefusalClass::BroadHostImportUnsupported,
                    TassadarInternalComputeRefusalClass::WiderNumericDataLayoutUnsupported,
                    TassadarInternalComputeRefusalClass::NonCpuBackendUnsupported,
                ],
                vec![
                    String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
                    String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF),
                    String::from(TASSADAR_GENERALIZED_ABI_FAMILY_REPORT_REF),
                ],
                "generalized i32-first ABI widening is now benchmarked as a separate implemented profile over multi-param scalar entrypoints, multiple pointer-length inputs, caller-owned output buffers, and bounded multi-export program shapes. It remains bounded and must not be widened into floating-point, host-import, callee-allocation, or arbitrary runtime-support claims",
            ),
            TassadarInternalComputeProfileSpec::new(
                TassadarInternalComputeProfileId::WiderNumericDataLayoutV1,
                TassadarInternalComputeProfileStatus::Implemented,
                vec![
                    String::from(TassadarWasmProfileId::ArticleI32ComputeV1.as_str()),
                    String::from("tassadar.wasm.generalized_abi.v1"),
                ],
                vec![
                    String::from("article_control_and_memory_core"),
                    String::from("wider_numeric_and_data_layout"),
                ],
                vec![
                    String::from("multi_param_i64_entrypoints"),
                    String::from("homogeneous_i32_pair_returns"),
                    String::from("result_code_plus_output_buffer_i64"),
                ],
                vec![
                    String::from("i32_integer_family"),
                    String::from("i64_integer_family"),
                    String::from("wider_integer_layouts"),
                ],
                vec![
                    String::from("byte_addressed_linear_memory_v2"),
                    String::from("wider_numeric_data_layout"),
                ],
                vec![
                    String::from("caller_owned_output_buffers"),
                    String::from("direct_scalar_i64_entrypoints"),
                    String::from("homogeneous_multi_value_returns"),
                    String::from("i64_region_layouts"),
                    String::from("panic_abort_loop_only"),
                ],
                TassadarInternalComputeImportPosture::NoImportsOnly,
                TassadarInternalComputeExactnessPosture::ExactRouteBounded,
                TassadarInternalComputePortabilityPosture::Planned,
                current_supported_machine_class_ids.clone(),
                vec![
                    TassadarInternalComputeRefusalClass::BroadHostImportUnsupported,
                    TassadarInternalComputeRefusalClass::BroadAbiUnsupported,
                    TassadarInternalComputeRefusalClass::RuntimeSupportSubsetUnsupported,
                    TassadarInternalComputeRefusalClass::NonCpuBackendUnsupported,
                ],
                vec![
                    String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
                    String::from(TASSADAR_GENERALIZED_ABI_FAMILY_REPORT_REF),
                ],
                "wider numeric and data-layout claims are now benchmarked as a separate implemented profile over exact i64 scalar entrypoints, homogeneous two-value i32 returns, and 8-byte caller-owned buffer layouts. The profile remains non-portable and non-promoted until its own portability and broader runtime-support evidence exists",
            ),
            TassadarInternalComputeProfileSpec::new(
                TassadarInternalComputeProfileId::RuntimeSupportSubsetV1,
                TassadarInternalComputeProfileStatus::Implemented,
                vec![
                    String::from(TassadarWasmProfileId::ArticleI32ComputeV1.as_str()),
                    String::from("tassadar.wasm.linked_program_bundle.v1"),
                ],
                vec![
                    String::from("article_control_and_memory_core"),
                    String::from("linked_program_runtime_support"),
                    String::from("linked_program_start_semantics"),
                ],
                vec![
                    String::from("generalized_abi_required"),
                    String::from("linked_program_bundle_abi"),
                ],
                vec![String::from("i32_integer_family")],
                vec![
                    String::from("byte_addressed_linear_memory_v2"),
                    String::from("linked_program_start_order"),
                    String::from("runtime_support_module_family"),
                ],
                vec![
                    String::from("helper_lineage_receipts"),
                    String::from("linked_program_bundles"),
                    String::from("linked_program_start_order_receipts"),
                    String::from("runtime_support_modules"),
                ],
                TassadarInternalComputeImportPosture::NoImportsOnly,
                TassadarInternalComputeExactnessPosture::ExactRouteBounded,
                TassadarInternalComputePortabilityPosture::Planned,
                current_supported_machine_class_ids.clone(),
                vec![
                    TassadarInternalComputeRefusalClass::ResumableMultiSliceUnsupported,
                    TassadarInternalComputeRefusalClass::BroadHostImportUnsupported,
                    TassadarInternalComputeRefusalClass::NonCpuBackendUnsupported,
                ],
                vec![
                    String::from(TASSADAR_LINKED_PROGRAM_BUNDLE_EVAL_REPORT_REF),
                    String::from(TASSADAR_LINKED_PROGRAM_BUNDLE_RUNTIME_REPORT_REF),
                    String::from(TASSADAR_LINKED_PROGRAM_BUNDLE_SUMMARY_REPORT_REF),
                ],
                "linked-program and bounded runtime-support claims are now benchmarked as a separate implemented profile over explicit module graphs, helper lineage, start-order receipts, rollback posture, and module-local versus shared-state bundle rules. The profile remains non-portable and non-promoted until portability evidence and broader publication gates go green",
            ),
            TassadarInternalComputeProfileSpec::new(
                TassadarInternalComputeProfileId::DeterministicImportSubsetV1,
                TassadarInternalComputeProfileStatus::Implemented,
                vec![String::from(
                    TassadarWasmProfileId::ArticleI32ComputeV1.as_str(),
                )],
                vec![
                    String::from("article_control_and_memory_core"),
                    String::from("deterministic_import_effects"),
                    String::from("effect_safe_resumable_continuation"),
                ],
                vec![
                    String::from("generalized_abi_required"),
                    String::from("resumable_slice_abi"),
                ],
                vec![String::from("i32_integer_family")],
                vec![
                    String::from("call_frame_checkpoint_receipts"),
                    String::from("deterministic_import_replay_limits"),
                ],
                vec![
                    String::from("deterministic_stub_imports_only"),
                    String::from("effect_safe_resume_receipts"),
                ],
                TassadarInternalComputeImportPosture::DeterministicStubImportsOnly,
                TassadarInternalComputeExactnessPosture::ExactRouteBounded,
                TassadarInternalComputePortabilityPosture::Planned,
                current_supported_machine_class_ids.clone(),
                vec![
                    TassadarInternalComputeRefusalClass::ArbitraryWasmUnsupported,
                    TassadarInternalComputeRefusalClass::NonCpuBackendUnsupported,
                ],
                vec![
                    String::from(TASSADAR_EFFECT_SAFE_RESUME_REPORT_REF),
                    String::from(TASSADAR_EFFECT_TAXONOMY_REPORT_REF),
                    String::from(TASSADAR_RESUMABLE_MULTI_SLICE_PROMOTION_REPORT_REF),
                ],
                "deterministic import-mediated continuation is now benchmarked as a separate implemented profile over explicit deterministic-stub effect receipts layered on the resumable base lane. It remains non-portable and non-promoted until portability evidence and broader publication gates go green, and it still refuses host-backed state, sandbox delegation, nondeterministic input, and unsafe effects",
            ),
            TassadarInternalComputeProfileSpec::new(
                TassadarInternalComputeProfileId::ResumableMultiSliceV1,
                TassadarInternalComputeProfileStatus::Implemented,
                vec![String::from("tassadar.wasm.resumable_multi_slice.v1")],
                vec![
                    String::from("call_frame_resume_execution"),
                    String::from("checkpointed_multi_slice_execution"),
                ],
                vec![String::from("resumable_slice_abi")],
                vec![String::from("i32_integer_family")],
                vec![
                    String::from("call_frame_checkpoint_receipts"),
                    String::from("checkpoint_memory_delta_receipts"),
                ],
                vec![
                    String::from("call_frame_resume_manifests"),
                    String::from("checkpoint_objects"),
                    String::from("frame_stack_checkpoint_objects"),
                    String::from("memory_delta_receipts"),
                    String::from("resumable_execution"),
                ],
                TassadarInternalComputeImportPosture::NoImportsOnly,
                TassadarInternalComputeExactnessPosture::ExactRouteBounded,
                TassadarInternalComputePortabilityPosture::Planned,
                current_supported_machine_class_ids.clone(),
                vec![
                    TassadarInternalComputeRefusalClass::BroadHostImportUnsupported,
                    TassadarInternalComputeRefusalClass::NonCpuBackendUnsupported,
                ],
                vec![
                    String::from(TASSADAR_RESUMABLE_MULTI_SLICE_PROMOTION_REPORT_REF),
                    String::from(
                        "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
                    ),
                    String::from(
                        "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
                    ),
                ],
                "resumable multi-slice execution is now benchmarked as a separate implemented profile over frame-stack checkpoints, linear-memory checkpoints, datastream-backed continuation manifests, and exact fresh-versus-resumed parity. It remains non-portable and non-promoted until portability evidence and a broader publication gate go green",
            ),
            TassadarInternalComputeProfileSpec::new(
                TassadarInternalComputeProfileId::PortableBroadFamilyV1,
                TassadarInternalComputeProfileStatus::Planned,
                vec![String::from("tassadar.wasm.portable_broad_family.v1")],
                vec![String::from("broad_program_family_matrix")],
                vec![String::from("generalized_abi_required")],
                vec![
                    String::from("wider_integer_layouts"),
                    String::from("declared_family_matrix"),
                ],
                vec![
                    String::from("runtime_support_modules"),
                    String::from("checkpoint_memory_delta_receipts"),
                ],
                vec![
                    String::from("cross_machine_portability_envelopes"),
                    String::from("broad_internal_compute_acceptance_gate"),
                ],
                TassadarInternalComputeImportPosture::Planned,
                TassadarInternalComputeExactnessPosture::Planned,
                TassadarInternalComputePortabilityPosture::Planned,
                Vec::new(),
                vec![TassadarInternalComputeRefusalClass::NonCpuBackendUnsupported],
                vec![
                    String::from("issue://OpenAgentsInc/psionic/179"),
                    String::from("issue://OpenAgentsInc/psionic/181"),
                ],
                "portable broad-family publication is a later rung and remains blocked on the broad program-family matrix plus the portability acceptance gate",
            ),
            TassadarInternalComputeProfileSpec::new(
                TassadarInternalComputeProfileId::PublicBroadFamilyV1,
                TassadarInternalComputeProfileStatus::Planned,
                vec![String::from("tassadar.wasm.public_broad_family.v1")],
                vec![String::from("broad_internal_compute_publication")],
                vec![String::from("generalized_abi_required")],
                vec![String::from("declared_family_matrix")],
                vec![String::from("published_route_policy")],
                vec![String::from("broad_profile_publication_and_route_policy")],
                TassadarInternalComputeImportPosture::Planned,
                TassadarInternalComputeExactnessPosture::Planned,
                TassadarInternalComputePortabilityPosture::Planned,
                Vec::new(),
                vec![TassadarInternalComputeRefusalClass::NonCpuBackendUnsupported],
                vec![
                    String::from(
                        "fixtures/tassadar/reports/tassadar_broad_internal_compute_portability_report.json",
                    ),
                    String::from(
                        "fixtures/tassadar/reports/tassadar_broad_internal_compute_acceptance_gate.json",
                    ),
                    String::from(TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                    String::from(TASSADAR_BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
                ],
                "broad public internal-compute publication remains the last rung and stays blocked until the earlier ladder entries are green",
            ),
        ];
        profiles.sort_by_key(|profile| profile.profile_id.clone());
        let mut publication = Self {
            schema_version: 1,
            publication_id: String::from("tassadar.internal_compute_profile_ladder.v1"),
            report_ref: String::from(TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF),
            profiles,
            claim_boundary: String::from(
                "this ladder prevents vague `supports Rust/Wasm` language by requiring every public internal-compute claim to cite one named profile with explicit admitted features, refusal classes, portability posture, and evidence requirements",
            ),
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_internal_compute_profile_ladder|",
            &publication,
        );
        publication
    }

    #[must_use]
    pub fn profile(&self, profile_id: &str) -> Option<&TassadarInternalComputeProfileSpec> {
        self.profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    }
}

/// One concrete served or public internal-compute claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputeProfileClaim {
    pub profile_id: String,
    pub evidence_refs: Vec<String>,
    pub supported_machine_class_ids: Vec<String>,
    pub refusal_classes: Vec<TassadarInternalComputeRefusalClass>,
}

impl TassadarInternalComputeProfileClaim {
    #[must_use]
    pub fn new(
        profile_id: impl Into<String>,
        mut evidence_refs: Vec<String>,
        mut supported_machine_class_ids: Vec<String>,
        mut refusal_classes: Vec<TassadarInternalComputeRefusalClass>,
    ) -> Self {
        evidence_refs.sort();
        evidence_refs.dedup();
        supported_machine_class_ids.sort();
        supported_machine_class_ids.dedup();
        refusal_classes.sort();
        refusal_classes.dedup();
        Self {
            profile_id: profile_id.into(),
            evidence_refs,
            supported_machine_class_ids,
            refusal_classes,
        }
    }
}

/// Result of checking one internal-compute claim against the named ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputeProfileClaimCheckResult {
    pub checker_id: String,
    pub claim: TassadarInternalComputeProfileClaim,
    pub green: bool,
    pub failed_requirement_ids: Vec<String>,
    pub detail: String,
    pub claim_digest: String,
}

impl TassadarInternalComputeProfileClaimCheckResult {
    fn new(
        claim: TassadarInternalComputeProfileClaim,
        failed_requirement_ids: Vec<String>,
        detail: impl Into<String>,
    ) -> Self {
        let mut failed_requirement_ids = failed_requirement_ids;
        failed_requirement_ids.sort();
        failed_requirement_ids.dedup();
        let green = failed_requirement_ids.is_empty();
        let mut result = Self {
            checker_id: String::from("tassadar.internal_compute_profile_claim_checker.v1"),
            claim,
            green,
            failed_requirement_ids,
            detail: detail.into(),
            claim_digest: String::new(),
        };
        result.claim_digest = stable_digest(
            b"psionic_tassadar_internal_compute_profile_claim_check|",
            &result,
        );
        result
    }
}

#[must_use]
pub fn tassadar_internal_compute_profile_ladder_publication(
) -> TassadarInternalComputeProfileLadderPublication {
    TassadarInternalComputeProfileLadderPublication::new()
}

#[must_use]
pub fn tassadar_current_served_internal_compute_profile_claim(
) -> TassadarInternalComputeProfileClaim {
    TassadarInternalComputeProfileClaim::new(
        TassadarInternalComputeProfileId::ArticleCloseoutV1.as_str(),
        current_article_closeout_required_evidence_refs(),
        vec![
            String::from("host_cpu_aarch64"),
            String::from("host_cpu_x86_64"),
        ],
        vec![
            TassadarInternalComputeRefusalClass::ArbitraryWasmUnsupported,
            TassadarInternalComputeRefusalClass::BroadHostImportUnsupported,
            TassadarInternalComputeRefusalClass::BroadAbiUnsupported,
            TassadarInternalComputeRefusalClass::UnsupportedMachineClass,
            TassadarInternalComputeRefusalClass::NonCpuBackendUnsupported,
        ],
    )
}

#[must_use]
pub fn check_tassadar_internal_compute_profile_claim(
    publication: &TassadarInternalComputeProfileLadderPublication,
    claim: TassadarInternalComputeProfileClaim,
) -> TassadarInternalComputeProfileClaimCheckResult {
    let Some(profile) = publication.profile(claim.profile_id.as_str()) else {
        return TassadarInternalComputeProfileClaimCheckResult::new(
            claim,
            vec![String::from("unknown_profile")],
            "claimed profile is absent from the named internal-compute profile ladder",
        );
    };

    let mut failures = Vec::new();
    if profile.status != TassadarInternalComputeProfileStatus::Implemented {
        failures.push(String::from("profile_not_implemented"));
    }

    let claimed_evidence_refs = claim.evidence_refs.iter().cloned().collect::<BTreeSet<_>>();
    for required_ref in &profile.required_evidence_refs {
        if !claimed_evidence_refs.contains(required_ref) {
            failures.push(format!("missing_required_evidence:{required_ref}"));
        }
    }

    let claimed_machine_class_ids = claim
        .supported_machine_class_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_machine_class_ids = profile
        .supported_machine_class_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for required_machine_class_id in &required_machine_class_ids {
        if !claimed_machine_class_ids.contains(required_machine_class_id) {
            failures.push(format!(
                "missing_supported_machine_class:{required_machine_class_id}"
            ));
        }
    }
    for claimed_machine_class_id in &claimed_machine_class_ids {
        if !required_machine_class_ids.contains(claimed_machine_class_id) {
            failures.push(format!(
                "unsupported_machine_class_claimed:{claimed_machine_class_id}"
            ));
        }
    }

    let claimed_refusal_classes = claim
        .refusal_classes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for refusal_class in &profile.refusal_classes {
        if !claimed_refusal_classes.contains(refusal_class) {
            failures.push(format!(
                "missing_refusal_class:{}",
                serde_json::to_string(refusal_class).unwrap_or_default()
            ));
        }
    }

    let detail = if failures.is_empty() {
        format!(
            "claim `{}` is publishable because it names one implemented profile, preserves {} evidence refs, preserves {} supported machine classes, and preserves {} refusal classes",
            claim.profile_id,
            profile.required_evidence_refs.len(),
            profile.supported_machine_class_ids.len(),
            profile.refusal_classes.len(),
        )
    } else {
        format!(
            "claim `{}` is not publishable because it failed {} checker requirements",
            claim.profile_id,
            failures.len(),
        )
    };
    TassadarInternalComputeProfileClaimCheckResult::new(claim, failures, detail)
}

fn current_article_closeout_required_evidence_refs() -> Vec<String> {
    vec![
        String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
        String::from(TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF),
        String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF),
        String::from(TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_REPORT_REF),
        String::from(TASSADAR_SUDOKU_9X9_ARTICLE_REPRODUCER_REPORT_REF),
        String::from(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF),
        String::from(TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF),
        String::from(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF),
        String::from(TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF),
        String::from(TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_REPORT_REF),
    ]
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
        check_tassadar_internal_compute_profile_claim,
        tassadar_current_served_internal_compute_profile_claim,
        tassadar_internal_compute_profile_ladder_publication, TassadarInternalComputeProfileId,
        TassadarInternalComputeProfileStatus, TassadarInternalComputeRefusalClass,
    };

    #[test]
    fn internal_compute_profile_ladder_keeps_article_closeout_separate_from_future_rungs() {
        let publication = tassadar_internal_compute_profile_ladder_publication();
        let article_profile = publication
            .profile(TassadarInternalComputeProfileId::ArticleCloseoutV1.as_str())
            .expect("article closeout profile");
        let generalized_abi = publication
            .profile(TassadarInternalComputeProfileId::GeneralizedAbiV1.as_str())
            .expect("generalized abi profile");
        let wider_numeric = publication
            .profile(TassadarInternalComputeProfileId::WiderNumericDataLayoutV1.as_str())
            .expect("wider numeric profile");

        assert_eq!(
            article_profile.status,
            TassadarInternalComputeProfileStatus::Implemented
        );
        assert_eq!(
            generalized_abi.status,
            TassadarInternalComputeProfileStatus::Implemented
        );
        assert_eq!(
            wider_numeric.status,
            TassadarInternalComputeProfileStatus::Implemented
        );
        assert!(article_profile
            .required_evidence_refs
            .contains(&String::from(
                "fixtures/tassadar/reports/tassadar_rust_only_article_closeout_audit_report.json"
            )));
        assert!(generalized_abi
            .required_evidence_refs
            .contains(&String::from(
                "fixtures/tassadar/reports/tassadar_generalized_abi_family_report.json"
            )));
        assert!(wider_numeric.required_evidence_refs.contains(&String::from(
            "fixtures/tassadar/reports/tassadar_generalized_abi_family_report.json"
        )));
    }

    #[test]
    fn current_served_internal_compute_claim_is_green() {
        let publication = tassadar_internal_compute_profile_ladder_publication();
        let claim = tassadar_current_served_internal_compute_profile_claim();
        let result = check_tassadar_internal_compute_profile_claim(&publication, claim);

        assert!(result.green);
        assert_eq!(
            result.claim.profile_id,
            TassadarInternalComputeProfileId::ArticleCloseoutV1.as_str()
        );
    }

    #[test]
    fn claim_checker_rejects_incomplete_claims() {
        let publication = tassadar_internal_compute_profile_ladder_publication();
        let mut claim = tassadar_current_served_internal_compute_profile_claim();
        claim.evidence_refs.pop();
        claim
            .supported_machine_class_ids
            .push(String::from("other_host_cpu"));
        claim
            .refusal_classes
            .retain(|class| *class != TassadarInternalComputeRefusalClass::BroadAbiUnsupported);
        let result = check_tassadar_internal_compute_profile_claim(&publication, claim);

        assert!(!result.green);
        assert!(result
            .failed_requirement_ids
            .iter()
            .any(|failure| failure.starts_with("missing_required_evidence:")));
        assert!(result.failed_requirement_ids.contains(&String::from(
            "unsupported_machine_class_claimed:other_host_cpu"
        )));
        assert!(result
            .failed_requirement_ids
            .iter()
            .any(|failure| failure.starts_with("missing_refusal_class:")));
    }

    #[test]
    fn claim_checker_rejects_incomplete_implemented_profiles() {
        let publication = tassadar_internal_compute_profile_ladder_publication();
        let claim = super::TassadarInternalComputeProfileClaim::new(
            TassadarInternalComputeProfileId::RuntimeSupportSubsetV1.as_str(),
            vec![String::from("issue://OpenAgentsInc/psionic/180")],
            Vec::new(),
            Vec::new(),
        );
        let result = check_tassadar_internal_compute_profile_claim(&publication, claim);

        assert!(!result.green);
        assert!(result
            .failed_requirement_ids
            .iter()
            .any(|failure| failure.starts_with("missing_required_evidence:")));
        assert!(result
            .failed_requirement_ids
            .iter()
            .any(|failure| failure.starts_with("missing_refusal_class:")));
    }
}
