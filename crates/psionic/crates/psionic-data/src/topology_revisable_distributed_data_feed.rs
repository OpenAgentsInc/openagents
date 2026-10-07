use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use psionic_datastream::{
    DatastreamEncoding, DatastreamManifest, DatastreamManifestRef, DatastreamSubjectKind,
};
use psionic_runtime::RuntimeDeterminismContract;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    digest_lines, DataIngressContract, DatasetBatchSamplerContract, DatasetIterationContract,
    DatasetIterationMode, DatasetKey, DatasetManifest, DatasetPackingMode, DatasetPackingPolicy,
    DatasetRecordEncoding, DatasetSamplerContract, DatasetSamplerKind, DatasetShardManifest,
    DatasetShardOrdering, DatasetSourceContract, DatasetSourceKind, DatasetSplitDeclaration,
    DatasetSplitKind, DistributedDataFeedContract, DistributedDataFeedContractError,
    DistributedDataFeedPlan, DistributedReplayOrderingContract,
    DistributedSamplerPartitionContract, DistributedSamplerPartitionKind,
    DistributedWorkerCoordinationContract, DistributedWorkerCoordinationMode,
    HostDeviceStagingContract, HostDeviceStagingMode, TokenizerDigest, TokenizerFamily,
};

/// Stable fixture path for the topology-revisable distributed data-feed report.
pub const TOPOLOGY_REVISABLE_DISTRIBUTED_DATA_FEED_REPORT_FIXTURE_PATH: &str =
    "fixtures/training/topology_revisable_distributed_data_feed_report_v1.json";
/// Stable checker path for the topology-revisable distributed data-feed report.
pub const TOPOLOGY_REVISABLE_DISTRIBUTED_DATA_FEED_CHECK_SCRIPT_PATH: &str =
    "scripts/check-topology-revisable-distributed-data-feed.sh";
/// Stable runner doc path for the topology-revisable distributed data-feed report.
pub const TOPOLOGY_REVISABLE_DISTRIBUTED_DATA_FEED_DOC_PATH: &str =
    "docs/DISTRIBUTED_DATA_FEED_SEMANTICS.md";

/// Supported topology-revision actions for the current cross-provider dense data-feed scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyRevisableDataFeedRevisionActionKind {
    /// Replace one failed or departed rank with one new node at the same rank.
    ReplaceRank,
    /// Remove one rank without replacement.
    RemoveRank,
    /// Grow the world size.
    GrowWorld,
    /// Shrink the world size.
    ShrinkWorld,
}

/// Reason attached to one admitted or refused topology-revision request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyRevisableDataFeedRevisionReasonKind {
    /// One host died or became unreachable.
    NodeLoss,
    /// One provider or zone lane dropped out.
    ProviderLoss,
    /// The operator replaced the node intentionally.
    PlannedReplacement,
}

/// One machine-visible dense-rank member in the current data-feed topology.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyRevisableDistributedWorkerMember {
    /// Stable node id.
    pub node_id: String,
    /// Stable provider or locality id.
    pub provider_id: String,
    /// Stable dense rank.
    pub rank: usize,
}

/// One explicit topology-revision request over a dense-rank data feed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyRevisableDataFeedRevisionRequest {
    /// Stable run id.
    pub run_id: String,
    /// Stable previous topology revision id.
    pub previous_topology_revision_id: String,
    /// Stable new topology revision id.
    pub topology_revision_id: String,
    /// Requested revision action.
    pub action_kind: TopologyRevisableDataFeedRevisionActionKind,
    /// Reason for the revision.
    pub reason_kind: TopologyRevisableDataFeedRevisionReasonKind,
    /// Departing rank.
    pub departing_rank: usize,
    /// Departing node id.
    pub departing_node_id: String,
    /// Requested world size after the revision.
    pub requested_world_size: usize,
    /// Replacement node id when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_node_id: Option<String>,
    /// Replacement provider id when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_provider_id: Option<String>,
}

impl TopologyRevisableDataFeedRevisionRequest {
    fn stable_digest(&self) -> String {
        digest_lines(vec![
            format!("run_id={}", self.run_id),
            format!(
                "previous_topology_revision_id={}",
                self.previous_topology_revision_id
            ),
            format!("topology_revision_id={}", self.topology_revision_id),
            format!(
                "action_kind={}",
                topology_revision_action_kind_label(self.action_kind)
            ),
            format!(
                "reason_kind={}",
                topology_revision_reason_kind_label(self.reason_kind)
            ),
            format!("departing_rank={}", self.departing_rank),
            format!("departing_node_id={}", self.departing_node_id),
            format!("requested_world_size={}", self.requested_world_size),
            format!(
                "replacement_node_id={}",
                self.replacement_node_id
                    .clone()
                    .unwrap_or_else(|| String::from("none"))
            ),
            format!(
                "replacement_provider_id={}",
                self.replacement_provider_id
                    .clone()
                    .unwrap_or_else(|| String::from("none"))
            ),
        ])
    }
}

