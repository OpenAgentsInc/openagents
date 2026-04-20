//! MCP server for OpenAgents compute control over the desktop-control contract.

#![allow(
    clippy::print_stdout,
    reason = "The MCP stdio transport writes protocol frames to stdout."
)]

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::blocking::Client;
use serde_json::{Map, Value, json};

use crate::desktop_control::{
    DesktopControlActionRequest, DesktopControlActionResponse, DesktopControlManifest,
    DesktopControlSnapshot, control_manifest_path,
};
use psionic_sandbox::ProviderSandboxEntrypointType;

const JSONRPC_VERSION: &str = "2.0";
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
const MCP_SERVER_NAME: &str = "openagents-compute-control";

#[derive(Clone, Debug)]
pub struct ResolvedDesktopControlTarget {
    pub base_url: String,
    pub auth_token: String,
    pub manifest_path: Option<PathBuf>,
}

impl ResolvedDesktopControlTarget {
    pub fn resolve(
        base_url: Option<&str>,
        auth_token: Option<&str>,
        manifest_path: Option<&Path>,
    ) -> Result<Self> {
        match (base_url, auth_token) {
            (Some(base_url), Some(auth_token)) => Ok(Self {
                base_url: base_url.trim().trim_end_matches('/').to_string(),
                auth_token: auth_token.trim().to_string(),
                manifest_path: manifest_path.map(Path::to_path_buf),
            }),
            (Some(_), None) | (None, Some(_)) => bail!(
                "--base-url and --auth-token must be provided together, or neither to use the manifest"
            ),
            (None, None) => {
                let manifest_path = manifest_path
                    .map(Path::to_path_buf)
                    .unwrap_or_else(control_manifest_path);
                let manifest = load_manifest_from_path(manifest_path.as_path())?;
                Ok(Self {
                    base_url: manifest.base_url.trim().trim_end_matches('/').to_string(),
                    auth_token: manifest.auth_token,
                    manifest_path: Some(manifest_path),
                })
            }
        }
    }
}

fn load_manifest_from_path(path: &Path) -> Result<DesktopControlManifest> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read desktop control manifest {}", path.display()))?;
    serde_json::from_str::<DesktopControlManifest>(raw.as_str())
        .with_context(|| format!("decode desktop control manifest {}", path.display()))
}

pub trait ComputeControlApi {
    fn snapshot(&self) -> Result<DesktopControlSnapshot>;
    fn action(&self, action: &DesktopControlActionRequest) -> Result<DesktopControlActionResponse>;
}

#[derive(Clone)]
pub struct DesktopControlHttpClient {
    http: Client,
    target: ResolvedDesktopControlTarget,
}

impl DesktopControlHttpClient {
    #[must_use]
    pub fn new(target: ResolvedDesktopControlTarget) -> Self {
        Self {
            http: Client::new(),
            target,
        }
    }

    fn authorized_get<T>(&self, path: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}/{}", self.target.base_url, path.trim_start_matches('/'));
        self.http
            .get(url)
            .bearer_auth(self.target.auth_token.as_str())
            .send()
            .context("request desktop control GET")?
            .error_for_status()
            .context("desktop control GET status")?
            .json::<T>()
            .context("decode desktop control GET response")
    }

    fn authorized_post<T>(&self, path: &str, body: &impl serde::Serialize) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}/{}", self.target.base_url, path.trim_start_matches('/'));
        self.http
            .post(url)
            .bearer_auth(self.target.auth_token.as_str())
            .json(body)
            .send()
            .context("request desktop control POST")?
            .error_for_status()
            .context("desktop control POST status")?
            .json::<T>()
            .context("decode desktop control POST response")
    }
}

impl ComputeControlApi for DesktopControlHttpClient {
    fn snapshot(&self) -> Result<DesktopControlSnapshot> {
        self.authorized_get("/v1/snapshot")
    }

    fn action(&self, action: &DesktopControlActionRequest) -> Result<DesktopControlActionResponse> {
        self.authorized_post("/v1/action", action)
    }
}

pub struct ComputeMcpServer<C> {
    client: C,
}

