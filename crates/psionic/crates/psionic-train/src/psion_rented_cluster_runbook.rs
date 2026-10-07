use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ArtifactArchiveClass, ArtifactColdRestoreAction, ArtifactColdRestoreReceipt,
    ArtifactRetentionProfile, ArtifactStorageSweepReceipt, PsionCheckpointRecoveryBundle,
    PsionCheckpointRecoveryContractError, PsionCheckpointRecoveryEventKind, TrainAdmissionOutcome,
    TrainAdmissionReceipt, TrainArtifactClass, TrainArtifactStorageController,
    TrainArtifactStorageError, TrainCompletionReceipt, TrainSchedulingAccountingController,
    TrainSchedulingAccountingError, TrainSchedulingAccountingPolicy, TrainingRecoveryMode,
};

/// Stable schema version for the Psion rented-cluster runbook.
pub const PSION_RENTED_CLUSTER_RUNBOOK_SCHEMA_VERSION: &str = "psion.rented_cluster_runbook.v1";
/// Stable schema version for one rented-cluster stop-condition receipt.
pub const PSION_RENTED_CLUSTER_STOP_CONDITION_SCHEMA_VERSION: &str =
    "psion.rented_cluster_stop_condition_receipt.v1";
/// Stable schema version for the rented-cluster failure rehearsal bundle.
pub const PSION_RENTED_CLUSTER_FAILURE_REHEARSAL_BUNDLE_SCHEMA_VERSION: &str =
    "psion.rented_cluster_failure_rehearsal_bundle.v1";

/// Infra mode evaluated by the rented-cluster runbook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRentedClusterInfraMode {
    /// Ephemeral on-demand workers within one trusted region.
    SingleRegionOnDemand,
    /// Ephemeral spot workers within one trusted region.
    SingleRegionSpot,
    /// Ephemeral workers spread across regions or mixed links.
    CrossRegionEphemeral,
    /// Shared or otherwise untrusted cluster fabric.
    UntrustedSharedCluster,
}

/// Allowed outcome for one rented-cluster infra mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRentedClusterInfraDisposition {
    /// Mode is supported under the runbook without additional downgrade.
    Supported,
    /// Mode is allowed only with resume-from-last-stable-checkpoint posture.
    DowngradedResumeOnly,
    /// Mode is refused.
    Refused,
}

/// One explicit rented-cluster mode evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRentedClusterModeEvaluation {
    /// Infra mode being evaluated.
    pub infra_mode: PsionRentedClusterInfraMode,
    /// Short topology label for the evaluated mode.
    pub topology_label: String,
    /// Final support, downgrade, or refusal outcome.
    pub disposition: PsionRentedClusterInfraDisposition,
    /// Recovery mode required by the outcome when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_recovery_mode: Option<TrainingRecoveryMode>,
    /// Short detail for the evaluation.
    pub detail: String,
}

/// Stop condition covered by the rented-cluster runbook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRentedClusterStopConditionKind {
    /// Preemption frequency exceeded the runbook downgrade threshold.
    PreemptionBudgetExceeded,
    /// Cost overrun exceeded the runbook stop threshold.
    CostGuardrailExceeded,
}

impl PsionRentedClusterStopConditionKind {
    #[must_use]
    pub const fn required_kinds() -> [Self; 2] {
        [Self::PreemptionBudgetExceeded, Self::CostGuardrailExceeded]
    }
}

/// Run action triggered by one rented-cluster stop condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRentedClusterRunAction {
    /// Continue but downgrade the run to explicit resume-only posture.
    DowngradeToResumeOnly,
    /// Stop the run and preserve receipts.
    StopRun,
}

/// One typed stop-condition receipt for rented-cluster operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRentedClusterStopConditionReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stop-condition class.
    pub condition_kind: PsionRentedClusterStopConditionKind,
    /// Receipt or workload id that triggered the condition.
    pub source_ref: String,
    /// Observed value for the threshold comparison.
    pub observed_value: u64,
    /// Threshold value frozen by the runbook.
    pub threshold_value: u64,
    /// Action selected by the runbook.
    pub action: PsionRentedClusterRunAction,
    /// Short summary of the condition.
    pub detail: String,
    /// Stable digest over the stop-condition receipt.
    pub receipt_digest: String,
}

impl PsionRentedClusterStopConditionReceipt {
    fn validate(&self) -> Result<(), PsionRentedClusterRunbookError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "rented_cluster_stop_condition.schema_version",
        )?;
        if self.schema_version != PSION_RENTED_CLUSTER_STOP_CONDITION_SCHEMA_VERSION {
            return Err(PsionRentedClusterRunbookError::SchemaVersionMismatch {
                expected: String::from(PSION_RENTED_CLUSTER_STOP_CONDITION_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "rented_cluster_stop_condition.receipt_id",
        )?;
        ensure_nonempty(
            self.source_ref.as_str(),
            "rented_cluster_stop_condition.source_ref",
        )?;
        ensure_nonempty(self.detail.as_str(), "rented_cluster_stop_condition.detail")?;
        if self.observed_value < self.threshold_value {
            return Err(PsionRentedClusterRunbookError::StopConditionNotTriggered {
                receipt_id: self.receipt_id.clone(),
                observed_value: self.observed_value,
                threshold_value: self.threshold_value,
            });
        }
        if self.receipt_digest != stable_stop_condition_digest(self) {
            return Err(
                PsionRentedClusterRunbookError::StopConditionDigestMismatch {
                    receipt_id: self.receipt_id.clone(),
                },
            );
        }
        Ok(())
    }
}

/// Machine-readable rented-cluster runbook bound to the checkpoint-recovery bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRentedClusterRunbook {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable runbook identifier.
    pub runbook_id: String,
    /// Stable digest of the bound checkpoint-recovery bundle.
    pub checkpoint_recovery_bundle_digest: String,
    /// Artifact-storage profiles frozen for rented-cluster operation.
    pub storage_profiles: BTreeMap<TrainArtifactClass, ArtifactRetentionProfile>,
    /// Scheduling and accounting policy frozen for rented-cluster operation.
    pub scheduling_policy: TrainSchedulingAccountingPolicy,
    /// Maximum preemption events allowed before downgrade.
    pub max_preemption_events_before_downgrade: u16,
    /// Maximum cost overrun in basis points before stopping the run.
    pub max_cost_overrun_bps_before_stop: u16,
    /// Explicit infra-mode evaluations.
    pub mode_evaluations: Vec<PsionRentedClusterModeEvaluation>,
    /// Short runbook summary.
    pub summary: String,
    /// Stable digest over the runbook.
    pub runbook_digest: String,
}

