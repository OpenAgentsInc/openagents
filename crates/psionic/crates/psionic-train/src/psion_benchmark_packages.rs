use std::collections::{BTreeMap, BTreeSet};

use psionic_data::{
    PsionBenchmarkIsolationError, PsionExclusionManifest, PsionSourceLifecycleManifest,
};
use psionic_eval::BenchmarkPackage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionBenchmarkEvidenceReceipt, PsionBenchmarkFamily, PsionMetricKind, PsionObservedMetric,
    PsionPhaseGate,
};

/// Stable schema version for the Psion benchmark package contract.
pub const PSION_BENCHMARK_PACKAGE_SCHEMA_VERSION: &str = "psion.benchmark_package_contract.v1";
/// Stable schema version for the Psion benchmark package receipt.
pub const PSION_BENCHMARK_PACKAGE_RECEIPT_SCHEMA_VERSION: &str =
    "psion.benchmark_package_receipt.v1";
/// Stable schema version for the Psion benchmark catalog.
pub const PSION_BENCHMARK_CATALOG_SCHEMA_VERSION: &str = "psion.benchmark_catalog.v1";
/// Stable schema version for the Psion benchmark receipt set.
pub const PSION_BENCHMARK_RECEIPT_SET_SCHEMA_VERSION: &str = "psion.benchmark_receipt_set.v1";

/// Main Psion benchmark package families covered by the shared contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionBenchmarkPackageFamily {
    ArchitectureReasoning,
    NormativeSpecReading,
    EngineeringSpecInterpretation,
    MemorizationVersusReasoning,
    RouteEvaluation,
    RefusalEvaluation,
}

impl PsionBenchmarkPackageFamily {
    #[must_use]
    pub const fn required_families() -> [Self; 6] {
        [
            Self::ArchitectureReasoning,
            Self::NormativeSpecReading,
            Self::EngineeringSpecInterpretation,
            Self::MemorizationVersusReasoning,
            Self::RouteEvaluation,
            Self::RefusalEvaluation,
        ]
    }
}

/// Probe kind for architecture-reasoning benchmark items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionArchitectureReasoningProbeKind {
    DominantConstraint,
    Bottleneck,
    SchedulingBehavior,
    TradeoffAnalysis,
}

/// Probe kind for normative spec-reading benchmark items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionNormativeSpecReadingProbeKind {
    ExactDefinition,
    NamedEdgeCondition,
    NamedGuarantee,
}

/// Probe kind for engineering spec-interpretation benchmark items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionEngineeringSpecInterpretationProbeKind {
    ImplementationImplication,
    AmbiguityRisk,
    UnspecifiedRegion,
    PortabilityConsequence,
}

/// Probe kind for memorization-versus-reasoning benchmark items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionMemorizationVersusReasoningProbeKind {
    AlteredConstraintRecombination,
    UnfamiliarDesignSynthesis,
    HistoricalAnalogyTransfer,
    ParaphraseVariation,
    SpecAdjacentEdgeCase,
}

/// Explicit route classes evaluated inside the Psion learned lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRouteClass {
    AnswerInLanguage,
    AnswerWithUncertainty,
    RequestStructuredInputs,
    DelegateToExactExecutor,
}

impl PsionRouteClass {
    #[must_use]
    pub const fn required_classes() -> [Self; 4] {
        [
            Self::AnswerInLanguage,
            Self::AnswerWithUncertainty,
            Self::RequestStructuredInputs,
            Self::DelegateToExactExecutor,
        ]
    }
}

/// Probe kind for refusal and unsupported-envelope benchmark items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRefusalProbeKind {
    UnsupportedExactnessRequest,
    MissingConstraints,
    OverContextEnvelope,
    FreshnessOrHiddenArtifactDependency,
    UnsupportedGeneralAssistantChat,
}

/// Prompt envelope shared by the bounded Psion benchmark families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionBenchmarkPromptEnvelope {
    SingleTurnText,
    CitedSectionPrompt,
    StructuredRouteDecisionJson,
    StructuredRefusalDecisionJson,
}

/// Expected model response shape for one prompt format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionBenchmarkExpectedResponseFormat {
    FreeformText,
    BoundedExplanationJson,
    RouteDecisionJson,
    RefusalDecisionJson,
}

/// One reusable prompt format for Psion benchmark items.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkPromptFormat {
    /// Stable prompt format identifier.
    pub format_id: String,
    /// Stable system-instruction reference.
    pub system_instruction_ref: String,
    /// Stable user-template reference.
    pub user_template_ref: String,
    /// Prompt envelope.
    pub envelope: PsionBenchmarkPromptEnvelope,
    /// Expected response shape.
    pub expected_response_format: PsionBenchmarkExpectedResponseFormat,
    /// Whether the prompt must preserve explicit source boundaries.
    pub preserve_source_boundaries: bool,
    /// Short explanation of the format.
    pub detail: String,
}

impl PsionBenchmarkPromptFormat {
    fn validate(&self) -> Result<(), PsionBenchmarkPackageError> {
        ensure_nonempty(
            self.format_id.as_str(),
            "psion_benchmark_prompt_format.format_id",
        )?;
        ensure_nonempty(
            self.system_instruction_ref.as_str(),
            "psion_benchmark_prompt_format.system_instruction_ref",
        )?;
        ensure_nonempty(
            self.user_template_ref.as_str(),
            "psion_benchmark_prompt_format.user_template_ref",
        )?;
        ensure_nonempty(self.detail.as_str(), "psion_benchmark_prompt_format.detail")?;
        Ok(())
    }
}

/// One rubric dimension for rubric-backed grading.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkRubricDimension {
    /// Stable dimension id.
    pub dimension_id: String,
    /// Weight in basis points.
    pub weight_bps: u32,
    /// Short explanation of the dimension.
    pub detail: String,
}

impl PsionBenchmarkRubricDimension {
    fn validate(&self) -> Result<(), PsionBenchmarkPackageError> {
        ensure_nonempty(
            self.dimension_id.as_str(),
            "psion_benchmark_rubric_dimension.dimension_id",
        )?;
        validate_bps(
            self.weight_bps,
            "psion_benchmark_rubric_dimension.weight_bps",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_benchmark_rubric_dimension.detail",
        )?;
        Ok(())
    }
}

/// Exact-label grader for package families that can be matched mechanically.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkExactLabelGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Label namespace.
    pub label_namespace: String,
    /// Accepted passing labels.
    pub accepted_labels: Vec<String>,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Rubric-backed grader for reasoning-heavy benchmark families.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkRubricGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Stable rubric reference.
    pub rubric_ref: String,
    /// Minimum passing score in basis points.
    pub minimum_pass_bps: u32,
    /// Weighted rubric dimensions.
    pub dimensions: Vec<PsionBenchmarkRubricDimension>,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Exact route grader for route-selection packages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkExactRouteGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Expected route.
    pub expected_route: PsionRouteClass,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Exact refusal grader for refusal packages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkExactRefusalGrader {
    /// Stable grader id.
    pub grader_id: String,
    /// Accepted refusal reason codes.
    pub accepted_reason_codes: Vec<String>,
    /// Short explanation of the grader.
    pub detail: String,
}

/// Typed grader surface for the main Psion benchmark families.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "grader_kind", rename_all = "snake_case")]
pub enum PsionBenchmarkGraderInterface {
    ExactLabel(PsionBenchmarkExactLabelGrader),
    RubricScore(PsionBenchmarkRubricGrader),
    ExactRoute(PsionBenchmarkExactRouteGrader),
    ExactRefusal(PsionBenchmarkExactRefusalGrader),
}

impl PsionBenchmarkGraderInterface {
    fn validate(&self) -> Result<(), PsionBenchmarkPackageError> {
        match self {
            Self::ExactLabel(grader) => {
                ensure_nonempty(grader.grader_id.as_str(), "exact_label_grader.grader_id")?;
                ensure_nonempty(
                    grader.label_namespace.as_str(),
                    "exact_label_grader.label_namespace",
                )?;
                ensure_nonempty(grader.detail.as_str(), "exact_label_grader.detail")?;
                if grader.accepted_labels.is_empty() {
                    return Err(PsionBenchmarkPackageError::MissingField {
                        field: String::from("exact_label_grader.accepted_labels"),
                    });
                }
            }
            Self::RubricScore(grader) => {
                ensure_nonempty(grader.grader_id.as_str(), "rubric_grader.grader_id")?;
                ensure_nonempty(grader.rubric_ref.as_str(), "rubric_grader.rubric_ref")?;
                validate_bps(grader.minimum_pass_bps, "rubric_grader.minimum_pass_bps")?;
                ensure_nonempty(grader.detail.as_str(), "rubric_grader.detail")?;
                if grader.dimensions.is_empty() {
                    return Err(PsionBenchmarkPackageError::MissingField {
                        field: String::from("rubric_grader.dimensions"),
                    });
                }
                let mut dimension_ids = BTreeSet::new();
                let mut total_weight_bps = 0_u32;
                for dimension in &grader.dimensions {
                    dimension.validate()?;
                    if !dimension_ids.insert(dimension.dimension_id.clone()) {
                        return Err(PsionBenchmarkPackageError::DuplicateRubricDimension {
                            dimension_id: dimension.dimension_id.clone(),
                        });
                    }
                    total_weight_bps = total_weight_bps.saturating_add(dimension.weight_bps);
                }
                if total_weight_bps != 10_000 {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from("rubric_grader.dimensions.weight_bps"),
                        expected: String::from("10000"),
                        actual: total_weight_bps.to_string(),
                    });
                }
            }
            Self::ExactRoute(grader) => {
                ensure_nonempty(grader.grader_id.as_str(), "exact_route_grader.grader_id")?;
                ensure_nonempty(grader.detail.as_str(), "exact_route_grader.detail")?;
            }
            Self::ExactRefusal(grader) => {
                ensure_nonempty(grader.grader_id.as_str(), "exact_refusal_grader.grader_id")?;
                ensure_nonempty(grader.detail.as_str(), "exact_refusal_grader.detail")?;
                if grader.accepted_reason_codes.is_empty() {
                    return Err(PsionBenchmarkPackageError::MissingField {
                        field: String::from("exact_refusal_grader.accepted_reason_codes"),
                    });
                }
            }
        }
        Ok(())
    }

    fn grader_id(&self) -> &str {
        match self {
            Self::ExactLabel(grader) => grader.grader_id.as_str(),
            Self::RubricScore(grader) => grader.grader_id.as_str(),
            Self::ExactRoute(grader) => grader.grader_id.as_str(),
            Self::ExactRefusal(grader) => grader.grader_id.as_str(),
        }
    }

    fn kind_label(&self) -> &'static str {
        match self {
            Self::ExactLabel(_) => "exact_label",
            Self::RubricScore(_) => "rubric_score",
            Self::ExactRoute(_) => "exact_route",
            Self::ExactRefusal(_) => "exact_refusal",
        }
    }
}

/// Family-specific task contract for one benchmark item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "task_kind", rename_all = "snake_case")]
pub enum PsionBenchmarkTaskContract {
    ArchitectureReasoning {
        target_architecture: String,
        workload_ref: String,
        probe_kind: PsionArchitectureReasoningProbeKind,
        dominant_constraint: String,
        explicit_assumptions_required: bool,
        expected_focus: String,
    },
    NormativeSpecReading {
        normative_source_ref: String,
        required_section_anchor: String,
        probe_kind: PsionNormativeSpecReadingProbeKind,
        expected_fact: String,
        grounded_reading_required: bool,
        engineering_inference_forbidden: bool,
    },
    EngineeringSpecInterpretation {
        normative_source_ref: String,
        required_section_anchor: String,
        probe_kind: PsionEngineeringSpecInterpretationProbeKind,
        implementation_target: String,
        expected_consequence: String,
        normative_boundary_required: bool,
        explicit_uncertainty_required: bool,
        unsupported_certainty_forbidden: bool,
    },
    MemorizationVersusReasoning {
        seed_fact_ref: String,
        perturbation_ref: String,
        probe_kind: PsionMemorizationVersusReasoningProbeKind,
        expected_transfer: String,
        recombination_required: bool,
        surface_form_shift_required: bool,
        verbatim_recall_forbidden: bool,
    },
    RouteEvaluation {
        route_class: PsionRouteClass,
        route_boundary_ref: String,
        required_signal: String,
        structured_input_schema_ref: Option<String>,
        uncertainty_required: bool,
    },
    RefusalEvaluation {
        expected_reason_code: String,
        refusal_boundary_ref: String,
        probe_kind: PsionRefusalProbeKind,
        capability_region_id: String,
        unsupported_region_evidence_ref: String,
        claim_boundary_required: bool,
    },
}

