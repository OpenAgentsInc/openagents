use std::collections::{BTreeMap, BTreeSet};

use psionic_data::{
    PsionPluginContaminationBundle, PsionPluginContaminationItemKind, PsionPluginRouteLabel,
};
use psionic_eval::BenchmarkPackage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for the shared Psion plugin benchmark-package contract.
pub const PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_benchmark_package.v1";
/// Stable schema version for the shared Psion plugin benchmark receipt.
pub const PSION_PLUGIN_BENCHMARK_PACKAGE_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_benchmark_receipt.v1";

/// Benchmark families admitted under the shared plugin-conditioned contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginBenchmarkFamily {
    DiscoverySelection,
    ArgumentConstruction,
    SequencingMultiCall,
    RefusalRequestStructure,
    ResultInterpretation,
    GuestPluginCapabilityBoundary,
}

/// Prompt envelope admitted by the shared plugin-conditioned contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginBenchmarkPromptEnvelope {
    SingleTurnDirectiveText,
    StructuredPluginSelectionJson,
    StructuredPluginArgumentsJson,
    StructuredPluginSequenceJson,
    StructuredPluginRefusalJson,
    StructuredPluginInterpretationJson,
    StructuredGuestPluginCapabilityJson,
}

/// Expected response shape for one plugin-conditioned prompt format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginBenchmarkExpectedResponseFormat {
    PluginSelectionDecisionJson,
    PluginArgumentsJson,
    PluginSequencePlanJson,
    PluginRefusalDecisionJson,
    PluginInterpretationJson,
    GuestPluginCapabilityDecisionJson,
}

/// Serial versus parallel expectation for sequencing tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginSequenceMode {
    Serial,
    Parallelizable,
}

/// What the model should do after the declared sequence goal is satisfied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginContinuationPosture {
    StopAfterGoalSatisfied,
    ContinueUntilAdmissionSetExhausted,
    StopOnTypedRuntimeRefusal,
}

/// Negative-case shape for discovery and selection items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginSelectionNegativeCaseKind {
    WrongToolChoice,
    UnsupportedToolChoice,
    Overdelegation,
}

/// Shared prompt format contract for plugin-conditioned benchmark items.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginBenchmarkPromptFormat {
    /// Stable prompt format identifier.
    pub format_id: String,
    /// Stable system-instruction reference.
    pub system_instruction_ref: String,
    /// Stable user-template reference.
    pub user_template_ref: String,
    /// Prompt envelope admitted by the shared contract.
    pub envelope: PsionPluginBenchmarkPromptEnvelope,
    /// Expected response format.
    pub expected_response_format: PsionPluginBenchmarkExpectedResponseFormat,
    /// Whether explicit receipt boundaries must be preserved.
    pub preserve_receipt_boundaries: bool,
    /// Short explanation of the prompt format.
    pub detail: String,
}

impl PsionPluginBenchmarkPromptFormat {
    fn validate(&self) -> Result<(), PsionPluginBenchmarkPackageError> {
        ensure_nonempty(
            self.format_id.as_str(),
            "psion_plugin_prompt_format.format_id",
        )?;
        ensure_nonempty(
            self.system_instruction_ref.as_str(),
            "psion_plugin_prompt_format.system_instruction_ref",
        )?;
        ensure_nonempty(
            self.user_template_ref.as_str(),
            "psion_plugin_prompt_format.user_template_ref",
        )?;
        ensure_nonempty(self.detail.as_str(), "psion_plugin_prompt_format.detail")?;
        Ok(())
    }
}

/// One rubric dimension for interpretation grading.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginBenchmarkRubricDimension {
    /// Stable rubric dimension id.
    pub dimension_id: String,
    /// Weight in basis points.
    pub weight_bps: u32,
    /// Short explanation of the dimension.
    pub detail: String,
}

impl PsionPluginBenchmarkRubricDimension {
    fn validate(&self) -> Result<(), PsionPluginBenchmarkPackageError> {
        ensure_nonempty(
            self.dimension_id.as_str(),
            "psion_plugin_rubric_dimension.dimension_id",
        )?;
        validate_bps(self.weight_bps, "psion_plugin_rubric_dimension.weight_bps")?;
        ensure_nonempty(self.detail.as_str(), "psion_plugin_rubric_dimension.detail")?;
        Ok(())
    }
}

/// Grader for selection and route decisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginSelectionDecisionGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Expected route label.
    pub expected_route: PsionPluginRouteLabel,
    /// Expected selected plugin ids in order when the task requires delegation.
    pub expected_plugin_ids: Vec<String>,
    /// Whether order matters for the expected plugin ids.
    pub order_matters: bool,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Grader for route-only plugin-conditioned tasks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginExactRouteGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Expected route label.
    pub expected_route: PsionPluginRouteLabel,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Grader for packet-schema-aware argument construction tasks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginArgumentSchemaGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Stable tool name whose argument packet is being graded.
    pub tool_name: String,
    /// Required argument field paths.
    pub required_argument_paths: Vec<String>,
    /// Expected JSON value type for each required path.
    pub required_argument_types: BTreeMap<String, PsionPluginArgumentJsonValueType>,
    /// Field paths that must stay absent for the case.
    pub forbidden_argument_paths: Vec<String>,
    /// Whether request-for-structure is an allowed outcome.
    pub request_for_structure_allowed: bool,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Expected JSON value type for one plugin argument path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginArgumentJsonValueType {
    String,
    Number,
    Integer,
    Boolean,
    Object,
    Array,
}

/// Grader for sequencing and continuation decisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginSequencePlanGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Expected tool-name sequence.
    pub expected_tool_names: Vec<String>,
    /// Serial versus parallelizable expectation.
    pub sequence_mode: PsionPluginSequenceMode,
    /// Expected continuation posture.
    pub continuation_posture: PsionPluginContinuationPosture,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Grader for exact refusal or request-for-structure cases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginExactRefusalGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Accepted reason codes.
    pub accepted_reason_codes: Vec<String>,
    /// Whether request-for-structure is an allowed route instead of refusal.
    pub request_for_structure_allowed: bool,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Rubric-backed grader for post-plugin interpretation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginInterpretationRubricGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Stable rubric reference.
    pub rubric_ref: String,
    /// Minimum passing score in basis points.
    pub minimum_pass_bps: u32,
    /// Weighted rubric dimensions.
    pub dimensions: Vec<PsionPluginBenchmarkRubricDimension>,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Grader for guest-plugin capability-boundary decisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestCapabilityBoundaryGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Expected route label.
    pub expected_route: PsionPluginRouteLabel,
    /// Expected delegated plugin ids when guest use is admitted.
    pub expected_plugin_ids: Vec<String>,
    /// Accepted reason codes when refusal is required.
    pub accepted_reason_codes: Vec<String>,
    /// Capability-boundary ids that must remain explicit in the answer.
    pub required_capability_boundary_ids: Vec<String>,
    /// Capability-boundary ids that must stay absent.
    pub forbidden_capability_boundary_ids: Vec<String>,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Shared grader interfaces for plugin-conditioned benchmark tasks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "grader_kind", rename_all = "snake_case")]
