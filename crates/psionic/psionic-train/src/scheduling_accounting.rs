use std::collections::BTreeMap;
use std::path::PathBuf;

use psionic_environments::EnvironmentPackageKey;
use psionic_eval::EvalArtifact;
use psionic_runtime::{
    RuntimeDispatchPlan, RuntimeDispatchPolicy, RuntimeWorkClass, RuntimeWorkItem,
};
use psionic_sandbox::{
    ProviderSandboxArtifactDigest, ProviderSandboxDeliveryEvidence, ProviderSandboxEntrypointType,
    ProviderSandboxEnvironmentVar, ProviderSandboxExecutionClass, ProviderSandboxExecutionReceipt,
    ProviderSandboxExecutionState, ProviderSandboxJobRequest, ProviderSandboxResourceRequest,
    ProviderSandboxResourceUsageSummary, ProviderSandboxStateTransition,
    ProviderSandboxTerminationReason,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{RolloutArtifact, TrainerBatch};

/// Error returned by the train scheduling and accounting layer.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TrainSchedulingAccountingError {
    /// One queue policy used an invalid priority.
    #[error("queue policy `{queue_class:?}` has invalid priority `{priority_bps}`")]
    InvalidQueuePriority {
        /// Queue class with the invalid priority.
        queue_class: TrainQueueClass,
        /// Priority that fell outside `0..=10000`.
        priority_bps: u16,
    },
    /// One queue class had no policy.
    #[error("missing scheduling policy for queue class `{queue_class:?}`")]
    MissingQueuePolicy {
        /// Queue class without a policy.
        queue_class: TrainQueueClass,
    },
    /// One workload role had no cost rate.
    #[error("missing role cost rate for workload role `{role:?}`")]
    MissingRoleCostRate {
        /// Role without a configured rate.
        role: TrainWorkloadRole,
    },
    /// A workload attempted to schedule without any runtime work.
    #[error("scheduled workload `{workload_id}` must carry at least one runtime work item")]
    EmptyRuntimeWork {
        /// Stable workload identifier.
        workload_id: String,
    },
    /// A requested workload was not active.
    #[error("scheduled workload `{workload_id}` is not active")]
    UnknownActiveWorkload {
        /// Stable workload identifier.
        workload_id: String,
    },
}

/// Role or service lane paying for one workload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainWorkloadRole {
    /// One trainer-step workload.
    Trainer,
    /// One rollout-generation workload.
    Rollout,
    /// One evaluation workload.
    Eval,
    /// One sandbox workload.
    Sandbox,
    /// One validator-owned verification workload.
    Validator,
}

/// Scheduling queue class used for admission and preemption.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainQueueClass {
    /// Highest-priority low-latency work.
    Realtime,
    /// General foreground work.
    Standard,
    /// Throughput-oriented batch work.
    Bulk,
    /// Lowest-priority background work.
    Background,
}

/// Preemption posture for one queue class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainPreemptionMode {
    /// This queue never preempts other active work.
    Never,
    /// This queue may preempt only lower-priority work.
    LowerPriorityOnly,
    /// This queue may preempt lower or equal-priority work.
    LowerOrEqualPriority,
}

/// Global capacity cap for active scheduled work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainBudgetCap {
    /// Maximum active work units across all admitted workloads.
    pub max_active_work_units: usize,
    /// Maximum active byte volume across all admitted workloads.
    pub max_active_bytes: u64,
    /// Maximum active estimated cost units across all admitted workloads.
    pub max_active_cost_units: u64,
}

impl TrainBudgetCap {
    /// Creates a global active-work budget.
    #[must_use]
    pub fn new(
        max_active_work_units: usize,
        max_active_bytes: u64,
        max_active_cost_units: u64,
    ) -> Self {
        Self {
            max_active_work_units: max_active_work_units.max(1),
            max_active_bytes: max_active_bytes.max(1),
            max_active_cost_units: max_active_cost_units.max(1),
        }
    }
}

/// Queue-specific scheduling policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainQueuePolicy {
    /// Relative queue priority in basis points.
    pub priority_bps: u16,
    /// Preemption posture for workloads admitted through this queue.
    pub preemption_mode: TrainPreemptionMode,
    /// Runtime-owned dispatch policy used to estimate work cost.
    pub dispatch_policy: RuntimeDispatchPolicy,
}

impl TrainQueuePolicy {
    /// Creates one queue policy.
    pub fn new(
        priority_bps: u16,
        preemption_mode: TrainPreemptionMode,
        dispatch_policy: RuntimeDispatchPolicy,
        queue_class: TrainQueueClass,
    ) -> Result<Self, TrainSchedulingAccountingError> {
        if priority_bps > 10_000 {
            return Err(TrainSchedulingAccountingError::InvalidQueuePriority {
                queue_class,
                priority_bps,
            });
        }
        Ok(Self {
            priority_bps,
            preemption_mode,
            dispatch_policy,
        })
    }
}

/// Cost-rate contract for one workload role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainRoleCostRate {
    /// Constant baseline for admitting work of this role.
    pub base_cost_units: u64,
    /// Cost multiplier for runtime-dispatch cost units.
    pub cost_units_per_dispatch_unit: u64,
    /// Cost multiplier for each kibibyte of touched data.
    pub cost_units_per_kibibyte: u64,
}

impl TrainRoleCostRate {
    /// Creates a role cost rate.
    #[must_use]
    pub const fn new(
        base_cost_units: u64,
        cost_units_per_dispatch_unit: u64,
        cost_units_per_kibibyte: u64,
    ) -> Self {
        Self {
            base_cost_units,
            cost_units_per_dispatch_unit,
            cost_units_per_kibibyte,
        }
    }
}

/// Scheduling and accounting policy over all active work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainSchedulingAccountingPolicy {
    /// Global budget for active work.
    pub global_budget: TrainBudgetCap,
    /// Queue-class scheduling policies.
    pub queue_policies: BTreeMap<TrainQueueClass, TrainQueuePolicy>,
    /// Role-specific cost rates.
    pub role_cost_rates: BTreeMap<TrainWorkloadRole, TrainRoleCostRate>,
}

