use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_elastic_device_mesh_contract, canonical_live_checkpoint_catchup_contract,
    canonical_public_network_registry_contract, canonical_wan_overlay_route_contract,
    CatchupDisposition, DecentralizedNetworkRoleClass, ElasticDeviceMeshContractError,
    ElasticMeshLeaseStatus, LiveCheckpointCatchupContractError, PublicNetworkRegistryContractError,
    WanOverlayRouteContractError, PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID,
};

pub const QUANTIZED_OUTER_SYNC_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.quantized_outer_sync_contract.v1";
pub const QUANTIZED_OUTER_SYNC_CONTRACT_ID: &str = "psionic.quantized_outer_sync_contract.v1";
pub const QUANTIZED_OUTER_SYNC_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/quantized_outer_sync_contract_v1.json";
pub const QUANTIZED_OUTER_SYNC_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-quantized-outer-sync-contract.sh";
pub const QUANTIZED_OUTER_SYNC_CONTRACT_DOC_PATH: &str = "docs/QUANTIZED_OUTER_SYNC_REFERENCE.md";
pub const QUANTIZED_OUTER_SYNC_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum QuantizedOuterSyncContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ElasticMesh(#[from] ElasticDeviceMeshContractError),
    #[error(transparent)]
    LiveCatchup(#[from] LiveCheckpointCatchupContractError),
    #[error(transparent)]
    PublicRegistry(#[from] PublicNetworkRegistryContractError),
    #[error(transparent)]
    WanRoute(#[from] WanOverlayRouteContractError),
    #[error("quantized outer sync contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OuterSyncDeltaFamily {
    QuantizedPseudoGradient,
    QuantizedResidualDelta,
    FullPrecisionDenseAllReduce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OuterSyncQuantizationKind {
    Int8Blockwise,
    Nf4Residual,
    Fp16Uncompressed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OuterSyncExchangeDisposition {
    Applied,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OuterSyncDeltaPolicy {
    pub policy_id: String,
    pub delta_family: OuterSyncDeltaFamily,
    pub quantization_kind: OuterSyncQuantizationKind,
    pub chunk_bytes: u32,
    pub expected_compression_ratio_bps: u16,
    pub cpu_offload_required: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OuterSyncExchangeReceipt {
    pub receipt_id: String,
    pub epoch_anchor_catchup_receipt_id: String,
    pub source_registry_record_id: String,
    pub destination_registry_record_id: String,
    pub route_id: String,
    pub delta_policy_id: String,
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub round_trip_latency_ms: u16,
    pub disposition: OuterSyncExchangeDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OuterSyncAggregationReceipt {
    pub aggregation_id: String,
    pub authority_registry_record_id: String,
    pub authority_role_class: DecentralizedNetworkRoleClass,
    pub input_receipt_ids: Vec<String>,
    pub anchored_advertisement_id: String,
    pub bandwidth_accounted_bytes: u64,
    pub published_checkpoint_step: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OuterSyncCorrectnessReceipt {
    pub receipt_id: String,
    pub delta_policy_id: String,
    pub exchange_receipt_id: String,
    pub max_abs_error_ppm: u32,
    pub checksum_match: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedOuterSyncAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedOuterSyncContract {
    pub schema_version: String,
    pub contract_id: String,
    pub current_epoch_id: String,
    pub elastic_device_mesh_contract_digest: String,
    pub wan_overlay_route_contract_digest: String,
    pub live_checkpoint_catchup_contract_digest: String,
    pub delta_policies: Vec<OuterSyncDeltaPolicy>,
    pub exchange_receipts: Vec<OuterSyncExchangeReceipt>,
    pub aggregation_receipts: Vec<OuterSyncAggregationReceipt>,
    pub correctness_receipts: Vec<OuterSyncCorrectnessReceipt>,
    pub authority_paths: QuantizedOuterSyncAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl QuantizedOuterSyncContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_quantized_outer_sync_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), QuantizedOuterSyncContractError> {
        let mesh = canonical_elastic_device_mesh_contract()?;
        let registry = canonical_public_network_registry_contract()?;
        let wan = canonical_wan_overlay_route_contract()?;
        let catchup = canonical_live_checkpoint_catchup_contract()?;

        let record_ids = registry
            .registry_records
            .iter()
            .map(|record| record.registry_record_id.as_str())
            .collect::<BTreeSet<_>>();
        let active_role_pairs = mesh
            .member_leases
            .iter()
            .filter(|lease| lease.status == ElasticMeshLeaseStatus::Active)
            .map(|lease| (lease.registry_record_id.as_str(), lease.role_class))
            .collect::<BTreeSet<_>>();
        let route_by_id = wan
            .route_records
            .iter()
            .map(|route| (route.route_id.as_str(), route))
            .collect::<BTreeMap<_, _>>();
        let catchup_by_id = catchup
            .catchup_receipts
            .iter()
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let advertisement_ids = catchup
            .advertisements
            .iter()
            .map(|advertisement| advertisement.advertisement_id.as_str())
            .collect::<BTreeSet<_>>();
        let policy_by_id = self
            .delta_policies
            .iter()
            .map(|policy| (policy.policy_id.as_str(), policy))
            .collect::<BTreeMap<_, _>>();
        let exchange_by_id = self
            .exchange_receipts
            .iter()
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != QUANTIZED_OUTER_SYNC_CONTRACT_SCHEMA_VERSION {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    QUANTIZED_OUTER_SYNC_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != QUANTIZED_OUTER_SYNC_CONTRACT_ID {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.current_epoch_id != PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: String::from("current_epoch_id drifted"),
            });
        }
        if self.elastic_device_mesh_contract_digest != mesh.contract_digest {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: String::from("elastic device mesh digest drifted"),
            });
        }
        if self.wan_overlay_route_contract_digest != wan.contract_digest {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: String::from("wan overlay route digest drifted"),
            });
        }
        if self.live_checkpoint_catchup_contract_digest != catchup.contract_digest {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: String::from("live checkpoint catchup digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != QUANTIZED_OUTER_SYNC_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != QUANTIZED_OUTER_SYNC_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != QUANTIZED_OUTER_SYNC_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != QUANTIZED_OUTER_SYNC_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        let mut policy_ids = BTreeSet::new();
        let mut saw_refused_policy = false;
        for policy in &self.delta_policies {
            if !policy_ids.insert(policy.policy_id.as_str()) {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!("duplicate delta policy `{}`", policy.policy_id),
                });
            }
            if policy.chunk_bytes == 0 || policy.expected_compression_ratio_bps == 0 {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!("delta policy `{}` lost transport sizing", policy.policy_id),
                });
            }
            if policy.delta_family == OuterSyncDeltaFamily::FullPrecisionDenseAllReduce {
                saw_refused_policy = true;
            }
        }
        if !saw_refused_policy {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: String::from(
                    "one full-precision dense all-reduce policy must stay explicit so WAN refusal remains grounded",
                ),
            });
        }

        let mut exchange_ids = BTreeSet::new();
        let mut saw_applied = false;
        let mut saw_refused = false;
        for exchange in &self.exchange_receipts {
            if !exchange_ids.insert(exchange.receipt_id.as_str()) {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!("duplicate outer-sync exchange `{}`", exchange.receipt_id),
                });
            }
            if !record_ids.contains(exchange.source_registry_record_id.as_str())
                || !record_ids.contains(exchange.destination_registry_record_id.as_str())
            {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "outer-sync exchange `{}` references an unknown registry record",
                        exchange.receipt_id
                    ),
                });
            }
            if !active_role_pairs.contains(&(
                exchange.source_registry_record_id.as_str(),
                DecentralizedNetworkRoleClass::PublicMiner,
            )) {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "outer-sync exchange `{}` source `{}` must stay an active public miner",
                        exchange.receipt_id, exchange.source_registry_record_id
                    ),
                });
            }
            if !active_role_pairs.contains(&(
                exchange.destination_registry_record_id.as_str(),
                DecentralizedNetworkRoleClass::CheckpointAuthority,
            )) {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "outer-sync exchange `{}` destination `{}` must stay an active checkpoint authority",
                        exchange.receipt_id, exchange.destination_registry_record_id
                    ),
                });
            }
            let route = route_by_id.get(exchange.route_id.as_str()).ok_or_else(|| {
                QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "outer-sync exchange `{}` lost route `{}`",
                        exchange.receipt_id, exchange.route_id
                    ),
                }
            })?;
            if !route_connects(
                route.src_registry_record_id.as_str(),
                route.dst_registry_record_id.as_str(),
                exchange.source_registry_record_id.as_str(),
                exchange.destination_registry_record_id.as_str(),
            ) {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "outer-sync exchange `{}` route `{}` no longer connects source and destination",
                        exchange.receipt_id, exchange.route_id
                    ),
                });
            }
            let policy = policy_by_id
                .get(exchange.delta_policy_id.as_str())
                .ok_or_else(|| QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "outer-sync exchange `{}` lost delta policy `{}`",
                        exchange.receipt_id, exchange.delta_policy_id
                    ),
                })?;
            let anchor = catchup_by_id
                .get(exchange.epoch_anchor_catchup_receipt_id.as_str())
                .ok_or_else(|| QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "outer-sync exchange `{}` lost catchup anchor `{}`",
                        exchange.receipt_id, exchange.epoch_anchor_catchup_receipt_id
                    ),
                })?;
            if anchor.disposition != CatchupDisposition::Completed {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "outer-sync exchange `{}` must stay anchored to a completed catchup receipt",
                        exchange.receipt_id
                    ),
                });
            }

            match exchange.disposition {
                OuterSyncExchangeDisposition::Applied => {
                    saw_applied = true;
                    if exchange.refusal.is_some()
                        || exchange.compressed_bytes == 0
                        || exchange.compressed_bytes >= exchange.uncompressed_bytes
                        || matches!(
                            policy.delta_family,
                            OuterSyncDeltaFamily::FullPrecisionDenseAllReduce
                        )
                    {
                        return Err(QuantizedOuterSyncContractError::InvalidContract {
                            detail: format!(
                                "applied outer-sync exchange `{}` lost compression semantics",
                                exchange.receipt_id
                            ),
                        });
                    }
                }
                OuterSyncExchangeDisposition::Refused => {
                    saw_refused = true;
                    if exchange.refusal.is_none()
                        || exchange.compressed_bytes != 0
                        || policy.delta_family != OuterSyncDeltaFamily::FullPrecisionDenseAllReduce
                    {
                        return Err(QuantizedOuterSyncContractError::InvalidContract {
                            detail: format!(
                                "refused outer-sync exchange `{}` lost WAN refusal semantics",
                                exchange.receipt_id
                            ),
                        });
                    }
                }
            }
        }
        if !(saw_applied && saw_refused) {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: String::from(
                    "the contract must retain both applied quantized exchanges and at least one refused full-precision WAN exchange",
                ),
            });
        }

        let mut aggregation_ids = BTreeSet::new();
        for aggregation in &self.aggregation_receipts {
            if !aggregation_ids.insert(aggregation.aggregation_id.as_str()) {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "duplicate aggregation receipt `{}`",
                        aggregation.aggregation_id
                    ),
                });
            }
            if aggregation.authority_role_class
                != DecentralizedNetworkRoleClass::CheckpointAuthority
                || !active_role_pairs.contains(&(
                    aggregation.authority_registry_record_id.as_str(),
                    DecentralizedNetworkRoleClass::CheckpointAuthority,
                ))
            {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "aggregation receipt `{}` must stay on an active checkpoint authority",
                        aggregation.aggregation_id
                    ),
                });
            }
            if !advertisement_ids.contains(aggregation.anchored_advertisement_id.as_str()) {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "aggregation receipt `{}` lost advertisement anchor `{}`",
                        aggregation.aggregation_id, aggregation.anchored_advertisement_id
                    ),
                });
            }
            let mut computed_bandwidth = 0_u64;
            for input_receipt_id in &aggregation.input_receipt_ids {
                let exchange = exchange_by_id
                    .get(input_receipt_id.as_str())
                    .ok_or_else(|| QuantizedOuterSyncContractError::InvalidContract {
                        detail: format!(
                            "aggregation receipt `{}` lost exchange input `{}`",
                            aggregation.aggregation_id, input_receipt_id
                        ),
                    })?;
                if exchange.disposition != OuterSyncExchangeDisposition::Applied {
                    return Err(QuantizedOuterSyncContractError::InvalidContract {
                        detail: format!(
                            "aggregation receipt `{}` cannot include refused exchange `{}`",
                            aggregation.aggregation_id, input_receipt_id
                        ),
                    });
                }
                computed_bandwidth += exchange.compressed_bytes;
            }
            if aggregation.bandwidth_accounted_bytes != computed_bandwidth {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "aggregation receipt `{}` bandwidth accounting drifted",
                        aggregation.aggregation_id
                    ),
                });
            }
        }

        let mut correctness_ids = BTreeSet::new();
        for correctness in &self.correctness_receipts {
            if !correctness_ids.insert(correctness.receipt_id.as_str()) {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!("duplicate correctness receipt `{}`", correctness.receipt_id),
                });
            }
            let policy = policy_by_id
                .get(correctness.delta_policy_id.as_str())
                .ok_or_else(|| QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "correctness receipt `{}` lost delta policy `{}`",
                        correctness.receipt_id, correctness.delta_policy_id
                    ),
                })?;
            let exchange = exchange_by_id
                .get(correctness.exchange_receipt_id.as_str())
                .ok_or_else(|| QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "correctness receipt `{}` lost exchange `{}`",
                        correctness.receipt_id, correctness.exchange_receipt_id
                    ),
                })?;
            if exchange.delta_policy_id != correctness.delta_policy_id
                || exchange.disposition != OuterSyncExchangeDisposition::Applied
                || policy.delta_family == OuterSyncDeltaFamily::FullPrecisionDenseAllReduce
                || !correctness.checksum_match
            {
                return Err(QuantizedOuterSyncContractError::InvalidContract {
                    detail: format!(
                        "correctness receipt `{}` lost quantized validation semantics",
                        correctness.receipt_id
                    ),
                });
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(QuantizedOuterSyncContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

static QUANTIZED_OUTER_SYNC_CONTRACT_CACHE: std::sync::OnceLock<QuantizedOuterSyncContract> =
    std::sync::OnceLock::new();

pub fn canonical_quantized_outer_sync_contract(
) -> Result<QuantizedOuterSyncContract, QuantizedOuterSyncContractError> {
    if let Some(contract) = QUANTIZED_OUTER_SYNC_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let mesh = canonical_elastic_device_mesh_contract()?;
    let wan = canonical_wan_overlay_route_contract()?;
    let catchup = canonical_live_checkpoint_catchup_contract()?;

    let delta_policies = vec![
        delta_policy(
            "policy.int8_pseudogradient_outer_sync",
            OuterSyncDeltaFamily::QuantizedPseudoGradient,
            OuterSyncQuantizationKind::Int8Blockwise,
            4_194_304,
            2_200,
            true,
            "Google emits blockwise int8 pseudo-gradients so WAN exchange stays smaller than the checkpoint payload and can run through the direct checkpoint-authority lane.",
        ),
        delta_policy(
            "policy.nf4_residual_outer_sync",
            OuterSyncDeltaFamily::QuantizedResidualDelta,
            OuterSyncQuantizationKind::Nf4Residual,
            2_097_152,
            1_400,
            true,
            "Apple MLX emits NF4 residual deltas so the newly rejoined miner can contribute over the overlay route without pretending full-density sync is free.",
        ),
        delta_policy(
            "policy.fp16_dense_allreduce_refused",
            OuterSyncDeltaFamily::FullPrecisionDenseAllReduce,
            OuterSyncQuantizationKind::Fp16Uncompressed,
            8_388_608,
            10_000,
            false,
            "The old dense all-reduce story remains explicit as a refusal target for WAN links rather than an implied fallback.",
        ),
    ];

    let exchange_receipts = vec![
        OuterSyncExchangeReceipt {
            receipt_id: String::from("exchange.public_miner.google_to_runpod.int8.1"),
            epoch_anchor_catchup_receipt_id: String::from("catchup.public_miner.local_mlx.after_deathrattle"),
            source_registry_record_id: String::from("google_l4_validator_node.registry"),
            destination_registry_record_id: String::from("runpod_8xh100_dense_node.registry"),
            route_id: String::from("route.checkpoint_authority.google_runpod.direct"),
            delta_policy_id: String::from("policy.int8_pseudogradient_outer_sync"),
            uncompressed_bytes: 100_663_296,
            compressed_bytes: 20_971_520,
            round_trip_latency_ms: 34,
            disposition: OuterSyncExchangeDisposition::Applied,
            refusal: None,
            detail: String::from(
                "Google contributes a quantized pseudo-gradient package directly to the RunPod checkpoint authority after the MLX replacement catch-up closes.",
            ),
        },
        OuterSyncExchangeReceipt {
            receipt_id: String::from("exchange.public_miner.local_mlx_to_runpod.nf4.1"),
            epoch_anchor_catchup_receipt_id: String::from("catchup.public_miner.local_mlx.after_deathrattle"),
            source_registry_record_id: String::from("local_mlx_mac_workstation.registry"),
            destination_registry_record_id: String::from("runpod_8xh100_dense_node.registry"),
            route_id: String::from("route.checkpoint_authority.local_mlx_runpod.overlay"),
            delta_policy_id: String::from("policy.nf4_residual_outer_sync"),
            uncompressed_bytes: 67_108_864,
            compressed_bytes: 8_388_608,
            round_trip_latency_ms: 62,
            disposition: OuterSyncExchangeDisposition::Applied,
            refusal: None,
            detail: String::from(
                "Apple MLX contributes an NF4 residual package over the overlay route so the rejoined miner has a WAN-feasible outer-sync lane.",
            ),
        },
        OuterSyncExchangeReceipt {
            receipt_id: String::from("exchange.public_miner.local_mlx_to_runpod.fp16_refused.1"),
            epoch_anchor_catchup_receipt_id: String::from("catchup.public_miner.local_mlx.after_deathrattle"),
            source_registry_record_id: String::from("local_mlx_mac_workstation.registry"),
            destination_registry_record_id: String::from("runpod_8xh100_dense_node.registry"),
            route_id: String::from("route.checkpoint_authority.local_mlx_runpod.overlay"),
            delta_policy_id: String::from("policy.fp16_dense_allreduce_refused"),
            uncompressed_bytes: 201_326_592,
            compressed_bytes: 0,
            round_trip_latency_ms: 62,
            disposition: OuterSyncExchangeDisposition::Refused,
            refusal: Some(String::from(
                "full-precision dense all-reduce over the WAN overlay is not admitted once the public path leaves provider-local networking",
            )),
            detail: String::from(
                "The contract keeps one honest refusal explicit so later reports cannot quietly upgrade WAN outer sync into dense all-reduce closure.",
            ),
        },
    ];

    let aggregation_receipts = vec![OuterSyncAggregationReceipt {
        aggregation_id: String::from("aggregation.outer_sync.runpod.round_2056"),
        authority_registry_record_id: String::from("runpod_8xh100_dense_node.registry"),
        authority_role_class: DecentralizedNetworkRoleClass::CheckpointAuthority,
        input_receipt_ids: vec![
            String::from("exchange.public_miner.google_to_runpod.int8.1"),
            String::from("exchange.public_miner.local_mlx_to_runpod.nf4.1"),
        ],
        anchored_advertisement_id: String::from("advertisement.checkpoint_authority.runpod.primary"),
        bandwidth_accounted_bytes: 29_360_128,
        published_checkpoint_step: 2_056,
        detail: String::from(
            "RunPod aggregates the two admitted quantized exchanges and publishes the next checkpoint-authority step after accounting for the actual compressed bytes transferred.",
        ),
    }];

    let correctness_receipts = vec![
        OuterSyncCorrectnessReceipt {
            receipt_id: String::from("correctness.int8_pseudogradient.google_to_runpod.1"),
            delta_policy_id: String::from("policy.int8_pseudogradient_outer_sync"),
            exchange_receipt_id: String::from("exchange.public_miner.google_to_runpod.int8.1"),
            max_abs_error_ppm: 840,
            checksum_match: true,
            detail: String::from(
                "The int8 pseudo-gradient path stays under the admitted quantization error budget and matches the retained checksum family.",
            ),
        },
        OuterSyncCorrectnessReceipt {
            receipt_id: String::from("correctness.nf4_residual.local_mlx_to_runpod.1"),
            delta_policy_id: String::from("policy.nf4_residual_outer_sync"),
            exchange_receipt_id: String::from("exchange.public_miner.local_mlx_to_runpod.nf4.1"),
            max_abs_error_ppm: 1_260,
            checksum_match: true,
            detail: String::from(
                "The NF4 residual path stays inside the admitted correctness envelope for WAN outer sync.",
            ),
        },
    ];

    let mut contract = QuantizedOuterSyncContract {
        schema_version: String::from(QUANTIZED_OUTER_SYNC_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(QUANTIZED_OUTER_SYNC_CONTRACT_ID),
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        elastic_device_mesh_contract_digest: mesh.contract_digest.clone(),
        wan_overlay_route_contract_digest: wan.contract_digest.clone(),
        live_checkpoint_catchup_contract_digest: catchup.contract_digest.clone(),
        delta_policies,
        exchange_receipts,
        aggregation_receipts,
        correctness_receipts,
        authority_paths: QuantizedOuterSyncAuthorityPaths {
            fixture_path: String::from(QUANTIZED_OUTER_SYNC_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(QUANTIZED_OUTER_SYNC_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(QUANTIZED_OUTER_SYNC_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(QUANTIZED_OUTER_SYNC_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first WAN-aware outer-sync surface above live catch-up: quantized delta policies, applied exchange receipts, checkpoint-authority aggregation, correctness receipts, bandwidth accounting, and one explicit refusal for full-precision dense all-reduce over WAN paths. It does not yet claim public internet fault or soak closure.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = QUANTIZED_OUTER_SYNC_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_quantized_outer_sync_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), QuantizedOuterSyncContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| QuantizedOuterSyncContractError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_quantized_outer_sync_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| QuantizedOuterSyncContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn delta_policy(
    policy_id: &str,
    delta_family: OuterSyncDeltaFamily,
    quantization_kind: OuterSyncQuantizationKind,
    chunk_bytes: u32,
    expected_compression_ratio_bps: u16,
    cpu_offload_required: bool,
    detail: &str,
) -> OuterSyncDeltaPolicy {
    OuterSyncDeltaPolicy {
        policy_id: String::from(policy_id),
        delta_family,
        quantization_kind,
        chunk_bytes,
        expected_compression_ratio_bps,
        cpu_offload_required,
        detail: String::from(detail),
    }
}

fn route_connects(route_src: &str, route_dst: &str, source: &str, destination: &str) -> bool {
    (route_src == source && route_dst == destination)
        || (route_src == destination && route_dst == source)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for outer sync contract"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_quantized_outer_sync_contract, OuterSyncExchangeDisposition,
        QuantizedOuterSyncContractError,
    };

    #[test]
    fn canonical_quantized_outer_sync_contract_is_valid(
    ) -> Result<(), QuantizedOuterSyncContractError> {
        let contract = canonical_quantized_outer_sync_contract()?;
        contract.validate()
    }

    #[test]
    fn full_precision_wan_path_must_stay_refused() -> Result<(), QuantizedOuterSyncContractError> {
        let mut contract = canonical_quantized_outer_sync_contract()?;
        let refused = contract
            .exchange_receipts
            .iter_mut()
            .find(|exchange| {
                exchange.receipt_id == "exchange.public_miner.local_mlx_to_runpod.fp16_refused.1"
            })
            .expect("canonical refused exchange must exist");
        refused.disposition = OuterSyncExchangeDisposition::Applied;
        refused.refusal = None;
        refused.compressed_bytes = 67_108_864;
        let error = contract
            .validate()
            .expect_err("full-precision wan exchange cannot flip to applied");
        assert!(matches!(
            error,
            QuantizedOuterSyncContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