impl PsionBenchmarkTaskContract {
    fn validate_for_family(
        &self,
        family: PsionBenchmarkPackageFamily,
    ) -> Result<(), PsionBenchmarkPackageError> {
        match (family, self) {
            (
                PsionBenchmarkPackageFamily::ArchitectureReasoning,
                Self::ArchitectureReasoning {
                    target_architecture,
                    workload_ref,
                    dominant_constraint,
                    explicit_assumptions_required,
                    expected_focus,
                    ..
                },
            ) => {
                ensure_nonempty(
                    target_architecture.as_str(),
                    "architecture_reasoning.target_architecture",
                )?;
                ensure_nonempty(workload_ref.as_str(), "architecture_reasoning.workload_ref")?;
                ensure_nonempty(
                    dominant_constraint.as_str(),
                    "architecture_reasoning.dominant_constraint",
                )?;
                if !explicit_assumptions_required {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from("architecture_reasoning.explicit_assumptions_required"),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
                ensure_nonempty(
                    expected_focus.as_str(),
                    "architecture_reasoning.expected_focus",
                )?;
            }
            (
                PsionBenchmarkPackageFamily::NormativeSpecReading,
                Self::NormativeSpecReading {
                    normative_source_ref,
                    required_section_anchor,
                    expected_fact,
                    grounded_reading_required,
                    engineering_inference_forbidden,
                    ..
                },
            ) => {
                ensure_nonempty(
                    normative_source_ref.as_str(),
                    "normative_spec_reading.normative_source_ref",
                )?;
                ensure_nonempty(
                    required_section_anchor.as_str(),
                    "normative_spec_reading.required_section_anchor",
                )?;
                ensure_nonempty(
                    expected_fact.as_str(),
                    "normative_spec_reading.expected_fact",
                )?;
                if !grounded_reading_required {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from("normative_spec_reading.grounded_reading_required"),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
                if !engineering_inference_forbidden {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from(
                            "normative_spec_reading.engineering_inference_forbidden",
                        ),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
            }
            (
                PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                Self::EngineeringSpecInterpretation {
                    normative_source_ref,
                    required_section_anchor,
                    implementation_target,
                    expected_consequence,
                    normative_boundary_required,
                    explicit_uncertainty_required,
                    unsupported_certainty_forbidden,
                    ..
                },
            ) => {
                ensure_nonempty(
                    normative_source_ref.as_str(),
                    "engineering_spec_interpretation.normative_source_ref",
                )?;
                ensure_nonempty(
                    required_section_anchor.as_str(),
                    "engineering_spec_interpretation.required_section_anchor",
                )?;
                ensure_nonempty(
                    implementation_target.as_str(),
                    "engineering_spec_interpretation.implementation_target",
                )?;
                ensure_nonempty(
                    expected_consequence.as_str(),
                    "engineering_spec_interpretation.expected_consequence",
                )?;
                if !normative_boundary_required {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from(
                            "engineering_spec_interpretation.normative_boundary_required",
                        ),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
                if !explicit_uncertainty_required {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from(
                            "engineering_spec_interpretation.explicit_uncertainty_required",
                        ),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
                if !unsupported_certainty_forbidden {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from(
                            "engineering_spec_interpretation.unsupported_certainty_forbidden",
                        ),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
            }
            (
                PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                Self::MemorizationVersusReasoning {
                    seed_fact_ref,
                    perturbation_ref,
                    expected_transfer,
                    recombination_required,
                    surface_form_shift_required,
                    verbatim_recall_forbidden,
                    ..
                },
            ) => {
                ensure_nonempty(
                    seed_fact_ref.as_str(),
                    "memorization_vs_reasoning.seed_fact_ref",
                )?;
                ensure_nonempty(
                    perturbation_ref.as_str(),
                    "memorization_vs_reasoning.perturbation_ref",
                )?;
                ensure_nonempty(
                    expected_transfer.as_str(),
                    "memorization_vs_reasoning.expected_transfer",
                )?;
                if !recombination_required {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from("memorization_vs_reasoning.recombination_required"),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
                if !surface_form_shift_required {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from(
                            "memorization_vs_reasoning.surface_form_shift_required",
                        ),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
                if !verbatim_recall_forbidden {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from("memorization_vs_reasoning.verbatim_recall_forbidden"),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
            }
            (
                PsionBenchmarkPackageFamily::RouteEvaluation,
                Self::RouteEvaluation {
                    route_class,
                    route_boundary_ref,
                    ..
                },
            ) => {
                ensure_nonempty(
                    route_boundary_ref.as_str(),
                    "route_evaluation.route_boundary_ref",
                )?;
                match self {
                    Self::RouteEvaluation {
                        required_signal,
                        structured_input_schema_ref,
                        uncertainty_required,
                        ..
                    } => {
                        ensure_nonempty(
                            required_signal.as_str(),
                            "route_evaluation.required_signal",
                        )?;
                        match route_class {
                            PsionRouteClass::AnswerInLanguage => {
                                if structured_input_schema_ref.is_some() {
                                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                                        field: String::from(
                                            "route_evaluation.structured_input_schema_ref",
                                        ),
                                        expected: String::from("none"),
                                        actual: String::from("some"),
                                    });
                                }
                                if *uncertainty_required {
                                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                                        field: String::from(
                                            "route_evaluation.uncertainty_required",
                                        ),
                                        expected: String::from("false"),
                                        actual: String::from("true"),
                                    });
                                }
                            }
                            PsionRouteClass::AnswerWithUncertainty => {
                                if structured_input_schema_ref.is_some() {
                                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                                        field: String::from(
                                            "route_evaluation.structured_input_schema_ref",
                                        ),
                                        expected: String::from("none"),
                                        actual: String::from("some"),
                                    });
                                }
                                if !*uncertainty_required {
                                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                                        field: String::from(
                                            "route_evaluation.uncertainty_required",
                                        ),
                                        expected: String::from("true"),
                                        actual: String::from("false"),
                                    });
                                }
                            }
                            PsionRouteClass::RequestStructuredInputs => {
                                let Some(schema_ref) = structured_input_schema_ref.as_deref()
                                else {
                                    return Err(PsionBenchmarkPackageError::MissingField {
                                        field: String::from(
                                            "route_evaluation.structured_input_schema_ref",
                                        ),
                                    });
                                };
                                ensure_nonempty(
                                    schema_ref,
                                    "route_evaluation.structured_input_schema_ref",
                                )?;
                                if *uncertainty_required {
                                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                                        field: String::from(
                                            "route_evaluation.uncertainty_required",
                                        ),
                                        expected: String::from("false"),
                                        actual: String::from("true"),
                                    });
                                }
                            }
                            PsionRouteClass::DelegateToExactExecutor => {
                                if structured_input_schema_ref.is_some() {
                                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                                        field: String::from(
                                            "route_evaluation.structured_input_schema_ref",
                                        ),
                                        expected: String::from("none"),
                                        actual: String::from("some"),
                                    });
                                }
                                if *uncertainty_required {
                                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                                        field: String::from(
                                            "route_evaluation.uncertainty_required",
                                        ),
                                        expected: String::from("false"),
                                        actual: String::from("true"),
                                    });
                                }
                            }
                        }
                    }
                    _ => unreachable!("route evaluation branch should only match route tasks"),
                }
            }
            (
                PsionBenchmarkPackageFamily::RefusalEvaluation,
                Self::RefusalEvaluation {
                    expected_reason_code,
                    refusal_boundary_ref,
                    capability_region_id,
                    unsupported_region_evidence_ref,
                    claim_boundary_required,
                    ..
                },
            ) => {
                ensure_nonempty(
                    expected_reason_code.as_str(),
                    "refusal_evaluation.expected_reason_code",
                )?;
                ensure_nonempty(
                    refusal_boundary_ref.as_str(),
                    "refusal_evaluation.refusal_boundary_ref",
                )?;
                ensure_nonempty(
                    capability_region_id.as_str(),
                    "refusal_evaluation.capability_region_id",
                )?;
                ensure_nonempty(
                    unsupported_region_evidence_ref.as_str(),
                    "refusal_evaluation.unsupported_region_evidence_ref",
                )?;
                if !claim_boundary_required {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from("refusal_evaluation.claim_boundary_required"),
                        expected: String::from("true"),
                        actual: String::from("false"),
                    });
                }
            }
            _ => {
                return Err(PsionBenchmarkPackageError::TaskFamilyMismatch {
                    family,
                    task_kind: task_kind_label(self).to_owned(),
                });
            }
        }
        Ok(())
    }
}

/// Contamination-review inputs preserved on one benchmark package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkContaminationInputs {
    /// Benchmark-visible source ids used to build the package.
    pub benchmark_source_ids: Vec<String>,
    /// Held-out source ids consumed by the package.
    pub held_out_source_ids: Vec<String>,
    /// Training-excluded source ids relevant to the package review.
    pub training_excluded_source_ids: Vec<String>,
    /// Stable reference to the near-duplicate review record.
    pub near_duplicate_review_ref: String,
    /// Short explanation of the contamination posture.
    pub detail: String,
    /// Stable digest over the contamination inputs.
    pub contamination_input_digest: String,
}

impl PsionBenchmarkContaminationInputs {
    fn validate_against_context(
        &self,
        lifecycle: &PsionSourceLifecycleManifest,
        exclusion: &PsionExclusionManifest,
    ) -> Result<(), PsionBenchmarkPackageError> {
        ensure_nonempty(
            self.near_duplicate_review_ref.as_str(),
            "benchmark_contamination_inputs.near_duplicate_review_ref",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "benchmark_contamination_inputs.detail",
        )?;
        if self.benchmark_source_ids.is_empty() && self.held_out_source_ids.is_empty() {
            return Err(PsionBenchmarkPackageError::MissingField {
                field: String::from(
                    "benchmark_contamination_inputs.benchmark_or_held_out_source_ids",
                ),
            });
        }
        reject_duplicate_strings(
            self.benchmark_source_ids.as_slice(),
            "benchmark_contamination_inputs.benchmark_source_ids",
        )?;
        reject_duplicate_strings(
            self.held_out_source_ids.as_slice(),
            "benchmark_contamination_inputs.held_out_source_ids",
        )?;
        reject_duplicate_strings(
            self.training_excluded_source_ids.as_slice(),
            "benchmark_contamination_inputs.training_excluded_source_ids",
        )?;
        exclusion
            .assert_source_ids_allowed(
                lifecycle,
                psionic_data::PsionLoaderSurface::BenchmarkPackage,
                self.benchmark_source_ids.as_slice(),
            )
            .map_err(PsionBenchmarkPackageError::BenchmarkIsolation)?;
        exclusion
            .assert_source_ids_allowed(
                lifecycle,
                psionic_data::PsionLoaderSurface::BenchmarkPackage,
                self.held_out_source_ids.as_slice(),
            )
            .map_err(PsionBenchmarkPackageError::BenchmarkIsolation)?;
        let held_out_source_ids = exclusion
            .held_out_source_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for source_id in &self.held_out_source_ids {
            if !held_out_source_ids.contains(source_id.as_str()) {
                return Err(PsionBenchmarkPackageError::UnknownHeldOutSource {
                    source_id: source_id.clone(),
                });
            }
        }
        let training_excluded_source_ids = exclusion
            .training_excluded_source_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for source_id in &self.training_excluded_source_ids {
            if !training_excluded_source_ids.contains(source_id.as_str()) {
                return Err(PsionBenchmarkPackageError::UnknownTrainingExcludedSource {
                    source_id: source_id.clone(),
                });
            }
        }
        if self.contamination_input_digest != stable_contamination_input_digest(self) {
            return Err(PsionBenchmarkPackageError::DigestMismatch {
                kind: String::from("benchmark_contamination_inputs"),
            });
        }
        Ok(())
    }
}

/// Records contamination-review inputs for one Psion benchmark package.
pub fn record_psion_benchmark_contamination_inputs(
    benchmark_source_ids: Vec<String>,
    held_out_source_ids: Vec<String>,
    training_excluded_source_ids: Vec<String>,
    near_duplicate_review_ref: impl Into<String>,
    detail: impl Into<String>,
) -> Result<PsionBenchmarkContaminationInputs, PsionBenchmarkPackageError> {
    let mut inputs = PsionBenchmarkContaminationInputs {
        benchmark_source_ids,
        held_out_source_ids,
        training_excluded_source_ids,
        near_duplicate_review_ref: near_duplicate_review_ref.into(),
        detail: detail.into(),
        contamination_input_digest: String::new(),
    };
    inputs.contamination_input_digest = stable_contamination_input_digest(&inputs);
    ensure_nonempty(
        inputs.near_duplicate_review_ref.as_str(),
        "benchmark_contamination_inputs.near_duplicate_review_ref",
    )?;
    ensure_nonempty(
        inputs.detail.as_str(),
        "benchmark_contamination_inputs.detail",
    )?;
    Ok(inputs)
}

/// One Psion benchmark item under the shared contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkItem {
    /// Stable item id.
    pub item_id: String,
    /// Package family.
    pub family: PsionBenchmarkPackageFamily,
    /// Prompt format used for the item.
    pub prompt_format_id: String,
    /// Grader interface used for the item.
    pub grader_id: String,
    /// Stable digest over the prompt payload.
    pub prompt_digest: String,
    /// Source ids used to build or label the item.
    pub source_ids: Vec<String>,
    /// Family-specific task contract.
    pub task: PsionBenchmarkTaskContract,
    /// Short explanation of the item.
    pub detail: String,
}

impl PsionBenchmarkItem {
    fn validate(&self) -> Result<(), PsionBenchmarkPackageError> {
        ensure_nonempty(self.item_id.as_str(), "psion_benchmark_item.item_id")?;
        ensure_nonempty(
            self.prompt_format_id.as_str(),
            "psion_benchmark_item.prompt_format_id",
        )?;
        ensure_nonempty(self.grader_id.as_str(), "psion_benchmark_item.grader_id")?;
        ensure_nonempty(
            self.prompt_digest.as_str(),
            "psion_benchmark_item.prompt_digest",
        )?;
        ensure_nonempty(self.detail.as_str(), "psion_benchmark_item.detail")?;
        if self.source_ids.is_empty() {
            return Err(PsionBenchmarkPackageError::MissingField {
                field: String::from("psion_benchmark_item.source_ids"),
            });
        }
        reject_duplicate_strings(
            self.source_ids.as_slice(),
            "psion_benchmark_item.source_ids",
        )?;
        self.task.validate_for_family(self.family)?;
        Ok(())
    }
}

/// One benchmark package built on the shared Psion contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionBenchmarkPackageContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable package id.
    pub package_id: String,
    /// Psion benchmark family covered by the package.
    pub package_family: PsionBenchmarkPackageFamily,
    /// Acceptance-matrix family this package feeds when one exists.
    pub acceptance_family: Option<PsionBenchmarkFamily>,
    /// Generic eval benchmark package bound to the contract.
    pub benchmark_package: BenchmarkPackage,
    /// Prompt formats used by the package.
    pub prompt_formats: Vec<PsionBenchmarkPromptFormat>,
    /// Grader interfaces used by the package.
    pub grader_interfaces: Vec<PsionBenchmarkGraderInterface>,
    /// Contamination-review inputs preserved for the package.
    pub contamination_inputs: PsionBenchmarkContaminationInputs,
    /// Benchmark items.
    pub items: Vec<PsionBenchmarkItem>,
    /// Short explanation of the package.
    pub summary: String,
    /// Stable digest over the package.
    pub package_digest: String,
}