/// One worker-specific replay plan under one explicit topology revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyRevisableDistributedWorkerPlan {
    /// Current worker member.
    pub member: TopologyRevisableDistributedWorkerMember,
    /// Worker-visible replay plan.
    pub plan: DistributedDataFeedPlan,
}

/// One shard ownership reassignment caused by one admitted topology revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyRevisableShardOwnershipReassignment {
    /// Stable shard identity.
    pub shard_key: String,
    /// Stable shard manifest ref.
    pub manifest: DatastreamManifestRef,
    /// Stable global shard index.
    pub global_shard_index: usize,
    /// Previous dense rank.
    pub previous_rank: usize,
    /// New dense rank.
    pub next_rank: usize,
    /// Previous node id.
    pub previous_node_id: String,
    /// New node id.
    pub next_node_id: String,
    /// Revision reason that caused the reassignment.
    pub reason_kind: TopologyRevisableDataFeedRevisionReasonKind,
}

/// One machine-legible receipt for one admitted topology revision over the dense data feed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyRevisableDistributedDataFeedReceipt {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable digest over the base dense data-feed contract.
    pub contract_digest: String,
    /// Stable revision request.
    pub revision_request: TopologyRevisableDataFeedRevisionRequest,
    /// Replay-safe global order before the revision.
    pub baseline_global_order_digest: String,
    /// Replay-safe global order after the revision.
    pub revised_global_order_digest: String,
    /// Per-rank worker plans before the revision.
    pub baseline_rank_plans: Vec<TopologyRevisableDistributedWorkerPlan>,
    /// Per-rank worker plans after the revision.
    pub revised_rank_plans: Vec<TopologyRevisableDistributedWorkerPlan>,
    /// Explicit shard ownership reassignments caused by the revision.
    pub shard_reassignments: Vec<TopologyRevisableShardOwnershipReassignment>,
    /// Stable digest proving replay continuity across the revision.
    pub replay_continuity_digest: String,
    /// Honest bounded claim window.
    pub bounded_scope: String,
    /// Stable digest over the whole receipt.
    pub receipt_digest: String,
}

impl TopologyRevisableDistributedDataFeedReceipt {
    fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        digest_lines(vec![
            format!("schema_version={}", digestible.schema_version),
            format!("contract_digest={}", digestible.contract_digest),
            format!(
                "revision_request={}",
                digestible.revision_request.stable_digest()
            ),
            format!(
                "baseline_global_order_digest={}",
                digestible.baseline_global_order_digest
            ),
            format!(
                "revised_global_order_digest={}",
                digestible.revised_global_order_digest
            ),
            format!(
                "replay_continuity_digest={}",
                digestible.replay_continuity_digest
            ),
            format!("bounded_scope={}", digestible.bounded_scope),
        ])
    }
}

/// Planning error for topology-revisable distributed data feed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TopologyRevisableDistributedDataFeedError {
    #[error(transparent)]
    DataFeed(#[from] DistributedDataFeedContractError),
    #[error("topology-revisable data feed requires one non-empty worker set")]
    EmptyWorkerSet,
    #[error("topology-revisable data feed lost rank coverage for rank {rank}")]
    MissingRank { rank: usize },
    #[error("topology-revisable data feed saw duplicate rank {rank}")]
    DuplicateRank { rank: usize },
    #[error("topology-revisable data feed saw duplicate node id `{node_id}`")]
    DuplicateNodeId { node_id: String },
    #[error(
        "topology-revisable data feed requires requested_world_size {current_world_size} for current revision action, found {requested_world_size}"
    )]
    UnsupportedWorldSizeChange {
        current_world_size: usize,
        requested_world_size: usize,
    },
    #[error("topology-revisable data feed requires departing rank {rank} to exist")]
    UnknownDepartingRank { rank: usize },
    #[error(
        "topology-revisable data feed departing node mismatch for rank {rank}: expected `{expected}`, found `{actual}`"
    )]
    DepartingNodeMismatch {
        rank: usize,
        expected: String,
        actual: String,
    },
    #[error("topology-revisable data feed replace-rank request is missing replacement_node_id")]
    MissingReplacementNodeId,
    #[error(
        "topology-revisable data feed replace-rank request is missing replacement_provider_id"
    )]
    MissingReplacementProviderId,
    #[error(
        "topology-revisable data feed refuses topology action `{action_kind}` in the current scope"
    )]
    UnsupportedRevisionAction { action_kind: String },
}

