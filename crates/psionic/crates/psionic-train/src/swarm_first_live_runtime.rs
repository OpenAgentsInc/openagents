use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::SigningKey;
use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterTargetFamily,
    LmHeadLoraAdapterArtifact,
};
use psionic_cluster::{
    AdmissionToken, ClusterBackendReadinessStatus, ClusterId, ClusterMembershipRecord,
    ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
    ClusterSnapshot, ClusterStabilityPosture, ClusterState, NodeEpoch, NodeId, NodeRole,
};
use psionic_core::QuantizationMode;
use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};
use psionic_environments::EnvironmentPackageKey;
use safetensors::{serialize, tensor::TensorView, Dtype as SafeTensorsDType, SafeTensors};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_first_swarm_open_adapter_contributor_receipt,
    compare_first_swarm_open_adapter_contributor_receipts, first_swarm_open_adapter_samples,
    first_swarm_open_adapter_sft_request, first_swarm_open_adapter_training_config,
    first_swarm_run_contract, run_open_adapter_sft_export, AdapterArtifactRetentionPolicy,
    AdapterArtifactStorageError, AdapterArtifactStorageState, AdapterAssignmentAckReceipt,
    AdapterAssignmentClaim, AdapterClusterCoordinationError, AdapterClusterMembershipReceipt,
    AdapterClusterWindowPlanReceipt, AdapterContributionArtifactDisposition,
    AdapterContributionExecutionSummary, AdapterContributionProgress,
    AdapterContributionSecurityController, AdapterContributionSecurityError,
    AdapterContributionSecurityPolicy, AdapterContributionSubmissionReceipt,
    AdapterContributionUploadLocator, AdapterContributionValidationBundle,
    AdapterContributionValidatorPolicy, AdapterContributionValidatorState, AdapterPolicyAggregator,
    AdapterPolicyPromotionReceipt, AdapterTrainingClusterCoordinator, AdapterValidationError,
    AdapterWindowContractError, AdapterWindowScoreSummary, AdapterWorkerHeartbeatReceipt,
    AdapterWorkerIdentity, AdapterWorkerProtocolError, AdapterWorkerProtocolPolicy,
    AdapterWorkerProtocolState, AdapterWorkerTrustClass, CheckpointRecoveryError,
    FirstSwarmOpenAdapterAggregationCompatibility, FirstSwarmOpenAdapterContributorReceipt,
    FirstSwarmOpenAdapterReceiptError, FixedBudgetTrainingRun, OpenAdapterExecutionConfig,
    OpenAdapterHiddenStateSample, OpenAdapterPrecisionPolicy, OpenAdapterSftError,
    OpenAdapterTrainingExecutionBackend, OpenAdapterTrainingExecutionError, PortableModelBundle,
    PortableTokenizerAssetFormat, PortableTokenizerBinding, TrainingCoreError, TrainingLoopBudget,
    TrainingParameterGroupState, TrainingRunGraphError, TrainingRunState, TrainingTensorBuffer,
    OPEN_ADAPTER_CUDA_BACKEND_LABEL, OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL,
};

const FIRST_SWARM_TRUSTED_LAN_RUNTIME_REPORT_SCHEMA_VERSION: &str =
    "swarm.first_trusted_lan_runtime_report.v1";
const FIRST_SWARM_TRUSTED_LAN_MERGED_ARTIFACT_REPORT_SCHEMA_VERSION: &str =
    "swarm.first_trusted_lan_merged_artifact_report.v1";
const FIRST_SWARM_TRUSTED_LAN_ENVIRONMENT_REF: &str = "env.swarm.local_open_adapter";
const FIRST_SWARM_TRUSTED_LAN_ENVIRONMENT_VERSION: &str = "2026.03.24";
const FIRST_SWARM_TRUSTED_LAN_POLICY_FAMILY: &str = "swarm.local.open_adapter.policy";
const FIRST_SWARM_CHUNK_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanRuntimeRole {
    Coordinator,
    Contributor,
}

