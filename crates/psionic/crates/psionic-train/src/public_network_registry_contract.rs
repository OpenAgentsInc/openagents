use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, canonical_decentralized_network_contract,
    canonical_signed_node_identity_contract_set, cross_provider_training_program_manifest,
    CrossProviderComputeSourceContract, CrossProviderComputeSourceContractError,
    CrossProviderExecutionClass, CrossProviderTrainingProgramManifestError, CrossProviderTrustTier,
    DecentralizedNetworkContractError, DecentralizedNetworkRoleClass,
    SignedNodeIdentityContractSetError, SignedNodeIdentityRecord, SignedNodeRevocationStatus,
    SIGNED_NODE_IDENTITY_RELEASE_ID,
};

pub const PUBLIC_NETWORK_REGISTRY_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.public_network_registry_contract.v1";
pub const PUBLIC_NETWORK_REGISTRY_CONTRACT_ID: &str = "psionic.public_network_registry_contract.v1";
pub const PUBLIC_NETWORK_REGISTRY_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/public_network_registry_contract_v1.json";
pub const PUBLIC_NETWORK_REGISTRY_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-public-network-registry-contract.sh";
pub const PUBLIC_NETWORK_REGISTRY_CONTRACT_DOC_PATH: &str =
    "docs/PUBLIC_NETWORK_REGISTRY_REFERENCE.md";
pub const PUBLIC_NETWORK_REGISTRY_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";
pub const PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID: &str = "network_epoch_00000123";