/// Status for one topology-revisable dense data-feed case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyRevisableDistributedDataFeedCapabilityStatus {
    Supported,
    Refused,
}

/// One machine-readable topology-revisable dense data-feed case result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyRevisableDistributedDataFeedCapabilityCaseResult {
    pub case_id: String,
    pub action_kind: TopologyRevisableDataFeedRevisionActionKind,
    pub reason_kind: TopologyRevisableDataFeedRevisionReasonKind,
    pub status: TopologyRevisableDistributedDataFeedCapabilityStatus,
    pub departing_rank: usize,
    pub departing_node_id: String,
    pub requested_world_size: usize,
    pub reassigned_shard_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_global_order_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_global_order_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay_continuity_digest: Option<String>,
    pub bounded_scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// Machine-readable report for topology-revisable distributed data-feed semantics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyRevisableDistributedDataFeedSemanticsReport {
    pub schema_version: u32,
    pub current_scope_window: String,
    pub cases: Vec<TopologyRevisableDistributedDataFeedCapabilityCaseResult>,
    pub report_digest: String,
}

impl TopologyRevisableDistributedDataFeedSemanticsReport {
    fn new(
        current_scope_window: impl Into<String>,
        cases: Vec<TopologyRevisableDistributedDataFeedCapabilityCaseResult>,
    ) -> Self {
        let current_scope_window = current_scope_window.into();
        let report_digest = digest_lines(
            std::iter::once(format!("current_scope_window={current_scope_window}"))
                .chain(cases.iter().flat_map(|case| {
                    let mut lines = vec![
                        format!("case_id={}", case.case_id),
                        format!(
                            "action_kind={}",
                            topology_revision_action_kind_label(case.action_kind)
                        ),
                        format!(
                            "reason_kind={}",
                            topology_revision_reason_kind_label(case.reason_kind)
                        ),
                        format!("status={:?}", case.status),
                        format!("departing_rank={}", case.departing_rank),
                        format!("departing_node_id={}", case.departing_node_id),
                        format!("requested_world_size={}", case.requested_world_size),
                        format!(
                            "reassigned_shard_keys={}",
                            case.reassigned_shard_keys.join(",")
                        ),
                        format!("bounded_scope={}", case.bounded_scope),
                    ];
                    if let Some(digest) = &case.baseline_global_order_digest {
                        lines.push(format!("baseline_global_order_digest={digest}"));
                    }
                    if let Some(digest) = &case.revised_global_order_digest {
                        lines.push(format!("revised_global_order_digest={digest}"));
                    }
                    if let Some(digest) = &case.replay_continuity_digest {
                        lines.push(format!("replay_continuity_digest={digest}"));
                    }
                    if let Some(refusal) = &case.refusal {
                        lines.push(format!("refusal={refusal}"));
                    }
                    lines
                }))
                .collect(),
        );
        Self {
            schema_version: 1,
            current_scope_window,
            cases,
            report_digest,
        }
    }
}

