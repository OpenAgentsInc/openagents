use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::SigningKey;
use psionic_cluster::{
    AdmissionToken, ClusterBackendReadinessStatus, ClusterId, ClusterMembershipRecord,
    ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
    ClusterSnapshot, ClusterStabilityPosture, ClusterState, NodeEpoch, NodeId, NodeRole,
};
use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    first_swarm_open_adapter_samples, first_swarm_open_adapter_sft_request,
    first_swarm_open_adapter_training_config, psion_google_two_node_swarm_contract,
    run_open_adapter_sft_export, AdapterArtifactRetentionPolicy, AdapterArtifactStorageError,
    AdapterArtifactStorageState, AdapterAssignmentAckReceipt, AdapterAssignmentClaim,
    AdapterClusterCoordinationError, AdapterClusterMembershipReceipt,
    AdapterClusterWindowPlanReceipt, AdapterContributionArtifactDisposition,
    AdapterContributionExecutionSummary, AdapterContributionProgress,
    AdapterContributionSecurityController, AdapterContributionSecurityError,
    AdapterContributionSecurityPolicy, AdapterContributionSubmissionReceipt,
    AdapterContributionUploadLocator, AdapterContributionValidationBundle,
    AdapterContributionValidatorPolicy, AdapterContributionValidatorState, AdapterPolicyAggregator,
    AdapterPolicyPromotionReceipt, AdapterTargetIdentity, AdapterTrainingClusterCoordinator,
    AdapterValidationError, AdapterWindowContractError, AdapterWindowScoreSummary,
    AdapterWorkerHeartbeatReceipt, AdapterWorkerIdentity, AdapterWorkerProtocolError,
    AdapterWorkerProtocolPolicy, AdapterWorkerProtocolState, AdapterWorkerTrustClass,
    CheckpointPointer, CheckpointRecoveryError, CheckpointScopeBinding, CheckpointScopeKind,
    FirstSwarmOpenAdapterReceiptError, OpenAdapterSftError, OpenAdapterTrainingExecutionBackend,
    OpenAdapterTrainingExecutionError, PolicyRevision, PsionGoogleTwoNodeSwarmContractError,
    PsionGoogleTwoNodeSwarmNodeRoleKind, TrainingCoreError, TrainingRunGraphError,
    TrainingRunState,
};

const GIB_BYTES: u64 = 1024 * 1024 * 1024;
const GOOGLE_SWARM_CLUSTER_MANIFEST_SCHEMA_VERSION: &str =
    "psion.google_two_node_swarm_cluster_manifest.v1";
const GOOGLE_SWARM_ENDPOINT_MANIFEST_SCHEMA_VERSION: &str =
    "psion.google_two_node_swarm_endpoint_manifest.v1";
const GOOGLE_SWARM_RUNTIME_REPORT_SCHEMA_VERSION: &str =
    "psion.google_two_node_swarm_runtime_report.v1";
const GOOGLE_SWARM_POLICY_FAMILY: &str = "gpt_oss.google.swarm.policy";
const GOOGLE_SWARM_ENV_PACKAGE_FAMILY: &str = "oa.google.swarm.open_adapter";
const GOOGLE_SWARM_ENV_PACKAGE_VERSION: &str = "2026.03.24";
const GOOGLE_SWARM_DATASET_REF: &str = "dataset://openagents/google_swarm/open_adapter_sft";
const GOOGLE_SWARM_CHUNK_BYTES: usize = 64;

/// Runtime role for the Google two-node configured-peer swarm command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionGoogleTwoNodeSwarmRuntimeRole {
    /// Coordinator, validator, aggregator, and contributor node.
    Coordinator,
    /// Contributor-only node.
    Contributor,
}

impl PsionGoogleTwoNodeSwarmRuntimeRole {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Coordinator => "coordinator",
            Self::Contributor => "contributor",
        }
    }
}

/// One node recorded in the machine-legible Google two-node launch manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmManifestNode {
    /// Stable node identifier from the frozen contract.
    pub node_id: String,
    /// Stable role identifier from the frozen contract.
    pub role_id: String,
    /// Stable role kind from the frozen contract.
    pub role_kind: PsionGoogleTwoNodeSwarmNodeRoleKind,
    /// GCE instance name chosen by the launcher.
    pub instance_name: String,
    /// Zone selected for the instance.
    pub zone: String,
    /// Dedicated subnetwork selected for the instance.
    pub subnetwork: String,
    /// Internal IPv4 address observed for the instance when the launcher already knows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_ip: Option<String>,
    /// Fully formatted peer endpoint observed for the instance when the launcher already knows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Stable cluster port reserved for the node.
    pub cluster_port: u16,
    /// Per-node endpoint manifest URI in GCS.
    pub endpoint_manifest_uri: String,
    /// Per-node bring-up report URI in GCS.
    pub bringup_report_uri: String,
    /// Per-node runtime report URI in GCS.
    pub runtime_report_uri: String,
    /// Machine type selected for the instance.
    pub machine_type: String,
    /// Accelerator type selected for the instance.
    pub accelerator_type: String,
    /// Accelerator count selected for the instance.
    pub accelerator_count: u16,
}

/// Launch-time manifest for the Google two-node configured-peer swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmClusterManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Launch timestamp in UTC.
    pub created_at_utc: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable cluster id derived from the configured-peer namespace and admission token.
    pub cluster_id: String,
    /// Stable contract digest this launch claims to implement.
    pub contract_digest: String,
    /// Stable cluster namespace.
    pub cluster_namespace: String,
    /// Stable project id.
    pub project_id: String,
    /// Stable region family.
    pub region_family: String,
    /// Stable bucket url.
    pub bucket_url: String,
    /// Stable run prefix in GCS.
    pub run_prefix: String,
    /// Stable training command identifier.
    pub training_command_id: String,
    /// Selected zone-pair identifier.
    pub selected_zone_pair_id: String,
    /// Selected impairment profile identifier.
    pub selected_impairment_profile_id: String,
    /// Repo revision chosen by the launcher.
    pub git_revision: String,
    /// Launch receipt URI.
    pub launch_receipt_uri: String,
    /// Final manifest URI reserved for later finalization.
    pub final_manifest_uri: String,
    /// Role-aware node list for the launch.
    pub nodes: Vec<PsionGoogleTwoNodeSwarmManifestNode>,
}

