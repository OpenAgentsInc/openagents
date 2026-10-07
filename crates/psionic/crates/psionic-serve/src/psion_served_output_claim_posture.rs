use psionic_train::{PsionRouteClass, PsionRouteKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionCapabilityMatrix, PsionCapabilityRefusalReason, PsionCapabilityRegionId,
    PsionServedEvidenceBundle, PsionServedEvidenceKind,
};

/// Stable schema version for the first Psion served-output claim-posture contract.
pub const PSION_SERVED_OUTPUT_CLAIM_POSTURE_SCHEMA_VERSION: &str =
    "psion.served_output_claim_posture.v1";

/// Typed assumption class surfaced on one served Psion output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionServedAssumptionKind {
    /// The answer depends on explicit constraints or bounds supplied by the caller.
    InputConstraint,
    /// The answer depends on an interpretation boundary inside the bounded learned lane.
    InterpretationBoundary,
    /// The answer depends on missing structure or unresolved fields being called out explicitly.
    MissingStructuredInput,
    /// The answer depends on an explicit environment or currentness boundary.
    EnvironmentBoundary,
}

impl PsionServedAssumptionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputConstraint => "input_constraint",
            Self::InterpretationBoundary => "interpretation_boundary",
            Self::MissingStructuredInput => "missing_structured_input",
            Self::EnvironmentBoundary => "environment_boundary",
        }
    }
}

/// One explicit assumption notice surfaced with a served Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedAssumptionNotice {
    /// Stable assumption identifier within the output posture.
    pub assumption_id: String,
    /// Typed assumption class.
    pub kind: PsionServedAssumptionKind,
    /// Whether the assumption is necessary to interpret the served answer safely.
    pub required_for_interpretation: bool,
    /// Plain-language assumption text.
    pub detail: String,
}

impl PsionServedAssumptionNotice {
    fn validate(&self, field: &str) -> Result<(), PsionServedOutputClaimPostureError> {
        ensure_nonempty(
            self.assumption_id.as_str(),
            format!("{field}.assumption_id").as_str(),
        )?;
        ensure_nonempty(self.detail.as_str(), format!("{field}.detail").as_str())?;
        Ok(())
    }
}

/// Visible claim flags surfaced on one served Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedVisibleClaims {
    /// Whether the output explicitly presents itself as learned judgment.
    pub learned_judgment_visible: bool,
    /// Whether the output explicitly presents itself as source-grounded.
    pub source_grounding_visible: bool,
    /// Whether the output explicitly presents itself as executor-backed.
    pub executor_backing_visible: bool,
    /// Whether the output explicitly presents itself as benchmark-backed.
    pub benchmark_backing_visible: bool,
    /// Whether the output implies formal verification.
    pub verification_visible: bool,
}

/// Visible route or refusal behavior surfaced on one served Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "snake_case")]
pub enum PsionServedBehaviorVisibility {
    /// The output stayed in the served or delegated route path.
    Route {
        /// Coarse route selected for the output.
        route_kind: PsionRouteKind,
        /// Fine route class surfaced to the caller.
        route_class: PsionRouteClass,
        /// Plain-language route note.
        detail: String,
    },
    /// The output surfaced a typed refusal.
    Refusal {
        /// Capability region that triggered the refusal.
        capability_region_id: PsionCapabilityRegionId,
        /// Typed refusal reason surfaced to the caller.
        refusal_reason: PsionCapabilityRefusalReason,
        /// Plain-language refusal note.
        detail: String,
    },
}

impl PsionServedBehaviorVisibility {
    fn validate(&self) -> Result<(), PsionServedOutputClaimPostureError> {
        let detail = match self {
            Self::Route { detail, .. } | Self::Refusal { detail, .. } => detail,
        };
        ensure_nonempty(
            detail.as_str(),
            "psion_served_output_claim_posture.behavior_visibility.detail",
        )?;
        Ok(())
    }
}

/// Published context envelope surfaced on one served Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedContextEnvelopeSurface {
    /// Maximum prompt tokens the lane supports directly.
    pub supported_prompt_tokens: u32,
    /// Maximum completion tokens the lane supports directly.
    pub supported_completion_tokens: u32,
    /// Prompt length after which the lane must route or refuse.
    pub route_required_above_prompt_tokens: u32,
    /// Prompt length after which the lane must refuse.
    pub hard_refusal_above_prompt_tokens: u32,
    /// Prompt tokens observed on the served request.
    pub prompt_tokens_observed: u32,
    /// Plain-language context-envelope note surfaced with the output.
    pub detail: String,
}