impl<C> ComputeMcpServer<C>
where
    C: ComputeControlApi,
{
    #[must_use]
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub fn handle_request(&self, request: &Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str)?;
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let response = match self.dispatch(method, &params) {
            Ok(Some(result)) => json!({
                "jsonrpc": JSONRPC_VERSION,
                "id": id,
                "result": result,
            }),
            Ok(None) => return None,
            Err(error) => json!({
                "jsonrpc": JSONRPC_VERSION,
                "id": id,
                "error": {
                    "code": -32000,
                    "message": error.to_string(),
                }
            }),
        };
        Some(response)
    }

    fn dispatch(&self, method: &str, params: &Value) -> Result<Option<Value>> {
        match method {
            "initialize" => Ok(Some(initialize_result())),
            "initialized" | "notifications/initialized" => Ok(None),
            "ping" => Ok(Some(json!({}))),
            "tools/list" => Ok(Some(json!({ "tools": tool_definitions() }))),
            "resources/list" => Ok(Some(json!({ "resources": [] }))),
            "prompts/list" => Ok(Some(json!({ "prompts": [] }))),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("tools/call requires params.name"))?;
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Object(Map::new()));
                Ok(Some(self.call_tool(name, &arguments)?))
            }
            _ => bail!("unsupported MCP method `{method}`"),
        }
    }

    fn call_tool(&self, name: &str, arguments: &Value) -> Result<Value> {
        match name {
            "compute_snapshot" => {
                let snapshot = self.client.snapshot()?;
                Ok(tool_success(
                    "Captured desktop compute snapshot",
                    serde_json::to_value(snapshot).context("encode desktop snapshot")?,
                ))
            }
            "compute_inventory_status" => {
                let snapshot = self.client.snapshot()?;
                let inventory = json!({
                    "provider": snapshot.provider,
                    "local_runtime": snapshot.local_runtime,
                    "cluster": snapshot.cluster,
                    "sandbox": snapshot.sandbox,
                    "proofs": snapshot.proofs,
                    "challenges": snapshot.challenges,
                });
                Ok(tool_success("Captured compute inventory status", inventory))
            }
            "compute_provider_mode_set" => {
                let online = required_bool(arguments, "online")?;
                self.call_action(DesktopControlActionRequest::SetProviderMode { online })
            }
            "compute_cluster_status" => {
                self.call_action(DesktopControlActionRequest::GetClusterStatus)
            }
            "compute_cluster_topology" => {
                self.call_action(DesktopControlActionRequest::GetClusterTopology)
            }
            "compute_sandbox_status" => {
                self.call_action(DesktopControlActionRequest::GetSandboxStatus)
            }
            "compute_sandbox_create_job" => {
                self.call_action(DesktopControlActionRequest::CreateSandboxJob {
                    profile_id: required_string(arguments, "profile_id")?,
                    job_id: required_string(arguments, "job_id")?,
                    workspace_root: required_string(arguments, "workspace_root")?,
                    entrypoint_type: parse_entrypoint_type(arguments.get("entrypoint_type"))?,
                    entrypoint: required_string(arguments, "entrypoint")?,
                    payload: optional_string(arguments, "payload"),
                    arguments: optional_string_list(arguments, "arguments")?,
                    expected_outputs: optional_string_list(arguments, "expected_outputs")?,
                    timeout_request_s: optional_u64(arguments, "timeout_request_s").unwrap_or(60),
                    network_request: optional_string(arguments, "network_request")
                        .unwrap_or_else(|| "host_inherit".to_string()),
                    filesystem_request: optional_string(arguments, "filesystem_request")
                        .unwrap_or_else(|| "host_inherit".to_string()),
                    payout_reference: optional_string(arguments, "payout_reference"),
                    verification_posture: optional_string(arguments, "verification_posture"),
                })
            }
            "compute_sandbox_get_job" => {
                self.call_action(DesktopControlActionRequest::GetSandboxJob {
                    job_id: required_string(arguments, "job_id")?,
                })
            }
            "compute_sandbox_upload_file" => {
                let content_base64 =
                    if let Some(value) = optional_string(arguments, "content_base64") {
                        value
                    } else if let Some(value) = optional_string(arguments, "utf8_text") {
                        URL_SAFE_NO_PAD.encode(value.as_bytes())
                    } else {
                        bail!("compute_sandbox_upload_file requires content_base64 or utf8_text");
                    };
                self.call_action(DesktopControlActionRequest::UploadSandboxFile {
                    job_id: required_string(arguments, "job_id")?,
                    relative_path: required_string(arguments, "relative_path")?,
                    content_base64,
                })
            }
            "compute_sandbox_start_job" => {
                self.call_action(DesktopControlActionRequest::StartSandboxJob {
                    job_id: required_string(arguments, "job_id")?,
                })
            }
            "compute_sandbox_wait_job" => {
                self.call_action(DesktopControlActionRequest::WaitSandboxJob {
                    job_id: required_string(arguments, "job_id")?,
                    timeout_ms: optional_u64(arguments, "timeout_ms").unwrap_or(20_000),
                })
            }
            "compute_sandbox_download_artifact" => {
                self.call_action(DesktopControlActionRequest::DownloadSandboxArtifact {
                    job_id: required_string(arguments, "job_id")?,
                    relative_path: required_string(arguments, "relative_path")?,
                })
            }
            "compute_sandbox_download_workspace_file" => {
                self.call_action(DesktopControlActionRequest::DownloadSandboxWorkspaceFile {
                    job_id: required_string(arguments, "job_id")?,
                    relative_path: required_string(arguments, "relative_path")?,
                })
            }
            "compute_proof_status" => self.call_action(DesktopControlActionRequest::GetProofStatus),
            "compute_challenge_status" => {
                self.call_action(DesktopControlActionRequest::GetChallengeStatus)
            }
            _ => Ok(tool_error(format!("unknown MCP tool `{name}`"))),
        }
    }

    fn call_action(&self, action: DesktopControlActionRequest) -> Result<Value> {
        let response = self.client.action(&action)?;
        if !response.success {
            return Ok(tool_error(response.message));
        }
        Ok(tool_success(
            response.message,
            json!({
                "payload": response.payload,
                "snapshot_revision": response.snapshot_revision,
                "state_signature": response.state_signature,
            }),
        ))
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "serverInfo": {
            "name": MCP_SERVER_NAME,
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "tools": {},
        },
        "instructions": "OpenAgents compute MCP forwards tool calls into the running desktop-control plane and preserves the same auth, policy, and app-owned truth contracts used by autopilotctl.",
    })
}

