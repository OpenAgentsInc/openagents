use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    pylon_bitcoin_payout_target_for_job, run_qwen_legal_pylon_worker_job,
    PylonLocalWorkerRunOptions, PylonTrainingArtifactRef, PylonTrainingExpectedOutputArtifact,
    PylonTrainingHardwareRequirements, PylonTrainingJobKind, PylonTrainingJobSpec,
    PylonTrainingOutputArtifactRef, PylonTrainingPaymentBudget, PylonTrainingPaymentStatus,
    PylonTrainingReceiptRequirements, PylonTrainingShardAssignment, PylonTrainingWorkerJobStatus,
    PylonTrainingWorkerReceipt, QwenLegalPylonTrainingJobError,
    QWEN_LEGAL_PYLON_TRAINING_JOB_SCHEMA_VERSION,
};

pub const QWEN_LEGAL_PYLON_DISPATCH_REPORT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_pylon_dispatch_report.v1";
pub const QWEN_LEGAL_PYLON_JOB_ENVELOPE_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_pylon_signed_job_envelope.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalPylonDispatchMode {
    LocalOnly,
    Loopback,
    Tailnet,
    Production,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalPylonDispatchStatus {
    Completed,
    CompletedWithBlocks,
    Blocked,
    Partial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalPylonDispatchRetryDecisionKind {
    NotNeeded,
    Reassigned,
    BlockedNoEligibleNode,
    BlockedModeNotImplemented,
    TimedOut,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPylonNodeDispatchCapability {
    pub node_id: String,
    pub backend_label: String,
    pub network_addr: String,
    pub host_memory_bytes: u64,
    pub accelerator_memory_bytes: u64,
    pub model_cached: bool,
    pub allowed_job_types: Vec<PylonTrainingJobKind>,
    pub payment_target_ref: String,
    pub trust_state: String,
    pub authenticated: bool,
    pub online: bool,
}

impl QwenLegalPylonNodeDispatchCapability {
    fn is_eligible_for(&self, job: &PylonTrainingJobSpec) -> bool {
        self.online
            && self.authenticated
            && self.trust_state == "trusted"
            && self.allowed_job_types.contains(&job.job_kind)
            && self.available_memory_bytes() >= job.hardware_requirements.min_memory_bytes
            && job
                .hardware_requirements
                .accepted_backend_labels
                .iter()
                .any(|label| label == &self.backend_label)
    }

    fn available_memory_bytes(&self) -> u64 {
        if self.accelerator_memory_bytes > 0 {
            self.accelerator_memory_bytes
        } else {
            self.host_memory_bytes
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalSignedJobEnvelope {
    pub schema_version: String,
    pub envelope_id: String,
    pub dispatch_mode: QwenLegalPylonDispatchMode,
    pub target_node_id: String,
    pub target_network_addr: String,
    pub job: PylonTrainingJobSpec,
    pub job_spec_digest: String,
    pub signed_payload_digest: String,
    pub scheduler_pubkey_hex: String,
    pub signature_hex: String,
    pub envelope_digest: String,
}

impl QwenLegalSignedJobEnvelope {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.envelope_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_signed_job_envelope|", &clone)
    }

    #[must_use]
    pub fn signable_payload_digest(&self) -> String {
        let mut clone = self.clone();
        clone.signed_payload_digest.clear();
        clone.scheduler_pubkey_hex.clear();
        clone.signature_hex.clear();
        clone.envelope_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_signed_job_envelope_payload|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPylonDispatchAssignment {
    pub assignment_id: String,
    pub job_id: String,
    pub job_kind: PylonTrainingJobKind,
    pub node_id: String,
    pub network_addr: String,
    pub envelope_digest: String,
    pub job_spec_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPylonDispatchRetryDecision {
    pub job_id: String,
    pub attempt: u32,
    pub decision: QwenLegalPylonDispatchRetryDecisionKind,
    pub reason: String,
    pub assigned_node_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPylonDispatchPaymentDecision {
    pub job_id: String,
    pub worker_id: String,
    pub payment_status: PylonTrainingPaymentStatus,
    pub agreed_price_microusd: u64,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPylonTransportEvent {
    pub job_id: String,
    pub target_node_id: String,
    pub transport: String,
    pub endpoint: String,
    pub request_digest: String,
    pub request_bytes: u64,
    pub response_digest: String,
    pub response_bytes: u64,
    pub scheduler_signature_verified: bool,
    pub worker_signature_present: bool,
    pub worker_receipt_digest: String,
    pub worker_receipt_verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPylonBlockedNode {
    pub node_id: String,
    pub network_addr: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPylonDispatchReport {
    pub schema_version: String,
    pub run_id: String,
    pub mode: QwenLegalPylonDispatchMode,
    pub status: QwenLegalPylonDispatchStatus,
    pub assignments: Vec<QwenLegalPylonDispatchAssignment>,
    pub retry_decisions: Vec<QwenLegalPylonDispatchRetryDecision>,
    pub blocked_nodes: Vec<QwenLegalPylonBlockedNode>,
    pub job_envelopes: Vec<QwenLegalSignedJobEnvelope>,
    pub worker_receipts: Vec<PylonTrainingWorkerReceipt>,
    pub payment_decisions: Vec<QwenLegalPylonDispatchPaymentDecision>,
    pub artifact_hashes: Vec<PylonTrainingOutputArtifactRef>,
    pub transport_events: Vec<QwenLegalPylonTransportEvent>,
    pub duplicate_successful_shards: Vec<String>,
    pub report_digest: String,
}

impl QwenLegalPylonDispatchReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_pylon_dispatch_report|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QwenLegalPylonDispatchRequest {
    pub run_id: String,
    pub mode: QwenLegalPylonDispatchMode,
    pub nodes: Vec<QwenLegalPylonNodeDispatchCapability>,
    pub jobs: Vec<PylonTrainingJobSpec>,
    pub output_dir: PathBuf,
    pub max_retries: u32,
}

#[derive(Debug, Error)]
pub enum QwenLegalPylonDispatchError {
    #[error("Qwen legal Pylon dispatch request is invalid: {detail}")]
    InvalidRequest { detail: String },
    #[error("Qwen legal Pylon worker failed: {0}")]
    Worker(#[from] QwenLegalPylonTrainingJobError),
    #[error("Qwen legal Pylon dispatch I/O failed at `{path}`: {message}")]
    Io { path: String, message: String },
    #[error("Qwen legal Pylon dispatch serialization failed: {message}")]
    Serialization { message: String },
}

pub fn dispatch_qwen_legal_pylon_jobs(
    request: &QwenLegalPylonDispatchRequest,
) -> Result<QwenLegalPylonDispatchReport, QwenLegalPylonDispatchError> {
    validate_dispatch_request(request)?;
    fs::create_dir_all(request.output_dir.as_path()).map_err(|error| {
        QwenLegalPylonDispatchError::Io {
            path: request.output_dir.display().to_string(),
            message: error.to_string(),
        }
    })?;

    let mut assignments = Vec::new();
    let mut retry_decisions = Vec::new();
    let mut blocked_nodes = blocked_node_reports(&request.nodes);
    let mut job_envelopes = Vec::new();
    let mut worker_receipts = Vec::new();
    let mut artifact_hashes = Vec::new();
    let mut payment_decisions = Vec::new();
    let mut transport_events = Vec::new();
    let mut duplicate_successful_shards = Vec::new();
    let mut paid_credit_keys = BTreeSet::new();
    let mut next_node_index = 0_usize;

    for (job_index, job) in request.jobs.iter().enumerate() {
        let eligible_nodes = request
            .nodes
            .iter()
            .filter(|node| node.is_eligible_for(job))
            .collect::<Vec<_>>();
        if eligible_nodes.is_empty() {
            retry_decisions.push(QwenLegalPylonDispatchRetryDecision {
                job_id: job.job_id.clone(),
                attempt: 1,
                decision: QwenLegalPylonDispatchRetryDecisionKind::BlockedNoEligibleNode,
                reason: String::from("no online authenticated trusted node accepts this job"),
                assigned_node_id: None,
            });
            continue;
        }
        let selected = eligible_nodes[next_node_index % eligible_nodes.len()];
        next_node_index = next_node_index.saturating_add(1);
        retry_decisions.push(QwenLegalPylonDispatchRetryDecision {
            job_id: job.job_id.clone(),
            attempt: 1,
            decision: QwenLegalPylonDispatchRetryDecisionKind::NotNeeded,
            reason: String::from("assigned on first eligible attempt"),
            assigned_node_id: Some(selected.node_id.clone()),
        });

        let prepared_job = prepare_job_for_dispatch(request, job, selected, job_index)?;
        let envelope = signed_job_envelope(&request.run_id, request.mode, selected, prepared_job);
        verify_signed_job_envelope(&envelope)?;
        write_json(
            request
                .output_dir
                .join("envelopes")
                .join(format!("{}.envelope.json", envelope.job.job_id))
                .as_path(),
            &envelope,
        )?;
        let (receipt, transport_event) =
            run_dispatched_worker(request.mode, selected, &envelope, job_index)?;
        artifact_hashes.extend(receipt.output_hashes.clone());
        let credit_key = shard_credit_key(&envelope.job);
        let payment = payment_decision_for_receipt(
            &receipt,
            credit_key,
            &mut paid_credit_keys,
            &mut duplicate_successful_shards,
        );
        assignments.push(QwenLegalPylonDispatchAssignment {
            assignment_id: format!("assignment.{}.{}", request.run_id, job_index + 1),
            job_id: envelope.job.job_id.clone(),
            job_kind: envelope.job.job_kind,
            node_id: selected.node_id.clone(),
            network_addr: selected.network_addr.clone(),
            envelope_digest: envelope.envelope_digest.clone(),
            job_spec_digest: envelope.job_spec_digest.clone(),
        });
        job_envelopes.push(envelope);
        worker_receipts.push(receipt);
        transport_events.push(transport_event);
        payment_decisions.push(payment);
    }

    blocked_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let status = dispatch_status(request.jobs.len(), assignments.len(), blocked_nodes.len());
    let mut report = QwenLegalPylonDispatchReport {
        schema_version: String::from(QWEN_LEGAL_PYLON_DISPATCH_REPORT_SCHEMA_VERSION),
        run_id: request.run_id.clone(),
        mode: request.mode,
        status,
        assignments,
        retry_decisions,
        blocked_nodes,
        job_envelopes,
        worker_receipts,
        payment_decisions,
        artifact_hashes,
        transport_events,
        duplicate_successful_shards,
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    write_json(
        request.output_dir.join("dispatch_report.json").as_path(),
        &report,
    )?;
    Ok(report)
}

fn run_dispatched_worker(
    mode: QwenLegalPylonDispatchMode,
    node: &QwenLegalPylonNodeDispatchCapability,
    envelope: &QwenLegalSignedJobEnvelope,
    job_index: usize,
) -> Result<(PylonTrainingWorkerReceipt, QwenLegalPylonTransportEvent), QwenLegalPylonDispatchError>
{
    let options = PylonLocalWorkerRunOptions {
        worker_id: node.node_id.clone(),
        started_at_ms: 100_000 + u64::try_from(job_index).unwrap_or(0) * 1_000,
        emit_outputs: true,
    };
    match mode {
        QwenLegalPylonDispatchMode::LocalOnly => {
            let receipt = run_qwen_legal_pylon_worker_job(&envelope.job, &options)?;
            verify_worker_receipt_signature(&receipt)?;
            let request_bytes = serde_json::to_vec(envelope).map_err(|error| {
                QwenLegalPylonDispatchError::Serialization {
                    message: error.to_string(),
                }
            })?;
            let response_bytes = serde_json::to_vec(&receipt).map_err(|error| {
                QwenLegalPylonDispatchError::Serialization {
                    message: error.to_string(),
                }
            })?;
            let event = transport_event(
                envelope,
                node,
                "local_only",
                node.network_addr.as_str(),
                request_bytes.len(),
                sha256_hex(response_bytes.as_slice()).as_str(),
                response_bytes.len(),
                true,
                !receipt.signature_hex.is_empty(),
                receipt.receipt_digest.as_str(),
                true,
            );
            Ok((receipt, event))
        }
        QwenLegalPylonDispatchMode::Loopback => run_loopback_tcp_worker(node, envelope, options),
        QwenLegalPylonDispatchMode::Tailnet => {
            run_remote_tcp_worker(node, envelope, "tailnet_tcp_signed_envelope")
        }
        QwenLegalPylonDispatchMode::Production => {
            run_remote_tcp_worker(node, envelope, "production_tcp_signed_envelope")
        }
    }
}

fn run_loopback_tcp_worker(
    node: &QwenLegalPylonNodeDispatchCapability,
    envelope: &QwenLegalSignedJobEnvelope,
    options: PylonLocalWorkerRunOptions,
) -> Result<(PylonTrainingWorkerReceipt, QwenLegalPylonTransportEvent), QwenLegalPylonDispatchError>
{
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|error| QwenLegalPylonDispatchError::Io {
            path: String::from("tcp://127.0.0.1:0"),
            message: error.to_string(),
        })?;
    let endpoint = listener
        .local_addr()
        .map_err(|error| QwenLegalPylonDispatchError::Io {
            path: String::from("tcp://127.0.0.1:0"),
            message: error.to_string(),
        })?
        .to_string();
    let worker = thread::spawn(move || -> Result<Vec<u8>, QwenLegalPylonDispatchError> {
        let (mut inbound, _) =
            listener
                .accept()
                .map_err(|error| QwenLegalPylonDispatchError::Io {
                    path: String::from("loopback tcp accept"),
                    message: error.to_string(),
                })?;
        let mut request_bytes = Vec::new();
        inbound.read_to_end(&mut request_bytes).map_err(|error| {
            QwenLegalPylonDispatchError::Io {
                path: String::from("loopback tcp read"),
                message: error.to_string(),
            }
        })?;
        let envelope = serde_json::from_slice::<QwenLegalSignedJobEnvelope>(&request_bytes)
            .map_err(|error| QwenLegalPylonDispatchError::Serialization {
                message: error.to_string(),
            })?;
        verify_signed_job_envelope(&envelope)?;
        let receipt = run_qwen_legal_pylon_worker_job(&envelope.job, &options)?;
        verify_worker_receipt_signature(&receipt)?;
        serde_json::to_vec(&receipt).map_err(|error| QwenLegalPylonDispatchError::Serialization {
            message: error.to_string(),
        })
    });

    let mut stream =
        TcpStream::connect(endpoint.as_str()).map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{endpoint}"),
            message: error.to_string(),
        })?;
    let request_bytes = serde_json::to_vec(envelope).map_err(|error| {
        QwenLegalPylonDispatchError::Serialization {
            message: error.to_string(),
        }
    })?;
    stream
        .write_all(request_bytes.as_slice())
        .map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{endpoint}"),
            message: error.to_string(),
        })?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{endpoint}"),
            message: error.to_string(),
        })?;
    let response_bytes =
        worker
            .join()
            .map_err(|_| QwenLegalPylonDispatchError::InvalidRequest {
                detail: String::from("loopback tcp worker thread panicked"),
            })??;
    let receipt =
        serde_json::from_slice::<PylonTrainingWorkerReceipt>(&response_bytes).map_err(|error| {
            QwenLegalPylonDispatchError::Serialization {
                message: error.to_string(),
            }
        })?;
    verify_worker_receipt_signature(&receipt)?;
    let event = transport_event(
        envelope,
        node,
        "loopback_tcp",
        endpoint.as_str(),
        request_bytes.len(),
        sha256_hex(response_bytes.as_slice()).as_str(),
        response_bytes.len(),
        true,
        !receipt.signature_hex.is_empty(),
        receipt.receipt_digest.as_str(),
        true,
    );
    Ok((receipt, event))
}

fn run_remote_tcp_worker(
    node: &QwenLegalPylonNodeDispatchCapability,
    envelope: &QwenLegalSignedJobEnvelope,
    transport: &str,
) -> Result<(PylonTrainingWorkerReceipt, QwenLegalPylonTransportEvent), QwenLegalPylonDispatchError>
{
    let endpoint = tcp_endpoint(node.network_addr.as_str())?;
    let request_bytes = serde_json::to_vec(envelope).map_err(|error| {
        QwenLegalPylonDispatchError::Serialization {
            message: error.to_string(),
        }
    })?;
    let mut stream =
        TcpStream::connect(endpoint.as_str()).map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{endpoint}"),
            message: error.to_string(),
        })?;
    stream
        .write_all(request_bytes.as_slice())
        .map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{endpoint}"),
            message: error.to_string(),
        })?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{endpoint}"),
            message: error.to_string(),
        })?;
    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{endpoint}"),
            message: error.to_string(),
        })?;
    let receipt =
        serde_json::from_slice::<PylonTrainingWorkerReceipt>(&response_bytes).map_err(|error| {
            QwenLegalPylonDispatchError::Serialization {
                message: error.to_string(),
            }
        })?;
    verify_worker_receipt_signature(&receipt)?;
    let event = transport_event(
        envelope,
        node,
        transport,
        endpoint.as_str(),
        request_bytes.len(),
        sha256_hex(response_bytes.as_slice()).as_str(),
        response_bytes.len(),
        true,
        !receipt.signature_hex.is_empty(),
        receipt.receipt_digest.as_str(),
        true,
    );
    Ok((receipt, event))
}

