use std::collections::BTreeSet;

use psionic_cluster::{ClusterMembershipStatus, ClusterState, NodeId, NodeRole};
use psionic_environments::EnvironmentPackageKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Lifecycle state for one full training run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingRunStatus {
    /// Runtime substrate and participants are still being prepared.
    Initializing,
    /// The run is active.
    Running,
    /// The run is draining existing work without taking new windows.
    Draining,
    /// The run completed successfully.
    Completed,
    /// The run failed terminally.
    Failed,
    /// The run was cancelled.
    Cancelled,
}

/// Participant role in the training run graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingParticipantRole {
    /// Participant only coordinates or trains.
    TrainerOnly,
    /// Participant only contributes execution work.
    ContributorOnly,
    /// Participant can both coordinate and contribute.
    TrainerContributor,
}

/// Admission posture for one participant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingParticipantAdmissionState {
    /// Participant is admitted to the run.
    Admitted,
    /// Participant is retained in history but suspended from admission.
    Suspended,
    /// Participant is fully removed from current admission.
    Removed,
}

/// Readiness posture for one participant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingParticipantReadinessState {
    /// Participant has been admitted but is not yet ready.
    Pending,
    /// Participant is ready for selection.
    Ready,
    /// Participant is admitted but currently unready.
    Unready,
}

/// Contribution posture for one participant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingParticipantContributionState {
    /// Participant is admitted but not currently selected.
    Standby,
    /// Participant is selected for the current contributor set.
    Selected,
    /// Participant is actively serving the current window.
    Active,
    /// Participant is admitted but temporarily suspended from contribution.
    Suspended,
}

/// Actual departure reason for one participant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingParticipantDepartureReason {
    /// Participant left normally.
    Left,
    /// Participant crashed.
    Crashed,
    /// Participant timed out or stopped heartbeating.
    TimedOut,
    /// Participant was explicitly evicted from the run.
    Evicted,
}

/// Reason the participant remains admitted but is removed from the contributor set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingContributorSuspensionReason {
    /// Duplicate contribution or replay suspicion.
    DuplicateContribution,
    /// Reliability or quality penalty.
    ReliabilityPenalty,
    /// Node is currently missing adapter-training prerequisites.
    CapabilityPrerequisiteMissing,
    /// Spam or abuse suspicion.
    SpamSuspected,
    /// Operator-driven hold.
    OperatorHold,
}

/// Window transition state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingWindowStatus {
    /// Planned but not yet active.
    Planned,
    /// Currently active.
    Active,
    /// No more contributions may enter.
    Sealed,
    /// Scoring or eval is complete.
    Scored,
    /// Final window accounting is reconciled.
    Reconciled,
}

/// Deterministic assignment rule for one window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingWindowAssignmentRule {
    /// Assign batch and eval slices in contributor-order round robin.
    RoundRobinByPriority {
        /// Number of trainer-batch slices.
        batch_slice_count: u32,
        /// Number of eval slices.
        eval_slice_count: u32,
    },
}

/// Lifecycle event captured by the run graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingLifecycleEventKind {
    /// Participant first joined the run graph.
    ParticipantJoined,
    /// Participant rejoined after an unready or departed phase.
    ParticipantRejoined,
    /// Participant heartbeat was recorded.
    ParticipantHeartbeat,
    /// Participant priority or reliability changed.
    ParticipantPriorityUpdated,
    /// Participant remained admitted but was suspended from contribution.
    ParticipantContributionSuspended,
    /// Participant departed.
    ParticipantDeparted,
    /// Topology revision changed.
    TopologyRevised,
    /// Contributor selection changed.
    ContributorSetRevised,
    /// Window planned.
    WindowPlanned,
    /// Window activated.
    WindowActivated,
    /// Window sealed.
    WindowSealed,
    /// Window scored.
    WindowScored,
    /// Window reconciled.
    WindowReconciled,
}

/// One participant in the run graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingParticipantRecord {
    /// Stable node id.
    pub node_id: NodeId,
    /// Stable participant role.
    pub role: TrainingParticipantRole,
    /// Admission posture.
    pub admission_state: TrainingParticipantAdmissionState,
    /// Readiness posture.
    pub readiness_state: TrainingParticipantReadinessState,
    /// Contribution posture.
    pub contribution_state: TrainingParticipantContributionState,
    /// Priority in basis points for contributor selection.
    pub priority_bps: u16,
    /// Reliability in basis points for contributor selection tie-breaks.
    pub reliability_bps: u16,
    /// First join time.
    pub joined_at_ms: u64,
    /// Latest heartbeat when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at_ms: Option<u64>,
    /// Last actual departure when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_departure_reason: Option<TrainingParticipantDepartureReason>,
    /// Last contributor-suspension reason when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributor_suspension_reason: Option<TrainingContributorSuspensionReason>,
}

impl TrainingParticipantRecord {
    fn from_cluster(
        node_id: &NodeId,
        node_role: NodeRole,
        status: ClusterMembershipStatus,
        observed_at_ms: u64,
    ) -> Self {
        let readiness_state = readiness_from_cluster_status(status);
        let contribution_state = match readiness_state {
            TrainingParticipantReadinessState::Ready => {
                TrainingParticipantContributionState::Standby
            }
            TrainingParticipantReadinessState::Pending
            | TrainingParticipantReadinessState::Unready => {
                TrainingParticipantContributionState::Suspended
            }
        };
        Self {
            node_id: node_id.clone(),
            role: participant_role_from_node_role(node_role),
            admission_state: TrainingParticipantAdmissionState::Admitted,
            readiness_state,
            contribution_state,
            priority_bps: 5_000,
            reliability_bps: 5_000,
            joined_at_ms: observed_at_ms,
            last_heartbeat_at_ms: if status == ClusterMembershipStatus::Offline {
                None
            } else {
                Some(observed_at_ms)
            },
            last_departure_reason: if status == ClusterMembershipStatus::Offline {
                Some(TrainingParticipantDepartureReason::TimedOut)
            } else {
                None
            },
            contributor_suspension_reason: None,
        }
    }
}