fn tool_success(summary: impl Into<String>, structured_content: Value) -> Value {
    let summary = summary.into();
    json!({
        "content": [
            {
                "type": "text",
                "text": format!("{summary}\n{}", pretty_json(&structured_content)),
            }
        ],
        "structuredContent": structured_content,
        "isError": false,
    })
}

fn tool_error(message: impl Into<String>) -> Value {
    let message = message.into();
    json!({
        "content": [
            {
                "type": "text",
                "text": message,
            }
        ],
        "isError": true,
    })
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn tool_definitions() -> Vec<Value> {
    vec![
        tool_definition(
            "compute_snapshot",
            "Fetch the full desktop-control compute snapshot from the running Autopilot app.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_inventory_status",
            "Fetch the app-owned compute inventory summary: provider mode, runtime, cluster, sandbox, proof, and challenge posture.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_provider_mode_set",
            "Turn the provider online or offline through the desktop-control policy gate.",
            json!({
                "type": "object",
                "properties": {
                    "online": { "type": "boolean", "description": "True to request provider online mode; false to request offline mode." }
                },
                "required": ["online"],
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_cluster_status",
            "Inspect cluster membership and transport status through the desktop-control surface.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_cluster_topology",
            "Inspect the current cluster topology payload through the desktop-control surface.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_sandbox_status",
            "Inspect declared sandbox profiles and current desktop-owned sandbox jobs.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_sandbox_create_job",
            "Create a sandbox job through the app-owned control plane.",
            json!({
                "type": "object",
                "properties": {
                    "profile_id": { "type": "string" },
                    "job_id": { "type": "string" },
                    "workspace_root": { "type": "string" },
                    "entrypoint_type": { "type": "string", "enum": ["workspace_file", "inline_payload", "command"] },
                    "entrypoint": { "type": "string" },
                    "payload": { "type": "string" },
                    "arguments": { "type": "array", "items": { "type": "string" } },
                    "expected_outputs": { "type": "array", "items": { "type": "string" } },
                    "timeout_request_s": { "type": "integer", "minimum": 1 },
                    "network_request": { "type": "string" },
                    "filesystem_request": { "type": "string" },
                    "payout_reference": { "type": "string" },
                    "verification_posture": { "type": "string" }
                },
                "required": ["profile_id", "job_id", "workspace_root", "entrypoint_type", "entrypoint"],
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_sandbox_get_job",
            "Inspect one sandbox job by id.",
            json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" }
                },
                "required": ["job_id"],
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_sandbox_upload_file",
            "Upload a workspace file into a desktop-owned sandbox job. Provide either `content_base64` or `utf8_text`.",
            json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" },
                    "relative_path": { "type": "string" },
                    "content_base64": { "type": "string" },
                    "utf8_text": { "type": "string" }
                },
                "required": ["job_id", "relative_path"],
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_sandbox_start_job",
            "Start a previously created sandbox job.",
            json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" }
                },
                "required": ["job_id"],
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_sandbox_wait_job",
            "Wait for a sandbox job to reach a terminal state.",
            json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" },
                    "timeout_ms": { "type": "integer", "minimum": 1 }
                },
                "required": ["job_id"],
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_sandbox_download_artifact",
            "Download a declared sandbox artifact. The result payload includes base64 content plus receipt metadata.",
            json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" },
                    "relative_path": { "type": "string" }
                },
                "required": ["job_id", "relative_path"],
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_sandbox_download_workspace_file",
            "Download a file from the sandbox workspace. The result payload includes base64 content plus receipt metadata.",
            json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" },
                    "relative_path": { "type": "string" }
                },
                "required": ["job_id", "relative_path"],
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_proof_status",
            "Inspect proof status through the same app-owned control plane used by autopilotctl.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        ),
        tool_definition(
            "compute_challenge_status",
            "Inspect challenge status through the same app-owned control plane used by autopilotctl.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        ),
    ]
}

fn tool_definition(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

fn required_string(arguments: &Value, key: &str) -> Result<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("tool arguments require string `{key}`"))
}

