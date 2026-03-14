use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use psionic_cluster::{AdmissionToken, ClusterNamespace};
use psionic_cluster::{
    ClusterId, ClusterMembershipRecord, ClusterMembershipStatus, ClusterNodeIdentity,
    ClusterSnapshot, ClusterState, NodeEpoch, NodeId, NodeRole,
};
use psionic_collectives::{
    CollectiveMeshMember, CollectiveReplanTriggerKind, CollectiveSyncCadenceClass,
    CollectiveSyncCadencePolicy, CollectiveTransportFeedback, ElasticCollectivePlanner,
    QuantizedCollectiveBenchmark, QuantizedCollectiveBenchmarkSample,
};
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest,
    DatastreamPolicyWeightBinding, DatastreamSubjectKind, DatastreamTransferError,
    InMemoryDatastreamServer, InMemoryPolicyWeightBroadcast,
};
use psionic_environments::EnvironmentPackageKey;
use psionic_runtime::{
    ClusterCommunicationClass, TrainingCollectiveKind, TrainingCollectiveQuantization,
    TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind, TrainingRecoveryPosture,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CheckpointDurabilityPosture, CheckpointReadSourceKind, CheckpointScopeBinding,
    CheckpointScopeKind, CheckpointStoreReadOptions, InMemoryCheckpointStore, PolicyRevision,
    RolloutAdmissionReceipt, RolloutArtifact, RolloutProofKind, RolloutProofReference,
    RolloutReceiptOutcome, RolloutSample, RolloutTerminationReason, RolloutUploadLocator,
    RolloutUploadTransport, RolloutValidatorPolicy, RolloutValidatorState,
    RolloutVerificationBundle, RolloutWorkerOutcomeKind, RolloutWorkerOutcomeReceipt,
    RolloutWorkerPolicyPosture, RolloutWorkerTrustClass, TrainingOffPolicyBudget,
    TrainingOrchestratorError, TrainingOrchestratorState, TrainingRecoveryAction,
    TrainingRecoveryMode, TrainingRunGraphError, TrainingRunState, TrainingSessionError,
    TrainingSessionState, TrainingWindowAssignmentRule, ValidatorDisposition,
};

/// Error returned by the train reliability suite.
#[derive(Debug, Error)]
pub enum TrainReliabilityError {
    /// Training-session recovery logic failed.
    #[error(transparent)]
    TrainingSession(#[from] TrainingSessionError),
    /// Checkpoint-recovery planning failed.
    #[error(transparent)]
    CheckpointRecovery(#[from] crate::CheckpointRecoveryError),
    /// Collective planning failed.
    #[error(transparent)]
    CollectivePlanning(#[from] psionic_collectives::CollectivePlanningError),
    /// Orchestrator logic failed.
    #[error(transparent)]
    Orchestrator(#[from] TrainingOrchestratorError),
    /// Run-graph control-plane logic failed.
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    /// Rollout artifacts or trainer batches were invalid.
    #[error(transparent)]
    RolloutContract(#[from] crate::RolloutContractError),
    /// Datastream or policy-weight broadcast logic failed.
    #[error(transparent)]
    Datastream(#[from] DatastreamTransferError),
    /// State round-tripping failed.
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

/// Fault class exercised by one reliability scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainReliabilityScenarioKind {
    /// Topology churn and elastic reconfiguration.
    TopologyChurn,
    /// Network degradation and collective fallback.
    NetworkDegradation,
    /// Stale-weight or stale-policy flood against rollout admission.
    StaleWeightFlood,
    /// Checkpoint corruption or stale-pointer fallback.
    CheckpointCorruption,
    /// Validator sampling under replay and stale-policy pressure.
    ValidatorSamplingStress,
    /// Orchestrator restart and state recovery.
    OrchestratorRestartRecovery,
}

/// Reference scenario spec for one reliability drill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainReliabilityScenarioSpec {
    /// Stable scenario identifier.
    pub scenario_id: String,
    /// Fault class exercised by the scenario.
    pub kind: TrainReliabilityScenarioKind,
    /// Number of repeated fault injections for this drill.
    pub injection_count: usize,
}

impl TrainReliabilityScenarioSpec {
    /// Creates a scenario spec.
    #[must_use]
    pub fn new(
        scenario_id: impl Into<String>,
        kind: TrainReliabilityScenarioKind,
        injection_count: usize,
    ) -> Self {
        Self {
            scenario_id: scenario_id.into(),
            kind,
            injection_count: injection_count.max(1),
        }
    }

    /// Returns the canonical reference suite.
    #[must_use]
    pub fn reference_suite() -> Vec<Self> {
        vec![
            Self::new(
                "topology-churn",
                TrainReliabilityScenarioKind::TopologyChurn,
                2,
            ),
            Self::new(
                "network-degradation",
                TrainReliabilityScenarioKind::NetworkDegradation,
                1,
            ),
            Self::new(
                "stale-weight-flood",
                TrainReliabilityScenarioKind::StaleWeightFlood,
                6,
            ),
            Self::new(
                "checkpoint-corruption",
                TrainReliabilityScenarioKind::CheckpointCorruption,
                1,
            ),
            Self::new(
                "validator-sampling-stress",
                TrainReliabilityScenarioKind::ValidatorSamplingStress,
                8,
            ),
            Self::new(
                "orchestrator-restart",
                TrainReliabilityScenarioKind::OrchestratorRestartRecovery,
                1,
            ),
        ]
    }
}

/// Machine-readable signal emitted by one reliability scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainReliabilitySignalKind {
    /// Cluster membership or mesh revision changed.
    TopologyChangeObserved,
    /// Recovery plan or checkpoint-backed fencing was activated.
    RecoveryPlanIssued,
    /// Transport degraded and collective cadence adapted.
    NetworkFallbackActivated,
    /// Stale flood was discarded or quarantined instead of accepted.
    StaleFloodContained,
    /// Restore fell back away from a stale or corrupt checkpoint source.
    CheckpointFallbackSelected,
    /// Validator stress produced accepted, normalized, and rejected outcomes.
    ValidatorStressMixedOutcomes,
    /// State round-tripped through restart and resumed work.
    RestartStateRecovered,
}

/// One signal emitted by a reliability scenario.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainReliabilitySignal {
    /// Signal family.
    pub kind: TrainReliabilitySignalKind,
    /// Plain-language detail.
    pub detail: String,
    /// Observed count or metric attached to the signal.
    pub observed_value: u64,
}

impl TrainReliabilitySignal {
    fn new(
        kind: TrainReliabilitySignalKind,
        detail: impl Into<String>,
        observed_value: u64,
    ) -> Self {
        Self {
            kind,
            detail: detail.into(),
            observed_value,
        }
    }
}

/// Final disposition for one reliability scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainReliabilityDisposition {
    /// The scenario passed without needing a degraded fallback.
    Passed,
    /// The fault was recovered or contained through an explicit fallback path.
    RecoveredAfterFault,
    /// The scenario failed to meet its containment or recovery goal.
    Failed,
}

/// Receipt for one scenario in the reliability suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainReliabilityScenarioReceipt {
    /// Stable scenario identifier.
    pub scenario_id: String,
    /// Fault class exercised by the scenario.
    pub kind: TrainReliabilityScenarioKind,
    /// Number of injected faults exercised.
    pub injection_count: usize,
    /// Final scenario disposition.
    pub disposition: TrainReliabilityDisposition,
    /// Machine-readable scenario signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<TrainReliabilitySignal>,
    /// Stable digest over the scenario receipt.
    pub receipt_digest: String,
}

/// Receipt for the full reliability suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainReliabilitySuiteReceipt {
    /// Scenario receipts emitted by the suite.
    pub scenario_receipts: Vec<TrainReliabilityScenarioReceipt>,
    /// Number of scenarios that passed directly.
    pub passed_count: usize,
    /// Number of scenarios that recovered after a fault.
    pub recovered_count: usize,
    /// Number of failed scenarios.
    pub failed_count: usize,
    /// Stable digest over the suite receipt.
    pub receipt_digest: String,
}