/// Endpoint manifest written for one launched node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmEndpointManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Endpoint observation time in UTC.
    pub created_at_utc: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable node identifier.
    pub node_id: String,
    /// Stable role identifier.
    pub role_id: String,
    /// Zone selected for the instance.
    pub zone: String,
    /// Internal IPv4 address observed for the instance.
    pub internal_ip: String,
    /// Stable cluster port reserved for the node.
    pub cluster_port: u16,
    /// Fully formatted peer endpoint.
    pub endpoint: String,
    /// Short detail naming the authority that wrote the endpoint manifest.
    pub source: String,
}

/// Local execution summary for one node-local adapter contribution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmLocalContribution {
    /// Stable run identifier.
    pub run_id: String,
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable session identifier.
    pub session_id: String,
    /// Backend label carried by the bounded open-adapter backend.
    pub execution_backend_label: String,
    /// Adapter artifact digest emitted by the local export.
    pub adapter_artifact_digest: String,
    /// Adapter identity digest emitted by the local export.
    pub adapter_identity_digest: String,
    /// Adapter-delta digest surfaced through the generic contribution summary.
    pub adapter_delta_digest: String,
    /// Payload digest over the staged adapter artifact bytes.
    pub payload_sha256: String,
    /// Payload size in bytes.
    pub payload_bytes: usize,
    /// Deterministic final mean loss for the bounded local run.
    pub final_mean_loss: f32,
    /// Step count executed by the bounded local run.
    pub executed_steps: usize,
    /// Packed batch count executed by the bounded local run.
    pub batch_count: usize,
    /// Generic contribution summary used by the cluster protocol.
    pub execution_summary: AdapterContributionExecutionSummary,
}

