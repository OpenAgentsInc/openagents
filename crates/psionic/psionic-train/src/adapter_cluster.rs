use std::{
    collections::{BTreeMap, BTreeSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use psionic_cluster::{
    AdmissionToken, ClusterBackendReadinessStatus, ClusterId, ClusterMembershipRecord,
    ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
    ClusterSnapshot, ClusterStabilityPosture, ClusterState, NodeEpoch, NodeId, NodeRole,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AdapterContributionAssignmentSpec, AdapterContributionExecutionSummary,
    AdapterContributionUploadLocator, AdapterContributionValidatorDisposition,
    AdapterDatasetSliceIdentity, AdapterTargetIdentity, AdapterTrainingWindowStateMachine,
    AdapterWindowContractError, CheckpointPointer, PolicyRevision,
    TrainingContributorSuspensionReason, TrainingParticipantContributionState,
    TrainingParticipantDepartureReason, TrainingParticipantReadinessState, TrainingRunGraphError,
    TrainingRunState, TrainingWindowAssignmentRule, TrainingWindowStatus,
};

const GIB_BYTES: u64 = 1024 * 1024 * 1024;

/// Failure surfaced by the adapter cluster/window coordinator.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AdapterClusterCoordinationError {
    /// The underlying run graph rejected the requested transition.
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    /// The adapter window contract rejected the requested transition.
    #[error(transparent)]
    WindowContract(#[from] AdapterWindowContractError),
    /// One adapter window is still open.
    #[error(
        "adapter-cluster coordinator cannot plan a new window while `{window_id}` is still `{status}`"
    )]
    CurrentWindowOpen {
        /// Stable window identifier.
        window_id: String,
        /// Current status label.
        status: String,
    },
    /// The dataset-slice plan was empty.
    #[error("adapter-cluster window planning requires at least one dataset slice")]
    EmptyDatasetSlicePlan,
    /// The coordinator has no current window.
    #[error("adapter-cluster coordinator has no current window")]
    MissingCurrentWindow,
    /// The current window id did not exist in the adapter-window history.
    #[error("adapter-cluster coordinator does not know window `{window_id}`")]
    UnknownWindow {
        /// Stable window identifier.
        window_id: String,
    },
    /// The generic run graph got ahead of the adapter-window state machine.
    #[error(
        "adapter-cluster window `{window_id}` run graph is `{run_status}` but adapter window is `{adapter_status}`"
    )]
    RunGraphAheadOfAdapterWindow {
        /// Stable window identifier.
        window_id: String,
        /// Generic run-graph status label.
        run_status: String,
        /// Adapter-window status label.
        adapter_status: String,
    },
}

/// Capability and readiness contract for one adapter-training contributor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributorCapabilityPolicy {
    /// Backend label that must be ready in cluster telemetry.
    pub backend_label: String,
    /// Minimum free memory required for one contributor selection.
    pub minimum_free_memory_bytes: u64,
    /// Whether a visible accelerator is mandatory.
    pub require_accelerator: bool,
    /// Whether degraded backend readiness may still contribute.
    pub allow_degraded_backend: bool,
    /// Whether flaky nodes may still contribute.
    pub allow_flaky_nodes: bool,
}

impl Default for AdapterContributorCapabilityPolicy {
    fn default() -> Self {
        Self {
            backend_label: String::from("apple.foundation_models.adapter_train"),
            minimum_free_memory_bytes: 8 * GIB_BYTES,
            require_accelerator: true,
            allow_degraded_backend: false,
            allow_flaky_nodes: false,
        }
    }
}

/// Machine-legible contributor eligibility state for adapter windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributorEligibility {
    /// The participant satisfies current adapter-training prerequisites.
    Eligible,
    /// The participant disappeared from current cluster membership truth.
    MissingFromSnapshot,
    /// The participant is not in a ready cluster-membership state.
    MembershipNotReady,
    /// The participant role is not eligible for contributor work.
    RoleNotContributor,
    /// The participant did not expose authoritative telemetry.
    TelemetryMissing,
    /// The required backend is not ready on the node.
    BackendUnavailable,
    /// The backend is only degraded and policy currently refuses it.
    BackendDegraded,
    /// The node is not currently stable enough for contributor selection.
    NodeUnstable,
    /// The node did not expose the minimum required free memory.
    InsufficientFreeMemory,
    /// The policy requires an accelerator and none was visible.
    AcceleratorRequired,
}

/// Inspectable contributor posture for one observed cluster epoch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterClusterContributorStatus {
    /// Stable node identifier.
    pub node_id: String,
    /// Cluster role when visible in membership truth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<NodeRole>,
    /// Membership posture when visible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership_status: Option<ClusterMembershipStatus>,
    /// Backend readiness for the required backend label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_readiness: Option<ClusterBackendReadinessStatus>,
    /// Stability posture when telemetry existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<ClusterStabilityPosture>,
    /// Free memory when telemetry existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free_memory_bytes: Option<u64>,
    /// Accelerator count when telemetry existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accelerator_count: Option<u16>,
    /// Current adapter-training eligibility state.
    pub eligibility: AdapterContributorEligibility,
    /// Deterministic contributor priority derived from telemetry.
    pub priority_bps: u16,
    /// Deterministic contributor reliability derived from telemetry.
    pub reliability_bps: u16,
    /// Current contribution posture in the run graph.
    pub contribution_state: TrainingParticipantContributionState,
}

