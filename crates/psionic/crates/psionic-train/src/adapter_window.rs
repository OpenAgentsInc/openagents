use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{CheckpointPointer, PolicyRevision, TrainingWindowStatus};

/// Contract error for decentralized adapter-window receipts.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AdapterWindowContractError {
    /// One required contract field was empty.
    #[error("adapter-window contract is missing required field `{field}`")]
    MissingField {
        /// Field name that failed validation.
        field: &'static str,
    },
    /// One contribution id did not exist in the current window.
    #[error("adapter-window contribution `{contribution_id}` is unknown")]
    UnknownContribution {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// The window is in the wrong lifecycle state for the requested action.
    #[error("adapter-window `{window_id}` cannot {action} while status is `{status}`")]
    InvalidWindowStatus {
        /// Stable window identifier.
        window_id: String,
        /// Requested action.
        action: &'static str,
        /// Current lifecycle state.
        status: String,
    },
    /// Execution was already recorded for the contribution.
    #[error("adapter-window contribution `{contribution_id}` already has an execution receipt")]
    ExecutionAlreadyRecorded {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// Upload requires an execution receipt first.
    #[error("adapter-window contribution `{contribution_id}` needs execution before upload")]
    ExecutionRequired {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// Upload was already recorded for the contribution.
    #[error("adapter-window contribution `{contribution_id}` already has an upload receipt")]
    UploadAlreadyRecorded {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// Validation requires an upload receipt first.
    #[error("adapter-window contribution `{contribution_id}` needs upload before validation")]
    UploadRequired {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// Validation was already recorded for the contribution.
    #[error("adapter-window contribution `{contribution_id}` already has a validator receipt")]
    ValidationAlreadyRecorded {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// Aggregation eligibility requires a validator receipt first.
    #[error(
        "adapter-window contribution `{contribution_id}` needs validator disposition before aggregation eligibility"
    )]
    ValidationRequired {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// Aggregation eligibility was already recorded for the contribution.
    #[error(
        "adapter-window contribution `{contribution_id}` already has an aggregation-eligibility receipt"
    )]
    AggregationEligibilityAlreadyRecorded {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// One summary timestamp was inverted.
    #[error("adapter-window execution completed at {completed_at_ms} before start {started_at_ms}")]
    InvalidExecutionTiming {
        /// Execution start time.
        started_at_ms: u64,
        /// Execution completion time.
        completed_at_ms: u64,
    },
    /// The window cannot seal while receipts are still missing.
    #[error(
        "adapter-window `{window_id}` cannot seal while contributions are incomplete: {pending_contribution_ids:?}"
    )]
    SealIncomplete {
        /// Stable window identifier.
        window_id: String,
        /// Contributions still missing validator or aggregation posture.
        pending_contribution_ids: Vec<String>,
    },
    /// Promotion was requested without one eligible contribution.
    #[error(
        "adapter-window `{window_id}` cannot promote a policy revision without one eligible accepted contribution"
    )]
    PromotionWithoutEligibleContributions {
        /// Stable window identifier.
        window_id: String,
    },
    /// A promoted policy revision must also carry a promoted checkpoint pointer.
    #[error(
        "adapter-window `{window_id}` promotion requires an output checkpoint pointer when an output policy revision is present"
    )]
    MissingPromotionCheckpoint {
        /// Stable window identifier.
        window_id: String,
    },
}

/// Stable adapter target identity for one decentralized adapter-training family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterTargetIdentity {
    /// Stable adapter target identifier.
    pub adapter_target_id: String,
    /// Stable adapter family such as `apple.foundation_models`.
    pub adapter_family: String,
    /// Stable base-model or runtime reference.
    pub base_model_ref: String,
    /// Stable adapter package format such as `apple.fmadapter`.
    pub adapter_format: String,
}

impl AdapterTargetIdentity {
    /// Creates an adapter target identity.
    pub fn new(
        adapter_target_id: impl Into<String>,
        adapter_family: impl Into<String>,
        base_model_ref: impl Into<String>,
        adapter_format: impl Into<String>,
    ) -> Result<Self, AdapterWindowContractError> {
        Ok(Self {
            adapter_target_id: required_field("adapter_target_id", adapter_target_id.into())?,
            adapter_family: required_field("adapter_family", adapter_family.into())?,
            base_model_ref: required_field("base_model_ref", base_model_ref.into())?,
            adapter_format: required_field("adapter_format", adapter_format.into())?,
        })
    }
}

/// Stable dataset-slice identity for one contribution assignment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterDatasetSliceIdentity {
    /// Stable dataset identifier.
    pub dataset_id: String,
    /// Stable split identifier.
    pub split_name: String,
    /// Stable slice identifier.
    pub slice_id: String,
    /// Stable digest over the slice declaration.
    pub slice_digest: String,
}

impl AdapterDatasetSliceIdentity {
    /// Creates a dataset-slice identity.
    pub fn new(
        dataset_id: impl Into<String>,
        split_name: impl Into<String>,
        slice_id: impl Into<String>,
        slice_digest: impl Into<String>,
    ) -> Result<Self, AdapterWindowContractError> {
        Ok(Self {
            dataset_id: required_field("dataset_id", dataset_id.into())?,
            split_name: required_field("split_name", split_name.into())?,
            slice_id: required_field("slice_id", slice_id.into())?,
            slice_digest: required_field("slice_digest", slice_digest.into())?,
        })
    }
}