/// Train-owned reliability harness over the existing substrate.
pub struct TrainReliabilityHarness;

impl TrainReliabilityHarness {
    /// Runs one scenario from the canonical reliability suite.
    pub fn run_scenario(
        spec: &TrainReliabilityScenarioSpec,
    ) -> Result<TrainReliabilityScenarioReceipt, TrainReliabilityError> {
        match spec.kind {
            TrainReliabilityScenarioKind::TopologyChurn => {
                topology_churn_scenario(spec.scenario_id.as_str(), spec.injection_count)
            }
            TrainReliabilityScenarioKind::NetworkDegradation => {
                network_degradation_scenario(spec.scenario_id.as_str())
            }
            TrainReliabilityScenarioKind::StaleWeightFlood => {
                stale_weight_flood_scenario(spec.scenario_id.as_str(), spec.injection_count)
            }
            TrainReliabilityScenarioKind::CheckpointCorruption => {
                checkpoint_corruption_scenario(spec.scenario_id.as_str())
            }
            TrainReliabilityScenarioKind::ValidatorSamplingStress => {
                validator_sampling_stress_scenario(spec.scenario_id.as_str(), spec.injection_count)
            }
            TrainReliabilityScenarioKind::OrchestratorRestartRecovery => {
                orchestrator_restart_scenario(spec.scenario_id.as_str())
            }
        }
    }

    /// Runs the canonical reference suite.
    pub fn run_reference_suite() -> Result<TrainReliabilitySuiteReceipt, TrainReliabilityError> {
        let scenario_receipts = TrainReliabilityScenarioSpec::reference_suite()
            .iter()
            .map(Self::run_scenario)
            .collect::<Result<Vec<_>, _>>()?;
        let passed_count = scenario_receipts
            .iter()
            .filter(|receipt| receipt.disposition == TrainReliabilityDisposition::Passed)
            .count();
        let recovered_count = scenario_receipts
            .iter()
            .filter(|receipt| {
                receipt.disposition == TrainReliabilityDisposition::RecoveredAfterFault
            })
            .count();
        let failed_count = scenario_receipts
            .iter()
            .filter(|receipt| receipt.disposition == TrainReliabilityDisposition::Failed)
            .count();
        let receipt_digest = stable_suite_receipt_digest(
            scenario_receipts.as_slice(),
            passed_count,
            recovered_count,
            failed_count,
        );
        Ok(TrainReliabilitySuiteReceipt {
            scenario_receipts,
            passed_count,
            recovered_count,
            failed_count,
            receipt_digest,
        })
    }
}