/// One inspectable membership epoch tying cluster truth to adapter selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterClusterMembershipReceipt {
    /// Stable run identifier.
    pub training_run_id: String,
    /// Stable stage identifier.
    pub stage_id: String,
    /// Stable topology revision id from the run graph.
    pub topology_revision_id: String,
    /// Currently selected contributor-set revision when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributor_set_revision_id: Option<String>,
    /// Current planned or active window when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_window_id: Option<String>,
    /// Contributors currently eligible for the next adapter window.
    pub eligible_node_ids: Vec<String>,
    /// Contributors currently blocked from the next adapter window.
    pub blocked_node_ids: Vec<String>,
    /// Contributors selected in the latest contributor-set revision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_node_ids: Vec<String>,
    /// Contributors standing by in the latest contributor-set revision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub standby_node_ids: Vec<String>,
    /// Inspectable per-node posture for this cluster epoch.
    pub contributor_statuses: Vec<AdapterClusterContributorStatus>,
    /// Observation time.
    pub observed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Inspectable window plan tying cluster selection to one adapter window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterClusterWindowPlanReceipt {
    /// Stable run identifier.
    pub training_run_id: String,
    /// Stable stage identifier.
    pub stage_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Topology revision used for planning.
    pub topology_revision_id: String,
    /// Contributor-set revision used for planning.
    pub contributor_set_revision_id: String,
    /// Deterministic assignment seed derived from planning truth.
    pub assignment_seed: u64,
    /// Selected contributor node ids in deterministic order.
    pub selected_node_ids: Vec<String>,
    /// Standby contributor node ids retained out of the active set.
    pub standby_node_ids: Vec<String>,
    /// Dataset slices assigned into this window.
    pub dataset_slices: Vec<AdapterDatasetSliceIdentity>,
    /// Adapter target bound to the window.
    pub adapter_target: AdapterTargetIdentity,
    /// Input policy revision consumed by the window.
    pub input_policy_revision: PolicyRevision,
    /// Input checkpoint pointer consumed by the window.
    pub input_checkpoint_pointer: CheckpointPointer,
    /// Plan timestamp.
    pub planned_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// One adapter window plus the cluster-backed plan that produced it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterClusterWindowRecord {
    /// Cluster-backed plan receipt.
    pub plan: AdapterClusterWindowPlanReceipt,
    /// Typed adapter-window state machine.
    pub window: AdapterTrainingWindowStateMachine,
}

/// Cluster-backed adapter window coordinator layered over the generic run graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterTrainingClusterCoordinator {
    /// Generic run graph the adapter lane reuses.
    pub run: TrainingRunState,
    /// Adapter target family this coordinator owns.
    pub adapter_target: AdapterTargetIdentity,
    /// Current policy revision consumed by planned windows.
    pub current_policy_revision: PolicyRevision,
    /// Current checkpoint pointer consumed by planned windows.
    pub current_checkpoint_pointer: CheckpointPointer,
    /// Contributor capability policy for adapter windows.
    pub capability_policy: AdapterContributorCapabilityPolicy,
    /// Current open window when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_window_id: Option<String>,
    /// Window history owned by the adapter coordinator.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<AdapterClusterWindowRecord>,
    /// Membership receipts tying cluster snapshots to contributor posture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub membership_receipts: Vec<AdapterClusterMembershipReceipt>,
}

impl AdapterTrainingClusterCoordinator {
    /// Creates a cluster-backed adapter window coordinator.
    #[must_use]
    pub fn new(
        run: TrainingRunState,
        adapter_target: AdapterTargetIdentity,
        current_policy_revision: PolicyRevision,
        current_checkpoint_pointer: CheckpointPointer,
        capability_policy: AdapterContributorCapabilityPolicy,
    ) -> Self {
        Self {
            run,
            adapter_target,
            current_policy_revision,
            current_checkpoint_pointer,
            capability_policy,
            current_window_id: None,
            windows: Vec::new(),
            membership_receipts: Vec::new(),
        }
    }