/// Assignment spec for one contributor in one adapter-training window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionAssignmentSpec {
    /// Stable contributor node identifier.
    pub contributor_node_id: String,
    /// Stable dataset slice assigned to the contributor.
    pub dataset_slice: AdapterDatasetSliceIdentity,
    /// Monotonic replay attempt index.
    pub replay_attempt: u32,
}

impl AdapterContributionAssignmentSpec {
    /// Creates a contribution assignment spec.
    pub fn new(
        contributor_node_id: impl Into<String>,
        dataset_slice: AdapterDatasetSliceIdentity,
        replay_attempt: u32,
    ) -> Result<Self, AdapterWindowContractError> {
        Ok(Self {
            contributor_node_id: required_field("contributor_node_id", contributor_node_id.into())?,
            dataset_slice,
            replay_attempt,
        })
    }
}

/// Shared binding facts carried by every adapter contribution receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionBinding {
    /// Stable training run identifier.
    pub training_run_id: String,
    /// Stable stage identifier.
    pub stage_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable contributor-set revision identifier.
    pub contributor_set_revision_id: String,
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable contributor node identifier.
    pub contributor_node_id: String,
    /// Stable adapter target.
    pub adapter_target: AdapterTargetIdentity,
    /// Stable dataset slice identity.
    pub dataset_slice: AdapterDatasetSliceIdentity,
    /// Policy revision consumed by the contribution.
    pub source_policy_revision: PolicyRevision,
    /// Checkpoint pointer consumed by the contribution.
    pub source_checkpoint_pointer: CheckpointPointer,
}

/// Typed assignment receipt for one adapter contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionAssignmentReceipt {
    /// Shared receipt binding.
    pub binding: AdapterContributionBinding,
    /// Monotonic replay attempt index.
    pub replay_attempt: u32,
    /// Assignment timestamp.
    pub assigned_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Local execution summary for one adapter contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionExecutionSummary {
    /// Local execution start time.
    pub started_at_ms: u64,
    /// Local execution end time.
    pub completed_at_ms: u64,
    /// Adapter-only local optimization steps completed.
    pub local_step_count: u32,
    /// Samples consumed locally.
    pub sample_count: u32,
    /// Optional average loss in basis points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_loss_bps: Option<u32>,
    /// Stable digest over the produced adapter delta or equivalent local output.
    pub adapter_delta_digest: String,
}

impl AdapterContributionExecutionSummary {
    /// Creates one local execution summary.
    pub fn new(
        started_at_ms: u64,
        completed_at_ms: u64,
        local_step_count: u32,
        sample_count: u32,
        average_loss_bps: Option<u32>,
        adapter_delta_digest: impl Into<String>,
    ) -> Result<Self, AdapterWindowContractError> {
        if completed_at_ms < started_at_ms {
            return Err(AdapterWindowContractError::InvalidExecutionTiming {
                started_at_ms,
                completed_at_ms,
            });
        }
        Ok(Self {
            started_at_ms,
            completed_at_ms,
            local_step_count,
            sample_count,
            average_loss_bps,
            adapter_delta_digest: required_field(
                "adapter_delta_digest",
                adapter_delta_digest.into(),
            )?,
        })
    }
}

/// Typed execution receipt for one adapter contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionExecutionReceipt {
    /// Shared receipt binding.
    pub binding: AdapterContributionBinding,
    /// Parent assignment receipt digest.
    pub assignment_receipt_digest: String,
    /// Local execution summary.
    pub summary: AdapterContributionExecutionSummary,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Stable upload locator for one contribution artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionUploadLocator {
    /// Stable upload or object reference.
    pub upload_reference: String,
    /// Manifest or object digest for the uploaded contribution.
    pub upload_manifest_digest: String,
    /// Declared uploaded payload size.
    pub payload_bytes: u64,
}

impl AdapterContributionUploadLocator {
    /// Creates an upload locator.
    pub fn new(
        upload_reference: impl Into<String>,
        upload_manifest_digest: impl Into<String>,
        payload_bytes: u64,
    ) -> Result<Self, AdapterWindowContractError> {
        Ok(Self {
            upload_reference: required_field("upload_reference", upload_reference.into())?,
            upload_manifest_digest: required_field(
                "upload_manifest_digest",
                upload_manifest_digest.into(),
            )?,
            payload_bytes,
        })
    }
}

/// Typed upload completion receipt for one adapter contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionUploadReceipt {
    /// Shared receipt binding.
    pub binding: AdapterContributionBinding,
    /// Parent execution receipt digest.
    pub execution_receipt_digest: String,
    /// Upload locator.
    pub upload: AdapterContributionUploadLocator,
    /// Upload completion time.
    pub uploaded_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Validator-owned disposition for one contribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributionValidatorDisposition {
    /// The contribution may participate in aggregation.
    Accepted,
    /// The contribution is retained for later review but is not aggregation-ready.
    Quarantined,
    /// The contribution is permanently excluded from the current window.
    Rejected,
    /// The contribution must be replayed or regenerated under a fresh assignment.
    ReplayRequired,
}

/// Typed validator disposition receipt for one adapter contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionValidatorReceipt {
    /// Shared receipt binding.
    pub binding: AdapterContributionBinding,
    /// Parent upload receipt digest.
    pub upload_receipt_digest: String,
    /// Machine-legible validator disposition.
    pub disposition: AdapterContributionValidatorDisposition,
    /// Stable validator reason or rubric code.
    pub validator_reason: String,
    /// Validation time.
    pub validated_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Aggregation-eligibility posture for one contribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributionAggregationEligibility {
    /// The contribution may participate in aggregation.
    Eligible,
    /// The contribution is excluded from aggregation.
    Ineligible,
}