fn topology_churn_scenario(
    scenario_id: &str,
    injection_count: usize,
) -> Result<TrainReliabilityScenarioReceipt, TrainReliabilityError> {
    let stable = cluster_state(&[
        ("worker-a", ClusterMembershipStatus::Ready),
        ("worker-b", ClusterMembershipStatus::Ready),
    ]);
    let churned = cluster_state(&[
        ("worker-a", ClusterMembershipStatus::Ready),
        ("worker-b", ClusterMembershipStatus::Offline),
        ("worker-c", ClusterMembershipStatus::Joining),
    ]);
    let manifest = DatastreamManifest::from_bytes(
        "checkpoint-stream",
        DatastreamSubjectKind::Checkpoint,
        b"checkpoint-bytes",
        4,
        DatastreamEncoding::Safetensors,
    )
    .with_checkpoint_binding(
        DatastreamCheckpointBinding::new("train.decoder")
            .with_checkpoint_ref("step-32")
            .with_step(32),
    );
    let mut session = TrainingSessionState::new(stable.cluster_id().as_str(), "train.decoder");
    let stable_epoch = session.observe_membership(&stable)?;
    let churn_epoch = session.observe_membership(&churned)?;
    let write =
        session.begin_async_checkpoint(&stable, &manifest, &NodeId::new("worker-a"), 1_000)?;
    session.mark_checkpoint_durable(write.write_id.as_str(), 1_250)?;
    let recovery = session.plan_live_recovery(
        &churned,
        &[NodeId::new("worker-b")],
        &[NodeId::new("worker-c")],
        1_500,
    )?;
    let mut signals = vec![
        TrainReliabilitySignal::new(
            TrainReliabilitySignalKind::TopologyChangeObserved,
            "membership epoch advanced under topology churn",
            churn_epoch
                .context
                .membership_epoch
                .saturating_sub(stable_epoch.context.membership_epoch),
        ),
        TrainReliabilitySignal::new(
            TrainReliabilitySignalKind::RecoveryPlanIssued,
            "checkpoint-backed recovery plan fenced the recovering node and staged the late joiner",
            recovery.actions.len() as u64,
        ),
    ];
    let recovered = recovery.checkpoint_required
        && recovery
            .actions
            .contains(&TrainingRecoveryAction::ResumeFromDurableCheckpoint)
        && recovery
            .actions
            .contains(&TrainingRecoveryAction::FenceRecoveringNodes)
        && recovery
            .actions
            .contains(&TrainingRecoveryAction::StageCheckpointForLateJoiners)
        && recovery.recovery_context.posture == TrainingRecoveryPosture::ElasticReconfiguration;
    if !recovered {
        signals.push(TrainReliabilitySignal::new(
            TrainReliabilitySignalKind::RecoveryPlanIssued,
            "elastic reconfiguration failed to produce the expected recovery actions",
            0,
        ));
    }
    Ok(build_reliability_receipt(
        scenario_id,
        TrainReliabilityScenarioKind::TopologyChurn,
        injection_count,
        if recovered {
            TrainReliabilityDisposition::RecoveredAfterFault
        } else {
            TrainReliabilityDisposition::Failed
        },
        signals,
    ))
}

fn network_degradation_scenario(
    scenario_id: &str,
) -> Result<TrainReliabilityScenarioReceipt, TrainReliabilityError> {
    let mut planner = four_way_collective_planner()?;
    planner.observe_transport_feedback(
        CollectiveTransportFeedback::new(2_000, 400, 12, 2)
            .with_detail("wan bandwidth fell below the safe floor"),
    );
    let policy = CollectiveSyncCadencePolicy::new()
        .with_degraded_global_interval_steps(4)
        .with_transport_thresholds(800, 8, 8);
    let plan = planner.plan_sync(
        2,
        TrainingCollectiveKind::AllReduce,
        16 * 1024 * 1024,
        TrainingCollectiveQuantization::Int8Symmetric,
        &policy,
    )?;
    let recovered = plan.cadence_receipt.degraded_transport
        && plan.cadence_receipt.cadence_class
            == CollectiveSyncCadenceClass::LocalOnlyDeferredGlobal
        && plan
            .cadence_receipt
            .triggers
            .iter()
            .any(|trigger| trigger.kind == CollectiveReplanTriggerKind::BandwidthBelowFloor);
    let signals = vec![TrainReliabilitySignal::new(
        TrainReliabilitySignalKind::NetworkFallbackActivated,
        "collective cadence deferred the global sync and stayed on local groups under degraded transport",
        plan.stages.len() as u64,
    )];
    Ok(build_reliability_receipt(
        scenario_id,
        TrainReliabilityScenarioKind::NetworkDegradation,
        1,
        if recovered {
            TrainReliabilityDisposition::RecoveredAfterFault
        } else {
            TrainReliabilityDisposition::Failed
        },
        signals,
    ))
}

