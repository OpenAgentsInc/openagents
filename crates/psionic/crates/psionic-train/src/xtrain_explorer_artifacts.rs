use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_parameter_golf_xtrain_visualization_bundle_v2,
    canonical_curated_decentralized_run_contract, canonical_multi_validator_consensus_contract,
    canonical_public_miner_protocol_contract, canonical_public_network_registry_contract,
    canonical_public_run_explorer_contract, canonical_settlement_publication_contract,
    CrossProviderExecutionClass, CuratedDecentralizedRunContractError,
    DecentralizedNetworkRoleClass, MultiValidatorConsensusContractError,
    PublicMinerProtocolContractError, PublicNetworkAvailabilityStatus,
    PublicNetworkRegistryContractError, PublicRunExplorerContractError,
    RemoteTrainingEventSeverity, SettlementPublicationContractError,
    TrainingExecutionPromotionOutcome, PARAMETER_GOLF_XTRAIN_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH,
};

pub const XTRAIN_EXPLORER_SNAPSHOT_SCHEMA_VERSION: &str = "psionic.xtrain_explorer_snapshot.v1";
pub const XTRAIN_EXPLORER_INDEX_SCHEMA_VERSION: &str = "psionic.xtrain_explorer_index.v1";
pub const XTRAIN_EXPLORER_SNAPSHOT_FIXTURE_PATH: &str =
    "fixtures/training/xtrain_explorer_snapshot_v1.json";
pub const XTRAIN_EXPLORER_INDEX_FIXTURE_PATH: &str =
    "fixtures/training/xtrain_explorer_index_v1.json";
pub const XTRAIN_EXPLORER_CHECK_SCRIPT_PATH: &str = "scripts/check-xtrain-explorer-artifacts.sh";
pub const XTRAIN_EXPLORER_DOC_PATH: &str = "docs/XTRAIN_EXPLORER_REFERENCE.md";

const XTRAIN_EXPLORER_SNAPSHOT_ID: &str = "snapshot.xtrain_explorer.window1231.v1";
const XTRAIN_EXPLORER_INDEX_ID: &str = "xtrain-explorer-index-v1";
const XTRAIN_EXPLORER_ACTIVE_WINDOW_ID: &str = "window1231";
const XTRAIN_EXPLORER_GENERATED_AT_MS: u64 = 1_711_112_311_000;

