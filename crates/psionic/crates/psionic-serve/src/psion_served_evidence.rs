use std::collections::BTreeSet;

use psionic_train::{PsionRouteClass, PsionRouteKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionCapabilityMatrix, PsionCapabilityPosture, PsionCapabilityRefusalReason,
    PsionCapabilityRegionId,
};

/// Stable schema version for the first Psion served-evidence bundle.
pub const PSION_SERVED_EVIDENCE_SCHEMA_VERSION: &str = "psion.served_evidence.v1";

/// Shared evidence class published on one served Psion output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionServedEvidenceKind {
    /// The output is a bounded learned synthesis rather than a quoted source or executor result.
    LearnedJudgment,
    /// The output is explicitly grounded to cited admitted sources and anchors.
    SourceGroundedStatement,
    /// The output is backed by an explicit executor surface and artifact.
    ExecutorBackedResult,
    /// The output makes a capability claim backed by a concrete benchmark artifact and receipt.
    BenchmarkBackedCapabilityClaim,
}

impl PsionServedEvidenceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LearnedJudgment => "learned_judgment",
            Self::SourceGroundedStatement => "source_grounded_statement",
            Self::ExecutorBackedResult => "executor_backed_result",
            Self::BenchmarkBackedCapabilityClaim => "benchmark_backed_capability_claim",
        }
    }
}

/// Stable id/digest reference carried by served-evidence payloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedArtifactReference {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the referenced artifact.
    pub artifact_digest: String,
}

impl PsionServedArtifactReference {
    fn validate(&self, field: &str) -> Result<(), PsionServedEvidenceError> {
        ensure_nonempty(
            self.artifact_id.as_str(),
            format!("{field}.artifact_id").as_str(),
        )?;
        ensure_nonempty(
            self.artifact_digest.as_str(),
            format!("{field}.artifact_digest").as_str(),
        )?;
        Ok(())
    }
}

/// One cited source anchor attached to a source-grounded statement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedSourceReference {
    /// Stable reviewed-source identifier.
    pub source_id: String,
    /// Stable digest over the reviewed source payload.
    pub source_digest: String,
    /// Stable chapter, section, page, file, or record anchor.
    pub boundary_ref: String,
    /// Short source-specific note for the grounded statement.
    pub detail: String,
}

impl PsionServedSourceReference {
    fn validate(&self, field: &str) -> Result<(), PsionServedEvidenceError> {
        ensure_nonempty(self.source_id.as_str(), format!("{field}.source_id").as_str())?;
        ensure_nonempty(
            self.source_digest.as_str(),
            format!("{field}.source_digest").as_str(),
        )?;
        ensure_nonempty(
            self.boundary_ref.as_str(),
            format!("{field}.boundary_ref").as_str(),
        )?;
        ensure_nonempty(self.detail.as_str(), format!("{field}.detail").as_str())?;
        Ok(())
    }
}

/// Shared route evidence for one served Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedRouteReceipt {
    /// Coarse route selected for the output.
    pub route_kind: PsionRouteKind,
    /// Fine-grained route class evaluated under the canonical route package.
    pub route_class: PsionRouteClass,
    /// Capability-matrix region that justified the route.
    pub capability_region_id: PsionCapabilityRegionId,
    /// Stable boundary pointer for the route decision.
    pub route_boundary_ref: String,
    /// Promotion-bound route-calibration receipt referenced by the current capability matrix.
    pub route_calibration_receipt_id: String,
    /// Detailed route-class evaluation receipt that backs the class distinction.
    pub route_class_evaluation_receipt_id: String,
    /// Stable digest over the route-class evaluation receipt.
    pub route_class_evaluation_receipt_digest: String,
    /// Short explanation of the route evidence.
    pub detail: String,
}

