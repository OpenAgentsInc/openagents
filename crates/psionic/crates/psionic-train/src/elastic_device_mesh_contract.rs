use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_decentralized_network_contract, canonical_dense_rank_recovery_contract,
    canonical_dense_topology_revision_contract, canonical_public_network_registry_contract,
    DecentralizedNetworkContractError, DecentralizedNetworkRoleClass,
    DenseRankRecoveryContractError, DenseTopologyRevisionContractError,
    PublicNetworkRegistryContractError, PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID,
};

pub const ELASTIC_DEVICE_MESH_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.elastic_device_mesh_contract.v1";
pub const ELASTIC_DEVICE_MESH_CONTRACT_ID: &str = "psionic.elastic_device_mesh_contract.v1";
pub const ELASTIC_DEVICE_MESH_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/elastic_device_mesh_contract_v1.json";
pub const ELASTIC_DEVICE_MESH_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-elastic-device-mesh-contract.sh";
pub const ELASTIC_DEVICE_MESH_CONTRACT_DOC_PATH: &str = "docs/ELASTIC_DEVICE_MESH_REFERENCE.md";
pub const ELASTIC_DEVICE_MESH_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum ElasticDeviceMeshContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    NetworkContract(#[from] DecentralizedNetworkContractError),
    #[error(transparent)]
    PublicRegistry(#[from] PublicNetworkRegistryContractError),
    #[error(transparent)]
    DenseTopology(#[from] DenseTopologyRevisionContractError),
    #[error(transparent)]
    DenseRecovery(#[from] DenseRankRecoveryContractError),
    #[error("elastic device mesh contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElasticMeshAssignmentState {
    Active,
    Standby,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElasticMeshLeaseStatus {
    Active,
    GracefulDepartureRequested,
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElasticMeshHeartbeatStatus {
    Fresh,
    Stale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElasticMeshRevisionTriggerKind {
    AdmissionGrant,
    DeathrattleNotice,
    BarrierRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElasticMeshRevisionKind {
    ActivateLeaseSet,
    PromoteStandbyReplacement,
    RefuseDenseWorldChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElasticMeshRevisionOutcome {
    Applied,
    Held,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticMeshRoleLeasePolicy {
    pub role_class: DecentralizedNetworkRoleClass,
    pub lease_duration_seconds: u16,
    pub heartbeat_interval_seconds: u16,
    pub max_missed_heartbeats: u16,
    pub deathrattle_grace_seconds: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticMeshMemberLease {
    pub lease_id: String,
    pub registry_record_id: String,
    pub role_class: DecentralizedNetworkRoleClass,
    pub assignment_state: ElasticMeshAssignmentState,
    pub status: ElasticMeshLeaseStatus,
    pub current_epoch_id: String,
    pub lease_duration_seconds: u16,
    pub heartbeat_interval_seconds: u16,
    pub max_missed_heartbeats: u16,
    pub issued_at_unix_ms: u64,
    pub last_heartbeat_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub source_selection_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticMeshHeartbeatSample {
    pub heartbeat_id: String,
    pub lease_id: String,
    pub observed_at_unix_ms: u64,
    pub round_trip_latency_ms: u16,
    pub missed_heartbeats: u16,
    pub status: ElasticMeshHeartbeatStatus,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticMeshDeathrattleNotice {
    pub deathrattle_id: String,
    pub lease_id: String,
    pub registry_record_id: String,
    pub requested_effective_unix_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_registry_record_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticMeshRevisionReceipt {
    pub revision_id: String,
    pub revision_kind: ElasticMeshRevisionKind,
    pub trigger_kind: ElasticMeshRevisionTriggerKind,
    pub outcome: ElasticMeshRevisionOutcome,
    pub affected_registry_record_ids: Vec<String>,
    pub source_selection_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_registry_record_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referenced_topology_revision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referenced_recovery_scenario_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticDeviceMeshAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticDeviceMeshContract {
    pub schema_version: String,
    pub contract_id: String,
    pub network_id: String,
    pub governance_revision_id: String,
    pub current_epoch_id: String,
    pub decentralized_network_contract_digest: String,
    pub public_network_registry_contract_digest: String,
    pub dense_topology_revision_contract_digest: String,
    pub dense_rank_recovery_contract_digest: String,
    pub role_lease_policies: Vec<ElasticMeshRoleLeasePolicy>,
    pub member_leases: Vec<ElasticMeshMemberLease>,
    pub heartbeat_samples: Vec<ElasticMeshHeartbeatSample>,
    pub deathrattles: Vec<ElasticMeshDeathrattleNotice>,
    pub revision_receipts: Vec<ElasticMeshRevisionReceipt>,
    pub authority_paths: ElasticDeviceMeshAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl ElasticDeviceMeshContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_elastic_device_mesh_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), ElasticDeviceMeshContractError> {
        let network = canonical_decentralized_network_contract()?;
        let registry = canonical_public_network_registry_contract()?;
        let topology = canonical_dense_topology_revision_contract()?;
        let recovery = canonical_dense_rank_recovery_contract()?;

        let record_by_id = registry
            .registry_records
            .iter()
            .map(|record| (record.registry_record_id.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let policy_by_role = self
            .role_lease_policies
            .iter()
            .map(|policy| (policy.role_class, policy))
            .collect::<BTreeMap<_, _>>();
        let lease_by_id = self
            .member_leases
            .iter()
            .map(|lease| (lease.lease_id.as_str(), lease))
            .collect::<BTreeMap<_, _>>();
        let revision_by_id = self
            .revision_receipts
            .iter()
            .map(|revision| (revision.revision_id.as_str(), revision))
            .collect::<BTreeMap<_, _>>();
        let offer_ids = registry
            .matchmaking_offers
            .iter()
            .map(|offer| offer.offer_id.as_str())
            .collect::<BTreeSet<_>>();
        let query_ids = registry
            .discovery_examples
            .iter()
            .map(|query| query.query_id.as_str())
            .collect::<BTreeSet<_>>();

        if self.schema_version != ELASTIC_DEVICE_MESH_CONTRACT_SCHEMA_VERSION {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    ELASTIC_DEVICE_MESH_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != ELASTIC_DEVICE_MESH_CONTRACT_ID {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.network_id != network.network_id {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("network_id drifted"),
            });
        }
        if self.governance_revision_id != network.governance_revision.governance_revision_id {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("governance_revision_id drifted"),
            });
        }
        if self.current_epoch_id != PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("current_epoch_id drifted"),
            });
        }
        if self.decentralized_network_contract_digest != network.contract_digest {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("decentralized network digest drifted"),
            });
        }
        if self.public_network_registry_contract_digest != registry.contract_digest {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("public network registry digest drifted"),
            });
        }
        if self.dense_topology_revision_contract_digest != topology.contract_digest {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("dense topology revision digest drifted"),
            });
        }
        if self.dense_rank_recovery_contract_digest != recovery.contract_digest {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("dense rank recovery digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != ELASTIC_DEVICE_MESH_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != ELASTIC_DEVICE_MESH_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != ELASTIC_DEVICE_MESH_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != ELASTIC_DEVICE_MESH_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        let expected_policy_roles = BTreeSet::from([
            DecentralizedNetworkRoleClass::PublicMiner,
            DecentralizedNetworkRoleClass::PublicValidator,
            DecentralizedNetworkRoleClass::Relay,
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            DecentralizedNetworkRoleClass::Aggregator,
        ]);
        let actual_policy_roles = self
            .role_lease_policies
            .iter()
            .map(|policy| policy.role_class)
            .collect::<BTreeSet<_>>();
        if actual_policy_roles != expected_policy_roles {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("role lease policy set drifted"),
            });
        }

        for policy in &self.role_lease_policies {
            if policy.heartbeat_interval_seconds != network.epoch_cadence.heartbeat_interval_seconds
            {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "role lease policy `{:?}` heartbeat interval drifted from decentralized network cadence",
                        policy.role_class
                    ),
                });
            }
            if policy.lease_duration_seconds <= network.epoch_cadence.stale_peer_timeout_seconds {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "role lease policy `{:?}` lease duration must stay above stale peer timeout",
                        policy.role_class
                    ),
                });
            }
            if policy.max_missed_heartbeats == 0 || policy.deathrattle_grace_seconds == 0 {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "role lease policy `{:?}` lost non-zero mesh timing fields",
                        policy.role_class
                    ),
                });
            }
        }

        let mut lease_ids = BTreeSet::new();
        for lease in &self.member_leases {
            if !lease_ids.insert(lease.lease_id.clone()) {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!("duplicate lease_id `{}`", lease.lease_id),
                });
            }
            let record = record_by_id
                .get(lease.registry_record_id.as_str())
                .ok_or_else(|| ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "lease `{}` references unknown registry record `{}`",
                        lease.lease_id, lease.registry_record_id
                    ),
                })?;
            if !record.role_classes.contains(&lease.role_class) {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "lease `{}` references role `{:?}` outside registry record `{}`",
                        lease.lease_id, lease.role_class, lease.registry_record_id
                    ),
                });
            }
            let policy = policy_by_role.get(&lease.role_class).ok_or_else(|| {
                ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "lease `{}` references missing role policy `{:?}`",
                        lease.lease_id, lease.role_class
                    ),
                }
            })?;
            if lease.current_epoch_id != self.current_epoch_id
                || lease.lease_duration_seconds != policy.lease_duration_seconds
                || lease.heartbeat_interval_seconds != policy.heartbeat_interval_seconds
                || lease.max_missed_heartbeats != policy.max_missed_heartbeats
                || lease.expires_at_unix_ms <= lease.last_heartbeat_unix_ms
            {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!("lease `{}` timing or policy drifted", lease.lease_id),
                });
            }
            if !offer_ids.contains(lease.source_selection_id.as_str())
                && !query_ids.contains(lease.source_selection_id.as_str())
                && !revision_by_id.contains_key(lease.source_selection_id.as_str())
            {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "lease `{}` source_selection_id `{}` no longer resolves to a known offer, discovery query, or revision",
                        lease.lease_id, lease.source_selection_id
                    ),
                });
            }
            if lease.status == ElasticMeshLeaseStatus::GracefulDepartureRequested
                && !self
                    .deathrattles
                    .iter()
                    .any(|notice| notice.lease_id == lease.lease_id)
            {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "lease `{}` entered graceful_departure_requested without a deathrattle",
                        lease.lease_id
                    ),
                });
            }
        }

        let mut heartbeat_ids = BTreeSet::new();
        for heartbeat in &self.heartbeat_samples {
            if !heartbeat_ids.insert(heartbeat.heartbeat_id.clone()) {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!("duplicate heartbeat_id `{}`", heartbeat.heartbeat_id),
                });
            }
            let lease = lease_by_id
                .get(heartbeat.lease_id.as_str())
                .ok_or_else(|| ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "heartbeat `{}` references unknown lease `{}`",
                        heartbeat.heartbeat_id, heartbeat.lease_id
                    ),
                })?;
            if heartbeat.observed_at_unix_ms < lease.last_heartbeat_unix_ms
                || heartbeat.round_trip_latency_ms == 0
            {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!("heartbeat `{}` timing drifted", heartbeat.heartbeat_id),
                });
            }
            match heartbeat.status {
                ElasticMeshHeartbeatStatus::Fresh => {
                    if heartbeat.missed_heartbeats >= lease.max_missed_heartbeats {
                        return Err(ElasticDeviceMeshContractError::InvalidContract {
                            detail: format!(
                                "heartbeat `{}` is marked fresh despite missed heartbeat overflow",
                                heartbeat.heartbeat_id
                            ),
                        });
                    }
                }
                ElasticMeshHeartbeatStatus::Stale => {
                    if heartbeat.missed_heartbeats < lease.max_missed_heartbeats {
                        return Err(ElasticDeviceMeshContractError::InvalidContract {
                            detail: format!(
                                "heartbeat `{}` is marked stale without missed heartbeat overflow",
                                heartbeat.heartbeat_id
                            ),
                        });
                    }
                }
            }
        }

        let mut deathrattle_ids = BTreeSet::new();
        for notice in &self.deathrattles {
            if !deathrattle_ids.insert(notice.deathrattle_id.clone()) {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!("duplicate deathrattle_id `{}`", notice.deathrattle_id),
                });
            }
            let lease = lease_by_id.get(notice.lease_id.as_str()).ok_or_else(|| {
                ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "deathrattle `{}` references unknown lease `{}`",
                        notice.deathrattle_id, notice.lease_id
                    ),
                }
            })?;
            if lease.registry_record_id != notice.registry_record_id
                || lease.status != ElasticMeshLeaseStatus::GracefulDepartureRequested
                || notice.requested_effective_unix_ms <= lease.last_heartbeat_unix_ms
            {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!(
                        "deathrattle `{}` lost its graceful departure binding",
                        notice.deathrattle_id
                    ),
                });
            }
            if let Some(replacement_registry_record_id) =
                notice.replacement_registry_record_id.as_deref()
            {
                let replacement_lease = self
                    .member_leases
                    .iter()
                    .find(|candidate| {
                        candidate.registry_record_id == replacement_registry_record_id
                            && candidate.role_class == lease.role_class
                            && candidate.status == ElasticMeshLeaseStatus::Active
                    })
                    .ok_or_else(|| ElasticDeviceMeshContractError::InvalidContract {
                        detail: format!(
                            "deathrattle `{}` replacement `{}` no longer resolves to an active lease on the same role",
                            notice.deathrattle_id, replacement_registry_record_id
                        ),
                    })?;
                if replacement_lease.registry_record_id == notice.registry_record_id {
                    return Err(ElasticDeviceMeshContractError::InvalidContract {
                        detail: format!(
                            "deathrattle `{}` replacement looped back to the departing lease",
                            notice.deathrattle_id
                        ),
                    });
                }
            }
        }

        let refused_topology_id = "dense_topology.remove_rank_without_replacement.live_refused";
        let refused_recovery_id = "dense_rank.provider_loss.rank3.shrink_world_refused";
        let topology_scenario_ids = topology
            .revisions
            .iter()
            .map(|scenario| scenario.scenario_id.as_str())
            .collect::<BTreeSet<_>>();
        let recovery_scenario_ids = recovery
            .scenarios
            .iter()
            .map(|scenario| scenario.scenario_id.as_str())
            .collect::<BTreeSet<_>>();

        let mut revision_ids = BTreeSet::new();
        for revision in &self.revision_receipts {
            if !revision_ids.insert(revision.revision_id.clone()) {
                return Err(ElasticDeviceMeshContractError::InvalidContract {
                    detail: format!("duplicate revision_id `{}`", revision.revision_id),
                });
            }
            for registry_record_id in &revision.affected_registry_record_ids {
                if !record_by_id.contains_key(registry_record_id.as_str()) {
                    return Err(ElasticDeviceMeshContractError::InvalidContract {
                        detail: format!(
                            "revision `{}` references unknown registry record `{}`",
                            revision.revision_id, registry_record_id
                        ),
                    });
                }
            }
            match revision.revision_kind {
                ElasticMeshRevisionKind::ActivateLeaseSet => {
                    if revision.trigger_kind != ElasticMeshRevisionTriggerKind::AdmissionGrant
                        || revision.outcome != ElasticMeshRevisionOutcome::Applied
                        || !offer_ids.contains(revision.source_selection_id.as_str())
                    {
                        return Err(ElasticDeviceMeshContractError::InvalidContract {
                            detail: format!(
                                "activation revision `{}` drifted from admission-grant semantics",
                                revision.revision_id
                            ),
                        });
                    }
                }
                ElasticMeshRevisionKind::PromoteStandbyReplacement => {
                    if revision.trigger_kind != ElasticMeshRevisionTriggerKind::DeathrattleNotice
                        || revision.outcome != ElasticMeshRevisionOutcome::Applied
                        || revision.replacement_registry_record_id.is_none()
                    {
                        return Err(ElasticDeviceMeshContractError::InvalidContract {
                            detail: format!(
                                "promotion revision `{}` lost deathrattle replacement semantics",
                                revision.revision_id
                            ),
                        });
                    }
                }
                ElasticMeshRevisionKind::RefuseDenseWorldChange => {
                    if revision.trigger_kind != ElasticMeshRevisionTriggerKind::BarrierRequired
                        || revision.outcome != ElasticMeshRevisionOutcome::Refused
                    {
                        return Err(ElasticDeviceMeshContractError::InvalidContract {
                            detail: format!(
                                "refusal revision `{}` drifted from barrier-required refusal semantics",
                                revision.revision_id
                            ),
                        });
                    }
                    if revision.referenced_topology_revision_id.as_deref()
                        != Some(refused_topology_id)
                        || revision.referenced_recovery_scenario_id.as_deref()
                            != Some(refused_recovery_id)
                        || !topology_scenario_ids.contains(refused_topology_id)
                        || !recovery_scenario_ids.contains(refused_recovery_id)
                    {
                        return Err(ElasticDeviceMeshContractError::InvalidContract {
                            detail: format!(
                                "refusal revision `{}` lost its refused topology or recovery binding",
                                revision.revision_id
                            ),
                        });
                    }
                }
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(ElasticDeviceMeshContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

static ELASTIC_DEVICE_MESH_CONTRACT_CACHE: std::sync::OnceLock<ElasticDeviceMeshContract> =
    std::sync::OnceLock::new();

pub fn canonical_elastic_device_mesh_contract(
) -> Result<ElasticDeviceMeshContract, ElasticDeviceMeshContractError> {
    if let Some(contract) = ELASTIC_DEVICE_MESH_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let network = canonical_decentralized_network_contract()?;
    let registry = canonical_public_network_registry_contract()?;
    let topology = canonical_dense_topology_revision_contract()?;
    let recovery = canonical_dense_rank_recovery_contract()?;
    let relay_lease_duration_seconds = network.epoch_cadence.stale_peer_timeout_seconds
        + network.epoch_cadence.heartbeat_interval_seconds;

    let role_lease_policies = vec![
        role_policy(
            DecentralizedNetworkRoleClass::PublicMiner,
            12,
            2,
            3,
            4,
            "Public miner leases stay short and heartbeat-bound so contributor-window replacement can happen quickly when a miner departs or dies on the permissioned testnet.",
        ),
        role_policy(
            DecentralizedNetworkRoleClass::PublicValidator,
            12,
            2,
            3,
            4,
            "Public validator leases match miner cadence so quorum liveness remains explicit and failure-detection timing does not drift into operator folklore.",
        ),
        role_policy(
            DecentralizedNetworkRoleClass::Relay,
            relay_lease_duration_seconds,
            2,
            2,
            4,
            "Relay leases stay just above the stale-peer cutoff so route support can fail over quickly without drifting below the network's own liveness boundary.",
        ),
        role_policy(
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            16,
            2,
            4,
            6,
            "Checkpoint-authority leases stay slightly longer than miner and validator leases to avoid flapping the durable-state lane while still making failure detection explicit.",
        ),
        role_policy(
            DecentralizedNetworkRoleClass::Aggregator,
            12,
            2,
            3,
            4,
            "Aggregator lease policy is frozen now even though the current mesh does not yet retain an active aggregator lease set in this issue.",
        ),
    ];

    let member_leases = vec![
        lease(
            "lease.public_miner.google.active",
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            ElasticMeshAssignmentState::Active,
            ElasticMeshLeaseStatus::Active,
            1_711_111_200_000,
            1_711_111_208_000,
            "public_miner_window_offer_v1",
            role_lease_policies.as_slice(),
            "Google remains one of the two active contributor-window miner leases after the mesh closes its first replacement event.",
        ),
        lease(
            "lease.public_miner.local_rtx4080.departing",
            "local_rtx4080_workstation.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            ElasticMeshAssignmentState::Active,
            ElasticMeshLeaseStatus::GracefulDepartureRequested,
            1_711_111_200_000,
            1_711_111_207_000,
            "public_miner_window_offer_v1",
            role_lease_policies.as_slice(),
            "The RTX 4080 miner lease is still retained in mesh history but has already requested graceful departure through a deathrattle notice.",
        ),
        lease(
            "lease.public_miner.local_mlx.promoted",
            "local_mlx_mac_workstation.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            ElasticMeshAssignmentState::Active,
            ElasticMeshLeaseStatus::Active,
            1_711_111_210_000,
            1_711_111_216_000,
            "promote_public_miner_standby_after_deathrattle_v1",
            role_lease_policies.as_slice(),
            "The Apple MLX miner lease is active because the mesh promoted the prior standby node after the RTX 4080 deathrattle arrived.",
        ),
        lease(
            "lease.public_validator.google.active",
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::PublicValidator,
            ElasticMeshAssignmentState::Active,
            ElasticMeshLeaseStatus::Active,
            1_711_111_200_000,
            1_711_111_208_000,
            "validator_quorum_offer_v1",
            role_lease_policies.as_slice(),
            "Google remains one active validator lease in the current two-validator quorum.",
        ),
        lease(
            "lease.public_validator.local_mlx.active",
            "local_mlx_mac_workstation.registry",
            DecentralizedNetworkRoleClass::PublicValidator,
            ElasticMeshAssignmentState::Active,
            ElasticMeshLeaseStatus::Active,
            1_711_111_200_000,
            1_711_111_209_000,
            "validator_quorum_offer_v1",
            role_lease_policies.as_slice(),
            "The Apple MLX node remains the second active validator lease in the current quorum.",
        ),
        lease(
            "lease.checkpoint_authority.google.active",
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            ElasticMeshAssignmentState::Active,
            ElasticMeshLeaseStatus::Active,
            1_711_111_200_000,
            1_711_111_210_000,
            "checkpoint_promotion_offer_v1",
            role_lease_policies.as_slice(),
            "Google remains one active checkpoint-authority lease for promoted public state.",
        ),
        lease(
            "lease.checkpoint_authority.runpod.active",
            "runpod_8xh100_dense_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            ElasticMeshAssignmentState::Active,
            ElasticMeshLeaseStatus::Active,
            1_711_111_200_000,
            1_711_111_210_000,
            "checkpoint_promotion_offer_v1",
            role_lease_policies.as_slice(),
            "RunPod remains the second active checkpoint-authority lease for promoted public state.",
        ),
        lease(
            "lease.relay.google.active",
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::Relay,
            ElasticMeshAssignmentState::Active,
            ElasticMeshLeaseStatus::Active,
            1_711_111_200_000,
            1_711_111_208_000,
            "discover_relay_nodes",
            role_lease_policies.as_slice(),
            "Google remains the single active relay lease in the current permissioned mesh.",
        ),
    ];

    let heartbeat_samples = vec![
        heartbeat("heartbeat.public_miner.google.1", "lease.public_miner.google.active", 1_711_111_208_500, 34, 0, ElasticMeshHeartbeatStatus::Fresh, "Google public miner heartbeat remains fresh."),
        heartbeat("heartbeat.public_miner.local_rtx4080.1", "lease.public_miner.local_rtx4080.departing", 1_711_111_207_500, 19, 0, ElasticMeshHeartbeatStatus::Fresh, "The departing RTX 4080 miner heartbeat stayed fresh long enough to publish a graceful deathrattle instead of vanishing silently."),
        heartbeat("heartbeat.public_miner.local_mlx.1", "lease.public_miner.local_mlx.promoted", 1_711_111_216_500, 27, 0, ElasticMeshHeartbeatStatus::Fresh, "The promoted Apple MLX miner lease heartbeats cleanly after standby replacement."),
        heartbeat("heartbeat.public_validator.google.1", "lease.public_validator.google.active", 1_711_111_208_600, 35, 0, ElasticMeshHeartbeatStatus::Fresh, "Google validator heartbeat remains fresh."),
        heartbeat("heartbeat.public_validator.local_mlx.1", "lease.public_validator.local_mlx.active", 1_711_111_209_400, 29, 0, ElasticMeshHeartbeatStatus::Fresh, "Apple MLX validator heartbeat remains fresh."),
        heartbeat("heartbeat.checkpoint_authority.google.1", "lease.checkpoint_authority.google.active", 1_711_111_210_300, 41, 0, ElasticMeshHeartbeatStatus::Fresh, "Google checkpoint-authority heartbeat remains fresh."),
        heartbeat("heartbeat.checkpoint_authority.runpod.1", "lease.checkpoint_authority.runpod.active", 1_711_111_210_400, 52, 0, ElasticMeshHeartbeatStatus::Fresh, "RunPod checkpoint-authority heartbeat remains fresh."),
        heartbeat("heartbeat.relay.google.1", "lease.relay.google.active", 1_711_111_208_550, 23, 0, ElasticMeshHeartbeatStatus::Fresh, "Google relay heartbeat remains fresh."),
    ];

    let deathrattles = vec![ElasticMeshDeathrattleNotice {
        deathrattle_id: String::from("deathrattle.public_miner.local_rtx4080.1"),
        lease_id: String::from("lease.public_miner.local_rtx4080.departing"),
        registry_record_id: String::from("local_rtx4080_workstation.registry"),
        requested_effective_unix_ms: 1_711_111_211_000,
        replacement_registry_record_id: Some(String::from("local_mlx_mac_workstation.registry")),
        detail: String::from(
            "The RTX 4080 miner published a graceful departure notice with Apple MLX named as the replacement target instead of forcing the mesh to infer failure from silence.",
        ),
    }];

    let revision_receipts = vec![
        ElasticMeshRevisionReceipt {
            revision_id: String::from("activate_public_miner_window_mesh_v1"),
            revision_kind: ElasticMeshRevisionKind::ActivateLeaseSet,
            trigger_kind: ElasticMeshRevisionTriggerKind::AdmissionGrant,
            outcome: ElasticMeshRevisionOutcome::Applied,
            affected_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("local_rtx4080_workstation.registry"),
            ],
            source_selection_id: String::from("public_miner_window_offer_v1"),
            replacement_registry_record_id: None,
            referenced_topology_revision_id: None,
            referenced_recovery_scenario_id: None,
            detail: String::from(
                "The first miner lease-set activation applied directly from the contributor-window offer: Google plus the RTX 4080 node entered the active mesh as the initial public miner pair.",
            ),
        },
        ElasticMeshRevisionReceipt {
            revision_id: String::from("activate_validator_quorum_mesh_v1"),
            revision_kind: ElasticMeshRevisionKind::ActivateLeaseSet,
            trigger_kind: ElasticMeshRevisionTriggerKind::AdmissionGrant,
            outcome: ElasticMeshRevisionOutcome::Applied,
            affected_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("local_mlx_mac_workstation.registry"),
            ],
            source_selection_id: String::from("validator_quorum_offer_v1"),
            replacement_registry_record_id: None,
            referenced_topology_revision_id: None,
            referenced_recovery_scenario_id: None,
            detail: String::from(
                "The first validator lease-set activation applied directly from the registry's validator quorum offer: Google plus Apple MLX became the active validator pair.",
            ),
        },
        ElasticMeshRevisionReceipt {
            revision_id: String::from("activate_checkpoint_authority_mesh_v1"),
            revision_kind: ElasticMeshRevisionKind::ActivateLeaseSet,
            trigger_kind: ElasticMeshRevisionTriggerKind::AdmissionGrant,
            outcome: ElasticMeshRevisionOutcome::Applied,
            affected_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
            ],
            source_selection_id: String::from("checkpoint_promotion_offer_v1"),
            replacement_registry_record_id: None,
            referenced_topology_revision_id: None,
            referenced_recovery_scenario_id: None,
            detail: String::from(
                "The first checkpoint-authority lease-set activation applied directly from the registry's checkpoint promotion offer: Google plus RunPod became the active authority pair.",
            ),
        },
        ElasticMeshRevisionReceipt {
            revision_id: String::from("promote_public_miner_standby_after_deathrattle_v1"),
            revision_kind: ElasticMeshRevisionKind::PromoteStandbyReplacement,
            trigger_kind: ElasticMeshRevisionTriggerKind::DeathrattleNotice,
            outcome: ElasticMeshRevisionOutcome::Applied,
            affected_registry_record_ids: vec![
                String::from("local_rtx4080_workstation.registry"),
                String::from("local_mlx_mac_workstation.registry"),
            ],
            source_selection_id: String::from("public_miner_window_offer_v1"),
            replacement_registry_record_id: Some(String::from("local_mlx_mac_workstation.registry")),
            referenced_topology_revision_id: None,
            referenced_recovery_scenario_id: None,
            detail: String::from(
                "The first mesh-side standby promotion applied after the RTX 4080 miner deathrattle: Apple MLX was promoted from the contributor-window standby lane into the active public miner set.",
            ),
        },
        ElasticMeshRevisionReceipt {
            revision_id: String::from("refuse_live_dense_world_change_without_checkpoint_barrier_v1"),
            revision_kind: ElasticMeshRevisionKind::RefuseDenseWorldChange,
            trigger_kind: ElasticMeshRevisionTriggerKind::BarrierRequired,
            outcome: ElasticMeshRevisionOutcome::Refused,
            affected_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
                String::from("local_mlx_mac_workstation.registry"),
            ],
            source_selection_id: String::from("dense_runtime_barrier_control"),
            replacement_registry_record_id: None,
            referenced_topology_revision_id: Some(String::from(
                "dense_topology.remove_rank_without_replacement.live_refused",
            )),
            referenced_recovery_scenario_id: Some(String::from(
                "dense_rank.provider_loss.rank3.shrink_world_refused",
            )),
            detail: String::from(
                "The mesh keeps live dense world-size change honest: runtime-managed public-role replacement is admitted, but live dense remove-without-replacement still refuses until the older topology and recovery surfaces widen beyond checkpoint-barrier-only closure.",
            ),
        },
    ];

    let mut contract = ElasticDeviceMeshContract {
        schema_version: String::from(ELASTIC_DEVICE_MESH_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(ELASTIC_DEVICE_MESH_CONTRACT_ID),
        network_id: network.network_id.clone(),
        governance_revision_id: network.governance_revision.governance_revision_id.clone(),
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        decentralized_network_contract_digest: network.contract_digest.clone(),
        public_network_registry_contract_digest: registry.contract_digest.clone(),
        dense_topology_revision_contract_digest: topology.contract_digest.clone(),
        dense_rank_recovery_contract_digest: recovery.contract_digest.clone(),
        role_lease_policies,
        member_leases,
        heartbeat_samples,
        deathrattles,
        revision_receipts,
        authority_paths: ElasticDeviceMeshAuthorityPaths {
            fixture_path: String::from(ELASTIC_DEVICE_MESH_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(ELASTIC_DEVICE_MESH_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(ELASTIC_DEVICE_MESH_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(ELASTIC_DEVICE_MESH_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract closes the first runtime-managed public mesh object above the registry: role-specific lease policy, current leases, current heartbeat samples, deathrattle notices, and typed membership revision receipts. It proves graceful public-role replacement and explicit dense-world live-change refusal. It does not yet claim full dense live elasticity, NAT traversal, or WAN-grade transport policy.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = ELASTIC_DEVICE_MESH_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_elastic_device_mesh_contract(
    output_path: impl AsRef<Path>,
) -> Result<ElasticDeviceMeshContract, ElasticDeviceMeshContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| ElasticDeviceMeshContractError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_elastic_device_mesh_contract()?;
    let json = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| ElasticDeviceMeshContractError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(contract)
}

fn role_policy(
    role_class: DecentralizedNetworkRoleClass,
    lease_duration_seconds: u16,
    heartbeat_interval_seconds: u16,
    max_missed_heartbeats: u16,
    deathrattle_grace_seconds: u16,
    detail: &str,
) -> ElasticMeshRoleLeasePolicy {
    ElasticMeshRoleLeasePolicy {
        role_class,
        lease_duration_seconds,
        heartbeat_interval_seconds,
        max_missed_heartbeats,
        deathrattle_grace_seconds,
        detail: String::from(detail),
    }
}

fn lease(
    lease_id: &str,
    registry_record_id: &str,
    role_class: DecentralizedNetworkRoleClass,
    assignment_state: ElasticMeshAssignmentState,
    status: ElasticMeshLeaseStatus,
    issued_at_unix_ms: u64,
    last_heartbeat_unix_ms: u64,
    source_selection_id: &str,
    role_policies: &[ElasticMeshRoleLeasePolicy],
    detail: &str,
) -> ElasticMeshMemberLease {
    let policy = role_policies
        .iter()
        .find(|policy| policy.role_class == role_class)
        .expect("lease role policy should exist");
    ElasticMeshMemberLease {
        lease_id: String::from(lease_id),
        registry_record_id: String::from(registry_record_id),
        role_class,
        assignment_state,
        status,
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        lease_duration_seconds: policy.lease_duration_seconds,
        heartbeat_interval_seconds: policy.heartbeat_interval_seconds,
        max_missed_heartbeats: policy.max_missed_heartbeats,
        issued_at_unix_ms,
        last_heartbeat_unix_ms,
        expires_at_unix_ms: last_heartbeat_unix_ms
            + u64::from(policy.lease_duration_seconds) * 1_000,
        source_selection_id: String::from(source_selection_id),
        detail: String::from(detail),
    }
}

fn heartbeat(
    heartbeat_id: &str,
    lease_id: &str,
    observed_at_unix_ms: u64,
    round_trip_latency_ms: u16,
    missed_heartbeats: u16,
    status: ElasticMeshHeartbeatStatus,
    detail: &str,
) -> ElasticMeshHeartbeatSample {
    ElasticMeshHeartbeatSample {
        heartbeat_id: String::from(heartbeat_id),
        lease_id: String::from(lease_id),
        observed_at_unix_ms,
        round_trip_latency_ms,
        missed_heartbeats,
        status,
        detail: String::from(detail),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("elastic device mesh contract digest serialization must work"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elastic_device_mesh_contract_stays_valid() {
        let contract = canonical_elastic_device_mesh_contract()
            .expect("elastic device mesh contract should build");
        contract.validate().expect("contract should validate");
    }

    #[test]
    fn elastic_device_mesh_contract_retains_single_deathrattle_replacement() {
        let contract = canonical_elastic_device_mesh_contract()
            .expect("elastic device mesh contract should build");
        assert_eq!(contract.deathrattles.len(), 1);
        let deathrattle = &contract.deathrattles[0];
        assert_eq!(
            deathrattle.replacement_registry_record_id.as_deref(),
            Some("local_mlx_mac_workstation.registry")
        );
    }

    #[test]
    fn elastic_device_mesh_contract_keeps_dense_live_change_refused() {
        let contract = canonical_elastic_device_mesh_contract()
            .expect("elastic device mesh contract should build");
        let refusal = contract
            .revision_receipts
            .iter()
            .find(|revision| {
                revision.revision_kind == ElasticMeshRevisionKind::RefuseDenseWorldChange
            })
            .expect("refusal revision should exist");
        assert_eq!(refusal.outcome, ElasticMeshRevisionOutcome::Refused);
        assert_eq!(
            refusal.referenced_topology_revision_id.as_deref(),
            Some("dense_topology.remove_rank_without_replacement.live_refused")
        );
    }
}