impl PsionServedContextEnvelopeSurface {
    fn validate(&self) -> Result<(), PsionServedOutputClaimPostureError> {
        ensure_nonempty(
            self.detail.as_str(),
            "psion_served_output_claim_posture.context_envelope.detail",
        )?;
        Ok(())
    }
}

/// Published latency envelope surfaced on one served Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedLatencyEnvelopeSurface {
    /// Published p50 first-token latency limit.
    pub p50_first_token_latency_ms: u32,
    /// Published p95 first-token latency limit.
    pub p95_first_token_latency_ms: u32,
    /// Published p95 end-to-end latency limit.
    pub p95_end_to_end_latency_ms: u32,
    /// Plain-language latency-envelope note surfaced with the output.
    pub detail: String,
}

impl PsionServedLatencyEnvelopeSurface {
    fn validate(&self) -> Result<(), PsionServedOutputClaimPostureError> {
        ensure_nonempty(
            self.detail.as_str(),
            "psion_served_output_claim_posture.latency_envelope.detail",
        )?;
        Ok(())
    }
}

/// Shared claim-posture contract surfaced on one served Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedOutputClaimPosture {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable posture identifier.
    pub posture_id: String,
    /// Capability-matrix identifier the posture targets.
    pub capability_matrix_id: String,
    /// Capability-matrix version the posture targets.
    pub capability_matrix_version: String,
    /// Evidence bundle identifier the claim posture is grounded in.
    pub evidence_bundle_id: String,
    /// Stable digest over the attached evidence bundle.
    pub evidence_bundle_digest: String,
    /// Visible claim flags surfaced to the caller.
    pub visible_claims: PsionServedVisibleClaims,
    /// Explicit assumptions surfaced to the caller.
    pub assumptions: Vec<PsionServedAssumptionNotice>,
    /// Explicit visible route or refusal behavior.
    pub behavior_visibility: PsionServedBehaviorVisibility,
    /// Published context envelope surfaced with the output.
    pub context_envelope: PsionServedContextEnvelopeSurface,
    /// Published latency envelope surfaced with the output.
    pub latency_envelope: PsionServedLatencyEnvelopeSurface,
    /// Short summary of the bounded claim posture.
    pub summary: String,
    /// Stable digest over the claim posture.
    pub posture_digest: String,
}