    /// Mirrors one live cluster snapshot into adapter contributor eligibility.
    pub fn observe_cluster_state(
        &mut self,
        cluster_state: &ClusterState,
        observed_at_ms: u64,
    ) -> Result<&AdapterClusterMembershipReceipt, AdapterClusterCoordinationError> {
        let topology_revision_id = self
            .run
            .apply_cluster_membership_snapshot(cluster_state, observed_at_ms)?
            .topology_revision_id
            .clone();
        let active_contributors = self.current_window_contributors();
        let mut contributor_statuses = Vec::new();

        for participant in &mut self.run.participants {
            let membership = cluster_state.memberships().get(&participant.node_id);
            let telemetry = cluster_state.telemetry().get(&participant.node_id);
            let evaluation = evaluate_contributor(
                membership,
                telemetry,
                &self.capability_policy,
                participant.node_id.as_str(),
            );

            match evaluation.eligibility {
                AdapterContributorEligibility::Eligible => {
                    participant.priority_bps = evaluation.priority_bps;
                    participant.reliability_bps = evaluation.reliability_bps;
                    if participant.contributor_suspension_reason
                        == Some(TrainingContributorSuspensionReason::CapabilityPrerequisiteMissing)
                    {
                        participant.contributor_suspension_reason = None;
                    }
                    if participant.readiness_state == TrainingParticipantReadinessState::Ready
                        && participant.contribution_state
                            == TrainingParticipantContributionState::Suspended
                        && participant.contributor_suspension_reason.is_none()
                    {
                        participant.contribution_state =
                            TrainingParticipantContributionState::Standby;
                    }
                }
                AdapterContributorEligibility::MissingFromSnapshot => {
                    participant.readiness_state = TrainingParticipantReadinessState::Unready;
                    participant.last_departure_reason =
                        Some(TrainingParticipantDepartureReason::TimedOut);
                    participant.priority_bps = 0;
                    participant.reliability_bps = 0;
                    participant.contributor_suspension_reason =
                        Some(TrainingContributorSuspensionReason::CapabilityPrerequisiteMissing);
                    if !active_contributors.contains(participant.node_id.as_str()) {
                        participant.contribution_state =
                            TrainingParticipantContributionState::Suspended;
                    }
                }
                _ => {
                    participant.priority_bps = 0;
                    participant.reliability_bps = 0;
                    participant.contributor_suspension_reason =
                        Some(TrainingContributorSuspensionReason::CapabilityPrerequisiteMissing);
                    if !active_contributors.contains(participant.node_id.as_str()) {
                        participant.contribution_state =
                            TrainingParticipantContributionState::Suspended;
                    }
                }
            }

            contributor_statuses.push(AdapterClusterContributorStatus {
                node_id: String::from(participant.node_id.as_str()),
                role: membership.map(|record| record.identity.role),
                membership_status: membership.map(|record| record.status),
                backend_readiness: evaluation.backend_readiness,
                stability: evaluation.stability,
                free_memory_bytes: evaluation.free_memory_bytes,
                accelerator_count: evaluation.accelerator_count,
                eligibility: evaluation.eligibility,
                priority_bps: participant.priority_bps,
                reliability_bps: participant.reliability_bps,
                contribution_state: participant.contribution_state,
            });
        }

        contributor_statuses.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        let eligible_node_ids = contributor_statuses
            .iter()
            .filter(|status| status.eligibility == AdapterContributorEligibility::Eligible)
            .map(|status| status.node_id.clone())
            .collect::<Vec<_>>();
        let blocked_node_ids = contributor_statuses
            .iter()
            .filter(|status| status.eligibility != AdapterContributorEligibility::Eligible)
            .map(|status| status.node_id.clone())
            .collect::<Vec<_>>();
        let latest_contributor_set = self.run.contributor_set_revisions.last();
        let contributor_set_revision_id =
            latest_contributor_set.map(|revision| revision.contributor_set_revision_id.clone());
        let selected_node_ids = latest_contributor_set
            .map(|revision| revision.contributor_node_ids.clone())
            .unwrap_or_default();
        let standby_node_ids = latest_contributor_set
            .map(|revision| revision.standby_node_ids.clone())
            .unwrap_or_default();
        let receipt_digest = stable_membership_receipt_digest(
            self.run.run_id.as_str(),
            self.run.stage_id.as_str(),
            topology_revision_id.as_str(),
            contributor_set_revision_id.as_deref(),
            self.current_window_id.as_deref(),
            eligible_node_ids.as_slice(),
            blocked_node_ids.as_slice(),
            contributor_statuses.as_slice(),
            observed_at_ms,
        );
        self.membership_receipts
            .push(AdapterClusterMembershipReceipt {
                training_run_id: self.run.run_id.clone(),
                stage_id: self.run.stage_id.clone(),
                topology_revision_id,
                contributor_set_revision_id,
                current_window_id: self.current_window_id.clone(),
                eligible_node_ids,
                blocked_node_ids,
                selected_node_ids,
                standby_node_ids,
                contributor_statuses,
                observed_at_ms,
                receipt_digest,
            });
        self.membership_receipts
            .last()
            .ok_or(AdapterClusterCoordinationError::MissingCurrentWindow)
    }

    /// Plans one adapter window from the current admitted contributor population.
    pub fn plan_next_window(
        &mut self,
        dataset_slices: Vec<AdapterDatasetSliceIdentity>,
        max_contributors: usize,
        planned_at_ms: u64,
    ) -> Result<AdapterClusterWindowRecord, AdapterClusterCoordinationError> {
        if let Some(current_window_id) = &self.current_window_id {
            let status = self.window_record(current_window_id)?.window.status;
            if status != TrainingWindowStatus::Reconciled {
                return Err(AdapterClusterCoordinationError::CurrentWindowOpen {
                    window_id: current_window_id.clone(),
                    status: training_window_status_label(status).to_string(),
                });
            }
        }
        if dataset_slices.is_empty() {
            return Err(AdapterClusterCoordinationError::EmptyDatasetSlicePlan);
        }

        let contributor_target = max_contributors.max(1).min(dataset_slices.len());
        let contributor_set_revision = self
            .run
            .select_contributors(contributor_target, planned_at_ms)?
            .clone();
        let generic_window = self
            .run
            .plan_window(
                Some(self.current_policy_revision.revision_id.clone()),
                TrainingWindowAssignmentRule::RoundRobinByPriority {
                    batch_slice_count: contributor_target as u32,
                    eval_slice_count: 0,
                },
                planned_at_ms,
            )?
            .clone();
        let planned_dataset_slices = dataset_slices
            .into_iter()
            .take(contributor_set_revision.contributor_node_ids.len())
            .collect::<Vec<_>>();
        let assignment_seed = stable_assignment_seed(
            self.run.run_id.as_str(),
            self.run.stage_id.as_str(),
            generic_window.topology_revision_id.as_str(),
            contributor_set_revision
                .contributor_set_revision_id
                .as_str(),
            self.adapter_target.adapter_target_id.as_str(),
            self.current_policy_revision.revision_id.as_str(),
            planned_dataset_slices.as_slice(),
        );
        let assignments = generic_window
            .batch_assignments
            .iter()
            .zip(planned_dataset_slices.iter().cloned())
            .map(|(assignment, dataset_slice)| {
                AdapterContributionAssignmentSpec::new(assignment.node_id.clone(), dataset_slice, 0)
            })
            .collect::<Result<Vec<_>, AdapterWindowContractError>>()?;
        let window = AdapterTrainingWindowStateMachine::new(
            self.run.run_id.clone(),
            self.run.stage_id.clone(),
            generic_window.window_id.clone(),
            contributor_set_revision.contributor_set_revision_id.clone(),
            self.adapter_target.clone(),
            self.current_policy_revision.clone(),
            self.current_checkpoint_pointer.clone(),
            assignments,
            planned_at_ms,
        )?;
        let receipt_digest = stable_window_plan_digest(
            self.run.run_id.as_str(),
            self.run.stage_id.as_str(),
            generic_window.window_id.as_str(),
            generic_window.topology_revision_id.as_str(),
            contributor_set_revision
                .contributor_set_revision_id
                .as_str(),
            assignment_seed,
            contributor_set_revision.contributor_node_ids.as_slice(),
            contributor_set_revision.standby_node_ids.as_slice(),
            planned_dataset_slices.as_slice(),
            self.adapter_target.adapter_target_id.as_str(),
            self.current_policy_revision.revision_id.as_str(),
            self.current_checkpoint_pointer.pointer_digest.as_str(),
            planned_at_ms,
        );
        let record = AdapterClusterWindowRecord {
            plan: AdapterClusterWindowPlanReceipt {
                training_run_id: self.run.run_id.clone(),
                stage_id: self.run.stage_id.clone(),
                window_id: generic_window.window_id.clone(),
                topology_revision_id: generic_window.topology_revision_id.clone(),
                contributor_set_revision_id: contributor_set_revision
                    .contributor_set_revision_id
                    .clone(),
                assignment_seed,
                selected_node_ids: contributor_set_revision.contributor_node_ids.clone(),
                standby_node_ids: contributor_set_revision.standby_node_ids.clone(),
                dataset_slices: planned_dataset_slices,
                adapter_target: self.adapter_target.clone(),
                input_policy_revision: self.current_policy_revision.clone(),
                input_checkpoint_pointer: self.current_checkpoint_pointer.clone(),
                planned_at_ms,
                receipt_digest,
            },
            window,
        };
        self.current_window_id = Some(record.plan.window_id.clone());
        self.windows.push(record.clone());
        Ok(record)
    }