#[derive(Debug, Error)]
pub enum XtrainExplorerArtifactsError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    PublicRegistry(#[from] PublicNetworkRegistryContractError),
    #[error(transparent)]
    PublicMiner(#[from] PublicMinerProtocolContractError),
    #[error(transparent)]
    Consensus(#[from] MultiValidatorConsensusContractError),
    #[error(transparent)]
    Settlement(#[from] SettlementPublicationContractError),
    #[error(transparent)]
    ExplorerFoundation(#[from] PublicRunExplorerContractError),
    #[error(transparent)]
    CuratedRun(#[from] CuratedDecentralizedRunContractError),
    #[error(transparent)]
    Visualization(#[from] crate::ParameterGolfXtrainVisualizationError),
    #[error("invalid XTRAIN explorer artifact: {detail}")]
    InvalidArtifact { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XtrainExplorerParticipantState {
    Active,
    Held,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XtrainExplorerEdgeKind {
    ValidatorScore,
    CheckpointSync,
    Refusal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XtrainExplorerWindowStatus {
    PromotionHeld,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XtrainExplorerRunSurfaceLinkKind {
    BoundedReferenceLane,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerParticipantNode {
    pub participant_id: String,
    pub node_identity_id: String,
    pub node_id: String,
    pub role_classes: Vec<DecentralizedNetworkRoleClass>,
    pub execution_classes: Vec<CrossProviderExecutionClass>,
    pub availability_status: PublicNetworkAvailabilityStatus,
    pub participant_state: XtrainExplorerParticipantState,
    pub current_window_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_microunits: Option<u64>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerParticipantEdge {
    pub edge_id: String,
    pub source_participant_id: String,
    pub target_participant_id: String,
    pub edge_kind: XtrainExplorerEdgeKind,
    pub reference_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerWindowState {
    pub window_id: String,
    pub dataset_page_ids: Vec<String>,
    pub miner_session_ids: Vec<String>,
    pub validator_vote_ids: Vec<String>,
    pub checkpoint_artifact_id: String,
    pub promotion_decision_id: String,
    pub status: XtrainExplorerWindowStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_record_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerCheckpointState {
    pub checkpoint_artifact_id: String,
    pub promotion_decision_id: String,
    pub outcome: TrainingExecutionPromotionOutcome,
    pub validator_vote_ids: Vec<String>,
    pub disagreement_receipt_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_record_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerRunSurfaceLink {
    pub link_id: String,
    pub relationship_kind: XtrainExplorerRunSurfaceLinkKind,
    pub bundle_artifact_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerEvent {
    pub event_id: String,
    pub observed_at_ms: u64,
    pub severity: RemoteTrainingEventSeverity,
    pub event_kind: String,
    pub reference_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerSourceArtifact {
    pub artifact_role: String,
    pub artifact_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    pub authoritative: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerSnapshot {
    pub schema_version: String,
    pub snapshot_id: String,
    pub generated_at_ms: u64,
    pub network_id: String,
    pub current_epoch_id: String,
    pub active_window_id: String,
    pub public_run_explorer_contract_digest: String,
    pub public_network_registry_contract_digest: String,
    pub public_miner_protocol_contract_digest: String,
    pub multi_validator_consensus_contract_digest: String,
    pub settlement_publication_contract_digest: String,
    pub participants: Vec<XtrainExplorerParticipantNode>,
    pub participant_edges: Vec<XtrainExplorerParticipantEdge>,
    pub windows: Vec<XtrainExplorerWindowState>,
    pub checkpoints: Vec<XtrainExplorerCheckpointState>,
    pub run_surface_links: Vec<XtrainExplorerRunSurfaceLink>,
    pub events: Vec<XtrainExplorerEvent>,
    pub source_artifacts: Vec<XtrainExplorerSourceArtifact>,
    pub detail: String,
    pub snapshot_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerIndexEntry {
    pub snapshot_id: String,
    pub generated_at_ms: u64,
    pub network_id: String,
    pub current_epoch_id: String,
    pub active_window_id: String,
    pub participant_count: u16,
    pub held_checkpoint_count: u16,
    pub published_settlement_count: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_artifact_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_digest: Option<String>,
    pub semantic_summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XtrainExplorerIndex {
    pub schema_version: String,
    pub index_id: String,
    pub generated_at_ms: u64,
    pub entries: Vec<XtrainExplorerIndexEntry>,
    pub detail: String,
    pub index_digest: String,
}

impl XtrainExplorerSnapshot {
    pub fn validate(&self) -> Result<(), XtrainExplorerArtifactsError> {
        let explorer = canonical_public_run_explorer_contract()?;
        let registry = canonical_public_network_registry_contract()?;
        let miner = canonical_public_miner_protocol_contract()?;
        let consensus = canonical_multi_validator_consensus_contract()?;
        let settlement = canonical_settlement_publication_contract()?;
        let xtrain_bundle = build_parameter_golf_xtrain_visualization_bundle_v2()?;

        if self.schema_version != XTRAIN_EXPLORER_SNAPSHOT_SCHEMA_VERSION {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("snapshot schema_version drifted"),
            });
        }
        if self.snapshot_id != XTRAIN_EXPLORER_SNAPSHOT_ID {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("snapshot_id drifted"),
            });
        }
        if self.active_window_id != XTRAIN_EXPLORER_ACTIVE_WINDOW_ID {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("active_window_id drifted"),
            });
        }
        if self.generated_at_ms != XTRAIN_EXPLORER_GENERATED_AT_MS {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("generated_at_ms drifted"),
            });
        }
        if self.public_run_explorer_contract_digest != explorer.contract_digest
            || self.public_network_registry_contract_digest != registry.contract_digest
            || self.public_miner_protocol_contract_digest != miner.contract_digest
            || self.multi_validator_consensus_contract_digest != consensus.contract_digest
            || self.settlement_publication_contract_digest != settlement.contract_digest
        {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("upstream explorer digest drifted"),
            });
        }

        let participant_ids = self
            .participants
            .iter()
            .map(|participant| participant.participant_id.as_str())
            .collect::<BTreeSet<_>>();
        if participant_ids.len() != self.participants.len() || self.participants.len() != 4 {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("participant count or ids drifted"),
            });
        }
        if self.windows.len() != 1
            || self.checkpoints.len() != 1
            || self.run_surface_links.len() != 1
            || self.participant_edges.len() != 4
            || self.events.len() != 4
            || self.source_artifacts.len() != 7
        {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from(
                    "participant-edge, window, checkpoint, event, run-surface, or source-artifact counts drifted",
                ),
            });
        }
        if self
            .windows
            .iter()
            .all(|window| window.window_id != self.active_window_id)
        {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("active_window_id is missing from windows"),
            });
        }
        for edge in &self.participant_edges {
            if !participant_ids.contains(edge.source_participant_id.as_str())
                || !participant_ids.contains(edge.target_participant_id.as_str())
                || edge.reference_ids.is_empty()
            {
                return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                    detail: format!("participant edge `{}` drifted", edge.edge_id),
                });
            }
        }
        let run_surface_link = &self.run_surface_links[0];
        if run_surface_link.relationship_kind
            != XtrainExplorerRunSurfaceLinkKind::BoundedReferenceLane
            || run_surface_link.bundle_artifact_uri
                != PARAMETER_GOLF_XTRAIN_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH
            || run_surface_link.bundle_digest.as_deref()
                != Some(xtrain_bundle.bundle_digest.as_str())
        {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("run_surface_link drifted"),
            });
        }
        for event in &self.events {
            if event.reference_ids.is_empty() || event.event_kind.trim().is_empty() {
                return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                    detail: format!("event `{}` drifted", event.event_id),
                });
            }
        }
        let mut roles = BTreeSet::new();
        for artifact in &self.source_artifacts {
            if !roles.insert(artifact.artifact_role.as_str())
                || artifact.artifact_uri.trim().is_empty()
            {
                return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                    detail: format!("duplicate source artifact `{}`", artifact.artifact_role),
                });
            }
        }
        if self.snapshot_digest != stable_xtrain_snapshot_digest(self) {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("snapshot_digest drifted"),
            });
        }
        Ok(())
    }
}

