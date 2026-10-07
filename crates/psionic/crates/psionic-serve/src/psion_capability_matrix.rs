use std::collections::BTreeSet;

use psionic_train::{
    PsionAcceptanceMatrix, PsionAcceptanceMatrixError, PsionPhaseGate,
    PsionPromotionDecisionDisposition, PsionPromotionDecisionReceipt, PsionRouteKind,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema version for the first Psion served capability matrix.
pub const PSION_CAPABILITY_MATRIX_SCHEMA_VERSION: &str = "psion.capability_matrix.v1";
/// Stable served product identifier for the first Psion learned-model lane.
pub const PSION_LEARNED_LANE_PRODUCT_ID: &str = "psionic.psion_learned_lane";

/// Published posture for one served task region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCapabilityPosture {
    /// Region can be served directly by the learned lane within the published bounds.
    Supported,
    /// Region must route to an exact or verifier-backed lane.
    RouteRequired,
    /// Region must refuse rather than answer directly.
    RefusalRequired,
    /// Region is out of the published learned-lane envelope.
    Unsupported,
}

/// Stable task region published for the first Psion capability matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCapabilityRegionId {
    /// Bounded architecture or systems reasoning inside the published context envelope.
    BoundedTechnicalReasoningShortContext,
    /// Bounded synthesis over specs and manuals inside the published context envelope.
    SpecificationAndManualSynthesis,
    /// Exact or verifier-backed computation requests.
    VerifiedOrExactExecutionRequests,
    /// Exact execution requests that do not expose a route into an exact executor surface.
    UnsupportedExactExecutionWithoutExecutorSurface,
    /// Design asks that omit required structured constraints.
    UnderspecifiedDesignWithoutRequiredConstraints,
    /// Requests that exceed the published context envelope.
    OverContextEnvelopeRequests,
    /// Requests that depend on currentness or hidden run artifacts.
    FreshnessOrRunArtifactDependentRequests,
    /// Open-ended assistant tasks outside the bounded learned lane.
    OpenEndedGeneralAssistantChat,
}

impl PsionCapabilityRegionId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BoundedTechnicalReasoningShortContext => {
                "bounded_technical_reasoning_short_context"
            }
            Self::SpecificationAndManualSynthesis => "specification_and_manual_synthesis",
            Self::VerifiedOrExactExecutionRequests => "verified_or_exact_execution_requests",
            Self::UnsupportedExactExecutionWithoutExecutorSurface => {
                "unsupported_exact_execution_without_executor_surface"
            }
            Self::UnderspecifiedDesignWithoutRequiredConstraints => {
                "underspecified_design_without_required_constraints"
            }
            Self::OverContextEnvelopeRequests => "over_context_envelope_requests",
            Self::FreshnessOrRunArtifactDependentRequests => {
                "freshness_or_run_artifact_dependent_requests"
            }
            Self::OpenEndedGeneralAssistantChat => "open_ended_general_assistant_chat",
        }
    }
}

/// Typed refusal reason published by the first Psion capability matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCapabilityRefusalReason {
    /// Exact-execution requests require a surfaced exact lane and cannot be improvised directly.
    UnsupportedExactnessRequest,
    /// The request omitted required structured constraints.
    MissingRequiredConstraints,
    /// Prompt or requested context exceeds the published envelope.
    UnsupportedContextLength,
    /// The request depends on current information or mutable run artifacts.
    CurrentnessOrRunArtifactDependency,
    /// The request depends on hidden tool or artifact state the lane does not expose.
    HiddenToolOrArtifactDependency,
    /// The request is an open-ended assistant ask outside the bounded learned lane.
    OpenEndedGeneralAssistantUnsupported,
}

/// Published context-length envelope for the served Psion lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityContextEnvelope {
    /// Maximum prompt tokens the lane supports directly.
    pub supported_prompt_tokens: u32,
    /// Maximum completion tokens the lane supports directly.
    pub supported_completion_tokens: u32,
    /// Prompt length after which the lane requires an explicit route instead of direct service.
    pub route_required_above_prompt_tokens: u32,
    /// Prompt length after which the lane must refuse.
    pub hard_refusal_above_prompt_tokens: u32,
    /// Short explanation of the published context boundary.
    pub detail: String,
}

