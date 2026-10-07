use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_content_addressed_artifact_exchange_contract,
    canonical_live_checkpoint_catchup_contract, canonical_public_dataset_authority_contract,
    canonical_public_network_registry_contract, canonical_public_work_assignment_contract,
    canonical_quantized_outer_sync_contract, ContentAddressedArtifactExchangeContractError,
    ContentAddressedArtifactKind, ContentAddressedExchangeBackendKind, CrossProviderExecutionClass,
    CatchupDisposition, DecentralizedNetworkRoleClass, LiveCheckpointCatchupContractError,
    PublicDataReceiptDisposition, PublicDatasetAuthorityContractError,
    PublicNetworkRegistryContractError, PublicWorkAssignmentContractError,
    PublicWorkAssignmentKind, QuantizedOuterSyncContractError,
};

pub const PUBLIC_MINER_EXECUTION_CLASS_ID: &str = "psionic.execution_class.public_miner.v1";
pub const PUBLIC_MINER_PROTOCOL_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.public_miner_protocol_contract.v1";
pub const PUBLIC_MINER_PROTOCOL_CONTRACT_ID: &str = "psionic.public_miner_protocol_contract.v1";
pub const PUBLIC_MINER_PROTOCOL_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/public_miner_protocol_contract_v1.json";
pub const PUBLIC_MINER_PROTOCOL_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-public-miner-protocol-contract.sh";
pub const PUBLIC_MINER_PROTOCOL_CONTRACT_DOC_PATH: &str = "docs/PUBLIC_MINER_PROTOCOL_REFERENCE.md";
pub const PUBLIC_MINER_PROTOCOL_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum PublicMinerProtocolContractError {
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
    DatasetAuthority(#[from] PublicDatasetAuthorityContractError),
    #[error(transparent)]
    LiveCatchup(#[from] LiveCheckpointCatchupContractError),
    #[error(transparent)]
    OuterSync(#[from] QuantizedOuterSyncContractError),
    #[error(transparent)]
    ArtifactExchange(#[from] ContentAddressedArtifactExchangeContractError),
    #[error("public miner protocol contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicMinerProtocolPhase {
    StartupComplete,
    AssignmentIntakeComplete,
    LocalTrainingComplete,
    DeltaPublished,
    CheckpointSynchronized,
    WindowFinalized,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicMinerRefusalKind {
    AssignmentWindowClosed,
    CheckpointSyncLagExceeded,
    ArtifactDigestMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicMinerExecutionClassBinding {
    pub execution_class_id: String,
    pub admitted_role_class: DecentralizedNetworkRoleClass,
    pub legacy_assignment_execution_class: CrossProviderExecutionClass,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicMinerRetryPolicy {
    pub max_assignment_intake_attempts: u8,
    pub max_delta_upload_attempts: u8,
    pub max_checkpoint_sync_attempts: u8,
    pub window_close_refusal_kind: PublicMinerRefusalKind,
    pub checkpoint_sync_refusal_kind: PublicMinerRefusalKind,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicMinerProtocolSession {
    pub session_id: String,
    pub miner_registry_record_id: String,
    pub assignment_id: String,
    pub assignment_receipt_id: String,
    pub dataset_receipt_id: String,
    pub checkpoint_reference_id: String,
    pub checkpoint_artifact_id: String,
    pub delta_artifact_id: String,
    pub quantized_exchange_receipt_id: String,
    pub planned_local_steps: u16,
    pub actual_local_steps: u16,
    pub delta_upload_retry_count: u8,
    pub final_phase: PublicMinerProtocolPhase,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicMinerLocalStepReceipt {
    pub receipt_id: String,
    pub session_id: String,
    pub step_start: u64,
    pub step_end: u64,
    pub consumed_page_id: String,
    pub produced_delta_artifact_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicMinerDeltaUploadReceipt {
    pub receipt_id: String,
    pub session_id: String,
    pub delta_artifact_id: String,
    pub selected_backend_id: String,
    pub quantized_exchange_receipt_id: String,
    pub completed_before_window_close: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicMinerCheckpointSyncReceipt {
    pub receipt_id: String,
    pub session_id: String,
    pub checkpoint_artifact_id: String,
    pub checkpoint_reference_id: String,
    pub step_lag_after_sync: u64,
    pub completed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicMinerProtocolRefusal {
    pub refusal_id: String,
    pub miner_registry_record_id: String,
    pub refusal_kind: PublicMinerRefusalKind,
    pub checkpoint_reference_id: String,
    pub final_phase: PublicMinerProtocolPhase,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicMinerProtocolAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicMinerProtocolContract {
    pub schema_version: String,
    pub contract_id: String,
    pub public_network_registry_contract_digest: String,
    pub public_work_assignment_contract_digest: String,
    pub public_dataset_authority_contract_digest: String,
    pub live_checkpoint_catchup_contract_digest: String,
    pub quantized_outer_sync_contract_digest: String,
    pub content_addressed_artifact_exchange_contract_digest: String,
    pub execution_class_binding: PublicMinerExecutionClassBinding,
    pub retry_policy: PublicMinerRetryPolicy,
    pub sessions: Vec<PublicMinerProtocolSession>,
    pub local_step_receipts: Vec<PublicMinerLocalStepReceipt>,
    pub delta_upload_receipts: Vec<PublicMinerDeltaUploadReceipt>,
    pub checkpoint_sync_receipts: Vec<PublicMinerCheckpointSyncReceipt>,
    pub refusals: Vec<PublicMinerProtocolRefusal>,
    pub authority_paths: PublicMinerProtocolAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl PublicMinerProtocolContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_public_miner_protocol_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), PublicMinerProtocolContractError> {
        let registry = canonical_public_network_registry_contract()?;
        let public_work = canonical_public_work_assignment_contract()?;
        let dataset_authority = canonical_public_dataset_authority_contract()?;
        let catchup = canonical_live_checkpoint_catchup_contract()?;
        let outer_sync = canonical_quantized_outer_sync_contract()?;
        let artifact_exchange = canonical_content_addressed_artifact_exchange_contract()?;

        let registry_by_id = registry
            .registry_records
            .iter()
            .map(|record| (record.registry_record_id.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let miner_assignment_by_id = public_work
            .assignments
            .iter()
            .filter(|assignment| {
                assignment.assignment_kind == PublicWorkAssignmentKind::PublicMinerTrain
            })
            .map(|assignment| (assignment.assignment_id.as_str(), assignment))
            .collect::<BTreeMap<_, _>>();
        let assignment_receipt_by_assignment_id = public_work
            .assignment_receipts
            .iter()
            .map(|receipt| (receipt.assignment_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let dataset_receipt_by_assignment_id = dataset_authority
            .anti_replay_receipts
            .iter()
            .filter(|receipt| receipt.disposition == PublicDataReceiptDisposition::Admitted)
            .map(|receipt| (receipt.assignment_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let catchup_receipt_by_id = catchup
            .catchup_receipts
            .iter()
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let advertisement_ids = catchup
            .advertisements
            .iter()
            .map(|advertisement| advertisement.advertisement_id.as_str())
            .collect::<BTreeSet<_>>();
        let applied_exchange_by_id = outer_sync
            .exchange_receipts
            .iter()
            .filter(|receipt| receipt.disposition == crate::OuterSyncExchangeDisposition::Applied)
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let artifact_by_id = artifact_exchange
            .published_artifacts
            .iter()
            .map(|artifact| (artifact.artifact_id.as_str(), artifact))
            .collect::<BTreeMap<_, _>>();
        let backend_by_id = artifact_exchange
            .exchange_backends
            .iter()
            .map(|backend| (backend.backend_id.as_str(), backend))
            .collect::<BTreeMap<_, _>>();
        let session_by_id = self
            .sessions
            .iter()
            .map(|session| (session.session_id.as_str(), session))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != PUBLIC_MINER_PROTOCOL_CONTRACT_SCHEMA_VERSION {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    PUBLIC_MINER_PROTOCOL_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != PUBLIC_MINER_PROTOCOL_CONTRACT_ID {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.public_network_registry_contract_digest != registry.contract_digest
            || self.public_work_assignment_contract_digest != public_work.contract_digest
            || self.public_dataset_authority_contract_digest != dataset_authority.contract_digest
            || self.live_checkpoint_catchup_contract_digest != catchup.contract_digest
            || self.quantized_outer_sync_contract_digest != outer_sync.contract_digest
            || self.content_addressed_artifact_exchange_contract_digest
                != artifact_exchange.contract_digest
        {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.execution_class_binding.execution_class_id != PUBLIC_MINER_EXECUTION_CLASS_ID
            || self.execution_class_binding.admitted_role_class
                != DecentralizedNetworkRoleClass::PublicMiner
            || self
                .execution_class_binding
                .legacy_assignment_execution_class
                != CrossProviderExecutionClass::ValidatedContributorWindow
        {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("execution class binding drifted"),
            });
        }
        if self.retry_policy.max_assignment_intake_attempts == 0
            || self.retry_policy.max_delta_upload_attempts == 0
            || self.retry_policy.max_checkpoint_sync_attempts == 0
            || self.retry_policy.window_close_refusal_kind
                != PublicMinerRefusalKind::AssignmentWindowClosed
            || self.retry_policy.checkpoint_sync_refusal_kind
                != PublicMinerRefusalKind::CheckpointSyncLagExceeded
        {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("retry policy drifted"),
            });
        }
        if self.authority_paths.fixture_path != PUBLIC_MINER_PROTOCOL_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != PUBLIC_MINER_PROTOCOL_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != PUBLIC_MINER_PROTOCOL_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != PUBLIC_MINER_PROTOCOL_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        if self.sessions.len() != 2 {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("expected exactly two canonical active public miner sessions"),
            });
        }

        let mut session_ids = BTreeSet::new();
        for session in &self.sessions {
            if !session_ids.insert(session.session_id.as_str()) {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!("duplicate session `{}`", session.session_id),
                });
            }
            let registry_record = registry_by_id
                .get(session.miner_registry_record_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "session `{}` references unknown miner `{}`",
                        session.session_id, session.miner_registry_record_id
                    ),
                })?;
            let assignment = miner_assignment_by_id
                .get(session.assignment_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "session `{}` references unknown miner assignment `{}`",
                        session.session_id, session.assignment_id
                    ),
                })?;
            let assignment_receipt = assignment_receipt_by_assignment_id
                .get(session.assignment_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "session `{}` lost assignment receipt for `{}`",
                        session.session_id, session.assignment_id
                    ),
                })?;
            let dataset_receipt = dataset_receipt_by_assignment_id
                .get(session.assignment_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "session `{}` lost dataset receipt for `{}`",
                        session.session_id, session.assignment_id
                    ),
                })?;
            let delta_artifact = artifact_by_id
                .get(session.delta_artifact_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "session `{}` references unknown delta artifact `{}`",
                        session.session_id, session.delta_artifact_id
                    ),
                })?;
            let quantized_exchange = applied_exchange_by_id
                .get(session.quantized_exchange_receipt_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "session `{}` references unknown applied exchange `{}`",
                        session.session_id, session.quantized_exchange_receipt_id
                    ),
                })?;
            let checkpoint_artifact = artifact_by_id
                .get(session.checkpoint_artifact_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "session `{}` references unknown checkpoint artifact `{}`",
                        session.session_id, session.checkpoint_artifact_id
                    ),
                })?;

            if !registry_record
                .role_classes
                .contains(&DecentralizedNetworkRoleClass::PublicMiner)
                || session.assignment_receipt_id != assignment_receipt.receipt_id
                || session.dataset_receipt_id != dataset_receipt.receipt_id
                || assignment.registry_record_id != session.miner_registry_record_id
                || session.planned_local_steps != assignment.planned_local_steps
                || session.actual_local_steps != assignment.planned_local_steps
                || session.delta_upload_retry_count >= self.retry_policy.max_delta_upload_attempts
                || session.final_phase != PublicMinerProtocolPhase::WindowFinalized
                || delta_artifact.artifact_kind != ContentAddressedArtifactKind::QuantizedDelta
                || delta_artifact.publisher_registry_record_id != session.miner_registry_record_id
                || quantized_exchange.source_registry_record_id != session.miner_registry_record_id
                || checkpoint_artifact.artifact_kind != ContentAddressedArtifactKind::LiveCheckpoint
            {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!("session `{}` drifted", session.session_id),
                });
            }
            if catchup_receipt_by_id.contains_key(session.checkpoint_reference_id.as_str()) {
                let catchup_receipt = catchup_receipt_by_id
                    .get(session.checkpoint_reference_id.as_str())
                    .expect("just checked membership");
                if catchup_receipt.disposition != crate::CatchupDisposition::Completed
                    || catchup_receipt.joining_registry_record_id
                        != session.miner_registry_record_id
                {
                    return Err(PublicMinerProtocolContractError::InvalidContract {
                        detail: format!(
                            "session `{}` lost completed catch-up binding",
                            session.session_id
                        ),
                    });
                }
            } else if !advertisement_ids.contains(session.checkpoint_reference_id.as_str()) {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "session `{}` references unknown checkpoint reference `{}`",
                        session.session_id, session.checkpoint_reference_id
                    ),
                });
            }
        }

        let mut local_step_receipt_ids = BTreeSet::new();
        for receipt in &self.local_step_receipts {
            if !local_step_receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!("duplicate local-step receipt `{}`", receipt.receipt_id),
                });
            }
            let session = session_by_id
                .get(receipt.session_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "local-step receipt `{}` references unknown session `{}`",
                        receipt.receipt_id, receipt.session_id
                    ),
                })?;
            let assignment = miner_assignment_by_id.get(session.assignment_id.as_str()).expect(
                "validated sessions must retain miner assignment bindings before local-step validation",
            );
            if receipt.step_end <= receipt.step_start
                || receipt.step_end - receipt.step_start != u64::from(session.actual_local_steps)
                || receipt.consumed_page_id != assignment.dataset_page_selector
                || receipt.produced_delta_artifact_id != session.delta_artifact_id
            {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!("local-step receipt `{}` drifted", receipt.receipt_id),
                });
            }
        }
        if self.local_step_receipts.len() != self.sessions.len() {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("every active session must retain one local-step receipt"),
            });
        }

        let mut delta_upload_receipt_ids = BTreeSet::new();
        for receipt in &self.delta_upload_receipts {
            if !delta_upload_receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!("duplicate delta-upload receipt `{}`", receipt.receipt_id),
                });
            }
            let session = session_by_id
                .get(receipt.session_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "delta-upload receipt `{}` references unknown session `{}`",
                        receipt.receipt_id, receipt.session_id
                    ),
                })?;
            let backend = backend_by_id
                .get(receipt.selected_backend_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "delta-upload receipt `{}` references unknown backend `{}`",
                        receipt.receipt_id, receipt.selected_backend_id
                    ),
                })?;
            if receipt.delta_artifact_id != session.delta_artifact_id
                || receipt.quantized_exchange_receipt_id != session.quantized_exchange_receipt_id
                || !receipt.completed_before_window_close
                || backend.backend_kind != ContentAddressedExchangeBackendKind::PeerSeed
            {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!("delta-upload receipt `{}` drifted", receipt.receipt_id),
                });
            }
        }
        if self.delta_upload_receipts.len() != self.sessions.len() {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("every active session must retain one delta-upload receipt"),
            });
        }

        let mut checkpoint_sync_receipt_ids = BTreeSet::new();
        for receipt in &self.checkpoint_sync_receipts {
            if !checkpoint_sync_receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!("duplicate checkpoint-sync receipt `{}`", receipt.receipt_id),
                });
            }
            let session = session_by_id
                .get(receipt.session_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "checkpoint-sync receipt `{}` references unknown session `{}`",
                        receipt.receipt_id, receipt.session_id
                    ),
                })?;
            if receipt.checkpoint_artifact_id != session.checkpoint_artifact_id
                || receipt.checkpoint_reference_id != session.checkpoint_reference_id
                || !receipt.completed
                || receipt.step_lag_after_sync != 0
            {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!("checkpoint-sync receipt `{}` drifted", receipt.receipt_id),
                });
            }
        }
        if self.checkpoint_sync_receipts.len() != self.sessions.len() {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from(
                    "every active session must retain one checkpoint-sync receipt",
                ),
            });
        }

        if self.refusals.len() != 1 {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("expected exactly one public miner refusal"),
            });
        }
        for refusal in &self.refusals {
            let registry_record = registry_by_id
                .get(refusal.miner_registry_record_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "refusal `{}` references unknown miner `{}`",
                        refusal.refusal_id, refusal.miner_registry_record_id
                    ),
                })?;
            let catchup_receipt = catchup_receipt_by_id
                .get(refusal.checkpoint_reference_id.as_str())
                .ok_or_else(|| PublicMinerProtocolContractError::InvalidContract {
                    detail: format!(
                        "refusal `{}` references unknown catch-up receipt `{}`",
                        refusal.refusal_id, refusal.checkpoint_reference_id
                    ),
                })?;
            if !registry_record
                .role_classes
                .contains(&DecentralizedNetworkRoleClass::PublicMiner)
                || refusal.refusal_kind != PublicMinerRefusalKind::CheckpointSyncLagExceeded
                || refusal.final_phase != PublicMinerProtocolPhase::Refused
                || catchup_receipt.disposition != CatchupDisposition::Refused
                || catchup_receipt.joining_registry_record_id != refusal.miner_registry_record_id
            {
                return Err(PublicMinerProtocolContractError::InvalidContract {
                    detail: format!("refusal `{}` drifted", refusal.refusal_id),
                });
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(PublicMinerProtocolContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_public_miner_protocol_contract(
) -> Result<PublicMinerProtocolContract, PublicMinerProtocolContractError> {
    let registry = canonical_public_network_registry_contract()?;
    let public_work = canonical_public_work_assignment_contract()?;
    let dataset_authority = canonical_public_dataset_authority_contract()?;
    let catchup = canonical_live_checkpoint_catchup_contract()?;
    let outer_sync = canonical_quantized_outer_sync_contract()?;
    let artifact_exchange = canonical_content_addressed_artifact_exchange_contract()?;

    let sessions = vec![
        protocol_session(
            "session.public_miner.google.window1231",
            "google_l4_validator_node.registry",
            "assignment.public_miner.window1231.google",
            "receipt.assignment.public_miner.window1231.google",
            "anti_replay.assignment.public_miner.window1231.google",
            "advertisement.checkpoint_authority.google.mirror",
            "artifact.checkpoint.step2048.live",
            "artifact.delta.google.int8.round2056",
            "exchange.public_miner.google_to_runpod.int8.1",
            64,
            64,
            1,
            "Google completes the canonical public miner flow for the current active window: assignment intake, local train steps, delta publication, checkpoint synchronization, and window finalization.",
        ),
        protocol_session(
            "session.public_miner.local_mlx.window1231",
            "local_mlx_mac_workstation.registry",
            "assignment.public_miner.window1231.local_mlx",
            "receipt.assignment.public_miner.window1231.local_mlx",
            "anti_replay.assignment.public_miner.window1231.local_mlx",
            "catchup.public_miner.local_mlx.after_deathrattle",
            "artifact.checkpoint.step2048.live",
            "artifact.delta.local_mlx.nf4.round2056",
            "exchange.public_miner.local_mlx_to_runpod.nf4.1",
            64,
            64,
            1,
            "Apple MLX completes the same public miner flow after live checkpoint catch-up, proving the protocol supports rejoin plus normal delta publication in one typed path.",
        ),
    ];

    let local_step_receipts = vec![
        local_step_receipt(
            "local_steps.public_miner.google.window1231",
            "session.public_miner.google.window1231",
            2_048,
            2_112,
            "dataset.page.train.0009_0012",
            "artifact.delta.google.int8.round2056",
            "Google consumes its assigned page slice and closes exactly sixty-four local steps before publishing the quantized delta artifact.",
        ),
        local_step_receipt(
            "local_steps.public_miner.local_mlx.window1231",
            "session.public_miner.local_mlx.window1231",
            2_048,
            2_112,
            "dataset.page.train.0013_0016",
            "artifact.delta.local_mlx.nf4.round2056",
            "Apple MLX consumes its assigned page slice and closes exactly sixty-four local steps before publishing the NF4 residual artifact.",
        ),
    ];

    let delta_upload_receipts = vec![
        delta_upload_receipt(
            "delta_upload.public_miner.google.window1231",
            "session.public_miner.google.window1231",
            "artifact.delta.google.int8.round2056",
            "backend.peer.google.seed",
            "exchange.public_miner.google_to_runpod.int8.1",
            "Google publishes its quantized delta through the peer seed and closes upload before the window boundary.",
        ),
        delta_upload_receipt(
            "delta_upload.public_miner.local_mlx.window1231",
            "session.public_miner.local_mlx.window1231",
            "artifact.delta.local_mlx.nf4.round2056",
            "backend.peer.local_mlx.seed",
            "exchange.public_miner.local_mlx_to_runpod.nf4.1",
            "Apple MLX publishes its NF4 residual artifact through the overlay-backed peer seed before the window closes.",
        ),
    ];

    let checkpoint_sync_receipts = vec![
        checkpoint_sync_receipt(
            "checkpoint_sync.public_miner.google.window1231",
            "session.public_miner.google.window1231",
            "artifact.checkpoint.step2048.live",
            "advertisement.checkpoint_authority.google.mirror",
            "Google confirms checkpoint synchronization against the mirrored checkpoint-authority advertisement before finalizing the window.",
        ),
        checkpoint_sync_receipt(
            "checkpoint_sync.public_miner.local_mlx.window1231",
            "session.public_miner.local_mlx.window1231",
            "artifact.checkpoint.step2048.live",
            "catchup.public_miner.local_mlx.after_deathrattle",
            "Apple MLX finalizes the window only after the admitted live catch-up receipt proves checkpoint synchronization closed successfully.",
        ),
    ];

    let refusals = vec![PublicMinerProtocolRefusal {
        refusal_id: String::from("refusal.public_miner.local_rtx4080.checkpoint_lag"),
        miner_registry_record_id: String::from("local_rtx4080_workstation.registry"),
        refusal_kind: PublicMinerRefusalKind::CheckpointSyncLagExceeded,
        checkpoint_reference_id: String::from(
            "catchup.public_miner.local_rtx4080.partial_state_refused",
        ),
        final_phase: PublicMinerProtocolPhase::Refused,
        detail: String::from(
            "The stale RTX 4080 standby miner never enters assignment intake for the active public window because checkpoint synchronization was already refused on lag and optimizer-state incompleteness.",
        ),
    }];

    let mut contract = PublicMinerProtocolContract {
        schema_version: String::from(PUBLIC_MINER_PROTOCOL_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(PUBLIC_MINER_PROTOCOL_CONTRACT_ID),
        public_network_registry_contract_digest: registry.contract_digest.clone(),
        public_work_assignment_contract_digest: public_work.contract_digest.clone(),
        public_dataset_authority_contract_digest: dataset_authority.contract_digest.clone(),
        live_checkpoint_catchup_contract_digest: catchup.contract_digest.clone(),
        quantized_outer_sync_contract_digest: outer_sync.contract_digest.clone(),
        content_addressed_artifact_exchange_contract_digest: artifact_exchange.contract_digest.clone(),
        execution_class_binding: PublicMinerExecutionClassBinding {
            execution_class_id: String::from(PUBLIC_MINER_EXECUTION_CLASS_ID),
            admitted_role_class: DecentralizedNetworkRoleClass::PublicMiner,
            legacy_assignment_execution_class: CrossProviderExecutionClass::ValidatedContributorWindow,
            detail: String::from(
                "The public miner protocol is now first-class under its own execution-class id while still binding back to the current validated contributor assignment vocabulary until the root manifest widens.",
            ),
        },
        retry_policy: PublicMinerRetryPolicy {
            max_assignment_intake_attempts: 2,
            max_delta_upload_attempts: 3,
            max_checkpoint_sync_attempts: 2,
            window_close_refusal_kind: PublicMinerRefusalKind::AssignmentWindowClosed,
            checkpoint_sync_refusal_kind: PublicMinerRefusalKind::CheckpointSyncLagExceeded,
            detail: String::from(
                "Public miners may retry bounded intake, upload, and checkpoint-sync operations, but the protocol still fails closed on closed windows or stale checkpoint recovery.",
            ),
        },
        sessions,
        local_step_receipts,
        delta_upload_receipts,
        checkpoint_sync_receipts,
        refusals,
        authority_paths: PublicMinerProtocolAuthorityPaths {
            fixture_path: String::from(PUBLIC_MINER_PROTOCOL_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(PUBLIC_MINER_PROTOCOL_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(PUBLIC_MINER_PROTOCOL_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(PUBLIC_MINER_PROTOCOL_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first public miner execution protocol: execution-class binding, retry posture, assignment intake, local-step receipts, delta publication, checkpoint synchronization, and one stale-standby refusal. It does not yet claim validator verdict truth, checkpoint-promotion consensus, or reward accounting.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_public_miner_protocol_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), PublicMinerProtocolContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PublicMinerProtocolContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_public_miner_protocol_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| PublicMinerProtocolContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn protocol_session(
    session_id: &str,
    miner_registry_record_id: &str,
    assignment_id: &str,
    assignment_receipt_id: &str,
    dataset_receipt_id: &str,
    checkpoint_reference_id: &str,
    checkpoint_artifact_id: &str,
    delta_artifact_id: &str,
    quantized_exchange_receipt_id: &str,
    planned_local_steps: u16,
    actual_local_steps: u16,
    delta_upload_retry_count: u8,
    detail: &str,
) -> PublicMinerProtocolSession {
    PublicMinerProtocolSession {
        session_id: String::from(session_id),
        miner_registry_record_id: String::from(miner_registry_record_id),
        assignment_id: String::from(assignment_id),
        assignment_receipt_id: String::from(assignment_receipt_id),
        dataset_receipt_id: String::from(dataset_receipt_id),
        checkpoint_reference_id: String::from(checkpoint_reference_id),
        checkpoint_artifact_id: String::from(checkpoint_artifact_id),
        delta_artifact_id: String::from(delta_artifact_id),
        quantized_exchange_receipt_id: String::from(quantized_exchange_receipt_id),
        planned_local_steps,
        actual_local_steps,
        delta_upload_retry_count,
        final_phase: PublicMinerProtocolPhase::WindowFinalized,
        detail: String::from(detail),
    }
}

fn local_step_receipt(
    receipt_id: &str,
    session_id: &str,
    step_start: u64,
    step_end: u64,
    consumed_page_id: &str,
    produced_delta_artifact_id: &str,
    detail: &str,
) -> PublicMinerLocalStepReceipt {
    PublicMinerLocalStepReceipt {
        receipt_id: String::from(receipt_id),
        session_id: String::from(session_id),
        step_start,
        step_end,
        consumed_page_id: String::from(consumed_page_id),
        produced_delta_artifact_id: String::from(produced_delta_artifact_id),
        detail: String::from(detail),
    }
}

fn delta_upload_receipt(
    receipt_id: &str,
    session_id: &str,
    delta_artifact_id: &str,
    selected_backend_id: &str,
    quantized_exchange_receipt_id: &str,
    detail: &str,
) -> PublicMinerDeltaUploadReceipt {
    PublicMinerDeltaUploadReceipt {
        receipt_id: String::from(receipt_id),
        session_id: String::from(session_id),
        delta_artifact_id: String::from(delta_artifact_id),
        selected_backend_id: String::from(selected_backend_id),
        quantized_exchange_receipt_id: String::from(quantized_exchange_receipt_id),
        completed_before_window_close: true,
        detail: String::from(detail),
    }
}

fn checkpoint_sync_receipt(
    receipt_id: &str,
    session_id: &str,
    checkpoint_artifact_id: &str,
    checkpoint_reference_id: &str,
    detail: &str,
) -> PublicMinerCheckpointSyncReceipt {
    PublicMinerCheckpointSyncReceipt {
        receipt_id: String::from(receipt_id),
        session_id: String::from(session_id),
        checkpoint_artifact_id: String::from(checkpoint_artifact_id),
        checkpoint_reference_id: String::from(checkpoint_reference_id),
        step_lag_after_sync: 0,
        completed: true,
        detail: String::from(detail),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for public miner protocol contract"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_public_miner_protocol_contract, PublicMinerProtocolContractError,
        PublicMinerProtocolPhase,
    };

    #[test]
    fn canonical_public_miner_protocol_contract_is_valid(
    ) -> Result<(), PublicMinerProtocolContractError> {
        let contract = canonical_public_miner_protocol_contract()?;
        contract.validate()
    }

    #[test]
    fn refused_standby_miner_cannot_become_finalized(
    ) -> Result<(), PublicMinerProtocolContractError> {
        let mut contract = canonical_public_miner_protocol_contract()?;
        let refusal = contract
            .refusals
            .iter_mut()
            .find(|refusal| {
                refusal.refusal_id == "refusal.public_miner.local_rtx4080.checkpoint_lag"
            })
            .expect("canonical contract should retain the stale standby refusal");
        refusal.final_phase = PublicMinerProtocolPhase::WindowFinalized;
        let error = contract
            .validate()
            .expect_err("stale standby refusal cannot silently become finalized");
        assert!(matches!(
            error,
            PublicMinerProtocolContractError::InvalidContract { .. }
        ));
        Ok(())
    }

    #[test]
    fn sessions_must_keep_miner_dataset_receipts() -> Result<(), PublicMinerProtocolContractError> {
        let mut contract = canonical_public_miner_protocol_contract()?;
        let session = contract
            .sessions
            .iter_mut()
            .find(|session| session.session_id == "session.public_miner.google.window1231")
            .expect("canonical contract should retain the Google public miner session");
        session.dataset_receipt_id =
            String::from("anti_replay.assignment.public_miner.window1230.google");
        let error = contract
            .validate()
            .expect_err("sessions must keep the matching miner dataset receipt");
        assert!(matches!(
            error,
            PublicMinerProtocolContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
