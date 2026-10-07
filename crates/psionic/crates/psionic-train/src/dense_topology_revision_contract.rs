use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_program_run_graph, canonical_dense_rank_recovery_contract,
    canonical_sharded_distributed_checkpoint_contract, cross_provider_training_program_manifest,
    CrossProviderProgramRunGraphError, CrossProviderTrainingProgramManifestError,
    DenseRankRecoveryContractError, DistributedCheckpointContractError,
};

/// Stable schema version for the dense topology-revision contract.
pub const DENSE_TOPOLOGY_REVISION_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.dense_topology_revision_contract.v1";
/// Stable fixture path for the dense topology-revision contract.
pub const DENSE_TOPOLOGY_REVISION_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/dense_topology_revision_contract_v1.json";
/// Stable checker path for the dense topology-revision contract.
pub const DENSE_TOPOLOGY_REVISION_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-dense-topology-revision-contract.sh";
/// Stable reference doc path for the dense topology-revision contract.
pub const DENSE_TOPOLOGY_REVISION_CONTRACT_DOC_PATH: &str =
    "docs/DENSE_TOPOLOGY_REVISION_REFERENCE.md";

/// Error surfaced while building, validating, or writing the dense topology-revision contract.
#[derive(Debug, Error)]
pub enum DenseTopologyRevisionContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    WholeProgramRunGraph(#[from] CrossProviderProgramRunGraphError),
    #[error(transparent)]
    DenseRecovery(#[from] DenseRankRecoveryContractError),
    #[error(transparent)]
    DistributedCheckpoint(#[from] DistributedCheckpointContractError),
    #[error("dense topology-revision contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// One controlled topology-revision action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseTopologyRevisionActionKind {
    ReplaceRank,
    GrowWorld,
    ShrinkWorld,
    RemoveWithoutReplacement,
}

/// Current support posture for one topology revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseTopologyRevisionDisposition {
    Supported,
    Refused,
}

/// Data-ordering policy used by one topology revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseTopologyDataOrderingPolicyKind {
    ReplayContinuation,
    CheckpointBarrierReseed,
    Refused,
}

/// Checkpoint transition policy used by one topology revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseTopologyCheckpointTransitionKind {
    ReuseLatestStableCheckpoint,
    BarrierCheckpointReshardAndRestore,
    RefusedNoCheckpointTransition,
}

/// Operator action required for one topology revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseTopologyRevisionOperatorAction {
    AdmitReplacementRank,
    PauseAtCheckpointBarrierAndReshard,
    RefuseLiveRevisionAndPageOperator,
}

/// Finalizer action required for one topology revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseTopologyRevisionFinalizerAction {
    PublishRevisionReceipt,
    PublishBarrierRevisionReceipt,
    PublishRefusalReceipt,
}

/// Data-ordering contract for one controlled topology revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseTopologyRevisionDataOrdering {
    pub policy_kind: DenseTopologyDataOrderingPolicyKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_case_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_global_order_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_global_order_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay_continuity_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reseed_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    pub detail: String,
}

/// Checkpoint transition contract for one controlled topology revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseTopologyRevisionCheckpointTransition {
    pub transition_kind: DenseTopologyCheckpointTransitionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_manifest_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_scenario_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restore_assignment_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub barrier_reshard_plan_id: Option<String>,
    pub detail: String,
}

/// One typed topology-revision scenario over the dense cluster.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseTopologyRevisionScenario {
    pub scenario_id: String,
    pub topology_revision_id: String,
    pub action_kind: DenseTopologyRevisionActionKind,
    pub previous_world_size: u16,
    pub next_world_size: u16,
    pub affected_rank_ids: Vec<u16>,
    pub data_ordering: DenseTopologyRevisionDataOrdering,
    pub checkpoint_transition: DenseTopologyRevisionCheckpointTransition,
    pub operator_action: DenseTopologyRevisionOperatorAction,
    pub finalizer_action: DenseTopologyRevisionFinalizerAction,
    pub disposition: DenseTopologyRevisionDisposition,
    pub detail: String,
}