/// One topology revision over the wider admitted set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingTopologyRevision {
    /// Stable topology revision id.
    pub topology_revision_id: String,
    /// Monotonic revision number.
    pub revision_number: u32,
    /// Admitted participants visible to the run.
    pub admitted_node_ids: Vec<String>,
    /// Ready participants visible to the run.
    pub ready_node_ids: Vec<String>,
    /// Contributor-set revision active when this topology was captured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributor_set_revision_id: Option<String>,
    /// Creation timestamp.
    pub created_at_ms: u64,
    /// Stable digest over the topology facts.
    pub revision_digest: String,
}

/// One contributor-set revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingContributorSetRevision {
    /// Stable contributor-set revision id.
    pub contributor_set_revision_id: String,
    /// Monotonic revision number.
    pub revision_number: u32,
    /// Contributors selected for window planning.
    pub contributor_node_ids: Vec<String>,
    /// Ready admitted participants not selected as contributors.
    pub standby_node_ids: Vec<String>,
    /// Stable digest over the selection.
    pub selection_digest: String,
    /// Selection timestamp.
    pub selected_at_ms: u64,
}

/// One trainer batch slice assigned to a contributor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingBatchSliceAssignment {
    /// Slice index.
    pub slice_index: u32,
    /// Assigned contributor node id.
    pub node_id: String,
}

/// One sampled eval slice assigned to a contributor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingEvalSliceAssignment {
    /// Slice index.
    pub slice_index: u32,
    /// Assigned contributor node id.
    pub node_id: String,
}

/// One planned or active training window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingWindow {
    /// Stable window id.
    pub window_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Topology revision that governed planning.
    pub topology_revision_id: String,
    /// Contributor-set revision used for planning.
    pub contributor_set_revision_id: String,
    /// Active policy revision when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_revision_id: Option<String>,
    /// Current transition state.
    pub status: TrainingWindowStatus,
    /// Assignment rule that produced the slices.
    pub assignment_rule: TrainingWindowAssignmentRule,
    /// Trainer-batch slice assignments.
    pub batch_assignments: Vec<TrainingBatchSliceAssignment>,
    /// Sampled eval slice assignments.
    pub eval_assignments: Vec<TrainingEvalSliceAssignment>,
    /// Stable digest over the window.
    pub window_digest: String,
}

/// One inspectable lifecycle event in the run graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingLifecycleEvent {
    /// Stable event id.
    pub event_id: String,
    /// Event kind.
    pub kind: TrainingLifecycleEventKind,
    /// Run id.
    pub run_id: String,
    /// Stage id.
    pub stage_id: String,
    /// Participant node id when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    /// Topology revision id when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_revision_id: Option<String>,
    /// Contributor-set revision id when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributor_set_revision_id: Option<String>,
    /// Window id when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    /// Timestamp.
    pub occurred_at_ms: u64,
    /// Stable digest over the event.
    pub event_digest: String,
}

/// Error returned by training run-graph state transitions.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TrainingRunGraphError {
    /// Missing run id.
    #[error("training run is missing `run_id`")]
    MissingRunId,
    /// Missing stage id.
    #[error("training run is missing `stage_id`")]
    MissingStageId,
    /// Missing cluster id.
    #[error("training run is missing `cluster_id`")]
    MissingClusterId,
    /// Missing checkpoint family.
    #[error("training run is missing `checkpoint_family`")]
    MissingCheckpointFamily,
    /// Missing environment ref.
    #[error("training run is missing environment `environment_ref`")]
    MissingEnvironmentRef,
    /// Missing environment version.
    #[error("training run is missing environment `version`")]
    MissingEnvironmentVersion,
    /// Cluster mismatch.
    #[error("training run cluster mismatch: expected `{expected}`, found `{actual}`")]
    ClusterMismatch {
        /// Expected cluster id.
        expected: String,
        /// Actual cluster id.
        actual: String,
    },
    /// Unknown participant.
    #[error("training run does not know participant `{node_id}`")]
    UnknownParticipant {
        /// Unknown node id.
        node_id: String,
    },
    /// Participant priority out of range.
    #[error("participant priority/reliability must be <= 10000")]
    InvalidPriority,
    /// Contributor selection has no ready admitted participants.
    #[error("training run has no ready admitted participants eligible for contributor selection")]
    NoReadyParticipants,
    /// Contributor selection revision missing.
    #[error("training run has no contributor-set revision to use for window planning")]
    MissingContributorSetRevision,
    /// Topology revision missing.
    #[error("training run has no topology revision to use for window planning")]
    MissingTopologyRevision,
    /// Unknown window.
    #[error("training run does not know window `{window_id}`")]
    UnknownWindow {
        /// Unknown window id.
        window_id: String,
    },
    /// Invalid window transition.
    #[error("training window `{window_id}` cannot transition from `{from}` to `{to}`")]
    InvalidWindowTransition {
        /// Window id.
        window_id: String,
        /// Current state.
        from: String,
        /// Requested state.
        to: String,
    },
}

/// Canonical run graph and participant lifecycle state for one training run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingRunState {
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Cluster id bound to this run.
    pub cluster_id: String,
    /// Checkpoint family bound to this run.
    pub checkpoint_family: String,
    /// Environment package identity bound to the run.
    pub environment: EnvironmentPackageKey,
    /// Current run status.
    pub status: TrainingRunStatus,
    /// Participant records.
    pub participants: Vec<TrainingParticipantRecord>,
    /// Topology revision history.
    pub topology_revisions: Vec<TrainingTopologyRevision>,
    /// Contributor-set revision history.
    pub contributor_set_revisions: Vec<TrainingContributorSetRevision>,
    /// Window history.
    pub windows: Vec<TrainingWindow>,
    /// Lifecycle events.
    pub lifecycle_events: Vec<TrainingLifecycleEvent>,
}