fn optional_string(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn required_bool(arguments: &Value, key: &str) -> Result<bool> {
    arguments
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("tool arguments require boolean `{key}`"))
}

fn optional_u64(arguments: &Value, key: &str) -> Option<u64> {
    arguments.get(key).and_then(Value::as_u64)
}

fn optional_string_list(arguments: &Value, key: &str) -> Result<Vec<String>> {
    let Some(values) = arguments.get(key) else {
        return Ok(Vec::new());
    };
    let array = values
        .as_array()
        .ok_or_else(|| anyhow!("tool argument `{key}` must be an array of strings"))?;
    array
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("tool argument `{key}` must contain only strings"))
        })
        .collect()
}

fn parse_entrypoint_type(value: Option<&Value>) -> Result<ProviderSandboxEntrypointType> {
    match value.and_then(Value::as_str) {
        Some("workspace_file") | None => Ok(ProviderSandboxEntrypointType::WorkspaceFile),
        Some("inline_payload") => Ok(ProviderSandboxEntrypointType::InlinePayload),
        Some("command") => Ok(ProviderSandboxEntrypointType::Command),
        Some(other) => bail!("unsupported sandbox entrypoint_type `{other}`"),
    }
}

pub fn run_stdio_server(target: ResolvedDesktopControlTarget) -> Result<()> {
    let client = DesktopControlHttpClient::new(target);
    let server = ComputeMcpServer::new(client);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    while let Some(message) = read_jsonrpc_message(&mut reader)? {
        if let Some(response) = server.handle_request(&message) {
            write_jsonrpc_message(&mut writer, &response)?;
        }
    }
    Ok(())
}

