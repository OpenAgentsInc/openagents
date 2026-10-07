use std::collections::BTreeSet;

use psionic_data::{
    PsionContaminationViolationConsequence, PSION_BENCHMARK_ISOLATION_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{PsionRefusalCalibrationError, PsionRefusalCalibrationReceipt};

/// Stable schema version for the first Psion acceptance-matrix artifact.
pub const PSION_ACCEPTANCE_MATRIX_SCHEMA_VERSION: &str = "psion.acceptance_matrix.v1";
/// Stable schema version for the first Psion phase-promotion decision receipt.
pub const PSION_PROMOTION_DECISION_SCHEMA_VERSION: &str = "psion.promotion_decision.v1";

/// Phase gate in the first Psion learned-model program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPhaseGate {
    /// First bounded pilot over the curated technical corpus.
    Pilot,
    /// Broader pretraining beyond the initial pilot slice.
    BroaderPretraining,
    /// Promotion from pretraining into supervised fine-tuning.
    SftPromotion,
    /// Internal serving on bounded route/refusal posture.
    InternalServing,
    /// Trusted-cluster multi-host scale-up.
    TrustedClusterScaleUp,
}

impl PsionPhaseGate {
    #[must_use]
    pub const fn required_phases() -> [Self; 5] {
        [
            Self::Pilot,
            Self::BroaderPretraining,
            Self::SftPromotion,
            Self::InternalServing,
            Self::TrustedClusterScaleUp,
        ]
    }
}

/// Benchmark family required by one Psion phase gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionBenchmarkFamily {
    /// Architecture reasoning on unseen or held-out architecture tasks.
    ArchitectureReasoning,
    /// Held-out improvement on the curated technical corpus.
    HeldOutTechnicalReasoning,
    /// Normative source-grounded reading on held-out specifications and manuals.
    NormativeSpecReading,
    /// Engineering interpretation that stays explicit about where text ends and inference begins.
    EngineeringSpecInterpretation,
    /// Recombination probes that distinguish memorized stock passages from transferable reasoning.
    MemorizationVersusReasoning,
    /// Specifications and manuals kept separate from the training lane.
    SpecificationAndManualComprehension,
    /// Route-selection calibration across direct, handoff, and refusal lanes.
    RouteSelection,
    /// Refusal on unsupported requests or unsupported route asks.
    UnsupportedRequestRefusal,
}

/// Metric kind carried in one phase gate threshold band or evidence receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionMetricKind {
    /// Generic pass-rate style metric.
    PassRateBps,
    /// Improvement over the seed pilot or prior frozen baseline.
    ImprovementOverSeedBaselineBps,
    /// Accuracy of the route-selection decision.
    RouteSelectionAccuracyBps,
    /// Refusal rate on unsupported requests.
    UnsupportedRequestRefusalBps,
    /// Over-refusal on supported requests.
    OverrefusalBps,
}

/// Route surface that must remain explicit in the Psion lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRouteKind {
    /// The learned lane answers directly.
    DirectModelAnswer,
    /// The learned lane routes to an exact executor or verifier-backed path.
    ExactExecutorHandoff,
    /// The learned lane refuses the request.
    Refusal,
}

/// Contamination review result recorded against one phase decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionContaminationReviewDisposition {
    /// Review completed and the reviewed slice stayed clean.
    Clean,
    /// Review found contamination or leakage.
    Contaminated,
    /// Review could not clear the slice yet.
    Inconclusive,
}

/// Final phase-promotion decision recorded against the matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPromotionDecisionDisposition {
    /// The candidate cleared the gate.
    Promoted,
    /// The candidate stays blocked at the current phase.
    Held,
}

/// Gate-failure reason that can force a held decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPromotionHoldReasonCode {
    /// One or more benchmark thresholds missed the current matrix band.
    BenchmarkThresholdMissed,
    /// Replay evidence did not meet the current gate.
    ReplayRequirementMissed,
    /// Checkpoint recovery evidence did not meet the current gate.
    CheckpointRequirementMissed,
    /// Contamination review did not clear the current gate.
    ContaminationReviewFailed,
    /// Route calibration stayed below the current gate.
    RouteCalibrationFailed,
    /// Refusal calibration stayed below the current gate.
    RefusalCalibrationFailed,
}

/// Threshold band for one benchmark or calibration metric.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionMetricThresholdBand {
    /// Metric covered by the band.
    pub metric_kind: PsionMetricKind,
    /// Minimum acceptable observed value when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_bps: Option<u32>,
    /// Maximum acceptable observed value when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_bps: Option<u32>,
    /// Maximum allowed regression against the last accepted baseline.
    pub allowed_regression_bps: u32,
    /// Short explanation of the threshold.
    pub rationale: String,
}

/// Required benchmark family for one Psion phase gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkRequirement {
    /// Benchmark family under gate review.
    pub family: PsionBenchmarkFamily,
    /// Bound benchmark artifact id when the phase must target one concrete package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_artifact_id: Option<String>,
    /// Bound benchmark artifact digest when the phase must target one concrete package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_artifact_digest: Option<String>,
    /// Threshold bands that must all be satisfied.
    pub threshold_bands: Vec<PsionMetricThresholdBand>,
    /// Short explanation of why the family is required.
    pub rationale: String,
}

/// Replay requirement for one Psion phase gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReplayRequirement {
    /// Minimum number of successful replay receipts.
    pub minimum_successful_replays: u16,
    /// Whether exact replay is mandatory for the gate.
    pub exact_replay_required: bool,
    /// Short explanation of the replay posture.
    pub rationale: String,
}

/// Checkpoint and restart requirement for one Psion phase gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCheckpointRequirement {
    /// Whether restart and recovery must be proven.
    pub restart_recovery_required: bool,
    /// Minimum successful restart round-trips.
    pub minimum_successful_restart_roundtrips: u16,
    /// Maximum allowed post-resume regression.
    pub maximum_resume_regression_bps: u32,
    /// Short explanation of the checkpoint posture.
    pub rationale: String,
}

/// Contamination review requirement bound to the held-out isolation contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionContaminationReviewRequirement {
    /// Whether near-duplicate review must be complete.
    pub near_duplicate_review_required: bool,
    /// Whether tokenizer-exposure review must be complete.
    pub tokenizer_exposure_review_required: bool,
    /// Whether only a clean review may promote the phase.
    pub clean_review_required_for_promotion: bool,
    /// Consequences that must fire when contamination is found.
    pub required_violation_consequences: Vec<PsionContaminationViolationConsequence>,
    /// Short explanation of the contamination posture.
    pub rationale: String,
}

/// Route calibration requirement for one phase gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRouteCalibrationRequirement {
    /// Routes that must be covered by the calibration evidence.
    pub required_supported_routes: Vec<PsionRouteKind>,
    /// Minimum route-selection accuracy.
    pub minimum_route_selection_accuracy_bps: u32,
    /// Maximum allowed regression against the last accepted baseline.
    pub maximum_route_regression_bps: u32,
    /// Short explanation of the route posture.
    pub rationale: String,
}

/// Refusal calibration requirement for one phase gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRefusalCalibrationRequirement {
    /// Minimum refusal rate on unsupported requests.
    pub minimum_unsupported_request_refusal_bps: u32,
    /// Maximum over-refusal rate on supported requests.
    pub maximum_overrefusal_bps: u32,
    /// Maximum allowed regression against the last accepted baseline.
    pub maximum_refusal_regression_bps: u32,
    /// Short explanation of the refusal posture.
    pub rationale: String,
}