impl XtrainExplorerIndex {
    pub fn validate(&self) -> Result<(), XtrainExplorerArtifactsError> {
        let snapshot = canonical_xtrain_explorer_snapshot()?;
        if self.schema_version != XTRAIN_EXPLORER_INDEX_SCHEMA_VERSION {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("index schema_version drifted"),
            });
        }
        if self.index_id != XTRAIN_EXPLORER_INDEX_ID || self.entries.len() != 1 {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("index id or entry count drifted"),
            });
        }
        if self.generated_at_ms != XTRAIN_EXPLORER_GENERATED_AT_MS {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("index generated_at_ms drifted"),
            });
        }
        let entry = &self.entries[0];
        if entry.snapshot_id != snapshot.snapshot_id
            || entry.generated_at_ms != snapshot.generated_at_ms
            || entry.network_id != snapshot.network_id
            || entry.current_epoch_id != snapshot.current_epoch_id
            || entry.active_window_id != snapshot.active_window_id
            || entry.participant_count != snapshot.participants.len() as u16
            || entry.held_checkpoint_count
                != snapshot
                    .checkpoints
                    .iter()
                    .filter(|checkpoint| {
                        checkpoint.outcome == TrainingExecutionPromotionOutcome::HeldNoPromotion
                    })
                    .count() as u16
            || entry.published_settlement_count
                != snapshot
                    .windows
                    .iter()
                    .filter(|window| window.settlement_record_id.is_some())
                    .count() as u16
            || entry.snapshot_artifact_uri.as_deref() != Some(XTRAIN_EXPLORER_SNAPSHOT_FIXTURE_PATH)
            || entry.snapshot_digest.as_deref() != Some(snapshot.snapshot_digest.as_str())
        {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("index entry snapshot binding drifted"),
            });
        }
        if self.index_digest != stable_xtrain_index_digest(self) {
            return Err(XtrainExplorerArtifactsError::InvalidArtifact {
                detail: String::from("index_digest drifted"),
            });
        }
        Ok(())
    }
}

