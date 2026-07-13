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
        return stage(root, provisioner, effective_kind, request);
    }
    let resource_ref = request.resource_ref.as_deref().ok_or_else(|| {
        CloudVmError::InvalidRequest("resourceRef is required after stage".to_string())
    })?;
    let resource_path = resource_path(root, resource_ref);
    let mut resource = read_json::<RetainedResource>(&resource_path)?.ok_or_else(|| {
        CloudVmError::InvalidRequest("retained resource was not found".to_string())
    })?;
    assert_resource(&resource, request)?;

    let response = if effective_kind == ProvisionerKind::Fake {
        fake_response(&resource, request)?
    } else {
        live_guest_response(provisioner, &resource.vm, request)?
    };

    match request.action.as_str() {
        "activate" => resource.state = "active".to_string(),
        "quiesce" => resource.state = "quiesced".to_string(),
        "abort" | "reclaim" => {
            provisioner.teardown(&resource.vm)?;
            resource.state = "reclaimed".to_string();
        }
        "checkpoint" => {}
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

fn stage(
    root: &Path,
    provisioner: &dyn CloudVmProvisioner,
    effective_kind: ProvisionerKind,
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
    let resource_ref = format!(
        "resource.agent-computer.{}",
        short_digest(&format!("{}|{}", request.target_ref, request.session_ref))
    );
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
        return if effective_kind == ProvisionerKind::Fake {
            fake_stage_response(&resource)
        } else {
            let mut response = live_guest_response(provisioner, &resource.vm, request)?;
            response
                .as_object_mut()
                .ok_or_else(|| {
                    CloudVmError::Runtime("stage response is not an object".to_string())
                })?
                .insert(
                    "resourceRef".to_string(),
                    Value::String(resource.resource_ref.clone()),
                );
            Ok(response)
        };
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
        state: "staged".to_string(),
        checkpoint: checkpoint.clone(),
        execution_binding,
        graph: graph.clone(),
        thread_cursors: thread_cursors.clone(),
    };
    let mut response = if effective_kind == ProvisionerKind::Fake {
        fake_stage_response(&resource)?
    } else {
        live_guest_response(provisioner, &resource.vm, request)?
    };
    response
        .as_object_mut()
        .ok_or_else(|| CloudVmError::Runtime("stage response is not an object".to_string()))?
        .insert(
            "resourceRef".to_string(),
            Value::String(resource_ref.clone()),
        );
    write_json_atomic(&retained_path, &resource)?;
    Ok(response)
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
        "reclaim" if !matches!(resource.state.as_str(), "quiesced" | "reclaimed") => Err(
            CloudVmError::InvalidRequest("reclaim requires quiescence".to_string()),
        ),
        "abort" if !matches!(resource.state.as_str(), "staged" | "reclaimed") => Err(
            CloudVmError::InvalidRequest("only a staged resource may abort".to_string()),
        ),
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
