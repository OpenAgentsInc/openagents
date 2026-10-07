use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_decentralized_network_contract, canonical_elastic_device_mesh_contract,
    canonical_public_network_registry_contract, DecentralizedNetworkContractError,
    DecentralizedNetworkRoleClass, ElasticDeviceMeshContractError, ElasticMeshLeaseStatus,
    PublicNetworkRegistryContractError, PublicNetworkSessionKind,
    PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID,
};

pub const WAN_OVERLAY_ROUTE_CONTRACT_SCHEMA_VERSION: &str = "psionic.wan_overlay_route_contract.v1";
pub const WAN_OVERLAY_ROUTE_CONTRACT_ID: &str = "psionic.wan_overlay_route_contract.v1";
pub const WAN_OVERLAY_ROUTE_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/wan_overlay_route_contract_v1.json";
pub const WAN_OVERLAY_ROUTE_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-wan-overlay-route-contract.sh";
pub const WAN_OVERLAY_ROUTE_CONTRACT_DOC_PATH: &str = "docs/WAN_OVERLAY_ROUTE_REFERENCE.md";
pub const WAN_OVERLAY_ROUTE_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum WanOverlayRouteContractError {
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
    ElasticMesh(#[from] ElasticDeviceMeshContractError),
    #[error("wan overlay route contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WanNatPosture {
    PublicReachable,
    ConeNat,
    SymmetricNat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WanPeerTransportKind {
    Direct,
    Relayed,
    OverlayTunnel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WanRouteFailoverTriggerKind {
    PacketLossOverflow,
    RelayLeaseLoss,
    HeartbeatStale,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WanPeerNatPostureRecord {
    pub registry_record_id: String,
    pub nat_posture: WanNatPosture,
    pub advertises_direct_route: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WanPeerRouteQualitySample {
    pub sample_id: String,
    pub route_id: String,
    pub round_trip_latency_ms: u16,
    pub sustained_bandwidth_mbps: u16,
    pub packet_loss_bps: u16,
    pub egress_cost_basis_points: u16,
    pub route_quality_score_bps: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WanPeerRouteRecord {
    pub route_id: String,
    pub session_kind: PublicNetworkSessionKind,
    pub src_registry_record_id: String,
    pub dst_registry_record_id: String,
    pub selected_transport: WanPeerTransportKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_registry_record_id: Option<String>,
    pub source_selection_id: String,
    pub evidence_sample_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WanRouteFailoverReceipt {
    pub receipt_id: String,
    pub session_kind: PublicNetworkSessionKind,
    pub previous_route_id: String,
    pub next_route_id: String,
    pub trigger_kind: WanRouteFailoverTriggerKind,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WanOverlayRouteAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WanOverlayRouteContract {
    pub schema_version: String,
    pub contract_id: String,
    pub network_id: String,
    pub governance_revision_id: String,
    pub current_epoch_id: String,
    pub decentralized_network_contract_digest: String,
    pub public_network_registry_contract_digest: String,
    pub elastic_device_mesh_contract_digest: String,
    pub nat_postures: Vec<WanPeerNatPostureRecord>,
    pub route_quality_samples: Vec<WanPeerRouteQualitySample>,
    pub route_records: Vec<WanPeerRouteRecord>,
    pub failover_receipts: Vec<WanRouteFailoverReceipt>,
    pub authority_paths: WanOverlayRouteAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl WanOverlayRouteContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_wan_overlay_route_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), WanOverlayRouteContractError> {
        let network = canonical_decentralized_network_contract()?;
        let registry = canonical_public_network_registry_contract()?;
        let mesh = canonical_elastic_device_mesh_contract()?;

        let record_ids = registry
            .registry_records
            .iter()
            .map(|record| record.registry_record_id.as_str())
            .collect::<BTreeSet<_>>();
        let nat_posture_by_record = self
            .nat_postures
            .iter()
            .map(|record| (record.registry_record_id.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let quality_by_route = self
            .route_quality_samples
            .iter()
            .map(|sample| (sample.route_id.as_str(), sample))
            .collect::<BTreeMap<_, _>>();
        let route_by_id = self
            .route_records
            .iter()
            .map(|route| (route.route_id.as_str(), route))
            .collect::<BTreeMap<_, _>>();
        let active_relay_registry_ids = mesh
            .member_leases
            .iter()
            .filter(|lease| {
                lease.role_class == DecentralizedNetworkRoleClass::Relay
                    && lease.status == ElasticMeshLeaseStatus::Active
            })
            .map(|lease| lease.registry_record_id.as_str())
            .collect::<BTreeSet<_>>();
        let admitted_source_selection_ids = registry
            .matchmaking_offers
            .iter()
            .map(|offer| offer.offer_id.as_str())
            .chain(
                registry
                    .discovery_examples
                    .iter()
                    .map(|query| query.query_id.as_str()),
            )
            .chain(
                mesh.revision_receipts
                    .iter()
                    .map(|revision| revision.revision_id.as_str()),
            )
            .collect::<BTreeSet<_>>();

        if self.schema_version != WAN_OVERLAY_ROUTE_CONTRACT_SCHEMA_VERSION {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    WAN_OVERLAY_ROUTE_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != WAN_OVERLAY_ROUTE_CONTRACT_ID {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.network_id != network.network_id {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("network_id drifted"),
            });
        }
        if self.governance_revision_id != network.governance_revision.governance_revision_id {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("governance_revision_id drifted"),
            });
        }
        if self.current_epoch_id != PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("current_epoch_id drifted"),
            });
        }
        if self.decentralized_network_contract_digest != network.contract_digest {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("decentralized network digest drifted"),
            });
        }
        if self.public_network_registry_contract_digest != registry.contract_digest {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("public network registry digest drifted"),
            });
        }
        if self.elastic_device_mesh_contract_digest != mesh.contract_digest {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("elastic device mesh digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != WAN_OVERLAY_ROUTE_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != WAN_OVERLAY_ROUTE_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != WAN_OVERLAY_ROUTE_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != WAN_OVERLAY_ROUTE_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        let actual_nat_records = self
            .nat_postures
            .iter()
            .map(|record| record.registry_record_id.as_str())
            .collect::<BTreeSet<_>>();
        if actual_nat_records != record_ids {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("nat posture coverage drifted from the public registry"),
            });
        }

        let mut sample_ids = BTreeSet::new();
        for sample in &self.route_quality_samples {
            if !sample_ids.insert(sample.sample_id.as_str()) {
                return Err(WanOverlayRouteContractError::InvalidContract {
                    detail: format!("duplicate sample_id `{}`", sample.sample_id),
                });
            }
            if sample.round_trip_latency_ms == 0
                || sample.sustained_bandwidth_mbps == 0
                || sample.route_quality_score_bps == 0
            {
                return Err(WanOverlayRouteContractError::InvalidContract {
                    detail: format!(
                        "route quality sample `{}` lost transport metrics",
                        sample.sample_id
                    ),
                });
            }
        }

        let mut saw_direct = false;
        let mut saw_relayed = false;
        let mut saw_overlay = false;
        let mut route_ids = BTreeSet::new();
        for route in &self.route_records {
            if !route_ids.insert(route.route_id.as_str()) {
                return Err(WanOverlayRouteContractError::InvalidContract {
                    detail: format!("duplicate route_id `{}`", route.route_id),
                });
            }
            if !record_ids.contains(route.src_registry_record_id.as_str())
                || !record_ids.contains(route.dst_registry_record_id.as_str())
            {
                return Err(WanOverlayRouteContractError::InvalidContract {
                    detail: format!(
                        "route `{}` references an unknown registry record",
                        route.route_id
                    ),
                });
            }
            if route.src_registry_record_id == route.dst_registry_record_id {
                return Err(WanOverlayRouteContractError::InvalidContract {
                    detail: format!(
                        "route `{}` collapsed source and destination",
                        route.route_id
                    ),
                });
            }
            if !admitted_source_selection_ids.contains(route.source_selection_id.as_str()) {
                return Err(WanOverlayRouteContractError::InvalidContract {
                    detail: format!(
                        "route `{}` lost its source selection binding `{}`",
                        route.route_id, route.source_selection_id
                    ),
                });
            }
            let src_nat = nat_posture_by_record
                .get(route.src_registry_record_id.as_str())
                .ok_or_else(|| WanOverlayRouteContractError::InvalidContract {
                    detail: format!("route `{}` lost src nat posture", route.route_id),
                })?;
            let dst_nat = nat_posture_by_record
                .get(route.dst_registry_record_id.as_str())
                .ok_or_else(|| WanOverlayRouteContractError::InvalidContract {
                    detail: format!("route `{}` lost dst nat posture", route.route_id),
                })?;
            let quality = quality_by_route
                .get(route.route_id.as_str())
                .ok_or_else(|| WanOverlayRouteContractError::InvalidContract {
                    detail: format!("route `{}` lost quality evidence", route.route_id),
                })?;

            match route.selected_transport {
                WanPeerTransportKind::Direct => {
                    saw_direct = true;
                    if route.relay_registry_record_id.is_some() {
                        return Err(WanOverlayRouteContractError::InvalidContract {
                            detail: format!(
                                "direct route `{}` should not name a relay",
                                route.route_id
                            ),
                        });
                    }
                    if !(src_nat.advertises_direct_route && dst_nat.advertises_direct_route) {
                        return Err(WanOverlayRouteContractError::InvalidContract {
                            detail: format!(
                                "direct route `{}` requires both peers to advertise direct reachability",
                                route.route_id
                            ),
                        });
                    }
                }
                WanPeerTransportKind::Relayed | WanPeerTransportKind::OverlayTunnel => {
                    if route.selected_transport == WanPeerTransportKind::Relayed {
                        saw_relayed = true;
                    } else {
                        saw_overlay = true;
                    }
                    let relay_registry_record_id = route
                        .relay_registry_record_id
                        .as_deref()
                        .ok_or_else(|| WanOverlayRouteContractError::InvalidContract {
                            detail: format!("route `{}` requires a relay binding", route.route_id),
                        })?;
                    if !active_relay_registry_ids.contains(relay_registry_record_id) {
                        return Err(WanOverlayRouteContractError::InvalidContract {
                            detail: format!(
                                "route `{}` names relay `{}` outside the active relay lease set",
                                route.route_id, relay_registry_record_id
                            ),
                        });
                    }
                    if relay_registry_record_id == route.src_registry_record_id
                        || relay_registry_record_id == route.dst_registry_record_id
                    {
                        return Err(WanOverlayRouteContractError::InvalidContract {
                            detail: format!(
                                "route `{}` reuses one endpoint as the relay hop",
                                route.route_id
                            ),
                        });
                    }
                    if quality.packet_loss_bps == 0 {
                        return Err(WanOverlayRouteContractError::InvalidContract {
                            detail: format!(
                                "route `{}` lost the degradation evidence that justified indirection",
                                route.route_id
                            ),
                        });
                    }
                }
            }
        }
        if !(saw_direct && saw_relayed && saw_overlay) {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from(
                    "transport mix drifted; direct, relayed, and overlay routes must all remain explicit",
                ),
            });
        }

        let mut failover_ids = BTreeSet::new();
        for receipt in &self.failover_receipts {
            if !failover_ids.insert(receipt.receipt_id.as_str()) {
                return Err(WanOverlayRouteContractError::InvalidContract {
                    detail: format!("duplicate failover receipt `{}`", receipt.receipt_id),
                });
            }
            let previous = route_by_id
                .get(receipt.previous_route_id.as_str())
                .ok_or_else(|| WanOverlayRouteContractError::InvalidContract {
                    detail: format!(
                        "failover receipt `{}` lost previous route `{}`",
                        receipt.receipt_id, receipt.previous_route_id
                    ),
                })?;
            let next = route_by_id
                .get(receipt.next_route_id.as_str())
                .ok_or_else(|| WanOverlayRouteContractError::InvalidContract {
                    detail: format!(
                        "failover receipt `{}` lost next route `{}`",
                        receipt.receipt_id, receipt.next_route_id
                    ),
                })?;
            if previous.session_kind != receipt.session_kind
                || next.session_kind != receipt.session_kind
                || previous.src_registry_record_id != next.src_registry_record_id
                || previous.dst_registry_record_id != next.dst_registry_record_id
                || previous.selected_transport == next.selected_transport
            {
                return Err(WanOverlayRouteContractError::InvalidContract {
                    detail: format!(
                        "failover receipt `{}` drifted from one peer-pair transport transition",
                        receipt.receipt_id
                    ),
                });
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(WanOverlayRouteContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

static WAN_OVERLAY_ROUTE_CONTRACT_CACHE: std::sync::OnceLock<WanOverlayRouteContract> =
    std::sync::OnceLock::new();

pub fn canonical_wan_overlay_route_contract(
) -> Result<WanOverlayRouteContract, WanOverlayRouteContractError> {
    if let Some(contract) = WAN_OVERLAY_ROUTE_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let network = canonical_decentralized_network_contract()?;
    let registry = canonical_public_network_registry_contract()?;
    let mesh = canonical_elastic_device_mesh_contract()?;

    let nat_postures = vec![
        nat_posture(
            "google_l4_validator_node.registry",
            WanNatPosture::PublicReachable,
            true,
            "Google exposes the current public relay and checkpoint-authority ingress, so it remains directly reachable for steady-state control traffic.",
        ),
        nat_posture(
            "runpod_8xh100_dense_node.registry",
            WanNatPosture::PublicReachable,
            true,
            "RunPod exposes public control and checkpoint mirror endpoints, so checkpoint-authority traffic can remain direct while the path is healthy.",
        ),
        nat_posture(
            "local_rtx4080_workstation.registry",
            WanNatPosture::ConeNat,
            false,
            "The RTX 4080 workstation remains reachable only through relay or overlay assistance on the public internet.",
        ),
        nat_posture(
            "local_mlx_mac_workstation.registry",
            WanNatPosture::SymmetricNat,
            false,
            "The Apple MLX workstation is the worst NAT case in the current mesh, so direct assumptions remain refused.",
        ),
    ];

    let route_quality_samples = vec![
        quality_sample(
            "quality.google_runpod.direct",
            "route.checkpoint_authority.google_runpod.direct",
            31,
            6_400,
            2,
            14,
            9_440,
            "Google-to-RunPod direct transport remains the best checkpoint-authority path while both endpoints stay public-reachable.",
        ),
        quality_sample(
            "quality.local_rtx4080_local_mlx.relayed",
            "route.public_miner.local_rtx4080_local_mlx.relayed",
            86,
            480,
            41,
            18,
            7_120,
            "The first public-miner path between the two local nodes uses the Google relay because neither endpoint currently advertises direct reachability.",
        ),
        quality_sample(
            "quality.local_rtx4080_local_mlx.overlay",
            "route.public_miner.local_rtx4080_local_mlx.overlay_failover",
            63,
            720,
            18,
            16,
            8_040,
            "The overlay-tunnel fallback improves latency and loss for the local-node pair once the relay-only path degrades.",
        ),
        quality_sample(
            "quality.local_mlx_runpod.overlay",
            "route.checkpoint_authority.local_mlx_runpod.overlay",
            58,
            910,
            15,
            17,
            7_880,
            "Apple MLX reaches RunPod through the same relay-assisted overlay control plane for live catch-up and checkpoint serving.",
        ),
    ];

    let route_records = vec![
        route_record(
            "route.checkpoint_authority.google_runpod.direct",
            PublicNetworkSessionKind::CheckpointPromotion,
            "google_l4_validator_node.registry",
            "runpod_8xh100_dense_node.registry",
            WanPeerTransportKind::Direct,
            None,
            "checkpoint_promotion_offer_v1",
            "quality.google_runpod.direct",
            "Checkpoint-authority peers use direct transport while both public endpoints stay healthy, keeping the lowest-latency durable-state lane explicit.",
        ),
        route_record(
            "route.public_miner.local_rtx4080_local_mlx.relayed",
            PublicNetworkSessionKind::ContributorWindow,
            "local_rtx4080_workstation.registry",
            "local_mlx_mac_workstation.registry",
            WanPeerTransportKind::Relayed,
            Some("google_l4_validator_node.registry"),
            "public_miner_window_offer_v1",
            "quality.local_rtx4080_local_mlx.relayed",
            "The initial public-miner route between the two local nodes is relay-bound through Google because both peers are still NAT-constrained.",
        ),
        route_record(
            "route.public_miner.local_rtx4080_local_mlx.overlay_failover",
            PublicNetworkSessionKind::ContributorWindow,
            "local_rtx4080_workstation.registry",
            "local_mlx_mac_workstation.registry",
            WanPeerTransportKind::OverlayTunnel,
            Some("google_l4_validator_node.registry"),
            "promote_public_miner_standby_after_deathrattle_v1",
            "quality.local_rtx4080_local_mlx.overlay",
            "The public-miner pair can fail over from a pure relay hop to a relay-assisted overlay tunnel without silently dropping the window.",
        ),
        route_record(
            "route.checkpoint_authority.local_mlx_runpod.overlay",
            PublicNetworkSessionKind::CheckpointPromotion,
            "local_mlx_mac_workstation.registry",
            "runpod_8xh100_dense_node.registry",
            WanPeerTransportKind::OverlayTunnel,
            Some("google_l4_validator_node.registry"),
            "checkpoint_promotion_offer_v1",
            "quality.local_mlx_runpod.overlay",
            "Apple MLX reaches the RunPod checkpoint authority through the overlay lane so live catch-up can reuse the same public path truth later.",
        ),
    ];

    let failover_receipts = vec![WanRouteFailoverReceipt {
        receipt_id: String::from("failover.public_miner.local_rtx4080_local_mlx.1"),
        session_kind: PublicNetworkSessionKind::ContributorWindow,
        previous_route_id: String::from("route.public_miner.local_rtx4080_local_mlx.relayed"),
        next_route_id: String::from("route.public_miner.local_rtx4080_local_mlx.overlay_failover"),
        trigger_kind: WanRouteFailoverTriggerKind::PacketLossOverflow,
        detail: String::from(
            "The first public-miner transport failover is explicit: packet loss on the relay-only path crossed the allowed threshold, so the mesh promoted the overlay-tunnel fallback for the same peer pair.",
        ),
    }];

    let mut contract = WanOverlayRouteContract {
        schema_version: String::from(WAN_OVERLAY_ROUTE_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(WAN_OVERLAY_ROUTE_CONTRACT_ID),
        network_id: network.network_id.clone(),
        governance_revision_id: network.governance_revision.governance_revision_id.clone(),
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        decentralized_network_contract_digest: network.contract_digest.clone(),
        public_network_registry_contract_digest: registry.contract_digest.clone(),
        elastic_device_mesh_contract_digest: mesh.contract_digest.clone(),
        nat_postures,
        route_quality_samples,
        route_records,
        failover_receipts,
        authority_paths: WanOverlayRouteAuthorityPaths {
            fixture_path: String::from(WAN_OVERLAY_ROUTE_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(WAN_OVERLAY_ROUTE_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(WAN_OVERLAY_ROUTE_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(WAN_OVERLAY_ROUTE_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first internet-native route truth above the elastic device mesh: peer NAT posture, route-quality evidence, direct-vs-relayed-vs-overlay selection, and explicit transport failover receipts. It does not yet claim live checkpoint catch-up, compressed outer sync, or public internet soak closure.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = WAN_OVERLAY_ROUTE_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_wan_overlay_route_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), WanOverlayRouteContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| WanOverlayRouteContractError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_wan_overlay_route_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| WanOverlayRouteContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn nat_posture(
    registry_record_id: &str,
    nat_posture: WanNatPosture,
    advertises_direct_route: bool,
    detail: &str,
) -> WanPeerNatPostureRecord {
    WanPeerNatPostureRecord {
        registry_record_id: String::from(registry_record_id),
        nat_posture,
        advertises_direct_route,
        detail: String::from(detail),
    }
}

fn quality_sample(
    sample_id: &str,
    route_id: &str,
    round_trip_latency_ms: u16,
    sustained_bandwidth_mbps: u16,
    packet_loss_bps: u16,
    egress_cost_basis_points: u16,
    route_quality_score_bps: u16,
    detail: &str,
) -> WanPeerRouteQualitySample {
    WanPeerRouteQualitySample {
        sample_id: String::from(sample_id),
        route_id: String::from(route_id),
        round_trip_latency_ms,
        sustained_bandwidth_mbps,
        packet_loss_bps,
        egress_cost_basis_points,
        route_quality_score_bps,
        detail: String::from(detail),
    }
}

fn route_record(
    route_id: &str,
    session_kind: PublicNetworkSessionKind,
    src_registry_record_id: &str,
    dst_registry_record_id: &str,
    selected_transport: WanPeerTransportKind,
    relay_registry_record_id: Option<&str>,
    source_selection_id: &str,
    evidence_sample_id: &str,
    detail: &str,
) -> WanPeerRouteRecord {
    WanPeerRouteRecord {
        route_id: String::from(route_id),
        session_kind,
        src_registry_record_id: String::from(src_registry_record_id),
        dst_registry_record_id: String::from(dst_registry_record_id),
        selected_transport,
        relay_registry_record_id: relay_registry_record_id.map(String::from),
        source_selection_id: String::from(source_selection_id),
        evidence_sample_id: String::from(evidence_sample_id),
        detail: String::from(detail),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for route contract"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_wan_overlay_route_contract, WanOverlayRouteContractError, WanPeerTransportKind,
    };

    #[test]
    fn canonical_wan_overlay_route_contract_is_valid() -> Result<(), WanOverlayRouteContractError> {
        let contract = canonical_wan_overlay_route_contract()?;
        contract.validate()
    }

    #[test]
    fn direct_relay_overlay_mix_is_required() -> Result<(), WanOverlayRouteContractError> {
        let mut contract = canonical_wan_overlay_route_contract()?;
        contract
            .route_records
            .retain(|route| route.selected_transport != WanPeerTransportKind::OverlayTunnel);
        let error = contract
            .validate()
            .expect_err("missing overlay routes must fail");
        assert!(matches!(
            error,
            WanOverlayRouteContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