impl Default for TrainSchedulingAccountingPolicy {
    fn default() -> Self {
        let mut queue_policies = BTreeMap::new();
        queue_policies.insert(
            TrainQueueClass::Realtime,
            TrainQueuePolicy {
                priority_bps: 9_000,
                preemption_mode: TrainPreemptionMode::LowerPriorityOnly,
                dispatch_policy: RuntimeDispatchPolicy::quantized_decode_default(2),
            },
        );
        queue_policies.insert(
            TrainQueueClass::Standard,
            TrainQueuePolicy {
                priority_bps: 6_000,
                preemption_mode: TrainPreemptionMode::LowerPriorityOnly,
                dispatch_policy: RuntimeDispatchPolicy::data_plane_default(4),
            },
        );
        queue_policies.insert(
            TrainQueueClass::Bulk,
            TrainQueuePolicy {
                priority_bps: 3_500,
                preemption_mode: TrainPreemptionMode::LowerPriorityOnly,
                dispatch_policy: RuntimeDispatchPolicy {
                    max_workers: 4,
                    target_batch_work_units: 8,
                    max_batch_bytes: 8 * 1024 * 1024,
                    park_after_idle_batches: 8,
                },
            },
        );
        queue_policies.insert(
            TrainQueueClass::Background,
            TrainQueuePolicy {
                priority_bps: 1_000,
                preemption_mode: TrainPreemptionMode::Never,
                dispatch_policy: RuntimeDispatchPolicy {
                    max_workers: 2,
                    target_batch_work_units: 6,
                    max_batch_bytes: 4 * 1024 * 1024,
                    park_after_idle_batches: 8,
                },
            },
        );

        let mut role_cost_rates = BTreeMap::new();
        role_cost_rates.insert(TrainWorkloadRole::Trainer, TrainRoleCostRate::new(24, 3, 1));
        role_cost_rates.insert(TrainWorkloadRole::Rollout, TrainRoleCostRate::new(16, 2, 1));
        role_cost_rates.insert(TrainWorkloadRole::Eval, TrainRoleCostRate::new(18, 3, 1));
        role_cost_rates.insert(TrainWorkloadRole::Sandbox, TrainRoleCostRate::new(12, 2, 1));
        role_cost_rates.insert(
            TrainWorkloadRole::Validator,
            TrainRoleCostRate::new(20, 3, 1),
        );

        Self {
            global_budget: TrainBudgetCap::new(12, 32 * 1024 * 1024, 3_000),
            queue_policies,
            role_cost_rates,
        }
    }
}

impl TrainSchedulingAccountingPolicy {
    /// Creates and validates a scheduling policy.
    pub fn new(
        global_budget: TrainBudgetCap,
        queue_policies: BTreeMap<TrainQueueClass, TrainQueuePolicy>,
        role_cost_rates: BTreeMap<TrainWorkloadRole, TrainRoleCostRate>,
    ) -> Result<Self, TrainSchedulingAccountingError> {
        for (queue_class, policy) in &queue_policies {
            if policy.priority_bps > 10_000 {
                return Err(TrainSchedulingAccountingError::InvalidQueuePriority {
                    queue_class: *queue_class,
                    priority_bps: policy.priority_bps,
                });
            }
        }
        Ok(Self {
            global_budget,
            queue_policies,
            role_cost_rates,
        })
    }
}

/// Inspectable provenance attached to one scheduled workload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainWorkloadProvenance {
    /// Stable environment package key when the work is environment-scoped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_key: Option<String>,
    /// Stable validator scope when the work belongs to a validator or benchmark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_scope_id: Option<String>,
    /// Stable policy revision when the work is policy-scoped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_revision_id: Option<String>,
    /// Stable source object reference for replay-safe accounting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

impl TrainWorkloadProvenance {
    /// Creates empty provenance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches environment identity.
    #[must_use]
    pub fn with_environment(mut self, environment_key: &EnvironmentPackageKey) -> Self {
        self.environment_key = Some(environment_key.storage_key());
        self
    }

    /// Attaches validator scope.
    #[must_use]
    pub fn with_validator_scope_id(mut self, validator_scope_id: impl Into<String>) -> Self {
        self.validator_scope_id = Some(validator_scope_id.into());
        self
    }

    /// Attaches policy revision identity.
    #[must_use]
    pub fn with_policy_revision_id(mut self, policy_revision_id: impl Into<String>) -> Self {
        self.policy_revision_id = Some(policy_revision_id.into());
        self
    }

    /// Attaches the replay-safe source reference.
    #[must_use]
    pub fn with_source_ref(mut self, source_ref: impl Into<String>) -> Self {
        self.source_ref = Some(source_ref.into());
        self
    }
}

/// One admitted or queued workload under train scheduling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainScheduledWorkload {
    /// Stable workload identifier.
    pub workload_id: String,
    /// Role paying for or owning the workload.
    pub role: TrainWorkloadRole,
    /// Queue class used for admission.
    pub queue_class: TrainQueueClass,
    /// Runtime-owned low-level work items.
    pub runtime_items: Vec<RuntimeWorkItem>,
    /// Provenance carried into accounting.
    pub provenance: TrainWorkloadProvenance,
    /// Submission timestamp.
    pub submitted_at_ms: u64,
}

impl TrainScheduledWorkload {
    /// Creates one scheduled workload from explicit runtime work.
    #[must_use]
    pub fn new(
        workload_id: impl Into<String>,
        role: TrainWorkloadRole,
        queue_class: TrainQueueClass,
        runtime_items: Vec<RuntimeWorkItem>,
        provenance: TrainWorkloadProvenance,
        submitted_at_ms: u64,
    ) -> Self {
        Self {
            workload_id: workload_id.into(),
            role,
            queue_class,
            runtime_items,
            provenance,
            submitted_at_ms,
        }
    }

    /// Creates a rollout workload from one rollout artifact.
    #[must_use]
    pub fn for_rollout_artifact(
        artifact: &RolloutArtifact,
        queue_class: TrainQueueClass,
        submitted_at_ms: u64,
    ) -> Self {
        let token_count = artifact.token_count().max(1);
        Self::new(
            format!("rollout-workload:{}", artifact.artifact_id),
            TrainWorkloadRole::Rollout,
            queue_class,
            vec![RuntimeWorkItem::new(
                RuntimeWorkClass::RolloutStep,
                token_count as usize,
                token_count.saturating_mul(16),
            )],
            TrainWorkloadProvenance::new()
                .with_environment(&artifact.environment)
                .with_policy_revision_id(artifact.source_policy_revision.revision_id.clone())
                .with_source_ref(artifact.artifact_id.clone()),
            submitted_at_ms,
        )
    }

    /// Creates a trainer workload from one trainer batch.
    #[must_use]
    pub fn for_trainer_batch(
        batch: &TrainerBatch,
        environment_key: &EnvironmentPackageKey,
        queue_class: TrainQueueClass,
        submitted_at_ms: u64,
    ) -> Self {
        Self::new(
            format!("trainer-workload:{}", batch.batch_id),
            TrainWorkloadRole::Trainer,
            queue_class,
            vec![RuntimeWorkItem::new(
                RuntimeWorkClass::TrainingStep,
                batch.rollout_count.max(1) as usize,
                batch.token_count.max(1),
            )],
            TrainWorkloadProvenance::new()
                .with_environment(environment_key)
                .with_policy_revision_id(batch.policy_lineage.target_revision.revision_id.clone())
                .with_source_ref(batch.batch_id.clone()),
            submitted_at_ms,
        )
    }

    /// Creates an eval workload from one eval artifact.
    #[must_use]
    pub fn for_eval_artifact(
        artifact: &EvalArtifact,
        environment_key: &EnvironmentPackageKey,
        queue_class: TrainQueueClass,
        validator_scope_id: Option<&str>,
        submitted_at_ms: u64,
    ) -> Self {
        let mut provenance = TrainWorkloadProvenance::new()
            .with_environment(environment_key)
            .with_source_ref(artifact.artifact_ref.clone());
        if let Some(validator_scope_id) = validator_scope_id {
            provenance = provenance.with_validator_scope_id(validator_scope_id);
        }
        Self::new(
            format!(
                "eval-workload:{}:{}",
                artifact.artifact_kind, artifact.artifact_ref
            ),
            TrainWorkloadRole::Eval,
            queue_class,
            vec![RuntimeWorkItem::new(
                RuntimeWorkClass::EvalStep,
                1,
                (artifact.artifact_kind.len() as u64 + artifact.artifact_ref.len() as u64)
                    .saturating_mul(128)
                    .max(1_024),
            )],
            provenance,
            submitted_at_ms,
        )
    }