impl TrainingRunState {
    /// Creates a new training run state.
    pub fn new(
        run_id: impl Into<String>,
        stage_id: impl Into<String>,
        cluster_id: impl Into<String>,
        checkpoint_family: impl Into<String>,
        environment: EnvironmentPackageKey,
    ) -> Result<Self, TrainingRunGraphError> {
        let run_id = run_id.into();
        if run_id.trim().is_empty() {
            return Err(TrainingRunGraphError::MissingRunId);
        }
        let stage_id = stage_id.into();
        if stage_id.trim().is_empty() {
            return Err(TrainingRunGraphError::MissingStageId);
        }
        let cluster_id = cluster_id.into();
        if cluster_id.trim().is_empty() {
            return Err(TrainingRunGraphError::MissingClusterId);
        }
        let checkpoint_family = checkpoint_family.into();
        if checkpoint_family.trim().is_empty() {
            return Err(TrainingRunGraphError::MissingCheckpointFamily);
        }
        if environment.environment_ref.trim().is_empty() {
            return Err(TrainingRunGraphError::MissingEnvironmentRef);
        }
        if environment.version.trim().is_empty() {
            return Err(TrainingRunGraphError::MissingEnvironmentVersion);
        }
        Ok(Self {
            run_id,
            stage_id,
            cluster_id,
            checkpoint_family,
            environment,
            status: TrainingRunStatus::Initializing,
            participants: Vec::new(),
            topology_revisions: Vec::new(),
            contributor_set_revisions: Vec::new(),
            windows: Vec::new(),
            lifecycle_events: Vec::new(),
        })
    }

    /// Mirrors current cluster membership into the run graph as admitted and ready truth.
    pub fn apply_cluster_membership_snapshot(
        &mut self,
        cluster_state: &ClusterState,
        observed_at_ms: u64,
    ) -> Result<&TrainingTopologyRevision, TrainingRunGraphError> {
        if cluster_state.cluster_id().as_str() != self.cluster_id {
            return Err(TrainingRunGraphError::ClusterMismatch {
                expected: self.cluster_id.clone(),
                actual: String::from(cluster_state.cluster_id().as_str()),
            });
        }

        let mut pending_events = Vec::new();
        for membership in cluster_state.memberships().values() {
            let node_id = membership.identity.node_id.clone();
            match self
                .participants
                .iter_mut()
                .find(|participant| participant.node_id == node_id)
            {
                Some(participant) => {
                    let previous_readiness = participant.readiness_state;
                    participant.role = participant_role_from_node_role(membership.identity.role);
                    participant.admission_state = TrainingParticipantAdmissionState::Admitted;
                    participant.readiness_state = readiness_from_cluster_status(membership.status);
                    participant.last_heartbeat_at_ms =
                        if membership.status == ClusterMembershipStatus::Offline {
                            participant.last_heartbeat_at_ms
                        } else {
                            Some(observed_at_ms)
                        };
                    if membership.status == ClusterMembershipStatus::Offline {
                        participant.last_departure_reason =
                            Some(TrainingParticipantDepartureReason::TimedOut);
                        participant.contribution_state =
                            TrainingParticipantContributionState::Suspended;
                    } else if participant.contribution_state
                        == TrainingParticipantContributionState::Suspended
                        && participant.contributor_suspension_reason.is_none()
                    {
                        participant.contribution_state =
                            TrainingParticipantContributionState::Standby;
                    }
                    if previous_readiness != TrainingParticipantReadinessState::Ready
                        && participant.readiness_state == TrainingParticipantReadinessState::Ready
                    {
                        pending_events.push((
                            TrainingLifecycleEventKind::ParticipantRejoined,
                            Some(String::from(participant.node_id.as_str())),
                        ));
                    }
                }
                None => {
                    self.participants
                        .push(TrainingParticipantRecord::from_cluster(
                            &node_id,
                            membership.identity.role,
                            membership.status,
                            observed_at_ms,
                        ));
                    pending_events.push((
                        TrainingLifecycleEventKind::ParticipantJoined,
                        Some(String::from(node_id.as_str())),
                    ));
                }
            }
        }
        for (kind, node_id) in pending_events {
            self.push_event(kind, node_id, None, None, None, observed_at_ms);
        }

        self.status = TrainingRunStatus::Running;
        let revision_number = self.topology_revisions.len() as u32 + 1;
        let admitted_node_ids = sorted_node_ids(
            self.participants
                .iter()
                .filter(|participant| {
                    participant.admission_state == TrainingParticipantAdmissionState::Admitted
                })
                .map(|participant| &participant.node_id),
        );
        let ready_node_ids = sorted_node_ids(
            self.participants
                .iter()
                .filter(|participant| {
                    participant.admission_state == TrainingParticipantAdmissionState::Admitted
                        && participant.readiness_state == TrainingParticipantReadinessState::Ready
                })
                .map(|participant| &participant.node_id),
        );
        let contributor_set_revision_id = self
            .contributor_set_revisions
            .last()
            .map(|revision| revision.contributor_set_revision_id.clone());
        let topology_revision_id = format!("{}-topology-{revision_number}", self.run_id);
        let revision_digest = stable_topology_revision_digest(
            self.run_id.as_str(),
            self.stage_id.as_str(),
            topology_revision_id.as_str(),
            admitted_node_ids.as_slice(),
            ready_node_ids.as_slice(),
            contributor_set_revision_id.as_deref(),
            observed_at_ms,
        );
        let revision = TrainingTopologyRevision {
            topology_revision_id: topology_revision_id.clone(),
            revision_number,
            admitted_node_ids,
            ready_node_ids,
            contributor_set_revision_id,
            created_at_ms: observed_at_ms,
            revision_digest,
        };
        self.topology_revisions.push(revision);
        self.push_event(
            TrainingLifecycleEventKind::TopologyRevised,
            None,
            Some(topology_revision_id),
            self.contributor_set_revisions
                .last()
                .map(|revision| revision.contributor_set_revision_id.clone()),
            None,
            observed_at_ms,
        );
        self.topology_revisions
            .last()
            .ok_or(TrainingRunGraphError::MissingTopologyRevision)
    }

