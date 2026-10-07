use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_live_checkpoint_catchup_contract, canonical_public_network_registry_contract,
    canonical_public_work_assignment_contract, canonical_quantized_outer_sync_contract,
    canonical_remote_train_artifact_backend_contract_set, canonical_wan_overlay_route_contract,
    DecentralizedNetworkRoleClass, LiveCheckpointCatchupContractError,
    PublicNetworkRegistryContractError, PublicWorkAssignmentContractError,
    QuantizedOuterSyncContractError, RemoteTrainArtifactBackendContractError, TrainArtifactClass,
    WanOverlayRouteContractError,
};

pub const CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.content_addressed_artifact_exchange_contract.v1";
pub const CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_ID: &str =
    "psionic.content_addressed_artifact_exchange_contract.v1";
pub const CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/content_addressed_artifact_exchange_contract_v1.json";
pub const CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-content-addressed-artifact-exchange-contract.sh";
pub const CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_DOC_PATH: &str =
    "docs/CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_REFERENCE.md";
pub const CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str =
    "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum ContentAddressedArtifactExchangeContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    PublicRegistry(#[from] PublicNetworkRegistryContractError),
    #[error(transparent)]
    PublicWork(#[from] PublicWorkAssignmentContractError),
    #[error(transparent)]
    WanRoute(#[from] WanOverlayRouteContractError),
    #[error(transparent)]
    LiveCatchup(#[from] LiveCheckpointCatchupContractError),
    #[error(transparent)]
    OuterSync(#[from] QuantizedOuterSyncContractError),
    #[error(transparent)]
    RemoteBackends(#[from] RemoteTrainArtifactBackendContractError),
    #[error("content-addressed artifact exchange contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentAddressedArtifactKind {
    QuantizedDelta,
    GradientSlice,
    LiveCheckpoint,
    ValidatorScore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentAddressedExchangeBackendKind {
    PeerSeed,
    RelayCache,
    AuthoritativeStore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentAddressedFetchDisposition {
    Verified,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentAddressedFetchRefusalKind {
    DigestMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentAddressedExchangeBackend {
    pub backend_id: String,
    pub backend_kind: ContentAddressedExchangeBackendKind,
    pub serving_registry_record_id: String,
    pub serving_role_class: DecentralizedNetworkRoleClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_remote_backend_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_route_id: Option<String>,
    pub ttl_ms: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentAddressedArtifactRecord {
    pub artifact_id: String,
    pub artifact_kind: ContentAddressedArtifactKind,
    pub publisher_registry_record_id: String,
    pub publisher_role_class: DecentralizedNetworkRoleClass,
    pub origin_contract_id: String,
    pub origin_reference_id: String,
    pub content_id: String,
    pub sha256: String,
    pub byte_length: u64,
    pub primary_backend_id: String,
    pub mirror_backend_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authoritative_artifact_class: Option<TrainArtifactClass>,
    pub detail: String,
}

impl ContentAddressedArtifactRecord {
    #[must_use]
    pub fn stable_content_id(&self) -> String {
        stable_content_id(self.artifact_kind, self.sha256.as_str(), self.byte_length)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentAddressedArtifactFetchReceipt {
    pub receipt_id: String,
    pub artifact_id: String,
    pub requester_registry_record_id: String,
    pub requester_role_class: DecentralizedNetworkRoleClass,
    pub selected_backend_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_route_id: Option<String>,
    pub observed_sha256: String,
    pub observed_byte_length: u64,
    pub disposition: ContentAddressedFetchDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<ContentAddressedFetchRefusalKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentAddressedArtifactExchangeAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentAddressedArtifactExchangeContract {
    pub schema_version: String,
    pub contract_id: String,
    pub public_network_registry_contract_digest: String,
    pub public_work_assignment_contract_digest: String,
    pub wan_overlay_route_contract_digest: String,
    pub live_checkpoint_catchup_contract_digest: String,
    pub quantized_outer_sync_contract_digest: String,
    pub remote_train_artifact_backend_contract_digest: String,
    pub exchange_backends: Vec<ContentAddressedExchangeBackend>,
    pub published_artifacts: Vec<ContentAddressedArtifactRecord>,
    pub fetch_receipts: Vec<ContentAddressedArtifactFetchReceipt>,
    pub authority_paths: ContentAddressedArtifactExchangeAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl ContentAddressedArtifactExchangeContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(
            b"psionic_content_addressed_artifact_exchange_contract|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), ContentAddressedArtifactExchangeContractError> {
        let registry = canonical_public_network_registry_contract()?;
        let public_work = canonical_public_work_assignment_contract()?;
        let wan = canonical_wan_overlay_route_contract()?;
        let catchup = canonical_live_checkpoint_catchup_contract()?;
        let outer_sync = canonical_quantized_outer_sync_contract()?;
        let remote_backends = canonical_remote_train_artifact_backend_contract_set()?;

        let registry_by_id = registry
            .registry_records
            .iter()
            .map(|record| (record.registry_record_id.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let route_ids = wan
            .route_records
            .iter()
            .map(|route| route.route_id.as_str())
            .collect::<BTreeSet<_>>();
        let applied_exchange_by_id = outer_sync
            .exchange_receipts
            .iter()
            .filter(|receipt| receipt.disposition == crate::OuterSyncExchangeDisposition::Applied)
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let advertisement_by_id = catchup
            .advertisements
            .iter()
            .map(|advertisement| (advertisement.advertisement_id.as_str(), advertisement))
            .collect::<BTreeMap<_, _>>();
        let validator_assignment_receipt_ids = public_work
            .assignment_receipts
            .iter()
            .filter_map(|receipt| {
                let assignment = public_work
                    .assignments
                    .iter()
                    .find(|candidate| candidate.assignment_id == receipt.assignment_id)?;
                (assignment.assignment_kind
                    == crate::PublicWorkAssignmentKind::PublicValidatorChallenge)
                    .then_some(receipt.receipt_id.as_str())
            })
            .collect::<BTreeSet<_>>();
        let remote_backend_ids = remote_backends
            .backends
            .iter()
            .map(|backend| backend.backend_id.as_str())
            .collect::<BTreeSet<_>>();
        let backend_by_id = self
            .exchange_backends
            .iter()
            .map(|backend| (backend.backend_id.as_str(), backend))
            .collect::<BTreeMap<_, _>>();
        let artifact_by_id = self
            .published_artifacts
            .iter()
            .map(|artifact| (artifact.artifact_id.as_str(), artifact))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_SCHEMA_VERSION {
            return Err(
                ContentAddressedArtifactExchangeContractError::InvalidContract {
                    detail: format!(
                        "schema_version must stay `{}` but was `{}`",
                        CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_SCHEMA_VERSION,
                        self.schema_version
                    ),
                },
            );
        }
        if self.contract_id != CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_ID {
            return Err(
                ContentAddressedArtifactExchangeContractError::InvalidContract {
                    detail: String::from("contract_id drifted"),
                },
            );
        }
        if self.public_network_registry_contract_digest != registry.contract_digest
            || self.public_work_assignment_contract_digest != public_work.contract_digest
            || self.wan_overlay_route_contract_digest != wan.contract_digest
            || self.live_checkpoint_catchup_contract_digest != catchup.contract_digest
            || self.quantized_outer_sync_contract_digest != outer_sync.contract_digest
            || self.remote_train_artifact_backend_contract_digest != remote_backends.contract_digest
        {
            return Err(
                ContentAddressedArtifactExchangeContractError::InvalidContract {
                    detail: String::from("upstream contract digest drifted"),
                },
            );
        }
        if self.authority_paths.fixture_path
            != CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path
                != CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(
                ContentAddressedArtifactExchangeContractError::InvalidContract {
                    detail: String::from("authority paths drifted"),
                },
            );
        }

        let mut backend_ids = BTreeSet::new();
        for backend in &self.exchange_backends {
            if !backend_ids.insert(backend.backend_id.as_str()) {
                return Err(
                    ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!("duplicate exchange backend `{}`", backend.backend_id),
                    },
                );
            }
            let registry_record = registry_by_id
                .get(backend.serving_registry_record_id.as_str())
                .ok_or_else(
                    || ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "exchange backend `{}` references unknown registry record `{}`",
                            backend.backend_id, backend.serving_registry_record_id
                        ),
                    },
                )?;
            if !registry_record
                .role_classes
                .contains(&backend.serving_role_class)
                || backend.ttl_ms == 0
            {
                return Err(
                    ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "exchange backend `{}` lost serving role or ttl",
                            backend.backend_id
                        ),
                    },
                );
            }
            match backend.backend_kind {
                ContentAddressedExchangeBackendKind::PeerSeed => {
                    if !matches!(
                        backend.serving_role_class,
                        DecentralizedNetworkRoleClass::PublicMiner
                            | DecentralizedNetworkRoleClass::PublicValidator
                    ) || backend.linked_remote_backend_id.is_some()
                        || backend
                            .preferred_route_id
                            .as_deref()
                            .is_none_or(|route_id| !route_ids.contains(route_id))
                    {
                        return Err(
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "peer seed backend `{}` drifted",
                                    backend.backend_id
                                ),
                            },
                        );
                    }
                }
                ContentAddressedExchangeBackendKind::RelayCache => {
                    if backend.serving_role_class != DecentralizedNetworkRoleClass::Relay
                        || backend.linked_remote_backend_id.is_some()
                        || backend
                            .preferred_route_id
                            .as_deref()
                            .is_none_or(|route_id| !route_ids.contains(route_id))
                    {
                        return Err(
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "relay cache backend `{}` drifted",
                                    backend.backend_id
                                ),
                            },
                        );
                    }
                }
                ContentAddressedExchangeBackendKind::AuthoritativeStore => {
                    let linked_remote_backend_id =
                        backend.linked_remote_backend_id.as_deref().ok_or_else(|| {
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "authoritative store backend `{}` lost remote backend binding",
                                    backend.backend_id
                                ),
                            }
                        })?;
                    if !matches!(
                        backend.serving_role_class,
                        DecentralizedNetworkRoleClass::CheckpointAuthority
                            | DecentralizedNetworkRoleClass::Aggregator
                    ) || backend.preferred_route_id.is_some()
                        || !remote_backend_ids.contains(linked_remote_backend_id)
                    {
                        return Err(
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "authoritative store backend `{}` drifted",
                                    backend.backend_id
                                ),
                            },
                        );
                    }
                }
            }
        }

        let mut artifact_ids = BTreeSet::new();
        for artifact in &self.published_artifacts {
            if !artifact_ids.insert(artifact.artifact_id.as_str()) {
                return Err(
                    ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!("duplicate artifact `{}`", artifact.artifact_id),
                    },
                );
            }
            let registry_record = registry_by_id
                .get(artifact.publisher_registry_record_id.as_str())
                .ok_or_else(
                    || ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "artifact `{}` references unknown publisher `{}`",
                            artifact.artifact_id, artifact.publisher_registry_record_id
                        ),
                    },
                )?;
            let primary_backend = backend_by_id
                .get(artifact.primary_backend_id.as_str())
                .ok_or_else(
                    || ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "artifact `{}` references unknown primary backend `{}`",
                            artifact.artifact_id, artifact.primary_backend_id
                        ),
                    },
                )?;
            if !registry_record
                .role_classes
                .contains(&artifact.publisher_role_class)
                || artifact.byte_length == 0
                || artifact.sha256.is_empty()
                || artifact.content_id != artifact.stable_content_id()
            {
                return Err(
                    ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "artifact `{}` lost publisher role, byte length, sha256, or content id",
                            artifact.artifact_id
                        ),
                    },
                );
            }
            for mirror_backend_id in &artifact.mirror_backend_ids {
                if mirror_backend_id == &artifact.primary_backend_id
                    || !backend_by_id.contains_key(mirror_backend_id.as_str())
                {
                    return Err(
                        ContentAddressedArtifactExchangeContractError::InvalidContract {
                            detail: format!(
                                "artifact `{}` drifted mirror backend `{}`",
                                artifact.artifact_id, mirror_backend_id
                            ),
                        },
                    );
                }
            }
            match artifact.artifact_kind {
                ContentAddressedArtifactKind::QuantizedDelta
                | ContentAddressedArtifactKind::GradientSlice => {
                    let exchange = applied_exchange_by_id
                        .get(artifact.origin_reference_id.as_str())
                        .ok_or_else(|| {
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "artifact `{}` lost applied outer-sync origin `{}`",
                                    artifact.artifact_id, artifact.origin_reference_id
                                ),
                            }
                        })?;
                    if artifact.origin_contract_id != outer_sync.contract_id
                        || artifact.publisher_role_class
                            != DecentralizedNetworkRoleClass::PublicMiner
                        || artifact.publisher_registry_record_id
                            != exchange.source_registry_record_id
                        || primary_backend.backend_kind
                            != ContentAddressedExchangeBackendKind::PeerSeed
                        || artifact.authoritative_artifact_class.is_some()
                    {
                        return Err(
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "delta or gradient artifact `{}` drifted",
                                    artifact.artifact_id
                                ),
                            },
                        );
                    }
                }
                ContentAddressedArtifactKind::LiveCheckpoint => {
                    let advertisement = advertisement_by_id
                        .get(artifact.origin_reference_id.as_str())
                        .ok_or_else(|| {
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "checkpoint artifact `{}` lost advertisement `{}`",
                                    artifact.artifact_id, artifact.origin_reference_id
                                ),
                            }
                        })?;
                    if artifact.origin_contract_id != catchup.contract_id
                        || artifact.publisher_role_class
                            != DecentralizedNetworkRoleClass::CheckpointAuthority
                        || artifact.publisher_registry_record_id
                            != advertisement.serving_registry_record_id
                        || primary_backend.backend_kind
                            != ContentAddressedExchangeBackendKind::AuthoritativeStore
                        || artifact.authoritative_artifact_class
                            != Some(TrainArtifactClass::Checkpoint)
                    {
                        return Err(
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "live checkpoint artifact `{}` drifted",
                                    artifact.artifact_id
                                ),
                            },
                        );
                    }
                }
                ContentAddressedArtifactKind::ValidatorScore => {
                    if !validator_assignment_receipt_ids
                        .contains(artifact.origin_reference_id.as_str())
                        || artifact.origin_contract_id != public_work.contract_id
                        || artifact.publisher_role_class
                            != DecentralizedNetworkRoleClass::PublicValidator
                        || primary_backend.backend_kind
                            != ContentAddressedExchangeBackendKind::RelayCache
                        || artifact.authoritative_artifact_class
                            != Some(TrainArtifactClass::MetricsBundle)
                        || !artifact.mirror_backend_ids.iter().any(|backend_id| {
                            backend_by_id
                                .get(backend_id.as_str())
                                .is_some_and(|backend| {
                                    backend.backend_kind
                                        == ContentAddressedExchangeBackendKind::AuthoritativeStore
                                })
                        })
                    {
                        return Err(
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "validator score artifact `{}` drifted",
                                    artifact.artifact_id
                                ),
                            },
                        );
                    }
                }
            }
        }

        let mut fetch_receipt_ids = BTreeSet::new();
        let mut refused_fetches = 0_u16;
        for receipt in &self.fetch_receipts {
            if !fetch_receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(
                    ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!("duplicate fetch receipt `{}`", receipt.receipt_id),
                    },
                );
            }
            let artifact = artifact_by_id
                .get(receipt.artifact_id.as_str())
                .ok_or_else(
                    || ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "fetch receipt `{}` references unknown artifact `{}`",
                            receipt.receipt_id, receipt.artifact_id
                        ),
                    },
                )?;
            let requester_record = registry_by_id
                .get(receipt.requester_registry_record_id.as_str())
                .ok_or_else(
                    || ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "fetch receipt `{}` references unknown requester `{}`",
                            receipt.receipt_id, receipt.requester_registry_record_id
                        ),
                    },
                )?;
            let backend = backend_by_id
                .get(receipt.selected_backend_id.as_str())
                .ok_or_else(
                    || ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "fetch receipt `{}` references unknown backend `{}`",
                            receipt.receipt_id, receipt.selected_backend_id
                        ),
                    },
                )?;
            if !requester_record
                .role_classes
                .contains(&receipt.requester_role_class)
            {
                return Err(
                    ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "fetch receipt `{}` lost requester role binding",
                            receipt.receipt_id
                        ),
                    },
                );
            }
            if let Some(selected_route_id) = receipt.selected_route_id.as_deref() {
                if !route_ids.contains(selected_route_id) {
                    return Err(
                        ContentAddressedArtifactExchangeContractError::InvalidContract {
                            detail: format!(
                                "fetch receipt `{}` references unknown route `{}`",
                                receipt.receipt_id, selected_route_id
                            ),
                        },
                    );
                }
            } else if backend.backend_kind
                != ContentAddressedExchangeBackendKind::AuthoritativeStore
            {
                return Err(
                    ContentAddressedArtifactExchangeContractError::InvalidContract {
                        detail: format!(
                            "non-store fetch receipt `{}` lost route selection",
                            receipt.receipt_id
                        ),
                    },
                );
            }
            match receipt.disposition {
                ContentAddressedFetchDisposition::Verified => {
                    if receipt.observed_sha256 != artifact.sha256
                        || receipt.observed_byte_length != artifact.byte_length
                        || receipt.refusal_kind.is_some()
                        || receipt.refusal.is_some()
                    {
                        return Err(
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "verified fetch receipt `{}` drifted",
                                    receipt.receipt_id
                                ),
                            },
                        );
                    }
                }
                ContentAddressedFetchDisposition::Refused => {
                    refused_fetches += 1;
                    if receipt.observed_sha256 == artifact.sha256
                        || receipt.observed_byte_length != artifact.byte_length
                        || receipt.refusal_kind
                            != Some(ContentAddressedFetchRefusalKind::DigestMismatch)
                        || receipt.refusal.is_none()
                    {
                        return Err(
                            ContentAddressedArtifactExchangeContractError::InvalidContract {
                                detail: format!(
                                    "refused fetch receipt `{}` drifted",
                                    receipt.receipt_id
                                ),
                            },
                        );
                    }
                }
            }
        }
        if refused_fetches != 1 {
            return Err(
                ContentAddressedArtifactExchangeContractError::InvalidContract {
                    detail: String::from("expected exactly one refused digest-mismatch fetch"),
                },
            );
        }

        if self.contract_digest != self.stable_digest() {
            return Err(
                ContentAddressedArtifactExchangeContractError::InvalidContract {
                    detail: String::from("contract_digest does not match the stable digest"),
                },
            );
        }

        Ok(())
    }
}

static CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_CACHE: std::sync::OnceLock<
    ContentAddressedArtifactExchangeContract,
> = std::sync::OnceLock::new();

pub fn canonical_content_addressed_artifact_exchange_contract(
) -> Result<ContentAddressedArtifactExchangeContract, ContentAddressedArtifactExchangeContractError>
{
    if let Some(contract) = CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let registry = canonical_public_network_registry_contract()?;
    let public_work = canonical_public_work_assignment_contract()?;
    let wan = canonical_wan_overlay_route_contract()?;
    let catchup = canonical_live_checkpoint_catchup_contract()?;
    let outer_sync = canonical_quantized_outer_sync_contract()?;
    let remote_backends = canonical_remote_train_artifact_backend_contract_set()?;

    let exchange_backends = vec![
        exchange_backend(
            "backend.peer.google.seed",
            ContentAddressedExchangeBackendKind::PeerSeed,
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            None,
            Some("route.checkpoint_authority.google_runpod.direct"),
            900_000,
            "Google seeds miner-owned delta and gradient artifacts directly over the same direct WAN path already admitted for outer sync.",
        ),
        exchange_backend(
            "backend.peer.local_mlx.seed",
            ContentAddressedExchangeBackendKind::PeerSeed,
            "local_mlx_mac_workstation.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            None,
            Some("route.checkpoint_authority.local_mlx_runpod.overlay"),
            900_000,
            "Apple MLX seeds miner-owned delta artifacts over the overlay path used by the live rejoin and outer-sync receipts.",
        ),
        exchange_backend(
            "backend.relay.google.cache",
            ContentAddressedExchangeBackendKind::RelayCache,
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::Relay,
            None,
            Some("route.checkpoint_authority.google_runpod.direct"),
            1_800_000,
            "Google also exposes the first relay cache so public artifacts can survive short-lived peer churn without becoming coordinator-only state.",
        ),
        exchange_backend(
            "backend.store.google.bucket",
            ContentAddressedExchangeBackendKind::AuthoritativeStore,
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            Some("google_train_bucket_backend"),
            None,
            14_400_000,
            "The canonical cloud bucket remains one authoritative content-addressed store for checkpoints and mirrored score outputs.",
        ),
        exchange_backend(
            "backend.store.runpod.workspace",
            ContentAddressedExchangeBackendKind::AuthoritativeStore,
            "runpod_8xh100_dense_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            Some("runpod_workspace_backend"),
            None,
            14_400_000,
            "RunPod keeps a mirrored authoritative store so checkpoint publication and replay do not collapse onto one backend.",
        ),
    ];

    let published_artifacts = vec![
        artifact_record(
            "artifact.delta.google.int8.round2056",
            ContentAddressedArtifactKind::QuantizedDelta,
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            outer_sync.contract_id.as_str(),
            "exchange.public_miner.google_to_runpod.int8.1",
            "sha256:artifact_delta_google_int8_round2056_v1",
            20_971_520,
            "backend.peer.google.seed",
            vec![String::from("backend.relay.google.cache")],
            None,
            "Google publishes its admitted INT8 outer-sync package as one content-addressed delta artifact with one relay-cache mirror.",
        ),
        artifact_record(
            "artifact.delta.local_mlx.nf4.round2056",
            ContentAddressedArtifactKind::QuantizedDelta,
            "local_mlx_mac_workstation.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            outer_sync.contract_id.as_str(),
            "exchange.public_miner.local_mlx_to_runpod.nf4.1",
            "sha256:artifact_delta_local_mlx_nf4_round2056_v1",
            8_388_608,
            "backend.peer.local_mlx.seed",
            vec![String::from("backend.relay.google.cache")],
            None,
            "Apple MLX publishes its admitted NF4 residual package as a content-addressed delta artifact over the overlay-backed peer seed and relay cache.",
        ),
        artifact_record(
            "artifact.gradient.google.full.round2056",
            ContentAddressedArtifactKind::GradientSlice,
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            outer_sync.contract_id.as_str(),
            "exchange.public_miner.google_to_runpod.int8.1",
            "sha256:artifact_gradient_google_full_round2056_v1",
            100_663_296,
            "backend.peer.google.seed",
            vec![String::from("backend.relay.google.cache")],
            None,
            "Google retains the pre-quantization gradient slice under the same content-addressed family even though WAN fetch of the raw slice is still expected to fail closed on mismatch.",
        ),
        artifact_record(
            "artifact.checkpoint.step2048.live",
            ContentAddressedArtifactKind::LiveCheckpoint,
            "runpod_8xh100_dense_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            catchup.contract_id.as_str(),
            "advertisement.checkpoint_authority.runpod.primary",
            "sha256:artifact_checkpoint_step2048_live_v1",
            402_653_184,
            "backend.store.runpod.workspace",
            vec![String::from("backend.store.google.bucket")],
            Some(TrainArtifactClass::Checkpoint),
            "RunPod publishes the admitted live checkpoint as one content-addressed store object while keeping the Google bucket as an authoritative mirror.",
        ),
        artifact_record(
            "artifact.score.window1230.google.provisional",
            ContentAddressedArtifactKind::ValidatorScore,
            "google_l4_validator_node.registry",
            DecentralizedNetworkRoleClass::PublicValidator,
            public_work.contract_id.as_str(),
            "receipt.assignment.public_validator.window1230.google",
            "sha256:artifact_score_window1230_google_provisional_v1",
            65_536,
            "backend.relay.google.cache",
            vec![String::from("backend.store.google.bucket")],
            Some(TrainArtifactClass::MetricsBundle),
            "Google publishes a provisional validator score artifact through the relay cache and mirrors it into the authoritative store; the transport is canonical even though final verdict semantics still wait on the validator contract.",
        ),
    ];

    let fetch_receipts = vec![
        fetch_receipt(
            "fetch.delta.google.runpod.verified",
            "artifact.delta.google.int8.round2056",
            "runpod_8xh100_dense_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            "backend.peer.google.seed",
            Some("route.checkpoint_authority.google_runpod.direct"),
            "sha256:artifact_delta_google_int8_round2056_v1",
            20_971_520,
            ContentAddressedFetchDisposition::Verified,
            None,
            None,
            "RunPod verifies and consumes the Google INT8 delta artifact directly from the peer seed over the direct WAN route.",
        ),
        fetch_receipt(
            "fetch.delta.local_mlx.runpod.verified",
            "artifact.delta.local_mlx.nf4.round2056",
            "runpod_8xh100_dense_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            "backend.peer.local_mlx.seed",
            Some("route.checkpoint_authority.local_mlx_runpod.overlay"),
            "sha256:artifact_delta_local_mlx_nf4_round2056_v1",
            8_388_608,
            ContentAddressedFetchDisposition::Verified,
            None,
            None,
            "RunPod verifies and consumes the Apple MLX NF4 residual artifact over the overlay-backed peer seed.",
        ),
        fetch_receipt(
            "fetch.checkpoint.local_mlx.google_store.verified",
            "artifact.checkpoint.step2048.live",
            "local_mlx_mac_workstation.registry",
            DecentralizedNetworkRoleClass::PublicMiner,
            "backend.store.google.bucket",
            None,
            "sha256:artifact_checkpoint_step2048_live_v1",
            402_653_184,
            ContentAddressedFetchDisposition::Verified,
            None,
            None,
            "The rejoined Apple MLX miner can fetch the same admitted checkpoint from the authoritative Google store without inventing a new path vocabulary.",
        ),
        fetch_receipt(
            "fetch.score.runpod.google_relay.verified",
            "artifact.score.window1230.google.provisional",
            "runpod_8xh100_dense_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            "backend.relay.google.cache",
            Some("route.checkpoint_authority.google_runpod.direct"),
            "sha256:artifact_score_window1230_google_provisional_v1",
            65_536,
            ContentAddressedFetchDisposition::Verified,
            None,
            None,
            "RunPod can verify the provisional validator score artifact through the relay cache before the final score-governance issue lands.",
        ),
        fetch_receipt(
            "fetch.gradient.google.runpod.refused",
            "artifact.gradient.google.full.round2056",
            "runpod_8xh100_dense_node.registry",
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            "backend.relay.google.cache",
            Some("route.checkpoint_authority.google_runpod.direct"),
            "sha256:artifact_gradient_google_full_round2056_corrupt_v1",
            100_663_296,
            ContentAddressedFetchDisposition::Refused,
            Some(ContentAddressedFetchRefusalKind::DigestMismatch),
            Some("relay cache fetch is refused because the observed raw-gradient digest mismatched the published content id"),
            "The contract keeps one corruption refusal explicit so raw artifact exchange cannot silently accept mismatched bytes just because the route itself was healthy.",
        ),
    ];

    let mut contract = ContentAddressedArtifactExchangeContract {
        schema_version: String::from(CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_ID),
        public_network_registry_contract_digest: registry.contract_digest.clone(),
        public_work_assignment_contract_digest: public_work.contract_digest.clone(),
        wan_overlay_route_contract_digest: wan.contract_digest.clone(),
        live_checkpoint_catchup_contract_digest: catchup.contract_digest.clone(),
        quantized_outer_sync_contract_digest: outer_sync.contract_digest.clone(),
        remote_train_artifact_backend_contract_digest: remote_backends.contract_digest.clone(),
        exchange_backends,
        published_artifacts,
        fetch_receipts,
        authority_paths: ContentAddressedArtifactExchangeAuthorityPaths {
            fixture_path: String::from(CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(
                CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_CHECK_SCRIPT_PATH,
            ),
            reference_doc_path: String::from(CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(
                CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract freezes the first content-addressed public artifact exchange family: peer seeds, relay caches, authoritative stores, published artifact ids, verified fetch receipts, and one refused digest-mismatch fetch. It does not yet claim validator verdict truth or the full public miner execution protocol.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = CONTENT_ADDRESSED_ARTIFACT_EXCHANGE_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_content_addressed_artifact_exchange_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), ContentAddressedArtifactExchangeContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ContentAddressedArtifactExchangeContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_content_addressed_artifact_exchange_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| {
        ContentAddressedArtifactExchangeContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })
}

fn exchange_backend(
    backend_id: &str,
    backend_kind: ContentAddressedExchangeBackendKind,
    serving_registry_record_id: &str,
    serving_role_class: DecentralizedNetworkRoleClass,
    linked_remote_backend_id: Option<&str>,
    preferred_route_id: Option<&str>,
    ttl_ms: u64,
    detail: &str,
) -> ContentAddressedExchangeBackend {
    ContentAddressedExchangeBackend {
        backend_id: String::from(backend_id),
        backend_kind,
        serving_registry_record_id: String::from(serving_registry_record_id),
        serving_role_class,
        linked_remote_backend_id: linked_remote_backend_id.map(String::from),
        preferred_route_id: preferred_route_id.map(String::from),
        ttl_ms,
        detail: String::from(detail),
    }
}

fn artifact_record(
    artifact_id: &str,
    artifact_kind: ContentAddressedArtifactKind,
    publisher_registry_record_id: &str,
    publisher_role_class: DecentralizedNetworkRoleClass,
    origin_contract_id: &str,
    origin_reference_id: &str,
    sha256: &str,
    byte_length: u64,
    primary_backend_id: &str,
    mirror_backend_ids: Vec<String>,
    authoritative_artifact_class: Option<TrainArtifactClass>,
    detail: &str,
) -> ContentAddressedArtifactRecord {
    let mut artifact = ContentAddressedArtifactRecord {
        artifact_id: String::from(artifact_id),
        artifact_kind,
        publisher_registry_record_id: String::from(publisher_registry_record_id),
        publisher_role_class,
        origin_contract_id: String::from(origin_contract_id),
        origin_reference_id: String::from(origin_reference_id),
        content_id: String::new(),
        sha256: String::from(sha256),
        byte_length,
        primary_backend_id: String::from(primary_backend_id),
        mirror_backend_ids,
        authoritative_artifact_class,
        detail: String::from(detail),
    };
    artifact.content_id = artifact.stable_content_id();
    artifact
}

fn fetch_receipt(
    receipt_id: &str,
    artifact_id: &str,
    requester_registry_record_id: &str,
    requester_role_class: DecentralizedNetworkRoleClass,
    selected_backend_id: &str,
    selected_route_id: Option<&str>,
    observed_sha256: &str,
    observed_byte_length: u64,
    disposition: ContentAddressedFetchDisposition,
    refusal_kind: Option<ContentAddressedFetchRefusalKind>,
    refusal: Option<&str>,
    detail: &str,
) -> ContentAddressedArtifactFetchReceipt {
    ContentAddressedArtifactFetchReceipt {
        receipt_id: String::from(receipt_id),
        artifact_id: String::from(artifact_id),
        requester_registry_record_id: String::from(requester_registry_record_id),
        requester_role_class,
        selected_backend_id: String::from(selected_backend_id),
        selected_route_id: selected_route_id.map(String::from),
        observed_sha256: String::from(observed_sha256),
        observed_byte_length,
        disposition,
        refusal_kind,
        refusal: refusal.map(String::from),
        detail: String::from(detail),
    }
}

fn stable_content_id(
    artifact_kind: ContentAddressedArtifactKind,
    sha256: &str,
    byte_length: u64,
) -> String {
    let payload = (artifact_kind, String::from(sha256), byte_length);
    format!(
        "cid.psionic.v1.{}",
        stable_digest(b"psionic_content_addressed_artifact_id|", &payload)
    )
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect(
        "stable digest serialization must succeed for content-addressed artifact exchange contract",
    ));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_content_addressed_artifact_exchange_contract,
        ContentAddressedArtifactExchangeContractError, ContentAddressedFetchDisposition,
    };

    #[test]
    fn canonical_content_addressed_artifact_exchange_contract_is_valid(
    ) -> Result<(), ContentAddressedArtifactExchangeContractError> {
        let contract = canonical_content_addressed_artifact_exchange_contract()?;
        contract.validate()
    }

    #[test]
    fn digest_mismatch_fetch_must_remain_refused(
    ) -> Result<(), ContentAddressedArtifactExchangeContractError> {
        let mut contract = canonical_content_addressed_artifact_exchange_contract()?;
        let refused_fetch = contract
            .fetch_receipts
            .iter_mut()
            .find(|receipt| receipt.receipt_id == "fetch.gradient.google.runpod.refused")
            .expect("canonical contract should retain the refused gradient fetch");
        refused_fetch.disposition = ContentAddressedFetchDisposition::Verified;
        refused_fetch.refusal_kind = None;
        refused_fetch.refusal = None;
        let error = contract
            .validate()
            .expect_err("digest mismatch fetch cannot silently become verified");
        assert!(matches!(
            error,
            ContentAddressedArtifactExchangeContractError::InvalidContract { .. }
        ));
        Ok(())
    }

    #[test]
    fn validator_score_artifacts_must_bind_to_validator_receipts(
    ) -> Result<(), ContentAddressedArtifactExchangeContractError> {
        let mut contract = canonical_content_addressed_artifact_exchange_contract()?;
        let validator_score = contract
            .published_artifacts
            .iter_mut()
            .find(|artifact| artifact.artifact_id == "artifact.score.window1230.google.provisional")
            .expect("canonical contract should retain the provisional validator score artifact");
        validator_score.origin_reference_id =
            String::from("exchange.public_miner.google_to_runpod.int8.1");
        let error = contract
            .validate()
            .expect_err("validator score artifacts must bind to validator receipts");
        assert!(matches!(
            error,
            ContentAddressedArtifactExchangeContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