/// Serves one signed Qwen legal Pylon worker job over TCP.
///
/// This is the worker-side protocol used by `Tailnet` and `Production`
/// dispatch modes. The worker verifies the scheduler envelope signature before
/// materializing outputs, signs the worker receipt, writes it back over the
/// same TCP connection, and then exits.
pub fn serve_qwen_legal_pylon_tcp_worker_once(
    bind_addr: &str,
    options: PylonLocalWorkerRunOptions,
) -> Result<(), QwenLegalPylonDispatchError> {
    let endpoint = tcp_endpoint(bind_addr)?;
    let listener =
        TcpListener::bind(endpoint.as_str()).map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{endpoint}"),
            message: error.to_string(),
        })?;
    serve_qwen_legal_pylon_tcp_worker_listener_once(listener, options)
}

fn serve_qwen_legal_pylon_tcp_worker_listener_once(
    listener: TcpListener,
    options: PylonLocalWorkerRunOptions,
) -> Result<(), QwenLegalPylonDispatchError> {
    let (mut inbound, peer) =
        listener
            .accept()
            .map_err(|error| QwenLegalPylonDispatchError::Io {
                path: String::from("remote tcp accept"),
                message: error.to_string(),
            })?;
    let mut request_bytes = Vec::new();
    inbound
        .read_to_end(&mut request_bytes)
        .map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{peer}"),
            message: error.to_string(),
        })?;
    let envelope =
        serde_json::from_slice::<QwenLegalSignedJobEnvelope>(&request_bytes).map_err(|error| {
            QwenLegalPylonDispatchError::Serialization {
                message: error.to_string(),
            }
        })?;
    verify_signed_job_envelope(&envelope)?;
    let receipt = run_qwen_legal_pylon_worker_job(&envelope.job, &options)?;
    verify_worker_receipt_signature(&receipt)?;
    let response_bytes = serde_json::to_vec(&receipt).map_err(|error| {
        QwenLegalPylonDispatchError::Serialization {
            message: error.to_string(),
        }
    })?;
    inbound
        .write_all(response_bytes.as_slice())
        .map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{peer}"),
            message: error.to_string(),
        })?;
    inbound
        .shutdown(Shutdown::Write)
        .map_err(|error| QwenLegalPylonDispatchError::Io {
            path: format!("tcp://{peer}"),
            message: error.to_string(),
        })?;
    Ok(())
}