/// One full phase gate inside the Psion acceptance matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionAcceptancePhaseGate {
    /// Phase governed by this gate.
    pub phase: PsionPhaseGate,
    /// Required benchmark families for the phase.
    pub benchmark_requirements: Vec<PsionBenchmarkRequirement>,
    /// Replay evidence contract.
    pub replay_requirement: PsionReplayRequirement,
    /// Checkpoint and recovery evidence contract.
    pub checkpoint_requirement: PsionCheckpointRequirement,
    /// Contamination review contract.
    pub contamination_review_requirement: PsionContaminationReviewRequirement,
    /// Route calibration contract.
    pub route_calibration_requirement: PsionRouteCalibrationRequirement,
    /// Refusal calibration contract.
    pub refusal_calibration_requirement: PsionRefusalCalibrationRequirement,
    /// Short phase summary.
    pub phase_summary: String,
}

impl PsionAcceptancePhaseGate {
    /// Returns the declared requirement for one benchmark family when present.
    #[must_use]
    pub fn benchmark_requirement(
        &self,
        family: PsionBenchmarkFamily,
    ) -> Option<&PsionBenchmarkRequirement> {
        self.benchmark_requirements
            .iter()
            .find(|requirement| requirement.family == family)
    }
}

/// Explicit pilot success criteria that block unjustified scale-up.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPilotSuccessCriteria {
    /// Minimum architecture-reasoning pass rate for the pilot.
    pub minimum_architecture_reasoning_pass_rate_bps: u32,
    /// Minimum held-out improvement over the seed baseline for the pilot.
    pub minimum_held_out_improvement_bps: u32,
    /// Minimum unsupported-request refusal rate for the pilot.
    pub minimum_unsupported_request_refusal_bps: u32,
    /// Whether later scale-up is blocked without a clean contamination review.
    pub scale_up_blocked_without_clean_contamination_review: bool,
    /// Short explanation of the pilot gate.
    pub rationale: String,
}

/// Versioned acceptance matrix for the first Psion learned-model lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionAcceptanceMatrix {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable matrix identifier.
    pub matrix_id: String,
    /// Stable matrix version.
    pub matrix_version: String,
    /// Held-out isolation schema the matrix is bound to.
    pub benchmark_isolation_schema_version: String,
    /// Phase gates in dependency order.
    pub phase_gates: Vec<PsionAcceptancePhaseGate>,
    /// Explicit pilot success criteria.
    pub pilot_success_criteria: PsionPilotSuccessCriteria,
}

impl PsionAcceptanceMatrix {
    /// Returns the gate for one phase when present.
    #[must_use]
    pub fn phase_gate(&self, phase: PsionPhaseGate) -> Option<&PsionAcceptancePhaseGate> {
        self.phase_gates.iter().find(|gate| gate.phase == phase)
    }

