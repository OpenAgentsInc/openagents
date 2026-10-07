use std::collections::BTreeSet;

use psionic_data::{
    PsionLifecycleChangeTrigger, PsionLifecycleImpactAction, PsionSourceImpactAnalysisReceipt,
};
use psionic_train::{
    PsionAcceptanceMatrix, PsionAcceptanceMatrixError, PsionCheckpointRecoveryReceipt,
    PsionPhaseGate, PsionPromotionDecisionReceipt, PsionRefusalCalibrationReceipt,
    PsionRouteCalibrationReceipt, ReplayVerificationDisposition, TrainingReplayVerificationReceipt,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionCapabilityMatrix, PsionCapabilityMatrixError, PsionCapabilityPosture,
    PsionCapabilityRefusalReason, PsionCapabilityRegionId, PsionServedBehaviorVisibility,
    PsionServedOutputClaimPosture, PsionServedVisibleClaims,
};

/// Stable schema version for the first Psion capability-withdrawal contract.
pub const PSION_CAPABILITY_WITHDRAWAL_RECEIPT_SCHEMA_VERSION: &str =
    "psion.capability_withdrawal_receipt.v1";

/// Stable trigger kind for one capability-withdrawal receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCapabilityWithdrawalTriggerKind {
    /// A source rights posture narrowed or was revoked after publication.
    RightsChange,
    /// A contamination finding invalidated published claims.
    ContaminationDiscovered,
    /// Deterministic replay drifted outside the accepted band.
    ReplayFailure,
    /// Route behavior regressed outside the accepted band.
    RouteRegression,
    /// Refusal behavior regressed outside the accepted band.
    RefusalRegression,
}

/// One generic artifact reference carried by the rollback contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityArtifactReference {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the artifact.
    pub artifact_digest: String,
}

impl PsionCapabilityArtifactReference {
    fn validate(&self, field: &str) -> Result<(), PsionCapabilityWithdrawalError> {
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

/// Published matrix identity under withdrawal review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPublishedCapabilityMatrixReference {
    /// Stable matrix identifier.
    pub matrix_id: String,
    /// Stable matrix version.
    pub matrix_version: String,
    /// Stable matrix digest computed by this contract.
    pub matrix_digest: String,
    /// Promoted phase that originally justified publication.
    pub acceptance_phase: PsionPhaseGate,
    /// Promotion decision receipt id carried by the matrix.
    pub promotion_decision_ref: String,
}

impl PsionPublishedCapabilityMatrixReference {
    fn from_matrix(matrix: &PsionCapabilityMatrix) -> Self {
        Self {
            matrix_id: matrix.matrix_id.clone(),
            matrix_version: matrix.matrix_version.clone(),
            matrix_digest: stable_capability_matrix_digest(matrix),
            acceptance_phase: matrix.acceptance_basis.acceptance_phase,
            promotion_decision_ref: matrix.acceptance_basis.promotion_decision_ref.clone(),
        }
    }

    fn validate_against(
        &self,
        matrix: &PsionCapabilityMatrix,
    ) -> Result<(), PsionCapabilityWithdrawalError> {
        ensure_nonempty(
            self.matrix_id.as_str(),
            "capability_withdrawal.published_matrix.matrix_id",
        )?;
        ensure_nonempty(
            self.matrix_version.as_str(),
            "capability_withdrawal.published_matrix.matrix_version",
        )?;
        ensure_nonempty(
            self.matrix_digest.as_str(),
            "capability_withdrawal.published_matrix.matrix_digest",
        )?;
        ensure_nonempty(
            self.promotion_decision_ref.as_str(),
            "capability_withdrawal.published_matrix.promotion_decision_ref",
        )?;
        if self.matrix_id != matrix.matrix_id || self.matrix_version != matrix.matrix_version {
            return Err(PsionCapabilityWithdrawalError::PublishedMatrixMismatch {
                detail: String::from(
                    "published matrix id or version drifted from the target matrix",
                ),
            });
        }
        let expected_digest = stable_capability_matrix_digest(matrix);
        if self.matrix_digest != expected_digest {
            return Err(PsionCapabilityWithdrawalError::PublishedMatrixMismatch {
                detail: String::from("published matrix digest drifted from the target matrix"),
            });
        }
        if self.acceptance_phase != matrix.acceptance_basis.acceptance_phase {
            return Err(PsionCapabilityWithdrawalError::PublishedMatrixMismatch {
                detail: String::from("published acceptance phase drifted from the target matrix"),
            });
        }
        if self.promotion_decision_ref != matrix.acceptance_basis.promotion_decision_ref {
            return Err(PsionCapabilityWithdrawalError::PublishedMatrixMismatch {
                detail: String::from(
                    "published promotion-decision ref drifted from the target matrix",
                ),
            });
        }
        Ok(())
    }
}

/// Checkpoint action taken against one promoted served checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCheckpointRollbackDisposition {
    /// The published served checkpoint is withdrawn until a corrected publication exists.
    WithdrawServedCheckpoint,
    /// Serving rolls back to one prior checkpoint target.
    RollbackToPriorCheckpoint,
}

/// Durable checkpoint downgrade plan tied to the withdrawal receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCheckpointRollbackPlan {
    /// Currently published checkpoint identifier.
    pub published_checkpoint_id: String,
    /// Withdrawal or rollback disposition.
    pub disposition: PsionCheckpointRollbackDisposition,
    /// Explicit rollback target when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_target_checkpoint_id: Option<String>,
    /// Plain-language checkpoint note.
    pub detail: String,
}