/// Canonical provider-neutral dense topology-revision contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseTopologyRevisionContract {
    pub schema_version: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub whole_program_run_graph_digest: String,
    pub dense_recovery_contract_digest: String,
    pub distributed_checkpoint_contract_digest: String,
    pub revisions: Vec<DenseTopologyRevisionScenario>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl DenseTopologyRevisionContract {
    /// Returns the stable digest over the contract payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_dense_topology_revision_contract|", &clone)
    }

    /// Validates the topology-revision contract against the canonical recovery and checkpoint surfaces.
    pub fn validate(&self) -> Result<(), DenseTopologyRevisionContractError> {
        let manifest = cross_provider_training_program_manifest()?;
        let run_graph = canonical_cross_provider_program_run_graph()?;
        let dense_recovery = canonical_dense_rank_recovery_contract()?;
        let checkpoint_contract = canonical_sharded_distributed_checkpoint_contract()?;

        if self.schema_version != DENSE_TOPOLOGY_REVISION_CONTRACT_SCHEMA_VERSION {
            return Err(DenseTopologyRevisionContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    DENSE_TOPOLOGY_REVISION_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(DenseTopologyRevisionContractError::InvalidContract {
                detail: String::from("program manifest binding drifted"),
            });
        }
        if self.whole_program_run_graph_digest != run_graph.contract_digest {
            return Err(DenseTopologyRevisionContractError::InvalidContract {
                detail: String::from("whole-program run-graph digest drifted"),
            });
        }
        if self.dense_recovery_contract_digest != dense_recovery.contract_digest {
            return Err(DenseTopologyRevisionContractError::InvalidContract {
                detail: String::from("dense recovery contract digest drifted"),
            });
        }
        if self.distributed_checkpoint_contract_digest != checkpoint_contract.contract_digest {
            return Err(DenseTopologyRevisionContractError::InvalidContract {
                detail: String::from("distributed checkpoint contract digest drifted"),
            });
        }
        if self.revisions.len() != 4 {
            return Err(DenseTopologyRevisionContractError::InvalidContract {
                detail: String::from("expected exactly four canonical topology revisions"),
            });
        }

        let recovery_scenarios = dense_recovery
            .scenarios
            .iter()
            .map(|scenario| (scenario.scenario_id.as_str(), scenario))
            .collect::<BTreeMap<_, _>>();
        let checkpoint_manifest_digest = checkpoint_contract
            .checkpoint_manifest
            .manifest_digest
            .as_str();

        for revision in &self.revisions {
            match revision.action_kind {
                DenseTopologyRevisionActionKind::ReplaceRank => {
                    if revision.previous_world_size != revision.next_world_size
                        || revision.disposition != DenseTopologyRevisionDisposition::Supported
                        || revision.data_ordering.policy_kind
                            != DenseTopologyDataOrderingPolicyKind::ReplayContinuation
                        || revision.checkpoint_transition.transition_kind
                            != DenseTopologyCheckpointTransitionKind::ReuseLatestStableCheckpoint
                        || revision
                            .checkpoint_transition
                            .barrier_reshard_plan_id
                            .is_some()
                    {
                        return Err(DenseTopologyRevisionContractError::InvalidContract {
                            detail: format!(
                                "replace-rank revision `{}` lost hot replace-rank policy",
                                revision.scenario_id
                            ),
                        });
                    }
                    let recovery_id = revision
                        .checkpoint_transition
                        .recovery_scenario_id
                        .as_deref()
                        .ok_or_else(|| DenseTopologyRevisionContractError::InvalidContract {
                            detail: format!(
                                "replace-rank revision `{}` must cite a recovery scenario",
                                revision.scenario_id
                            ),
                        })?;
                    let recovery = recovery_scenarios.get(recovery_id).ok_or_else(|| {
                        DenseTopologyRevisionContractError::InvalidContract {
                            detail: format!(
                                "replace-rank revision `{}` cites unknown recovery scenario `{}`",
                                revision.scenario_id, recovery_id
                            ),
                        }
                    })?;
                    if recovery.disposition != crate::DenseRankRecoveryDisposition::Recovered
                        || recovery.checkpoint_manifest_digest.as_deref()
                            != Some(checkpoint_manifest_digest)
                        || revision
                            .checkpoint_transition
                            .checkpoint_manifest_digest
                            .as_deref()
                            != Some(checkpoint_manifest_digest)
                        || revision.checkpoint_transition.restore_assignment_ids
                            != recovery
                                .checkpoint_restore_assignment_id
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>()
                        || revision.data_ordering.topology_case_id
                            != recovery.data_ordering.topology_case_id
                        || revision.data_ordering.baseline_global_order_digest
                            != recovery.data_ordering.baseline_global_order_digest
                        || revision.data_ordering.revised_global_order_digest
                            != recovery.data_ordering.revised_global_order_digest
                        || revision.data_ordering.replay_continuity_digest
                            != recovery.data_ordering.replay_continuity_digest
                    {
                        return Err(DenseTopologyRevisionContractError::InvalidContract {
                            detail: format!(
                                "replace-rank revision `{}` drifted from its recovery receipt",
                                revision.scenario_id
                            ),
                        });
                    }
                }
                DenseTopologyRevisionActionKind::GrowWorld
                | DenseTopologyRevisionActionKind::ShrinkWorld => {
                    if revision.disposition != DenseTopologyRevisionDisposition::Supported
                        || revision.data_ordering.policy_kind
                            != DenseTopologyDataOrderingPolicyKind::CheckpointBarrierReseed
                        || revision.checkpoint_transition.transition_kind
                            != DenseTopologyCheckpointTransitionKind::BarrierCheckpointReshardAndRestore
                        || revision.data_ordering.topology_case_id.is_some()
                        || revision.data_ordering.baseline_global_order_digest.is_some()
                        || revision.data_ordering.revised_global_order_digest.is_some()
                        || revision.data_ordering.replay_continuity_digest.is_some()
                    {
                        return Err(DenseTopologyRevisionContractError::InvalidContract {
                            detail: format!(
                                "{} revision `{}` lost checkpoint-barrier policy",
                                action_label(revision.action_kind),
                                revision.scenario_id
                            ),
                        });
                    }
                    if revision
                        .checkpoint_transition
                        .checkpoint_manifest_digest
                        .as_deref()
                        != Some(checkpoint_manifest_digest)
                        || revision
                            .checkpoint_transition
                            .recovery_scenario_id
                            .is_some()
                        || !revision
                            .checkpoint_transition
                            .restore_assignment_ids
                            .is_empty()
                        || revision
                            .checkpoint_transition
                            .barrier_reshard_plan_id
                            .is_none()
                        || revision.data_ordering.reseed_digest.is_none()
                    {
                        return Err(DenseTopologyRevisionContractError::InvalidContract {
                            detail: format!(
                                "{} revision `{}` lost its barrier checkpoint or reseed binding",
                                action_label(revision.action_kind),
                                revision.scenario_id
                            ),
                        });
                    }
                    match revision.action_kind {
                        DenseTopologyRevisionActionKind::GrowWorld => {
                            if revision.next_world_size <= revision.previous_world_size {
                                return Err(DenseTopologyRevisionContractError::InvalidContract {
                                    detail: String::from(
                                        "grow-world revision must increase world size",
                                    ),
                                });
                            }
                        }
                        DenseTopologyRevisionActionKind::ShrinkWorld => {
                            if revision.next_world_size >= revision.previous_world_size {
                                return Err(DenseTopologyRevisionContractError::InvalidContract {
                                    detail: String::from(
                                        "shrink-world revision must decrease world size",
                                    ),
                                });
                            }
                        }
                        DenseTopologyRevisionActionKind::ReplaceRank
                        | DenseTopologyRevisionActionKind::RemoveWithoutReplacement => {}
                    }
                }
                DenseTopologyRevisionActionKind::RemoveWithoutReplacement => {
                    if revision.disposition != DenseTopologyRevisionDisposition::Refused
                        || revision.data_ordering.policy_kind
                            != DenseTopologyDataOrderingPolicyKind::Refused
                        || revision.checkpoint_transition.transition_kind
                            != DenseTopologyCheckpointTransitionKind::RefusedNoCheckpointTransition
                        || revision
                            .checkpoint_transition
                            .checkpoint_manifest_digest
                            .is_some()
                        || revision
                            .checkpoint_transition
                            .recovery_scenario_id
                            .is_some()
                        || !revision
                            .checkpoint_transition
                            .restore_assignment_ids
                            .is_empty()
                        || revision
                            .checkpoint_transition
                            .barrier_reshard_plan_id
                            .is_some()
                        || revision.data_ordering.reseed_digest.is_some()
                    {
                        return Err(DenseTopologyRevisionContractError::InvalidContract {
                            detail: format!(
                                "remove-without-replacement revision `{}` lost deterministic refusal posture",
                                revision.scenario_id
                            ),
                        });
                    }
                }
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(DenseTopologyRevisionContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical provider-neutral dense topology-revision contract.
pub fn canonical_dense_topology_revision_contract(
) -> Result<DenseTopologyRevisionContract, DenseTopologyRevisionContractError> {
    let manifest = cross_provider_training_program_manifest()?;
    let run_graph = canonical_cross_provider_program_run_graph()?;
    let dense_recovery = canonical_dense_rank_recovery_contract()?;
    let checkpoint_contract = canonical_sharded_distributed_checkpoint_contract()?;
    let checkpoint_manifest_digest = checkpoint_contract
        .checkpoint_manifest
        .manifest_digest
        .clone();

    let provider_loss_recovery = dense_recovery
        .scenarios
        .iter()
        .find(|scenario| {
            scenario.scenario_id == "dense_rank.provider_loss.rank2.cross_provider_replace"
        })
        .expect("canonical dense recovery must keep provider-loss replacement");
    let grow_reseed_digest = barrier_reseed_digest(
        checkpoint_manifest_digest.as_str(),
        8,
        10,
        DenseTopologyRevisionActionKind::GrowWorld,
    );
    let shrink_reseed_digest = barrier_reseed_digest(
        checkpoint_manifest_digest.as_str(),
        8,
        6,
        DenseTopologyRevisionActionKind::ShrinkWorld,
    );

    let revisions = vec![
        DenseTopologyRevisionScenario {
            scenario_id: String::from("dense_topology.replace_rank.rank2_cross_provider"),
            topology_revision_id: String::from("psion-xprovider-pretrain-topology-r2"),
            action_kind: DenseTopologyRevisionActionKind::ReplaceRank,
            previous_world_size: 8,
            next_world_size: 8,
            affected_rank_ids: vec![2],
            data_ordering: DenseTopologyRevisionDataOrdering {
                policy_kind: DenseTopologyDataOrderingPolicyKind::ReplayContinuation,
                topology_case_id: provider_loss_recovery.data_ordering.topology_case_id.clone(),
                baseline_global_order_digest: provider_loss_recovery
                    .data_ordering
                    .baseline_global_order_digest
                    .clone(),
                revised_global_order_digest: provider_loss_recovery
                    .data_ordering
                    .revised_global_order_digest
                    .clone(),
                replay_continuity_digest: provider_loss_recovery
                    .data_ordering
                    .replay_continuity_digest
                    .clone(),
                reseed_digest: None,
                refusal: None,
                detail: String::from(
                    "Hot replace-rank topology revision reuses the admitted provider-loss replay-continuity proof from the dense recovery contract.",
                ),
            },
            checkpoint_transition: DenseTopologyRevisionCheckpointTransition {
                transition_kind: DenseTopologyCheckpointTransitionKind::ReuseLatestStableCheckpoint,
                checkpoint_manifest_digest: Some(checkpoint_manifest_digest.clone()),
                recovery_scenario_id: Some(provider_loss_recovery.scenario_id.clone()),
                restore_assignment_ids: provider_loss_recovery
                    .checkpoint_restore_assignment_id
                    .iter()
                    .cloned()
                    .collect(),
                barrier_reshard_plan_id: None,
                detail: String::from(
                    "Hot replace-rank revision reuses the last stable checkpoint and the admitted dense recovery assignment for the replaced rank.",
                ),
            },
            operator_action: DenseTopologyRevisionOperatorAction::AdmitReplacementRank,
            finalizer_action: DenseTopologyRevisionFinalizerAction::PublishRevisionReceipt,
            disposition: DenseTopologyRevisionDisposition::Supported,
            detail: String::from(
                "Current scope supports same-world-size replace-rank revisions as a hot topology change when recovery already proved replay continuity and checkpoint restore binding.",
            ),
        },
        DenseTopologyRevisionScenario {
            scenario_id: String::from("dense_topology.grow_world.8_to_10_checkpoint_barrier"),
            topology_revision_id: String::from("psion-xprovider-pretrain-topology-r3"),
            action_kind: DenseTopologyRevisionActionKind::GrowWorld,
            previous_world_size: 8,
            next_world_size: 10,
            affected_rank_ids: vec![8, 9],
            data_ordering: DenseTopologyRevisionDataOrdering {
                policy_kind: DenseTopologyDataOrderingPolicyKind::CheckpointBarrierReseed,
                topology_case_id: None,
                baseline_global_order_digest: None,
                revised_global_order_digest: None,
                replay_continuity_digest: None,
                reseed_digest: Some(grow_reseed_digest),
                refusal: None,
                detail: String::from(
                    "Grow-world revision is admitted only by pausing at a durable checkpoint barrier, reseeding data ordering for the new world size, and restoring through a new reshard plan.",
                ),
            },
            checkpoint_transition: DenseTopologyRevisionCheckpointTransition {
                transition_kind: DenseTopologyCheckpointTransitionKind::BarrierCheckpointReshardAndRestore,
                checkpoint_manifest_digest: Some(checkpoint_manifest_digest.clone()),
                recovery_scenario_id: None,
                restore_assignment_ids: Vec::new(),
                barrier_reshard_plan_id: Some(String::from("grow-world-8-to-10-reshard")),
                detail: String::from(
                    "Grow-world revision requires a barrier checkpoint and a fresh reshard plan across the larger dense world.",
                ),
            },
            operator_action: DenseTopologyRevisionOperatorAction::PauseAtCheckpointBarrierAndReshard,
            finalizer_action: DenseTopologyRevisionFinalizerAction::PublishBarrierRevisionReceipt,
            disposition: DenseTopologyRevisionDisposition::Supported,
            detail: String::from(
                "Current scope admits grow-world only as a checkpoint-barrier elasticity operation. It is not a live elastic data-feed mutation.",
            ),
        },
        DenseTopologyRevisionScenario {
            scenario_id: String::from("dense_topology.shrink_world.8_to_6_checkpoint_barrier"),
            topology_revision_id: String::from("psion-xprovider-pretrain-topology-r4"),
            action_kind: DenseTopologyRevisionActionKind::ShrinkWorld,
            previous_world_size: 8,
            next_world_size: 6,
            affected_rank_ids: vec![6, 7],
            data_ordering: DenseTopologyRevisionDataOrdering {
                policy_kind: DenseTopologyDataOrderingPolicyKind::CheckpointBarrierReseed,
                topology_case_id: None,
                baseline_global_order_digest: None,
                revised_global_order_digest: None,
                replay_continuity_digest: None,
                reseed_digest: Some(shrink_reseed_digest),
                refusal: None,
                detail: String::from(
                    "Shrink-world revision is admitted only by pausing at a durable checkpoint barrier, reseeding ordering for the smaller world size, and restoring through a new reshard plan.",
                ),
            },
            checkpoint_transition: DenseTopologyRevisionCheckpointTransition {
                transition_kind: DenseTopologyCheckpointTransitionKind::BarrierCheckpointReshardAndRestore,
                checkpoint_manifest_digest: Some(checkpoint_manifest_digest.clone()),
                recovery_scenario_id: None,
                restore_assignment_ids: Vec::new(),
                barrier_reshard_plan_id: Some(String::from("shrink-world-8-to-6-reshard")),
                detail: String::from(
                    "Shrink-world revision requires a barrier checkpoint and a fresh reshard plan across the smaller dense world.",
                ),
            },
            operator_action: DenseTopologyRevisionOperatorAction::PauseAtCheckpointBarrierAndReshard,
            finalizer_action: DenseTopologyRevisionFinalizerAction::PublishBarrierRevisionReceipt,
            disposition: DenseTopologyRevisionDisposition::Supported,
            detail: String::from(
                "Current scope admits shrink-world only as a checkpoint-barrier elasticity operation with an explicit retirement of ranks 6 and 7.",
            ),
        },
        DenseTopologyRevisionScenario {
            scenario_id: String::from("dense_topology.remove_rank_without_replacement.live_refused"),
            topology_revision_id: String::from("psion-xprovider-pretrain-topology-r5"),
            action_kind: DenseTopologyRevisionActionKind::RemoveWithoutReplacement,
            previous_world_size: 8,
            next_world_size: 7,
            affected_rank_ids: vec![3],
            data_ordering: DenseTopologyRevisionDataOrdering {
                policy_kind: DenseTopologyDataOrderingPolicyKind::Refused,
                topology_case_id: None,
                baseline_global_order_digest: None,
                revised_global_order_digest: None,
                replay_continuity_digest: None,
                reseed_digest: None,
                refusal: Some(String::from(
                    "remove-without-replacement still refuses because the current dense control plane requires either hot replace-rank or a checkpoint-barrier grow or shrink revision.",
                )),
                detail: String::from(
                    "Live remove-without-replacement remains out of scope because it would silently blur hot repair and checkpoint-barrier elasticity.",
                ),
            },
            checkpoint_transition: DenseTopologyRevisionCheckpointTransition {
                transition_kind: DenseTopologyCheckpointTransitionKind::RefusedNoCheckpointTransition,
                checkpoint_manifest_digest: None,
                recovery_scenario_id: None,
                restore_assignment_ids: Vec::new(),
                barrier_reshard_plan_id: None,
                detail: String::from(
                    "Refused live removal emits no fake checkpoint transition.",
                ),
            },
            operator_action: DenseTopologyRevisionOperatorAction::RefuseLiveRevisionAndPageOperator,
            finalizer_action: DenseTopologyRevisionFinalizerAction::PublishRefusalReceipt,
            disposition: DenseTopologyRevisionDisposition::Refused,
            detail: String::from(
                "Current scope still refuses live removal without replacement. Operators must choose hot replace-rank or a checkpoint-barrier shrink revision instead.",
            ),
        },
    ];

    let mut contract = DenseTopologyRevisionContract {
        schema_version: String::from(DENSE_TOPOLOGY_REVISION_CONTRACT_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        whole_program_run_graph_digest: run_graph.contract_digest.clone(),
        dense_recovery_contract_digest: dense_recovery.contract_digest.clone(),
        distributed_checkpoint_contract_digest: checkpoint_contract.contract_digest.clone(),
        revisions,
        claim_boundary: String::from(
            "This contract closes controlled replace, grow, and shrink topology revisions for the current dense cluster only under explicit hot-repair or checkpoint-barrier policy. It does not claim generic live elasticity, public-swarm membership, or hidden world-size changes.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

/// Writes the canonical dense topology-revision contract to disk.
pub fn write_dense_topology_revision_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), DenseTopologyRevisionContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DenseTopologyRevisionContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_dense_topology_revision_contract()?;
    let json = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| DenseTopologyRevisionContractError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn barrier_reseed_digest(
    checkpoint_manifest_digest: &str,
    previous_world_size: u16,
    next_world_size: u16,
    action_kind: DenseTopologyRevisionActionKind,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_dense_topology_barrier_reseed|");
    hasher.update(checkpoint_manifest_digest.as_bytes());
    hasher.update([0]);
    hasher.update(previous_world_size.to_le_bytes());
    hasher.update([0]);
    hasher.update(next_world_size.to_le_bytes());
    hasher.update([0]);
    hasher.update(action_label(action_kind).as_bytes());
    hex::encode(hasher.finalize())
}

fn action_label(action_kind: DenseTopologyRevisionActionKind) -> &'static str {
    match action_kind {
        DenseTopologyRevisionActionKind::ReplaceRank => "replace_rank",
        DenseTopologyRevisionActionKind::GrowWorld => "grow_world",
        DenseTopologyRevisionActionKind::ShrinkWorld => "shrink_world",
        DenseTopologyRevisionActionKind::RemoveWithoutReplacement => "remove_without_replacement",
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("dense topology-revision contract digest serialization must work"),
    );
    hex::encode(hasher.finalize())
}