    /// Validates the matrix artifact.
    pub fn validate(&self) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "acceptance_matrix.schema_version",
        )?;
        if self.schema_version != PSION_ACCEPTANCE_MATRIX_SCHEMA_VERSION {
            return Err(PsionAcceptanceMatrixError::SchemaVersionMismatch {
                expected: String::from(PSION_ACCEPTANCE_MATRIX_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.matrix_id.as_str(), "acceptance_matrix.matrix_id")?;
        ensure_nonempty(
            self.matrix_version.as_str(),
            "acceptance_matrix.matrix_version",
        )?;
        ensure_nonempty(
            self.benchmark_isolation_schema_version.as_str(),
            "acceptance_matrix.benchmark_isolation_schema_version",
        )?;
        if self.benchmark_isolation_schema_version != PSION_BENCHMARK_ISOLATION_SCHEMA_VERSION {
            return Err(PsionAcceptanceMatrixError::SchemaVersionMismatch {
                expected: String::from(PSION_BENCHMARK_ISOLATION_SCHEMA_VERSION),
                actual: self.benchmark_isolation_schema_version.clone(),
            });
        }
        if self.phase_gates.is_empty() {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: String::from("acceptance_matrix.phase_gates"),
            });
        }

        let mut phase_set = BTreeSet::new();
        for gate in &self.phase_gates {
            if !phase_set.insert(gate.phase) {
                return Err(PsionAcceptanceMatrixError::DuplicatePhase { phase: gate.phase });
            }
            self.validate_phase_gate(gate)?;
        }
        for phase in PsionPhaseGate::required_phases() {
            if !phase_set.contains(&phase) {
                return Err(PsionAcceptanceMatrixError::MissingPhase { phase });
            }
        }

        self.validate_pilot_success_criteria()?;
        Ok(())
    }

    fn validate_phase_gate(
        &self,
        gate: &PsionAcceptancePhaseGate,
    ) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            gate.phase_summary.as_str(),
            format!("phase_gates.{:?}.phase_summary", gate.phase).as_str(),
        )?;
        if gate.benchmark_requirements.is_empty() {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: format!("phase_gates.{:?}.benchmark_requirements", gate.phase),
            });
        }

        let mut families = BTreeSet::new();
        for requirement in &gate.benchmark_requirements {
            if !families.insert(requirement.family) {
                return Err(PsionAcceptanceMatrixError::DuplicateBenchmarkFamily {
                    phase: gate.phase,
                    family: requirement.family,
                });
            }
            self.validate_benchmark_requirement(gate.phase, requirement)?;
        }

        if gate.replay_requirement.minimum_successful_replays == 0 {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: format!(
                    "phase_gates.{:?}.replay_requirement.minimum_successful_replays",
                    gate.phase
                ),
            });
        }
        ensure_nonempty(
            gate.replay_requirement.rationale.as_str(),
            format!("phase_gates.{:?}.replay_requirement.rationale", gate.phase).as_str(),
        )?;

        if gate.checkpoint_requirement.restart_recovery_required
            && gate
                .checkpoint_requirement
                .minimum_successful_restart_roundtrips
                == 0
        {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: format!(
                    "phase_gates.{:?}.checkpoint_requirement.minimum_successful_restart_roundtrips",
                    gate.phase
                ),
            });
        }
        validate_bps(
            gate.checkpoint_requirement.maximum_resume_regression_bps,
            format!(
                "phase_gates.{:?}.checkpoint_requirement.maximum_resume_regression_bps",
                gate.phase
            )
            .as_str(),
        )?;
        ensure_nonempty(
            gate.checkpoint_requirement.rationale.as_str(),
            format!(
                "phase_gates.{:?}.checkpoint_requirement.rationale",
                gate.phase
            )
            .as_str(),
        )?;

        self.validate_contamination_requirement(
            gate.phase,
            &gate.contamination_review_requirement,
        )?;
        self.validate_route_requirement(gate.phase, &gate.route_calibration_requirement)?;
        self.validate_refusal_requirement(gate.phase, &gate.refusal_calibration_requirement)?;
        Ok(())
    }

    fn validate_benchmark_requirement(
        &self,
        phase: PsionPhaseGate,
        requirement: &PsionBenchmarkRequirement,
    ) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            requirement.rationale.as_str(),
            format!(
                "phase_gates.{:?}.benchmark_requirements.{:?}.rationale",
                phase, requirement.family
            )
            .as_str(),
        )?;
        match (
            requirement.benchmark_artifact_id.as_deref(),
            requirement.benchmark_artifact_digest.as_deref(),
        ) {
            (Some(artifact_id), Some(artifact_digest)) => {
                ensure_nonempty(
                    artifact_id,
                    format!(
                        "phase_gates.{:?}.benchmark_requirements.{:?}.benchmark_artifact_id",
                        phase, requirement.family
                    )
                    .as_str(),
                )?;
                ensure_nonempty(
                    artifact_digest,
                    format!(
                        "phase_gates.{:?}.benchmark_requirements.{:?}.benchmark_artifact_digest",
                        phase, requirement.family
                    )
                    .as_str(),
                )?;
            }
            (None, None) => {
                if matches!(
                    requirement.family,
                    PsionBenchmarkFamily::ArchitectureReasoning
                        | PsionBenchmarkFamily::NormativeSpecReading
                        | PsionBenchmarkFamily::EngineeringSpecInterpretation
                        | PsionBenchmarkFamily::MemorizationVersusReasoning
                        | PsionBenchmarkFamily::RouteSelection
                        | PsionBenchmarkFamily::UnsupportedRequestRefusal
                ) {
                    return Err(PsionAcceptanceMatrixError::MissingField {
                        field: format!(
                            "phase_gates.{:?}.benchmark_requirements.{:?}.benchmark_artifact_id",
                            phase, requirement.family
                        ),
                    });
                }
            }
            _ => {
                return Err(PsionAcceptanceMatrixError::MissingField {
                    field: format!(
                        "phase_gates.{:?}.benchmark_requirements.{:?}.benchmark_artifact_binding",
                        phase, requirement.family
                    ),
                });
            }
        }
        if requirement.threshold_bands.is_empty() {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: format!(
                    "phase_gates.{:?}.benchmark_requirements.{:?}.threshold_bands",
                    phase, requirement.family
                ),
            });
        }
        let mut metrics = BTreeSet::new();
        for band in &requirement.threshold_bands {
            if !metrics.insert(band.metric_kind) {
                return Err(PsionAcceptanceMatrixError::DuplicateMetric {
                    phase,
                    family: requirement.family,
                    metric: band.metric_kind,
                });
            }
            if band.minimum_bps.is_none() && band.maximum_bps.is_none() {
                return Err(PsionAcceptanceMatrixError::MissingMetricBounds {
                    phase,
                    family: requirement.family,
                    metric: band.metric_kind,
                });
            }
            if let Some(minimum_bps) = band.minimum_bps {
                validate_bps(
                    minimum_bps,
                    format!(
                        "phase_gates.{:?}.benchmark_requirements.{:?}.threshold_bands.{:?}.minimum_bps",
                        phase, requirement.family, band.metric_kind
                    )
                    .as_str(),
                )?;
            }
            if let Some(maximum_bps) = band.maximum_bps {
                validate_bps(
                    maximum_bps,
                    format!(
                        "phase_gates.{:?}.benchmark_requirements.{:?}.threshold_bands.{:?}.maximum_bps",
                        phase, requirement.family, band.metric_kind
                    )
                    .as_str(),
                )?;
            }
            if let (Some(minimum_bps), Some(maximum_bps)) = (band.minimum_bps, band.maximum_bps) {
                if minimum_bps > maximum_bps {
                    return Err(PsionAcceptanceMatrixError::ThresholdRangeInverted {
                        phase,
                        family: requirement.family,
                        metric: band.metric_kind,
                        minimum_bps,
                        maximum_bps,
                    });
                }
            }
            validate_bps(
                band.allowed_regression_bps,
                format!(
                    "phase_gates.{:?}.benchmark_requirements.{:?}.threshold_bands.{:?}.allowed_regression_bps",
                    phase, requirement.family, band.metric_kind
                )
                .as_str(),
            )?;
            ensure_nonempty(
                band.rationale.as_str(),
                format!(
                    "phase_gates.{:?}.benchmark_requirements.{:?}.threshold_bands.{:?}.rationale",
                    phase, requirement.family, band.metric_kind
                )
                .as_str(),
            )?;
        }
        Ok(())
    }

    fn validate_contamination_requirement(
        &self,
        phase: PsionPhaseGate,
        requirement: &PsionContaminationReviewRequirement,
    ) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            requirement.rationale.as_str(),
            format!(
                "phase_gates.{:?}.contamination_review_requirement.rationale",
                phase
            )
            .as_str(),
        )?;
        if requirement.required_violation_consequences.is_empty() {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: format!(
                    "phase_gates.{:?}.contamination_review_requirement.required_violation_consequences",
                    phase
                ),
            });
        }
        reject_duplicate_enum_entries(
            requirement.required_violation_consequences.as_slice(),
            |consequence| PsionAcceptanceMatrixError::DuplicateContaminationConsequence {
                phase,
                consequence,
            },
        )?;
        Ok(())
    }

    fn validate_route_requirement(
        &self,
        phase: PsionPhaseGate,
        requirement: &PsionRouteCalibrationRequirement,
    ) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            requirement.rationale.as_str(),
            format!(
                "phase_gates.{:?}.route_calibration_requirement.rationale",
                phase
            )
            .as_str(),
        )?;
        if requirement.required_supported_routes.is_empty() {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: format!(
                    "phase_gates.{:?}.route_calibration_requirement.required_supported_routes",
                    phase
                ),
            });
        }
        let mut routes = BTreeSet::new();
        for route in &requirement.required_supported_routes {
            if !routes.insert(*route) {
                return Err(PsionAcceptanceMatrixError::DuplicateRoute {
                    phase,
                    route: *route,
                });
            }
        }
        validate_bps(
            requirement.minimum_route_selection_accuracy_bps,
            format!(
                "phase_gates.{:?}.route_calibration_requirement.minimum_route_selection_accuracy_bps",
                phase
            )
            .as_str(),
        )?;
        validate_bps(
            requirement.maximum_route_regression_bps,
            format!(
                "phase_gates.{:?}.route_calibration_requirement.maximum_route_regression_bps",
                phase
            )
            .as_str(),
        )?;
        Ok(())
    }

    fn validate_refusal_requirement(
        &self,
        phase: PsionPhaseGate,
        requirement: &PsionRefusalCalibrationRequirement,
    ) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            requirement.rationale.as_str(),
            format!(
                "phase_gates.{:?}.refusal_calibration_requirement.rationale",
                phase
            )
            .as_str(),
        )?;
        validate_bps(
            requirement.minimum_unsupported_request_refusal_bps,
            format!(
                "phase_gates.{:?}.refusal_calibration_requirement.minimum_unsupported_request_refusal_bps",
                phase
            )
            .as_str(),
        )?;
        validate_bps(
            requirement.maximum_overrefusal_bps,
            format!(
                "phase_gates.{:?}.refusal_calibration_requirement.maximum_overrefusal_bps",
                phase
            )
            .as_str(),
        )?;
        validate_bps(
            requirement.maximum_refusal_regression_bps,
            format!(
                "phase_gates.{:?}.refusal_calibration_requirement.maximum_refusal_regression_bps",
                phase
            )
            .as_str(),
        )?;
        Ok(())
    }

    fn validate_pilot_success_criteria(&self) -> Result<(), PsionAcceptanceMatrixError> {
        let criteria = &self.pilot_success_criteria;
        validate_bps(
            criteria.minimum_architecture_reasoning_pass_rate_bps,
            "pilot_success_criteria.minimum_architecture_reasoning_pass_rate_bps",
        )?;
        validate_bps(
            criteria.minimum_held_out_improvement_bps,
            "pilot_success_criteria.minimum_held_out_improvement_bps",
        )?;
        validate_bps(
            criteria.minimum_unsupported_request_refusal_bps,
            "pilot_success_criteria.minimum_unsupported_request_refusal_bps",
        )?;
        ensure_nonempty(
            criteria.rationale.as_str(),
            "pilot_success_criteria.rationale",
        )?;

        let pilot = self.phase_gate(PsionPhaseGate::Pilot).ok_or(
            PsionAcceptanceMatrixError::MissingPhase {
                phase: PsionPhaseGate::Pilot,
            },
        )?;
        require_pilot_threshold(
            pilot,
            PsionBenchmarkFamily::ArchitectureReasoning,
            PsionMetricKind::PassRateBps,
            criteria.minimum_architecture_reasoning_pass_rate_bps,
        )?;
        require_pilot_threshold(
            pilot,
            PsionBenchmarkFamily::HeldOutTechnicalReasoning,
            PsionMetricKind::ImprovementOverSeedBaselineBps,
            criteria.minimum_held_out_improvement_bps,
        )?;
        require_pilot_threshold(
            pilot,
            PsionBenchmarkFamily::UnsupportedRequestRefusal,
            PsionMetricKind::UnsupportedRequestRefusalBps,
            criteria.minimum_unsupported_request_refusal_bps,
        )?;
        if criteria.scale_up_blocked_without_clean_contamination_review {
            for phase in [
                PsionPhaseGate::BroaderPretraining,
                PsionPhaseGate::SftPromotion,
                PsionPhaseGate::InternalServing,
                PsionPhaseGate::TrustedClusterScaleUp,
            ] {
                let gate = self
                    .phase_gate(phase)
                    .ok_or(PsionAcceptanceMatrixError::MissingPhase { phase })?;
                if !gate
                    .contamination_review_requirement
                    .clean_review_required_for_promotion
                {
                    return Err(
                        PsionAcceptanceMatrixError::PilotScaleUpNotBlockedByCleanReview { phase },
                    );
                }
            }
        }
        Ok(())
    }
}