    /// Records one participant heartbeat.
    pub fn record_heartbeat(
        &mut self,
        node_id: &NodeId,
        observed_at_ms: u64,
    ) -> Result<(), TrainingRunGraphError> {
        let participant = self
            .participants
            .iter_mut()
            .find(|participant| &participant.node_id == node_id)
            .ok_or_else(|| TrainingRunGraphError::UnknownParticipant {
                node_id: String::from(node_id.as_str()),
            })?;
        participant.last_heartbeat_at_ms = Some(observed_at_ms);
        if participant.admission_state == TrainingParticipantAdmissionState::Admitted
            && participant.readiness_state != TrainingParticipantReadinessState::Unready
        {
            participant.readiness_state = TrainingParticipantReadinessState::Ready;
            if participant.contribution_state == TrainingParticipantContributionState::Suspended
                && participant.contributor_suspension_reason.is_none()
            {
                participant.contribution_state = TrainingParticipantContributionState::Standby;
            }
        }
        self.push_event(
            TrainingLifecycleEventKind::ParticipantHeartbeat,
            Some(String::from(node_id.as_str())),
            self.topology_revisions
                .last()
                .map(|revision| revision.topology_revision_id.clone()),
            self.contributor_set_revisions
                .last()
                .map(|revision| revision.contributor_set_revision_id.clone()),
            None,
            observed_at_ms,
        );
        Ok(())
    }

    /// Updates participant ranking facts.
    pub fn update_participant_priority(
        &mut self,
        node_id: &NodeId,
        priority_bps: u16,
        reliability_bps: u16,
        observed_at_ms: u64,
    ) -> Result<(), TrainingRunGraphError> {
        if priority_bps > 10_000 || reliability_bps > 10_000 {
            return Err(TrainingRunGraphError::InvalidPriority);
        }
        let participant = self
            .participants
            .iter_mut()
            .find(|participant| &participant.node_id == node_id)
            .ok_or_else(|| TrainingRunGraphError::UnknownParticipant {
                node_id: String::from(node_id.as_str()),
            })?;
        participant.priority_bps = priority_bps;
        participant.reliability_bps = reliability_bps;
        self.push_event(
            TrainingLifecycleEventKind::ParticipantPriorityUpdated,
            Some(String::from(node_id.as_str())),
            None,
            None,
            None,
            observed_at_ms,
        );
        Ok(())
    }

    /// Suspends a participant from contribution without removing admission.
    pub fn suspend_contributor(
        &mut self,
        node_id: &NodeId,
        reason: TrainingContributorSuspensionReason,
        observed_at_ms: u64,
    ) -> Result<(), TrainingRunGraphError> {
        let participant = self
            .participants
            .iter_mut()
            .find(|participant| &participant.node_id == node_id)
            .ok_or_else(|| TrainingRunGraphError::UnknownParticipant {
                node_id: String::from(node_id.as_str()),
            })?;
        participant.contribution_state = TrainingParticipantContributionState::Suspended;
        participant.contributor_suspension_reason = Some(reason);
        self.push_event(
            TrainingLifecycleEventKind::ParticipantContributionSuspended,
            Some(String::from(node_id.as_str())),
            None,
            None,
            None,
            observed_at_ms,
        );
        Ok(())
    }

    /// Records an actual participant departure.
    pub fn record_departure(
        &mut self,
        node_id: &NodeId,
        reason: TrainingParticipantDepartureReason,
        observed_at_ms: u64,
    ) -> Result<(), TrainingRunGraphError> {
        let participant = self
            .participants
            .iter_mut()
            .find(|participant| &participant.node_id == node_id)
            .ok_or_else(|| TrainingRunGraphError::UnknownParticipant {
                node_id: String::from(node_id.as_str()),
            })?;
        participant.last_departure_reason = Some(reason);
        participant.readiness_state = TrainingParticipantReadinessState::Unready;
        participant.contribution_state = TrainingParticipantContributionState::Suspended;
        if reason == TrainingParticipantDepartureReason::Evicted {
            participant.admission_state = TrainingParticipantAdmissionState::Suspended;
        }
        self.push_event(
            TrainingLifecycleEventKind::ParticipantDeparted,
            Some(String::from(node_id.as_str())),
            None,
            None,
            None,
            observed_at_ms,
        );
        Ok(())
    }