impl FirstSwarmTrustedLanRuntimeRole {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Coordinator => "coordinator",
            Self::Contributor => "contributor",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanLocalContribution {
    pub run_id: String,
    pub assignment_id: String,
    pub worker_id: String,
    pub session_id: String,
    pub role_id: String,
    pub execution_backend_label: String,
    pub adapter_artifact_digest: String,
    pub adapter_identity_digest: String,
    pub adapter_delta_digest: String,
    pub payload_sha256: String,
    pub payload_bytes: usize,
    pub final_mean_loss: f32,
    pub executed_steps: usize,
    pub batch_count: usize,
    pub contributor_receipt: FirstSwarmOpenAdapterContributorReceipt,
    pub execution_summary: AdapterContributionExecutionSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanRetainedArtifacts {
    pub artifact_root: String,
    pub local_contributor_adapter_path: String,
    pub remote_contributor_adapter_path: String,
    pub merged_adapter_path: String,
    pub merged_portable_bundle_path: String,
    pub merged_report_path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanMergedArtifactReport {
    pub schema_version: String,
    pub run_id: String,
    pub aggregation_policy_id: String,
    pub merge_strategy: String,
    pub contributor_weight_bps: BTreeMap<String, u32>,
    pub local_contributor_receipt_digest: String,
    pub remote_contributor_receipt_digest: String,
    pub local_adapter_artifact_digest: String,
    pub remote_adapter_artifact_digest: String,
    pub merged_adapter_artifact_digest: String,
    pub merged_adapter_identity_digest: String,
    pub merged_portable_bundle_artifact_digest: String,
    pub merged_portable_bundle_state_dict_digest: String,
    pub merged_lora_rank: usize,
    pub merged_lora_alpha: f32,
    pub evaluation_backend_label: String,
    pub canonical_profile_mean_loss: f64,
    pub canonical_profile_bits_per_token: f64,
    pub canonical_profile_batch_count: usize,
    pub canonical_profile_sample_count: usize,
    pub deterministic_probe_top_token_id: usize,
    pub claim_boundary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanRuntimeReport {
    pub schema_version: String,
    pub run_id: String,
    pub run_family_id: String,
    pub topology_contract_digest: String,
    pub workflow_plan_digest: String,
    pub cluster_id: String,
    pub node_id: String,
    pub role_id: String,
    pub runtime_role: FirstSwarmTrustedLanRuntimeRole,
    pub execution_backend_label: String,
    pub local_endpoint: String,
    pub peer_endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership_receipt: Option<AdapterClusterMembershipReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_plan: Option<AdapterClusterWindowPlanReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heartbeat_receipts: Vec<AdapterWorkerHeartbeatReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acknowledgement_receipts: Vec<AdapterAssignmentAckReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub submission_receipts: Vec<AdapterContributionSubmissionReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_summary: Option<AdapterWindowScoreSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promotion_receipt: Option<AdapterPolicyPromotionReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregation_compatibility: Option<FirstSwarmOpenAdapterAggregationCompatibility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replay_receipt_digests: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_contribution: Option<FirstSwarmTrustedLanLocalContribution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retained_artifacts: Option<FirstSwarmTrustedLanRetainedArtifacts>,
    pub protocol_detail: String,
    pub claim_boundary: String,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub observed_wallclock_ms: u64,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ContributionEnvelope {
    started_at_ms: u64,
    completed_at_ms: u64,
    local_step_count: u32,
    sample_count: u32,
    average_loss_bps: Option<u32>,
    adapter_delta_digest: String,
    adapter_artifact_digest: String,
    adapter_identity_digest: String,
    final_mean_loss: f32,
    executed_steps: usize,
    batch_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "message_kind", rename_all = "snake_case")]
enum FirstSwarmTrustedLanMessage {
    Hello {
        node_id: String,
        session_id: String,
        signing_public_key_hex: String,
    },
    Assignment {
        assignment: crate::AdapterContributionWorkAssignment,
        claim: AdapterAssignmentClaim,
    },
    Heartbeat {
        active_claim_id: Option<String>,
        progress: Option<AdapterContributionProgress>,
    },
    Contribution {
        claim_id: String,
        assignment_id: String,
        session_id: String,
        contributor_receipt: FirstSwarmOpenAdapterContributorReceipt,
        execution: ContributionEnvelope,
        payload_hex: String,
    },
    CompleteAck {
        submission_receipt_digest: String,
    },
    Error {
        detail: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedFirstSwarmTopologyContract {
    run_family_id: String,
    swarm_contract_digest: String,
    contract_digest: String,
    cluster_namespace: String,
    coordinator_node_id: String,
    contributor_node_ids: Vec<String>,
    nodes: Vec<RetainedFirstSwarmNodeContract>,
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedFirstSwarmNodeContract {
    node_id: String,
    role_id: String,
    backend_label: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedFirstSwarmWorkflowPlan {
    run_family_id: String,
    swarm_contract_digest: String,
    capability_policy: crate::AdapterContributorCapabilityPolicy,
    membership_receipt: AdapterClusterMembershipReceipt,
    window_plan: AdapterClusterWindowPlanReceipt,
    contributor_assignments: Vec<RetainedFirstSwarmContributorAssignment>,
    plan_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedFirstSwarmContributorAssignment {
    contributor_node_id: String,
    matched_backend_label: String,
    dataset_slice: crate::AdapterDatasetSliceIdentity,
}

#[derive(Clone, Debug)]
struct ExecutionNode {
    node_id: String,
    role_id: String,
    backend_label: String,
    dataset_slice: crate::AdapterDatasetSliceIdentity,
}

#[derive(Clone, Debug)]
struct ExecutionContext {
    run_id: String,
    run_family_id: String,
    topology_contract_digest: String,
    workflow_plan_digest: String,
    capability_policy: crate::AdapterContributorCapabilityPolicy,
    input_window_plan: AdapterClusterWindowPlanReceipt,
    local_node: ExecutionNode,
    peer_node: ExecutionNode,
    local_endpoint: SocketAddr,
    peer_endpoint: SocketAddr,
}

struct CoordinatorPlan {
    coordinator: AdapterTrainingClusterCoordinator,
    membership_receipt: AdapterClusterMembershipReceipt,
    window_plan: AdapterClusterWindowPlanReceipt,
    protocol: AdapterWorkerProtocolState,
    local_assignment: crate::AdapterContributionWorkAssignment,
    peer_assignment: crate::AdapterContributionWorkAssignment,
    local_identity: AdapterWorkerIdentity,
    local_claim: AdapterAssignmentClaim,
    expected_peer_signing_public_key_hex: String,
    cluster_id: String,
}

struct LocalContributionRun {
    contribution: FirstSwarmTrustedLanLocalContribution,
    payload: Vec<u8>,
}

#[derive(Clone, Debug)]
struct DecodedLoraAdapter {
    rank: usize,
    alpha: f32,
    hidden_size: usize,
    vocab_size: usize,
    lora_a: Vec<f32>,
    lora_b: Vec<f32>,
}

#[derive(Debug, Error)]
pub enum FirstSwarmTrustedLanRuntimeError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    Cluster(#[from] AdapterClusterCoordinationError),
    #[error(transparent)]
    WindowContract(#[from] AdapterWindowContractError),
    #[error(transparent)]
    Checkpoint(#[from] CheckpointRecoveryError),
    #[error(transparent)]
    WorkerProtocol(#[from] AdapterWorkerProtocolError),
    #[error(transparent)]
    ArtifactStorage(#[from] AdapterArtifactStorageError),
    #[error(transparent)]
    Security(#[from] AdapterContributionSecurityError),
    #[error(transparent)]
    Validation(#[from] AdapterValidationError),
    #[error(transparent)]
    Aggregation(#[from] crate::AdapterAggregationError),
    #[error(transparent)]
    OpenAdapter(#[from] OpenAdapterTrainingExecutionError),
    #[error(transparent)]
    OpenAdapterSft(#[from] OpenAdapterSftError),
    #[error(transparent)]
    ModelIo(#[from] crate::ModelIoError),
    #[error(transparent)]
    OpenAdapterFixture(#[from] FirstSwarmOpenAdapterReceiptError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    #[error("first swarm trusted-LAN runtime refused the inputs: {detail}")]
    InvalidInput { detail: String },
    #[error("first swarm trusted-LAN runtime protocol failure: {detail}")]
    Protocol { detail: String },
    #[error("first swarm trusted-LAN runtime timed out: {detail}")]
    Timeout { detail: String },
}

pub fn run_first_swarm_trusted_lan_runtime(
    role: FirstSwarmTrustedLanRuntimeRole,
    run_id: impl Into<String>,
    topology_contract_path: impl AsRef<Path>,
    workflow_plan_path: impl AsRef<Path>,
    local_endpoint: &str,
    peer_endpoint: &str,
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmTrustedLanRuntimeReport, FirstSwarmTrustedLanRuntimeError> {
    let run_id = run_id.into();
    let output_path = output_path.as_ref();
    let started_at_ms = now_ms();
    let started = Instant::now();
    let context = load_execution_context(
        role,
        run_id,
        topology_contract_path.as_ref(),
        workflow_plan_path.as_ref(),
        local_endpoint,
        peer_endpoint,
    )?;
    let mut report = match role {
        FirstSwarmTrustedLanRuntimeRole::Coordinator => run_coordinator(
            context,
            started_at_ms,
            output_path
                .parent()
                .map(|parent| parent.join("retained_artifacts")),
        )?,
        FirstSwarmTrustedLanRuntimeRole::Contributor => run_contributor(context, started_at_ms)?,
    };
    report.finished_at_ms = now_ms();
    report.observed_wallclock_ms = (started.elapsed().as_millis() as u64).max(1);
    report.report_digest =
        stable_digest(b"psionic_first_swarm_trusted_lan_runtime_report|", &report);
    write_runtime_report(output_path, &report)?;
    Ok(report)
}

fn run_coordinator(
    context: ExecutionContext,
    started_at_ms: u64,
    artifact_root: Option<PathBuf>,
) -> Result<FirstSwarmTrustedLanRuntimeReport, FirstSwarmTrustedLanRuntimeError> {
    let mut plan = build_coordinator_plan(&context, started_at_ms)?;
    let local_claim_id = plan.local_claim.claim_id.clone();
    let local_assignment_id = plan.local_assignment.assignment_id.clone();
    let local_node = context.local_node.clone();
    let local_identity = plan.local_identity.clone();
    let local_run_id = context.run_id.clone();

    let listener = TcpListener::bind(context.local_endpoint).map_err(|error| {
        FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!(
                "failed to bind coordinator listener on {}: {error}",
                context.local_endpoint
            ),
        }
    })?;
    listener.set_nonblocking(false).map_err(|error| {
        FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("failed to configure coordinator listener: {error}"),
        }
    })?;

    let local_execution_handle = thread::spawn(move || {
        run_local_contribution(
            local_run_id.as_str(),
            local_claim_id.as_str(),
            local_assignment_id.as_str(),
            local_identity.session_id.as_str(),
            &local_node,
        )
    });

    let (mut stream, _) =
        listener
            .accept()
            .map_err(|error| FirstSwarmTrustedLanRuntimeError::Timeout {
                detail: format!("coordinator failed to accept contributor connection: {error}"),
            })?;
    configure_stream(&stream)?;

    let hello = receive_message(&mut stream)?;
    let (peer_node_id, peer_session_id, peer_public_key_hex) = match hello {
        FirstSwarmTrustedLanMessage::Hello {
            node_id,
            session_id,
            signing_public_key_hex,
        } => (node_id, session_id, signing_public_key_hex),
        other => {
            return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
                detail: format!("expected contributor hello, found {other:?}"),
            });
        }
    };
    if peer_node_id != context.peer_node.node_id {
        return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!(
                "coordinator expected peer node `{}` but connected node reported `{peer_node_id}`",
                context.peer_node.node_id
            ),
        });
    }
    if peer_public_key_hex != plan.expected_peer_signing_public_key_hex {
        return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: String::from(
                "contributor signing public key did not match the deterministic trusted-LAN key",
            ),
        });
    }
    let peer_identity = AdapterWorkerIdentity::new(
        peer_node_id.clone(),
        peer_session_id.clone(),
        AdapterWorkerTrustClass::SemiTrustedContributor,
        format!("swarm-trusted-lan:{peer_node_id}"),
    )
    .with_submission_signing_public_key_hex(peer_public_key_hex.clone());
    plan.protocol
        .record_heartbeat(peer_identity.clone(), None, None, now_ms())?;
    let peer_claim = plan.protocol.claim_assignment(
        context.peer_node.node_id.as_str(),
        plan.peer_assignment.assignment_id.as_str(),
        now_ms(),
    )?;
    plan.protocol.acknowledge_assignment(
        context.peer_node.node_id.as_str(),
        peer_session_id.as_str(),
        peer_claim.claim_id.as_str(),
        now_ms(),
    )?;
    send_message(
        &mut stream,
        &FirstSwarmTrustedLanMessage::Assignment {
            assignment: plan.peer_assignment.clone(),
            claim: peer_claim.clone(),
        },
    )?;

    let mut remote_execution = None;
    let mut remote_payload = Vec::new();
    let mut remote_contributor_receipt = None;
    while remote_execution.is_none() {
        match receive_message(&mut stream)? {
            FirstSwarmTrustedLanMessage::Heartbeat {
                active_claim_id,
                progress,
            } => {
                let active_claim_id = active_claim_id.as_deref();
                plan.protocol.record_heartbeat(
                    peer_identity.clone(),
                    active_claim_id,
                    progress,
                    now_ms(),
                )?;
            }
            FirstSwarmTrustedLanMessage::Contribution {
                claim_id,
                assignment_id,
                session_id,
                contributor_receipt,
                execution,
                payload_hex,
            } => {
                if claim_id != peer_claim.claim_id
                    || assignment_id != plan.peer_assignment.assignment_id
                    || session_id != peer_session_id
                {
                    return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
                        detail: String::from(
                            "contributor submitted a contribution for the wrong claim, assignment, or session",
                        ),
                    });
                }
                remote_contributor_receipt = Some(contributor_receipt);
                remote_execution = Some(execution);
                remote_payload = hex::decode(payload_hex).map_err(|error| {
                    FirstSwarmTrustedLanRuntimeError::Protocol {
                        detail: format!("failed to decode contributor payload hex: {error}"),
                    }
                })?;
            }
            other => {
                return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
                    detail: format!(
                        "coordinator expected heartbeats or contribution, found {other:?}"
                    ),
                });
            }
        }
    }

    let local_run = local_execution_handle.join().map_err(|_| {
        FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: String::from("coordinator local contribution thread panicked"),
        }
    })??;
    plan.protocol.record_heartbeat(
        plan.local_identity.clone(),
        Some(plan.local_claim.claim_id.as_str()),
        Some(AdapterContributionProgress {
            completed_steps: local_run.contribution.executed_steps as u32,
            processed_samples: local_run.contribution.execution_summary.sample_count,
        }),
        now_ms(),
    )?;

    let remote_execution = remote_execution.expect("remote execution populated");
    let remote_contributor_receipt =
        remote_contributor_receipt.expect("remote contributor receipt populated");
    let compatibility = compare_first_swarm_open_adapter_contributor_receipts(&[
        local_run.contribution.contributor_receipt.clone(),
        remote_contributor_receipt.clone(),
    ])?;

    let remote_upload = upload_locator_for_assignment(
        &plan.peer_assignment,
        remote_payload.as_slice(),
        "linux-contributor",
    )?;
    let remote_submission = plan.protocol.submit_contribution(
        peer_claim.claim_id.as_str(),
        plan.peer_assignment.worker_id.as_str(),
        peer_session_id.as_str(),
        plan.peer_assignment
            .source_policy_revision
            .revision_id
            .as_str(),
        plan.peer_assignment
            .source_checkpoint_pointer
            .pointer_digest
            .as_str(),
        AdapterContributionExecutionSummary::new(
            remote_execution.started_at_ms,
            remote_execution.completed_at_ms,
            remote_execution.local_step_count,
            remote_execution.sample_count,
            remote_execution.average_loss_bps,
            remote_execution.adapter_delta_digest.clone(),
        )?,
        remote_upload.clone(),
        now_ms(),
    )?;
    send_message(
        &mut stream,
        &FirstSwarmTrustedLanMessage::CompleteAck {
            submission_receipt_digest: remote_submission.receipt_digest.clone(),
        },
    )?;

    plan.protocol.record_heartbeat(
        plan.local_identity.clone(),
        Some(plan.local_claim.claim_id.as_str()),
        Some(AdapterContributionProgress {
            completed_steps: local_run.contribution.executed_steps as u32,
            processed_samples: local_run.contribution.execution_summary.sample_count,
        }),
        now_ms(),
    )?;

    let local_upload = upload_locator_for_assignment(
        &plan.local_assignment,
        local_run.payload.as_slice(),
        "mac-coordinator",
    )?;
    let local_submission = plan.protocol.submit_contribution(
        plan.local_claim.claim_id.as_str(),
        plan.local_assignment.worker_id.as_str(),
        plan.local_identity.session_id.as_str(),
        plan.local_assignment
            .source_policy_revision
            .revision_id
            .as_str(),
        plan.local_assignment
            .source_checkpoint_pointer
            .pointer_digest
            .as_str(),
        local_run.contribution.execution_summary.clone(),
        local_upload.clone(),
        now_ms(),
    )?;

    let mut storage = AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
    let mut security =
        AdapterContributionSecurityController::new(AdapterContributionSecurityPolicy::default());
    let swarm_contract = first_swarm_run_contract();
    let mut validator =
        AdapterContributionValidatorState::new(AdapterContributionValidatorPolicy {
            validator_policy_id: swarm_contract.governance.validator_policy_id,
            replay_sample_bps: 10_000,
            ..AdapterContributionValidatorPolicy::default()
        });
    let mut aggregator = AdapterPolicyAggregator::new(Default::default());

    let local_bundle = materialize_validation_bundle(
        &mut storage,
        &mut security,
        &plan.protocol,
        &plan.local_assignment,
        &plan.local_claim,
        &plan.local_identity,
        &local_submission,
        local_run.payload.as_slice(),
        now_ms(),
    )?;
    let remote_bundle = materialize_validation_bundle(
        &mut storage,
        &mut security,
        &plan.protocol,
        &plan.peer_assignment,
        &peer_claim,
        &peer_identity,
        &remote_submission,
        remote_payload.as_slice(),
        now_ms() + 50,
    )?;
    let replay_receipt_digests = vec![
        local_bundle
            .replay
            .as_ref()
            .map(|receipt| receipt.receipt_digest.clone())
            .unwrap_or_default(),
        remote_bundle
            .replay
            .as_ref()
            .map(|receipt| receipt.receipt_digest.clone())
            .unwrap_or_default(),
    ];
    let bundles = vec![local_bundle, remote_bundle];
    let scored_at_ms = now_ms() + 100;
    let summary = validator.validate_window(
        &mut plan.protocol.window,
        bundles.clone(),
        None,
        scored_at_ms,
    )?;
    *plan.coordinator.current_window_mut()? = plan.protocol.window.clone();
    let promotion = aggregator.promote_current_window(
        &mut plan.coordinator,
        &summary,
        bundles,
        scored_at_ms + 50,
        scored_at_ms + 60,
    )?;
    let retained_artifacts = if let Some(artifact_root) = artifact_root {
        Some(retain_coordinator_artifacts(
            artifact_root.as_path(),
            context.run_id.as_str(),
            &context.local_node,
            &local_run,
            &context.peer_node,
            &remote_contributor_receipt,
            remote_payload.as_slice(),
        )?)
    } else {
        None
    };

    Ok(FirstSwarmTrustedLanRuntimeReport {
        schema_version: String::from(FIRST_SWARM_TRUSTED_LAN_RUNTIME_REPORT_SCHEMA_VERSION),
        run_id: context.run_id,
        run_family_id: context.run_family_id,
        topology_contract_digest: context.topology_contract_digest,
        workflow_plan_digest: context.workflow_plan_digest,
        cluster_id: plan.cluster_id,
        node_id: context.local_node.node_id,
        role_id: context.local_node.role_id,
        runtime_role: FirstSwarmTrustedLanRuntimeRole::Coordinator,
        execution_backend_label: context.local_node.backend_label,
        local_endpoint: context.local_endpoint.to_string(),
        peer_endpoint: context.peer_endpoint.to_string(),
        membership_receipt: Some(plan.membership_receipt),
        window_plan: Some(plan.window_plan),
        heartbeat_receipts: plan.protocol.heartbeat_receipts.clone(),
        acknowledgement_receipts: plan.protocol.acknowledgement_receipts.clone(),
        submission_receipts: plan.protocol.submission_receipts.clone(),
        validator_summary: Some(summary),
        promotion_receipt: Some(promotion),
        aggregation_compatibility: Some(compatibility),
        replay_receipt_digests,
        local_contribution: Some(local_run.contribution),
        retained_artifacts,
        protocol_detail: String::from(
            "The Mac coordinator ran one MLX open-adapter contribution locally, admitted one Linux CUDA contributor over the trusted-LAN cluster port, compared the retained contributor receipts across both backends, and sealed validator, replay, and aggregation truth through the generic adapter-cluster state machines.",
        ),
        claim_boundary: String::from(
            "This runtime proves one bounded trusted-LAN mixed-hardware open-adapter window across one Mac MLX coordinator and one Linux RTX 4080 CUDA contributor. It does not claim full-model mixed-backend dense training, internet discovery, elastic membership, or automatic served promotion.",
        ),
        started_at_ms,
        finished_at_ms: started_at_ms,
        observed_wallclock_ms: 0,
        report_digest: String::new(),
    })
}

fn run_contributor(
    context: ExecutionContext,
    started_at_ms: u64,
) -> Result<FirstSwarmTrustedLanRuntimeReport, FirstSwarmTrustedLanRuntimeError> {
    const PEER_CONNECT_TIMEOUT_SECONDS: u64 = 600;

    let session_id = format!("{}-session-{}", context.local_node.node_id, context.run_id);
    let signing_key =
        signing_key_for_worker(context.run_id.as_str(), context.local_node.node_id.as_str());
    let signing_public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());

    let mut stream = connect_with_retry(
        context.peer_endpoint.to_string().as_str(),
        PEER_CONNECT_TIMEOUT_SECONDS,
    )?;
    configure_stream(&stream)?;
    send_message(
        &mut stream,
        &FirstSwarmTrustedLanMessage::Hello {
            node_id: context.local_node.node_id.clone(),
            session_id: session_id.clone(),
            signing_public_key_hex: signing_public_key_hex.clone(),
        },
    )?;

    let assignment_message = receive_message(&mut stream)?;
    let (assignment, claim) = match assignment_message {
        FirstSwarmTrustedLanMessage::Assignment { assignment, claim } => (assignment, claim),
        other => {
            return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
                detail: format!("contributor expected assignment, found {other:?}"),
            });
        }
    };
    if assignment.worker_id != context.local_node.node_id
        || claim.worker_id != context.local_node.node_id
    {
        return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: String::from(
                "coordinator assigned the contributor command to the wrong worker identifier",
            ),
        });
    }

    let local_run = run_local_contribution(
        context.run_id.as_str(),
        claim.claim_id.as_str(),
        assignment.assignment_id.as_str(),
        session_id.as_str(),
        &context.local_node,
    )?;
    send_message(
        &mut stream,
        &FirstSwarmTrustedLanMessage::Heartbeat {
            active_claim_id: Some(claim.claim_id.clone()),
            progress: Some(AdapterContributionProgress {
                completed_steps: 4,
                processed_samples: 4,
            }),
        },
    )?;
    thread::sleep(Duration::from_millis(400));
    send_message(
        &mut stream,
        &FirstSwarmTrustedLanMessage::Heartbeat {
            active_claim_id: Some(claim.claim_id.clone()),
            progress: Some(AdapterContributionProgress {
                completed_steps: 8,
                processed_samples: 8,
            }),
        },
    )?;
    thread::sleep(Duration::from_millis(400));

    let execution = ContributionEnvelope {
        started_at_ms: local_run.contribution.execution_summary.started_at_ms,
        completed_at_ms: local_run.contribution.execution_summary.completed_at_ms,
        local_step_count: local_run.contribution.execution_summary.local_step_count,
        sample_count: local_run.contribution.execution_summary.sample_count,
        average_loss_bps: local_run.contribution.execution_summary.average_loss_bps,
        adapter_delta_digest: local_run
            .contribution
            .execution_summary
            .adapter_delta_digest
            .clone(),
        adapter_artifact_digest: local_run.contribution.adapter_artifact_digest.clone(),
        adapter_identity_digest: local_run.contribution.adapter_identity_digest.clone(),
        final_mean_loss: local_run.contribution.final_mean_loss,
        executed_steps: local_run.contribution.executed_steps,
        batch_count: local_run.contribution.batch_count,
    };
    send_message(
        &mut stream,
        &FirstSwarmTrustedLanMessage::Contribution {
            claim_id: claim.claim_id.clone(),
            assignment_id: assignment.assignment_id.clone(),
            session_id: session_id.clone(),
            contributor_receipt: local_run.contribution.contributor_receipt.clone(),
            execution,
            payload_hex: hex::encode(local_run.payload.as_slice()),
        },
    )?;
    match receive_message(&mut stream)? {
        FirstSwarmTrustedLanMessage::CompleteAck { .. } => {}
        other => {
            return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
                detail: format!("contributor expected completion ack, found {other:?}"),
            });
        }
    }

    let cluster_id = build_cluster_id(
        context.run_id.as_str(),
        context.topology_contract_digest.as_str(),
    )
    .as_str()
    .to_string();

    Ok(FirstSwarmTrustedLanRuntimeReport {
        schema_version: String::from(FIRST_SWARM_TRUSTED_LAN_RUNTIME_REPORT_SCHEMA_VERSION),
        run_id: context.run_id,
        run_family_id: context.run_family_id,
        topology_contract_digest: context.topology_contract_digest,
        workflow_plan_digest: context.workflow_plan_digest,
        cluster_id,
        node_id: context.local_node.node_id,
        role_id: context.local_node.role_id,
        runtime_role: FirstSwarmTrustedLanRuntimeRole::Contributor,
        execution_backend_label: context.local_node.backend_label,
        local_endpoint: context.local_endpoint.to_string(),
        peer_endpoint: context.peer_endpoint.to_string(),
        membership_receipt: None,
        window_plan: None,
        heartbeat_receipts: Vec::new(),
        acknowledgement_receipts: Vec::new(),
        submission_receipts: Vec::new(),
        validator_summary: None,
        promotion_receipt: None,
        aggregation_compatibility: None,
        replay_receipt_digests: Vec::new(),
        local_contribution: Some(local_run.contribution),
        retained_artifacts: None,
        protocol_detail: String::from(
            "The Linux contributor dialed the configured Mac coordinator endpoint over the trusted LAN, accepted one bounded open-adapter assignment, emitted worker heartbeats, and returned one CUDA contributor receipt plus adapter payload.",
        ),
        claim_boundary: String::from(
            "This runtime proves one bounded Linux RTX 4080 contributor path for the first trusted-LAN swarm lane. It does not claim validator, aggregation, or publication authority on the contributor node.",
        ),
        started_at_ms,
        finished_at_ms: started_at_ms,
        observed_wallclock_ms: 0,
        report_digest: String::new(),
    })
}

fn load_execution_context(
    role: FirstSwarmTrustedLanRuntimeRole,
    run_id: String,
    topology_contract_path: &Path,
    workflow_plan_path: &Path,
    local_endpoint: &str,
    peer_endpoint: &str,
) -> Result<ExecutionContext, FirstSwarmTrustedLanRuntimeError> {
    let swarm_contract = first_swarm_run_contract();
    let topology: RetainedFirstSwarmTopologyContract = read_json(topology_contract_path)?;
    let workflow_plan: RetainedFirstSwarmWorkflowPlan = read_json(workflow_plan_path)?;
    if topology.run_family_id != swarm_contract.run_family_id
        || workflow_plan.run_family_id != swarm_contract.run_family_id
    {
        return Err(FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: String::from(
                "trusted-LAN topology or workflow plan run family drifted from the swarm contract",
            ),
        });
    }
    if topology.swarm_contract_digest != swarm_contract.contract_digest
        || workflow_plan.swarm_contract_digest != swarm_contract.contract_digest
    {
        return Err(FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: String::from(
                "trusted-LAN topology or workflow plan swarm digest drifted from the swarm contract",
            ),
        });
    }
    if workflow_plan.membership_receipt.receipt_digest.is_empty() {
        return Err(FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: String::from("workflow plan membership receipt digest must be present"),
        });
    }

    let coordinator_node = topology
        .nodes
        .iter()
        .find(|node| node.node_id == topology.coordinator_node_id)
        .ok_or_else(|| FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: String::from("topology contract is missing the coordinator node"),
        })?;
    let contributor_node_id = topology
        .contributor_node_ids
        .iter()
        .find(|node_id| *node_id != &topology.coordinator_node_id)
        .cloned()
        .ok_or_else(|| FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: String::from("topology contract is missing the contributor node"),
        })?;
    let contributor_node = topology
        .nodes
        .iter()
        .find(|node| node.node_id == contributor_node_id)
        .ok_or_else(|| FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: String::from("topology contract contributor node id does not resolve"),
        })?;

    let (local_node_ref, peer_node_ref) = match role {
        FirstSwarmTrustedLanRuntimeRole::Coordinator => (coordinator_node, contributor_node),
        FirstSwarmTrustedLanRuntimeRole::Contributor => (contributor_node, coordinator_node),
    };
    let local_assignment = workflow_plan
        .contributor_assignments
        .iter()
        .find(|assignment| assignment.contributor_node_id == local_node_ref.node_id)
        .ok_or_else(|| FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: format!(
                "workflow plan is missing a contributor assignment for node `{}`",
                local_node_ref.node_id
            ),
        })?;
    let peer_assignment = workflow_plan
        .contributor_assignments
        .iter()
        .find(|assignment| assignment.contributor_node_id == peer_node_ref.node_id)
        .ok_or_else(|| FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: format!(
                "workflow plan is missing a contributor assignment for node `{}`",
                peer_node_ref.node_id
            ),
        })?;
    if local_node_ref.backend_label != local_assignment.matched_backend_label
        || peer_node_ref.backend_label != peer_assignment.matched_backend_label
    {
        return Err(FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: String::from(
                "topology backend labels drifted from workflow-plan contributor assignments",
            ),
        });
    }

    Ok(ExecutionContext {
        run_id,
        run_family_id: topology.run_family_id,
        topology_contract_digest: topology.contract_digest,
        workflow_plan_digest: workflow_plan.plan_digest,
        capability_policy: workflow_plan.capability_policy,
        input_window_plan: workflow_plan.window_plan,
        local_node: ExecutionNode {
            node_id: local_node_ref.node_id.clone(),
            role_id: local_node_ref.role_id.clone(),
            backend_label: local_node_ref.backend_label.clone(),
            dataset_slice: local_assignment.dataset_slice.clone(),
        },
        peer_node: ExecutionNode {
            node_id: peer_node_ref.node_id.clone(),
            role_id: peer_node_ref.role_id.clone(),
            backend_label: peer_node_ref.backend_label.clone(),
            dataset_slice: peer_assignment.dataset_slice.clone(),
        },
        local_endpoint: parse_socket_addr(local_endpoint)?,
        peer_endpoint: parse_socket_addr(peer_endpoint)?,
    })
}