fn read_jsonrpc_message<R: BufRead>(reader: &mut R) -> Result<Option<Value>> {
    let mut content_length = None;
    let mut saw_header = false;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .context("read MCP header line")?;
        if bytes == 0 {
            return if saw_header {
                Err(anyhow!("unexpected EOF while reading MCP headers"))
            } else {
                Ok(None)
            };
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        saw_header = true;
        if let Some((name, value)) = trimmed.split_once(':')
            && name.eq_ignore_ascii_case("Content-Length")
        {
            let parsed = value
                .trim()
                .parse::<usize>()
                .with_context(|| format!("parse MCP Content-Length header `{trimmed}`"))?;
            content_length = Some(parsed);
        }
    }
    let content_length =
        content_length.ok_or_else(|| anyhow!("missing MCP Content-Length header"))?;
    let mut body = vec![0_u8; content_length];
    reader
        .read_exact(body.as_mut_slice())
        .context("read MCP message body")?;
    Ok(Some(
        serde_json::from_slice::<Value>(body.as_slice()).context("decode MCP message body")?,
    ))
}

fn write_jsonrpc_message<W: Write>(writer: &mut W, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value).context("encode MCP response body")?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len()).context("write MCP headers")?;
    writer
        .write_all(body.as_slice())
        .context("write MCP response body")?;
    writer.flush().context("flush MCP response")
}

#[cfg(test)]
mod tests {
    use super::{
        ComputeControlApi, ComputeMcpServer, read_jsonrpc_message, tool_definitions,
        write_jsonrpc_message,
    };
    use crate::desktop_control::{
        DesktopControlActionRequest, DesktopControlActionResponse, DesktopControlChallengeStatus,
        DesktopControlClusterStatus, DesktopControlLocalRuntimeStatus, DesktopControlProofStatus,
        DesktopControlProviderStatus, DesktopControlSandboxStatus, DesktopControlSnapshot,
    };
    use serde_json::{Value, json};
    use std::collections::VecDeque;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingClient {
        actions: Arc<Mutex<Vec<DesktopControlActionRequest>>>,
        responses: Arc<Mutex<VecDeque<DesktopControlActionResponse>>>,
        snapshot: DesktopControlSnapshot,
    }

    impl RecordingClient {
        fn with_snapshot(snapshot: DesktopControlSnapshot) -> Self {
            Self {
                snapshot,
                ..Self::default()
            }
        }

        fn push_response(&self, response: DesktopControlActionResponse) {
            self.responses
                .lock()
                .expect("responses lock")
                .push_back(response);
        }

        fn recorded_actions(&self) -> Vec<DesktopControlActionRequest> {
            self.actions.lock().expect("actions lock").clone()
        }
    }

    impl ComputeControlApi for RecordingClient {
        fn snapshot(&self) -> anyhow::Result<DesktopControlSnapshot> {
            Ok(self.snapshot.clone())
        }

        fn action(
            &self,
            action: &DesktopControlActionRequest,
        ) -> anyhow::Result<DesktopControlActionResponse> {
            self.actions
                .lock()
                .expect("actions lock")
                .push(action.clone());
            Ok(self
                .responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .unwrap_or_else(|| DesktopControlActionResponse {
                    success: true,
                    message: "ok".to_string(),
                    payload: Some(json!({ "echo": action })),
                    snapshot_revision: Some(7),
                    state_signature: Some("sig-7".to_string()),
                }))
        }
    }

    fn sample_snapshot() -> DesktopControlSnapshot {
        DesktopControlSnapshot {
            provider: DesktopControlProviderStatus {
                online: true,
                runtime_mode: "seller".to_string(),
                ..DesktopControlProviderStatus::default()
            },
            local_runtime: DesktopControlLocalRuntimeStatus {
                lane: Some("cuda".to_string()),
                runtime_ready: true,
                ..DesktopControlLocalRuntimeStatus::default()
            },
            cluster: DesktopControlClusterStatus {
                available: true,
                topology_label: "replicated".to_string(),
                member_count: 2,
                ..DesktopControlClusterStatus::default()
            },
            sandbox: DesktopControlSandboxStatus {
                available: true,
                job_count: 1,
                ..DesktopControlSandboxStatus::default()
            },
            proofs: DesktopControlProofStatus {
                available: true,
                pending_count: 2,
                ..DesktopControlProofStatus::default()
            },
            challenges: DesktopControlChallengeStatus {
                available: true,
                open_count: 1,
                ..DesktopControlChallengeStatus::default()
            },
            ..DesktopControlSnapshot::default()
        }
    }