/// One observed benchmark or calibration metric in a decision receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionObservedMetric {
    /// Metric kind.
    pub metric_kind: PsionMetricKind,
    /// Observed value for the metric.
    pub observed_bps: u32,
    /// Observed regression against the last accepted baseline.
    pub regression_from_baseline_bps: u32,
}

/// Attached evidence for one required benchmark family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkEvidenceReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Phase this evidence was evaluated against.
    pub phase: PsionPhaseGate,
    /// Benchmark family.
    pub family: PsionBenchmarkFamily,
    /// Stable benchmark artifact identifier.
    pub benchmark_artifact_id: String,
    /// Stable digest over the benchmark artifact.
    pub benchmark_artifact_digest: String,
    /// Observed metrics carried on the receipt.
    pub metrics: Vec<PsionObservedMetric>,
    /// Short summary of the benchmark evidence.
    pub summary: String,
}

impl PsionBenchmarkEvidenceReceipt {
    fn metric(&self, metric_kind: PsionMetricKind) -> Option<&PsionObservedMetric> {
        self.metrics
            .iter()
            .find(|metric| metric.metric_kind == metric_kind)
    }
}

/// Replay evidence attached to one phase decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReplayEvidenceReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Number of successful replay receipts attached to the decision.
    pub successful_replays: u16,
    /// Whether exact replay was observed.
    pub exact_replay_observed: bool,
    /// Short summary of the replay evidence.
    pub summary: String,
}

/// Checkpoint recovery evidence attached to one phase decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCheckpointRecoveryReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable checkpoint or candidate artifact identifier.
    pub checkpoint_id: String,
    /// Number of successful restart round-trips.
    pub successful_restart_roundtrips: u16,
    /// Whether restart and recovery was observed.
    pub restart_recovery_observed: bool,
    /// Regression observed after resume.
    pub resume_regression_bps: u32,
    /// Short summary of the checkpoint evidence.
    pub summary: String,
}

/// Contamination review evidence attached to one phase decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionContaminationReviewReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Held-out isolation schema reviewed against.
    pub benchmark_isolation_schema_version: String,
    /// Stable digest over the reviewed exclusion manifest.
    pub exclusion_manifest_digest: String,
    /// Whether near-duplicate review completed.
    pub near_duplicate_review_completed: bool,
    /// Whether tokenizer exposure review completed.
    pub tokenizer_exposure_review_completed: bool,
    /// Final review disposition.
    pub disposition: PsionContaminationReviewDisposition,
    /// Consequences fired when contamination was found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied_consequences: Vec<PsionContaminationViolationConsequence>,
    /// Short summary of the contamination review.
    pub summary: String,
}

/// Route calibration evidence attached to one phase decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRouteCalibrationReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Routes covered by the calibration.
    pub covered_routes: Vec<PsionRouteKind>,
    /// Observed route-selection accuracy.
    pub route_selection_accuracy_bps: u32,
    /// Regression observed against the last accepted baseline.
    pub route_regression_bps: u32,
    /// Short summary of the route evidence.
    pub summary: String,
}

/// One recorded phase decision against the Psion acceptance matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPromotionDecisionReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable decision identifier.
    pub decision_id: String,
    /// Matrix identifier the decision targets.
    pub matrix_id: String,
    /// Matrix version the decision targets.
    pub matrix_version: String,
    /// Phase under review.
    pub phase: PsionPhaseGate,
    /// Stable candidate artifact identifier.
    pub candidate_artifact_id: String,
    /// Benchmark evidence required by the phase gate.
    pub benchmark_receipts: Vec<PsionBenchmarkEvidenceReceipt>,
    /// Replay evidence required by the phase gate.
    pub replay_receipt: PsionReplayEvidenceReceipt,
    /// Checkpoint evidence required by the phase gate.
    pub checkpoint_receipt: PsionCheckpointRecoveryReceipt,
    /// Contamination review evidence required by the phase gate.
    pub contamination_review_receipt: PsionContaminationReviewReceipt,
    /// Route calibration evidence required by the phase gate.
    pub route_calibration_receipt: PsionRouteCalibrationReceipt,
    /// Refusal calibration evidence required by the phase gate.
    pub refusal_calibration_receipt: PsionRefusalCalibrationReceipt,
    /// Final promotion disposition.
    pub decision: PsionPromotionDecisionDisposition,
    /// Gate-failure reasons when the decision was held.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hold_reason_codes: Vec<PsionPromotionHoldReasonCode>,
    /// Short operator-visible summary of the decision.
    pub decision_summary: String,
}

