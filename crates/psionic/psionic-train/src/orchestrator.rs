use std::collections::BTreeMap;

use psionic_datastream::DatastreamPolicyWeightBroadcastManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PolicyRevision, RolloutArtifact, RolloutContractError, TrainingRunGraphError, TrainingRunState,
    TrainingWindowAssignmentRule, TrainingWindowStatus, TrainerBatch,
};

/// Control-plane failure for the train orchestrator.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TrainingOrchestratorError {
    /// The run-graph state rejected the requested transition.
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    /// Rollout or trainer-batch validation failed.
    #[error(transparent)]
    RolloutContract(#[from] RolloutContractError),
    /// The policy-weight broadcast does not match the run policy family.
    #[error(
        "policy-weight broadcast family mismatch: expected `{expected_policy_id}`, found `{actual_policy_id}`"
    )]
    PolicyWeightBroadcastMismatch {
        /// Expected policy family from the target revision.
        expected_policy_id: String,
        /// Actual policy id from the datastream broadcast.
        actual_policy_id: String,
    },
    /// The orchestrator currently has no active or planned window.
    #[error("training orchestrator has no current window")]
    MissingCurrentWindow,
    /// A new window was requested while the previous one is still unresolved.
    #[error(
        "training orchestrator cannot plan a new window while `{window_id}` is still `{status}`"
    )]
    WindowStillOpen {
        /// Current window id.
        window_id: String,
        /// Current window status.
        status: String,
    },
    /// The current window must be active before collecting rollouts.
    #[error(
        "training orchestrator window `{window_id}` must be active before collecting rollouts; found `{status}`"
    )]
    WindowNotCollecting {
        /// Current window id.
        window_id: String,
        /// Observed window status.
        status: String,
    },
    /// A submitted rollout came from a standby participant.
    #[error(
        "rollout `{artifact_id}` from worker `{worker_id}` is not assigned to window `{window_id}`"
    )]
    RolloutWorkerNotAssigned {
        /// Stable artifact identifier.
        artifact_id: String,
        /// Worker id on the rollout.
        worker_id: String,
        /// Window id.
        window_id: String,
    },
    /// A rollout targeted a different policy revision than the active window.
    #[error(
        "rollout `{artifact_id}` targeted policy revision `{actual_revision_id}` but orchestrator expects `{expected_revision_id}`"
    )]
    RolloutPolicyRevisionMismatch {
        /// Stable artifact identifier.
        artifact_id: String,
        /// Expected policy revision.
        expected_revision_id: String,
        /// Actual policy revision.
        actual_revision_id: String,
    },
    /// One requested rollout id was not present in the current window.
    #[error("training orchestrator window `{window_id}` does not know rollout `{artifact_id}`")]
    UnknownRolloutArtifact {
        /// Current window id.
        window_id: String,
        /// Missing rollout id.
        artifact_id: String,
    },
    /// Trainer-batch assembly requires the window to be sealed or later.
    #[error(
        "training orchestrator window `{window_id}` must be sealed before trainer-batch assembly; found `{status}`"
    )]
    WindowNotSealed {
        /// Current window id.
        window_id: String,
        /// Observed status.
        status: String,
    },
}

/// Deterministic assignment posture for one orchestrated window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingWindowAssignmentPosture {
    /// Stable seed for deterministic task assignment.
    pub assignment_seed: u64,
    /// Target policy revision id.
    pub policy_revision_id: String,
    /// Lightweight policy-weight broadcast digest.
    pub policy_weight_broadcast_digest: String,
    /// Stable digest over the posture.
    pub posture_digest: String,
}

impl TrainingWindowAssignmentPosture {
    fn new(
        assignment_seed: u64,
        policy_revision_id: impl Into<String>,
        policy_weight_broadcast_digest: impl Into<String>,
    ) -> Self {
        let policy_revision_id = policy_revision_id.into();
        let policy_weight_broadcast_digest = policy_weight_broadcast_digest.into();
        let posture_digest = stable_assignment_posture_digest(
            assignment_seed,
            policy_revision_id.as_str(),
            policy_weight_broadcast_digest.as_str(),
        );
        Self {
            assignment_seed,
            policy_revision_id,
            policy_weight_broadcast_digest,
            posture_digest,
        }
    }
}

