use std::{collections::BTreeMap, fs, path::Path};

use psionic_data::{
    builtin_topology_revisable_distributed_data_feed_semantics_report,
    TopologyRevisableDataFeedRevisionActionKind, TopologyRevisableDataFeedRevisionReasonKind,
    TopologyRevisableDistributedDataFeedCapabilityStatus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_remote_train_artifact_backend_contract_set,
    canonical_sharded_distributed_checkpoint_contract, cross_provider_training_program_manifest,
    CrossProviderTrainingProgramManifestError, DistributedCheckpointContractError,
    RemoteTrainArtifactBackendContractError, TrainArtifactClass, TrainingRecoveryMode,
};

/// Stable schema version for the dense-rank recovery contract.
pub const DENSE_RANK_RECOVERY_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.dense_rank_recovery_contract.v1";
/// Stable fixture path for the dense-rank recovery contract.
pub const DENSE_RANK_RECOVERY_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/dense_rank_recovery_contract_v1.json";
/// Stable checker path for the dense-rank recovery contract.
pub const DENSE_RANK_RECOVERY_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-dense-rank-recovery-contract.sh";
/// Stable reference doc path for the dense-rank recovery contract.
pub const DENSE_RANK_RECOVERY_CONTRACT_DOC_PATH: &str = "docs/DENSE_RANK_RECOVERY_REFERENCE.md";

/// Error surfaced while building, validating, or writing the dense-rank recovery contract.
#[derive(Debug, Error)]
pub enum DenseRankRecoveryContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    DistributedCheckpoint(#[from] DistributedCheckpointContractError),
    #[error(transparent)]
    RemoteArtifactBackend(#[from] RemoteTrainArtifactBackendContractError),
    #[error("dense-rank recovery contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Failure family admitted or refused by the current dense recovery contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseRankFailureKind {
    Preemption,
    NodeLoss,
    ProviderLoss,
}

/// Final recovery posture for one dense-rank scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseRankRecoveryDisposition {
    Recovered,
    Refused,
}

/// How replay ordering is preserved or refused under one recovery scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseRankRecoveryContinuityKind {
    CheckpointResumeNoTopologyChange,
    ReplaceRankReplayContinuation,
    RefusedWorldChange,
}

/// Operator action required for one recovery scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseRankRecoveryOperatorAction {
    RehydrateRankInPlace,
    AdmitReplacementAtSameRank,
    HoldRunAndPageOperator,
}

/// Finalizer action required for one recovery scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseRankRecoveryFinalizerAction {
    PublishRecoveryReceiptOnly,
    PublishRecoveryReceiptAndTopologyRevision,
    PublishRefusalAndHoldRun,
}

/// Data-ordering binding for one recovery scenario.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseRankRecoveryDataOrderingBinding {
    pub continuity_kind: DenseRankRecoveryContinuityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_case_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_global_order_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_global_order_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay_continuity_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    pub detail: String,
}

/// One admitted or refused dense-rank recovery scenario.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseRankRecoveryScenario {
    pub scenario_id: String,
    pub failure_kind: DenseRankFailureKind,
    pub dense_rank: u16,
    pub departing_source_id: String,
    pub departing_node_id: String,
    pub departing_provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_mode: Option<TrainingRecoveryMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_restore_assignment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_manifest_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_shard_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimizer_shard_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_order: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_authority_backend_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirror_backend_ids: Vec<String>,
    pub data_ordering: DenseRankRecoveryDataOrderingBinding,
    pub operator_action: DenseRankRecoveryOperatorAction,
    pub finalizer_action: DenseRankRecoveryFinalizerAction,
    pub disposition: DenseRankRecoveryDisposition,
    pub detail: String,
}