impl PsionPromotionDecisionReceipt {
    /// Validates the decision against the acceptance matrix.
    pub fn validate_against_matrix(
        &self,
        matrix: &PsionAcceptanceMatrix,
    ) -> Result<(), PsionAcceptanceMatrixError> {
        matrix.validate()?;

        ensure_nonempty(
            self.schema_version.as_str(),
            "promotion_decision.schema_version",
        )?;
        if self.schema_version != PSION_PROMOTION_DECISION_SCHEMA_VERSION {
            return Err(PsionAcceptanceMatrixError::SchemaVersionMismatch {
                expected: String::from(PSION_PROMOTION_DECISION_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.decision_id.as_str(), "promotion_decision.decision_id")?;
        ensure_nonempty(
            self.candidate_artifact_id.as_str(),
            "promotion_decision.candidate_artifact_id",
        )?;
        ensure_nonempty(
            self.decision_summary.as_str(),
            "promotion_decision.decision_summary",
        )?;
        if self.matrix_id != matrix.matrix_id || self.matrix_version != matrix.matrix_version {
            return Err(PsionAcceptanceMatrixError::DecisionMatrixMismatch {
                expected_id: matrix.matrix_id.clone(),
                expected_version: matrix.matrix_version.clone(),
                actual_id: self.matrix_id.clone(),
                actual_version: self.matrix_version.clone(),
            });
        }
        if self.benchmark_receipts.is_empty() {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: String::from("promotion_decision.benchmark_receipts"),
            });
        }

        let gate = matrix
            .phase_gate(self.phase)
            .ok_or(PsionAcceptanceMatrixError::MissingPhase { phase: self.phase })?;

        reject_duplicate_enum_entries(self.hold_reason_codes.as_slice(), |reason| {
            PsionAcceptanceMatrixError::DuplicateHoldReason { reason }
        })?;

        let mut families = BTreeSet::new();
        for receipt in &self.benchmark_receipts {
            if receipt.phase != self.phase {
                return Err(PsionAcceptanceMatrixError::BenchmarkReceiptPhaseMismatch {
                    family: receipt.family,
                    expected_phase: self.phase,
                    actual_phase: receipt.phase,
                });
            }
            if !families.insert(receipt.family) {
                return Err(PsionAcceptanceMatrixError::DuplicateBenchmarkReceipt {
                    phase: self.phase,
                    family: receipt.family,
                });
            }
            self.validate_benchmark_receipt(receipt)?;
        }
        for requirement in &gate.benchmark_requirements {
            if !families.contains(&requirement.family) {
                return Err(PsionAcceptanceMatrixError::MissingBenchmarkReceipt {
                    phase: self.phase,
                    family: requirement.family,
                });
            }
        }

        self.validate_replay_receipt()?;
        self.validate_checkpoint_receipt()?;
        self.validate_contamination_receipt(matrix)?;
        self.validate_route_receipt()?;
        self.validate_refusal_receipt()?;

        let required_hold_reasons = self.required_hold_reasons(gate)?;
        match self.decision {
            PsionPromotionDecisionDisposition::Promoted => {
                if !self.hold_reason_codes.is_empty() {
                    return Err(PsionAcceptanceMatrixError::PromotedDecisionHasHoldReasons {
                        phase: self.phase,
                    });
                }
                if let Some(reason) = required_hold_reasons.iter().next().copied() {
                    return Err(PsionAcceptanceMatrixError::PromotedDecisionBlocked {
                        phase: self.phase,
                        reason,
                    });
                }
            }
            PsionPromotionDecisionDisposition::Held => {
                if self.hold_reason_codes.is_empty() {
                    return Err(PsionAcceptanceMatrixError::HeldDecisionMissingHoldReasons {
                        phase: self.phase,
                    });
                }
                if required_hold_reasons.is_empty() {
                    return Err(PsionAcceptanceMatrixError::HeldDecisionWithoutGateFailure {
                        phase: self.phase,
                    });
                }
                for reason in &required_hold_reasons {
                    if !self.hold_reason_codes.contains(reason) {
                        return Err(PsionAcceptanceMatrixError::HeldDecisionMissingReason {
                            phase: self.phase,
                            reason: *reason,
                        });
                    }
                }
                for reason in &self.hold_reason_codes {
                    if !required_hold_reasons.contains(reason) {
                        return Err(PsionAcceptanceMatrixError::HeldDecisionUnexpectedReason {
                            phase: self.phase,
                            reason: *reason,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_benchmark_receipt(
        &self,
        receipt: &PsionBenchmarkEvidenceReceipt,
    ) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            receipt.receipt_id.as_str(),
            format!(
                "promotion_decision.benchmark_receipts.{:?}.receipt_id",
                receipt.family
            )
            .as_str(),
        )?;
        ensure_nonempty(
            receipt.benchmark_artifact_id.as_str(),
            format!(
                "promotion_decision.benchmark_receipts.{:?}.benchmark_artifact_id",
                receipt.family
            )
            .as_str(),
        )?;
        ensure_nonempty(
            receipt.benchmark_artifact_digest.as_str(),
            format!(
                "promotion_decision.benchmark_receipts.{:?}.benchmark_artifact_digest",
                receipt.family
            )
            .as_str(),
        )?;
        ensure_nonempty(
            receipt.summary.as_str(),
            format!(
                "promotion_decision.benchmark_receipts.{:?}.summary",
                receipt.family
            )
            .as_str(),
        )?;
        if receipt.metrics.is_empty() {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: format!(
                    "promotion_decision.benchmark_receipts.{:?}.metrics",
                    receipt.family
                ),
            });
        }
        let mut metrics = BTreeSet::new();
        for metric in &receipt.metrics {
            if !metrics.insert(metric.metric_kind) {
                return Err(PsionAcceptanceMatrixError::DuplicateMetric {
                    phase: self.phase,
                    family: receipt.family,
                    metric: metric.metric_kind,
                });
            }
            validate_bps(
                metric.observed_bps,
                format!(
                    "promotion_decision.benchmark_receipts.{:?}.metrics.{:?}.observed_bps",
                    receipt.family, metric.metric_kind
                )
                .as_str(),
            )?;
            validate_bps(
                metric.regression_from_baseline_bps,
                format!(
                    "promotion_decision.benchmark_receipts.{:?}.metrics.{:?}.regression_from_baseline_bps",
                    receipt.family, metric.metric_kind
                )
                .as_str(),
            )?;
        }
        Ok(())
    }

    fn validate_replay_receipt(&self) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            self.replay_receipt.receipt_id.as_str(),
            "promotion_decision.replay_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.replay_receipt.summary.as_str(),
            "promotion_decision.replay_receipt.summary",
        )?;
        Ok(())
    }

    fn validate_checkpoint_receipt(&self) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            self.checkpoint_receipt.receipt_id.as_str(),
            "promotion_decision.checkpoint_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.checkpoint_receipt.checkpoint_id.as_str(),
            "promotion_decision.checkpoint_receipt.checkpoint_id",
        )?;
        ensure_nonempty(
            self.checkpoint_receipt.summary.as_str(),
            "promotion_decision.checkpoint_receipt.summary",
        )?;
        validate_bps(
            self.checkpoint_receipt.resume_regression_bps,
            "promotion_decision.checkpoint_receipt.resume_regression_bps",
        )?;
        Ok(())
    }