fn transport_event(
    envelope: &QwenLegalSignedJobEnvelope,
    node: &QwenLegalPylonNodeDispatchCapability,
    transport: &str,
    endpoint: &str,
    request_bytes: usize,
    response_digest: &str,
    response_bytes: usize,
    scheduler_signature_verified: bool,
    worker_signature_present: bool,
    worker_receipt_digest: &str,
    worker_receipt_verified: bool,
) -> QwenLegalPylonTransportEvent {
    QwenLegalPylonTransportEvent {
        job_id: envelope.job.job_id.clone(),
        target_node_id: node.node_id.clone(),
        transport: String::from(transport),
        endpoint: String::from(endpoint),
        request_digest: envelope.envelope_digest.clone(),
        request_bytes: u64::try_from(request_bytes).unwrap_or(u64::MAX),
        response_digest: String::from(response_digest),
        response_bytes: u64::try_from(response_bytes).unwrap_or(u64::MAX),
        scheduler_signature_verified,
        worker_signature_present,
        worker_receipt_digest: String::from(worker_receipt_digest),
        worker_receipt_verified,
    }
}

pub fn canonical_qwen_legal_loopback_dispatch_request(
    output_dir: impl Into<PathBuf>,
) -> Result<QwenLegalPylonDispatchRequest, QwenLegalPylonDispatchError> {
    let output_dir = output_dir.into();
    Ok(QwenLegalPylonDispatchRequest {
        run_id: String::from("run.qwen-legal.pylon.loopback-dispatch.000001"),
        mode: QwenLegalPylonDispatchMode::Loopback,
        nodes: vec![
            loopback_node("pylon.loopback.qwen-legal.01", "loopback://qwen-legal-01"),
            loopback_node("pylon.loopback.qwen-legal.02", "loopback://qwen-legal-02"),
        ],
        jobs: canonical_dispatch_jobs(output_dir.as_path())?,
        output_dir,
        max_retries: 1,
    })
}