/// Canonical provider-neutral dense-rank recovery contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseRankRecoveryContract {
    pub schema_version: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub distributed_checkpoint_contract_digest: String,
    pub remote_artifact_backend_contract_digest: String,
    pub topology_revisable_data_feed_report_digest: String,
    pub scenarios: Vec<DenseRankRecoveryScenario>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl DenseRankRecoveryContract {
    /// Returns the stable digest over the contract payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_dense_rank_recovery_contract|", &clone)
    }

    /// Validates the dense-rank recovery contract against canonical checkpoint, storage, and data-ordering truth.
    pub fn validate(&self) -> Result<(), DenseRankRecoveryContractError> {
        let manifest = cross_provider_training_program_manifest()?;
        let checkpoint_contract = canonical_sharded_distributed_checkpoint_contract()?;
        let remote_backends = canonical_remote_train_artifact_backend_contract_set()?;
        let topology_report = builtin_topology_revisable_distributed_data_feed_semantics_report();

        if self.schema_version != DENSE_RANK_RECOVERY_CONTRACT_SCHEMA_VERSION {
            return Err(DenseRankRecoveryContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    DENSE_RANK_RECOVERY_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(DenseRankRecoveryContractError::InvalidContract {
                detail: String::from("program manifest binding drifted"),
            });
        }
        if self.distributed_checkpoint_contract_digest != checkpoint_contract.contract_digest {
            return Err(DenseRankRecoveryContractError::InvalidContract {
                detail: String::from("distributed checkpoint contract digest drifted"),
            });
        }
        if self.remote_artifact_backend_contract_digest != remote_backends.contract_digest {
            return Err(DenseRankRecoveryContractError::InvalidContract {
                detail: String::from("remote artifact backend contract digest drifted"),
            });
        }
        if self.topology_revisable_data_feed_report_digest != topology_report.report_digest {
            return Err(DenseRankRecoveryContractError::InvalidContract {
                detail: String::from("topology-revisable data-feed report digest drifted"),
            });
        }
        if self.scenarios.len() != 4 {
            return Err(DenseRankRecoveryContractError::InvalidContract {
                detail: String::from(
                    "expected exactly four canonical dense-rank recovery scenarios",
                ),
            });
        }

        let checkpoint_restore_by_rank = checkpoint_contract
            .restore_plan
            .assignments
            .iter()
            .map(|assignment| (assignment.dense_rank, assignment))
            .collect::<BTreeMap<_, _>>();
        let checkpoint_decision = remote_backends
            .placement_decisions
            .iter()
            .find(|decision| decision.artifact_class == TrainArtifactClass::Checkpoint)
            .ok_or_else(|| DenseRankRecoveryContractError::InvalidContract {
                detail: String::from(
                    "remote artifact backend contract lost checkpoint placement decision",
                ),
            })?;
        let topology_cases = topology_report
            .cases
            .iter()
            .map(|case| (case.case_id.as_str(), case))
            .collect::<BTreeMap<_, _>>();

        let mut recovered_failures = BTreeMap::new();
        let mut refused_failures = BTreeMap::new();

        for scenario in &self.scenarios {
            match scenario.disposition {
                DenseRankRecoveryDisposition::Recovered => {
                    recovered_failures.insert(scenario.scenario_id.as_str(), scenario.failure_kind);
                    let recovery_mode = scenario.recovery_mode.ok_or_else(|| {
                        DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "recovered scenario `{}` must declare a recovery mode",
                                scenario.scenario_id
                            ),
                        }
                    })?;
                    if recovery_mode != TrainingRecoveryMode::ResumeFromLastStableCheckpoint {
                        return Err(DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "recovered scenario `{}` must currently use resume_from_last_stable_checkpoint",
                                scenario.scenario_id
                            ),
                        });
                    }
                    let assignment_id = scenario
                        .checkpoint_restore_assignment_id
                        .as_deref()
                        .ok_or_else(|| DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "recovered scenario `{}` must name one checkpoint restore assignment",
                                scenario.scenario_id
                            ),
                        })?;
                    let assignment = checkpoint_restore_by_rank
                        .get(&scenario.dense_rank)
                        .ok_or_else(|| DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "recovered scenario `{}` references unknown dense rank {}",
                                scenario.scenario_id, scenario.dense_rank
                            ),
                        })?;
                    if assignment.assignment_id != assignment_id {
                        return Err(DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "recovered scenario `{}` drifted from canonical restore assignment `{}`",
                                scenario.scenario_id, assignment.assignment_id
                            ),
                        });
                    }
                    if scenario.checkpoint_manifest_digest.as_deref()
                        != Some(
                            checkpoint_contract
                                .checkpoint_manifest
                                .manifest_digest
                                .as_str(),
                        )
                        || scenario.parameter_shard_id.as_deref()
                            != Some(assignment.parameter_shard_id.as_str())
                        || scenario.optimizer_shard_id.as_deref()
                            != Some(assignment.optimizer_shard_id.as_str())
                        || scenario.load_order != assignment.load_order
                    {
                        return Err(DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "recovered scenario `{}` lost checkpoint shard or load-order truth",
                                scenario.scenario_id
                            ),
                        });
                    }
                    if scenario.restore_authority_backend_id.as_deref()
                        != Some(checkpoint_decision.restore_authority_backend_id.as_str())
                        || scenario.mirror_backend_ids != checkpoint_decision.mirror_backend_ids
                    {
                        return Err(DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "recovered scenario `{}` drifted from checkpoint artifact backend policy",
                                scenario.scenario_id
                            ),
                        });
                    }
                    match scenario.failure_kind {
                        DenseRankFailureKind::Preemption => {
                            if scenario.replacement_source_id.is_some()
                                || scenario.replacement_node_id.is_some()
                                || scenario.replacement_provider_id.is_some()
                            {
                                return Err(DenseRankRecoveryContractError::InvalidContract {
                                    detail: String::from(
                                        "preemption recovery must stay in-place and may not invent a replacement topology",
                                    ),
                                });
                            }
                            if scenario.data_ordering.continuity_kind
                                != DenseRankRecoveryContinuityKind::CheckpointResumeNoTopologyChange
                            {
                                return Err(DenseRankRecoveryContractError::InvalidContract {
                                    detail: String::from(
                                        "preemption recovery must keep checkpoint_resume_no_topology_change continuity",
                                    ),
                                });
                            }
                        }
                        DenseRankFailureKind::NodeLoss => {
                            validate_replace_rank_data_ordering(
                                scenario,
                                &topology_cases,
                                "dense_rank.node_loss.replace_rank1",
                                TopologyRevisableDataFeedRevisionReasonKind::NodeLoss,
                            )?;
                            if scenario.replacement_provider_id.as_deref()
                                != Some(scenario.departing_provider_id.as_str())
                            {
                                return Err(DenseRankRecoveryContractError::InvalidContract {
                                    detail: String::from(
                                        "node-loss recovery must keep the replacement in the same provider in the current scope",
                                    ),
                                });
                            }
                        }
                        DenseRankFailureKind::ProviderLoss => {
                            validate_replace_rank_data_ordering(
                                scenario,
                                &topology_cases,
                                "dense_rank.provider_loss.replace_rank2",
                                TopologyRevisableDataFeedRevisionReasonKind::ProviderLoss,
                            )?;
                            if scenario.replacement_provider_id.as_deref()
                                == Some(scenario.departing_provider_id.as_str())
                            {
                                return Err(DenseRankRecoveryContractError::InvalidContract {
                                    detail: String::from(
                                        "provider-loss recovery must actually switch providers in the current scope",
                                    ),
                                });
                            }
                        }
                    }
                }
                DenseRankRecoveryDisposition::Refused => {
                    refused_failures.insert(scenario.scenario_id.as_str(), scenario.failure_kind);
                    if scenario.recovery_mode.is_some()
                        || scenario.checkpoint_restore_assignment_id.is_some()
                        || scenario.checkpoint_manifest_digest.is_some()
                        || scenario.parameter_shard_id.is_some()
                        || scenario.optimizer_shard_id.is_some()
                        || !scenario.load_order.is_empty()
                        || scenario.restore_authority_backend_id.is_some()
                        || !scenario.mirror_backend_ids.is_empty()
                    {
                        return Err(DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "refused scenario `{}` must fail closed without fake restore bindings",
                                scenario.scenario_id
                            ),
                        });
                    }
                    if scenario.data_ordering.continuity_kind
                        != DenseRankRecoveryContinuityKind::RefusedWorldChange
                    {
                        return Err(DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "refused scenario `{}` must keep refused_world_change continuity",
                                scenario.scenario_id
                            ),
                        });
                    }
                    let shrink_world = topology_cases
                        .get("dense_rank.shrink_world.refused")
                        .ok_or_else(|| DenseRankRecoveryContractError::InvalidContract {
                            detail: String::from(
                                "topology-revisable report lost dense_rank.shrink_world.refused case",
                            ),
                        })?;
                    if scenario.data_ordering.topology_case_id.as_deref()
                        != Some(shrink_world.case_id.as_str())
                        || scenario.data_ordering.refusal != shrink_world.refusal
                    {
                        return Err(DenseRankRecoveryContractError::InvalidContract {
                            detail: format!(
                                "refused scenario `{}` lost the canonical shrink-world refusal binding",
                                scenario.scenario_id
                            ),
                        });
                    }
                }
            }
        }

        if recovered_failures.len() != 3 || refused_failures.len() != 1 {
            return Err(DenseRankRecoveryContractError::InvalidContract {
                detail: String::from(
                    "dense-rank recovery must keep three recovered scenarios and one refused scenario in the current scope",
                ),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(DenseRankRecoveryContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical provider-neutral dense-rank recovery contract.
pub fn canonical_dense_rank_recovery_contract(
) -> Result<DenseRankRecoveryContract, DenseRankRecoveryContractError> {
    let manifest = cross_provider_training_program_manifest()?;
    let checkpoint_contract = canonical_sharded_distributed_checkpoint_contract()?;
    let remote_backends = canonical_remote_train_artifact_backend_contract_set()?;
    let topology_report = builtin_topology_revisable_distributed_data_feed_semantics_report();

    let checkpoint_decision = remote_backends
        .placement_decisions
        .iter()
        .find(|decision| decision.artifact_class == TrainArtifactClass::Checkpoint)
        .expect("canonical remote backend contract must keep checkpoint decision");
    let node_loss_case = topology_report
        .cases
        .iter()
        .find(|case| case.case_id == "dense_rank.node_loss.replace_rank1")
        .expect("canonical topology report must keep node-loss replacement case");
    let provider_loss_case = topology_report
        .cases
        .iter()
        .find(|case| case.case_id == "dense_rank.provider_loss.replace_rank2")
        .expect("canonical topology report must keep provider-loss replacement case");
    let shrink_world_case = topology_report
        .cases
        .iter()
        .find(|case| case.case_id == "dense_rank.shrink_world.refused")
        .expect("canonical topology report must keep shrink-world refusal case");
    let restore_rank1 = checkpoint_contract
        .restore_plan
        .assignments
        .iter()
        .find(|assignment| assignment.dense_rank == 1)
        .expect("canonical checkpoint contract must keep rank 1 restore assignment");
    let restore_rank2 = checkpoint_contract
        .restore_plan
        .assignments
        .iter()
        .find(|assignment| assignment.dense_rank == 2)
        .expect("canonical checkpoint contract must keep rank 2 restore assignment");
    let restore_rank3 = checkpoint_contract
        .restore_plan
        .assignments
        .iter()
        .find(|assignment| assignment.dense_rank == 3)
        .expect("canonical checkpoint contract must keep rank 3 restore assignment");

    let scenarios = vec![
        DenseRankRecoveryScenario {
            scenario_id: String::from("dense_rank.preemption.rank3.resume_in_place"),
            failure_kind: DenseRankFailureKind::Preemption,
            dense_rank: 3,
            departing_source_id: String::from("runpod_8xh100_dense_node"),
            departing_node_id: String::from("runpod-h100-rank3"),
            departing_provider_id: String::from("runpod"),
            replacement_source_id: None,
            replacement_node_id: None,
            replacement_provider_id: None,
            recovery_mode: Some(TrainingRecoveryMode::ResumeFromLastStableCheckpoint),
            checkpoint_restore_assignment_id: Some(restore_rank3.assignment_id.clone()),
            checkpoint_manifest_digest: Some(checkpoint_contract.checkpoint_manifest.manifest_digest.clone()),
            parameter_shard_id: Some(restore_rank3.parameter_shard_id.clone()),
            optimizer_shard_id: Some(restore_rank3.optimizer_shard_id.clone()),
            load_order: restore_rank3.load_order.clone(),
            restore_authority_backend_id: Some(checkpoint_decision.restore_authority_backend_id.clone()),
            mirror_backend_ids: checkpoint_decision.mirror_backend_ids.clone(),
            data_ordering: DenseRankRecoveryDataOrderingBinding {
                continuity_kind: DenseRankRecoveryContinuityKind::CheckpointResumeNoTopologyChange,
                topology_case_id: None,
                baseline_global_order_digest: None,
                revised_global_order_digest: None,
                replay_continuity_digest: None,
                refusal: None,
                detail: String::from(
                    "Preemption keeps the dense-rank topology unchanged, so replay ordering stays on the already accepted global shard order and resumes from the last stable checkpoint without a topology revision.",
                ),
            },
            operator_action: DenseRankRecoveryOperatorAction::RehydrateRankInPlace,
            finalizer_action: DenseRankRecoveryFinalizerAction::PublishRecoveryReceiptOnly,
            disposition: DenseRankRecoveryDisposition::Recovered,
            detail: String::from(
                "Current scope admits preemption recovery by rehydrating the same dense rank in place from the last durable checkpoint under the checkpoint bucket authority.",
            ),
        },
        DenseRankRecoveryScenario {
            scenario_id: String::from("dense_rank.node_loss.rank1.same_provider_replace"),
            failure_kind: DenseRankFailureKind::NodeLoss,
            dense_rank: 1,
            departing_source_id: String::from("runpod_8xh100_dense_node"),
            departing_node_id: String::from("runpod-h100-rank1"),
            departing_provider_id: String::from("runpod"),
            replacement_source_id: Some(String::from("runpod_8xh100_dense_node_spare")),
            replacement_node_id: Some(String::from("runpod-h100-rank1b")),
            replacement_provider_id: Some(String::from("runpod")),
            recovery_mode: Some(TrainingRecoveryMode::ResumeFromLastStableCheckpoint),
            checkpoint_restore_assignment_id: Some(restore_rank1.assignment_id.clone()),
            checkpoint_manifest_digest: Some(checkpoint_contract.checkpoint_manifest.manifest_digest.clone()),
            parameter_shard_id: Some(restore_rank1.parameter_shard_id.clone()),
            optimizer_shard_id: Some(restore_rank1.optimizer_shard_id.clone()),
            load_order: restore_rank1.load_order.clone(),
            restore_authority_backend_id: Some(checkpoint_decision.restore_authority_backend_id.clone()),
            mirror_backend_ids: checkpoint_decision.mirror_backend_ids.clone(),
            data_ordering: DenseRankRecoveryDataOrderingBinding {
                continuity_kind: DenseRankRecoveryContinuityKind::ReplaceRankReplayContinuation,
                topology_case_id: Some(node_loss_case.case_id.clone()),
                baseline_global_order_digest: node_loss_case.baseline_global_order_digest.clone(),
                revised_global_order_digest: node_loss_case.revised_global_order_digest.clone(),
                replay_continuity_digest: node_loss_case.replay_continuity_digest.clone(),
                refusal: None,
                detail: String::from(
                    "Same-provider node loss uses the admitted replace-rank data-feed path so rank 1 keeps the same shard ordering while the replacement node inherits the departed rank's replay plan.",
                ),
            },
            operator_action: DenseRankRecoveryOperatorAction::AdmitReplacementAtSameRank,
            finalizer_action: DenseRankRecoveryFinalizerAction::PublishRecoveryReceiptAndTopologyRevision,
            disposition: DenseRankRecoveryDisposition::Recovered,
            detail: String::from(
                "Current scope admits same-provider node-loss recovery by replacing the departed dense rank at the same world size and rehydrating the replacement from the durable checkpoint shards.",
            ),
        },
        DenseRankRecoveryScenario {
            scenario_id: String::from("dense_rank.provider_loss.rank2.cross_provider_replace"),
            failure_kind: DenseRankFailureKind::ProviderLoss,
            dense_rank: 2,
            departing_source_id: String::from("runpod_8xh100_dense_node"),
            departing_node_id: String::from("runpod-h100-rank2"),
            departing_provider_id: String::from("runpod"),
            replacement_source_id: Some(String::from("google_a3_dense_recovery_spare")),
            replacement_node_id: Some(String::from("google-h100-rank2")),
            replacement_provider_id: Some(String::from("google")),
            recovery_mode: Some(TrainingRecoveryMode::ResumeFromLastStableCheckpoint),
            checkpoint_restore_assignment_id: Some(restore_rank2.assignment_id.clone()),
            checkpoint_manifest_digest: Some(checkpoint_contract.checkpoint_manifest.manifest_digest.clone()),
            parameter_shard_id: Some(restore_rank2.parameter_shard_id.clone()),
            optimizer_shard_id: Some(restore_rank2.optimizer_shard_id.clone()),
            load_order: restore_rank2.load_order.clone(),
            restore_authority_backend_id: Some(checkpoint_decision.restore_authority_backend_id.clone()),
            mirror_backend_ids: checkpoint_decision.mirror_backend_ids.clone(),
            data_ordering: DenseRankRecoveryDataOrderingBinding {
                continuity_kind: DenseRankRecoveryContinuityKind::ReplaceRankReplayContinuation,
                topology_case_id: Some(provider_loss_case.case_id.clone()),
                baseline_global_order_digest: provider_loss_case.baseline_global_order_digest.clone(),
                revised_global_order_digest: provider_loss_case.revised_global_order_digest.clone(),
                replay_continuity_digest: provider_loss_case.replay_continuity_digest.clone(),
                refusal: None,
                detail: String::from(
                    "Cross-provider provider loss uses the admitted replace-rank data-feed path so the replacement provider inherits the departed rank's replay order at the same world size.",
                ),
            },
            operator_action: DenseRankRecoveryOperatorAction::AdmitReplacementAtSameRank,
            finalizer_action: DenseRankRecoveryFinalizerAction::PublishRecoveryReceiptAndTopologyRevision,
            disposition: DenseRankRecoveryDisposition::Recovered,
            detail: String::from(
                "Current scope admits provider-loss recovery only when one replacement rank is admitted on another provider at the same world size and restores from the shared checkpoint authority backend.",
            ),
        },
        DenseRankRecoveryScenario {
            scenario_id: String::from("dense_rank.provider_loss.rank3.shrink_world_refused"),
            failure_kind: DenseRankFailureKind::ProviderLoss,
            dense_rank: 3,
            departing_source_id: String::from("runpod_8xh100_dense_node"),
            departing_node_id: String::from("runpod-h100-rank3"),
            departing_provider_id: String::from("runpod"),
            replacement_source_id: None,
            replacement_node_id: None,
            replacement_provider_id: None,
            recovery_mode: None,
            checkpoint_restore_assignment_id: None,
            checkpoint_manifest_digest: None,
            parameter_shard_id: None,
            optimizer_shard_id: None,
            load_order: Vec::new(),
            restore_authority_backend_id: None,
            mirror_backend_ids: Vec::new(),
            data_ordering: DenseRankRecoveryDataOrderingBinding {
                continuity_kind: DenseRankRecoveryContinuityKind::RefusedWorldChange,
                topology_case_id: Some(shrink_world_case.case_id.clone()),
                baseline_global_order_digest: None,
                revised_global_order_digest: None,
                replay_continuity_digest: None,
                refusal: shrink_world_case.refusal.clone(),
                detail: String::from(
                    "Current scope still refuses shrink-world recovery because the dense data-feed contract only admits fixed-world replace-rank revisions.",
                ),
            },
            operator_action: DenseRankRecoveryOperatorAction::HoldRunAndPageOperator,
            finalizer_action: DenseRankRecoveryFinalizerAction::PublishRefusalAndHoldRun,
            disposition: DenseRankRecoveryDisposition::Refused,
            detail: String::from(
                "Provider loss without a replacement rank stays refused. The run must hold with an explicit refusal receipt instead of silently shrinking the dense world size.",
            ),
        },
    ];

    let mut contract = DenseRankRecoveryContract {
        schema_version: String::from(DENSE_RANK_RECOVERY_CONTRACT_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        distributed_checkpoint_contract_digest: checkpoint_contract.contract_digest.clone(),
        remote_artifact_backend_contract_digest: remote_backends.contract_digest.clone(),
        topology_revisable_data_feed_report_digest: topology_report.report_digest.clone(),
        scenarios,
        claim_boundary: String::from(
            "This contract closes admitted dense-rank recovery receipts for preemption, same-provider node loss, and cross-provider provider loss under one checkpoint and replay-ordering contract. It still refuses shrink-world recovery, public-internet swarm repair, and mixed-backend dense recovery.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

/// Writes the canonical dense-rank recovery contract to disk.
pub fn write_dense_rank_recovery_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), DenseRankRecoveryContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| DenseRankRecoveryContractError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_dense_rank_recovery_contract()?;
    let json = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| DenseRankRecoveryContractError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn validate_replace_rank_data_ordering(
    scenario: &DenseRankRecoveryScenario,
    topology_cases: &BTreeMap<
        &str,
        &psionic_data::TopologyRevisableDistributedDataFeedCapabilityCaseResult,
    >,
    expected_case_id: &str,
    expected_reason_kind: TopologyRevisableDataFeedRevisionReasonKind,
) -> Result<(), DenseRankRecoveryContractError> {
    if scenario.data_ordering.continuity_kind
        != DenseRankRecoveryContinuityKind::ReplaceRankReplayContinuation
    {
        return Err(DenseRankRecoveryContractError::InvalidContract {
            detail: format!(
                "scenario `{}` must keep replace_rank_replay_continuation continuity",
                scenario.scenario_id
            ),
        });
    }
    let case = topology_cases.get(expected_case_id).ok_or_else(|| {
        DenseRankRecoveryContractError::InvalidContract {
            detail: format!("topology report lost expected case `{expected_case_id}`"),
        }
    })?;
    if case.status != TopologyRevisableDistributedDataFeedCapabilityStatus::Supported
        || case.action_kind != TopologyRevisableDataFeedRevisionActionKind::ReplaceRank
        || case.reason_kind != expected_reason_kind
    {
        return Err(DenseRankRecoveryContractError::InvalidContract {
            detail: format!(
                "topology report case `{expected_case_id}` lost the expected replace-rank support posture",
            ),
        });
    }
    if scenario.data_ordering.topology_case_id.as_deref() != Some(case.case_id.as_str())
        || scenario.data_ordering.baseline_global_order_digest != case.baseline_global_order_digest
        || scenario.data_ordering.revised_global_order_digest != case.revised_global_order_digest
        || scenario.data_ordering.replay_continuity_digest != case.replay_continuity_digest
    {
        return Err(DenseRankRecoveryContractError::InvalidContract {
            detail: format!(
                "scenario `{}` drifted from topology report case `{expected_case_id}`",
                scenario.scenario_id
            ),
        });
    }
    if scenario.replacement_source_id.is_none()
        || scenario.replacement_node_id.is_none()
        || scenario.replacement_provider_id.is_none()
    {
        return Err(DenseRankRecoveryContractError::InvalidContract {
            detail: format!(
                "scenario `{}` must name a replacement source, node, and provider",
                scenario.scenario_id
            ),
        });
    }
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("dense-rank recovery contract digest serialization must work"),
    );
    hex::encode(hasher.finalize())
}
