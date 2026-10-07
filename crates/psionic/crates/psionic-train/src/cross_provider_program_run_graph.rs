use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::OnceLock,
};

use psionic_cluster::{
    AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterMembershipStatus, ClusterNamespace,
    ClusterNodeIdentity, ClusterSnapshot, NodeEpoch, NodeId, NodeRole,
};
use psionic_datastream::{
    DatastreamEncoding, DatastreamPolicyWeightBinding, DatastreamSubjectKind,
    InMemoryDatastreamServer, InMemoryPolicyWeightBroadcast,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_contributor_program_lineage_contract, canonical_cross_provider_admission_plan,
    canonical_hybrid_pretraining_plan, canonical_shared_validator_promotion_contract,
    canonical_training_execution_evidence_bundle, cross_provider_training_program_manifest,
    ContributorProgramLineageError, CrossProviderAdmissionPlan, CrossProviderAdmissionPlannerError,
    CrossProviderComputeSourceContractError, CrossProviderExecutionClass,
    CrossProviderTrainingProgramManifestError, HybridPretrainingPlan, HybridPretrainingPlanError,
    PolicyRevision, SharedValidatorPromotionContractError, TrainingOrchestratorError,
    TrainingOrchestratorState, TrainingParticipantAdmissionState,
    TrainingParticipantContributionState, TrainingParticipantReadinessState,
    TrainingParticipantRole, TrainingRunGraphError, TrainingRunState, TrainingWindowAssignmentRule,
};

/// Stable schema version for the whole-program cross-provider run graph.
pub const CROSS_PROVIDER_PROGRAM_RUN_GRAPH_SCHEMA_VERSION: &str =
    "psionic.cross_provider_program_run_graph.v1";
/// Stable fixture path for the whole-program cross-provider run graph.
pub const CROSS_PROVIDER_PROGRAM_RUN_GRAPH_FIXTURE_PATH: &str =
    "fixtures/training/cross_provider_program_run_graph_v1.json";
/// Stable checker path for the whole-program cross-provider run graph.
pub const CROSS_PROVIDER_PROGRAM_RUN_GRAPH_CHECK_SCRIPT_PATH: &str =
    "scripts/check-cross-provider-program-run-graph.sh";
/// Stable reference doc path for the whole-program cross-provider run graph.
pub const CROSS_PROVIDER_PROGRAM_RUN_GRAPH_DOC_PATH: &str =
    "docs/CROSS_PROVIDER_PROGRAM_RUN_GRAPH_REFERENCE.md";

const CROSS_PROVIDER_PROGRAM_RUN_ID: &str = "psion-xprovider-pretrain-whole-program-001";
const CROSS_PROVIDER_PROGRAM_CLUSTER_NAMESPACE: &str = "cross-provider-whole-program";
const CROSS_PROVIDER_PROGRAM_CLUSTER_ADMISSION_TOKEN: &str = "xtrain-shared-secret";
const CROSS_PROVIDER_PROGRAM_BASE_TIME_MS: u64 = 1_710_002_000_000;

/// Error surfaced while building, validating, or writing the whole-program run graph.
#[derive(Debug, Error)]
pub enum CrossProviderProgramRunGraphError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    AdmissionPlan(#[from] CrossProviderAdmissionPlannerError),
    #[error(transparent)]
    ComputeSource(#[from] CrossProviderComputeSourceContractError),
    #[error(transparent)]
    HybridPlan(#[from] HybridPretrainingPlanError),
    #[error(transparent)]
    ContributorLineage(#[from] ContributorProgramLineageError),
    #[error(transparent)]
    ValidatorPromotion(#[from] SharedValidatorPromotionContractError),
    #[error(transparent)]
    EvidenceBundle(#[from] crate::TrainingExecutionEvidenceBundleError),
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    #[error(transparent)]
    Orchestrator(#[from] TrainingOrchestratorError),
    #[error("cross-provider program run graph is invalid: {detail}")]
    InvalidGraph { detail: String },
}

/// How one role is reconstructed from retained evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderProgramEvidenceBindingKind {
    /// The role is reconstructed from one retained execution segment.
    ExecutionSegment,
    /// The role is reconstructed from manifest-owned dataset authority.
    ManifestDatasetAuthority,
}

/// Program-level posture for one role participant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderProgramParticipantPosture {
    Active,
    Standby,
    Quarantined,
}

/// Typed transition over one whole-program participant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderProgramTransitionKind {
    Admitted,
    Activated,
    HeldStandby,
    Quarantined,
    EvidenceLinked,
}

/// Evidence binding carried by one participant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderProgramEvidenceBinding {
    pub binding_kind: CrossProviderProgramEvidenceBindingKind,
    pub binding_id: String,
    pub detail: String,
}

/// One typed whole-program participant under the shared run id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderProgramParticipant {
    pub participant_id: String,
    pub node_id: String,
    pub source_id: String,
    pub execution_class: CrossProviderExecutionClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage_slot_id: Option<String>,
    pub run_graph_role: TrainingParticipantRole,
    pub admission_state: TrainingParticipantAdmissionState,
    pub readiness_state: TrainingParticipantReadinessState,
    pub contribution_state: TrainingParticipantContributionState,
    pub program_posture: CrossProviderProgramParticipantPosture,
    pub evidence_binding: CrossProviderProgramEvidenceBinding,
    pub detail: String,
}

/// One typed role-composition window over the whole pretraining program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderProgramRoleWindow {
    pub role_window_id: String,
    pub run_id: String,
    pub orchestrator_window_id: String,
    pub topology_revision_id: String,
    pub contributor_set_revision_id: String,
    pub active_dense_rank_participant_ids: Vec<String>,
    pub active_contributor_window_participant_ids: Vec<String>,
    pub active_validator_participant_ids: Vec<String>,
    pub quarantined_validator_participant_ids: Vec<String>,
    pub active_checkpoint_writer_participant_ids: Vec<String>,
    pub standby_checkpoint_writer_participant_ids: Vec<String>,
    pub active_eval_worker_participant_ids: Vec<String>,
    pub active_data_builder_participant_ids: Vec<String>,
    pub role_window_digest: String,
}

