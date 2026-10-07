use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, canonical_decentralized_network_contract,
    cross_provider_training_program_manifest, CrossProviderComputeSourceContract,
    CrossProviderComputeSourceContractError, CrossProviderExecutionClass,
    CrossProviderTrainingProgramManifestError, DecentralizedNetworkContractError,
    DecentralizedNetworkRoleBindingKind, DecentralizedNetworkRoleClass,
};

pub const SIGNED_NODE_IDENTITY_CONTRACT_SET_SCHEMA_VERSION: &str =
    "psionic.signed_node_identity_contract_set.v1";
pub const SIGNED_NODE_IDENTITY_CONTRACT_SET_ID: &str =
    "psionic.signed_node_identity_contract_set.v1";
pub const SIGNED_NODE_IDENTITY_CONTRACT_SET_FIXTURE_PATH: &str =
    "fixtures/training/signed_node_identity_contract_set_v1.json";
pub const SIGNED_NODE_IDENTITY_CONTRACT_SET_CHECK_SCRIPT_PATH: &str =
    "scripts/check-signed-node-identity-contract-set.sh";
pub const SIGNED_NODE_IDENTITY_CONTRACT_SET_DOC_PATH: &str =
    "docs/SIGNED_NODE_IDENTITY_CONTRACT_REFERENCE.md";
pub const SIGNED_NODE_IDENTITY_CONTRACT_SET_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";
pub const SIGNED_NODE_IDENTITY_RELEASE_ID: &str =
    "psionic.decentralized_training_identity_release.v1";
pub const SIGNED_NODE_IDENTITY_REVOCATION_POLICY_ID: &str =
    "psionic.decentralized_training_identity_revocation.v1";