fn build_coordinator_plan(
    context: &ExecutionContext,
    started_at_ms: u64,
) -> Result<CoordinatorPlan, FirstSwarmTrustedLanRuntimeError> {
    let cluster_id = build_cluster_id(
        context.run_id.as_str(),
        context.topology_contract_digest.as_str(),
    );
    let state = cluster_state_from_context(context, &cluster_id);
    let run = TrainingRunState::new(
        context.run_id.clone(),
        context.input_window_plan.stage_id.clone(),
        cluster_id.as_str(),
        context
            .input_window_plan
            .input_checkpoint_pointer
            .checkpoint_family
            .clone(),
        EnvironmentPackageKey::new(
            FIRST_SWARM_TRUSTED_LAN_ENVIRONMENT_REF,
            FIRST_SWARM_TRUSTED_LAN_ENVIRONMENT_VERSION,
        ),
    )?;
    let mut coordinator = AdapterTrainingClusterCoordinator::new(
        run,
        context.input_window_plan.adapter_target.clone(),
        context.input_window_plan.input_policy_revision.clone(),
        context.input_window_plan.input_checkpoint_pointer.clone(),
        context.capability_policy.clone(),
    );
    let membership_receipt = coordinator
        .observe_cluster_state(&state, started_at_ms + 10)?
        .clone();

    let dataset_slices = context
        .input_window_plan
        .selected_node_ids
        .iter()
        .map(|node_id| {
            if node_id == &context.local_node.node_id {
                Ok(context.local_node.dataset_slice.clone())
            } else if node_id == &context.peer_node.node_id {
                Ok(context.peer_node.dataset_slice.clone())
            } else {
                Err(FirstSwarmTrustedLanRuntimeError::InvalidInput {
                    detail: format!("workflow plan selected unexpected node `{node_id}`"),
                })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let window_record = coordinator.plan_next_window_with_selected_nodes(
        dataset_slices,
        context.input_window_plan.selected_node_ids.clone(),
        started_at_ms + 20,
    )?;
    let mut protocol = AdapterWorkerProtocolState::from_window_record(
        &window_record,
        AdapterWorkerProtocolPolicy {
            heartbeat_timeout_ms: 1_800_000,
            claim_ttl_ms: 1_800_000,
        },
    );
    protocol.activate_window()?;
    let local_assignment = protocol
        .assignments
        .iter()
        .find(|assignment| assignment.worker_id == context.local_node.node_id)
        .cloned()
        .ok_or_else(|| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: String::from("worker protocol did not assign the coordinator node"),
        })?;
    let peer_assignment = protocol
        .assignments
        .iter()
        .find(|assignment| assignment.worker_id == context.peer_node.node_id)
        .cloned()
        .ok_or_else(|| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: String::from("worker protocol did not assign the contributor node"),
        })?;
    let local_identity =
        build_identity(context.run_id.as_str(), context.local_node.node_id.as_str());
    protocol.record_heartbeat(local_identity.clone(), None, None, started_at_ms + 30)?;
    let local_claim = protocol.claim_assignment(
        context.local_node.node_id.as_str(),
        local_assignment.assignment_id.as_str(),
        started_at_ms + 40,
    )?;
    protocol.acknowledge_assignment(
        context.local_node.node_id.as_str(),
        local_identity.session_id.as_str(),
        local_claim.claim_id.as_str(),
        started_at_ms + 41,
    )?;
    let expected_peer_signing_public_key_hex =
        build_identity(context.run_id.as_str(), context.peer_node.node_id.as_str())
            .submission_signing_public_key_hex;

    Ok(CoordinatorPlan {
        coordinator,
        membership_receipt,
        window_plan: window_record.plan.clone(),
        protocol,
        local_assignment,
        peer_assignment,
        local_identity,
        local_claim,
        expected_peer_signing_public_key_hex,
        cluster_id: cluster_id.as_str().to_string(),
    })
}

fn run_local_contribution(
    run_id: &str,
    _claim_id: &str,
    assignment_id: &str,
    session_id: &str,
    node: &ExecutionNode,
) -> Result<LocalContributionRun, FirstSwarmTrustedLanRuntimeError> {
    let started_at_ms = now_ms();
    let config = first_swarm_open_adapter_training_config(
        format!("{run_id}-{}", node.node_id),
        format!("{FIRST_SWARM_TRUSTED_LAN_POLICY_FAMILY}:{run_id}"),
        node.backend_label.clone(),
    );
    let samples = first_swarm_samples_for_backend(node.backend_label.as_str())?;
    let backend = OpenAdapterTrainingExecutionBackend::new(config, samples)?;
    let outcome = run_open_adapter_sft_export(
        &backend,
        &first_swarm_open_adapter_sft_request(node.node_id.clone(), "r1", started_at_ms, 25),
    )?;
    let unsupported_precision_refusal = unsupported_precision_refusal(&backend)?;
    let adapter = outcome.load_lm_head_lora_artifact()?;
    let mut logits = vec![0.0_f32; backend.config().model.vocab_size];
    adapter
        .apply_to_logits(&[1.0, 0.0, 0.0, 0.0], logits.as_mut_slice())
        .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("failed to compute deterministic probe logits: {error}"),
        })?;
    let probe_top_token_id = logits
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.partial_cmp(right.1).expect("finite logits"))
        .map(|(index, _)| index)
        .unwrap_or_default();
    let final_mean_loss = outcome
        .gradient_records
        .last()
        .map(|record| record.mean_loss)
        .unwrap_or_default();
    let average_loss_bps = Some((final_mean_loss * 10_000.0).round() as u32);
    let contributor_receipt = build_first_swarm_open_adapter_contributor_receipt(
        node.role_id.as_str(),
        &backend,
        &outcome,
        probe_top_token_id,
        unsupported_precision_refusal.clone(),
    )?;
    let execution_summary = AdapterContributionExecutionSummary::new(
        started_at_ms,
        now_ms(),
        outcome.step_receipts.len() as u32,
        outcome.gradient_records.len() as u32,
        average_loss_bps,
        outcome.summary.adapter_artifact_digest.clone(),
    )?;
    let payload_sha256 = hex::encode(Sha256::digest(outcome.adapter_bytes.as_slice()));
    Ok(LocalContributionRun {
        contribution: FirstSwarmTrustedLanLocalContribution {
            run_id: String::from(run_id),
            assignment_id: String::from(assignment_id),
            worker_id: node.node_id.clone(),
            session_id: String::from(session_id),
            role_id: node.role_id.clone(),
            execution_backend_label: node.backend_label.clone(),
            adapter_artifact_digest: outcome.summary.adapter_artifact_digest.clone(),
            adapter_identity_digest: outcome.summary.adapter_identity_digest.clone(),
            adapter_delta_digest: outcome.summary.adapter_artifact_digest.clone(),
            payload_sha256,
            payload_bytes: outcome.adapter_bytes.len(),
            final_mean_loss,
            executed_steps: outcome.step_receipts.len(),
            batch_count: backend.batches().len(),
            contributor_receipt,
            execution_summary,
        },
        payload: outcome.adapter_bytes,
    })
}