fn stale_weight_flood_scenario(
    scenario_id: &str,
    injection_count: usize,
) -> Result<TrainReliabilityScenarioReceipt, TrainReliabilityError> {
    let mut orchestrator = orchestrator_state(
        TrainingOffPolicyBudget::new(100)
            .with_revision_drift(1, Some(2))
            .with_rollout_quarantine_age_ms(Some(200)),
    )?;
    let window = orchestrator.plan_next_window(
        2,
        TrainingWindowAssignmentRule::RoundRobinByPriority {
            batch_slice_count: 2,
            eval_slice_count: 1,
        },
        13,
        1_100,
    )?;
    orchestrator.activate_current_window(1_110)?;
    let mut assigned_workers = window
        .rollout_assignments
        .iter()
        .map(|assignment| assignment.contributor_node_id.clone())
        .collect::<Vec<_>>();
    assigned_workers.sort();
    assigned_workers.dedup();
    let mut discarded = 0_u64;
    let mut quarantined = 0_u64;
    let mut accepted = 0_u64;
    for index in 0..injection_count {
        let worker_id = assigned_workers[index % assigned_workers.len()].clone();
        let receipt = orchestrator.submit_rollout(
            rollout_artifact(
                worker_id.as_str(),
                format!("artifact-stale-{index}").as_str(),
                PolicyRevision::new("train.decoder", "policy-rev-1", "policy-digest-1", 900)
                    .with_revision_number(1),
                1_000,
            )?,
            70_000 + index as u64,
        )?;
        match receipt.outcome {
            RolloutReceiptOutcome::AcceptedExact | RolloutReceiptOutcome::AcceptedOffPolicy => {
                accepted = accepted.saturating_add(1);
            }
            RolloutReceiptOutcome::Quarantined => {
                quarantined = quarantined.saturating_add(1);
            }
            RolloutReceiptOutcome::Discarded => {
                discarded = discarded.saturating_add(1);
            }
        }
    }
    let contained = accepted == 0
        && (discarded.saturating_add(quarantined) == injection_count as u64)
        && orchestrator
            .orchestrator_windows
            .iter()
            .find(|candidate| candidate.window_id == "run-1-window-1")
            .is_some_and(|window| window.accepted_rollouts.is_empty());
    let signals = vec![TrainReliabilitySignal::new(
        TrainReliabilitySignalKind::StaleFloodContained,
        "stale-policy flood was discarded or quarantined by rollout admission",
        discarded.saturating_add(quarantined),
    )];
    Ok(build_reliability_receipt(
        scenario_id,
        TrainReliabilityScenarioKind::StaleWeightFlood,
        injection_count,
        if contained {
            TrainReliabilityDisposition::Passed
        } else {
            TrainReliabilityDisposition::Failed
        },
        signals,
    ))
}

fn checkpoint_corruption_scenario(
    scenario_id: &str,
) -> Result<TrainReliabilityScenarioReceipt, TrainReliabilityError> {
    let (session, _) = durable_session()?;
    let scope = CheckpointScopeBinding::new(CheckpointScopeKind::Run, "train-run-corrupt");
    let manifest = session.checkpoint_manifest_for_latest_durable(
        scope.clone(),
        &NodeId::new("worker-a"),
        1_250,
    )?;
    let stale_pointer = crate::CheckpointPointer::new(
        scope.clone(),
        "train.decoder",
        manifest.checkpoint.clone(),
        "missing-manifest-digest",
        1_260,
    )?;
    let mut store = InMemoryCheckpointStore::default();
    store.store_manifest(manifest.clone());
    store.store_pointer(stale_pointer);
    let restore = store.plan_restore(
        &scope,
        "train.decoder",
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        &[NodeId::new("worker-a")],
        CheckpointStoreReadOptions::default(),
    )?;
    let recovered = restore.source_kind == CheckpointReadSourceKind::ManifestListingFallback
        && restore.selected_manifest.durability == CheckpointDurabilityPosture::Durable;
    let signals = vec![TrainReliabilitySignal::new(
        TrainReliabilitySignalKind::CheckpointFallbackSelected,
        "restore path fell back away from a stale pointer and selected the durable manifest listing",
        restore.attempts.len() as u64,
    )];
    Ok(build_reliability_receipt(
        scenario_id,
        TrainReliabilityScenarioKind::CheckpointCorruption,
        1,
        if recovered {
            TrainReliabilityDisposition::RecoveredAfterFault
        } else {
            TrainReliabilityDisposition::Failed
        },
        signals,
    ))
}