impl PsionRentedClusterRunbook {
    /// Validates the runbook against the bound checkpoint-recovery bundle.
    pub fn validate_against_recovery_bundle(
        &self,
        recovery_bundle: &PsionCheckpointRecoveryBundle,
    ) -> Result<(), PsionRentedClusterRunbookError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "rented_cluster_runbook.schema_version",
        )?;
        if self.schema_version != PSION_RENTED_CLUSTER_RUNBOOK_SCHEMA_VERSION {
            return Err(PsionRentedClusterRunbookError::SchemaVersionMismatch {
                expected: String::from(PSION_RENTED_CLUSTER_RUNBOOK_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.runbook_id.as_str(),
            "rented_cluster_runbook.runbook_id",
        )?;
        check_string_match(
            self.checkpoint_recovery_bundle_digest.as_str(),
            recovery_bundle.bundle_digest.as_str(),
            "rented_cluster_runbook.checkpoint_recovery_bundle_digest",
        )?;
        if self.max_preemption_events_before_downgrade == 0 {
            return Err(PsionRentedClusterRunbookError::InvalidPreemptionThreshold);
        }
        check_bps(
            self.max_cost_overrun_bps_before_stop,
            "rented_cluster_runbook.max_cost_overrun_bps_before_stop",
        )?;
        ensure_nonempty(self.summary.as_str(), "rented_cluster_runbook.summary")?;
        self.validate_storage_profiles()?;
        TrainArtifactStorageController::new(self.storage_profiles.clone())?;
        TrainSchedulingAccountingController::new(self.scheduling_policy.clone())?;
        self.validate_mode_evaluations()?;
        if self.runbook_digest != stable_rented_cluster_runbook_digest(self) {
            return Err(PsionRentedClusterRunbookError::RunbookDigestMismatch);
        }
        Ok(())
    }

    fn validate_storage_profiles(&self) -> Result<(), PsionRentedClusterRunbookError> {
        let checkpoint_profile = self
            .storage_profiles
            .get(&TrainArtifactClass::Checkpoint)
            .ok_or(PsionRentedClusterRunbookError::MissingStorageProfile {
                artifact_class: TrainArtifactClass::Checkpoint,
            })?;
        if checkpoint_profile.archive_class == ArtifactArchiveClass::Ephemeral {
            return Err(PsionRentedClusterRunbookError::CheckpointProfileEphemeral);
        }
        if checkpoint_profile.cold_restore_sla_ms == 0 {
            return Err(PsionRentedClusterRunbookError::MissingField {
                field: String::from("rented_cluster_runbook.storage_profiles.checkpoint"),
            });
        }
        let log_profile = self
            .storage_profiles
            .get(&TrainArtifactClass::LogBundle)
            .ok_or(PsionRentedClusterRunbookError::MissingStorageProfile {
                artifact_class: TrainArtifactClass::LogBundle,
            })?;
        if log_profile.archive_class != ArtifactArchiveClass::Ephemeral {
            return Err(PsionRentedClusterRunbookError::LogProfileMustStayEphemeral);
        }
        Ok(())
    }

    fn validate_mode_evaluations(&self) -> Result<(), PsionRentedClusterRunbookError> {
        if self.mode_evaluations.is_empty() {
            return Err(PsionRentedClusterRunbookError::MissingField {
                field: String::from("rented_cluster_runbook.mode_evaluations"),
            });
        }
        let mut seen_modes = BTreeSet::new();
        let mut supported = false;
        let mut downgraded = false;
        let mut refused = false;
        for evaluation in &self.mode_evaluations {
            if !seen_modes.insert(evaluation.infra_mode) {
                return Err(
                    PsionRentedClusterRunbookError::DuplicateInfraModeEvaluation {
                        infra_mode: format!("{:?}", evaluation.infra_mode),
                    },
                );
            }
            ensure_nonempty(
                evaluation.topology_label.as_str(),
                "rented_cluster_runbook.mode_evaluations[].topology_label",
            )?;
            ensure_nonempty(
                evaluation.detail.as_str(),
                "rented_cluster_runbook.mode_evaluations[].detail",
            )?;
            match evaluation.disposition {
                PsionRentedClusterInfraDisposition::Supported => {
                    supported = true;
                }
                PsionRentedClusterInfraDisposition::DowngradedResumeOnly => {
                    downgraded = true;
                    if evaluation.required_recovery_mode
                        != Some(TrainingRecoveryMode::ResumeFromLastStableCheckpoint)
                    {
                        return Err(
                            PsionRentedClusterRunbookError::InfraModeEvaluationViolation {
                                infra_mode: format!("{:?}", evaluation.infra_mode),
                                detail: String::from(
                                    "downgraded rented-cluster modes must require resume_from_last_stable_checkpoint",
                                ),
                            },
                        );
                    }
                }
                PsionRentedClusterInfraDisposition::Refused => {
                    refused = true;
                    if evaluation.required_recovery_mode.is_some() {
                        return Err(
                            PsionRentedClusterRunbookError::InfraModeEvaluationViolation {
                                infra_mode: format!("{:?}", evaluation.infra_mode),
                                detail: String::from(
                                    "refused rented-cluster modes may not carry a recovery mode",
                                ),
                            },
                        );
                    }
                }
            }
        }
        if !supported || !downgraded || !refused {
            return Err(PsionRentedClusterRunbookError::IncompleteInfraCoverage {
                supported,
                downgraded,
                refused,
            });
        }
        Ok(())
    }
}

/// Concrete failure rehearsal bundle for rented-cluster execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRentedClusterFailureRehearsalBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable runbook identifier.
    pub runbook_id: String,
    /// Stable digest of the bound checkpoint-recovery bundle.
    pub checkpoint_recovery_bundle_digest: String,
    /// Recovery event ids cited by the runbook for resume and rollback semantics.
    pub recovery_event_receipt_ids: Vec<String>,
    /// Recovery artifact id that must remain restorable on rented infrastructure.
    pub checkpoint_recovery_artifact_id: String,
    /// Storage-controller artifact id used for archive and restore receipts.
    pub checkpoint_storage_artifact_id: String,
    /// Admission receipt showing explicit preemption handling.
    pub preemption_admission_receipt: TrainAdmissionReceipt,
    /// Completion receipt used to evaluate cost guardrails.
    pub trainer_completion_receipt: TrainCompletionReceipt,
    /// Storage sweep receipts for the checkpoint artifact.
    pub checkpoint_storage_sweep_receipts: Vec<ArtifactStorageSweepReceipt>,
    /// Requested and completed cold-restore receipts for the checkpoint artifact.
    pub checkpoint_cold_restore_receipts: Vec<ArtifactColdRestoreReceipt>,
    /// Stop conditions fired by the rehearsal.
    pub stop_conditions: Vec<PsionRentedClusterStopConditionReceipt>,
    /// Short bundle summary.
    pub summary: String,
    /// Stable digest over the rehearsal bundle.
    pub bundle_digest: String,
}