pub enum PsionPluginBenchmarkGraderInterface {
    SelectionDecision(PsionPluginSelectionDecisionGrader),
    ExactRoute(PsionPluginExactRouteGrader),
    ArgumentSchema(PsionPluginArgumentSchemaGrader),
    SequencePlan(PsionPluginSequencePlanGrader),
    ExactRefusal(PsionPluginExactRefusalGrader),
    InterpretationRubric(PsionPluginInterpretationRubricGrader),
    GuestCapabilityBoundary(PsionPluginGuestCapabilityBoundaryGrader),
}

impl PsionPluginBenchmarkGraderInterface {
    fn validate(&self) -> Result<(), PsionPluginBenchmarkPackageError> {
        match self {
            Self::SelectionDecision(grader) => {
                ensure_nonempty(
                    grader.grader_id.as_str(),
                    "selection_decision_grader.grader_id",
                )?;
                ensure_nonempty(grader.detail.as_str(), "selection_decision_grader.detail")?;
                reject_duplicate_strings(
                    grader.expected_plugin_ids.as_slice(),
                    "selection_decision_grader.expected_plugin_ids",
                )?;
            }
            Self::ExactRoute(grader) => {
                ensure_nonempty(grader.grader_id.as_str(), "exact_route_grader.grader_id")?;
                ensure_nonempty(grader.detail.as_str(), "exact_route_grader.detail")?;
            }
            Self::ArgumentSchema(grader) => {
                ensure_nonempty(
                    grader.grader_id.as_str(),
                    "argument_schema_grader.grader_id",
                )?;
                ensure_nonempty(
                    grader.tool_name.as_str(),
                    "argument_schema_grader.tool_name",
                )?;
                ensure_nonempty(grader.detail.as_str(), "argument_schema_grader.detail")?;
                if grader.required_argument_paths.is_empty()
                    && !grader.request_for_structure_allowed
                {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from("argument_schema_grader.required_argument_paths"),
                    });
                }
                reject_duplicate_strings(
                    grader.required_argument_paths.as_slice(),
                    "argument_schema_grader.required_argument_paths",
                )?;
                for required_path in &grader.required_argument_paths {
                    if !grader.required_argument_types.contains_key(required_path) {
                        return Err(PsionPluginBenchmarkPackageError::MissingField {
                            field: format!(
                                "argument_schema_grader.required_argument_types[{required_path}]"
                            ),
                        });
                    }
                }
                for required_path in grader.required_argument_types.keys() {
                    ensure_nonempty(
                        required_path.as_str(),
                        "argument_schema_grader.required_argument_types.key",
                    )?;
                }
                reject_duplicate_strings(
                    grader.forbidden_argument_paths.as_slice(),
                    "argument_schema_grader.forbidden_argument_paths",
                )?;
            }
            Self::SequencePlan(grader) => {
                ensure_nonempty(grader.grader_id.as_str(), "sequence_plan_grader.grader_id")?;
                ensure_nonempty(grader.detail.as_str(), "sequence_plan_grader.detail")?;
                if grader.expected_tool_names.is_empty() {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from("sequence_plan_grader.expected_tool_names"),
                    });
                }
                reject_duplicate_strings(
                    grader.expected_tool_names.as_slice(),
                    "sequence_plan_grader.expected_tool_names",
                )?;
            }
            Self::ExactRefusal(grader) => {
                ensure_nonempty(grader.grader_id.as_str(), "exact_refusal_grader.grader_id")?;
                ensure_nonempty(grader.detail.as_str(), "exact_refusal_grader.detail")?;
                if grader.accepted_reason_codes.is_empty() && !grader.request_for_structure_allowed
                {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from("exact_refusal_grader.accepted_reason_codes"),
                    });
                }
                reject_duplicate_strings(
                    grader.accepted_reason_codes.as_slice(),
                    "exact_refusal_grader.accepted_reason_codes",
                )?;
            }
            Self::InterpretationRubric(grader) => {
                ensure_nonempty(
                    grader.grader_id.as_str(),
                    "interpretation_rubric_grader.grader_id",
                )?;
                ensure_nonempty(
                    grader.rubric_ref.as_str(),
                    "interpretation_rubric_grader.rubric_ref",
                )?;
                validate_bps(
                    grader.minimum_pass_bps,
                    "interpretation_rubric_grader.minimum_pass_bps",
                )?;
                ensure_nonempty(
                    grader.detail.as_str(),
                    "interpretation_rubric_grader.detail",
                )?;
                if grader.dimensions.is_empty() {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from("interpretation_rubric_grader.dimensions"),
                    });
                }
                let mut total_weight_bps = 0_u32;
                for dimension in &grader.dimensions {
                    dimension.validate()?;
                    total_weight_bps = total_weight_bps.saturating_add(dimension.weight_bps);
                }
                if total_weight_bps != 10_000 {
                    return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                        field: String::from("interpretation_rubric_grader.dimensions.weight_bps"),
                        expected: String::from("10000"),
                        actual: total_weight_bps.to_string(),
                    });
                }
            }
            Self::GuestCapabilityBoundary(grader) => {
                ensure_nonempty(
                    grader.grader_id.as_str(),
                    "guest_capability_boundary_grader.grader_id",
                )?;
                ensure_nonempty(
                    grader.detail.as_str(),
                    "guest_capability_boundary_grader.detail",
                )?;
                reject_duplicate_strings(
                    grader.expected_plugin_ids.as_slice(),
                    "guest_capability_boundary_grader.expected_plugin_ids",
                )?;
                reject_duplicate_strings(
                    grader.accepted_reason_codes.as_slice(),
                    "guest_capability_boundary_grader.accepted_reason_codes",
                )?;
                reject_duplicate_strings(
                    grader.required_capability_boundary_ids.as_slice(),
                    "guest_capability_boundary_grader.required_capability_boundary_ids",
                )?;
                reject_duplicate_strings(
                    grader.forbidden_capability_boundary_ids.as_slice(),
                    "guest_capability_boundary_grader.forbidden_capability_boundary_ids",
                )?;
                if matches!(
                    grader.expected_route,
                    PsionPluginRouteLabel::DelegateToAdmittedPlugin
                ) && grader.expected_plugin_ids.is_empty()
                {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from(
                            "guest_capability_boundary_grader.expected_plugin_ids",
                        ),
                    });
                }
                if matches!(
                    grader.expected_route,
                    PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability
                ) && grader.accepted_reason_codes.is_empty()
                {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from(
                            "guest_capability_boundary_grader.accepted_reason_codes",
                        ),
                    });
                }
                if grader.required_capability_boundary_ids.is_empty() {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from(
                            "guest_capability_boundary_grader.required_capability_boundary_ids",
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    fn grader_id(&self) -> &str {
        match self {
            Self::SelectionDecision(grader) => grader.grader_id.as_str(),
            Self::ExactRoute(grader) => grader.grader_id.as_str(),
            Self::ArgumentSchema(grader) => grader.grader_id.as_str(),
            Self::SequencePlan(grader) => grader.grader_id.as_str(),
            Self::ExactRefusal(grader) => grader.grader_id.as_str(),
            Self::InterpretationRubric(grader) => grader.grader_id.as_str(),
            Self::GuestCapabilityBoundary(grader) => grader.grader_id.as_str(),
        }
    }
}

/// Discovery and selection task contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginDiscoverySelectionTask {
    /// Admitted plugin ids visible to the task.
    pub admitted_plugin_ids: Vec<String>,
    /// Whether direct natural-language answer is admissible.
    pub direct_answer_allowed: bool,
    /// Expected route for the task.
    pub expected_route: PsionPluginRouteLabel,
    /// Expected selected plugin ids when delegation is required.
    pub expected_plugin_ids: Vec<String>,
    /// Optional negative-case classification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_case_kind: Option<PsionPluginSelectionNegativeCaseKind>,
}