    fn validate_contamination_receipt(
        &self,
        matrix: &PsionAcceptanceMatrix,
    ) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            self.contamination_review_receipt.receipt_id.as_str(),
            "promotion_decision.contamination_review_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.contamination_review_receipt
                .benchmark_isolation_schema_version
                .as_str(),
            "promotion_decision.contamination_review_receipt.benchmark_isolation_schema_version",
        )?;
        if self
            .contamination_review_receipt
            .benchmark_isolation_schema_version
            != matrix.benchmark_isolation_schema_version
        {
            return Err(
                PsionAcceptanceMatrixError::DecisionIsolationSchemaMismatch {
                    expected: matrix.benchmark_isolation_schema_version.clone(),
                    actual: self
                        .contamination_review_receipt
                        .benchmark_isolation_schema_version
                        .clone(),
                },
            );
        }
        ensure_nonempty(
            self.contamination_review_receipt
                .exclusion_manifest_digest
                .as_str(),
            "promotion_decision.contamination_review_receipt.exclusion_manifest_digest",
        )?;
        ensure_nonempty(
            self.contamination_review_receipt.summary.as_str(),
            "promotion_decision.contamination_review_receipt.summary",
        )?;
        reject_duplicate_enum_entries(
            self.contamination_review_receipt
                .applied_consequences
                .as_slice(),
            |consequence| PsionAcceptanceMatrixError::DuplicateAppliedConsequence { consequence },
        )?;
        Ok(())
    }

    fn validate_route_receipt(&self) -> Result<(), PsionAcceptanceMatrixError> {
        ensure_nonempty(
            self.route_calibration_receipt.receipt_id.as_str(),
            "promotion_decision.route_calibration_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.route_calibration_receipt.summary.as_str(),
            "promotion_decision.route_calibration_receipt.summary",
        )?;
        if self.route_calibration_receipt.covered_routes.is_empty() {
            return Err(PsionAcceptanceMatrixError::MissingField {
                field: String::from("promotion_decision.route_calibration_receipt.covered_routes"),
            });
        }
        let mut routes = BTreeSet::new();
        for route in &self.route_calibration_receipt.covered_routes {
            if !routes.insert(*route) {
                return Err(PsionAcceptanceMatrixError::DuplicateRecordedRoute { route: *route });
            }
        }
        validate_bps(
            self.route_calibration_receipt.route_selection_accuracy_bps,
            "promotion_decision.route_calibration_receipt.route_selection_accuracy_bps",
        )?;
        validate_bps(
            self.route_calibration_receipt.route_regression_bps,
            "promotion_decision.route_calibration_receipt.route_regression_bps",
        )?;
        Ok(())
    }

    fn validate_refusal_receipt(&self) -> Result<(), PsionAcceptanceMatrixError> {
        self.refusal_calibration_receipt.validate()?;
        Ok(())
    }

    fn required_hold_reasons(
        &self,
        gate: &PsionAcceptancePhaseGate,
    ) -> Result<BTreeSet<PsionPromotionHoldReasonCode>, PsionAcceptanceMatrixError> {
        let mut hold_reasons = BTreeSet::new();
        for requirement in &gate.benchmark_requirements {
            let receipt = self
                .benchmark_receipts
                .iter()
                .find(|receipt| receipt.family == requirement.family)
                .ok_or(PsionAcceptanceMatrixError::MissingBenchmarkReceipt {
                    phase: self.phase,
                    family: requirement.family,
                })?;
            if !benchmark_requirement_satisfied(self.phase, requirement, receipt)? {
                hold_reasons.insert(PsionPromotionHoldReasonCode::BenchmarkThresholdMissed);
            }
        }
        if self.replay_receipt.successful_replays
            < gate.replay_requirement.minimum_successful_replays
            || (gate.replay_requirement.exact_replay_required
                && !self.replay_receipt.exact_replay_observed)
        {
            hold_reasons.insert(PsionPromotionHoldReasonCode::ReplayRequirementMissed);
        }
        if gate.checkpoint_requirement.restart_recovery_required
            && (self.checkpoint_receipt.successful_restart_roundtrips
                < gate
                    .checkpoint_requirement
                    .minimum_successful_restart_roundtrips
                || !self.checkpoint_receipt.restart_recovery_observed
                || self.checkpoint_receipt.resume_regression_bps
                    > gate.checkpoint_requirement.maximum_resume_regression_bps)
        {
            hold_reasons.insert(PsionPromotionHoldReasonCode::CheckpointRequirementMissed);
        }

        let contamination = &self.contamination_review_receipt;
        let contamination_requirement = &gate.contamination_review_requirement;
        let contamination_failed = (contamination_requirement.near_duplicate_review_required
            && !contamination.near_duplicate_review_completed)
            || (contamination_requirement.tokenizer_exposure_review_required
                && !contamination.tokenizer_exposure_review_completed)
            || (contamination_requirement.clean_review_required_for_promotion
                && contamination.disposition != PsionContaminationReviewDisposition::Clean)
            || (matches!(
                contamination.disposition,
                PsionContaminationReviewDisposition::Contaminated
            ) && !contamination_requirement
                .required_violation_consequences
                .iter()
                .all(|consequence| contamination.applied_consequences.contains(consequence)));
        if contamination_failed {
            hold_reasons.insert(PsionPromotionHoldReasonCode::ContaminationReviewFailed);
        }

        let route_requirement = &gate.route_calibration_requirement;
        if !route_requirement
            .required_supported_routes
            .iter()
            .all(|route| {
                self.route_calibration_receipt
                    .covered_routes
                    .contains(route)
            })
            || self.route_calibration_receipt.route_selection_accuracy_bps
                < route_requirement.minimum_route_selection_accuracy_bps
            || self.route_calibration_receipt.route_regression_bps
                > route_requirement.maximum_route_regression_bps
        {
            hold_reasons.insert(PsionPromotionHoldReasonCode::RouteCalibrationFailed);
        }

        let refusal_requirement = &gate.refusal_calibration_requirement;
        if self
            .refusal_calibration_receipt
            .aggregate_unsupported_request_refusal_bps
            < refusal_requirement.minimum_unsupported_request_refusal_bps
            || self
                .refusal_calibration_receipt
                .supported_control_overrefusal_bps
                > refusal_requirement.maximum_overrefusal_bps
            || self.refusal_calibration_receipt.refusal_regression_bps
                > refusal_requirement.maximum_refusal_regression_bps
        {
            hold_reasons.insert(PsionPromotionHoldReasonCode::RefusalCalibrationFailed);
        }

        Ok(hold_reasons)
    }
}

/// Training-owned ledger for Psion phase-promotion decisions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPhasePromotionLedger {
    decisions: Vec<PsionPromotionDecisionReceipt>,
}

impl PsionPhasePromotionLedger {
    /// Records one decision after validating it against the matrix.
    pub fn record_decision(
        &mut self,
        matrix: &PsionAcceptanceMatrix,
        receipt: PsionPromotionDecisionReceipt,
    ) -> Result<(), PsionAcceptanceMatrixError> {
        receipt.validate_against_matrix(matrix)?;
        if self.decisions.iter().any(|existing| {
            existing.phase == receipt.phase
                && existing.candidate_artifact_id == receipt.candidate_artifact_id
        }) {
            return Err(PsionAcceptanceMatrixError::DuplicateRecordedDecision {
                phase: receipt.phase,
                candidate_artifact_id: receipt.candidate_artifact_id.clone(),
            });
        }
        self.decisions.push(receipt);
        Ok(())
    }

    /// Returns the recorded decisions in insertion order.
    #[must_use]
    pub fn decisions(&self) -> &[PsionPromotionDecisionReceipt] {
        self.decisions.as_slice()
    }
}