/// Plans one admitted topology revision over the distributed data feed.
pub fn plan_topology_revisable_distributed_data_feed_revision(
    manifest: &DatasetManifest,
    contract_template: &DistributedDataFeedContract,
    members: &[TopologyRevisableDistributedWorkerMember],
    revision_request: &TopologyRevisableDataFeedRevisionRequest,
) -> Result<TopologyRevisableDistributedDataFeedReceipt, TopologyRevisableDistributedDataFeedError>
{
    validate_members(members)?;
    let current_world_size = members.len();
    if revision_request.requested_world_size != current_world_size {
        return Err(
            TopologyRevisableDistributedDataFeedError::UnsupportedWorldSizeChange {
                current_world_size,
                requested_world_size: revision_request.requested_world_size,
            },
        );
    }
    let departing = members
        .iter()
        .find(|member| member.rank == revision_request.departing_rank)
        .ok_or(
            TopologyRevisableDistributedDataFeedError::UnknownDepartingRank {
                rank: revision_request.departing_rank,
            },
        )?;
    if departing.node_id != revision_request.departing_node_id {
        return Err(
            TopologyRevisableDistributedDataFeedError::DepartingNodeMismatch {
                rank: departing.rank,
                expected: departing.node_id.clone(),
                actual: revision_request.departing_node_id.clone(),
            },
        );
    }
    let baseline_rank_plans =
        plan_topology_revisable_rank_plans(manifest, contract_template, members)?;
    let baseline_global_order_digest = baseline_rank_plans
        .first()
        .map(|plan| plan.plan.global_order_digest.clone())
        .unwrap_or_default();

    let revised_members = revised_members_for_request(members, revision_request)?;
    let revised_rank_plans = plan_topology_revisable_rank_plans(
        manifest,
        contract_template,
        revised_members.as_slice(),
    )?;
    let revised_global_order_digest = revised_rank_plans
        .first()
        .map(|plan| plan.plan.global_order_digest.clone())
        .unwrap_or_default();

    let departing_baseline = baseline_rank_plans
        .iter()
        .find(|plan| plan.member.rank == revision_request.departing_rank)
        .ok_or(
            TopologyRevisableDistributedDataFeedError::UnknownDepartingRank {
                rank: revision_request.departing_rank,
            },
        )?;
    let replacement_plan = revised_rank_plans
        .iter()
        .find(|plan| plan.member.rank == revision_request.departing_rank)
        .ok_or(
            TopologyRevisableDistributedDataFeedError::UnknownDepartingRank {
                rank: revision_request.departing_rank,
            },
        )?;
    let shard_reassignments = departing_baseline
        .plan
        .assigned_shards
        .iter()
        .zip(replacement_plan.plan.assigned_shards.iter())
        .map(
            |(previous, _next)| TopologyRevisableShardOwnershipReassignment {
                shard_key: previous.shard_key.clone(),
                manifest: previous.manifest.clone(),
                global_shard_index: previous.global_shard_index,
                previous_rank: departing_baseline.member.rank,
                next_rank: replacement_plan.member.rank,
                previous_node_id: departing_baseline.member.node_id.clone(),
                next_node_id: replacement_plan.member.node_id.clone(),
                reason_kind: revision_request.reason_kind,
            },
        )
        .collect::<Vec<_>>();
    let replay_continuity_digest = stable_topology_revisable_replay_continuity_digest(
        &baseline_rank_plans,
        &revised_rank_plans,
        &shard_reassignments,
        revision_request,
    );
    let bounded_scope = String::from(
        "Current scope admits same-world-size rank replacement only. Global shard order stays fixed, replacement nodes inherit the departed rank's replay plan, and grow/shrink or removal without replacement still refuse explicitly.",
    );
    let mut receipt = TopologyRevisableDistributedDataFeedReceipt {
        schema_version: 1,
        contract_digest: contract_template.stable_digest(),
        revision_request: revision_request.clone(),
        baseline_global_order_digest,
        revised_global_order_digest,
        baseline_rank_plans,
        revised_rank_plans,
        shard_reassignments,
        replay_continuity_digest,
        bounded_scope,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    Ok(receipt)
}

/// Builds the canonical topology-revisable distributed data-feed semantics report.
#[must_use]
pub fn builtin_topology_revisable_distributed_data_feed_semantics_report(
) -> TopologyRevisableDistributedDataFeedSemanticsReport {
    let manifest = topology_revisable_training_manifest();
    let contract = topology_revisable_dense_contract(&manifest.key);
    let members = canonical_topology_revisable_members();
    TopologyRevisableDistributedDataFeedSemanticsReport::new(
        String::from("psionic_topology_revisable_distributed_data_feed_v1"),
        vec![
            supported_topology_revisable_case(
                "dense_rank.provider_loss.replace_rank2",
                &manifest,
                &contract,
                members.as_slice(),
                TopologyRevisableDataFeedRevisionRequest {
                    run_id: String::from("xtrain-topology-revision-run"),
                    previous_topology_revision_id: String::from(
                        "xtrain-topology-revision-run-topology-1",
                    ),
                    topology_revision_id: String::from("xtrain-topology-revision-run-topology-2"),
                    action_kind: TopologyRevisableDataFeedRevisionActionKind::ReplaceRank,
                    reason_kind: TopologyRevisableDataFeedRevisionReasonKind::ProviderLoss,
                    departing_rank: 2,
                    departing_node_id: String::from("google-l4-rank2"),
                    requested_world_size: 4,
                    replacement_node_id: Some(String::from("runpod-h100-rank2")),
                    replacement_provider_id: Some(String::from("runpod")),
                },
                "Current scope supports replacing one departed dense rank with one new node at the same rank while preserving the replay-safe global shard order.",
            ),
            supported_topology_revisable_case(
                "dense_rank.node_loss.replace_rank1",
                &manifest,
                &contract,
                members.as_slice(),
                TopologyRevisableDataFeedRevisionRequest {
                    run_id: String::from("xtrain-topology-revision-run"),
                    previous_topology_revision_id: String::from(
                        "xtrain-topology-revision-run-topology-2",
                    ),
                    topology_revision_id: String::from("xtrain-topology-revision-run-topology-3"),
                    action_kind: TopologyRevisableDataFeedRevisionActionKind::ReplaceRank,
                    reason_kind: TopologyRevisableDataFeedRevisionReasonKind::NodeLoss,
                    departing_rank: 1,
                    departing_node_id: String::from("google-l4-rank1"),
                    requested_world_size: 4,
                    replacement_node_id: Some(String::from("google-l4-rank1b")),
                    replacement_provider_id: Some(String::from("google")),
                },
                "Current scope supports same-provider node replacement at the same dense rank without changing replay order or world size.",
            ),
            refused_topology_revisable_case(
                "dense_rank.shrink_world.refused",
                &manifest,
                &contract,
                members.as_slice(),
                TopologyRevisableDataFeedRevisionRequest {
                    run_id: String::from("xtrain-topology-revision-run"),
                    previous_topology_revision_id: String::from(
                        "xtrain-topology-revision-run-topology-3",
                    ),
                    topology_revision_id: String::from("xtrain-topology-revision-run-topology-4"),
                    action_kind: TopologyRevisableDataFeedRevisionActionKind::ShrinkWorld,
                    reason_kind: TopologyRevisableDataFeedRevisionReasonKind::ProviderLoss,
                    departing_rank: 3,
                    departing_node_id: String::from("runpod-h100-rank3"),
                    requested_world_size: 3,
                    replacement_node_id: None,
                    replacement_provider_id: None,
                },
                "Current scope still refuses shrink-world revisions because the existing dense data-feed substrate does not yet admit world-size changes.",
            ),
        ],
    )
}

/// Writes the canonical topology-revisable distributed data-feed report fixture.
pub fn write_topology_revisable_distributed_data_feed_semantics_report(
    output_path: impl AsRef<Path>,
) -> Result<(), std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(
        &builtin_topology_revisable_distributed_data_feed_semantics_report(),
    )
    .expect("topology-revisable distributed data-feed report should serialize");
    fs::write(output_path, bytes)
}