#[derive(Debug, Error)]
pub enum PublicNetworkRegistryContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    ComputeSource(#[from] CrossProviderComputeSourceContractError),
    #[error(transparent)]
    NetworkContract(#[from] DecentralizedNetworkContractError),
    #[error(transparent)]
    SignedNodeIdentity(#[from] SignedNodeIdentityContractSetError),
    #[error("public network registry contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicNetworkAvailabilityStatus {
    Online,
    Standby,
    Draining,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicNetworkRelayPosture {
    Advertised,
    NotAdvertised,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicNetworkEndpointKind {
    TrainingControl,
    RelayIngress,
    CheckpointMirror,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicNetworkDiscoveryRefusalKind {
    AvailabilityNotOnline,
    CompatibilityPolicyDrift,
    RoleClassNotAdmitted,
    ExecutionClassNotAdmitted,
    TrustTierNotAdmitted,
    RelayNotAdvertised,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicNetworkSessionKind {
    ContributorWindow,
    ValidatorQuorum,
    CheckpointPromotion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkCompatibilityPolicy {
    pub required_release_id: String,
    pub required_environment_ref: String,
    pub required_environment_version: String,
    pub required_program_manifest_id: String,
    pub required_program_manifest_digest: String,
    pub require_active_revocation_status: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkRegistryCompatibility {
    pub release_id: String,
    pub build_digest: String,
    pub environment_ref: String,
    pub environment_version: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub revocation_status: SignedNodeRevocationStatus,
    pub identity_record_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkEndpoint {
    pub endpoint_id: String,
    pub endpoint_kind: PublicNetworkEndpointKind,
    pub uri: String,
    pub advertised_role_classes: Vec<DecentralizedNetworkRoleClass>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkRegistryRecord {
    pub registry_record_id: String,
    pub node_identity_id: String,
    pub node_id: String,
    pub source_id: String,
    pub role_classes: Vec<DecentralizedNetworkRoleClass>,
    pub execution_classes: Vec<CrossProviderExecutionClass>,
    pub trust_tier: CrossProviderTrustTier,
    pub availability_status: PublicNetworkAvailabilityStatus,
    pub relay_posture: PublicNetworkRelayPosture,
    pub current_epoch_id: String,
    pub compatibility: PublicNetworkRegistryCompatibility,
    pub endpoints: Vec<PublicNetworkEndpoint>,
    pub detail: String,
    pub record_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkDiscoveryFilter {
    pub requested_role_class: DecentralizedNetworkRoleClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_execution_class: Option<CrossProviderExecutionClass>,
    pub admitted_trust_tiers: Vec<CrossProviderTrustTier>,
    pub require_online: bool,
    pub require_relay_advertisement: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkDiscoveryRefusal {
    pub registry_record_id: String,
    pub refusal_kind: PublicNetworkDiscoveryRefusalKind,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkDiscoveryExample {
    pub query_id: String,
    pub current_epoch_id: String,
    pub filter: PublicNetworkDiscoveryFilter,
    pub matched_registry_record_ids: Vec<String>,
    pub refusals: Vec<PublicNetworkDiscoveryRefusal>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkMatchmakingOffer {
    pub offer_id: String,
    pub session_kind: PublicNetworkSessionKind,
    pub source_query_id: String,
    pub selected_registry_record_ids: Vec<String>,
    pub standby_registry_record_ids: Vec<String>,
    pub relay_registry_record_ids: Vec<String>,
    pub checkpoint_authority_registry_record_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkRegistryAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNetworkRegistryContract {
    pub schema_version: String,
    pub contract_id: String,
    pub network_id: String,
    pub governance_revision_id: String,
    pub current_epoch_id: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub decentralized_network_contract_digest: String,
    pub signed_node_identity_contract_digest: String,
    pub compatibility_policy: PublicNetworkCompatibilityPolicy,
    pub registry_records: Vec<PublicNetworkRegistryRecord>,
    pub discovery_examples: Vec<PublicNetworkDiscoveryExample>,
    pub matchmaking_offers: Vec<PublicNetworkMatchmakingOffer>,
    pub authority_paths: PublicNetworkRegistryAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl PublicNetworkRegistryRecord {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.record_digest.clear();
        stable_digest(b"psionic_public_network_registry_record|", &clone)
    }
}

impl PublicNetworkRegistryContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_public_network_registry_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), PublicNetworkRegistryContractError> {
        let manifest = cross_provider_training_program_manifest()?;
        let network = canonical_decentralized_network_contract()?;
        let identity_contract = canonical_signed_node_identity_contract_set()?;
        let sources = canonical_cross_provider_compute_source_contracts()?;

        let identity_by_source = identity_contract
            .identities
            .iter()
            .map(|identity| (identity.source_id.as_str(), identity))
            .collect::<BTreeMap<_, _>>();
        let source_by_id = sources
            .iter()
            .map(|source| (source.source_id.as_str(), source))
            .collect::<BTreeMap<_, _>>();
        let record_by_id = self
            .registry_records
            .iter()
            .map(|record| (record.registry_record_id.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let discovery_by_id = self
            .discovery_examples
            .iter()
            .map(|query| (query.query_id.as_str(), query))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != PUBLIC_NETWORK_REGISTRY_CONTRACT_SCHEMA_VERSION {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    PUBLIC_NETWORK_REGISTRY_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != PUBLIC_NETWORK_REGISTRY_CONTRACT_ID {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.network_id != network.network_id {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("network_id drifted"),
            });
        }
        if self.governance_revision_id != network.governance_revision.governance_revision_id {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("governance_revision_id drifted"),
            });
        }
        if self.current_epoch_id != PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("current_epoch_id drifted"),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("program manifest binding drifted"),
            });
        }
        if self.decentralized_network_contract_digest != network.contract_digest {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("decentralized network contract digest drifted"),
            });
        }
        if self.signed_node_identity_contract_digest != identity_contract.contract_digest {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("signed node identity contract digest drifted"),
            });
        }
        if self.compatibility_policy.required_release_id != SIGNED_NODE_IDENTITY_RELEASE_ID
            || self.compatibility_policy.required_environment_ref
                != manifest.environment.environment_ref
            || self.compatibility_policy.required_environment_version
                != manifest.environment.version
            || self.compatibility_policy.required_program_manifest_id
                != manifest.program_manifest_id
            || self.compatibility_policy.required_program_manifest_digest
                != manifest.program_manifest_digest
            || !self.compatibility_policy.require_active_revocation_status
        {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("compatibility policy drifted"),
            });
        }
        if self.authority_paths.fixture_path != PUBLIC_NETWORK_REGISTRY_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != PUBLIC_NETWORK_REGISTRY_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != PUBLIC_NETWORK_REGISTRY_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != PUBLIC_NETWORK_REGISTRY_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.registry_records.len() != identity_contract.identities.len() {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: format!(
                    "expected exactly {} registry records but found {}",
                    identity_contract.identities.len(),
                    self.registry_records.len()
                ),
            });
        }

        let mut registry_ids = BTreeSet::new();
        let mut node_ids = BTreeSet::new();
        let mut source_ids = BTreeSet::new();

        for record in &self.registry_records {
            if !registry_ids.insert(record.registry_record_id.clone()) {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "duplicate registry_record_id `{}`",
                        record.registry_record_id
                    ),
                });
            }
            if !node_ids.insert(record.node_id.clone()) {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!("duplicate node_id `{}`", record.node_id),
                });
            }
            if !source_ids.insert(record.source_id.clone()) {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!("duplicate source_id `{}`", record.source_id),
                });
            }

            let identity = identity_by_source
                .get(record.source_id.as_str())
                .ok_or_else(|| PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` references unknown signed identity source `{}`",
                        record.registry_record_id, record.source_id
                    ),
                })?;
            let source = source_by_id.get(record.source_id.as_str()).ok_or_else(|| {
                PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` references unknown compute source `{}`",
                        record.registry_record_id, record.source_id
                    ),
                }
            })?;

            if record.node_identity_id != identity.node_identity_id
                || record.node_id != identity.node_id
            {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` lost its identity binding",
                        record.registry_record_id
                    ),
                });
            }
            if record.role_classes != identity.admitted_role_classes
                || record.execution_classes != identity.admitted_execution_classes
            {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` drifted from signed identity role or execution classes",
                        record.registry_record_id
                    ),
                });
            }
            if record.trust_tier != source.network.trust_tier {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` trust tier drifted from compute source",
                        record.registry_record_id
                    ),
                });
            }
            if record.current_epoch_id != self.current_epoch_id {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` epoch binding drifted",
                        record.registry_record_id
                    ),
                });
            }

            let expected_compatibility = registry_compatibility(
                identity,
                manifest.program_manifest_id.as_str(),
                manifest.program_manifest_digest.as_str(),
            );
            if record.compatibility != expected_compatibility {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` compatibility projection drifted",
                        record.registry_record_id
                    ),
                });
            }

            let expects_relay = record
                .role_classes
                .contains(&DecentralizedNetworkRoleClass::Relay);
            match (expects_relay, record.relay_posture) {
                (true, PublicNetworkRelayPosture::Advertised)
                | (false, PublicNetworkRelayPosture::NotAdvertised) => {}
                _ => {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "registry record `{}` relay posture drifted from role admission",
                            record.registry_record_id
                        ),
                    });
                }
            }
            if record.endpoints.is_empty() {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` must retain at least one endpoint",
                        record.registry_record_id
                    ),
                });
            }
            if expects_relay
                && !record.endpoints.iter().any(|endpoint| {
                    endpoint.endpoint_kind == PublicNetworkEndpointKind::RelayIngress
                })
            {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` advertises relay without a relay_ingress endpoint",
                        record.registry_record_id
                    ),
                });
            }
            for endpoint in &record.endpoints {
                if endpoint.advertised_role_classes.is_empty() {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "registry record `{}` endpoint `{}` lost advertised_role_classes",
                            record.registry_record_id, endpoint.endpoint_id
                        ),
                    });
                }
                if endpoint
                    .advertised_role_classes
                    .iter()
                    .any(|role_class| !record.role_classes.contains(role_class))
                {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "registry record `{}` endpoint `{}` advertises roles outside the node registry record",
                            record.registry_record_id, endpoint.endpoint_id
                        ),
                    });
                }
                if endpoint.endpoint_kind == PublicNetworkEndpointKind::RelayIngress
                    && !endpoint
                        .advertised_role_classes
                        .contains(&DecentralizedNetworkRoleClass::Relay)
                {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "registry record `{}` relay endpoint `{}` lost relay role binding",
                            record.registry_record_id, endpoint.endpoint_id
                        ),
                    });
                }
            }
            if record.record_digest != record.stable_digest() {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "registry record `{}` record_digest drifted",
                        record.registry_record_id
                    ),
                });
            }
        }

        let mut query_ids = BTreeSet::new();
        for query in &self.discovery_examples {
            if !query_ids.insert(query.query_id.clone()) {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!("duplicate query_id `{}`", query.query_id),
                });
            }
            if query.current_epoch_id != self.current_epoch_id {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!("query `{}` epoch binding drifted", query.query_id),
                });
            }
            if query.filter.admitted_trust_tiers.is_empty() {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!("query `{}` lost admitted_trust_tiers", query.query_id),
                });
            }

            let matched_ids = query
                .matched_registry_record_ids
                .iter()
                .collect::<BTreeSet<_>>();
            if matched_ids.len() != query.matched_registry_record_ids.len() {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!("query `{}` repeats matched ids", query.query_id),
                });
            }

            let refusal_ids = query
                .refusals
                .iter()
                .map(|refusal| refusal.registry_record_id.as_str())
                .collect::<BTreeSet<_>>();
            if refusal_ids.len() != query.refusals.len() {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!("query `{}` repeats refusal ids", query.query_id),
                });
            }

            let covered_ids = query
                .matched_registry_record_ids
                .iter()
                .map(String::as_str)
                .chain(
                    query
                        .refusals
                        .iter()
                        .map(|refusal| refusal.registry_record_id.as_str()),
                )
                .collect::<BTreeSet<_>>();
            let all_registry_ids = self
                .registry_records
                .iter()
                .map(|record| record.registry_record_id.as_str())
                .collect::<BTreeSet<_>>();
            if covered_ids != all_registry_ids {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "query `{}` does not cover the full registry set",
                        query.query_id
                    ),
                });
            }

            for registry_record_id in &query.matched_registry_record_ids {
                let record = record_by_id
                    .get(registry_record_id.as_str())
                    .ok_or_else(|| PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "query `{}` matched unknown registry record `{}`",
                            query.query_id, registry_record_id
                        ),
                    })?;
                if !record_matches_filter(record, &query.filter, &self.compatibility_policy) {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "query `{}` matched registry record `{}` that fails its filter",
                            query.query_id, registry_record_id
                        ),
                    });
                }
            }

            for refusal in &query.refusals {
                let record = record_by_id
                    .get(refusal.registry_record_id.as_str())
                    .ok_or_else(|| PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "query `{}` refusal references unknown registry record `{}`",
                            query.query_id, refusal.registry_record_id
                        ),
                    })?;
                if record_matches_filter(record, &query.filter, &self.compatibility_policy) {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "query `{}` refused registry record `{}` that actually matches",
                            query.query_id, refusal.registry_record_id
                        ),
                    });
                }
                let expected_kind =
                    first_refusal_kind(record, &query.filter, &self.compatibility_policy)
                        .ok_or_else(|| PublicNetworkRegistryContractError::InvalidContract {
                            detail: format!(
                                "query `{}` refusal on `{}` could not be reconstructed",
                                query.query_id, refusal.registry_record_id
                            ),
                        })?;
                if refusal.refusal_kind != expected_kind {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "query `{}` refusal kind for `{}` drifted",
                            query.query_id, refusal.registry_record_id
                        ),
                    });
                }
            }
        }

        let mut offer_ids = BTreeSet::new();
        for offer in &self.matchmaking_offers {
            if !offer_ids.insert(offer.offer_id.clone()) {
                return Err(PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!("duplicate offer_id `{}`", offer.offer_id),
                });
            }
            let source_query = discovery_by_id
                .get(offer.source_query_id.as_str())
                .ok_or_else(|| PublicNetworkRegistryContractError::InvalidContract {
                    detail: format!(
                        "offer `{}` references unknown source query `{}`",
                        offer.offer_id, offer.source_query_id
                    ),
                })?;
            let matched_ids = source_query
                .matched_registry_record_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();

            for registry_record_id in offer
                .selected_registry_record_ids
                .iter()
                .chain(offer.standby_registry_record_ids.iter())
            {
                if !matched_ids.contains(registry_record_id.as_str()) {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "offer `{}` selects registry record `{}` outside query `{}`",
                            offer.offer_id, registry_record_id, offer.source_query_id
                        ),
                    });
                }
            }

            for relay_registry_record_id in &offer.relay_registry_record_ids {
                let relay_record = record_by_id
                    .get(relay_registry_record_id.as_str())
                    .ok_or_else(|| PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "offer `{}` references unknown relay record `{}`",
                            offer.offer_id, relay_registry_record_id
                        ),
                    })?;
                if !relay_record
                    .role_classes
                    .contains(&DecentralizedNetworkRoleClass::Relay)
                {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "offer `{}` uses non-relay record `{}` as relay",
                            offer.offer_id, relay_registry_record_id
                        ),
                    });
                }
            }

            for checkpoint_registry_record_id in &offer.checkpoint_authority_registry_record_ids {
                let checkpoint_record = record_by_id
                    .get(checkpoint_registry_record_id.as_str())
                    .ok_or_else(|| PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "offer `{}` references unknown checkpoint authority record `{}`",
                            offer.offer_id, checkpoint_registry_record_id
                        ),
                    })?;
                if !checkpoint_record
                    .role_classes
                    .contains(&DecentralizedNetworkRoleClass::CheckpointAuthority)
                {
                    return Err(PublicNetworkRegistryContractError::InvalidContract {
                        detail: format!(
                            "offer `{}` uses non-checkpoint record `{}` as checkpoint authority",
                            offer.offer_id, checkpoint_registry_record_id
                        ),
                    });
                }
            }

            match offer.session_kind {
                PublicNetworkSessionKind::ContributorWindow => {
                    if offer.selected_registry_record_ids.is_empty() {
                        return Err(PublicNetworkRegistryContractError::InvalidContract {
                            detail: format!(
                                "offer `{}` lost contributor selections",
                                offer.offer_id
                            ),
                        });
                    }
                    for registry_record_id in &offer.selected_registry_record_ids {
                        let record = record_by_id
                            .get(registry_record_id.as_str())
                            .expect("matched registry ids must resolve");
                        if !record
                            .role_classes
                            .contains(&DecentralizedNetworkRoleClass::PublicMiner)
                        {
                            return Err(PublicNetworkRegistryContractError::InvalidContract {
                                detail: format!(
                                    "offer `{}` selected non-miner record `{}` for contributor_window",
                                    offer.offer_id, registry_record_id
                                ),
                            });
                        }
                    }
                }
                PublicNetworkSessionKind::ValidatorQuorum => {
                    let required_quorum =
                        usize::from(network.governance_revision.minimum_validator_quorum);
                    if offer.selected_registry_record_ids.len() < required_quorum {
                        return Err(PublicNetworkRegistryContractError::InvalidContract {
                            detail: format!(
                                "offer `{}` lost validator quorum; need at least {}",
                                offer.offer_id, required_quorum
                            ),
                        });
                    }
                    for registry_record_id in &offer.selected_registry_record_ids {
                        let record = record_by_id
                            .get(registry_record_id.as_str())
                            .expect("matched registry ids must resolve");
                        if !record
                            .role_classes
                            .contains(&DecentralizedNetworkRoleClass::PublicValidator)
                        {
                            return Err(PublicNetworkRegistryContractError::InvalidContract {
                                detail: format!(
                                    "offer `{}` selected non-validator record `{}` for validator_quorum",
                                    offer.offer_id, registry_record_id
                                ),
                            });
                        }
                    }
                }
                PublicNetworkSessionKind::CheckpointPromotion => {
                    let required_authorities = usize::from(
                        network
                            .checkpoint_authority_policy
                            .minimum_checkpoint_authorities,
                    );
                    if offer.selected_registry_record_ids.len() < required_authorities {
                        return Err(PublicNetworkRegistryContractError::InvalidContract {
                            detail: format!(
                                "offer `{}` lost minimum checkpoint authorities; need at least {}",
                                offer.offer_id, required_authorities
                            ),
                        });
                    }
                    for registry_record_id in &offer.selected_registry_record_ids {
                        let record = record_by_id
                            .get(registry_record_id.as_str())
                            .expect("matched registry ids must resolve");
                        if !record
                            .role_classes
                            .contains(&DecentralizedNetworkRoleClass::CheckpointAuthority)
                        {
                            return Err(PublicNetworkRegistryContractError::InvalidContract {
                                detail: format!(
                                    "offer `{}` selected non-checkpoint-authority record `{}` for checkpoint_promotion",
                                    offer.offer_id, registry_record_id
                                ),
                            });
                        }
                    }
                }
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(PublicNetworkRegistryContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

static PUBLIC_NETWORK_REGISTRY_CONTRACT_CACHE: std::sync::OnceLock<PublicNetworkRegistryContract> =
    std::sync::OnceLock::new();

pub fn canonical_public_network_registry_contract(
) -> Result<PublicNetworkRegistryContract, PublicNetworkRegistryContractError> {
    if let Some(contract) = PUBLIC_NETWORK_REGISTRY_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let manifest = cross_provider_training_program_manifest()?;
    let network = canonical_decentralized_network_contract()?;
    let identity_contract = canonical_signed_node_identity_contract_set()?;
    let sources = canonical_cross_provider_compute_source_contracts()?;
    let source_by_id = sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let identity_by_source = identity_contract
        .identities
        .iter()
        .map(|identity| (identity.source_id.as_str(), identity))
        .collect::<BTreeMap<_, _>>();

    let registry_records = vec![
        build_registry_record(
            source_by_id["google_l4_validator_node"],
            identity_by_source["google_l4_validator_node"],
            manifest.program_manifest_id.as_str(),
            manifest.program_manifest_digest.as_str(),
            vec![
                endpoint(
                    "google_training_control",
                    PublicNetworkEndpointKind::TrainingControl,
                    "tls://google-l4-validator.psionic.testnet:443",
                    vec![
                        DecentralizedNetworkRoleClass::PublicMiner,
                        DecentralizedNetworkRoleClass::PublicValidator,
                        DecentralizedNetworkRoleClass::CheckpointAuthority,
                        DecentralizedNetworkRoleClass::Aggregator,
                    ],
                    "Google primary training control endpoint for miner, validator, checkpoint, and aggregation traffic on the permissioned testnet.",
                ),
                endpoint(
                    "google_relay_ingress",
                    PublicNetworkEndpointKind::RelayIngress,
                    "tls://google-l4-validator.psionic.testnet:8443",
                    vec![DecentralizedNetworkRoleClass::Relay],
                    "Google relay ingress endpoint for overlay and NAT-bypass support while relay is still a network-only support role.",
                ),
                endpoint(
                    "google_checkpoint_mirror",
                    PublicNetworkEndpointKind::CheckpointMirror,
                    "s3://psionic-testnet-google-validator/checkpoints",
                    vec![DecentralizedNetworkRoleClass::CheckpointAuthority],
                    "Google checkpoint mirror endpoint for promoted public-network state and recovery-side fetches.",
                ),
            ],
            PublicNetworkAvailabilityStatus::Online,
            PublicNetworkRelayPosture::Advertised,
            "Google remains the strongest multi-role registry record, carrying current miner, validator, checkpoint-authority, aggregator, and relay admissions together on the permissioned testnet.",
        ),
        build_registry_record(
            source_by_id["runpod_8xh100_dense_node"],
            identity_by_source["runpod_8xh100_dense_node"],
            manifest.program_manifest_id.as_str(),
            manifest.program_manifest_digest.as_str(),
            vec![
                endpoint(
                    "runpod_training_control",
                    PublicNetworkEndpointKind::TrainingControl,
                    "tls://runpod-dense.psionic.testnet:443",
                    vec![
                        DecentralizedNetworkRoleClass::CheckpointAuthority,
                        DecentralizedNetworkRoleClass::Aggregator,
                    ],
                    "RunPod training control endpoint for dense-side checkpoint and aggregation support under the permissioned registry.",
                ),
                endpoint(
                    "runpod_checkpoint_mirror",
                    PublicNetworkEndpointKind::CheckpointMirror,
                    "oci://runpod-dense.psionic.testnet/checkpoints",
                    vec![DecentralizedNetworkRoleClass::CheckpointAuthority],
                    "RunPod checkpoint mirror endpoint that makes public checkpoint-authority participation explicit without implying contributor-window miner admission.",
                ),
            ],
            PublicNetworkAvailabilityStatus::Online,
            PublicNetworkRelayPosture::NotAdvertised,
            "RunPod remains visible in registry and matchmaking for checkpoint and aggregation support, while the current public miner role remains explicitly unavailable on this dense-rank node.",
        ),
        build_registry_record(
            source_by_id["local_rtx4080_workstation"],
            identity_by_source["local_rtx4080_workstation"],
            manifest.program_manifest_id.as_str(),
            manifest.program_manifest_digest.as_str(),
            vec![endpoint(
                "local_rtx_training_control",
                PublicNetworkEndpointKind::TrainingControl,
                "tailscale://local-rtx4080.psionic.testnet:8443",
                vec![
                    DecentralizedNetworkRoleClass::PublicMiner,
                    DecentralizedNetworkRoleClass::Aggregator,
                ],
                "Trusted-LAN or Tailnet training control endpoint for the local RTX 4080 contributor-window miner and aggregator role set.",
            )],
            PublicNetworkAvailabilityStatus::Online,
            PublicNetworkRelayPosture::NotAdvertised,
            "The local RTX 4080 node is registry-visible for contributor-window miner and aggregator selection, but still refuses validator or checkpoint-authority roles.",
        ),
        build_registry_record(
            source_by_id["local_mlx_mac_workstation"],
            identity_by_source["local_mlx_mac_workstation"],
            manifest.program_manifest_id.as_str(),
            manifest.program_manifest_digest.as_str(),
            vec![endpoint(
                "local_mlx_training_control",
                PublicNetworkEndpointKind::TrainingControl,
                "tailscale://local-mlx.psionic.testnet:8443",
                vec![
                    DecentralizedNetworkRoleClass::PublicMiner,
                    DecentralizedNetworkRoleClass::PublicValidator,
                    DecentralizedNetworkRoleClass::Aggregator,
                ],
                "Tailnet training control endpoint for the Apple MLX node as a miner, validator, and aggregation-capable registry participant.",
            )],
            PublicNetworkAvailabilityStatus::Online,
            PublicNetworkRelayPosture::NotAdvertised,
            "The Apple MLX node stays registry-visible for miner and validator selection while checkpoint authority remains explicitly unavailable.",
        ),
    ];

    let compatibility_policy = PublicNetworkCompatibilityPolicy {
        required_release_id: String::from(SIGNED_NODE_IDENTITY_RELEASE_ID),
        required_environment_ref: manifest.environment.environment_ref.clone(),
        required_environment_version: manifest.environment.version.clone(),
        required_program_manifest_id: manifest.program_manifest_id.clone(),
        required_program_manifest_digest: manifest.program_manifest_digest.clone(),
        require_active_revocation_status: true,
        detail: String::from(
            "Registry admission and discovery stay bound to the current signed-node release id, the current program manifest digest, the canonical environment key, and active-only revocation posture. Nodes that drift outside that policy remain visible only as typed refusals.",
        ),
    };

    let discovery_examples = vec![
        discovery_example(
            "discover_public_miner_nodes",
            discovery_filter(
                DecentralizedNetworkRoleClass::PublicMiner,
                Some(CrossProviderExecutionClass::ValidatedContributorWindow),
                vec![
                    CrossProviderTrustTier::PrivateCloudOperatorManaged,
                    CrossProviderTrustTier::LocalOperatorManaged,
                ],
                true,
                false,
                "Discover contributor-window-capable public miner nodes for the current epoch while excluding dense-rank-only or validator-only participants.",
            ),
            &compatibility_policy,
            registry_records.as_slice(),
            "Registry discovery for the current public_miner role resolves the Google validator node plus the two local contributor-window nodes, and it explicitly refuses RunPod because the role and execution binding do not match yet.",
        ),
        discovery_example(
            "discover_public_validator_nodes",
            discovery_filter(
                DecentralizedNetworkRoleClass::PublicValidator,
                Some(CrossProviderExecutionClass::Validator),
                vec![
                    CrossProviderTrustTier::PrivateCloudOperatorManaged,
                    CrossProviderTrustTier::LocalOperatorManaged,
                ],
                true,
                false,
                "Discover validator-capable nodes for the current epoch with role, execution-class, and compatibility policy checks kept explicit.",
            ),
            &compatibility_policy,
            registry_records.as_slice(),
            "Registry discovery for the current validator quorum resolves the Google and Apple MLX nodes and refuses the remaining nodes through typed role or execution-class mismatch.",
        ),
        discovery_example(
            "discover_checkpoint_authority_nodes",
            discovery_filter(
                DecentralizedNetworkRoleClass::CheckpointAuthority,
                Some(CrossProviderExecutionClass::CheckpointWriter),
                vec![
                    CrossProviderTrustTier::PrivateCloudOperatorManaged,
                    CrossProviderTrustTier::RentedProviderOperatorManaged,
                ],
                true,
                false,
                "Discover checkpoint-authority nodes that can publish promoted checkpoint state for the current public epoch.",
            ),
            &compatibility_policy,
            registry_records.as_slice(),
            "Registry discovery for checkpoint authority resolves the Google and RunPod nodes only, which matches the current role and execution-class bindings frozen by the network contract and signed identities.",
        ),
        discovery_example(
            "discover_relay_nodes",
            discovery_filter(
                DecentralizedNetworkRoleClass::Relay,
                None,
                vec![
                    CrossProviderTrustTier::PrivateCloudOperatorManaged,
                    CrossProviderTrustTier::RentedProviderOperatorManaged,
                    CrossProviderTrustTier::LocalOperatorManaged,
                ],
                true,
                true,
                "Discover relay-advertising support nodes for the current public epoch without pretending every participant is a relay.",
            ),
            &compatibility_policy,
            registry_records.as_slice(),
            "Registry discovery for relay support resolves the Google node only, which keeps relay posture explicit instead of burying it in endpoint prose or private host notes.",
        ),
    ];

    let matchmaking_offers = vec![
        PublicNetworkMatchmakingOffer {
            offer_id: String::from("public_miner_window_offer_v1"),
            session_kind: PublicNetworkSessionKind::ContributorWindow,
            source_query_id: String::from("discover_public_miner_nodes"),
            selected_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("local_rtx4080_workstation.registry"),
            ],
            standby_registry_record_ids: vec![String::from("local_mlx_mac_workstation.registry")],
            relay_registry_record_ids: vec![String::from("google_l4_validator_node.registry")],
            checkpoint_authority_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
            ],
            detail: String::from(
                "The first contributor-window matchmaking offer picks Google plus the RTX 4080 node as the active miner pair, retains the Apple MLX node as standby, routes through the single current relay-capable node, and still points checkpoint promotion at the canonical Google plus RunPod authority set.",
            ),
        },
        PublicNetworkMatchmakingOffer {
            offer_id: String::from("validator_quorum_offer_v1"),
            session_kind: PublicNetworkSessionKind::ValidatorQuorum,
            source_query_id: String::from("discover_public_validator_nodes"),
            selected_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("local_mlx_mac_workstation.registry"),
            ],
            standby_registry_record_ids: vec![],
            relay_registry_record_ids: vec![String::from("google_l4_validator_node.registry")],
            checkpoint_authority_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
            ],
            detail: String::from(
                "The first validator matchmaking offer closes the current two-validator quorum directly against registry truth: Google plus Apple MLX as validators, Google as relay, and Google plus RunPod as checkpoint-authority support.",
            ),
        },
        PublicNetworkMatchmakingOffer {
            offer_id: String::from("checkpoint_promotion_offer_v1"),
            session_kind: PublicNetworkSessionKind::CheckpointPromotion,
            source_query_id: String::from("discover_checkpoint_authority_nodes"),
            selected_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
            ],
            standby_registry_record_ids: vec![],
            relay_registry_record_ids: vec![String::from("google_l4_validator_node.registry")],
            checkpoint_authority_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
            ],
            detail: String::from(
                "The first checkpoint-promotion offer keeps the authority pair explicit: Google plus RunPod are discoverable and selectable as checkpoint authorities, with Google also advertising the current relay ingress path.",
            ),
        },
    ];

    let mut contract = PublicNetworkRegistryContract {
        schema_version: String::from(PUBLIC_NETWORK_REGISTRY_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(PUBLIC_NETWORK_REGISTRY_CONTRACT_ID),
        network_id: network.network_id.clone(),
        governance_revision_id: network.governance_revision.governance_revision_id.clone(),
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        decentralized_network_contract_digest: network.contract_digest.clone(),
        signed_node_identity_contract_digest: identity_contract.contract_digest.clone(),
        compatibility_policy,
        registry_records,
        discovery_examples,
        matchmaking_offers,
        authority_paths: PublicNetworkRegistryAuthorityPaths {
            fixture_path: String::from(PUBLIC_NETWORK_REGISTRY_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(PUBLIC_NETWORK_REGISTRY_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(PUBLIC_NETWORK_REGISTRY_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(
                PUBLIC_NETWORK_REGISTRY_CONTRACT_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract closes one permissioned-testnet registry, discovery, and matchmaking surface above the signed node identity layer. It proves that nodes, roles, execution classes, trust tiers, relay posture, compatibility policy, and selected session offers can be discovered from one machine-legible contract instead of operator-only host lists. It does not claim permissionless gossip, public internet admission, or live elastic runtime yet.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = PUBLIC_NETWORK_REGISTRY_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_public_network_registry_contract(
    output_path: impl AsRef<Path>,
) -> Result<PublicNetworkRegistryContract, PublicNetworkRegistryContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PublicNetworkRegistryContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_public_network_registry_contract()?;
    let json = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| PublicNetworkRegistryContractError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(contract)
}

fn build_registry_record(
    source: &CrossProviderComputeSourceContract,
    identity: &SignedNodeIdentityRecord,
    program_manifest_id: &str,
    program_manifest_digest: &str,
    endpoints: Vec<PublicNetworkEndpoint>,
    availability_status: PublicNetworkAvailabilityStatus,
    relay_posture: PublicNetworkRelayPosture,
    detail: &str,
) -> PublicNetworkRegistryRecord {
    let mut record = PublicNetworkRegistryRecord {
        registry_record_id: format!("{}.registry", source.source_id),
        node_identity_id: identity.node_identity_id.clone(),
        node_id: identity.node_id.clone(),
        source_id: source.source_id.clone(),
        role_classes: identity.admitted_role_classes.clone(),
        execution_classes: identity.admitted_execution_classes.clone(),
        trust_tier: source.network.trust_tier,
        availability_status,
        relay_posture,
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        compatibility: registry_compatibility(
            identity,
            program_manifest_id,
            program_manifest_digest,
        ),
        endpoints,
        detail: String::from(detail),
        record_digest: String::new(),
    };
    record.record_digest = record.stable_digest();
    record
}

fn registry_compatibility(
    identity: &SignedNodeIdentityRecord,
    program_manifest_id: &str,
    program_manifest_digest: &str,
) -> PublicNetworkRegistryCompatibility {
    PublicNetworkRegistryCompatibility {
        release_id: identity.software_attestation.release_id.clone(),
        build_digest: identity.software_attestation.build_digest.clone(),
        environment_ref: identity.software_attestation.environment_ref.clone(),
        environment_version: identity.software_attestation.environment_version.clone(),
        program_manifest_id: String::from(program_manifest_id),
        program_manifest_digest: String::from(program_manifest_digest),
        revocation_status: identity.revocation_status,
        identity_record_digest: identity.record_digest.clone(),
        detail: format!(
            "Registry compatibility for `{}` binds release id, build digest, environment key, revocation posture, and identity record digest into one discovery-time compatibility object.",
            identity.source_id
        ),
    }
}

fn endpoint(
    endpoint_id: &str,
    endpoint_kind: PublicNetworkEndpointKind,
    uri: &str,
    advertised_role_classes: Vec<DecentralizedNetworkRoleClass>,
    detail: &str,
) -> PublicNetworkEndpoint {
    PublicNetworkEndpoint {
        endpoint_id: String::from(endpoint_id),
        endpoint_kind,
        uri: String::from(uri),
        advertised_role_classes,
        detail: String::from(detail),
    }
}

fn discovery_filter(
    requested_role_class: DecentralizedNetworkRoleClass,
    requested_execution_class: Option<CrossProviderExecutionClass>,
    admitted_trust_tiers: Vec<CrossProviderTrustTier>,
    require_online: bool,
    require_relay_advertisement: bool,
    detail: &str,
) -> PublicNetworkDiscoveryFilter {
    PublicNetworkDiscoveryFilter {
        requested_role_class,
        requested_execution_class,
        admitted_trust_tiers,
        require_online,
        require_relay_advertisement,
        detail: String::from(detail),
    }
}

fn discovery_example(
    query_id: &str,
    filter: PublicNetworkDiscoveryFilter,
    compatibility_policy: &PublicNetworkCompatibilityPolicy,
    registry_records: &[PublicNetworkRegistryRecord],
    detail: &str,
) -> PublicNetworkDiscoveryExample {
    let mut matched_registry_record_ids = Vec::new();
    let mut refusals = Vec::new();

    for record in registry_records {
        if record_matches_filter(record, &filter, compatibility_policy) {
            matched_registry_record_ids.push(record.registry_record_id.clone());
        } else {
            let refusal_kind = first_refusal_kind(record, &filter, compatibility_policy)
                .expect("non-matching record should produce a refusal kind");
            refusals.push(PublicNetworkDiscoveryRefusal {
                registry_record_id: record.registry_record_id.clone(),
                refusal_kind,
                detail: refusal_detail(record, &filter, refusal_kind),
            });
        }
    }

    PublicNetworkDiscoveryExample {
        query_id: String::from(query_id),
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        filter,
        matched_registry_record_ids,
        refusals,
        detail: String::from(detail),
    }
}

fn record_matches_filter(
    record: &PublicNetworkRegistryRecord,
    filter: &PublicNetworkDiscoveryFilter,
    compatibility_policy: &PublicNetworkCompatibilityPolicy,
) -> bool {
    first_refusal_kind(record, filter, compatibility_policy).is_none()
}

fn first_refusal_kind(
    record: &PublicNetworkRegistryRecord,
    filter: &PublicNetworkDiscoveryFilter,
    compatibility_policy: &PublicNetworkCompatibilityPolicy,
) -> Option<PublicNetworkDiscoveryRefusalKind> {
    if filter.require_online
        && record.availability_status != PublicNetworkAvailabilityStatus::Online
    {
        return Some(PublicNetworkDiscoveryRefusalKind::AvailabilityNotOnline);
    }
    if record.compatibility.release_id != compatibility_policy.required_release_id
        || record.compatibility.environment_ref != compatibility_policy.required_environment_ref
        || record.compatibility.environment_version
            != compatibility_policy.required_environment_version
        || record.compatibility.program_manifest_id
            != compatibility_policy.required_program_manifest_id
        || record.compatibility.program_manifest_digest
            != compatibility_policy.required_program_manifest_digest
        || (compatibility_policy.require_active_revocation_status
            && record.compatibility.revocation_status != SignedNodeRevocationStatus::Active)
    {
        return Some(PublicNetworkDiscoveryRefusalKind::CompatibilityPolicyDrift);
    }
    if !record.role_classes.contains(&filter.requested_role_class) {
        return Some(PublicNetworkDiscoveryRefusalKind::RoleClassNotAdmitted);
    }
    if let Some(requested_execution_class) = filter.requested_execution_class {
        if !record
            .execution_classes
            .contains(&requested_execution_class)
        {
            return Some(PublicNetworkDiscoveryRefusalKind::ExecutionClassNotAdmitted);
        }
    }
    if !filter.admitted_trust_tiers.contains(&record.trust_tier) {
        return Some(PublicNetworkDiscoveryRefusalKind::TrustTierNotAdmitted);
    }
    if filter.require_relay_advertisement
        && record.relay_posture != PublicNetworkRelayPosture::Advertised
    {
        return Some(PublicNetworkDiscoveryRefusalKind::RelayNotAdvertised);
    }
    None
}

fn refusal_detail(
    record: &PublicNetworkRegistryRecord,
    filter: &PublicNetworkDiscoveryFilter,
    refusal_kind: PublicNetworkDiscoveryRefusalKind,
) -> String {
    match refusal_kind {
        PublicNetworkDiscoveryRefusalKind::AvailabilityNotOnline => format!(
            "Registry record `{}` is not currently online for epoch `{}`.",
            record.registry_record_id, record.current_epoch_id
        ),
        PublicNetworkDiscoveryRefusalKind::CompatibilityPolicyDrift => format!(
            "Registry record `{}` drifted outside the current release, environment, or revocation compatibility policy.",
            record.registry_record_id
        ),
        PublicNetworkDiscoveryRefusalKind::RoleClassNotAdmitted => format!(
            "Registry record `{}` does not admit role `{:?}`.",
            record.registry_record_id, filter.requested_role_class
        ),
        PublicNetworkDiscoveryRefusalKind::ExecutionClassNotAdmitted => format!(
            "Registry record `{}` does not admit execution class `{:?}` for role `{:?}`.",
            record.registry_record_id,
            filter
                .requested_execution_class
                .expect("execution-class refusal must have requested execution class"),
            filter.requested_role_class
        ),
        PublicNetworkDiscoveryRefusalKind::TrustTierNotAdmitted => format!(
            "Registry record `{}` trust tier `{:?}` is outside the admitted discovery filter.",
            record.registry_record_id, record.trust_tier
        ),
        PublicNetworkDiscoveryRefusalKind::RelayNotAdvertised => format!(
            "Registry record `{}` does not advertise relay posture for the current discovery query.",
            record.registry_record_id
        ),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("public network registry contract digest serialization must work"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_network_registry_contract_stays_valid() {
        let contract = canonical_public_network_registry_contract()
            .expect("public network registry contract should build");
        contract.validate().expect("contract should validate");
    }

    #[test]
    fn public_network_registry_contract_discovers_expected_validator_quorum() {
        let contract = canonical_public_network_registry_contract()
            .expect("public network registry contract should build");
        let query = contract
            .discovery_examples
            .iter()
            .find(|query| query.query_id == "discover_public_validator_nodes")
            .expect("validator discovery query should exist");
        assert_eq!(
            query.matched_registry_record_ids,
            vec![
                String::from("google_l4_validator_node.registry"),
                String::from("local_mlx_mac_workstation.registry"),
            ]
        );
    }

    #[test]
    fn public_network_registry_contract_retains_single_relay_node() {
        let contract = canonical_public_network_registry_contract()
            .expect("public network registry contract should build");
        let relay_records = contract
            .registry_records
            .iter()
            .filter(|record| {
                record
                    .role_classes
                    .contains(&DecentralizedNetworkRoleClass::Relay)
            })
            .map(|record| record.registry_record_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(relay_records, vec!["google_l4_validator_node.registry"]);
    }
}