impl PsionCheckpointRollbackPlan {
    fn validate_against_checkpoint(
        &self,
        checkpoint: &PsionCheckpointRecoveryReceipt,
    ) -> Result<(), PsionCapabilityWithdrawalError> {
        ensure_nonempty(
            self.published_checkpoint_id.as_str(),
            "capability_withdrawal.checkpoint_plan.published_checkpoint_id",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "capability_withdrawal.checkpoint_plan.detail",
        )?;
        if self.published_checkpoint_id != checkpoint.checkpoint_id {
            return Err(PsionCapabilityWithdrawalError::CheckpointPlanMismatch {
                detail: String::from(
                    "checkpoint plan does not target the promoted checkpoint from the decision receipt",
                ),
            });
        }
        match self.disposition {
            PsionCheckpointRollbackDisposition::WithdrawServedCheckpoint => {
                if self.rollback_target_checkpoint_id.is_some() {
                    return Err(PsionCapabilityWithdrawalError::CheckpointPlanMismatch {
                        detail: String::from(
                            "withdraw-served-checkpoint may not declare a rollback target",
                        ),
                    });
                }
            }
            PsionCheckpointRollbackDisposition::RollbackToPriorCheckpoint => {
                let target = self.rollback_target_checkpoint_id.as_ref().ok_or(
                    PsionCapabilityWithdrawalError::CheckpointPlanMismatch {
                        detail: String::from(
                            "rollback-to-prior-checkpoint requires one explicit rollback target",
                        ),
                    },
                )?;
                ensure_nonempty(
                    target.as_str(),
                    "capability_withdrawal.checkpoint_plan.rollback_target_checkpoint_id",
                )?;
                if target == &self.published_checkpoint_id {
                    return Err(PsionCapabilityWithdrawalError::CheckpointPlanMismatch {
                        detail: String::from(
                            "rollback target must differ from the published checkpoint",
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Capability-matrix change kind preserved in rollback history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCapabilityMatrixChangeKind {
    /// The published matrix is withdrawn instead of silently replaced.
    WithdrawPublication,
    /// One or more regions are downgraded to a stricter posture.
    DowngradeRegionPosture,
}

/// One region downgrade preserved in capability-matrix history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityRegionDowngrade {
    /// Stable region identifier.
    pub region_id: PsionCapabilityRegionId,
    /// Previously published posture.
    pub previous_posture: PsionCapabilityPosture,
    /// Replacement stricter posture.
    pub next_posture: PsionCapabilityPosture,
    /// Replacement refusal reasons when the region now refuses or is unsupported.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next_refusal_reasons: Vec<PsionCapabilityRefusalReason>,
    /// Plain-language downgrade note.
    pub detail: String,
}

impl PsionCapabilityRegionDowngrade {
    fn validate_against_matrix(
        &self,
        matrix: &PsionCapabilityMatrix,
    ) -> Result<(), PsionCapabilityWithdrawalError> {
        ensure_nonempty(
            self.detail.as_str(),
            "capability_withdrawal.matrix_change.downgraded_regions[].detail",
        )?;
        let published_region = matrix
            .regions
            .iter()
            .find(|region| region.region_id == self.region_id)
            .ok_or(PsionCapabilityWithdrawalError::UnknownCapabilityRegion {
                region_id: self.region_id,
            })?;
        if self.previous_posture != published_region.posture {
            return Err(PsionCapabilityWithdrawalError::InvalidMatrixChange {
                detail: format!(
                    "region `{}` previous posture drifted from the published matrix",
                    self.region_id.as_str()
                ),
            });
        }
        if posture_rank(self.next_posture) <= posture_rank(self.previous_posture) {
            return Err(PsionCapabilityWithdrawalError::InvalidMatrixChange {
                detail: format!(
                    "region `{}` must downgrade to a stricter posture",
                    self.region_id.as_str()
                ),
            });
        }
        match self.next_posture {
            PsionCapabilityPosture::Supported | PsionCapabilityPosture::RouteRequired => {
                if !self.next_refusal_reasons.is_empty() {
                    return Err(PsionCapabilityWithdrawalError::InvalidMatrixChange {
                        detail: format!(
                            "region `{}` may not attach refusal reasons to non-refusal postures",
                            self.region_id.as_str()
                        ),
                    });
                }
            }
            PsionCapabilityPosture::RefusalRequired | PsionCapabilityPosture::Unsupported => {
                if self.next_refusal_reasons.is_empty() {
                    return Err(PsionCapabilityWithdrawalError::InvalidMatrixChange {
                        detail: format!(
                            "region `{}` must declare refusal reasons after downgrading to refusal or unsupported",
                            self.region_id.as_str()
                        ),
                    });
                }
                reject_duplicate_enum_entries(self.next_refusal_reasons.as_slice(), |reason| {
                    PsionCapabilityWithdrawalError::InvalidMatrixChange {
                        detail: format!(
                            "region `{}` repeated refusal reason `{reason:?}`",
                            self.region_id.as_str()
                        ),
                    }
                })?;
            }
        }
        Ok(())
    }
}

/// One capability-matrix history entry carried by rollback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityMatrixChange {
    /// Stable change identifier.
    pub change_id: String,
    /// Withdrawal or downgrade kind.
    pub kind: PsionCapabilityMatrixChangeKind,
    /// Matrix identity under change.
    pub matrix_id: String,
    /// Matrix version under change.
    pub matrix_version: String,
    /// Matrix digest under change.
    pub matrix_digest: String,
    /// Region downgrades when the change narrows the matrix instead of withdrawing it fully.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub downgraded_regions: Vec<PsionCapabilityRegionDowngrade>,
    /// Plain-language history note.
    pub detail: String,
}

impl PsionCapabilityMatrixChange {
    fn validate_against_matrix(
        &self,
        matrix: &PsionCapabilityMatrix,
    ) -> Result<(), PsionCapabilityWithdrawalError> {
        ensure_nonempty(
            self.change_id.as_str(),
            "capability_withdrawal.matrix_history[].change_id",
        )?;
        ensure_nonempty(
            self.matrix_id.as_str(),
            "capability_withdrawal.matrix_history[].matrix_id",
        )?;
        ensure_nonempty(
            self.matrix_version.as_str(),
            "capability_withdrawal.matrix_history[].matrix_version",
        )?;
        ensure_nonempty(
            self.matrix_digest.as_str(),
            "capability_withdrawal.matrix_history[].matrix_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "capability_withdrawal.matrix_history[].detail",
        )?;
        if self.matrix_id != matrix.matrix_id || self.matrix_version != matrix.matrix_version {
            return Err(PsionCapabilityWithdrawalError::InvalidMatrixChange {
                detail: String::from(
                    "matrix history entry did not target the published capability matrix",
                ),
            });
        }
        if self.matrix_digest != stable_capability_matrix_digest(matrix) {
            return Err(PsionCapabilityWithdrawalError::InvalidMatrixChange {
                detail: String::from("matrix history entry used the wrong published matrix digest"),
            });
        }
        match self.kind {
            PsionCapabilityMatrixChangeKind::WithdrawPublication => {
                if !self.downgraded_regions.is_empty() {
                    return Err(PsionCapabilityWithdrawalError::InvalidMatrixChange {
                        detail: String::from(
                            "matrix withdrawal entries may not also list downgraded regions",
                        ),
                    });
                }
            }
            PsionCapabilityMatrixChangeKind::DowngradeRegionPosture => {
                if self.downgraded_regions.is_empty() {
                    return Err(PsionCapabilityWithdrawalError::InvalidMatrixChange {
                        detail: String::from(
                            "region-downgrade entries must list at least one downgraded region",
                        ),
                    });
                }
                let mut regions = BTreeSet::new();
                for region in &self.downgraded_regions {
                    region.validate_against_matrix(matrix)?;
                    if !regions.insert(region.region_id) {
                        return Err(PsionCapabilityWithdrawalError::InvalidMatrixChange {
                            detail: format!(
                                "matrix history entry repeated region `{}`",
                                region.region_id.as_str()
                            ),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// Visible claim flag that can be narrowed or removed by rollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionServedClaimFlag {
    LearnedJudgment,
    SourceGrounding,
    ExecutorBacking,
    BenchmarkBacking,
    Verification,
}

/// Minimal served-claim posture view kept in rollback history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedClaimPostureSummary {
    /// Stable posture identifier.
    pub posture_id: String,
    /// Stable posture digest.
    pub posture_digest: String,
    /// Visible claim flags published on the surface.
    pub visible_claims: PsionServedVisibleClaims,
    /// Visible route or refusal behavior.
    pub behavior_visibility: PsionServedBehaviorVisibility,
}

impl PsionServedClaimPostureSummary {
    #[must_use]
    pub fn from_posture(posture: &PsionServedOutputClaimPosture) -> Self {
        Self {
            posture_id: posture.posture_id.clone(),
            posture_digest: posture.posture_digest.clone(),
            visible_claims: posture.visible_claims.clone(),
            behavior_visibility: posture.behavior_visibility.clone(),
        }
    }

    fn validate(&self, field: &str) -> Result<(), PsionCapabilityWithdrawalError> {
        ensure_nonempty(
            self.posture_id.as_str(),
            format!("{field}.posture_id").as_str(),
        )?;
        ensure_nonempty(
            self.posture_digest.as_str(),
            format!("{field}.posture_digest").as_str(),
        )?;
        ensure_behavior_detail(
            &self.behavior_visibility,
            format!("{field}.behavior_visibility").as_str(),
        )?;
        Ok(())
    }
}

/// Served-claim history change kind preserved by rollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionServedClaimChangeKind {
    /// The served posture is removed from publication.
    Depublish,
    /// The served posture survives, but visible claims are narrowed.
    NarrowVisibleClaims,
    /// The served posture changes route or refusal behavior.
    ChangeBehaviorVisibility,
}

/// One serving-surface claim-history update recorded by rollback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionServedClaimChange {
    /// Stable served surface identifier.
    pub surface_id: String,
    /// Depublish, narrow, or behavior-change kind.
    pub kind: PsionServedClaimChangeKind,
    /// Previous served posture summary.
    pub previous_posture: PsionServedClaimPostureSummary,
    /// Replacement served posture summary when the surface remains published.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_posture: Option<PsionServedClaimPostureSummary>,
    /// Visible claim flags removed by the rollback.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_visible_claims: Vec<PsionServedClaimFlag>,
    /// Plain-language history note.
    pub detail: String,
}

impl PsionServedClaimChange {
    fn validate(&self) -> Result<(), PsionCapabilityWithdrawalError> {
        ensure_nonempty(
            self.surface_id.as_str(),
            "capability_withdrawal.served_claim_history[].surface_id",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "capability_withdrawal.served_claim_history[].detail",
        )?;
        self.previous_posture
            .validate("capability_withdrawal.served_claim_history[].previous_posture")?;
        reject_duplicate_enum_entries(self.removed_visible_claims.as_slice(), |flag| {
            PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                detail: format!("served-claim change repeated removed flag `{flag:?}`"),
            }
        })?;

        match self.kind {
            PsionServedClaimChangeKind::Depublish => {
                if self.replacement_posture.is_some() {
                    return Err(PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                        detail: String::from(
                            "depublish claim changes may not carry a replacement posture",
                        ),
                    });
                }
            }
            PsionServedClaimChangeKind::NarrowVisibleClaims => {
                let replacement = self.replacement_posture.as_ref().ok_or(
                    PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                        detail: String::from(
                            "narrow-visible-claims changes require one replacement posture",
                        ),
                    },
                )?;
                replacement
                    .validate("capability_withdrawal.served_claim_history[].replacement_posture")?;
                if self.removed_visible_claims.is_empty() {
                    return Err(PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                        detail: String::from(
                            "narrow-visible-claims changes must name at least one removed visible claim",
                        ),
                    });
                }
                for flag in &self.removed_visible_claims {
                    if !claim_flag_visible(&self.previous_posture.visible_claims, *flag) {
                        return Err(PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                            detail: format!(
                                "removed claim flag `{flag:?}` was not visible on the previous posture",
                            ),
                        });
                    }
                    if claim_flag_visible(&replacement.visible_claims, *flag) {
                        return Err(PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                            detail: format!(
                                "removed claim flag `{flag:?}` remained visible on the replacement posture",
                            ),
                        });
                    }
                }
                for flag in all_claim_flags() {
                    if !claim_flag_visible(&self.previous_posture.visible_claims, flag)
                        && claim_flag_visible(&replacement.visible_claims, flag)
                    {
                        return Err(PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                            detail: format!(
                                "replacement posture introduced new visible claim `{flag:?}` during rollback",
                            ),
                        });
                    }
                }
                if behavior_rank(&replacement.behavior_visibility)
                    < behavior_rank(&self.previous_posture.behavior_visibility)
                {
                    return Err(PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                        detail: String::from(
                            "replacement posture may not become less conservative during rollback",
                        ),
                    });
                }
            }
            PsionServedClaimChangeKind::ChangeBehaviorVisibility => {
                let replacement = self.replacement_posture.as_ref().ok_or(
                    PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                        detail: String::from(
                            "behavior-visibility changes require one replacement posture",
                        ),
                    },
                )?;
                replacement
                    .validate("capability_withdrawal.served_claim_history[].replacement_posture")?;
                if replacement.behavior_visibility == self.previous_posture.behavior_visibility {
                    return Err(PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                        detail: String::from(
                            "behavior-visibility change requires different previous and replacement behavior",
                        ),
                    });
                }
                if behavior_rank(&replacement.behavior_visibility)
                    < behavior_rank(&self.previous_posture.behavior_visibility)
                {
                    return Err(PsionCapabilityWithdrawalError::InvalidServedClaimChange {
                        detail: String::from(
                            "behavior rollback may not become less conservative than the previous posture",
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Follow-on analysis kind triggerable from the rollback receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCapabilityFollowOnAnalysisKind {
    /// Correct the source manifest or lifecycle record that underlies the rollback.
    SourceManifestCorrection,
    /// Review bounded retraining or bounded checkpoint replacement.
    BoundedRetrainingAnalysis,
    /// Review whether earlier publications must be narrowed or removed.
    DepublicationAnalysis,
}

/// One follow-on analysis explicitly triggered from recorded rollback artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityFollowOnAnalysis {
    /// Stable analysis identifier.
    pub analysis_id: String,
    /// Analysis kind.
    pub kind: PsionCapabilityFollowOnAnalysisKind,
    /// Artifact that triggered the follow-on analysis.
    pub triggered_by: PsionCapabilityArtifactReference,
    /// Planned analysis artifact when one has already been assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_analysis_artifact: Option<PsionCapabilityArtifactReference>,
    /// Plain-language note describing the follow-on work.
    pub detail: String,
}

impl PsionCapabilityFollowOnAnalysis {
    fn validate(&self, field: &str) -> Result<(), PsionCapabilityWithdrawalError> {
        ensure_nonempty(
            self.analysis_id.as_str(),
            format!("{field}.analysis_id").as_str(),
        )?;
        ensure_nonempty(self.detail.as_str(), format!("{field}.detail").as_str())?;
        self.triggered_by
            .validate(format!("{field}.triggered_by").as_str())?;
        if let Some(artifact) = &self.planned_analysis_artifact {
            artifact.validate(format!("{field}.planned_analysis_artifact").as_str())?;
        }
        Ok(())
    }
}

/// Rights-change trigger evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRightsChangeRollbackEvidence {
    /// Source impact receipt that triggered rollback.
    pub source_impact_analysis: PsionSourceImpactAnalysisReceipt,
    /// Corrected source-manifest or lifecycle artifact that records the new posture.
    pub corrected_source_manifest: PsionCapabilityArtifactReference,
    /// Plain-language note.
    pub detail: String,
}

/// Contamination trigger evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionContaminationRollbackEvidence {
    /// Source impact receipt that triggered rollback.
    pub source_impact_analysis: PsionSourceImpactAnalysisReceipt,
    /// Corrected source-manifest or lifecycle artifact that records the new posture.
    pub corrected_source_manifest: PsionCapabilityArtifactReference,
    /// Plain-language note.
    pub detail: String,
}

/// Replay-failure trigger evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReplayFailureRollbackEvidence {
    /// Baseline replay receipt recorded by the promoted decision.
    pub baseline_replay_receipt_ref: String,
    /// Stable verification receipt id for the replay comparison.
    pub replay_verification_receipt_id: String,
    /// Verification receipt showing replay drift.
    pub replay_verification: TrainingReplayVerificationReceipt,
    /// Stable digest over the replay verification receipt.
    pub replay_verification_digest: String,
    /// Plain-language note.
    pub detail: String,
}

/// Route-regression trigger evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRouteRegressionRollbackEvidence {
    /// Baseline route-calibration receipt recorded by the promoted decision.
    pub baseline_route_calibration_receipt_ref: String,
    /// Newly observed route-calibration receipt that fell outside the accepted band.
    pub observed_route_calibration_receipt: PsionRouteCalibrationReceipt,
    /// Stable digest over the observed route-calibration receipt.
    pub observed_route_calibration_digest: String,
    /// Route-class evaluation artifact that captures class-level delegation drift.
    pub observed_route_class_evaluation_artifact: PsionCapabilityArtifactReference,
    /// Plain-language note.
    pub detail: String,
}

/// Refusal-regression trigger evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRefusalRegressionRollbackEvidence {
    /// Baseline refusal-calibration receipt recorded by the promoted decision.
    pub baseline_refusal_calibration_receipt_ref: String,
    /// Newly observed refusal-calibration receipt that fell outside the accepted band.
    pub observed_refusal_calibration_receipt: PsionRefusalCalibrationReceipt,
    /// Plain-language note.
    pub detail: String,
}

/// Trigger evidence for one capability-withdrawal receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "trigger_kind", rename_all = "snake_case")]
pub enum PsionCapabilityWithdrawalTrigger {
    RightsChange(PsionRightsChangeRollbackEvidence),
    ContaminationDiscovered(PsionContaminationRollbackEvidence),
    ReplayFailure(PsionReplayFailureRollbackEvidence),
    RouteRegression(PsionRouteRegressionRollbackEvidence),
    RefusalRegression(PsionRefusalRegressionRollbackEvidence),
}

impl PsionCapabilityWithdrawalTrigger {
    #[must_use]
    pub const fn kind(&self) -> PsionCapabilityWithdrawalTriggerKind {
        match self {
            Self::RightsChange(_) => PsionCapabilityWithdrawalTriggerKind::RightsChange,
            Self::ContaminationDiscovered(_) => {
                PsionCapabilityWithdrawalTriggerKind::ContaminationDiscovered
            }
            Self::ReplayFailure(_) => PsionCapabilityWithdrawalTriggerKind::ReplayFailure,
            Self::RouteRegression(_) => PsionCapabilityWithdrawalTriggerKind::RouteRegression,
            Self::RefusalRegression(_) => PsionCapabilityWithdrawalTriggerKind::RefusalRegression,
        }
    }
}

/// Canonical rollback receipt for one Psion capability withdrawal or regression event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityWithdrawalReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable withdrawal receipt id.
    pub withdrawal_id: String,
    /// Published matrix identity under review.
    pub published_matrix: PsionPublishedCapabilityMatrixReference,
    /// Promoted checkpoint plan affected by rollback.
    pub checkpoint_plan: PsionCheckpointRollbackPlan,
    /// Trigger evidence that caused the rollback.
    pub trigger: PsionCapabilityWithdrawalTrigger,
    /// Capability-matrix history updates.
    pub matrix_history: Vec<PsionCapabilityMatrixChange>,
    /// Served-claim history updates.
    pub served_claim_history: Vec<PsionServedClaimChange>,
    /// Follow-on analyses explicitly triggered by recorded artifacts.
    pub follow_on_analyses: Vec<PsionCapabilityFollowOnAnalysis>,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionCapabilityWithdrawalReceipt {
    /// Validates the withdrawal receipt against the published matrix and acceptance basis.
    pub fn validate_against_matrix_and_acceptance(
        &self,
        matrix: &PsionCapabilityMatrix,
        acceptance_matrix: &PsionAcceptanceMatrix,
        decision: &PsionPromotionDecisionReceipt,
    ) -> Result<(), PsionCapabilityWithdrawalError> {
        decision.validate_against_matrix(acceptance_matrix)?;
        matrix.validate_publication(acceptance_matrix, decision)?;

        ensure_nonempty(
            self.schema_version.as_str(),
            "capability_withdrawal.schema_version",
        )?;
        if self.schema_version != PSION_CAPABILITY_WITHDRAWAL_RECEIPT_SCHEMA_VERSION {
            return Err(PsionCapabilityWithdrawalError::SchemaVersionMismatch {
                expected: String::from(PSION_CAPABILITY_WITHDRAWAL_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.withdrawal_id.as_str(),
            "capability_withdrawal.withdrawal_id",
        )?;
        ensure_nonempty(self.summary.as_str(), "capability_withdrawal.summary")?;
        self.published_matrix.validate_against(matrix)?;
        self.checkpoint_plan
            .validate_against_checkpoint(&decision.checkpoint_receipt)?;

        if self.matrix_history.is_empty() {
            return Err(PsionCapabilityWithdrawalError::MissingField {
                field: String::from("capability_withdrawal.matrix_history"),
            });
        }
        if self.served_claim_history.is_empty() {
            return Err(PsionCapabilityWithdrawalError::MissingField {
                field: String::from("capability_withdrawal.served_claim_history"),
            });
        }
        if self.follow_on_analyses.is_empty() {
            return Err(PsionCapabilityWithdrawalError::MissingField {
                field: String::from("capability_withdrawal.follow_on_analyses"),
            });
        }

        let mut matrix_change_ids = BTreeSet::new();
        for change in &self.matrix_history {
            change.validate_against_matrix(matrix)?;
            if !matrix_change_ids.insert(change.change_id.as_str()) {
                return Err(PsionCapabilityWithdrawalError::DuplicateHistoryId {
                    field: String::from("matrix_history"),
                    id: change.change_id.clone(),
                });
            }
        }

        let mut surfaces = BTreeSet::new();
        for change in &self.served_claim_history {
            change.validate()?;
            if !surfaces.insert(change.surface_id.as_str()) {
                return Err(PsionCapabilityWithdrawalError::DuplicateHistoryId {
                    field: String::from("served_claim_history"),
                    id: change.surface_id.clone(),
                });
            }
        }

        let mut analyses = BTreeSet::new();
        for (index, analysis) in self.follow_on_analyses.iter().enumerate() {
            analysis
                .validate(format!("capability_withdrawal.follow_on_analyses[{index}]").as_str())?;
            if !analyses.insert(analysis.analysis_id.as_str()) {
                return Err(PsionCapabilityWithdrawalError::DuplicateHistoryId {
                    field: String::from("follow_on_analyses"),
                    id: analysis.analysis_id.clone(),
                });
            }
        }

        self.validate_trigger(decision, acceptance_matrix)?;

        if self.receipt_digest != stable_capability_withdrawal_digest(self) {
            return Err(PsionCapabilityWithdrawalError::DigestMismatch {
                kind: String::from("psion_capability_withdrawal_receipt"),
            });
        }
        Ok(())
    }

    fn validate_trigger(
        &self,
        decision: &PsionPromotionDecisionReceipt,
        acceptance_matrix: &PsionAcceptanceMatrix,
    ) -> Result<(), PsionCapabilityWithdrawalError> {
        let phase_gate = acceptance_matrix
            .phase_gate(self.published_matrix.acceptance_phase)
            .ok_or(PsionCapabilityWithdrawalError::AcceptancePhaseMissing {
                phase: self.published_matrix.acceptance_phase,
            })?;

        match &self.trigger {
            PsionCapabilityWithdrawalTrigger::RightsChange(evidence) => {
                self.validate_source_driven_trigger(
                    &evidence.source_impact_analysis,
                    &evidence.corrected_source_manifest,
                    PsionLifecycleChangeTrigger::RightsChangedOrRevoked,
                    "capability_withdrawal.trigger.rights_change",
                )?;
                ensure_nonempty(
                    evidence.detail.as_str(),
                    "capability_withdrawal.trigger.rights_change.detail",
                )?;
                if self.checkpoint_plan.disposition
                    != PsionCheckpointRollbackDisposition::WithdrawServedCheckpoint
                {
                    return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                        detail: String::from(
                            "rights changes must withdraw the currently served checkpoint until corrected publication exists",
                        ),
                    });
                }
                require_follow_on_kind(
                    &self.follow_on_analyses,
                    PsionCapabilityFollowOnAnalysisKind::SourceManifestCorrection,
                    self.trigger.kind(),
                )?;
                if evidence
                    .source_impact_analysis
                    .required_actions
                    .contains(&PsionLifecycleImpactAction::RetrainingReview)
                {
                    require_follow_on_kind(
                        &self.follow_on_analyses,
                        PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
                        self.trigger.kind(),
                    )?;
                }
                if evidence
                    .source_impact_analysis
                    .required_actions
                    .contains(&PsionLifecycleImpactAction::DepublicationReview)
                {
                    require_follow_on_kind(
                        &self.follow_on_analyses,
                        PsionCapabilityFollowOnAnalysisKind::DepublicationAnalysis,
                        self.trigger.kind(),
                    )?;
                }
            }
            PsionCapabilityWithdrawalTrigger::ContaminationDiscovered(evidence) => {
                self.validate_source_driven_trigger(
                    &evidence.source_impact_analysis,
                    &evidence.corrected_source_manifest,
                    PsionLifecycleChangeTrigger::BenchmarkContaminationDiscovered,
                    "capability_withdrawal.trigger.contamination_discovered",
                )?;
                ensure_nonempty(
                    evidence.detail.as_str(),
                    "capability_withdrawal.trigger.contamination_discovered.detail",
                )?;
                require_follow_on_kind(
                    &self.follow_on_analyses,
                    PsionCapabilityFollowOnAnalysisKind::SourceManifestCorrection,
                    self.trigger.kind(),
                )?;
                require_follow_on_kind(
                    &self.follow_on_analyses,
                    PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
                    self.trigger.kind(),
                )?;
                require_follow_on_kind(
                    &self.follow_on_analyses,
                    PsionCapabilityFollowOnAnalysisKind::DepublicationAnalysis,
                    self.trigger.kind(),
                )?;
            }
            PsionCapabilityWithdrawalTrigger::ReplayFailure(evidence) => {
                ensure_nonempty(
                    evidence.baseline_replay_receipt_ref.as_str(),
                    "capability_withdrawal.trigger.replay_failure.baseline_replay_receipt_ref",
                )?;
                ensure_nonempty(
                    evidence.replay_verification_receipt_id.as_str(),
                    "capability_withdrawal.trigger.replay_failure.replay_verification_receipt_id",
                )?;
                ensure_nonempty(
                    evidence.replay_verification_digest.as_str(),
                    "capability_withdrawal.trigger.replay_failure.replay_verification_digest",
                )?;
                ensure_nonempty(
                    evidence.detail.as_str(),
                    "capability_withdrawal.trigger.replay_failure.detail",
                )?;
                if evidence.baseline_replay_receipt_ref != decision.replay_receipt.receipt_id {
                    return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                        detail: String::from(
                            "replay-failure evidence did not cite the promoted baseline replay receipt",
                        ),
                    });
                }
                if evidence.replay_verification.disposition
                    != ReplayVerificationDisposition::Drifted
                {
                    return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                        detail: String::from(
                            "replay-failure evidence must carry a drifted replay verification receipt",
                        ),
                    });
                }
                if evidence.replay_verification.signals.is_empty() {
                    return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                        detail: String::from(
                            "replay-failure evidence must list one or more replay drift signals",
                        ),
                    });
                }
                if evidence.replay_verification_digest
                    != stable_replay_verification_digest(&evidence.replay_verification)
                {
                    return Err(PsionCapabilityWithdrawalError::DigestMismatch {
                        kind: String::from("psion_replay_verification_receipt"),
                    });
                }
                if self.checkpoint_plan.disposition
                    != PsionCheckpointRollbackDisposition::RollbackToPriorCheckpoint
                {
                    return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                        detail: String::from(
                            "replay failure must roll back to one prior checkpoint target",
                        ),
                    });
                }
                require_follow_on_kind(
                    &self.follow_on_analyses,
                    PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
                    self.trigger.kind(),
                )?;
                require_history_change(
                    &self.matrix_history,
                    &self.served_claim_history,
                    self.trigger.kind(),
                )?;
            }
            PsionCapabilityWithdrawalTrigger::RouteRegression(evidence) => {
                ensure_nonempty(
                    evidence.baseline_route_calibration_receipt_ref.as_str(),
                    "capability_withdrawal.trigger.route_regression.baseline_route_calibration_receipt_ref",
                )?;
                ensure_nonempty(
                    evidence.detail.as_str(),
                    "capability_withdrawal.trigger.route_regression.detail",
                )?;
                if evidence.baseline_route_calibration_receipt_ref
                    != decision.route_calibration_receipt.receipt_id
                {
                    return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                        detail: String::from(
                            "route-regression evidence did not cite the promoted baseline route receipt",
                        ),
                    });
                }
                ensure_nonempty(
                    evidence
                        .observed_route_calibration_receipt
                        .receipt_id
                        .as_str(),
                    "capability_withdrawal.trigger.route_regression.observed_route_calibration_receipt.receipt_id",
                )?;
                ensure_nonempty(
                    evidence.observed_route_calibration_receipt.summary.as_str(),
                    "capability_withdrawal.trigger.route_regression.observed_route_calibration_receipt.summary",
                )?;
                ensure_nonempty(
                    evidence.observed_route_calibration_digest.as_str(),
                    "capability_withdrawal.trigger.route_regression.observed_route_calibration_digest",
                )?;
                validate_bps(
                    evidence
                        .observed_route_calibration_receipt
                        .route_selection_accuracy_bps,
                    "capability_withdrawal.trigger.route_regression.observed_route_calibration_receipt.route_selection_accuracy_bps",
                )?;
                validate_bps(
                    evidence
                        .observed_route_calibration_receipt
                        .route_regression_bps,
                    "capability_withdrawal.trigger.route_regression.observed_route_calibration_receipt.route_regression_bps",
                )?;
                if evidence.observed_route_calibration_digest
                    != stable_route_calibration_digest(&evidence.observed_route_calibration_receipt)
                {
                    return Err(PsionCapabilityWithdrawalError::DigestMismatch {
                        kind: String::from("psion_route_calibration_receipt"),
                    });
                }
                evidence
                    .observed_route_class_evaluation_artifact
                    .validate(
                        "capability_withdrawal.trigger.route_regression.observed_route_class_evaluation_artifact",
                    )?;
                let route_requirement = &phase_gate.route_calibration_requirement;
                let outside_band = evidence
                    .observed_route_calibration_receipt
                    .route_regression_bps
                    > route_requirement.maximum_route_regression_bps
                    || evidence
                        .observed_route_calibration_receipt
                        .route_selection_accuracy_bps
                        < route_requirement.minimum_route_selection_accuracy_bps;
                if !outside_band {
                    return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                        detail: String::from(
                            "route-regression evidence stayed inside the accepted route band",
                        ),
                    });
                }
                require_follow_on_kind(
                    &self.follow_on_analyses,
                    PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
                    self.trigger.kind(),
                )?;
                require_history_change(
                    &self.matrix_history,
                    &self.served_claim_history,
                    self.trigger.kind(),
                )?;
            }
            PsionCapabilityWithdrawalTrigger::RefusalRegression(evidence) => {
                ensure_nonempty(
                    evidence.baseline_refusal_calibration_receipt_ref.as_str(),
                    "capability_withdrawal.trigger.refusal_regression.baseline_refusal_calibration_receipt_ref",
                )?;
                ensure_nonempty(
                    evidence.detail.as_str(),
                    "capability_withdrawal.trigger.refusal_regression.detail",
                )?;
                if evidence.baseline_refusal_calibration_receipt_ref
                    != decision.refusal_calibration_receipt.receipt_id
                {
                    return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                        detail: String::from(
                            "refusal-regression evidence did not cite the promoted baseline refusal receipt",
                        ),
                    });
                }
                ensure_nonempty(
                    evidence
                        .observed_refusal_calibration_receipt
                        .receipt_id
                        .as_str(),
                    "capability_withdrawal.trigger.refusal_regression.observed_refusal_calibration_receipt.receipt_id",
                )?;
                ensure_nonempty(
                    evidence
                        .observed_refusal_calibration_receipt
                        .summary
                        .as_str(),
                    "capability_withdrawal.trigger.refusal_regression.observed_refusal_calibration_receipt.summary",
                )?;
                ensure_nonempty(
                    evidence
                        .observed_refusal_calibration_receipt
                        .receipt_digest
                        .as_str(),
                    "capability_withdrawal.trigger.refusal_regression.observed_refusal_calibration_receipt.receipt_digest",
                )?;
                validate_bps(
                    evidence
                        .observed_refusal_calibration_receipt
                        .aggregate_unsupported_request_refusal_bps,
                    "capability_withdrawal.trigger.refusal_regression.observed_refusal_calibration_receipt.aggregate_unsupported_request_refusal_bps",
                )?;
                validate_bps(
                    evidence
                        .observed_refusal_calibration_receipt
                        .aggregate_reason_code_match_bps,
                    "capability_withdrawal.trigger.refusal_regression.observed_refusal_calibration_receipt.aggregate_reason_code_match_bps",
                )?;
                validate_bps(
                    evidence
                        .observed_refusal_calibration_receipt
                        .supported_control_overrefusal_bps,
                    "capability_withdrawal.trigger.refusal_regression.observed_refusal_calibration_receipt.supported_control_overrefusal_bps",
                )?;
                validate_bps(
                    evidence
                        .observed_refusal_calibration_receipt
                        .refusal_regression_bps,
                    "capability_withdrawal.trigger.refusal_regression.observed_refusal_calibration_receipt.refusal_regression_bps",
                )?;
                let refusal_requirement = &phase_gate.refusal_calibration_requirement;
                let outside_band = evidence
                    .observed_refusal_calibration_receipt
                    .refusal_regression_bps
                    > refusal_requirement.maximum_refusal_regression_bps
                    || evidence
                        .observed_refusal_calibration_receipt
                        .aggregate_unsupported_request_refusal_bps
                        < refusal_requirement.minimum_unsupported_request_refusal_bps
                    || evidence
                        .observed_refusal_calibration_receipt
                        .supported_control_overrefusal_bps
                        > refusal_requirement.maximum_overrefusal_bps;
                if !outside_band {
                    return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                        detail: String::from(
                            "refusal-regression evidence stayed inside the accepted refusal band",
                        ),
                    });
                }
                require_follow_on_kind(
                    &self.follow_on_analyses,
                    PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
                    self.trigger.kind(),
                )?;
                require_history_change(
                    &self.matrix_history,
                    &self.served_claim_history,
                    self.trigger.kind(),
                )?;
            }
        }
        Ok(())
    }

    fn validate_source_driven_trigger(
        &self,
        impact: &PsionSourceImpactAnalysisReceipt,
        corrected_source_manifest: &PsionCapabilityArtifactReference,
        expected_trigger: PsionLifecycleChangeTrigger,
        field: &str,
    ) -> Result<(), PsionCapabilityWithdrawalError> {
        corrected_source_manifest
            .validate(format!("{field}.corrected_source_manifest").as_str())?;
        ensure_nonempty(
            impact.source_id.as_str(),
            format!("{field}.source_id").as_str(),
        )?;
        if impact.trigger != expected_trigger {
            return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                detail: format!(
                    "source-driven rollback expected lifecycle trigger `{expected_trigger:?}`, found `{}`",
                    lifecycle_trigger_label(impact.trigger)
                ),
            });
        }
        if !impact
            .required_actions
            .contains(&PsionLifecycleImpactAction::CapabilityMatrixReview)
            || !impact
                .required_actions
                .contains(&PsionLifecycleImpactAction::DepublicationReview)
        {
            return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
                detail: String::from(
                    "source-driven rollback must carry lifecycle actions for both capability-matrix review and depublication review",
                ),
            });
        }
        Ok(())
    }
}