/// Argument-construction task contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginArgumentConstructionTask {
    /// Tool name whose packet is being constructed.
    pub tool_name: String,
    /// Stable input schema id.
    pub input_schema_id: String,
    /// Required argument field paths.
    pub required_argument_paths: Vec<String>,
    /// Field paths known to be missing from user input.
    pub missing_argument_paths: Vec<String>,
    /// Field paths intentionally malformed for the case.
    pub malformed_argument_paths: Vec<String>,
    /// Expected route for the task.
    pub expected_route: PsionPluginRouteLabel,
}

/// Sequencing and multi-call task contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginSequencingTask {
    /// Admitted tool names visible to the task.
    pub admitted_tool_names: Vec<String>,
    /// Expected tool-name sequence.
    pub expected_tool_names: Vec<String>,
    /// Serial versus parallelizable expectation.
    pub sequence_mode: PsionPluginSequenceMode,
    /// Expected continuation posture.
    pub continuation_posture: PsionPluginContinuationPosture,
    /// Expected route for the task.
    pub expected_route: PsionPluginRouteLabel,
}

/// Refusal and request-for-structure task contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginRefusalTask {
    /// Expected route for the task.
    pub expected_route: PsionPluginRouteLabel,
    /// Accepted refusal reason codes when refusal is expected.
    pub accepted_reason_codes: Vec<String>,
    /// Missing field paths that justify request-for-structure.
    pub missing_argument_paths: Vec<String>,
    /// Unsupported plugin ids that justify refusal.
    pub unsupported_plugin_ids: Vec<String>,
    /// Whether overdelegation is the negative case being measured.
    pub overdelegation_negative: bool,
}

/// Post-plugin interpretation task contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginResultInterpretationTask {
    /// Receipt refs whose outputs must be interpreted.
    pub referenced_receipt_refs: Vec<String>,
    /// Whether execution-backed versus inferred statements must stay distinct.
    pub distinguish_execution_backed_from_inferred: bool,
    /// Whether continuation after refusal or failure is part of the task.
    pub continuation_after_failure_or_refusal: bool,
}

/// Scenario type for guest-plugin capability-boundary items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginGuestCapabilityScenarioKind {
    AdmittedDigestBoundUse,
    UnsupportedDigestLoadClaim,
    PublicationOverclaim,
    ArbitraryBinaryOverclaim,
    ServedUniversalityOverclaim,
}

/// Capability-boundary task contract for guest-plugin prompts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestCapabilityBoundaryTask {
    /// Admitted guest plugin ids visible to the task.
    pub admitted_guest_plugin_ids: Vec<String>,
    /// Expected route for the task.
    pub expected_route: PsionPluginRouteLabel,
    /// Expected delegated plugin ids when guest use is admitted.
    pub expected_plugin_ids: Vec<String>,
    /// Accepted refusal reason codes when refusal is required.
    pub accepted_reason_codes: Vec<String>,
    /// Scenario kind being tested.
    pub scenario_kind: PsionPluginGuestCapabilityScenarioKind,
    /// Capability-boundary ids that must remain explicit in the answer.
    pub required_capability_boundary_ids: Vec<String>,
    /// Capability-boundary ids that must stay absent.
    pub forbidden_capability_boundary_ids: Vec<String>,
}

/// Shared task contract for plugin-conditioned benchmark items.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "task_kind", rename_all = "snake_case")]
pub enum PsionPluginBenchmarkTaskContract {
    DiscoverySelection(PsionPluginDiscoverySelectionTask),
    ArgumentConstruction(PsionPluginArgumentConstructionTask),
    SequencingMultiCall(PsionPluginSequencingTask),
    RefusalRequestStructure(PsionPluginRefusalTask),
    ResultInterpretation(PsionPluginResultInterpretationTask),
    GuestPluginCapabilityBoundary(PsionPluginGuestCapabilityBoundaryTask),
}

impl PsionPluginBenchmarkTaskContract {
    fn family(&self) -> PsionPluginBenchmarkFamily {
        match self {
            Self::DiscoverySelection(_) => PsionPluginBenchmarkFamily::DiscoverySelection,
            Self::ArgumentConstruction(_) => PsionPluginBenchmarkFamily::ArgumentConstruction,
            Self::SequencingMultiCall(_) => PsionPluginBenchmarkFamily::SequencingMultiCall,
            Self::RefusalRequestStructure(_) => PsionPluginBenchmarkFamily::RefusalRequestStructure,
            Self::ResultInterpretation(_) => PsionPluginBenchmarkFamily::ResultInterpretation,
            Self::GuestPluginCapabilityBoundary(_) => {
                PsionPluginBenchmarkFamily::GuestPluginCapabilityBoundary
            }
        }
    }