    /// Selects the bounded contributor set deterministically from admitted ready participants.
    pub fn select_contributors(
        &mut self,
        max_contributors: usize,
        selected_at_ms: u64,
    ) -> Result<&TrainingContributorSetRevision, TrainingRunGraphError> {
        let mut candidates = self
            .participants
            .iter()
            .filter(|participant| {
                participant.admission_state == TrainingParticipantAdmissionState::Admitted
                    && participant.readiness_state == TrainingParticipantReadinessState::Ready
                    && participant.contributor_suspension_reason.is_none()
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(TrainingRunGraphError::NoReadyParticipants);
        }
        candidates.sort_by(|left, right| {
            right
                .priority_bps
                .cmp(&left.priority_bps)
                .then_with(|| right.reliability_bps.cmp(&left.reliability_bps))
                .then_with(|| left.node_id.as_str().cmp(right.node_id.as_str()))
        });
        let contributor_count = max_contributors.max(1).min(candidates.len());
        let contributor_node_ids = candidates
            .iter()
            .take(contributor_count)
            .map(|participant| String::from(participant.node_id.as_str()))
            .collect::<Vec<_>>();
        let contributor_set: BTreeSet<&str> =
            contributor_node_ids.iter().map(String::as_str).collect();
        let standby_node_ids = candidates
            .iter()
            .skip(contributor_count)
            .map(|participant| String::from(participant.node_id.as_str()))
            .collect::<Vec<_>>();

        for participant in &mut self.participants {
            if contributor_set.contains(participant.node_id.as_str()) {
                participant.contribution_state = TrainingParticipantContributionState::Selected;
                participant.contributor_suspension_reason = None;
            } else if participant.admission_state == TrainingParticipantAdmissionState::Admitted
                && participant.readiness_state == TrainingParticipantReadinessState::Ready
                && participant.contributor_suspension_reason.is_none()
            {
                participant.contribution_state = TrainingParticipantContributionState::Standby;
            }
        }

        let revision_number = self.contributor_set_revisions.len() as u32 + 1;
        let contributor_set_revision_id = format!("{}-contributors-{revision_number}", self.run_id);
        let selection_digest = stable_contributor_set_digest(
            self.run_id.as_str(),
            self.stage_id.as_str(),
            contributor_set_revision_id.as_str(),
            contributor_node_ids.as_slice(),
            standby_node_ids.as_slice(),
            selected_at_ms,
        );
        let revision = TrainingContributorSetRevision {
            contributor_set_revision_id: contributor_set_revision_id.clone(),
            revision_number,
            contributor_node_ids,
            standby_node_ids,
            selection_digest,
            selected_at_ms,
        };
        self.contributor_set_revisions.push(revision);
        self.push_event(
            TrainingLifecycleEventKind::ContributorSetRevised,
            None,
            self.topology_revisions
                .last()
                .map(|revision| revision.topology_revision_id.clone()),
            Some(contributor_set_revision_id),
            None,
            selected_at_ms,
        );
        self.contributor_set_revisions
            .last()
            .ok_or(TrainingRunGraphError::MissingContributorSetRevision)
    }

    /// Plans a new window using the most recent topology and contributor-set revisions.
    pub fn plan_window(
        &mut self,
        policy_revision_id: Option<String>,
        assignment_rule: TrainingWindowAssignmentRule,
        planned_at_ms: u64,
    ) -> Result<&TrainingWindow, TrainingRunGraphError> {
        let topology_revision = self
            .topology_revisions
            .last()
            .cloned()
            .ok_or(TrainingRunGraphError::MissingTopologyRevision)?;
        let contributor_set_revision = self
            .contributor_set_revisions
            .last()
            .cloned()
            .ok_or(TrainingRunGraphError::MissingContributorSetRevision)?;
        let window_number = self.windows.len() as u32 + 1;
        let window_id = format!("{}-window-{window_number}", self.run_id);
        let (batch_assignments, eval_assignments) = assign_window_slices(
            contributor_set_revision.contributor_node_ids.as_slice(),
            assignment_rule,
        );
        for assignment in &batch_assignments {
            if let Some(participant) = self
                .participants
                .iter_mut()
                .find(|participant| participant.node_id.as_str() == assignment.node_id)
            {
                participant.contribution_state = TrainingParticipantContributionState::Selected;
            }
        }
        let window_digest = stable_window_digest(
            self.run_id.as_str(),
            self.stage_id.as_str(),
            window_id.as_str(),
            topology_revision.topology_revision_id.as_str(),
            contributor_set_revision
                .contributor_set_revision_id
                .as_str(),
            policy_revision_id.as_deref(),
            assignment_rule,
            batch_assignments.as_slice(),
            eval_assignments.as_slice(),
            planned_at_ms,
        );
        let window = TrainingWindow {
            window_id: window_id.clone(),
            run_id: self.run_id.clone(),
            stage_id: self.stage_id.clone(),
            topology_revision_id: topology_revision.topology_revision_id.clone(),
            contributor_set_revision_id: contributor_set_revision
                .contributor_set_revision_id
                .clone(),
            policy_revision_id,
            status: TrainingWindowStatus::Planned,
            assignment_rule,
            batch_assignments,
            eval_assignments,
            window_digest,
        };
        self.windows.push(window);
        self.push_event(
            TrainingLifecycleEventKind::WindowPlanned,
            None,
            Some(topology_revision.topology_revision_id),
            Some(contributor_set_revision.contributor_set_revision_id),
            Some(window_id),
            planned_at_ms,
        );
        self.windows
            .last()
            .ok_or_else(|| TrainingRunGraphError::UnknownWindow {
                window_id: String::from("missing"),
            })
    }

    /// Transitions one window through the canonical state machine.
    pub fn transition_window(
        &mut self,
        window_id: &str,
        next_status: TrainingWindowStatus,
        occurred_at_ms: u64,
    ) -> Result<(), TrainingRunGraphError> {
        let (
            topology_revision_id,
            contributor_set_revision_id,
            window_id_owned,
            contributor_node_ids,
        ) = {
            let window = self
                .windows
                .iter_mut()
                .find(|window| window.window_id == window_id)
                .ok_or_else(|| TrainingRunGraphError::UnknownWindow {
                    window_id: String::from(window_id),
                })?;
            if !window_transition_allowed(window.status, next_status) {
                return Err(TrainingRunGraphError::InvalidWindowTransition {
                    window_id: String::from(window_id),
                    from: window_status_label(window.status).to_string(),
                    to: window_status_label(next_status).to_string(),
                });
            }
            window.status = next_status;
            (
                window.topology_revision_id.clone(),
                window.contributor_set_revision_id.clone(),
                window.window_id.clone(),
                window
                    .batch_assignments
                    .iter()
                    .map(|assignment| assignment.node_id.clone())
                    .collect::<Vec<_>>(),
            )
        };
        if next_status == TrainingWindowStatus::Active {
            let contributor_set: BTreeSet<&str> =
                contributor_node_ids.iter().map(String::as_str).collect();
            for participant in &mut self.participants {
                if contributor_set.contains(participant.node_id.as_str()) {
                    participant.contribution_state = TrainingParticipantContributionState::Active;
                }
            }
        } else if next_status == TrainingWindowStatus::Reconciled {
            let contributor_set: BTreeSet<&str> =
                contributor_node_ids.iter().map(String::as_str).collect();
            for participant in &mut self.participants {
                if contributor_set.contains(participant.node_id.as_str())
                    && participant.contributor_suspension_reason.is_none()
                {
                    participant.contribution_state = TrainingParticipantContributionState::Standby;
                }
            }
        }
        let event_kind = match next_status {
            TrainingWindowStatus::Planned => TrainingLifecycleEventKind::WindowPlanned,
            TrainingWindowStatus::Active => TrainingLifecycleEventKind::WindowActivated,
            TrainingWindowStatus::Sealed => TrainingLifecycleEventKind::WindowSealed,
            TrainingWindowStatus::Scored => TrainingLifecycleEventKind::WindowScored,
            TrainingWindowStatus::Reconciled => TrainingLifecycleEventKind::WindowReconciled,
        };
        self.push_event(
            event_kind,
            None,
            Some(topology_revision_id),
            Some(contributor_set_revision_id),
            Some(window_id_owned),
            occurred_at_ms,
        );
        Ok(())
    }

    fn push_event(
        &mut self,
        kind: TrainingLifecycleEventKind,
        node_id: Option<String>,
        topology_revision_id: Option<String>,
        contributor_set_revision_id: Option<String>,
        window_id: Option<String>,
        occurred_at_ms: u64,
    ) {
        let event_index = self.lifecycle_events.len() as u32 + 1;
        let event_id = format!("{}-event-{event_index}", self.run_id);
        let event_digest = stable_event_digest(
            event_id.as_str(),
            kind,
            self.run_id.as_str(),
            self.stage_id.as_str(),
            node_id.as_deref(),
            topology_revision_id.as_deref(),
            contributor_set_revision_id.as_deref(),
            window_id.as_deref(),
            occurred_at_ms,
        );
        self.lifecycle_events.push(TrainingLifecycleEvent {
            event_id,
            kind,
            run_id: self.run_id.clone(),
            stage_id: self.stage_id.clone(),
            node_id,
            topology_revision_id,
            contributor_set_revision_id,
            window_id,
            occurred_at_ms,
            event_digest,
        });
    }
}

fn participant_role_from_node_role(node_role: NodeRole) -> TrainingParticipantRole {
    match node_role {
        NodeRole::CoordinatorOnly => TrainingParticipantRole::TrainerOnly,
        NodeRole::ExecutorOnly => TrainingParticipantRole::ContributorOnly,
        NodeRole::Mixed => TrainingParticipantRole::TrainerContributor,
    }
}

fn readiness_from_cluster_status(
    status: ClusterMembershipStatus,
) -> TrainingParticipantReadinessState {
    match status {
        ClusterMembershipStatus::Joining => TrainingParticipantReadinessState::Pending,
        ClusterMembershipStatus::Ready => TrainingParticipantReadinessState::Ready,
        ClusterMembershipStatus::Draining | ClusterMembershipStatus::Offline => {
            TrainingParticipantReadinessState::Unready
        }
    }
}

fn sorted_node_ids<'a>(node_ids: impl Iterator<Item = &'a NodeId>) -> Vec<String> {
    let mut node_ids = node_ids
        .map(|node_id| String::from(node_id.as_str()))
        .collect::<Vec<_>>();
    node_ids.sort();
    node_ids
}