fn validate_members(
    members: &[TopologyRevisableDistributedWorkerMember],
) -> Result<(), TopologyRevisableDistributedDataFeedError> {
    if members.is_empty() {
        return Err(TopologyRevisableDistributedDataFeedError::EmptyWorkerSet);
    }
    let mut ranks = BTreeSet::new();
    let mut node_ids = BTreeSet::new();
    for member in members {
        if !ranks.insert(member.rank) {
            return Err(TopologyRevisableDistributedDataFeedError::DuplicateRank {
                rank: member.rank,
            });
        }
        if !node_ids.insert(member.node_id.clone()) {
            return Err(TopologyRevisableDistributedDataFeedError::DuplicateNodeId {
                node_id: member.node_id.clone(),
            });
        }
    }
    for rank in 0..members.len() {
        if !ranks.contains(&rank) {
            return Err(TopologyRevisableDistributedDataFeedError::MissingRank { rank });
        }
    }
    Ok(())
}

fn revised_members_for_request(
    members: &[TopologyRevisableDistributedWorkerMember],
    revision_request: &TopologyRevisableDataFeedRevisionRequest,
) -> Result<Vec<TopologyRevisableDistributedWorkerMember>, TopologyRevisableDistributedDataFeedError>
{
    match revision_request.action_kind {
        TopologyRevisableDataFeedRevisionActionKind::ReplaceRank => {
            let replacement_node_id = revision_request
                .replacement_node_id
                .clone()
                .ok_or(TopologyRevisableDistributedDataFeedError::MissingReplacementNodeId)?;
            let replacement_provider_id = revision_request
                .replacement_provider_id
                .clone()
                .ok_or(TopologyRevisableDistributedDataFeedError::MissingReplacementProviderId)?;
            Ok(members
                .iter()
                .map(|member| {
                    if member.rank == revision_request.departing_rank {
                        TopologyRevisableDistributedWorkerMember {
                            node_id: replacement_node_id.clone(),
                            provider_id: replacement_provider_id.clone(),
                            rank: member.rank,
                        }
                    } else {
                        member.clone()
                    }
                })
                .collect())
        }
        other => Err(
            TopologyRevisableDistributedDataFeedError::UnsupportedRevisionAction {
                action_kind: String::from(topology_revision_action_kind_label(other)),
            },
        ),
    }
}