    /// Activates the current adapter window and the matching run-graph window.
    pub fn activate_current_window(
        &mut self,
        active_at_ms: u64,
    ) -> Result<(), AdapterClusterCoordinationError> {
        let current_window_id = self
            .current_window_id
            .clone()
            .ok_or(AdapterClusterCoordinationError::MissingCurrentWindow)?;
        self.run.transition_window(
            current_window_id.as_str(),
            TrainingWindowStatus::Active,
            active_at_ms,
        )?;
        self.window_record_mut(current_window_id.as_str())?
            .window
            .activate()?;
        Ok(())
    }

    /// Returns the current mutable adapter window.
    pub fn current_window_mut(
        &mut self,
    ) -> Result<&mut AdapterTrainingWindowStateMachine, AdapterClusterCoordinationError> {
        let current_window_id = self
            .current_window_id
            .clone()
            .ok_or(AdapterClusterCoordinationError::MissingCurrentWindow)?;
        Ok(&mut self.window_record_mut(current_window_id.as_str())?.window)
    }

    /// Advances the generic run graph until it matches the current adapter-window status.
    pub fn synchronize_current_window_status(
        &mut self,
        occurred_at_ms: u64,
    ) -> Result<(), AdapterClusterCoordinationError> {
        let current_window_id = self
            .current_window_id
            .clone()
            .ok_or(AdapterClusterCoordinationError::MissingCurrentWindow)?;
        let adapter_status = self
            .window_record(current_window_id.as_str())?
            .window
            .status;
        loop {
            let run_status = self
                .run
                .windows
                .iter()
                .find(|window| window.window_id == current_window_id)
                .ok_or_else(|| AdapterClusterCoordinationError::UnknownWindow {
                    window_id: current_window_id.clone(),
                })?
                .status;
            if run_status == adapter_status {
                break;
            }
            if training_window_status_rank(run_status) > training_window_status_rank(adapter_status)
            {
                return Err(
                    AdapterClusterCoordinationError::RunGraphAheadOfAdapterWindow {
                        window_id: current_window_id,
                        run_status: training_window_status_label(run_status).to_string(),
                        adapter_status: training_window_status_label(adapter_status).to_string(),
                    },
                );
            }
            let next_status = next_training_window_status(run_status).expect(
                "run status rank only advances while adapter window is further along in the state machine",
            );
            self.run
                .transition_window(current_window_id.as_str(), next_status, occurred_at_ms)?;
        }
        if adapter_status == TrainingWindowStatus::Reconciled {
            self.current_window_id = None;
        }
        Ok(())
    }