fn retain_coordinator_artifacts(
    artifact_root: &Path,
    run_id: &str,
    local_node: &ExecutionNode,
    local_run: &LocalContributionRun,
    remote_node: &ExecutionNode,
    remote_receipt: &FirstSwarmOpenAdapterContributorReceipt,
    remote_payload: &[u8],
) -> Result<FirstSwarmTrustedLanRetainedArtifacts, FirstSwarmTrustedLanRuntimeError> {
    fs::create_dir_all(artifact_root).map_err(|error| {
        FirstSwarmTrustedLanRuntimeError::CreateDir {
            path: artifact_root.display().to_string(),
            error,
        }
    })?;

    let local_contributor_adapter_path =
        artifact_root.join(format!("{}_adapter.safetensors", local_node.node_id));
    let remote_contributor_adapter_path =
        artifact_root.join(format!("{}_adapter.safetensors", remote_node.node_id));
    let merged_adapter_path = artifact_root.join("merged_mean_delta_adapter.safetensors");
    let merged_portable_bundle_path = artifact_root.join("merged_portable_bundle.safetensors");
    let merged_report_path = artifact_root.join("merged_artifact_report.json");

    write_bytes(
        local_contributor_adapter_path.as_path(),
        local_run.payload.as_slice(),
    )?;
    write_bytes(remote_contributor_adapter_path.as_path(), remote_payload)?;

    build_and_write_merged_artifacts(
        run_id,
        local_node,
        &local_run.contribution.contributor_receipt,
        local_run.payload.as_slice(),
        remote_node,
        remote_receipt,
        remote_payload,
        merged_adapter_path.as_path(),
        merged_portable_bundle_path.as_path(),
        merged_report_path.as_path(),
    )?;
    Ok(FirstSwarmTrustedLanRetainedArtifacts {
        artifact_root: artifact_root.display().to_string(),
        local_contributor_adapter_path: local_contributor_adapter_path.display().to_string(),
        remote_contributor_adapter_path: remote_contributor_adapter_path.display().to_string(),
        merged_adapter_path: merged_adapter_path.display().to_string(),
        merged_portable_bundle_path: merged_portable_bundle_path.display().to_string(),
        merged_report_path: merged_report_path.display().to_string(),
    })
}