impl PsionRentedClusterFailureRehearsalBundle {
    /// Validates the rehearsal bundle against the runbook and recovery bundle.
    pub fn validate_against_runbook(
        &self,
        runbook: &PsionRentedClusterRunbook,
        recovery_bundle: &PsionCheckpointRecoveryBundle,
    ) -> Result<(), PsionRentedClusterRunbookError> {
        runbook.validate_against_recovery_bundle(recovery_bundle)?;
        ensure_nonempty(
            self.schema_version.as_str(),
            "rented_cluster_failure_bundle.schema_version",
        )?;
        if self.schema_version != PSION_RENTED_CLUSTER_FAILURE_REHEARSAL_BUNDLE_SCHEMA_VERSION {
            return Err(PsionRentedClusterRunbookError::SchemaVersionMismatch {
                expected: String::from(
                    PSION_RENTED_CLUSTER_FAILURE_REHEARSAL_BUNDLE_SCHEMA_VERSION,
                ),
                actual: self.schema_version.clone(),
            });
        }
        check_string_match(
            self.runbook_id.as_str(),
            runbook.runbook_id.as_str(),
            "rented_cluster_failure_bundle.runbook_id",
        )?;
        check_string_match(
            self.checkpoint_recovery_bundle_digest.as_str(),
            runbook.checkpoint_recovery_bundle_digest.as_str(),
            "rented_cluster_failure_bundle.checkpoint_recovery_bundle_digest",
        )?;
        ensure_nonempty(
            self.bundle_id.as_str(),
            "rented_cluster_failure_bundle.bundle_id",
        )?;
        ensure_nonempty(
            self.checkpoint_recovery_artifact_id.as_str(),
            "rented_cluster_failure_bundle.checkpoint_recovery_artifact_id",
        )?;
        ensure_nonempty(
            self.checkpoint_storage_artifact_id.as_str(),
            "rented_cluster_failure_bundle.checkpoint_storage_artifact_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "rented_cluster_failure_bundle.summary",
        )?;
        if self.recovery_event_receipt_ids.is_empty() {
            return Err(PsionRentedClusterRunbookError::MissingField {
                field: String::from("rented_cluster_failure_bundle.recovery_event_receipt_ids"),
            });
        }
        let recovery_artifact = recovery_bundle
            .checkpoint_artifacts
            .iter()
            .find(|artifact| artifact.artifact_id == self.checkpoint_recovery_artifact_id)
            .ok_or(PsionRentedClusterRunbookError::UnknownRecoveryArtifact {
                artifact_id: self.checkpoint_recovery_artifact_id.clone(),
            })?;
        if recovery_artifact.artifact_id != recovery_bundle.last_stable_artifact_id {
            return Err(
                PsionRentedClusterRunbookError::RecoveryArtifactMustBeLastStable {
                    artifact_id: recovery_artifact.artifact_id.clone(),
                    expected: recovery_bundle.last_stable_artifact_id.clone(),
                },
            );
        }

        let mut event_kinds = BTreeSet::new();
        for event_id in &self.recovery_event_receipt_ids {
            let event = recovery_bundle
                .recovery_events
                .iter()
                .find(|event| &event.receipt_id == event_id)
                .ok_or(PsionRentedClusterRunbookError::UnknownRecoveryEvent {
                    receipt_id: event_id.clone(),
                })?;
            event_kinds.insert(event.event_kind);
        }
        for required in [
            PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart,
            PsionCheckpointRecoveryEventKind::CorruptionDetectedRollback,
        ] {
            if !event_kinds.contains(&required) {
                return Err(
                    PsionRentedClusterRunbookError::MissingRecoveryEventForRunbook {
                        event_kind: format!("{required:?}"),
                    },
                );
            }
        }

        self.validate_preemption_rehearsal(runbook)?;
        self.validate_cost_guardrail_rehearsal(runbook)?;
        self.validate_storage_rehearsal()?;
        self.validate_stop_conditions(runbook)?;
        if self.bundle_digest != stable_rented_cluster_failure_bundle_digest(self) {
            return Err(PsionRentedClusterRunbookError::FailureBundleDigestMismatch);
        }
        Ok(())
    }

    fn validate_preemption_rehearsal(
        &self,
        runbook: &PsionRentedClusterRunbook,
    ) -> Result<(), PsionRentedClusterRunbookError> {
        if self.preemption_admission_receipt.preemptions.len()
            < usize::from(runbook.max_preemption_events_before_downgrade)
        {
            return Err(
                PsionRentedClusterRunbookError::PreemptionThresholdNotReached {
                    observed: self.preemption_admission_receipt.preemptions.len() as u16,
                    required: runbook.max_preemption_events_before_downgrade,
                },
            );
        }
        if self.preemption_admission_receipt.outcome
            != TrainAdmissionOutcome::AdmittedAfterPreemption
        {
            return Err(PsionRentedClusterRunbookError::PreemptionReceiptMissingOutcome);
        }
        if self
            .preemption_admission_receipt
            .receipt_digest
            .trim()
            .is_empty()
        {
            return Err(PsionRentedClusterRunbookError::MissingField {
                field: String::from("rented_cluster_failure_bundle.preemption_admission_receipt"),
            });
        }
        Ok(())
    }

    fn validate_cost_guardrail_rehearsal(
        &self,
        runbook: &PsionRentedClusterRunbook,
    ) -> Result<(), PsionRentedClusterRunbookError> {
        if self
            .trainer_completion_receipt
            .receipt_digest
            .trim()
            .is_empty()
        {
            return Err(PsionRentedClusterRunbookError::MissingField {
                field: String::from("rented_cluster_failure_bundle.trainer_completion_receipt"),
            });
        }
        if self.trainer_completion_receipt.estimated_cost_units == 0
            || self.trainer_completion_receipt.actual_cost_units
                <= self.trainer_completion_receipt.estimated_cost_units
        {
            return Err(PsionRentedClusterRunbookError::MissingCostOverrun);
        }
        let overrun_bps = cost_overrun_bps(&self.trainer_completion_receipt);
        if overrun_bps <= u64::from(runbook.max_cost_overrun_bps_before_stop) {
            return Err(PsionRentedClusterRunbookError::CostOverrunBelowThreshold {
                observed_bps: overrun_bps,
                threshold_bps: u64::from(runbook.max_cost_overrun_bps_before_stop),
            });
        }
        Ok(())
    }