/// Lightweight rollout work assignment for one contributor and batch slice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutWorkAssignment {
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Window id that owns the assignment.
    pub window_id: String,
    /// Assigned contributor node id.
    pub contributor_node_id: String,
    /// Stable batch-slice index from the run graph.
    pub batch_slice_index: u32,
    /// Environment identity for the worker.
    pub environment_key: String,
    /// Policy revision id the worker should use.
    pub policy_revision_id: String,
    /// Lightweight weight broadcast digest.
    pub policy_weight_broadcast_digest: String,
    /// Manifest digests for the heavy weight shards.
    pub policy_weight_shard_manifest_digests: Vec<String>,
    /// Stable assignment digest.
    pub assignment_digest: String,
}

/// Lightweight eval work assignment interleaved with the current window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingEvalWorkAssignment {
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Window id that owns the assignment.
    pub window_id: String,
    /// Assigned contributor node id.
    pub contributor_node_id: String,
    /// Stable eval-slice index from the run graph.
    pub eval_slice_index: u32,
    /// Policy revision id the sampled eval should score.
    pub policy_revision_id: String,
    /// Stable assignment digest.
    pub assignment_digest: String,
}

/// Lightweight control-plane ref for one accepted rollout artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutArtifactRef {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// Worker id that produced the rollout.
    pub worker_id: String,
    /// Source policy revision id.
    pub policy_revision_id: String,
    /// Stable task id.
    pub task_id: String,
    /// Stable token count.
    pub token_count: u64,
    /// Stable proof digests carried by the rollout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proof_digests: Vec<String>,
    /// Stable control-plane digest.
    pub reference_digest: String,
}

impl RolloutArtifactRef {
    fn from_artifact(artifact: &RolloutArtifact) -> Self {
        let proof_digests = artifact
            .proof_references
            .iter()
            .map(|proof| proof.digest.clone())
            .collect::<Vec<_>>();
        let reference_digest = stable_rollout_reference_digest(
            artifact.artifact_id.as_str(),
            artifact.artifact_digest.as_str(),
            artifact.worker_id.as_str(),
            artifact.source_policy_revision.revision_id.as_str(),
            artifact.task_id.as_str(),
            artifact.token_count(),
            proof_digests.as_slice(),
        );
        Self {
            artifact_id: artifact.artifact_id.clone(),
            artifact_digest: artifact.artifact_digest.clone(),
            worker_id: artifact.worker_id.clone(),
            policy_revision_id: artifact.source_policy_revision.revision_id.clone(),
            task_id: artifact.task_id.clone(),
            token_count: artifact.token_count(),
            proof_digests,
            reference_digest,
        }
    }
}

/// Lightweight trainer-batch assembly request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainerBatchAssemblyRequest {
    /// Stable batch id.
    pub batch_id: String,
    /// Window id owning the batch.
    pub window_id: String,
    /// Contributor-set revision used by the window.
    pub contributor_set_revision_id: String,
    /// Target policy revision id.
    pub policy_revision_id: String,
    /// Rollout ids selected for the batch.
    pub rollout_ids: Vec<String>,
    /// Rollout digests selected for the batch.
    pub rollout_digests: Vec<String>,
    /// Weight broadcast digest bound to the assignments.
    pub policy_weight_broadcast_digest: String,
    /// Stable request digest.
    pub request_digest: String,
}

/// One assembled trainer-batch record owned by the orchestrator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingOrchestratorBatchRecord {
    /// Lightweight assembly request.
    pub request: TrainerBatchAssemblyRequest,
    /// Assembled trainer batch.
    pub batch: TrainerBatch,
}