#[allow(clippy::too_many_arguments)]
fn build_and_write_merged_artifacts(
    run_id: &str,
    local_node: &ExecutionNode,
    local_receipt: &FirstSwarmOpenAdapterContributorReceipt,
    local_payload: &[u8],
    remote_node: &ExecutionNode,
    remote_receipt: &FirstSwarmOpenAdapterContributorReceipt,
    remote_payload: &[u8],
    merged_adapter_path: &Path,
    merged_portable_bundle_path: &Path,
    merged_report_path: &Path,
) -> Result<FirstSwarmTrustedLanMergedArtifactReport, FirstSwarmTrustedLanRuntimeError> {
    let local_adapter = decode_lora_adapter(local_payload, local_receipt.manifest.lora_alpha)?;
    let remote_adapter = decode_lora_adapter(remote_payload, remote_receipt.manifest.lora_alpha)?;
    if local_adapter.hidden_size != remote_adapter.hidden_size
        || local_adapter.vocab_size != remote_adapter.vocab_size
    {
        return Err(FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: String::from(
                "first swarm merged artifact export requires matching hidden and vocabulary shapes",
            ),
        });
    }

    let local_weight = 0.5_f32;
    let remote_weight = 0.5_f32;
    let local_scale = (local_weight * (local_adapter.alpha / local_adapter.rank as f32)).sqrt();
    let remote_scale = (remote_weight * (remote_adapter.alpha / remote_adapter.rank as f32)).sqrt();
    let merged_rank = local_adapter.rank + remote_adapter.rank;
    let merged_alpha = merged_rank as f32;

    let mut merged_lora_a =
        Vec::with_capacity(merged_rank.saturating_mul(local_adapter.hidden_size));
    merged_lora_a.extend(local_adapter.lora_a.iter().map(|value| value * local_scale));
    merged_lora_a.extend(
        remote_adapter
            .lora_a
            .iter()
            .map(|value| value * remote_scale),
    );

    let mut merged_lora_b =
        Vec::with_capacity(local_adapter.vocab_size.saturating_mul(merged_rank));
    for vocab_index in 0..local_adapter.vocab_size {
        let local_row_start = vocab_index * local_adapter.rank;
        let remote_row_start = vocab_index * remote_adapter.rank;
        merged_lora_b.extend(
            local_adapter.lora_b[local_row_start..local_row_start + local_adapter.rank]
                .iter()
                .map(|value| value * local_scale),
        );
        merged_lora_b.extend(
            remote_adapter.lora_b[remote_row_start..remote_row_start + remote_adapter.rank]
                .iter()
                .map(|value| value * remote_scale),
        );
    }

    let merged_adapter_bytes = serialize_lora_adapter(
        merged_rank,
        local_adapter.hidden_size,
        local_adapter.vocab_size,
        merged_lora_a.as_slice(),
        merged_lora_b.as_slice(),
    )?;
    let merged_adapter_artifact_digest =
        hex::encode(Sha256::digest(merged_adapter_bytes.as_slice()));
    write_bytes(merged_adapter_path, merged_adapter_bytes.as_slice())?;

    let merged_identity = merged_adapter_identity(
        run_id,
        local_receipt,
        merged_adapter_artifact_digest.as_str(),
        merged_rank,
    );
    let merged_artifact = LmHeadLoraAdapterArtifact::from_safetensors_bytes(
        merged_adapter_bytes.as_slice(),
        merged_identity.clone(),
        merged_alpha,
    )
    .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
        detail: format!("failed to decode exact mean-delta merged adapter artifact: {error}"),
    })?;
    let deterministic_probe_top_token_id =
        deterministic_probe_top_token_id(&merged_artifact, local_adapter.vocab_size)?;

    let mut config = first_swarm_open_adapter_training_config(
        format!("{run_id}-merged"),
        format!("{FIRST_SWARM_TRUSTED_LAN_POLICY_FAMILY}:{run_id}:merged"),
        local_node.backend_label.clone(),
    );
    config.model.target.lora_rank = merged_rank;
    config.model.target.lora_alpha = merged_alpha;
    let evaluation_samples = first_swarm_samples_for_backend(local_node.backend_label.as_str())?;
    let backend = OpenAdapterTrainingExecutionBackend::new(config.clone(), evaluation_samples)?;
    let training_groups =
        merged_training_groups(&backend, merged_lora_a.as_slice(), merged_lora_b.as_slice())?;
    let tokenizer = PortableTokenizerBinding::new(
        config.model.tokenizer.clone(),
        PortableTokenizerAssetFormat::PsionicDigest,
        format!(
            "{}@{}",
            config.model.base_model_id, config.model.base_model_revision
        ),
    );
    let portable_bundle = PortableModelBundle::from_training_groups(
        config.admissible_model_family.adapter_family(),
        config.model.base_model_revision.clone(),
        config.checkpoint_family.clone(),
        Some(format!("checkpoint://{}/merged", config.run_id)),
        training_groups.as_slice(),
        tokenizer,
        config.model.tokenizer.template_digest.clone(),
    )?;
    let (portable_bundle_bytes, portable_bundle_receipt) = portable_bundle.export_safetensors()?;
    write_bytes(
        merged_portable_bundle_path,
        portable_bundle_bytes.as_slice(),
    )?;
    let (
        canonical_profile_mean_loss,
        canonical_profile_batch_count,
        canonical_profile_sample_count,
    ) = evaluate_training_groups(&backend, training_groups)?;

    let mut contributor_weight_bps = BTreeMap::new();
    contributor_weight_bps.insert(local_node.node_id.clone(), 5_000);
    contributor_weight_bps.insert(remote_node.node_id.clone(), 5_000);
    let mut report = FirstSwarmTrustedLanMergedArtifactReport {
        schema_version: String::from(FIRST_SWARM_TRUSTED_LAN_MERGED_ARTIFACT_REPORT_SCHEMA_VERSION),
        run_id: String::from(run_id),
        aggregation_policy_id: String::from("aggregation.open_adapter.mean_delta"),
        merge_strategy: String::from("exact_mean_delta_rank_stacking"),
        contributor_weight_bps,
        local_contributor_receipt_digest: local_receipt.receipt_digest.clone(),
        remote_contributor_receipt_digest: remote_receipt.receipt_digest.clone(),
        local_adapter_artifact_digest: local_receipt.adapter_artifact_digest.clone(),
        remote_adapter_artifact_digest: remote_receipt.adapter_artifact_digest.clone(),
        merged_adapter_artifact_digest,
        merged_adapter_identity_digest: merged_identity.stable_digest(),
        merged_portable_bundle_artifact_digest: portable_bundle_receipt.artifact_digest.clone(),
        merged_portable_bundle_state_dict_digest: portable_bundle_receipt
            .state_dict_digest
            .clone(),
        merged_lora_rank: merged_rank,
        merged_lora_alpha: merged_alpha,
        evaluation_backend_label: local_node.backend_label.clone(),
        canonical_profile_mean_loss,
        canonical_profile_bits_per_token: canonical_profile_mean_loss / std::f64::consts::LN_2,
        canonical_profile_batch_count,
        canonical_profile_sample_count,
        deterministic_probe_top_token_id,
        claim_boundary: String::from(
            "This report records one exact mean-delta merged open-adapter artifact derived from the two admitted trusted-LAN contributor payloads. It proves that the retained mixed-device run now emits an inferable merged adapter and portable bundle, but it does not claim exact HOMEGOLF or public Parameter Golf score parity.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_first_swarm_trusted_lan_merged_artifact_report|",
        &report,
    );
    write_json(merged_report_path, &report)?;
    Ok(report)
}