    fn validate_storage_rehearsal(&self) -> Result<(), PsionRentedClusterRunbookError> {
        if self.checkpoint_storage_sweep_receipts.len() < 2 {
            return Err(PsionRentedClusterRunbookError::MissingField {
                field: String::from(
                    "rented_cluster_failure_bundle.checkpoint_storage_sweep_receipts",
                ),
            });
        }
        if self.checkpoint_cold_restore_receipts.len() != 2 {
            return Err(PsionRentedClusterRunbookError::MissingField {
                field: String::from(
                    "rented_cluster_failure_bundle.checkpoint_cold_restore_receipts",
                ),
            });
        }
        let mut saw_warm = false;
        let mut saw_archive = false;
        for receipt in &self.checkpoint_storage_sweep_receipts {
            if receipt.receipt_digest.trim().is_empty() {
                return Err(PsionRentedClusterRunbookError::MissingField {
                    field: String::from(
                        "rented_cluster_failure_bundle.checkpoint_storage_sweep_receipts[]",
                    ),
                });
            }
            for transition in &receipt.transitions {
                if transition.artifact_id != self.checkpoint_storage_artifact_id {
                    continue;
                }
                if transition.from_tier == crate::ArtifactStorageTier::Hot
                    && transition.to_tier == crate::ArtifactStorageTier::Warm
                {
                    saw_warm = true;
                }
                if transition.from_tier == crate::ArtifactStorageTier::Warm
                    && transition.to_tier == crate::ArtifactStorageTier::ColdArchive
                {
                    saw_archive = true;
                }
            }
        }
        if !saw_warm || !saw_archive {
            return Err(PsionRentedClusterRunbookError::StorageLifecycleIncomplete {
                saw_warm,
                saw_archive,
            });
        }
        if self.checkpoint_cold_restore_receipts[0].artifact_id
            != self.checkpoint_storage_artifact_id
            || self.checkpoint_cold_restore_receipts[1].artifact_id
                != self.checkpoint_storage_artifact_id
            || self.checkpoint_cold_restore_receipts[0].action
                != ArtifactColdRestoreAction::Requested
            || self.checkpoint_cold_restore_receipts[1].action
                != ArtifactColdRestoreAction::Completed
        {
            return Err(PsionRentedClusterRunbookError::ColdRestoreSequenceInvalid);
        }
        if self.checkpoint_cold_restore_receipts[0]
            .receipt_digest
            .trim()
            .is_empty()
            || self.checkpoint_cold_restore_receipts[1]
                .receipt_digest
                .trim()
                .is_empty()
        {
            return Err(PsionRentedClusterRunbookError::MissingField {
                field: String::from(
                    "rented_cluster_failure_bundle.checkpoint_cold_restore_receipts[]",
                ),
            });
        }
        Ok(())
    }

    fn validate_stop_conditions(
        &self,
        runbook: &PsionRentedClusterRunbook,
    ) -> Result<(), PsionRentedClusterRunbookError> {
        if self.stop_conditions.is_empty() {
            return Err(PsionRentedClusterRunbookError::MissingField {
                field: String::from("rented_cluster_failure_bundle.stop_conditions"),
            });
        }
        let mut kinds = BTreeSet::new();
        for receipt in &self.stop_conditions {
            receipt.validate()?;
            kinds.insert(receipt.condition_kind);
            match receipt.condition_kind {
                PsionRentedClusterStopConditionKind::PreemptionBudgetExceeded => {
                    if receipt.action != PsionRentedClusterRunAction::DowngradeToResumeOnly
                        || receipt.source_ref != self.preemption_admission_receipt.workload_id
                        || receipt.threshold_value
                            != u64::from(runbook.max_preemption_events_before_downgrade)
                    {
                        return Err(PsionRentedClusterRunbookError::StopConditionMismatch {
                            receipt_id: receipt.receipt_id.clone(),
                            detail: String::from(
                                "preemption stop condition must downgrade to resume-only using the admission workload id and preemption threshold",
                            ),
                        });
                    }
                }
                PsionRentedClusterStopConditionKind::CostGuardrailExceeded => {
                    if receipt.action != PsionRentedClusterRunAction::StopRun
                        || receipt.source_ref != self.trainer_completion_receipt.workload_id
                        || receipt.threshold_value
                            != u64::from(runbook.max_cost_overrun_bps_before_stop)
                    {
                        return Err(PsionRentedClusterRunbookError::StopConditionMismatch {
                            receipt_id: receipt.receipt_id.clone(),
                            detail: String::from(
                                "cost stop condition must stop the run using the completion workload id and overrun threshold",
                            ),
                        });
                    }
                }
            }
        }
        for kind in PsionRentedClusterStopConditionKind::required_kinds() {
            if !kinds.contains(&kind) {
                return Err(PsionRentedClusterRunbookError::MissingStopCondition {
                    condition_kind: format!("{kind:?}"),
                });
            }
        }
        Ok(())
    }
}

/// Records one rented-cluster runbook and computes its stable digest.
pub fn record_psion_rented_cluster_runbook(
    runbook_id: impl Into<String>,
    storage_profiles: BTreeMap<TrainArtifactClass, ArtifactRetentionProfile>,
    scheduling_policy: TrainSchedulingAccountingPolicy,
    max_preemption_events_before_downgrade: u16,
    max_cost_overrun_bps_before_stop: u16,
    mode_evaluations: Vec<PsionRentedClusterModeEvaluation>,
    summary: impl Into<String>,
    recovery_bundle: &PsionCheckpointRecoveryBundle,
) -> Result<PsionRentedClusterRunbook, PsionRentedClusterRunbookError> {
    let mut runbook = PsionRentedClusterRunbook {
        schema_version: String::from(PSION_RENTED_CLUSTER_RUNBOOK_SCHEMA_VERSION),
        runbook_id: runbook_id.into(),
        checkpoint_recovery_bundle_digest: recovery_bundle.bundle_digest.clone(),
        storage_profiles,
        scheduling_policy,
        max_preemption_events_before_downgrade,
        max_cost_overrun_bps_before_stop,
        mode_evaluations,
        summary: summary.into(),
        runbook_digest: String::new(),
    };
    runbook.runbook_digest = stable_rented_cluster_runbook_digest(&runbook);
    runbook.validate_against_recovery_bundle(recovery_bundle)?;
    Ok(runbook)
}