/// One accepted rollout stored under the current window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AcceptedRolloutRecord {
    /// Lightweight rollout ref.
    pub reference: RolloutArtifactRef,
    /// Full rollout artifact retained for trainer-batch assembly.
    pub artifact: RolloutArtifact,
}

/// Inspectable orchestrator view for one window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingOrchestratorWindow {
    /// Stable window id.
    pub window_id: String,
    /// Contributor-set revision used by this window.
    pub contributor_set_revision_id: String,
    /// Deterministic assignment posture.
    pub assignment_posture: TrainingWindowAssignmentPosture,
    /// Lightweight rollout work assignments.
    pub rollout_assignments: Vec<RolloutWorkAssignment>,
    /// Lightweight sampled eval assignments.
    pub eval_assignments: Vec<TrainingEvalWorkAssignment>,
    /// Accepted rollout artifacts for this window.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_rollouts: Vec<AcceptedRolloutRecord>,
    /// Trainer batches assembled for this window.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trainer_batches: Vec<TrainingOrchestratorBatchRecord>,
}

impl TrainingOrchestratorWindow {
    fn assigned_contributor_node_ids(&self) -> Vec<String> {
        let mut node_ids = self
            .rollout_assignments
            .iter()
            .map(|assignment| assignment.contributor_node_id.clone())
            .collect::<Vec<_>>();
        node_ids.sort();
        node_ids.dedup();
        node_ids
    }
}

/// First-class train orchestrator state over the run graph and rollout contracts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingOrchestratorState {
    /// Inspectable run graph owned by the orchestrator.
    pub run: TrainingRunState,
    /// Active target policy revision.
    pub target_policy_revision: PolicyRevision,
    /// Lightweight policy-weight broadcast for the active policy revision.
    pub policy_weight_broadcast: DatastreamPolicyWeightBroadcastManifest,
    /// Current planned or active window id when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_window_id: Option<String>,
    /// Inspectable orchestrator windows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orchestrator_windows: Vec<TrainingOrchestratorWindow>,
}

impl TrainingOrchestratorState {
    /// Creates an orchestrator from the run graph and active policy-weight broadcast.
    pub fn new(
        run: TrainingRunState,
        target_policy_revision: PolicyRevision,
        policy_weight_broadcast: DatastreamPolicyWeightBroadcastManifest,
    ) -> Result<Self, TrainingOrchestratorError> {
        if policy_weight_broadcast.policy_id != target_policy_revision.policy_family {
            return Err(TrainingOrchestratorError::PolicyWeightBroadcastMismatch {
                expected_policy_id: target_policy_revision.policy_family.clone(),
                actual_policy_id: policy_weight_broadcast.policy_id.clone(),
            });
        }
        Ok(Self {
            run,
            target_policy_revision,
            policy_weight_broadcast,
            current_window_id: None,
            orchestrator_windows: Vec::new(),
        })
    }