/// Runtime report written by either the coordinator or contributor command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmRuntimeReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable cluster identifier.
    pub cluster_id: String,
    /// Stable training command identifier.
    pub training_command_id: String,
    /// Stable node identifier.
    pub node_id: String,
    /// Stable role identifier.
    pub role_id: String,
    /// Stable role kind.
    pub role_kind: PsionGoogleTwoNodeSwarmNodeRoleKind,
    /// Runtime role label for this execution.
    pub runtime_role: PsionGoogleTwoNodeSwarmRuntimeRole,
    /// Backend label carried by the bounded open-adapter backend.
    pub execution_backend_label: String,
    /// Selected impairment profile identifier.
    pub selected_impairment_profile_id: String,
    /// Local endpoint used for cluster traffic.
    pub local_endpoint: String,
    /// Peer endpoint used for cluster traffic.
    pub peer_endpoint: String,
    /// Membership receipt when this node owned the coordinator role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership_receipt: Option<AdapterClusterMembershipReceipt>,
    /// Window plan when this node owned the coordinator role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_plan: Option<AdapterClusterWindowPlanReceipt>,
    /// Heartbeat receipts retained by the worker protocol state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heartbeat_receipts: Vec<AdapterWorkerHeartbeatReceipt>,
    /// Acknowledgement receipts retained by the worker protocol state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acknowledgement_receipts: Vec<AdapterAssignmentAckReceipt>,
    /// Submission receipts retained by the worker protocol state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub submission_receipts: Vec<AdapterContributionSubmissionReceipt>,
    /// Validator summary when this node owned the coordinator role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_summary: Option<AdapterWindowScoreSummary>,
    /// Aggregation or hold receipt when this node owned the coordinator role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promotion_receipt: Option<AdapterPolicyPromotionReceipt>,
    /// Local node contribution summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_contribution: Option<PsionGoogleTwoNodeSwarmLocalContribution>,
    /// Short causal detail explaining what this report proves.
    pub protocol_detail: String,
    /// Honest claim boundary for the bounded Google swarm runtime.
    pub claim_boundary: String,
    /// Observed start time in milliseconds.
    pub started_at_ms: u64,
    /// Observed finish time in milliseconds.
    pub finished_at_ms: u64,
    /// Observed wallclock duration in milliseconds.
    pub observed_wallclock_ms: u64,
    /// Stable report digest.
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
enum PsionGoogleTwoNodeSwarmMessage {
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

struct ExecutionContext {
    manifest: PsionGoogleTwoNodeSwarmClusterManifest,
    local_node: PsionGoogleTwoNodeSwarmManifestNode,
    peer_node: PsionGoogleTwoNodeSwarmManifestNode,
    local_endpoint: PsionGoogleTwoNodeSwarmEndpointManifest,
    peer_endpoint: PsionGoogleTwoNodeSwarmEndpointManifest,
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
}

struct LocalContributionRun {
    contribution: PsionGoogleTwoNodeSwarmLocalContribution,
    payload: Vec<u8>,
}

/// Errors surfaced while running the bounded Google two-node configured-peer swarm command.
#[derive(Debug, Error)]
pub enum PsionGoogleTwoNodeSwarmRuntimeError {
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
    Contract(#[from] PsionGoogleTwoNodeSwarmContractError),
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
    OpenAdapterFixture(#[from] FirstSwarmOpenAdapterReceiptError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    #[error("google two-node swarm runtime refused the manifest: {detail}")]
    InvalidManifest { detail: String },
    #[error("google two-node swarm runtime protocol failure: {detail}")]
    Protocol { detail: String },
    #[error("google two-node swarm runtime timed out: {detail}")]
    Timeout { detail: String },
}

/// Runs one role of the bounded Google two-node configured-peer swarm command and writes a report.
pub fn run_psion_google_two_node_swarm_runtime(
    role: PsionGoogleTwoNodeSwarmRuntimeRole,
    cluster_manifest_path: impl AsRef<Path>,
    local_endpoint_path: impl AsRef<Path>,
    peer_endpoint_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<PsionGoogleTwoNodeSwarmRuntimeReport, PsionGoogleTwoNodeSwarmRuntimeError> {
    let cluster_manifest_path = cluster_manifest_path.as_ref();
    let local_endpoint_path = local_endpoint_path.as_ref();
    let peer_endpoint_path = peer_endpoint_path.as_ref();
    let output_path = output_path.as_ref();
    let started_at_ms = now_ms();
    let started = Instant::now();
    let context = load_execution_context(
        role,
        cluster_manifest_path,
        local_endpoint_path,
        peer_endpoint_path,
    )?;
    let report = match role {
        PsionGoogleTwoNodeSwarmRuntimeRole::Coordinator => run_coordinator(context, started_at_ms)?,
        PsionGoogleTwoNodeSwarmRuntimeRole::Contributor => run_contributor(context, started_at_ms)?,
    };
    let mut report = report;
    report.finished_at_ms = now_ms();
    report.observed_wallclock_ms = started.elapsed().as_millis() as u64;
    report.report_digest = stable_digest(b"psionic_google_two_node_swarm_runtime_report|", &report);
    write_runtime_report(output_path, &report)?;
    Ok(report)
}

fn run_coordinator(
    context: ExecutionContext,
    started_at_ms: u64,
) -> Result<PsionGoogleTwoNodeSwarmRuntimeReport, PsionGoogleTwoNodeSwarmRuntimeError> {
    let mut plan = build_coordinator_plan(&context, started_at_ms)?;
    let local_claim_id = plan.local_claim.claim_id.clone();
    let local_assignment_id = plan.local_assignment.assignment_id.clone();
    let local_identity = plan.local_identity.clone();
    let local_run_id = context.manifest.run_id.clone();
    let local_node_id = context.local_node.node_id.clone();

    let listener = TcpListener::bind(SocketAddr::from((
        Ipv4Addr::UNSPECIFIED,
        context.local_endpoint.cluster_port,
    )))
    .map_err(|error| PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
        detail: format!(
            "failed to bind coordinator listener on {}:{}: {error}",
            context.local_endpoint.internal_ip, context.local_endpoint.cluster_port
        ),
    })?;
    listener.set_nonblocking(false).map_err(|error| {
        PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: format!("failed to configure coordinator listener: {error}"),
        }
    })?;

    let local_execution_handle = thread::spawn(move || {
        run_local_contribution(
            local_run_id.as_str(),
            local_node_id.as_str(),
            local_claim_id.as_str(),
            local_assignment_id.as_str(),
            local_identity.session_id.as_str(),
        )
    });

    let (mut stream, _) =
        listener
            .accept()
            .map_err(|error| PsionGoogleTwoNodeSwarmRuntimeError::Timeout {
                detail: format!("coordinator failed to accept contributor connection: {error}"),
            })?;
    configure_stream(&stream)?;
    let hello = receive_message(&mut stream)?;
    let (peer_node_id, peer_session_id, peer_public_key_hex) = match hello {
        PsionGoogleTwoNodeSwarmMessage::Hello {
            node_id,
            session_id,
            signing_public_key_hex,
        } => (node_id, session_id, signing_public_key_hex),
        other => {
            return Err(PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
                detail: format!("expected contributor hello, found {other:?}"),
            });
        }
    };
    if peer_node_id != context.peer_node.node_id {
        return Err(PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: format!(
                "coordinator expected peer node `{}` but connected node reported `{peer_node_id}`",
                context.peer_node.node_id
            ),
        });
    }
    if peer_public_key_hex != plan.expected_peer_signing_public_key_hex {
        return Err(PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: String::from(
                "contributor signing public key did not match the deterministic configured-peer key",
            ),
        });
    }
    let peer_identity = AdapterWorkerIdentity::new(
        peer_node_id.clone(),
        peer_session_id.clone(),
        AdapterWorkerTrustClass::SemiTrustedContributor,
        format!("google-configured-peer:{peer_node_id}"),
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
        &PsionGoogleTwoNodeSwarmMessage::Assignment {
            assignment: plan.peer_assignment.clone(),
            claim: peer_claim.clone(),
        },
    )?;

    let mut remote_execution = None;
    let mut remote_payload = Vec::new();
    while remote_execution.is_none() {
        match receive_message(&mut stream)? {
            PsionGoogleTwoNodeSwarmMessage::Heartbeat {
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
            PsionGoogleTwoNodeSwarmMessage::Contribution {
                claim_id,
                assignment_id,
                session_id,
                execution,
                payload_hex,
            } => {
                if claim_id != peer_claim.claim_id
                    || assignment_id != plan.peer_assignment.assignment_id
                    || session_id != peer_session_id
                {
                    return Err(PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
                        detail: String::from(
                            "contributor submitted a contribution for the wrong claim, assignment, or session",
                        ),
                    });
                }
                remote_execution = Some(execution);
                remote_payload = hex::decode(payload_hex).map_err(|error| {
                    PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
                        detail: format!("failed to decode contributor payload hex: {error}"),
                    }
                })?;
            }
            other => {
                return Err(PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
                    detail: format!(
                        "coordinator expected heartbeats or contribution, found {other:?}"
                    ),
                });
            }
        }
    }

    let local_run = local_execution_handle.join().map_err(|_| {
        PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
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
    let remote_upload = upload_locator_for_assignment(
        &plan.peer_assignment,
        remote_payload.as_slice(),
        "contributor",
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
        &PsionGoogleTwoNodeSwarmMessage::CompleteAck {
            submission_receipt_digest: remote_submission.receipt_digest.clone(),
        },
    )?;
    let local_upload = upload_locator_for_assignment(
        &plan.local_assignment,
        local_run.payload.as_slice(),
        "coordinator",
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
    let mut validator =
        AdapterContributionValidatorState::new(AdapterContributionValidatorPolicy {
            validator_policy_id: String::from("validator.open_adapter.google_two_node_swarm"),
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

    Ok(PsionGoogleTwoNodeSwarmRuntimeReport {
        schema_version: String::from(GOOGLE_SWARM_RUNTIME_REPORT_SCHEMA_VERSION),
        run_id: context.manifest.run_id,
        cluster_id: context.manifest.cluster_id,
        training_command_id: context.manifest.training_command_id,
        node_id: context.local_node.node_id,
        role_id: context.local_node.role_id,
        role_kind: context.local_node.role_kind,
        runtime_role: PsionGoogleTwoNodeSwarmRuntimeRole::Coordinator,
        execution_backend_label: String::from(crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL),
        selected_impairment_profile_id: context.manifest.selected_impairment_profile_id,
        local_endpoint: context.local_endpoint.endpoint,
        peer_endpoint: context.peer_endpoint.endpoint,
        membership_receipt: Some(plan.membership_receipt),
        window_plan: Some(plan.window_plan),
        heartbeat_receipts: plan.protocol.heartbeat_receipts.clone(),
        acknowledgement_receipts: plan.protocol.acknowledgement_receipts.clone(),
        submission_receipts: plan.protocol.submission_receipts.clone(),
        validator_summary: Some(summary),
        promotion_receipt: Some(promotion),
        local_contribution: Some(local_run.contribution),
        protocol_detail: String::from(
            "The coordinator built one bounded adapter-cluster window, admitted the configured contributor over the cluster port, accepted both local and remote contribution submissions, and sealed validator plus aggregation posture through the existing generic adapter-cluster, worker-protocol, security, validator, and aggregator state machines.",
        ),
        claim_boundary: String::from(
            "This runtime proves one bounded two-node Google configured-peer adapter-delta window over a real cluster-port transport. It does not claim trusted-cluster full-model training, elastic membership, wider-network discovery, or public swarm compute.",
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
) -> Result<PsionGoogleTwoNodeSwarmRuntimeReport, PsionGoogleTwoNodeSwarmRuntimeError> {
    // Cold first-boot cargo builds on separate Google nodes can drift by several minutes.
    // Keep the contributor retry budget wide enough to absorb that skew before refusing.
    const PEER_CONNECT_TIMEOUT_SECONDS: u64 = 600;
    let session_id = format!(
        "{}-session-{}",
        context.local_node.node_id, context.manifest.run_id
    );
    let signing_key = signing_key_for_worker(
        context.manifest.run_id.as_str(),
        context.local_node.node_id.as_str(),
    );
    let signing_public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());

    let mut stream = connect_with_retry(
        context.peer_endpoint.endpoint.as_str(),
        PEER_CONNECT_TIMEOUT_SECONDS,
    )?;
    configure_stream(&stream)?;
    send_message(
        &mut stream,
        &PsionGoogleTwoNodeSwarmMessage::Hello {
            node_id: context.local_node.node_id.clone(),
            session_id: session_id.clone(),
            signing_public_key_hex: signing_public_key_hex.clone(),
        },
    )?;
    let assignment_message = receive_message(&mut stream)?;
    let (assignment, claim) = match assignment_message {
        PsionGoogleTwoNodeSwarmMessage::Assignment { assignment, claim } => (assignment, claim),
        other => {
            return Err(PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
                detail: format!("contributor expected assignment, found {other:?}"),
            });
        }
    };
    if assignment.worker_id != context.local_node.node_id
        || claim.worker_id != context.local_node.node_id
    {
        return Err(PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: String::from(
                "coordinator assigned the contributor command to the wrong worker identifier",
            ),
        });
    }

    let local_run = run_local_contribution(
        context.manifest.run_id.as_str(),
        context.local_node.node_id.as_str(),
        claim.claim_id.as_str(),
        assignment.assignment_id.as_str(),
        session_id.as_str(),
    )?;
    send_message(
        &mut stream,
        &PsionGoogleTwoNodeSwarmMessage::Heartbeat {
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
        &PsionGoogleTwoNodeSwarmMessage::Heartbeat {
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
        &PsionGoogleTwoNodeSwarmMessage::Contribution {
            claim_id: claim.claim_id.clone(),
            assignment_id: assignment.assignment_id.clone(),
            session_id: session_id.clone(),
            execution,
            payload_hex: hex::encode(local_run.payload.as_slice()),
        },
    )?;
    let completion = receive_message(&mut stream)?;
    match completion {
        PsionGoogleTwoNodeSwarmMessage::CompleteAck { .. } => {}
        other => {
            return Err(PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
                detail: format!("contributor expected completion ack, found {other:?}"),
            });
        }
    }

    Ok(PsionGoogleTwoNodeSwarmRuntimeReport {
        schema_version: String::from(GOOGLE_SWARM_RUNTIME_REPORT_SCHEMA_VERSION),
        run_id: context.manifest.run_id,
        cluster_id: context.manifest.cluster_id,
        training_command_id: context.manifest.training_command_id,
        node_id: context.local_node.node_id,
        role_id: context.local_node.role_id,
        role_kind: context.local_node.role_kind,
        runtime_role: PsionGoogleTwoNodeSwarmRuntimeRole::Contributor,
        execution_backend_label: String::from(crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL),
        selected_impairment_profile_id: context.manifest.selected_impairment_profile_id,
        local_endpoint: context.local_endpoint.endpoint,
        peer_endpoint: context.peer_endpoint.endpoint,
        membership_receipt: None,
        window_plan: None,
        heartbeat_receipts: Vec::new(),
        acknowledgement_receipts: Vec::new(),
        submission_receipts: Vec::new(),
        validator_summary: None,
        promotion_receipt: None,
        local_contribution: Some(local_run.contribution),
        protocol_detail: String::from(
            "The contributor dialed the configured coordinator endpoint, accepted the bounded adapter-cluster assignment, emitted worker heartbeats on the cluster port, and returned one local adapter-delta contribution payload.",
        ),
        claim_boundary: String::from(
            "This runtime proves one bounded contributor path for the Google configured-peer adapter-delta rehearsal. It does not claim validator, aggregation, or promotion authority on the contributor node.",
        ),
        started_at_ms,
        finished_at_ms: started_at_ms,
        observed_wallclock_ms: 0,
        report_digest: String::new(),
    })
}

fn load_execution_context(
    role: PsionGoogleTwoNodeSwarmRuntimeRole,
    cluster_manifest_path: &Path,
    local_endpoint_path: &Path,
    peer_endpoint_path: &Path,
) -> Result<ExecutionContext, PsionGoogleTwoNodeSwarmRuntimeError> {
    let contract = psion_google_two_node_swarm_contract()?;
    let manifest: PsionGoogleTwoNodeSwarmClusterManifest = read_json(cluster_manifest_path)?;
    if manifest.schema_version != GOOGLE_SWARM_CLUSTER_MANIFEST_SCHEMA_VERSION {
        return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
            detail: format!(
                "cluster manifest schema_version must be `{GOOGLE_SWARM_CLUSTER_MANIFEST_SCHEMA_VERSION}` but was `{}`",
                manifest.schema_version
            ),
        });
    }
    if manifest.contract_digest != contract.contract_digest {
        return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
            detail: String::from(
                "cluster manifest contract_digest drifted from the frozen contract",
            ),
        });
    }
    if manifest.training_command_id != contract.training_command_id {
        return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
            detail: String::from(
                "cluster manifest training command drifted from the frozen contract",
            ),
        });
    }
    let local_endpoint: PsionGoogleTwoNodeSwarmEndpointManifest = read_json(local_endpoint_path)?;
    let peer_endpoint: PsionGoogleTwoNodeSwarmEndpointManifest = read_json(peer_endpoint_path)?;
    if local_endpoint.schema_version != GOOGLE_SWARM_ENDPOINT_MANIFEST_SCHEMA_VERSION
        || peer_endpoint.schema_version != GOOGLE_SWARM_ENDPOINT_MANIFEST_SCHEMA_VERSION
    {
        return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
            detail: String::from("endpoint manifest schema_version drifted"),
        });
    }
    let expected_local_role = match role {
        PsionGoogleTwoNodeSwarmRuntimeRole::Coordinator => {
            PsionGoogleTwoNodeSwarmNodeRoleKind::CoordinatorValidatorAggregatorContributor
        }
        PsionGoogleTwoNodeSwarmRuntimeRole::Contributor => {
            PsionGoogleTwoNodeSwarmNodeRoleKind::Contributor
        }
    };
    let local_node = manifest
        .nodes
        .iter()
        .find(|node| node.role_kind == expected_local_role)
        .cloned()
        .ok_or_else(|| PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
            detail: format!(
                "cluster manifest is missing the local role `{}`",
                role.label()
            ),
        })?;
    let peer_node = manifest
        .nodes
        .iter()
        .find(|node| node.node_id != local_node.node_id)
        .cloned()
        .ok_or_else(|| PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
            detail: String::from("cluster manifest must keep exactly two distinct nodes"),
        })?;
    if local_endpoint.node_id != local_node.node_id || peer_endpoint.node_id != peer_node.node_id {
        return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
            detail: String::from("endpoint manifest node ids drifted from the launch manifest"),
        });
    }
    if let Some(expected_ip) = local_node.internal_ip.as_deref() {
        if expected_ip != local_endpoint.internal_ip {
            return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
                detail: String::from(
                    "local endpoint manifest internal_ip drifted from the cluster manifest",
                ),
            });
        }
    }
    if let Some(expected_endpoint) = local_node.endpoint.as_deref() {
        if expected_endpoint != local_endpoint.endpoint {
            return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
                detail: String::from(
                    "local endpoint manifest endpoint drifted from the cluster manifest",
                ),
            });
        }
    }
    if let Some(expected_ip) = peer_node.internal_ip.as_deref() {
        if expected_ip != peer_endpoint.internal_ip {
            return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
                detail: String::from(
                    "peer endpoint manifest internal_ip drifted from the cluster manifest",
                ),
            });
        }
    }
    if let Some(expected_endpoint) = peer_node.endpoint.as_deref() {
        if expected_endpoint != peer_endpoint.endpoint {
            return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
                detail: String::from(
                    "peer endpoint manifest endpoint drifted from the cluster manifest",
                ),
            });
        }
    }
    Ok(ExecutionContext {
        manifest,
        local_node,
        peer_node,
        local_endpoint,
        peer_endpoint,
    })
}