fn merged_training_groups(
    backend: &OpenAdapterTrainingExecutionBackend,
    merged_lora_a: &[f32],
    merged_lora_b: &[f32],
) -> Result<Vec<TrainingParameterGroupState>, FirstSwarmTrustedLanRuntimeError> {
    let mut groups = backend.initial_training_groups()?;
    if groups.len() != 2 {
        return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!(
                "expected two open-adapter training groups, found {}",
                groups.len()
            ),
        });
    }
    groups[0].parameter = TrainingTensorBuffer::from_f32(
        groups[0].group_id.clone(),
        groups[0].parameter.spec.clone(),
        merged_lora_a.to_vec(),
    )?;
    groups[1].parameter = TrainingTensorBuffer::from_f32(
        groups[1].group_id.clone(),
        groups[1].parameter.spec.clone(),
        merged_lora_b.to_vec(),
    )?;
    Ok(groups)
}

fn evaluate_training_groups(
    backend: &OpenAdapterTrainingExecutionBackend,
    training_groups: Vec<TrainingParameterGroupState>,
) -> Result<(f64, usize, usize), FirstSwarmTrustedLanRuntimeError> {
    let run = FixedBudgetTrainingRun::new(
        backend.config().run_id.clone(),
        backend.config().checkpoint_family.clone(),
        TrainingLoopBudget::new(1, 1, 1)?,
        training_groups,
    )?;
    let mut weighted_loss = 0.0_f64;
    let mut sample_count = 0_u64;
    for batch_index in 0..backend.batches().len() {
        let record = backend.produce_gradient_batch(&run, batch_index)?;
        weighted_loss += record.mean_loss as f64 * f64::from(record.training_batch.sample_count);
        sample_count += u64::from(record.training_batch.sample_count);
    }
    Ok((
        weighted_loss / sample_count.max(1) as f64,
        backend.batches().len(),
        sample_count as usize,
    ))
}

fn deterministic_probe_top_token_id(
    artifact: &LmHeadLoraAdapterArtifact,
    vocab_size: usize,
) -> Result<usize, FirstSwarmTrustedLanRuntimeError> {
    let mut logits = vec![0.0_f32; vocab_size];
    artifact
        .apply_to_logits(&[1.0, 0.0, 0.0, 0.0], logits.as_mut_slice())
        .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("failed to compute merged deterministic probe logits: {error}"),
        })?;
    Ok(logits
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.partial_cmp(right.1).expect("finite logits"))
        .map(|(index, _)| index)
        .unwrap_or_default())
}

fn merged_adapter_identity(
    run_id: &str,
    receipt: &FirstSwarmOpenAdapterContributorReceipt,
    artifact_digest: &str,
    merged_rank: usize,
) -> AdapterArtifactIdentity {
    AdapterArtifactIdentity::new(
        format!("first-swarm-merged-{run_id}"),
        String::from("mean-delta-rank-stacked"),
        AdapterArtifactKind::Lora,
        AdapterArtifactFormat::Safetensors,
        receipt.manifest.base_model_id.clone(),
        receipt.manifest.base_model_revision.clone(),
        receipt.manifest.base_served_artifact_digest.clone(),
        String::from(artifact_digest),
        QuantizationMode::None,
        AdapterTargetFamily::DecoderComposite,
        u64::try_from(
            merged_rank.saturating_mul(receipt.manifest.hidden_size + receipt.manifest.vocab_size),
        )
        .unwrap_or(u64::MAX),
    )
    .with_provenance_digest(receipt.execution_provenance_digest.clone())
    .with_governance_digest(receipt.manifest.receipt_contract_digest.clone())
}