    fn validate(&self) -> Result<(), PsionPluginBenchmarkPackageError> {
        match self {
            Self::DiscoverySelection(task) => {
                reject_duplicate_strings(
                    task.admitted_plugin_ids.as_slice(),
                    "discovery_selection_task.admitted_plugin_ids",
                )?;
                reject_duplicate_strings(
                    task.expected_plugin_ids.as_slice(),
                    "discovery_selection_task.expected_plugin_ids",
                )?;
                if matches!(
                    task.expected_route,
                    PsionPluginRouteLabel::DelegateToAdmittedPlugin
                ) && task.expected_plugin_ids.is_empty()
                {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from("discovery_selection_task.expected_plugin_ids"),
                    });
                }
            }
            Self::ArgumentConstruction(task) => {
                ensure_nonempty(
                    task.tool_name.as_str(),
                    "argument_construction_task.tool_name",
                )?;
                ensure_nonempty(
                    task.input_schema_id.as_str(),
                    "argument_construction_task.input_schema_id",
                )?;
                reject_duplicate_strings(
                    task.required_argument_paths.as_slice(),
                    "argument_construction_task.required_argument_paths",
                )?;
                reject_duplicate_strings(
                    task.missing_argument_paths.as_slice(),
                    "argument_construction_task.missing_argument_paths",
                )?;
                reject_duplicate_strings(
                    task.malformed_argument_paths.as_slice(),
                    "argument_construction_task.malformed_argument_paths",
                )?;
            }
            Self::SequencingMultiCall(task) => {
                reject_duplicate_strings(
                    task.admitted_tool_names.as_slice(),
                    "sequencing_task.admitted_tool_names",
                )?;
                if task.expected_tool_names.is_empty() {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from("sequencing_task.expected_tool_names"),
                    });
                }
                reject_duplicate_strings(
                    task.expected_tool_names.as_slice(),
                    "sequencing_task.expected_tool_names",
                )?;
            }
            Self::RefusalRequestStructure(task) => {
                reject_duplicate_strings(
                    task.accepted_reason_codes.as_slice(),
                    "refusal_task.accepted_reason_codes",
                )?;
                reject_duplicate_strings(
                    task.missing_argument_paths.as_slice(),
                    "refusal_task.missing_argument_paths",
                )?;
                reject_duplicate_strings(
                    task.unsupported_plugin_ids.as_slice(),
                    "refusal_task.unsupported_plugin_ids",
                )?;
            }
            Self::ResultInterpretation(task) => {
                if task.referenced_receipt_refs.is_empty() {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from("result_interpretation_task.referenced_receipt_refs"),
                    });
                }
                reject_duplicate_strings(
                    task.referenced_receipt_refs.as_slice(),
                    "result_interpretation_task.referenced_receipt_refs",
                )?;
            }
            Self::GuestPluginCapabilityBoundary(task) => {
                reject_duplicate_strings(
                    task.admitted_guest_plugin_ids.as_slice(),
                    "guest_capability_boundary_task.admitted_guest_plugin_ids",
                )?;
                reject_duplicate_strings(
                    task.expected_plugin_ids.as_slice(),
                    "guest_capability_boundary_task.expected_plugin_ids",
                )?;
                reject_duplicate_strings(
                    task.accepted_reason_codes.as_slice(),
                    "guest_capability_boundary_task.accepted_reason_codes",
                )?;
                reject_duplicate_strings(
                    task.required_capability_boundary_ids.as_slice(),
                    "guest_capability_boundary_task.required_capability_boundary_ids",
                )?;
                reject_duplicate_strings(
                    task.forbidden_capability_boundary_ids.as_slice(),
                    "guest_capability_boundary_task.forbidden_capability_boundary_ids",
                )?;
                if matches!(
                    task.expected_route,
                    PsionPluginRouteLabel::DelegateToAdmittedPlugin
                ) && task.expected_plugin_ids.is_empty()
                {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from(
                            "guest_capability_boundary_task.expected_plugin_ids",
                        ),
                    });
                }
                if matches!(
                    task.expected_route,
                    PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability
                ) && task.accepted_reason_codes.is_empty()
                {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from(
                            "guest_capability_boundary_task.accepted_reason_codes",
                        ),
                    });
                }
                if task.required_capability_boundary_ids.is_empty() {
                    return Err(PsionPluginBenchmarkPackageError::MissingField {
                        field: String::from(
                            "guest_capability_boundary_task.required_capability_boundary_ids",
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Item-level contamination attachment bound to the PSION_PLUGIN-6 bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginBenchmarkContaminationAttachment {
    /// Stable contamination bundle ref.
    pub contamination_bundle_ref: String,
    /// Stable contamination bundle digest.
    pub contamination_bundle_digest: String,
    /// Optional authored prompt ref when the item is benchmark-authored rather than
    /// derived from held-out lineage rows directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored_prompt_ref: Option<String>,
    /// Parent lineage ids used to build or grade the item.
    pub parent_lineage_ids: Vec<String>,
    /// Source case ids used to build or label the item.
    pub source_case_ids: Vec<String>,
    /// Receipt refs visible to the item.
    pub receipt_refs: Vec<String>,
    /// Short explanation of the contamination posture.
    pub detail: String,
}

impl PsionPluginBenchmarkContaminationAttachment {
    fn validate_against_bundle(
        &self,
        bundle: &PsionPluginContaminationBundle,
    ) -> Result<(), PsionPluginBenchmarkPackageError> {
        ensure_nonempty(
            self.contamination_bundle_ref.as_str(),
            "benchmark_contamination_attachment.contamination_bundle_ref",
        )?;
        if self.contamination_bundle_digest != bundle.bundle_digest {
            return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                field: String::from(
                    "benchmark_contamination_attachment.contamination_bundle_digest",
                ),
                expected: bundle.bundle_digest.clone(),
                actual: self.contamination_bundle_digest.clone(),
            });
        }
        ensure_nonempty(
            self.detail.as_str(),
            "benchmark_contamination_attachment.detail",
        )?;
        reject_duplicate_strings(
            self.parent_lineage_ids.as_slice(),
            "benchmark_contamination_attachment.parent_lineage_ids",
        )?;
        reject_duplicate_strings(
            self.source_case_ids.as_slice(),
            "benchmark_contamination_attachment.source_case_ids",
        )?;
        reject_duplicate_strings(
            self.receipt_refs.as_slice(),
            "benchmark_contamination_attachment.receipt_refs",
        )?;
        if let Some(authored_prompt_ref) = &self.authored_prompt_ref {
            ensure_nonempty(
                authored_prompt_ref.as_str(),
                "benchmark_contamination_attachment.authored_prompt_ref",
            )?;
        }
        if self.parent_lineage_ids.is_empty() {
            if self.authored_prompt_ref.is_none() {
                return Err(PsionPluginBenchmarkPackageError::MissingField {
                    field: String::from(
                        "benchmark_contamination_attachment.parent_lineage_ids_or_authored_prompt_ref",
                    ),
                });
            }
            if !self.source_case_ids.is_empty() || !self.receipt_refs.is_empty() {
                return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                    field: String::from(
                        "benchmark_contamination_attachment.source_case_ids_or_receipt_refs",
                    ),
                    expected: String::from(
                        "empty when the item uses authored prompt provenance only",
                    ),
                    actual: String::from("non-empty"),
                });
            }
            return Ok(());
        }

        let mut expected_source_case_ids = BTreeSet::new();
        let mut expected_receipt_refs = BTreeSet::new();
        for lineage_id in &self.parent_lineage_ids {
            let row = bundle
                .parent_lineage_rows
                .iter()
                .find(|row| row.lineage_id == *lineage_id)
                .ok_or_else(
                    || PsionPluginBenchmarkPackageError::UnknownContaminationLineage {
                        lineage_id: lineage_id.clone(),
                    },
                )?;
            if row.item_kind != PsionPluginContaminationItemKind::HeldOutEvalRecord {
                return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                    field: String::from("benchmark_contamination_attachment.parent_lineage_ids"),
                    expected: String::from("held_out_eval_record lineage"),
                    actual: format!("{:?}", row.item_kind),
                });
            }
            expected_source_case_ids.insert(row.source_trace.source_case_id.as_str());
            for receipt_ref in &row.receipt_refs {
                expected_receipt_refs.insert(receipt_ref.as_str());
            }
        }
        for source_case_id in &self.source_case_ids {
            if !expected_source_case_ids.contains(source_case_id.as_str()) {
                return Err(PsionPluginBenchmarkPackageError::UnknownSourceCase {
                    source_case_id: source_case_id.clone(),
                });
            }
        }
        for receipt_ref in &self.receipt_refs {
            if !expected_receipt_refs.contains(receipt_ref.as_str()) {
                return Err(PsionPluginBenchmarkPackageError::UnknownReceiptRef {
                    receipt_ref: receipt_ref.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Metric kinds preserved on one plugin benchmark receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginBenchmarkMetricKind {
    RouteAccuracyBps,
    SelectionAccuracyBps,
    ArgumentSchemaAccuracyBps,
    SequencePlanAccuracyBps,
    ContinuationBoundaryAccuracyBps,
    WrongToolRejectionAccuracyBps,
    UnsupportedToolRefusalAccuracyBps,
    UnsupportedCapabilityRefusalAccuracyBps,
    OverdelegationRejectionAccuracyBps,
    RequestForStructureAccuracyBps,
    TypedRuntimeRefusalAccuracyBps,
    ExecutionBackedBoundaryAccuracyBps,
    InterpretationScoreBps,
    GuestPluginAdmittedUseAccuracyBps,
    GuestPluginUnsupportedLoadRefusalAccuracyBps,
    GuestPluginPublicationBoundaryAccuracyBps,
    GuestPluginArbitraryBinaryBoundaryAccuracyBps,
    GuestPluginServedUniversalityBoundaryAccuracyBps,
}

/// One observed metric preserved on one plugin benchmark receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginObservedMetric {
    /// Metric kind.
    pub kind: PsionPluginBenchmarkMetricKind,
    /// Metric value in basis points.
    pub value_bps: u32,
    /// Short explanation of the observed metric.
    pub detail: String,
}

impl PsionPluginObservedMetric {
    fn validate(&self) -> Result<(), PsionPluginBenchmarkPackageError> {
        validate_bps(self.value_bps, "psion_plugin_observed_metric.value_bps")?;
        ensure_nonempty(self.detail.as_str(), "psion_plugin_observed_metric.detail")?;
        Ok(())
    }
}

/// Declares whether execution evidence is required by one benchmark item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginBenchmarkReceiptPosture {
    /// Whether execution-backed evidence is required.
    pub execution_evidence_required: bool,
    /// Receipt refs that must remain visible in scoring artifacts.
    pub required_receipt_refs: Vec<String>,
    /// Whether the item forbids claims about unseen execution.
    pub forbid_unseen_execution_claims: bool,
    /// Short explanation of the receipt posture.
    pub detail: String,
}

impl PsionPluginBenchmarkReceiptPosture {
    fn validate_against_attachment(
        &self,
        attachment: &PsionPluginBenchmarkContaminationAttachment,
    ) -> Result<(), PsionPluginBenchmarkPackageError> {
        ensure_nonempty(self.detail.as_str(), "benchmark_receipt_posture.detail")?;
        reject_duplicate_strings(
            self.required_receipt_refs.as_slice(),
            "benchmark_receipt_posture.required_receipt_refs",
        )?;
        if self.execution_evidence_required && self.required_receipt_refs.is_empty() {
            return Err(PsionPluginBenchmarkPackageError::MissingField {
                field: String::from("benchmark_receipt_posture.required_receipt_refs"),
            });
        }
        let attachment_receipts = attachment
            .receipt_refs
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for receipt_ref in &self.required_receipt_refs {
            if !attachment_receipts.contains(receipt_ref.as_str()) {
                return Err(PsionPluginBenchmarkPackageError::UnknownReceiptRef {
                    receipt_ref: receipt_ref.clone(),
                });
            }
        }
        Ok(())
    }
}

/// One benchmark item under the shared plugin-conditioned contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginBenchmarkItem {
    /// Stable item id.
    pub item_id: String,
    /// Benchmark family covered by the item.
    pub family: PsionPluginBenchmarkFamily,
    /// Prompt format used by the item.
    pub prompt_format_id: String,
    /// Grader interface used by the item.
    pub grader_id: String,
    /// Stable digest over the prompt payload.
    pub prompt_digest: String,
    /// Item-level contamination attachment.
    pub contamination_attachment: PsionPluginBenchmarkContaminationAttachment,
    /// Item-level receipt posture.
    pub receipt_posture: PsionPluginBenchmarkReceiptPosture,
    /// Shared task contract.
    pub task: PsionPluginBenchmarkTaskContract,
    /// Short explanation of the item.
    pub detail: String,
}

impl PsionPluginBenchmarkItem {
    fn validate(&self) -> Result<(), PsionPluginBenchmarkPackageError> {
        ensure_nonempty(self.item_id.as_str(), "psion_plugin_benchmark_item.item_id")?;
        ensure_nonempty(
            self.prompt_format_id.as_str(),
            "psion_plugin_benchmark_item.prompt_format_id",
        )?;
        ensure_nonempty(
            self.grader_id.as_str(),
            "psion_plugin_benchmark_item.grader_id",
        )?;
        ensure_nonempty(
            self.prompt_digest.as_str(),
            "psion_plugin_benchmark_item.prompt_digest",
        )?;
        ensure_nonempty(self.detail.as_str(), "psion_plugin_benchmark_item.detail")?;
        self.task.validate()?;
        if self.task.family() != self.family {
            return Err(PsionPluginBenchmarkPackageError::TaskFamilyMismatch {
                expected_family: format!("{:?}", self.family),
                actual_family: format!("{:?}", self.task.family()),
            });
        }
        Ok(())
    }
}

/// One shared plugin-conditioned benchmark package contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginBenchmarkPackageContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable package id.
    pub package_id: String,
    /// Package family covered by the package.
    pub package_family: PsionPluginBenchmarkFamily,
    /// Generic eval benchmark package bound to the contract.
    pub benchmark_package: BenchmarkPackage,
    /// Prompt formats used by the package.
    pub prompt_formats: Vec<PsionPluginBenchmarkPromptFormat>,
    /// Grader interfaces used by the package.
    pub grader_interfaces: Vec<PsionPluginBenchmarkGraderInterface>,
    /// Benchmark items in the package.
    pub items: Vec<PsionPluginBenchmarkItem>,
    /// Short explanation of the package.
    pub summary: String,
    /// Stable digest over the package.
    pub package_digest: String,
}