fn window_transition_allowed(from: TrainingWindowStatus, to: TrainingWindowStatus) -> bool {
    matches!(
        (from, to),
        (TrainingWindowStatus::Planned, TrainingWindowStatus::Active)
            | (TrainingWindowStatus::Active, TrainingWindowStatus::Sealed)
            | (TrainingWindowStatus::Sealed, TrainingWindowStatus::Scored)
            | (
                TrainingWindowStatus::Scored,
                TrainingWindowStatus::Reconciled
            )
    )
}

fn assign_window_slices(
    contributor_node_ids: &[String],
    assignment_rule: TrainingWindowAssignmentRule,
) -> (
    Vec<TrainingBatchSliceAssignment>,
    Vec<TrainingEvalSliceAssignment>,
) {
    match assignment_rule {
        TrainingWindowAssignmentRule::RoundRobinByPriority {
            batch_slice_count,
            eval_slice_count,
        } => {
            let batch_assignments = (0..batch_slice_count)
                .map(|slice_index| TrainingBatchSliceAssignment {
                    slice_index,
                    node_id: contributor_node_ids
                        [slice_index as usize % contributor_node_ids.len()]
                    .clone(),
                })
                .collect::<Vec<_>>();
            let eval_assignments = (0..eval_slice_count)
                .map(|slice_index| TrainingEvalSliceAssignment {
                    slice_index,
                    node_id: contributor_node_ids
                        [slice_index as usize % contributor_node_ids.len()]
                    .clone(),
                })
                .collect::<Vec<_>>();
            (batch_assignments, eval_assignments)
        }
    }
}