    /// Creates a validator-owned workload from one eval artifact.
    #[must_use]
    pub fn for_validator_artifact(
        artifact: &EvalArtifact,
        environment_key: &EnvironmentPackageKey,
        queue_class: TrainQueueClass,
        validator_scope_id: impl Into<String>,
        submitted_at_ms: u64,
    ) -> Self {
        Self::new(
            format!(
                "validator-workload:{}:{}",
                artifact.artifact_kind, artifact.artifact_ref
            ),
            TrainWorkloadRole::Validator,
            queue_class,
            vec![RuntimeWorkItem::new(
                RuntimeWorkClass::ValidatorStep,
                1,
                (artifact.artifact_kind.len() as u64 + artifact.artifact_ref.len() as u64)
                    .saturating_mul(160)
                    .max(2_048),
            )],
            TrainWorkloadProvenance::new()
                .with_environment(environment_key)
                .with_validator_scope_id(validator_scope_id)
                .with_source_ref(artifact.artifact_ref.clone()),
            submitted_at_ms,
        )
    }

    /// Creates a sandbox workload from one sandbox job request.
    #[must_use]
    pub fn for_sandbox_job(
        request: &ProviderSandboxJobRequest,
        environment_key: &EnvironmentPackageKey,
        queue_class: TrainQueueClass,
        validator_scope_id: Option<&str>,
        submitted_at_ms: u64,
    ) -> Self {
        let mut provenance = TrainWorkloadProvenance::new()
            .with_environment(environment_key)
            .with_source_ref(request.job_id.clone());
        if let Some(validator_scope_id) = validator_scope_id {
            provenance = provenance.with_validator_scope_id(validator_scope_id);
        }
        let work_units = request.resource_request.cpu_limit.unwrap_or(1) as usize;
        let bytes = request
            .resource_request
            .memory_limit_mb
            .unwrap_or(64)
            .saturating_mul(1024 * 1024);
        Self::new(
            format!("sandbox-workload:{}", request.job_id),
            TrainWorkloadRole::Sandbox,
            queue_class,
            vec![RuntimeWorkItem::new(
                RuntimeWorkClass::SandboxStep,
                work_units.max(1),
                bytes.max(1),
            )],
            provenance,
            submitted_at_ms,
        )
    }
}

/// Current active-budget usage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainBudgetUsage {
    /// Aggregate active work units.
    pub active_work_units: usize,
    /// Aggregate active byte volume.
    pub active_bytes: u64,
    /// Aggregate active estimated cost units.
    pub active_cost_units: u64,
}

impl TrainBudgetUsage {
    fn with_added(self, work_units: usize, bytes: u64, cost_units: u64) -> Self {
        Self {
            active_work_units: self.active_work_units.saturating_add(work_units),
            active_bytes: self.active_bytes.saturating_add(bytes),
            active_cost_units: self.active_cost_units.saturating_add(cost_units),
        }
    }

    fn fits_inside(self, cap: TrainBudgetCap) -> bool {
        self.active_work_units <= cap.max_active_work_units
            && self.active_bytes <= cap.max_active_bytes
            && self.active_cost_units <= cap.max_active_cost_units
    }
}

/// One active workload with estimated dispatch and cost truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveTrainWorkloadRecord {
    /// Scheduled workload.
    pub workload: TrainScheduledWorkload,
    /// Runtime dispatch plan used for cost estimation.
    pub dispatch_plan: RuntimeDispatchPlan,
    /// Estimated cost units for the active workload.
    pub estimated_cost_units: u64,
    /// Aggregate work units across runtime items.
    pub total_work_units: usize,
    /// Aggregate byte volume across runtime items.
    pub total_bytes: u64,
    /// Admission timestamp.
    pub admitted_at_ms: u64,
}

/// One queued workload waiting for budget or preemption opportunity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedTrainWorkloadRecord {
    /// Scheduled workload.
    pub workload: TrainScheduledWorkload,
    /// Estimated cost units for the queued workload.
    pub estimated_cost_units: u64,
    /// Aggregate work units across runtime items.
    pub total_work_units: usize,
    /// Aggregate byte volume across runtime items.
    pub total_bytes: u64,
    /// Queue timestamp.
    pub queued_at_ms: u64,
}

/// One completed accounting entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainAccountingEntry {
    /// Stable workload identifier.
    pub workload_id: String,
    /// Workload role.
    pub role: TrainWorkloadRole,
    /// Queue class used for admission.
    pub queue_class: TrainQueueClass,
    /// Environment key when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_key: Option<String>,
    /// Validator scope when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_scope_id: Option<String>,
    /// Aggregate work units.
    pub total_work_units: usize,
    /// Aggregate bytes.
    pub total_bytes: u64,
    /// Estimated cost units from admission.
    pub estimated_cost_units: u64,
    /// Actual cost units recorded at completion.
    pub actual_cost_units: u64,
    /// Completion timestamp.
    pub completed_at_ms: u64,
}

/// Admission outcome for one workload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainAdmissionOutcome {
    /// The workload was admitted directly.
    Admitted,
    /// The workload was admitted after lower-priority work was preempted.
    AdmittedAfterPreemption,
    /// The workload could not fit and was queued.
    Queued,
}

/// Reason code for one preemption event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainPreemptionReason {
    /// Higher-priority work displaced lower-priority work under budget pressure.
    HigherPriorityBudgetPressure,
}

/// Receipt for one preempted workload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainPreemptionReceipt {
    /// Preempted workload id.
    pub workload_id: String,
    /// Role of the preempted workload.
    pub role: TrainWorkloadRole,
    /// Queue class of the preempted workload.
    pub queue_class: TrainQueueClass,
    /// Estimated cost units being displaced.
    pub estimated_cost_units: u64,
    /// Reason code for the preemption.
    pub reason: TrainPreemptionReason,
}

/// Admission receipt for one workload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainAdmissionReceipt {
    /// Stable workload identifier.
    pub workload_id: String,
    /// Admission outcome.
    pub outcome: TrainAdmissionOutcome,
    /// Runtime dispatch plan used for cost estimation.
    pub dispatch_plan: RuntimeDispatchPlan,
    /// Estimated cost units for the workload.
    pub estimated_cost_units: u64,
    /// Active-budget usage after the admission or queueing decision.
    pub budget_usage: TrainBudgetUsage,
    /// Explicit preemption receipts when the workload displaced other work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preemptions: Vec<TrainPreemptionReceipt>,
    /// Queue depth after the admission or queueing decision.
    pub queue_depth: usize,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

/// Completion receipt for one workload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainCompletionReceipt {
    /// Stable workload identifier.
    pub workload_id: String,
    /// Estimated cost units from admission time.
    pub estimated_cost_units: u64,
    /// Actual cost units recorded at completion.
    pub actual_cost_units: u64,
    /// Any queued workloads that became active after completion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admitted_from_queue: Vec<TrainAdmissionReceipt>,
    /// Stable digest over the completion receipt.
    pub receipt_digest: String,
}

/// Role-scoped accounting summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainRoleAccountingSummary {
    /// Workload role.
    pub role: TrainWorkloadRole,
    /// Number of currently active workloads for this role.
    pub active_workloads: usize,
    /// Estimated active cost units.
    pub active_estimated_cost_units: u64,
    /// Number of completed workloads for this role.
    pub completed_workloads: usize,
    /// Completed actual cost units.
    pub completed_actual_cost_units: u64,
    /// Completed work units.
    pub completed_work_units: usize,
    /// Completed bytes.
    pub completed_bytes: u64,
}