/// Records one stop-condition receipt and computes its stable digest.
pub fn record_psion_rented_cluster_stop_condition(
    receipt_id: impl Into<String>,
    condition_kind: PsionRentedClusterStopConditionKind,
    source_ref: impl Into<String>,
    observed_value: u64,
    threshold_value: u64,
    action: PsionRentedClusterRunAction,
    detail: impl Into<String>,
) -> Result<PsionRentedClusterStopConditionReceipt, PsionRentedClusterRunbookError> {
    let mut receipt = PsionRentedClusterStopConditionReceipt {
        schema_version: String::from(PSION_RENTED_CLUSTER_STOP_CONDITION_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        condition_kind,
        source_ref: source_ref.into(),
        observed_value,
        threshold_value,
        action,
        detail: detail.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_stop_condition_digest(&receipt);
    receipt.validate()?;
    Ok(receipt)
}

/// Records one rented-cluster failure rehearsal bundle and computes its stable digest.
pub fn record_psion_rented_cluster_failure_rehearsal_bundle(
    bundle_id: impl Into<String>,
    recovery_event_receipt_ids: Vec<String>,
    checkpoint_recovery_artifact_id: impl Into<String>,
    checkpoint_storage_artifact_id: impl Into<String>,
    preemption_admission_receipt: TrainAdmissionReceipt,
    trainer_completion_receipt: TrainCompletionReceipt,
    checkpoint_storage_sweep_receipts: Vec<ArtifactStorageSweepReceipt>,
    checkpoint_cold_restore_receipts: Vec<ArtifactColdRestoreReceipt>,
    stop_conditions: Vec<PsionRentedClusterStopConditionReceipt>,
    summary: impl Into<String>,
    runbook: &PsionRentedClusterRunbook,
    recovery_bundle: &PsionCheckpointRecoveryBundle,
) -> Result<PsionRentedClusterFailureRehearsalBundle, PsionRentedClusterRunbookError> {
    let mut bundle = PsionRentedClusterFailureRehearsalBundle {
        schema_version: String::from(PSION_RENTED_CLUSTER_FAILURE_REHEARSAL_BUNDLE_SCHEMA_VERSION),
        bundle_id: bundle_id.into(),
        runbook_id: runbook.runbook_id.clone(),
        checkpoint_recovery_bundle_digest: recovery_bundle.bundle_digest.clone(),
        recovery_event_receipt_ids,
        checkpoint_recovery_artifact_id: checkpoint_recovery_artifact_id.into(),
        checkpoint_storage_artifact_id: checkpoint_storage_artifact_id.into(),
        preemption_admission_receipt,
        trainer_completion_receipt,
        checkpoint_storage_sweep_receipts,
        checkpoint_cold_restore_receipts,
        stop_conditions,
        summary: summary.into(),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_rented_cluster_failure_bundle_digest(&bundle);
    bundle.validate_against_runbook(runbook, recovery_bundle)?;
    Ok(bundle)
}

/// Error for the rented-cluster runbook and rehearsal bundle.
#[derive(Debug, Error)]
pub enum PsionRentedClusterRunbookError {
    /// Wrapped checkpoint-recovery validation error.
    #[error(transparent)]
    CheckpointRecovery(#[from] PsionCheckpointRecoveryContractError),
    /// Wrapped artifact-storage validation error.
    #[error(transparent)]
    ArtifactStorage(#[from] TrainArtifactStorageError),
    /// Wrapped scheduling/accounting validation error.
    #[error(transparent)]
    Scheduling(#[from] TrainSchedulingAccountingError),
    /// Missing field.
    #[error("psion rented-cluster contract is missing `{field}`")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// Schema mismatch.
    #[error("psion rented-cluster schema mismatch: expected `{expected}`, got `{actual}`")]
    SchemaVersionMismatch {
        /// Expected version.
        expected: String,
        /// Actual version.
        actual: String,
    },
    /// Field mismatch.
    #[error(
        "psion rented-cluster field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        /// Field path.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// Missing storage profile.
    #[error("psion rented-cluster runbook is missing storage profile for `{artifact_class:?}`")]
    MissingStorageProfile {
        /// Artifact class.
        artifact_class: TrainArtifactClass,
    },
    /// Checkpoint profile cannot be ephemeral.
    #[error("psion rented-cluster checkpoint profile cannot be ephemeral")]
    CheckpointProfileEphemeral,
    /// Log profile must remain ephemeral.
    #[error("psion rented-cluster log profile must remain ephemeral")]
    LogProfileMustStayEphemeral,
    /// Invalid preemption threshold.
    #[error("psion rented-cluster preemption threshold must be non-zero")]
    InvalidPreemptionThreshold,
    /// Duplicate infra-mode evaluation.
    #[error("psion rented-cluster runbook repeats infra mode `{infra_mode}`")]
    DuplicateInfraModeEvaluation {
        /// Infra mode label.
        infra_mode: String,
    },
    /// Infra evaluation violated the runbook contract.
    #[error(
        "psion rented-cluster infra mode `{infra_mode}` violates the runbook contract: {detail}"
    )]
    InfraModeEvaluationViolation {
        /// Infra mode label.
        infra_mode: String,
        /// Detail.
        detail: String,
    },
    /// Infra coverage is incomplete.
    #[error("psion rented-cluster runbook must include supported, downgraded, and refused modes")]
    IncompleteInfraCoverage {
        /// Whether supported coverage exists.
        supported: bool,
        /// Whether downgraded coverage exists.
        downgraded: bool,
        /// Whether refused coverage exists.
        refused: bool,
    },
    /// Runbook digest mismatch.
    #[error("psion rented-cluster runbook digest did not recompute")]
    RunbookDigestMismatch,
    /// Stop condition failed to breach its threshold.
    #[error("psion rented-cluster stop condition `{receipt_id}` did not breach the threshold: observed `{observed_value}`, threshold `{threshold_value}`")]
    StopConditionNotTriggered {
        /// Receipt id.
        receipt_id: String,
        /// Observed value.
        observed_value: u64,
        /// Threshold.
        threshold_value: u64,
    },
    /// Stop-condition digest mismatch.
    #[error("psion rented-cluster stop condition `{receipt_id}` digest did not recompute")]
    StopConditionDigestMismatch {
        /// Receipt id.
        receipt_id: String,
    },
    /// Unknown recovery artifact id.
    #[error("psion rented-cluster bundle references unknown recovery artifact `{artifact_id}`")]
    UnknownRecoveryArtifact {
        /// Artifact id.
        artifact_id: String,
    },
    /// Recovery artifact must match the last stable recovery target.
    #[error("psion rented-cluster bundle requires last stable recovery artifact `{expected}` but got `{artifact_id}`")]
    RecoveryArtifactMustBeLastStable {
        /// Actual artifact id.
        artifact_id: String,
        /// Expected artifact id.
        expected: String,
    },
    /// Unknown recovery event id.
    #[error("psion rented-cluster bundle references unknown recovery event `{receipt_id}`")]
    UnknownRecoveryEvent {
        /// Event receipt id.
        receipt_id: String,
    },
    /// Missing required recovery event.
    #[error("psion rented-cluster bundle is missing required recovery event `{event_kind}`")]
    MissingRecoveryEventForRunbook {
        /// Event kind label.
        event_kind: String,
    },
    /// Preemption threshold was not exercised.
    #[error("psion rented-cluster preemption rehearsal observed `{observed}` preemptions but requires at least `{required}`")]
    PreemptionThresholdNotReached {
        /// Observed count.
        observed: u16,
        /// Required count.
        required: u16,
    },
    /// Preemption admission outcome was wrong.
    #[error("psion rented-cluster preemption rehearsal must admit after preemption")]
    PreemptionReceiptMissingOutcome,
    /// Cost overrun was missing.
    #[error("psion rented-cluster rehearsal must preserve an actual cost overrun")]
    MissingCostOverrun,
    /// Cost overrun did not breach the threshold.
    #[error("psion rented-cluster cost overrun `{observed_bps}` bps did not exceed the stop threshold `{threshold_bps}` bps")]
    CostOverrunBelowThreshold {
        /// Observed overrun.
        observed_bps: u64,
        /// Threshold.
        threshold_bps: u64,
    },
    /// Storage lifecycle rehearsal did not progress through warm and archive states.
    #[error(
        "psion rented-cluster storage lifecycle incomplete: warm={saw_warm}, archive={saw_archive}"
    )]
    StorageLifecycleIncomplete {
        /// Whether the artifact moved to warm.
        saw_warm: bool,
        /// Whether the artifact moved to cold archive.
        saw_archive: bool,
    },
    /// Cold restore sequence was invalid.
    #[error("psion rented-cluster cold restore sequence must request and then complete restore for the checkpoint artifact")]
    ColdRestoreSequenceInvalid,
    /// Stop condition mismatch.
    #[error(
        "psion rented-cluster stop condition `{receipt_id}` does not match the runbook: {detail}"
    )]
    StopConditionMismatch {
        /// Receipt id.
        receipt_id: String,
        /// Detail.
        detail: String,
    },
    /// Missing required stop condition.
    #[error("psion rented-cluster bundle is missing stop condition `{condition_kind}`")]
    MissingStopCondition {
        /// Condition kind label.
        condition_kind: String,
    },
    /// Failure bundle digest mismatch.
    #[error("psion rented-cluster failure rehearsal bundle digest did not recompute")]
    FailureBundleDigestMismatch,
}