pub fn run_canonical_qwen_legal_loopback_dispatch(
    output_dir: impl Into<PathBuf>,
) -> Result<QwenLegalPylonDispatchReport, QwenLegalPylonDispatchError> {
    let request = canonical_qwen_legal_loopback_dispatch_request(output_dir)?;
    dispatch_qwen_legal_pylon_jobs(&request)
}

fn validate_dispatch_request(
    request: &QwenLegalPylonDispatchRequest,
) -> Result<(), QwenLegalPylonDispatchError> {
    if request.run_id.trim().is_empty() {
        return invalid_request("run_id must be present");
    }
    if request.nodes.is_empty() {
        return invalid_request("at least one node capability report is required");
    }
    if request.jobs.is_empty() {
        return invalid_request("at least one job is required");
    }
    for node in &request.nodes {
        if node.node_id.trim().is_empty() {
            return invalid_request("node_id must be present");
        }
        if node.network_addr.trim().is_empty() {
            return invalid_request("network_addr must be present");
        }
    }
    Ok(())
}

fn invalid_request<T>(detail: impl Into<String>) -> Result<T, QwenLegalPylonDispatchError> {
    Err(QwenLegalPylonDispatchError::InvalidRequest {
        detail: detail.into(),
    })
}

fn blocked_node_reports(
    nodes: &[QwenLegalPylonNodeDispatchCapability],
) -> Vec<QwenLegalPylonBlockedNode> {
    nodes
        .iter()
        .filter_map(|node| {
            let reason = if !node.online {
                Some("node is offline")
            } else if !node.authenticated {
                Some("node is not authenticated")
            } else if node.trust_state != "trusted" {
                Some("node is not trusted")
            } else {
                None
            }?;
            Some(QwenLegalPylonBlockedNode {
                node_id: node.node_id.clone(),
                network_addr: node.network_addr.clone(),
                reason: String::from(reason),
            })
        })
        .collect()
}