/// Environment-scoped cost summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainEnvironmentAccountingSummary {
    /// Stable environment key.
    pub environment_key: String,
    /// Completed workload count.
    pub completed_workloads: usize,
    /// Completed actual cost units.
    pub completed_actual_cost_units: u64,
    /// Completed work units.
    pub completed_work_units: usize,
    /// Completed bytes.
    pub completed_bytes: u64,
}

/// Validator-scoped cost summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainValidatorAccountingSummary {
    /// Stable validator scope identifier.
    pub validator_scope_id: String,
    /// Completed workload count.
    pub completed_workloads: usize,
    /// Completed actual cost units.
    pub completed_actual_cost_units: u64,
    /// Completed work units.
    pub completed_work_units: usize,
    /// Completed bytes.
    pub completed_bytes: u64,
}

/// Full snapshot of train scheduling and accounting truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainSchedulingAccountingSnapshot {
    /// Active budget usage.
    pub budget_usage: TrainBudgetUsage,
    /// Current active workloads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_workloads: Vec<ActiveTrainWorkloadRecord>,
    /// Current queued workloads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued_workloads: Vec<QueuedTrainWorkloadRecord>,
    /// Role-scoped summaries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_summaries: Vec<TrainRoleAccountingSummary>,
    /// Environment-scoped summaries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_summaries: Vec<TrainEnvironmentAccountingSummary>,
    /// Validator-scoped summaries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validator_summaries: Vec<TrainValidatorAccountingSummary>,
    /// Stable digest over the snapshot.
    pub snapshot_digest: String,
}

/// Controller for queue admission, preemption, and cost attribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainSchedulingAccountingController {
    /// Scheduling and accounting policy.
    pub policy: TrainSchedulingAccountingPolicy,
    /// Active workloads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_workloads: Vec<ActiveTrainWorkloadRecord>,
    /// Queued workloads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued_workloads: Vec<QueuedTrainWorkloadRecord>,
    /// Completed accounting entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completed_entries: Vec<TrainAccountingEntry>,
}

impl TrainSchedulingAccountingController {
    /// Creates a scheduling and accounting controller from explicit policy.
    pub fn new(
        policy: TrainSchedulingAccountingPolicy,
    ) -> Result<Self, TrainSchedulingAccountingError> {
        for (queue_class, queue_policy) in &policy.queue_policies {
            if queue_policy.priority_bps > 10_000 {
                return Err(TrainSchedulingAccountingError::InvalidQueuePriority {
                    queue_class: *queue_class,
                    priority_bps: queue_policy.priority_bps,
                });
            }
        }
        Ok(Self {
            policy,
            active_workloads: Vec::new(),
            queued_workloads: Vec::new(),
            completed_entries: Vec::new(),
        })
    }

    /// Admits or queues one workload under the current budget and priority rules.
    pub fn admit(
        &mut self,
        workload: TrainScheduledWorkload,
    ) -> Result<TrainAdmissionReceipt, TrainSchedulingAccountingError> {
        let estimated = self.estimate_workload(&workload)?;
        let current_usage = self.current_budget_usage();
        let projected = current_usage.with_added(
            estimated.total_work_units,
            estimated.total_bytes,
            estimated.estimated_cost_units,
        );
        if projected.fits_inside(self.policy.global_budget) {
            let active_record = estimated.into_active_record(workload.submitted_at_ms);
            self.active_workloads.push(active_record.clone());
            return Ok(build_admission_receipt(
                &active_record,
                TrainAdmissionOutcome::Admitted,
                self.current_budget_usage(),
                Vec::new(),
                self.queued_workloads.len(),
            ));
        }

        let incoming_queue_policy = self.queue_policy(workload.queue_class)?;
        let preemption_targets =
            self.select_preemption_targets(incoming_queue_policy, current_usage, &estimated)?;
        if !preemption_targets.is_empty() {
            let mut preemptions = Vec::new();
            for workload_id in &preemption_targets {
                if let Some(index) = self
                    .active_workloads
                    .iter()
                    .position(|candidate| &candidate.workload.workload_id == workload_id)
                {
                    let preempted = self.active_workloads.remove(index);
                    preemptions.push(TrainPreemptionReceipt {
                        workload_id: preempted.workload.workload_id.clone(),
                        role: preempted.workload.role,
                        queue_class: preempted.workload.queue_class,
                        estimated_cost_units: preempted.estimated_cost_units,
                        reason: TrainPreemptionReason::HigherPriorityBudgetPressure,
                    });
                    self.queued_workloads.push(QueuedTrainWorkloadRecord {
                        workload: preempted.workload,
                        estimated_cost_units: preempted.estimated_cost_units,
                        total_work_units: preempted.total_work_units,
                        total_bytes: preempted.total_bytes,
                        queued_at_ms: workload.submitted_at_ms,
                    });
                }
            }
            self.sort_queue();
            let active_record = estimated.into_active_record(workload.submitted_at_ms);
            self.active_workloads.push(active_record.clone());
            return Ok(build_admission_receipt(
                &active_record,
                TrainAdmissionOutcome::AdmittedAfterPreemption,
                self.current_budget_usage(),
                preemptions,
                self.queued_workloads.len(),
            ));
        }

        let queued_record = estimated.into_queued_record(workload.submitted_at_ms);
        self.queued_workloads.push(queued_record.clone());
        self.sort_queue();
        Ok(build_queued_receipt(
            &queued_record,
            self.current_budget_usage(),
            self.queued_workloads.len(),
        ))
    }

    /// Completes one active workload and records actual cost units.
    pub fn complete_workload(
        &mut self,
        workload_id: &str,
        actual_cost_units: Option<u64>,
        completed_at_ms: u64,
    ) -> Result<TrainCompletionReceipt, TrainSchedulingAccountingError> {
        let index = self
            .active_workloads
            .iter()
            .position(|record| record.workload.workload_id == workload_id)
            .ok_or_else(|| TrainSchedulingAccountingError::UnknownActiveWorkload {
                workload_id: String::from(workload_id),
            })?;
        let completed = self.active_workloads.remove(index);
        let actual_cost_units = actual_cost_units.unwrap_or(completed.estimated_cost_units);
        self.completed_entries.push(TrainAccountingEntry {
            workload_id: completed.workload.workload_id.clone(),
            role: completed.workload.role,
            queue_class: completed.workload.queue_class,
            environment_key: completed.workload.provenance.environment_key.clone(),
            validator_scope_id: completed.workload.provenance.validator_scope_id.clone(),
            total_work_units: completed.total_work_units,
            total_bytes: completed.total_bytes,
            estimated_cost_units: completed.estimated_cost_units,
            actual_cost_units,
            completed_at_ms,
        });

        let admitted_from_queue = self.drain_queue(completed_at_ms)?;
        let receipt_digest = stable_completion_receipt_digest(
            completed.workload.workload_id.as_str(),
            completed.estimated_cost_units,
            actual_cost_units,
            admitted_from_queue.as_slice(),
        );
        Ok(TrainCompletionReceipt {
            workload_id: completed.workload.workload_id,
            estimated_cost_units: completed.estimated_cost_units,
            actual_cost_units,
            admitted_from_queue,
            receipt_digest,
        })
    }