pub fn canonical_xtrain_explorer_snapshot(
) -> Result<XtrainExplorerSnapshot, XtrainExplorerArtifactsError> {
    let explorer = canonical_public_run_explorer_contract()?;
    let registry = canonical_public_network_registry_contract()?;
    let miner = canonical_public_miner_protocol_contract()?;
    let consensus = canonical_multi_validator_consensus_contract()?;
    let settlement = canonical_settlement_publication_contract()?;
    let curated = canonical_curated_decentralized_run_contract()?;
    let xtrain_bundle = build_parameter_golf_xtrain_visualization_bundle_v2()?;

    let google = find_registry_record(&registry, "google_l4_validator_node.registry")?;
    let runpod = find_registry_record(&registry, "runpod_8xh100_dense_node.registry")?;
    let local_rtx = find_registry_record(&registry, "local_rtx4080_workstation.registry")?;
    let local_mlx = find_registry_record(&registry, "local_mlx_mac_workstation.registry")?;

    let google_session = find_miner_session(&miner, "session.public_miner.google.window1231")?;
    let local_mlx_session =
        find_miner_session(&miner, "session.public_miner.local_mlx.window1231")?;
    let local_rtx_refusal =
        find_miner_refusal(&miner, "refusal.public_miner.local_rtx4080.checkpoint_lag")?;

    let decision = find_consensus_decision(&consensus, "decision.checkpoint.step2048.round2056")?;
    let disagreement =
        find_disagreement_receipt(&consensus, "disagreement.checkpoint.step2048.round2056")?;
    let settlement_record = find_settlement_record(&settlement, "settlement.window1231.signed")?;
    let google_payout = find_payout_export(&settlement, "google_l4_validator_node.identity")?;
    let runpod_payout = find_payout_export(&settlement, "runpod_8xh100_dense_node.identity")?;
    let local_mlx_payout = find_payout_export(&settlement, "local_mlx_mac_workstation.identity")?;

    let mut snapshot = XtrainExplorerSnapshot {
        schema_version: String::from(XTRAIN_EXPLORER_SNAPSHOT_SCHEMA_VERSION),
        snapshot_id: String::from(XTRAIN_EXPLORER_SNAPSHOT_ID),
        generated_at_ms: XTRAIN_EXPLORER_GENERATED_AT_MS,
        network_id: registry.network_id.clone(),
        current_epoch_id: registry.current_epoch_id.clone(),
        active_window_id: String::from(XTRAIN_EXPLORER_ACTIVE_WINDOW_ID),
        public_run_explorer_contract_digest: explorer.contract_digest.clone(),
        public_network_registry_contract_digest: registry.contract_digest.clone(),
        public_miner_protocol_contract_digest: miner.contract_digest.clone(),
        multi_validator_consensus_contract_digest: consensus.contract_digest.clone(),
        settlement_publication_contract_digest: settlement.contract_digest.clone(),
        participants: vec![
            XtrainExplorerParticipantNode {
                participant_id: google.registry_record_id.clone(),
                node_identity_id: google.node_identity_id.clone(),
                node_id: google.node_id.clone(),
                role_classes: google.role_classes.clone(),
                execution_classes: google.execution_classes.clone(),
                availability_status: google.availability_status,
                participant_state: XtrainExplorerParticipantState::Active,
                current_window_id: String::from(XTRAIN_EXPLORER_ACTIVE_WINDOW_ID),
                active_session_id: Some(google_session.session_id.clone()),
                payout_microunits: Some(google_payout.payout_microunits),
                detail: String::from(
                    "Google is the multi-role participant in the retained explorer view: miner session closed, one validator vote accepted, relay advertised, and payout publication retained.",
                ),
            },
            XtrainExplorerParticipantNode {
                participant_id: runpod.registry_record_id.clone(),
                node_identity_id: runpod.node_identity_id.clone(),
                node_id: runpod.node_id.clone(),
                role_classes: runpod.role_classes.clone(),
                execution_classes: runpod.execution_classes.clone(),
                availability_status: runpod.availability_status,
                participant_state: XtrainExplorerParticipantState::Active,
                current_window_id: String::from(XTRAIN_EXPLORER_ACTIVE_WINDOW_ID),
                active_session_id: None,
                payout_microunits: Some(runpod_payout.payout_microunits),
                detail: String::from(
                    "RunPod remains the retained checkpoint-authority mirror and settlement participant in the explorer view.",
                ),
            },
            XtrainExplorerParticipantNode {
                participant_id: local_rtx.registry_record_id.clone(),
                node_identity_id: local_rtx.node_identity_id.clone(),
                node_id: local_rtx.node_id.clone(),
                role_classes: local_rtx.role_classes.clone(),
                execution_classes: local_rtx.execution_classes.clone(),
                availability_status: local_rtx.availability_status,
                participant_state: XtrainExplorerParticipantState::Refused,
                current_window_id: String::from(XTRAIN_EXPLORER_ACTIVE_WINDOW_ID),
                active_session_id: None,
                payout_microunits: None,
                detail: String::from(
                    "The local RTX 4080 participant stays visible in the graph even though miner entry for the retained window was refused on checkpoint lag.",
                ),
            },
            XtrainExplorerParticipantNode {
                participant_id: local_mlx.registry_record_id.clone(),
                node_identity_id: local_mlx.node_identity_id.clone(),
                node_id: local_mlx.node_id.clone(),
                role_classes: local_mlx.role_classes.clone(),
                execution_classes: local_mlx.execution_classes.clone(),
                availability_status: local_mlx.availability_status,
                participant_state: XtrainExplorerParticipantState::Held,
                current_window_id: String::from(XTRAIN_EXPLORER_ACTIVE_WINDOW_ID),
                active_session_id: Some(local_mlx_session.session_id.clone()),
                payout_microunits: Some(local_mlx_payout.payout_microunits),
                detail: String::from(
                    "Apple MLX remains active as both miner and validator, but the retained validator vote still holds checkpoint promotion.",
                ),
            },
        ],
        participant_edges: vec![
            XtrainExplorerParticipantEdge {
                edge_id: String::from("edge.google_validates_local_mlx.window1231"),
                source_participant_id: google.registry_record_id.clone(),
                target_participant_id: local_mlx.registry_record_id.clone(),
                edge_kind: XtrainExplorerEdgeKind::ValidatorScore,
                reference_ids: vec![
                    String::from("vote.public_validator.google.local_mlx.window1231"),
                    local_mlx_session.session_id.clone(),
                ],
                detail: String::from(
                    "Google carries one accepted validator vote over the Apple MLX miner session.",
                ),
            },
            XtrainExplorerParticipantEdge {
                edge_id: String::from("edge.local_mlx_validates_google.window1231"),
                source_participant_id: local_mlx.registry_record_id.clone(),
                target_participant_id: google.registry_record_id.clone(),
                edge_kind: XtrainExplorerEdgeKind::ValidatorScore,
                reference_ids: vec![
                    String::from("vote.public_validator.local_mlx.google.window1231"),
                    google_session.session_id.clone(),
                ],
                detail: String::from(
                    "Apple MLX carries the replay-required validator vote over the Google miner session.",
                ),
            },
            XtrainExplorerParticipantEdge {
                edge_id: String::from("edge.google_to_runpod_checkpoint_sync.window1231"),
                source_participant_id: google.registry_record_id.clone(),
                target_participant_id: runpod.registry_record_id.clone(),
                edge_kind: XtrainExplorerEdgeKind::CheckpointSync,
                reference_ids: vec![
                    String::from("checkpoint_sync.public_miner.google.window1231"),
                    settlement_record.record_id.clone(),
                ],
                detail: String::from(
                    "Google remains linked to the RunPod checkpoint-authority mirror through the retained checkpoint-sync and settlement surfaces.",
                ),
            },
            XtrainExplorerParticipantEdge {
                edge_id: String::from("edge.local_rtx_refused.window1231"),
                source_participant_id: local_rtx.registry_record_id.clone(),
                target_participant_id: google.registry_record_id.clone(),
                edge_kind: XtrainExplorerEdgeKind::Refusal,
                reference_ids: vec![local_rtx_refusal.refusal_id.clone()],
                detail: String::from(
                    "The local RTX standby path remains visible as a refused edge so the explorer can show checkpoint-lag exclusion instead of hiding the participant.",
                ),
            },
        ],
        windows: vec![XtrainExplorerWindowState {
            window_id: String::from(XTRAIN_EXPLORER_ACTIVE_WINDOW_ID),
            dataset_page_ids: miner
                .local_step_receipts
                .iter()
                .map(|receipt| receipt.consumed_page_id.clone())
                .collect(),
            miner_session_ids: miner
                .sessions
                .iter()
                .map(|session| session.session_id.clone())
                .collect(),
            validator_vote_ids: consensus.votes.iter().map(|vote| vote.vote_id.clone()).collect(),
            checkpoint_artifact_id: decision.candidate_reference_id.clone(),
            promotion_decision_id: decision.decision_id.clone(),
            status: XtrainExplorerWindowStatus::PromotionHeld,
            settlement_record_id: Some(settlement_record.record_id.clone()),
            detail: String::from(
                "The retained explorer window binds the two public miner sessions, the split validator outcome, the held checkpoint candidate, and the signed settlement publication in one explorer-oriented row.",
            ),
        }],
        checkpoints: vec![XtrainExplorerCheckpointState {
            checkpoint_artifact_id: decision.candidate_reference_id.clone(),
            promotion_decision_id: decision.decision_id.clone(),
            outcome: decision.outcome,
            validator_vote_ids: decision.vote_ids.clone(),
            disagreement_receipt_ids: vec![disagreement.receipt_id.clone()],
            settlement_record_id: Some(settlement_record.record_id.clone()),
            detail: String::from(
                "The current retained checkpoint candidate stays held-no-promotion because validator quorum was reached without unanimous acceptance.",
            ),
        }],
        run_surface_links: vec![XtrainExplorerRunSurfaceLink {
            link_id: String::from("bounded_xtrain_reference_lane"),
            relationship_kind: XtrainExplorerRunSurfaceLinkKind::BoundedReferenceLane,
            bundle_artifact_uri: String::from(
                PARAMETER_GOLF_XTRAIN_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH,
            ),
            bundle_digest: Some(xtrain_bundle.bundle_digest.clone()),
            detail: String::from(
                "The decentralized explorer links to the bounded XTRAIN run-centric bundle as a sibling surface. The explorer keeps participant and window truth; the run-centric bundle keeps the bounded local-reference score lane.",
            ),
        }],
        events: vec![
            XtrainExplorerEvent {
                event_id: String::from("event.window1231.sessions_closed"),
                observed_at_ms: XTRAIN_EXPLORER_GENERATED_AT_MS,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("window_sessions_closed"),
                reference_ids: vec![
                    google_session.session_id.clone(),
                    local_mlx_session.session_id.clone(),
                ],
                detail: String::from(
                    "Both retained public miner sessions closed sixty-four local steps and published deltas for the explorer window.",
                ),
            },
            XtrainExplorerEvent {
                event_id: String::from("event.window1231.promotion_held"),
                observed_at_ms: XTRAIN_EXPLORER_GENERATED_AT_MS + 1_000,
                severity: RemoteTrainingEventSeverity::Warning,
                event_kind: String::from("checkpoint_promotion_held"),
                reference_ids: vec![decision.decision_id.clone(), disagreement.receipt_id.clone()],
                detail: String::from(
                    "Validator disagreement held the checkpoint candidate instead of silently promoting it.",
                ),
            },
            XtrainExplorerEvent {
                event_id: String::from("event.window1231.settlement_published"),
                observed_at_ms: XTRAIN_EXPLORER_GENERATED_AT_MS + 2_000,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("settlement_published"),
                reference_ids: vec![settlement_record.record_id.clone()],
                detail: String::from(
                    "The signed-ledger settlement record remained published even though promotion stayed held-no-promotion.",
                ),
            },
            XtrainExplorerEvent {
                event_id: String::from("event.window1231.local_rtx_refused"),
                observed_at_ms: XTRAIN_EXPLORER_GENERATED_AT_MS + 3_000,
                severity: RemoteTrainingEventSeverity::Warning,
                event_kind: String::from("participant_refused"),
                reference_ids: vec![local_rtx_refusal.refusal_id.clone()],
                detail: String::from(
                    "The local RTX standby miner remained excluded on checkpoint lag and is kept visible as explicit explorer truth.",
                ),
            },
        ],
        source_artifacts: vec![
            source_artifact(
                "public_run_explorer_contract",
                "fixtures/training/public_run_explorer_contract_v1.json",
                Some(explorer.contract_digest.clone()),
                true,
                "The prior public explorer contract remains the pane-level explorer foundation.",
            ),
            source_artifact(
                "public_network_registry_contract",
                "fixtures/training/public_network_registry_contract_v1.json",
                Some(registry.contract_digest.clone()),
                true,
                "The registry contract is authoritative for participant identity, roles, and endpoint truth.",
            ),
            source_artifact(
                "public_miner_protocol_contract",
                "fixtures/training/public_miner_protocol_contract_v1.json",
                Some(miner.contract_digest.clone()),
                true,
                "The public miner protocol contract is authoritative for miner sessions, local steps, and checkpoint-sync posture.",
            ),
            source_artifact(
                "multi_validator_consensus_contract",
                "fixtures/training/multi_validator_consensus_contract_v1.json",
                Some(consensus.contract_digest.clone()),
                true,
                "The multi-validator consensus contract is authoritative for held-no-promotion checkpoint state.",
            ),
            source_artifact(
                "settlement_publication_contract",
                "fixtures/training/settlement_publication_contract_v1.json",
                Some(settlement.contract_digest.clone()),
                true,
                "The settlement publication contract is authoritative for validator weight, settlement record, and payout export truth.",
            ),
            source_artifact(
                "curated_decentralized_run_contract",
                "fixtures/training/curated_decentralized_run_contract_v1.json",
                Some(curated.contract_digest.clone()),
                false,
                "The curated decentralized run contract keeps the retained evidence-bundle and after-action path explicit.",
            ),
            source_artifact(
                "bounded_xtrain_run_bundle",
                PARAMETER_GOLF_XTRAIN_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH,
                Some(xtrain_bundle.bundle_digest.clone()),
                false,
                "The bounded XTRAIN run-centric bundle remains the sibling local-reference score lane linked from the explorer snapshot.",
            ),
        ],
        detail: String::from(
            "The first XTRAIN explorer snapshot binds registry participants, current miner sessions, held checkpoint promotion, settlement publication, and the sibling bounded XTRAIN run surface into one explorer-oriented artifact family.",
        ),
        snapshot_digest: String::new(),
    };
    snapshot.snapshot_digest = stable_xtrain_snapshot_digest(&snapshot);
    snapshot.validate()?;
    Ok(snapshot)
}