fn dispatch_status(
    job_count: usize,
    assignment_count: usize,
    blocked_node_count: usize,
) -> QwenLegalPylonDispatchStatus {
    if assignment_count == job_count && blocked_node_count == 0 {
        QwenLegalPylonDispatchStatus::Completed
    } else if assignment_count == job_count {
        QwenLegalPylonDispatchStatus::CompletedWithBlocks
    } else if assignment_count == 0 {
        QwenLegalPylonDispatchStatus::Blocked
    } else {
        QwenLegalPylonDispatchStatus::Partial
    }
}

fn prepare_job_for_dispatch(
    request: &QwenLegalPylonDispatchRequest,
    job: &PylonTrainingJobSpec,
    node: &QwenLegalPylonNodeDispatchCapability,
    job_index: usize,
) -> Result<PylonTrainingJobSpec, QwenLegalPylonDispatchError> {
    let mut job = job.clone();
    let job_dir = request.output_dir.join("jobs").join(format!(
        "{:02}-{}",
        job_index + 1,
        safe_path_id(job.job_id.as_str())
    ));
    job.output_dir = job_dir.display().to_string();
    job.receipt_path = job_dir.join("worker_receipt.json").display().to_string();
    job.payment_budget.payment_account_ref = node.payment_target_ref.clone();
    for output in &mut job.expected_output_artifacts {
        output.path = job_dir
            .join(format!("{}.json", output.artifact_type))
            .display()
            .to_string();
    }
    Ok(job)
}

fn signed_job_envelope(
    run_id: &str,
    mode: QwenLegalPylonDispatchMode,
    node: &QwenLegalPylonNodeDispatchCapability,
    job: PylonTrainingJobSpec,
) -> QwenLegalSignedJobEnvelope {
    let signing_key = scheduler_signing_key(run_id);
    let mut envelope = QwenLegalSignedJobEnvelope {
        schema_version: String::from(QWEN_LEGAL_PYLON_JOB_ENVELOPE_SCHEMA_VERSION),
        envelope_id: format!("envelope.{}.{}", run_id, job.job_id),
        dispatch_mode: mode,
        target_node_id: node.node_id.clone(),
        target_network_addr: node.network_addr.clone(),
        job_spec_digest: job.stable_digest(),
        job,
        signed_payload_digest: String::new(),
        scheduler_pubkey_hex: String::new(),
        signature_hex: String::new(),
        envelope_digest: String::new(),
    };
    envelope.signed_payload_digest = envelope.signable_payload_digest();
    envelope.scheduler_pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());
    envelope.signature_hex = hex::encode(
        signing_key
            .sign(envelope.signed_payload_digest.as_bytes())
            .to_bytes(),
    );
    envelope.envelope_digest = envelope.stable_digest();
    envelope
}

fn payment_decision_for_receipt(
    receipt: &PylonTrainingWorkerReceipt,
    credit_key: String,
    paid_credit_keys: &mut BTreeSet<String>,
    duplicate_successful_shards: &mut Vec<String>,
) -> QwenLegalPylonDispatchPaymentDecision {
    if receipt.status != PylonTrainingWorkerJobStatus::Succeeded {
        return QwenLegalPylonDispatchPaymentDecision {
            job_id: receipt.job_id.clone(),
            worker_id: receipt.worker_id.clone(),
            payment_status: PylonTrainingPaymentStatus::Withheld,
            agreed_price_microusd: receipt.agreed_price_microusd,
            reason: receipt
                .failure_reason
                .clone()
                .unwrap_or_else(|| String::from("worker job did not succeed")),
        };
    }
    if !paid_credit_keys.insert(credit_key.clone()) {
        duplicate_successful_shards.push(credit_key);
        return QwenLegalPylonDispatchPaymentDecision {
            job_id: receipt.job_id.clone(),
            worker_id: receipt.worker_id.clone(),
            payment_status: PylonTrainingPaymentStatus::Withheld,
            agreed_price_microusd: receipt.agreed_price_microusd,
            reason: String::from("duplicate successful shard submission"),
        };
    }
    QwenLegalPylonDispatchPaymentDecision {
        job_id: receipt.job_id.clone(),
        worker_id: receipt.worker_id.clone(),
        payment_status: PylonTrainingPaymentStatus::Payable,
        agreed_price_microusd: receipt.agreed_price_microusd,
        reason: String::from("valid signed worker receipt"),
    }
}

fn shard_credit_key(job: &PylonTrainingJobSpec) -> String {
    format!(
        "{:?}|{}|{}",
        job.job_kind, job.parent_run_id, job.shard_assignment.shard_id
    )
}