impl PsionBenchmarkPackageContract {
    /// Validates one package against the benchmark-isolation context.
    pub fn validate_against_context(
        &self,
        lifecycle: &PsionSourceLifecycleManifest,
        exclusion: &PsionExclusionManifest,
    ) -> Result<(), PsionBenchmarkPackageError> {
        self.validate_shape()?;
        exclusion
            .validate_against_lifecycle(lifecycle)
            .map_err(PsionBenchmarkPackageError::BenchmarkIsolation)?;
        self.contamination_inputs
            .validate_against_context(lifecycle, exclusion)?;

        let expected_acceptance = expected_acceptance_family(self.package_family);
        if self.acceptance_family != expected_acceptance {
            return Err(PsionBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_benchmark_package.acceptance_family"),
                expected: expected_acceptance
                    .map(|family| format!("{family:?}"))
                    .unwrap_or_else(|| String::from("none")),
                actual: self
                    .acceptance_family
                    .map(|family| format!("{family:?}"))
                    .unwrap_or_else(|| String::from("none")),
            });
        }
        self.benchmark_package
            .validate()
            .map_err(PsionBenchmarkPackageError::EvalRuntime)?;
        if self.benchmark_package.cases.len() != self.items.len() {
            return Err(PsionBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_benchmark_package.items"),
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
                return Err(PsionBenchmarkPackageError::DuplicatePromptFormat {
                    format_id: format.format_id.clone(),
                });
            }
        }
        let mut graders = BTreeMap::new();
        for grader in &self.grader_interfaces {
            grader.validate()?;
            if graders.insert(grader.grader_id(), grader).is_some() {
                return Err(PsionBenchmarkPackageError::DuplicateGrader {
                    grader_id: grader.grader_id().to_owned(),
                });
            }
        }

        let package_source_ids = self
            .contamination_inputs
            .benchmark_source_ids
            .iter()
            .chain(self.contamination_inputs.held_out_source_ids.iter())
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
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
                return Err(PsionBenchmarkPackageError::FieldMismatch {
                    field: format!("psion_benchmark_package.items[{}].family", item.item_id),
                    expected: format!("{:?}", self.package_family),
                    actual: format!("{:?}", item.family),
                });
            }
            if !seen_item_ids.insert(item.item_id.clone()) {
                return Err(PsionBenchmarkPackageError::DuplicateItem {
                    item_id: item.item_id.clone(),
                });
            }
            if !expected_case_ids.contains(item.item_id.as_str()) {
                return Err(PsionBenchmarkPackageError::UnknownBenchmarkCase {
                    case_id: item.item_id.clone(),
                });
            }
            let prompt_format = prompt_formats
                .get(item.prompt_format_id.as_str())
                .ok_or_else(|| PsionBenchmarkPackageError::UnknownPromptFormat {
                    format_id: item.prompt_format_id.clone(),
                })?;
            let grader = graders.get(item.grader_id.as_str()).ok_or_else(|| {
                PsionBenchmarkPackageError::UnknownGrader {
                    grader_id: item.grader_id.clone(),
                }
            })?;
            for source_id in &item.source_ids {
                if !package_source_ids.contains(source_id.as_str()) {
                    return Err(PsionBenchmarkPackageError::UnknownPackageSource {
                        package_id: self.package_id.clone(),
                        source_id: source_id.clone(),
                    });
                }
            }
            exclusion
                .assert_source_ids_allowed(
                    lifecycle,
                    psionic_data::PsionLoaderSurface::BenchmarkPackage,
                    item.source_ids.as_slice(),
                )
                .map_err(PsionBenchmarkPackageError::BenchmarkIsolation)?;
            validate_item_prompt_compatibility(item, prompt_format)?;
            validate_item_grader_compatibility(item, grader)?;
            validate_item_task_grader_alignment(item, grader)?;
        }

        if self.package_digest != stable_benchmark_package_digest(self) {
            return Err(PsionBenchmarkPackageError::DigestMismatch {
                kind: String::from("psion_benchmark_package"),
            });
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), PsionBenchmarkPackageError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_benchmark_package.schema_version",
        )?;
        if self.schema_version != PSION_BENCHMARK_PACKAGE_SCHEMA_VERSION {
            return Err(PsionBenchmarkPackageError::SchemaVersionMismatch {
                expected: String::from(PSION_BENCHMARK_PACKAGE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.package_id.as_str(),
            "psion_benchmark_package.package_id",
        )?;
        ensure_nonempty(self.summary.as_str(), "psion_benchmark_package.summary")?;
        if self.prompt_formats.is_empty() {
            return Err(PsionBenchmarkPackageError::MissingField {
                field: String::from("psion_benchmark_package.prompt_formats"),
            });
        }
        if self.grader_interfaces.is_empty() {
            return Err(PsionBenchmarkPackageError::MissingField {
                field: String::from("psion_benchmark_package.grader_interfaces"),
            });
        }
        if self.items.is_empty() {
            return Err(PsionBenchmarkPackageError::MissingField {
                field: String::from("psion_benchmark_package.items"),
            });
        }
        Ok(())
    }
}

/// Grader-shape breakdown preserved on one benchmark receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkGraderSummary {
    /// Number of exact-label graded items.
    pub exact_label_item_count: u32,
    /// Number of rubric-graded items.
    pub rubric_item_count: u32,
    /// Number of exact-route graded items.
    pub exact_route_item_count: u32,
    /// Number of exact-refusal graded items.
    pub exact_refusal_item_count: u32,
}

/// Receipt output for one benchmark package under the shared contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkPackageReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Phase the receipt is suitable for.
    pub phase: PsionPhaseGate,
    /// Stable package id.
    pub package_id: String,
    /// Stable package digest.
    pub package_digest: String,
    /// Benchmark family covered by the receipt.
    pub package_family: PsionBenchmarkPackageFamily,
    /// Acceptance family fed by the receipt.
    pub acceptance_family: Option<PsionBenchmarkFamily>,
    /// Stable contamination-input digest.
    pub contamination_input_digest: String,
    /// Number of items covered by the receipt.
    pub item_count: u32,
    /// Grader-shape summary.
    pub grader_summary: PsionBenchmarkGraderSummary,
    /// Observed metrics.
    pub observed_metrics: Vec<PsionObservedMetric>,
    /// Acceptance-ready benchmark evidence receipt when the family maps into the matrix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_receipt: Option<PsionBenchmarkEvidenceReceipt>,
    /// Short explanation of the receipt.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionBenchmarkPackageReceipt {
    /// Validates the receipt against one package contract.
    pub fn validate_against_package(
        &self,
        package: &PsionBenchmarkPackageContract,
    ) -> Result<(), PsionBenchmarkPackageError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_benchmark_receipt.schema_version",
        )?;
        if self.schema_version != PSION_BENCHMARK_PACKAGE_RECEIPT_SCHEMA_VERSION {
            return Err(PsionBenchmarkPackageError::SchemaVersionMismatch {
                expected: String::from(PSION_BENCHMARK_PACKAGE_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "psion_benchmark_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.package_id.as_str(),
            "psion_benchmark_receipt.package_id",
        )?;
        ensure_nonempty(
            self.package_digest.as_str(),
            "psion_benchmark_receipt.package_digest",
        )?;
        ensure_nonempty(
            self.contamination_input_digest.as_str(),
            "psion_benchmark_receipt.contamination_input_digest",
        )?;
        ensure_nonempty(self.summary.as_str(), "psion_benchmark_receipt.summary")?;
        check_string_match(
            self.package_id.as_str(),
            package.package_id.as_str(),
            "psion_benchmark_receipt.package_id",
        )?;
        check_string_match(
            self.package_digest.as_str(),
            package.package_digest.as_str(),
            "psion_benchmark_receipt.package_digest",
        )?;
        check_string_match(
            self.contamination_input_digest.as_str(),
            package
                .contamination_inputs
                .contamination_input_digest
                .as_str(),
            "psion_benchmark_receipt.contamination_input_digest",
        )?;
        if self.package_family != package.package_family {
            return Err(PsionBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_benchmark_receipt.package_family"),
                expected: format!("{:?}", package.package_family),
                actual: format!("{:?}", self.package_family),
            });
        }
        if self.acceptance_family != package.acceptance_family {
            return Err(PsionBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_benchmark_receipt.acceptance_family"),
                expected: package
                    .acceptance_family
                    .map(|family| format!("{family:?}"))
                    .unwrap_or_else(|| String::from("none")),
                actual: self
                    .acceptance_family
                    .map(|family| format!("{family:?}"))
                    .unwrap_or_else(|| String::from("none")),
            });
        }
        if self.item_count != package.items.len() as u32 {
            return Err(PsionBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_benchmark_receipt.item_count"),
                expected: package.items.len().to_string(),
                actual: self.item_count.to_string(),
            });
        }
        let expected_grader_summary = expected_grader_summary(package)?;
        if self.grader_summary != expected_grader_summary {
            return Err(PsionBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_benchmark_receipt.grader_summary"),
                expected: String::from("derived from package grader assignments"),
                actual: String::from("grader summary drifted from the package"),
            });
        }

        let mut metric_kinds = BTreeSet::new();
        for metric in &self.observed_metrics {
            if !metric_kinds.insert(metric.metric_kind) {
                return Err(PsionBenchmarkPackageError::DuplicateObservedMetric {
                    metric_kind: metric.metric_kind,
                });
            }
            validate_bps(
                metric.observed_bps,
                "psion_benchmark_receipt.observed_metrics[].observed_bps",
            )?;
            validate_bps(
                metric.regression_from_baseline_bps,
                "psion_benchmark_receipt.observed_metrics[].regression_from_baseline_bps",
            )?;
        }
        for required_metric in required_metric_kinds(package.package_family) {
            if !metric_kinds.contains(&required_metric) {
                return Err(PsionBenchmarkPackageError::MissingRequiredMetric {
                    family: package.package_family,
                    metric_kind: *required_metric,
                });
            }
        }

        match (&package.acceptance_family, &self.evidence_receipt) {
            (Some(expected_family), Some(evidence_receipt)) => {
                if evidence_receipt.phase != self.phase {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from("psion_benchmark_receipt.evidence_receipt.phase"),
                        expected: format!("{:?}", self.phase),
                        actual: format!("{:?}", evidence_receipt.phase),
                    });
                }
                if evidence_receipt.family != *expected_family {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from("psion_benchmark_receipt.evidence_receipt.family"),
                        expected: format!("{expected_family:?}"),
                        actual: format!("{:?}", evidence_receipt.family),
                    });
                }
                check_string_match(
                    evidence_receipt.benchmark_artifact_id.as_str(),
                    package.package_id.as_str(),
                    "psion_benchmark_receipt.evidence_receipt.benchmark_artifact_id",
                )?;
                check_string_match(
                    evidence_receipt.benchmark_artifact_digest.as_str(),
                    package.package_digest.as_str(),
                    "psion_benchmark_receipt.evidence_receipt.benchmark_artifact_digest",
                )?;
                if evidence_receipt.metrics != self.observed_metrics {
                    return Err(PsionBenchmarkPackageError::FieldMismatch {
                        field: String::from("psion_benchmark_receipt.evidence_receipt.metrics"),
                        expected: String::from("metrics must match the receipt observed_metrics"),
                        actual: String::from("metrics drifted"),
                    });
                }
                ensure_nonempty(
                    evidence_receipt.summary.as_str(),
                    "psion_benchmark_receipt.evidence_receipt.summary",
                )?;
            }
            (Some(_), None) => {
                return Err(PsionBenchmarkPackageError::MissingField {
                    field: String::from("psion_benchmark_receipt.evidence_receipt"),
                });
            }
            (None, Some(_)) => {
                return Err(PsionBenchmarkPackageError::FieldMismatch {
                    field: String::from("psion_benchmark_receipt.evidence_receipt"),
                    expected: String::from("none"),
                    actual: String::from("present"),
                });
            }
            (None, None) => {}
        }

        if self.receipt_digest != stable_benchmark_receipt_digest(self) {
            return Err(PsionBenchmarkPackageError::DigestMismatch {
                kind: String::from("psion_benchmark_receipt"),
            });
        }
        Ok(())
    }
}

/// Catalog of benchmark packages under the shared Psion contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionBenchmarkCatalog {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable catalog identifier.
    pub catalog_id: String,
    /// Bound lifecycle schema version.
    pub lifecycle_schema_version: String,
    /// Bound exclusion schema version.
    pub exclusion_schema_version: String,
    /// Included packages.
    pub packages: Vec<PsionBenchmarkPackageContract>,
    /// Short explanation of the catalog.
    pub summary: String,
    /// Stable digest over the catalog.
    pub catalog_digest: String,
}

impl PsionBenchmarkCatalog {
    /// Validates the catalog against the benchmark-isolation context.
    pub fn validate_against_context(
        &self,
        lifecycle: &PsionSourceLifecycleManifest,
        exclusion: &PsionExclusionManifest,
    ) -> Result<(), PsionBenchmarkPackageError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_benchmark_catalog.schema_version",
        )?;
        if self.schema_version != PSION_BENCHMARK_CATALOG_SCHEMA_VERSION {
            return Err(PsionBenchmarkPackageError::SchemaVersionMismatch {
                expected: String::from(PSION_BENCHMARK_CATALOG_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.catalog_id.as_str(),
            "psion_benchmark_catalog.catalog_id",
        )?;
        ensure_nonempty(
            self.lifecycle_schema_version.as_str(),
            "psion_benchmark_catalog.lifecycle_schema_version",
        )?;
        ensure_nonempty(
            self.exclusion_schema_version.as_str(),
            "psion_benchmark_catalog.exclusion_schema_version",
        )?;
        ensure_nonempty(self.summary.as_str(), "psion_benchmark_catalog.summary")?;
        check_string_match(
            self.lifecycle_schema_version.as_str(),
            lifecycle.schema_version.as_str(),
            "psion_benchmark_catalog.lifecycle_schema_version",
        )?;
        check_string_match(
            self.exclusion_schema_version.as_str(),
            exclusion.schema_version.as_str(),
            "psion_benchmark_catalog.exclusion_schema_version",
        )?;
        if self.packages.is_empty() {
            return Err(PsionBenchmarkPackageError::MissingField {
                field: String::from("psion_benchmark_catalog.packages"),
            });
        }
        let mut package_ids = BTreeSet::new();
        let mut covered_families = BTreeSet::new();
        for package in &self.packages {
            if !package_ids.insert(package.package_id.clone()) {
                return Err(PsionBenchmarkPackageError::DuplicatePackage {
                    package_id: package.package_id.clone(),
                });
            }
            covered_families.insert(package.package_family);
            package.validate_against_context(lifecycle, exclusion)?;
        }
        for family in PsionBenchmarkPackageFamily::required_families() {
            if !covered_families.contains(&family) {
                return Err(PsionBenchmarkPackageError::MissingPackageFamily { family });
            }
        }
        if self.catalog_digest != stable_benchmark_catalog_digest(self) {
            return Err(PsionBenchmarkPackageError::DigestMismatch {
                kind: String::from("psion_benchmark_catalog"),
            });
        }
        Ok(())
    }
}

/// Receipt set over the shared Psion benchmark catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkReceiptSet {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt-set identifier.
    pub receipt_set_id: String,
    /// Bound benchmark catalog digest.
    pub catalog_digest: String,
    /// Package receipts in deterministic order.
    pub receipts: Vec<PsionBenchmarkPackageReceipt>,
    /// Short explanation of the receipt set.
    pub summary: String,
    /// Stable digest over the receipt set.
    pub receipt_set_digest: String,
}