/// Published latency envelope for the served Psion lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityLatencyEnvelope {
    /// P50 first-token latency in milliseconds.
    pub p50_first_token_latency_ms: u32,
    /// P95 first-token latency in milliseconds.
    pub p95_first_token_latency_ms: u32,
    /// P95 end-to-end latency in milliseconds.
    pub p95_end_to_end_latency_ms: u32,
    /// Short explanation of the latency envelope.
    pub detail: String,
}

/// Acceptance and receipt linkage carried by one published capability matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityPublicationBasis {
    /// Acceptance-matrix identifier.
    pub acceptance_matrix_id: String,
    /// Acceptance-matrix version.
    pub acceptance_matrix_version: String,
    /// Promoted phase that justified publication.
    pub acceptance_phase: PsionPhaseGate,
    /// Promotion-decision receipt identifier.
    pub promotion_decision_ref: String,
    /// Benchmark receipt identifiers carried forward from the promotion decision.
    pub benchmark_receipt_refs: Vec<String>,
    /// Replay receipt identifier carried forward from the promotion decision.
    pub replay_receipt_ref: String,
    /// Checkpoint receipt identifier carried forward from the promotion decision.
    pub checkpoint_receipt_ref: String,
    /// Contamination review receipt identifier carried forward from the promotion decision.
    pub contamination_review_receipt_ref: String,
    /// Route calibration receipt identifier carried forward from the promotion decision.
    pub route_calibration_receipt_ref: String,
    /// Refusal calibration receipt identifier carried forward from the promotion decision.
    pub refusal_calibration_receipt_ref: String,
}

impl PsionCapabilityPublicationBasis {
    /// Creates the publication basis directly from the promoted acceptance decision.
    #[must_use]
    pub fn from_promotion_decision(
        acceptance_matrix: &PsionAcceptanceMatrix,
        decision: &PsionPromotionDecisionReceipt,
    ) -> Self {
        Self {
            acceptance_matrix_id: acceptance_matrix.matrix_id.clone(),
            acceptance_matrix_version: acceptance_matrix.matrix_version.clone(),
            acceptance_phase: decision.phase,
            promotion_decision_ref: decision.decision_id.clone(),
            benchmark_receipt_refs: decision
                .benchmark_receipts
                .iter()
                .map(|receipt| receipt.receipt_id.clone())
                .collect(),
            replay_receipt_ref: decision.replay_receipt.receipt_id.clone(),
            checkpoint_receipt_ref: decision.checkpoint_receipt.receipt_id.clone(),
            contamination_review_receipt_ref: decision
                .contamination_review_receipt
                .receipt_id
                .clone(),
            route_calibration_receipt_ref: decision.route_calibration_receipt.receipt_id.clone(),
            refusal_calibration_receipt_ref: decision
                .refusal_calibration_receipt
                .receipt_id
                .clone(),
        }
    }
}

/// One published region in the served Psion capability matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityRegion {
    /// Stable task region identifier.
    pub region_id: PsionCapabilityRegionId,
    /// Published posture for the region.
    pub posture: PsionCapabilityPosture,
    /// Required route when the posture is `route_required`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_route: Option<PsionRouteKind>,
    /// Typed refusal reasons when the posture is `refusal_required` or `unsupported`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusal_reasons: Vec<PsionCapabilityRefusalReason>,
    /// Concrete examples that anchor the region.
    pub task_examples: Vec<String>,
    /// Plain-language boundary note for the region.
    pub detail: String,
}

/// Published served capability matrix for the first Psion learned lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityMatrix {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable matrix identifier.
    pub matrix_id: String,
    /// Stable matrix version.
    pub matrix_version: String,
    /// Served product identifier.
    pub product_id: String,
    /// Served model identifier.
    pub model_id: String,
    /// Acceptance and receipt linkage that justifies the publication.
    pub acceptance_basis: PsionCapabilityPublicationBasis,
    /// Published context envelope.
    pub context_envelope: PsionCapabilityContextEnvelope,
    /// Published latency envelope.
    pub latency_envelope: PsionCapabilityLatencyEnvelope,
    /// Published task regions.
    pub regions: Vec<PsionCapabilityRegion>,
    /// Plain-language claim boundary for the full matrix.
    pub claim_boundary: String,
}