fn canonical_dispatch_jobs(
    output_dir: &Path,
) -> Result<Vec<PylonTrainingJobSpec>, QwenLegalPylonDispatchError> {
    let kinds = [
        PylonTrainingJobKind::DatasetShardBuild,
        PylonTrainingJobKind::SftTrainShard,
        PylonTrainingJobKind::DpoTrainShard,
        PylonTrainingJobKind::GrpoSampleBatch,
        PylonTrainingJobKind::GrpoTrainShard,
        PylonTrainingJobKind::EvalShard,
        PylonTrainingJobKind::AdapterMerge,
        PylonTrainingJobKind::ArtifactVerify,
    ];
    kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| {
            dispatch_job(
                *kind,
                format!("job.qwen-legal.dispatch.{:06}", index + 1).as_str(),
                output_dir,
                index,
            )
        })
        .collect()
}

fn dispatch_job(
    kind: PylonTrainingJobKind,
    job_id: &str,
    output_dir: &Path,
    index: usize,
) -> Result<PylonTrainingJobSpec, QwenLegalPylonDispatchError> {
    let input_path = match kind {
        PylonTrainingJobKind::EvalShard => "suites/harvey_public_three.json",
        _ => "configs/legal/qwen36_grpo_smoke.json",
    };
    let input_type = match kind {
        PylonTrainingJobKind::EvalShard => "eval_suite",
        _ => "training_config",
    };
    let output_type = match kind {
        PylonTrainingJobKind::DatasetShardBuild => "dataset_shard_manifest",
        PylonTrainingJobKind::SftTrainShard => "sft_adapter_delta",
        PylonTrainingJobKind::DpoTrainShard => "dpo_adapter_delta",
        PylonTrainingJobKind::GrpoSampleBatch => "grpo_rollout_batch",
        PylonTrainingJobKind::GrpoTrainShard => "grpo_adapter_delta",
        PylonTrainingJobKind::EvalShard => "eval_shard_report",
        PylonTrainingJobKind::AdapterMerge => "adapter_merge_report",
        PylonTrainingJobKind::ArtifactVerify => "artifact_verify_report",
    };
    let input = artifact_ref_for_existing(
        format!("artifact.{job_id}.input").as_str(),
        input_type,
        input_path,
    )?;
    Ok(PylonTrainingJobSpec {
        schema_version: String::from(QWEN_LEGAL_PYLON_TRAINING_JOB_SCHEMA_VERSION),
        job_id: String::from(job_id),
        parent_run_id: String::from("run.qwen-legal.pylon.loopback-dispatch.000001"),
        job_kind: kind,
        model_id: String::from("Qwen/Qwen3.6-27B"),
        model_hash: String::from("sha256:qwen36-27b-loopback-dispatch"),
        adapter_id: Some(String::from("qwen36-27b-legal-loopback-dispatch")),
        adapter_hash: Some(String::from(
            "sha256:qwen36-27b-legal-loopback-dispatch-adapter",
        )),
        dataset_manifest_hash: String::from("sha256:qwen-legal-corpus-loopback-dispatch"),
        shard_assignment: PylonTrainingShardAssignment {
            assignment_id: format!("assignment.{job_id}"),
            shard_id: format!("qwen-legal-corpus-loopback.sft_train.shard.{index:05}"),
            shard_index: u32::try_from(index).unwrap_or(u32::MAX),
            shard_count: 8,
            start_index: Some(u64::try_from(index.saturating_mul(10)).unwrap_or(u64::MAX)),
            end_index: Some(
                u64::try_from(index.saturating_mul(10).saturating_add(9)).unwrap_or(u64::MAX),
            ),
        },
        training_config_hash: input.sha256.clone(),
        expected_input_artifacts: vec![input],
        expected_output_artifacts: vec![PylonTrainingExpectedOutputArtifact {
            artifact_id: format!("artifact.{job_id}.output"),
            artifact_type: String::from(output_type),
            path: output_dir
                .join(format!("{job_id}.{output_type}.json"))
                .display()
                .to_string(),
            required: true,
        }],
        max_runtime_ms: 60_000,
        hardware_requirements: PylonTrainingHardwareRequirements {
            min_memory_bytes: 512 * 1024 * 1024,
            require_accelerator: false,
            accepted_backend_labels: vec![String::from("loopback")],
        },
        payment_budget: PylonTrainingPaymentBudget {
            budget_id: format!("budget.{job_id}"),
            agreed_price_microusd: 2_500,
            max_cost_microusd: 2_500,
            currency: String::from("USD"),
            payment_account_ref: String::from("bitcoin+lightning://unassigned"),
            bitcoin_payout: pylon_bitcoin_payout_target_for_job(job_id, 25_000),
            pay_failed_but_valid_eval_attempts: false,
        },
        receipt_requirements: PylonTrainingReceiptRequirements {
            require_signature: true,
            require_logs_hash: true,
            require_metrics: true,
            required_output_artifact_types: vec![String::from(output_type)],
        },
        output_dir: output_dir.display().to_string(),
        receipt_path: output_dir
            .join(format!("{job_id}.receipt.json"))
            .display()
            .to_string(),
    })
}