fn validator_sampling_stress_scenario(
    scenario_id: &str,
    injection_count: usize,
) -> Result<TrainReliabilityScenarioReceipt, TrainReliabilityError> {
    let mut validator = RolloutValidatorState::new(RolloutValidatorPolicy::default());
    let accepted_artifact = rollout_artifact_with_task(
        "worker-a",
        "validator-accepted",
        "validator-task-shared",
        PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100)
            .with_revision_number(7),
        1_120,
    )?;
    let duplicate_artifact = rollout_artifact_with_task(
        "worker-b",
        "validator-duplicate",
        "validator-task-shared",
        PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100)
            .with_revision_number(7),
        1_121,
    )?;
    let stale_artifact = rollout_artifact_with_task(
        "worker-c",
        "validator-stale",
        "validator-task-stale",
        PolicyRevision::new("train.decoder", "policy-rev-1", "policy-digest-1", 900)
            .with_revision_number(1),
        1_122,
    )?;
    let mut accepted = 0_u64;
    let mut normalized = 0_u64;
    let mut rejected = 0_u64;
    for index in 0..injection_count {
        let (bundle_artifact, outcome_kind, policy_posture, admission_outcome) = match index % 3 {
            0 => (
                accepted_artifact.clone(),
                RolloutWorkerOutcomeKind::UploadedAcceptedExact,
                RolloutWorkerPolicyPosture::ExactTarget,
                RolloutReceiptOutcome::AcceptedExact,
            ),
            1 => (
                stale_artifact.clone(),
                RolloutWorkerOutcomeKind::UploadedDiscarded,
                RolloutWorkerPolicyPosture::DiscardedOffPolicy,
                RolloutReceiptOutcome::Discarded,
            ),
            _ => (
                duplicate_artifact.clone(),
                RolloutWorkerOutcomeKind::UploadedAcceptedExact,
                RolloutWorkerPolicyPosture::ExactTarget,
                RolloutReceiptOutcome::AcceptedExact,
            ),
        };
        let verdict = validator.verify_bundle(RolloutVerificationBundle::new(
            format!("bundle-{index}"),
            bundle_artifact.clone(),
            worker_outcome(
                &bundle_artifact,
                outcome_kind,
                policy_posture,
                admission_outcome,
            ),
            None,
            None,
        ));
        match verdict.disposition {
            ValidatorDisposition::Accepted => {
                accepted = accepted.saturating_add(1);
            }
            ValidatorDisposition::Normalized => {
                normalized = normalized.saturating_add(1);
            }
            ValidatorDisposition::Rejected => {
                rejected = rejected.saturating_add(1);
            }
        }
    }
    let healthy_mix = accepted > 0 && normalized > 0 && rejected > 0;
    let signals = vec![TrainReliabilitySignal::new(
        TrainReliabilitySignalKind::ValidatorStressMixedOutcomes,
        "validator stress run produced accepted, normalized, and rejected verdict classes",
        accepted + normalized + rejected,
    )];
    Ok(build_reliability_receipt(
        scenario_id,
        TrainReliabilityScenarioKind::ValidatorSamplingStress,
        injection_count,
        if healthy_mix {
            TrainReliabilityDisposition::Passed
        } else {
            TrainReliabilityDisposition::Failed
        },
        signals,
    ))
}

fn orchestrator_restart_scenario(
    scenario_id: &str,
) -> Result<TrainReliabilityScenarioReceipt, TrainReliabilityError> {
    let mut orchestrator = orchestrator_state(TrainingOffPolicyBudget::default())?;
    orchestrator.plan_next_window(
        2,
        TrainingWindowAssignmentRule::RoundRobinByPriority {
            batch_slice_count: 2,
            eval_slice_count: 1,
        },
        41,
        1_100,
    )?;
    orchestrator.activate_current_window(1_110)?;
    let target_policy = orchestrator.target_policy_revision.clone();
    let assigned_worker = orchestrator
        .orchestrator_windows
        .iter()
        .find(|window| window.window_id == "run-1-window-1")
        .and_then(|window| window.rollout_assignments.first())
        .map(|assignment| assignment.contributor_node_id.clone())
        .expect("active window should expose one assignment");
    let receipt = orchestrator.submit_rollout(
        rollout_artifact(
            assigned_worker.as_str(),
            "restart-artifact",
            target_policy,
            1_118,
        )?,
        1_121,
    )?;
    let serialized = serde_json::to_string(&orchestrator)?;
    let mut restored: TrainingOrchestratorState = serde_json::from_str(&serialized)?;
    restored.seal_current_window(1_130)?;
    let batch = restored.assemble_trainer_batch(
        "restart-batch",
        vec![String::from("restart-artifact")],
        1_140,
    )?;
    restored.score_current_window(1_150)?;
    restored.reconcile_current_window(1_160)?;
    let recovered = receipt.outcome == RolloutReceiptOutcome::AcceptedExact
        && batch.batch.rollout_count == 1
        && restored.current_window_id.is_none();
    let signals = vec![TrainReliabilitySignal::new(
        TrainReliabilitySignalKind::RestartStateRecovered,
        "orchestrator state round-tripped through restart and resumed batch assembly",
        batch.batch.rollout_count as u64,
    )];
    Ok(build_reliability_receipt(
        scenario_id,
        TrainReliabilityScenarioKind::OrchestratorRestartRecovery,
        1,
        if recovered {
            TrainReliabilityDisposition::RecoveredAfterFault
        } else {
            TrainReliabilityDisposition::Failed
        },
        signals,
    ))
}

fn build_reliability_receipt(
    scenario_id: &str,
    kind: TrainReliabilityScenarioKind,
    injection_count: usize,
    disposition: TrainReliabilityDisposition,
    signals: Vec<TrainReliabilitySignal>,
) -> TrainReliabilityScenarioReceipt {
    let receipt_digest = stable_scenario_receipt_digest(
        scenario_id,
        kind,
        injection_count,
        disposition,
        signals.as_slice(),
    );
    TrainReliabilityScenarioReceipt {
        scenario_id: String::from(scenario_id),
        kind,
        injection_count,
        disposition,
        signals,
        receipt_digest,
    }
}