impl PsionServedOutputClaimPosture {
    /// Validates the claim posture against the shared schema, evidence bundle, and capability matrix.
    pub fn validate_against_evidence_and_capability_matrix(
        &self,
        evidence_bundle: &PsionServedEvidenceBundle,
        capability_matrix: &PsionCapabilityMatrix,
    ) -> Result<(), PsionServedOutputClaimPostureError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_served_output_claim_posture.schema_version",
        )?;
        if self.schema_version != PSION_SERVED_OUTPUT_CLAIM_POSTURE_SCHEMA_VERSION {
            return Err(PsionServedOutputClaimPostureError::SchemaVersionMismatch {
                expected: String::from(PSION_SERVED_OUTPUT_CLAIM_POSTURE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.posture_id.as_str(),
            "psion_served_output_claim_posture.posture_id",
        )?;
        ensure_nonempty(
            self.capability_matrix_id.as_str(),
            "psion_served_output_claim_posture.capability_matrix_id",
        )?;
        ensure_nonempty(
            self.capability_matrix_version.as_str(),
            "psion_served_output_claim_posture.capability_matrix_version",
        )?;
        ensure_nonempty(
            self.evidence_bundle_id.as_str(),
            "psion_served_output_claim_posture.evidence_bundle_id",
        )?;
        ensure_nonempty(
            self.evidence_bundle_digest.as_str(),
            "psion_served_output_claim_posture.evidence_bundle_digest",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_served_output_claim_posture.summary",
        )?;

        evidence_bundle.validate_against_capability_matrix(capability_matrix)?;
        check_string_match(
            self.capability_matrix_id.as_str(),
            capability_matrix.matrix_id.as_str(),
            "psion_served_output_claim_posture.capability_matrix_id",
        )?;
        check_string_match(
            self.capability_matrix_version.as_str(),
            capability_matrix.matrix_version.as_str(),
            "psion_served_output_claim_posture.capability_matrix_version",
        )?;
        check_string_match(
            self.evidence_bundle_id.as_str(),
            evidence_bundle.bundle_id.as_str(),
            "psion_served_output_claim_posture.evidence_bundle_id",
        )?;
        check_string_match(
            self.evidence_bundle_digest.as_str(),
            evidence_bundle.bundle_digest.as_str(),
            "psion_served_output_claim_posture.evidence_bundle_digest",
        )?;

        if self.assumptions.is_empty() {
            return Err(PsionServedOutputClaimPostureError::MissingAssumptions);
        }
        for (index, assumption) in self.assumptions.iter().enumerate() {
            assumption.validate(
                format!("psion_served_output_claim_posture.assumptions[{index}]").as_str(),
            )?;
        }

        self.behavior_visibility.validate()?;
        match &self.behavior_visibility {
            PsionServedBehaviorVisibility::Route {
                route_kind,
                route_class,
                ..
            } => {
                let route_receipt = evidence_bundle.route_receipt.as_ref().ok_or(
                    PsionServedOutputClaimPostureError::BehaviorDoesNotMatchEvidenceBundle {
                        expected: String::from("route_receipt"),
                        actual: String::from("refusal_receipt_or_missing_route"),
                    },
                )?;
                if route_receipt.route_kind != *route_kind
                    || route_receipt.route_class != *route_class
                {
                    return Err(
                        PsionServedOutputClaimPostureError::BehaviorDoesNotMatchEvidenceBundle {
                            expected: format!(
                                "{}:{}",
                                route_kind_label(*route_kind),
                                route_class_label(*route_class)
                            ),
                            actual: format!(
                                "{}:{}",
                                route_kind_label(route_receipt.route_kind),
                                route_class_label(route_receipt.route_class)
                            ),
                        },
                    );
                }
            }
            PsionServedBehaviorVisibility::Refusal {
                capability_region_id,
                refusal_reason,
                ..
            } => {
                let refusal_receipt = evidence_bundle.refusal_receipt.as_ref().ok_or(
                    PsionServedOutputClaimPostureError::BehaviorDoesNotMatchEvidenceBundle {
                        expected: String::from("refusal_receipt"),
                        actual: String::from("route_receipt_or_missing_refusal"),
                    },
                )?;
                if refusal_receipt.capability_region_id != *capability_region_id
                    || refusal_receipt.refusal_reason != *refusal_reason
                {
                    return Err(
                        PsionServedOutputClaimPostureError::BehaviorDoesNotMatchEvidenceBundle {
                            expected: format!(
                                "{}:{}",
                                capability_region_id.as_str(),
                                refusal_reason_label(*refusal_reason)
                            ),
                            actual: format!(
                                "{}:{}",
                                refusal_receipt.capability_region_id.as_str(),
                                refusal_reason_label(refusal_receipt.refusal_reason)
                            ),
                        },
                    );
                }
            }
        }

        self.context_envelope.validate()?;
        if self.context_envelope.supported_prompt_tokens
            != capability_matrix.context_envelope.supported_prompt_tokens
            || self.context_envelope.supported_completion_tokens
                != capability_matrix.context_envelope.supported_completion_tokens
            || self.context_envelope.route_required_above_prompt_tokens
                != capability_matrix.context_envelope.route_required_above_prompt_tokens
            || self.context_envelope.hard_refusal_above_prompt_tokens
                != capability_matrix.context_envelope.hard_refusal_above_prompt_tokens
        {
            return Err(PsionServedOutputClaimPostureError::EnvelopeMismatch {
                field: String::from("context_envelope"),
            });
        }
        self.latency_envelope.validate()?;
        if self.latency_envelope.p50_first_token_latency_ms
            != capability_matrix.latency_envelope.p50_first_token_latency_ms
            || self.latency_envelope.p95_first_token_latency_ms
                != capability_matrix.latency_envelope.p95_first_token_latency_ms
            || self.latency_envelope.p95_end_to_end_latency_ms
                != capability_matrix.latency_envelope.p95_end_to_end_latency_ms
        {
            return Err(PsionServedOutputClaimPostureError::EnvelopeMismatch {
                field: String::from("latency_envelope"),
            });
        }

        if self.visible_claims.verification_visible {
            return Err(PsionServedOutputClaimPostureError::VerificationClaimsUnsupported);
        }
        require_visible_claim_backing(
            self.visible_claims.learned_judgment_visible,
            PsionServedEvidenceKind::LearnedJudgment,
            evidence_bundle,
        )?;
        require_visible_claim_backing(
            self.visible_claims.source_grounding_visible,
            PsionServedEvidenceKind::SourceGroundedStatement,
            evidence_bundle,
        )?;
        require_visible_claim_backing(
            self.visible_claims.executor_backing_visible,
            PsionServedEvidenceKind::ExecutorBackedResult,
            evidence_bundle,
        )?;
        require_visible_claim_backing(
            self.visible_claims.benchmark_backing_visible,
            PsionServedEvidenceKind::BenchmarkBackedCapabilityClaim,
            evidence_bundle,
        )?;

        let expected_digest = stable_psion_served_output_claim_posture_digest(self);
        if self.posture_digest != expected_digest {
            return Err(PsionServedOutputClaimPostureError::DigestMismatch {
                expected: expected_digest,
                actual: self.posture_digest.clone(),
            });
        }

        Ok(())
    }
}

/// Records one served-output claim posture with canonical schema version and digest.
pub fn record_psion_served_output_claim_posture(
    posture_id: impl Into<String>,
    capability_matrix: &PsionCapabilityMatrix,
    evidence_bundle: &PsionServedEvidenceBundle,
    visible_claims: PsionServedVisibleClaims,
    mut assumptions: Vec<PsionServedAssumptionNotice>,
    behavior_visibility: PsionServedBehaviorVisibility,
    context_envelope: PsionServedContextEnvelopeSurface,
    latency_envelope: PsionServedLatencyEnvelopeSurface,
    summary: impl Into<String>,
) -> Result<PsionServedOutputClaimPosture, PsionServedOutputClaimPostureError> {
    assumptions.sort_by(|left, right| left.assumption_id.cmp(&right.assumption_id));
    let mut posture = PsionServedOutputClaimPosture {
        schema_version: String::from(PSION_SERVED_OUTPUT_CLAIM_POSTURE_SCHEMA_VERSION),
        posture_id: posture_id.into(),
        capability_matrix_id: capability_matrix.matrix_id.clone(),
        capability_matrix_version: capability_matrix.matrix_version.clone(),
        evidence_bundle_id: evidence_bundle.bundle_id.clone(),
        evidence_bundle_digest: evidence_bundle.bundle_digest.clone(),
        visible_claims,
        assumptions,
        behavior_visibility,
        context_envelope,
        latency_envelope,
        summary: summary.into(),
        posture_digest: String::new(),
    };
    posture.posture_digest = stable_psion_served_output_claim_posture_digest(&posture);
    posture.validate_against_evidence_and_capability_matrix(evidence_bundle, capability_matrix)?;
    Ok(posture)
}

/// Validation errors for Psion served-output claim posture.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PsionServedOutputClaimPostureError {
    #[error("Psion served-output claim posture is missing `{field}`")]
    MissingField { field: String },
    #[error(
        "Psion served-output claim posture schema version mismatch: expected `{expected}`, got `{actual}`"
    )]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("Psion served-output claim posture requires at least one explicit assumption")]
    MissingAssumptions,
    #[error(
        "Psion served-output claim posture behavior does not match the attached evidence bundle: expected `{expected}`, got `{actual}`"
    )]
    BehaviorDoesNotMatchEvidenceBundle {
        expected: String,
        actual: String,
    },
    #[error("Psion served-output claim posture envelope `{field}` does not match the capability matrix")]
    EnvelopeMismatch { field: String },
    #[error("Psion served-output claim posture may not imply verification on the current learned lane")]
    VerificationClaimsUnsupported,
    #[error(
        "Psion served-output claim posture surfaced `{claim_kind}` without attached supporting evidence"
    )]
    MissingAttachedEvidence { claim_kind: String },
    #[error("Psion served-output claim posture digest mismatch: expected `{expected}`, got `{actual}`")]
    DigestMismatch { expected: String, actual: String },
    #[error("Psion served-output claim posture mismatch for `{field}`: expected `{expected}`, got `{actual}`")]
    Mismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("{0}")]
    ServedEvidence(#[from] crate::PsionServedEvidenceError),
}

