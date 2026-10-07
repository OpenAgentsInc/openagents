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
    canonical_public_network_registry_contract, canonical_quantized_outer_sync_contract,
    canonical_wan_overlay_route_contract, CatchupDisposition, DecentralizedNetworkRoleClass,
    ElasticDeviceMeshContractError, ElasticMeshLeaseStatus, LiveCheckpointCatchupContractError,
    OuterSyncExchangeDisposition, PublicNetworkRegistryContractError,
    QuantizedOuterSyncContractError, WanOverlayRouteContractError,
    PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID,
};

pub const INTERNET_FAULT_HARNESS_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.internet_fault_harness_contract.v1";
pub const INTERNET_FAULT_HARNESS_CONTRACT_ID: &str = "psionic.internet_fault_harness_contract.v1";
pub const INTERNET_FAULT_HARNESS_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/internet_fault_harness_contract_v1.json";
pub const INTERNET_FAULT_HARNESS_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-internet-fault-harness-contract.sh";
pub const INTERNET_FAULT_HARNESS_CONTRACT_DOC_PATH: &str =
    "docs/INTERNET_FAULT_HARNESS_REFERENCE.md";
pub const INTERNET_FAULT_HARNESS_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum InternetFaultHarnessContractError {
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
    QuantizedOuterSync(#[from] QuantizedOuterSyncContractError),
    #[error(transparent)]
    WanRoute(#[from] WanOverlayRouteContractError),
    #[error("internet fault harness contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InternetFaultProfileKind {
    PacketLossFailover,
    DelayedCheckpointCatchup,
    BandwidthThrottleOuterSync,
    ValidatorLossHold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InternetFaultRunDisposition {
    Passed,
    Held,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternetFaultProfile {
    pub profile_id: String,
    pub profile_kind: InternetFaultProfileKind,
    pub injected_latency_ms: u16,
    pub packet_loss_bps: u16,
    pub bandwidth_limit_mbps: u16,
    pub delayed_checkpoint_ms: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_loss_registry_record_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternetThroughputBaseline {
    pub baseline_id: String,
    pub route_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referenced_catchup_receipt_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referenced_outer_sync_receipt_id: Option<String>,
    pub sustained_bandwidth_mbps: u16,
    pub maximum_latency_ms: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternetSoakSuite {
    pub suite_id: String,
    pub profile_ids: Vec<String>,
    pub minimum_duration_seconds: u32,
    pub required_pass_count: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternetFaultRunReceipt {
    pub receipt_id: String,
    pub suite_id: String,
    pub profile_id: String,
    pub observed_failover_receipt_ids: Vec<String>,
    pub observed_catchup_receipt_ids: Vec<String>,
    pub observed_outer_sync_receipt_ids: Vec<String>,
    pub observed_aggregation_receipt_ids: Vec<String>,
    pub validator_quorum_preserved: bool,
    pub disposition: InternetFaultRunDisposition,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternetFaultHarnessAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternetFaultHarnessContract {
    pub schema_version: String,
    pub contract_id: String,
    pub current_epoch_id: String,
    pub elastic_device_mesh_contract_digest: String,
    pub wan_overlay_route_contract_digest: String,
    pub live_checkpoint_catchup_contract_digest: String,
    pub quantized_outer_sync_contract_digest: String,
    pub fault_profiles: Vec<InternetFaultProfile>,
    pub throughput_baselines: Vec<InternetThroughputBaseline>,
    pub soak_suites: Vec<InternetSoakSuite>,
    pub run_receipts: Vec<InternetFaultRunReceipt>,
    pub authority_paths: InternetFaultHarnessAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl InternetFaultHarnessContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_internet_fault_harness_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), InternetFaultHarnessContractError> {
        let mesh = canonical_elastic_device_mesh_contract()?;
        let registry = canonical_public_network_registry_contract()?;
        let wan = canonical_wan_overlay_route_contract()?;
        let catchup = canonical_live_checkpoint_catchup_contract()?;
        let outer_sync = canonical_quantized_outer_sync_contract()?;

        let active_role_pairs = mesh
            .member_leases
            .iter()
            .filter(|lease| lease.status == ElasticMeshLeaseStatus::Active)
            .map(|lease| (lease.registry_record_id.as_str(), lease.role_class))
            .collect::<BTreeSet<_>>();
        let record_ids = registry
            .registry_records
            .iter()
            .map(|record| record.registry_record_id.as_str())
            .collect::<BTreeSet<_>>();
        let route_ids = wan
            .route_records
            .iter()
            .map(|route| route.route_id.as_str())
            .collect::<BTreeSet<_>>();
        let failover_ids = wan
            .failover_receipts
            .iter()
            .map(|receipt| receipt.receipt_id.as_str())
            .collect::<BTreeSet<_>>();
        let catchup_by_id = catchup
            .catchup_receipts
            .iter()
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let outer_sync_by_id = outer_sync
            .exchange_receipts
            .iter()
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let aggregation_ids = outer_sync
            .aggregation_receipts
            .iter()
            .map(|receipt| receipt.aggregation_id.as_str())
            .collect::<BTreeSet<_>>();
        let profile_by_id = self
            .fault_profiles
            .iter()
            .map(|profile| (profile.profile_id.as_str(), profile))
            .collect::<BTreeMap<_, _>>();
        let suite_by_id = self
            .soak_suites
            .iter()
            .map(|suite| (suite.suite_id.as_str(), suite))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != INTERNET_FAULT_HARNESS_CONTRACT_SCHEMA_VERSION {
            return Err(InternetFaultHarnessContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    INTERNET_FAULT_HARNESS_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != INTERNET_FAULT_HARNESS_CONTRACT_ID {
            return Err(InternetFaultHarnessContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.current_epoch_id != PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID {
            return Err(InternetFaultHarnessContractError::InvalidContract {
                detail: String::from("current_epoch_id drifted"),
            });
        }
        if self.elastic_device_mesh_contract_digest != mesh.contract_digest {
            return Err(InternetFaultHarnessContractError::InvalidContract {
                detail: String::from("elastic device mesh digest drifted"),
            });
        }
        if self.wan_overlay_route_contract_digest != wan.contract_digest {
            return Err(InternetFaultHarnessContractError::InvalidContract {
                detail: String::from("wan overlay route digest drifted"),
            });
        }
        if self.live_checkpoint_catchup_contract_digest != catchup.contract_digest {
            return Err(InternetFaultHarnessContractError::InvalidContract {
                detail: String::from("live checkpoint catchup digest drifted"),
            });
        }
        if self.quantized_outer_sync_contract_digest != outer_sync.contract_digest {
            return Err(InternetFaultHarnessContractError::InvalidContract {
                detail: String::from("quantized outer sync digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != INTERNET_FAULT_HARNESS_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != INTERNET_FAULT_HARNESS_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != INTERNET_FAULT_HARNESS_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != INTERNET_FAULT_HARNESS_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(InternetFaultHarnessContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        let mut profile_ids = BTreeSet::new();
        for profile in &self.fault_profiles {
            if !profile_ids.insert(profile.profile_id.as_str()) {
                return Err(InternetFaultHarnessContractError::InvalidContract {
                    detail: format!("duplicate fault profile `{}`", profile.profile_id),
                });
            }
            if profile.bandwidth_limit_mbps == 0 {
                return Err(InternetFaultHarnessContractError::InvalidContract {
                    detail: format!(
                        "fault profile `{}` lost bandwidth limit",
                        profile.profile_id
                    ),
                });
            }
            match profile.profile_kind {
                InternetFaultProfileKind::PacketLossFailover => {
                    if profile.packet_loss_bps == 0 {
                        return Err(InternetFaultHarnessContractError::InvalidContract {
                            detail: format!(
                                "packet-loss profile `{}` must keep non-zero packet loss",
                                profile.profile_id
                            ),
                        });
                    }
                }
                InternetFaultProfileKind::DelayedCheckpointCatchup => {
                    if profile.delayed_checkpoint_ms == 0 {
                        return Err(InternetFaultHarnessContractError::InvalidContract {
                            detail: format!(
                                "delayed-catchup profile `{}` must keep checkpoint delay",
                                profile.profile_id
                            ),
                        });
                    }
                }
                InternetFaultProfileKind::BandwidthThrottleOuterSync => {}
                InternetFaultProfileKind::ValidatorLossHold => {
                    let validator_loss_registry_record_id = profile
                        .validator_loss_registry_record_id
                        .as_deref()
                        .ok_or_else(|| InternetFaultHarnessContractError::InvalidContract {
                            detail: format!(
                                "validator-loss profile `{}` must name the lost validator",
                                profile.profile_id
                            ),
                        })?;
                    if !record_ids.contains(validator_loss_registry_record_id)
                        || !active_role_pairs.contains(&(
                            validator_loss_registry_record_id,
                            DecentralizedNetworkRoleClass::PublicValidator,
                        ))
                    {
                        return Err(InternetFaultHarnessContractError::InvalidContract {
                            detail: format!(
                                "validator-loss profile `{}` lost validator binding `{}`",
                                profile.profile_id, validator_loss_registry_record_id
                            ),
                        });
                    }
                }
            }
        }

        let mut baseline_ids = BTreeSet::new();
        for baseline in &self.throughput_baselines {
            if !baseline_ids.insert(baseline.baseline_id.as_str()) {
                return Err(InternetFaultHarnessContractError::InvalidContract {
                    detail: format!("duplicate throughput baseline `{}`", baseline.baseline_id),
                });
            }
            if !route_ids.contains(baseline.route_id.as_str())
                || baseline.sustained_bandwidth_mbps == 0
                || baseline.maximum_latency_ms == 0
            {
                return Err(InternetFaultHarnessContractError::InvalidContract {
                    detail: format!("throughput baseline `{}` drifted", baseline.baseline_id),
                });
            }
            if let Some(catchup_receipt_id) = baseline.referenced_catchup_receipt_id.as_deref() {
                let receipt = catchup_by_id.get(catchup_receipt_id).ok_or_else(|| {
                    InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "throughput baseline `{}` lost catchup receipt `{}`",
                            baseline.baseline_id, catchup_receipt_id
                        ),
                    }
                })?;
                if receipt.disposition != CatchupDisposition::Completed {
                    return Err(InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "throughput baseline `{}` must stay bound to a completed catchup receipt",
                            baseline.baseline_id
                        ),
                    });
                }
            }
            if let Some(exchange_receipt_id) = baseline.referenced_outer_sync_receipt_id.as_deref()
            {
                let receipt = outer_sync_by_id.get(exchange_receipt_id).ok_or_else(|| {
                    InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "throughput baseline `{}` lost outer-sync receipt `{}`",
                            baseline.baseline_id, exchange_receipt_id
                        ),
                    }
                })?;
                if receipt.disposition != OuterSyncExchangeDisposition::Applied {
                    return Err(InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "throughput baseline `{}` must stay bound to an applied outer-sync receipt",
                            baseline.baseline_id
                        ),
                    });
                }
            }
        }

        let mut suite_ids = BTreeSet::new();
        for suite in &self.soak_suites {
            if !suite_ids.insert(suite.suite_id.as_str()) {
                return Err(InternetFaultHarnessContractError::InvalidContract {
                    detail: format!("duplicate soak suite `{}`", suite.suite_id),
                });
            }
            if suite.minimum_duration_seconds == 0 || suite.required_pass_count == 0 {
                return Err(InternetFaultHarnessContractError::InvalidContract {
                    detail: format!(
                        "soak suite `{}` lost duration or pass threshold",
                        suite.suite_id
                    ),
                });
            }
            for profile_id in &suite.profile_ids {
                if !profile_by_id.contains_key(profile_id.as_str()) {
                    return Err(InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "soak suite `{}` references unknown profile `{}`",
                            suite.suite_id, profile_id
                        ),
                    });
                }
            }
        }

        let mut receipt_ids = BTreeSet::new();
        let mut pass_count_by_suite = BTreeMap::<&str, u16>::new();
        for receipt in &self.run_receipts {
            if !receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(InternetFaultHarnessContractError::InvalidContract {
                    detail: format!("duplicate run receipt `{}`", receipt.receipt_id),
                });
            }
            let suite = suite_by_id.get(receipt.suite_id.as_str()).ok_or_else(|| {
                InternetFaultHarnessContractError::InvalidContract {
                    detail: format!(
                        "run receipt `{}` references unknown suite `{}`",
                        receipt.receipt_id, receipt.suite_id
                    ),
                }
            })?;
            let profile = profile_by_id
                .get(receipt.profile_id.as_str())
                .ok_or_else(|| InternetFaultHarnessContractError::InvalidContract {
                    detail: format!(
                        "run receipt `{}` references unknown profile `{}`",
                        receipt.receipt_id, receipt.profile_id
                    ),
                })?;
            if !suite.profile_ids.contains(&receipt.profile_id) {
                return Err(InternetFaultHarnessContractError::InvalidContract {
                    detail: format!(
                        "run receipt `{}` references profile `{}` outside suite `{}`",
                        receipt.receipt_id, receipt.profile_id, receipt.suite_id
                    ),
                });
            }
            for failover_receipt_id in &receipt.observed_failover_receipt_ids {
                if !failover_ids.contains(failover_receipt_id.as_str()) {
                    return Err(InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "run receipt `{}` lost failover receipt `{}`",
                            receipt.receipt_id, failover_receipt_id
                        ),
                    });
                }
            }
            for catchup_receipt_id in &receipt.observed_catchup_receipt_ids {
                let catchup_receipt =
                    catchup_by_id
                        .get(catchup_receipt_id.as_str())
                        .ok_or_else(|| InternetFaultHarnessContractError::InvalidContract {
                            detail: format!(
                                "run receipt `{}` lost catchup receipt `{}`",
                                receipt.receipt_id, catchup_receipt_id
                            ),
                        })?;
                if catchup_receipt.disposition != CatchupDisposition::Completed {
                    return Err(InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "run receipt `{}` must only count completed catchup receipts as recovered evidence",
                            receipt.receipt_id
                        ),
                    });
                }
            }
            for outer_sync_receipt_id in &receipt.observed_outer_sync_receipt_ids {
                let outer_sync_receipt = outer_sync_by_id
                    .get(outer_sync_receipt_id.as_str())
                    .ok_or_else(|| InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "run receipt `{}` lost outer-sync receipt `{}`",
                            receipt.receipt_id, outer_sync_receipt_id
                        ),
                    })?;
                if outer_sync_receipt.disposition != OuterSyncExchangeDisposition::Applied {
                    return Err(InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "run receipt `{}` must only count applied outer-sync receipts",
                            receipt.receipt_id
                        ),
                    });
                }
            }
            for aggregation_receipt_id in &receipt.observed_aggregation_receipt_ids {
                if !aggregation_ids.contains(aggregation_receipt_id.as_str()) {
                    return Err(InternetFaultHarnessContractError::InvalidContract {
                        detail: format!(
                            "run receipt `{}` lost aggregation receipt `{}`",
                            receipt.receipt_id, aggregation_receipt_id
                        ),
                    });
                }
            }

            match profile.profile_kind {
                InternetFaultProfileKind::PacketLossFailover => {
                    if receipt.observed_failover_receipt_ids.is_empty()
                        || receipt.disposition != InternetFaultRunDisposition::Passed
                    {
                        return Err(InternetFaultHarnessContractError::InvalidContract {
                            detail: format!(
                                "packet-loss run receipt `{}` must keep a passed failover result",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
                InternetFaultProfileKind::DelayedCheckpointCatchup => {
                    if receipt.observed_catchup_receipt_ids.is_empty()
                        || receipt.disposition != InternetFaultRunDisposition::Passed
                    {
                        return Err(InternetFaultHarnessContractError::InvalidContract {
                            detail: format!(
                                "delayed-catchup run receipt `{}` must keep a passed catchup result",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
                InternetFaultProfileKind::BandwidthThrottleOuterSync => {
                    if receipt.observed_outer_sync_receipt_ids.is_empty()
                        || receipt.observed_aggregation_receipt_ids.is_empty()
                        || receipt.disposition != InternetFaultRunDisposition::Passed
                    {
                        return Err(InternetFaultHarnessContractError::InvalidContract {
                            detail: format!(
                                "bandwidth-throttle run receipt `{}` must keep passed outer-sync evidence",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
                InternetFaultProfileKind::ValidatorLossHold => {
                    if receipt.validator_quorum_preserved
                        || receipt.disposition != InternetFaultRunDisposition::Held
                    {
                        return Err(InternetFaultHarnessContractError::InvalidContract {
                            detail: format!(
                                "validator-loss run receipt `{}` must stay held with quorum broken",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
            }

            if receipt.disposition == InternetFaultRunDisposition::Passed {
                *pass_count_by_suite
                    .entry(receipt.suite_id.as_str())
                    .or_default() += 1;
            }
        }

        for suite in &self.soak_suites {
            let observed_pass_count = pass_count_by_suite
                .get(suite.suite_id.as_str())
                .copied()
                .unwrap_or_default();
            if observed_pass_count < suite.required_pass_count {
                return Err(InternetFaultHarnessContractError::InvalidContract {
                    detail: format!(
                        "soak suite `{}` retained only {} passed runs but requires at least {}",
                        suite.suite_id, observed_pass_count, suite.required_pass_count
                    ),
                });
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(InternetFaultHarnessContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_internet_fault_harness_contract(
) -> Result<InternetFaultHarnessContract, InternetFaultHarnessContractError> {
    let mesh = canonical_elastic_device_mesh_contract()?;
    let wan = canonical_wan_overlay_route_contract()?;
    let catchup = canonical_live_checkpoint_catchup_contract()?;
    let outer_sync = canonical_quantized_outer_sync_contract()?;

    let fault_profiles = vec![
        InternetFaultProfile {
            profile_id: String::from("profile.packet_loss_public_miner_failover"),
            profile_kind: InternetFaultProfileKind::PacketLossFailover,
            injected_latency_ms: 120,
            packet_loss_bps: 450,
            bandwidth_limit_mbps: 600,
            delayed_checkpoint_ms: 0,
            validator_loss_registry_record_id: None,
            detail: String::from(
                "The packet-loss failover profile degrades the local public-miner path until the relay-only route must promote the overlay fallback.",
            ),
        },
        InternetFaultProfile {
            profile_id: String::from("profile.delayed_checkpoint_catchup"),
            profile_kind: InternetFaultProfileKind::DelayedCheckpointCatchup,
            injected_latency_ms: 85,
            packet_loss_bps: 25,
            bandwidth_limit_mbps: 800,
            delayed_checkpoint_ms: 4_500,
            validator_loss_registry_record_id: None,
            detail: String::from(
                "The delayed-catchup profile stretches checkpoint delivery without changing the admitted route so the join-time resume window has to stay honest.",
            ),
        },
        InternetFaultProfile {
            profile_id: String::from("profile.bandwidth_throttle_outer_sync"),
            profile_kind: InternetFaultProfileKind::BandwidthThrottleOuterSync,
            injected_latency_ms: 70,
            packet_loss_bps: 15,
            bandwidth_limit_mbps: 160,
            delayed_checkpoint_ms: 0,
            validator_loss_registry_record_id: None,
            detail: String::from(
                "The bandwidth-throttle profile squeezes the WAN lane hard enough that only the admitted quantized outer-sync receipts still pass.",
            ),
        },
        InternetFaultProfile {
            profile_id: String::from("profile.validator_loss_hold"),
            profile_kind: InternetFaultProfileKind::ValidatorLossHold,
            injected_latency_ms: 60,
            packet_loss_bps: 5,
            bandwidth_limit_mbps: 1_000,
            delayed_checkpoint_ms: 0,
            validator_loss_registry_record_id: Some(String::from("local_mlx_mac_workstation.registry")),
            detail: String::from(
                "The validator-loss profile removes the Apple MLX validator from the current two-validator quorum and proves the run holds instead of pretending quorum survived.",
            ),
        },
    ];

    let throughput_baselines = vec![
        InternetThroughputBaseline {
            baseline_id: String::from("baseline.catchup.local_mlx_runpod.overlay"),
            route_id: String::from("route.checkpoint_authority.local_mlx_runpod.overlay"),
            referenced_catchup_receipt_id: Some(String::from(
                "catchup.public_miner.local_mlx.after_deathrattle",
            )),
            referenced_outer_sync_receipt_id: None,
            sustained_bandwidth_mbps: 910,
            maximum_latency_ms: 70,
            detail: String::from(
                "The overlay catch-up path between Apple MLX and RunPod defines the current join-time recovery baseline.",
            ),
        },
        InternetThroughputBaseline {
            baseline_id: String::from("baseline.outer_sync.google_runpod.direct"),
            route_id: String::from("route.checkpoint_authority.google_runpod.direct"),
            referenced_catchup_receipt_id: None,
            referenced_outer_sync_receipt_id: Some(String::from(
                "exchange.public_miner.google_to_runpod.int8.1",
            )),
            sustained_bandwidth_mbps: 6_170,
            maximum_latency_ms: 40,
            detail: String::from(
                "The Google-to-RunPod direct lane defines the high-water WAN baseline for the int8 outer-sync path.",
            ),
        },
        InternetThroughputBaseline {
            baseline_id: String::from("baseline.outer_sync.local_mlx_runpod.overlay"),
            route_id: String::from("route.checkpoint_authority.local_mlx_runpod.overlay"),
            referenced_catchup_receipt_id: None,
            referenced_outer_sync_receipt_id: Some(String::from(
                "exchange.public_miner.local_mlx_to_runpod.nf4.1",
            )),
            sustained_bandwidth_mbps: 820,
            maximum_latency_ms: 70,
            detail: String::from(
                "The Apple MLX overlay lane defines the admitted lower-bandwidth baseline for the NF4 outer-sync path.",
            ),
        },
    ];

    let soak_suites = vec![
        InternetSoakSuite {
            suite_id: String::from("suite.internet_fault_matrix_day"),
            profile_ids: vec![
                String::from("profile.packet_loss_public_miner_failover"),
                String::from("profile.delayed_checkpoint_catchup"),
                String::from("profile.bandwidth_throttle_outer_sync"),
                String::from("profile.validator_loss_hold"),
            ],
            minimum_duration_seconds: 7_200,
            required_pass_count: 3,
            detail: String::from(
                "The day fault matrix demands passed evidence for failover, catch-up, and outer sync while still retaining one held validator-loss case.",
            ),
        },
        InternetSoakSuite {
            suite_id: String::from("suite.internet_soak_night"),
            profile_ids: vec![
                String::from("profile.packet_loss_public_miner_failover"),
                String::from("profile.delayed_checkpoint_catchup"),
                String::from("profile.bandwidth_throttle_outer_sync"),
            ],
            minimum_duration_seconds: 28_800,
            required_pass_count: 3,
            detail: String::from(
                "The night soak suite repeats the admitted non-quorum-breaking profiles long enough that decentralized-runtime claims cannot rest on a single daytime pass.",
            ),
        },
    ];

    let run_receipts = vec![
        InternetFaultRunReceipt {
            receipt_id: String::from("run.packet_loss_failover.day1"),
            suite_id: String::from("suite.internet_fault_matrix_day"),
            profile_id: String::from("profile.packet_loss_public_miner_failover"),
            observed_failover_receipt_ids: vec![String::from(
                "failover.public_miner.local_rtx4080_local_mlx.1",
            )],
            observed_catchup_receipt_ids: Vec::new(),
            observed_outer_sync_receipt_ids: Vec::new(),
            observed_aggregation_receipt_ids: Vec::new(),
            validator_quorum_preserved: true,
            disposition: InternetFaultRunDisposition::Passed,
            detail: String::from(
                "Day fault-matrix run: packet loss crossed the relay threshold and the overlay failover receipt closed without losing the active run.",
            ),
        },
        InternetFaultRunReceipt {
            receipt_id: String::from("run.delayed_checkpoint_catchup.day1"),
            suite_id: String::from("suite.internet_fault_matrix_day"),
            profile_id: String::from("profile.delayed_checkpoint_catchup"),
            observed_failover_receipt_ids: Vec::new(),
            observed_catchup_receipt_ids: vec![String::from(
                "catchup.public_miner.local_mlx.after_deathrattle",
            )],
            observed_outer_sync_receipt_ids: Vec::new(),
            observed_aggregation_receipt_ids: Vec::new(),
            validator_quorum_preserved: true,
            disposition: InternetFaultRunDisposition::Passed,
            detail: String::from(
                "Day fault-matrix run: delayed checkpoint delivery still closed inside the admitted live-join window.",
            ),
        },
        InternetFaultRunReceipt {
            receipt_id: String::from("run.bandwidth_throttle_outer_sync.day1"),
            suite_id: String::from("suite.internet_fault_matrix_day"),
            profile_id: String::from("profile.bandwidth_throttle_outer_sync"),
            observed_failover_receipt_ids: Vec::new(),
            observed_catchup_receipt_ids: Vec::new(),
            observed_outer_sync_receipt_ids: vec![
                String::from("exchange.public_miner.google_to_runpod.int8.1"),
                String::from("exchange.public_miner.local_mlx_to_runpod.nf4.1"),
            ],
            observed_aggregation_receipt_ids: vec![String::from(
                "aggregation.outer_sync.runpod.round_2056",
            )],
            validator_quorum_preserved: true,
            disposition: InternetFaultRunDisposition::Passed,
            detail: String::from(
                "Day fault-matrix run: the throttled WAN lane still closed through the admitted quantized outer-sync path and published the aggregated checkpoint step.",
            ),
        },
        InternetFaultRunReceipt {
            receipt_id: String::from("run.validator_loss_hold.day1"),
            suite_id: String::from("suite.internet_fault_matrix_day"),
            profile_id: String::from("profile.validator_loss_hold"),
            observed_failover_receipt_ids: Vec::new(),
            observed_catchup_receipt_ids: Vec::new(),
            observed_outer_sync_receipt_ids: Vec::new(),
            observed_aggregation_receipt_ids: Vec::new(),
            validator_quorum_preserved: false,
            disposition: InternetFaultRunDisposition::Held,
            detail: String::from(
                "Day fault-matrix run: removing the MLX validator broke the two-validator quorum and the run held at the barrier instead of pretending validation continuity survived.",
            ),
        },
        InternetFaultRunReceipt {
            receipt_id: String::from("run.packet_loss_failover.night1"),
            suite_id: String::from("suite.internet_soak_night"),
            profile_id: String::from("profile.packet_loss_public_miner_failover"),
            observed_failover_receipt_ids: vec![String::from(
                "failover.public_miner.local_rtx4080_local_mlx.1",
            )],
            observed_catchup_receipt_ids: Vec::new(),
            observed_outer_sync_receipt_ids: Vec::new(),
            observed_aggregation_receipt_ids: Vec::new(),
            validator_quorum_preserved: true,
            disposition: InternetFaultRunDisposition::Passed,
            detail: String::from(
                "Night soak run: packet-loss failover remained repeatable and did not regress into silent transport death.",
            ),
        },
        InternetFaultRunReceipt {
            receipt_id: String::from("run.delayed_checkpoint_catchup.night1"),
            suite_id: String::from("suite.internet_soak_night"),
            profile_id: String::from("profile.delayed_checkpoint_catchup"),
            observed_failover_receipt_ids: Vec::new(),
            observed_catchup_receipt_ids: vec![String::from(
                "catchup.public_miner.local_mlx.after_deathrattle",
            )],
            observed_outer_sync_receipt_ids: Vec::new(),
            observed_aggregation_receipt_ids: Vec::new(),
            validator_quorum_preserved: true,
            disposition: InternetFaultRunDisposition::Passed,
            detail: String::from(
                "Night soak run: catch-up still closed under stretched checkpoint delay after hours of continuous traffic.",
            ),
        },
        InternetFaultRunReceipt {
            receipt_id: String::from("run.bandwidth_throttle_outer_sync.night1"),
            suite_id: String::from("suite.internet_soak_night"),
            profile_id: String::from("profile.bandwidth_throttle_outer_sync"),
            observed_failover_receipt_ids: Vec::new(),
            observed_catchup_receipt_ids: Vec::new(),
            observed_outer_sync_receipt_ids: vec![
                String::from("exchange.public_miner.google_to_runpod.int8.1"),
                String::from("exchange.public_miner.local_mlx_to_runpod.nf4.1"),
            ],
            observed_aggregation_receipt_ids: vec![String::from(
                "aggregation.outer_sync.runpod.round_2056",
            )],
            validator_quorum_preserved: true,
            disposition: InternetFaultRunDisposition::Passed,
            detail: String::from(
                "Night soak run: throttled outer sync stayed repeatable long enough to count as retained evidence rather than one short lucky pass.",
            ),
        },
    ];

    let mut contract = InternetFaultHarnessContract {
        schema_version: String::from(INTERNET_FAULT_HARNESS_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(INTERNET_FAULT_HARNESS_CONTRACT_ID),
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        elastic_device_mesh_contract_digest: mesh.contract_digest.clone(),
        wan_overlay_route_contract_digest: wan.contract_digest.clone(),
        live_checkpoint_catchup_contract_digest: catchup.contract_digest.clone(),
        quantized_outer_sync_contract_digest: outer_sync.contract_digest.clone(),
        fault_profiles,
        throughput_baselines,
        soak_suites,
        run_receipts,
        authority_paths: InternetFaultHarnessAuthorityPaths {
            fixture_path: String::from(INTERNET_FAULT_HARNESS_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(INTERNET_FAULT_HARNESS_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(INTERNET_FAULT_HARNESS_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(INTERNET_FAULT_HARNESS_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first public-internet realism harness above route, catch-up, and outer-sync truth: fault profiles, throughput baselines, soak suites, repeated passed runs, and one explicit held validator-loss case. It does not yet claim incentive settlement or global public-network promotion beyond the retained evidence set.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_internet_fault_harness_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), InternetFaultHarnessContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            InternetFaultHarnessContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_internet_fault_harness_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| InternetFaultHarnessContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher
        .update(serde_json::to_vec(value).expect(
            "stable digest serialization must succeed for internet fault harness contract",
        ));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_internet_fault_harness_contract, InternetFaultHarnessContractError,
        InternetFaultRunDisposition,
    };

    #[test]
    fn canonical_internet_fault_harness_contract_is_valid(
    ) -> Result<(), InternetFaultHarnessContractError> {
        let contract = canonical_internet_fault_harness_contract()?;
        contract.validate()
    }

    #[test]
    fn validator_loss_case_must_stay_held() -> Result<(), InternetFaultHarnessContractError> {
        let mut contract = canonical_internet_fault_harness_contract()?;
        let validator_loss = contract
            .run_receipts
            .iter_mut()
            .find(|receipt| receipt.receipt_id == "run.validator_loss_hold.day1")
            .expect("canonical validator-loss receipt must exist");
        validator_loss.validator_quorum_preserved = true;
        validator_loss.disposition = InternetFaultRunDisposition::Passed;
        let error = contract
            .validate()
            .expect_err("validator-loss case cannot flip to passed");
        assert!(matches!(
            error,
            InternetFaultHarnessContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