/// Typed aggregation-eligibility receipt for one contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionAggregationEligibilityReceipt {
    /// Shared receipt binding.
    pub binding: AdapterContributionBinding,
    /// Parent validator receipt digest.
    pub validator_receipt_digest: String,
    /// Aggregation eligibility decision.
    pub eligibility: AdapterContributionAggregationEligibility,
    /// Optional aggregation weight in basis points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregation_weight_bps: Option<u16>,
    /// Decision time.
    pub decided_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Promotion posture for one sealed adapter window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterWindowPromotionDisposition {
    /// Accepted contributions promoted a new policy revision.
    Promoted,
    /// The window sealed without policy promotion.
    Held,
}

/// Window-level aggregation and promotion receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWindowAggregationReceipt {
    /// Stable training run identifier.
    pub training_run_id: String,
    /// Stable stage identifier.
    pub stage_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable contributor-set revision identifier.
    pub contributor_set_revision_id: String,
    /// Adapter target for the window.
    pub adapter_target: AdapterTargetIdentity,
    /// Policy revision active when the window started.
    pub input_policy_revision: PolicyRevision,
    /// Checkpoint pointer active when the window started.
    pub input_checkpoint_pointer: CheckpointPointer,
    /// Accepted contribution identifiers included in aggregation.
    pub accepted_contribution_ids: Vec<String>,
    /// Output policy revision when one was promoted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_policy_revision: Option<PolicyRevision>,
    /// Output checkpoint pointer when one was promoted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_checkpoint_pointer: Option<CheckpointPointer>,
    /// Promotion disposition.
    pub promotion_disposition: AdapterWindowPromotionDisposition,
    /// Aggregation time.
    pub aggregated_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Full receipt state for one contribution inside one adapter window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWindowContributionRecord {
    /// Typed assignment receipt.
    pub assignment: AdapterContributionAssignmentReceipt,
    /// Typed execution receipt when local work completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<AdapterContributionExecutionReceipt>,
    /// Typed upload receipt when artifact staging completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload: Option<AdapterContributionUploadReceipt>,
    /// Typed validator disposition receipt when validation completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator: Option<AdapterContributionValidatorReceipt>,
    /// Typed aggregation-eligibility receipt when the window is ready to seal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregation: Option<AdapterContributionAggregationEligibilityReceipt>,
}

/// One adapter-training window state machine layered over generic training-window status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterTrainingWindowStateMachine {
    /// Stable training run identifier.
    pub training_run_id: String,
    /// Stable stage identifier.
    pub stage_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable contributor-set revision identifier.
    pub contributor_set_revision_id: String,
    /// Window lifecycle state.
    pub status: TrainingWindowStatus,
    /// Target adapter identity.
    pub adapter_target: AdapterTargetIdentity,
    /// Policy revision active when the window started.
    pub input_policy_revision: PolicyRevision,
    /// Checkpoint pointer active when the window started.
    pub input_checkpoint_pointer: CheckpointPointer,
    /// Contribution receipts collected for the window.
    pub contributions: Vec<AdapterWindowContributionRecord>,
    /// Window-level aggregation receipt when the sealed window was aggregated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregation_receipt: Option<AdapterWindowAggregationReceipt>,
}

impl AdapterTrainingWindowStateMachine {
    /// Creates one planned adapter-training window with typed assignment receipts.
    pub fn new(
        training_run_id: impl Into<String>,
        stage_id: impl Into<String>,
        window_id: impl Into<String>,
        contributor_set_revision_id: impl Into<String>,
        adapter_target: AdapterTargetIdentity,
        input_policy_revision: PolicyRevision,
        input_checkpoint_pointer: CheckpointPointer,
        assignments: Vec<AdapterContributionAssignmentSpec>,
        planned_at_ms: u64,
    ) -> Result<Self, AdapterWindowContractError> {
        let training_run_id = required_field("training_run_id", training_run_id.into())?;
        let stage_id = required_field("stage_id", stage_id.into())?;
        let window_id = required_field("window_id", window_id.into())?;
        let contributor_set_revision_id = required_field(
            "contributor_set_revision_id",
            contributor_set_revision_id.into(),
        )?;
        if assignments.is_empty() {
            return Err(AdapterWindowContractError::MissingField {
                field: "assignments",
            });
        }
        let contributions = assignments
            .into_iter()
            .map(|spec| {
                let assignment_id = format!(
                    "adapter-assignment:{}:{}:{}",
                    window_id, spec.contributor_node_id, spec.replay_attempt
                );
                let contribution_id = format!(
                    "adapter-contribution:{}:{}:{}",
                    window_id, spec.contributor_node_id, spec.replay_attempt
                );
                let binding = AdapterContributionBinding {
                    training_run_id: training_run_id.clone(),
                    stage_id: stage_id.clone(),
                    window_id: window_id.clone(),
                    contributor_set_revision_id: contributor_set_revision_id.clone(),
                    assignment_id,
                    contribution_id,
                    contributor_node_id: spec.contributor_node_id,
                    adapter_target: adapter_target.clone(),
                    dataset_slice: spec.dataset_slice,
                    source_policy_revision: input_policy_revision.clone(),
                    source_checkpoint_pointer: input_checkpoint_pointer.clone(),
                };
                let receipt_digest =
                    stable_assignment_receipt_digest(&binding, spec.replay_attempt, planned_at_ms);
                Ok(AdapterWindowContributionRecord {
                    assignment: AdapterContributionAssignmentReceipt {
                        binding,
                        replay_attempt: spec.replay_attempt,
                        assigned_at_ms: planned_at_ms,
                        receipt_digest,
                    },
                    execution: None,
                    upload: None,
                    validator: None,
                    aggregation: None,
                })
            })
            .collect::<Result<Vec<_>, AdapterWindowContractError>>()?;
        Ok(Self {
            training_run_id,
            stage_id,
            window_id,
            contributor_set_revision_id,
            status: TrainingWindowStatus::Planned,
            adapter_target,
            input_policy_revision,
            input_checkpoint_pointer,
            contributions,
            aggregation_receipt: None,
        })
    }