fn stable_scenario_receipt_digest(
    scenario_id: &str,
    kind: TrainReliabilityScenarioKind,
    injection_count: usize,
    disposition: TrainReliabilityDisposition,
    signals: &[TrainReliabilitySignal],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_reliability_scenario|");
    hasher.update(scenario_id.as_bytes());
    hasher.update(b"|");
    hasher.update(reliability_kind_label(kind));
    hasher.update(b"|");
    hasher.update(injection_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(reliability_disposition_label(disposition));
    for signal in signals {
        hasher.update(stable_json_bytes(signal));
    }
    hex::encode(hasher.finalize())
}

fn stable_suite_receipt_digest(
    scenario_receipts: &[TrainReliabilityScenarioReceipt],
    passed_count: usize,
    recovered_count: usize,
    failed_count: usize,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_reliability_suite|");
    for receipt in scenario_receipts {
        hasher.update(stable_json_bytes(receipt));
    }
    hasher.update(passed_count.to_string().as_bytes());
    hasher.update(recovered_count.to_string().as_bytes());
    hasher.update(failed_count.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn reliability_kind_label(kind: TrainReliabilityScenarioKind) -> &'static [u8] {
    match kind {
        TrainReliabilityScenarioKind::TopologyChurn => b"topology_churn",
        TrainReliabilityScenarioKind::NetworkDegradation => b"network_degradation",
        TrainReliabilityScenarioKind::StaleWeightFlood => b"stale_weight_flood",
        TrainReliabilityScenarioKind::CheckpointCorruption => b"checkpoint_corruption",
        TrainReliabilityScenarioKind::ValidatorSamplingStress => b"validator_sampling_stress",
        TrainReliabilityScenarioKind::OrchestratorRestartRecovery => {
            b"orchestrator_restart_recovery"
        }
    }
}

fn reliability_disposition_label(disposition: TrainReliabilityDisposition) -> &'static [u8] {
    match disposition {
        TrainReliabilityDisposition::Passed => b"passed",
        TrainReliabilityDisposition::RecoveredAfterFault => b"recovered_after_fault",
        TrainReliabilityDisposition::Failed => b"failed",
    }
}

fn stable_json_bytes(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("stable JSON serialization failed")
}

fn cluster_id() -> ClusterId {
    ClusterId::new(
        &ClusterNamespace::new("train-reliability"),
        &AdmissionToken::new("admission-token"),
    )
}

fn membership_record(
    cluster_id: &ClusterId,
    node_id: &str,
    status: ClusterMembershipStatus,
) -> ClusterMembershipRecord {
    ClusterMembershipRecord::new(
        ClusterNodeIdentity {
            cluster_id: cluster_id.clone(),
            node_id: NodeId::new(node_id),
            node_epoch: NodeEpoch::initial(),
            role: NodeRole::Mixed,
            auth_public_key: format!("{node_id}-pub"),
            attestation: None,
        },
        Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9_000)),
        status,
    )
}

fn cluster_state(records: &[(&str, ClusterMembershipStatus)]) -> ClusterState {
    let cluster_id = cluster_id();
    let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
    snapshot.memberships = records
        .iter()
        .map(|(node_id, status)| {
            (
                NodeId::new(*node_id),
                membership_record(&cluster_id, node_id, *status),
            )
        })
        .collect::<BTreeMap<_, _>>();
    ClusterState::from_snapshot(snapshot)
}

fn membership_context(
    mesh_revision: u64,
    ready_nodes: Vec<&str>,
    joining_nodes: Vec<&str>,
) -> psionic_runtime::TrainingElasticMembershipContext {
    let active_node_ids = ready_nodes
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let joining_node_ids = joining_nodes
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    psionic_runtime::TrainingElasticMembershipContext::new(
        mesh_revision,
        format!("cluster-state-{mesh_revision}"),
        format!("topology-{mesh_revision}"),
        active_node_ids,
    )
    .with_joining_node_ids(joining_node_ids)
}

fn mesh_members(node_ids: &[&str]) -> Vec<CollectiveMeshMember> {
    node_ids
        .iter()
        .enumerate()
        .map(|(index, node_id)| {
            CollectiveMeshMember::new(*node_id, index, index, format!("cuda:{index}"))
        })
        .collect()
}

fn four_way_collective_planner() -> Result<ElasticCollectivePlanner, TrainReliabilityError> {
    let mut planner = ElasticCollectivePlanner::new(
        "mesh-train",
        "cuda",
        ClusterCommunicationClass::TensorCollectiveMesh,
        vec![
            TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 2)
                .with_collective_group_size(2),
            TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2)
                .with_collective_group_size(2),
        ],
    );
    planner.observe_mesh(
        membership_context(
            1,
            vec!["worker-a", "worker-b", "worker-c", "worker-d"],
            Vec::new(),
        ),
        mesh_members(&["worker-a", "worker-b", "worker-c", "worker-d"]),
    )?;
    planner.record_benchmark(QuantizedCollectiveBenchmark::new(
        TrainingCollectiveKind::AllReduce,
        TrainingCollectiveQuantization::Int8Symmetric,
        QuantizedCollectiveBenchmarkSample::new(2_400, 32 * 1024 * 1024, 0),
        QuantizedCollectiveBenchmarkSample::new(1_200, 8 * 1024 * 1024, 55),
        100,
        1_000,
    ));
    Ok(planner)
}