    /// Plans the next orchestrated window from the wider admitted participant set.
    pub fn plan_next_window(
        &mut self,
        max_contributors: usize,
        assignment_rule: TrainingWindowAssignmentRule,
        assignment_seed: u64,
        planned_at_ms: u64,
    ) -> Result<TrainingOrchestratorWindow, TrainingOrchestratorError> {
        if let Some(current_window_id) = &self.current_window_id {
            let current_window = self
                .run
                .windows
                .iter()
                .find(|window| &window.window_id == current_window_id)
                .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
            if current_window.status != TrainingWindowStatus::Reconciled {
                return Err(TrainingOrchestratorError::WindowStillOpen {
                    window_id: current_window.window_id.clone(),
                    status: training_window_status_label(current_window.status).to_string(),
                });
            }
        }

        let contributor_set_revision_id = self
            .run
            .select_contributors(max_contributors, planned_at_ms)?
            .contributor_set_revision_id
            .clone();
        let environment_key = self.run.environment.storage_key();
        let window = self.run.plan_window(
            Some(self.target_policy_revision.revision_id.clone()),
            assignment_rule,
            planned_at_ms,
        )?;
        let assignment_posture = TrainingWindowAssignmentPosture::new(
            assignment_seed,
            self.target_policy_revision.revision_id.clone(),
            self.policy_weight_broadcast.broadcast_digest.clone(),
        );
        let policy_weight_shard_manifest_digests = self
            .policy_weight_broadcast
            .shards
            .iter()
            .map(|shard| shard.manifest_digest.clone())
            .collect::<Vec<_>>();
        let rollout_assignments = window
            .batch_assignments
            .iter()
            .map(|assignment| RolloutWorkAssignment {
                assignment_id: format!("{}-rollout-{}", window.window_id, assignment.slice_index),
                window_id: window.window_id.clone(),
                contributor_node_id: assignment.node_id.clone(),
                batch_slice_index: assignment.slice_index,
                environment_key: environment_key.clone(),
                policy_revision_id: self.target_policy_revision.revision_id.clone(),
                policy_weight_broadcast_digest: self.policy_weight_broadcast.broadcast_digest.clone(),
                policy_weight_shard_manifest_digests: policy_weight_shard_manifest_digests.clone(),
                assignment_digest: stable_rollout_assignment_digest(
                    window.window_id.as_str(),
                    assignment.slice_index,
                    assignment.node_id.as_str(),
                    environment_key.as_str(),
                    self.target_policy_revision.revision_id.as_str(),
                    self.policy_weight_broadcast.broadcast_digest.as_str(),
                    assignment_posture.assignment_seed,
                ),
            })
            .collect::<Vec<_>>();
        let eval_assignments = window
            .eval_assignments
            .iter()
            .map(|assignment| TrainingEvalWorkAssignment {
                assignment_id: format!("{}-eval-{}", window.window_id, assignment.slice_index),
                window_id: window.window_id.clone(),
                contributor_node_id: assignment.node_id.clone(),
                eval_slice_index: assignment.slice_index,
                policy_revision_id: self.target_policy_revision.revision_id.clone(),
                assignment_digest: stable_eval_assignment_digest(
                    window.window_id.as_str(),
                    assignment.slice_index,
                    assignment.node_id.as_str(),
                    self.target_policy_revision.revision_id.as_str(),
                    assignment_posture.assignment_seed,
                ),
            })
            .collect::<Vec<_>>();
        self.current_window_id = Some(window.window_id.clone());
        self.orchestrator_windows.push(TrainingOrchestratorWindow {
            window_id: window.window_id.clone(),
            contributor_set_revision_id,
            assignment_posture,
            rollout_assignments,
            eval_assignments,
            accepted_rollouts: Vec::new(),
            trainer_batches: Vec::new(),
        });
        self.orchestrator_windows
            .last()
            .cloned()
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)
    }

    /// Activates the current window.
    pub fn activate_current_window(
        &mut self,
        active_at_ms: u64,
    ) -> Result<(), TrainingOrchestratorError> {
        let window_id = self
            .current_window_id
            .clone()
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
        self.run
            .transition_window(window_id.as_str(), TrainingWindowStatus::Active, active_at_ms)?;
        Ok(())
    }

    /// Records one rollout artifact against the current window.
    pub fn submit_rollout(
        &mut self,
        artifact: RolloutArtifact,
    ) -> Result<RolloutArtifactRef, TrainingOrchestratorError> {
        let current_window_id = self
            .current_window_id
            .clone()
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
        let current_window = self
            .run
            .windows
            .iter()
            .find(|candidate| candidate.window_id == current_window_id)
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
        let expected_revision_id = self.target_policy_revision.revision_id.clone();
        if current_window.status != TrainingWindowStatus::Active {
            return Err(TrainingOrchestratorError::WindowNotCollecting {
                window_id: current_window_id,
                status: training_window_status_label(current_window.status).to_string(),
            });
        }
        if artifact.source_policy_revision.revision_id != expected_revision_id {
            return Err(TrainingOrchestratorError::RolloutPolicyRevisionMismatch {
                artifact_id: artifact.artifact_id.clone(),
                expected_revision_id,
                actual_revision_id: artifact.source_policy_revision.revision_id.clone(),
            });
        }
        let window = self.current_window_mut()?;
        if !window
            .assigned_contributor_node_ids()
            .contains(&artifact.worker_id)
        {
            return Err(TrainingOrchestratorError::RolloutWorkerNotAssigned {
                artifact_id: artifact.artifact_id.clone(),
                worker_id: artifact.worker_id.clone(),
                window_id: window.window_id.clone(),
            });
        }
        let reference = RolloutArtifactRef::from_artifact(&artifact);
        window.accepted_rollouts.push(AcceptedRolloutRecord {
            reference: reference.clone(),
            artifact,
        });
        Ok(reference)
    }

    /// Seals the current window.
    pub fn seal_current_window(
        &mut self,
        sealed_at_ms: u64,
    ) -> Result<(), TrainingOrchestratorError> {
        let window_id = self
            .current_window_id
            .clone()
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
        self.run
            .transition_window(window_id.as_str(), TrainingWindowStatus::Sealed, sealed_at_ms)?;
        Ok(())
    }

    /// Assembles a trainer batch from accepted rollout refs in the current window.
    pub fn assemble_trainer_batch(
        &mut self,
        batch_id: impl Into<String>,
        rollout_ids: Vec<String>,
        assembled_at_ms: u64,
    ) -> Result<TrainingOrchestratorBatchRecord, TrainingOrchestratorError> {
        let batch_id = batch_id.into();
        let window_id = self
            .current_window_id
            .clone()
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
        let current_window = self
            .run
            .windows
            .iter()
            .find(|candidate| candidate.window_id == window_id)
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
        if current_window.status != TrainingWindowStatus::Sealed
            && current_window.status != TrainingWindowStatus::Scored
            && current_window.status != TrainingWindowStatus::Reconciled
        {
            return Err(TrainingOrchestratorError::WindowNotSealed {
                window_id: current_window.window_id.clone(),
                status: training_window_status_label(current_window.status).to_string(),
            });
        }

        let target_policy_revision = self.target_policy_revision.clone();
        let target_policy_revision_id = target_policy_revision.revision_id.clone();
        let policy_weight_broadcast_digest = self.policy_weight_broadcast.broadcast_digest.clone();
        let window = self.current_window_mut()?;
        let mut selected_rollouts = Vec::new();
        let mut rollout_digests = Vec::new();
        let accepted_rollouts_by_id = window
            .accepted_rollouts
            .iter()
            .map(|record| (record.reference.artifact_id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        for rollout_id in &rollout_ids {
            let record = accepted_rollouts_by_id
                .get(rollout_id)
                .ok_or_else(|| TrainingOrchestratorError::UnknownRolloutArtifact {
                    window_id: window.window_id.clone(),
                    artifact_id: rollout_id.clone(),
                })?;
            rollout_digests.push(record.reference.artifact_digest.clone());
            selected_rollouts.push(record.artifact.clone());
        }
        let request = TrainerBatchAssemblyRequest {
            batch_id: batch_id.clone(),
            window_id: window.window_id.clone(),
            contributor_set_revision_id: window.contributor_set_revision_id.clone(),
            policy_revision_id: target_policy_revision_id.clone(),
            rollout_ids: rollout_ids.clone(),
            rollout_digests,
            policy_weight_broadcast_digest: policy_weight_broadcast_digest.clone(),
            request_digest: stable_batch_request_digest(
                batch_id.as_str(),
                window.window_id.as_str(),
                window.contributor_set_revision_id.as_str(),
                target_policy_revision_id.as_str(),
                rollout_ids.as_slice(),
                policy_weight_broadcast_digest.as_str(),
            ),
        };
        let batch = TrainerBatch::assemble(
            batch_id,
            target_policy_revision,
            selected_rollouts,
            assembled_at_ms,
        )?;
        let record = TrainingOrchestratorBatchRecord { request, batch };
        window.trainer_batches.push(record.clone());
        window
            .trainer_batches
            .last()
            .cloned()
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)
    }

    /// Scores the current window.
    pub fn score_current_window(
        &mut self,
        scored_at_ms: u64,
    ) -> Result<(), TrainingOrchestratorError> {
        let window_id = self
            .current_window_id
            .clone()
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
        self.run
            .transition_window(window_id.as_str(), TrainingWindowStatus::Scored, scored_at_ms)?;
        Ok(())
    }

    /// Reconciles the current window and clears the active-window pointer.
    pub fn reconcile_current_window(
        &mut self,
        reconciled_at_ms: u64,
    ) -> Result<(), TrainingOrchestratorError> {
        let window_id = self
            .current_window_id
            .clone()
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
        self.run.transition_window(
            window_id.as_str(),
            TrainingWindowStatus::Reconciled,
            reconciled_at_ms,
        )?;
        self.current_window_id = None;
        Ok(())
    }

    fn current_window_mut(
        &mut self,
    ) -> Result<&mut TrainingOrchestratorWindow, TrainingOrchestratorError> {
        let window_id = self
            .current_window_id
            .clone()
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)?;
        self.orchestrator_windows
            .iter_mut()
            .find(|window| window.window_id == window_id)
            .ok_or(TrainingOrchestratorError::MissingCurrentWindow)
    }
}