impl PsionPluginBenchmarkPackageContract {
    /// Validates the shared package contract against the contamination bundle.
    pub fn validate_against_contamination(
        &self,
        contamination: &PsionPluginContaminationBundle,
    ) -> Result<(), PsionPluginBenchmarkPackageError> {
        self.validate_shape()?;
        self.benchmark_package
            .validate()
            .map_err(PsionPluginBenchmarkPackageError::EvalRuntime)?;
        if self.benchmark_package.cases.len() != self.items.len() {
            return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_plugin_benchmark_package.items"),
                expected: self.benchmark_package.cases.len().to_string(),
                actual: self.items.len().to_string(),
            });
        }

        let mut prompt_formats = BTreeMap::new();
        for format in &self.prompt_formats {
            format.validate()?;
            if prompt_formats
                .insert(format.format_id.as_str(), format)
                .is_some()
            {
                return Err(PsionPluginBenchmarkPackageError::DuplicatePromptFormat {
                    format_id: format.format_id.clone(),
                });
            }
        }
        let mut graders = BTreeMap::new();
        for grader in &self.grader_interfaces {
            grader.validate()?;
            if graders.insert(grader.grader_id(), grader).is_some() {
                return Err(PsionPluginBenchmarkPackageError::DuplicateGrader {
                    grader_id: grader.grader_id().to_owned(),
                });
            }
        }
        let expected_case_ids = self
            .benchmark_package
            .cases
            .iter()
            .map(|case| case.case_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut seen_item_ids = BTreeSet::new();
        for item in &self.items {
            item.validate()?;
            if item.family != self.package_family {
                return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                    field: format!(
                        "psion_plugin_benchmark_package.items[{}].family",
                        item.item_id
                    ),
                    expected: format!("{:?}", self.package_family),
                    actual: format!("{:?}", item.family),
                });
            }
            if !seen_item_ids.insert(item.item_id.as_str()) {
                return Err(PsionPluginBenchmarkPackageError::DuplicateItem {
                    item_id: item.item_id.clone(),
                });
            }
            if !expected_case_ids.contains(item.item_id.as_str()) {
                return Err(PsionPluginBenchmarkPackageError::UnknownBenchmarkCase {
                    case_id: item.item_id.clone(),
                });
            }
            let prompt_format = prompt_formats
                .get(item.prompt_format_id.as_str())
                .ok_or_else(|| PsionPluginBenchmarkPackageError::UnknownPromptFormat {
                    format_id: item.prompt_format_id.clone(),
                })?;
            let grader = graders.get(item.grader_id.as_str()).ok_or_else(|| {
                PsionPluginBenchmarkPackageError::UnknownGrader {
                    grader_id: item.grader_id.clone(),
                }
            })?;
            item.contamination_attachment
                .validate_against_bundle(contamination)?;
            item.receipt_posture
                .validate_against_attachment(&item.contamination_attachment)?;
            validate_item_prompt_compatibility(item, prompt_format)?;
            validate_item_grader_compatibility(item, grader)?;
        }
        if self.package_digest != stable_package_digest(self) {
            return Err(PsionPluginBenchmarkPackageError::DigestMismatch {
                kind: String::from("psion_plugin_benchmark_package"),
            });
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), PsionPluginBenchmarkPackageError> {
        if self.schema_version != PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION {
            return Err(PsionPluginBenchmarkPackageError::SchemaVersionMismatch {
                expected: String::from(PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.package_id.as_str(),
            "psion_plugin_benchmark_package.package_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_plugin_benchmark_package.summary",
        )?;
        if self.prompt_formats.is_empty() {
            return Err(PsionPluginBenchmarkPackageError::MissingField {
                field: String::from("psion_plugin_benchmark_package.prompt_formats"),
            });
        }
        if self.grader_interfaces.is_empty() {
            return Err(PsionPluginBenchmarkPackageError::MissingField {
                field: String::from("psion_plugin_benchmark_package.grader_interfaces"),
            });
        }
        if self.items.is_empty() {
            return Err(PsionPluginBenchmarkPackageError::MissingField {
                field: String::from("psion_plugin_benchmark_package.items"),
            });
        }
        Ok(())
    }
}

/// One benchmark receipt under the shared plugin-conditioned contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginBenchmarkPackageReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable package id.
    pub package_id: String,
    /// Stable package digest.
    pub package_digest: String,
    /// Package family covered by the receipt.
    pub package_family: PsionPluginBenchmarkFamily,
    /// Contamination bundle digest the package cited.
    pub contamination_bundle_digest: String,
    /// Number of benchmark items covered by the receipt.
    pub item_count: u32,
    /// Observed metrics.
    pub observed_metrics: Vec<PsionPluginObservedMetric>,
    /// Short explanation of the receipt.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionPluginBenchmarkPackageReceipt {
    /// Validates the receipt against the shared package contract.
    pub fn validate_against_package(
        &self,
        package: &PsionPluginBenchmarkPackageContract,
        contamination: &PsionPluginContaminationBundle,
    ) -> Result<(), PsionPluginBenchmarkPackageError> {
        if self.schema_version != PSION_PLUGIN_BENCHMARK_PACKAGE_RECEIPT_SCHEMA_VERSION {
            return Err(PsionPluginBenchmarkPackageError::SchemaVersionMismatch {
                expected: String::from(PSION_PLUGIN_BENCHMARK_PACKAGE_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "psion_plugin_benchmark_receipt.receipt_id",
        )?;
        if self.package_id != package.package_id {
            return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_plugin_benchmark_receipt.package_id"),
                expected: package.package_id.clone(),
                actual: self.package_id.clone(),
            });
        }
        if self.package_digest != package.package_digest {
            return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_plugin_benchmark_receipt.package_digest"),
                expected: package.package_digest.clone(),
                actual: self.package_digest.clone(),
            });
        }
        if self.package_family != package.package_family {
            return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_plugin_benchmark_receipt.package_family"),
                expected: format!("{:?}", package.package_family),
                actual: format!("{:?}", self.package_family),
            });
        }
        if self.contamination_bundle_digest != contamination.bundle_digest {
            return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_plugin_benchmark_receipt.contamination_bundle_digest"),
                expected: contamination.bundle_digest.clone(),
                actual: self.contamination_bundle_digest.clone(),
            });
        }
        if self.item_count != package.items.len() as u32 {
            return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_plugin_benchmark_receipt.item_count"),
                expected: package.items.len().to_string(),
                actual: self.item_count.to_string(),
            });
        }
        if self.observed_metrics.is_empty() {
            return Err(PsionPluginBenchmarkPackageError::MissingField {
                field: String::from("psion_plugin_benchmark_receipt.observed_metrics"),
            });
        }
        let mut metric_kinds = BTreeSet::new();
        for metric in &self.observed_metrics {
            metric.validate()?;
            if !metric_kinds.insert(metric.kind) {
                return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
                    field: String::from("psion_plugin_benchmark_receipt.observed_metrics.kind"),
                    expected: String::from("unique metric kinds"),
                    actual: String::from("duplicate metric kind"),
                });
            }
        }
        ensure_nonempty(
            self.summary.as_str(),
            "psion_plugin_benchmark_receipt.summary",
        )?;
        if self.receipt_digest != stable_receipt_digest(self) {
            return Err(PsionPluginBenchmarkPackageError::DigestMismatch {
                kind: String::from("psion_plugin_benchmark_receipt"),
            });
        }
        Ok(())
    }
}