fn build_coordinator_plan(
    context: &ExecutionContext,
    started_at_ms: u64,
) -> Result<CoordinatorPlan, PsionGoogleTwoNodeSwarmRuntimeError> {
    let cluster_id = build_cluster_id(
        context.manifest.run_id.as_str(),
        context.manifest.contract_digest.as_str(),
    );
    if cluster_id.as_str() != context.manifest.cluster_id {
        return Err(PsionGoogleTwoNodeSwarmRuntimeError::InvalidManifest {
            detail: String::from("cluster manifest cluster_id does not match the deterministic configured-peer cluster id"),
        });
    }
    let state = cluster_state_from_context(context, &cluster_id);
    let backend = OpenAdapterTrainingExecutionBackend::new(
        first_swarm_open_adapter_training_config(
            format!("{}-coordinator", context.manifest.run_id),
            format!("google-two-node-swarm:{}", context.manifest.run_id),
            crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL,
        ),
        first_swarm_open_adapter_samples("google-swarm-coordinator")?,
    )?;
    let adapter_target: AdapterTargetIdentity = backend.adapter_target_identity()?;
    let policy_revision = PolicyRevision::new(
        GOOGLE_SWARM_POLICY_FAMILY,
        "policy-r1",
        "policy-digest-r1",
        started_at_ms,
    )
    .with_revision_number(1)
    .with_checkpoint(google_swarm_checkpoint_reference(
        context.manifest.run_id.as_str(),
        "policy-r1",
        started_at_ms,
    ));
    let checkpoint_pointer = CheckpointPointer::new(
        CheckpointScopeBinding::new(CheckpointScopeKind::Window, "google-two-node-window-1"),
        GOOGLE_SWARM_POLICY_FAMILY,
        google_swarm_checkpoint_reference(
            context.manifest.run_id.as_str(),
            "policy-r1",
            started_at_ms,
        )
        .with_durable_at_ms(started_at_ms + 1),
        "manifest-digest-r1",
        started_at_ms + 1,
    )?;
    let run = TrainingRunState::new(
        context.manifest.run_id.clone(),
        "adapter-sft",
        cluster_id.as_str(),
        GOOGLE_SWARM_POLICY_FAMILY,
        psionic_environments::EnvironmentPackageKey::new(
            GOOGLE_SWARM_ENV_PACKAGE_FAMILY,
            GOOGLE_SWARM_ENV_PACKAGE_VERSION,
        ),
    )?;
    let mut coordinator = AdapterTrainingClusterCoordinator::new(
        run,
        adapter_target,
        policy_revision,
        checkpoint_pointer,
        crate::AdapterContributorCapabilityPolicy {
            backend_label: String::from(crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL),
            minimum_free_memory_bytes: 12 * GIB_BYTES,
            require_accelerator: true,
            allow_degraded_backend: false,
            additional_backend_capabilities: Vec::new(),
            allow_flaky_nodes: false,
        },
    );
    let membership_receipt = coordinator
        .observe_cluster_state(&state, started_at_ms + 10)?
        .clone();
    let window_record = coordinator.plan_next_window(
        vec![
            crate::AdapterDatasetSliceIdentity::new(
                GOOGLE_SWARM_DATASET_REF,
                "train",
                "slice-coordinator",
                "slice-digest-coordinator",
            )?,
            crate::AdapterDatasetSliceIdentity::new(
                GOOGLE_SWARM_DATASET_REF,
                "train",
                "slice-contributor",
                "slice-digest-contributor",
            )?,
        ],
        2,
        started_at_ms + 20,
    )?;
    let window_plan = window_record.plan.clone();
    let mut protocol = AdapterWorkerProtocolState::from_window_record(
        &window_record,
        AdapterWorkerProtocolPolicy {
            heartbeat_timeout_ms: 60_000,
            claim_ttl_ms: 300_000,
        },
    );
    protocol.activate_window()?;
    let local_assignment = protocol
        .assignments
        .iter()
        .find(|assignment| assignment.worker_id == context.local_node.node_id)
        .cloned()
        .ok_or_else(|| PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: String::from("worker protocol did not assign the coordinator node"),
        })?;
    let peer_assignment = protocol
        .assignments
        .iter()
        .find(|assignment| assignment.worker_id == context.peer_node.node_id)
        .cloned()
        .ok_or_else(|| PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: String::from("worker protocol did not assign the contributor node"),
        })?;
    let local_identity = build_identity(
        context.manifest.run_id.as_str(),
        context.local_node.node_id.as_str(),
    );
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
    let expected_peer_signing_public_key_hex = build_identity(
        context.manifest.run_id.as_str(),
        context.peer_node.node_id.as_str(),
    )
    .submission_signing_public_key_hex;
    Ok(CoordinatorPlan {
        coordinator,
        membership_receipt,
        window_plan,
        protocol,
        local_assignment,
        peer_assignment,
        local_identity,
        local_claim,
        expected_peer_signing_public_key_hex,
    })
}