fn stable_assignment_posture_digest(
    assignment_seed: u64,
    policy_revision_id: &str,
    policy_weight_broadcast_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_assignment_posture|");
    hasher.update(assignment_seed.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(policy_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(policy_weight_broadcast_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_rollout_assignment_digest(
    window_id: &str,
    batch_slice_index: u32,
    contributor_node_id: &str,
    environment_key: &str,
    policy_revision_id: &str,
    policy_weight_broadcast_digest: &str,
    assignment_seed: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_rollout_assignment|");
    hasher.update(window_id.as_bytes());
    hasher.update(b"|");
    hasher.update(batch_slice_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(contributor_node_id.as_bytes());
    hasher.update(b"|");
    hasher.update(environment_key.as_bytes());
    hasher.update(b"|");
    hasher.update(policy_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(policy_weight_broadcast_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(assignment_seed.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_eval_assignment_digest(
    window_id: &str,
    eval_slice_index: u32,
    contributor_node_id: &str,
    policy_revision_id: &str,
    assignment_seed: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_eval_assignment|");
    hasher.update(window_id.as_bytes());
    hasher.update(b"|");
    hasher.update(eval_slice_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(contributor_node_id.as_bytes());
    hasher.update(b"|");
    hasher.update(policy_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(assignment_seed.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_rollout_reference_digest(
    artifact_id: &str,
    artifact_digest: &str,
    worker_id: &str,
    policy_revision_id: &str,
    task_id: &str,
    token_count: u64,
    proof_digests: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_rollout_ref|");
    hasher.update(artifact_id.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(policy_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(task_id.as_bytes());
    hasher.update(b"|");
    hasher.update(token_count.to_string().as_bytes());
    for digest in proof_digests {
        hasher.update(b"|proof|");
        hasher.update(digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_batch_request_digest(
    batch_id: &str,
    window_id: &str,
    contributor_set_revision_id: &str,
    policy_revision_id: &str,
    rollout_ids: &[String],
    policy_weight_broadcast_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_batch_request|");
    hasher.update(batch_id.as_bytes());
    hasher.update(b"|");
    hasher.update(window_id.as_bytes());
    hasher.update(b"|");
    hasher.update(contributor_set_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(policy_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(policy_weight_broadcast_digest.as_bytes());
    for rollout_id in rollout_ids {
        hasher.update(b"|rollout|");
        hasher.update(rollout_id.as_bytes());
    }
    hex::encode(hasher.finalize())
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

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use psionic_cluster::{
        AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterMembershipStatus,
        ClusterNamespace, ClusterNodeIdentity, ClusterSnapshot, NodeEpoch, NodeId, NodeRole,
    };
    use psionic_datastream::{
        DatastreamEncoding, DatastreamPolicyWeightBinding, DatastreamSubjectKind,
        InMemoryDatastreamServer, InMemoryPolicyWeightBroadcast,
    };
    use psionic_environments::EnvironmentPackageKey;
    use sha2::{Digest, Sha256};

    use super::{TrainingOrchestratorError, TrainingOrchestratorState};
    use crate::{
        PolicyRevision, RolloutArtifact, RolloutProofKind, RolloutProofReference, RolloutSample,
        TrainingRunState, TrainingWindowAssignmentRule,
    };

    fn cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("train-orchestrator"),
            &AdmissionToken::new("shared-secret"),
        )
    }

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = BTreeMap::from([
            (
                NodeId::new("trainer-a"),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id: cluster_id.clone(),
                        node_id: NodeId::new("trainer-a"),
                        node_epoch: NodeEpoch::initial(),
                        role: NodeRole::CoordinatorOnly,
                        auth_public_key: String::from("trainer-a-pk"),
                        attestation: None,
                    },
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 31_000)),
                    ClusterMembershipStatus::Ready,
                ),
            ),
            (
                NodeId::new("worker-b"),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id: cluster_id.clone(),
                        node_id: NodeId::new("worker-b"),
                        node_epoch: NodeEpoch::initial(),
                        role: NodeRole::ExecutorOnly,
                        auth_public_key: String::from("worker-b-pk"),
                        attestation: None,
                    },
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 31_001)),
                    ClusterMembershipStatus::Ready,
                ),
            ),
            (
                NodeId::new("worker-c"),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id,
                        node_id: NodeId::new("worker-c"),
                        node_epoch: NodeEpoch::initial(),
                        role: NodeRole::ExecutorOnly,
                        auth_public_key: String::from("worker-c-pk"),
                        attestation: None,
                    },
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 31_002)),
                    ClusterMembershipStatus::Ready,
                ),
            ),
        ]);
        psionic_cluster::ClusterState::from_snapshot(snapshot)
    }

    fn policy_weight_broadcast()
    -> Result<psionic_datastream::DatastreamPolicyWeightBroadcastManifest, Box<dyn std::error::Error>>
    {
        let shard_a = b"weights-a".repeat(16);
        let shard_b = b"weights-b".repeat(16);
        let assembled = {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&shard_a);
            bytes.extend_from_slice(&shard_b);
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            hex::encode(hasher.finalize())
        };
        let manifest_a = psionic_datastream::DatastreamManifest::from_bytes(
            "policy-shard-a",
            DatastreamSubjectKind::PolicyWeights,
            &shard_a,
            8,
            DatastreamEncoding::Safetensors,
        )
        .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
            "train.decoder",
            7,
            "shard-a",
            0,
            2,
            assembled.clone(),
            1_000,
            10_000,
        ));
        let manifest_b = psionic_datastream::DatastreamManifest::from_bytes(
            "policy-shard-b",
            DatastreamSubjectKind::PolicyWeights,
            &shard_b,
            8,
            DatastreamEncoding::Safetensors,
        )
        .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
            "train.decoder",
            7,
            "shard-b",
            1,
            2,
            assembled,
            1_000,
            10_000,
        ));
        Ok(
            InMemoryPolicyWeightBroadcast::new(
                vec![
                    InMemoryDatastreamServer::new(manifest_a, shard_a)?,
                    InMemoryDatastreamServer::new(manifest_b, shard_b)?,
                ],
                1_500,
            )?
            .broadcast()
            .clone(),
        )
    }

    fn orchestrator() -> Result<TrainingOrchestratorState, Box<dyn std::error::Error>> {
        let state = cluster_state();
        let environment = EnvironmentPackageKey::new("oa.train", "2026.03");
        let mut run = TrainingRunState::new(
            "run-1",
            "stage-rl",
            state.cluster_id().as_str(),
            "train.decoder",
            environment,
        )?;
        run.apply_cluster_membership_snapshot(&state, 1_000)?;
        run.update_participant_priority(&NodeId::new("worker-b"), 9_200, 9_000, 1_010)?;
        run.update_participant_priority(&NodeId::new("trainer-a"), 8_700, 8_500, 1_020)?;
        run.update_participant_priority(&NodeId::new("worker-c"), 4_800, 4_900, 1_030)?;
        let policy_revision =
            PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100);
        Ok(TrainingOrchestratorState::new(
            run,
            policy_revision,
            policy_weight_broadcast()?,
        )?)
    }

    fn rollout(
        worker_id: &str,
        artifact_id: &str,
        target_policy_revision: PolicyRevision,
    ) -> Result<RolloutArtifact, Box<dyn std::error::Error>> {
        Ok(RolloutArtifact::new(
            artifact_id,
            worker_id,
            EnvironmentPackageKey::new("oa.train", "2026.03"),
            format!("task-{artifact_id}"),
            target_policy_revision,
            vec![
                RolloutSample::new(1, -0.2, 1.0, 0.8),
                RolloutSample::new(2, -0.1, 0.6, 0.4),
            ],
            crate::RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                format!("proof-{artifact_id}"),
                format!("exec://{artifact_id}"),
            )],
            1_200,
        )?)
    }

    #[test]
    fn orchestrator_window_selection_and_batch_assembly_are_typed()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut orchestrator = orchestrator()?;
        let window = orchestrator.plan_next_window(
            2,
            TrainingWindowAssignmentRule::RoundRobinByPriority {
                batch_slice_count: 3,
                eval_slice_count: 1,
            },
            42,
            1_100,
        )?;
        let contributor_count = window.assigned_contributor_node_ids().len();
        assert_eq!(orchestrator.run.participants.len(), 3);
        assert_eq!(contributor_count, 2);
        assert_eq!(window.eval_assignments.len(), 1);
        assert_eq!(
            window.assignment_posture.policy_weight_broadcast_digest,
            orchestrator.policy_weight_broadcast.broadcast_digest
        );

        orchestrator.activate_current_window(1_110)?;
        let policy_revision = orchestrator.target_policy_revision.clone();
        orchestrator.submit_rollout(rollout("worker-b", "artifact-b", policy_revision.clone())?)?;
        orchestrator.submit_rollout(rollout("trainer-a", "artifact-a", policy_revision)?)?;
        orchestrator.seal_current_window(1_120)?;
        let batch = orchestrator.assemble_trainer_batch(
            "batch-1",
            vec![String::from("artifact-b")],
            1_130,
        )?;
        assert_eq!(batch.batch.rollout_count, 1);
        assert_eq!(contributor_count, 2);
        assert_ne!(orchestrator.run.participants.len(), batch.batch.rollout_count as usize);
        assert_ne!(
            contributor_count,
            batch.batch.rollout_count as usize
        );
        orchestrator.score_current_window(1_140)?;
        orchestrator.reconcile_current_window(1_150)?;
        assert!(orchestrator.current_window_id.is_none());
        Ok(())
    }

    #[test]
    fn orchestrator_refuses_rollout_from_standby_participant()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut orchestrator = orchestrator()?;
        orchestrator.plan_next_window(
            2,
            TrainingWindowAssignmentRule::RoundRobinByPriority {
                batch_slice_count: 2,
                eval_slice_count: 1,
            },
            11,
            1_100,
        )?;
        orchestrator.activate_current_window(1_110)?;
        let error = orchestrator
            .submit_rollout(rollout(
                "worker-c",
                "artifact-c",
                orchestrator.target_policy_revision.clone(),
            )?)
            .expect_err("standby worker should be refused");
        assert_eq!(
            error,
            TrainingOrchestratorError::RolloutWorkerNotAssigned {
                artifact_id: String::from("artifact-c"),
                worker_id: String::from("worker-c"),
                window_id: String::from("run-1-window-1"),
            }
        );
        Ok(())
    }
}