#[derive(Debug, Error)]
pub enum SignedNodeIdentityContractSetError {
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
    #[error("signed node identity contract set is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignedNodeWalletKind {
    EthereumSecp256k1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignedNodeSignatureScheme {
    Ed25519Detached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignedNodeRevocationStatus {
    Active,
    PendingRevocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignedNodeCapabilityRefusalKind {
    RoleClassNotAdmitted,
    ExecutionClassNotAdmitted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeIdentityWalletBinding {
    pub wallet_kind: SignedNodeWalletKind,
    pub wallet_address: String,
    pub settlement_namespace: String,
    pub operator_namespace: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeSoftwareAttestation {
    pub release_id: String,
    pub environment_ref: String,
    pub environment_version: String,
    pub build_digest: String,
    pub authority_artifact_paths: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeCapabilityProjection {
    pub accelerator_inventory_digest: String,
    pub backend_posture_digest: String,
    pub network_posture_digest: String,
    pub storage_posture_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeBenchmarkEvidence {
    pub benchmark_id: String,
    pub authority_artifact_path: String,
    pub authority_artifact_sha256: String,
    pub admitted_execution_classes: Vec<CrossProviderExecutionClass>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeCapabilityRefusalExample {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_role_class: Option<DecentralizedNetworkRoleClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_execution_class: Option<CrossProviderExecutionClass>,
    pub refusal_kind: SignedNodeCapabilityRefusalKind,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeCapabilitySignature {
    pub signature_scheme: SignedNodeSignatureScheme,
    pub signer_key_id: String,
    pub public_key_hex: String,
    pub payload_digest: String,
    pub signature_hex: String,
    pub signed_at_unix_ms: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeIdentityRecord {
    pub node_identity_id: String,
    pub node_id: String,
    pub source_id: String,
    pub source_contract_digest: String,
    pub wallet: SignedNodeIdentityWalletBinding,
    pub software_attestation: SignedNodeSoftwareAttestation,
    pub capability_projection: SignedNodeCapabilityProjection,
    pub benchmark_evidence: Vec<SignedNodeBenchmarkEvidence>,
    pub admitted_role_classes: Vec<DecentralizedNetworkRoleClass>,
    pub admitted_execution_classes: Vec<CrossProviderExecutionClass>,
    pub revocation_status: SignedNodeRevocationStatus,
    pub capability_signature: SignedNodeCapabilitySignature,
    pub refusal_examples: Vec<SignedNodeCapabilityRefusalExample>,
    pub detail: String,
    pub record_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeIdentityRevocationAuthority {
    pub policy_id: String,
    pub revocation_feed_template: String,
    pub grace_epochs_before_enforcement: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeIdentityAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeIdentityContractSet {
    pub schema_version: String,
    pub contract_id: String,
    pub network_id: String,
    pub governance_revision_id: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub environment_ref: String,
    pub environment_version: String,
    pub decentralized_network_contract_digest: String,
    pub revocation_authority: SignedNodeIdentityRevocationAuthority,
    pub identities: Vec<SignedNodeIdentityRecord>,
    pub authority_paths: SignedNodeIdentityAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct SignedNodeIdentitySigningPayload {
    node_identity_id: String,
    node_id: String,
    source_id: String,
    source_contract_digest: String,
    wallet: SignedNodeIdentityWalletBinding,
    software_attestation: SignedNodeSoftwareAttestation,
    capability_projection: SignedNodeCapabilityProjection,
    benchmark_evidence: Vec<SignedNodeBenchmarkEvidence>,
    admitted_role_classes: Vec<DecentralizedNetworkRoleClass>,
    admitted_execution_classes: Vec<CrossProviderExecutionClass>,
    revocation_status: SignedNodeRevocationStatus,
    refusal_examples: Vec<SignedNodeCapabilityRefusalExample>,
    detail: String,
}

impl SignedNodeIdentityRecord {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.record_digest.clear();
        stable_digest(b"psionic_signed_node_identity_record|", &clone)
    }

    fn signing_payload(&self) -> SignedNodeIdentitySigningPayload {
        SignedNodeIdentitySigningPayload {
            node_identity_id: self.node_identity_id.clone(),
            node_id: self.node_id.clone(),
            source_id: self.source_id.clone(),
            source_contract_digest: self.source_contract_digest.clone(),
            wallet: self.wallet.clone(),
            software_attestation: self.software_attestation.clone(),
            capability_projection: self.capability_projection.clone(),
            benchmark_evidence: self.benchmark_evidence.clone(),
            admitted_role_classes: self.admitted_role_classes.clone(),
            admitted_execution_classes: self.admitted_execution_classes.clone(),
            revocation_status: self.revocation_status,
            refusal_examples: self.refusal_examples.clone(),
            detail: self.detail.clone(),
        }
    }

    fn signing_payload_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.signing_payload())
            .expect("signed node identity signing payload serialization must work")
    }
}

impl SignedNodeIdentityContractSet {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_signed_node_identity_contract_set|", &clone)
    }

    pub fn validate(&self) -> Result<(), SignedNodeIdentityContractSetError> {
        let manifest = cross_provider_training_program_manifest()?;
        let network = canonical_decentralized_network_contract()?;
        let sources = canonical_cross_provider_compute_source_contracts()?;
        let source_by_id = sources
            .iter()
            .map(|source| (source.source_id.as_str(), source))
            .collect::<BTreeMap<_, _>>();
        let role_binding_by_role = network
            .role_bindings
            .iter()
            .map(|binding| (binding.role_class, binding))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != SIGNED_NODE_IDENTITY_CONTRACT_SET_SCHEMA_VERSION {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    SIGNED_NODE_IDENTITY_CONTRACT_SET_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != SIGNED_NODE_IDENTITY_CONTRACT_SET_ID {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.network_id != network.network_id {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("network_id drifted"),
            });
        }
        if self.governance_revision_id != network.governance_revision.governance_revision_id {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("governance_revision_id drifted"),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("program manifest binding drifted"),
            });
        }
        if self.environment_ref != manifest.environment.environment_ref
            || self.environment_version != manifest.environment.version
        {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("environment binding drifted"),
            });
        }
        if self.decentralized_network_contract_digest != network.contract_digest {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("decentralized network contract digest drifted"),
            });
        }
        if self.revocation_authority.policy_id != SIGNED_NODE_IDENTITY_REVOCATION_POLICY_ID {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("revocation policy id drifted"),
            });
        }
        if self.revocation_authority.grace_epochs_before_enforcement == 0 {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("revocation grace must stay non-zero"),
            });
        }
        if self.authority_paths.fixture_path != SIGNED_NODE_IDENTITY_CONTRACT_SET_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != SIGNED_NODE_IDENTITY_CONTRACT_SET_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != SIGNED_NODE_IDENTITY_CONTRACT_SET_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != SIGNED_NODE_IDENTITY_CONTRACT_SET_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.identities.len() != sources.len() {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: format!(
                    "expected exactly {} canonical signed identities but found {}",
                    sources.len(),
                    self.identities.len()
                ),
            });
        }

        let mut identity_ids = BTreeSet::new();
        let mut node_ids = BTreeSet::new();
        let mut source_ids = BTreeSet::new();
        let mut wallet_addresses = BTreeSet::new();

        for identity in &self.identities {
            if !identity_ids.insert(identity.node_identity_id.clone()) {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!("duplicate node_identity_id `{}`", identity.node_identity_id),
                });
            }
            if !node_ids.insert(identity.node_id.clone()) {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!("duplicate node_id `{}`", identity.node_id),
                });
            }
            if !source_ids.insert(identity.source_id.clone()) {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!("duplicate source_id `{}`", identity.source_id),
                });
            }
            if !wallet_addresses.insert(identity.wallet.wallet_address.clone()) {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "duplicate wallet address `{}`",
                        identity.wallet.wallet_address
                    ),
                });
            }

            let source = source_by_id
                .get(identity.source_id.as_str())
                .ok_or_else(|| SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` references unknown source",
                        identity.node_identity_id
                    ),
                })?;
            if identity.source_contract_digest != source.contract_digest {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` lost its source contract digest binding",
                        identity.node_identity_id
                    ),
                });
            }
            if identity.admitted_execution_classes != source.admitted_execution_classes {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` execution classes drifted from source `{}`",
                        identity.node_identity_id, identity.source_id
                    ),
                });
            }
            if identity.wallet.wallet_kind != SignedNodeWalletKind::EthereumSecp256k1 {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` wallet kind drifted",
                        identity.node_identity_id
                    ),
                });
            }
            if identity.wallet.settlement_namespace
                != network.settlement_backend.settlement_namespace
            {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` settlement namespace drifted",
                        identity.node_identity_id
                    ),
                });
            }
            if identity.software_attestation.release_id != SIGNED_NODE_IDENTITY_RELEASE_ID {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` release id drifted",
                        identity.node_identity_id
                    ),
                });
            }
            if identity.software_attestation.environment_ref != manifest.environment.environment_ref
                || identity.software_attestation.environment_version != manifest.environment.version
            {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` software environment drifted",
                        identity.node_identity_id
                    ),
                });
            }
            let expected_build_digest = build_digest(
                identity.source_id.as_str(),
                manifest.environment.environment_ref.as_str(),
                manifest.environment.version.as_str(),
                network.network_id.as_str(),
            );
            if identity.software_attestation.build_digest != expected_build_digest {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` build digest drifted",
                        identity.node_identity_id
                    ),
                });
            }
            let expected_capability_projection = capability_projection(source);
            if identity.capability_projection != expected_capability_projection {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` capability projection drifted from source contract",
                        identity.node_identity_id
                    ),
                });
            }
            if identity.benchmark_evidence.is_empty() {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` must retain benchmark evidence",
                        identity.node_identity_id
                    ),
                });
            }
            for evidence in &identity.benchmark_evidence {
                if evidence.admitted_execution_classes.is_empty() {
                    return Err(SignedNodeIdentityContractSetError::InvalidContract {
                        detail: format!(
                            "identity `{}` benchmark `{}` lost admitted execution classes",
                            identity.node_identity_id, evidence.benchmark_id
                        ),
                    });
                }
                if evidence
                    .admitted_execution_classes
                    .iter()
                    .any(|execution_class| {
                        !identity
                            .admitted_execution_classes
                            .contains(execution_class)
                    })
                {
                    return Err(SignedNodeIdentityContractSetError::InvalidContract {
                        detail: format!(
                            "identity `{}` benchmark `{}` cites execution classes outside the node admission set",
                            identity.node_identity_id, evidence.benchmark_id
                        ),
                    });
                }
                let matched_artifact = source.authority_artifacts.iter().any(|artifact| {
                    artifact.path == evidence.authority_artifact_path
                        && artifact.sha256 == evidence.authority_artifact_sha256
                });
                if !matched_artifact {
                    return Err(SignedNodeIdentityContractSetError::InvalidContract {
                        detail: format!(
                            "identity `{}` benchmark `{}` no longer resolves to a source authority artifact",
                            identity.node_identity_id, evidence.benchmark_id
                        ),
                    });
                }
            }

            let role_classes = identity
                .admitted_role_classes
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if role_classes.len() != identity.admitted_role_classes.len() {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` repeats admitted role classes",
                        identity.node_identity_id
                    ),
                });
            }
            for role_class in &identity.admitted_role_classes {
                let binding = role_binding_by_role.get(role_class).ok_or_else(|| {
                    SignedNodeIdentityContractSetError::InvalidContract {
                        detail: format!(
                            "identity `{}` cites unknown role class `{:?}`",
                            identity.node_identity_id, role_class
                        ),
                    }
                })?;
                match binding.binding_kind {
                    DecentralizedNetworkRoleBindingKind::DirectExecutionBinding => {
                        let execution_class = binding.execution_class.ok_or_else(|| {
                            SignedNodeIdentityContractSetError::InvalidContract {
                                detail: format!(
                                    "role `{:?}` lost its execution class binding",
                                    role_class
                                ),
                            }
                        })?;
                        if !identity
                            .admitted_execution_classes
                            .contains(&execution_class)
                        {
                            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                                detail: format!(
                                    "identity `{}` admits role `{:?}` without execution class `{:?}`",
                                    identity.node_identity_id, role_class, execution_class
                                ),
                            });
                        }
                    }
                    DecentralizedNetworkRoleBindingKind::NetworkOnlySupportRole => {}
                }
            }

            if identity.refusal_examples.is_empty() {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` must retain refusal examples",
                        identity.node_identity_id
                    ),
                });
            }
            for refusal in &identity.refusal_examples {
                if refusal.requested_role_class.is_none()
                    && refusal.requested_execution_class.is_none()
                {
                    return Err(SignedNodeIdentityContractSetError::InvalidContract {
                        detail: format!(
                            "identity `{}` has an empty refusal example",
                            identity.node_identity_id
                        ),
                    });
                }
                if let Some(role_class) = refusal.requested_role_class {
                    if identity.admitted_role_classes.contains(&role_class) {
                        return Err(SignedNodeIdentityContractSetError::InvalidContract {
                            detail: format!(
                                "identity `{}` refusal example cites already admitted role `{:?}`",
                                identity.node_identity_id, role_class
                            ),
                        });
                    }
                    if !role_binding_by_role.contains_key(&role_class) {
                        return Err(SignedNodeIdentityContractSetError::InvalidContract {
                            detail: format!(
                                "identity `{}` refusal example cites unknown role `{:?}`",
                                identity.node_identity_id, role_class
                            ),
                        });
                    }
                }
                if let Some(execution_class) = refusal.requested_execution_class {
                    if identity
                        .admitted_execution_classes
                        .contains(&execution_class)
                    {
                        return Err(SignedNodeIdentityContractSetError::InvalidContract {
                            detail: format!(
                                "identity `{}` refusal example cites already admitted execution class `{:?}`",
                                identity.node_identity_id, execution_class
                            ),
                        });
                    }
                }
            }

            let expected_signer_key_id = signer_key_id(identity.source_id.as_str());
            if identity.capability_signature.signature_scheme
                != SignedNodeSignatureScheme::Ed25519Detached
            {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` signature scheme drifted",
                        identity.node_identity_id
                    ),
                });
            }
            if identity.capability_signature.signer_key_id != expected_signer_key_id {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` signer key id drifted",
                        identity.node_identity_id
                    ),
                });
            }
            let payload_bytes = identity.signing_payload_bytes();
            let payload_digest = sha256_hex(payload_bytes.as_slice());
            if identity.capability_signature.payload_digest != payload_digest {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` payload digest drifted",
                        identity.node_identity_id
                    ),
                });
            }
            let signing_key = deterministic_signing_key(expected_signer_key_id.as_str());
            let verifying_key = signing_key.verifying_key();
            let expected_public_key_hex = hex::encode(verifying_key.to_bytes());
            if identity.capability_signature.public_key_hex != expected_public_key_hex {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` public key drifted",
                        identity.node_identity_id
                    ),
                });
            }
            let signature_bytes = hex::decode(&identity.capability_signature.signature_hex)
                .map_err(
                    |error| SignedNodeIdentityContractSetError::InvalidContract {
                        detail: format!(
                            "identity `{}` signature hex is invalid: {error}",
                            identity.node_identity_id
                        ),
                    },
                )?;
            let signature_array: [u8; 64] = signature_bytes.try_into().map_err(|_| {
                SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` signature length drifted",
                        identity.node_identity_id
                    ),
                }
            })?;
            let signature = Signature::from_bytes(&signature_array);
            verifying_key
                .verify_strict(payload_bytes.as_slice(), &signature)
                .map_err(
                    |error| SignedNodeIdentityContractSetError::InvalidContract {
                        detail: format!(
                            "identity `{}` signature verification failed: {error}",
                            identity.node_identity_id
                        ),
                    },
                )?;
            if identity.record_digest != identity.stable_digest() {
                return Err(SignedNodeIdentityContractSetError::InvalidContract {
                    detail: format!(
                        "identity `{}` record digest drifted",
                        identity.node_identity_id
                    ),
                });
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(SignedNodeIdentityContractSetError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

static SIGNED_NODE_IDENTITY_CONTRACT_SET_CACHE: std::sync::OnceLock<SignedNodeIdentityContractSet> =
    std::sync::OnceLock::new();

pub fn canonical_signed_node_identity_contract_set(
) -> Result<SignedNodeIdentityContractSet, SignedNodeIdentityContractSetError> {
    if let Some(contract) = SIGNED_NODE_IDENTITY_CONTRACT_SET_CACHE.get() {
        return Ok(contract.clone());
    }
    let manifest = cross_provider_training_program_manifest()?;
    let network = canonical_decentralized_network_contract()?;
    let sources = canonical_cross_provider_compute_source_contracts()?;
    let source_by_id = sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();

    let mut contract = SignedNodeIdentityContractSet {
        schema_version: String::from(SIGNED_NODE_IDENTITY_CONTRACT_SET_SCHEMA_VERSION),
        contract_id: String::from(SIGNED_NODE_IDENTITY_CONTRACT_SET_ID),
        network_id: network.network_id.clone(),
        governance_revision_id: network.governance_revision.governance_revision_id.clone(),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        environment_ref: manifest.environment.environment_ref.clone(),
        environment_version: manifest.environment.version.clone(),
        decentralized_network_contract_digest: network.contract_digest.clone(),
        revocation_authority: SignedNodeIdentityRevocationAuthority {
            policy_id: String::from(SIGNED_NODE_IDENTITY_REVOCATION_POLICY_ID),
            revocation_feed_template: String::from(
                "runs/${RUN_ID}/decentralized/revocations/network_epoch_${NETWORK_EPOCH}.json",
            ),
            grace_epochs_before_enforcement: 2,
            detail: String::from(
                "The first signed-node identity surface keeps revocation posture explicit through an epoch-scoped revocation feed with a two-epoch grace window. It does not claim automatic slashing or hardware-backed remote attestation yet.",
            ),
        },
        identities: vec![
            build_identity_record(
                source_by_id["google_l4_validator_node"],
                &manifest.environment.environment_ref,
                &manifest.environment.version,
                &network.network_id,
                &network.settlement_backend.settlement_namespace,
                "psionic-google-validator-001",
                vec![
                    DecentralizedNetworkRoleClass::PublicMiner,
                    DecentralizedNetworkRoleClass::PublicValidator,
                    DecentralizedNetworkRoleClass::CheckpointAuthority,
                    DecentralizedNetworkRoleClass::Aggregator,
                    DecentralizedNetworkRoleClass::Relay,
                ],
                vec![
                    benchmark_evidence(
                        source_by_id["google_l4_validator_node"],
                        "google_swarm_contract_benchmark",
                        "fixtures/psion/google/psion_google_two_node_swarm_contract_v1.json",
                        vec![
                            CrossProviderExecutionClass::ValidatedContributorWindow,
                            CrossProviderExecutionClass::Validator,
                            CrossProviderExecutionClass::CheckpointWriter,
                        ],
                        "Google coordinator evidence ties the admitted validator, contributor-window, and checkpoint-writer posture back to the retained two-node swarm contract.",
                    ),
                    benchmark_evidence(
                        source_by_id["google_l4_validator_node"],
                        "google_identity_profile_benchmark",
                        "fixtures/psion/google/psion_google_two_node_swarm_identity_profile_v1.json",
                        vec![
                            CrossProviderExecutionClass::ValidatedContributorWindow,
                            CrossProviderExecutionClass::Validator,
                            CrossProviderExecutionClass::DataBuilder,
                        ],
                        "Identity-profile evidence freezes operator role tags and the admitted aggregator-side data-builder seam for the same Google node.",
                    ),
                ],
                vec![refuse_execution_class(
                    CrossProviderExecutionClass::DenseFullModelRank,
                    "The Google L4 validator node does not admit dense_full_model_rank in the current source contract, so the signed identity refuses dense-rank claims explicitly instead of letting public admission prose widen the node.",
                )],
                "Google private-cloud coordinator node retained as the strongest multi-role public testnet participant: validator, checkpoint authority, contributor-window miner, aggregator, and relay support.",
            ),
            build_identity_record(
                source_by_id["runpod_8xh100_dense_node"],
                &manifest.environment.environment_ref,
                &manifest.environment.version,
                &network.network_id,
                &network.settlement_backend.settlement_namespace,
                "psionic-runpod-dense-001",
                vec![
                    DecentralizedNetworkRoleClass::CheckpointAuthority,
                    DecentralizedNetworkRoleClass::Aggregator,
                ],
                vec![
                    benchmark_evidence(
                        source_by_id["runpod_8xh100_dense_node"],
                        "runpod_launch_profile_benchmark",
                        "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_launch_profiles_v1.json",
                        vec![
                            CrossProviderExecutionClass::DenseFullModelRank,
                            CrossProviderExecutionClass::CheckpointWriter,
                        ],
                        "RunPod launch-profile evidence proves the exact eight-H100 dense-rank lane and the attached checkpoint-writer posture retained in the current source contract.",
                    ),
                    benchmark_evidence(
                        source_by_id["runpod_8xh100_dense_node"],
                        "runpod_operator_preflight_benchmark",
                        "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_operator_preflight_policy_v1.json",
                        vec![
                            CrossProviderExecutionClass::DenseFullModelRank,
                            CrossProviderExecutionClass::EvalWorker,
                            CrossProviderExecutionClass::DataBuilder,
                        ],
                        "RunPod preflight evidence keeps dense-rank readiness, eval-worker posture, and artifact-building support tied to the retained operator preflight policy.",
                    ),
                ],
                vec![
                    refuse_role_class(
                        DecentralizedNetworkRoleClass::PublicMiner,
                        "The current decentralized network binds public_miner to validated_contributor_window rather than dense_full_model_rank, so the RunPod dense node refuses public_miner identity claims honestly in this issue.",
                    ),
                    refuse_execution_class(
                        CrossProviderExecutionClass::Validator,
                        "The RunPod eight-H100 dense node does not admit validator execution in the current source contract, so validator identity claims stay fail-closed here.",
                    ),
                ],
                "RunPod dense node retained as dense compute and public checkpoint sidecar capacity, but not yet as a public contributor-window miner under the current network role binding.",
            ),
            build_identity_record(
                source_by_id["local_rtx4080_workstation"],
                &manifest.environment.environment_ref,
                &manifest.environment.version,
                &network.network_id,
                &network.settlement_backend.settlement_namespace,
                "psionic-local-rtx4080-001",
                vec![
                    DecentralizedNetworkRoleClass::PublicMiner,
                    DecentralizedNetworkRoleClass::Aggregator,
                ],
                vec![benchmark_evidence(
                    source_by_id["local_rtx4080_workstation"],
                    "local_rtx4080_bringup_benchmark",
                    "fixtures/swarm/reports/swarm_linux_rtx4080_bringup_v1.json",
                    vec![
                        CrossProviderExecutionClass::ValidatedContributorWindow,
                        CrossProviderExecutionClass::EvalWorker,
                        CrossProviderExecutionClass::DataBuilder,
                    ],
                    "RTX 4080 bring-up evidence keeps contributor-window, eval-worker, and data-builder capability claims tied to the retained trusted-LAN workstation report.",
                )],
                vec![
                    refuse_role_class(
                        DecentralizedNetworkRoleClass::PublicValidator,
                        "The local RTX 4080 workstation does not admit validator execution in the current source contract, so public_validator admission remains explicitly refused.",
                    ),
                    refuse_execution_class(
                        CrossProviderExecutionClass::CheckpointWriter,
                        "The local RTX 4080 workstation does not currently carry checkpoint_writer in the source contract, so checkpoint-authority identity claims remain blocked here.",
                    ),
                ],
                "Local Linux RTX 4080 workstation retained as a contributor-window public miner and aggregator-side support node under operator-managed admission.",
            ),
            build_identity_record(
                source_by_id["local_mlx_mac_workstation"],
                &manifest.environment.environment_ref,
                &manifest.environment.version,
                &network.network_id,
                &network.settlement_backend.settlement_namespace,
                "psionic-local-mlx-001",
                vec![
                    DecentralizedNetworkRoleClass::PublicMiner,
                    DecentralizedNetworkRoleClass::PublicValidator,
                    DecentralizedNetworkRoleClass::Aggregator,
                ],
                vec![benchmark_evidence(
                    source_by_id["local_mlx_mac_workstation"],
                    "local_mlx_bringup_benchmark",
                    "fixtures/swarm/reports/swarm_mac_mlx_bringup_v1.json",
                    vec![
                        CrossProviderExecutionClass::DenseFullModelRank,
                        CrossProviderExecutionClass::ValidatedContributorWindow,
                        CrossProviderExecutionClass::Validator,
                        CrossProviderExecutionClass::DataBuilder,
                    ],
                    "MLX bring-up evidence keeps the Apple workstation's dense-rank, contributor-window, validator, and data-builder claims bound to the retained bring-up report on the committed host family.",
                )],
                vec![refuse_role_class(
                    DecentralizedNetworkRoleClass::CheckpointAuthority,
                    "The Apple MLX workstation does not admit checkpoint_writer in the current source contract, so checkpoint_authority stays explicitly refused even though the node can validate and aggregate.",
                )],
                "Local Apple Silicon workstation retained as a mixed-role testnet node for contributor-window mining, validation, and aggregation, while dense-rank capability remains outside the current public role map.",
            ),
        ],
        authority_paths: SignedNodeIdentityAuthorityPaths {
            fixture_path: String::from(SIGNED_NODE_IDENTITY_CONTRACT_SET_FIXTURE_PATH),
            check_script_path: String::from(SIGNED_NODE_IDENTITY_CONTRACT_SET_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(SIGNED_NODE_IDENTITY_CONTRACT_SET_DOC_PATH),
            train_system_doc_path: String::from(
                SIGNED_NODE_IDENTITY_CONTRACT_SET_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract set closes one signed public-node identity layer above the canonical compute-source contracts and the decentralized network contract. It binds wallet, software build digest, capability digests, benchmark evidence, admitted roles, admitted execution classes, typed refusals, and revocation posture into one machine-legible surface. It does not claim hardware-backed attestation, on-chain wallet verification, permissionless registration, or automatic slashing.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = SIGNED_NODE_IDENTITY_CONTRACT_SET_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_signed_node_identity_contract_set(
    output_path: impl AsRef<Path>,
) -> Result<SignedNodeIdentityContractSet, SignedNodeIdentityContractSetError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            SignedNodeIdentityContractSetError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_signed_node_identity_contract_set()?;
    let json = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| SignedNodeIdentityContractSetError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(contract)
}

fn build_identity_record(
    source: &CrossProviderComputeSourceContract,
    environment_ref: &str,
    environment_version: &str,
    network_id: &str,
    settlement_namespace: &str,
    node_label: &str,
    admitted_role_classes: Vec<DecentralizedNetworkRoleClass>,
    benchmark_evidence: Vec<SignedNodeBenchmarkEvidence>,
    refusal_examples: Vec<SignedNodeCapabilityRefusalExample>,
    detail: &str,
) -> SignedNodeIdentityRecord {
    let node_identity_id = format!("{}.identity", source.source_id);
    let wallet = SignedNodeIdentityWalletBinding {
        wallet_kind: SignedNodeWalletKind::EthereumSecp256k1,
        wallet_address: wallet_address(source.source_id.as_str()),
        settlement_namespace: String::from(settlement_namespace),
        operator_namespace: format!("psionic.decentralized_testnet.{}", source.source_id),
        detail: format!(
            "The first signed identity retains one operator wallet namespace for `{}` so settlement and reward accounting can bind to a machine-legible address before on-chain publication exists.",
            source.source_id
        ),
    };
    let software_attestation = SignedNodeSoftwareAttestation {
        release_id: String::from(SIGNED_NODE_IDENTITY_RELEASE_ID),
        environment_ref: String::from(environment_ref),
        environment_version: String::from(environment_version),
        build_digest: build_digest(
            source.source_id.as_str(),
            environment_ref,
            environment_version,
            network_id,
        ),
        authority_artifact_paths: source
            .authority_artifacts
            .iter()
            .map(|artifact| artifact.path.clone())
            .collect(),
        detail: format!(
            "The first public identity layer freezes one deterministic software build digest for `{}` above the canonical environment key and the retained source authority artifacts.",
            source.source_id
        ),
    };
    let capability_projection = capability_projection(source);
    let mut identity = SignedNodeIdentityRecord {
        node_identity_id,
        node_id: String::from(node_label),
        source_id: source.source_id.clone(),
        source_contract_digest: source.contract_digest.clone(),
        wallet,
        software_attestation,
        capability_projection,
        benchmark_evidence,
        admitted_role_classes,
        admitted_execution_classes: source.admitted_execution_classes.clone(),
        revocation_status: SignedNodeRevocationStatus::Active,
        capability_signature: SignedNodeCapabilitySignature {
            signature_scheme: SignedNodeSignatureScheme::Ed25519Detached,
            signer_key_id: signer_key_id(source.source_id.as_str()),
            public_key_hex: String::new(),
            payload_digest: String::new(),
            signature_hex: String::new(),
            signed_at_unix_ms: 1_711_111_123_000,
            detail: format!(
                "The canonical signed identity uses one deterministic ed25519 testnet signer for `{}` so admission, validator scoring, and fraud review all point at the same stable payload.",
                source.source_id
            ),
        },
        refusal_examples,
        detail: String::from(detail),
        record_digest: String::new(),
    };
    sign_identity_record(&mut identity);
    identity.record_digest = identity.stable_digest();
    identity
}

fn capability_projection(
    source: &CrossProviderComputeSourceContract,
) -> SignedNodeCapabilityProjection {
    SignedNodeCapabilityProjection {
        accelerator_inventory_digest: stable_digest(
            b"psionic_signed_node_identity_accelerators|",
            &source.accelerators,
        ),
        backend_posture_digest: stable_digest(
            b"psionic_signed_node_identity_backend|",
            &source.backend,
        ),
        network_posture_digest: stable_digest(
            b"psionic_signed_node_identity_network|",
            &source.network,
        ),
        storage_posture_digest: stable_digest(
            b"psionic_signed_node_identity_storage|",
            &source.storage,
        ),
        detail: format!(
            "The capability projection freezes accelerator, backend, network, and storage truth for `{}` without widening beyond the source contract already admitted by the program manifest.",
            source.source_id
        ),
    }
}

fn benchmark_evidence(
    source: &CrossProviderComputeSourceContract,
    benchmark_id: &str,
    authority_artifact_path: &str,
    admitted_execution_classes: Vec<CrossProviderExecutionClass>,
    detail: &str,
) -> SignedNodeBenchmarkEvidence {
    let authority_artifact = source
        .authority_artifacts
        .iter()
        .find(|artifact| artifact.path == authority_artifact_path)
        .unwrap_or_else(|| {
            panic!(
                "source `{}` missing benchmark authority artifact `{authority_artifact_path}`",
                source.source_id
            )
        });
    SignedNodeBenchmarkEvidence {
        benchmark_id: String::from(benchmark_id),
        authority_artifact_path: authority_artifact.path.clone(),
        authority_artifact_sha256: authority_artifact.sha256.clone(),
        admitted_execution_classes,
        detail: String::from(detail),
    }
}

fn refuse_role_class(
    requested_role_class: DecentralizedNetworkRoleClass,
    detail: &str,
) -> SignedNodeCapabilityRefusalExample {
    SignedNodeCapabilityRefusalExample {
        requested_role_class: Some(requested_role_class),
        requested_execution_class: None,
        refusal_kind: SignedNodeCapabilityRefusalKind::RoleClassNotAdmitted,
        detail: String::from(detail),
    }
}

fn refuse_execution_class(
    requested_execution_class: CrossProviderExecutionClass,
    detail: &str,
) -> SignedNodeCapabilityRefusalExample {
    SignedNodeCapabilityRefusalExample {
        requested_role_class: None,
        requested_execution_class: Some(requested_execution_class),
        refusal_kind: SignedNodeCapabilityRefusalKind::ExecutionClassNotAdmitted,
        detail: String::from(detail),
    }
}

fn sign_identity_record(identity: &mut SignedNodeIdentityRecord) {
    let payload_bytes = identity.signing_payload_bytes();
    let payload_digest = sha256_hex(payload_bytes.as_slice());
    let signing_key =
        deterministic_signing_key(identity.capability_signature.signer_key_id.as_str());
    let verifying_key: VerifyingKey = signing_key.verifying_key();
    let signature = signing_key.sign(payload_bytes.as_slice());
    identity.capability_signature.public_key_hex = hex::encode(verifying_key.to_bytes());
    identity.capability_signature.payload_digest = payload_digest;
    identity.capability_signature.signature_hex = hex::encode(signature.to_bytes());
}

fn signer_key_id(source_id: &str) -> String {
    format!("psionic.testnet.identity_signer.{source_id}")
}

fn deterministic_signing_key(label: &str) -> SigningKey {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_signed_node_identity_signing_key|");
    hasher.update(label.as_bytes());
    let seed = hasher.finalize();
    let secret: [u8; 32] = seed.into();
    SigningKey::from_bytes(&secret)
}

fn wallet_address(source_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_signed_node_identity_wallet|");
    hasher.update(source_id.as_bytes());
    let digest = hasher.finalize();
    format!("0x{}", hex::encode(&digest[..20]))
}

fn build_digest(
    source_id: &str,
    environment_ref: &str,
    environment_version: &str,
    network_id: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_signed_node_identity_build|");
    hasher.update(source_id.as_bytes());
    hasher.update(b"|");
    hasher.update(environment_ref.as_bytes());
    hasher.update(b"|");
    hasher.update(environment_version.as_bytes());
    hasher.update(b"|");
    hasher.update(network_id.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("signed node identity contract digest serialization must work"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_node_identity_contract_set_stays_valid() {
        let contract = canonical_signed_node_identity_contract_set()
            .expect("signed node identity contract set should build");
        contract.validate().expect("contract set should validate");
    }

    #[test]
    fn signed_node_identity_contract_set_tracks_one_identity_per_source() {
        let contract = canonical_signed_node_identity_contract_set()
            .expect("signed node identity contract set should build");
        let source_ids = contract
            .identities
            .iter()
            .map(|identity| identity.source_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            source_ids,
            BTreeSet::from([
                "google_l4_validator_node",
                "runpod_8xh100_dense_node",
                "local_rtx4080_workstation",
                "local_mlx_mac_workstation",
            ])
        );
    }

    #[test]
    fn signed_node_identity_contract_set_signatures_verify() {
        let contract = canonical_signed_node_identity_contract_set()
            .expect("signed node identity contract set should build");
        for identity in &contract.identities {
            let payload_bytes = identity.signing_payload_bytes();
            let signing_key =
                deterministic_signing_key(identity.capability_signature.signer_key_id.as_str());
            let expected_public = hex::encode(signing_key.verifying_key().to_bytes());
            assert_eq!(
                identity.capability_signature.public_key_hex,
                expected_public
            );
            assert_eq!(
                identity.capability_signature.payload_digest,
                sha256_hex(payload_bytes.as_slice())
            );
        }
    }
}