/// Records one shared benchmark receipt above a plugin package contract.
pub fn record_psion_plugin_benchmark_package_receipt(
    receipt_id: impl Into<String>,
    package: &PsionPluginBenchmarkPackageContract,
    contamination: &PsionPluginContaminationBundle,
    observed_metrics: Vec<PsionPluginObservedMetric>,
    summary: impl Into<String>,
) -> Result<PsionPluginBenchmarkPackageReceipt, PsionPluginBenchmarkPackageError> {
    let mut receipt = PsionPluginBenchmarkPackageReceipt {
        schema_version: String::from(PSION_PLUGIN_BENCHMARK_PACKAGE_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        package_id: package.package_id.clone(),
        package_digest: package.package_digest.clone(),
        package_family: package.package_family,
        contamination_bundle_digest: contamination.bundle_digest.clone(),
        item_count: package.items.len() as u32,
        observed_metrics,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_receipt_digest(&receipt);
    receipt.validate_against_package(package, contamination)?;
    Ok(receipt)
}

#[derive(Debug, Error)]
pub enum PsionPluginBenchmarkPackageError {
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("duplicate prompt format `{format_id}`")]
    DuplicatePromptFormat { format_id: String },
    #[error("duplicate grader `{grader_id}`")]
    DuplicateGrader { grader_id: String },
    #[error("duplicate benchmark item `{item_id}`")]
    DuplicateItem { item_id: String },
    #[error("unknown prompt format `{format_id}`")]
    UnknownPromptFormat { format_id: String },
    #[error("unknown grader `{grader_id}`")]
    UnknownGrader { grader_id: String },
    #[error("unknown benchmark case `{case_id}`")]
    UnknownBenchmarkCase { case_id: String },
    #[error("unknown contamination lineage `{lineage_id}`")]
    UnknownContaminationLineage { lineage_id: String },
    #[error("unknown source case `{source_case_id}`")]
    UnknownSourceCase { source_case_id: String },
    #[error("unknown receipt ref `{receipt_ref}`")]
    UnknownReceiptRef { receipt_ref: String },
    #[error("item task family mismatch: expected `{expected_family}`, found `{actual_family}`")]
    TaskFamilyMismatch {
        expected_family: String,
        actual_family: String,
    },
    #[error("prompt format `{format_id}` is incompatible with item `{item_id}`")]
    PromptFormatIncompatible { format_id: String, item_id: String },
    #[error("grader `{grader_id}` is incompatible with item `{item_id}`")]
    GraderIncompatible { grader_id: String, item_id: String },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
    #[error("duplicate string entry in `{field}`")]
    DuplicateString { field: String },
    #[error(transparent)]
    EvalRuntime(#[from] psionic_eval::EvalRuntimeError),
}

fn validate_item_prompt_compatibility(
    item: &PsionPluginBenchmarkItem,
    prompt_format: &PsionPluginBenchmarkPromptFormat,
) -> Result<(), PsionPluginBenchmarkPackageError> {
    let compatible = match item.task {
        PsionPluginBenchmarkTaskContract::DiscoverySelection(_) => {
            prompt_format.expected_response_format
                == PsionPluginBenchmarkExpectedResponseFormat::PluginSelectionDecisionJson
        }
        PsionPluginBenchmarkTaskContract::ArgumentConstruction(_) => {
            prompt_format.expected_response_format
                == PsionPluginBenchmarkExpectedResponseFormat::PluginArgumentsJson
        }
        PsionPluginBenchmarkTaskContract::SequencingMultiCall(_) => {
            prompt_format.expected_response_format
                == PsionPluginBenchmarkExpectedResponseFormat::PluginSequencePlanJson
        }
        PsionPluginBenchmarkTaskContract::RefusalRequestStructure(_) => {
            prompt_format.expected_response_format
                == PsionPluginBenchmarkExpectedResponseFormat::PluginRefusalDecisionJson
        }
        PsionPluginBenchmarkTaskContract::ResultInterpretation(_) => {
            prompt_format.expected_response_format
                == PsionPluginBenchmarkExpectedResponseFormat::PluginInterpretationJson
        }
        PsionPluginBenchmarkTaskContract::GuestPluginCapabilityBoundary(_) => {
            prompt_format.expected_response_format
                == PsionPluginBenchmarkExpectedResponseFormat::GuestPluginCapabilityDecisionJson
        }
    };
    if !compatible {
        return Err(PsionPluginBenchmarkPackageError::PromptFormatIncompatible {
            format_id: prompt_format.format_id.clone(),
            item_id: item.item_id.clone(),
        });
    }
    Ok(())
}

fn validate_item_grader_compatibility(
    item: &PsionPluginBenchmarkItem,
    grader: &PsionPluginBenchmarkGraderInterface,
) -> Result<(), PsionPluginBenchmarkPackageError> {
    let compatible = match (&item.task, grader) {
        (
            PsionPluginBenchmarkTaskContract::DiscoverySelection(_),
            PsionPluginBenchmarkGraderInterface::SelectionDecision(_)
            | PsionPluginBenchmarkGraderInterface::ExactRoute(_),
        ) => true,
        (
            PsionPluginBenchmarkTaskContract::ArgumentConstruction(_),
            PsionPluginBenchmarkGraderInterface::ArgumentSchema(_)
            | PsionPluginBenchmarkGraderInterface::ExactRefusal(_),
        ) => true,
        (
            PsionPluginBenchmarkTaskContract::SequencingMultiCall(_),
            PsionPluginBenchmarkGraderInterface::SequencePlan(_),
        ) => true,
        (
            PsionPluginBenchmarkTaskContract::RefusalRequestStructure(_),
            PsionPluginBenchmarkGraderInterface::ExactRefusal(_)
            | PsionPluginBenchmarkGraderInterface::ExactRoute(_),
        ) => true,
        (
            PsionPluginBenchmarkTaskContract::ResultInterpretation(_),
            PsionPluginBenchmarkGraderInterface::InterpretationRubric(_),
        ) => true,
        (
            PsionPluginBenchmarkTaskContract::GuestPluginCapabilityBoundary(_),
            PsionPluginBenchmarkGraderInterface::GuestCapabilityBoundary(_),
        ) => true,
        _ => false,
    };
    if !compatible {
        return Err(PsionPluginBenchmarkPackageError::GraderIncompatible {
            grader_id: grader.grader_id().to_owned(),
            item_id: item.item_id.clone(),
        });
    }
    Ok(())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPluginBenchmarkPackageError> {
    if value.trim().is_empty() {
        return Err(PsionPluginBenchmarkPackageError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionPluginBenchmarkPackageError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(PsionPluginBenchmarkPackageError::DuplicateString {
                field: String::from(field),
            });
        }
    }
    Ok(())
}

fn validate_bps(value: u32, field: &str) -> Result<(), PsionPluginBenchmarkPackageError> {
    if value > 10_000 {
        return Err(PsionPluginBenchmarkPackageError::FieldMismatch {
            field: String::from(field),
            expected: String::from("<=10000"),
            actual: value.to_string(),
        });
    }
    Ok(())
}

fn stable_package_digest(package: &PsionPluginBenchmarkPackageContract) -> String {
    let mut canonical = package.clone();
    canonical.package_digest.clear();
    let encoded = serde_json::to_vec(&canonical).expect("package should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_psion_plugin_benchmark_package|");
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn stable_receipt_digest(receipt: &PsionPluginBenchmarkPackageReceipt) -> String {
    let mut canonical = receipt.clone();
    canonical.receipt_digest.clear();
    let encoded = serde_json::to_vec(&canonical).expect("receipt should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_psion_plugin_benchmark_receipt|");
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        stable_package_digest, PsionPluginBenchmarkContaminationAttachment,
        PsionPluginBenchmarkExpectedResponseFormat, PsionPluginBenchmarkFamily,
        PsionPluginBenchmarkGraderInterface, PsionPluginBenchmarkItem,
        PsionPluginBenchmarkPackageContract, PsionPluginBenchmarkPackageError,
        PsionPluginBenchmarkPromptEnvelope, PsionPluginBenchmarkPromptFormat,
        PsionPluginBenchmarkReceiptPosture, PsionPluginBenchmarkTaskContract,
        PsionPluginDiscoverySelectionTask, PsionPluginExactRouteGrader,
        PsionPluginSelectionDecisionGrader, PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION,
    };
    use psionic_data::{
        build_psion_plugin_contamination_bundle, PsionPluginContaminationBundle,
        PsionPluginRouteLabel,
    };
    use psionic_environments::EnvironmentPackageKey;
    use psionic_eval::{
        BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
        BenchmarkVerificationPolicy,
    };

    #[test]
    fn shared_plugin_benchmark_contract_validates() -> Result<(), Box<dyn std::error::Error>> {
        let contamination = build_psion_plugin_contamination_bundle()?;
        let package = synthetic_discovery_selection_package(&contamination);
        package.validate_against_contamination(&contamination)?;
        Ok(())
    }

    #[test]
    fn shared_plugin_benchmark_contract_rejects_unknown_contamination_lineage(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contamination = build_psion_plugin_contamination_bundle()?;
        let mut package = synthetic_discovery_selection_package(&contamination);
        package.items[0].contamination_attachment.parent_lineage_ids =
            vec![String::from("held_out.missing")];
        package.package_digest = stable_package_digest(&package);
        let error = package
            .validate_against_contamination(&contamination)
            .expect_err("unknown contamination lineage should fail");
        assert!(matches!(
            error,
            PsionPluginBenchmarkPackageError::UnknownContaminationLineage { .. }
        ));
        Ok(())
    }

    fn synthetic_discovery_selection_package(
        contamination: &PsionPluginContaminationBundle,
    ) -> PsionPluginBenchmarkPackageContract {
        let held_out_row = contamination
            .parent_lineage_rows
            .iter()
            .find(|row| {
                row.item_kind == psionic_data::PsionPluginContaminationItemKind::HeldOutEvalRecord
            })
            .expect("held-out lineage row should exist");
        let prompt_format = PsionPluginBenchmarkPromptFormat {
            format_id: String::from("plugin_selection_v1"),
            system_instruction_ref: String::from(
                "prompt://psion/benchmark/system/plugin-selection",
            ),
            user_template_ref: String::from("prompt://psion/benchmark/user/plugin-selection"),
            envelope: PsionPluginBenchmarkPromptEnvelope::StructuredPluginSelectionJson,
            expected_response_format:
                PsionPluginBenchmarkExpectedResponseFormat::PluginSelectionDecisionJson,
            preserve_receipt_boundaries: true,
            detail: String::from(
                "Selection prompts force explicit route and selected-plugin decisions.",
            ),
        };
        let grader = PsionPluginBenchmarkGraderInterface::SelectionDecision(
            PsionPluginSelectionDecisionGrader {
                grader_id: String::from("selection_decision_v1"),
                expected_route: PsionPluginRouteLabel::AnswerInLanguage,
                expected_plugin_ids: Vec::new(),
                order_matters: false,
                detail: String::from(
                    "Selection grader freezes route truth and admitted-plugin choice.",
                ),
            },
        );
        let item = PsionPluginBenchmarkItem {
            item_id: String::from("plugin_selection_case_a"),
            family: PsionPluginBenchmarkFamily::DiscoverySelection,
            prompt_format_id: prompt_format.format_id.clone(),
            grader_id: String::from("selection_decision_v1"),
            prompt_digest: String::from("prompt_digest_selection_a"),
            contamination_attachment: PsionPluginBenchmarkContaminationAttachment {
                contamination_bundle_ref: String::from(
                    psionic_data::PSION_PLUGIN_CONTAMINATION_BUNDLE_REF,
                ),
                contamination_bundle_digest: contamination.bundle_digest.clone(),
                authored_prompt_ref: None,
                parent_lineage_ids: vec![held_out_row.lineage_id.clone()],
                source_case_ids: vec![held_out_row.source_trace.source_case_id.clone()],
                receipt_refs: held_out_row.receipt_refs.clone(),
                detail: String::from(
                    "Item is bound to one held-out lineage row from the contamination bundle.",
                ),
            },
            receipt_posture: PsionPluginBenchmarkReceiptPosture {
                execution_evidence_required: false,
                required_receipt_refs: Vec::new(),
                forbid_unseen_execution_claims: true,
                detail: String::from(
                    "Selection item does not require execution evidence for the direct-answer route.",
                ),
            },
            task: PsionPluginBenchmarkTaskContract::DiscoverySelection(
                PsionPluginDiscoverySelectionTask {
                    admitted_plugin_ids: Vec::new(),
                    direct_answer_allowed: true,
                    expected_route: PsionPluginRouteLabel::AnswerInLanguage,
                    expected_plugin_ids: Vec::new(),
                    negative_case_kind: None,
                },
            ),
            detail: String::from("Synthetic shared-schema validation item."),
        };

        let benchmark_package = BenchmarkPackage::new(
            BenchmarkPackageKey::new("benchmark://openagents/psion/plugin_selection", "v1"),
            "Synthetic PsionPlugin Discovery Selection",
            EnvironmentPackageKey::new("env.psion.plugin.benchmark", "2026.03.22"),
            3,
            BenchmarkAggregationKind::MedianScore,
        )
        .with_cases(vec![BenchmarkCase::new(item.item_id.clone())])
        .with_verification_policy(BenchmarkVerificationPolicy::default());

        let mut package = PsionPluginBenchmarkPackageContract {
            schema_version: String::from(PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION),
            package_id: String::from("psion.plugin.discovery_selection.synthetic.v1"),
            package_family: PsionPluginBenchmarkFamily::DiscoverySelection,
            benchmark_package,
            prompt_formats: vec![prompt_format],
            grader_interfaces: vec![grader],
            items: vec![item],
            summary: String::from(
                "Synthetic shared-schema package used only to validate the common plugin benchmark contract.",
            ),
            package_digest: String::new(),
        };
        package.package_digest = stable_package_digest(&package);
        package
    }

    #[test]
    fn exact_route_grader_is_compatible_with_discovery_selection_items(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contamination = build_psion_plugin_contamination_bundle()?;
        let mut package = synthetic_discovery_selection_package(&contamination);
        package.grader_interfaces = vec![PsionPluginBenchmarkGraderInterface::ExactRoute(
            PsionPluginExactRouteGrader {
                grader_id: String::from("exact_route_v1"),
                expected_route: PsionPluginRouteLabel::AnswerInLanguage,
                detail: String::from("Route-only grading remains valid for some discovery cases."),
            },
        )];
        package.items[0].grader_id = String::from("exact_route_v1");
        package.package_digest = stable_package_digest(&package);
        package.validate_against_contamination(&contamination)?;
        Ok(())
    }
}