fn durable_session() -> Result<
    (
        TrainingSessionState,
        psionic_datastream::DatastreamManifestRef,
    ),
    TrainReliabilityError,
> {
    let state = cluster_state(&[
        ("worker-a", ClusterMembershipStatus::Ready),
        ("worker-b", ClusterMembershipStatus::Ready),
    ]);
    let payload = vec![7_u8; 32];
    let manifest = DatastreamManifest::from_bytes(
        "checkpoint-stream",
        DatastreamSubjectKind::Checkpoint,
        payload.as_slice(),
        8,
        DatastreamEncoding::Safetensors,
    )
    .with_checkpoint_binding(
        DatastreamCheckpointBinding::new("train.decoder")
            .with_checkpoint_ref("step-12")
            .with_step(12),
    );
    let mut session = TrainingSessionState::new(state.cluster_id().as_str(), "train.decoder");
    let write =
        session.begin_async_checkpoint(&state, &manifest, &NodeId::new("worker-a"), 1_000)?;
    session.mark_checkpoint_durable(write.write_id.as_str(), 1_200)?;
    Ok((session, manifest.manifest_ref()))
}

fn policy_weight_broadcast()
-> Result<psionic_datastream::DatastreamPolicyWeightBroadcastManifest, TrainReliabilityError> {
    let shard_a = b"weight-shard-a".to_vec();
    let shard_b = b"weight-shard-b".to_vec();
    let assembled = {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&shard_a);
        bytes.extend_from_slice(&shard_b);
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    };
    let manifest_a = DatastreamManifest::from_bytes(
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
    let manifest_b = DatastreamManifest::from_bytes(
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
    Ok(InMemoryPolicyWeightBroadcast::new(
        vec![
            InMemoryDatastreamServer::new(manifest_a, shard_a)?,
            InMemoryDatastreamServer::new(manifest_b, shard_b)?,
        ],
        1_500,
    )?
    .broadcast()
    .clone())
}

fn orchestrator_state(
    off_policy_budget: TrainingOffPolicyBudget,
) -> Result<TrainingOrchestratorState, TrainReliabilityError> {
    let state = cluster_state(&[
        ("worker-b", ClusterMembershipStatus::Ready),
        ("trainer-a", ClusterMembershipStatus::Ready),
        ("worker-c", ClusterMembershipStatus::Ready),
    ]);
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
        PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100)
            .with_revision_number(7)
            .with_parent_revision_id("policy-rev-6");
    Ok(TrainingOrchestratorState::new_with_budget(
        run,
        policy_revision,
        policy_weight_broadcast()?,
        off_policy_budget,
    )?)
}

fn rollout_artifact(
    worker_id: &str,
    artifact_id: &str,
    source_policy_revision: PolicyRevision,
    created_at_ms: u64,
) -> Result<RolloutArtifact, TrainReliabilityError> {
    rollout_artifact_with_task(
        worker_id,
        artifact_id,
        format!("task-{artifact_id}").as_str(),
        source_policy_revision,
        created_at_ms,
    )
}

fn rollout_artifact_with_task(
    worker_id: &str,
    artifact_id: &str,
    task_id: &str,
    source_policy_revision: PolicyRevision,
    created_at_ms: u64,
) -> Result<RolloutArtifact, TrainReliabilityError> {
    Ok(RolloutArtifact::new(
        artifact_id,
        worker_id,
        EnvironmentPackageKey::new("oa.train", "2026.03"),
        task_id,
        source_policy_revision,
        vec![
            RolloutSample::new(1, -0.2, 1.0, 0.8),
            RolloutSample::new(2, -0.1, 0.6, 0.4),
        ],
        RolloutTerminationReason::Completed,
        vec![RolloutProofReference::new(
            RolloutProofKind::ExecutionProof,
            format!("proof-{artifact_id}"),
            format!("exec://{artifact_id}"),
        )],
        created_at_ms,
    )?)
}

fn admission_receipt(
    artifact: &RolloutArtifact,
    outcome: RolloutReceiptOutcome,
) -> RolloutAdmissionReceipt {
    RolloutAdmissionReceipt {
        receipt_id: format!("receipt-{}", artifact.artifact_id),
        run_id: String::from("run-1"),
        stage_id: String::from("stage-rl"),
        window_id: String::from("run-1-window-1"),
        artifact_id: artifact.artifact_id.clone(),
        artifact_digest: artifact.artifact_digest.clone(),
        worker_id: artifact.worker_id.clone(),
        environment_key: artifact.environment.storage_key(),
        target_policy_revision_id: String::from("policy-rev-7"),
        source_policy_revision_id: artifact.source_policy_revision.revision_id.clone(),
        source_policy_digest: artifact.source_policy_revision.policy_digest.clone(),
        outcome,
        revision_drift: None,
        policy_age_ms: None,
        rollout_age_ms: 5,
        signals: Vec::new(),
        token_count: artifact.token_count(),
        reward_sum: artifact.reward_sum(),
        termination_reason: artifact.termination_reason,
        observed_at_ms: 1_125,
        receipt_digest: format!("receipt-digest-{}", artifact.artifact_id),
    }
}