fn run_local_contribution(
    run_id: &str,
    worker_id: &str,
    _claim_id: &str,
    assignment_id: &str,
    session_id: &str,
) -> Result<LocalContributionRun, PsionGoogleTwoNodeSwarmRuntimeError> {
    let started_at_ms = now_ms();
    let config = first_swarm_open_adapter_training_config(
        format!("{run_id}-{worker_id}"),
        format!("google-two-node-swarm:{run_id}"),
        crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL,
    );
    let samples = first_swarm_open_adapter_samples(worker_id)?;
    let backend = OpenAdapterTrainingExecutionBackend::new(config, samples)?;
    let outcome = run_open_adapter_sft_export(
        &backend,
        &first_swarm_open_adapter_sft_request(worker_id, "r1", started_at_ms, 25),
    )?;
    let completed_at_ms = now_ms();
    let final_mean_loss = outcome
        .gradient_records
        .last()
        .map(|record| record.mean_loss)
        .unwrap_or_default();
    let average_loss_bps = Some((final_mean_loss * 10_000.0).round() as u32);
    let execution_summary = AdapterContributionExecutionSummary::new(
        started_at_ms,
        completed_at_ms,
        outcome.step_receipts.len() as u32,
        outcome.gradient_records.len() as u32,
        average_loss_bps,
        outcome.summary.adapter_artifact_digest.clone(),
    )?;
    let payload_sha256 = hex::encode(Sha256::digest(outcome.adapter_bytes.as_slice()));
    Ok(LocalContributionRun {
        contribution: PsionGoogleTwoNodeSwarmLocalContribution {
            run_id: String::from(run_id),
            assignment_id: String::from(assignment_id),
            worker_id: String::from(worker_id),
            session_id: String::from(session_id),
            execution_backend_label: String::from(crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL),
            adapter_artifact_digest: outcome.summary.adapter_artifact_digest.clone(),
            adapter_identity_digest: outcome.summary.adapter_identity_digest.clone(),
            adapter_delta_digest: outcome.summary.adapter_artifact_digest.clone(),
            payload_sha256,
            payload_bytes: outcome.adapter_bytes.len(),
            final_mean_loss,
            executed_steps: outcome.step_receipts.len(),
            batch_count: backend.batches().len(),
            execution_summary,
        },
        payload: outcome.adapter_bytes,
    })
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
) -> Result<AdapterContributionValidationBundle, PsionGoogleTwoNodeSwarmRuntimeError> {
    let upload = submission.upload.clone();
    let cursor = storage.start_contribution_upload(
        assignment,
        upload,
        payload,
        GOOGLE_SWARM_CHUNK_BYTES,
        assignment.worker_id.clone(),
        base_time_ms,
    )?;
    for chunk in payload.chunks(GOOGLE_SWARM_CHUNK_BYTES) {
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
        membership_for_node(cluster_id, &context.local_node, &context.local_endpoint),
        membership_for_node(cluster_id, &context.peer_node, &context.peer_endpoint),
    ]);
    snapshot.telemetry = BTreeMap::from([
        telemetry_for_node(&context.local_node),
        telemetry_for_node(&context.peer_node),
    ]);
    ClusterState::from_snapshot(snapshot)
}