impl PsionServedRouteReceipt {
    fn validate(&self) -> Result<(), PsionServedEvidenceError> {
        if self.route_kind == PsionRouteKind::Refusal {
            return Err(PsionServedEvidenceError::UnsupportedRouteKindForRouteReceipt {
                route_kind: route_kind_label(self.route_kind).to_string(),
            });
        }
        let valid_mapping = match (self.route_kind, self.route_class) {
            (
                PsionRouteKind::DirectModelAnswer,
                PsionRouteClass::AnswerInLanguage
                | PsionRouteClass::AnswerWithUncertainty
                | PsionRouteClass::RequestStructuredInputs,
            ) => true,
            (
                PsionRouteKind::ExactExecutorHandoff,
                PsionRouteClass::DelegateToExactExecutor,
            ) => true,
            _ => false,
        };
        if !valid_mapping {
            return Err(PsionServedEvidenceError::InvalidRouteClassMapping {
                route_kind: route_kind_label(self.route_kind).to_string(),
                route_class: route_class_label(self.route_class).to_string(),
            });
        }
        ensure_nonempty(
            self.route_boundary_ref.as_str(),
            "psion_served_evidence_bundle.route_receipt.route_boundary_ref",
        )?;
        ensure_nonempty(
            self.route_calibration_receipt_id.as_str(),
            "psion_served_evidence_bundle.route_receipt.route_calibration_receipt_id",
        )?;
        ensure_nonempty(
            self.route_class_evaluation_receipt_id.as_str(),
            "psion_served_evidence_bundle.route_receipt.route_class_evaluation_receipt_id",
        )?;
        ensure_nonempty(
            self.route_class_evaluation_receipt_digest.as_str(),
            "psion_served_evidence_bundle.route_receipt.route_class_evaluation_receipt_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_served_evidence_bundle.route_receipt.detail",
        )?;
        Ok(())
    }
}

/// Shared refusal evidence for one refused Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedRefusalReceipt {
    /// Capability-matrix region that required the refusal.
    pub capability_region_id: PsionCapabilityRegionId,
    /// Typed refusal reason surfaced to the caller.
    pub refusal_reason: PsionCapabilityRefusalReason,
    /// Stable boundary pointer for the refusal.
    pub refusal_boundary_ref: String,
    /// Promotion-bound refusal-calibration receipt referenced by the current capability matrix.
    pub refusal_calibration_receipt_id: String,
    /// Stable digest over the refusal-calibration receipt.
    pub refusal_calibration_receipt_digest: String,
    /// Short explanation of the refusal evidence.
    pub detail: String,
}

impl PsionServedRefusalReceipt {
    fn validate(&self) -> Result<(), PsionServedEvidenceError> {
        ensure_nonempty(
            self.refusal_boundary_ref.as_str(),
            "psion_served_evidence_bundle.refusal_receipt.refusal_boundary_ref",
        )?;
        ensure_nonempty(
            self.refusal_calibration_receipt_id.as_str(),
            "psion_served_evidence_bundle.refusal_receipt.refusal_calibration_receipt_id",
        )?;
        ensure_nonempty(
            self.refusal_calibration_receipt_digest.as_str(),
            "psion_served_evidence_bundle.refusal_receipt.refusal_calibration_receipt_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_served_evidence_bundle.refusal_receipt.detail",
        )?;
        Ok(())
    }
}

/// Explicit no-implicit-execution posture for one served Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionNoImplicitExecutionStatus {
    /// Execution may only be claimed via an explicit published executor surface.
    pub execution_only_via_explicit_surface: bool,
    /// Whether the output actually invoked an explicit executor surface.
    pub executor_surface_invoked: bool,
    /// Explicit executor artifact when an executor surface was invoked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit_executor_artifact: Option<PsionServedArtifactReference>,
    /// Short explanation of the no-implicit-execution posture.
    pub detail: String,
}

impl PsionNoImplicitExecutionStatus {
    fn validate(&self) -> Result<(), PsionServedEvidenceError> {
        if !self.execution_only_via_explicit_surface {
            return Err(PsionServedEvidenceError::NoImplicitExecutionMustRequireExplicitSurface);
        }
        ensure_nonempty(
            self.detail.as_str(),
            "psion_served_evidence_bundle.no_implicit_execution.detail",
        )?;
        if self.executor_surface_invoked {
            let artifact = self.explicit_executor_artifact.as_ref().ok_or(
                PsionServedEvidenceError::MissingExecutorArtifact,
            )?;
            artifact.validate("psion_served_evidence_bundle.no_implicit_execution.explicit_executor_artifact")?;
        } else if self.explicit_executor_artifact.is_some() {
            return Err(PsionServedEvidenceError::UnexpectedExecutorArtifact);
        }
        Ok(())
    }
}