/// One typed transition over one whole-program participant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderProgramTransitionRecord {
    pub transition_id: String,
    pub participant_id: String,
    pub execution_class: CrossProviderExecutionClass,
    pub kind: CrossProviderProgramTransitionKind,
    pub occurred_at_ms: u64,
    pub detail: String,
    pub transition_digest: String,
}

/// Whole-program cross-provider run graph over the shared training run id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrossProviderProgramRunGraph {
    pub schema_version: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub admission_plan_digest: String,
    pub contributor_lineage_digest: String,
    pub validator_promotion_contract_id: String,
    pub evidence_bundle_id: String,
    pub evidence_bundle_digest: String,
    pub run_id: String,
    pub stage_id: String,
    pub orchestrator: TrainingOrchestratorState,
    pub participants: Vec<CrossProviderProgramParticipant>,
    pub role_windows: Vec<CrossProviderProgramRoleWindow>,
    pub transition_log: Vec<CrossProviderProgramTransitionRecord>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl CrossProviderProgramRunGraph {
    /// Returns the stable digest for the run-graph payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_cross_provider_program_run_graph|", &clone)
    }

    /// Validates the whole-program run graph against the canonical program inputs.
    pub fn validate(&self) -> Result<(), CrossProviderProgramRunGraphError> {
        let manifest = cross_provider_training_program_manifest()?;
        let admission_plan = canonical_cross_provider_admission_plan()?;
        let hybrid_plan = canonical_hybrid_pretraining_plan()?;
        let contributor_lineage = canonical_contributor_program_lineage_contract()?;
        let validator_contract = canonical_shared_validator_promotion_contract()?;
        let evidence_bundle = canonical_training_execution_evidence_bundle()?;

        manifest.validate()?;
        admission_plan.validate()?;
        hybrid_plan.validate(
            &manifest,
            &crate::canonical_cross_provider_compute_source_contracts()?,
        )?;
        contributor_lineage.validate()?;
        validator_contract.validate()?;
        evidence_bundle.validate(
            &manifest,
            &crate::canonical_cross_provider_compute_source_contracts()?,
        )?;

        if self.schema_version != CROSS_PROVIDER_PROGRAM_RUN_GRAPH_SCHEMA_VERSION {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    CROSS_PROVIDER_PROGRAM_RUN_GRAPH_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("program manifest binding drifted"),
            });
        }
        if self.admission_plan_digest != admission_plan.plan_digest {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("admission plan digest drifted"),
            });
        }
        if self.contributor_lineage_digest != contributor_lineage.contract_digest {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("contributor lineage digest drifted"),
            });
        }
        if self.validator_promotion_contract_id != validator_contract.contract_id {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("validator promotion contract id drifted"),
            });
        }
        if self.evidence_bundle_id != evidence_bundle.bundle_id
            || self.evidence_bundle_digest != evidence_bundle.bundle_digest
        {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("evidence bundle binding drifted"),
            });
        }
        if self.run_id != self.orchestrator.run.run_id
            || self.stage_id != self.orchestrator.run.stage_id
        {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("top-level run identity drifted from orchestrator run"),
            });
        }
        if self.orchestrator.run.program_manifest_id.as_deref()
            != Some(self.program_manifest_id.as_str())
            || self.orchestrator.run.program_manifest_digest.as_deref()
                != Some(self.program_manifest_digest.as_str())
        {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("orchestrator run lost manifest binding"),
            });
        }
        if self.orchestrator.orchestrator_windows.len() != 1 || self.role_windows.len() != 1 {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from(
                    "whole-program run graph must keep exactly one canonical orchestrator window and one role window",
                ),
            });
        }

        let expected_participants = expected_participant_slots(&hybrid_plan, &admission_plan);
        if self.participants.len() != expected_participants.len() {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: format!(
                    "participant count drifted: expected {}, found {}",
                    expected_participants.len(),
                    self.participants.len()
                ),
            });
        }
        let actual_by_id = self
            .participants
            .iter()
            .map(|participant| (participant.participant_id.as_str(), participant))
            .collect::<BTreeMap<_, _>>();
        for seed in &expected_participants {
            let participant = actual_by_id
                .get(seed.participant_id.as_str())
                .ok_or_else(|| CrossProviderProgramRunGraphError::InvalidGraph {
                    detail: format!("missing participant `{}`", seed.participant_id),
                })?;
            if participant.node_id != seed.node_id
                || participant.source_id != seed.source_id
                || participant.execution_class != seed.execution_class
                || participant.program_posture != seed.program_posture
            {
                return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                    detail: format!(
                        "participant `{}` drifted from canonical whole-program role seed",
                        seed.participant_id
                    ),
                });
            }
            if participant.evidence_binding.binding_kind
                == CrossProviderProgramEvidenceBindingKind::ExecutionSegment
                && !evidence_bundle
                    .segment_evidence
                    .iter()
                    .any(|segment| segment.segment_id == participant.evidence_binding.binding_id)
            {
                return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                    detail: format!(
                        "participant `{}` references unknown evidence segment `{}`",
                        participant.participant_id, participant.evidence_binding.binding_id
                    ),
                });
            }
            if participant.evidence_binding.binding_kind
                == CrossProviderProgramEvidenceBindingKind::ManifestDatasetAuthority
                && participant.evidence_binding.binding_id != manifest.dataset_family_id
            {
                return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                    detail: format!(
                        "participant `{}` lost canonical dataset authority binding",
                        participant.participant_id
                    ),
                });
            }
            if !self
                .orchestrator
                .run
                .participants
                .iter()
                .any(|record| record.node_id.as_str() == participant.node_id)
            {
                return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                    detail: format!(
                        "participant `{}` node `{}` does not exist in the run graph",
                        participant.participant_id, participant.node_id
                    ),
                });
            }
        }
        let actual_classes = self
            .participants
            .iter()
            .map(|participant| participant.execution_class)
            .collect::<BTreeSet<_>>();
        let expected_classes = manifest
            .admitted_execution_classes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if actual_classes != expected_classes {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from(
                    "whole-program participant class set must cover every admitted execution class exactly once or more",
                ),
            });
        }
        let role_window = &self.role_windows[0];
        let orchestrator_window = &self.orchestrator.orchestrator_windows[0];
        if role_window.run_id != self.run_id
            || role_window.orchestrator_window_id != orchestrator_window.window_id
        {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("role window lost orchestrator window binding"),
            });
        }
        if role_window.contributor_set_revision_id
            != orchestrator_window.contributor_set_revision_id
        {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("role window lost contributor-set revision binding"),
            });
        }
        let run_topology = self
            .orchestrator
            .run
            .topology_revisions
            .last()
            .ok_or_else(|| CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("orchestrator run lost topology revisions"),
            })?;
        if role_window.topology_revision_id != run_topology.topology_revision_id {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("role window lost topology revision binding"),
            });
        }
        let active_contributor_nodes = orchestrator_window
            .rollout_assignments
            .iter()
            .map(|assignment| assignment.contributor_node_id.as_str())
            .collect::<BTreeSet<_>>();
        let active_contributor_participant_nodes = role_window
            .active_contributor_window_participant_ids
            .iter()
            .filter_map(|participant_id| actual_by_id.get(participant_id.as_str()))
            .map(|participant| participant.node_id.as_str())
            .collect::<BTreeSet<_>>();
        if active_contributor_nodes != active_contributor_participant_nodes {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from(
                    "active contributor-window participants must stay aligned with orchestrator rollout assignments",
                ),
            });
        }
        if role_window.active_dense_rank_participant_ids.len()
            != hybrid_plan.dense_rank_assignments.len()
        {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("dense-rank role count drifted from the hybrid plan"),
            });
        }
        if role_window.active_validator_participant_ids.len() != 1
            || role_window.quarantined_validator_participant_ids.len() != 1
            || role_window.active_checkpoint_writer_participant_ids.len() != 1
            || role_window.standby_checkpoint_writer_participant_ids.len() != 1
        {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from(
                    "whole-program role window lost the canonical active versus standby validator or checkpoint split",
                ),
            });
        }
        if self.transition_log.len() < self.participants.len() * 2 {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("transition log is too small to audit participant lifecycle"),
            });
        }
        for participant in &self.participants {
            let participant_transitions = self
                .transition_log
                .iter()
                .filter(|transition| transition.participant_id == participant.participant_id)
                .collect::<Vec<_>>();
            if !participant_transitions
                .iter()
                .any(|transition| transition.kind == CrossProviderProgramTransitionKind::Admitted)
                || !participant_transitions.iter().any(|transition| {
                    transition.kind == CrossProviderProgramTransitionKind::EvidenceLinked
                })
            {
                return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                    detail: format!(
                        "participant `{}` lost an admitted or evidence-linked transition",
                        participant.participant_id
                    ),
                });
            }
        }
        if self.contract_digest != self.stable_digest() {
            return Err(CrossProviderProgramRunGraphError::InvalidGraph {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical whole-program cross-provider run graph.
static CROSS_PROVIDER_PROGRAM_RUN_GRAPH_CACHE: OnceLock<CrossProviderProgramRunGraph> =
    OnceLock::new();

pub fn canonical_cross_provider_program_run_graph(
) -> Result<CrossProviderProgramRunGraph, CrossProviderProgramRunGraphError> {
    if let Some(graph) = CROSS_PROVIDER_PROGRAM_RUN_GRAPH_CACHE.get() {
        return Ok(graph.clone());
    }
    let manifest = cross_provider_training_program_manifest()?;
    let admission_plan = canonical_cross_provider_admission_plan()?;
    let hybrid_plan = canonical_hybrid_pretraining_plan()?;
    let contributor_lineage = canonical_contributor_program_lineage_contract()?;
    let validator_contract = canonical_shared_validator_promotion_contract()?;
    let evidence_bundle = canonical_training_execution_evidence_bundle()?;
    let participant_seeds = expected_participant_slots(&hybrid_plan, &admission_plan);

    let cluster_state = cross_provider_program_cluster_state(&participant_seeds);
    let mut run = TrainingRunState::new(
        CROSS_PROVIDER_PROGRAM_RUN_ID,
        manifest.stage_authority.stage_id.clone(),
        cluster_state.cluster_id().as_str(),
        manifest.checkpoint_family.clone(),
        manifest.environment.clone(),
    )?;
    manifest.bind_run_state(&mut run)?;
    run.apply_cluster_membership_snapshot(&cluster_state, CROSS_PROVIDER_PROGRAM_BASE_TIME_MS)?;
    for seed in &participant_seeds {
        run.update_participant_priority(
            &NodeId::new(seed.node_id.clone()),
            priority_for_execution_class(seed.execution_class),
            reliability_for_posture(seed.program_posture),
            CROSS_PROVIDER_PROGRAM_BASE_TIME_MS + 20,
        )?;
    }

    let target_policy_revision = contributor_lineage.input_policy_revision.clone();
    let policy_weight_broadcast = policy_weight_broadcast(&target_policy_revision)?;
    let mut orchestrator =
        TrainingOrchestratorState::new(run, target_policy_revision, policy_weight_broadcast)?;
    orchestrator.plan_next_window(
        hybrid_plan.contributor_window_assignments.len(),
        TrainingWindowAssignmentRule::RoundRobinByPriority {
            batch_slice_count: 6,
            eval_slice_count: 3,
        },
        701,
        CROSS_PROVIDER_PROGRAM_BASE_TIME_MS + 200,
    )?;
    orchestrator.activate_current_window(CROSS_PROVIDER_PROGRAM_BASE_TIME_MS + 220)?;

    let run_records = orchestrator
        .run
        .participants
        .iter()
        .map(|record| (record.node_id.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    let participants = participant_seeds
        .iter()
        .map(|seed| {
            let run_record = run_records
                .get(seed.node_id.as_str())
                .expect("canonical participant node must exist in run graph");
            CrossProviderProgramParticipant {
                participant_id: seed.participant_id.clone(),
                node_id: seed.node_id.clone(),
                source_id: seed.source_id.clone(),
                execution_class: seed.execution_class,
                assignment_id: seed.assignment_id.clone(),
                lineage_slot_id: seed.lineage_slot_id.clone(),
                run_graph_role: run_record.role,
                admission_state: run_record.admission_state,
                readiness_state: run_record.readiness_state,
                contribution_state: run_record.contribution_state,
                program_posture: seed.program_posture,
                evidence_binding: seed.evidence_binding.clone(),
                detail: seed.detail.clone(),
            }
        })
        .collect::<Vec<_>>();

    let role_window = canonical_role_window(&participants, &orchestrator);
    let transition_log = canonical_transition_log(&participants);
    let mut graph = CrossProviderProgramRunGraph {
        schema_version: String::from(CROSS_PROVIDER_PROGRAM_RUN_GRAPH_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        admission_plan_digest: admission_plan.plan_digest.clone(),
        contributor_lineage_digest: contributor_lineage.contract_digest.clone(),
        validator_promotion_contract_id: validator_contract.contract_id.clone(),
        evidence_bundle_id: evidence_bundle.bundle_id.clone(),
        evidence_bundle_digest: evidence_bundle.bundle_digest.clone(),
        run_id: orchestrator.run.run_id.clone(),
        stage_id: orchestrator.run.stage_id.clone(),
        orchestrator,
        participants,
        role_windows: vec![role_window],
        transition_log,
        claim_boundary: String::from(
            "This artifact proves one provider-neutral run graph can carry dense ranks, contributor windows, validators, checkpoint writers, eval workers, and data builders under one shared run id. It does not yet claim dense recovery, elastic world-size closure, or a retained multi-provider dense proof run.",
        ),
        contract_digest: String::new(),
    };
    graph.contract_digest = graph.stable_digest();
    graph.validate()?;
    let _ = CROSS_PROVIDER_PROGRAM_RUN_GRAPH_CACHE.set(graph.clone());
    Ok(graph)
}

/// Writes the canonical whole-program cross-provider run graph to disk.
pub fn write_cross_provider_program_run_graph(
    output_path: impl AsRef<Path>,
) -> Result<(), CrossProviderProgramRunGraphError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CrossProviderProgramRunGraphError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let graph = canonical_cross_provider_program_run_graph()?;
    let json = serde_json::to_vec_pretty(&graph)?;
    fs::write(output_path, json).map_err(|error| CrossProviderProgramRunGraphError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

#[derive(Clone, Debug)]
struct WholeProgramParticipantSeed {
    participant_id: String,
    node_id: String,
    source_id: String,
    execution_class: CrossProviderExecutionClass,
    assignment_id: Option<String>,
    lineage_slot_id: Option<String>,
    program_posture: CrossProviderProgramParticipantPosture,
    evidence_binding: CrossProviderProgramEvidenceBinding,
    detail: String,
}

fn expected_participant_slots(
    hybrid_plan: &HybridPretrainingPlan,
    admission_plan: &CrossProviderAdmissionPlan,
) -> Vec<WholeProgramParticipantSeed> {
    let mut seeds = Vec::new();
    for assignment in &hybrid_plan.dense_rank_assignments {
        seeds.push(WholeProgramParticipantSeed {
            participant_id: assignment.assignment_id.clone(),
            node_id: format!("{}--dense-rank-{}", assignment.source_id, assignment.dense_rank),
            source_id: assignment.source_id.clone(),
            execution_class: CrossProviderExecutionClass::DenseFullModelRank,
            assignment_id: Some(assignment.assignment_id.clone()),
            lineage_slot_id: Some(assignment.lineage_slot_id.clone()),
            program_posture: CrossProviderProgramParticipantPosture::Active,
            evidence_binding: CrossProviderProgramEvidenceBinding {
                binding_kind: CrossProviderProgramEvidenceBindingKind::ExecutionSegment,
                binding_id: String::from("hybrid-program"),
                detail: String::from(
                    "Dense ranks are reconstructed from the retained hybrid-program segment and the dense-distributed runtime receipts that segment points at.",
                ),
            },
            detail: format!(
                "Dense rank {} stays under the shared run id instead of splintering into a provider-local distributed job identity.",
                assignment.dense_rank
            ),
        });
    }
    for assignment in &hybrid_plan.contributor_window_assignments {
        seeds.push(WholeProgramParticipantSeed {
            participant_id: assignment.window_id.clone(),
            node_id: format!("{}--{}", assignment.source_id, assignment.window_id),
            source_id: assignment.source_id.clone(),
            execution_class: CrossProviderExecutionClass::ValidatedContributorWindow,
            assignment_id: Some(assignment.window_id.clone()),
            lineage_slot_id: Some(assignment.lineage_slot_id.clone()),
            program_posture: CrossProviderProgramParticipantPosture::Active,
            evidence_binding: CrossProviderProgramEvidenceBinding {
                binding_kind: CrossProviderProgramEvidenceBindingKind::ExecutionSegment,
                binding_id: String::from("contributor-window"),
                detail: String::from(
                    "Validated contributor windows are reconstructed from the contributor-window execution segment and the shared lineage bridge.",
                ),
            },
            detail: format!(
                "Validated contributor window `{}` keeps its own slot identity while staying inside the same pretraining run id.",
                assignment.window_id
            ),
        });
    }
    for assignment in &hybrid_plan.validator_assignments {
        let posture = if assignment.source_id == "local_mlx_mac_workstation" {
            CrossProviderProgramParticipantPosture::Quarantined
        } else {
            CrossProviderProgramParticipantPosture::Active
        };
        seeds.push(WholeProgramParticipantSeed {
            participant_id: assignment.assignment_id.clone(),
            node_id: format!("{}--{}", assignment.source_id, assignment.assignment_id),
            source_id: assignment.source_id.clone(),
            execution_class: CrossProviderExecutionClass::Validator,
            assignment_id: Some(assignment.assignment_id.clone()),
            lineage_slot_id: Some(assignment.lineage_slot_id.clone()),
            program_posture: posture,
            evidence_binding: CrossProviderProgramEvidenceBinding {
                binding_kind: CrossProviderProgramEvidenceBindingKind::ExecutionSegment,
                binding_id: if posture == CrossProviderProgramParticipantPosture::Active {
                    String::from("validator-only")
                } else {
                    String::from("hybrid-program")
                },
                detail: if posture == CrossProviderProgramParticipantPosture::Active {
                    String::from(
                        "The primary validator is reconstructed from the validator-only retained evidence segment.",
                    )
                } else {
                    String::from(
                        "The secondary validator remains present under the shared run id but is quarantined in the hybrid evidence segment until replay closure exists.",
                    )
                },
            },
            detail: format!(
                "Validator `{}` stays typed as validator work instead of collapsing into a contributor or eval role.",
                assignment.assignment_id
            ),
        });
    }
    for assignment in &hybrid_plan.checkpoint_writer_assignments {
        let posture = if assignment.source_id == "runpod_8xh100_dense_node" {
            CrossProviderProgramParticipantPosture::Active
        } else {
            CrossProviderProgramParticipantPosture::Standby
        };
        seeds.push(WholeProgramParticipantSeed {
            participant_id: assignment.assignment_id.clone(),
            node_id: format!("{}--{}", assignment.source_id, assignment.assignment_id),
            source_id: assignment.source_id.clone(),
            execution_class: CrossProviderExecutionClass::CheckpointWriter,
            assignment_id: Some(assignment.assignment_id.clone()),
            lineage_slot_id: Some(assignment.lineage_slot_id.clone()),
            program_posture: posture,
            evidence_binding: CrossProviderProgramEvidenceBinding {
                binding_kind: CrossProviderProgramEvidenceBindingKind::ExecutionSegment,
                binding_id: String::from("hybrid-program"),
                detail: String::from(
                    "Checkpoint writers are reconstructed from the hybrid segment plus the provider-neutral checkpoint authority that segment retains.",
                ),
            },
            detail: if posture == CrossProviderProgramParticipantPosture::Active {
                String::from(
                    "The dense provider-local writer stays active as the current shard writer for the whole program.",
                )
            } else {
                String::from(
                    "The secondary checkpoint writer stays admitted under the same run id but remains standby until recovery or topology revision promotes it.",
                )
            },
        });
    }
    for assignment in &hybrid_plan.eval_assignments {
        seeds.push(WholeProgramParticipantSeed {
            participant_id: assignment.assignment_id.clone(),
            node_id: format!("{}--{}", assignment.source_id, assignment.assignment_id),
            source_id: assignment.source_id.clone(),
            execution_class: CrossProviderExecutionClass::EvalWorker,
            assignment_id: Some(assignment.assignment_id.clone()),
            lineage_slot_id: Some(assignment.lineage_slot_id.clone()),
            program_posture: CrossProviderProgramParticipantPosture::Active,
            evidence_binding: CrossProviderProgramEvidenceBinding {
                binding_kind: CrossProviderProgramEvidenceBindingKind::ExecutionSegment,
                binding_id: String::from("hybrid-program"),
                detail: String::from(
                    "Eval workers are reconstructed from the hybrid execution segment because they score the same program revision while staying separate from validators.",
                ),
            },
            detail: format!(
                "Eval worker `{}` remains a typed eval lane under the shared run id.",
                assignment.assignment_id
            ),
        });
    }
    for source_id in
        selected_sources_for_class(admission_plan, CrossProviderExecutionClass::DataBuilder)
    {
        seeds.push(WholeProgramParticipantSeed {
            participant_id: format!("data-builder-{source_id}"),
            node_id: format!("{source_id}--data-builder"),
            source_id: source_id.clone(),
            execution_class: CrossProviderExecutionClass::DataBuilder,
            assignment_id: Some(format!("data-builder-{source_id}")),
            lineage_slot_id: None,
            program_posture: CrossProviderProgramParticipantPosture::Active,
            evidence_binding: CrossProviderProgramEvidenceBinding {
                binding_kind: CrossProviderProgramEvidenceBindingKind::ManifestDatasetAuthority,
                binding_id: String::from(
                    crate::CROSS_PROVIDER_TRAINING_PROGRAM_DATASET_FAMILY_ID,
                ),
                detail: String::from(
                    "Data builders are reconstructed from the canonical dataset-family authority because provider-neutral final execution segments do not yet emit a dedicated data-builder segment.",
                ),
            },
            detail: String::from(
                "The data-builder role is admitted under the same run id as the dense and validation roles instead of living in a separate hidden data pipeline identity.",
            ),
        });
    }
    seeds
}

fn cross_provider_program_cluster_state(
    participant_seeds: &[WholeProgramParticipantSeed],
) -> psionic_cluster::ClusterState {
    let cluster_id = ClusterId::new(
        &ClusterNamespace::new(CROSS_PROVIDER_PROGRAM_CLUSTER_NAMESPACE),
        &AdmissionToken::new(CROSS_PROVIDER_PROGRAM_CLUSTER_ADMISSION_TOKEN),
    );
    let memberships = participant_seeds
        .iter()
        .enumerate()
        .map(|(index, seed)| {
            let node_id = NodeId::new(seed.node_id.clone());
            (
                node_id.clone(),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id: cluster_id.clone(),
                        node_id,
                        node_epoch: NodeEpoch::initial(),
                        role: node_role_for_execution_class(seed.execution_class),
                        auth_public_key: format!("{}-pk", seed.node_id),
                        attestation: None,
                    },
                    Some(SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::LOCALHOST),
                        32_000 + index as u16,
                    )),
                    ClusterMembershipStatus::Ready,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut snapshot = ClusterSnapshot::new(cluster_id);
    snapshot.memberships = memberships;
    psionic_cluster::ClusterState::from_snapshot(snapshot)
}

fn node_role_for_execution_class(execution_class: CrossProviderExecutionClass) -> NodeRole {
    match execution_class {
        CrossProviderExecutionClass::DenseFullModelRank
        | CrossProviderExecutionClass::CheckpointWriter => NodeRole::Mixed,
        CrossProviderExecutionClass::ValidatedContributorWindow
        | CrossProviderExecutionClass::Validator
        | CrossProviderExecutionClass::EvalWorker
        | CrossProviderExecutionClass::DataBuilder => NodeRole::ExecutorOnly,
    }
}

fn priority_for_execution_class(execution_class: CrossProviderExecutionClass) -> u16 {
    match execution_class {
        CrossProviderExecutionClass::ValidatedContributorWindow => 9_800,
        CrossProviderExecutionClass::DenseFullModelRank => 7_400,
        CrossProviderExecutionClass::CheckpointWriter => 6_900,
        CrossProviderExecutionClass::Validator => 6_600,
        CrossProviderExecutionClass::EvalWorker => 6_400,
        CrossProviderExecutionClass::DataBuilder => 6_200,
    }
}

fn reliability_for_posture(posture: CrossProviderProgramParticipantPosture) -> u16 {
    match posture {
        CrossProviderProgramParticipantPosture::Active => 9_200,
        CrossProviderProgramParticipantPosture::Standby => 8_600,
        CrossProviderProgramParticipantPosture::Quarantined => 6_000,
    }
}

fn policy_weight_broadcast(
    policy_revision: &PolicyRevision,
) -> Result<
    psionic_datastream::DatastreamPolicyWeightBroadcastManifest,
    CrossProviderProgramRunGraphError,
> {
    let shard_a = b"xtrain-whole-program-weights-a".repeat(8);
    let shard_b = b"xtrain-whole-program-weights-b".repeat(8);
    let assembled_digest = {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&shard_a);
        bytes.extend_from_slice(&shard_b);
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    };
    let revision_number = policy_revision.revision_number.unwrap_or(7);
    let manifest_a = psionic_datastream::DatastreamManifest::from_bytes(
        "xtrain-program-policy-shard-a",
        DatastreamSubjectKind::PolicyWeights,
        &shard_a,
        8,
        DatastreamEncoding::Safetensors,
    )
    .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
        policy_revision.policy_family.as_str(),
        revision_number,
        "shard-a",
        0,
        2,
        assembled_digest.clone(),
        policy_revision.produced_at_ms,
        policy_revision.produced_at_ms + 60_000,
    ));
    let manifest_b = psionic_datastream::DatastreamManifest::from_bytes(
        "xtrain-program-policy-shard-b",
        DatastreamSubjectKind::PolicyWeights,
        &shard_b,
        8,
        DatastreamEncoding::Safetensors,
    )
    .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
        policy_revision.policy_family.as_str(),
        revision_number,
        "shard-b",
        1,
        2,
        assembled_digest,
        policy_revision.produced_at_ms,
        policy_revision.produced_at_ms + 60_000,
    ));
    Ok(InMemoryPolicyWeightBroadcast::new(
        vec![
            InMemoryDatastreamServer::new(manifest_a, shard_a).map_err(|error| {
                CrossProviderProgramRunGraphError::InvalidGraph {
                    detail: format!("failed to build policy-weight shard a: {error}"),
                }
            })?,
            InMemoryDatastreamServer::new(manifest_b, shard_b).map_err(|error| {
                CrossProviderProgramRunGraphError::InvalidGraph {
                    detail: format!("failed to build policy-weight shard b: {error}"),
                }
            })?,
        ],
        policy_revision.produced_at_ms + 500,
    )
    .map_err(|error| CrossProviderProgramRunGraphError::InvalidGraph {
        detail: format!("failed to build policy-weight broadcast: {error}"),
    })?
    .broadcast()
    .clone())
}

fn canonical_role_window(
    participants: &[CrossProviderProgramParticipant],
    orchestrator: &TrainingOrchestratorState,
) -> CrossProviderProgramRoleWindow {
    let run_topology = orchestrator
        .run
        .topology_revisions
        .last()
        .expect("canonical run graph must keep a topology revision");
    let orchestrator_window = orchestrator
        .orchestrator_windows
        .first()
        .expect("canonical whole-program graph must keep an orchestrator window");
    let participant_ids_for =
        |execution_class: CrossProviderExecutionClass,
         posture: CrossProviderProgramParticipantPosture| {
            participants
                .iter()
                .filter(|participant| {
                    participant.execution_class == execution_class
                        && participant.program_posture == posture
                })
                .map(|participant| participant.participant_id.clone())
                .collect::<Vec<_>>()
        };
    let mut window = CrossProviderProgramRoleWindow {
        role_window_id: format!("{}-role-window-1", orchestrator.run.run_id),
        run_id: orchestrator.run.run_id.clone(),
        orchestrator_window_id: orchestrator_window.window_id.clone(),
        topology_revision_id: run_topology.topology_revision_id.clone(),
        contributor_set_revision_id: orchestrator_window.contributor_set_revision_id.clone(),
        active_dense_rank_participant_ids: participant_ids_for(
            CrossProviderExecutionClass::DenseFullModelRank,
            CrossProviderProgramParticipantPosture::Active,
        ),
        active_contributor_window_participant_ids: participant_ids_for(
            CrossProviderExecutionClass::ValidatedContributorWindow,
            CrossProviderProgramParticipantPosture::Active,
        ),
        active_validator_participant_ids: participant_ids_for(
            CrossProviderExecutionClass::Validator,
            CrossProviderProgramParticipantPosture::Active,
        ),
        quarantined_validator_participant_ids: participant_ids_for(
            CrossProviderExecutionClass::Validator,
            CrossProviderProgramParticipantPosture::Quarantined,
        ),
        active_checkpoint_writer_participant_ids: participant_ids_for(
            CrossProviderExecutionClass::CheckpointWriter,
            CrossProviderProgramParticipantPosture::Active,
        ),
        standby_checkpoint_writer_participant_ids: participant_ids_for(
            CrossProviderExecutionClass::CheckpointWriter,
            CrossProviderProgramParticipantPosture::Standby,
        ),
        active_eval_worker_participant_ids: participant_ids_for(
            CrossProviderExecutionClass::EvalWorker,
            CrossProviderProgramParticipantPosture::Active,
        ),
        active_data_builder_participant_ids: participant_ids_for(
            CrossProviderExecutionClass::DataBuilder,
            CrossProviderProgramParticipantPosture::Active,
        ),
        role_window_digest: String::new(),
    };
    window.role_window_digest = stable_role_window_digest(&window);
    window
}

fn canonical_transition_log(
    participants: &[CrossProviderProgramParticipant],
) -> Vec<CrossProviderProgramTransitionRecord> {
    let mut transitions = Vec::new();
    for (index, participant) in participants.iter().enumerate() {
        let offset = index as u64 * 10;
        transitions.push(transition_record(
            participant,
            CrossProviderProgramTransitionKind::Admitted,
            CROSS_PROVIDER_PROGRAM_BASE_TIME_MS + offset,
            "Participant joined the whole-program run graph under the shared run id.",
        ));
        let (kind, detail) = match participant.program_posture {
            CrossProviderProgramParticipantPosture::Active => (
                CrossProviderProgramTransitionKind::Activated,
                "Participant was activated for the current whole-program role window.",
            ),
            CrossProviderProgramParticipantPosture::Standby => (
                CrossProviderProgramTransitionKind::HeldStandby,
                "Participant remains admitted but standby inside the current whole-program role window.",
            ),
            CrossProviderProgramParticipantPosture::Quarantined => (
                CrossProviderProgramTransitionKind::Quarantined,
                "Participant remains admitted but quarantined under the shared validator contract.",
            ),
        };
        transitions.push(transition_record(
            participant,
            kind,
            CROSS_PROVIDER_PROGRAM_BASE_TIME_MS + 500 + offset,
            detail,
        ));
        transitions.push(transition_record(
            participant,
            CrossProviderProgramTransitionKind::EvidenceLinked,
            CROSS_PROVIDER_PROGRAM_BASE_TIME_MS + 1_000 + offset,
            "Participant role was bound to retained provider-neutral evidence for later whole-program reconstruction.",
        ));
    }
    transitions
}

fn transition_record(
    participant: &CrossProviderProgramParticipant,
    kind: CrossProviderProgramTransitionKind,
    occurred_at_ms: u64,
    detail: &str,
) -> CrossProviderProgramTransitionRecord {
    let transition_id = format!(
        "{}-{}-{}",
        participant.participant_id,
        transition_kind_label(kind),
        occurred_at_ms
    );
    CrossProviderProgramTransitionRecord {
        transition_id: transition_id.clone(),
        participant_id: participant.participant_id.clone(),
        execution_class: participant.execution_class,
        kind,
        occurred_at_ms,
        detail: String::from(detail),
        transition_digest: stable_transition_digest(
            transition_id.as_str(),
            participant.participant_id.as_str(),
            participant.execution_class,
            kind,
            occurred_at_ms,
            detail,
        ),
    }
}

fn selected_sources_for_class(
    admission_plan: &CrossProviderAdmissionPlan,
    execution_class: CrossProviderExecutionClass,
) -> Vec<String> {
    let mut selected = admission_plan
        .candidate_evaluations
        .iter()
        .filter(|evaluation| {
            evaluation.requested_execution_class == execution_class && evaluation.selected
        })
        .collect::<Vec<_>>();
    selected.sort_by_key(|evaluation| {
        (
            evaluation.placement_rank.unwrap_or(u16::MAX),
            evaluation.source_id.clone(),
        )
    });
    selected
        .into_iter()
        .map(|evaluation| evaluation.source_id.clone())
        .collect()
}

fn stable_role_window_digest(window: &CrossProviderProgramRoleWindow) -> String {
    let mut clone = window.clone();
    clone.role_window_digest.clear();
    stable_digest(b"psionic_cross_provider_role_window|", &clone)
}

fn stable_transition_digest(
    transition_id: &str,
    participant_id: &str,
    execution_class: CrossProviderExecutionClass,
    kind: CrossProviderProgramTransitionKind,
    occurred_at_ms: u64,
    detail: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_cross_provider_program_transition|");
    hasher.update(transition_id.as_bytes());
    hasher.update([0]);
    hasher.update(participant_id.as_bytes());
    hasher.update([0]);
    hasher.update(format!("{execution_class:?}").as_bytes());
    hasher.update([0]);
    hasher.update(transition_kind_label(kind).as_bytes());
    hasher.update([0]);
    hasher.update(occurred_at_ms.to_le_bytes());
    hasher.update([0]);
    hasher.update(detail.as_bytes());
    hex::encode(hasher.finalize())
}

fn transition_kind_label(kind: CrossProviderProgramTransitionKind) -> &'static str {
    match kind {
        CrossProviderProgramTransitionKind::Admitted => "admitted",
        CrossProviderProgramTransitionKind::Activated => "activated",
        CrossProviderProgramTransitionKind::HeldStandby => "held_standby",
        CrossProviderProgramTransitionKind::Quarantined => "quarantined",
        CrossProviderProgramTransitionKind::EvidenceLinked => "evidence_linked",
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect("whole-program run graph digest serialization must work"),
    );
    hex::encode(hasher.finalize())
}