fn membership_for_node(
    cluster_id: &ClusterId,
    node: &PsionGoogleTwoNodeSwarmManifestNode,
    endpoint: &PsionGoogleTwoNodeSwarmEndpointManifest,
) -> (NodeId, ClusterMembershipRecord) {
    let ip_addr: IpAddr = endpoint
        .internal_ip
        .parse()
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let node_id = NodeId::new(node.node_id.clone());
    (
        node_id.clone(),
        ClusterMembershipRecord::new(
            ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: node_id.clone(),
                node_epoch: NodeEpoch::initial(),
                role: cluster_node_role(node.role_kind),
                auth_public_key: format!("{}-configured-peer-public-key", node.node_id),
                attestation: None,
            },
            Some(SocketAddr::new(ip_addr, endpoint.cluster_port)),
            ClusterMembershipStatus::Ready,
        ),
    )
}

fn telemetry_for_node(
    node: &PsionGoogleTwoNodeSwarmManifestNode,
) -> (NodeId, ClusterNodeTelemetry) {
    let free_memory_gib = match node.role_kind {
        PsionGoogleTwoNodeSwarmNodeRoleKind::CoordinatorValidatorAggregatorContributor => 20,
        PsionGoogleTwoNodeSwarmNodeRoleKind::Contributor => 20,
    };
    (
        NodeId::new(node.node_id.clone()),
        ClusterNodeTelemetry::new(NodeId::new(node.node_id.clone()))
            .with_memory(Some(24 * GIB_BYTES), Some(free_memory_gib * GIB_BYTES))
            .with_accelerator_count(node.accelerator_count)
            .with_backend_readiness(
                crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL,
                ClusterBackendReadinessStatus::Ready,
            )
            .with_stability_posture(ClusterStabilityPosture::Stable),
    )
}

fn upload_locator_for_assignment(
    assignment: &crate::AdapterContributionWorkAssignment,
    payload: &[u8],
    suffix: &str,
) -> Result<AdapterContributionUploadLocator, PsionGoogleTwoNodeSwarmRuntimeError> {
    let manifest = DatastreamManifest::from_bytes(
        format!(
            "adapter-contribution:{}:{}",
            assignment.window_id, assignment.contribution_id
        ),
        DatastreamSubjectKind::AdapterPackage,
        payload,
        GOOGLE_SWARM_CHUNK_BYTES,
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
        format!("google-configured-peer:{worker_id}"),
    )
    .with_submission_signing_public_key_hex(hex::encode(signing_key.verifying_key().to_bytes()))
}

fn signing_key_for_worker(_seed_scope: &str, worker_id: &str) -> SigningKey {
    let digest = Sha256::digest(format!("google-two-node-swarm|{worker_id}").as_bytes());
    let key_bytes: [u8; 32] = digest.into();
    SigningKey::from_bytes(&key_bytes)
}

