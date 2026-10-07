use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_dense_rank_recovery_contract, canonical_elastic_device_mesh_contract,
    canonical_public_network_registry_contract, canonical_sharded_distributed_checkpoint_contract,
    canonical_wan_overlay_route_contract, DecentralizedNetworkRoleClass,
    DenseRankRecoveryContractError, DistributedCheckpointContractError,
    ElasticDeviceMeshContractError, ElasticMeshLeaseStatus, PublicNetworkRegistryContractError,
    WanOverlayRouteContractError, PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID,
};

pub const LIVE_CHECKPOINT_CATCHUP_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.live_checkpoint_catchup_contract.v1";
pub const LIVE_CHECKPOINT_CATCHUP_CONTRACT_ID: &str = "psionic.live_checkpoint_catchup_contract.v1";
pub const LIVE_CHECKPOINT_CATCHUP_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/live_checkpoint_catchup_contract_v1.json";
pub const LIVE_CHECKPOINT_CATCHUP_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-live-checkpoint-catchup-contract.sh";
pub const LIVE_CHECKPOINT_CATCHUP_CONTRACT_DOC_PATH: &str =
    "docs/LIVE_CHECKPOINT_CATCHUP_REFERENCE.md";
pub const LIVE_CHECKPOINT_CATCHUP_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum LiveCheckpointCatchupContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    DistributedCheckpoint(#[from] DistributedCheckpointContractError),
    #[error(transparent)]
    DenseRecovery(#[from] DenseRankRecoveryContractError),
    #[error(transparent)]
    ElasticMesh(#[from] ElasticDeviceMeshContractError),
    #[error(transparent)]
    PublicRegistry(#[from] PublicNetworkRegistryContractError),
    #[error(transparent)]
    WanRoute(#[from] WanOverlayRouteContractError),
    #[error("live checkpoint catch-up contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatchupAdvertisementKind {
    CheckpointAuthorityMirror,
    ActivePeerSidecar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatchupDisposition {
    Completed,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveCheckpointAdvertisement {
    pub advertisement_id: String,
    pub serving_registry_record_id: String,
    pub serving_role_class: DecentralizedNetworkRoleClass,
    pub advertisement_kind: CatchupAdvertisementKind,
    pub checkpoint_manifest_digest: String,
    pub checkpoint_pointer_digest: String,
    pub advertised_step: u64,
    pub available_resume_window_ids: Vec<String>,
    pub serves_parameter_state: bool,
    pub serves_optimizer_state: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveResumeWindow {
    pub window_id: String,
    pub start_step: u64,
    pub end_step: u64,
    pub minimum_fresh_step: u64,
    pub maximum_step_lag: u16,
    pub checkpoint_authority_registry_record_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveCheckpointCatchupReceipt {
    pub receipt_id: String,
    pub joining_registry_record_id: String,
    pub joining_role_class: DecentralizedNetworkRoleClass,
    pub trigger_mesh_revision_id: String,
    pub selected_advertisement_id: String,
    pub selected_route_id: String,
    pub restore_assignment_id: String,
    pub resume_window_id: String,
    pub target_checkpoint_step: u64,
    pub joining_step_lag: u16,
    pub served_parameter_bytes: u64,
    pub served_optimizer_bytes: u64,
    pub disposition: CatchupDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveCheckpointCatchupAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveCheckpointCatchupContract {
    pub schema_version: String,
    pub contract_id: String,
    pub current_epoch_id: String,
    pub checkpoint_manifest_digest: String,
    pub checkpoint_pointer_digest: String,
    pub distributed_checkpoint_contract_digest: String,
    pub dense_rank_recovery_contract_digest: String,
    pub elastic_device_mesh_contract_digest: String,
    pub wan_overlay_route_contract_digest: String,
    pub advertisements: Vec<LiveCheckpointAdvertisement>,
    pub resume_windows: Vec<LiveResumeWindow>,
    pub catchup_receipts: Vec<LiveCheckpointCatchupReceipt>,
    pub authority_paths: LiveCheckpointCatchupAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl LiveCheckpointCatchupContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_live_checkpoint_catchup_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), LiveCheckpointCatchupContractError> {
        let checkpoint_contract = canonical_sharded_distributed_checkpoint_contract()?;
        let dense_recovery = canonical_dense_rank_recovery_contract()?;
        let mesh = canonical_elastic_device_mesh_contract()?;
        let registry = canonical_public_network_registry_contract()?;
        let wan = canonical_wan_overlay_route_contract()?;
        let checkpoint_step = checkpoint_contract
            .checkpoint_manifest
            .checkpoint
            .step
            .or(checkpoint_contract.checkpoint_pointer.checkpoint.step)
            .ok_or_else(|| LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("distributed checkpoint contract lost its admitted step"),
            })?;

        let record_ids = registry
            .registry_records
            .iter()
            .map(|record| record.registry_record_id.as_str())
            .collect::<BTreeSet<_>>();
        let record_by_id = registry
            .registry_records
            .iter()
            .map(|record| (record.registry_record_id.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let active_role_pairs = mesh
            .member_leases
            .iter()
            .filter(|lease| lease.status == ElasticMeshLeaseStatus::Active)
            .map(|lease| (lease.registry_record_id.as_str(), lease.role_class))
            .collect::<BTreeSet<_>>();
        let revision_ids = mesh
            .revision_receipts
            .iter()
            .map(|revision| revision.revision_id.as_str())
            .collect::<BTreeSet<_>>();
        let route_by_id = wan
            .route_records
            .iter()
            .map(|route| (route.route_id.as_str(), route))
            .collect::<BTreeMap<_, _>>();
        let restore_assignment_by_id = checkpoint_contract
            .restore_plan
            .assignments
            .iter()
            .map(|assignment| (assignment.assignment_id.as_str(), assignment))
            .collect::<BTreeMap<_, _>>();
        let window_by_id = self
            .resume_windows
            .iter()
            .map(|window| (window.window_id.as_str(), window))
            .collect::<BTreeMap<_, _>>();
        let advertisement_by_id = self
            .advertisements
            .iter()
            .map(|advertisement| (advertisement.advertisement_id.as_str(), advertisement))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != LIVE_CHECKPOINT_CATCHUP_CONTRACT_SCHEMA_VERSION {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    LIVE_CHECKPOINT_CATCHUP_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != LIVE_CHECKPOINT_CATCHUP_CONTRACT_ID {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.current_epoch_id != PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("current_epoch_id drifted"),
            });
        }
        if self.checkpoint_manifest_digest
            != checkpoint_contract.checkpoint_manifest.manifest_digest
        {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("checkpoint_manifest_digest drifted"),
            });
        }
        if self.checkpoint_pointer_digest != checkpoint_contract.checkpoint_pointer.pointer_digest {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("checkpoint_pointer_digest drifted"),
            });
        }
        if self.distributed_checkpoint_contract_digest != checkpoint_contract.contract_digest {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("distributed checkpoint digest drifted"),
            });
        }
        if self.dense_rank_recovery_contract_digest != dense_recovery.contract_digest {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("dense rank recovery digest drifted"),
            });
        }
        if self.elastic_device_mesh_contract_digest != mesh.contract_digest {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("elastic device mesh digest drifted"),
            });
        }
        if self.wan_overlay_route_contract_digest != wan.contract_digest {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("wan overlay route digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != LIVE_CHECKPOINT_CATCHUP_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != LIVE_CHECKPOINT_CATCHUP_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != LIVE_CHECKPOINT_CATCHUP_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != LIVE_CHECKPOINT_CATCHUP_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        let mut advertisement_ids = BTreeSet::new();
        for advertisement in &self.advertisements {
            if !advertisement_ids.insert(advertisement.advertisement_id.as_str()) {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "duplicate advertisement_id `{}`",
                        advertisement.advertisement_id
                    ),
                });
            }
            if !record_ids.contains(advertisement.serving_registry_record_id.as_str()) {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "advertisement `{}` references unknown registry record `{}`",
                        advertisement.advertisement_id, advertisement.serving_registry_record_id
                    ),
                });
            }
            if advertisement.checkpoint_manifest_digest != self.checkpoint_manifest_digest
                || advertisement.checkpoint_pointer_digest != self.checkpoint_pointer_digest
            {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "advertisement `{}` lost checkpoint digest binding",
                        advertisement.advertisement_id
                    ),
                });
            }
            match advertisement.advertisement_kind {
                CatchupAdvertisementKind::CheckpointAuthorityMirror => {
                    if advertisement.serving_role_class
                        != DecentralizedNetworkRoleClass::CheckpointAuthority
                        || !active_role_pairs.contains(&(
                            advertisement.serving_registry_record_id.as_str(),
                            DecentralizedNetworkRoleClass::CheckpointAuthority,
                        ))
                    {
                        return Err(LiveCheckpointCatchupContractError::InvalidContract {
                            detail: format!(
                                "advertisement `{}` must stay on an active checkpoint authority",
                                advertisement.advertisement_id
                            ),
                        });
                    }
                    if !(advertisement.serves_parameter_state
                        && advertisement.serves_optimizer_state)
                    {
                        return Err(LiveCheckpointCatchupContractError::InvalidContract {
                            detail: format!(
                                "checkpoint-authority advertisement `{}` lost full-state service",
                                advertisement.advertisement_id
                            ),
                        });
                    }
                }
                CatchupAdvertisementKind::ActivePeerSidecar => {
                    let sidecar_role = advertisement.serving_role_class;
                    if !matches!(
                        sidecar_role,
                        DecentralizedNetworkRoleClass::PublicMiner
                            | DecentralizedNetworkRoleClass::PublicValidator
                    ) || !active_role_pairs.contains(&(
                        advertisement.serving_registry_record_id.as_str(),
                        sidecar_role,
                    )) {
                        return Err(LiveCheckpointCatchupContractError::InvalidContract {
                            detail: format!(
                                "sidecar advertisement `{}` must stay on an active public peer role",
                                advertisement.advertisement_id
                            ),
                        });
                    }
                    if !advertisement.serves_parameter_state {
                        return Err(LiveCheckpointCatchupContractError::InvalidContract {
                            detail: format!(
                                "sidecar advertisement `{}` must keep parameter-state service explicit",
                                advertisement.advertisement_id
                            ),
                        });
                    }
                }
            }
            if advertisement.advertised_step != checkpoint_step {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "advertisement `{}` drifted from the admitted checkpoint step",
                        advertisement.advertisement_id
                    ),
                });
            }
            for window_id in &advertisement.available_resume_window_ids {
                if !window_by_id.contains_key(window_id.as_str()) {
                    return Err(LiveCheckpointCatchupContractError::InvalidContract {
                        detail: format!(
                            "advertisement `{}` references unknown resume window `{}`",
                            advertisement.advertisement_id, window_id
                        ),
                    });
                }
            }
        }

        let mut window_ids = BTreeSet::new();
        for window in &self.resume_windows {
            if !window_ids.insert(window.window_id.as_str()) {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!("duplicate resume window `{}`", window.window_id),
                });
            }
            if !(window.start_step < window.minimum_fresh_step
                && window.minimum_fresh_step <= window.end_step)
            {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "resume window `{}` lost step ordering semantics",
                        window.window_id
                    ),
                });
            }
            for checkpoint_authority_registry_record_id in
                &window.checkpoint_authority_registry_record_ids
            {
                if !active_role_pairs.contains(&(
                    checkpoint_authority_registry_record_id.as_str(),
                    DecentralizedNetworkRoleClass::CheckpointAuthority,
                )) {
                    return Err(LiveCheckpointCatchupContractError::InvalidContract {
                        detail: format!(
                            "resume window `{}` lost checkpoint authority `{}`",
                            window.window_id, checkpoint_authority_registry_record_id
                        ),
                    });
                }
            }
        }

        let mut receipt_ids = BTreeSet::new();
        for receipt in &self.catchup_receipts {
            if !receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!("duplicate catchup receipt `{}`", receipt.receipt_id),
                });
            }
            if !record_ids.contains(receipt.joining_registry_record_id.as_str()) {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "catchup receipt `{}` references unknown joining record `{}`",
                        receipt.receipt_id, receipt.joining_registry_record_id
                    ),
                });
            }
            if !revision_ids.contains(receipt.trigger_mesh_revision_id.as_str()) {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "catchup receipt `{}` lost mesh revision binding `{}`",
                        receipt.receipt_id, receipt.trigger_mesh_revision_id
                    ),
                });
            }
            let advertisement = advertisement_by_id
                .get(receipt.selected_advertisement_id.as_str())
                .ok_or_else(|| LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "catchup receipt `{}` references unknown advertisement `{}`",
                        receipt.receipt_id, receipt.selected_advertisement_id
                    ),
                })?;
            let advertisement_record = record_by_id.get(
                advertisement.serving_registry_record_id.as_str(),
            )
            .ok_or_else(|| LiveCheckpointCatchupContractError::InvalidContract {
                detail: format!(
                    "catchup receipt `{}` advertisement `{}` references unknown serving record `{}`",
                    receipt.receipt_id,
                    receipt.selected_advertisement_id,
                    advertisement.serving_registry_record_id
                ),
            })?;
            let route = route_by_id
                .get(receipt.selected_route_id.as_str())
                .ok_or_else(|| LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "catchup receipt `{}` references unknown route `{}`",
                        receipt.receipt_id, receipt.selected_route_id
                    ),
                })?;
            let restore_assignment = restore_assignment_by_id
                .get(receipt.restore_assignment_id.as_str())
                .ok_or_else(|| LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "catchup receipt `{}` references unknown restore assignment `{}`",
                        receipt.receipt_id, receipt.restore_assignment_id
                    ),
                })?;
            let window = window_by_id
                .get(receipt.resume_window_id.as_str())
                .ok_or_else(|| LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "catchup receipt `{}` references unknown resume window `{}`",
                        receipt.receipt_id, receipt.resume_window_id
                    ),
                })?;

            if receipt.target_checkpoint_step != advertisement.advertised_step
                || receipt.target_checkpoint_step < window.start_step
                || receipt.target_checkpoint_step > window.end_step
            {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "catchup receipt `{}` drifted from advertisement/window step bounds",
                        receipt.receipt_id
                    ),
                });
            }
            if !route_connects(
                route.src_registry_record_id.as_str(),
                route.dst_registry_record_id.as_str(),
                receipt.joining_registry_record_id.as_str(),
                advertisement.serving_registry_record_id.as_str(),
            ) {
                return Err(LiveCheckpointCatchupContractError::InvalidContract {
                    detail: format!(
                        "catchup receipt `{}` route `{}` no longer connects joiner `{}` and serving peer `{}`",
                        receipt.receipt_id,
                        receipt.selected_route_id,
                        receipt.joining_registry_record_id,
                        advertisement.serving_registry_record_id
                    ),
                });
            }
            match receipt.disposition {
                CatchupDisposition::Completed => {
                    if receipt.refusal.is_some()
                        || !(receipt.served_parameter_bytes > 0
                            && receipt.served_optimizer_bytes > 0)
                        || !advertisement.serves_optimizer_state
                        || receipt.joining_step_lag > window.maximum_step_lag
                        || restore_assignment.restore_source_id != advertisement_record.source_id
                    {
                        return Err(LiveCheckpointCatchupContractError::InvalidContract {
                            detail: format!(
                                "completed catchup receipt `{}` lost full-state join semantics",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
                CatchupDisposition::Refused => {
                    let refusal = receipt.refusal.as_deref().ok_or_else(|| {
                        LiveCheckpointCatchupContractError::InvalidContract {
                            detail: format!(
                                "refused catchup receipt `{}` must keep an explicit refusal",
                                receipt.receipt_id
                            ),
                        }
                    })?;
                    if receipt.served_parameter_bytes == 0
                        || receipt.served_optimizer_bytes != 0
                        || !(!advertisement.serves_optimizer_state
                            || receipt.joining_step_lag > window.maximum_step_lag)
                        || refusal.is_empty()
                    {
                        return Err(LiveCheckpointCatchupContractError::InvalidContract {
                            detail: format!(
                                "refused catchup receipt `{}` lost stale or partial-state refusal semantics",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(LiveCheckpointCatchupContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

static LIVE_CHECKPOINT_CATCHUP_CONTRACT_CACHE: std::sync::OnceLock<LiveCheckpointCatchupContract> =
    std::sync::OnceLock::new();

pub fn canonical_live_checkpoint_catchup_contract(
) -> Result<LiveCheckpointCatchupContract, LiveCheckpointCatchupContractError> {
    if let Some(contract) = LIVE_CHECKPOINT_CATCHUP_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let checkpoint_contract = canonical_sharded_distributed_checkpoint_contract()?;
    let dense_recovery = canonical_dense_rank_recovery_contract()?;
    let mesh = canonical_elastic_device_mesh_contract()?;
    let wan = canonical_wan_overlay_route_contract()?;
    let checkpoint_step = checkpoint_contract
        .checkpoint_manifest
        .checkpoint
        .step
        .or(checkpoint_contract.checkpoint_pointer.checkpoint.step)
        .ok_or_else(|| LiveCheckpointCatchupContractError::InvalidContract {
            detail: String::from("distributed checkpoint contract lost its admitted step"),
        })?;

    let advertisements = vec![
        advertisement(
            "advertisement.checkpoint_authority.runpod.primary",
            "runpod_8xh100_dense_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            CatchupAdvertisementKind::CheckpointAuthorityMirror,
            &checkpoint_contract.checkpoint_manifest.manifest_digest,
            &checkpoint_contract.checkpoint_pointer.pointer_digest,
            checkpoint_step,
            &["resume_window.live_join_2048", "resume_window.partial_state_refused_2048"],
            true,
            true,
            "RunPod remains the primary full-state live catch-up source because the distributed checkpoint restore plan already assigns it as the canonical restore authority.",
        ),
        advertisement(
            "advertisement.checkpoint_authority.google.mirror",
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            CatchupAdvertisementKind::CheckpointAuthorityMirror,
            &checkpoint_contract.checkpoint_manifest.manifest_digest,
            &checkpoint_contract.checkpoint_pointer.pointer_digest,
            checkpoint_step,
            &["resume_window.live_join_2048"],
            true,
            true,
            "Google mirrors the same admitted checkpoint so a second checkpoint-authority source remains explicit.",
        ),
        advertisement(
            "advertisement.active_peer.local_mlx.sidecar",
            "local_mlx_mac_workstation.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            CatchupAdvertisementKind::ActivePeerSidecar,
            &checkpoint_contract.checkpoint_manifest.manifest_digest,
            &checkpoint_contract.checkpoint_pointer.pointer_digest,
            checkpoint_step,
            &["resume_window.partial_state_refused_2048"],
            true,
            false,
            "The promoted Apple MLX public miner exposes a sidecar checkpoint lane for faster handoff, but it intentionally does not claim optimizer-state completeness.",
        ),
    ];

    let resume_windows = vec![
        LiveResumeWindow {
            window_id: String::from("resume_window.live_join_2048"),
            start_step: 1_984,
            end_step: 2_080,
            minimum_fresh_step: 2_016,
            maximum_step_lag: 32,
            checkpoint_authority_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
            ],
            detail: String::from(
                "The main live-join window admits replacement nodes that are at most 32 steps behind the admitted checkpoint and can fetch full optimizer plus parameter state from checkpoint authorities.",
            ),
        },
        LiveResumeWindow {
            window_id: String::from("resume_window.partial_state_refused_2048"),
            start_step: 2_016,
            end_step: 2_080,
            minimum_fresh_step: 2_040,
            maximum_step_lag: 16,
            checkpoint_authority_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
            ],
            detail: String::from(
                "The tighter sidecar window exists only to prove that stale or optimizer-incomplete peer-sidecar resume attempts are refused instead of being treated as equivalent to checkpoint-authority recovery.",
            ),
        },
    ];

    let catchup_receipts = vec![
        LiveCheckpointCatchupReceipt {
            receipt_id: String::from("catchup.public_miner.local_mlx.after_deathrattle"),
            joining_registry_record_id: String::from("local_mlx_mac_workstation.registry"),
            joining_role_class: DecentralizedNetworkRoleClass::PublicMiner,
            trigger_mesh_revision_id: String::from("promote_public_miner_standby_after_deathrattle_v1"),
            selected_advertisement_id: String::from("advertisement.checkpoint_authority.runpod.primary"),
            selected_route_id: String::from("route.checkpoint_authority.local_mlx_runpod.overlay"),
            restore_assignment_id: String::from("restore-rank-2"),
            resume_window_id: String::from("resume_window.live_join_2048"),
            target_checkpoint_step: checkpoint_step,
            joining_step_lag: 0,
            served_parameter_bytes: 134_217_728,
            served_optimizer_bytes: 268_435_456,
            disposition: CatchupDisposition::Completed,
            refusal: None,
            detail: String::from(
                "The promoted Apple MLX miner rejoins the admitted public-miner window through a real live catch-up flow: RunPod serves the canonical checkpoint over the overlay path and satisfies the canonical restore assignment.",
            ),
        },
        LiveCheckpointCatchupReceipt {
            receipt_id: String::from("catchup.public_miner.local_rtx4080.partial_state_refused"),
            joining_registry_record_id: String::from("local_rtx4080_workstation.registry"),
            joining_role_class: DecentralizedNetworkRoleClass::PublicMiner,
            trigger_mesh_revision_id: String::from("promote_public_miner_standby_after_deathrattle_v1"),
            selected_advertisement_id: String::from("advertisement.active_peer.local_mlx.sidecar"),
            selected_route_id: String::from("route.public_miner.local_rtx4080_local_mlx.overlay_failover"),
            restore_assignment_id: String::from("restore-rank-3"),
            resume_window_id: String::from("resume_window.partial_state_refused_2048"),
            target_checkpoint_step: checkpoint_step,
            joining_step_lag: 24,
            served_parameter_bytes: 67_108_864,
            served_optimizer_bytes: 0,
            disposition: CatchupDisposition::Refused,
            refusal: Some(String::from(
                "active-peer sidecar could not satisfy optimizer-state completeness inside the 16-step freshness window",
            )),
            detail: String::from(
                "The old RTX 4080 miner does not get to claim live resume parity with checkpoint-authority recovery: the MLX sidecar can stream parameter state over the overlay route, but the attempt is refused because the joiner is too stale and optimizer state is incomplete.",
            ),
        },
    ];

    let mut contract = LiveCheckpointCatchupContract {
        schema_version: String::from(LIVE_CHECKPOINT_CATCHUP_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(LIVE_CHECKPOINT_CATCHUP_CONTRACT_ID),
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        checkpoint_manifest_digest: checkpoint_contract.checkpoint_manifest.manifest_digest.clone(),
        checkpoint_pointer_digest: checkpoint_contract.checkpoint_pointer.pointer_digest.clone(),
        distributed_checkpoint_contract_digest: checkpoint_contract.contract_digest.clone(),
        dense_rank_recovery_contract_digest: dense_recovery.contract_digest.clone(),
        elastic_device_mesh_contract_digest: mesh.contract_digest.clone(),
        wan_overlay_route_contract_digest: wan.contract_digest.clone(),
        advertisements,
        resume_windows,
        catchup_receipts,
        authority_paths: LiveCheckpointCatchupAuthorityPaths {
            fixture_path: String::from(LIVE_CHECKPOINT_CATCHUP_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(LIVE_CHECKPOINT_CATCHUP_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(LIVE_CHECKPOINT_CATCHUP_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(LIVE_CHECKPOINT_CATCHUP_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first truthful live catch-up layer above the WAN route contract: checkpoint advertisements, resume windows, one successful replacement catch-up, and one explicit stale or optimizer-incomplete refusal. It does not yet claim WAN-efficient outer sync or public internet soak closure.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = LIVE_CHECKPOINT_CATCHUP_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_live_checkpoint_catchup_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), LiveCheckpointCatchupContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            LiveCheckpointCatchupContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_live_checkpoint_catchup_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| LiveCheckpointCatchupContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn advertisement(
    advertisement_id: &str,
    serving_registry_record_id: &str,
    serving_role_class: DecentralizedNetworkRoleClass,
    advertisement_kind: CatchupAdvertisementKind,
    checkpoint_manifest_digest: &str,
    checkpoint_pointer_digest: &str,
    advertised_step: u64,
    available_resume_window_ids: &[&str],
    serves_parameter_state: bool,
    serves_optimizer_state: bool,
    detail: &str,
) -> LiveCheckpointAdvertisement {
    LiveCheckpointAdvertisement {
        advertisement_id: String::from(advertisement_id),
        serving_registry_record_id: String::from(serving_registry_record_id),
        serving_role_class,
        advertisement_kind,
        checkpoint_manifest_digest: String::from(checkpoint_manifest_digest),
        checkpoint_pointer_digest: String::from(checkpoint_pointer_digest),
        advertised_step,
        available_resume_window_ids: available_resume_window_ids
            .iter()
            .copied()
            .map(String::from)
            .collect(),
        serves_parameter_state,
        serves_optimizer_state,
        detail: String::from(detail),
    }
}

fn route_connects(
    route_src: &str,
    route_dst: &str,
    joining_registry_record_id: &str,
    serving_registry_record_id: &str,
) -> bool {
    (route_src == joining_registry_record_id && route_dst == serving_registry_record_id)
        || (route_src == serving_registry_record_id && route_dst == joining_registry_record_id)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for catchup contract"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_live_checkpoint_catchup_contract, CatchupDisposition,
        LiveCheckpointCatchupContractError,
    };

    #[test]
    fn canonical_live_checkpoint_catchup_contract_is_valid(
    ) -> Result<(), LiveCheckpointCatchupContractError> {
        let contract = canonical_live_checkpoint_catchup_contract()?;
        contract.validate()
    }

    #[test]
    fn partial_state_refusal_must_stay_refused() -> Result<(), LiveCheckpointCatchupContractError> {
        let mut contract = canonical_live_checkpoint_catchup_contract()?;
        let refused_receipt = contract
            .catchup_receipts
            .iter_mut()
            .find(|receipt| {
                receipt.receipt_id == "catchup.public_miner.local_rtx4080.partial_state_refused"
            })
            .expect("canonical refused catchup receipt must exist");
        refused_receipt.disposition = CatchupDisposition::Completed;
        refused_receipt.refusal = None;
        let error = contract
            .validate()
            .expect_err("sidecar partial-state receipt cannot flip to completed");
        assert!(matches!(
            error,
            LiveCheckpointCatchupContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