    /// Activates one planned window.
    pub fn activate(&mut self) -> Result<(), AdapterWindowContractError> {
        if self.status != TrainingWindowStatus::Planned {
            return Err(AdapterWindowContractError::InvalidWindowStatus {
                window_id: self.window_id.clone(),
                action: "activate",
                status: training_window_status_label(self.status).to_string(),
            });
        }
        self.status = TrainingWindowStatus::Active;
        Ok(())
    }

    /// Records one local execution receipt.
    pub fn record_execution(
        &mut self,
        contribution_id: &str,
        summary: AdapterContributionExecutionSummary,
    ) -> Result<&AdapterContributionExecutionReceipt, AdapterWindowContractError> {
        self.require_status(TrainingWindowStatus::Active, "record execution")?;
        let contribution = self.contribution_mut(contribution_id)?;
        if contribution.execution.is_some() {
            return Err(AdapterWindowContractError::ExecutionAlreadyRecorded {
                contribution_id: contribution_id.to_string(),
            });
        }
        let receipt = AdapterContributionExecutionReceipt {
            binding: contribution.assignment.binding.clone(),
            assignment_receipt_digest: contribution.assignment.receipt_digest.clone(),
            receipt_digest: stable_execution_receipt_digest(
                &contribution.assignment.binding,
                contribution.assignment.receipt_digest.as_str(),
                &summary,
            ),
            summary,
        };
        contribution.execution = Some(receipt);
        Ok(contribution.execution.as_ref().expect("execution inserted"))
    }

    /// Records one upload completion receipt.
    pub fn record_upload(
        &mut self,
        contribution_id: &str,
        upload: AdapterContributionUploadLocator,
        uploaded_at_ms: u64,
    ) -> Result<&AdapterContributionUploadReceipt, AdapterWindowContractError> {
        self.require_status(TrainingWindowStatus::Active, "record upload")?;
        let contribution = self.contribution_mut(contribution_id)?;
        let execution = contribution.execution.as_ref().ok_or_else(|| {
            AdapterWindowContractError::ExecutionRequired {
                contribution_id: contribution_id.to_string(),
            }
        })?;
        if contribution.upload.is_some() {
            return Err(AdapterWindowContractError::UploadAlreadyRecorded {
                contribution_id: contribution_id.to_string(),
            });
        }
        let receipt = AdapterContributionUploadReceipt {
            binding: contribution.assignment.binding.clone(),
            execution_receipt_digest: execution.receipt_digest.clone(),
            upload,
            uploaded_at_ms,
            receipt_digest: stable_upload_receipt_digest(
                &contribution.assignment.binding,
                execution.receipt_digest.as_str(),
                contribution
                    .execution
                    .as_ref()
                    .expect("execution still present")
                    .summary
                    .adapter_delta_digest
                    .as_str(),
                uploaded_at_ms,
            ),
        };
        contribution.upload = Some(receipt);
        Ok(contribution.upload.as_ref().expect("upload inserted"))
    }

    /// Records one validator disposition receipt.
    pub fn record_validator_disposition(
        &mut self,
        contribution_id: &str,
        disposition: AdapterContributionValidatorDisposition,
        validator_reason: impl Into<String>,
        validated_at_ms: u64,
    ) -> Result<&AdapterContributionValidatorReceipt, AdapterWindowContractError> {
        self.require_status(TrainingWindowStatus::Active, "record validator disposition")?;
        let contribution = self.contribution_mut(contribution_id)?;
        let upload = contribution.upload.as_ref().ok_or_else(|| {
            AdapterWindowContractError::UploadRequired {
                contribution_id: contribution_id.to_string(),
            }
        })?;
        if contribution.validator.is_some() {
            return Err(AdapterWindowContractError::ValidationAlreadyRecorded {
                contribution_id: contribution_id.to_string(),
            });
        }
        let validator_reason = required_field("validator_reason", validator_reason.into())?;
        let receipt = AdapterContributionValidatorReceipt {
            binding: contribution.assignment.binding.clone(),
            upload_receipt_digest: upload.receipt_digest.clone(),
            disposition,
            validator_reason: validator_reason.clone(),
            validated_at_ms,
            receipt_digest: stable_validator_receipt_digest(
                &contribution.assignment.binding,
                upload.receipt_digest.as_str(),
                disposition,
                validator_reason.as_str(),
                validated_at_ms,
            ),
        };
        contribution.validator = Some(receipt);
        Ok(contribution.validator.as_ref().expect("validator inserted"))
    }