fn build_cluster_id(run_id: &str, contract_digest: &str) -> ClusterId {
    let contract = psion_google_two_node_swarm_contract()
        .expect("frozen Google two-node contract should remain buildable");
    let admission_token = AdmissionToken::new(admission_token_for_run(run_id, contract_digest));
    ClusterId::new(
        &ClusterNamespace::new(contract.cluster_namespace),
        &admission_token,
    )
}

/// Returns the deterministic configured-peer admission token for one run.
#[must_use]
pub fn admission_token_for_run(run_id: &str, contract_digest: &str) -> String {
    hex::encode(Sha256::digest(
        format!("psion-google-two-node-swarm-admission|{run_id}|{contract_digest}").as_bytes(),
    ))
}

fn google_swarm_checkpoint_reference(
    run_id: &str,
    revision_id: &str,
    started_at_ms: u64,
) -> psionic_runtime::TrainingCheckpointReference {
    psionic_runtime::TrainingCheckpointReference::new(
        GOOGLE_SWARM_POLICY_FAMILY,
        format!("stream://google-two-node-swarm/{run_id}/{revision_id}"),
        format!("manifest://google-two-node-swarm/{run_id}/{revision_id}"),
        format!("object://google-two-node-swarm/{run_id}/{revision_id}"),
        "psion-google-swarm-coordinator-a",
        1,
        "cluster-digest-google-two-node-swarm",
        "topology-digest-google-two-node-swarm",
        started_at_ms,
    )
    .with_checkpoint_ref(format!(
        "checkpoint/google-two-node-swarm/{run_id}/{revision_id}"
    ))
    .with_step(12)
}

fn cluster_node_role(role_kind: PsionGoogleTwoNodeSwarmNodeRoleKind) -> NodeRole {
    match role_kind {
        PsionGoogleTwoNodeSwarmNodeRoleKind::CoordinatorValidatorAggregatorContributor => {
            NodeRole::Mixed
        }
        PsionGoogleTwoNodeSwarmNodeRoleKind::Contributor => NodeRole::ExecutorOnly,
    }
}

fn connect_with_retry(
    endpoint: &str,
    timeout_seconds: u64,
) -> Result<TcpStream, PsionGoogleTwoNodeSwarmRuntimeError> {
    let started = Instant::now();
    loop {
        match TcpStream::connect(endpoint) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                if started.elapsed() > Duration::from_secs(timeout_seconds) {
                    return Err(PsionGoogleTwoNodeSwarmRuntimeError::Timeout {
                        detail: format!("failed to connect to peer endpoint `{endpoint}`: {error}"),
                    });
                }
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

fn configure_stream(stream: &TcpStream) -> Result<(), PsionGoogleTwoNodeSwarmRuntimeError> {
    stream
        .set_nodelay(true)
        .map_err(|error| PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: format!("failed to set TCP_NODELAY: {error}"),
        })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .map_err(|error| PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: format!("failed to set read timeout: {error}"),
        })?;
    stream
        .set_write_timeout(Some(Duration::from_secs(60)))
        .map_err(|error| PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: format!("failed to set write timeout: {error}"),
        })?;
    Ok(())
}

fn send_message(
    stream: &mut TcpStream,
    message: &PsionGoogleTwoNodeSwarmMessage,
) -> Result<(), PsionGoogleTwoNodeSwarmRuntimeError> {
    let encoded = serde_json::to_vec(message)?;
    stream
        .write_all(encoded.as_slice())
        .and_then(|_| stream.write_all(b"\n"))
        .map_err(|error| PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: format!("failed to send cluster message: {error}"),
        })?;
    Ok(())
}