    /// Completes one active sandbox workload using actual sandbox resource usage.
    pub fn complete_with_sandbox_receipt(
        &mut self,
        workload_id: &str,
        receipt: &ProviderSandboxExecutionReceipt,
    ) -> Result<TrainCompletionReceipt, TrainSchedulingAccountingError> {
        let actual_cost_units = receipt
            .evidence
            .resource_usage
            .wall_time_ms
            .saturating_add(receipt.evidence.resource_usage.stdout_bytes.div_ceil(1024))
            .saturating_add(receipt.evidence.resource_usage.stderr_bytes.div_ceil(1024))
            .saturating_add(
                receipt
                    .evidence
                    .resource_usage
                    .artifact_bytes
                    .div_ceil(1024),
            );
        let completed_at_ms = receipt.evidence.end_time_ms.max(0) as u64;
        self.complete_workload(workload_id, Some(actual_cost_units), completed_at_ms)
    }

    /// Returns a full snapshot of active, queued, and completed accounting truth.
    #[must_use]
    pub fn snapshot(&self) -> TrainSchedulingAccountingSnapshot {
        let budget_usage = self.current_budget_usage();

        let mut role_summaries = BTreeMap::<TrainWorkloadRole, TrainRoleAccountingSummary>::new();
        for record in &self.active_workloads {
            let summary =
                role_summaries
                    .entry(record.workload.role)
                    .or_insert(TrainRoleAccountingSummary {
                        role: record.workload.role,
                        active_workloads: 0,
                        active_estimated_cost_units: 0,
                        completed_workloads: 0,
                        completed_actual_cost_units: 0,
                        completed_work_units: 0,
                        completed_bytes: 0,
                    });
            summary.active_workloads = summary.active_workloads.saturating_add(1);
            summary.active_estimated_cost_units = summary
                .active_estimated_cost_units
                .saturating_add(record.estimated_cost_units);
        }
        let mut environment_summaries =
            BTreeMap::<String, TrainEnvironmentAccountingSummary>::new();
        let mut validator_summaries = BTreeMap::<String, TrainValidatorAccountingSummary>::new();
        for entry in &self.completed_entries {
            let summary = role_summaries
                .entry(entry.role)
                .or_insert(TrainRoleAccountingSummary {
                    role: entry.role,
                    active_workloads: 0,
                    active_estimated_cost_units: 0,
                    completed_workloads: 0,
                    completed_actual_cost_units: 0,
                    completed_work_units: 0,
                    completed_bytes: 0,
                });
            summary.completed_workloads = summary.completed_workloads.saturating_add(1);
            summary.completed_actual_cost_units = summary
                .completed_actual_cost_units
                .saturating_add(entry.actual_cost_units);
            summary.completed_work_units = summary
                .completed_work_units
                .saturating_add(entry.total_work_units);
            summary.completed_bytes = summary.completed_bytes.saturating_add(entry.total_bytes);

            if let Some(environment_key) = &entry.environment_key {
                let environment_summary = environment_summaries
                    .entry(environment_key.clone())
                    .or_insert(TrainEnvironmentAccountingSummary {
                        environment_key: environment_key.clone(),
                        completed_workloads: 0,
                        completed_actual_cost_units: 0,
                        completed_work_units: 0,
                        completed_bytes: 0,
                    });
                environment_summary.completed_workloads =
                    environment_summary.completed_workloads.saturating_add(1);
                environment_summary.completed_actual_cost_units = environment_summary
                    .completed_actual_cost_units
                    .saturating_add(entry.actual_cost_units);
                environment_summary.completed_work_units = environment_summary
                    .completed_work_units
                    .saturating_add(entry.total_work_units);
                environment_summary.completed_bytes = environment_summary
                    .completed_bytes
                    .saturating_add(entry.total_bytes);
            }

            if let Some(validator_scope_id) = &entry.validator_scope_id {
                let validator_summary = validator_summaries
                    .entry(validator_scope_id.clone())
                    .or_insert(TrainValidatorAccountingSummary {
                        validator_scope_id: validator_scope_id.clone(),
                        completed_workloads: 0,
                        completed_actual_cost_units: 0,
                        completed_work_units: 0,
                        completed_bytes: 0,
                    });
                validator_summary.completed_workloads =
                    validator_summary.completed_workloads.saturating_add(1);
                validator_summary.completed_actual_cost_units = validator_summary
                    .completed_actual_cost_units
                    .saturating_add(entry.actual_cost_units);
                validator_summary.completed_work_units = validator_summary
                    .completed_work_units
                    .saturating_add(entry.total_work_units);
                validator_summary.completed_bytes = validator_summary
                    .completed_bytes
                    .saturating_add(entry.total_bytes);
            }
        }

        let role_summaries = role_summaries.into_values().collect::<Vec<_>>();
        let environment_summaries = environment_summaries.into_values().collect::<Vec<_>>();
        let validator_summaries = validator_summaries.into_values().collect::<Vec<_>>();
        let snapshot_digest = stable_snapshot_digest(
            budget_usage,
            self.active_workloads.as_slice(),
            self.queued_workloads.as_slice(),
            role_summaries.as_slice(),
            environment_summaries.as_slice(),
            validator_summaries.as_slice(),
        );
        TrainSchedulingAccountingSnapshot {
            budget_usage,
            active_workloads: self.active_workloads.clone(),
            queued_workloads: self.queued_workloads.clone(),
            role_summaries,
            environment_summaries,
            validator_summaries,
            snapshot_digest,
        }
    }

    fn drain_queue(
        &mut self,
        observed_at_ms: u64,
    ) -> Result<Vec<TrainAdmissionReceipt>, TrainSchedulingAccountingError> {
        let mut admitted = Vec::new();
        let mut index = 0usize;
        while index < self.queued_workloads.len() {
            let candidate = self.queued_workloads[index].clone();
            let projected = self.current_budget_usage().with_added(
                candidate.total_work_units,
                candidate.total_bytes,
                candidate.estimated_cost_units,
            );
            if projected.fits_inside(self.policy.global_budget) {
                let queued = self.queued_workloads.remove(index);
                let active_record = ActiveTrainWorkloadRecord {
                    workload: queued.workload,
                    dispatch_plan: self
                        .dispatch_plan_for_queue(candidate.workload.queue_class)?
                        .plan_clone(&candidate)?,
                    estimated_cost_units: candidate.estimated_cost_units,
                    total_work_units: candidate.total_work_units,
                    total_bytes: candidate.total_bytes,
                    admitted_at_ms: observed_at_ms,
                };
                self.active_workloads.push(active_record.clone());
                admitted.push(build_admission_receipt(
                    &active_record,
                    TrainAdmissionOutcome::Admitted,
                    self.current_budget_usage(),
                    Vec::new(),
                    self.queued_workloads.len(),
                ));
                continue;
            }
            index = index.saturating_add(1);
        }
        Ok(admitted)
    }

    fn current_budget_usage(&self) -> TrainBudgetUsage {
        self.active_workloads
            .iter()
            .fold(TrainBudgetUsage::default(), |usage, record| {
                usage.with_added(
                    record.total_work_units,
                    record.total_bytes,
                    record.estimated_cost_units,
                )
            })
    }