/// Records one capability-withdrawal receipt after validating the rollback contract.
pub fn record_psion_capability_withdrawal_receipt(
    withdrawal_id: impl Into<String>,
    matrix: &PsionCapabilityMatrix,
    acceptance_matrix: &PsionAcceptanceMatrix,
    decision: &PsionPromotionDecisionReceipt,
    checkpoint_plan: PsionCheckpointRollbackPlan,
    trigger: PsionCapabilityWithdrawalTrigger,
    matrix_history: Vec<PsionCapabilityMatrixChange>,
    served_claim_history: Vec<PsionServedClaimChange>,
    follow_on_analyses: Vec<PsionCapabilityFollowOnAnalysis>,
    summary: impl Into<String>,
) -> Result<PsionCapabilityWithdrawalReceipt, PsionCapabilityWithdrawalError> {
    let mut receipt = PsionCapabilityWithdrawalReceipt {
        schema_version: String::from(PSION_CAPABILITY_WITHDRAWAL_RECEIPT_SCHEMA_VERSION),
        withdrawal_id: withdrawal_id.into(),
        published_matrix: PsionPublishedCapabilityMatrixReference::from_matrix(matrix),
        checkpoint_plan,
        trigger,
        matrix_history,
        served_claim_history,
        follow_on_analyses,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_capability_withdrawal_digest(&receipt);
    receipt.validate_against_matrix_and_acceptance(matrix, acceptance_matrix, decision)?;
    Ok(receipt)
}

/// Errors surfaced while recording or validating capability-withdrawal receipts.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionCapabilityWithdrawalError {
    /// The linked acceptance contract rejected the publication basis.
    #[error(transparent)]
    Acceptance(#[from] PsionAcceptanceMatrixError),
    /// The linked capability-matrix publication basis rejected the target matrix.
    #[error(transparent)]
    CapabilityMatrix(#[from] PsionCapabilityMatrixError),
    /// One required field was missing.
    #[error("Psion capability-withdrawal field `{field}` is missing")]
    MissingField {
        /// Missing field label.
        field: String,
    },
    /// The rollback schema version drifted from the current contract.
    #[error("Psion capability-withdrawal expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// The published matrix linkage drifted from the target matrix.
    #[error("Psion capability-withdrawal published matrix mismatch: {detail}")]
    PublishedMatrixMismatch {
        /// Machine-readable detail.
        detail: String,
    },
    /// The checkpoint plan drifted from the promoted checkpoint.
    #[error("Psion capability-withdrawal checkpoint plan mismatch: {detail}")]
    CheckpointPlanMismatch {
        /// Machine-readable detail.
        detail: String,
    },
    /// One capability region was unknown.
    #[error("Psion capability-withdrawal references unknown capability region `{region_id:?}`")]
    UnknownCapabilityRegion {
        /// Unknown region identifier.
        region_id: PsionCapabilityRegionId,
    },
    /// One history entry was malformed.
    #[error("Psion capability-withdrawal matrix history is invalid: {detail}")]
    InvalidMatrixChange {
        /// Machine-readable detail.
        detail: String,
    },
    /// One served-claim history entry was malformed.
    #[error("Psion capability-withdrawal served-claim history is invalid: {detail}")]
    InvalidServedClaimChange {
        /// Machine-readable detail.
        detail: String,
    },
    /// A trigger was inconsistent with the attached receipts or downgrade plan.
    #[error("Psion capability-withdrawal trigger mismatch: {detail}")]
    TriggerMismatch {
        /// Machine-readable detail.
        detail: String,
    },
    /// A phase was missing from the acceptance matrix.
    #[error("Psion capability-withdrawal acceptance matrix is missing phase `{phase:?}`")]
    AcceptancePhaseMissing {
        /// Missing phase.
        phase: PsionPhaseGate,
    },
    /// One history or analysis id was repeated.
    #[error("Psion capability-withdrawal repeated `{field}` id `{id}`")]
    DuplicateHistoryId {
        /// Repeated collection name.
        field: String,
        /// Repeated id.
        id: String,
    },
    /// The stable digest drifted from the receipt contents.
    #[error("Psion capability-withdrawal digest mismatch for `{kind}`")]
    DigestMismatch {
        /// Artifact kind.
        kind: String,
    },
}

/// Computes the stable digest over one published capability matrix.
#[must_use]
pub fn stable_capability_matrix_digest(matrix: &PsionCapabilityMatrix) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_capability_matrix_publication|");
    hasher.update(serde_json::to_vec(matrix).expect("capability matrix should serialize"));
    hex::encode(hasher.finalize())
}

/// Computes the stable digest over one replay-verification receipt.
#[must_use]
pub fn stable_replay_verification_digest(receipt: &TrainingReplayVerificationReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_replay_verification_receipt|");
    hasher.update(serde_json::to_vec(receipt).expect("replay verification should serialize"));
    hex::encode(hasher.finalize())
}

/// Computes the stable digest over one route-calibration receipt.
#[must_use]
pub fn stable_route_calibration_digest(receipt: &PsionRouteCalibrationReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_route_calibration_receipt|");
    hasher.update(serde_json::to_vec(receipt).expect("route calibration should serialize"));
    hex::encode(hasher.finalize())
}

fn stable_capability_withdrawal_digest(receipt: &PsionCapabilityWithdrawalReceipt) -> String {
    let mut stripped = receipt.clone();
    stripped.receipt_digest.clear();
    let mut hasher = Sha256::new();
    hasher.update(b"psion_capability_withdrawal_receipt|");
    hasher.update(serde_json::to_vec(&stripped).expect("capability withdrawal should serialize"));
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionCapabilityWithdrawalError> {
    if value.trim().is_empty() {
        return Err(PsionCapabilityWithdrawalError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn validate_bps(value: u32, field: &str) -> Result<(), PsionCapabilityWithdrawalError> {
    if value > 10_000 {
        return Err(PsionCapabilityWithdrawalError::TriggerMismatch {
            detail: format!("field `{field}` expected bps in 0..=10000, found `{value}`"),
        });
    }
    Ok(())
}

fn reject_duplicate_enum_entries<T, F>(
    values: &[T],
    error_fn: F,
) -> Result<(), PsionCapabilityWithdrawalError>
where
    T: Copy + Ord,
    F: Fn(T) -> PsionCapabilityWithdrawalError,
{
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(*value) {
            return Err(error_fn(*value));
        }
    }
    Ok(())
}

fn posture_rank(posture: PsionCapabilityPosture) -> u8 {
    match posture {
        PsionCapabilityPosture::Supported => 0,
        PsionCapabilityPosture::RouteRequired => 1,
        PsionCapabilityPosture::RefusalRequired => 2,
        PsionCapabilityPosture::Unsupported => 3,
    }
}

fn behavior_rank(behavior: &PsionServedBehaviorVisibility) -> u8 {
    match behavior {
        PsionServedBehaviorVisibility::Route { route_kind, .. } => match route_kind {
            psionic_train::PsionRouteKind::DirectModelAnswer => 0,
            psionic_train::PsionRouteKind::ExactExecutorHandoff => 1,
            psionic_train::PsionRouteKind::Refusal => 2,
        },
        PsionServedBehaviorVisibility::Refusal { .. } => 2,
    }
}

fn ensure_behavior_detail(
    behavior: &PsionServedBehaviorVisibility,
    field: &str,
) -> Result<(), PsionCapabilityWithdrawalError> {
    let detail = match behavior {
        PsionServedBehaviorVisibility::Route { detail, .. }
        | PsionServedBehaviorVisibility::Refusal { detail, .. } => detail,
    };
    ensure_nonempty(detail.as_str(), format!("{field}.detail").as_str())
}

fn claim_flag_visible(claims: &PsionServedVisibleClaims, flag: PsionServedClaimFlag) -> bool {
    match flag {
        PsionServedClaimFlag::LearnedJudgment => claims.learned_judgment_visible,
        PsionServedClaimFlag::SourceGrounding => claims.source_grounding_visible,
        PsionServedClaimFlag::ExecutorBacking => claims.executor_backing_visible,
        PsionServedClaimFlag::BenchmarkBacking => claims.benchmark_backing_visible,
        PsionServedClaimFlag::Verification => claims.verification_visible,
    }
}

fn all_claim_flags() -> [PsionServedClaimFlag; 5] {
    [
        PsionServedClaimFlag::LearnedJudgment,
        PsionServedClaimFlag::SourceGrounding,
        PsionServedClaimFlag::ExecutorBacking,
        PsionServedClaimFlag::BenchmarkBacking,
        PsionServedClaimFlag::Verification,
    ]
}

fn lifecycle_trigger_label(trigger: PsionLifecycleChangeTrigger) -> &'static str {
    match trigger {
        PsionLifecycleChangeTrigger::RightsChangedOrRevoked => "rights_changed_or_revoked",
        PsionLifecycleChangeTrigger::ProvenanceInvalidated => "provenance_invalidated",
        PsionLifecycleChangeTrigger::ContentDigestMismatch => "content_digest_mismatch",
        PsionLifecycleChangeTrigger::BenchmarkContaminationDiscovered => {
            "benchmark_contamination_discovered"
        }
        PsionLifecycleChangeTrigger::ReclassifiedToRestrictedUse => {
            "reclassified_to_restricted_use"
        }
        PsionLifecycleChangeTrigger::ReclassifiedToEvaluationOnly => {
            "reclassified_to_evaluation_only"
        }
        PsionLifecycleChangeTrigger::ReviewMisclassificationCorrected => {
            "review_misclassification_corrected"
        }
        PsionLifecycleChangeTrigger::SourceRetractedOrCanonicallyReplaced => {
            "source_retracted_or_canonically_replaced"
        }
    }
}

fn require_follow_on_kind(
    analyses: &[PsionCapabilityFollowOnAnalysis],
    kind: PsionCapabilityFollowOnAnalysisKind,
    trigger: PsionCapabilityWithdrawalTriggerKind,
) -> Result<(), PsionCapabilityWithdrawalError> {
    if analyses.iter().any(|analysis| analysis.kind == kind) {
        return Ok(());
    }
    Err(PsionCapabilityWithdrawalError::TriggerMismatch {
        detail: format!("trigger `{trigger:?}` requires follow-on analysis `{kind:?}`",),
    })
}

fn require_history_change(
    matrix_history: &[PsionCapabilityMatrixChange],
    served_claim_history: &[PsionServedClaimChange],
    trigger: PsionCapabilityWithdrawalTriggerKind,
) -> Result<(), PsionCapabilityWithdrawalError> {
    let has_matrix_downgrade = matrix_history.iter().any(|change| {
        matches!(
            change.kind,
            PsionCapabilityMatrixChangeKind::WithdrawPublication
                | PsionCapabilityMatrixChangeKind::DowngradeRegionPosture
        )
    });
    let has_claim_change = served_claim_history.iter().any(|change| {
        matches!(
            change.kind,
            PsionServedClaimChangeKind::Depublish
                | PsionServedClaimChangeKind::NarrowVisibleClaims
                | PsionServedClaimChangeKind::ChangeBehaviorVisibility
        )
    });
    if has_matrix_downgrade && has_claim_change {
        return Ok(());
    }
    Err(PsionCapabilityWithdrawalError::TriggerMismatch {
        detail: format!(
            "trigger `{trigger:?}` requires both matrix-history and served-claim-history downgrade entries",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use psionic_train::PsionRouteClassEvaluationReceipt;

    fn acceptance_matrix() -> PsionAcceptanceMatrix {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/acceptance/psion_acceptance_matrix_v1.json"
        ))
        .expect("acceptance matrix fixture should parse")
    }

    fn promotion_decision() -> PsionPromotionDecisionReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/acceptance/psion_promotion_decision_receipt_v1.json"
        ))
        .expect("promotion decision fixture should parse")
    }

    fn capability_matrix() -> PsionCapabilityMatrix {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/capability/psion_capability_matrix_v1.json"
        ))
        .expect("capability matrix fixture should parse")
    }

    fn route_class_evaluation() -> PsionRouteClassEvaluationReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/route/psion_route_class_evaluation_receipt_v1.json"
        ))
        .expect("route class evaluation fixture should parse")
    }

    fn direct_claim_posture() -> PsionServedOutputClaimPosture {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_served_output_claim_direct_v1.json"
        ))
        .expect("direct claim posture fixture should parse")
    }

    fn executor_claim_posture() -> PsionServedOutputClaimPosture {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_served_output_claim_executor_backed_v1.json"
        ))
        .expect("executor claim posture fixture should parse")
    }

    fn refusal_claim_posture() -> PsionServedOutputClaimPosture {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_served_output_claim_refusal_v1.json"
        ))
        .expect("refusal claim posture fixture should parse")
    }

    #[test]
    fn capability_withdrawal_route_regression_fixture_validates() {
        let receipt: PsionCapabilityWithdrawalReceipt = serde_json::from_str(include_str!(
            "../../../fixtures/psion/withdrawal/psion_capability_withdrawal_route_regression_v1.json"
        ))
        .expect("route regression fixture should parse");
        receipt
            .validate_against_matrix_and_acceptance(
                &capability_matrix(),
                &acceptance_matrix(),
                &promotion_decision(),
            )
            .expect("route regression fixture should validate");
    }

    #[test]
    fn capability_withdrawal_rejects_non_drifted_replay_evidence() {
        let acceptance = acceptance_matrix();
        let decision = promotion_decision();
        let matrix = capability_matrix();
        let mut receipt: PsionCapabilityWithdrawalReceipt = serde_json::from_str(include_str!(
            "../../../fixtures/psion/withdrawal/psion_capability_withdrawal_replay_failure_v1.json"
        ))
        .expect("replay failure fixture should parse");
        if let PsionCapabilityWithdrawalTrigger::ReplayFailure(evidence) = &mut receipt.trigger {
            evidence.replay_verification.disposition = ReplayVerificationDisposition::Match;
            evidence.replay_verification.signals.clear();
            evidence.replay_verification_digest =
                stable_replay_verification_digest(&evidence.replay_verification);
        }
        receipt.receipt_digest = stable_capability_withdrawal_digest(&receipt);
        let error = receipt
            .validate_against_matrix_and_acceptance(&matrix, &acceptance, &decision)
            .expect_err("non-drifted replay evidence should be rejected");
        assert!(matches!(
            error,
            PsionCapabilityWithdrawalError::TriggerMismatch { .. }
        ));
    }

    #[test]
    fn served_claim_change_cannot_introduce_new_visible_claims() {
        let acceptance = acceptance_matrix();
        let decision = promotion_decision();
        let matrix = capability_matrix();
        let direct = direct_claim_posture();
        let executor = executor_claim_posture();
        let route_class = route_class_evaluation();
        let route_history = vec![PsionCapabilityMatrixChange {
            change_id: String::from("change.route_regression.v1"),
            kind: PsionCapabilityMatrixChangeKind::DowngradeRegionPosture,
            matrix_id: matrix.matrix_id.clone(),
            matrix_version: matrix.matrix_version.clone(),
            matrix_digest: stable_capability_matrix_digest(&matrix),
            downgraded_regions: vec![PsionCapabilityRegionDowngrade {
                region_id: PsionCapabilityRegionId::VerifiedOrExactExecutionRequests,
                previous_posture: PsionCapabilityPosture::RouteRequired,
                next_posture: PsionCapabilityPosture::Unsupported,
                next_refusal_reasons: vec![
                    PsionCapabilityRefusalReason::UnsupportedExactnessRequest,
                ],
                detail: String::from("Disable exactness routing after route drift."),
            }],
            detail: String::from("The exactness route is withdrawn after route regression."),
        }];
        let mut claim_history = vec![PsionServedClaimChange {
            surface_id: String::from("psion.direct_generation"),
            kind: PsionServedClaimChangeKind::NarrowVisibleClaims,
            previous_posture: PsionServedClaimPostureSummary::from_posture(&direct),
            replacement_posture: Some(PsionServedClaimPostureSummary::from_posture(&executor)),
            removed_visible_claims: vec![
                PsionServedClaimFlag::LearnedJudgment,
                PsionServedClaimFlag::SourceGrounding,
            ],
            detail: String::from("Route regression narrows direct claims."),
        }];
        claim_history[0]
            .replacement_posture
            .as_mut()
            .expect("replacement")
            .visible_claims
            .benchmark_backing_visible = true;
        let route_receipt = PsionRouteCalibrationReceipt {
            receipt_id: String::from("psion-route-calibration-regressed-v1"),
            covered_routes: vec![
                psionic_train::PsionRouteKind::DirectModelAnswer,
                psionic_train::PsionRouteKind::ExactExecutorHandoff,
                psionic_train::PsionRouteKind::Refusal,
            ],
            route_selection_accuracy_bps: 9300,
            route_regression_bps: 480,
            summary: String::from("Route calibration drifted below the accepted band."),
        };
        let receipt = record_psion_capability_withdrawal_receipt(
            "psion-capability-withdrawal-route-invalid-v1",
            &matrix,
            &acceptance,
            &decision,
            PsionCheckpointRollbackPlan {
                published_checkpoint_id: decision.checkpoint_receipt.checkpoint_id.clone(),
                disposition: PsionCheckpointRollbackDisposition::RollbackToPriorCheckpoint,
                rollback_target_checkpoint_id: Some(String::from("pilot-pretrain-last-known-good")),
                detail: String::from("Rollback to the last known good checkpoint."),
            },
            PsionCapabilityWithdrawalTrigger::RouteRegression(
                PsionRouteRegressionRollbackEvidence {
                    baseline_route_calibration_receipt_ref: decision
                        .route_calibration_receipt
                        .receipt_id
                        .clone(),
                    observed_route_calibration_digest: stable_route_calibration_digest(
                        &route_receipt,
                    ),
                    observed_route_calibration_receipt: route_receipt,
                    observed_route_class_evaluation_artifact: PsionCapabilityArtifactReference {
                        artifact_id: route_class.receipt_id.clone(),
                        artifact_digest: route_class.receipt_digest.clone(),
                    },
                    detail: String::from("Route drift disables the exactness lane."),
                },
            ),
            route_history,
            claim_history,
            vec![PsionCapabilityFollowOnAnalysis {
                analysis_id: String::from("analysis.route_retraining.v1"),
                kind: PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
                triggered_by: PsionCapabilityArtifactReference {
                    artifact_id: String::from("psion-route-calibration-regressed-v1"),
                    artifact_digest: stable_route_calibration_digest(
                        &PsionRouteCalibrationReceipt {
                            receipt_id: String::from("psion-route-calibration-regressed-v1"),
                            covered_routes: vec![
                                psionic_train::PsionRouteKind::DirectModelAnswer,
                                psionic_train::PsionRouteKind::ExactExecutorHandoff,
                                psionic_train::PsionRouteKind::Refusal,
                            ],
                            route_selection_accuracy_bps: 9300,
                            route_regression_bps: 480,
                            summary: String::from(
                                "Route calibration drifted below the accepted band.",
                            ),
                        },
                    ),
                },
                planned_analysis_artifact: None,
                detail: String::from(
                    "Retraining analysis is required before republishing route claims.",
                ),
            }],
            "Invalid example used to prove served-claim narrowing cannot introduce new claims.",
        );
        assert!(matches!(
            receipt,
            Err(PsionCapabilityWithdrawalError::InvalidServedClaimChange { .. })
        ));
    }

    #[test]
    fn matrix_change_requires_stricter_posture() {
        let matrix = capability_matrix();
        let error = PsionCapabilityRegionDowngrade {
            region_id: PsionCapabilityRegionId::VerifiedOrExactExecutionRequests,
            previous_posture: PsionCapabilityPosture::RouteRequired,
            next_posture: PsionCapabilityPosture::RouteRequired,
            next_refusal_reasons: Vec::new(),
            detail: String::from("No-op."),
        }
        .validate_against_matrix(&matrix)
        .expect_err("no-op posture change should be rejected");
        assert!(matches!(
            error,
            PsionCapabilityWithdrawalError::InvalidMatrixChange { .. }
        ));
    }

    #[test]
    fn refusal_regression_example_moves_executor_claim_to_refusal() {
        let receipt: PsionCapabilityWithdrawalReceipt = serde_json::from_str(include_str!(
            "../../../fixtures/psion/withdrawal/psion_capability_withdrawal_refusal_regression_v1.json"
        ))
        .expect("refusal regression fixture should parse");
        let change = receipt
            .served_claim_history
            .iter()
            .find(|change| change.surface_id == "psion.exact_executor_surface")
            .expect("expected served claim change");
        assert_eq!(
            change.kind,
            PsionServedClaimChangeKind::ChangeBehaviorVisibility
        );
        assert_eq!(
            change.previous_posture,
            PsionServedClaimPostureSummary::from_posture(&executor_claim_posture())
        );
        assert_eq!(
            change.replacement_posture,
            Some(PsionServedClaimPostureSummary::from_posture(
                &refusal_claim_posture()
            ))
        );
    }
}