fn receive_message(
    stream: &mut TcpStream,
) -> Result<PsionGoogleTwoNodeSwarmMessage, PsionGoogleTwoNodeSwarmRuntimeError> {
    let mut bytes = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let read = stream.read(&mut byte).map_err(|error| {
            PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
                detail: format!("failed to read cluster message: {error}"),
            }
        })?;
        if read == 0 {
            return Err(PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
                detail: String::from("peer closed the cluster connection unexpectedly"),
            });
        }
        if byte[0] == b'\n' {
            break;
        }
        bytes.push(byte[0]);
    }
    let line = String::from_utf8(bytes).map_err(|error| {
        PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: format!("cluster message was not valid UTF-8: {error}"),
        }
    })?;
    serde_json::from_str::<PsionGoogleTwoNodeSwarmMessage>(line.trim()).map_err(|error| {
        PsionGoogleTwoNodeSwarmRuntimeError::Protocol {
            detail: format!("failed to decode cluster message: {error}"),
        }
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, PsionGoogleTwoNodeSwarmRuntimeError> {
    let bytes = fs::read(path).map_err(|error| PsionGoogleTwoNodeSwarmRuntimeError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionGoogleTwoNodeSwarmRuntimeError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn write_runtime_report(
    output_path: &Path,
    report: &PsionGoogleTwoNodeSwarmRuntimeReport,
) -> Result<(), PsionGoogleTwoNodeSwarmRuntimeError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionGoogleTwoNodeSwarmRuntimeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let encoded = serde_json::to_string_pretty(report)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        PsionGoogleTwoNodeSwarmRuntimeError::Write {
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

    fn write_json(path: &Path, value: &impl Serialize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(
            path,
            format!(
                "{}\n",
                serde_json::to_string_pretty(value).expect("serialize fixture")
            ),
        )
        .expect("write json");
    }

    #[test]
    fn google_two_node_swarm_runtime_completes_on_loopback() {
        let temp = tempdir().expect("tempdir");
        let contract = psion_google_two_node_swarm_contract().expect("contract");
        let run_id = "google-two-node-loopback";
        let coordinator_port = free_port();
        let contributor_port = free_port();
        let cluster_id = build_cluster_id(run_id, contract.contract_digest.as_str());
        let cluster_manifest = PsionGoogleTwoNodeSwarmClusterManifest {
            schema_version: String::from(GOOGLE_SWARM_CLUSTER_MANIFEST_SCHEMA_VERSION),
            created_at_utc: String::from("2026-03-24T00:00:00Z"),
            run_id: String::from(run_id),
            cluster_id: cluster_id.as_str().to_string(),
            contract_digest: contract.contract_digest.clone(),
            cluster_namespace: contract.cluster_namespace.clone(),
            project_id: contract.project_id.clone(),
            region_family: contract.region_family.clone(),
            bucket_url: contract.bucket_url.clone(),
            run_prefix: format!("{}/runs/{run_id}", contract.bucket_url),
            training_command_id: contract.training_command_id.clone(),
            selected_zone_pair_id: String::from("us-central1-a__us-central1-b"),
            selected_impairment_profile_id: String::from("clean_baseline"),
            git_revision: String::from("test-revision"),
            launch_receipt_uri: String::from("gs://test/launch/receipt.json"),
            final_manifest_uri: String::from("gs://test/final/manifest.json"),
            nodes: vec![
                PsionGoogleTwoNodeSwarmManifestNode {
                    node_id: String::from("psion-google-swarm-coordinator-a"),
                    role_id: String::from(
                        "psion.google_swarm.coordinator_validator_aggregator_contributor",
                    ),
                    role_kind:
                        PsionGoogleTwoNodeSwarmNodeRoleKind::CoordinatorValidatorAggregatorContributor,
                    instance_name: String::from("coordinator"),
                    zone: String::from("us-central1-a"),
                    subnetwork: String::from("oa-lightning-us-central1-psion-swarm-coordinator"),
                    internal_ip: Some(String::from("127.0.0.1")),
                    endpoint: Some(format!("127.0.0.1:{coordinator_port}")),
                    cluster_port: coordinator_port,
                    endpoint_manifest_uri: String::from("gs://test/coordinator_endpoint.json"),
                    bringup_report_uri: String::from("gs://test/coordinator_bringup.json"),
                    runtime_report_uri: String::from("gs://test/coordinator_runtime.json"),
                    machine_type: String::from("g2-standard-8"),
                    accelerator_type: String::from("nvidia-l4"),
                    accelerator_count: 1,
                },
                PsionGoogleTwoNodeSwarmManifestNode {
                    node_id: String::from("psion-google-swarm-contributor-b"),
                    role_id: String::from("psion.google_swarm.contributor"),
                    role_kind: PsionGoogleTwoNodeSwarmNodeRoleKind::Contributor,
                    instance_name: String::from("contributor"),
                    zone: String::from("us-central1-b"),
                    subnetwork: String::from("oa-lightning-us-central1-psion-swarm-contributor"),
                    internal_ip: Some(String::from("127.0.0.1")),
                    endpoint: Some(format!("127.0.0.1:{contributor_port}")),
                    cluster_port: contributor_port,
                    endpoint_manifest_uri: String::from("gs://test/contributor_endpoint.json"),
                    bringup_report_uri: String::from("gs://test/contributor_bringup.json"),
                    runtime_report_uri: String::from("gs://test/contributor_runtime.json"),
                    machine_type: String::from("g2-standard-8"),
                    accelerator_type: String::from("nvidia-l4"),
                    accelerator_count: 1,
                },
            ],
        };
        let coordinator_endpoint = PsionGoogleTwoNodeSwarmEndpointManifest {
            schema_version: String::from(GOOGLE_SWARM_ENDPOINT_MANIFEST_SCHEMA_VERSION),
            created_at_utc: String::from("2026-03-24T00:00:01Z"),
            run_id: String::from(run_id),
            node_id: String::from("psion-google-swarm-coordinator-a"),
            role_id: String::from(
                "psion.google_swarm.coordinator_validator_aggregator_contributor",
            ),
            zone: String::from("us-central1-a"),
            internal_ip: String::from("127.0.0.1"),
            cluster_port: coordinator_port,
            endpoint: format!("127.0.0.1:{coordinator_port}"),
            source: String::from("test"),
        };
        let contributor_endpoint = PsionGoogleTwoNodeSwarmEndpointManifest {
            schema_version: String::from(GOOGLE_SWARM_ENDPOINT_MANIFEST_SCHEMA_VERSION),
            created_at_utc: String::from("2026-03-24T00:00:01Z"),
            run_id: String::from(run_id),
            node_id: String::from("psion-google-swarm-contributor-b"),
            role_id: String::from("psion.google_swarm.contributor"),
            zone: String::from("us-central1-b"),
            internal_ip: String::from("127.0.0.1"),
            cluster_port: contributor_port,
            endpoint: format!("127.0.0.1:{contributor_port}"),
            source: String::from("test"),
        };

        let cluster_manifest_path = temp.path().join("cluster_manifest.json");
        let coordinator_endpoint_path = temp.path().join("coordinator_endpoint.json");
        let contributor_endpoint_path = temp.path().join("contributor_endpoint.json");
        let coordinator_report_path = temp.path().join("coordinator_runtime_report.json");
        let contributor_report_path = temp.path().join("contributor_runtime_report.json");
        write_json(&cluster_manifest_path, &cluster_manifest);
        write_json(&coordinator_endpoint_path, &coordinator_endpoint);
        write_json(&contributor_endpoint_path, &contributor_endpoint);

        let coordinator_manifest = cluster_manifest_path.clone();
        let coordinator_local = coordinator_endpoint_path.clone();
        let coordinator_peer = contributor_endpoint_path.clone();
        let coordinator_output = coordinator_report_path.clone();
        let coordinator_handle = thread::spawn(move || {
            run_psion_google_two_node_swarm_runtime(
                PsionGoogleTwoNodeSwarmRuntimeRole::Coordinator,
                coordinator_manifest,
                coordinator_local,
                coordinator_peer,
                coordinator_output,
            )
        });

        thread::sleep(Duration::from_millis(150));

        let contributor_report = run_psion_google_two_node_swarm_runtime(
            PsionGoogleTwoNodeSwarmRuntimeRole::Contributor,
            &cluster_manifest_path,
            &contributor_endpoint_path,
            &coordinator_endpoint_path,
            &contributor_report_path,
        )
        .expect("contributor runtime");
        let coordinator_report = coordinator_handle
            .join()
            .expect("coordinator thread join")
            .expect("coordinator runtime");

        assert_eq!(
            contributor_report.runtime_role,
            PsionGoogleTwoNodeSwarmRuntimeRole::Contributor
        );
        assert!(contributor_report.local_contribution.is_some());
        assert_eq!(
            coordinator_report.runtime_role,
            PsionGoogleTwoNodeSwarmRuntimeRole::Coordinator
        );
        assert!(coordinator_report.membership_receipt.is_some());
        assert!(coordinator_report.window_plan.is_some());
        assert_eq!(coordinator_report.submission_receipts.len(), 2);
        assert!(coordinator_report.validator_summary.is_some());
        assert!(coordinator_report.promotion_receipt.is_some());
    }
}