    /// Records one aggregation-eligibility receipt from the validator disposition.
    pub fn record_aggregation_eligibility(
        &mut self,
        contribution_id: &str,
        accepted_weight_bps: Option<u16>,
        decided_at_ms: u64,
    ) -> Result<&AdapterContributionAggregationEligibilityReceipt, AdapterWindowContractError> {
        self.require_status(
            TrainingWindowStatus::Active,
            "record aggregation eligibility",
        )?;
        let contribution = self.contribution_mut(contribution_id)?;
        let validator = contribution.validator.as_ref().ok_or_else(|| {
            AdapterWindowContractError::ValidationRequired {
                contribution_id: contribution_id.to_string(),
            }
        })?;
        if contribution.aggregation.is_some() {
            return Err(
                AdapterWindowContractError::AggregationEligibilityAlreadyRecorded {
                    contribution_id: contribution_id.to_string(),
                },
            );
        }
        let (eligibility, aggregation_weight_bps) =
            if validator.disposition == AdapterContributionValidatorDisposition::Accepted {
                (
                    AdapterContributionAggregationEligibility::Eligible,
                    Some(accepted_weight_bps.unwrap_or(10_000)),
                )
            } else {
                (AdapterContributionAggregationEligibility::Ineligible, None)
            };
        let receipt = AdapterContributionAggregationEligibilityReceipt {
            binding: contribution.assignment.binding.clone(),
            validator_receipt_digest: validator.receipt_digest.clone(),
            eligibility,
            aggregation_weight_bps,
            decided_at_ms,
            receipt_digest: stable_aggregation_eligibility_receipt_digest(
                &contribution.assignment.binding,
                validator.receipt_digest.as_str(),
                eligibility,
                aggregation_weight_bps,
                decided_at_ms,
            ),
        };
        contribution.aggregation = Some(receipt);
        Ok(contribution
            .aggregation
            .as_ref()
            .expect("aggregation receipt inserted"))
    }

    /// Seals one active window after every contribution has a terminal validator and aggregation posture.
    pub fn seal(&mut self) -> Result<(), AdapterWindowContractError> {
        self.require_status(TrainingWindowStatus::Active, "seal")?;
        let pending_contribution_ids = self
            .contributions
            .iter()
            .filter(|contribution| {
                contribution.validator.is_none() || contribution.aggregation.is_none()
            })
            .map(|contribution| contribution.assignment.binding.contribution_id.clone())
            .collect::<Vec<_>>();
        if !pending_contribution_ids.is_empty() {
            return Err(AdapterWindowContractError::SealIncomplete {
                window_id: self.window_id.clone(),
                pending_contribution_ids,
            });
        }
        self.status = TrainingWindowStatus::Sealed;
        Ok(())
    }

    /// Aggregates one sealed window into a new policy revision or an explicit hold.
    pub fn aggregate(
        &mut self,
        output_policy_revision: Option<PolicyRevision>,
        output_checkpoint_pointer: Option<CheckpointPointer>,
        aggregated_at_ms: u64,
    ) -> Result<&AdapterWindowAggregationReceipt, AdapterWindowContractError> {
        self.require_status(TrainingWindowStatus::Sealed, "aggregate")?;
        let accepted_contribution_ids = self
            .contributions
            .iter()
            .filter_map(|contribution| {
                contribution
                    .aggregation
                    .as_ref()
                    .is_some_and(|receipt| {
                        receipt.eligibility == AdapterContributionAggregationEligibility::Eligible
                    })
                    .then(|| contribution.assignment.binding.contribution_id.clone())
            })
            .collect::<Vec<_>>();
        if output_policy_revision.is_some() && accepted_contribution_ids.is_empty() {
            return Err(
                AdapterWindowContractError::PromotionWithoutEligibleContributions {
                    window_id: self.window_id.clone(),
                },
            );
        }
        if output_policy_revision.is_some() && output_checkpoint_pointer.is_none() {
            return Err(AdapterWindowContractError::MissingPromotionCheckpoint {
                window_id: self.window_id.clone(),
            });
        }
        let promotion_disposition = if output_policy_revision.is_some() {
            AdapterWindowPromotionDisposition::Promoted
        } else {
            AdapterWindowPromotionDisposition::Held
        };
        let receipt = AdapterWindowAggregationReceipt {
            training_run_id: self.training_run_id.clone(),
            stage_id: self.stage_id.clone(),
            window_id: self.window_id.clone(),
            contributor_set_revision_id: self.contributor_set_revision_id.clone(),
            adapter_target: self.adapter_target.clone(),
            input_policy_revision: self.input_policy_revision.clone(),
            input_checkpoint_pointer: self.input_checkpoint_pointer.clone(),
            accepted_contribution_ids,
            output_policy_revision,
            output_checkpoint_pointer,
            promotion_disposition,
            aggregated_at_ms,
            receipt_digest: stable_window_aggregation_receipt_digest(
                self.training_run_id.as_str(),
                self.stage_id.as_str(),
                self.window_id.as_str(),
                self.contributor_set_revision_id.as_str(),
                self.adapter_target.adapter_target_id.as_str(),
                self.input_policy_revision.revision_id.as_str(),
                self.input_checkpoint_pointer.pointer_digest.as_str(),
                self.contributions.iter().filter_map(|contribution| {
                    contribution
                        .aggregation
                        .as_ref()
                        .is_some_and(|receipt| {
                            receipt.eligibility
                                == AdapterContributionAggregationEligibility::Eligible
                        })
                        .then(|| contribution.assignment.binding.contribution_id.as_str())
                }),
                promotion_disposition,
                aggregated_at_ms,
            ),
        };
        self.aggregation_receipt = Some(receipt);
        self.status = TrainingWindowStatus::Scored;
        Ok(self
            .aggregation_receipt
            .as_ref()
            .expect("aggregation receipt inserted"))
    }

    /// Finalizes one aggregated window.
    pub fn reconcile(&mut self) -> Result<(), AdapterWindowContractError> {
        self.require_status(TrainingWindowStatus::Scored, "reconcile")?;
        self.status = TrainingWindowStatus::Reconciled;
        Ok(())
    }