fn decode_lora_adapter(
    payload: &[u8],
    alpha: f32,
) -> Result<DecodedLoraAdapter, FirstSwarmTrustedLanRuntimeError> {
    let tensors = SafeTensors::deserialize(payload).map_err(|error| {
        FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("failed to decode contributor safetensors payload: {error}"),
        }
    })?;
    let (lora_a_shape, lora_a) =
        decode_lora_tensor(&tensors, &["lm_head.lora_A.weight", "output.lora_A.weight"])?;
    let (lora_b_shape, lora_b) =
        decode_lora_tensor(&tensors, &["lm_head.lora_B.weight", "output.lora_B.weight"])?;
    let [rank, hidden_size] = lora_a_shape.as_slice() else {
        return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("unexpected LoRA A shape {lora_a_shape:?}"),
        });
    };
    let [vocab_size, lora_b_rank] = lora_b_shape.as_slice() else {
        return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("unexpected LoRA B shape {lora_b_shape:?}"),
        });
    };
    if rank != lora_b_rank {
        return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("LoRA rank mismatch between A ({rank}) and B ({lora_b_rank}) tensors"),
        });
    }
    Ok(DecodedLoraAdapter {
        rank: *rank,
        alpha,
        hidden_size: *hidden_size,
        vocab_size: *vocab_size,
        lora_a,
        lora_b,
    })
}

fn decode_lora_tensor(
    tensors: &SafeTensors<'_>,
    names: &[&str],
) -> Result<(Vec<usize>, Vec<f32>), FirstSwarmTrustedLanRuntimeError> {
    let tensor = names
        .iter()
        .find_map(|name| {
            tensors
                .tensor(name)
                .ok()
                .map(|tensor| ((*name).to_string(), tensor))
        })
        .ok_or_else(|| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("missing expected LoRA tensor from payload: {:?}", names),
        })?;
    let (tensor_name, tensor_view) = tensor;
    if tensor_view.dtype() != SafeTensorsDType::F32 {
        return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!(
                "tensor `{tensor_name}` used unsupported dtype {:?}",
                tensor_view.dtype()
            ),
        });
    }
    let data = tensor_view.data();
    if data.len() % 4 != 0 {
        return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("tensor `{tensor_name}` did not carry a valid f32 byte length"),
        });
    }
    let values = data
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect::<Vec<_>>();
    Ok((tensor_view.shape().to_vec(), values))
}

fn serialize_lora_adapter(
    rank: usize,
    hidden_size: usize,
    vocab_size: usize,
    lora_a: &[f32],
    lora_b: &[f32],
) -> Result<Vec<u8>, FirstSwarmTrustedLanRuntimeError> {
    let raw_a = encode_f32_bytes(lora_a);
    let raw_b = encode_f32_bytes(lora_b);
    let view_a = TensorView::new(
        SafeTensorsDType::F32,
        vec![rank, hidden_size],
        raw_a.as_slice(),
    )
    .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
        detail: format!("failed to build merged LoRA A tensor view: {error}"),
    })?;
    let view_b = TensorView::new(
        SafeTensorsDType::F32,
        vec![vocab_size, rank],
        raw_b.as_slice(),
    )
    .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
        detail: format!("failed to build merged LoRA B tensor view: {error}"),
    })?;
    serialize(
        [
            ("lm_head.lora_A.weight", view_a),
            ("lm_head.lora_B.weight", view_b),
        ],
        None,
    )
    .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
        detail: format!("failed to serialize merged LoRA adapter artifact: {error}"),
    })
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len().saturating_mul(4));
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), FirstSwarmTrustedLanRuntimeError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FirstSwarmTrustedLanRuntimeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(path, bytes).map_err(|error| FirstSwarmTrustedLanRuntimeError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn first_swarm_samples_for_backend(
    backend_label: &str,
) -> Result<Vec<crate::OpenAdapterHiddenStateSample>, FirstSwarmTrustedLanRuntimeError> {
    let sample_prefix = match backend_label {
        OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL => "swarm-mlx-live",
        OPEN_ADAPTER_CUDA_BACKEND_LABEL => "swarm-cuda-live",
        other => {
            return Err(FirstSwarmTrustedLanRuntimeError::InvalidInput {
                detail: format!("unsupported first-swarm backend label `{other}`"),
            });
        }
    };
    Ok(first_swarm_open_adapter_samples(sample_prefix)?)
}

fn unsupported_precision_refusal(
    backend: &OpenAdapterTrainingExecutionBackend,
) -> Result<String, FirstSwarmTrustedLanRuntimeError> {
    Ok(OpenAdapterTrainingExecutionBackend::new(
        OpenAdapterExecutionConfig {
            precision_policy: OpenAdapterPrecisionPolicy::Bf16Mixed,
            ..backend.config().clone()
        },
        vec![
            OpenAdapterHiddenStateSample::new("unsupported", vec![1.0, 0.0, 0.0, 0.0], 2, 1)
                .map_err(|error| FirstSwarmTrustedLanRuntimeError::InvalidInput {
                    detail: error.to_string(),
                })?,
        ],
    )
    .expect_err("bf16 should stay unsupported")
    .to_string())
}

fn materialize_validation_bundle(
    storage: &mut AdapterArtifactStorageState,
    security: &mut AdapterContributionSecurityController,
    protocol: &AdapterWorkerProtocolState,
    assignment: &crate::AdapterContributionWorkAssignment,
    claim: &AdapterAssignmentClaim,
    identity: &AdapterWorkerIdentity,
    submission: &AdapterContributionSubmissionReceipt,
    payload: &[u8],
    base_time_ms: u64,
) -> Result<AdapterContributionValidationBundle, FirstSwarmTrustedLanRuntimeError> {
    let upload = submission.upload.clone();
    let cursor = storage.start_contribution_upload(
        assignment,
        upload,
        payload,
        FIRST_SWARM_CHUNK_BYTES,
        assignment.worker_id.clone(),
        base_time_ms,
    )?;
    for chunk in payload.chunks(FIRST_SWARM_CHUNK_BYTES) {
        let _ = storage.commit_next_chunk(cursor.upload_id.as_str(), chunk)?;
    }
    let artifact =
        storage.complete_contribution_upload(cursor.upload_id.as_str(), base_time_ms + 1)?;
    let signing_key =
        signing_key_for_worker(submission.window_id.as_str(), identity.worker_id.as_str());
    let provenance = crate::AdapterContributionProvenanceBundle::new_signed(
        assignment,
        claim,
        identity,
        submission,
        &artifact,
        &signing_key,
        base_time_ms + 2,
    );
    let security_receipt = security.assess_submission(
        protocol,
        &artifact,
        submission,
        provenance.clone(),
        base_time_ms + 3,
    )?;
    storage.set_contribution_disposition(
        artifact.contribution_id.as_str(),
        AdapterContributionArtifactDisposition::Accepted,
        base_time_ms + 4,
    )?;
    let replay = crate::AdapterContributionReplayReceipt::new(
        submission.contribution_id.clone(),
        submission.execution_summary.adapter_delta_digest.clone(),
        submission.execution_summary.adapter_delta_digest.clone(),
        base_time_ms + 5,
    );
    Ok(AdapterContributionValidationBundle::new(
        submission.clone(),
        artifact,
        provenance,
        security_receipt,
        Some(replay),
    ))
}

fn cluster_state_from_context(context: &ExecutionContext, cluster_id: &ClusterId) -> ClusterState {
    let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
    snapshot.memberships = BTreeMap::from([
        membership_for_node(cluster_id, &context.local_node, context.local_endpoint),
        membership_for_node(cluster_id, &context.peer_node, context.peer_endpoint),
    ]);
    snapshot.telemetry = BTreeMap::from([
        telemetry_for_node(&context.local_node),
        telemetry_for_node(&context.peer_node),
    ]);
    ClusterState::from_snapshot(snapshot)
}

fn membership_for_node(
    cluster_id: &ClusterId,
    node: &ExecutionNode,
    endpoint: SocketAddr,
) -> (NodeId, ClusterMembershipRecord) {
    let node_id = NodeId::new(node.node_id.clone());
    (
        node_id.clone(),
        ClusterMembershipRecord::new(
            ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: node_id.clone(),
                node_epoch: NodeEpoch::initial(),
                role: cluster_node_role(node),
                auth_public_key: format!("{}-trusted-lan-public-key", node.node_id),
                attestation: None,
            },
            Some(endpoint),
            ClusterMembershipStatus::Ready,
        ),
    )
}

fn telemetry_for_node(node: &ExecutionNode) -> (NodeId, ClusterNodeTelemetry) {
    let (memory_total, memory_free) = match node.backend_label.as_str() {
        OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL => (32_u64, 24_u64),
        OPEN_ADAPTER_CUDA_BACKEND_LABEL => (24_u64, 20_u64),
        _ => (16_u64, 12_u64),
    };
    (
        NodeId::new(node.node_id.clone()),
        ClusterNodeTelemetry::new(NodeId::new(node.node_id.clone()))
            .with_memory(
                Some(memory_total * 1024 * 1024 * 1024),
                Some(memory_free * 1024 * 1024 * 1024),
            )
            .with_accelerator_count(1)
            .with_backend_readiness(
                node.backend_label.clone(),
                ClusterBackendReadinessStatus::Ready,
            )
            .with_stability_posture(ClusterStabilityPosture::Stable),
    )
}

fn upload_locator_for_assignment(
    assignment: &crate::AdapterContributionWorkAssignment,
    payload: &[u8],
    suffix: &str,
) -> Result<AdapterContributionUploadLocator, FirstSwarmTrustedLanRuntimeError> {
    let manifest = DatastreamManifest::from_bytes(
        format!(
            "adapter-contribution:{}:{}",
            assignment.window_id, assignment.contribution_id
        ),
        DatastreamSubjectKind::AdapterPackage,
        payload,
        FIRST_SWARM_CHUNK_BYTES,
        DatastreamEncoding::RawBinary,
    )
    .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
    Ok(AdapterContributionUploadLocator::new(
        format!(
            "{}/artifact-{suffix}",
            assignment.upload_expectation.upload_reference_prefix
        ),
        manifest.manifest_ref().manifest_digest.clone(),
        manifest.manifest_ref().total_bytes,
    )?)
}