fn plan_topology_revisable_rank_plans(
    manifest: &DatasetManifest,
    contract_template: &DistributedDataFeedContract,
    members: &[TopologyRevisableDistributedWorkerMember],
) -> Result<Vec<TopologyRevisableDistributedWorkerPlan>, TopologyRevisableDistributedDataFeedError>
{
    let mut plans = members
        .iter()
        .cloned()
        .map(|member| {
            let mut contract = contract_template.clone();
            contract.coordination.rank = member.rank;
            contract.coordination.world_size = members.len();
            let plan = contract.plan_worker_input_order(manifest)?;
            Ok(TopologyRevisableDistributedWorkerPlan { member, plan })
        })
        .collect::<Result<Vec<_>, DistributedDataFeedContractError>>()?;
    plans.sort_by(|left, right| left.member.rank.cmp(&right.member.rank));
    Ok(plans)
}

fn supported_topology_revisable_case(
    case_id: &str,
    manifest: &DatasetManifest,
    contract: &DistributedDataFeedContract,
    members: &[TopologyRevisableDistributedWorkerMember],
    revision_request: TopologyRevisableDataFeedRevisionRequest,
    bounded_scope: &str,
) -> TopologyRevisableDistributedDataFeedCapabilityCaseResult {
    let receipt = plan_topology_revisable_distributed_data_feed_revision(
        manifest,
        contract,
        members,
        &revision_request,
    )
    .expect("topology-revisable dense data-feed case should validate");
    TopologyRevisableDistributedDataFeedCapabilityCaseResult {
        case_id: String::from(case_id),
        action_kind: revision_request.action_kind,
        reason_kind: revision_request.reason_kind,
        status: TopologyRevisableDistributedDataFeedCapabilityStatus::Supported,
        departing_rank: revision_request.departing_rank,
        departing_node_id: revision_request.departing_node_id,
        requested_world_size: revision_request.requested_world_size,
        reassigned_shard_keys: receipt
            .shard_reassignments
            .iter()
            .map(|assignment| assignment.shard_key.clone())
            .collect(),
        baseline_global_order_digest: Some(receipt.baseline_global_order_digest),
        revised_global_order_digest: Some(receipt.revised_global_order_digest),
        replay_continuity_digest: Some(receipt.replay_continuity_digest),
        bounded_scope: String::from(bounded_scope),
        refusal: None,
    }
}

fn refused_topology_revisable_case(
    case_id: &str,
    manifest: &DatasetManifest,
    contract: &DistributedDataFeedContract,
    members: &[TopologyRevisableDistributedWorkerMember],
    revision_request: TopologyRevisableDataFeedRevisionRequest,
    bounded_scope: &str,
) -> TopologyRevisableDistributedDataFeedCapabilityCaseResult {
    let refusal = plan_topology_revisable_distributed_data_feed_revision(
        manifest,
        contract,
        members,
        &revision_request,
    )
    .expect_err("topology-revisable dense data-feed case should refuse");
    TopologyRevisableDistributedDataFeedCapabilityCaseResult {
        case_id: String::from(case_id),
        action_kind: revision_request.action_kind,
        reason_kind: revision_request.reason_kind,
        status: TopologyRevisableDistributedDataFeedCapabilityStatus::Refused,
        departing_rank: revision_request.departing_rank,
        departing_node_id: revision_request.departing_node_id,
        requested_world_size: revision_request.requested_world_size,
        reassigned_shard_keys: Vec::new(),
        baseline_global_order_digest: None,
        revised_global_order_digest: None,
        replay_continuity_digest: None,
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal.to_string()),
    }
}

fn topology_revisable_training_manifest() -> DatasetManifest {
    let dataset = DatasetKey::new("psion/cross_provider_pretrain", "2026.03.25");
    let train_split = DatasetSplitDeclaration::new(
        &dataset,
        "train",
        DatasetSplitKind::Train,
        (0..8)
            .map(|index| {
                topology_revisable_shard(
                    &dataset,
                    "train",
                    format!("shard-{index}"),
                    4 + index as u64,
                    32 + (index as u64 * 8),
                )
            })
            .collect(),
    )
    .expect("topology-revisable train split should validate");
    DatasetManifest::new(
        dataset,
        "Cross-provider dense pretraining",
        DatasetRecordEncoding::TokenIdsLeU32,
        TokenizerDigest::new(
            TokenizerFamily::SentencePiece,
            "topology-revisable-tokenizer-digest",
            65_536,
        ),
    )
    .with_context_window_tokens(8192)
    .with_splits(vec![train_split])
}

fn topology_revisable_shard(
    dataset: &DatasetKey,
    split: &str,
    shard_key: impl Into<String>,
    sequence_count: u64,
    token_count: u64,
) -> DatasetShardManifest {
    let shard_key = shard_key.into();
    let payload = vec![1_u8; 32];
    let manifest = DatastreamManifest::from_bytes(
        format!("{split}:{shard_key}"),
        DatastreamSubjectKind::TokenizedCorpus,
        payload.as_slice(),
        16,
        DatastreamEncoding::RawBinary,
    )
    .with_dataset_binding(dataset.datastream_binding(split, shard_key.as_str()));
    DatasetShardManifest::new(
        dataset,
        split,
        shard_key.as_str(),
        manifest.manifest_ref(),
        sequence_count,
        token_count,
        4,
        12,
    )
    .expect("topology-revisable shard should validate")
}