impl PsionBenchmarkReceiptSet {
    /// Validates the receipt set against the canonical catalog.
    pub fn validate_against_catalog(
        &self,
        catalog: &PsionBenchmarkCatalog,
        lifecycle: &PsionSourceLifecycleManifest,
        exclusion: &PsionExclusionManifest,
    ) -> Result<(), PsionBenchmarkPackageError> {
        catalog.validate_against_context(lifecycle, exclusion)?;
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_benchmark_receipt_set.schema_version",
        )?;
        if self.schema_version != PSION_BENCHMARK_RECEIPT_SET_SCHEMA_VERSION {
            return Err(PsionBenchmarkPackageError::SchemaVersionMismatch {
                expected: String::from(PSION_BENCHMARK_RECEIPT_SET_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_set_id.as_str(),
            "psion_benchmark_receipt_set.receipt_set_id",
        )?;
        ensure_nonempty(
            self.catalog_digest.as_str(),
            "psion_benchmark_receipt_set.catalog_digest",
        )?;
        ensure_nonempty(self.summary.as_str(), "psion_benchmark_receipt_set.summary")?;
        check_string_match(
            self.catalog_digest.as_str(),
            catalog.catalog_digest.as_str(),
            "psion_benchmark_receipt_set.catalog_digest",
        )?;
        if self.receipts.len() != catalog.packages.len() {
            return Err(PsionBenchmarkPackageError::FieldMismatch {
                field: String::from("psion_benchmark_receipt_set.receipts"),
                expected: catalog.packages.len().to_string(),
                actual: self.receipts.len().to_string(),
            });
        }
        let mut package_ids = BTreeSet::new();
        for receipt in &self.receipts {
            if !package_ids.insert(receipt.package_id.clone()) {
                return Err(PsionBenchmarkPackageError::DuplicatePackageReceipt {
                    package_id: receipt.package_id.clone(),
                });
            }
            let package = catalog
                .packages
                .iter()
                .find(|package| package.package_id == receipt.package_id)
                .ok_or_else(|| PsionBenchmarkPackageError::UnknownReceiptPackage {
                    package_id: receipt.package_id.clone(),
                })?;
            receipt.validate_against_package(package)?;
        }
        if self.receipt_set_digest != stable_benchmark_receipt_set_digest(self) {
            return Err(PsionBenchmarkPackageError::DigestMismatch {
                kind: String::from("psion_benchmark_receipt_set"),
            });
        }
        Ok(())
    }
}

/// Records one benchmark package contract.
pub fn record_psion_benchmark_package(
    package_id: impl Into<String>,
    package_family: PsionBenchmarkPackageFamily,
    benchmark_package: BenchmarkPackage,
    prompt_formats: Vec<PsionBenchmarkPromptFormat>,
    grader_interfaces: Vec<PsionBenchmarkGraderInterface>,
    contamination_inputs: PsionBenchmarkContaminationInputs,
    items: Vec<PsionBenchmarkItem>,
    summary: impl Into<String>,
) -> Result<PsionBenchmarkPackageContract, PsionBenchmarkPackageError> {
    let mut package = PsionBenchmarkPackageContract {
        schema_version: String::from(PSION_BENCHMARK_PACKAGE_SCHEMA_VERSION),
        package_id: package_id.into(),
        package_family,
        acceptance_family: expected_acceptance_family(package_family),
        benchmark_package,
        prompt_formats,
        grader_interfaces,
        contamination_inputs,
        items,
        summary: summary.into(),
        package_digest: String::new(),
    };
    package.package_digest = stable_benchmark_package_digest(&package);
    package.validate_shape()?;
    Ok(package)
}

/// Records one package receipt under the shared benchmark contract.
pub fn record_psion_benchmark_package_receipt(
    receipt_id: impl Into<String>,
    phase: PsionPhaseGate,
    package: &PsionBenchmarkPackageContract,
    observed_metrics: Vec<PsionObservedMetric>,
    summary: impl Into<String>,
) -> Result<PsionBenchmarkPackageReceipt, PsionBenchmarkPackageError> {
    let receipt_id = receipt_id.into();
    let summary = summary.into();
    let evidence_receipt = package
        .acceptance_family
        .map(|family| PsionBenchmarkEvidenceReceipt {
            receipt_id: format!("{receipt_id}-evidence"),
            phase,
            family,
            benchmark_artifact_id: package.package_id.clone(),
            benchmark_artifact_digest: package.package_digest.clone(),
            metrics: observed_metrics.clone(),
            summary: summary.clone(),
        });
    let mut receipt = PsionBenchmarkPackageReceipt {
        schema_version: String::from(PSION_BENCHMARK_PACKAGE_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.clone(),
        phase,
        package_id: package.package_id.clone(),
        package_digest: package.package_digest.clone(),
        package_family: package.package_family,
        acceptance_family: package.acceptance_family,
        contamination_input_digest: package
            .contamination_inputs
            .contamination_input_digest
            .clone(),
        item_count: package.items.len() as u32,
        grader_summary: expected_grader_summary(package)?,
        observed_metrics,
        evidence_receipt,
        summary,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_benchmark_receipt_digest(&receipt);
    receipt.validate_against_package(package)?;
    Ok(receipt)
}

/// Records the full catalog over the shared Psion benchmark contract.
pub fn record_psion_benchmark_catalog(
    catalog_id: impl Into<String>,
    lifecycle: &PsionSourceLifecycleManifest,
    exclusion: &PsionExclusionManifest,
    packages: Vec<PsionBenchmarkPackageContract>,
    summary: impl Into<String>,
) -> Result<PsionBenchmarkCatalog, PsionBenchmarkPackageError> {
    let mut catalog = PsionBenchmarkCatalog {
        schema_version: String::from(PSION_BENCHMARK_CATALOG_SCHEMA_VERSION),
        catalog_id: catalog_id.into(),
        lifecycle_schema_version: lifecycle.schema_version.clone(),
        exclusion_schema_version: exclusion.schema_version.clone(),
        packages,
        summary: summary.into(),
        catalog_digest: String::new(),
    };
    catalog.catalog_digest = stable_benchmark_catalog_digest(&catalog);
    catalog.validate_against_context(lifecycle, exclusion)?;
    Ok(catalog)
}

/// Records the canonical receipt set over the shared Psion benchmark catalog.
pub fn record_psion_benchmark_receipt_set(
    receipt_set_id: impl Into<String>,
    catalog: &PsionBenchmarkCatalog,
    lifecycle: &PsionSourceLifecycleManifest,
    exclusion: &PsionExclusionManifest,
    receipts: Vec<PsionBenchmarkPackageReceipt>,
    summary: impl Into<String>,
) -> Result<PsionBenchmarkReceiptSet, PsionBenchmarkPackageError> {
    let mut receipt_set = PsionBenchmarkReceiptSet {
        schema_version: String::from(PSION_BENCHMARK_RECEIPT_SET_SCHEMA_VERSION),
        receipt_set_id: receipt_set_id.into(),
        catalog_digest: catalog.catalog_digest.clone(),
        receipts,
        summary: summary.into(),
        receipt_set_digest: String::new(),
    };
    receipt_set.receipt_set_digest = stable_benchmark_receipt_set_digest(&receipt_set);
    receipt_set.validate_against_catalog(catalog, lifecycle, exclusion)?;
    Ok(receipt_set)
}

fn expected_acceptance_family(
    package_family: PsionBenchmarkPackageFamily,
) -> Option<PsionBenchmarkFamily> {
    Some(match package_family {
        PsionBenchmarkPackageFamily::ArchitectureReasoning => {
            PsionBenchmarkFamily::ArchitectureReasoning
        }
        PsionBenchmarkPackageFamily::NormativeSpecReading => {
            PsionBenchmarkFamily::NormativeSpecReading
        }
        PsionBenchmarkPackageFamily::EngineeringSpecInterpretation => {
            PsionBenchmarkFamily::EngineeringSpecInterpretation
        }
        PsionBenchmarkPackageFamily::MemorizationVersusReasoning => {
            PsionBenchmarkFamily::MemorizationVersusReasoning
        }
        PsionBenchmarkPackageFamily::RouteEvaluation => PsionBenchmarkFamily::RouteSelection,
        PsionBenchmarkPackageFamily::RefusalEvaluation => {
            PsionBenchmarkFamily::UnsupportedRequestRefusal
        }
    })
}

fn required_metric_kinds(
    package_family: PsionBenchmarkPackageFamily,
) -> &'static [PsionMetricKind] {
    match package_family {
        PsionBenchmarkPackageFamily::ArchitectureReasoning
        | PsionBenchmarkPackageFamily::NormativeSpecReading
        | PsionBenchmarkPackageFamily::EngineeringSpecInterpretation => {
            &[PsionMetricKind::PassRateBps]
        }
        PsionBenchmarkPackageFamily::MemorizationVersusReasoning => &[PsionMetricKind::PassRateBps],
        PsionBenchmarkPackageFamily::RouteEvaluation => {
            &[PsionMetricKind::RouteSelectionAccuracyBps]
        }
        PsionBenchmarkPackageFamily::RefusalEvaluation => &[
            PsionMetricKind::UnsupportedRequestRefusalBps,
            PsionMetricKind::OverrefusalBps,
        ],
    }
}

fn expected_grader_summary(
    package: &PsionBenchmarkPackageContract,
) -> Result<PsionBenchmarkGraderSummary, PsionBenchmarkPackageError> {
    let graders = package
        .grader_interfaces
        .iter()
        .map(|grader| (grader.grader_id(), grader))
        .collect::<BTreeMap<_, _>>();
    let mut summary = PsionBenchmarkGraderSummary {
        exact_label_item_count: 0,
        rubric_item_count: 0,
        exact_route_item_count: 0,
        exact_refusal_item_count: 0,
    };
    for item in &package.items {
        let grader = graders.get(item.grader_id.as_str()).ok_or_else(|| {
            PsionBenchmarkPackageError::UnknownGrader {
                grader_id: item.grader_id.clone(),
            }
        })?;
        match grader {
            PsionBenchmarkGraderInterface::ExactLabel(_) => {
                summary.exact_label_item_count = summary.exact_label_item_count.saturating_add(1);
            }
            PsionBenchmarkGraderInterface::RubricScore(_) => {
                summary.rubric_item_count = summary.rubric_item_count.saturating_add(1);
            }
            PsionBenchmarkGraderInterface::ExactRoute(_) => {
                summary.exact_route_item_count = summary.exact_route_item_count.saturating_add(1);
            }
            PsionBenchmarkGraderInterface::ExactRefusal(_) => {
                summary.exact_refusal_item_count =
                    summary.exact_refusal_item_count.saturating_add(1);
            }
        }
    }
    Ok(summary)
}

fn validate_item_prompt_compatibility(
    item: &PsionBenchmarkItem,
    prompt_format: &PsionBenchmarkPromptFormat,
) -> Result<(), PsionBenchmarkPackageError> {
    match item.family {
        PsionBenchmarkPackageFamily::RouteEvaluation => {
            if prompt_format.expected_response_format
                != PsionBenchmarkExpectedResponseFormat::RouteDecisionJson
            {
                return Err(PsionBenchmarkPackageError::FieldMismatch {
                    field: format!("psion_benchmark_item[{}].prompt_format_id", item.item_id),
                    expected: String::from("route_decision_json"),
                    actual: format!("{:?}", prompt_format.expected_response_format),
                });
            }
        }
        PsionBenchmarkPackageFamily::RefusalEvaluation => {
            if prompt_format.expected_response_format
                != PsionBenchmarkExpectedResponseFormat::RefusalDecisionJson
            {
                return Err(PsionBenchmarkPackageError::FieldMismatch {
                    field: format!("psion_benchmark_item[{}].prompt_format_id", item.item_id),
                    expected: String::from("refusal_decision_json"),
                    actual: format!("{:?}", prompt_format.expected_response_format),
                });
            }
        }
        _ => {
            if matches!(
                prompt_format.expected_response_format,
                PsionBenchmarkExpectedResponseFormat::RouteDecisionJson
                    | PsionBenchmarkExpectedResponseFormat::RefusalDecisionJson
            ) {
                return Err(PsionBenchmarkPackageError::FieldMismatch {
                    field: format!("psion_benchmark_item[{}].prompt_format_id", item.item_id),
                    expected: String::from("freeform or bounded explanation"),
                    actual: format!("{:?}", prompt_format.expected_response_format),
                });
            }
        }
    }
    Ok(())
}

fn validate_item_grader_compatibility(
    item: &PsionBenchmarkItem,
    grader: &PsionBenchmarkGraderInterface,
) -> Result<(), PsionBenchmarkPackageError> {
    match item.family {
        PsionBenchmarkPackageFamily::ArchitectureReasoning
        | PsionBenchmarkPackageFamily::NormativeSpecReading
        | PsionBenchmarkPackageFamily::EngineeringSpecInterpretation
        | PsionBenchmarkPackageFamily::MemorizationVersusReasoning => {
            if !matches!(
                grader,
                PsionBenchmarkGraderInterface::ExactLabel(_)
                    | PsionBenchmarkGraderInterface::RubricScore(_)
            ) {
                return Err(PsionBenchmarkPackageError::GraderFamilyMismatch {
                    family: item.family,
                    grader_kind: grader.kind_label().to_owned(),
                });
            }
        }
        PsionBenchmarkPackageFamily::RouteEvaluation => {
            if !matches!(grader, PsionBenchmarkGraderInterface::ExactRoute(_)) {
                return Err(PsionBenchmarkPackageError::GraderFamilyMismatch {
                    family: item.family,
                    grader_kind: grader.kind_label().to_owned(),
                });
            }
        }
        PsionBenchmarkPackageFamily::RefusalEvaluation => {
            if !matches!(grader, PsionBenchmarkGraderInterface::ExactRefusal(_)) {
                return Err(PsionBenchmarkPackageError::GraderFamilyMismatch {
                    family: item.family,
                    grader_kind: grader.kind_label().to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn validate_item_task_grader_alignment(
    item: &PsionBenchmarkItem,
    grader: &PsionBenchmarkGraderInterface,
) -> Result<(), PsionBenchmarkPackageError> {
    match (&item.task, grader) {
        (
            PsionBenchmarkTaskContract::RouteEvaluation { route_class, .. },
            PsionBenchmarkGraderInterface::ExactRoute(grader),
        ) => {
            if *route_class != grader.expected_route {
                return Err(PsionBenchmarkPackageError::FieldMismatch {
                    field: format!("psion_benchmark_item[{}].task.route_class", item.item_id),
                    expected: format!("{:?}", grader.expected_route),
                    actual: format!("{:?}", route_class),
                });
            }
        }
        (
            PsionBenchmarkTaskContract::RefusalEvaluation {
                expected_reason_code,
                ..
            },
            PsionBenchmarkGraderInterface::ExactRefusal(grader),
        ) => {
            if !grader
                .accepted_reason_codes
                .iter()
                .any(|reason| reason == expected_reason_code)
            {
                return Err(PsionBenchmarkPackageError::FieldMismatch {
                    field: format!(
                        "psion_benchmark_item[{}].task.expected_reason_code",
                        item.item_id
                    ),
                    expected: grader.accepted_reason_codes.join(","),
                    actual: expected_reason_code.clone(),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn task_kind_label(task: &PsionBenchmarkTaskContract) -> &'static str {
    match task {
        PsionBenchmarkTaskContract::ArchitectureReasoning { .. } => "architecture_reasoning",
        PsionBenchmarkTaskContract::NormativeSpecReading { .. } => "normative_spec_reading",
        PsionBenchmarkTaskContract::EngineeringSpecInterpretation { .. } => {
            "engineering_spec_interpretation"
        }
        PsionBenchmarkTaskContract::MemorizationVersusReasoning { .. } => {
            "memorization_versus_reasoning"
        }
        PsionBenchmarkTaskContract::RouteEvaluation { .. } => "route_evaluation",
        PsionBenchmarkTaskContract::RefusalEvaluation { .. } => "refusal_evaluation",
    }
}

fn stable_contamination_input_digest(inputs: &PsionBenchmarkContaminationInputs) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_benchmark_contamination_inputs|");
    for source_id in &inputs.benchmark_source_ids {
        hasher.update(b"|benchmark|");
        hasher.update(source_id.as_bytes());
    }
    for source_id in &inputs.held_out_source_ids {
        hasher.update(b"|held_out|");
        hasher.update(source_id.as_bytes());
    }
    for source_id in &inputs.training_excluded_source_ids {
        hasher.update(b"|training_excluded|");
        hasher.update(source_id.as_bytes());
    }
    hasher.update(b"|review|");
    hasher.update(inputs.near_duplicate_review_ref.as_bytes());
    hasher.update(b"|detail|");
    hasher.update(inputs.detail.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_benchmark_package_digest(package: &PsionBenchmarkPackageContract) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_benchmark_package|");
    hasher.update(package.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(package.package_id.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", package.package_family).as_bytes());
    hasher.update(b"|");
    hasher.update(
        package
            .acceptance_family
            .map(|family| format!("{family:?}"))
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(package.benchmark_package.stable_digest().as_bytes());
    hasher.update(b"|");
    hasher.update(
        package
            .contamination_inputs
            .contamination_input_digest
            .as_bytes(),
    );
    for format in &package.prompt_formats {
        hasher.update(b"|prompt_format|");
        hasher.update(format.format_id.as_bytes());
        hasher.update(b"|");
        hasher.update(format.system_instruction_ref.as_bytes());
        hasher.update(b"|");
        hasher.update(format.user_template_ref.as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", format.envelope).as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", format.expected_response_format).as_bytes());
    }
    for grader in &package.grader_interfaces {
        hasher.update(b"|grader|");
        hasher.update(grader.grader_id().as_bytes());
        hasher.update(b"|");
        hasher.update(grader.kind_label().as_bytes());
        hasher.update(b"|");
        match grader {
            PsionBenchmarkGraderInterface::ExactLabel(grader) => {
                hasher.update(grader.label_namespace.as_bytes());
                for label in &grader.accepted_labels {
                    hasher.update(b"|label|");
                    hasher.update(label.as_bytes());
                }
            }
            PsionBenchmarkGraderInterface::RubricScore(grader) => {
                hasher.update(grader.rubric_ref.as_bytes());
                hasher.update(b"|");
                hasher.update(grader.minimum_pass_bps.to_string().as_bytes());
                for dimension in &grader.dimensions {
                    hasher.update(b"|dimension|");
                    hasher.update(dimension.dimension_id.as_bytes());
                    hasher.update(b"|");
                    hasher.update(dimension.weight_bps.to_string().as_bytes());
                }
            }
            PsionBenchmarkGraderInterface::ExactRoute(grader) => {
                hasher.update(format!("{:?}", grader.expected_route).as_bytes());
            }
            PsionBenchmarkGraderInterface::ExactRefusal(grader) => {
                for code in &grader.accepted_reason_codes {
                    hasher.update(b"|reason|");
                    hasher.update(code.as_bytes());
                }
            }
        }
    }
    for item in &package.items {
        hasher.update(b"|item|");
        hasher.update(item.item_id.as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", item.family).as_bytes());
        hasher.update(b"|");
        hasher.update(item.prompt_format_id.as_bytes());
        hasher.update(b"|");
        hasher.update(item.grader_id.as_bytes());
        hasher.update(b"|");
        hasher.update(item.prompt_digest.as_bytes());
        for source_id in &item.source_ids {
            hasher.update(b"|source|");
            hasher.update(source_id.as_bytes());
        }
        hasher.update(b"|");
        hasher.update(task_kind_label(&item.task).as_bytes());
    }
    hasher.update(b"|summary|");
    hasher.update(package.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_benchmark_receipt_digest(receipt: &PsionBenchmarkPackageReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_benchmark_receipt|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", receipt.phase).as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.package_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.package_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", receipt.package_family).as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.contamination_input_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.item_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(
        receipt
            .grader_summary
            .exact_label_item_count
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .grader_summary
            .rubric_item_count
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .grader_summary
            .exact_route_item_count
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .grader_summary
            .exact_refusal_item_count
            .to_string()
            .as_bytes(),
    );
    for metric in &receipt.observed_metrics {
        hasher.update(b"|metric|");
        hasher.update(format!("{:?}", metric.metric_kind).as_bytes());
        hasher.update(b"|");
        hasher.update(metric.observed_bps.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(metric.regression_from_baseline_bps.to_string().as_bytes());
    }
    if let Some(evidence_receipt) = &receipt.evidence_receipt {
        hasher.update(b"|evidence|");
        hasher.update(evidence_receipt.receipt_id.as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", evidence_receipt.family).as_bytes());
    }
    hasher.update(b"|summary|");
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_benchmark_catalog_digest(catalog: &PsionBenchmarkCatalog) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_benchmark_catalog|");
    hasher.update(catalog.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(catalog.catalog_id.as_bytes());
    hasher.update(b"|");
    hasher.update(catalog.lifecycle_schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(catalog.exclusion_schema_version.as_bytes());
    for package in &catalog.packages {
        hasher.update(b"|package|");
        hasher.update(package.package_digest.as_bytes());
    }
    hasher.update(b"|summary|");
    hasher.update(catalog.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_benchmark_receipt_set_digest(receipt_set: &PsionBenchmarkReceiptSet) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_benchmark_receipt_set|");
    hasher.update(receipt_set.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt_set.receipt_set_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt_set.catalog_digest.as_bytes());
    for receipt in &receipt_set.receipts {
        hasher.update(b"|receipt|");
        hasher.update(receipt.receipt_digest.as_bytes());
    }
    hasher.update(b"|summary|");
    hasher.update(receipt_set.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionBenchmarkPackageError> {
    if value.trim().is_empty() {
        return Err(PsionBenchmarkPackageError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionBenchmarkPackageError> {
    if actual != expected {
        return Err(PsionBenchmarkPackageError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn validate_bps(value: u32, field: &str) -> Result<(), PsionBenchmarkPackageError> {
    if value > 10_000 {
        return Err(PsionBenchmarkPackageError::FieldMismatch {
            field: String::from(field),
            expected: String::from("0..=10000"),
            actual: value.to_string(),
        });
    }
    Ok(())
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionBenchmarkPackageError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(PsionBenchmarkPackageError::FieldMismatch {
                field: String::from(field),
                expected: String::from("unique values"),
                actual: value.clone(),
            });
        }
    }
    Ok(())
}

/// Validation failures for the shared Psion benchmark contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionBenchmarkPackageError {
    /// One required field was missing.
    #[error("Psion benchmark contract is missing `{field}`")]
    MissingField {
        /// Missing field.
        field: String,
    },
    /// One field drifted from the required value.
    #[error("Psion benchmark contract field mismatch on `{field}`: expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// One schema version drifted.
    #[error("Psion benchmark contract schema mismatch: expected `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema.
        expected: String,
        /// Actual schema.
        actual: String,
    },
    /// One prompt format id repeated.
    #[error("Psion benchmark package repeats prompt format `{format_id}`")]
    DuplicatePromptFormat {
        /// Prompt format id.
        format_id: String,
    },
    /// One grader id repeated.
    #[error("Psion benchmark package repeats grader `{grader_id}`")]
    DuplicateGrader {
        /// Grader id.
        grader_id: String,
    },
    /// One item id repeated.
    #[error("Psion benchmark package repeats item `{item_id}`")]
    DuplicateItem {
        /// Item id.
        item_id: String,
    },
    /// One catalog package repeated.
    #[error("Psion benchmark catalog repeats package `{package_id}`")]
    DuplicatePackage {
        /// Package id.
        package_id: String,
    },
    /// One receipt repeated a package id.
    #[error("Psion benchmark receipt set repeats package receipt `{package_id}`")]
    DuplicatePackageReceipt {
        /// Package id.
        package_id: String,
    },
    /// One rubric dimension repeated.
    #[error("Psion benchmark rubric repeats dimension `{dimension_id}`")]
    DuplicateRubricDimension {
        /// Dimension id.
        dimension_id: String,
    },
    /// One observed metric repeated.
    #[error("Psion benchmark receipt repeats observed metric `{metric_kind:?}`")]
    DuplicateObservedMetric {
        /// Metric kind.
        metric_kind: PsionMetricKind,
    },
    /// A package family is missing from the canonical catalog.
    #[error("Psion benchmark catalog is missing required family `{family:?}`")]
    MissingPackageFamily {
        /// Missing family.
        family: PsionBenchmarkPackageFamily,
    },
    /// One item referenced a task contract for the wrong family.
    #[error("Psion benchmark item family `{family:?}` does not admit task `{task_kind}`")]
    TaskFamilyMismatch {
        /// Item family.
        family: PsionBenchmarkPackageFamily,
        /// Task kind label.
        task_kind: String,
    },
    /// One item referenced a grader incompatible with its family.
    #[error("Psion benchmark family `{family:?}` does not admit grader `{grader_kind}`")]
    GraderFamilyMismatch {
        /// Item family.
        family: PsionBenchmarkPackageFamily,
        /// Grader kind label.
        grader_kind: String,
    },
    /// The package referred to a missing benchmark case.
    #[error(
        "Psion benchmark package is missing case `{case_id}` from the generic benchmark package"
    )]
    UnknownBenchmarkCase {
        /// Case id.
        case_id: String,
    },
    /// One item referenced an unknown prompt format.
    #[error("Psion benchmark item references unknown prompt format `{format_id}`")]
    UnknownPromptFormat {
        /// Prompt format id.
        format_id: String,
    },
    /// One item referenced an unknown grader.
    #[error("Psion benchmark item references unknown grader `{grader_id}`")]
    UnknownGrader {
        /// Grader id.
        grader_id: String,
    },
    /// One receipt referenced an unknown package id.
    #[error("Psion benchmark receipt set references unknown package `{package_id}`")]
    UnknownReceiptPackage {
        /// Package id.
        package_id: String,
    },
    /// One item referenced a source outside the package contamination inputs.
    #[error("Psion benchmark package `{package_id}` references source `{source_id}` outside its contamination inputs")]
    UnknownPackageSource {
        /// Package id.
        package_id: String,
        /// Source id.
        source_id: String,
    },
    /// One held-out source id was not in the exclusion manifest.
    #[error(
        "Psion benchmark contamination inputs reference unknown held-out source `{source_id}`"
    )]
    UnknownHeldOutSource {
        /// Source id.
        source_id: String,
    },
    /// One training-excluded source id was not in the exclusion manifest.
    #[error("Psion benchmark contamination inputs reference unknown training-excluded source `{source_id}`")]
    UnknownTrainingExcludedSource {
        /// Source id.
        source_id: String,
    },
    /// One required metric was missing from a receipt.
    #[error("Psion benchmark family `{family:?}` is missing required metric `{metric_kind:?}`")]
    MissingRequiredMetric {
        /// Package family.
        family: PsionBenchmarkPackageFamily,
        /// Missing metric.
        metric_kind: PsionMetricKind,
    },
    /// One digest drifted from the canonical payload.
    #[error("Psion benchmark digest drifted for `{kind}`")]
    DigestMismatch {
        /// Artifact kind.
        kind: String,
    },
    /// Generic eval benchmark package validation failed.
    #[error(transparent)]
    EvalRuntime(#[from] psionic_eval::EvalRuntimeError),
    /// Benchmark isolation validation failed.
    #[error(transparent)]
    BenchmarkIsolation(#[from] PsionBenchmarkIsolationError),
}

#[cfg(test)]
mod tests {
    use super::{
        record_psion_benchmark_catalog, record_psion_benchmark_package,
        record_psion_benchmark_package_receipt, record_psion_benchmark_receipt_set,
        stable_contamination_input_digest, PsionArchitectureReasoningProbeKind,
        PsionBenchmarkContaminationInputs, PsionBenchmarkExactLabelGrader,
        PsionBenchmarkExactRefusalGrader, PsionBenchmarkExactRouteGrader,
        PsionBenchmarkExpectedResponseFormat, PsionBenchmarkGraderInterface, PsionBenchmarkItem,
        PsionBenchmarkPackageContract, PsionBenchmarkPackageError, PsionBenchmarkPackageFamily,
        PsionBenchmarkPromptEnvelope, PsionBenchmarkPromptFormat, PsionBenchmarkRubricDimension,
        PsionBenchmarkRubricGrader, PsionBenchmarkTaskContract,
        PsionEngineeringSpecInterpretationProbeKind, PsionMemorizationVersusReasoningProbeKind,
        PsionNormativeSpecReadingProbeKind, PsionRefusalProbeKind, PsionRouteClass,
    };
    use crate::{PsionBenchmarkFamily, PsionMetricKind, PsionObservedMetric, PsionPhaseGate};
    use psionic_data::{PsionExclusionManifest, PsionSourceLifecycleManifest};
    use psionic_environments::EnvironmentPackageKey;
    use psionic_eval::{BenchmarkAggregationKind, BenchmarkPackage, BenchmarkPackageKey};

    fn lifecycle_manifest() -> PsionSourceLifecycleManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"
        ))
        .expect("lifecycle manifest should parse")
    }

    fn exclusion_manifest() -> PsionExclusionManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/isolation/psion_exclusion_manifest_v1.json"
        ))
        .expect("exclusion manifest should parse")
    }

    fn contamination_inputs(source_ids: &[&str]) -> PsionBenchmarkContaminationInputs {
        let mut inputs = PsionBenchmarkContaminationInputs {
            benchmark_source_ids: source_ids.iter().map(|source| String::from(*source)).collect(),
            held_out_source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
            training_excluded_source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
            near_duplicate_review_ref: String::from("review://psion/benchmark/near-duplicate-v1"),
            detail: String::from(
                "Benchmark package preserves the held-out source ids, training-excluded ids, and near-duplicate review reference required by the isolation contract.",
            ),
            contamination_input_digest: String::new(),
        };
        inputs.contamination_input_digest = stable_contamination_input_digest(&inputs);
        inputs
    }

    fn benchmark_package(package_id: &str, case_ids: &[&str]) -> BenchmarkPackage {
        BenchmarkPackage::new(
            BenchmarkPackageKey::new(package_id, "v1"),
            format!("Display {package_id}"),
            EnvironmentPackageKey::new("env.psion.benchmark", "2026.03.22"),
            3,
            BenchmarkAggregationKind::MedianScore,
        )
        .with_cases(
            case_ids
                .iter()
                .map(|case_id| psionic_eval::BenchmarkCase::new(*case_id))
                .collect(),
        )
    }

    fn explanation_prompt_format() -> PsionBenchmarkPromptFormat {
        PsionBenchmarkPromptFormat {
            format_id: String::from("bounded_explanation_v1"),
            system_instruction_ref: String::from("prompt://psion/benchmark/system/bounded-explanation"),
            user_template_ref: String::from("prompt://psion/benchmark/user/bounded-explanation"),
            envelope: PsionBenchmarkPromptEnvelope::CitedSectionPrompt,
            expected_response_format: PsionBenchmarkExpectedResponseFormat::BoundedExplanationJson,
            preserve_source_boundaries: true,
            detail: String::from(
                "Explanation prompts preserve source boundaries and produce bounded explanation JSON.",
            ),
        }
    }

    fn route_prompt_format() -> PsionBenchmarkPromptFormat {
        PsionBenchmarkPromptFormat {
            format_id: String::from("route_decision_v1"),
            system_instruction_ref: String::from("prompt://psion/benchmark/system/route"),
            user_template_ref: String::from("prompt://psion/benchmark/user/route"),
            envelope: PsionBenchmarkPromptEnvelope::StructuredRouteDecisionJson,
            expected_response_format: PsionBenchmarkExpectedResponseFormat::RouteDecisionJson,
            preserve_source_boundaries: true,
            detail: String::from(
                "Route prompts require a structured route-decision JSON response.",
            ),
        }
    }

    fn refusal_prompt_format() -> PsionBenchmarkPromptFormat {
        PsionBenchmarkPromptFormat {
            format_id: String::from("refusal_decision_v1"),
            system_instruction_ref: String::from("prompt://psion/benchmark/system/refusal"),
            user_template_ref: String::from("prompt://psion/benchmark/user/refusal"),
            envelope: PsionBenchmarkPromptEnvelope::StructuredRefusalDecisionJson,
            expected_response_format: PsionBenchmarkExpectedResponseFormat::RefusalDecisionJson,
            preserve_source_boundaries: true,
            detail: String::from(
                "Refusal prompts require a structured refusal-decision JSON response.",
            ),
        }
    }

    fn rubric_grader() -> PsionBenchmarkGraderInterface {
        PsionBenchmarkGraderInterface::RubricScore(PsionBenchmarkRubricGrader {
            grader_id: String::from("rubric_reasoning_v1"),
            rubric_ref: String::from("rubric://psion/benchmark/reasoning"),
            minimum_pass_bps: 7800,
            dimensions: vec![
                PsionBenchmarkRubricDimension {
                    dimension_id: String::from("correctness"),
                    weight_bps: 6000,
                    detail: String::from("Checks the substantive answer."),
                },
                PsionBenchmarkRubricDimension {
                    dimension_id: String::from("truth_boundary"),
                    weight_bps: 4000,
                    detail: String::from(
                        "Checks explicit assumptions, uncertainty, and normative-versus-inference separation.",
                    ),
                },
            ],
            detail: String::from(
                "Rubric grader supports bounded reasoning labels without collapsing them into one exact string.",
            ),
        })
    }

    fn exact_label_grader() -> PsionBenchmarkGraderInterface {
        PsionBenchmarkGraderInterface::ExactLabel(PsionBenchmarkExactLabelGrader {
            grader_id: String::from("exact_label_v1"),
            label_namespace: String::from("psion.spec.reading"),
            accepted_labels: vec![String::from("pass"), String::from("boundary_clear")],
            detail: String::from("Exact-label grader supports deterministic label-based grading."),
        })
    }

    fn exact_route_grader(
        grader_id: &str,
        expected_route: PsionRouteClass,
    ) -> PsionBenchmarkGraderInterface {
        PsionBenchmarkGraderInterface::ExactRoute(PsionBenchmarkExactRouteGrader {
            grader_id: String::from(grader_id),
            expected_route,
            detail: String::from("Route grader requires the declared route exactly."),
        })
    }

    fn exact_refusal_grader(
        grader_id: &str,
        accepted_reason_code: &str,
    ) -> PsionBenchmarkGraderInterface {
        PsionBenchmarkGraderInterface::ExactRefusal(PsionBenchmarkExactRefusalGrader {
            grader_id: String::from(grader_id),
            accepted_reason_codes: vec![String::from(accepted_reason_code)],
            detail: String::from(
                "Refusal grader requires one of the admitted refusal codes exactly.",
            ),
        })
    }

    fn package_contracts() -> Result<Vec<PsionBenchmarkPackageContract>, Box<dyn std::error::Error>>
    {
        Ok(vec![
            record_psion_benchmark_package(
                "psion_architecture_reasoning_benchmark_v1",
                PsionBenchmarkPackageFamily::ArchitectureReasoning,
                benchmark_package(
                    "psion_architecture_reasoning_benchmark_v1",
                    &[
                        "arch-case-constraint",
                        "arch-case-bottleneck",
                        "arch-case-scheduling",
                        "arch-case-tradeoff",
                    ],
                ),
                vec![explanation_prompt_format()],
                vec![rubric_grader()],
                contamination_inputs(&["spec_quiz_eval_pack_v1"]),
                vec![
                    PsionBenchmarkItem {
                        item_id: String::from("arch-case-constraint"),
                        family: PsionBenchmarkPackageFamily::ArchitectureReasoning,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("rubric_reasoning_v1"),
                        prompt_digest: String::from("arch-prompt-digest-1"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::ArchitectureReasoning {
                            target_architecture: String::from("bounded_three_stage_pipeline"),
                            workload_ref: String::from("workload://psion/ingest/cache-sensitive"),
                            probe_kind: PsionArchitectureReasoningProbeKind::DominantConstraint,
                            dominant_constraint: String::from("memory bandwidth"),
                            explicit_assumptions_required: true,
                            expected_focus: String::from("dominant resource constraint"),
                        },
                        detail: String::from(
                            "Architecture constraint item requires explicit assumptions about the bounded ingest workload before naming the dominant constraint.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("arch-case-bottleneck"),
                        family: PsionBenchmarkPackageFamily::ArchitectureReasoning,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("rubric_reasoning_v1"),
                        prompt_digest: String::from("arch-prompt-digest-2"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::ArchitectureReasoning {
                            target_architecture: String::from("bounded_three_stage_pipeline"),
                            workload_ref: String::from("workload://psion/ingest/high-fanout"),
                            probe_kind: PsionArchitectureReasoningProbeKind::Bottleneck,
                            dominant_constraint: String::from("queueing pressure"),
                            explicit_assumptions_required: true,
                            expected_focus: String::from("steady-state bottleneck"),
                        },
                        detail: String::from(
                            "Architecture bottleneck item requires the answer to surface the limiting stage under the stated assumptions instead of generic optimization advice.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("arch-case-scheduling"),
                        family: PsionBenchmarkPackageFamily::ArchitectureReasoning,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("rubric_reasoning_v1"),
                        prompt_digest: String::from("arch-prompt-digest-3"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::ArchitectureReasoning {
                            target_architecture: String::from("bounded_three_stage_pipeline"),
                            workload_ref: String::from("workload://psion/ingest/bursty-latency"),
                            probe_kind: PsionArchitectureReasoningProbeKind::SchedulingBehavior,
                            dominant_constraint: String::from("tail-latency budget"),
                            explicit_assumptions_required: true,
                            expected_focus: String::from("scheduler behavior under burst load"),
                        },
                        detail: String::from(
                            "Architecture scheduling item checks whether the answer reasons about bounded scheduling behavior rather than only naming hardware resources.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("arch-case-tradeoff"),
                        family: PsionBenchmarkPackageFamily::ArchitectureReasoning,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("rubric_reasoning_v1"),
                        prompt_digest: String::from("arch-prompt-digest-4"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::ArchitectureReasoning {
                            target_architecture: String::from("bounded_three_stage_pipeline"),
                            workload_ref: String::from("workload://psion/ingest/cost-capped"),
                            probe_kind: PsionArchitectureReasoningProbeKind::TradeoffAnalysis,
                            dominant_constraint: String::from("cost ceiling"),
                            explicit_assumptions_required: true,
                            expected_focus: String::from("throughput versus resilience tradeoff"),
                        },
                        detail: String::from(
                            "Architecture tradeoff item checks whether the answer keeps the stated assumptions explicit while reasoning about bounded tradeoffs.",
                        ),
                    },
                ],
                "Architecture reasoning benchmark package uses typed items that explicitly cover dominant constraints, bottlenecks, scheduling behavior, and tradeoff analysis under stated assumptions.",
            )?,
            record_psion_benchmark_package(
                "psion_normative_spec_benchmark_v1",
                PsionBenchmarkPackageFamily::NormativeSpecReading,
                benchmark_package(
                    "psion_normative_spec_benchmark_v1",
                    &[
                        "spec-case-definition",
                        "spec-case-edge-condition",
                        "spec-case-guarantee",
                    ],
                ),
                vec![explanation_prompt_format()],
                vec![exact_label_grader()],
                contamination_inputs(&["spec_quiz_eval_pack_v1", "wasm_core_spec_release_2"]),
                vec![
                    PsionBenchmarkItem {
                        item_id: String::from("spec-case-definition"),
                        family: PsionBenchmarkPackageFamily::NormativeSpecReading,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("exact_label_v1"),
                        prompt_digest: String::from("spec-prompt-digest-1"),
                        source_ids: vec![
                            String::from("spec_quiz_eval_pack_v1"),
                            String::from("wasm_core_spec_release_2"),
                        ],
                        task: PsionBenchmarkTaskContract::NormativeSpecReading {
                            normative_source_ref: String::from("wasm://core/validation"),
                            required_section_anchor: String::from("2.5.1"),
                            probe_kind: PsionNormativeSpecReadingProbeKind::ExactDefinition,
                            expected_fact: String::from("definition of validation context"),
                            grounded_reading_required: true,
                            engineering_inference_forbidden: true,
                        },
                        detail: String::from(
                            "Normative definition item checks exact source-grounded reading of the named term without allowing later engineering inference to stand in for the quoted fact.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("spec-case-edge-condition"),
                        family: PsionBenchmarkPackageFamily::NormativeSpecReading,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("exact_label_v1"),
                        prompt_digest: String::from("spec-prompt-digest-2"),
                        source_ids: vec![
                            String::from("spec_quiz_eval_pack_v1"),
                            String::from("wasm_core_spec_release_2"),
                        ],
                        task: PsionBenchmarkTaskContract::NormativeSpecReading {
                            normative_source_ref: String::from("wasm://core/validation"),
                            required_section_anchor: String::from("2.5.4"),
                            probe_kind: PsionNormativeSpecReadingProbeKind::NamedEdgeCondition,
                            expected_fact: String::from("named malformed-module condition"),
                            grounded_reading_required: true,
                            engineering_inference_forbidden: true,
                        },
                        detail: String::from(
                            "Normative edge-condition item checks whether the answer names the specific edge case stated in the text rather than smoothing it into generic advice.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("spec-case-guarantee"),
                        family: PsionBenchmarkPackageFamily::NormativeSpecReading,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("exact_label_v1"),
                        prompt_digest: String::from("spec-prompt-digest-3"),
                        source_ids: vec![
                            String::from("spec_quiz_eval_pack_v1"),
                            String::from("wasm_core_spec_release_2"),
                        ],
                        task: PsionBenchmarkTaskContract::NormativeSpecReading {
                            normative_source_ref: String::from("wasm://core/execution"),
                            required_section_anchor: String::from("4.2.15"),
                            probe_kind: PsionNormativeSpecReadingProbeKind::NamedGuarantee,
                            expected_fact: String::from("named execution guarantee"),
                            grounded_reading_required: true,
                            engineering_inference_forbidden: true,
                        },
                        detail: String::from(
                            "Normative guarantee item checks whether the answer states what the spec actually guarantees without adding portability claims that the text does not name.",
                        ),
                    },
                ],
                "Normative spec package uses typed exact-label items that separate exact definitions and named edge conditions from any later engineering inference.",
            )?,
            record_psion_benchmark_package(
                "psion_engineering_spec_benchmark_v1",
                PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                benchmark_package(
                    "psion_engineering_spec_benchmark_v1",
                    &[
                        "eng-case-implication",
                        "eng-case-ambiguity",
                        "eng-case-unspecified",
                        "eng-case-portability",
                    ],
                ),
                vec![explanation_prompt_format()],
                vec![rubric_grader(), exact_label_grader()],
                contamination_inputs(&["spec_quiz_eval_pack_v1", "wasm_core_spec_release_2"]),
                vec![
                    PsionBenchmarkItem {
                        item_id: String::from("eng-case-implication"),
                        family: PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("rubric_reasoning_v1"),
                        prompt_digest: String::from("eng-prompt-digest-implication"),
                        source_ids: vec![
                            String::from("spec_quiz_eval_pack_v1"),
                            String::from("wasm_core_spec_release_2"),
                        ],
                        task: PsionBenchmarkTaskContract::EngineeringSpecInterpretation {
                            normative_source_ref: String::from("wasm://core/execution"),
                            required_section_anchor: String::from("4.4.9"),
                            probe_kind: PsionEngineeringSpecInterpretationProbeKind::ImplementationImplication,
                            implementation_target: String::from(
                                "single-threaded runtime scheduler",
                            ),
                            expected_consequence: String::from(
                                "trap propagation ordering must remain visible to the runtime",
                            ),
                            normative_boundary_required: true,
                            explicit_uncertainty_required: true,
                            unsupported_certainty_forbidden: true,
                        },
                        detail: String::from(
                            "Engineering implication item checks whether the answer separates what the execution text guarantees from the scheduling implication an embedding should preserve.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("eng-case-ambiguity"),
                        family: PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("rubric_reasoning_v1"),
                        prompt_digest: String::from("eng-prompt-digest-ambiguity"),
                        source_ids: vec![
                            String::from("spec_quiz_eval_pack_v1"),
                            String::from("wasm_core_spec_release_2"),
                        ],
                        task: PsionBenchmarkTaskContract::EngineeringSpecInterpretation {
                            normative_source_ref: String::from("wasm://core/validation"),
                            required_section_anchor: String::from("2.5.7"),
                            probe_kind: PsionEngineeringSpecInterpretationProbeKind::AmbiguityRisk,
                            implementation_target: String::from("module loader import matcher"),
                            expected_consequence: String::from(
                                "ambiguous import matching must be surfaced as a compatibility risk",
                            ),
                            normative_boundary_required: true,
                            explicit_uncertainty_required: true,
                            unsupported_certainty_forbidden: true,
                        },
                        detail: String::from(
                            "Engineering ambiguity item checks whether the answer admits where the normative text leaves matching behavior open instead of projecting one implementation as universal.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("eng-case-unspecified"),
                        family: PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("exact_label_v1"),
                        prompt_digest: String::from("eng-prompt-digest-unspecified"),
                        source_ids: vec![
                            String::from("spec_quiz_eval_pack_v1"),
                            String::from("wasm_core_spec_release_2"),
                        ],
                        task: PsionBenchmarkTaskContract::EngineeringSpecInterpretation {
                            normative_source_ref: String::from("wasm://core/memory"),
                            required_section_anchor: String::from("4.3.6"),
                            probe_kind: PsionEngineeringSpecInterpretationProbeKind::UnspecifiedRegion,
                            implementation_target: String::from("embedding memory-growth policy"),
                            expected_consequence: String::from(
                                "the answer must name the policy choice as implementation-defined rather than spec-mandated",
                            ),
                            normative_boundary_required: true,
                            explicit_uncertainty_required: true,
                            unsupported_certainty_forbidden: true,
                        },
                        detail: String::from(
                            "Engineering unspecified-region item checks deterministic recognition that the embedding policy remains outside the normative text.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("eng-case-portability"),
                        family: PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("rubric_reasoning_v1"),
                        prompt_digest: String::from("eng-prompt-digest-portability"),
                        source_ids: vec![
                            String::from("spec_quiz_eval_pack_v1"),
                            String::from("wasm_core_spec_release_2"),
                        ],
                        task: PsionBenchmarkTaskContract::EngineeringSpecInterpretation {
                            normative_source_ref: String::from("wasm://core/modules"),
                            required_section_anchor: String::from("2.5.11"),
                            probe_kind: PsionEngineeringSpecInterpretationProbeKind::PortabilityConsequence,
                            implementation_target: String::from("cross-engine module artifact cache"),
                            expected_consequence: String::from(
                                "the answer must identify portability risk when engine-specific assumptions outrun the spec",
                            ),
                            normative_boundary_required: true,
                            explicit_uncertainty_required: true,
                            unsupported_certainty_forbidden: true,
                        },
                        detail: String::from(
                            "Engineering portability item checks whether the answer names cross-engine compatibility risk without claiming the spec guarantees more than it states.",
                        ),
                    },
                ],
                "Engineering spec package uses typed items that keep normative anchors explicit while testing implementation implications, ambiguity risks, unspecified regions, and portability consequences.",
            )?,
            record_psion_benchmark_package(
                "psion_memorization_reasoning_benchmark_v1",
                PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                benchmark_package(
                    "psion_memorization_reasoning_benchmark_v1",
                    &[
                        "mem-case-altered-constraints",
                        "mem-case-unfamiliar-synthesis",
                        "mem-case-historical-transfer",
                        "mem-case-paraphrase",
                        "mem-case-spec-edge",
                    ],
                ),
                vec![explanation_prompt_format()],
                vec![exact_label_grader()],
                contamination_inputs(&["spec_quiz_eval_pack_v1", "wasm_core_spec_release_2"]),
                vec![
                    PsionBenchmarkItem {
                        item_id: String::from("mem-case-altered-constraints"),
                        family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("exact_label_v1"),
                        prompt_digest: String::from("mem-prompt-digest-altered"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                            seed_fact_ref: String::from(
                                "seed://psion/memorization/cache-hierarchy-baseline",
                            ),
                            perturbation_ref: String::from(
                                "perturbation://psion/memorization/cache-latency-budget-tightened",
                            ),
                            probe_kind:
                                PsionMemorizationVersusReasoningProbeKind::AlteredConstraintRecombination,
                            expected_transfer: String::from(
                                "adapt the pipeline recommendation under a tightened latency budget instead of replaying the baseline answer",
                            ),
                            recombination_required: true,
                            surface_form_shift_required: true,
                            verbatim_recall_forbidden: true,
                        },
                        detail: String::from(
                            "Altered-constraint probe checks whether the answer recombines the learned architecture ontology after one key resource bound is changed.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("mem-case-unfamiliar-synthesis"),
                        family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("exact_label_v1"),
                        prompt_digest: String::from("mem-prompt-digest-synthesis"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                            seed_fact_ref: String::from(
                                "seed://psion/memorization/queueing-and-backpressure",
                            ),
                            perturbation_ref: String::from(
                                "perturbation://psion/memorization/queueing-plus-capability-routing",
                            ),
                            probe_kind:
                                PsionMemorizationVersusReasoningProbeKind::UnfamiliarDesignSynthesis,
                            expected_transfer: String::from(
                                "combine queueing and capability-routing ideas for an unfamiliar system composition",
                            ),
                            recombination_required: true,
                            surface_form_shift_required: true,
                            verbatim_recall_forbidden: true,
                        },
                        detail: String::from(
                            "Unfamiliar-synthesis probe checks whether the answer composes known mechanisms in a new configuration rather than reciting one familiar design.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("mem-case-historical-transfer"),
                        family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("exact_label_v1"),
                        prompt_digest: String::from("mem-prompt-digest-historical"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                            seed_fact_ref: String::from(
                                "seed://psion/memorization/vector-era-scheduling",
                            ),
                            perturbation_ref: String::from(
                                "perturbation://psion/memorization/gpu-batch-scheduler-analogy",
                            ),
                            probe_kind:
                                PsionMemorizationVersusReasoningProbeKind::HistoricalAnalogyTransfer,
                            expected_transfer: String::from(
                                "transfer a bounded historical scheduling analogy without claiming the old and new systems are identical",
                            ),
                            recombination_required: true,
                            surface_form_shift_required: true,
                            verbatim_recall_forbidden: true,
                        },
                        detail: String::from(
                            "Historical-transfer probe checks whether the answer uses historical systems knowledge as an analogy instead of as a canned template.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("mem-case-paraphrase"),
                        family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("exact_label_v1"),
                        prompt_digest: String::from("mem-prompt-digest-paraphrase"),
                        source_ids: vec![
                            String::from("spec_quiz_eval_pack_v1"),
                            String::from("wasm_core_spec_release_2"),
                        ],
                        task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                            seed_fact_ref: String::from(
                                "seed://psion/memorization/wasm-validation-definition",
                            ),
                            perturbation_ref: String::from(
                                "perturbation://psion/memorization/wasm-validation-paraphrase-variant",
                            ),
                            probe_kind:
                                PsionMemorizationVersusReasoningProbeKind::ParaphraseVariation,
                            expected_transfer: String::from(
                                "answer the paraphrased specification prompt without depending on the memorized stock wording",
                            ),
                            recombination_required: true,
                            surface_form_shift_required: true,
                            verbatim_recall_forbidden: true,
                        },
                        detail: String::from(
                            "Paraphrase-variation probe checks whether the answer survives surface-form changes around one familiar technical fact.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("mem-case-spec-edge"),
                        family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                        prompt_format_id: String::from("bounded_explanation_v1"),
                        grader_id: String::from("exact_label_v1"),
                        prompt_digest: String::from("mem-prompt-digest-edge"),
                        source_ids: vec![
                            String::from("spec_quiz_eval_pack_v1"),
                            String::from("wasm_core_spec_release_2"),
                        ],
                        task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                            seed_fact_ref: String::from(
                                "seed://psion/memorization/module-linking-baseline",
                            ),
                            perturbation_ref: String::from(
                                "perturbation://psion/memorization/spec-adjacent-edge-case",
                            ),
                            probe_kind:
                                PsionMemorizationVersusReasoningProbeKind::SpecAdjacentEdgeCase,
                            expected_transfer: String::from(
                                "apply the learned specification ontology to a nearby edge case that is not quoted verbatim in the seed material",
                            ),
                            recombination_required: true,
                            surface_form_shift_required: true,
                            verbatim_recall_forbidden: true,
                        },
                        detail: String::from(
                            "Spec-adjacent edge-case probe checks whether the answer can reason next to the source text instead of only replaying the exact benchmark passage.",
                        ),
                    },
                ],
                "Memorization-versus-reasoning package uses typed exact-label probes that force altered constraints, unfamiliar synthesis, historical transfer, paraphrase variation, and spec-adjacent edge cases.",
            )?,
            record_psion_benchmark_package(
                "psion_route_benchmark_v1",
                PsionBenchmarkPackageFamily::RouteEvaluation,
                benchmark_package(
                    "psion_route_benchmark_v1",
                    &[
                        "route-case-answer",
                        "route-case-uncertainty",
                        "route-case-structure",
                        "route-case-delegate",
                    ],
                ),
                vec![route_prompt_format()],
                vec![
                    exact_route_grader("route_answer_v1", PsionRouteClass::AnswerInLanguage),
                    exact_route_grader(
                        "route_uncertainty_v1",
                        PsionRouteClass::AnswerWithUncertainty,
                    ),
                    exact_route_grader(
                        "route_structure_v1",
                        PsionRouteClass::RequestStructuredInputs,
                    ),
                    exact_route_grader(
                        "route_delegate_v1",
                        PsionRouteClass::DelegateToExactExecutor,
                    ),
                ],
                contamination_inputs(&["spec_quiz_eval_pack_v1"]),
                vec![
                    PsionBenchmarkItem {
                        item_id: String::from("route-case-answer"),
                        family: PsionBenchmarkPackageFamily::RouteEvaluation,
                        prompt_format_id: String::from("route_decision_v1"),
                        grader_id: String::from("route_answer_v1"),
                        prompt_digest: String::from("route-prompt-digest-answer"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::RouteEvaluation {
                            route_class: PsionRouteClass::AnswerInLanguage,
                            route_boundary_ref: String::from(
                                "route://psion/language_judgment_boundary",
                            ),
                            required_signal: String::from("answer directly in language"),
                            structured_input_schema_ref: None,
                            uncertainty_required: false,
                        },
                        detail: String::from(
                            "Route answer item checks the bounded direct-answer class without forcing delegation or a structured-input ask.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("route-case-uncertainty"),
                        family: PsionBenchmarkPackageFamily::RouteEvaluation,
                        prompt_format_id: String::from("route_decision_v1"),
                        grader_id: String::from("route_uncertainty_v1"),
                        prompt_digest: String::from("route-prompt-digest-uncertainty"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::RouteEvaluation {
                            route_class: PsionRouteClass::AnswerWithUncertainty,
                            route_boundary_ref: String::from("route://psion/uncertainty_boundary"),
                            required_signal: String::from("answer with explicit uncertainty"),
                            structured_input_schema_ref: None,
                            uncertainty_required: true,
                        },
                        detail: String::from(
                            "Route uncertainty item checks whether the learned lane can stay in-language while making uncertainty explicit.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("route-case-structure"),
                        family: PsionBenchmarkPackageFamily::RouteEvaluation,
                        prompt_format_id: String::from("route_decision_v1"),
                        grader_id: String::from("route_structure_v1"),
                        prompt_digest: String::from("route-prompt-digest-structure"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::RouteEvaluation {
                            route_class: PsionRouteClass::RequestStructuredInputs,
                            route_boundary_ref: String::from(
                                "route://psion/structured-input-boundary",
                            ),
                            required_signal: String::from(
                                "request the missing structured inputs before answering",
                            ),
                            structured_input_schema_ref: Some(String::from(
                                "schema://psion/route/design_inputs_v1",
                            )),
                            uncertainty_required: false,
                        },
                        detail: String::from(
                            "Route structure item checks whether the learned lane asks for the named structured fields instead of guessing them.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("route-case-delegate"),
                        family: PsionBenchmarkPackageFamily::RouteEvaluation,
                        prompt_format_id: String::from("route_decision_v1"),
                        grader_id: String::from("route_delegate_v1"),
                        prompt_digest: String::from("route-prompt-digest-delegate"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::RouteEvaluation {
                            route_class: PsionRouteClass::DelegateToExactExecutor,
                            route_boundary_ref: String::from("route://psion/exactness_boundary"),
                            required_signal: String::from(
                                "delegate to the exact executor instead of improvising",
                            ),
                            structured_input_schema_ref: None,
                            uncertainty_required: false,
                        },
                        detail: String::from(
                            "Route delegate item checks the bounded exact-executor handoff class without over-reading delegation as learned execution.",
                        ),
                    },
                ],
                "Route package uses the shared structured route prompt and exact route-class graders across answer, uncertainty, structure-request, and exact-delegation paths.",
            )?,
            record_psion_benchmark_package(
                "psion_unsupported_request_refusal_benchmark_v1",
                PsionBenchmarkPackageFamily::RefusalEvaluation,
                benchmark_package(
                    "psion_unsupported_request_refusal_benchmark_v1",
                    &[
                        "refusal-case-exactness",
                        "refusal-case-missing-constraints",
                        "refusal-case-over-context",
                        "refusal-case-freshness",
                        "refusal-case-open-ended",
                    ],
                ),
                vec![refusal_prompt_format()],
                vec![
                    exact_refusal_grader(
                        "exact_refusal_exactness_v1",
                        "unsupported_exactness_request",
                    ),
                    exact_refusal_grader(
                        "exact_refusal_missing_constraints_v1",
                        "missing_required_constraints",
                    ),
                    exact_refusal_grader(
                        "exact_refusal_context_v1",
                        "unsupported_context_length",
                    ),
                    exact_refusal_grader(
                        "exact_refusal_freshness_v1",
                        "currentness_or_run_artifact_dependency",
                    ),
                    exact_refusal_grader(
                        "exact_refusal_open_ended_v1",
                        "open_ended_general_assistant_unsupported",
                    ),
                ],
                contamination_inputs(&["spec_quiz_eval_pack_v1"]),
                vec![
                    PsionBenchmarkItem {
                        item_id: String::from("refusal-case-exactness"),
                        family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                        prompt_format_id: String::from("refusal_decision_v1"),
                        grader_id: String::from("exact_refusal_exactness_v1"),
                        prompt_digest: String::from("refusal-prompt-digest-exactness"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::RefusalEvaluation {
                            expected_reason_code: String::from("unsupported_exactness_request"),
                            refusal_boundary_ref: String::from(
                                "route://psion/refusal/exactness-without-executor",
                            ),
                            probe_kind: PsionRefusalProbeKind::UnsupportedExactnessRequest,
                            capability_region_id: String::from(
                                "unsupported_exact_execution_without_executor_surface",
                            ),
                            unsupported_region_evidence_ref: String::from(
                                "evidence://psion/refusal/exactness-without-executor",
                            ),
                            claim_boundary_required: true,
                        },
                        detail: String::from(
                            "Refusal exactness item checks that exact-execution requests without a surfaced executor path refuse instead of improvising an answer.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("refusal-case-missing-constraints"),
                        family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                        prompt_format_id: String::from("refusal_decision_v1"),
                        grader_id: String::from("exact_refusal_missing_constraints_v1"),
                        prompt_digest: String::from("refusal-prompt-digest-missing-constraints"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::RefusalEvaluation {
                            expected_reason_code: String::from("missing_required_constraints"),
                            refusal_boundary_ref: String::from(
                                "route://psion/refusal/missing-required-constraints",
                            ),
                            probe_kind: PsionRefusalProbeKind::MissingConstraints,
                            capability_region_id: String::from(
                                "underspecified_design_without_required_constraints",
                            ),
                            unsupported_region_evidence_ref: String::from(
                                "evidence://psion/refusal/missing-required-constraints",
                            ),
                            claim_boundary_required: true,
                        },
                        detail: String::from(
                            "Refusal missing-constraints item checks that underspecified design asks refuse instead of fabricating requirements.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("refusal-case-over-context"),
                        family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                        prompt_format_id: String::from("refusal_decision_v1"),
                        grader_id: String::from("exact_refusal_context_v1"),
                        prompt_digest: String::from("refusal-prompt-digest-over-context"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::RefusalEvaluation {
                            expected_reason_code: String::from("unsupported_context_length"),
                            refusal_boundary_ref: String::from(
                                "route://psion/refusal/hard-context-boundary",
                            ),
                            probe_kind: PsionRefusalProbeKind::OverContextEnvelope,
                            capability_region_id: String::from("over_context_envelope_requests"),
                            unsupported_region_evidence_ref: String::from(
                                "evidence://psion/refusal/over-context-envelope",
                            ),
                            claim_boundary_required: true,
                        },
                        detail: String::from(
                            "Refusal over-context item checks that prompts beyond the hard context envelope refuse instead of truncating or stretching the lane silently.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("refusal-case-freshness"),
                        family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                        prompt_format_id: String::from("refusal_decision_v1"),
                        grader_id: String::from("exact_refusal_freshness_v1"),
                        prompt_digest: String::from("refusal-prompt-digest-freshness"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::RefusalEvaluation {
                            expected_reason_code: String::from(
                                "currentness_or_run_artifact_dependency",
                            ),
                            refusal_boundary_ref: String::from(
                                "route://psion/refusal/currentness-or-hidden-artifact",
                            ),
                            probe_kind: PsionRefusalProbeKind::FreshnessOrHiddenArtifactDependency,
                            capability_region_id: String::from(
                                "freshness_or_run_artifact_dependent_requests",
                            ),
                            unsupported_region_evidence_ref: String::from(
                                "evidence://psion/refusal/currentness-or-hidden-artifact",
                            ),
                            claim_boundary_required: true,
                        },
                        detail: String::from(
                            "Refusal freshness item checks that currentness-sensitive or hidden-artifact asks refuse instead of pretending the lane can inspect mutable state.",
                        ),
                    },
                    PsionBenchmarkItem {
                        item_id: String::from("refusal-case-open-ended"),
                        family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                        prompt_format_id: String::from("refusal_decision_v1"),
                        grader_id: String::from("exact_refusal_open_ended_v1"),
                        prompt_digest: String::from("refusal-prompt-digest-open-ended"),
                        source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                        task: PsionBenchmarkTaskContract::RefusalEvaluation {
                            expected_reason_code: String::from(
                                "open_ended_general_assistant_unsupported",
                            ),
                            refusal_boundary_ref: String::from(
                                "route://psion/refusal/open-ended-assistant",
                            ),
                            probe_kind: PsionRefusalProbeKind::UnsupportedGeneralAssistantChat,
                            capability_region_id: String::from(
                                "open_ended_general_assistant_chat",
                            ),
                            unsupported_region_evidence_ref: String::from(
                                "evidence://psion/refusal/open-ended-assistant",
                            ),
                            claim_boundary_required: true,
                        },
                        detail: String::from(
                            "Refusal open-ended item checks that broad assistant chat stays explicitly unsupported instead of being half-served by vibe.",
                        ),
                    },
                ],
                "Refusal package uses the shared structured refusal prompt with capability-matrix-bound refusal probes for exactness, missing constraints, context overflow, freshness, and open-ended assistant asks.",
            )?,
        ])
    }

    #[test]
    fn benchmark_catalog_fixture_validates() -> Result<(), Box<dyn std::error::Error>> {
        let lifecycle = lifecycle_manifest();
        let exclusion = exclusion_manifest();
        let packages = package_contracts()?;
        let catalog = record_psion_benchmark_catalog(
            "psion-benchmark-catalog-v1",
            &lifecycle,
            &exclusion,
            packages.clone(),
            "Canonical catalog proving the main Psion benchmark families can all build on one shared prompt, item, grader, contamination-input, and receipt contract.",
        )?;
        let engineering = catalog
            .packages
            .iter()
            .find(|package| package.package_id == "psion_engineering_spec_benchmark_v1")
            .expect("engineering package should exist");
        assert_eq!(
            engineering.acceptance_family,
            Some(PsionBenchmarkFamily::EngineeringSpecInterpretation),
            "engineering package should map into the dedicated acceptance family"
        );
        let memorization = catalog
            .packages
            .iter()
            .find(|package| package.package_id == "psion_memorization_reasoning_benchmark_v1")
            .expect("memorization package should exist");
        assert_eq!(
            memorization.acceptance_family,
            Some(PsionBenchmarkFamily::MemorizationVersusReasoning),
            "memorization package should map into the dedicated acceptance family"
        );
        let receipts = vec![
            record_psion_benchmark_package_receipt(
                "psion-architecture-benchmark-receipt-v1",
                PsionPhaseGate::Pilot,
                &packages[0],
                vec![PsionObservedMetric {
                    metric_kind: PsionMetricKind::PassRateBps,
                    observed_bps: 8420,
                    regression_from_baseline_bps: 0,
                }],
                "Architecture reasoning benchmark receipt on the shared contract.",
            )?,
            record_psion_benchmark_package_receipt(
                "psion-normative-benchmark-receipt-v1",
                PsionPhaseGate::Pilot,
                &packages[1],
                vec![PsionObservedMetric {
                    metric_kind: PsionMetricKind::PassRateBps,
                    observed_bps: 8910,
                    regression_from_baseline_bps: 0,
                }],
                "Normative spec reading benchmark receipt on the shared contract.",
            )?,
            record_psion_benchmark_package_receipt(
                "psion-engineering-benchmark-receipt-v1",
                PsionPhaseGate::Pilot,
                &packages[2],
                vec![PsionObservedMetric {
                    metric_kind: PsionMetricKind::PassRateBps,
                    observed_bps: 8760,
                    regression_from_baseline_bps: 0,
                }],
                "Engineering spec interpretation benchmark receipt on the shared contract.",
            )?,
            record_psion_benchmark_package_receipt(
                "psion-memorization-benchmark-receipt-v1",
                PsionPhaseGate::Pilot,
                &packages[3],
                vec![PsionObservedMetric {
                    metric_kind: PsionMetricKind::PassRateBps,
                    observed_bps: 8040,
                    regression_from_baseline_bps: 0,
                }],
                "Memorization-versus-reasoning benchmark receipt on the shared contract.",
            )?,
            record_psion_benchmark_package_receipt(
                "psion-route-benchmark-receipt-v1",
                PsionPhaseGate::Pilot,
                &packages[4],
                vec![PsionObservedMetric {
                    metric_kind: PsionMetricKind::RouteSelectionAccuracyBps,
                    observed_bps: 9730,
                    regression_from_baseline_bps: 0,
                }],
                "Route benchmark receipt on the shared contract.",
            )?,
            record_psion_benchmark_package_receipt(
                "psion-unsupported-request-refusal-benchmark-receipt-v1",
                PsionPhaseGate::Pilot,
                &packages[5],
                vec![
                    PsionObservedMetric {
                        metric_kind: PsionMetricKind::UnsupportedRequestRefusalBps,
                        observed_bps: 9910,
                        regression_from_baseline_bps: 0,
                    },
                    PsionObservedMetric {
                        metric_kind: PsionMetricKind::OverrefusalBps,
                        observed_bps: 340,
                        regression_from_baseline_bps: 0,
                    },
                ],
                "Unsupported-request refusal benchmark receipt on the shared contract.",
            )?,
        ];
        let receipt_set = record_psion_benchmark_receipt_set(
            "psion-benchmark-receipt-set-v1",
            &catalog,
            &lifecycle,
            &exclusion,
            receipts,
            "Canonical receipt set proving the shared Psion benchmark contract emits acceptance-ready package receipts across the main benchmark families.",
        )?;
        receipt_set.validate_against_catalog(&catalog, &lifecycle, &exclusion)?;
        Ok(())
    }

    #[test]
    fn benchmark_package_rejects_benchmark_hidden_source() -> Result<(), Box<dyn std::error::Error>>
    {
        let lifecycle = lifecycle_manifest();
        let exclusion = exclusion_manifest();
        let mut packages = package_contracts()?;
        packages[0].contamination_inputs.benchmark_source_ids =
            vec![String::from("arch_textbook_foster_1985")];
        packages[0].contamination_inputs.contamination_input_digest =
            stable_contamination_input_digest(&packages[0].contamination_inputs);
        packages[0].items[0].source_ids = vec![String::from("arch_textbook_foster_1985")];
        packages[0].package_digest = super::stable_benchmark_package_digest(&packages[0]);
        let error = packages[0]
            .validate_against_context(&lifecycle, &exclusion)
            .expect_err("benchmark-hidden source should be rejected");
        assert!(matches!(
            error,
            PsionBenchmarkPackageError::BenchmarkIsolation { .. }
        ));
        Ok(())
    }

    #[test]
    fn route_package_rejects_non_route_grader() -> Result<(), Box<dyn std::error::Error>> {
        let lifecycle = lifecycle_manifest();
        let exclusion = exclusion_manifest();
        let mut packages = package_contracts()?;
        packages[4].grader_interfaces = vec![exact_label_grader()];
        for item in &mut packages[4].items {
            item.grader_id = String::from("exact_label_v1");
        }
        packages[4].package_digest = super::stable_benchmark_package_digest(&packages[4]);
        let error = packages[4]
            .validate_against_context(&lifecycle, &exclusion)
            .expect_err("route package should reject exact-label grader");
        assert!(matches!(
            error,
            PsionBenchmarkPackageError::GraderFamilyMismatch { .. }
        ));
        Ok(())
    }

    #[test]
    fn refusal_receipt_requires_overrefusal_metric() -> Result<(), Box<dyn std::error::Error>> {
        let packages = package_contracts()?;
        let package = &packages[5];
        let error = record_psion_benchmark_package_receipt(
            "psion-unsupported-request-refusal-benchmark-receipt-v1",
            PsionPhaseGate::Pilot,
            package,
            vec![PsionObservedMetric {
                metric_kind: PsionMetricKind::UnsupportedRequestRefusalBps,
                observed_bps: 9910,
                regression_from_baseline_bps: 0,
            }],
            "Unsupported-request refusal benchmark receipt on the shared contract.",
        )
        .expect_err("refusal receipt should require overrefusal metric");
        assert!(matches!(
            error,
            PsionBenchmarkPackageError::MissingRequiredMetric { .. }
        ));
        Ok(())
    }
}