fn stable_rented_cluster_runbook_digest(runbook: &PsionRentedClusterRunbook) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_rented_cluster_runbook|");
    hasher.update(runbook.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(runbook.runbook_id.as_bytes());
    hasher.update(b"|");
    hasher.update(runbook.checkpoint_recovery_bundle_digest.as_bytes());
    hasher.update(b"|storage|");
    hasher.update(stable_json_bytes(&runbook.storage_profiles));
    hasher.update(b"|scheduling|");
    hasher.update(stable_json_bytes(&runbook.scheduling_policy));
    hasher.update(b"|");
    hasher.update(
        runbook
            .max_preemption_events_before_downgrade
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        runbook
            .max_cost_overrun_bps_before_stop
            .to_string()
            .as_bytes(),
    );
    for evaluation in &runbook.mode_evaluations {
        hasher.update(b"|mode|");
        hasher.update(stable_json_bytes(evaluation));
    }
    hasher.update(b"|");
    hasher.update(runbook.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_stop_condition_digest(receipt: &PsionRentedClusterStopConditionReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_rented_cluster_stop_condition|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", receipt.condition_kind).as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.source_ref.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.observed_value.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.threshold_value.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", receipt.action).as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.detail.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_rented_cluster_failure_bundle_digest(
    bundle: &PsionRentedClusterFailureRehearsalBundle,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_rented_cluster_failure_bundle|");
    hasher.update(bundle.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.bundle_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.runbook_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.checkpoint_recovery_bundle_digest.as_bytes());
    for event_id in &bundle.recovery_event_receipt_ids {
        hasher.update(b"|recovery_event|");
        hasher.update(event_id.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(bundle.checkpoint_recovery_artifact_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.checkpoint_storage_artifact_id.as_bytes());
    hasher.update(b"|preemption|");
    hasher.update(stable_json_bytes(&bundle.preemption_admission_receipt));
    hasher.update(b"|completion|");
    hasher.update(stable_json_bytes(&bundle.trainer_completion_receipt));
    for receipt in &bundle.checkpoint_storage_sweep_receipts {
        hasher.update(b"|sweep|");
        hasher.update(stable_json_bytes(receipt));
    }
    for receipt in &bundle.checkpoint_cold_restore_receipts {
        hasher.update(b"|restore|");
        hasher.update(stable_json_bytes(receipt));
    }
    for receipt in &bundle.stop_conditions {
        hasher.update(b"|stop|");
        hasher.update(receipt.receipt_digest.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(bundle.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("serializing stable rented-cluster JSON should succeed")
}

fn cost_overrun_bps(receipt: &TrainCompletionReceipt) -> u64 {
    let estimated = receipt.estimated_cost_units.max(1);
    if receipt.actual_cost_units <= estimated {
        return 0;
    }
    receipt
        .actual_cost_units
        .saturating_sub(estimated)
        .saturating_mul(10_000)
        / estimated
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionRentedClusterRunbookError> {
    if value.trim().is_empty() {
        return Err(PsionRentedClusterRunbookError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionRentedClusterRunbookError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionRentedClusterRunbookError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn check_bps(value: u16, field: &str) -> Result<(), PsionRentedClusterRunbookError> {
    if value == 0 || value > 10_000 {
        return Err(PsionRentedClusterRunbookError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use psionic_environments::EnvironmentPackageKey;
    use psionic_eval::EvalArtifact;
    use psionic_runtime::RuntimeDispatchPolicy;
    use psionic_sandbox::{
        ProviderSandboxEntrypointType, ProviderSandboxEnvironmentVar,
        ProviderSandboxExecutionClass, ProviderSandboxJobRequest, ProviderSandboxResourceRequest,
    };

    use super::*;
    use crate::{
        record_psion_rented_cluster_failure_rehearsal_bundle, record_psion_rented_cluster_runbook,
        record_psion_rented_cluster_stop_condition, ArtifactStorageTier, PolicyRevision,
        RolloutArtifact, RolloutSample, RolloutTerminationReason, TrainArtifactStorageController,
        TrainBudgetCap, TrainPreemptionMode, TrainQueueClass, TrainQueuePolicy,
        TrainScheduledWorkload,
    };

    fn load_fixture<T: for<'de> Deserialize<'de>>(relative: &str) -> T {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(relative);
        serde_json::from_str(
            &fs::read_to_string(path).expect("fixture should be present after generator run"),
        )
        .expect("fixture should parse")
    }

    fn recovery_bundle() -> PsionCheckpointRecoveryBundle {
        load_fixture("fixtures/psion/checkpoint_recovery/psion_checkpoint_recovery_bundle_v1.json")
    }

    fn runbook() -> PsionRentedClusterRunbook {
        load_fixture("fixtures/psion/rented_cluster/psion_rented_cluster_runbook_v1.json")
    }

    fn rehearsal_bundle() -> PsionRentedClusterFailureRehearsalBundle {
        load_fixture(
            "fixtures/psion/rented_cluster/psion_rented_cluster_failure_rehearsal_bundle_v1.json",
        )
    }

    #[test]
    fn rented_cluster_runbook_and_rehearsal_bundle_validate(
    ) -> Result<(), Box<dyn std::error::Error>> {
        runbook().validate_against_recovery_bundle(&recovery_bundle())?;
        rehearsal_bundle().validate_against_runbook(&runbook(), &recovery_bundle())?;
        Ok(())
    }

    #[test]
    fn runbook_rejects_ephemeral_checkpoint_profile() {
        let mut runbook = runbook();
        runbook.storage_profiles.insert(
            TrainArtifactClass::Checkpoint,
            ArtifactRetentionProfile::new(5_000, 30_000, ArtifactArchiveClass::Ephemeral, 45_000),
        );
        runbook.runbook_digest = stable_rented_cluster_runbook_digest(&runbook);
        let error = runbook
            .validate_against_recovery_bundle(&recovery_bundle())
            .expect_err("checkpoint profile should not stay ephemeral");
        assert!(matches!(
            error,
            PsionRentedClusterRunbookError::CheckpointProfileEphemeral
        ));
    }

    #[test]
    fn runbook_requires_downgrade_and_refusal_modes() {
        let mut runbook = runbook();
        runbook.mode_evaluations.retain(|evaluation| {
            evaluation.disposition != PsionRentedClusterInfraDisposition::Refused
        });
        runbook.runbook_digest = stable_rented_cluster_runbook_digest(&runbook);
        let error = runbook
            .validate_against_recovery_bundle(&recovery_bundle())
            .expect_err("runbook should require refused modes");
        assert!(matches!(
            error,
            PsionRentedClusterRunbookError::IncompleteInfraCoverage { .. }
        ));
    }

    #[test]
    fn rehearsal_bundle_requires_cost_guardrail_stop_condition() {
        let mut bundle = rehearsal_bundle();
        bundle.stop_conditions.retain(|receipt| {
            receipt.condition_kind != PsionRentedClusterStopConditionKind::CostGuardrailExceeded
        });
        bundle.bundle_digest = stable_rented_cluster_failure_bundle_digest(&bundle);
        let error = bundle
            .validate_against_runbook(&runbook(), &recovery_bundle())
            .expect_err("bundle should require a cost stop condition");
        assert!(matches!(
            error,
            PsionRentedClusterRunbookError::MissingStopCondition { .. }
        ));
    }

    #[test]
    fn rented_cluster_builder_recomputes_fixture_digest() -> Result<(), Box<dyn std::error::Error>>
    {
        let recovery_bundle = recovery_bundle();
        let built_runbook = build_runbook(&recovery_bundle)?;
        let bundle = build_rehearsal_bundle(&built_runbook, &recovery_bundle)?;
        assert_eq!(built_runbook.runbook_digest, runbook().runbook_digest);
        assert_eq!(bundle.bundle_digest, rehearsal_bundle().bundle_digest);
        Ok(())
    }

    fn build_runbook(
        recovery_bundle: &PsionCheckpointRecoveryBundle,
    ) -> Result<PsionRentedClusterRunbook, Box<dyn std::error::Error>> {
        Ok(record_psion_rented_cluster_runbook(
            "psion-rented-cluster-runbook-v1",
            storage_profiles(),
            scheduling_policy()?,
            1,
            2_500,
            mode_evaluations(),
            "Rented-cluster runbook freezes storage persistence, preemption downgrade, cost stop conditions, and explicit refusal on unsupported infra modes before broader cluster work.",
            recovery_bundle,
        )?)
    }

    fn build_rehearsal_bundle(
        runbook: &PsionRentedClusterRunbook,
        recovery_bundle: &PsionCheckpointRecoveryBundle,
    ) -> Result<PsionRentedClusterFailureRehearsalBundle, Box<dyn std::error::Error>> {
        let (preemption_admission_receipt, trainer_completion_receipt) =
            scheduling_rehearsal_receipts()?;
        let (checkpoint_storage_artifact_id, sweep_receipts, cold_restore_receipts) =
            storage_rehearsal_receipts(recovery_bundle)?;
        let preemption_stop = record_psion_rented_cluster_stop_condition(
            "psion-rented-cluster-preemption-stop-v1",
            PsionRentedClusterStopConditionKind::PreemptionBudgetExceeded,
            preemption_admission_receipt.workload_id.clone(),
            preemption_admission_receipt.preemptions.len() as u64,
            u64::from(runbook.max_preemption_events_before_downgrade),
            PsionRentedClusterRunAction::DowngradeToResumeOnly,
            "Repeated preemption on rented spot capacity downgrades the run to resume-only posture.",
        )?;
        let cost_stop = record_psion_rented_cluster_stop_condition(
            "psion-rented-cluster-cost-stop-v1",
            PsionRentedClusterStopConditionKind::CostGuardrailExceeded,
            trainer_completion_receipt.workload_id.clone(),
            cost_overrun_bps(&trainer_completion_receipt),
            u64::from(runbook.max_cost_overrun_bps_before_stop),
            PsionRentedClusterRunAction::StopRun,
            "Rented-cluster cost overrun breached the stop threshold and terminated the run instead of hiding the spend drift.",
        )?;
        let recovery_event_receipt_ids = recovery_bundle
            .recovery_events
            .iter()
            .filter(|event| {
                matches!(
                    event.event_kind,
                    PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart
                        | PsionCheckpointRecoveryEventKind::CorruptionDetectedRollback
                )
            })
            .map(|event| event.receipt_id.clone())
            .collect::<Vec<_>>();
        Ok(record_psion_rented_cluster_failure_rehearsal_bundle(
            "psion-rented-cluster-failure-rehearsal-bundle-v1",
            recovery_event_receipt_ids,
            recovery_bundle.last_stable_artifact_id.clone(),
            checkpoint_storage_artifact_id,
            preemption_admission_receipt,
            trainer_completion_receipt,
            sweep_receipts,
            cold_restore_receipts,
            vec![preemption_stop, cost_stop],
            "Rented-cluster rehearsal bundle binds preemption downgrade, cost stop, checkpoint archive, cold restore, and cited recovery receipts into one bounded failure policy artifact.",
            runbook,
            recovery_bundle,
        )?)
    }

    fn storage_profiles() -> BTreeMap<TrainArtifactClass, ArtifactRetentionProfile> {
        BTreeMap::from([
            (
                TrainArtifactClass::Checkpoint,
                ArtifactRetentionProfile::new(
                    5_000,
                    30_000,
                    ArtifactArchiveClass::Restorable,
                    45_000,
                )
                .with_delete_after_ms(Some(172_800_000)),
            ),
            (
                TrainArtifactClass::EvalArtifact,
                ArtifactRetentionProfile::new(
                    10_000,
                    60_000,
                    ArtifactArchiveClass::Restorable,
                    60_000,
                ),
            ),
            (
                TrainArtifactClass::LogBundle,
                ArtifactRetentionProfile::new(
                    5_000,
                    15_000,
                    ArtifactArchiveClass::Ephemeral,
                    10_000,
                )
                .with_delete_after_ms(Some(3_600_000)),
            ),
        ])
    }

    fn mode_evaluations() -> Vec<PsionRentedClusterModeEvaluation> {
        vec![
            PsionRentedClusterModeEvaluation {
                infra_mode: PsionRentedClusterInfraMode::SingleRegionOnDemand,
                topology_label: String::from("single_region_homogeneous"),
                disposition: PsionRentedClusterInfraDisposition::Supported,
                required_recovery_mode: None,
                detail: String::from(
                    "Single-region on-demand rented clusters stay within the bounded runbook when checkpoint persistence and cold restore are green.",
                ),
            },
            PsionRentedClusterModeEvaluation {
                infra_mode: PsionRentedClusterInfraMode::SingleRegionSpot,
                topology_label: String::from("single_region_spot_preemptible"),
                disposition: PsionRentedClusterInfraDisposition::DowngradedResumeOnly,
                required_recovery_mode: Some(
                    TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
                ),
                detail: String::from(
                    "Single-region spot capacity is allowed only on explicit resume-from-last-stable-checkpoint posture.",
                ),
            },
            PsionRentedClusterModeEvaluation {
                infra_mode: PsionRentedClusterInfraMode::CrossRegionEphemeral,
                topology_label: String::from("cross_region_mixed_latency"),
                disposition: PsionRentedClusterInfraDisposition::Refused,
                required_recovery_mode: None,
                detail: String::from(
                    "Cross-region ephemeral clusters are refused because mixed-latency recovery and storage-loss risk are outside the bounded runbook.",
                ),
            },
            PsionRentedClusterModeEvaluation {
                infra_mode: PsionRentedClusterInfraMode::UntrustedSharedCluster,
                topology_label: String::from("shared_untrusted_fabric"),
                disposition: PsionRentedClusterInfraDisposition::Refused,
                required_recovery_mode: None,
                detail: String::from(
                    "Untrusted shared clusters are refused rather than backfilling trusted-cluster claims onto rented infrastructure.",
                ),
            },
        ]
    }

    fn scheduling_policy() -> Result<TrainSchedulingAccountingPolicy, TrainSchedulingAccountingError>
    {
        let mut policy = TrainSchedulingAccountingPolicy::default();
        policy.global_budget = TrainBudgetCap::new(8, 256 * 1024 * 1024 + 2_000, 400_000);
        policy.queue_policies.insert(
            TrainQueueClass::Realtime,
            TrainQueuePolicy::new(
                9_500,
                TrainPreemptionMode::LowerPriorityOnly,
                RuntimeDispatchPolicy::quantized_decode_default(2),
                TrainQueueClass::Realtime,
            )?,
        );
        policy.queue_policies.insert(
            TrainQueueClass::Background,
            TrainQueuePolicy::new(
                1_000,
                TrainPreemptionMode::Never,
                RuntimeDispatchPolicy {
                    max_workers: 1,
                    target_batch_work_units: 1,
                    max_batch_bytes: 64 * 1024 * 1024,
                    park_after_idle_batches: 8,
                },
                TrainQueueClass::Background,
            )?,
        );
        Ok(policy)
    }

    fn scheduling_rehearsal_receipts(
    ) -> Result<(TrainAdmissionReceipt, TrainCompletionReceipt), Box<dyn std::error::Error>> {
        let environment = EnvironmentPackageKey::new("psion.rented.cluster", "1.0.0");

        let mut preemption_controller =
            TrainSchedulingAccountingController::new(scheduling_policy()?)?;
        let sandbox = TrainScheduledWorkload::for_sandbox_job(
            &sandbox_job_request("rented-sbx-1", 256),
            &environment,
            TrainQueueClass::Background,
            None,
            1_000,
        );
        preemption_controller.admit(sandbox)?;
        let validator = TrainScheduledWorkload::for_validator_artifact(
            &EvalArtifact::new("rented_health", "eval://psion/rented/health", b"health"),
            &environment,
            TrainQueueClass::Realtime,
            "validator.psion.rented",
            1_100,
        );
        let preemption_receipt = preemption_controller.admit(validator)?;

        let mut trainer_controller =
            TrainSchedulingAccountingController::new(scheduling_policy()?)?;
        let trainer_workload = TrainScheduledWorkload::for_trainer_batch(
            &trainer_batch("psion-rented-batch", &environment)?,
            &environment,
            TrainQueueClass::Standard,
            2_000,
        );
        let trainer_admission = trainer_controller.admit(trainer_workload)?;
        let trainer_completion = trainer_controller.complete_workload(
            trainer_admission.workload_id.as_str(),
            Some(trainer_admission.estimated_cost_units.saturating_mul(4)),
            2_500,
        )?;
        Ok((preemption_receipt, trainer_completion))
    }

    fn storage_rehearsal_receipts(
        recovery_bundle: &PsionCheckpointRecoveryBundle,
    ) -> Result<
        (
            String,
            Vec<ArtifactStorageSweepReceipt>,
            Vec<ArtifactColdRestoreReceipt>,
        ),
        Box<dyn std::error::Error>,
    > {
        let mut controller = TrainArtifactStorageController::new(storage_profiles())?;
        let recovery_artifact = recovery_bundle
            .checkpoint_artifacts
            .iter()
            .find(|artifact| artifact.artifact_id == recovery_bundle.last_stable_artifact_id)
            .expect("recovery bundle should carry the last stable artifact");
        let checkpoint_storage_artifact_id = controller.register_checkpoint(
            recovery_artifact.checkpoint_manifest.shards[0]
                .manifest
                .clone(),
            recovery_artifact.checkpoint_manifest.checkpoint.clone(),
            recovery_artifact.checkpoint_manifest.shards[0]
                .manifest
                .total_bytes,
            0,
        )?;
        let warm = controller.sweep(6_000)?;
        let archived = controller.sweep(40_000)?;
        let requested =
            controller.request_cold_restore(checkpoint_storage_artifact_id.as_str(), 42_000)?;
        let completed =
            controller.complete_cold_restore(checkpoint_storage_artifact_id.as_str(), 60_000)?;
        assert_eq!(
            controller
                .artifact(checkpoint_storage_artifact_id.as_str())
                .expect("checkpoint artifact should remain tracked")
                .tier,
            ArtifactStorageTier::Warm
        );
        Ok((
            checkpoint_storage_artifact_id,
            vec![warm, archived],
            vec![requested, completed],
        ))
    }

    fn trainer_batch(
        batch_id: &str,
        environment: &EnvironmentPackageKey,
    ) -> Result<crate::TrainerBatch, Box<dyn std::error::Error>> {
        Ok(crate::TrainerBatch::assemble(
            batch_id,
            PolicyRevision::new(
                "psion.rented.policy",
                "rev-target",
                "policy-target-digest",
                3_000,
            )
            .with_revision_number(2),
            vec![sample_rollout(
                "rollout-rented",
                "worker-rented",
                environment,
            )],
            4_000,
        )?)
    }

    fn sample_rollout(
        artifact_id: &str,
        worker_id: &str,
        environment: &EnvironmentPackageKey,
    ) -> RolloutArtifact {
        RolloutArtifact::new(
            artifact_id,
            worker_id,
            environment.clone(),
            "task-rented",
            PolicyRevision::new("psion.rented.policy", "rev-1", "policy-digest", 1_000)
                .with_revision_number(1),
            vec![RolloutSample::new(1, -0.2, 0.8, 0.6)],
            RolloutTerminationReason::Completed,
            Vec::new(),
            2_000,
        )
        .expect("sample rollout should assemble")
    }

    fn sandbox_job_request(job_id: &str, memory_limit_mb: u64) -> ProviderSandboxJobRequest {
        ProviderSandboxJobRequest {
            job_id: String::from(job_id),
            provider_id: String::from("provider-rented"),
            compute_product_id: String::from("sandbox.python.exec"),
            execution_class: ProviderSandboxExecutionClass::PythonExec,
            entrypoint_type: ProviderSandboxEntrypointType::InlinePayload,
            entrypoint: String::from("print('psion-rented')"),
            payload: None,
            arguments: Vec::new(),
            workspace_root: PathBuf::from("."),
            expected_outputs: vec![String::from("receipt.json")],
            timeout_request_s: 30,
            network_request: String::from("none"),
            filesystem_request: String::from("workspace_rw"),
            environment: vec![ProviderSandboxEnvironmentVar {
                key: String::from("MODE"),
                value: String::from("rented"),
            }],
            resource_request: ProviderSandboxResourceRequest {
                cpu_limit: Some(2),
                memory_limit_mb: Some(memory_limit_mb),
                disk_limit_mb: Some(512),
            },
            payout_reference: None,
            verification_posture: Some(String::from("local")),
        }
    }
}