fn loopback_node(node_id: &str, network_addr: &str) -> QwenLegalPylonNodeDispatchCapability {
    QwenLegalPylonNodeDispatchCapability {
        node_id: String::from(node_id),
        backend_label: String::from("loopback"),
        network_addr: String::from(network_addr),
        host_memory_bytes: 64 * 1024 * 1024 * 1024,
        accelerator_memory_bytes: 0,
        model_cached: true,
        allowed_job_types: vec![
            PylonTrainingJobKind::DatasetShardBuild,
            PylonTrainingJobKind::SftTrainShard,
            PylonTrainingJobKind::DpoTrainShard,
            PylonTrainingJobKind::GrpoSampleBatch,
            PylonTrainingJobKind::GrpoTrainShard,
            PylonTrainingJobKind::EvalShard,
            PylonTrainingJobKind::AdapterMerge,
            PylonTrainingJobKind::ArtifactVerify,
        ],
        payment_target_ref: format!("bitcoin+lightning://{node_id}"),
        trust_state: String::from("trusted"),
        authenticated: true,
        online: true,
    }
}

fn artifact_ref_for_existing(
    artifact_id: &str,
    artifact_type: &str,
    path: &str,
) -> Result<PylonTrainingArtifactRef, QwenLegalPylonDispatchError> {
    let resolved = resolve_workspace_path(path);
    let bytes = fs::read(&resolved).map_err(|error| QwenLegalPylonDispatchError::Io {
        path: resolved.display().to_string(),
        message: error.to_string(),
    })?;
    Ok(PylonTrainingArtifactRef {
        artifact_id: String::from(artifact_id),
        artifact_type: String::from(artifact_type),
        path: String::from(path),
        sha256: sha256_hex(bytes.as_slice()),
    })
}