impl PsionCapabilityMatrix {
    /// Creates one served capability matrix and validates the publication contract.
    pub fn new(
        matrix_id: impl Into<String>,
        matrix_version: impl Into<String>,
        model_id: impl Into<String>,
        context_envelope: PsionCapabilityContextEnvelope,
        latency_envelope: PsionCapabilityLatencyEnvelope,
        mut regions: Vec<PsionCapabilityRegion>,
        claim_boundary: impl Into<String>,
        acceptance_matrix: &PsionAcceptanceMatrix,
        decision: &PsionPromotionDecisionReceipt,
    ) -> Result<Self, PsionCapabilityMatrixError> {
        regions.sort_by_key(|region| region.region_id.as_str());
        let matrix = Self {
            schema_version: String::from(PSION_CAPABILITY_MATRIX_SCHEMA_VERSION),
            matrix_id: matrix_id.into(),
            matrix_version: matrix_version.into(),
            product_id: String::from(PSION_LEARNED_LANE_PRODUCT_ID),
            model_id: model_id.into(),
            acceptance_basis: PsionCapabilityPublicationBasis::from_promotion_decision(
                acceptance_matrix,
                decision,
            ),
            context_envelope,
            latency_envelope,
            regions,
            claim_boundary: claim_boundary.into(),
        };
        matrix.validate_publication(acceptance_matrix, decision)?;
        Ok(matrix)
    }

