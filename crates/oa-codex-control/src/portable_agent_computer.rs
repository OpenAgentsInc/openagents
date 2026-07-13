//! Retained Firecracker binding for PORT-03 managed Agent Computers.
//!
//! Unlike the one-shot Cloud-VM session driver, this service keeps the exact
//! provisioned VM alive between `stage` and `reclaim`.  The filesystem journal
//! stores only public-safe refs, digests, cursors, and operation responses; the
//! guest workspace and capability material stay inside the disposable VM.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::cloud_vm::{
    provisioner_for, CloudVmError, CloudVmOs, CloudVmProvisioner, CloudVmRequest, ProvisionedVm,
    ProvisionerKind,
};

pub const PORTABLE_AGENT_COMPUTER_VERSION: &str =
    "openagents.portable_agent_computer_provisioner.v1";
const GUEST_CONTROL_BIN: &str = "/opt/agent/portable-session-control";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableAgentComputerRequest {
    pub operation_ref: String,
    pub action: String,
    pub owner_ref: String,
    pub target_ref: String,
    pub session_ref: String,
    pub attachment_ref: String,
    pub generation: u64,
    #[serde(default)]
    pub resource_ref: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableCapabilityInstallRequest {
    pub operation_ref: String,
    pub owner_ref: String,
    pub target_ref: String,
    pub resource_ref: String,
    pub session_ref: String,
    pub attachment_ref: String,
    pub generation: u64,
    pub lease_ref: String,
    pub evidence_ref: String,
    pub capability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableCheckpointMaterializeRequest {
    pub operation_ref: String,
    pub owner_ref: String,
    pub target_ref: String,
    pub session_ref: String,
    pub attachment_ref: String,
    pub generation: u64,
    pub checkpoint_ref: String,
    pub artifact_ref: String,
    pub artifact_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableContinuationTurn {
    pub agent_ref: String,
    pub turn_ref: String,
    pub task: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableContinuationRequest {
    pub operation_ref: String,
    pub owner_ref: String,
    pub target_ref: String,
    pub session_ref: String,
    pub attachment_ref: String,
    pub generation: u64,
    pub provider_lease_ref: String,
    pub turns: Vec<PortableContinuationTurn>,
}

pub fn staged_resource_ref(target_ref: &str, session_ref: &str) -> String {
    format!(
        "resource.agent-computer.{}",
        short_digest(&format!("{target_ref}|{session_ref}"))
    )
}

pub fn continue_work(
    state_root: &Path,
    configured_kind: ProvisionerKind,
    request: &PortableContinuationRequest,
) -> Result<Value, CloudVmError> {
    for (field, value) in [
        ("operationRef", request.operation_ref.as_str()),
        ("ownerRef", request.owner_ref.as_str()),
        ("targetRef", request.target_ref.as_str()),
        ("sessionRef", request.session_ref.as_str()),
        ("attachmentRef", request.attachment_ref.as_str()),
        ("providerLeaseRef", request.provider_lease_ref.as_str()),
    ] {
        if !safe_ref(value) {
            return Err(CloudVmError::InvalidRequest(format!(
                "{field} is not a public-safe ref"
            )));
        }
    }
    if request.generation == 0 || request.turns.is_empty() || request.turns.len() > 64 {
        return Err(CloudVmError::InvalidRequest(
            "continuation generation or turn count is invalid".to_string(),
        ));
    }
    for turn in &request.turns {
        if !safe_ref(&turn.agent_ref)
            || !safe_ref(&turn.turn_ref)
            || turn.task.trim().is_empty()
            || turn.task.len() > 16 * 1024
        {
            return Err(CloudVmError::InvalidRequest(
                "continuation turn is invalid".to_string(),
            ));
        }
    }
    let encoded = serde_json::to_vec(&request)
        .map_err(|error| CloudVmError::InvalidRequest(format!("encode continuation: {error}")))?;
    let fingerprint = digest(&encoded);
    let root = state_root.join("portable-agent-computers");
    fs::create_dir_all(root.join("operations"))
        .map_err(|error| CloudVmError::Runtime(format!("create operation journal: {error}")))?;
    let operation_path = root
        .join("operations")
        .join(format!("{}.json", short_digest(&request.operation_ref)));
    if let Some(record) = read_json::<OperationRecord>(&operation_path)? {
        if record.fingerprint != fingerprint {
            return Err(CloudVmError::InvalidRequest(
                "operationRef was replayed with different bytes".to_string(),
            ));
        }
        if record.status == "completed" {
            let mut response = record.response.ok_or_else(|| {
                CloudVmError::Runtime("completed continuation has no response".to_string())
            })?;
            response
                .as_object_mut()
                .ok_or_else(|| {
                    CloudVmError::Runtime("continuation response is not an object".to_string())
                })?
                .insert("replay".to_string(), Value::String("replayed".to_string()));
            return Ok(response);
        }
    } else {
        write_json_atomic(
            &operation_path,
            &OperationRecord {
                schema: PORTABLE_AGENT_COMPUTER_VERSION.to_string(),
                fingerprint: fingerprint.clone(),
                status: "pending".to_string(),
                response: None,
            },
        )?;
    }
    let resource_ref = staged_resource_ref(&request.target_ref, &request.session_ref);
    let resource = read_json::<RetainedResource>(&resource_path(&root, &resource_ref))?
        .ok_or_else(|| {
            CloudVmError::InvalidRequest("active retained resource was not found".to_string())
        })?;
    if resource.state != "active"
        || resource.owner_ref != request.owner_ref
        || resource.target_ref != request.target_ref
        || resource.session_ref != request.session_ref
        || resource.attachment_ref != request.attachment_ref
        || resource.generation != request.generation
    {
        return Err(CloudVmError::InvalidRequest(
            "continuation scope differs from active retained resource".to_string(),
        ));
    }
    if !resource
        .capability_lease_refs
        .as_array()
        .is_some_and(|leases| {
            leases
                .iter()
                .any(|lease| lease.as_str() == Some(&request.provider_lease_ref))
        })
    {
        return Err(CloudVmError::InvalidRequest(
            "continuation provider lease was not planned".to_string(),
        ));
    }
    let agents = agent_refs(&resource.graph)?;
    if request.turns.len() != agents.len()
        || request
            .turns
            .iter()
            .zip(&agents)
            .any(|(turn, agent)| &turn.agent_ref != agent)
        || request
            .turns
            .iter()
            .map(|turn| turn.turn_ref.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len()
            != request.turns.len()
    {
        return Err(CloudVmError::InvalidRequest(
            "continuation must bind one unique turn to every graph agent".to_string(),
        ));
    }
    let (provisioner, effective_kind) = provisioner_for(configured_kind);
    if configured_kind == ProvisionerKind::Live && effective_kind != ProvisionerKind::Live {
        return Err(CloudVmError::KvmUnavailable(
            "continuation requires the configured retained Firecracker host".to_string(),
        ));
    }
    let response = if effective_kind == ProvisionerKind::Fake {
        let cursors = request.turns.iter().enumerate().map(|(index, turn)| json!({
            "agentRef": turn.agent_ref,
            "threadRef": resource.graph["nodes"][index]["threadRef"],
            "activityCursor": resource.graph["nodes"][index].get("activityCursor").and_then(Value::as_u64).unwrap_or(0) + 1,
            "eventCursor": resource.thread_cursors.as_array().and_then(|rows| rows.get(index)).and_then(|row| row.get("eventCursor")).and_then(Value::as_u64).unwrap_or(0) + 1,
        })).collect::<Vec<_>>();
        json!({
            "acceptedWorkRefs": request.turns.iter().map(|turn| json!({"agentRef": turn.agent_ref, "turnRef": turn.turn_ref})).collect::<Vec<_>>(),
            "threadCursors": cursors,
            "evidenceRefs": [evidence_ref("continuation", &request.operation_ref)],
            "replay": "executed", "material": "excluded",
        })
    } else {
        let mut private_body = encoded;
        let result = provisioner.exec_with_stdin(
            &resource.vm,
            GUEST_CONTROL_BIN,
            &["continue".to_string()],
            &mut private_body,
        );
        private_body.fill(0);
        let result = result?;
        if result.code != 0 {
            return Err(CloudVmError::Runtime(
                "portable guest continuation failed".to_string(),
            ));
        }
        serde_json::from_str(&result.output).map_err(|_| {
            CloudVmError::Runtime(
                "portable guest returned invalid continuation receipt".to_string(),
            )
        })?
    };
    response.as_object().ok_or_else(|| {
        CloudVmError::Runtime("continuation response is not an object".to_string())
    })?;
    reject_private_material(&serde_json::to_vec(&response).unwrap_or_default())?;
    write_json_atomic(
        &operation_path,
        &OperationRecord {
            schema: PORTABLE_AGENT_COMPUTER_VERSION.to_string(),
            fingerprint,
            status: "completed".to_string(),
            response: Some(response.clone()),
        },
    )?;
    Ok(response)
}

pub fn zero_continuation_tasks(request: &mut PortableContinuationRequest) {
    for turn in &mut request.turns {
        // SAFETY: overwriting existing UTF-8 bytes with zero preserves String's
        // allocation invariants; the value is never read again before drop.
        unsafe { turn.task.as_bytes_mut().fill(0) };
    }
}

pub fn materialize_checkpoint(
    state_root: &Path,
    configured_kind: ProvisionerKind,
    request: PortableCheckpointMaterializeRequest,
    artifact: &mut [u8],
) -> Result<Value, CloudVmError> {
    for (field, value) in [
        ("operationRef", request.operation_ref.as_str()),
        ("ownerRef", request.owner_ref.as_str()),
        ("targetRef", request.target_ref.as_str()),
        ("sessionRef", request.session_ref.as_str()),
        ("attachmentRef", request.attachment_ref.as_str()),
        ("checkpointRef", request.checkpoint_ref.as_str()),
        ("artifactRef", request.artifact_ref.as_str()),
        ("artifactDigest", request.artifact_digest.as_str()),
    ] {
        if !safe_ref(value) {
            return Err(CloudVmError::InvalidRequest(format!(
                "{field} is not a public-safe ref"
            )));
        }
    }
    if request.generation == 0 || artifact.is_empty() || artifact.len() > 128 * 1024 * 1024 {
        return Err(CloudVmError::InvalidRequest(
            "checkpoint artifact generation or length is invalid".to_string(),
        ));
    }
    let actual_digest = digest(artifact);
    if actual_digest != request.artifact_digest {
        return Err(CloudVmError::InvalidRequest(
            "checkpoint artifact digest differs from the declared digest".to_string(),
        ));
    }
    let root = state_root.join("portable-agent-computers");
    let resource_ref = staged_resource_ref(&request.target_ref, &request.session_ref);
    let path = resource_path(&root, &resource_ref);
    let mut resource = read_json::<RetainedResource>(&path)?.ok_or_else(|| {
        CloudVmError::InvalidRequest("prepared retained resource was not found".to_string())
    })?;
    if request.owner_ref != resource.owner_ref
        || request.target_ref != resource.target_ref
        || request.session_ref != resource.session_ref
        || request.attachment_ref != resource.attachment_ref
        || request.generation != resource.generation
        || request.checkpoint_ref != required_string(&resource.checkpoint, "checkpointRef")?
    {
        return Err(CloudVmError::InvalidRequest(
            "checkpoint artifact scope differs from prepared resource".to_string(),
        ));
    }
    if resource.state == "staged" {
        if resource.artifact_ref.as_deref() != Some(request.artifact_ref.as_str())
            || resource.artifact_digest.as_deref() != Some(request.artifact_digest.as_str())
        {
            return Err(CloudVmError::InvalidRequest(
                "checkpoint artifact replay conflicts with verified stage".to_string(),
            ));
        }
        return resource.stage_receipt.ok_or_else(|| {
            CloudVmError::Runtime("verified stage is missing its receipt".to_string())
        });
    }
    if resource.state != "prepared" {
        return Err(CloudVmError::InvalidRequest(
            "checkpoint artifact requires a prepared nonaccepting resource".to_string(),
        ));
    }
    let stage_operation = PortableAgentComputerRequest {
        operation_ref: resource.stage_operation_ref.clone(),
        action: "stage".to_string(),
        owner_ref: resource.owner_ref.clone(),
        target_ref: resource.target_ref.clone(),
        session_ref: resource.session_ref.clone(),
        attachment_ref: resource.attachment_ref.clone(),
        generation: resource.generation,
        resource_ref: None,
        payload: json!({
            "bundle": {
                "checkpoint": resource.checkpoint.clone(),
                "executionBinding": resource.execution_binding.clone(),
                "graph": resource.graph.clone(),
                "threadCursors": resource.thread_cursors.clone(),
            },
            "capabilityLeaseRefs": resource.capability_lease_refs.clone(),
        }),
    };
    let (provisioner, effective_kind) = provisioner_for(configured_kind);
    if configured_kind == ProvisionerKind::Live && effective_kind != ProvisionerKind::Live {
        return Err(CloudVmError::KvmUnavailable(
            "checkpoint materialization requires the configured retained Firecracker host"
                .to_string(),
        ));
    }
    let mut response = if effective_kind == ProvisionerKind::Fake {
        fake_stage_response(&resource)?
    } else {
        let metadata = serde_json::to_string(&json!({
            "operationRef": request.operation_ref,
            "artifactRef": request.artifact_ref,
            "artifactDigest": request.artifact_digest,
            "stageOperation": stage_operation,
        }))
        .map_err(|error| CloudVmError::Runtime(format!("encode checkpoint metadata: {error}")))?;
        let result = provisioner.exec_with_stdin(
            &resource.vm,
            GUEST_CONTROL_BIN,
            &["checkpoint-materialize".to_string(), metadata],
            artifact,
        )?;
        if result.code != 0 {
            return Err(CloudVmError::Runtime(
                "portable guest refused checkpoint materialization".to_string(),
            ));
        }
        serde_json::from_str(&result.output).map_err(|_| {
            CloudVmError::Runtime("portable guest returned invalid stage receipt".to_string())
        })?
    };
    response
        .as_object_mut()
        .ok_or_else(|| CloudVmError::Runtime("stage receipt is not an object".to_string()))?
        .insert("resourceRef".to_string(), Value::String(resource_ref));
    reject_private_material(&serde_json::to_vec(&response).unwrap_or_default())?;
    resource.state = "staged".to_string();
    resource.artifact_ref = Some(request.artifact_ref);
    resource.artifact_digest = Some(request.artifact_digest);
    resource.stage_receipt = Some(response.clone());
    write_json_atomic(&path, &resource)?;
    Ok(response)
}

pub fn install_capability(
    state_root: &Path,
    configured_kind: ProvisionerKind,
    request: PortableCapabilityInstallRequest,
    material: &mut [u8],
) -> Result<Value, CloudVmError> {
    for (field, value) in [
        ("operationRef", request.operation_ref.as_str()),
        ("ownerRef", request.owner_ref.as_str()),
        ("targetRef", request.target_ref.as_str()),
        ("resourceRef", request.resource_ref.as_str()),
        ("sessionRef", request.session_ref.as_str()),
        ("attachmentRef", request.attachment_ref.as_str()),
        ("leaseRef", request.lease_ref.as_str()),
        ("evidenceRef", request.evidence_ref.as_str()),
        ("capability", request.capability.as_str()),
    ] {
        if !safe_ref(value) {
            return Err(CloudVmError::InvalidRequest(format!(
                "{field} is not a public-safe ref"
            )));
        }
    }
    if request.generation == 0 || material.is_empty() || material.len() > 128 * 1024 {
        return Err(CloudVmError::InvalidRequest(
            "capability generation or material length is invalid".to_string(),
        ));
    }
    if request.resource_ref != staged_resource_ref(&request.target_ref, &request.session_ref) {
        return Err(CloudVmError::InvalidRequest(
            "capability install resource is not the exact staged target/session".to_string(),
        ));
    }
    let root = state_root.join("portable-agent-computers");
    let resource = read_json::<RetainedResource>(&resource_path(&root, &request.resource_ref))?
        .ok_or_else(|| {
            CloudVmError::InvalidRequest("retained resource was not found".to_string())
        })?;
    if request.owner_ref != resource.owner_ref
        || request.target_ref != resource.target_ref
        || request.session_ref != resource.session_ref
        || request.attachment_ref != resource.attachment_ref
        || request.generation != resource.generation
        || request.resource_ref != resource.resource_ref
        || resource.state != "staged"
    {
        return Err(CloudVmError::InvalidRequest(
            "capability install scope differs from staged resource".to_string(),
        ));
    }
    if !resource
        .capability_lease_refs
        .as_array()
        .is_some_and(|leases| {
            leases
                .iter()
                .any(|lease| lease.as_str() == Some(request.lease_ref.as_str()))
        })
    {
        return Err(CloudVmError::InvalidRequest(
            "capability lease was not planned by the staged checkpoint".to_string(),
        ));
    }
    let (provisioner, effective_kind) = provisioner_for(configured_kind);
    if configured_kind == ProvisionerKind::Live && effective_kind != ProvisionerKind::Live {
        return Err(CloudVmError::KvmUnavailable(
            "capability install requires the configured retained Firecracker host".to_string(),
        ));
    }
    if effective_kind == ProvisionerKind::Fake {
        return Ok(capability_install_response(&request));
    }
    let metadata = serde_json::to_string(&request)
        .map_err(|error| CloudVmError::Runtime(format!("encode capability metadata: {error}")))?;
    let result = provisioner.exec_with_stdin(
        &resource.vm,
        GUEST_CONTROL_BIN,
        &["capability-install".to_string(), metadata],
        material,
    )?;
    if result.code != 0 {
        return Err(CloudVmError::Runtime(
            "portable guest refused capability installation".to_string(),
        ));
    }
    let mut response: Value = serde_json::from_str(&result.output).map_err(|_| {
        CloudVmError::Runtime("portable guest returned invalid capability receipt".to_string())
    })?;
    response
        .as_object_mut()
        .ok_or_else(|| CloudVmError::Runtime("capability receipt is not an object".to_string()))?
        .insert(
            "resourceRef".to_string(),
            Value::String(request.resource_ref.clone()),
        );
    reject_private_material(&serde_json::to_vec(&response).unwrap_or_default())?;
    Ok(response)
}

fn capability_install_response(request: &PortableCapabilityInstallRequest) -> Value {
    json!({
        "installationRef": format!("installation.agent-computer.capability.{}", short_digest(&format!("{}|{}", request.resource_ref, request.lease_ref))),
        "evidenceRef": request.evidence_ref,
        "marker": { "leaseRef": request.lease_ref, "evidenceRef": request.evidence_ref },
        "material": "excluded",
        "resourceRef": request.resource_ref,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetainedResource {
    schema: String,
    owner_ref: String,
    target_ref: String,
    session_ref: String,
    attachment_ref: String,
    generation: u64,
    resource_ref: String,
    stage_operation_ref: String,
    vm: ProvisionedVm,
    state: String,
    checkpoint: Value,
    execution_binding: Value,
    graph: Value,
    thread_cursors: Value,
    #[serde(default)]
    capability_lease_refs: Value,
    #[serde(default)]
    artifact_ref: Option<String>,
    #[serde(default)]
    artifact_digest: Option<String>,
    #[serde(default)]
    stage_receipt: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OperationRecord {
    schema: String,
    fingerprint: String,
    status: String,
    response: Option<Value>,
}

pub fn execute(
    state_root: &Path,
    configured_kind: ProvisionerKind,
    request: PortableAgentComputerRequest,
) -> Result<Value, CloudVmError> {
    validate_request(&request)?;
    let encoded = serde_json::to_vec(&request)
        .map_err(|error| CloudVmError::InvalidRequest(format!("encode request: {error}")))?;
    reject_private_material(&encoded)?;
    let fingerprint = digest(&encoded);
    let root = state_root.join("portable-agent-computers");
    fs::create_dir_all(root.join("operations"))
        .map_err(|error| CloudVmError::Runtime(format!("create operation journal: {error}")))?;
    fs::create_dir_all(root.join("resources"))
        .map_err(|error| CloudVmError::Runtime(format!("create resource journal: {error}")))?;

    let operation_path = root
        .join("operations")
        .join(format!("{}.json", short_digest(&request.operation_ref)));
    if let Some(record) = read_json::<OperationRecord>(&operation_path)? {
        if record.fingerprint != fingerprint {
            return Err(CloudVmError::InvalidRequest(
                "operationRef was replayed with different bytes".to_string(),
            ));
        }
        if record.status == "completed" {
            return record.response.ok_or_else(|| {
                CloudVmError::Runtime("completed operation has no response".to_string())
            });
        }
    } else {
        write_json_atomic(
            &operation_path,
            &OperationRecord {
                schema: PORTABLE_AGENT_COMPUTER_VERSION.to_string(),
                fingerprint: fingerprint.clone(),
                status: "pending".to_string(),
                response: None,
            },
        )?;
    }

    let (provisioner, effective_kind) = provisioner_for(configured_kind);
    if configured_kind == ProvisionerKind::Live && effective_kind != ProvisionerKind::Live {
        return Err(CloudVmError::KvmUnavailable(
            "retained Agent Computer was armed for live Firecracker but this host is not ready"
                .to_string(),
        ));
    }
    let response = execute_effect(&root, provisioner.as_ref(), effective_kind, &request)?;
    reject_private_material(
        &serde_json::to_vec(&response)
            .map_err(|error| CloudVmError::Runtime(format!("encode response: {error}")))?,
    )?;
    write_json_atomic(
        &operation_path,
        &OperationRecord {
            schema: PORTABLE_AGENT_COMPUTER_VERSION.to_string(),
            fingerprint,
            status: "completed".to_string(),
            response: Some(response.clone()),
        },
    )?;
    Ok(response)
}

fn execute_effect(
    root: &Path,
    provisioner: &dyn CloudVmProvisioner,
    effective_kind: ProvisionerKind,
    request: &PortableAgentComputerRequest,
) -> Result<Value, CloudVmError> {
    if request.action == "stage" {
        return stage(root, provisioner, request);
    }
    if request.action == "abortPrepared" {
        return abort_prepared(root, provisioner, request);
    }
    let resource_ref = request.resource_ref.as_deref().ok_or_else(|| {
        CloudVmError::InvalidRequest("resourceRef is required after stage".to_string())
    })?;
    let resource_path = resource_path(root, resource_ref);
    let mut resource = read_json::<RetainedResource>(&resource_path)?.ok_or_else(|| {
        CloudVmError::InvalidRequest("retained resource was not found".to_string())
    })?;
    assert_resource(&resource, request)?;

    let cleanup_reconciliation = matches!(request.action.as_str(), "abort" | "reclaim")
        && matches!(resource.state.as_str(), "teardown_pending" | "reclaimed");
    let response = if effective_kind == ProvisionerKind::Fake || cleanup_reconciliation {
        fake_response(&resource, request)?
    } else {
        live_guest_response(provisioner, &resource.vm, request)?
    };

    match request.action.as_str() {
        "activate" => resource.state = "active".to_string(),
        "quiesce" => resource.state = "quiesced".to_string(),
        "abort" | "reclaim" => {
            if resource.state != "reclaimed" {
                resource.state = "teardown_pending".to_string();
                write_json_atomic(&resource_path, &resource)?;
                provisioner.teardown(&resource.vm)?;
                resource.state = "reclaimed".to_string();
            }
        }
        "checkpoint" => {}
        "wipeCapability" => {}
        _ => {
            return Err(CloudVmError::InvalidRequest(format!(
                "unknown portable Agent Computer action '{}'",
                request.action
            )))
        }
    }
    write_json_atomic(&resource_path, &resource)?;
    Ok(response)
}

fn abort_prepared(
    root: &Path,
    provisioner: &dyn CloudVmProvisioner,
    request: &PortableAgentComputerRequest,
) -> Result<Value, CloudVmError> {
    if request.resource_ref.is_some() {
        return Err(CloudVmError::InvalidRequest(
            "abortPrepared derives its retained resource".to_string(),
        ));
    }
    let stage_operation_ref = required_payload_string(request, "stageOperationRef")?;
    let resource_ref = staged_resource_ref(&request.target_ref, &request.session_ref);
    let path = resource_path(root, &resource_ref);
    let Some(mut resource) = read_json::<RetainedResource>(&path)? else {
        return Ok(
            json!({ "evidenceRefs": [evidence_ref("abort-prepared", &request.operation_ref)], "material": "excluded" }),
        );
    };
    if resource.owner_ref != request.owner_ref
        || resource.target_ref != request.target_ref
        || resource.session_ref != request.session_ref
        || resource.attachment_ref != request.attachment_ref
        || resource.generation != request.generation
        || resource.stage_operation_ref != stage_operation_ref
    {
        return Err(CloudVmError::InvalidRequest(
            "abortPrepared scope differs from prepared resource".to_string(),
        ));
    }
    if resource.state != "reclaimed" {
        resource.state = "teardown_pending".to_string();
        write_json_atomic(&path, &resource)?;
        provisioner.teardown(&resource.vm)?;
        resource.state = "reclaimed".to_string();
        write_json_atomic(&path, &resource)?;
    }
    let stage_record_path = root
        .join("operations")
        .join(format!("{}.json", short_digest(&stage_operation_ref)));
    if let Some(mut stage_record) = read_json::<OperationRecord>(&stage_record_path)? {
        stage_record.status = "compensated".to_string();
        stage_record.response = None;
        write_json_atomic(&stage_record_path, &stage_record)?;
    }
    Ok(
        json!({ "evidenceRefs": [evidence_ref("abort-prepared", &request.operation_ref)], "material": "excluded" }),
    )
}

fn stage(
    root: &Path,
    provisioner: &dyn CloudVmProvisioner,
    request: &PortableAgentComputerRequest,
) -> Result<Value, CloudVmError> {
    if request.resource_ref.is_some() {
        return Err(CloudVmError::InvalidRequest(
            "stage must not supply resourceRef".to_string(),
        ));
    }
    let bundle = request
        .payload
        .get("bundle")
        .ok_or_else(|| CloudVmError::InvalidRequest("stage payload requires bundle".to_string()))?;
    let checkpoint = bundle.get("checkpoint").cloned().ok_or_else(|| {
        CloudVmError::InvalidRequest("stage bundle requires checkpoint".to_string())
    })?;
    let execution_binding = bundle.get("executionBinding").cloned().ok_or_else(|| {
        CloudVmError::InvalidRequest("stage bundle requires executionBinding".to_string())
    })?;
    let graph = bundle
        .get("graph")
        .cloned()
        .ok_or_else(|| CloudVmError::InvalidRequest("stage bundle requires graph".to_string()))?;
    let thread_cursors = bundle.get("threadCursors").cloned().ok_or_else(|| {
        CloudVmError::InvalidRequest("stage bundle requires threadCursors".to_string())
    })?;
    let capability_lease_refs = request
        .payload
        .get("capabilityLeaseRefs")
        .cloned()
        .ok_or_else(|| {
            CloudVmError::InvalidRequest("stage payload requires capabilityLeaseRefs".to_string())
        })?;
    if !capability_lease_refs.as_array().is_some_and(|leases| {
        !leases.is_empty()
            && leases
                .iter()
                .all(|lease| lease.as_str().is_some_and(safe_ref))
    }) {
        return Err(CloudVmError::InvalidRequest(
            "stage capabilityLeaseRefs must be non-empty public-safe refs".to_string(),
        ));
    }
    let resource_ref = staged_resource_ref(&request.target_ref, &request.session_ref);
    let retained_path = resource_path(root, &resource_ref);
    if let Some(resource) = read_json::<RetainedResource>(&retained_path)? {
        if resource.stage_operation_ref != request.operation_ref {
            return Err(CloudVmError::InvalidRequest(
                "a different stage operation already owns this session resource".to_string(),
            ));
        }
        let mut replay = request.clone();
        replay.resource_ref = Some(resource_ref);
        assert_resource(&resource, &replay)?;
        if resource.state == "reclaimed" || resource.state == "teardown_pending" {
            return Err(CloudVmError::InvalidRequest(
                "prepared resource was compensated and cannot be resumed".to_string(),
            ));
        }
        return Ok(prepared_stage_response(&resource));
    }
    let vm_request = CloudVmRequest {
        run_id: request.operation_ref.clone(),
        os: CloudVmOs::Linux,
        target_name: request.target_ref.clone(),
        owner_ref: request.owner_ref.clone(),
    };
    let vm = provisioner.provision(&vm_request)?;
    if !vm.healthy {
        let _ = provisioner.teardown(&vm);
        return Err(CloudVmError::Runtime(
            "retained Agent Computer failed its boot health check".to_string(),
        ));
    }
    let resource = RetainedResource {
        schema: PORTABLE_AGENT_COMPUTER_VERSION.to_string(),
        owner_ref: request.owner_ref.clone(),
        target_ref: request.target_ref.clone(),
        session_ref: request.session_ref.clone(),
        attachment_ref: request.attachment_ref.clone(),
        generation: request.generation,
        resource_ref: resource_ref.clone(),
        stage_operation_ref: request.operation_ref.clone(),
        vm,
        state: "prepared".to_string(),
        checkpoint: checkpoint.clone(),
        execution_binding,
        graph: graph.clone(),
        thread_cursors: thread_cursors.clone(),
        capability_lease_refs,
        artifact_ref: None,
        artifact_digest: None,
        stage_receipt: None,
    };
    write_json_atomic(&retained_path, &resource)?;
    Ok(prepared_stage_response(&resource))
}

fn prepared_stage_response(resource: &RetainedResource) -> Value {
    json!({
        "resourceRef": resource.resource_ref,
        "acceptingWork": false,
        "materializationRequired": true,
        "evidenceRefs": [evidence_ref("prepare-stage", &resource.resource_ref)],
    })
}

fn fake_stage_response(resource: &RetainedResource) -> Result<Value, CloudVmError> {
    let checkpoint = &resource.checkpoint;
    Ok(json!({
        "resourceRef": resource.resource_ref,
        "checkpointDigest": required_string(checkpoint, "digest")?,
        "repositoryPostImageDigest": required_string(checkpoint, "repositoryPostImageDigest")?,
        "diffDigest": required_string(checkpoint, "diffDigest")?,
        "graphDigest": required_string(checkpoint, "graphDigest")?,
        "threadCursors": resource.thread_cursors,
        "acceptingWork": false,
        "evidenceRefs": [evidence_ref("stage", &resource.resource_ref)],
    }))
}

fn fake_response(
    resource: &RetainedResource,
    request: &PortableAgentComputerRequest,
) -> Result<Value, CloudVmError> {
    let agents = agent_refs(request.payload.get("graph").unwrap_or(&resource.graph))?;
    match request.action.as_str() {
        "activate" => Ok(json!({
            "activatedAgentRefs": agents,
            // Activation makes the retained runtime eligible to accept work;
            // it never fabricates a continued turn in the no-KVM contract lane.
            "acceptedWorkRefs": [],
            "evidenceRefs": [evidence_ref("activate", &request.operation_ref)],
        })),
        "abort" => Ok(json!({
            "evidenceRefs": [evidence_ref("abort", &request.operation_ref)]
        })),
        "quiesce" => Ok(json!({
            "quiescedAgentRefs": agents,
            "evidenceRefs": [evidence_ref("quiesce", &request.operation_ref)]
        })),
        "checkpoint" => {
            let mut checkpoint = resource.checkpoint.clone();
            {
                let object = checkpoint.as_object_mut().ok_or_else(|| {
                    CloudVmError::Runtime("retained checkpoint is not an object".to_string())
                })?;
                object.insert(
                    "checkpointRef".to_string(),
                    Value::String(required_payload_string(request, "checkpointRef")?),
                );
                object.insert(
                    "sourceAttachmentRef".to_string(),
                    Value::String(request.attachment_ref.clone()),
                );
                object.insert(
                    "sourceGeneration".to_string(),
                    Value::Number(request.generation.into()),
                );
                if let Some(cursor) = request.payload.get("eventLogCursor") {
                    object.insert("eventLogCursor".to_string(), cursor.clone());
                }
                object.remove("digest");
            }
            // The production guest recomputes this digest. The fake contract lane
            // marks its deterministic recomputation with the exact operation.
            let canonical = canonical_json(&checkpoint);
            checkpoint
                .as_object_mut()
                .expect("checkpoint object")
                .insert(
                    "digest".to_string(),
                    Value::String(format!("sha256:{}", digest_hex(canonical.as_bytes()))),
                );
            Ok(json!({
                "checkpoint": checkpoint,
                "executionBinding": request.payload.get("executionBinding").cloned().unwrap_or_else(|| resource.execution_binding.clone()),
                "graph": request.payload.get("graph").cloned().unwrap_or_else(|| resource.graph.clone()),
                "threadCursors": request.payload.get("threadCursors").cloned().unwrap_or_else(|| resource.thread_cursors.clone()),
            }))
        }
        "reclaim" => Ok(json!({
            "cleanedAgentRefs": request.payload.get("agentRefs").cloned().unwrap_or_else(|| json!(agents)),
            "processes": "released",
            "scratch": "released",
            "ports": "released",
            "evidenceRefs": [evidence_ref("reclaim", &request.operation_ref)]
        })),
        "wipeCapability" => Ok(json!({
            "wipeReceiptRef": evidence_ref("capability-wipe", &request.operation_ref),
            "material": "excluded"
        })),
        other => Err(CloudVmError::InvalidRequest(format!(
            "unknown action '{other}'"
        ))),
    }
}

fn live_guest_response(
    provisioner: &dyn CloudVmProvisioner,
    vm: &ProvisionedVm,
    request: &PortableAgentComputerRequest,
) -> Result<Value, CloudVmError> {
    let encoded = serde_json::to_string(request)
        .map_err(|error| CloudVmError::Runtime(format!("encode guest operation: {error}")))?;
    let result = provisioner.exec(vm, GUEST_CONTROL_BIN, &[encoded])?;
    if result.code != 0 {
        return Err(CloudVmError::Runtime(
            "portable session guest controller refused the operation".to_string(),
        ));
    }
    serde_json::from_str(&result.output).map_err(|_| {
        CloudVmError::Runtime("portable session guest controller returned invalid JSON".to_string())
    })
}

fn validate_request(request: &PortableAgentComputerRequest) -> Result<(), CloudVmError> {
    for (field, value) in [
        ("operationRef", request.operation_ref.as_str()),
        ("ownerRef", request.owner_ref.as_str()),
        ("targetRef", request.target_ref.as_str()),
        ("sessionRef", request.session_ref.as_str()),
        ("attachmentRef", request.attachment_ref.as_str()),
    ] {
        if !safe_ref(value) {
            return Err(CloudVmError::InvalidRequest(format!(
                "{field} is not a public-safe ref"
            )));
        }
    }
    if request.generation == 0 {
        return Err(CloudVmError::InvalidRequest(
            "generation must be positive".to_string(),
        ));
    }
    Ok(())
}

fn assert_resource(
    resource: &RetainedResource,
    request: &PortableAgentComputerRequest,
) -> Result<(), CloudVmError> {
    if request.resource_ref.as_deref() != Some(resource.resource_ref.as_str())
        || request.owner_ref != resource.owner_ref
        || request.target_ref != resource.target_ref
        || request.session_ref != resource.session_ref
        || request.attachment_ref != resource.attachment_ref
        || request.generation != resource.generation
    {
        return Err(CloudVmError::InvalidRequest(
            "retained resource scope or attachment generation differs".to_string(),
        ));
    }
    match request.action.as_str() {
        "activate" if !matches!(resource.state.as_str(), "staged" | "active") => Err(
            CloudVmError::InvalidRequest("only a staged resource may activate".to_string()),
        ),
        "quiesce" if !matches!(resource.state.as_str(), "active" | "quiesced") => Err(
            CloudVmError::InvalidRequest("only an active resource may quiesce".to_string()),
        ),
        "checkpoint" if resource.state != "quiesced" => Err(CloudVmError::InvalidRequest(
            "checkpoint requires quiescence".to_string(),
        )),
        "reclaim"
            if !matches!(
                resource.state.as_str(),
                "quiesced" | "teardown_pending" | "reclaimed"
            ) =>
        {
            Err(CloudVmError::InvalidRequest(
                "reclaim requires quiescence".to_string(),
            ))
        }
        "abort"
            if !matches!(
                resource.state.as_str(),
                "staged" | "teardown_pending" | "reclaimed"
            ) =>
        {
            Err(CloudVmError::InvalidRequest(
                "only a staged resource may abort".to_string(),
            ))
        }
        "wipeCapability" if resource.state == "reclaimed" => Err(CloudVmError::InvalidRequest(
            "cannot wipe a reclaimed resource".to_string(),
        )),
        _ => Ok(()),
    }
}

fn agent_refs(graph: &Value) -> Result<Vec<String>, CloudVmError> {
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| CloudVmError::InvalidRequest("graph nodes are required".to_string()))?;
    nodes
        .iter()
        .map(|node| {
            node.get("agentRef")
                .and_then(Value::as_str)
                .map(str::to_string)
                .filter(|value| safe_ref(value))
                .ok_or_else(|| {
                    CloudVmError::InvalidRequest("graph agentRef is invalid".to_string())
                })
        })
        .collect()
}

fn required_string(value: &Value, field: &str) -> Result<String, CloudVmError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| CloudVmError::InvalidRequest(format!("{field} is required")))
}

fn required_payload_string(
    request: &PortableAgentComputerRequest,
    field: &str,
) -> Result<String, CloudVmError> {
    required_string(&request.payload, field)
}

fn resource_path(root: &Path, resource_ref: &str) -> PathBuf {
    root.join("resources")
        .join(format!("{}.json", short_digest(resource_ref)))
}

fn read_json<A: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<A>, CloudVmError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| CloudVmError::Runtime(format!("decode retained journal: {error}"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(CloudVmError::Runtime(format!(
            "read retained journal: {error}"
        ))),
    }
}

fn write_json_atomic<A: Serialize>(path: &Path, value: &A) -> Result<(), CloudVmError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| CloudVmError::Runtime(format!("encode retained journal: {error}")))?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temp, bytes)
        .map_err(|error| CloudVmError::Runtime(format!("write retained journal: {error}")))?;
    fs::rename(&temp, path)
        .map_err(|error| CloudVmError::Runtime(format!("commit retained journal: {error}")))
}

fn reject_private_material(bytes: &[u8]) -> Result<(), CloudVmError> {
    let lower = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    let forbidden = [
        "bearer ",
        "basic ",
        "\"token\"",
        "\"apikey\"",
        "\"password\"",
        "\"secret\"",
        "\"credential\"",
        "\"mnemonic\"",
        "\"hostname\"",
        "\"processid\"",
        "\"socket\"",
        "/users/",
        "/home/",
        "auth.json",
    ];
    if forbidden.iter().any(|needle| lower.contains(needle)) {
        return Err(CloudVmError::InvalidRequest(
            "portable Agent Computer payload contains forbidden private material".to_string(),
        ));
    }
    Ok(())
}

fn safe_ref(value: &str) -> bool {
    (3..=256).contains(&value.len())
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

fn evidence_ref(kind: &str, seed: &str) -> String {
    format!(
        "evidence.portable-agent-computer.{kind}.{}",
        short_digest(seed)
    )
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", digest_hex(bytes))
}

fn digest_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).unwrap_or_default(),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|key| format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical_json(&values[key])
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

fn short_digest(value: &str) -> String {
    digest_hex(value.as_bytes())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "oa-portable-agent-computer-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn request(
        action: &str,
        operation_ref: &str,
        resource_ref: Option<&str>,
    ) -> PortableAgentComputerRequest {
        PortableAgentComputerRequest {
            operation_ref: operation_ref.to_string(),
            action: action.to_string(),
            owner_ref: "owner.port03.binding".to_string(),
            target_ref: "target.port03.binding.managed".to_string(),
            session_ref: "session.port03.binding".to_string(),
            attachment_ref: "attachment.port03.binding.managed".to_string(),
            generation: 2,
            resource_ref: resource_ref.map(str::to_string),
            payload: json!({}),
        }
    }

    fn stage_request(operation_ref: &str) -> PortableAgentComputerRequest {
        let mut value = request("stage", operation_ref, None);
        value.payload = json!({
            "bundle": {
                "checkpoint": {
                    "schema": "openagents.portable_checkpoint.v1",
                    "checkpointRef": "checkpoint.port03.binding.source",
                    "sessionRef": "session.port03.binding",
                    "sourceAttachmentRef": "attachment.port03.binding.source",
                    "sourceGeneration": 1,
                    "repositoryRef": "repository.OpenAgentsInc.openagents",
                    "repositoryRevisionRef": "revision.port03.binding.source",
                    "repositoryPostImageDigest": format!("sha256:{}", "a".repeat(64)),
                    "diffDigest": format!("sha256:{}", "b".repeat(64)),
                    "eventLogCursor": 9,
                    "catalogGenerationRef": "catalog.port03.binding.1",
                    "graphDigest": format!("sha256:{}", "c".repeat(64)),
                    "digest": format!("sha256:{}", "d".repeat(64)),
                    "approvalRefs": [], "artifactRefs": [], "receiptRefs": [],
                    "secretMaterial": "excluded", "processState": "excluded"
                },
                "executionBinding": {
                    "schema": "openagents.portable_session_execution_binding.v1",
                    "sessionRef": "session.port03.binding",
                    "ownerRef": "owner.port03.binding",
                    "runRef": "run.port03.binding",
                    "repositoryRef": "repository.OpenAgentsInc.openagents",
                    "pinnedBaseRef": "revision.port03.binding.base"
                },
                "graph": {
                    "rootAgentRef": "agent.port03.binding.root",
                    "nodes": [{
                        "agentRef": "agent.port03.binding.root",
                        "threadRef": "thread.port03.binding.root",
                        "transcriptRef": "transcript.port03.binding.root",
                        "activityCursor": 3,
                        "lifecycle": "waiting",
                        "attachmentGeneration": 1
                    }]
                },
                "threadCursors": [{
                    "threadRef": "thread.port03.binding.root",
                    "transcriptRef": "transcript.port03.binding.root",
                    "activityCursor": 3,
                    "eventCursor": 9
                }]
            },
            "capabilityLeaseRefs": ["lease.port03.binding.provider"]
        });
        value
    }

    fn materialize_fake(state_root: &Path, operation_ref: &str) -> Value {
        let mut artifact = b"fake-private-checkpoint-archive".to_vec();
        materialize_checkpoint(
            state_root,
            ProvisionerKind::Fake,
            PortableCheckpointMaterializeRequest {
                operation_ref: operation_ref.to_string(),
                owner_ref: "owner.port03.binding".to_string(),
                target_ref: "target.port03.binding.managed".to_string(),
                session_ref: "session.port03.binding".to_string(),
                attachment_ref: "attachment.port03.binding.managed".to_string(),
                generation: 2,
                checkpoint_ref: "checkpoint.port03.binding.source".to_string(),
                artifact_ref: "artifact.port03.binding.private".to_string(),
                artifact_digest: digest(&artifact),
            },
            &mut artifact,
        )
        .unwrap()
    }

    #[test]
    fn retained_lifecycle_replays_and_reclaims_the_exact_resource() {
        let state_root = root("lifecycle");
        let stage = stage_request("operation.port03.binding.stage");
        let staged = execute(&state_root, ProvisionerKind::Fake, stage.clone()).unwrap();
        assert_eq!(
            staged.get("acceptingWork").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            execute(&state_root, ProvisionerKind::Fake, stage).unwrap(),
            staged
        );
        let resource_ref = staged.get("resourceRef").and_then(Value::as_str).unwrap();
        let _staged = materialize_fake(&state_root, "operation.port03.binding.materialize");

        let mut activate = request(
            "activate",
            "operation.port03.binding.activate",
            Some(resource_ref),
        );
        activate.payload = json!({
            "checkpointRef": "checkpoint.port03.binding.source",
            "authorityEvidenceRef": "evidence.port03.binding.authority",
            "capabilityLeaseRefs": ["lease.port03.binding.provider"]
        });
        let activated = execute(&state_root, ProvisionerKind::Fake, activate.clone()).unwrap();
        assert_eq!(
            activated["activatedAgentRefs"][0],
            "agent.port03.binding.root"
        );
        assert_eq!(
            execute(&state_root, ProvisionerKind::Fake, activate).unwrap(),
            activated
        );

        let mut quiesce = request(
            "quiesce",
            "operation.port03.binding.quiesce",
            Some(resource_ref),
        );
        quiesce.payload =
            json!({ "graph": stage_request("unused").payload["bundle"]["graph"].clone() });
        execute(&state_root, ProvisionerKind::Fake, quiesce).unwrap();

        let mut checkpoint = request(
            "checkpoint",
            "operation.port03.binding.checkpoint",
            Some(resource_ref),
        );
        checkpoint.payload = json!({
            "checkpointRef": "checkpoint.port03.binding.managed",
            "eventLogCursor": 10
        });
        let checkpointed = execute(&state_root, ProvisionerKind::Fake, checkpoint).unwrap();
        assert_eq!(checkpointed["checkpoint"]["sourceGeneration"], 2);
        assert_eq!(
            checkpointed["checkpoint"]["checkpointRef"],
            "checkpoint.port03.binding.managed"
        );

        let mut reclaim = request(
            "reclaim",
            "operation.port03.binding.reclaim",
            Some(resource_ref),
        );
        reclaim.payload = json!({ "agentRefs": ["agent.port03.binding.root"] });
        let reclaimed = execute(&state_root, ProvisionerKind::Fake, reclaim.clone()).unwrap();
        assert_eq!(reclaimed["scratch"], "released");
        assert_eq!(
            execute(&state_root, ProvisionerKind::Fake, reclaim).unwrap(),
            reclaimed
        );
        let _ = fs::remove_dir_all(state_root);
    }

    #[test]
    fn continuation_executes_exact_graph_turns_once_without_journaling_tasks() {
        let state_root = root("continuation");
        let staged = execute(
            &state_root,
            ProvisionerKind::Fake,
            stage_request("operation.port03.binding.continue.stage"),
        )
        .unwrap();
        let resource_ref = staged["resourceRef"].as_str().unwrap();
        materialize_fake(&state_root, "operation.port03.binding.continue.materialize");
        let mut activate = request(
            "activate",
            "operation.port03.binding.continue.activate",
            Some(resource_ref),
        );
        activate.payload = json!({
            "authorityEvidenceRef": "evidence.port03.binding.authority",
            "capabilityLeaseRefs": ["lease.port03.binding.provider"]
        });
        execute(&state_root, ProvisionerKind::Fake, activate).unwrap();
        let continuation = PortableContinuationRequest {
            operation_ref: "operation.port03.binding.continue".to_string(),
            owner_ref: "owner.port03.binding".to_string(),
            target_ref: "target.port03.binding.managed".to_string(),
            session_ref: "session.port03.binding".to_string(),
            attachment_ref: "attachment.port03.binding.managed".to_string(),
            generation: 2,
            provider_lease_ref: "lease.port03.binding.provider".to_string(),
            turns: vec![PortableContinuationTurn {
                agent_ref: "agent.port03.binding.root".to_string(),
                turn_ref: "turn.port03.binding.root".to_string(),
                task: "PRIVATE-CONTINUATION-TASK-SENTINEL".to_string(),
            }],
        };
        let executed = continue_work(&state_root, ProvisionerKind::Fake, &continuation).unwrap();
        assert_eq!(executed["replay"], "executed");
        assert_eq!(
            continue_work(&state_root, ProvisionerKind::Fake, &continuation).unwrap()["replay"],
            "replayed"
        );
        let journal =
            fs::read_to_string(state_root.join("portable-agent-computers/operations").join(
                format!("{}.json", short_digest(&continuation.operation_ref)),
            ))
            .unwrap();
        assert!(!journal.contains("PRIVATE-CONTINUATION-TASK-SENTINEL"));
    }

    #[test]
    fn abort_prepared_compensates_and_cleanup_reconciles_a_missing_vm() {
        let state_root = root("compensation");
        let stage = stage_request("operation.port03.binding.compensated.stage");
        execute(&state_root, ProvisionerKind::Fake, stage.clone()).unwrap();
        let mut abort = request(
            "abortPrepared",
            "operation.port03.binding.compensated.abort-prepared",
            None,
        );
        abort.payload = json!({ "stageOperationRef": stage.operation_ref });
        let first = execute(&state_root, ProvisionerKind::Fake, abort.clone()).unwrap();
        assert_eq!(
            execute(&state_root, ProvisionerKind::Fake, abort).unwrap(),
            first
        );
        assert!(execute(&state_root, ProvisionerKind::Fake, stage).is_err());

        let resource_ref =
            staged_resource_ref("target.port03.binding.managed", "session.port03.binding");
        let path = resource_path(&state_root.join("portable-agent-computers"), &resource_ref);
        let mut resource = read_json::<RetainedResource>(&path).unwrap().unwrap();
        assert_eq!(resource.state, "reclaimed");
        resource.state = "teardown_pending".to_string();
        write_json_atomic(&path, &resource).unwrap();
        let mut reclaim = request(
            "reclaim",
            "operation.port03.binding.compensated.reclaim-replay",
            Some(&resource_ref),
        );
        reclaim.payload = json!({ "agentRefs": ["agent.port03.binding.root"] });
        assert_eq!(
            execute(&state_root, ProvisionerKind::Fake, reclaim).unwrap()["scratch"],
            "released"
        );
        assert_eq!(
            read_json::<RetainedResource>(&path).unwrap().unwrap().state,
            "reclaimed"
        );
    }

    #[test]
    fn capability_install_is_refs_only_and_wipe_is_journaled_without_material() {
        let state_root = root("capability");
        let staged = execute(
            &state_root,
            ProvisionerKind::Fake,
            stage_request("operation.port03.binding.capability-stage"),
        )
        .unwrap();
        let resource_ref = staged["resourceRef"].as_str().unwrap().to_string();
        materialize_fake(
            &state_root,
            "operation.port03.binding.capability-materialize",
        );
        let install_request = PortableCapabilityInstallRequest {
            operation_ref: "operation.port03.binding.capability-install".to_string(),
            owner_ref: "owner.port03.binding".to_string(),
            target_ref: "target.port03.binding.managed".to_string(),
            resource_ref: resource_ref.clone(),
            session_ref: "session.port03.binding".to_string(),
            attachment_ref: "attachment.port03.binding.managed".to_string(),
            generation: 2,
            lease_ref: "lease.port03.binding.provider".to_string(),
            evidence_ref: "evidence.port03.binding.provider".to_string(),
            capability: "capability.provider.codex".to_string(),
        };
        let mut material = b"opaque-test-material".to_vec();
        let installed = install_capability(
            &state_root,
            ProvisionerKind::Fake,
            install_request,
            &mut material,
        )
        .unwrap();
        assert_eq!(installed["material"], "excluded");

        let mut wipe = request(
            "wipeCapability",
            "operation.port03.binding.capability-wipe",
            Some(&resource_ref),
        );
        wipe.payload = json!({
            "leaseRef": "lease.port03.binding.provider",
            "installationRef": installed["installationRef"],
        });
        let wiped = execute(&state_root, ProvisionerKind::Fake, wipe).unwrap();
        assert_eq!(wiped["material"], "excluded");
        let journals = fs::read_dir(state_root.join("portable-agent-computers/operations"))
            .unwrap()
            .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
            .collect::<String>();
        assert!(!journals.contains("opaque-test-material"));
    }

    #[test]
    fn conflicting_replay_and_private_material_refuse_before_effects() {
        let state_root = root("refusal");
        let stage = stage_request("operation.port03.binding.stage");
        execute(&state_root, ProvisionerKind::Fake, stage.clone()).unwrap();
        let mut conflict = stage;
        conflict.payload["bundle"]["checkpoint"]["diffDigest"] =
            Value::String(format!("sha256:{}", "e".repeat(64)));
        assert!(matches!(
            execute(&state_root, ProvisionerKind::Fake, conflict),
            Err(CloudVmError::InvalidRequest(message)) if message.contains("different bytes")
        ));

        let mut unsafe_request = stage_request("operation.port03.binding.unsafe");
        unsafe_request.payload["password"] = Value::String("not-retainable".to_string());
        assert!(matches!(
            execute(&state_root, ProvisionerKind::Fake, unsafe_request),
            Err(CloudVmError::InvalidRequest(message)) if message.contains("forbidden private material")
        ));
        let _ = fs::remove_dir_all(state_root);
    }
}