    fn require_status(
        &self,
        expected: TrainingWindowStatus,
        action: &'static str,
    ) -> Result<(), AdapterWindowContractError> {
        if self.status == expected {
            Ok(())
        } else {
            Err(AdapterWindowContractError::InvalidWindowStatus {
                window_id: self.window_id.clone(),
                action,
                status: training_window_status_label(self.status).to_string(),
            })
        }
    }

    fn contribution_mut(
        &mut self,
        contribution_id: &str,
    ) -> Result<&mut AdapterWindowContributionRecord, AdapterWindowContractError> {
        self.contributions
            .iter_mut()
            .find(|contribution| contribution.assignment.binding.contribution_id == contribution_id)
            .ok_or_else(|| AdapterWindowContractError::UnknownContribution {
                contribution_id: contribution_id.to_string(),
            })
    }
}

/// Runnable end-to-end harness proving the adapter-window receipt state machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterTrainingWindowHarnessReceipt {
    /// Final reconciled state machine snapshot.
    pub window: AdapterTrainingWindowStateMachine,
    /// Stable digest over the harness result.
    pub receipt_digest: String,
}

/// Runs one deterministic adapter-window harness from planning through reconcile.
pub fn run_adapter_training_window_harness()
-> Result<AdapterTrainingWindowHarnessReceipt, AdapterWindowContractError> {
    let adapter_target = AdapterTargetIdentity::new(
        "apple.weather.v1",
        "apple.foundation_models",
        "apple://foundation-model/base",
        "apple.fmadapter",
    )?;
    let input_policy_revision = PolicyRevision::new(
        "apple.weather.policy",
        "policy-r7",
        "policy-digest-r7",
        1_000,
    )
    .with_revision_number(7)
    .with_checkpoint(harness_checkpoint_reference("checkpoint/weather/r7", 1_000));
    let input_checkpoint_pointer = CheckpointPointer::new(
        crate::CheckpointScopeBinding::new(crate::CheckpointScopeKind::Window, "window-weather-1"),
        "apple.weather.policy",
        harness_checkpoint_reference("checkpoint/weather/r7", 1_000).with_durable_at_ms(1_001),
        "manifest-digest-r7",
        1_001,
    )
    .expect("harness checkpoint pointer should validate");
    let assignments = vec![
        AdapterContributionAssignmentSpec::new(
            "node-a",
            AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-a",
                "slice-digest-a",
            )?,
            0,
        )?,
        AdapterContributionAssignmentSpec::new(
            "node-b",
            AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-b",
                "slice-digest-b",
            )?,
            0,
        )?,
        AdapterContributionAssignmentSpec::new(
            "node-c",
            AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-c",
                "slice-digest-c",
            )?,
            1,
        )?,
    ];

    let mut window = AdapterTrainingWindowStateMachine::new(
        "training-run-weather",
        "adapter-sft",
        "window-weather-1",
        "contributors-r3",
        adapter_target,
        input_policy_revision,
        input_checkpoint_pointer,
        assignments,
        1_010,
    )?;
    window.activate()?;

    let contribution_ids = window
        .contributions
        .iter()
        .map(|contribution| contribution.assignment.binding.contribution_id.clone())
        .collect::<Vec<_>>();

    let accepted = contribution_ids[0].clone();
    let quarantined = contribution_ids[1].clone();
    let replay_required = contribution_ids[2].clone();

    for (index, contribution_id) in contribution_ids.iter().enumerate() {
        window.record_execution(
            contribution_id.as_str(),
            AdapterContributionExecutionSummary::new(
                1_020 + index as u64 * 10,
                1_025 + index as u64 * 10,
                12 + index as u32,
                64 + index as u32,
                Some(220 + index as u32 * 10),
                format!("delta-digest-{index}"),
            )?,
        )?;
        window.record_upload(
            contribution_id.as_str(),
            AdapterContributionUploadLocator::new(
                format!("object://adapter-window/{contribution_id}"),
                format!("upload-manifest-{index}"),
                4_096 + index as u64,
            )?,
            1_040 + index as u64 * 10,
        )?;
    }

    window.record_validator_disposition(
        accepted.as_str(),
        AdapterContributionValidatorDisposition::Accepted,
        "validator.accepted",
        1_080,
    )?;
    window.record_aggregation_eligibility(accepted.as_str(), Some(7_500), 1_081)?;

    window.record_validator_disposition(
        quarantined.as_str(),
        AdapterContributionValidatorDisposition::Quarantined,
        "validator.quarantined.runtime_smoke_mismatch",
        1_082,
    )?;
    window.record_aggregation_eligibility(quarantined.as_str(), None, 1_083)?;

    window.record_validator_disposition(
        replay_required.as_str(),
        AdapterContributionValidatorDisposition::ReplayRequired,
        "validator.replay_required.timer_integrity",
        1_084,
    )?;
    window.record_aggregation_eligibility(replay_required.as_str(), None, 1_085)?;

    window.seal()?;
    window.aggregate(
        Some(
            PolicyRevision::new(
                "apple.weather.policy",
                "policy-r8",
                "policy-digest-r8",
                1_100,
            )
            .with_revision_number(8)
            .with_parent_revision_id("policy-r7")
            .with_checkpoint(harness_checkpoint_reference("checkpoint/weather/r8", 1_100)),
        ),
        Some(
            CheckpointPointer::new(
                crate::CheckpointScopeBinding::new(
                    crate::CheckpointScopeKind::Window,
                    "window-weather-1",
                ),
                "apple.weather.policy",
                harness_checkpoint_reference("checkpoint/weather/r8", 1_100)
                    .with_durable_at_ms(1_101),
                "manifest-digest-r8",
                1_101,
            )
            .expect("harness promoted checkpoint pointer should validate"),
        ),
        1_101,
    )?;
    window.reconcile()?;

    let receipt_digest = stable_digest([
        window.window_id.as_str(),
        training_window_status_label(window.status),
        window
            .aggregation_receipt
            .as_ref()
            .map(|receipt| receipt.receipt_digest.as_str())
            .unwrap_or("-"),
    ]);
    Ok(AdapterTrainingWindowHarnessReceipt {
        window,
        receipt_digest,
    })
}