    /// Validates that the capability matrix is publishable against the promoted acceptance decision.
    pub fn validate_publication(
        &self,
        acceptance_matrix: &PsionAcceptanceMatrix,
        decision: &PsionPromotionDecisionReceipt,
    ) -> Result<(), PsionCapabilityMatrixError> {
        decision.validate_against_matrix(acceptance_matrix)?;
        if decision.decision != PsionPromotionDecisionDisposition::Promoted {
            return Err(PsionCapabilityMatrixError::DecisionNotPromoted {
                phase: decision.phase,
            });
        }
        ensure_nonempty(
            self.schema_version.as_str(),
            "capability_matrix.schema_version",
        )?;
        if self.schema_version != PSION_CAPABILITY_MATRIX_SCHEMA_VERSION {
            return Err(PsionCapabilityMatrixError::SchemaVersionMismatch {
                expected: String::from(PSION_CAPABILITY_MATRIX_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.matrix_id.as_str(), "capability_matrix.matrix_id")?;
        ensure_nonempty(
            self.matrix_version.as_str(),
            "capability_matrix.matrix_version",
        )?;
        ensure_nonempty(self.model_id.as_str(), "capability_matrix.model_id")?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "capability_matrix.claim_boundary",
        )?;
        if self.product_id != PSION_LEARNED_LANE_PRODUCT_ID {
            return Err(PsionCapabilityMatrixError::InvalidProductId {
                actual: self.product_id.clone(),
            });
        }
        self.validate_publication_basis(acceptance_matrix, decision)?;
        self.validate_context_envelope()?;
        self.validate_latency_envelope()?;
        self.validate_regions()?;
        Ok(())
    }

    fn validate_publication_basis(
        &self,
        acceptance_matrix: &PsionAcceptanceMatrix,
        decision: &PsionPromotionDecisionReceipt,
    ) -> Result<(), PsionCapabilityMatrixError> {
        let expected =
            PsionCapabilityPublicationBasis::from_promotion_decision(acceptance_matrix, decision);
        if self.acceptance_basis != expected {
            return Err(PsionCapabilityMatrixError::PublicationBasisMismatch {
                detail: String::from(
                    "acceptance matrix id/version, promoted phase, or receipt references drifted from the promoted decision",
                ),
            });
        }
        ensure_nonempty(
            self.acceptance_basis.acceptance_matrix_id.as_str(),
            "capability_matrix.acceptance_basis.acceptance_matrix_id",
        )?;
        ensure_nonempty(
            self.acceptance_basis.acceptance_matrix_version.as_str(),
            "capability_matrix.acceptance_basis.acceptance_matrix_version",
        )?;
        ensure_nonempty(
            self.acceptance_basis.promotion_decision_ref.as_str(),
            "capability_matrix.acceptance_basis.promotion_decision_ref",
        )?;
        if self.acceptance_basis.benchmark_receipt_refs.is_empty() {
            return Err(PsionCapabilityMatrixError::MissingField {
                field: String::from("capability_matrix.acceptance_basis.benchmark_receipt_refs"),
            });
        }
        let mut benchmark_refs = BTreeSet::new();
        for receipt_ref in &self.acceptance_basis.benchmark_receipt_refs {
            ensure_nonempty(
                receipt_ref.as_str(),
                "capability_matrix.acceptance_basis.benchmark_receipt_refs[]",
            )?;
            if !benchmark_refs.insert(receipt_ref.clone()) {
                return Err(PsionCapabilityMatrixError::PublicationBasisMismatch {
                    detail: format!("benchmark receipt ref `{receipt_ref}` repeated"),
                });
            }
        }
        ensure_nonempty(
            self.acceptance_basis.replay_receipt_ref.as_str(),
            "capability_matrix.acceptance_basis.replay_receipt_ref",
        )?;
        ensure_nonempty(
            self.acceptance_basis.checkpoint_receipt_ref.as_str(),
            "capability_matrix.acceptance_basis.checkpoint_receipt_ref",
        )?;
        ensure_nonempty(
            self.acceptance_basis
                .contamination_review_receipt_ref
                .as_str(),
            "capability_matrix.acceptance_basis.contamination_review_receipt_ref",
        )?;
        ensure_nonempty(
            self.acceptance_basis.route_calibration_receipt_ref.as_str(),
            "capability_matrix.acceptance_basis.route_calibration_receipt_ref",
        )?;
        ensure_nonempty(
            self.acceptance_basis
                .refusal_calibration_receipt_ref
                .as_str(),
            "capability_matrix.acceptance_basis.refusal_calibration_receipt_ref",
        )?;
        Ok(())
    }

    fn validate_context_envelope(&self) -> Result<(), PsionCapabilityMatrixError> {
        let envelope = &self.context_envelope;
        ensure_nonempty(
            envelope.detail.as_str(),
            "capability_matrix.context_envelope.detail",
        )?;
        if envelope.supported_prompt_tokens == 0 || envelope.supported_completion_tokens == 0 {
            return Err(PsionCapabilityMatrixError::InvalidContextEnvelope {
                detail: String::from(
                    "supported prompt and completion tokens must both be non-zero",
                ),
            });
        }
        if envelope.route_required_above_prompt_tokens <= envelope.supported_prompt_tokens {
            return Err(PsionCapabilityMatrixError::InvalidContextEnvelope {
                detail: String::from(
                    "route_required_above_prompt_tokens must be greater than supported_prompt_tokens",
                ),
            });
        }
        if envelope.hard_refusal_above_prompt_tokens <= envelope.route_required_above_prompt_tokens
        {
            return Err(PsionCapabilityMatrixError::InvalidContextEnvelope {
                detail: String::from(
                    "hard_refusal_above_prompt_tokens must be greater than route_required_above_prompt_tokens",
                ),
            });
        }
        Ok(())
    }

    fn validate_latency_envelope(&self) -> Result<(), PsionCapabilityMatrixError> {
        let envelope = &self.latency_envelope;
        ensure_nonempty(
            envelope.detail.as_str(),
            "capability_matrix.latency_envelope.detail",
        )?;
        if envelope.p50_first_token_latency_ms == 0
            || envelope.p95_first_token_latency_ms == 0
            || envelope.p95_end_to_end_latency_ms == 0
        {
            return Err(PsionCapabilityMatrixError::InvalidLatencyEnvelope {
                detail: String::from("published latency values must all be non-zero"),
            });
        }
        if envelope.p50_first_token_latency_ms > envelope.p95_first_token_latency_ms {
            return Err(PsionCapabilityMatrixError::InvalidLatencyEnvelope {
                detail: String::from(
                    "p50 first-token latency cannot exceed p95 first-token latency",
                ),
            });
        }
        if envelope.p95_first_token_latency_ms > envelope.p95_end_to_end_latency_ms {
            return Err(PsionCapabilityMatrixError::InvalidLatencyEnvelope {
                detail: String::from(
                    "p95 first-token latency cannot exceed p95 end-to-end latency",
                ),
            });
        }
        Ok(())
    }

    fn validate_regions(&self) -> Result<(), PsionCapabilityMatrixError> {
        if self.regions.is_empty() {
            return Err(PsionCapabilityMatrixError::MissingField {
                field: String::from("capability_matrix.regions"),
            });
        }

        let mut seen_regions = BTreeSet::new();
        let mut seen_postures = BTreeSet::new();
        for region in &self.regions {
            if !seen_regions.insert(region.region_id) {
                return Err(PsionCapabilityMatrixError::DuplicateRegion {
                    region_id: region.region_id,
                });
            }
            seen_postures.insert(region.posture);
            ensure_nonempty(
                region.detail.as_str(),
                format!(
                    "capability_matrix.regions.{}.detail",
                    region.region_id.as_str()
                )
                .as_str(),
            )?;
            if region.task_examples.is_empty() {
                return Err(PsionCapabilityMatrixError::MissingField {
                    field: format!(
                        "capability_matrix.regions.{}.task_examples",
                        region.region_id.as_str()
                    ),
                });
            }
            for example in &region.task_examples {
                ensure_nonempty(
                    example.as_str(),
                    format!(
                        "capability_matrix.regions.{}.task_examples[]",
                        region.region_id.as_str()
                    )
                    .as_str(),
                )?;
            }
            self.validate_region_contract(region)?;
        }

        for posture in [
            PsionCapabilityPosture::Supported,
            PsionCapabilityPosture::RouteRequired,
            PsionCapabilityPosture::RefusalRequired,
            PsionCapabilityPosture::Unsupported,
        ] {
            if !seen_postures.contains(&posture) {
                return Err(PsionCapabilityMatrixError::MissingPosture { posture });
            }
        }
        Ok(())
    }

    fn validate_region_contract(
        &self,
        region: &PsionCapabilityRegion,
    ) -> Result<(), PsionCapabilityMatrixError> {
        reject_duplicate_refusal_reasons(region)?;
        match region.posture {
            PsionCapabilityPosture::Supported => {
                if region.required_route.is_some() {
                    return Err(PsionCapabilityMatrixError::InvalidRegion {
                        region_id: region.region_id,
                        detail: String::from("supported regions cannot declare a required route"),
                    });
                }
                if !region.refusal_reasons.is_empty() {
                    return Err(PsionCapabilityMatrixError::InvalidRegion {
                        region_id: region.region_id,
                        detail: String::from("supported regions cannot declare refusal reasons"),
                    });
                }
            }
            PsionCapabilityPosture::RouteRequired => {
                if region.required_route != Some(PsionRouteKind::ExactExecutorHandoff) {
                    return Err(PsionCapabilityMatrixError::InvalidRegion {
                        region_id: region.region_id,
                        detail: String::from(
                            "route-required regions must declare `exact_executor_handoff`",
                        ),
                    });
                }
                if !region.refusal_reasons.is_empty() {
                    return Err(PsionCapabilityMatrixError::InvalidRegion {
                        region_id: region.region_id,
                        detail: String::from(
                            "route-required regions cannot also declare refusal reasons",
                        ),
                    });
                }
            }
            PsionCapabilityPosture::RefusalRequired | PsionCapabilityPosture::Unsupported => {
                if region.required_route.is_some() {
                    return Err(PsionCapabilityMatrixError::InvalidRegion {
                        region_id: region.region_id,
                        detail: String::from(
                            "refusal or unsupported regions cannot declare a required route",
                        ),
                    });
                }
                if region.refusal_reasons.is_empty() {
                    return Err(PsionCapabilityMatrixError::InvalidRegion {
                        region_id: region.region_id,
                        detail: String::from(
                            "refusal-required and unsupported regions must declare refusal reasons",
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Error returned by Psion capability-matrix publication validation.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionCapabilityMatrixError {
    /// The linked acceptance contract rejected the publication basis.
    #[error(transparent)]
    Acceptance(#[from] PsionAcceptanceMatrixError),
    /// One required field was empty or missing.
    #[error("Psion capability matrix field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// The matrix schema version did not match the current contract.
    #[error("Psion capability matrix expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// The matrix used an unexpected product identifier.
    #[error("Psion capability matrix expected product id `{PSION_LEARNED_LANE_PRODUCT_ID}`, found `{actual}`")]
    InvalidProductId {
        /// Actual product identifier.
        actual: String,
    },
    /// The promoted acceptance decision was not green.
    #[error("Psion capability matrix requires a promoted decision for phase `{phase:?}`")]
    DecisionNotPromoted {
        /// Phase linked by the decision.
        phase: PsionPhaseGate,
    },
    /// The publication basis drifted from the linked decision or receipt set.
    #[error("Psion capability matrix publication basis mismatch: {detail}")]
    PublicationBasisMismatch {
        /// Machine-readable detail.
        detail: String,
    },
    /// The context envelope was internally inconsistent.
    #[error("Psion capability matrix context envelope is invalid: {detail}")]
    InvalidContextEnvelope {
        /// Machine-readable detail.
        detail: String,
    },
    /// The latency envelope was internally inconsistent.
    #[error("Psion capability matrix latency envelope is invalid: {detail}")]
    InvalidLatencyEnvelope {
        /// Machine-readable detail.
        detail: String,
    },
    /// The matrix repeated a region.
    #[error("Psion capability matrix repeated region `{region_id:?}`")]
    DuplicateRegion {
        /// Repeated region.
        region_id: PsionCapabilityRegionId,
    },
    /// The matrix omitted one required posture class.
    #[error("Psion capability matrix is missing a `{posture:?}` region")]
    MissingPosture {
        /// Missing posture.
        posture: PsionCapabilityPosture,
    },
    /// One region violated the posture contract.
    #[error("Psion capability region `{region_id:?}` is invalid: {detail}")]
    InvalidRegion {
        /// Region identifier.
        region_id: PsionCapabilityRegionId,
        /// Machine-readable detail.
        detail: String,
    },
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionCapabilityMatrixError> {
    if value.trim().is_empty() {
        return Err(PsionCapabilityMatrixError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn reject_duplicate_refusal_reasons(
    region: &PsionCapabilityRegion,
) -> Result<(), PsionCapabilityMatrixError> {
    let mut reasons = BTreeSet::new();
    for reason in &region.refusal_reasons {
        if !reasons.insert(*reason) {
            return Err(PsionCapabilityMatrixError::InvalidRegion {
                region_id: region.region_id,
                detail: format!("refusal reason `{reason:?}` repeated"),
            });
        }
    }
    Ok(())
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
        .expect("promotion receipt fixture should parse")
    }

    fn capability_matrix() -> PsionCapabilityMatrix {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/capability/psion_capability_matrix_v1.json"
        ))
        .expect("capability matrix fixture should parse")
    }

    #[test]
    fn capability_matrix_fixture_validates_against_promoted_acceptance_decision() {
        let acceptance = acceptance_matrix();
        let decision = promotion_receipt();
        let matrix = capability_matrix();
        matrix
            .validate_publication(&acceptance, &decision)
            .expect("capability matrix should validate");
        assert!(matrix
            .regions
            .iter()
            .any(|region| region.posture == PsionCapabilityPosture::RouteRequired));
        assert!(matrix
            .regions
            .iter()
            .any(|region| region.posture == PsionCapabilityPosture::RefusalRequired));
    }

    #[test]
    fn capability_matrix_requires_every_posture_class() {
        let acceptance = acceptance_matrix();
        let decision = promotion_receipt();
        let mut matrix = capability_matrix();
        matrix
            .regions
            .retain(|region| region.posture != PsionCapabilityPosture::RouteRequired);
        let error = matrix
            .validate_publication(&acceptance, &decision)
            .expect_err("route-required posture should be mandatory");
        assert!(matches!(
            error,
            PsionCapabilityMatrixError::MissingPosture {
                posture: PsionCapabilityPosture::RouteRequired
            }
        ));
    }

    #[test]
    fn capability_matrix_rejects_receipt_linkage_drift() {
        let acceptance = acceptance_matrix();
        let decision = promotion_receipt();
        let mut matrix = capability_matrix();
        matrix.acceptance_basis.route_calibration_receipt_ref =
            String::from("different_route_receipt");
        let error = matrix
            .validate_publication(&acceptance, &decision)
            .expect_err("publication basis drift should be rejected");
        assert!(matches!(
            error,
            PsionCapabilityMatrixError::PublicationBasisMismatch { .. }
        ));
    }
}