fn worker_outcome(
    artifact: &RolloutArtifact,
    outcome: RolloutWorkerOutcomeKind,
    policy_posture: RolloutWorkerPolicyPosture,
    admission_outcome: RolloutReceiptOutcome,
) -> RolloutWorkerOutcomeReceipt {
    RolloutWorkerOutcomeReceipt {
        receipt_id: format!("worker-outcome-{}", artifact.artifact_id),
        claim_id: format!("claim-{}", artifact.artifact_id),
        assignment_id: format!("assignment-{}", artifact.worker_id),
        window_id: String::from("run-1-window-1"),
        worker_id: artifact.worker_id.clone(),
        trust_class: RolloutWorkerTrustClass::UntrustedWorker,
        target_policy_revision_id: String::from("policy-rev-7"),
        source_policy_revision_id: artifact.source_policy_revision.revision_id.clone(),
        outcome,
        policy_posture,
        upload: RolloutUploadLocator::new(
            RolloutUploadTransport::InlineArtifact,
            format!("inline://{}", artifact.artifact_id),
            512,
            artifact.artifact_digest.as_str(),
        ),
        rejection_reason: None,
        admission_receipt: Some(admission_receipt(artifact, admission_outcome)),
        observed_at_ms: 1_125,
        receipt_digest: format!("worker-outcome-digest-{}", artifact.artifact_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_reliability_suite_covers_all_fault_classes()
    -> Result<(), Box<dyn std::error::Error>> {
        let suite = TrainReliabilityHarness::run_reference_suite()?;
        assert_eq!(suite.scenario_receipts.len(), 6);
        assert_eq!(suite.failed_count, 0);
        assert!(suite.recovered_count >= 4);
        assert!(
            suite
                .scenario_receipts
                .iter()
                .any(|receipt| receipt.kind == TrainReliabilityScenarioKind::TopologyChurn)
        );
        assert!(
            suite
                .scenario_receipts
                .iter()
                .any(|receipt| receipt.kind == TrainReliabilityScenarioKind::NetworkDegradation)
        );
        assert!(
            suite
                .scenario_receipts
                .iter()
                .any(|receipt| receipt.kind == TrainReliabilityScenarioKind::StaleWeightFlood)
        );
        assert!(
            suite
                .scenario_receipts
                .iter()
                .any(|receipt| receipt.kind == TrainReliabilityScenarioKind::CheckpointCorruption)
        );
        assert!(
            suite.scenario_receipts.iter().any(
                |receipt| receipt.kind == TrainReliabilityScenarioKind::ValidatorSamplingStress
            )
        );
        assert!(suite.scenario_receipts.iter().any(
            |receipt| receipt.kind == TrainReliabilityScenarioKind::OrchestratorRestartRecovery
        ));
        Ok(())
    }

    #[test]
    fn checkpoint_and_restart_faults_recover_cleanly() -> Result<(), Box<dyn std::error::Error>> {
        let checkpoint =
            TrainReliabilityHarness::run_scenario(&TrainReliabilityScenarioSpec::new(
                "checkpoint-corruption",
                TrainReliabilityScenarioKind::CheckpointCorruption,
                1,
            ))?;
        assert_eq!(
            checkpoint.disposition,
            TrainReliabilityDisposition::RecoveredAfterFault
        );
        assert!(checkpoint.signals.iter().any(|signal| {
            signal.kind == TrainReliabilitySignalKind::CheckpointFallbackSelected
        }));

        let restart = TrainReliabilityHarness::run_scenario(&TrainReliabilityScenarioSpec::new(
            "orchestrator-restart",
            TrainReliabilityScenarioKind::OrchestratorRestartRecovery,
            1,
        ))?;
        assert_eq!(
            restart.disposition,
            TrainReliabilityDisposition::RecoveredAfterFault
        );
        assert!(
            restart
                .signals
                .iter()
                .any(|signal| { signal.kind == TrainReliabilitySignalKind::RestartStateRecovered })
        );
        Ok(())
    }

    #[test]
    fn stale_flood_and_validator_stress_are_contained() -> Result<(), Box<dyn std::error::Error>> {
        let stale = TrainReliabilityHarness::run_scenario(&TrainReliabilityScenarioSpec::new(
            "stale-weight-flood",
            TrainReliabilityScenarioKind::StaleWeightFlood,
            6,
        ))?;
        assert_eq!(stale.disposition, TrainReliabilityDisposition::Passed);
        assert!(
            stale
                .signals
                .iter()
                .any(|signal| { signal.kind == TrainReliabilitySignalKind::StaleFloodContained })
        );

        let validator = TrainReliabilityHarness::run_scenario(&TrainReliabilityScenarioSpec::new(
            "validator-sampling-stress",
            TrainReliabilityScenarioKind::ValidatorSamplingStress,
            8,
        ))?;
        assert_eq!(validator.disposition, TrainReliabilityDisposition::Passed);
        assert!(validator.signals.iter().any(|signal| {
            signal.kind == TrainReliabilitySignalKind::ValidatorStressMixedOutcomes
        }));
        Ok(())
    }
}