fn topology_revisable_dense_contract(dataset: &DatasetKey) -> DistributedDataFeedContract {
    DistributedDataFeedContract::new(
        DataIngressContract::new(
            DatasetSourceContract::new(
                DatasetSourceKind::IterableStreaming,
                DatasetIterationContract::new(dataset.clone(), "train")
                    .with_mode(DatasetIterationMode::Repeat)
                    .with_shard_ordering(DatasetShardOrdering::DeterministicShuffle)
                    .with_shuffle_seed(13),
            ),
            DatasetBatchSamplerContract::new(
                DatasetSamplerContract::new(DatasetSamplerKind::DeterministicShuffle).with_seed(13),
                8,
                128,
                DatasetPackingPolicy::new(DatasetPackingMode::PackIntoContextWindow, 32, 128, 2),
            ),
            HostDeviceStagingContract::new(HostDeviceStagingMode::PinnedPrefetch, "cuda:0")
                .with_prefetch_batch_count(2)
                .with_pin_host_memory(true),
        ),
        DistributedSamplerPartitionContract::new(DistributedSamplerPartitionKind::StridedShards),
        DistributedWorkerCoordinationContract::new(
            "cross_provider_dense",
            0,
            4,
            DistributedWorkerCoordinationMode::StepBarrier,
        )
        .with_sync_interval_batches(2),
        DistributedReplayOrderingContract::new(RuntimeDeterminismContract::strict(401), 2),
    )
}

fn canonical_topology_revisable_members() -> Vec<TopologyRevisableDistributedWorkerMember> {
    vec![
        TopologyRevisableDistributedWorkerMember {
            node_id: String::from("google-l4-rank0"),
            provider_id: String::from("google"),
            rank: 0,
        },
        TopologyRevisableDistributedWorkerMember {
            node_id: String::from("google-l4-rank1"),
            provider_id: String::from("google"),
            rank: 1,
        },
        TopologyRevisableDistributedWorkerMember {
            node_id: String::from("google-l4-rank2"),
            provider_id: String::from("google"),
            rank: 2,
        },
        TopologyRevisableDistributedWorkerMember {
            node_id: String::from("runpod-h100-rank3"),
            provider_id: String::from("runpod"),
            rank: 3,
        },
    ]
}

fn stable_topology_revisable_replay_continuity_digest(
    baseline_rank_plans: &[TopologyRevisableDistributedWorkerPlan],
    revised_rank_plans: &[TopologyRevisableDistributedWorkerPlan],
    shard_reassignments: &[TopologyRevisableShardOwnershipReassignment],
    revision_request: &TopologyRevisableDataFeedRevisionRequest,
) -> String {
    let baseline_by_rank = baseline_rank_plans
        .iter()
        .map(|plan| (plan.member.rank, plan))
        .collect::<BTreeMap<_, _>>();
    let revised_by_rank = revised_rank_plans
        .iter()
        .map(|plan| (plan.member.rank, plan))
        .collect::<BTreeMap<_, _>>();
    digest_lines(
        std::iter::once(format!(
            "revision_request={}",
            revision_request.stable_digest()
        ))
        .chain(baseline_by_rank.keys().flat_map(|rank| {
            let baseline = baseline_by_rank
                .get(rank)
                .expect("baseline rank should exist");
            let revised = revised_by_rank
                .get(rank)
                .expect("revised rank should exist");
            [
                format!("rank={rank}"),
                format!(
                    "baseline_global_order_digest={}",
                    baseline.plan.global_order_digest
                ),
                format!(
                    "revised_global_order_digest={}",
                    revised.plan.global_order_digest
                ),
                format!("baseline_plan_id={}", baseline.plan.plan_id),
                format!("revised_plan_id={}", revised.plan.plan_id),
            ]
        }))
        .chain(shard_reassignments.iter().flat_map(|assignment| {
            [
                format!("reassigned_shard_key={}", assignment.shard_key),
                format!("previous_rank={}", assignment.previous_rank),
                format!("next_rank={}", assignment.next_rank),
                format!("previous_node_id={}", assignment.previous_node_id),
                format!("next_node_id={}", assignment.next_node_id),
            ]
        }))
        .collect(),
    )
}