    fn estimate_workload(
        &self,
        workload: &TrainScheduledWorkload,
    ) -> Result<EstimatedTrainWorkload, TrainSchedulingAccountingError> {
        if workload.runtime_items.is_empty() {
            return Err(TrainSchedulingAccountingError::EmptyRuntimeWork {
                workload_id: workload.workload_id.clone(),
            });
        }
        let queue_policy = self.queue_policy(workload.queue_class)?;
        let role_cost_rate = self
            .policy
            .role_cost_rates
            .get(&workload.role)
            .copied()
            .ok_or(TrainSchedulingAccountingError::MissingRoleCostRate {
                role: workload.role,
            })?;
        let dispatch_plan = RuntimeDispatchPlan::plan(
            queue_policy.dispatch_policy.clone(),
            &workload.runtime_items,
        );
        let total_work_units = workload
            .runtime_items
            .iter()
            .map(|item| item.work_units)
            .sum::<usize>();
        let total_bytes = workload
            .runtime_items
            .iter()
            .map(|item| item.bytes)
            .sum::<u64>();
        let estimated_cost_units = role_cost_rate
            .base_cost_units
            .saturating_add(
                dispatch_plan
                    .simulated_cost_units()
                    .saturating_mul(role_cost_rate.cost_units_per_dispatch_unit),
            )
            .saturating_add(
                total_bytes
                    .div_ceil(1024)
                    .saturating_mul(role_cost_rate.cost_units_per_kibibyte),
            );
        Ok(EstimatedTrainWorkload {
            workload: workload.clone(),
            dispatch_plan,
            estimated_cost_units,
            total_work_units,
            total_bytes,
        })
    }

    fn select_preemption_targets(
        &self,
        incoming_queue_policy: &TrainQueuePolicy,
        current_usage: TrainBudgetUsage,
        incoming: &EstimatedTrainWorkload,
    ) -> Result<Vec<String>, TrainSchedulingAccountingError> {
        if incoming_queue_policy.preemption_mode == TrainPreemptionMode::Never {
            return Ok(Vec::new());
        }
        let mut candidates = self
            .active_workloads
            .iter()
            .filter_map(|candidate| {
                let candidate_priority = self
                    .queue_policy(candidate.workload.queue_class)
                    .ok()?
                    .priority_bps;
                let preemptible = match incoming_queue_policy.preemption_mode {
                    TrainPreemptionMode::Never => false,
                    TrainPreemptionMode::LowerPriorityOnly => {
                        candidate_priority < incoming_queue_policy.priority_bps
                    }
                    TrainPreemptionMode::LowerOrEqualPriority => {
                        candidate_priority <= incoming_queue_policy.priority_bps
                    }
                };
                preemptible.then_some((candidate_priority, candidate))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.admitted_at_ms.cmp(&right.1.admitted_at_ms))
        });