/// One typed evidence label carried by the shared served-evidence bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PsionServedEvidenceLabel {
    /// A bounded learned answer that is not claiming source quotation or execution.
    LearnedJudgment {
        /// Whether the answer explicitly disclosed uncertainty.
        uncertainty_disclosed: bool,
        /// Short explanation of the learned judgment claim.
        detail: String,
    },
    /// A statement grounded to explicit reviewed sources and anchors.
    SourceGroundedStatement {
        /// Stable source refs supporting the statement.
        sources: Vec<PsionServedSourceReference>,
        /// Short explanation of the grounded statement.
        detail: String,
    },
    /// A result backed by an explicit executor surface and artifact.
    ExecutorBackedResult {
        /// Published executor surface that produced the result.
        executor_surface_product_id: String,
        /// Stable artifact produced or surfaced by that executor.
        executor_artifact: PsionServedArtifactReference,
        /// Short explanation of the executor-backed result.
        detail: String,
    },
    /// A bounded capability claim backed by one concrete benchmark receipt.
    BenchmarkBackedCapabilityClaim {
        /// Capability-matrix region the claim targets.
        capability_region_id: PsionCapabilityRegionId,
        /// Stable boundary pointer for the claim.
        claim_boundary_ref: String,
        /// Promotion decision that carried the cited receipt set.
        promotion_decision_id: String,
        /// Benchmark receipt directly backing the claim.
        benchmark_receipt_id: String,
        /// Stable benchmark artifact id/digest for the claim.
        benchmark_artifact: PsionServedArtifactReference,
        /// Short explanation of the benchmark-backed claim.
        detail: String,
    },
}

impl PsionServedEvidenceLabel {
    #[must_use]
    pub const fn kind(&self) -> PsionServedEvidenceKind {
        match self {
            Self::LearnedJudgment { .. } => PsionServedEvidenceKind::LearnedJudgment,
            Self::SourceGroundedStatement { .. } => {
                PsionServedEvidenceKind::SourceGroundedStatement
            }
            Self::ExecutorBackedResult { .. } => PsionServedEvidenceKind::ExecutorBackedResult,
            Self::BenchmarkBackedCapabilityClaim { .. } => {
                PsionServedEvidenceKind::BenchmarkBackedCapabilityClaim
            }
        }
    }

    fn validate(&self, index: usize) -> Result<(), PsionServedEvidenceError> {
        match self {
            Self::LearnedJudgment { detail, .. } => ensure_nonempty(
                detail.as_str(),
                format!("psion_served_evidence_bundle.evidence_labels[{index}].detail").as_str(),
            )?,
            Self::SourceGroundedStatement { sources, detail } => {
                ensure_nonempty(
                    detail.as_str(),
                    format!("psion_served_evidence_bundle.evidence_labels[{index}].detail")
                        .as_str(),
                )?;
                if sources.is_empty() {
                    return Err(PsionServedEvidenceError::MissingField {
                        field: format!(
                            "psion_served_evidence_bundle.evidence_labels[{index}].sources"
                        ),
                    });
                }
                for (source_index, source) in sources.iter().enumerate() {
                    source.validate(
                        format!(
                            "psion_served_evidence_bundle.evidence_labels[{index}].sources[{source_index}]"
                        )
                        .as_str(),
                    )?;
                }
            }
            Self::ExecutorBackedResult {
                executor_surface_product_id,
                executor_artifact,
                detail,
            } => {
                ensure_nonempty(
                    executor_surface_product_id.as_str(),
                    format!(
                        "psion_served_evidence_bundle.evidence_labels[{index}].executor_surface_product_id"
                    )
                    .as_str(),
                )?;
                executor_artifact.validate(
                    format!(
                        "psion_served_evidence_bundle.evidence_labels[{index}].executor_artifact"
                    )
                    .as_str(),
                )?;
                ensure_nonempty(
                    detail.as_str(),
                    format!("psion_served_evidence_bundle.evidence_labels[{index}].detail")
                        .as_str(),
                )?;
            }
            Self::BenchmarkBackedCapabilityClaim {
                claim_boundary_ref,
                promotion_decision_id,
                benchmark_receipt_id,
                benchmark_artifact,
                detail,
                ..
            } => {
                ensure_nonempty(
                    claim_boundary_ref.as_str(),
                    format!(
                        "psion_served_evidence_bundle.evidence_labels[{index}].claim_boundary_ref"
                    )
                    .as_str(),
                )?;
                ensure_nonempty(
                    promotion_decision_id.as_str(),
                    format!(
                        "psion_served_evidence_bundle.evidence_labels[{index}].promotion_decision_id"
                    )
                    .as_str(),
                )?;
                ensure_nonempty(
                    benchmark_receipt_id.as_str(),
                    format!(
                        "psion_served_evidence_bundle.evidence_labels[{index}].benchmark_receipt_id"
                    )
                    .as_str(),
                )?;
                benchmark_artifact.validate(
                    format!(
                        "psion_served_evidence_bundle.evidence_labels[{index}].benchmark_artifact"
                    )
                    .as_str(),
                )?;
                ensure_nonempty(
                    detail.as_str(),
                    format!("psion_served_evidence_bundle.evidence_labels[{index}].detail")
                        .as_str(),
                )?;
            }
        }
        Ok(())
    }
}