pub fn canonical_xtrain_explorer_index() -> Result<XtrainExplorerIndex, XtrainExplorerArtifactsError>
{
    let snapshot = canonical_xtrain_explorer_snapshot()?;
    let mut index = XtrainExplorerIndex {
        schema_version: String::from(XTRAIN_EXPLORER_INDEX_SCHEMA_VERSION),
        index_id: String::from(XTRAIN_EXPLORER_INDEX_ID),
        generated_at_ms: snapshot.generated_at_ms,
        entries: vec![XtrainExplorerIndexEntry {
            snapshot_id: snapshot.snapshot_id.clone(),
            generated_at_ms: snapshot.generated_at_ms,
            network_id: snapshot.network_id.clone(),
            current_epoch_id: snapshot.current_epoch_id.clone(),
            active_window_id: snapshot.active_window_id.clone(),
            participant_count: snapshot.participants.len() as u16,
            held_checkpoint_count: snapshot
                .checkpoints
                .iter()
                .filter(|checkpoint| {
                    checkpoint.outcome == TrainingExecutionPromotionOutcome::HeldNoPromotion
                })
                .count() as u16,
            published_settlement_count: snapshot
                .windows
                .iter()
                .filter(|window| window.settlement_record_id.is_some())
                .count() as u16,
            snapshot_artifact_uri: Some(String::from(XTRAIN_EXPLORER_SNAPSHOT_FIXTURE_PATH)),
            snapshot_digest: Some(snapshot.snapshot_digest.clone()),
            semantic_summary: String::from(
                "The first XTRAIN explorer snapshot exposes participant graph state, held checkpoint promotion, settlement publication, and one sibling bounded XTRAIN score lane link.",
            ),
        }],
        detail: String::from(
            "The XTRAIN explorer index is the dedicated discovery surface for decentralized explorer snapshots. It stays separate from the run-centric remote-training index while linking to the bounded XTRAIN run lane where relevant.",
        ),
        index_digest: String::new(),
    };
    index.index_digest = stable_xtrain_index_digest(&index);
    index.validate()?;
    Ok(index)
}