fn topology_revision_action_kind_label(
    action_kind: TopologyRevisableDataFeedRevisionActionKind,
) -> &'static str {
    match action_kind {
        TopologyRevisableDataFeedRevisionActionKind::ReplaceRank => "replace_rank",
        TopologyRevisableDataFeedRevisionActionKind::RemoveRank => "remove_rank",
        TopologyRevisableDataFeedRevisionActionKind::GrowWorld => "grow_world",
        TopologyRevisableDataFeedRevisionActionKind::ShrinkWorld => "shrink_world",
    }
}

fn topology_revision_reason_kind_label(
    reason_kind: TopologyRevisableDataFeedRevisionReasonKind,
) -> &'static str {
    match reason_kind {
        TopologyRevisableDataFeedRevisionReasonKind::NodeLoss => "node_loss",
        TopologyRevisableDataFeedRevisionReasonKind::ProviderLoss => "provider_loss",
        TopologyRevisableDataFeedRevisionReasonKind::PlannedReplacement => "planned_replacement",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        builtin_topology_revisable_distributed_data_feed_semantics_report,
        canonical_topology_revisable_members,
        plan_topology_revisable_distributed_data_feed_revision, topology_revisable_dense_contract,
        topology_revisable_training_manifest, TopologyRevisableDataFeedRevisionActionKind,
        TopologyRevisableDataFeedRevisionReasonKind, TopologyRevisableDataFeedRevisionRequest,
    };

    #[test]
    fn rank_replacement_preserves_global_order_and_reassigns_departed_rank_shards() {
        let manifest = topology_revisable_training_manifest();
        let contract = topology_revisable_dense_contract(&manifest.key);
        let members = canonical_topology_revisable_members();
        let receipt = plan_topology_revisable_distributed_data_feed_revision(
            &manifest,
            &contract,
            members.as_slice(),
            &TopologyRevisableDataFeedRevisionRequest {
                run_id: String::from("xtrain-topology-revision-run"),
                previous_topology_revision_id: String::from(
                    "xtrain-topology-revision-run-topology-1",
                ),
                topology_revision_id: String::from("xtrain-topology-revision-run-topology-2"),
                action_kind: TopologyRevisableDataFeedRevisionActionKind::ReplaceRank,
                reason_kind: TopologyRevisableDataFeedRevisionReasonKind::ProviderLoss,
                departing_rank: 2,
                departing_node_id: String::from("google-l4-rank2"),
                requested_world_size: 4,
                replacement_node_id: Some(String::from("runpod-h100-rank2")),
                replacement_provider_id: Some(String::from("runpod")),
            },
        )
        .expect("rank replacement should validate");
        assert_eq!(
            receipt.baseline_global_order_digest,
            receipt.revised_global_order_digest
        );
        assert!(!receipt.shard_reassignments.is_empty());
        assert!(receipt
            .shard_reassignments
            .iter()
            .all(|assignment| assignment.previous_rank == 2 && assignment.next_rank == 2));
    }

    #[test]
    fn shrink_world_refuses_deterministically() {
        let manifest = topology_revisable_training_manifest();
        let contract = topology_revisable_dense_contract(&manifest.key);
        let members = canonical_topology_revisable_members();
        let error = plan_topology_revisable_distributed_data_feed_revision(
            &manifest,
            &contract,
            members.as_slice(),
            &TopologyRevisableDataFeedRevisionRequest {
                run_id: String::from("xtrain-topology-revision-run"),
                previous_topology_revision_id: String::from(
                    "xtrain-topology-revision-run-topology-3",
                ),
                topology_revision_id: String::from("xtrain-topology-revision-run-topology-4"),
                action_kind: TopologyRevisableDataFeedRevisionActionKind::ShrinkWorld,
                reason_kind: TopologyRevisableDataFeedRevisionReasonKind::ProviderLoss,
                departing_rank: 3,
                departing_node_id: String::from("runpod-h100-rank3"),
                requested_world_size: 3,
                replacement_node_id: None,
                replacement_provider_id: None,
            },
        )
        .expect_err("shrink-world request should refuse");
        assert!(error.to_string().contains("requested_world_size"));
    }

    #[test]
    fn topology_revisable_report_tracks_supported_and_refused_cases() {
        let report = builtin_topology_revisable_distributed_data_feed_semantics_report();
        assert_eq!(
            report.current_scope_window,
            "psionic_topology_revisable_distributed_data_feed_v1"
        );
        assert_eq!(report.cases.len(), 3);
        assert!(report.cases.iter().any(|case| case.case_id
            == "dense_rank.provider_loss.replace_rank2"
            && case.refusal.is_none()));
        assert!(report.cases.iter().any(
            |case| case.case_id == "dense_rank.shrink_world.refused" && case.refusal.is_some()
        ));
    }
}