    fn current_window_contributors(&self) -> BTreeSet<String> {
        self.current_window_id
            .as_deref()
            .and_then(|window_id| {
                self.run
                    .windows
                    .iter()
                    .find(|window| window.window_id == window_id)
            })
            .map(|window| {
                window
                    .batch_assignments
                    .iter()
                    .map(|assignment| assignment.node_id.clone())
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default()
    }

    fn window_record(
        &self,
        window_id: &str,
    ) -> Result<&AdapterClusterWindowRecord, AdapterClusterCoordinationError> {
        self.windows
            .iter()
            .find(|record| record.plan.window_id == window_id)
            .ok_or_else(|| AdapterClusterCoordinationError::UnknownWindow {
                window_id: window_id.to_string(),
            })
    }

    fn window_record_mut(
        &mut self,
        window_id: &str,
    ) -> Result<&mut AdapterClusterWindowRecord, AdapterClusterCoordinationError> {
        self.windows
            .iter_mut()
            .find(|record| record.plan.window_id == window_id)
            .ok_or_else(|| AdapterClusterCoordinationError::UnknownWindow {
                window_id: window_id.to_string(),
            })
    }
}

#[derive(Clone, Copy)]
struct ContributorEvaluation {
    eligibility: AdapterContributorEligibility,
    backend_readiness: Option<ClusterBackendReadinessStatus>,
    stability: Option<ClusterStabilityPosture>,
    free_memory_bytes: Option<u64>,
    accelerator_count: Option<u16>,
    priority_bps: u16,
    reliability_bps: u16,
}

fn evaluate_contributor(
    membership: Option<&ClusterMembershipRecord>,
    telemetry: Option<&ClusterNodeTelemetry>,
    policy: &AdapterContributorCapabilityPolicy,
    node_id: &str,
) -> ContributorEvaluation {
    let Some(membership) = membership else {
        return ContributorEvaluation {
            eligibility: AdapterContributorEligibility::MissingFromSnapshot,
            backend_readiness: None,
            stability: None,
            free_memory_bytes: None,
            accelerator_count: None,
            priority_bps: 0,
            reliability_bps: 0,
        };
    };
    if membership.status != ClusterMembershipStatus::Ready {
        return ContributorEvaluation {
            eligibility: AdapterContributorEligibility::MembershipNotReady,
            backend_readiness: None,
            stability: None,
            free_memory_bytes: None,
            accelerator_count: None,
            priority_bps: 0,
            reliability_bps: 0,
        };
    }
    if membership.identity.role == NodeRole::CoordinatorOnly {
        return ContributorEvaluation {
            eligibility: AdapterContributorEligibility::RoleNotContributor,
            backend_readiness: None,
            stability: None,
            free_memory_bytes: None,
            accelerator_count: None,
            priority_bps: 0,
            reliability_bps: 0,
        };
    }
    let Some(telemetry) = telemetry else {
        return ContributorEvaluation {
            eligibility: AdapterContributorEligibility::TelemetryMissing,
            backend_readiness: None,
            stability: None,
            free_memory_bytes: None,
            accelerator_count: None,
            priority_bps: 0,
            reliability_bps: 0,
        };
    };
    let backend_readiness = telemetry
        .backend_readiness
        .get(&policy.backend_label)
        .copied()
        .unwrap_or(ClusterBackendReadinessStatus::Unknown);
    let free_memory_bytes = telemetry.free_memory_bytes;
    let accelerator_count = telemetry.accelerator_count;
    let stability = Some(telemetry.stability);
    let eligibility = match backend_readiness {
        ClusterBackendReadinessStatus::Ready => {
            if !policy.allow_flaky_nodes && telemetry.stability == ClusterStabilityPosture::Flaky {
                AdapterContributorEligibility::NodeUnstable
            } else if telemetry.stability == ClusterStabilityPosture::Unstable {
                AdapterContributorEligibility::NodeUnstable
            } else if policy.require_accelerator
                && telemetry.accelerator_count.unwrap_or_default() == 0
            {
                AdapterContributorEligibility::AcceleratorRequired
            } else if telemetry.free_memory_bytes.unwrap_or_default()
                < policy.minimum_free_memory_bytes
            {
                AdapterContributorEligibility::InsufficientFreeMemory
            } else {
                AdapterContributorEligibility::Eligible
            }
        }
        ClusterBackendReadinessStatus::Degraded if policy.allow_degraded_backend => {
            if policy.require_accelerator && telemetry.accelerator_count.unwrap_or_default() == 0 {
                AdapterContributorEligibility::AcceleratorRequired
            } else if telemetry.free_memory_bytes.unwrap_or_default()
                < policy.minimum_free_memory_bytes
            {
                AdapterContributorEligibility::InsufficientFreeMemory
            } else {
                AdapterContributorEligibility::Eligible
            }
        }
        ClusterBackendReadinessStatus::Degraded => AdapterContributorEligibility::BackendDegraded,
        ClusterBackendReadinessStatus::Refused | ClusterBackendReadinessStatus::Unknown => {
            AdapterContributorEligibility::BackendUnavailable
        }
    };
    let (priority_bps, reliability_bps) = if eligibility == AdapterContributorEligibility::Eligible
    {
        (
            contributor_priority_bps(telemetry, membership.identity.role, node_id),
            contributor_reliability_bps(telemetry, backend_readiness),
        )
    } else {
        (0, 0)
    };
    ContributorEvaluation {
        eligibility,
        backend_readiness: Some(backend_readiness),
        stability,
        free_memory_bytes,
        accelerator_count,
        priority_bps,
        reliability_bps,
    }
}

fn contributor_priority_bps(
    telemetry: &ClusterNodeTelemetry,
    role: NodeRole,
    node_id: &str,
) -> u16 {
    let free_memory_gib = telemetry.free_memory_bytes.unwrap_or_default() / GIB_BYTES;
    let accelerator_score =
        u16::from(telemetry.accelerator_count.unwrap_or_default().min(4)).saturating_mul(1_250);
    let memory_score = u16::try_from(free_memory_gib.min(32))
        .unwrap_or(32)
        .saturating_mul(150);
    let role_bonus = if role == NodeRole::Mixed { 200 } else { 0 };
    let node_tiebreak_bonus = node_id
        .bytes()
        .fold(0_u16, |acc, byte| acc.wrapping_add(u16::from(byte)))
        % 37;
    500_u16
        .saturating_add(accelerator_score)
        .saturating_add(memory_score)
        .saturating_add(role_bonus)
        .saturating_add(node_tiebreak_bonus)
        .min(10_000)
}

fn contributor_reliability_bps(
    telemetry: &ClusterNodeTelemetry,
    backend_readiness: ClusterBackendReadinessStatus,
) -> u16 {
    let backend_score: u16 = match backend_readiness {
        ClusterBackendReadinessStatus::Ready => 9_500,
        ClusterBackendReadinessStatus::Degraded => 7_000,
        ClusterBackendReadinessStatus::Refused | ClusterBackendReadinessStatus::Unknown => 0,
    };
    let stability_penalty = match telemetry.stability {
        ClusterStabilityPosture::Stable => 0,
        ClusterStabilityPosture::Flaky => 2_000,
        ClusterStabilityPosture::Unstable => 6_000,
    };
    backend_score.saturating_sub(stability_penalty)
}

fn training_window_status_rank(status: TrainingWindowStatus) -> u8 {
    match status {
        TrainingWindowStatus::Planned => 0,
        TrainingWindowStatus::Active => 1,
        TrainingWindowStatus::Sealed => 2,
        TrainingWindowStatus::Scored => 3,
        TrainingWindowStatus::Reconciled => 4,
    }
}

fn next_training_window_status(status: TrainingWindowStatus) -> Option<TrainingWindowStatus> {
    match status {
        TrainingWindowStatus::Planned => Some(TrainingWindowStatus::Active),
        TrainingWindowStatus::Active => Some(TrainingWindowStatus::Sealed),
        TrainingWindowStatus::Sealed => Some(TrainingWindowStatus::Scored),
        TrainingWindowStatus::Scored => Some(TrainingWindowStatus::Reconciled),
        TrainingWindowStatus::Reconciled => None,
    }
}

fn stable_membership_receipt_digest(
    training_run_id: &str,
    stage_id: &str,
    topology_revision_id: &str,
    contributor_set_revision_id: Option<&str>,
    current_window_id: Option<&str>,
    eligible_node_ids: &[String],
    blocked_node_ids: &[String],
    contributor_statuses: &[AdapterClusterContributorStatus],
    observed_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        "adapter_cluster_membership_receipt",
        training_run_id,
        stage_id,
        topology_revision_id,
        contributor_set_revision_id.unwrap_or("-"),
        current_window_id.unwrap_or("-"),
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    for node_id in eligible_node_ids {
        hasher.update(b"eligible|");
        hasher.update(node_id.as_bytes());
        hasher.update(b"|");
    }
    for node_id in blocked_node_ids {
        hasher.update(b"blocked|");
        hasher.update(node_id.as_bytes());
        hasher.update(b"|");
    }
    for status in contributor_statuses {
        for part in [
            status.node_id.as_str(),
            adapter_contributor_eligibility_label(status.eligibility),
            training_contribution_state_label(status.contribution_state),
        ] {
            hasher.update(part.as_bytes());
            hasher.update(b"|");
        }
        hasher.update(status.priority_bps.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(status.reliability_bps.to_string().as_bytes());
        hasher.update(b"|");
    }
    hasher.update(observed_at_ms.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stable_assignment_seed(
    training_run_id: &str,
    stage_id: &str,
    topology_revision_id: &str,
    contributor_set_revision_id: &str,
    adapter_target_id: &str,
    policy_revision_id: &str,
    dataset_slices: &[AdapterDatasetSliceIdentity],
) -> u64 {
    let mut hasher = Sha256::new();
    for part in [
        "adapter_cluster_assignment_seed",
        training_run_id,
        stage_id,
        topology_revision_id,
        contributor_set_revision_id,
        adapter_target_id,
        policy_revision_id,
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    for slice in dataset_slices {
        hasher.update(slice.slice_digest.as_bytes());
        hasher.update(b"|");
    }
    let digest = hasher.finalize();
    let mut seed_bytes = [0_u8; 8];
    seed_bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(seed_bytes)
}

fn stable_window_plan_digest(
    training_run_id: &str,
    stage_id: &str,
    window_id: &str,
    topology_revision_id: &str,
    contributor_set_revision_id: &str,
    assignment_seed: u64,
    selected_node_ids: &[String],
    standby_node_ids: &[String],
    dataset_slices: &[AdapterDatasetSliceIdentity],
    adapter_target_id: &str,
    policy_revision_id: &str,
    checkpoint_pointer_digest: &str,
    planned_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        "adapter_cluster_window_plan",
        training_run_id,
        stage_id,
        window_id,
        topology_revision_id,
        contributor_set_revision_id,
        adapter_target_id,
        policy_revision_id,
        checkpoint_pointer_digest,
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    hasher.update(assignment_seed.to_string().as_bytes());
    hasher.update(b"|");
    for node_id in selected_node_ids {
        hasher.update(b"selected|");
        hasher.update(node_id.as_bytes());
        hasher.update(b"|");
    }
    for node_id in standby_node_ids {
        hasher.update(b"standby|");
        hasher.update(node_id.as_bytes());
        hasher.update(b"|");
    }
    for slice in dataset_slices {
        hasher.update(slice.slice_digest.as_bytes());
        hasher.update(b"|");
    }
    hasher.update(planned_at_ms.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn adapter_contributor_eligibility_label(
    eligibility: AdapterContributorEligibility,
) -> &'static str {
    match eligibility {
        AdapterContributorEligibility::Eligible => "eligible",
        AdapterContributorEligibility::MissingFromSnapshot => "missing_from_snapshot",
        AdapterContributorEligibility::MembershipNotReady => "membership_not_ready",
        AdapterContributorEligibility::RoleNotContributor => "role_not_contributor",
        AdapterContributorEligibility::TelemetryMissing => "telemetry_missing",
        AdapterContributorEligibility::BackendUnavailable => "backend_unavailable",
        AdapterContributorEligibility::BackendDegraded => "backend_degraded",
        AdapterContributorEligibility::NodeUnstable => "node_unstable",
        AdapterContributorEligibility::InsufficientFreeMemory => "insufficient_free_memory",
        AdapterContributorEligibility::AcceleratorRequired => "accelerator_required",
    }
}

fn training_contribution_state_label(state: TrainingParticipantContributionState) -> &'static str {
    match state {
        TrainingParticipantContributionState::Standby => "standby",
        TrainingParticipantContributionState::Selected => "selected",
        TrainingParticipantContributionState::Active => "active",
        TrainingParticipantContributionState::Suspended => "suspended",
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

/// Result of the adapter cluster membership-churn reference harness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterClusterMembershipHarnessReceipt {
    /// Final coordinator state after the harness completed.
    pub coordinator: AdapterTrainingClusterCoordinator,
    /// First membership receipt before planning.
    pub initial_membership: AdapterClusterMembershipReceipt,
    /// Membership receipt captured under churn.
    pub churned_membership: AdapterClusterMembershipReceipt,
    /// First planned window.
    pub first_window: AdapterClusterWindowPlanReceipt,
    /// Second planned window after churn and reconcile.
    pub second_window: AdapterClusterWindowPlanReceipt,
    /// Stable digest over the harness outcome.
    pub receipt_digest: String,
}

/// Runs one deterministic cluster-membership churn harness for adapter windows.
pub fn run_adapter_cluster_membership_harness()
-> Result<AdapterClusterMembershipHarnessReceipt, AdapterClusterCoordinationError> {
    let initial_cluster = cluster_state(
        &[
            (
                "trainer-a",
                NodeRole::CoordinatorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-a",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-b",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-c",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
        ],
        &[
            ("trainer-a", 24, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-a", 20, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-b", 28, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-c", 6, 1, ClusterBackendReadinessStatus::Ready),
        ],
    );
    let adapter_target = AdapterTargetIdentity::new(
        "apple.weather.adapter",
        "apple.foundation_models",
        "apple://foundation-model/base",
        "apple.fmadapter",
    )?;
    let policy_revision = PolicyRevision::new(
        "apple.weather.policy",
        "policy-r7",
        "policy-digest-r7",
        1_000,
    )
    .with_revision_number(7)
    .with_checkpoint(harness_checkpoint_reference("checkpoint/weather/r7", 1_000));
    let checkpoint_pointer = CheckpointPointer::new(
        crate::CheckpointScopeBinding::new(crate::CheckpointScopeKind::Window, "window-weather-1"),
        "apple.weather.policy",
        harness_checkpoint_reference("checkpoint/weather/r7", 1_000).with_durable_at_ms(1_001),
        "manifest-digest-r7",
        1_001,
    )
    .expect("harness checkpoint pointer should validate");
    let mut run = TrainingRunState::new(
        "adapter-run-1",
        "adapter-sft",
        initial_cluster.cluster_id().as_str(),
        "apple.weather.policy",
        psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03"),
    )?;
    let mut coordinator = AdapterTrainingClusterCoordinator::new(
        run.clone(),
        adapter_target,
        policy_revision,
        checkpoint_pointer,
        AdapterContributorCapabilityPolicy {
            minimum_free_memory_bytes: 12 * GIB_BYTES,
            ..AdapterContributorCapabilityPolicy::default()
        },
    );
    let initial_membership = coordinator
        .observe_cluster_state(&initial_cluster, 1_010)?
        .clone();
    let first_window = coordinator.plan_next_window(
        vec![
            AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-a",
                "slice-digest-a",
            )?,
            AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-b",
                "slice-digest-b",
            )?,
            AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-c",
                "slice-digest-c",
            )?,
        ],
        2,
        1_020,
    )?;
    coordinator.activate_current_window(1_030)?;

    let churned_cluster = cluster_state(
        &[
            (
                "trainer-a",
                NodeRole::CoordinatorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-a",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-c",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-d",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
        ],
        &[
            ("trainer-a", 24, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-a", 20, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-c", 10, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-d", 30, 1, ClusterBackendReadinessStatus::Ready),
        ],
    );
    let churned_membership = coordinator
        .observe_cluster_state(&churned_cluster, 1_040)?
        .clone();

    let contribution_ids = coordinator
        .current_window_mut()?
        .contributions
        .iter()
        .map(|contribution| contribution.assignment.binding.contribution_id.clone())
        .collect::<Vec<_>>();
    for (index, contribution_id) in contribution_ids.iter().enumerate() {
        coordinator.current_window_mut()?.record_execution(
            contribution_id.as_str(),
            AdapterContributionExecutionSummary::new(
                1_050 + index as u64 * 10,
                1_055 + index as u64 * 10,
                8 + index as u32,
                32 + index as u32,
                Some(180 + index as u32 * 10),
                format!("delta-digest-{index}"),
            )?,
        )?;
        coordinator.current_window_mut()?.record_upload(
            contribution_id.as_str(),
            AdapterContributionUploadLocator::new(
                format!("object://adapter-cluster/{contribution_id}"),
                format!("upload-manifest-{index}"),
                2_048 + index as u64,
            )?,
            1_070 + index as u64 * 10,
        )?;
    }
    coordinator
        .current_window_mut()?
        .record_validator_disposition(
            contribution_ids[0].as_str(),
            AdapterContributionValidatorDisposition::Accepted,
            "validator.accepted",
            1_090,
        )?;
    coordinator
        .current_window_mut()?
        .record_aggregation_eligibility(contribution_ids[0].as_str(), Some(10_000), 1_091)?;
    coordinator
        .current_window_mut()?
        .record_validator_disposition(
            contribution_ids[1].as_str(),
            AdapterContributionValidatorDisposition::ReplayRequired,
            "validator.replay_required.departed_worker",
            1_092,
        )?;
    coordinator
        .current_window_mut()?
        .record_aggregation_eligibility(contribution_ids[1].as_str(), None, 1_093)?;
    coordinator.current_window_mut()?.seal()?;
    coordinator
        .current_window_mut()?
        .aggregate(None, None, 1_094)?;
    coordinator.current_window_mut()?.reconcile()?;
    coordinator.synchronize_current_window_status(1_095)?;

    let second_window = coordinator.plan_next_window(
        vec![
            AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-d",
                "slice-digest-d",
            )?,
            AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-e",
                "slice-digest-e",
            )?,
        ],
        2,
        1_100,
    )?;
    run = coordinator.run.clone();
    let receipt_digest = stable_digest([
        run.run_id.as_str(),
        initial_membership.receipt_digest.as_str(),
        churned_membership.receipt_digest.as_str(),
        first_window.plan.receipt_digest.as_str(),
        second_window.plan.receipt_digest.as_str(),
    ]);
    Ok(AdapterClusterMembershipHarnessReceipt {
        coordinator,
        initial_membership,
        churned_membership,
        first_window: first_window.plan,
        second_window: second_window.plan,
        receipt_digest,
    })
}

fn stable_digest<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    format!("{:x}", hasher.finalize())
}

fn cluster_state(
    memberships: &[(&str, NodeRole, ClusterMembershipStatus)],
    telemetry: &[(&str, u64, u16, ClusterBackendReadinessStatus)],
) -> ClusterState {
    let cluster_id = ClusterId::new(
        &ClusterNamespace::new("adapter-cluster-harness"),
        &AdmissionToken::new("shared-secret"),
    );
    let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
    snapshot.memberships = memberships
        .iter()
        .map(|(node_id, role, status)| {
            (
                NodeId::new(*node_id),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id: cluster_id.clone(),
                        node_id: NodeId::new(*node_id),
                        node_epoch: NodeEpoch::initial(),
                        role: *role,
                        auth_public_key: format!("{node_id}-pk"),
                        attestation: None,
                    },
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 20_000)),
                    *status,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    snapshot.telemetry = telemetry
        .iter()
        .map(
            |(node_id, free_memory_gib, accelerator_count, backend_status)| {
                (
                    NodeId::new(*node_id),
                    ClusterNodeTelemetry::new(NodeId::new(*node_id))
                        .with_memory(
                            Some(free_memory_gib.saturating_mul(GIB_BYTES)),
                            Some(free_memory_gib.saturating_mul(GIB_BYTES)),
                        )
                        .with_accelerator_count(*accelerator_count)
                        .with_backend_readiness(
                            AdapterContributorCapabilityPolicy::default().backend_label,
                            *backend_status,
                        )
                        .with_stability_posture(ClusterStabilityPosture::Stable),
                )
            },
        )
        .collect::<BTreeMap<_, _>>();
    ClusterState::from_snapshot(snapshot)
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
        AdapterContributorCapabilityPolicy, AdapterTrainingClusterCoordinator,
        run_adapter_cluster_membership_harness,
    };
    use crate::{
        AdapterDatasetSliceIdentity, AdapterTargetIdentity, CheckpointPointer,
        CheckpointScopeBinding, CheckpointScopeKind, PolicyRevision, TrainingWindowStatus,
    };

    #[test]
    fn adapter_cluster_filters_capabilities_and_plans_window()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = super::cluster_state(
            &[
                (
                    "trainer-a",
                    psionic_cluster::NodeRole::CoordinatorOnly,
                    psionic_cluster::ClusterMembershipStatus::Ready,
                ),
                (
                    "worker-a",
                    psionic_cluster::NodeRole::ExecutorOnly,
                    psionic_cluster::ClusterMembershipStatus::Ready,
                ),
                (
                    "worker-b",
                    psionic_cluster::NodeRole::ExecutorOnly,
                    psionic_cluster::ClusterMembershipStatus::Ready,
                ),
                (
                    "worker-c",
                    psionic_cluster::NodeRole::ExecutorOnly,
                    psionic_cluster::ClusterMembershipStatus::Ready,
                ),
            ],
            &[
                (
                    "trainer-a",
                    24,
                    1,
                    psionic_cluster::ClusterBackendReadinessStatus::Ready,
                ),
                (
                    "worker-a",
                    16,
                    1,
                    psionic_cluster::ClusterBackendReadinessStatus::Ready,
                ),
                (
                    "worker-b",
                    26,
                    1,
                    psionic_cluster::ClusterBackendReadinessStatus::Ready,
                ),
                (
                    "worker-c",
                    6,
                    1,
                    psionic_cluster::ClusterBackendReadinessStatus::Ready,
                ),
            ],
        );
        let run = crate::TrainingRunState::new(
            "adapter-run-2",
            "adapter-sft",
            state.cluster_id().as_str(),
            "apple.weather.policy",
            psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03"),
        )?;
        let mut coordinator = AdapterTrainingClusterCoordinator::new(
            run,
            AdapterTargetIdentity::new(
                "apple.weather.adapter",
                "apple.foundation_models",
                "apple://foundation-model/base",
                "apple.fmadapter",
            )?,
            PolicyRevision::new(
                "apple.weather.policy",
                "policy-r7",
                "policy-digest-r7",
                1_000,
            ),
            CheckpointPointer::new(
                CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-weather-2"),
                "apple.weather.policy",
                super::harness_checkpoint_reference("checkpoint/weather/r7", 1_000),
                "manifest-digest-r7",
                1_001,
            )?,
            AdapterContributorCapabilityPolicy {
                minimum_free_memory_bytes: 12 * super::GIB_BYTES,
                ..AdapterContributorCapabilityPolicy::default()
            },
        );
        let receipt = coordinator.observe_cluster_state(&state, 1_010)?.clone();
        assert_eq!(
            receipt
                .contributor_statuses
                .iter()
                .find(|status| status.node_id == "trainer-a")
                .expect("trainer exists")
                .eligibility,
            super::AdapterContributorEligibility::RoleNotContributor
        );
        assert_eq!(
            receipt
                .contributor_statuses
                .iter()
                .find(|status| status.node_id == "worker-c")
                .expect("worker-c exists")
                .eligibility,
            super::AdapterContributorEligibility::InsufficientFreeMemory
        );

        let window = coordinator.plan_next_window(
            vec![
                AdapterDatasetSliceIdentity::new(
                    "dataset.weather",
                    "train",
                    "slice-a",
                    "slice-digest-a",
                )?,
                AdapterDatasetSliceIdentity::new(
                    "dataset.weather",
                    "train",
                    "slice-b",
                    "slice-digest-b",
                )?,
            ],
            2,
            1_020,
        )?;
        assert_eq!(window.plan.selected_node_ids, vec!["worker-b", "worker-a"]);
        assert!(!window.plan.receipt_digest.is_empty());
        Ok(())
    }

    #[test]
    fn adapter_cluster_harness_reselects_after_membership_churn()
    -> Result<(), Box<dyn std::error::Error>> {
        let receipt = run_adapter_cluster_membership_harness()?;
        assert_eq!(
            receipt.first_window.selected_node_ids,
            vec!["worker-b", "worker-a"]
        );
        assert_eq!(
            receipt.churned_membership.current_window_id.as_deref(),
            Some("adapter-run-1-window-1")
        );
        assert_eq!(
            receipt
                .coordinator
                .run
                .windows
                .iter()
                .find(|window| window.window_id == "adapter-run-1-window-1")
                .expect("first window exists")
                .status,
            TrainingWindowStatus::Reconciled
        );
        assert_eq!(
            receipt.second_window.selected_node_ids,
            vec!["worker-d", "worker-a"]
        );
        assert!(!receipt.receipt_digest.is_empty());
        Ok(())
    }
}