pub fn write_xtrain_explorer_artifacts(
    snapshot_path: &Path,
    index_path: &Path,
) -> Result<(XtrainExplorerSnapshot, XtrainExplorerIndex), XtrainExplorerArtifactsError> {
    let snapshot = canonical_xtrain_explorer_snapshot()?;
    let index = canonical_xtrain_explorer_index()?;
    write_json(snapshot_path, &snapshot)?;
    write_json(index_path, &index)?;
    Ok((snapshot, index))
}

fn stable_xtrain_snapshot_digest(snapshot: &XtrainExplorerSnapshot) -> String {
    let mut canonical = snapshot.clone();
    canonical.snapshot_digest.clear();
    stable_digest(b"xtrain_explorer_snapshot|", &canonical)
}

fn stable_xtrain_index_digest(index: &XtrainExplorerIndex) -> String {
    let mut canonical = index.clone();
    canonical.index_digest.clear();
    stable_digest(b"xtrain_explorer_index|", &canonical)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("xtrain explorer artifact should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn source_artifact(
    artifact_role: &str,
    artifact_uri: &str,
    artifact_digest: Option<String>,
    authoritative: bool,
    detail: &str,
) -> XtrainExplorerSourceArtifact {
    XtrainExplorerSourceArtifact {
        artifact_role: String::from(artifact_role),
        artifact_uri: String::from(artifact_uri),
        artifact_digest,
        authoritative,
        detail: String::from(detail),
    }
}

fn find_registry_record<'a>(
    registry: &'a crate::PublicNetworkRegistryContract,
    registry_record_id: &str,
) -> Result<&'a crate::PublicNetworkRegistryRecord, XtrainExplorerArtifactsError> {
    registry
        .registry_records
        .iter()
        .find(|record| record.registry_record_id == registry_record_id)
        .ok_or_else(|| XtrainExplorerArtifactsError::InvalidArtifact {
            detail: format!("missing registry record `{registry_record_id}`"),
        })
}