fn build_identity(run_id: &str, worker_id: &str) -> AdapterWorkerIdentity {
    let signing_key = signing_key_for_worker(run_id, worker_id);
    AdapterWorkerIdentity::new(
        worker_id,
        format!("{worker_id}-session-{run_id}"),
        AdapterWorkerTrustClass::SemiTrustedContributor,
        format!("swarm-trusted-lan:{worker_id}"),
    )
    .with_submission_signing_public_key_hex(hex::encode(signing_key.verifying_key().to_bytes()))
}

fn signing_key_for_worker(_seed_scope: &str, worker_id: &str) -> SigningKey {
    let digest = Sha256::digest(format!("first-swarm-trusted-lan|{worker_id}").as_bytes());
    let key_bytes: [u8; 32] = digest.into();
    SigningKey::from_bytes(&key_bytes)
}

fn build_cluster_id(run_id: &str, topology_contract_digest: &str) -> ClusterId {
    let swarm_contract = first_swarm_run_contract();
    let admission_token = AdmissionToken::new(hex::encode(Sha256::digest(
        format!("first-swarm-trusted-lan-admission|{run_id}|{topology_contract_digest}").as_bytes(),
    )));
    ClusterId::new(
        &ClusterNamespace::new(swarm_contract.cluster_namespace),
        &admission_token,
    )
}

fn cluster_node_role(node: &ExecutionNode) -> NodeRole {
    if node.role_id == "swarm.mac.mlx.coordinator_validator_contributor" {
        NodeRole::Mixed
    } else {
        NodeRole::ExecutorOnly
    }
}

fn parse_socket_addr(raw: &str) -> Result<SocketAddr, FirstSwarmTrustedLanRuntimeError> {
    raw.parse()
        .map_err(|error| FirstSwarmTrustedLanRuntimeError::InvalidInput {
            detail: format!("invalid socket address `{raw}`: {error}"),
        })
}

fn connect_with_retry(
    endpoint: &str,
    timeout_seconds: u64,
) -> Result<TcpStream, FirstSwarmTrustedLanRuntimeError> {
    let started = Instant::now();
    loop {
        match TcpStream::connect(endpoint) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                if started.elapsed() > Duration::from_secs(timeout_seconds) {
                    return Err(FirstSwarmTrustedLanRuntimeError::Timeout {
                        detail: format!("failed to connect to peer endpoint `{endpoint}`: {error}"),
                    });
                }
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

fn configure_stream(stream: &TcpStream) -> Result<(), FirstSwarmTrustedLanRuntimeError> {
    stream
        .set_nodelay(true)
        .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("failed to set TCP_NODELAY: {error}"),
        })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("failed to set read timeout: {error}"),
        })?;
    stream
        .set_write_timeout(Some(Duration::from_secs(60)))
        .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("failed to set write timeout: {error}"),
        })?;
    Ok(())
}

fn send_message(
    stream: &mut TcpStream,
    message: &FirstSwarmTrustedLanMessage,
) -> Result<(), FirstSwarmTrustedLanRuntimeError> {
    let encoded = serde_json::to_vec(message)?;
    stream
        .write_all(encoded.as_slice())
        .and_then(|_| stream.write_all(b"\n"))
        .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("failed to send cluster message: {error}"),
        })?;
    Ok(())
}

fn receive_message(
    stream: &mut TcpStream,
) -> Result<FirstSwarmTrustedLanMessage, FirstSwarmTrustedLanRuntimeError> {
    let mut bytes = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let read =
            stream
                .read(&mut byte)
                .map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
                    detail: format!("failed to read cluster message: {error}"),
                })?;
        if read == 0 {
            return Err(FirstSwarmTrustedLanRuntimeError::Protocol {
                detail: String::from("peer closed the cluster connection unexpectedly"),
            });
        }
        if byte[0] == b'\n' {
            break;
        }
        bytes.push(byte[0]);
    }
    let line =
        String::from_utf8(bytes).map_err(|error| FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("cluster message was not valid UTF-8: {error}"),
        })?;
    serde_json::from_str::<FirstSwarmTrustedLanMessage>(line.trim()).map_err(|error| {
        FirstSwarmTrustedLanRuntimeError::Protocol {
            detail: format!("failed to decode cluster message: {error}"),
        }
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, FirstSwarmTrustedLanRuntimeError> {
    let bytes = fs::read(path).map_err(|error| FirstSwarmTrustedLanRuntimeError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| FirstSwarmTrustedLanRuntimeError::Deserialize {
        path: path.display().to_string(),
        error,
    })
}

fn write_runtime_report(
    output_path: &Path,
    report: &FirstSwarmTrustedLanRuntimeReport,
) -> Result<(), FirstSwarmTrustedLanRuntimeError> {
    write_json(output_path, report)
}

fn write_json(
    output_path: &Path,
    value: &impl Serialize,
) -> Result<(), FirstSwarmTrustedLanRuntimeError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FirstSwarmTrustedLanRuntimeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let encoded = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmTrustedLanRuntimeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn free_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = listener.local_addr().expect("listener addr").port();
        drop(listener);
        port
    }

    #[test]
    fn trusted_lan_runtime_runs_over_loopback() {
        let temp = tempdir().expect("tempdir");
        let topology_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/swarm/first_swarm_trusted_lan_topology_contract_v1.json");
        let workflow_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/swarm/first_swarm_live_workflow_plan_v1.json");
        let coordinator_report_path = temp.path().join("coordinator.json");
        let contributor_report_path = temp.path().join("contributor.json");
        let coordinator_port = free_port();
        let contributor_port = free_port();
        let coordinator_endpoint = format!("127.0.0.1:{coordinator_port}");
        let contributor_endpoint = format!("127.0.0.1:{contributor_port}");

        let coordinator_output = coordinator_report_path.clone();
        let topology_for_thread = topology_path.clone();
        let workflow_for_thread = workflow_path.clone();
        let contributor_endpoint_for_thread = contributor_endpoint.clone();
        let coordinator_endpoint_for_thread = coordinator_endpoint.clone();
        let coordinator_handle = thread::spawn(move || {
            run_first_swarm_trusted_lan_runtime(
                FirstSwarmTrustedLanRuntimeRole::Coordinator,
                "first-swarm-loopback-test",
                topology_for_thread,
                workflow_for_thread,
                coordinator_endpoint_for_thread.as_str(),
                contributor_endpoint_for_thread.as_str(),
                coordinator_output,
            )
        });

        thread::sleep(Duration::from_millis(200));
        let contributor_report = run_first_swarm_trusted_lan_runtime(
            FirstSwarmTrustedLanRuntimeRole::Contributor,
            "first-swarm-loopback-test",
            topology_path,
            workflow_path,
            contributor_endpoint.as_str(),
            coordinator_endpoint.as_str(),
            contributor_report_path,
        )
        .expect("contributor runtime should succeed");
        let coordinator_report = coordinator_handle
            .join()
            .expect("coordinator thread should join")
            .expect("coordinator runtime should succeed");

        assert_eq!(
            contributor_report.execution_backend_label,
            OPEN_ADAPTER_CUDA_BACKEND_LABEL
        );
        assert!(contributor_report.local_contribution.is_some());
        assert_eq!(
            coordinator_report.execution_backend_label,
            OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL
        );
        assert!(coordinator_report.local_contribution.is_some());
        assert_eq!(coordinator_report.submission_receipts.len(), 2);
        assert!(coordinator_report.validator_summary.is_some());
        assert!(coordinator_report.promotion_receipt.is_some());
        assert!(coordinator_report.aggregation_compatibility.is_some());
        assert_eq!(coordinator_report.replay_receipt_digests.len(), 2);
        let retained_artifacts = coordinator_report
            .retained_artifacts
            .as_ref()
            .expect("coordinator should retain merged artifacts");
        assert!(Path::new(&retained_artifacts.local_contributor_adapter_path).exists());
        assert!(Path::new(&retained_artifacts.remote_contributor_adapter_path).exists());
        assert!(Path::new(&retained_artifacts.merged_adapter_path).exists());
        assert!(Path::new(&retained_artifacts.merged_portable_bundle_path).exists());
        assert!(Path::new(&retained_artifacts.merged_report_path).exists());
        let merged_report: FirstSwarmTrustedLanMergedArtifactReport = serde_json::from_slice(
            &fs::read(&retained_artifacts.merged_report_path)
                .expect("merged report should be readable"),
        )
        .expect("merged report should decode");
        assert_eq!(
            merged_report.schema_version,
            FIRST_SWARM_TRUSTED_LAN_MERGED_ARTIFACT_REPORT_SCHEMA_VERSION
        );
        assert_eq!(
            merged_report.merge_strategy,
            "exact_mean_delta_rank_stacking"
        );
        assert_eq!(merged_report.merged_lora_rank, 4);
        let bundle = PortableModelBundle::import_safetensors(
            fs::read(&retained_artifacts.merged_portable_bundle_path)
                .expect("merged portable bundle should be readable")
                .as_slice(),
        )
        .expect("merged portable bundle should import");
        assert_eq!(bundle.to_training_groups().expect("groups").len(), 2);
    }
}