fn resolve_workspace_path(path: &str) -> PathBuf {
    let direct = PathBuf::from(path);
    if direct.exists() {
        return direct;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), QwenLegalPylonDispatchError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| QwenLegalPylonDispatchError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        QwenLegalPylonDispatchError::Serialization {
            message: error.to_string(),
        }
    })?;
    fs::write(path, bytes).map_err(|error| QwenLegalPylonDispatchError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn scheduler_signing_key(run_id: &str) -> SigningKey {
    let digest = Sha256::digest(format!("qwen-legal-pylon-dispatch|{run_id}").as_bytes());
    let mut secret = [0_u8; 32];
    secret.copy_from_slice(&digest[..32]);
    SigningKey::from_bytes(&secret)
}

fn verify_signed_job_envelope(
    envelope: &QwenLegalSignedJobEnvelope,
) -> Result<(), QwenLegalPylonDispatchError> {
    if envelope.schema_version != QWEN_LEGAL_PYLON_JOB_ENVELOPE_SCHEMA_VERSION {
        return invalid_request("job envelope schema version drifted");
    }
    if envelope.job_spec_digest != envelope.job.stable_digest() {
        return invalid_request("job envelope job_spec_digest drifted");
    }
    if envelope.signed_payload_digest != envelope.signable_payload_digest() {
        return invalid_request("job envelope signed_payload_digest drifted");
    }
    if envelope.envelope_digest != envelope.stable_digest() {
        return invalid_request("job envelope digest drifted");
    }
    let verifying_key = decode_verifying_key(envelope.scheduler_pubkey_hex.as_str())?;
    let signature = decode_signature(envelope.signature_hex.as_str())?;
    verifying_key
        .verify(envelope.signed_payload_digest.as_bytes(), &signature)
        .map_err(|_| QwenLegalPylonDispatchError::InvalidRequest {
            detail: String::from("job envelope scheduler signature verification failed"),
        })
}

fn verify_worker_receipt_signature(
    receipt: &PylonTrainingWorkerReceipt,
) -> Result<(), QwenLegalPylonDispatchError> {
    if receipt.receipt_digest != receipt.stable_digest() {
        return invalid_request("worker receipt digest drifted");
    }
    if receipt.signed_payload_digest != receipt.signable_payload_digest() {
        return invalid_request("worker receipt signed_payload_digest drifted");
    }
    let verifying_key = decode_verifying_key(receipt.worker_pubkey.as_str())?;
    let signature = decode_signature(receipt.signature_hex.as_str())?;
    verifying_key
        .verify(receipt.signed_payload_digest.as_bytes(), &signature)
        .map_err(|_| QwenLegalPylonDispatchError::InvalidRequest {
            detail: String::from("worker receipt signature verification failed"),
        })
}

fn decode_verifying_key(public_key_hex: &str) -> Result<VerifyingKey, QwenLegalPylonDispatchError> {
    let bytes = hex::decode(public_key_hex).map_err(|error| {
        QwenLegalPylonDispatchError::InvalidRequest {
            detail: format!("invalid public key hex: {error}"),
        }
    })?;
    let bytes: [u8; 32] =
        bytes
            .try_into()
            .map_err(|_| QwenLegalPylonDispatchError::InvalidRequest {
                detail: String::from("public key must be 32 bytes"),
            })?;
    VerifyingKey::from_bytes(&bytes).map_err(|error| QwenLegalPylonDispatchError::InvalidRequest {
        detail: format!("invalid public key: {error}"),
    })
}

fn decode_signature(signature_hex: &str) -> Result<Signature, QwenLegalPylonDispatchError> {
    let bytes = hex::decode(signature_hex).map_err(|error| {
        QwenLegalPylonDispatchError::InvalidRequest {
            detail: format!("invalid signature hex: {error}"),
        }
    })?;
    let bytes: [u8; 64] =
        bytes
            .try_into()
            .map_err(|_| QwenLegalPylonDispatchError::InvalidRequest {
                detail: String::from("signature must be 64 bytes"),
            })?;
    Ok(Signature::from_bytes(&bytes))
}

fn tcp_endpoint(network_addr: &str) -> Result<String, QwenLegalPylonDispatchError> {
    let trimmed = network_addr.trim();
    if let Some(endpoint) = trimmed.strip_prefix("tcp://") {
        return non_empty_endpoint(endpoint, network_addr);
    }
    if let Some(endpoint) = trimmed.strip_prefix("tailnet://") {
        return non_empty_endpoint(endpoint, network_addr);
    }
    if !trimmed.contains("://") && trimmed.contains(':') {
        return non_empty_endpoint(trimmed, network_addr);
    }
    invalid_request(format!(
        "remote Pylon worker address `{network_addr}` must be tcp://host:port, tailnet://host:port, or host:port"
    ))
}

fn non_empty_endpoint(
    endpoint: &str,
    original: &str,
) -> Result<String, QwenLegalPylonDispatchError> {
    if endpoint.trim().is_empty() {
        return invalid_request(format!(
            "remote Pylon worker address `{original}` is missing host:port"
        ));
    }
    Ok(endpoint.trim().to_string())
}

fn stable_json_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    if let Ok(bytes) = serde_json::to_vec(value) {
        hasher.update(bytes);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn safe_path_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_dispatch_runs_two_jobs_with_non_local_nodes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut request =
            canonical_qwen_legal_loopback_dispatch_request(temp.path().join("dispatch"))
                .expect("request");
        request.jobs.truncate(2);
        let report = dispatch_qwen_legal_pylon_jobs(&request).expect("dispatch");

        assert_eq!(report.status, QwenLegalPylonDispatchStatus::Completed);
        assert_eq!(report.assignments.len(), 2);
        assert_eq!(report.worker_receipts.len(), 2);
        assert!(report.job_envelopes.iter().all(|envelope| {
            !envelope.signature_hex.is_empty()
                && !envelope.scheduler_pubkey_hex.is_empty()
                && envelope.target_network_addr.starts_with("loopback://")
        }));
        assert!(report
            .worker_receipts
            .iter()
            .all(|receipt| { receipt.worker_id.starts_with("pylon.loopback.qwen-legal.") }));
        assert_eq!(report.transport_events.len(), 2);
        assert!(report.transport_events.iter().all(|event| {
            event.transport == "loopback_tcp"
                && event.endpoint.starts_with("127.0.0.1:")
                && event.request_bytes > 0
                && event.response_bytes > 0
                && event.scheduler_signature_verified
                && event.worker_signature_present
                && !event.worker_receipt_digest.is_empty()
                && event.worker_receipt_verified
        }));
    }

    #[test]
    fn tailnet_dispatch_uses_signed_remote_tcp_worker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut request =
            canonical_qwen_legal_loopback_dispatch_request(temp.path().join("dispatch"))
                .expect("request");
        request.mode = QwenLegalPylonDispatchMode::Tailnet;
        request.jobs.truncate(1);

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let endpoint = listener.local_addr().expect("addr").to_string();
        request.nodes = vec![loopback_node(
            "pylon.tailnet.qwen-legal.01",
            format!("tcp://{endpoint}").as_str(),
        )];
        let worker_options = PylonLocalWorkerRunOptions {
            worker_id: String::from("pylon.tailnet.qwen-legal.01"),
            started_at_ms: 100_000,
            emit_outputs: true,
        };
        let worker = thread::spawn(move || {
            serve_qwen_legal_pylon_tcp_worker_listener_once(listener, worker_options)
        });

        let report = dispatch_qwen_legal_pylon_jobs(&request).expect("dispatch");
        worker.join().expect("worker join").expect("worker ok");

        assert_eq!(report.status, QwenLegalPylonDispatchStatus::Completed);
        assert_eq!(report.assignments.len(), 1);
        assert_eq!(report.transport_events.len(), 1);
        let event = &report.transport_events[0];
        assert_eq!(event.transport, "tailnet_tcp_signed_envelope");
        assert_eq!(event.endpoint, endpoint);
        assert!(event.scheduler_signature_verified);
        assert!(event.worker_signature_present);
        assert!(event.worker_receipt_verified);
        assert_eq!(
            event.worker_receipt_digest,
            report.worker_receipts[0].receipt_digest
        );
        assert_eq!(
            report.payment_decisions[0].payment_status,
            PylonTrainingPaymentStatus::Payable
        );
    }

    #[test]
    fn loopback_dispatch_blocks_offline_node_and_continues() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut request =
            canonical_qwen_legal_loopback_dispatch_request(temp.path().join("dispatch"))
                .expect("request");
        request.jobs.truncate(2);
        request.nodes[0].online = false;
        let report = dispatch_qwen_legal_pylon_jobs(&request).expect("dispatch");

        assert_eq!(
            report.status,
            QwenLegalPylonDispatchStatus::CompletedWithBlocks
        );
        assert_eq!(report.assignments.len(), 2);
        assert_eq!(report.blocked_nodes.len(), 1);
        assert!(report
            .assignments
            .iter()
            .all(|assignment| { assignment.node_id == "pylon.loopback.qwen-legal.02" }));
    }

    #[test]
    fn duplicate_successful_shard_submissions_are_not_paid_twice() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut request =
            canonical_qwen_legal_loopback_dispatch_request(temp.path().join("dispatch"))
                .expect("request");
        request.jobs.truncate(2);
        let original = request.jobs[0].clone();
        request.jobs[1] = original;
        request.jobs[1].job_id = String::from("job.qwen-legal.dispatch.duplicate");
        let report = dispatch_qwen_legal_pylon_jobs(&request).expect("dispatch");

        assert_eq!(report.payment_decisions.len(), 2);
        assert_eq!(
            report.payment_decisions[0].payment_status,
            PylonTrainingPaymentStatus::Payable
        );
        assert_eq!(
            report.payment_decisions[1].payment_status,
            PylonTrainingPaymentStatus::Withheld
        );
        assert_eq!(report.duplicate_successful_shards.len(), 1);
    }
}