/// Shared served-evidence and provenance bundle for one Psion output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedEvidenceBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Capability-matrix identifier the bundle targets.
    pub capability_matrix_id: String,
    /// Capability-matrix version the bundle targets.
    pub capability_matrix_version: String,
    /// Shared route evidence when the output stayed in the served or routed lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_receipt: Option<PsionServedRouteReceipt>,
    /// Shared refusal evidence when the output refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_receipt: Option<PsionServedRefusalReceipt>,
    /// Explicit no-implicit-execution posture for the output.
    pub no_implicit_execution: PsionNoImplicitExecutionStatus,
    /// Typed evidence labels carried by the output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_labels: Vec<PsionServedEvidenceLabel>,
    /// Short summary of the served-evidence bundle.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionServedEvidenceBundle {
    /// Validates one bundle against the shared served-evidence schema.
    pub fn validate(&self) -> Result<(), PsionServedEvidenceError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_served_evidence_bundle.schema_version",
        )?;
        if self.schema_version != PSION_SERVED_EVIDENCE_SCHEMA_VERSION {
            return Err(PsionServedEvidenceError::SchemaVersionMismatch {
                expected: String::from(PSION_SERVED_EVIDENCE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.bundle_id.as_str(),
            "psion_served_evidence_bundle.bundle_id",
        )?;
        ensure_nonempty(
            self.capability_matrix_id.as_str(),
            "psion_served_evidence_bundle.capability_matrix_id",
        )?;
        ensure_nonempty(
            self.capability_matrix_version.as_str(),
            "psion_served_evidence_bundle.capability_matrix_version",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_served_evidence_bundle.summary",
        )?;
        if self.route_receipt.is_none() && self.refusal_receipt.is_none() {
            return Err(PsionServedEvidenceError::MissingRouteOrRefusal);
        }
        if self.route_receipt.is_some() && self.refusal_receipt.is_some() {
            return Err(PsionServedEvidenceError::RouteAndRefusalBothPresent);
        }
        if self.refusal_receipt.is_none() && self.evidence_labels.is_empty() {
            return Err(PsionServedEvidenceError::MissingEvidenceLabels);
        }
        self.no_implicit_execution.validate()?;
        if let Some(route_receipt) = self.route_receipt.as_ref() {
            route_receipt.validate()?;
        }
        if let Some(refusal_receipt) = self.refusal_receipt.as_ref() {
            refusal_receipt.validate()?;
        }

        let mut seen_kinds = BTreeSet::new();
        let mut executor_artifact = None;
        for (index, label) in self.evidence_labels.iter().enumerate() {
            label.validate(index)?;
            let kind = label.kind();
            if !seen_kinds.insert(kind) {
                return Err(PsionServedEvidenceError::DuplicateEvidenceKind {
                    kind: kind.as_str().to_string(),
                });
            }
            if let PsionServedEvidenceLabel::ExecutorBackedResult {
                executor_artifact: artifact,
                ..
            } = label
            {
                executor_artifact = Some(artifact);
            }
        }

        if self.refusal_receipt.is_some() && executor_artifact.is_some() {
            return Err(PsionServedEvidenceError::RefusalCannotCarryExecutorEvidence);
        }

        if let Some(executor_artifact) = executor_artifact {
            if self.route_receipt.as_ref().map(|value| value.route_kind)
                != Some(PsionRouteKind::ExactExecutorHandoff)
            {
                return Err(PsionServedEvidenceError::ExecutorEvidenceRequiresExactExecutorHandoff);
            }
            if !self.no_implicit_execution.executor_surface_invoked {
                return Err(PsionServedEvidenceError::MissingExecutorArtifact);
            }
            if self.no_implicit_execution.explicit_executor_artifact.as_ref()
                != Some(executor_artifact)
            {
                return Err(PsionServedEvidenceError::ExecutorArtifactMismatch);
            }
        } else {
            if self.route_receipt.as_ref().map(|value| value.route_kind)
                == Some(PsionRouteKind::ExactExecutorHandoff)
            {
                return Err(PsionServedEvidenceError::ExactExecutorHandoffRequiresExecutorEvidence);
            }
            if self.no_implicit_execution.executor_surface_invoked {
                return Err(PsionServedEvidenceError::MissingExecutorArtifact);
            }
            if self.no_implicit_execution.explicit_executor_artifact.is_some() {
                return Err(PsionServedEvidenceError::UnexpectedExecutorArtifact);
            }
        }

        let expected_digest = stable_psion_served_evidence_bundle_digest(self);
        if self.bundle_digest != expected_digest {
            return Err(PsionServedEvidenceError::DigestMismatch {
                expected: expected_digest,
                actual: self.bundle_digest.clone(),
            });
        }

        Ok(())
    }

    /// Validates one bundle against the current shared schema and a published capability matrix.
    pub fn validate_against_capability_matrix(
        &self,
        capability_matrix: &PsionCapabilityMatrix,
    ) -> Result<(), PsionServedEvidenceError> {
        self.validate()?;
        check_string_match(
            self.capability_matrix_id.as_str(),
            capability_matrix.matrix_id.as_str(),
            "psion_served_evidence_bundle.capability_matrix_id",
        )?;
        check_string_match(
            self.capability_matrix_version.as_str(),
            capability_matrix.matrix_version.as_str(),
            "psion_served_evidence_bundle.capability_matrix_version",
        )?;

        if let Some(route_receipt) = self.route_receipt.as_ref() {
            check_string_match(
                route_receipt.route_calibration_receipt_id.as_str(),
                capability_matrix
                    .acceptance_basis
                    .route_calibration_receipt_ref
                    .as_str(),
                "psion_served_evidence_bundle.route_receipt.route_calibration_receipt_id",
            )?;
            let region = capability_region(capability_matrix, route_receipt.capability_region_id)?;
            let expected_posture = match route_receipt.route_kind {
                PsionRouteKind::DirectModelAnswer => PsionCapabilityPosture::Supported,
                PsionRouteKind::ExactExecutorHandoff => PsionCapabilityPosture::RouteRequired,
                PsionRouteKind::Refusal => unreachable!("route receipts reject refusal route kind"),
            };
            if region.posture != expected_posture {
                return Err(PsionServedEvidenceError::CapabilityPostureMismatch {
                    region_id: route_receipt.capability_region_id.as_str().to_string(),
                    expected: capability_posture_label(expected_posture).to_string(),
                    actual: capability_posture_label(region.posture).to_string(),
                });
            }
        }

        if let Some(refusal_receipt) = self.refusal_receipt.as_ref() {
            check_string_match(
                refusal_receipt.refusal_calibration_receipt_id.as_str(),
                capability_matrix
                    .acceptance_basis
                    .refusal_calibration_receipt_ref
                    .as_str(),
                "psion_served_evidence_bundle.refusal_receipt.refusal_calibration_receipt_id",
            )?;
            let region = capability_region(capability_matrix, refusal_receipt.capability_region_id)?;
            if !matches!(
                region.posture,
                PsionCapabilityPosture::RefusalRequired | PsionCapabilityPosture::Unsupported
            ) {
                return Err(PsionServedEvidenceError::CapabilityPostureMismatch {
                    region_id: refusal_receipt.capability_region_id.as_str().to_string(),
                    expected: String::from("refusal_required_or_unsupported"),
                    actual: capability_posture_label(region.posture).to_string(),
                });
            }
            if !region.refusal_reasons.contains(&refusal_receipt.refusal_reason) {
                return Err(PsionServedEvidenceError::RefusalReasonNotPublished {
                    region_id: refusal_receipt.capability_region_id.as_str().to_string(),
                    reason: refusal_reason_label(refusal_receipt.refusal_reason).to_string(),
                });
            }
        }

        for label in &self.evidence_labels {
            if let PsionServedEvidenceLabel::BenchmarkBackedCapabilityClaim {
                capability_region_id,
                ..
            } = label
            {
                capability_region(capability_matrix, *capability_region_id)?;
            }
        }

        Ok(())
    }
}

/// Records one served-evidence bundle with canonical schema version and digest.
pub fn record_psion_served_evidence_bundle(
    bundle_id: impl Into<String>,
    capability_matrix_id: impl Into<String>,
    capability_matrix_version: impl Into<String>,
    route_receipt: Option<PsionServedRouteReceipt>,
    refusal_receipt: Option<PsionServedRefusalReceipt>,
    no_implicit_execution: PsionNoImplicitExecutionStatus,
    mut evidence_labels: Vec<PsionServedEvidenceLabel>,
    summary: impl Into<String>,
) -> Result<PsionServedEvidenceBundle, PsionServedEvidenceError> {
    evidence_labels.sort_by_key(|label| label.kind());
    let mut bundle = PsionServedEvidenceBundle {
        schema_version: String::from(PSION_SERVED_EVIDENCE_SCHEMA_VERSION),
        bundle_id: bundle_id.into(),
        capability_matrix_id: capability_matrix_id.into(),
        capability_matrix_version: capability_matrix_version.into(),
        route_receipt,
        refusal_receipt,
        no_implicit_execution,
        evidence_labels,
        summary: summary.into(),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_psion_served_evidence_bundle_digest(&bundle);
    bundle.validate()?;
    Ok(bundle)
}

/// Validation errors for Psion served-evidence bundles.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PsionServedEvidenceError {
    #[error("Psion served-evidence bundle is missing `{field}`")]
    MissingField { field: String },
    #[error(
        "Psion served-evidence bundle schema version mismatch: expected `{expected}`, got `{actual}`"
    )]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("Psion served-evidence bundle must carry either a route receipt or a refusal receipt")]
    MissingRouteOrRefusal,
    #[error("Psion served-evidence bundle may not carry both a route receipt and a refusal receipt")]
    RouteAndRefusalBothPresent,
    #[error("Psion served-evidence bundle requires at least one evidence label when no refusal receipt is present")]
    MissingEvidenceLabels,
    #[error("Psion served-evidence bundle repeats evidence kind `{kind}`")]
    DuplicateEvidenceKind { kind: String },
    #[error("Psion served-evidence bundle route receipt may not use route kind `{route_kind}`")]
    UnsupportedRouteKindForRouteReceipt { route_kind: String },
    #[error("Psion served-evidence bundle route kind `{route_kind}` does not match route class `{route_class}`")]
    InvalidRouteClassMapping {
        route_kind: String,
        route_class: String,
    },
    #[error("Psion served-evidence bundle executor-backed results require exact-executor handoff")]
    ExecutorEvidenceRequiresExactExecutorHandoff,
    #[error("Psion served-evidence bundle exact-executor handoff requires executor-backed evidence")]
    ExactExecutorHandoffRequiresExecutorEvidence,
    #[error("Psion served-evidence bundle refusal receipts may not also imply executor-backed results")]
    RefusalCannotCarryExecutorEvidence,
    #[error("Psion served-evidence bundle no-implicit-execution posture must require an explicit surface")]
    NoImplicitExecutionMustRequireExplicitSurface,
    #[error("Psion served-evidence bundle executor surface invocation is missing its artifact reference")]
    MissingExecutorArtifact,
    #[error("Psion served-evidence bundle unexpectedly carries executor artifact reference without executor-backed evidence")]
    UnexpectedExecutorArtifact,
    #[error("Psion served-evidence bundle executor artifact reference does not match executor-backed evidence")]
    ExecutorArtifactMismatch,
    #[error("Psion served-evidence bundle capability matrix mismatch for `{field}`: expected `{expected}`, got `{actual}`")]
    CapabilityMatrixMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("Psion served-evidence bundle references unknown capability region `{region_id}`")]
    UnknownCapabilityRegion { region_id: String },
    #[error("Psion served-evidence bundle expected capability posture `{expected}` for region `{region_id}` but found `{actual}`")]
    CapabilityPostureMismatch {
        region_id: String,
        expected: String,
        actual: String,
    },
    #[error("Psion served-evidence bundle refusal reason `{reason}` is not published for region `{region_id}`")]
    RefusalReasonNotPublished {
        region_id: String,
        reason: String,
    },
    #[error("Psion served-evidence bundle digest mismatch: expected `{expected}`, got `{actual}`")]
    DigestMismatch { expected: String, actual: String },
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionServedEvidenceError> {
    if value.trim().is_empty() {
        return Err(PsionServedEvidenceError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionServedEvidenceError> {
    ensure_nonempty(actual, field)?;
    ensure_nonempty(expected, field)?;
    if actual != expected {
        return Err(PsionServedEvidenceError::CapabilityMatrixMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn capability_region(
    capability_matrix: &PsionCapabilityMatrix,
    region_id: PsionCapabilityRegionId,
) -> Result<&crate::PsionCapabilityRegion, PsionServedEvidenceError> {
    capability_matrix
        .regions
        .iter()
        .find(|region| region.region_id == region_id)
        .ok_or_else(|| PsionServedEvidenceError::UnknownCapabilityRegion {
            region_id: region_id.as_str().to_string(),
        })
}

fn stable_psion_served_evidence_bundle_digest(bundle: &PsionServedEvidenceBundle) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_served_evidence_bundle|");
    hasher.update(bundle.schema_version.as_bytes());
    hasher.update(bundle.bundle_id.as_bytes());
    hasher.update(bundle.capability_matrix_id.as_bytes());
    hasher.update(bundle.capability_matrix_version.as_bytes());

    if let Some(route_receipt) = bundle.route_receipt.as_ref() {
        hasher.update(b"route|");
        hasher.update(route_kind_label(route_receipt.route_kind).as_bytes());
        hasher.update(route_class_label(route_receipt.route_class).as_bytes());
        hasher.update(route_receipt.capability_region_id.as_str().as_bytes());
        hasher.update(route_receipt.route_boundary_ref.as_bytes());
        hasher.update(route_receipt.route_calibration_receipt_id.as_bytes());
        hasher.update(route_receipt.route_class_evaluation_receipt_id.as_bytes());
        hasher.update(route_receipt.route_class_evaluation_receipt_digest.as_bytes());
        hasher.update(route_receipt.detail.as_bytes());
    } else {
        hasher.update(b"no_route|");
    }

    if let Some(refusal_receipt) = bundle.refusal_receipt.as_ref() {
        hasher.update(b"refusal|");
        hasher.update(refusal_receipt.capability_region_id.as_str().as_bytes());
        hasher.update(refusal_reason_label(refusal_receipt.refusal_reason).as_bytes());
        hasher.update(refusal_receipt.refusal_boundary_ref.as_bytes());
        hasher.update(refusal_receipt.refusal_calibration_receipt_id.as_bytes());
        hasher.update(refusal_receipt.refusal_calibration_receipt_digest.as_bytes());
        hasher.update(refusal_receipt.detail.as_bytes());
    } else {
        hasher.update(b"no_refusal|");
    }

    hasher.update(
        if bundle
            .no_implicit_execution
            .execution_only_via_explicit_surface
        {
            "explicit_surface_only|"
        } else {
            "implicit_execution_allowed|"
        }
        .as_bytes(),
    );
    hasher.update(
        if bundle.no_implicit_execution.executor_surface_invoked {
            "executor_surface_invoked|"
        } else {
            "executor_surface_not_invoked|"
        }
        .as_bytes(),
    );
    if let Some(artifact) = bundle.no_implicit_execution.explicit_executor_artifact.as_ref() {
        hasher.update(artifact.artifact_id.as_bytes());
        hasher.update(artifact.artifact_digest.as_bytes());
    } else {
        hasher.update(b"no_executor_artifact|");
    }
    hasher.update(bundle.no_implicit_execution.detail.as_bytes());

    for label in &bundle.evidence_labels {
        hasher.update(label.kind().as_str().as_bytes());
        match label {
            PsionServedEvidenceLabel::LearnedJudgment {
                uncertainty_disclosed,
                detail,
            } => {
                hasher.update(
                    if *uncertainty_disclosed {
                        "uncertainty_disclosed|"
                    } else {
                        "uncertainty_not_disclosed|"
                    }
                    .as_bytes(),
                );
                hasher.update(detail.as_bytes());
            }
            PsionServedEvidenceLabel::SourceGroundedStatement { sources, detail } => {
                for source in sources {
                    hasher.update(source.source_id.as_bytes());
                    hasher.update(source.source_digest.as_bytes());
                    hasher.update(source.boundary_ref.as_bytes());
                    hasher.update(source.detail.as_bytes());
                }
                hasher.update(detail.as_bytes());
            }
            PsionServedEvidenceLabel::ExecutorBackedResult {
                executor_surface_product_id,
                executor_artifact,
                detail,
            } => {
                hasher.update(executor_surface_product_id.as_bytes());
                hasher.update(executor_artifact.artifact_id.as_bytes());
                hasher.update(executor_artifact.artifact_digest.as_bytes());
                hasher.update(detail.as_bytes());
            }
            PsionServedEvidenceLabel::BenchmarkBackedCapabilityClaim {
                capability_region_id,
                claim_boundary_ref,
                promotion_decision_id,
                benchmark_receipt_id,
                benchmark_artifact,
                detail,
            } => {
                hasher.update(capability_region_id.as_str().as_bytes());
                hasher.update(claim_boundary_ref.as_bytes());
                hasher.update(promotion_decision_id.as_bytes());
                hasher.update(benchmark_receipt_id.as_bytes());
                hasher.update(benchmark_artifact.artifact_id.as_bytes());
                hasher.update(benchmark_artifact.artifact_digest.as_bytes());
                hasher.update(detail.as_bytes());
            }
        }
    }

    hasher.update(bundle.summary.as_bytes());
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

fn capability_posture_label(posture: PsionCapabilityPosture) -> &'static str {
    match posture {
        PsionCapabilityPosture::Supported => "supported",
        PsionCapabilityPosture::RouteRequired => "route_required",
        PsionCapabilityPosture::RefusalRequired => "refusal_required",
        PsionCapabilityPosture::Unsupported => "unsupported",
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
    fn served_evidence_examples_validate_against_capability_matrix() {
        let capability_matrix = capability_matrix();
        for fixture in [
            include_str!(
                "../../../fixtures/psion/serve/psion_served_evidence_direct_grounded_v1.json"
            ),
            include_str!(
                "../../../fixtures/psion/serve/psion_served_evidence_executor_backed_v1.json"
            ),
            include_str!("../../../fixtures/psion/serve/psion_served_evidence_refusal_v1.json"),
        ] {
            let bundle: PsionServedEvidenceBundle =
                serde_json::from_str(fixture).expect("bundle fixture should parse");
            bundle
                .validate_against_capability_matrix(&capability_matrix)
                .expect("bundle fixture should validate");
        }
    }

    #[test]
    fn exact_executor_handoff_requires_executor_backed_evidence() {
        let mut bundle = serde_json::from_str::<PsionServedEvidenceBundle>(include_str!(
            "../../../fixtures/psion/serve/psion_served_evidence_executor_backed_v1.json"
        ))
        .expect("executor-backed bundle should parse");
        bundle.evidence_labels = vec![PsionServedEvidenceLabel::LearnedJudgment {
            uncertainty_disclosed: false,
            detail: String::from("This stayed in language without executor proof."),
        }];
        bundle.bundle_digest = stable_psion_served_evidence_bundle_digest(&bundle);

        assert_eq!(
            bundle.validate(),
            Err(PsionServedEvidenceError::ExactExecutorHandoffRequiresExecutorEvidence)
        );
    }

    #[test]
    fn refusal_bundle_rejects_executor_surface_artifacts() {
        let mut bundle = serde_json::from_str::<PsionServedEvidenceBundle>(include_str!(
            "../../../fixtures/psion/serve/psion_served_evidence_refusal_v1.json"
        ))
        .expect("refusal bundle should parse");
        bundle.no_implicit_execution.explicit_executor_artifact =
            Some(PsionServedArtifactReference {
                artifact_id: String::from("tassadar://trace/example"),
                artifact_digest: String::from("sha256:trace-example"),
            });
        bundle.bundle_digest = stable_psion_served_evidence_bundle_digest(&bundle);

        assert_eq!(
            bundle.validate(),
            Err(PsionServedEvidenceError::UnexpectedExecutorArtifact)
        );
    }
}