fn require_visible_claim_backing(
    claim_visible: bool,
    required_kind: PsionServedEvidenceKind,
    evidence_bundle: &PsionServedEvidenceBundle,
) -> Result<(), PsionServedOutputClaimPostureError> {
    if claim_visible
        && !evidence_bundle
            .evidence_labels
            .iter()
            .any(|label| label.kind() == required_kind)
    {
        return Err(PsionServedOutputClaimPostureError::MissingAttachedEvidence {
            claim_kind: required_kind.as_str().to_string(),
        });
    }
    Ok(())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionServedOutputClaimPostureError> {
    if value.trim().is_empty() {
        return Err(PsionServedOutputClaimPostureError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionServedOutputClaimPostureError> {
    ensure_nonempty(actual, field)?;
    ensure_nonempty(expected, field)?;
    if actual != expected {
        return Err(PsionServedOutputClaimPostureError::Mismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn stable_psion_served_output_claim_posture_digest(
    posture: &PsionServedOutputClaimPosture,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_served_output_claim_posture|");
    hasher.update(posture.schema_version.as_bytes());
    hasher.update(posture.posture_id.as_bytes());
    hasher.update(posture.capability_matrix_id.as_bytes());
    hasher.update(posture.capability_matrix_version.as_bytes());
    hasher.update(posture.evidence_bundle_id.as_bytes());
    hasher.update(posture.evidence_bundle_digest.as_bytes());

    hasher.update(if posture.visible_claims.learned_judgment_visible {
        "learned_visible|".as_bytes()
    } else {
        "learned_hidden|".as_bytes()
    });
    hasher.update(if posture.visible_claims.source_grounding_visible {
        "source_visible|".as_bytes()
    } else {
        "source_hidden|".as_bytes()
    });
    hasher.update(if posture.visible_claims.executor_backing_visible {
        "executor_visible|".as_bytes()
    } else {
        "executor_hidden|".as_bytes()
    });
    hasher.update(if posture.visible_claims.benchmark_backing_visible {
        "benchmark_visible|".as_bytes()
    } else {
        "benchmark_hidden|".as_bytes()
    });
    hasher.update(if posture.visible_claims.verification_visible {
        "verification_visible|".as_bytes()
    } else {
        "verification_hidden|".as_bytes()
    });

    for assumption in &posture.assumptions {
        hasher.update(assumption.assumption_id.as_bytes());
        hasher.update(assumption.kind.as_str().as_bytes());
        hasher.update(if assumption.required_for_interpretation {
            "required|".as_bytes()
        } else {
            "optional|".as_bytes()
        });
        hasher.update(assumption.detail.as_bytes());
    }

    match &posture.behavior_visibility {
        PsionServedBehaviorVisibility::Route {
            route_kind,
            route_class,
            detail,
        } => {
            hasher.update(b"route|");
            hasher.update(route_kind_label(*route_kind).as_bytes());
            hasher.update(route_class_label(*route_class).as_bytes());
            hasher.update(detail.as_bytes());
        }
        PsionServedBehaviorVisibility::Refusal {
            capability_region_id,
            refusal_reason,
            detail,
        } => {
            hasher.update(b"refusal|");
            hasher.update(capability_region_id.as_str().as_bytes());
            hasher.update(refusal_reason_label(*refusal_reason).as_bytes());
            hasher.update(detail.as_bytes());
        }
    }

    hasher.update(posture.context_envelope.supported_prompt_tokens.to_string().as_bytes());
    hasher.update(
        posture
            .context_envelope
            .supported_completion_tokens
            .to_string()
            .as_bytes(),
    );
    hasher.update(
        posture
            .context_envelope
            .route_required_above_prompt_tokens
            .to_string()
            .as_bytes(),
    );
    hasher.update(
        posture
            .context_envelope
            .hard_refusal_above_prompt_tokens
            .to_string()
            .as_bytes(),
    );
    hasher.update(
        posture
            .context_envelope
            .prompt_tokens_observed
            .to_string()
            .as_bytes(),
    );
    hasher.update(posture.context_envelope.detail.as_bytes());

    hasher.update(
        posture
            .latency_envelope
            .p50_first_token_latency_ms
            .to_string()
            .as_bytes(),
    );
    hasher.update(
        posture
            .latency_envelope
            .p95_first_token_latency_ms
            .to_string()
            .as_bytes(),
    );
    hasher.update(
        posture
            .latency_envelope
            .p95_end_to_end_latency_ms
            .to_string()
            .as_bytes(),
    );
    hasher.update(posture.latency_envelope.detail.as_bytes());
    hasher.update(posture.summary.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn route_kind_label(route_kind: PsionRouteKind) -> &'static str {
    match route_kind {
        PsionRouteKind::DirectModelAnswer => "direct_model_answer",
        PsionRouteKind::ExactExecutorHandoff => "exact_executor_handoff",
        PsionRouteKind::Refusal => "refusal",
    }
}

fn route_class_label(route_class: PsionRouteClass) -> &'static str {
    match route_class {
        PsionRouteClass::AnswerInLanguage => "answer_in_language",
        PsionRouteClass::AnswerWithUncertainty => "answer_with_uncertainty",
        PsionRouteClass::RequestStructuredInputs => "request_structured_inputs",
        PsionRouteClass::DelegateToExactExecutor => "delegate_to_exact_executor",
    }
}

fn refusal_reason_label(reason: PsionCapabilityRefusalReason) -> &'static str {
    match reason {
        PsionCapabilityRefusalReason::UnsupportedExactnessRequest => {
            "unsupported_exactness_request"
        }
        PsionCapabilityRefusalReason::MissingRequiredConstraints => {
            "missing_required_constraints"
        }
        PsionCapabilityRefusalReason::UnsupportedContextLength => {
            "unsupported_context_length"
        }
        PsionCapabilityRefusalReason::CurrentnessOrRunArtifactDependency => {
            "currentness_or_run_artifact_dependency"
        }
        PsionCapabilityRefusalReason::HiddenToolOrArtifactDependency => {
            "hidden_tool_or_artifact_dependency"
        }
        PsionCapabilityRefusalReason::OpenEndedGeneralAssistantUnsupported => {
            "open_ended_general_assistant_unsupported"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability_matrix() -> PsionCapabilityMatrix {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/capability/psion_capability_matrix_v1.json"
        ))
        .expect("capability matrix fixture should parse")
    }

    #[test]
    fn served_output_claim_examples_validate_against_evidence_and_capability_matrix() {
        let capability_matrix = capability_matrix();
        for (claim_fixture, evidence_fixture) in [
            (
                include_str!(
                    "../../../fixtures/psion/serve/psion_served_output_claim_direct_v1.json"
                ),
                include_str!(
                    "../../../fixtures/psion/serve/psion_served_evidence_direct_grounded_v1.json"
                ),
            ),
            (
                include_str!(
                    "../../../fixtures/psion/serve/psion_served_output_claim_executor_backed_v1.json"
                ),
                include_str!(
                    "../../../fixtures/psion/serve/psion_served_evidence_executor_backed_v1.json"
                ),
            ),
            (
                include_str!(
                    "../../../fixtures/psion/serve/psion_served_output_claim_refusal_v1.json"
                ),
                include_str!(
                    "../../../fixtures/psion/serve/psion_served_evidence_refusal_v1.json"
                ),
            ),
        ] {
            let posture: PsionServedOutputClaimPosture =
                serde_json::from_str(claim_fixture).expect("claim posture fixture should parse");
            let evidence_bundle: PsionServedEvidenceBundle =
                serde_json::from_str(evidence_fixture).expect("evidence fixture should parse");
            posture
                .validate_against_evidence_and_capability_matrix(
                    &evidence_bundle,
                    &capability_matrix,
                )
                .expect("claim posture fixture should validate");
        }
    }

    #[test]
    fn visible_executor_claim_requires_executor_backed_evidence() {
        let capability_matrix = capability_matrix();
        let evidence_bundle: PsionServedEvidenceBundle = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_served_evidence_direct_grounded_v1.json"
        ))
        .expect("evidence fixture should parse");
        let mut posture: PsionServedOutputClaimPosture = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_served_output_claim_direct_v1.json"
        ))
        .expect("claim posture fixture should parse");
        posture.visible_claims.executor_backing_visible = true;
        posture.posture_digest = stable_psion_served_output_claim_posture_digest(&posture);

        assert_eq!(
            posture.validate_against_evidence_and_capability_matrix(
                &evidence_bundle,
                &capability_matrix,
            ),
            Err(PsionServedOutputClaimPostureError::MissingAttachedEvidence {
                claim_kind: String::from("executor_backed_result"),
            })
        );
    }

    #[test]
    fn verification_claims_are_rejected() {
        let capability_matrix = capability_matrix();
        let evidence_bundle: PsionServedEvidenceBundle = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_served_evidence_executor_backed_v1.json"
        ))
        .expect("evidence fixture should parse");
        let mut posture: PsionServedOutputClaimPosture = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_served_output_claim_executor_backed_v1.json"
        ))
        .expect("claim posture fixture should parse");
        posture.visible_claims.verification_visible = true;
        posture.posture_digest = stable_psion_served_output_claim_posture_digest(&posture);

        assert_eq!(
            posture.validate_against_evidence_and_capability_matrix(
                &evidence_bundle,
                &capability_matrix,
            ),
            Err(PsionServedOutputClaimPostureError::VerificationClaimsUnsupported)
        );
    }
}