        let mut usage = current_usage;
        let mut selected = Vec::new();
        usage = usage.with_added(
            incoming.total_work_units,
            incoming.total_bytes,
            incoming.estimated_cost_units,
        );
        if usage.fits_inside(self.policy.global_budget) {
            return Ok(Vec::new());
        }
        for (_, candidate) in candidates {
            usage.active_work_units = usage
                .active_work_units
                .saturating_sub(candidate.total_work_units);
            usage.active_bytes = usage.active_bytes.saturating_sub(candidate.total_bytes);
            usage.active_cost_units = usage
                .active_cost_units
                .saturating_sub(candidate.estimated_cost_units);
            selected.push(candidate.workload.workload_id.clone());
            if usage.fits_inside(self.policy.global_budget) {
                return Ok(selected);
            }
        }
        Ok(Vec::new())
    }

    fn queue_policy(
        &self,
        queue_class: TrainQueueClass,
    ) -> Result<&TrainQueuePolicy, TrainSchedulingAccountingError> {
        self.policy
            .queue_policies
            .get(&queue_class)
            .ok_or(TrainSchedulingAccountingError::MissingQueuePolicy { queue_class })
    }

    fn dispatch_plan_for_queue(
        &self,
        queue_class: TrainQueueClass,
    ) -> Result<DispatchPlanBuilder, TrainSchedulingAccountingError> {
        Ok(DispatchPlanBuilder {
            dispatch_policy: self.queue_policy(queue_class)?.dispatch_policy.clone(),
        })
    }

    fn sort_queue(&mut self) {
        self.queued_workloads.sort_by(|left, right| {
            let left_priority = self
                .policy
                .queue_policies
                .get(&left.workload.queue_class)
                .map_or(0, |policy| policy.priority_bps);
            let right_priority = self
                .policy
                .queue_policies
                .get(&right.workload.queue_class)
                .map_or(0, |policy| policy.priority_bps);
            right_priority.cmp(&left_priority).then_with(|| {
                left.workload
                    .submitted_at_ms
                    .cmp(&right.workload.submitted_at_ms)
            })
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EstimatedTrainWorkload {
    workload: TrainScheduledWorkload,
    dispatch_plan: RuntimeDispatchPlan,
    estimated_cost_units: u64,
    total_work_units: usize,
    total_bytes: u64,
}

impl EstimatedTrainWorkload {
    fn into_active_record(self, admitted_at_ms: u64) -> ActiveTrainWorkloadRecord {
        ActiveTrainWorkloadRecord {
            workload: self.workload,
            dispatch_plan: self.dispatch_plan,
            estimated_cost_units: self.estimated_cost_units,
            total_work_units: self.total_work_units,
            total_bytes: self.total_bytes,
            admitted_at_ms,
        }
    }

    fn into_queued_record(self, queued_at_ms: u64) -> QueuedTrainWorkloadRecord {
        QueuedTrainWorkloadRecord {
            workload: self.workload,
            estimated_cost_units: self.estimated_cost_units,
            total_work_units: self.total_work_units,
            total_bytes: self.total_bytes,
            queued_at_ms,
        }
    }
}

struct DispatchPlanBuilder {
    dispatch_policy: RuntimeDispatchPolicy,
}

impl DispatchPlanBuilder {
    fn plan_clone(
        &self,
        candidate: &QueuedTrainWorkloadRecord,
    ) -> Result<RuntimeDispatchPlan, TrainSchedulingAccountingError> {
        if candidate.workload.runtime_items.is_empty() {
            return Err(TrainSchedulingAccountingError::EmptyRuntimeWork {
                workload_id: candidate.workload.workload_id.clone(),
            });
        }
        Ok(RuntimeDispatchPlan::plan(
            self.dispatch_policy.clone(),
            &candidate.workload.runtime_items,
        ))
    }
}

fn build_admission_receipt(
    active_record: &ActiveTrainWorkloadRecord,
    outcome: TrainAdmissionOutcome,
    budget_usage: TrainBudgetUsage,
    preemptions: Vec<TrainPreemptionReceipt>,
    queue_depth: usize,
) -> TrainAdmissionReceipt {
    let receipt_digest = stable_admission_receipt_digest(
        active_record.workload.workload_id.as_str(),
        outcome,
        &active_record.dispatch_plan,
        active_record.estimated_cost_units,
        budget_usage,
        preemptions.as_slice(),
        queue_depth,
    );
    TrainAdmissionReceipt {
        workload_id: active_record.workload.workload_id.clone(),
        outcome,
        dispatch_plan: active_record.dispatch_plan.clone(),
        estimated_cost_units: active_record.estimated_cost_units,
        budget_usage,
        preemptions,
        queue_depth,
        receipt_digest,
    }
}

fn build_queued_receipt(
    queued_record: &QueuedTrainWorkloadRecord,
    budget_usage: TrainBudgetUsage,
    queue_depth: usize,
) -> TrainAdmissionReceipt {
    let dispatch_plan = RuntimeDispatchPlan::naive(&queued_record.workload.runtime_items, 1);
    let receipt_digest = stable_admission_receipt_digest(
        queued_record.workload.workload_id.as_str(),
        TrainAdmissionOutcome::Queued,
        &dispatch_plan,
        queued_record.estimated_cost_units,
        budget_usage,
        &[],
        queue_depth,
    );
    TrainAdmissionReceipt {
        workload_id: queued_record.workload.workload_id.clone(),
        outcome: TrainAdmissionOutcome::Queued,
        dispatch_plan,
        estimated_cost_units: queued_record.estimated_cost_units,
        budget_usage,
        preemptions: Vec::new(),
        queue_depth,
        receipt_digest,
    }
}

fn stable_admission_receipt_digest(
    workload_id: &str,
    outcome: TrainAdmissionOutcome,
    dispatch_plan: &RuntimeDispatchPlan,
    estimated_cost_units: u64,
    budget_usage: TrainBudgetUsage,
    preemptions: &[TrainPreemptionReceipt],
    queue_depth: usize,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_admission_receipt|");
    hasher.update(workload_id.as_bytes());
    hasher.update(b"|");
    hasher.update(train_admission_outcome_label(outcome).as_bytes());
    hasher.update(b"|");
    hasher.update(stable_json_bytes(dispatch_plan));
    hasher.update(b"|");
    hasher.update(estimated_cost_units.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(stable_json_bytes(&budget_usage));
    for preemption in preemptions {
        hasher.update(stable_json_bytes(preemption));
    }
    hasher.update(b"|");
    hasher.update(queue_depth.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_completion_receipt_digest(
    workload_id: &str,
    estimated_cost_units: u64,
    actual_cost_units: u64,
    admitted_from_queue: &[TrainAdmissionReceipt],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_completion_receipt|");
    hasher.update(workload_id.as_bytes());
    hasher.update(b"|");
    hasher.update(estimated_cost_units.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(actual_cost_units.to_string().as_bytes());
    for receipt in admitted_from_queue {
        hasher.update(stable_json_bytes(receipt));
    }
    hex::encode(hasher.finalize())
}

fn stable_snapshot_digest(
    budget_usage: TrainBudgetUsage,
    active_workloads: &[ActiveTrainWorkloadRecord],
    queued_workloads: &[QueuedTrainWorkloadRecord],
    role_summaries: &[TrainRoleAccountingSummary],
    environment_summaries: &[TrainEnvironmentAccountingSummary],
    validator_summaries: &[TrainValidatorAccountingSummary],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_scheduling_snapshot|");
    hasher.update(stable_json_bytes(&budget_usage));
    for record in active_workloads {
        hasher.update(stable_json_bytes(record));
    }
    for record in queued_workloads {
        hasher.update(stable_json_bytes(record));
    }
    for summary in role_summaries {
        hasher.update(stable_json_bytes(summary));
    }
    for summary in environment_summaries {
        hasher.update(stable_json_bytes(summary));
    }
    for summary in validator_summaries {
        hasher.update(stable_json_bytes(summary));
    }
    hex::encode(hasher.finalize())
}

fn stable_json_bytes(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("stable JSON serialization failed")
}

fn train_admission_outcome_label(outcome: TrainAdmissionOutcome) -> &'static str {
    match outcome {
        TrainAdmissionOutcome::Admitted => "admitted",
        TrainAdmissionOutcome::AdmittedAfterPreemption => "admitted_after_preemption",
        TrainAdmissionOutcome::Queued => "queued",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{PolicyRevision, RolloutSample, RolloutTerminationReason};

    #[test]
    fn higher_priority_eval_preempts_background_sandbox_under_budget_cap()
    -> Result<(), Box<dyn std::error::Error>> {
        let environment = EnvironmentPackageKey::new("weather.agent", "1.0.0");
        let policy = custom_policy(TrainBudgetCap::new(8, 256 * 1024 * 1024 + 2_000, 400_000))?;
        let mut controller = TrainSchedulingAccountingController::new(policy)?;

        let sandbox = TrainScheduledWorkload::for_sandbox_job(
            &sandbox_job_request("sbx-1", 256),
            &environment,
            TrainQueueClass::Background,
            None,
            1_000,
        );
        let sandbox_receipt = controller.admit(sandbox)?;
        assert_eq!(sandbox_receipt.outcome, TrainAdmissionOutcome::Admitted);

        let eval = TrainScheduledWorkload::for_validator_artifact(
            &EvalArtifact::new("held_out", "eval://weather/1", b"eval"),
            &environment,
            TrainQueueClass::Realtime,
            "validator.weather",
            1_100,
        );
        let eval_receipt = controller.admit(eval)?;
        assert_eq!(
            eval_receipt.outcome,
            TrainAdmissionOutcome::AdmittedAfterPreemption
        );
        assert_eq!(eval_receipt.preemptions.len(), 1);
        assert_eq!(controller.active_workloads.len(), 1);
        assert_eq!(controller.queued_workloads.len(), 1);
        assert_eq!(
            controller.active_workloads[0].workload.role,
            TrainWorkloadRole::Validator
        );
        assert_eq!(
            controller.queued_workloads[0].workload.role,
            TrainWorkloadRole::Sandbox
        );
        Ok(())
    }

    #[test]
    fn completion_drains_queue_and_tracks_environment_and_validator_costs()
    -> Result<(), Box<dyn std::error::Error>> {
        let environment = EnvironmentPackageKey::new("weather.agent", "1.0.0");
        let policy = custom_policy(TrainBudgetCap::new(8, 256 * 1024 * 1024 + 2_000, 400_000))?;
        let mut controller = TrainSchedulingAccountingController::new(policy)?;

        let trainer_batch = trainer_batch("batch-1", &environment)?;
        let trainer_workload = TrainScheduledWorkload::for_trainer_batch(
            &trainer_batch,
            &environment,
            TrainQueueClass::Standard,
            2_000,
        );
        let trainer_admission = controller.admit(trainer_workload)?;
        assert_eq!(trainer_admission.outcome, TrainAdmissionOutcome::Admitted);
        let trainer_completion = controller.complete_workload(
            trainer_admission.workload_id.as_str(),
            Some(240),
            2_500,
        )?;
        assert!(trainer_completion.admitted_from_queue.is_empty());

        let validator_workload = TrainScheduledWorkload::for_validator_artifact(
            &EvalArtifact::new("benchmark", "eval://weather/validator", b"validator"),
            &environment,
            TrainQueueClass::Realtime,
            "validator.weather",
            2_600,
        );
        let validator_admission = controller.admit(validator_workload)?;
        let queued_sandbox = TrainScheduledWorkload::for_sandbox_job(
            &sandbox_job_request("sbx-queued", 256),
            &environment,
            TrainQueueClass::Background,
            Some("validator.weather"),
            2_650,
        );
        let sandbox_admission = controller.admit(queued_sandbox)?;
        assert_eq!(sandbox_admission.outcome, TrainAdmissionOutcome::Queued);

        let validator_completion = controller.complete_workload(
            validator_admission.workload_id.as_str(),
            Some(160),
            2_900,
        )?;
        assert_eq!(validator_completion.admitted_from_queue.len(), 1);
        assert_eq!(controller.active_workloads.len(), 1);
        let sandbox_workload_id = controller.active_workloads[0].workload.workload_id.clone();
        let sandbox_completion = controller.complete_with_sandbox_receipt(
            sandbox_workload_id.as_str(),
            &sandbox_execution_receipt("sbx-queued", 3_000, 3_250, 180, 4_096),
        )?;
        assert!(sandbox_completion.actual_cost_units > 0);

        let snapshot = controller.snapshot();
        let trainer_summary = snapshot
            .role_summaries
            .iter()
            .find(|summary| summary.role == TrainWorkloadRole::Trainer)
            .expect("trainer summary");
        assert_eq!(trainer_summary.completed_actual_cost_units, 240);
        let validator_summary = snapshot
            .validator_summaries
            .iter()
            .find(|summary| summary.validator_scope_id == "validator.weather")
            .expect("validator summary");
        assert_eq!(validator_summary.completed_workloads, 2);
        let environment_summary = snapshot
            .environment_summaries
            .iter()
            .find(|summary| summary.environment_key == environment.storage_key())
            .expect("environment summary");
        assert_eq!(environment_summary.completed_workloads, 3);
        assert_eq!(
            environment_summary.completed_actual_cost_units,
            240 + 160 + sandbox_completion.actual_cost_units
        );
        Ok(())
    }

    #[test]
    fn constructors_preserve_role_and_provenance_for_train_workloads()
    -> Result<(), Box<dyn std::error::Error>> {
        let environment = EnvironmentPackageKey::new("weather.agent", "1.0.0");
        let rollout = sample_rollout("rollout-a", "worker-a", &environment);
        let rollout_workload =
            TrainScheduledWorkload::for_rollout_artifact(&rollout, TrainQueueClass::Bulk, 100);
        assert_eq!(rollout_workload.role, TrainWorkloadRole::Rollout);
        assert_eq!(
            rollout_workload.provenance.environment_key,
            Some(environment.storage_key())
        );

        let trainer_batch = trainer_batch("batch-a", &environment)?;
        let trainer_workload = TrainScheduledWorkload::for_trainer_batch(
            &trainer_batch,
            &environment,
            TrainQueueClass::Standard,
            200,
        );
        assert_eq!(trainer_workload.role, TrainWorkloadRole::Trainer);

        let eval_workload = TrainScheduledWorkload::for_eval_artifact(
            &EvalArtifact::new("held_out", "eval://weather/held-out", b"held-out"),
            &environment,
            TrainQueueClass::Realtime,
            Some("validator.weather"),
            300,
        );
        assert_eq!(eval_workload.role, TrainWorkloadRole::Eval);
        assert_eq!(
            eval_workload.provenance.validator_scope_id.as_deref(),
            Some("validator.weather")
        );
        assert_eq!(
            eval_workload.runtime_items[0].class,
            RuntimeWorkClass::EvalStep
        );
        Ok(())
    }

    fn custom_policy(
        global_budget: TrainBudgetCap,
    ) -> Result<TrainSchedulingAccountingPolicy, TrainSchedulingAccountingError> {
        let mut policy = TrainSchedulingAccountingPolicy::default();
        policy.global_budget = global_budget;
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

    fn sample_rollout(
        artifact_id: &str,
        worker_id: &str,
        environment: &EnvironmentPackageKey,
    ) -> RolloutArtifact {
        RolloutArtifact::new(
            artifact_id,
            worker_id,
            environment.clone(),
            "task-a",
            PolicyRevision::new("weather.policy", "rev-1", "policy-digest", 1_000)
                .with_revision_number(1),
            vec![RolloutSample::new(1, -0.2, 0.8, 0.6)],
            RolloutTerminationReason::Completed,
            Vec::new(),
            2_000,
        )
        .expect("rollout artifact should be valid")
    }

    fn trainer_batch(
        batch_id: &str,
        environment: &EnvironmentPackageKey,
    ) -> Result<TrainerBatch, Box<dyn std::error::Error>> {
        Ok(TrainerBatch::assemble(
            batch_id,
            PolicyRevision::new(
                "weather.policy",
                "rev-target",
                "policy-target-digest",
                3_000,
            )
            .with_revision_number(2),
            vec![sample_rollout("rollout-batch", "worker-b", environment)],
            4_000,
        )?)
    }

    fn sandbox_job_request(job_id: &str, memory_limit_mb: u64) -> ProviderSandboxJobRequest {
        ProviderSandboxJobRequest {
            job_id: String::from(job_id),
            provider_id: String::from("provider-1"),
            compute_product_id: String::from("sandbox.python.exec"),
            execution_class: ProviderSandboxExecutionClass::PythonExec,
            entrypoint_type: ProviderSandboxEntrypointType::InlinePayload,
            entrypoint: String::from("print('weather')"),
            payload: None,
            arguments: Vec::new(),
            workspace_root: PathBuf::from("."),
            expected_outputs: vec![String::from("weather.json")],
            timeout_request_s: 30,
            network_request: String::from("none"),
            filesystem_request: String::from("workspace_rw"),
            environment: vec![ProviderSandboxEnvironmentVar {
                key: String::from("MODE"),
                value: String::from("test"),
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

    fn sandbox_execution_receipt(
        job_id: &str,
        start_time_ms: i64,
        end_time_ms: i64,
        wall_time_ms: u64,
        artifact_bytes: u64,
    ) -> ProviderSandboxExecutionReceipt {
        ProviderSandboxExecutionReceipt {
            receipt_id: format!("receipt-{job_id}"),
            receipt_type: String::from("provider_sandbox_execution_receipt"),
            job_id: String::from(job_id),
            provider_id: String::from("provider-1"),
            compute_product_id: String::from("sandbox.python.exec"),
            final_state: ProviderSandboxExecutionState::Succeeded,
            evidence: ProviderSandboxDeliveryEvidence {
                evidence_id: format!("evidence-{job_id}"),
                profile_id: String::from("python"),
                profile_digest: String::from("profile-digest"),
                runtime_environment_digest: String::from("runtime-digest"),
                job_input_digest: String::from("input-digest"),
                entrypoint_digest: String::from("entrypoint-digest"),
                start_time_ms,
                end_time_ms,
                exit_code: Some(0),
                termination_reason: ProviderSandboxTerminationReason::CleanExit,
                stdout_digest: String::from("stdout-digest"),
                stderr_digest: String::from("stderr-digest"),
                artifact_digests: vec![ProviderSandboxArtifactDigest {
                    relative_path: String::from("weather.json"),
                    sha256_digest: String::from("artifact-digest"),
                    size_bytes: artifact_bytes,
                }],
                resource_usage: ProviderSandboxResourceUsageSummary {
                    wall_time_ms,
                    stdout_bytes: 512,
                    stderr_bytes: 0,
                    artifact_bytes,
                },
                payout_reference: None,
                verification_posture: Some(String::from("local")),
                state_trace: vec![ProviderSandboxStateTransition {
                    state: ProviderSandboxExecutionState::Succeeded,
                    observed_at_ms: end_time_ms,
                    detail: Some(String::from("finished")),
                }],
                policy_detail: Some(String::from("local")),
            },
        }
    }
}