fn find_miner_session<'a>(
    miner: &'a crate::PublicMinerProtocolContract,
    session_id: &str,
) -> Result<&'a crate::PublicMinerProtocolSession, XtrainExplorerArtifactsError> {
    miner
        .sessions
        .iter()
        .find(|session| session.session_id == session_id)
        .ok_or_else(|| XtrainExplorerArtifactsError::InvalidArtifact {
            detail: format!("missing miner session `{session_id}`"),
        })
}

fn find_miner_refusal<'a>(
    miner: &'a crate::PublicMinerProtocolContract,
    refusal_id: &str,
) -> Result<&'a crate::PublicMinerProtocolRefusal, XtrainExplorerArtifactsError> {
    miner
        .refusals
        .iter()
        .find(|refusal| refusal.refusal_id == refusal_id)
        .ok_or_else(|| XtrainExplorerArtifactsError::InvalidArtifact {
            detail: format!("missing miner refusal `{refusal_id}`"),
        })
}

fn find_consensus_decision<'a>(
    consensus: &'a crate::MultiValidatorConsensusContract,
    decision_id: &str,
) -> Result<&'a crate::MultiValidatorPromotionDecision, XtrainExplorerArtifactsError> {
    consensus
        .promotion_decisions
        .iter()
        .find(|decision| decision.decision_id == decision_id)
        .ok_or_else(|| XtrainExplorerArtifactsError::InvalidArtifact {
            detail: format!("missing consensus decision `{decision_id}`"),
        })
}