/// Error returned by Psion acceptance-matrix and promotion-decision validation.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionAcceptanceMatrixError {
    /// Refusal-calibration receipt validation failed.
    #[error(transparent)]
    RefusalCalibration(#[from] PsionRefusalCalibrationError),
    /// One required field was empty or missing.
    #[error("Psion acceptance contract field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// One schema version did not match the expected contract.
    #[error("Psion acceptance contract expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// The matrix repeated a phase gate.
    #[error("Psion acceptance matrix repeated phase `{phase:?}`")]
    DuplicatePhase {
        /// Duplicated phase.
        phase: PsionPhaseGate,
    },
    /// The matrix omitted a required phase gate.
    #[error("Psion acceptance matrix is missing phase `{phase:?}`")]
    MissingPhase {
        /// Missing phase.
        phase: PsionPhaseGate,
    },
    /// A phase repeated the same benchmark family.
    #[error("Psion phase `{phase:?}` repeated benchmark family `{family:?}`")]
    DuplicateBenchmarkFamily {
        /// Phase under validation.
        phase: PsionPhaseGate,
        /// Repeated family.
        family: PsionBenchmarkFamily,
    },
    /// A benchmark family repeated the same metric.
    #[error("Psion phase `{phase:?}` benchmark family `{family:?}` repeated metric `{metric:?}`")]
    DuplicateMetric {
        /// Phase under validation.
        phase: PsionPhaseGate,
        /// Benchmark family.
        family: PsionBenchmarkFamily,
        /// Repeated metric.
        metric: PsionMetricKind,
    },
    /// A threshold band omitted both minimum and maximum bounds.
    #[error(
        "Psion phase `{phase:?}` benchmark family `{family:?}` metric `{metric:?}` is missing threshold bounds"
    )]
    MissingMetricBounds {
        /// Phase under validation.
        phase: PsionPhaseGate,
        /// Benchmark family.
        family: PsionBenchmarkFamily,
        /// Metric kind.
        metric: PsionMetricKind,
    },
    /// A threshold band used an invalid range.
    #[error(
        "Psion phase `{phase:?}` benchmark family `{family:?}` metric `{metric:?}` inverted range `{minimum_bps}`..`{maximum_bps}`"
    )]
    ThresholdRangeInverted {
        /// Phase under validation.
        phase: PsionPhaseGate,
        /// Benchmark family.
        family: PsionBenchmarkFamily,
        /// Metric kind.
        metric: PsionMetricKind,
        /// Minimum value.
        minimum_bps: u32,
        /// Maximum value.
        maximum_bps: u32,
    },
    /// One bps field exceeded the valid range.
    #[error("Psion acceptance contract field `{field}` has invalid bps value `{value}`")]
    InvalidBps {
        /// Field path.
        field: String,
        /// Invalid value.
        value: u32,
    },
    /// One route was repeated in the matrix requirement.
    #[error("Psion phase `{phase:?}` repeated route `{route:?}`")]
    DuplicateRoute {
        /// Phase under validation.
        phase: PsionPhaseGate,
        /// Repeated route.
        route: PsionRouteKind,
    },
    /// One contamination consequence was repeated in the matrix requirement.
    #[error("Psion phase `{phase:?}` repeated contamination consequence `{consequence:?}`")]
    DuplicateContaminationConsequence {
        /// Phase under validation.
        phase: PsionPhaseGate,
        /// Repeated consequence.
        consequence: PsionContaminationViolationConsequence,
    },
    /// Pilot success criteria were not strong enough to block later scale-up.
    #[error(
        "Psion pilot criteria require clean contamination review before scale-up, but phase `{phase:?}` does not"
    )]
    PilotScaleUpNotBlockedByCleanReview {
        /// Phase that failed the policy.
        phase: PsionPhaseGate,
    },
    /// Pilot criteria referenced a missing benchmark family or threshold.
    #[error(
        "Psion pilot criteria require benchmark family `{family:?}` metric `{metric:?}` with minimum `{minimum_bps}`"
    )]
    MissingPilotThreshold {
        /// Required family.
        family: PsionBenchmarkFamily,
        /// Required metric.
        metric: PsionMetricKind,
        /// Required minimum.
        minimum_bps: u32,
    },
    /// Decision receipt targeted a different matrix id or version.
    #[error(
        "Psion promotion decision expected matrix `{expected_id}@{expected_version}`, found `{actual_id}@{actual_version}`"
    )]
    DecisionMatrixMismatch {
        /// Expected matrix identifier.
        expected_id: String,
        /// Expected matrix version.
        expected_version: String,
        /// Actual matrix identifier.
        actual_id: String,
        /// Actual matrix version.
        actual_version: String,
    },
    /// Decision receipt targeted a different held-out isolation schema.
    #[error("Psion promotion decision expected isolation schema `{expected}`, found `{actual}`")]
    DecisionIsolationSchemaMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// A required benchmark receipt was missing from the decision.
    #[error("Psion phase `{phase:?}` decision is missing benchmark receipt `{family:?}`")]
    MissingBenchmarkReceipt {
        /// Phase under review.
        phase: PsionPhaseGate,
        /// Missing benchmark family.
        family: PsionBenchmarkFamily,
    },
    /// One benchmark receipt referenced the wrong concrete artifact for the gate.
    #[error(
        "Psion phase `{phase:?}` benchmark family `{family:?}` expected artifact `{expected_id}` `{expected_digest}`, found `{actual_id}` `{actual_digest}`"
    )]
    BenchmarkArtifactMismatch {
        /// Phase under review.
        phase: PsionPhaseGate,
        /// Benchmark family.
        family: PsionBenchmarkFamily,
        /// Expected artifact id.
        expected_id: String,
        /// Expected artifact digest.
        expected_digest: String,
        /// Actual artifact id.
        actual_id: String,
        /// Actual artifact digest.
        actual_digest: String,
    },
    /// The decision repeated a benchmark family.
    #[error("Psion phase `{phase:?}` decision repeated benchmark receipt `{family:?}`")]
    DuplicateBenchmarkReceipt {
        /// Phase under review.
        phase: PsionPhaseGate,
        /// Repeated benchmark family.
        family: PsionBenchmarkFamily,
    },
    /// The benchmark receipt carried a mismatched phase.
    #[error(
        "Psion benchmark receipt `{family:?}` expected phase `{expected_phase:?}`, found `{actual_phase:?}`"
    )]
    BenchmarkReceiptPhaseMismatch {
        /// Benchmark family.
        family: PsionBenchmarkFamily,
        /// Expected phase.
        expected_phase: PsionPhaseGate,
        /// Actual phase.
        actual_phase: PsionPhaseGate,
    },
    /// A required benchmark metric was absent from the decision evidence.
    #[error(
        "Psion phase `{phase:?}` benchmark family `{family:?}` is missing observed metric `{metric:?}`"
    )]
    MissingObservedMetric {
        /// Phase under review.
        phase: PsionPhaseGate,
        /// Benchmark family.
        family: PsionBenchmarkFamily,
        /// Missing metric.
        metric: PsionMetricKind,
    },
    /// The decision repeated a hold reason.
    #[error("Psion promotion decision repeated hold reason `{reason:?}`")]
    DuplicateHoldReason {
        /// Repeated hold reason.
        reason: PsionPromotionHoldReasonCode,
    },
    /// The decision repeated a contamination consequence.
    #[error(
        "Psion promotion decision repeated applied contamination consequence `{consequence:?}`"
    )]
    DuplicateAppliedConsequence {
        /// Repeated applied consequence.
        consequence: PsionContaminationViolationConsequence,
    },
    /// The decision repeated a route in its route coverage list.
    #[error("Psion promotion decision repeated covered route `{route:?}`")]
    DuplicateRecordedRoute {
        /// Repeated route.
        route: PsionRouteKind,
    },
    /// A promoted decision still had an active blocking reason.
    #[error("Psion promoted decision for phase `{phase:?}` is blocked by `{reason:?}`")]
    PromotedDecisionBlocked {
        /// Phase under review.
        phase: PsionPhaseGate,
        /// Blocking reason.
        reason: PsionPromotionHoldReasonCode,
    },
    /// A promoted decision carried hold reasons even though it claimed success.
    #[error("Psion promoted decision for phase `{phase:?}` cannot carry hold reasons")]
    PromotedDecisionHasHoldReasons {
        /// Phase under review.
        phase: PsionPhaseGate,
    },
    /// A held decision omitted hold reasons.
    #[error("Psion held decision for phase `{phase:?}` must include hold reasons")]
    HeldDecisionMissingHoldReasons {
        /// Phase under review.
        phase: PsionPhaseGate,
    },
    /// A held decision had no gate failures at all.
    #[error("Psion held decision for phase `{phase:?}` has no gate failure to justify a hold")]
    HeldDecisionWithoutGateFailure {
        /// Phase under review.
        phase: PsionPhaseGate,
    },
    /// A held decision omitted a required reason code.
    #[error("Psion held decision for phase `{phase:?}` is missing hold reason `{reason:?}`")]
    HeldDecisionMissingReason {
        /// Phase under review.
        phase: PsionPhaseGate,
        /// Missing reason code.
        reason: PsionPromotionHoldReasonCode,
    },
    /// A held decision reported a reason code unsupported by the evidence.
    #[error(
        "Psion held decision for phase `{phase:?}` reported unexpected hold reason `{reason:?}`"
    )]
    HeldDecisionUnexpectedReason {
        /// Phase under review.
        phase: PsionPhaseGate,
        /// Unexpected reason code.
        reason: PsionPromotionHoldReasonCode,
    },
    /// The ledger already contains a decision for that phase and candidate.
    #[error(
        "Psion promotion ledger already recorded phase `{phase:?}` for candidate `{candidate_artifact_id}`"
    )]
    DuplicateRecordedDecision {
        /// Phase under review.
        phase: PsionPhaseGate,
        /// Candidate artifact identifier.
        candidate_artifact_id: String,
    },
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionAcceptanceMatrixError> {
    if value.trim().is_empty() {
        return Err(PsionAcceptanceMatrixError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn validate_bps(value: u32, field: &str) -> Result<(), PsionAcceptanceMatrixError> {
    if value > 10_000 {
        return Err(PsionAcceptanceMatrixError::InvalidBps {
            field: String::from(field),
            value,
        });
    }
    Ok(())
}

fn reject_duplicate_enum_entries<T, F>(
    entries: &[T],
    to_error: F,
) -> Result<(), PsionAcceptanceMatrixError>
where
    T: Copy + Ord,
    F: Fn(T) -> PsionAcceptanceMatrixError,
{
    let mut seen = BTreeSet::new();
    for entry in entries {
        if !seen.insert(*entry) {
            return Err(to_error(*entry));
        }
    }
    Ok(())
}

fn require_pilot_threshold(
    gate: &PsionAcceptancePhaseGate,
    family: PsionBenchmarkFamily,
    metric: PsionMetricKind,
    minimum_bps: u32,
) -> Result<(), PsionAcceptanceMatrixError> {
    let requirement = gate.benchmark_requirement(family).ok_or(
        PsionAcceptanceMatrixError::MissingPilotThreshold {
            family,
            metric,
            minimum_bps,
        },
    )?;
    let band = requirement
        .threshold_bands
        .iter()
        .find(|band| band.metric_kind == metric)
        .ok_or(PsionAcceptanceMatrixError::MissingPilotThreshold {
            family,
            metric,
            minimum_bps,
        })?;
    if band.minimum_bps.unwrap_or_default() < minimum_bps {
        return Err(PsionAcceptanceMatrixError::MissingPilotThreshold {
            family,
            metric,
            minimum_bps,
        });
    }
    Ok(())
}

fn benchmark_requirement_satisfied(
    phase: PsionPhaseGate,
    requirement: &PsionBenchmarkRequirement,
    receipt: &PsionBenchmarkEvidenceReceipt,
) -> Result<bool, PsionAcceptanceMatrixError> {
    if let (Some(expected_id), Some(expected_digest)) = (
        requirement.benchmark_artifact_id.as_deref(),
        requirement.benchmark_artifact_digest.as_deref(),
    ) {
        if receipt.benchmark_artifact_id != expected_id
            || receipt.benchmark_artifact_digest != expected_digest
        {
            return Err(PsionAcceptanceMatrixError::BenchmarkArtifactMismatch {
                phase,
                family: requirement.family,
                expected_id: expected_id.to_string(),
                expected_digest: expected_digest.to_string(),
                actual_id: receipt.benchmark_artifact_id.clone(),
                actual_digest: receipt.benchmark_artifact_digest.clone(),
            });
        }
    }
    for band in &requirement.threshold_bands {
        let metric = receipt.metric(band.metric_kind).ok_or(
            PsionAcceptanceMatrixError::MissingObservedMetric {
                phase,
                family: requirement.family,
                metric: band.metric_kind,
            },
        )?;
        if let Some(minimum_bps) = band.minimum_bps {
            if metric.observed_bps < minimum_bps {
                return Ok(false);
            }
        }
        if let Some(maximum_bps) = band.maximum_bps {
            if metric.observed_bps > maximum_bps {
                return Ok(false);
            }
        }
        if metric.regression_from_baseline_bps > band.allowed_regression_bps {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acceptance_matrix() -> PsionAcceptanceMatrix {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/acceptance/psion_acceptance_matrix_v1.json"
        ))
        .expect("acceptance matrix fixture should parse")
    }

    fn promotion_receipt() -> PsionPromotionDecisionReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/acceptance/psion_promotion_decision_receipt_v1.json"
        ))
        .expect("promotion decision receipt fixture should parse")
    }

    #[test]
    fn acceptance_matrix_validates_and_covers_all_required_phases() {
        let matrix = acceptance_matrix();
        matrix.validate().expect("matrix should validate");
        assert_eq!(
            matrix.phase_gates.len(),
            PsionPhaseGate::required_phases().len()
        );
        assert!(matrix
            .phase_gate(PsionPhaseGate::Pilot)
            .expect("pilot gate")
            .benchmark_requirement(PsionBenchmarkFamily::ArchitectureReasoning)
            .is_some());
        assert!(matrix
            .phase_gate(PsionPhaseGate::SftPromotion)
            .expect("sft promotion gate")
            .benchmark_requirement(PsionBenchmarkFamily::EngineeringSpecInterpretation)
            .is_some());
        assert!(matrix
            .phase_gate(PsionPhaseGate::SftPromotion)
            .expect("sft promotion gate")
            .benchmark_requirement(PsionBenchmarkFamily::MemorizationVersusReasoning)
            .is_some());
    }

    #[test]
    fn promotion_receipt_validates_and_records_in_ledger() {
        let matrix = acceptance_matrix();
        let receipt = promotion_receipt();
        receipt
            .validate_against_matrix(&matrix)
            .expect("promotion decision should validate");

        let mut ledger = PsionPhasePromotionLedger::default();
        ledger
            .record_decision(&matrix, receipt.clone())
            .expect("ledger should record one decision");
        assert_eq!(ledger.decisions().len(), 1);
        assert_eq!(
            ledger.decisions()[0].decision,
            PsionPromotionDecisionDisposition::Promoted
        );
    }

    #[test]
    fn missing_required_benchmark_receipt_blocks_recording() {
        let matrix = acceptance_matrix();
        let mut receipt = promotion_receipt();
        receipt
            .benchmark_receipts
            .retain(|benchmark| benchmark.family != PsionBenchmarkFamily::ArchitectureReasoning);
        let error = receipt
            .validate_against_matrix(&matrix)
            .expect_err("missing benchmark receipt should be rejected");
        assert!(matches!(
            error,
            PsionAcceptanceMatrixError::MissingBenchmarkReceipt { .. }
        ));
    }

    #[test]
    fn held_decision_requires_exact_matching_reason_codes() {
        let matrix = acceptance_matrix();
        let mut receipt = promotion_receipt();
        receipt.decision = PsionPromotionDecisionDisposition::Held;
        receipt
            .hold_reason_codes
            .push(PsionPromotionHoldReasonCode::ContaminationReviewFailed);
        receipt.contamination_review_receipt.disposition =
            PsionContaminationReviewDisposition::Contaminated;
        receipt.contamination_review_receipt.applied_consequences = vec![
            PsionContaminationViolationConsequence::InvalidateAffectedBenchmark,
            PsionContaminationViolationConsequence::TriggerCapabilityMatrixReview,
        ];
        receipt
            .validate_against_matrix(&matrix)
            .expect("held decision should validate when the reason matches the evidence");

        let mut unexpected = receipt.clone();
        unexpected
            .hold_reason_codes
            .push(PsionPromotionHoldReasonCode::ReplayRequirementMissed);
        let error = unexpected
            .validate_against_matrix(&matrix)
            .expect_err("unexpected reason code should be rejected");
        assert!(matches!(
            error,
            PsionAcceptanceMatrixError::HeldDecisionUnexpectedReason { .. }
        ));
    }
}