fn required_field(
    field: &'static str,
    value: String,
) -> Result<String, AdapterWindowContractError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AdapterWindowContractError::MissingField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn stable_assignment_receipt_digest(
    binding: &AdapterContributionBinding,
    replay_attempt: u32,
    assigned_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_assignment_receipt",
        binding.training_run_id.as_str(),
        binding.stage_id.as_str(),
        binding.window_id.as_str(),
        binding.contributor_set_revision_id.as_str(),
        binding.assignment_id.as_str(),
        binding.contribution_id.as_str(),
        binding.contributor_node_id.as_str(),
        binding.adapter_target.adapter_target_id.as_str(),
        binding.dataset_slice.slice_digest.as_str(),
        binding.source_policy_revision.revision_id.as_str(),
        binding.source_checkpoint_pointer.pointer_digest.as_str(),
        replay_attempt.to_string().as_str(),
        assigned_at_ms.to_string().as_str(),
    ])
}

fn stable_execution_receipt_digest(
    binding: &AdapterContributionBinding,
    assignment_receipt_digest: &str,
    summary: &AdapterContributionExecutionSummary,
) -> String {
    stable_digest([
        "adapter_execution_receipt",
        binding.contribution_id.as_str(),
        assignment_receipt_digest,
        summary.started_at_ms.to_string().as_str(),
        summary.completed_at_ms.to_string().as_str(),
        summary.local_step_count.to_string().as_str(),
        summary.sample_count.to_string().as_str(),
        summary
            .average_loss_bps
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
        summary.adapter_delta_digest.as_str(),
    ])
}

fn stable_upload_receipt_digest(
    binding: &AdapterContributionBinding,
    execution_receipt_digest: &str,
    adapter_delta_digest: &str,
    uploaded_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_upload_receipt",
        binding.contribution_id.as_str(),
        execution_receipt_digest,
        adapter_delta_digest,
        uploaded_at_ms.to_string().as_str(),
    ])
}

fn stable_validator_receipt_digest(
    binding: &AdapterContributionBinding,
    upload_receipt_digest: &str,
    disposition: AdapterContributionValidatorDisposition,
    validator_reason: &str,
    validated_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_validator_receipt",
        binding.contribution_id.as_str(),
        upload_receipt_digest,
        adapter_validator_disposition_label(disposition),
        validator_reason,
        validated_at_ms.to_string().as_str(),
    ])
}

fn stable_aggregation_eligibility_receipt_digest(
    binding: &AdapterContributionBinding,
    validator_receipt_digest: &str,
    eligibility: AdapterContributionAggregationEligibility,
    aggregation_weight_bps: Option<u16>,
    decided_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_aggregation_eligibility_receipt",
        binding.contribution_id.as_str(),
        validator_receipt_digest,
        adapter_aggregation_eligibility_label(eligibility),
        aggregation_weight_bps
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
        decided_at_ms.to_string().as_str(),
    ])
}

fn stable_window_aggregation_receipt_digest<'a>(
    training_run_id: &str,
    stage_id: &str,
    window_id: &str,
    contributor_set_revision_id: &str,
    adapter_target_id: &str,
    input_policy_revision_id: &str,
    input_checkpoint_pointer_digest: &str,
    accepted_contribution_ids: impl Iterator<Item = &'a str>,
    promotion_disposition: AdapterWindowPromotionDisposition,
    aggregated_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        "adapter_window_aggregation_receipt",
        training_run_id,
        stage_id,
        window_id,
        contributor_set_revision_id,
        adapter_target_id,
        input_policy_revision_id,
        input_checkpoint_pointer_digest,
        adapter_window_promotion_disposition_label(promotion_disposition),
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    for contribution_id in accepted_contribution_ids {
        hasher.update(contribution_id.as_bytes());
        hasher.update(b"|");
    }
    hasher.update(aggregated_at_ms.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stable_digest<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    format!("{:x}", hasher.finalize())
}

fn adapter_validator_disposition_label(
    disposition: AdapterContributionValidatorDisposition,
) -> &'static str {
    match disposition {
        AdapterContributionValidatorDisposition::Accepted => "accepted",
        AdapterContributionValidatorDisposition::Quarantined => "quarantined",
        AdapterContributionValidatorDisposition::Rejected => "rejected",
        AdapterContributionValidatorDisposition::ReplayRequired => "replay_required",
    }
}

fn adapter_aggregation_eligibility_label(
    eligibility: AdapterContributionAggregationEligibility,
) -> &'static str {
    match eligibility {
        AdapterContributionAggregationEligibility::Eligible => "eligible",
        AdapterContributionAggregationEligibility::Ineligible => "ineligible",
    }
}

fn adapter_window_promotion_disposition_label(
    disposition: AdapterWindowPromotionDisposition,
) -> &'static str {
    match disposition {
        AdapterWindowPromotionDisposition::Promoted => "promoted",
        AdapterWindowPromotionDisposition::Held => "held",
    }
}