    #[test]
    fn tool_definitions_surface_compute_contracts() {
        let tools = tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(names.contains(&"compute_inventory_status"));
        assert!(names.contains(&"compute_cluster_status"));
        assert!(names.contains(&"compute_sandbox_create_job"));
        assert!(names.contains(&"compute_proof_status"));
        assert!(names.contains(&"compute_challenge_status"));
    }

    #[test]
    fn server_maps_representative_tools_to_desktop_actions() {
        let client = RecordingClient::with_snapshot(sample_snapshot());
        let server = ComputeMcpServer::new(client.clone());
        client.push_response(DesktopControlActionResponse {
            success: true,
            message: "Created sandbox job".to_string(),
            payload: Some(json!({ "job_id": "job-1" })),
            snapshot_revision: Some(11),
            state_signature: Some("sig-11".to_string()),
        });
        client.push_response(DesktopControlActionResponse {
            success: true,
            message: "Waited for sandbox job".to_string(),
            payload: Some(json!({ "job_id": "job-1", "state": "succeeded" })),
            snapshot_revision: Some(12),
            state_signature: Some("sig-12".to_string()),
        });

        let inventory = server
            .call_tool("compute_inventory_status", &json!({}))
            .expect("inventory tool");
        assert_eq!(
            inventory.get("isError").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            inventory
                .get("structuredContent")
                .and_then(|value| value.get("cluster"))
                .and_then(|value| value.get("member_count"))
                .and_then(Value::as_u64),
            Some(2)
        );

        let create = server
            .call_tool(
                "compute_sandbox_create_job",
                &json!({
                    "profile_id": "pythonexec-profile",
                    "job_id": "job-1",
                    "workspace_root": "/tmp/openagents",
                    "entrypoint_type": "workspace_file",
                    "entrypoint": "scripts/job.py",
                    "expected_outputs": ["result.txt"]
                }),
            )
            .expect("create tool");
        assert_eq!(create.get("isError").and_then(Value::as_bool), Some(false));

        let wait = server
            .call_tool(
                "compute_sandbox_wait_job",
                &json!({
                    "job_id": "job-1",
                    "timeout_ms": 30_000
                }),
            )
            .expect("wait tool");
        assert_eq!(wait.get("isError").and_then(Value::as_bool), Some(false));

        assert_eq!(
            client.recorded_actions(),
            vec![
                DesktopControlActionRequest::CreateSandboxJob {
                    profile_id: "pythonexec-profile".to_string(),
                    job_id: "job-1".to_string(),
                    workspace_root: "/tmp/openagents".to_string(),
                    entrypoint_type: psionic_sandbox::ProviderSandboxEntrypointType::WorkspaceFile,
                    entrypoint: "scripts/job.py".to_string(),
                    payload: None,
                    arguments: Vec::new(),
                    expected_outputs: vec!["result.txt".to_string()],
                    timeout_request_s: 60,
                    network_request: "host_inherit".to_string(),
                    filesystem_request: "host_inherit".to_string(),
                    payout_reference: None,
                    verification_posture: None,
                },
                DesktopControlActionRequest::WaitSandboxJob {
                    job_id: "job-1".to_string(),
                    timeout_ms: 30_000,
                }
            ]
        );
    }

    #[test]
    fn sandbox_upload_accepts_utf8_text_and_encodes_base64() {
        let client = RecordingClient::default();
        let server = ComputeMcpServer::new(client.clone());
        let _ = server
            .call_tool(
                "compute_sandbox_upload_file",
                &json!({
                    "job_id": "job-1",
                    "relative_path": "notes.txt",
                    "utf8_text": "hello sandbox"
                }),
            )
            .expect("upload tool");
        assert_eq!(
            client.recorded_actions(),
            vec![DesktopControlActionRequest::UploadSandboxFile {
                job_id: "job-1".to_string(),
                relative_path: "notes.txt".to_string(),
                content_base64: "aGVsbG8gc2FuZGJveA".to_string(),
            }]
        );
    }

    #[test]
    fn stdio_message_round_trip_uses_content_length_frames() {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "ping"
        });
        let mut encoded = Vec::new();
        write_jsonrpc_message(&mut encoded, &request).expect("encode framed request");
        let mut cursor = Cursor::new(encoded);
        let decoded = read_jsonrpc_message(&mut cursor).expect("decode frame");
        assert_eq!(decoded, Some(request));
    }
}