fn find_disagreement_receipt<'a>(
    consensus: &'a crate::MultiValidatorConsensusContract,
    receipt_id: &str,
) -> Result<&'a crate::MultiValidatorDisagreementReceipt, XtrainExplorerArtifactsError> {
    consensus
        .disagreement_receipts
        .iter()
        .find(|receipt| receipt.receipt_id == receipt_id)
        .ok_or_else(|| XtrainExplorerArtifactsError::InvalidArtifact {
            detail: format!("missing disagreement receipt `{receipt_id}`"),
        })
}

fn find_settlement_record<'a>(
    settlement: &'a crate::SettlementPublicationContract,
    record_id: &str,
) -> Result<&'a crate::SettlementRecord, XtrainExplorerArtifactsError> {
    settlement
        .settlement_records
        .iter()
        .find(|record| record.record_id == record_id)
        .ok_or_else(|| XtrainExplorerArtifactsError::InvalidArtifact {
            detail: format!("missing settlement record `{record_id}`"),
        })
}

fn find_payout_export<'a>(
    settlement: &'a crate::SettlementPublicationContract,
    node_identity_id: &str,
) -> Result<&'a crate::PayoutExport, XtrainExplorerArtifactsError> {
    settlement
        .payout_exports
        .iter()
        .find(|export| export.node_identity_id == node_identity_id)
        .ok_or_else(|| XtrainExplorerArtifactsError::InvalidArtifact {
            detail: format!("missing payout export for `{node_identity_id}`"),
        })
}

fn write_json<T: Serialize>(
    output_path: &Path,
    value: &T,
) -> Result<(), XtrainExplorerArtifactsError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| XtrainExplorerArtifactsError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    fs::write(
        output_path,
        format!("{}\n", serde_json::to_string_pretty(value)?),
    )
    .map_err(|error| XtrainExplorerArtifactsError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot() -> XtrainExplorerSnapshot {
        serde_json::from_str(include_str!(
            "../../../fixtures/training/xtrain_explorer_snapshot_v1.json"
        ))
        .expect("xtrain explorer snapshot should parse")
    }

    fn sample_index() -> XtrainExplorerIndex {
        serde_json::from_str(include_str!(
            "../../../fixtures/training/xtrain_explorer_index_v1.json"
        ))
        .expect("xtrain explorer index should parse")
    }

    #[test]
    fn xtrain_explorer_snapshot_stays_valid() -> Result<(), XtrainExplorerArtifactsError> {
        sample_snapshot().validate()
    }

    #[test]
    fn xtrain_explorer_index_stays_valid() -> Result<(), XtrainExplorerArtifactsError> {
        sample_index().validate()
    }

    #[test]
    fn canonical_snapshot_matches_fixture() -> Result<(), XtrainExplorerArtifactsError> {
        assert_eq!(canonical_xtrain_explorer_snapshot()?, sample_snapshot());
        Ok(())
    }

    #[test]
    fn canonical_index_matches_fixture() -> Result<(), XtrainExplorerArtifactsError> {
        assert_eq!(canonical_xtrain_explorer_index()?, sample_index());
        Ok(())
    }
}