fn training_window_status_label(status: TrainingWindowStatus) -> &'static str {
    match status {
        TrainingWindowStatus::Planned => "planned",
        TrainingWindowStatus::Active => "active",
        TrainingWindowStatus::Sealed => "sealed",
        TrainingWindowStatus::Scored => "scored",
        TrainingWindowStatus::Reconciled => "reconciled",
    }
}

fn harness_checkpoint_reference(
    checkpoint_ref: &str,
    started_at_ms: u64,
) -> psionic_runtime::TrainingCheckpointReference {
    psionic_runtime::TrainingCheckpointReference::new(
        "apple.weather.policy",
        format!("stream://{checkpoint_ref}"),
        format!("manifest://{checkpoint_ref}"),
        format!("object://{checkpoint_ref}"),
        "node-a",
        7,
        "cluster-digest-weather",
        "topology-digest-weather",
        started_at_ms,
    )
    .with_checkpoint_ref(checkpoint_ref)
    .with_step(70)
}

#[cfg(test)]
mod tests {
    use super::{
        AdapterContributionAggregationEligibility, AdapterContributionExecutionSummary,
        AdapterContributionUploadLocator, AdapterContributionValidatorDisposition,
        AdapterDatasetSliceIdentity, AdapterTargetIdentity, AdapterTrainingWindowStateMachine,
        AdapterWindowContractError, run_adapter_training_window_harness,
    };
    use crate::{CheckpointPointer, CheckpointScopeBinding, CheckpointScopeKind, PolicyRevision};

    #[test]
    fn adapter_window_requires_complete_receipts_before_seal() {
        let adapter_target = AdapterTargetIdentity::new(
            "apple.weather.v1",
            "apple.foundation_models",
            "apple://foundation-model/base",
            "apple.fmadapter",
        )
        .expect("adapter target");
        let policy = PolicyRevision::new(
            "apple.weather.policy",
            "policy-r7",
            "policy-digest-r7",
            1_000,
        );
        let checkpoint_pointer = CheckpointPointer::new(
            CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-weather-2"),
            "apple.weather.policy",
            super::harness_checkpoint_reference("checkpoint/weather/r7", 1_000),
            "manifest-digest-r7",
            1_001,
        )
        .expect("checkpoint pointer");
        let mut window = AdapterTrainingWindowStateMachine::new(
            "training-run-weather",
            "adapter-sft",
            "window-weather-2",
            "contributors-r3",
            adapter_target,
            policy,
            checkpoint_pointer,
            vec![
                super::AdapterContributionAssignmentSpec::new(
                    "node-a",
                    AdapterDatasetSliceIdentity::new(
                        "dataset.weather",
                        "train",
                        "slice-a",
                        "slice-digest-a",
                    )
                    .expect("slice"),
                    0,
                )
                .expect("assignment"),
            ],
            1_010,
        )
        .expect("window");
        window.activate().expect("activate");
        let contribution_id = window.contributions[0]
            .assignment
            .binding
            .contribution_id
            .clone();
        window
            .record_execution(
                contribution_id.as_str(),
                AdapterContributionExecutionSummary::new(
                    1_020,
                    1_025,
                    12,
                    64,
                    Some(220),
                    "delta-digest-0",
                )
                .expect("execution"),
            )
            .expect("record execution");
        window
            .record_upload(
                contribution_id.as_str(),
                AdapterContributionUploadLocator::new(
                    "object://adapter-window/a",
                    "upload-manifest-a",
                    4_096,
                )
                .expect("upload"),
                1_040,
            )
            .expect("record upload");
        let error = window
            .seal()
            .expect_err("seal should fail without validator");
        assert_eq!(
            error,
            AdapterWindowContractError::SealIncomplete {
                window_id: "window-weather-2".to_string(),
                pending_contribution_ids: vec![contribution_id],
            }
        );
    }

    #[test]
    fn adapter_window_harness_reconciles_and_preserves_terminal_states() {
        let receipt = run_adapter_training_window_harness().expect("harness");
        assert_eq!(
            receipt.window.status,
            crate::TrainingWindowStatus::Reconciled
        );
        assert_eq!(
            receipt
                .window
                .aggregation_receipt
                .as_ref()
                .expect("aggregation")
                .promotion_disposition,
            super::AdapterWindowPromotionDisposition::Promoted
        );
        let eligibility = receipt
            .window
            .contributions
            .iter()
            .map(|contribution| {
                (
                    contribution.assignment.binding.contributor_node_id.clone(),
                    contribution
                        .aggregation
                        .as_ref()
                        .expect("aggregation")
                        .eligibility,
                    contribution
                        .validator
                        .as_ref()
                        .expect("validator")
                        .disposition,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            eligibility[0].1,
            AdapterContributionAggregationEligibility::Eligible
        );
        assert_eq!(
            eligibility[0].2,
            AdapterContributionValidatorDisposition::Accepted
        );
        assert_eq!(
            eligibility[1].1,
            AdapterContributionAggregationEligibility::Ineligible
        );
        assert_eq!(
            eligibility[1].2,
            AdapterContributionValidatorDisposition::Quarantined
        );
        assert_eq!(
            eligibility[2].1,
            AdapterContributionAggregationEligibility::Ineligible
        );
        assert_eq!(
            eligibility[2].2,
            AdapterContributionValidatorDisposition::ReplayRequired
        );
        assert!(!receipt.receipt_digest.is_empty());
    }
}