fn stable_topology_revision_digest(
    run_id: &str,
    stage_id: &str,
    topology_revision_id: &str,
    admitted_node_ids: &[String],
    ready_node_ids: &[String],
    contributor_set_revision_id: Option<&str>,
    created_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_topology_revision|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(topology_revision_id.as_bytes());
    if let Some(contributor_set_revision_id) = contributor_set_revision_id {
        hasher.update(b"|contributors|");
        hasher.update(contributor_set_revision_id.as_bytes());
    }
    for node_id in admitted_node_ids {
        hasher.update(b"|admitted|");
        hasher.update(node_id.as_bytes());
    }
    for node_id in ready_node_ids {
        hasher.update(b"|ready|");
        hasher.update(node_id.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(created_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_contributor_set_digest(
    run_id: &str,
    stage_id: &str,
    contributor_set_revision_id: &str,
    contributor_node_ids: &[String],
    standby_node_ids: &[String],
    selected_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_contributor_set|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(contributor_set_revision_id.as_bytes());
    for node_id in contributor_node_ids {
        hasher.update(b"|contributor|");
        hasher.update(node_id.as_bytes());
    }
    for node_id in standby_node_ids {
        hasher.update(b"|standby|");
        hasher.update(node_id.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(selected_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_window_digest(
    run_id: &str,
    stage_id: &str,
    window_id: &str,
    topology_revision_id: &str,
    contributor_set_revision_id: &str,
    policy_revision_id: Option<&str>,
    assignment_rule: TrainingWindowAssignmentRule,
    batch_assignments: &[TrainingBatchSliceAssignment],
    eval_assignments: &[TrainingEvalSliceAssignment],
    planned_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_window|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(window_id.as_bytes());
    hasher.update(b"|");
    hasher.update(topology_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(contributor_set_revision_id.as_bytes());
    if let Some(policy_revision_id) = policy_revision_id {
        hasher.update(b"|policy|");
        hasher.update(policy_revision_id.as_bytes());
    }
    match assignment_rule {
        TrainingWindowAssignmentRule::RoundRobinByPriority {
            batch_slice_count,
            eval_slice_count,
        } => {
            hasher.update(b"|round_robin|");
            hasher.update(batch_slice_count.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(eval_slice_count.to_string().as_bytes());
        }
    }
    for assignment in batch_assignments {
        hasher.update(b"|batch|");
        hasher.update(assignment.slice_index.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(assignment.node_id.as_bytes());
    }
    for assignment in eval_assignments {
        hasher.update(b"|eval|");
        hasher.update(assignment.slice_index.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(assignment.node_id.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(planned_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_event_digest(
    event_id: &str,
    kind: TrainingLifecycleEventKind,
    run_id: &str,
    stage_id: &str,
    node_id: Option<&str>,
    topology_revision_id: Option<&str>,
    contributor_set_revision_id: Option<&str>,
    window_id: Option<&str>,
    occurred_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_lifecycle_event|");
    hasher.update(event_id.as_bytes());
    hasher.update(b"|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(training_lifecycle_event_kind_label(kind));
    if let Some(node_id) = node_id {
        hasher.update(b"|node|");
        hasher.update(node_id.as_bytes());
    }
    if let Some(topology_revision_id) = topology_revision_id {
        hasher.update(b"|topology|");
        hasher.update(topology_revision_id.as_bytes());
    }
    if let Some(contributor_set_revision_id) = contributor_set_revision_id {
        hasher.update(b"|contributors|");
        hasher.update(contributor_set_revision_id.as_bytes());
    }
    if let Some(window_id) = window_id {
        hasher.update(b"|window|");
        hasher.update(window_id.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(occurred_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn training_lifecycle_event_kind_label(kind: TrainingLifecycleEventKind) -> &'static [u8] {
    match kind {
        TrainingLifecycleEventKind::ParticipantJoined => b"participant_joined",
        TrainingLifecycleEventKind::ParticipantRejoined => b"participant_rejoined",
        TrainingLifecycleEventKind::ParticipantHeartbeat => b"participant_heartbeat",
        TrainingLifecycleEventKind::ParticipantPriorityUpdated => b"participant_priority_updated",
        TrainingLifecycleEventKind::ParticipantContributionSuspended => {
            b"participant_contribution_suspended"
        }
        TrainingLifecycleEventKind::ParticipantDeparted => b"participant_departed",
        TrainingLifecycleEventKind::TopologyRevised => b"topology_revised",
        TrainingLifecycleEventKind::ContributorSetRevised => b"contributor_set_revised",
        TrainingLifecycleEventKind::WindowPlanned => b"window_planned",
        TrainingLifecycleEventKind::WindowActivated => b"window_activated",
        TrainingLifecycleEventKind::WindowSealed => b"window_sealed",
        TrainingLifecycleEventKind::WindowScored => b"window_scored",
        TrainingLifecycleEventKind::WindowReconciled => b"window_reconciled",
    }
}

fn window_status_label(status: TrainingWindowStatus) -> &'static str {
    match status {
        TrainingWindowStatus::Planned => "planned",
        TrainingWindowStatus::Active => "active",
        TrainingWindowStatus::Sealed => "sealed",
        TrainingWindowStatus::Scored => "scored",
        TrainingWindowStatus::Reconciled => "reconciled",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use psionic_cluster::{
        AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterNamespace, ClusterNodeIdentity,
        ClusterSnapshot, NodeEpoch, NodeId, NodeRole,
    };

    use super::{
        TrainingContributorSuspensionReason, TrainingParticipantAdmissionState,
        TrainingParticipantContributionState, TrainingParticipantDepartureReason,
        TrainingParticipantReadinessState, TrainingRunGraphError, TrainingRunState,
        TrainingWindowAssignmentRule, TrainingWindowStatus,
    };

    fn cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("train-run-graph"),
            &AdmissionToken::new("shared-secret"),
        )
    }

    fn membership(
        cluster_id: &ClusterId,
        node_id: &str,
        role: NodeRole,
        status: psionic_cluster::ClusterMembershipStatus,
    ) -> ClusterMembershipRecord {
        ClusterMembershipRecord::new(
            ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: NodeId::new(node_id),
                node_epoch: NodeEpoch::initial(),
                role,
                auth_public_key: format!("{node_id}-pk"),
                attestation: None,
            },
            Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 30_000)),
            status,
        )
    }

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = BTreeMap::from([
            (
                NodeId::new("trainer-a"),
                membership(
                    &cluster_id,
                    "trainer-a",
                    NodeRole::CoordinatorOnly,
                    psionic_cluster::ClusterMembershipStatus::Ready,
                ),
            ),
            (
                NodeId::new("worker-a"),
                membership(
                    &cluster_id,
                    "worker-a",
                    NodeRole::ExecutorOnly,
                    psionic_cluster::ClusterMembershipStatus::Ready,
                ),
            ),
            (
                NodeId::new("worker-b"),
                membership(
                    &cluster_id,
                    "worker-b",
                    NodeRole::ExecutorOnly,
                    psionic_cluster::ClusterMembershipStatus::Ready,
                ),
            ),
            (
                NodeId::new("worker-c"),
                membership(
                    &cluster_id,
                    "worker-c",
                    NodeRole::ExecutorOnly,
                    psionic_cluster::ClusterMembershipStatus::Joining,
                ),
            ),
        ]);
        psionic_cluster::ClusterState::from_snapshot(snapshot)
    }

    #[test]
    fn run_graph_tracks_admission_readiness_and_contributor_selection()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = cluster_state();
        let mut run = TrainingRunState::new(
            "train-run-1",
            "stage-1",
            state.cluster_id().as_str(),
            "train.decoder",
            psionic_environments::EnvironmentPackageKey::new("env.train", "1"),
        )?;
        let topology = run.apply_cluster_membership_snapshot(&state, 1_000)?;
        assert_eq!(topology.admitted_node_ids.len(), 4);
        assert_eq!(topology.ready_node_ids.len(), 3);

        run.update_participant_priority(&NodeId::new("worker-b"), 9_000, 7_500, 1_010)?;
        let contributor_set = run.select_contributors(2, 1_020)?;
        assert_eq!(contributor_set.contributor_node_ids.len(), 2);
        assert_eq!(contributor_set.contributor_node_ids[0], "worker-b");
        assert_eq!(
            run.participants
                .iter()
                .find(|participant| participant.node_id.as_str() == "worker-c")
                .expect("worker-c must exist")
                .readiness_state,
            TrainingParticipantReadinessState::Pending
        );
        Ok(())
    }

    #[test]
    fn contributor_suspension_and_departure_stay_distinct() -> Result<(), Box<dyn std::error::Error>>
    {
        let state = cluster_state();
        let mut run = TrainingRunState::new(
            "train-run-2",
            "stage-1",
            state.cluster_id().as_str(),
            "train.decoder",
            psionic_environments::EnvironmentPackageKey::new("env.train", "1"),
        )?;
        run.apply_cluster_membership_snapshot(&state, 1_000)?;
        run.select_contributors(2, 1_010)?;
        run.suspend_contributor(
            &NodeId::new("worker-a"),
            TrainingContributorSuspensionReason::ReliabilityPenalty,
            1_020,
        )?;
        let suspended = run
            .participants
            .iter()
            .find(|participant| participant.node_id.as_str() == "worker-a")
            .expect("worker-a must exist");
        assert_eq!(
            suspended.admission_state,
            TrainingParticipantAdmissionState::Admitted
        );
        assert_eq!(
            suspended.contribution_state,
            TrainingParticipantContributionState::Suspended
        );

        run.record_departure(
            &NodeId::new("worker-b"),
            TrainingParticipantDepartureReason::Evicted,
            1_030,
        )?;
        let evicted = run
            .participants
            .iter()
            .find(|participant| participant.node_id.as_str() == "worker-b")
            .expect("worker-b must exist");
        assert_eq!(
            evicted.admission_state,
            TrainingParticipantAdmissionState::Suspended
        );
        assert_eq!(
            evicted.last_departure_reason,
            Some(TrainingParticipantDepartureReason::Evicted)
        );
        Ok(())
    }

    #[test]
    fn windows_track_contributor_set_revisions_and_transitions()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = cluster_state();
        let mut run = TrainingRunState::new(
            "train-run-3",
            "stage-1",
            state.cluster_id().as_str(),
            "train.decoder",
            psionic_environments::EnvironmentPackageKey::new("env.train", "1"),
        )?;
        run.apply_cluster_membership_snapshot(&state, 1_000)?;
        run.select_contributors(2, 1_010)?;
        let first_window = run.plan_window(
            Some(String::from("policy-r1")),
            TrainingWindowAssignmentRule::RoundRobinByPriority {
                batch_slice_count: 4,
                eval_slice_count: 2,
            },
            1_020,
        )?;
        assert_eq!(first_window.status, TrainingWindowStatus::Planned);
        assert_eq!(first_window.batch_assignments.len(), 4);
        assert_eq!(first_window.eval_assignments.len(), 2);

        let first_window_id = first_window.window_id.clone();
        let first_contributor_set_revision_id = first_window.contributor_set_revision_id.clone();
        run.transition_window(&first_window_id, TrainingWindowStatus::Active, 1_030)?;
        run.transition_window(&first_window_id, TrainingWindowStatus::Sealed, 1_040)?;
        run.transition_window(&first_window_id, TrainingWindowStatus::Scored, 1_050)?;
        run.transition_window(&first_window_id, TrainingWindowStatus::Reconciled, 1_060)?;

        run.update_participant_priority(&NodeId::new("trainer-a"), 9_500, 9_000, 1_070)?;
        run.select_contributors(2, 1_080)?;
        let second_window = run.plan_window(
            Some(String::from("policy-r2")),
            TrainingWindowAssignmentRule::RoundRobinByPriority {
                batch_slice_count: 2,
                eval_slice_count: 1,
            },
            1_090,
        )?;
        assert_ne!(
            first_contributor_set_revision_id,
            second_window.contributor_set_revision_id
        );
        assert_eq!(run.windows.len(), 2);
        Ok(())
    }

    #[test]
    fn invalid_window_transition_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let state = cluster_state();
        let mut run = TrainingRunState::new(
            "train-run-4",
            "stage-1",
            state.cluster_id().as_str(),
            "train.decoder",
            psionic_environments::EnvironmentPackageKey::new("env.train", "1"),
        )?;
        run.apply_cluster_membership_snapshot(&state, 1_000)?;
        run.select_contributors(2, 1_010)?;
        let window = run.plan_window(
            None,
            TrainingWindowAssignmentRule::RoundRobinByPriority {
                batch_slice_count: 1,
                eval_slice_count: 0,
            },
            1_020,
        )?;
        let window_id = window.window_id.clone();
        let err = run
            .transition_window(&window_id, TrainingWindowStatus::Scored, 1_030)
            .expect_err("invalid transition should fail");
        assert!(matches!(
            err,
            TrainingRunGraphError::InvalidWindowTransition { .. }
        ));
        Ok(())
    }
}
