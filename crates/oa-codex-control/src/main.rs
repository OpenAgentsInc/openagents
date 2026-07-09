use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use openagents_cloud_contract::{
    AgentComputerIsolationPolicy, ArtanisBootstrapAssignment, CodexAuthGrant, CodexRequestedMode,
    CodexSandboxMode,
    CodexWorkroomAssignment, ComputeLane, ComputeQuotaCaps, ComputeUsage, LaneCostModel,
    ModelUsageRecord, PlacementAssignment, ProviderLane, ResourceHostSnapshot,
    ResourceUsageReceipt, RunResourceUsage, RunnerBinding, TokenCountSource, TrainingRetentionMode,
    TrainingRunAssignment, VirtualizationFacts, CODEX_AUTH_GRANT_VERSION,
    CODEX_WORKROOM_ASSIGNMENT_VERSION, GCE_EPHEMERAL_CAPACITY_CLASS_ID,
    RESOURCE_USAGE_RECEIPT_VERSION, SHC_FALLBACK_RUNNER_ID,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

mod gce_capacity;
use gce_capacity::{provisioner_for, CapacityRequest, GceLease, ProvisionerKind, ReleaseReason};

mod cloud_vm;

const DEFAULT_BIND: &str = "127.0.0.1:8787";
const DEFAULT_CODEX_BIN: &str = "/usr/local/bin/codex";
const DEFAULT_OPENCODE_BIN: &str = "/usr/local/bin/opencode";
const DEFAULT_STATE_ROOT: &str = "/var/lib/openagents/codex-control";
const DEFAULT_WORKROOMD_BIN: &str = "/usr/local/bin/oa-workroomd";
const MAX_BODY_BYTES: usize = 128 * 1024;
static JSON_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);
/// cloud#97: count of queued jobs the internal tick worker has dispatched and
/// not yet observed reach a terminal state. Bounds queue concurrency.
static QUEUE_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone)]
struct Config {
    auth_json_file: Option<PathBuf>,
    auth_json_root: Option<PathBuf>,
    bind: String,
    codex_bin: PathBuf,
    github_write_grant_resolver: Option<CodexGrantResolver>,
    local_auth_without_grant_resolver: bool,
    provider_account_ref: Option<String>,
    opencode_bin: PathBuf,
    state_root: PathBuf,
    token: String,
    grant_resolver: Option<CodexGrantResolver>,
    event_ingest: Option<EventIngest>,
    workroomd_bin: PathBuf,
    /// Placement policy: whether the GCE primary lane is currently selectable
    /// (cloud#86/#88). Defaults to true so owner sessions land on GCE by
    /// default; full GCE provisioning/warm-pool is deferred to the density
    /// phase. Set OA_CODEX_PLACEMENT_GCE_AVAILABLE=false to force SHC.
    placement_gce_available: bool,
    /// SHC secondary/fallback runner id; defaults to oa-shc-katy-01.
    placement_shc_runner_id: String,
    /// CND-042: whether `Auto` placement compares lanes on measured
    /// cost-plus-10% instead of using the policy-driven Google-first default.
    /// Default true per the CND-042 report; set
    /// OA_CODEX_PLACEMENT_COST_DRIVEN=false to restore policy-driven Google-first.
    /// Even when true, GCE wins ties/near-ties and SHC is chosen only when it is
    /// materially cheaper AND `placement_shc_pilot_expand` is set.
    placement_cost_driven: bool,
    /// CND-042: whether the SHC pilot recommendation is "expand". The report
    /// recommends HOLD, so this defaults false: SHC is never promoted on cost
    /// alone. Set OA_CODEX_PLACEMENT_SHC_PILOT_EXPAND=true only after a real SHC
    /// invoice + metered GCE receipts justify expansion.
    placement_shc_pilot_expand: bool,
    /// Which GCE provisioner backs the `cloud-gcp` lane: `fake` (default,
    /// dry-run, no GCP calls) or `live` (gated behind ADC + raw project id). Set
    /// OA_CODEX_GCE_PROVISIONER=live to attempt real provisioning. The live path
    /// shells `gcloud` and additionally requires OA_CODEX_GCE_PROJECT_ID (raw
    /// project id) plus optional OA_CODEX_GCE_ZONE / OA_CODEX_GCE_MACHINE_TYPE /
    /// OA_CODEX_GCE_IMAGE_FAMILY / OA_CODEX_GCE_IMAGE_PROJECT / OA_CODEX_GCE_GCLOUD_BIN
    /// overrides (see gce_capacity::LiveGceConfig). Without ADC or the raw
    /// project id, the live lane falls back to fake so no-cloud envs never bill.
    gce_provisioner_kind: ProvisionerKind,
    /// Which Cloud-VM provisioner backs the qa-runner CloudVm seam (#6200):
    /// `fake` (default, deterministic, no KVM) or `live` (gated firecracker,
    /// requires a Linux KVM host + OA_CLOUD_VM_KERNEL_IMAGE/ROOTFS_IMAGE). Set
    /// OA_CLOUD_VM_PROVISIONER=live to attempt real microVM provisioning; without
    /// /dev/kvm or the images the live lane falls back to fake so no-KVM envs
    /// never boot. See cloud_vm::LiveFirecrackerConfig for the full env set.
    cloud_vm_provisioner_kind: cloud_vm::ProvisionerKind,
    /// Redacted OpenAgents GCP project ref for the GCE capacity class. Never a
    /// raw project id. Set OA_CODEX_GCE_PROJECT_REF to override.
    gce_project_ref: String,
    /// Runtime identity ref used for Application Default Credentials.
    gce_provisioner_identity_ref: String,
    /// cloud#97: durable unattended queue config. When enabled, an internal tick
    /// worker drains queued coding jobs on the configured lane with no external
    /// driver, bounded by `queue_max_concurrency`.
    queue: QueueConfig,
}

/// cloud#97 durable unattended queue settings.
#[derive(Debug, Clone)]
struct QueueConfig {
    /// Whether the internal tick worker runs (OA_CODEX_QUEUE_ENABLED).
    enabled: bool,
    /// Default lane applied to a dequeued job that did not pin a lane.
    lane: ComputeLane,
    /// Maximum number of concurrently-dispatched queued jobs.
    max_concurrency: usize,
    /// Poll interval for the tick worker.
    tick_ms: u64,
}

impl QueueConfig {
    fn from_env() -> Self {
        Self {
            enabled: env_flag("OA_CODEX_QUEUE_ENABLED"),
            lane: optional_env("OA_CODEX_QUEUE_LANE")
                .as_deref()
                .and_then(parse_compute_lane)
                .unwrap_or(ComputeLane::CloudGcp),
            max_concurrency: optional_env("OA_CODEX_QUEUE_MAX_CONCURRENCY")
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(1),
            tick_ms: optional_env("OA_CODEX_QUEUE_TICK_MS")
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(2000),
        }
    }
}

fn parse_compute_lane(value: &str) -> Option<ComputeLane> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(ComputeLane::Auto),
        "local" => Some(ComputeLane::Local),
        "cloud-gcp" | "cloud_gcp" | "gcp" | "gce" => Some(ComputeLane::CloudGcp),
        "cloud-shc" | "cloud_shc" | "shc" => Some(ComputeLane::CloudShc),
        _ => None,
    }
}

/// Caller-neutral grant resolver binding. The endpoint may still point at the
/// reused Vortex credential/endpoint, but the daemon no longer couples to the
/// deprecated Vortex codebase (cloud#87).
#[derive(Debug, Clone)]
struct CodexGrantResolver {
    token: String,
    url: String,
}

/// Caller-neutral event ingest binding for `openagents.codex_workroom_event.v1`
/// callbacks. A generic Pylon control front door consumes these the same way
/// the deprecated Vortex ingest did (cloud#87).
#[derive(Debug, Clone)]
struct EventIngest {
    token: String,
    url: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum AgentRuntime {
    Codex,
    OpencodeCodex,
}

impl AgentRuntime {
    fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::OpencodeCodex => "opencode_codex",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::OpencodeCodex => "OpenCode/Codex",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlRequest {
    agent_runtime: Option<AgentRuntime>,
    auth_grant_ref: String,
    github_write_connection_ref: Option<String>,
    github_write_grant_ref: Option<String>,
    github_work_order: Option<GitHubWorkOrder>,
    goal: String,
    /// Optional lane-agnostic selector (cloud#86). When set on a run
    /// assignment, the daemon resolves the runner/lane via placement policy
    /// (GCE primary, SHC secondary) instead of trusting a caller-supplied
    /// `runner_id`.
    lane: Option<ComputeLane>,
    /// Optional redacted owner ref for per-owner quota evaluation.
    owner_ref: Option<String>,
    provider_account_ref: String,
    repository: Option<String>,
    repository_clone_url: Option<String>,
    repository_ref: Option<String>,
    required_artifacts: Option<Vec<String>>,
    retention_mode: Option<String>,
    runner_id: String,
    run_id: String,
    sandbox_mode: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubWorkOrder {
    provider: String,
    repository: GitHubRepositoryRef,
    base_ref: String,
    branch_name: String,
    commit_message: String,
    issue_comment: Option<String>,
    issue_number: Option<u64>,
    issue_url: Option<String>,
    pull_request_body: Option<String>,
    pull_request_title: Option<String>,
    writeback: GitHubWritebackPlan,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubRepositoryRef {
    provider: String,
    owner: String,
    repo: String,
    #[serde(rename = "ref")]
    ref_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubWritebackPlan {
    comment_on_issue: bool,
    open_pull_request: bool,
    push_branch: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FollowUpRequest {
    auth_grant_ref: Option<String>,
    instruction: Option<String>,
    prompt: Option<String>,
    reason: Option<String>,
    run_id: Option<String>,
    turn_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlResponse {
    events: Vec<ControlEvent>,
    external_run_id: String,
    run: ControlRun,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlRun {
    external_run_id: String,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlEvent {
    artifact_refs: Vec<String>,
    data_json: Option<String>,
    detail: Option<String>,
    kind: String,
    receipt_refs: Vec<String>,
    redacted: bool,
    summary: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct JobRecord {
    cancel_requested: bool,
    created_at_ms: u128,
    external_run_id: String,
    last_sequence: u64,
    request: ControlRequest,
    run_id: String,
    runner_id: String,
    status: String,
    updated_at_ms: u128,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct JobEvent {
    artifact_refs: Vec<String>,
    created_at_ms: u128,
    data_json: Option<String>,
    detail: Option<String>,
    digest: Option<String>,
    kind: String,
    receipt_refs: Vec<String>,
    redacted: bool,
    sequence: u64,
    source: String,
    summary: String,
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlacementResponse {
    binding: RunnerBinding,
    external_run_id: String,
    run: ControlRun,
    status: String,
    events: Vec<ControlEvent>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JobEventsResponse {
    events: Vec<JobEvent>,
    next_cursor: u64,
    run: ControlRun,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: String,
    events: Vec<ControlEvent>,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VortexGrantResolveRequest<'a> {
    auth_grant_ref: &'a str,
    include_auth_material: bool,
    provider_account_ref: &'a str,
    run_id: &'a str,
    runner_id: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VortexGrantResolveResponse {
    auth_material: Option<VortexAuthMaterial>,
    grant: VortexResolvedGrant,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VortexAuthMaterial {
    auth_content_env: String,
    auth_content_json: String,
}

#[derive(Debug)]
struct ResolvedCodexAuthGrant {
    auth_material: Option<VortexAuthMaterial>,
    grant: VortexResolvedGrant,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VortexResolvedGrant {
    expires_at: u128,
    grant_ref: String,
    provider: String,
    provider_account_ref: String,
    provider_secret_ref: String,
    requested_action: Option<String>,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubWriteGrantResolveRequest<'a> {
    github_write_grant_ref: &'a str,
    run_id: &'a str,
    runner_id: &'a str,
    runner_session_id: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubWriteGrantResolveResponse {
    grant: GitHubResolvedWriteGrant,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubResolvedWriteGrant {
    connection_ref: String,
    credential: GitHubResolvedCredential,
    expires_at: u128,
    github_login: String,
    grant_ref: String,
    materialization: GitHubWriteMaterialization,
    requested_action: Option<String>,
    runner_session_id: Option<String>,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubResolvedCredential {
    access_token: String,
    provider: String,
    scopes: Vec<String>,
    token_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubWriteMaterialization {
    auth_ref: String,
    git_credential_env: String,
    provider: String,
    remote_url_mode: String,
    scrub_after_closeout: bool,
}

fn main() {
    if env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "oa-codex-control\n\nUsage:\n  oa-codex-control --help\n  oa-codex-control --version\n  OA_CODEX_CONTROL_TOKEN=<token> OA_CODEX_AUTH_JSON_ROOT=<dir> OA_CODEX_GRANT_RESOLVE_URL=<url> OA_CODEX_RUNNER_GRANT_TOKEN=<token> oa-codex-control\n\nNeutral env vars (preferred) fall back to legacy OA_VORTEX_* when unset:\n  OA_CODEX_GRANT_RESOLVE_URL   <- OA_VORTEX_GRANT_RESOLVE_URL\n  OA_CODEX_RUNNER_GRANT_TOKEN  <- OA_VORTEX_CLOUD_RUNNER_GRANT_TOKEN\n  OA_CODEX_EVENT_INGEST_URL    <- OA_VORTEX_CODEX_INGEST_URL\n  OA_CODEX_EVENT_INGEST_TOKEN  <- OA_VORTEX_CODEX_INGEST_TOKEN\n"
        );
        return;
    }
    if env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!("oa-codex-control {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    if config.queue.enabled {
        eprintln!(
            "oa-codex-control queue worker enabled (lane={}, max_concurrency={}, tick_ms={})",
            queue_config_lane_label(&config.queue.lane),
            config.queue.max_concurrency,
            config.queue.tick_ms
        );
        let queue_config = config.clone();
        thread::spawn(move || run_queue_tick_loop(queue_config));
    }

    let listener = TcpListener::bind(&config.bind).unwrap_or_else(|error| {
        panic!("failed to bind {}: {error}", config.bind);
    });
    eprintln!("oa-codex-control listening on {}", config.bind);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = config.clone();
                thread::spawn(move || {
                    if let Err(error) = handle_stream(stream, &config) {
                        eprintln!("request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let token = required_env("OA_CODEX_CONTROL_TOKEN")?;
        let auth_json_file = optional_env("OA_CODEX_AUTH_JSON_FILE").map(PathBuf::from);
        let auth_json_root = optional_env("OA_CODEX_AUTH_JSON_ROOT").map(PathBuf::from);
        if auth_json_file.is_none() && auth_json_root.is_none() {
            return Err("missing OA_CODEX_AUTH_JSON_ROOT or OA_CODEX_AUTH_JSON_FILE".to_string());
        }
        let local_auth_without_grant_resolver = env_flag("OA_CODEX_CONTROL_ALLOW_LOCAL_AUTH_ONLY");
        let grant_resolver = grant_resolver_from_env(local_auth_without_grant_resolver)?;
        let github_write_grant_resolver = github_write_grant_resolver_from_env();

        Ok(Self {
            auth_json_file,
            auth_json_root,
            bind: env::var("OA_CODEX_CONTROL_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string()),
            codex_bin: env::var("OA_CODEX_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_CODEX_BIN)),
            github_write_grant_resolver,
            local_auth_without_grant_resolver,
            provider_account_ref: env::var("OA_CODEX_PROVIDER_ACCOUNT_REF")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            opencode_bin: env::var("OA_OPENCODE_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_OPENCODE_BIN)),
            state_root: env::var("OA_CODEX_CONTROL_STATE_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_STATE_ROOT)),
            token: token.clone(),
            grant_resolver,
            event_ingest: event_ingest_from_env(&token)?,
            workroomd_bin: env::var("OA_WORKROOMD_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_WORKROOMD_BIN)),
            placement_gce_available: optional_env("OA_CODEX_PLACEMENT_GCE_AVAILABLE")
                .map(|value| {
                    let normalized = value.to_ascii_lowercase();
                    !(normalized == "0" || normalized == "false" || normalized == "no")
                })
                .unwrap_or(true),
            placement_shc_runner_id: optional_env("OA_CODEX_PLACEMENT_SHC_RUNNER_ID")
                .unwrap_or_else(|| SHC_FALLBACK_RUNNER_ID.to_string()),
            placement_cost_driven: optional_env("OA_CODEX_PLACEMENT_COST_DRIVEN")
                .map(|value| {
                    let normalized = value.to_ascii_lowercase();
                    !(normalized == "0" || normalized == "false" || normalized == "no")
                })
                .unwrap_or(true),
            placement_shc_pilot_expand: optional_env("OA_CODEX_PLACEMENT_SHC_PILOT_EXPAND")
                .map(|value| {
                    let normalized = value.to_ascii_lowercase();
                    normalized == "1" || normalized == "true" || normalized == "yes"
                })
                .unwrap_or(false),
            gce_provisioner_kind: ProvisionerKind::from_env_value(
                optional_env("OA_CODEX_GCE_PROVISIONER").as_deref(),
            ),
            cloud_vm_provisioner_kind: cloud_vm::ProvisionerKind::from_env_value(
                optional_env("OA_CLOUD_VM_PROVISIONER").as_deref(),
            ),
            gce_project_ref: optional_env("OA_CODEX_GCE_PROJECT_REF")
                .unwrap_or_else(|| "gcp-project-ref://openagents/cloud-primary".to_string()),
            gce_provisioner_identity_ref: optional_env("OA_CODEX_GCE_PROVISIONER_IDENTITY_REF")
                .unwrap_or_else(|| "gce-provisioner://openagents/cloud".to_string()),
            queue: QueueConfig::from_env(),
        })
    }
}

fn handle_stream(mut stream: TcpStream, config: &Config) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(15)))
        .map_err(|error| format!("failed to set write timeout: {error}"))?;
    let request = read_http_request(&mut stream)?;
    let (path, query) = split_path_query(&request.path);

    if request.method == "GET" && path == "/healthz" {
        return write_response(
            &mut stream,
            200,
            &serde_json::json!({ "status": "ok", "service": "oa-codex-control" }),
        );
    }

    if !authorized(&request.authorization, &config.token) {
        return write_response(
            &mut stream,
            401,
            &ErrorResponse {
                error: "unauthorized".to_string(),
                events: vec![failed_event("Codex control API request was unauthorized.")],
                status: "failed".to_string(),
            },
        );
    }

    if request.method == "GET" {
        return handle_get_route(&mut stream, config, path, query);
    }

    if request.method != "POST" {
        return write_not_found(&mut stream);
    }

    match path {
        "/v1/codex-runs" | "/v1/codex-runs/start" => {
            let control_request = match decode_control_request(&request.body) {
                Ok(value) => value,
                Err(error) => return write_invalid_request(&mut stream, error),
            };
            let (status, response) = match start_codex_run_async(config, control_request) {
                Ok(response) => (
                    202,
                    serde_json::to_value(response).map_err(|error| error.to_string())?,
                ),
                Err(error) => runner_failed_response(error)?,
            };

            write_response(&mut stream, status, &response)
        }
        "/v1/queue" | "/v1/queue/start" => {
            let control_request = match decode_control_request(&request.body) {
                Ok(value) => value,
                Err(error) => return write_invalid_request(&mut stream, error),
            };
            let (status, response) = match enqueue_codex_run(config, control_request) {
                Ok(response) => (
                    202,
                    serde_json::to_value(response).map_err(|error| error.to_string())?,
                ),
                Err(error) => runner_failed_response(error)?,
            };

            write_response(&mut stream, status, &response)
        }
        "/v1/placement" | "/v1/placement/start" => {
            let assignment = match decode_placement_assignment(&request.body) {
                Ok(value) => value,
                Err(error) => return write_invalid_request(&mut stream, error),
            };
            let (status, response) = match start_placement_async(config, assignment) {
                Ok(response) => (
                    202,
                    serde_json::to_value(response).map_err(|error| error.to_string())?,
                ),
                Err(error) => runner_failed_response(error)?,
            };

            write_response(&mut stream, status, &response)
        }
        "/v1/training-runs" | "/v1/training-runs/start" => {
            let assignment = match decode_training_run_assignment(&request.body) {
                Ok(value) => value,
                Err(error) => return write_invalid_request(&mut stream, error),
            };
            let (status, response) = match start_training_run_async(config, assignment) {
                Ok(response) => (
                    202,
                    serde_json::to_value(response).map_err(|error| error.to_string())?,
                ),
                Err(error) => runner_failed_response(error)?,
            };

            write_response(&mut stream, status, &response)
        }
        "/v1/artanis/bootstrap" | "/v1/artanis/bootstrap/start" => {
            let assignment = match decode_artanis_bootstrap_assignment(&request.body) {
                Ok(value) => value,
                Err(error) => return write_invalid_request(&mut stream, error),
            };
            let (status, response) = match start_artanis_bootstrap_async(config, assignment) {
                Ok(response) => (
                    202,
                    serde_json::to_value(response).map_err(|error| error.to_string())?,
                ),
                Err(error) => runner_failed_response(error)?,
            };

            write_response(&mut stream, status, &response)
        }
        "/v1/cloud-vm/sessions" | "/v1/cloud-vm/sessions/start" => {
            // Production cross-OS Cloud-VM provisioner seam (cloud, issue #6200):
            // satisfies the qa-runner `CloudVmProvisionerV2` lifecycle
            // (provision -> exec -> copyOut -> teardown) over firecracker microVMs.
            let cloud_vm_request = match decode_cloud_vm_request(&request.body) {
                Ok(value) => value,
                Err(error) => return write_invalid_request(&mut stream, error),
            };
            let (status, response) = match run_cloud_vm_session_route(config, cloud_vm_request) {
                Ok(response) => (
                    200,
                    serde_json::to_value(response).map_err(|error| error.to_string())?,
                ),
                Err(error) => cloud_vm_failed_response(error)?,
            };

            write_response(&mut stream, status, &response)
        }
        "/v1/workrooms/codex/start" => {
            let control_request = match decode_control_request(&request.body) {
                Ok(value) => value,
                Err(error) => return write_invalid_request(&mut stream, error),
            };
            let (status, response) = match start_codex_run(config, control_request) {
                Ok(response) => (
                    200,
                    serde_json::to_value(response).map_err(|error| error.to_string())?,
                ),
                Err(error) => runner_failed_response(error)?,
            };

            write_response(&mut stream, status, &response)
        }
        "/v1/codex-runs/continue" => handle_follow_up_route(
            &mut stream,
            config,
            &request.body,
            None,
            "turn.continue_requested",
        ),
        "/v1/codex-runs/steer" => handle_follow_up_route(
            &mut stream,
            config,
            &request.body,
            None,
            "turn.steer_requested",
        ),
        "/v1/codex-runs/cancel" => handle_cancel_route(&mut stream, config, &request.body, None),
        _ => {
            if let Some((run_id, action)) = parse_run_action(path) {
                match action {
                    "turns" => handle_follow_up_route(
                        &mut stream,
                        config,
                        &request.body,
                        Some(run_id),
                        "turn.continue_requested",
                    ),
                    "cancel" => {
                        handle_cancel_route(&mut stream, config, &request.body, Some(run_id))
                    }
                    _ => write_not_found(&mut stream),
                }
            } else {
                write_not_found(&mut stream)
            }
        }
    }
}

fn handle_get_route(
    stream: &mut TcpStream,
    config: &Config,
    path: &str,
    query: Option<&str>,
) -> Result<(), String> {
    let Some(rest) = path.strip_prefix("/v1/codex-runs/") else {
        return write_not_found(stream);
    };
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let Some(run_id) = parts.next() else {
        return write_not_found(stream);
    };
    let action = parts.next();

    match action {
        None => match load_job_record(config, run_id) {
            Ok(job) => write_response(
                stream,
                200,
                &response_from_job(&job, load_job_events(config, run_id, 0)?),
            ),
            Err(error) => write_response(
                stream,
                404,
                &ErrorResponse {
                    error,
                    events: Vec::new(),
                    status: "failed".to_string(),
                },
            ),
        },
        Some("events") => {
            let cursor = cursor_from_query(query);
            let job = match load_job_record(config, run_id) {
                Ok(job) => job,
                Err(error) => {
                    return write_response(
                        stream,
                        404,
                        &ErrorResponse {
                            error,
                            events: Vec::new(),
                            status: "failed".to_string(),
                        },
                    )
                }
            };
            let events = load_job_events(config, run_id, cursor)?;
            let next_cursor = events
                .iter()
                .map(|event| event.sequence)
                .max()
                .unwrap_or(cursor);
            write_response(
                stream,
                200,
                &JobEventsResponse {
                    events,
                    next_cursor,
                    run: ControlRun {
                        external_run_id: job.external_run_id.clone(),
                        status: job.status.clone(),
                    },
                    status: job.status.clone(),
                },
            )
        }
        Some("stream") => {
            let cursor = cursor_from_query(query);
            let job = match load_job_record(config, run_id) {
                Ok(job) => job,
                Err(error) => {
                    return write_response(
                        stream,
                        404,
                        &ErrorResponse {
                            error,
                            events: Vec::new(),
                            status: "failed".to_string(),
                        },
                    )
                }
            };
            let events = load_job_events(config, run_id, cursor)?;
            let body = sse_snapshot(&job, &events);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-store\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            )
            .map_err(|error| error.to_string())?;
            stream
                .write_all(body.as_bytes())
                .map_err(|error| error.to_string())
        }
        _ => write_not_found(stream),
    }
}

fn handle_follow_up_route(
    stream: &mut TcpStream,
    config: &Config,
    body: &[u8],
    route_run_id: Option<&str>,
    event_type: &str,
) -> Result<(), String> {
    let follow_up = decode_follow_up_request(body)?;
    let Some(run_id) = route_run_id
        .map(ToString::to_string)
        .or_else(|| follow_up.run_id.clone())
    else {
        return write_invalid_request(stream, "missing runId".to_string());
    };

    let prompt = follow_up
        .prompt
        .or(follow_up.instruction)
        .unwrap_or_else(|| "Continuation requested without prompt content.".to_string());
    let auth_grant_ref = follow_up.auth_grant_ref.clone();
    let turn_id = follow_up.turn_id.clone();
    let event = append_job_event(
        config,
        &run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "authGrantRef": auth_grant_ref,
                "promptLength": prompt.len(),
                "turnId": turn_id,
            }))?),
            detail: Some(redact_for_log(&prompt)),
            digest: None,
            kind: "turn".to_string(),
            receipt_refs: Vec::new(),
            redacted: contains_secret_marker(&prompt),
            source: "control".to_string(),
            summary: "Continuation turn was queued for the Codex supervisor.".to_string(),
            type_: event_type.to_string(),
        },
    )?;
    queue_follow_up_turn(config, &run_id, &prompt, follow_up.auth_grant_ref)?;
    send_pending_callbacks(config, &run_id)?;
    let job = load_job_record(config, &run_id)?;
    write_response(
        stream,
        202,
        &serde_json::json!({
            "accepted": true,
            "event": event,
            "runId": run_id,
            "status": job.status
        }),
    )
}

fn queue_follow_up_turn(
    config: &Config,
    run_id: &str,
    prompt: &str,
    auth_grant_ref: Option<String>,
) -> Result<(), String> {
    let mut job = load_job_record(config, run_id)?;
    if job.status == "running" {
        job.status = "waiting_for_input".to_string();
        job.updated_at_ms = now_ms()?;
        save_job_record(config, &job)?;
        return Ok(());
    }

    job.status = "queued".to_string();
    job.cancel_requested = false;
    job.request.goal = prompt.to_string();
    if let Some(auth_grant_ref) = auth_grant_ref {
        job.request.auth_grant_ref = auth_grant_ref;
    }
    job.updated_at_ms = now_ms()?;
    let request = job.request.clone();
    save_job_record(config, &job)?;

    let worker_config = config.clone();
    thread::spawn(move || {
        if let Err(error) = run_codex_worker(worker_config, request) {
            eprintln!("{}", redact_for_log(&error));
        }
    });

    Ok(())
}

fn handle_cancel_route(
    stream: &mut TcpStream,
    config: &Config,
    body: &[u8],
    route_run_id: Option<&str>,
) -> Result<(), String> {
    let follow_up = decode_follow_up_request(body)?;
    let Some(run_id) = route_run_id
        .map(ToString::to_string)
        .or_else(|| follow_up.run_id.clone())
    else {
        return write_invalid_request(stream, "missing runId".to_string());
    };
    let reason = follow_up
        .reason
        .unwrap_or_else(|| "Cancellation requested through Codex control API.".to_string());
    request_job_cancel(config, &run_id, &reason)?;
    send_pending_callbacks(config, &run_id)?;
    let job = load_job_record(config, &run_id)?;
    write_response(
        stream,
        202,
        &serde_json::json!({
            "accepted": true,
            "cancelRequested": true,
            "run": {
                "externalRunId": job.external_run_id.clone(),
                "status": job.status.clone()
            },
            "runId": run_id
        }),
    )
}

fn decode_control_request(body: &[u8]) -> Result<ControlRequest, String> {
    serde_json::from_slice::<ControlRequest>(body)
        .map_err(|error| format!("invalid_request: {error}"))
}

fn decode_training_run_assignment(body: &[u8]) -> Result<TrainingRunAssignment, String> {
    serde_json::from_slice::<TrainingRunAssignment>(body)
        .map_err(|error| format!("invalid_request: {error}"))
}

fn decode_placement_assignment(body: &[u8]) -> Result<PlacementAssignment, String> {
    serde_json::from_slice::<PlacementAssignment>(body)
        .map_err(|error| format!("invalid_request: {error}"))
}

fn decode_artanis_bootstrap_assignment(body: &[u8]) -> Result<ArtanisBootstrapAssignment, String> {
    serde_json::from_slice::<ArtanisBootstrapAssignment>(body)
        .map_err(|error| format!("invalid_request: {error}"))
}

/// HTTP request body for the Cloud-VM session route (issue #6200). This is the
/// over-the-wire shape the qa-runner's `CloudVmProvisionerV2` calls: it carries
/// the run id, OS tier (`linux`/`macos`/`windows`), the redacted target name +
/// owner ref, and the in-VM session command. The host extraction dir is chosen
/// by the daemon under its state root and returned as `extractedTo`, so a caller
/// never supplies a host path.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudVmSessionRequest {
    run_id: String,
    /// `linux` | `macos` | `windows` (qa-runner `CloudVmOs`).
    os: String,
    /// Redacted target name (public-safe label; never a credential).
    target_name: String,
    /// Redacted owner ref for audit/quota.
    owner_ref: String,
    /// The command run INSIDE the VM to produce the session + artifacts; it must
    /// write outputs under `/qa/artifacts`. Mirrors the container backend's
    /// `sessionCommand`.
    session_command: Vec<String>,
}

fn decode_cloud_vm_request(body: &[u8]) -> Result<CloudVmSessionRequest, String> {
    serde_json::from_slice::<CloudVmSessionRequest>(body)
        .map_err(|error| format!("invalid_request: {error}"))
}

/// Run a full Cloud-VM session (provision -> exec -> copyOut -> teardown) and
/// return the public-safe outcome. Owner-gated/default-OFF: the live firecracker
/// lane is opt-in via `OA_CLOUD_VM_PROVISIONER=live` and additionally requires a
/// Linux KVM host + configured kernel/rootfs images. Absent those it uses the
/// deterministic fake lane (no KVM, no fake green for the live path) so no-KVM
/// hosts and tests never attempt a real boot.
fn run_cloud_vm_session_route(
    config: &Config,
    request: CloudVmSessionRequest,
) -> Result<cloud_vm::CloudVmSessionOutcome, cloud_vm::CloudVmError> {
    let os = cloud_vm::CloudVmOs::parse(&request.os)
        .map_err(cloud_vm::CloudVmError::InvalidRequest)?;
    let vm_request = cloud_vm::CloudVmRequest {
        run_id: request.run_id.clone(),
        os,
        target_name: request.target_name,
        owner_ref: request.owner_ref,
    };

    // Per-run host extraction dir under the daemon's state root. The caller never
    // supplies a host path; the daemon owns where artifacts land.
    let host_artifact_dir = config
        .state_root
        .join("cloud-vm-artifacts")
        .join(short_digest_hex(&request.run_id));

    let (provisioner, _kind) = cloud_vm::provisioner_for(config.cloud_vm_provisioner_kind);
    let now = now_ms().map_err(cloud_vm::CloudVmError::Runtime)?;
    cloud_vm::run_cloud_vm_session(
        provisioner.as_ref(),
        &vm_request,
        &request.session_command,
        &host_artifact_dir,
        now,
    )
}

fn cloud_vm_failed_response(error: cloud_vm::CloudVmError) -> Result<(u16, Value), String> {
    // Map the typed provisioner error to an honest HTTP status. Invalid requests
    // and unavailable OS tiers are 4xx; KVM-unavailable / runtime failures are
    // 5xx. None of these fake a green.
    let status = match error {
        cloud_vm::CloudVmError::InvalidRequest(_)
        | cloud_vm::CloudVmError::OsTierUnavailable(_) => 400,
        cloud_vm::CloudVmError::KvmUnavailable(_) | cloud_vm::CloudVmError::Runtime(_) => 500,
    };
    Ok((
        status,
        serde_json::to_value(ErrorResponse {
            error: "cloud_vm_failed".to_string(),
            events: vec![failed_event(error.message().as_str())],
            status: "failed".to_string(),
        })
        .map_err(|error| error.to_string())?,
    ))
}

/// Short hex digest used to name the per-run host artifact dir (filesystem-safe).
fn short_digest_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn decode_follow_up_request(body: &[u8]) -> Result<FollowUpRequest, String> {
    if body.is_empty() {
        return Ok(FollowUpRequest {
            auth_grant_ref: None,
            instruction: None,
            prompt: None,
            reason: None,
            run_id: None,
            turn_id: None,
        });
    }
    serde_json::from_slice::<FollowUpRequest>(body)
        .map_err(|error| format!("invalid_request: {error}"))
}

fn runner_failed_response(error: String) -> Result<(u16, Value), String> {
    Ok((
        500,
        serde_json::to_value(ErrorResponse {
            error: "runner_failed".to_string(),
            events: vec![failed_event(error.as_str())],
            status: "failed".to_string(),
        })
        .map_err(|error| error.to_string())?,
    ))
}

fn write_invalid_request(stream: &mut TcpStream, error: String) -> Result<(), String> {
    write_response(
        stream,
        400,
        &ErrorResponse {
            error,
            events: vec![failed_event(
                "Codex control API received an invalid request.",
            )],
            status: "failed".to_string(),
        },
    )
}

fn write_not_found(stream: &mut TcpStream) -> Result<(), String> {
    write_response(
        stream,
        404,
        &ErrorResponse {
            error: "not_found".to_string(),
            events: Vec::new(),
            status: "failed".to_string(),
        },
    )
}

fn start_codex_run(config: &Config, request: ControlRequest) -> Result<ControlResponse, String> {
    if let Some(expected) = &config.provider_account_ref {
        if request.provider_account_ref != *expected {
            return Err("provider account ref is not allowed on this runner".to_string());
        }
    }

    let agent_runtime = agent_runtime_for_request(&request);
    let now = now_ms()?;
    let run_dir = config
        .state_root
        .join(safe_path_component(&request.run_id))
        .join(now.to_string());
    fs::create_dir_all(&run_dir).map_err(|error| format!("failed to create run dir: {error}"))?;
    let state_dir = run_dir.join("state");
    let grant_file = run_dir.join("grant.json");
    let assignment_file = run_dir.join("assignment.json");
    let sandbox = parse_sandbox_mode(request.sandbox_mode.as_deref());
    let workroom_id = format!("workroom_{}", safe_path_component(&request.run_id));
    let timeout_ms = request
        .timeout_ms
        .unwrap_or(300_000)
        .clamp(1, 60 * 60 * 1000);
    let required_artifacts = required_artifacts_for_request(&request);
    if let Some(order) = &request.github_work_order {
        validate_github_work_order(order, &request)?;
    }
    let resolved_auth_grant = resolve_codex_auth_grant(config, &request, now)?;
    let resolved_grant = &resolved_auth_grant.grant;
    let resolved_github_write_grant = resolve_github_write_grant(config, &request, now)?;
    let prompt = codex_prompt(
        &request.goal,
        &required_artifacts,
        request.github_work_order.as_ref(),
        resolved_github_write_grant.as_ref(),
    );
    let auth_json_file = codex_auth_cache_path(config, &resolved_auth_grant, &request, &run_dir)?;
    validate_codex_auth_cache_file(&auth_json_file)?;
    let auth_event = auth_grant_resolved_event(resolved_grant);
    let github_write_event = resolved_github_write_grant
        .as_ref()
        .map(github_write_grant_resolved_event);
    let grant_expires_at_ms = resolved_grant.expires_at.min(now + timeout_ms as u128);

    let grant = CodexAuthGrant {
        contract_version: CODEX_AUTH_GRANT_VERSION.to_string(),
        workroom_id: workroom_id.clone(),
        user_ref: request
            .run_id
            .strip_prefix("workroom_")
            .map(|suffix| format!("vortex-run:{suffix}"))
            .unwrap_or_else(|| format!("vortex-run:{}", request.run_id)),
        organization_ref: None,
        project_ref: None,
        provider_account_ref: request.provider_account_ref.clone(),
        grant_ref: request.auth_grant_ref.clone(),
        provider_secret_ref: resolved_grant.provider_secret_ref.clone(),
        requested_mode: CodexRequestedMode::Exec,
        issued_at_ms: now,
        expires_at_ms: grant_expires_at_ms,
        audit_context: format!("vortex.control.run:{}", request.run_id),
    };
    grant.validate_for_session(now)?;

    let assignment = CodexWorkroomAssignment {
        contract_version: CODEX_WORKROOM_ASSIGNMENT_VERSION.to_string(),
        assignment_id: request.run_id.clone(),
        workroom_id,
        target_node_id: request.runner_id.clone(),
        user_ref: format!("vortex-thread:{}", request.run_id),
        organization_ref: None,
        project_ref: None,
        provider_account_ref: request.provider_account_ref.clone(),
        auth_grant_ref: request.auth_grant_ref.clone(),
        repo_ref: assignment_repo_ref(&request),
        prompt,
        required_artifacts,
        sandbox,
        timeout_ms: Some(timeout_ms as u128),
        wallet_authority: false,
        created_at_ms: now,
        audit_context: format!("vortex.control.run:{}", request.run_id),
    };
    assignment.validate_contract(now)?;

    write_json_file(&grant_file, &grant)?;
    write_json_file(&assignment_file, &assignment)?;

    run_workroomd(
        config,
        &[
            "codex",
            "auth",
            "materialize",
            "--grant-file",
            path_str(&grant_file)?,
            "--auth-json-file",
            path_str(&auth_json_file)?,
            "--state-dir",
            path_str(&state_dir)?,
            "--json",
        ],
    )?;
    run_workroomd(
        config,
        &[
            "codex",
            "auth",
            "status",
            "--codex-bin",
            path_str(&config.codex_bin)?,
            "--state-dir",
            path_str(&state_dir)?,
            "--json",
        ],
    )?;

    let github_write_env = github_write_environment(&resolved_github_write_grant);
    let run_result = run_workroomd_with_env(
        config,
        &[
            "codex",
            "run",
            "--assignment-file",
            path_str(&assignment_file)?,
            "--agent-runtime",
            agent_runtime.as_str(),
            "--codex-bin",
            path_str(&config.codex_bin)?,
            "--opencode-bin",
            path_str(&config.opencode_bin)?,
            "--state-dir",
            path_str(&state_dir)?,
            "--json",
        ],
        &github_write_env,
    );

    let raw = match run_result {
        Ok(stdout) => stdout,
        Err(error) => {
            let events = load_event_log(&state_dir)?;
            let runner_events = load_runner_event_log(&state_dir)?;
            let external_run_id = external_run_id(&request.runner_id, &request.run_id);
            eprintln!("{}", redact_for_log(&error));
            let mut normalized = vec![auth_event];
            if let Some(github_write_event) = github_write_event.clone() {
                normalized.push(github_write_event);
            }
            normalized.extend(normalized_events(events, true));
            normalized.extend(normalized_runner_events(runner_events));
            return Ok(ControlResponse {
                events: normalized,
                external_run_id: external_run_id.clone(),
                run: ControlRun {
                    external_run_id,
                    status: "failed".to_string(),
                },
                status: "failed".to_string(),
            });
        }
    };

    let parsed: Value =
        serde_json::from_str(&raw).map_err(|error| format!("invalid runner output: {error}"))?;
    let status = parsed
        .pointer("/state/status")
        .and_then(Value::as_str)
        .unwrap_or("completed")
        .to_string();
    let events = parsed
        .pointer("/state/events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let runner_events = parsed
        .pointer("/runner_events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let external_run_id = external_run_id(&request.runner_id, &request.run_id);
    let mut normalized = vec![auth_event];
    if let Some(github_write_event) = github_write_event {
        normalized.push(github_write_event);
    }
    normalized.extend(normalized_events(events, false));
    normalized.extend(normalized_runner_events(runner_events));

    Ok(ControlResponse {
        events: normalized,
        external_run_id: external_run_id.clone(),
        run: ControlRun {
            external_run_id,
            status: status.clone(),
        },
        status,
    })
}

fn required_artifacts_for_request(request: &ControlRequest) -> Vec<String> {
    let mut artifacts = request
        .required_artifacts
        .clone()
        .unwrap_or_else(|| vec!["result.md".to_string()]);
    if request.github_work_order.is_some()
        && !artifacts
            .iter()
            .any(|artifact| artifact == "github-writeback.json")
    {
        artifacts.push("github-writeback.json".to_string());
    }
    artifacts
}

fn assignment_repo_ref(request: &ControlRequest) -> String {
    let Some(repository) = request.repository.as_ref() else {
        return "none".to_string();
    };

    if repository.contains('@') {
        return repository.clone();
    }

    request
        .repository_ref
        .as_deref()
        .filter(|ref_name| !ref_name.trim().is_empty())
        .map(|ref_name| format!("{repository}@{ref_name}"))
        .unwrap_or_else(|| repository.clone())
}

/// Resolve a runner binding for a lane-agnostic assignment using fleet
/// placement policy (cloud#86/#88). Cost-driven since CND-042: for a non-pinned
/// `Auto` assignment with both lanes eligible, placement compares lanes on
/// measured cost-plus-10% (per the CND-042 report). Google GCE wins ties and
/// near-ties (owner direction); SHC is chosen only when materially cheaper AND
/// the SHC pilot recommendation is "expand". Set OA_CODEX_PLACEMENT_COST_DRIVEN
/// =false to restore the policy-driven Google-first default.
fn resolve_placement_binding(
    config: &Config,
    assignment: &PlacementAssignment,
) -> Result<RunnerBinding, String> {
    assignment.resolve_runner_binding_cost_aware(
        config.placement_gce_available,
        &config.placement_shc_runner_id,
        ComputeQuotaCaps::default(),
        config.placement_cost_driven,
        config.placement_shc_pilot_expand,
        LaneCostModel::default(),
    )
}

/// Build a `ControlRequest` from a placement assignment + resolved binding so
/// the existing async Codex run path executes the placed run on the bound
/// runner/lane.
fn control_request_from_placement(
    assignment: &PlacementAssignment,
    binding: &RunnerBinding,
) -> ControlRequest {
    ControlRequest {
        agent_runtime: None,
        auth_grant_ref: assignment.auth_grant_ref.clone(),
        github_write_connection_ref: None,
        github_write_grant_ref: None,
        github_work_order: None,
        goal: assignment.goal.clone(),
        lane: Some(binding.lane),
        owner_ref: Some(assignment.owner_ref.clone()),
        provider_account_ref: assignment.provider_account_ref.clone(),
        repository: assignment.repository.clone(),
        repository_clone_url: None,
        repository_ref: None,
        required_artifacts: None,
        retention_mode: None,
        runner_id: binding.runner_id.clone(),
        run_id: assignment.run_id.clone(),
        sandbox_mode: Some(binding.sandbox_mode.clone()),
        timeout_ms: None,
    }
}

fn placement_bound_event(binding: &RunnerBinding) -> ControlEvent {
    ControlEvent {
        artifact_refs: Vec::new(),
        data_json: serde_json::to_string(binding).ok(),
        detail: None,
        kind: "placement.bound".to_string(),
        receipt_refs: Vec::new(),
        redacted: false,
        summary: format!(
            "Placement bound run to {} lane on runner {} ({}).",
            binding.lane.as_str(),
            binding.runner_id,
            placement_reason_summary(binding)
        ),
    }
}

fn placement_reason_summary(binding: &RunnerBinding) -> &'static str {
    use openagents_cloud_contract::PlacementReason;
    match binding.reason {
        PlacementReason::LanePinned => "caller-pinned lane",
        PlacementReason::PolicyDefaultGce => "policy default GCE primary",
        PlacementReason::GceUnavailableShcFallback => "GCE unavailable, SHC fallback",
        PlacementReason::CostDriven => "cost-driven lane selection (CND-042)",
    }
}

fn start_placement_async(
    config: &Config,
    assignment: PlacementAssignment,
) -> Result<PlacementResponse, String> {
    let now = now_ms()?;
    assignment.validate_contract(now)?;

    // CX-1 (openagents#8545) provider-credential law: the custodied subscription
    // grant is broker-redeemed only, owner-scoped, never pooled, and never
    // resold. Enforce it fail-closed before any runner binds. The default policy
    // encodes the lawful posture; a placement missing its per-session
    // broker-redeemed grant (`auth_grant_ref`), missing its owner-scoped provider
    // account, or requesting wallet authority is refused here rather than run.
    AgentComputerIsolationPolicy::default().validate_placement(&assignment, None)?;

    let binding = resolve_placement_binding(config, &assignment)?;

    // AC-1 (openagents#8503): org-cloud-runtime microVM lane. When the caller
    // pins/binds `cloud-gcp` AND ships an opaque work-context blob, run the turn
    // inside a Firecracker microVM (`cloud_vm::run_cloud_vm_session`) instead of
    // the Codex runner. The lane is gated on a genuinely live provisioner; a
    // non-KVM / unarmed host refuses honestly rather than faking a boot.
    if binding.lane == ComputeLane::CloudGcp {
        if let Some(work_context_b64) = assignment.work_context_b64.clone() {
            return start_org_cloud_microvm_placement(config, &assignment, &binding, work_context_b64);
        }
    }

    let request = control_request_from_placement(&assignment, &binding);
    let bound_event = placement_bound_event(&binding);

    let run_response = start_codex_run_async_with_initial_events(
        config,
        request,
        vec![JobEventInput {
            artifact_refs: Vec::new(),
            data_json: bound_event.data_json.clone(),
            detail: bound_event.detail.clone(),
            digest: None,
            kind: bound_event.kind.clone(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "placement".to_string(),
            summary: bound_event.summary.clone(),
            type_: bound_event.kind.clone(),
        }],
    )?;

    let mut events = vec![bound_event];
    events.extend(run_response.events);

    Ok(PlacementResponse {
        binding: binding.clone(),
        external_run_id: binding.external_run_id.clone(),
        run: ControlRun {
            external_run_id: binding.external_run_id,
            status: run_response.status.clone(),
        },
        status: run_response.status,
        events,
    })
}

/// AC-1 (openagents#8503) org-cloud-runtime microVM placement. Runs the in-guest
/// turn-runner inside a Firecracker microVM via `cloud_vm::run_cloud_vm_session`,
/// emitting `openagents.resource_usage_receipt.v1`-shaped lifecycle events
/// (provisioning -> provisioned -> resource_usage_receipt -> cleanup) so the
/// caller's isolation-posture validator sees the same work-context ref plus the
/// scratch-wipe and microvm-destroy reclaim receipt refs. Gated on a genuinely
/// live provisioner: a non-KVM / unarmed host refuses honestly (no fake boot).
fn start_org_cloud_microvm_placement(
    config: &Config,
    assignment: &PlacementAssignment,
    binding: &RunnerBinding,
    work_context_b64: String,
) -> Result<PlacementResponse, String> {
    // Guard the opaque blob: base64 alphabet only, so single-quoting it into the
    // guest shell command cannot inject. Reject anything else rather than run it.
    if !is_valid_work_context_b64(&work_context_b64) {
        return Err("invalid_request: work_context_b64 must be non-empty base64".to_string());
    }

    // Refuse (never fake) when this host has no live Firecracker cloud-vm lane.
    let (_probe, effective_kind) = cloud_vm::provisioner_for(config.cloud_vm_provisioner_kind);
    if effective_kind != cloud_vm::ProvisionerKind::Live {
        return Err(
            "org-cloud microVM lane requires a live Firecracker provisioner \
             (OA_CLOUD_VM_PROVISIONER=live + KVM + kernel/rootfs); refusing rather \
             than faking a boot"
                .to_string(),
        );
    }

    let now = now_ms()?;
    let external_run_id = external_run_id(&binding.runner_id, &assignment.run_id);
    let work_context_ref = assignment
        .work_context_ref
        .clone()
        .unwrap_or_else(|| format!("work-context.agent-computer.{}", short_digest_hex(&assignment.run_id)));

    let job = JobRecord {
        cancel_requested: false,
        created_at_ms: now,
        external_run_id: external_run_id.clone(),
        last_sequence: 0,
        request: control_request_from_placement(assignment, binding),
        run_id: assignment.run_id.clone(),
        runner_id: binding.runner_id.clone(),
        status: "provisioning".to_string(),
        updated_at_ms: now,
    };
    fs::create_dir_all(job_dir(config, &assignment.run_id))
        .map_err(|error| format!("failed to create microVM job registry: {error}"))?;
    save_job_record(config, &job)?;

    let mut response_events = Vec::new();
    let bound = append_job_event(
        config,
        &assignment.run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: serde_json::to_string(binding).ok(),
            detail: Some("Placement bound to the cloud-gcp Agent Computer microVM lane.".to_string()),
            digest: None,
            kind: "placement.bound".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "placement".to_string(),
            summary: format!(
                "Placement bound run to cloud-gcp Agent Computer microVM on runner {}.",
                binding.runner_id
            ),
            type_: "placement.bound".to_string(),
        },
    )?;
    response_events.push(control_event_from_job_event(&bound));

    let provisioning = append_job_event(
        config,
        &assignment.run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "workContextRef": work_context_ref,
                "lane": "cloud-gcp",
                "externalRunId": external_run_id,
            }))?),
            detail: Some("Agent Computer microVM provisioning requested.".to_string()),
            digest: None,
            kind: "cloud.gce.provisioning".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Agent Computer microVM provisioning on the GCE host.".to_string(),
            type_: "cloud.gce.provisioning".to_string(),
        },
    )?;
    response_events.push(control_event_from_job_event(&provisioning));

    let worker_config = config.clone();
    let worker_assignment = assignment.clone();
    let worker_runner_id = binding.runner_id.clone();
    let worker_wc_ref = work_context_ref.clone();
    thread::spawn(move || {
        if let Err(error) = run_org_cloud_microvm_worker(
            worker_config,
            worker_assignment,
            worker_runner_id,
            worker_wc_ref,
            work_context_b64,
        ) {
            eprintln!("{}", redact_for_log(&error));
        }
    });

    Ok(PlacementResponse {
        binding: binding.clone(),
        external_run_id: external_run_id.clone(),
        run: ControlRun {
            external_run_id,
            status: "provisioning".to_string(),
        },
        status: "provisioning".to_string(),
        events: response_events,
    })
}

/// The in-guest command that decodes the opaque work-context blob and runs the
/// baked turn-runner against it. The blob is validated base64 (single-quote-safe)
/// before this is built. Mirrors the proven proof path: decode -> /tmp/wc.json ->
/// `/opt/agent/turn-runner`, writing artifacts under [`cloud_vm::VM_ARTIFACT_DIR`].
/// True iff `value` is a non-empty standard base64 string (alphabet only). Used
/// to gate the opaque work-context blob before it is single-quoted into the guest
/// shell command, so a malformed/hostile blob can never inject.
fn is_valid_work_context_b64(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

fn microvm_turn_runner_command(work_context_b64: &str) -> Vec<String> {
    let script = format!(
        "mkdir -p {artifact} && printf '%s' '{b64}' | base64 -d > /tmp/wc.json && \
         OA_ARTIFACT_DIR={artifact} OA_CACHE_ROOT=/root/turns \
         /opt/agent/turn-runner /tmp/wc.json",
        artifact = cloud_vm::VM_ARTIFACT_DIR,
        b64 = work_context_b64,
    );
    vec!["bash".to_string(), "-lc".to_string(), script]
}

/// Public-safe subset of the in-guest `result.json` the turn-runner copies out.
/// Only non-secret refs/counts are read; the agent bearer is never serialized by
/// the turn-runner, so it cannot appear here.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MicrovmTurnResult {
    model: Option<String>,
    model_token_receipt: Option<MicrovmModelReceipt>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MicrovmModelReceipt {
    ok: bool,
    #[serde(default)]
    token_usage_event_ref: Option<String>,
    #[serde(default)]
    inserted_token_usage: Option<bool>,
    #[serde(default)]
    tokens_served_delta: Option<u64>,
}

/// Worker thread: boot the microVM, run the turn, then emit lifecycle receipts
/// and a terminal status. Teardown (scratch wipe + microVM destroy) is guaranteed
/// inside `run_cloud_vm_session`; we surface its cleanup receipt as the reclaim
/// evidence the isolation posture requires.
fn run_org_cloud_microvm_worker(
    config: Config,
    assignment: PlacementAssignment,
    runner_id: String,
    work_context_ref: String,
    work_context_b64: String,
) -> Result<(), String> {
    let run_id = assignment.run_id.clone();
    let run_short = short_digest_hex(&run_id);
    let (provisioner, kind) = cloud_vm::provisioner_for(config.cloud_vm_provisioner_kind);
    if kind != cloud_vm::ProvisionerKind::Live {
        append_microvm_failed(&config, &run_id, "live Firecracker provisioner unavailable at boot");
        let _ = update_job_status(&config, &run_id, "failed");
        return Err("live Firecracker provisioner unavailable at boot".to_string());
    }

    let vm_request = cloud_vm::CloudVmRequest {
        run_id: run_id.clone(),
        os: cloud_vm::CloudVmOs::Linux,
        target_name: "openagents-agent-computer".to_string(),
        owner_ref: assignment.owner_ref.clone(),
    };
    let session_command = microvm_turn_runner_command(&work_context_b64);
    let host_artifact_dir = config
        .state_root
        .join("cloud-vm-artifacts")
        .join(&run_short);

    let started_ms = now_ms()?;
    let outcome = cloud_vm::run_cloud_vm_session(
        provisioner.as_ref(),
        &vm_request,
        &session_command,
        &host_artifact_dir,
        started_ms,
    );
    let ended_ms = now_ms()?;
    let vm_seconds = ((ended_ms.saturating_sub(started_ms)) / 1000) as u64;

    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            append_microvm_failed(&config, &run_id, &format!("microVM turn failed: {}", error.message()));
            let _ = update_job_status(&config, &run_id, "failed");
            return Err(format!("microVM turn failed: {}", error.message()));
        }
    };

    // Read the public-safe result the turn-runner copied out (no secrets in it).
    let result: MicrovmTurnResult = fs::read_to_string(host_artifact_dir.join("result.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    // provisioned (active) — echoes the work-context ref for the isolation check.
    let provision_receipt_ref = format!("receipt.cloud.gce.provision.{run_short}");
    let provisioned = append_job_event(
        &config,
        &run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "workContextRef": work_context_ref,
                "vmId": outcome.vm_id,
                "provisionerKind": outcome.provisioner_kind,
            }))?),
            detail: Some("Agent Computer microVM booted and ran the turn.".to_string()),
            digest: Some(outcome.provision_receipt.receipt_digest.clone()),
            kind: "cloud.gce.provisioned".to_string(),
            receipt_refs: vec![provision_receipt_ref.clone()],
            redacted: false,
            source: "control".to_string(),
            summary: "Agent Computer microVM provisioned (healthy).".to_string(),
            type_: "cloud.gce.provisioned".to_string(),
        },
    )?;
    let _ = provisioned;

    // resource_usage_receipt: measured VM-seconds + the exact model-token receipt
    // ref minted from inside the guest (public-safe event id, not a token).
    let resource_receipt_ref = format!("receipt.cloud.gce.resource_usage.{run_short}");
    let model_receipt_json = match &result.model_token_receipt {
        Some(r) => serde_json::json!({
            "ok": r.ok,
            "tokenUsageEventRef": r.token_usage_event_ref,
            "insertedTokenUsage": r.inserted_token_usage,
            "tokensServedDelta": r.tokens_served_delta,
        }),
        None => serde_json::Value::Null,
    };
    let usage = append_job_event(
        &config,
        &run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "schema": "openagents.resource_usage_receipt.v1",
                "workContextRef": work_context_ref,
                "providerLane": "gcp",
                "computeUsage": {
                    "vmSeconds": vm_seconds,
                    "meteringSource": "node_measured",
                    "costInputBasis": "cost_plus_10pct_gcp_catalog",
                },
                "model": result.model,
                "modelTokenReceipt": model_receipt_json,
            }))?),
            detail: Some("Agent Computer resource + model usage receipt.".to_string()),
            digest: None,
            kind: "cloud.gce.resource_usage_receipt".to_string(),
            receipt_refs: vec![resource_receipt_ref, provision_receipt_ref],
            redacted: false,
            source: "control".to_string(),
            summary: format!("Agent Computer microVM used {vm_seconds} VM-seconds."),
            type_: "cloud.gce.resource_usage_receipt".to_string(),
        },
    )?;
    let _ = usage;

    // cleanup / reclaim: scratch-wipe + microVM-destroy receipt refs (the reclaim
    // evidence the isolation posture requires). Guaranteed teardown ran already.
    let scratch_wipe_ref = format!("receipt.cloud.gce.scratch_wipe.{run_short}");
    let microvm_destroy_ref = format!("receipt.cloud.gce.microvm_destroy.{run_short}");
    let cleanup = append_job_event(
        &config,
        &run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "workContextRef": work_context_ref,
                "tornDown": outcome.cleanup_receipt.torn_down,
                "artifactsExtracted": outcome.cleanup_receipt.artifacts_extracted,
                "scratchWipeReceiptRef": scratch_wipe_ref,
                "microvmDestroyReceiptRef": microvm_destroy_ref,
            }))?),
            detail: Some("Agent Computer microVM reclaimed: scratch wiped, microVM destroyed.".to_string()),
            digest: Some(outcome.cleanup_receipt.receipt_digest.clone()),
            kind: "cloud.gce.cleanup".to_string(),
            receipt_refs: vec![scratch_wipe_ref, microvm_destroy_ref],
            redacted: false,
            source: "control".to_string(),
            summary: "Agent Computer microVM reclaimed (scratch wiped, microVM destroyed).".to_string(),
            type_: "cloud.gce.cleanup".to_string(),
        },
    )?;
    let _ = cleanup;

    let exit_code = outcome.exec.code;
    let terminal_status = if exit_code == 0 { "completed" } else { "failed" };
    append_job_event(
        &config,
        &run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "workContextRef": work_context_ref,
                "runnerId": runner_id,
                "exitCode": exit_code,
            }))?),
            detail: Some("Agent Computer microVM turn finished.".to_string()),
            digest: None,
            kind: "turn.completed".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: format!("Agent Computer microVM turn {terminal_status} (exit {exit_code})."),
            type_: "turn.completed".to_string(),
        },
    )?;
    update_job_status(&config, &run_id, terminal_status)?;
    Ok(())
}

fn append_microvm_failed(config: &Config, run_id: &str, message: &str) {
    let _ = append_job_event(
        config,
        run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: None,
            detail: Some(redact_for_log(message)),
            digest: None,
            kind: "cloud.gce.degraded".to_string(),
            receipt_refs: Vec::new(),
            redacted: true,
            source: "control".to_string(),
            summary: "Agent Computer microVM turn degraded/failed.".to_string(),
            type_: "cloud.gce.degraded".to_string(),
        },
    );
}

fn start_codex_run_async(
    config: &Config,
    request: ControlRequest,
) -> Result<ControlResponse, String> {
    start_codex_run_async_with_initial_events(config, request, Vec::new())
}

fn start_codex_run_async_with_initial_events(
    config: &Config,
    request: ControlRequest,
    initial_events: Vec<JobEventInput>,
) -> Result<ControlResponse, String> {
    if let Some(expected) = &config.provider_account_ref {
        if request.provider_account_ref != *expected {
            return Err("provider account ref is not allowed on this runner".to_string());
        }
    }

    if let Ok(existing) = load_job_record(config, &request.run_id) {
        return Ok(response_from_job(
            &existing,
            load_job_events(config, &request.run_id, 0)?,
        ));
    }

    let now = now_ms()?;
    let external_run_id = external_run_id(&request.runner_id, &request.run_id);
    let job = JobRecord {
        cancel_requested: false,
        created_at_ms: now,
        external_run_id: external_run_id.clone(),
        last_sequence: 0,
        request: request.clone(),
        run_id: request.run_id.clone(),
        runner_id: request.runner_id.clone(),
        status: "queued".to_string(),
        updated_at_ms: now,
    };
    fs::create_dir_all(job_dir(config, &request.run_id))
        .map_err(|error| format!("failed to create Codex job registry: {error}"))?;
    save_job_record(config, &job)?;
    let mut response_events = Vec::new();
    let queued = append_job_event(
        config,
        &request.run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "externalRunId": external_run_id.clone(),
                "runnerId": request.runner_id.clone(),
            }))?),
            detail: Some("Run accepted by oa-codex-control and queued locally.".to_string()),
            digest: None,
            kind: "queued".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Codex run queued on SHC control daemon.".to_string(),
            type_: "cloud.run.queued".to_string(),
        },
    )?;
    response_events.push(control_event_from_job_event(&queued));
    for input in initial_events {
        let event = append_job_event(config, &request.run_id, input)?;
        response_events.push(control_event_from_job_event(&event));
    }

    let worker_config = config.clone();
    let worker_request = request.clone();
    thread::spawn(move || {
        if let Err(error) = run_codex_worker(worker_config, worker_request) {
            eprintln!("{}", redact_for_log(&error));
        }
    });

    Ok(ControlResponse {
        events: response_events,
        external_run_id: external_run_id.clone(),
        run: ControlRun {
            external_run_id,
            status: "queued".to_string(),
        },
        status: "queued".to_string(),
    })
}

/// cloud#97: enqueue a coding job for the internal tick worker to dispatch with
/// no external driver. Unlike `start_codex_run_async`, this persists the job in
/// `queued` status with a durable `queue.pending` marker and does NOT spawn a
/// worker. The tick worker drains pending jobs on the configured lane with
/// bounded concurrency; the marker survives daemon restarts so draining resumes.
fn enqueue_codex_run(config: &Config, request: ControlRequest) -> Result<ControlResponse, String> {
    if let Some(expected) = &config.provider_account_ref {
        if request.provider_account_ref != *expected {
            return Err("provider account ref is not allowed on this runner".to_string());
        }
    }

    if let Ok(existing) = load_job_record(config, &request.run_id) {
        return Ok(response_from_job(
            &existing,
            load_job_events(config, &request.run_id, 0)?,
        ));
    }

    let now = now_ms()?;
    let external_run_id = external_run_id(&request.runner_id, &request.run_id);
    let job = JobRecord {
        cancel_requested: false,
        created_at_ms: now,
        external_run_id: external_run_id.clone(),
        last_sequence: 0,
        request: request.clone(),
        run_id: request.run_id.clone(),
        runner_id: request.runner_id.clone(),
        status: "queued".to_string(),
        updated_at_ms: now,
    };
    fs::create_dir_all(job_dir(config, &request.run_id))
        .map_err(|error| format!("failed to create Codex job registry: {error}"))?;
    save_job_record(config, &job)?;
    // Durable pending marker the tick worker scans. Written last so a partially
    // created job is never dispatched.
    fs::write(queue_pending_path(config, &request.run_id), "pending\n")
        .map_err(|error| format!("failed to write queue pending marker: {error}"))?;

    let queued = append_job_event(
        config,
        &request.run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "externalRunId": external_run_id.clone(),
                "runnerId": request.runner_id.clone(),
                "queueLane": queue_config_lane_label(&config.queue.lane),
            }))?),
            detail: Some("Run enqueued for unattended draining by the control daemon.".to_string()),
            digest: None,
            kind: "queued".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Codex run enqueued on control daemon queue.".to_string(),
            type_: "cloud.run.enqueued".to_string(),
        },
    )?;

    Ok(ControlResponse {
        events: vec![control_event_from_job_event(&queued)],
        external_run_id: external_run_id.clone(),
        run: ControlRun {
            external_run_id,
            status: "queued".to_string(),
        },
        status: "queued".to_string(),
    })
}

fn queue_pending_path(config: &Config, run_id: &str) -> PathBuf {
    job_dir(config, run_id).join("queue.pending")
}

fn queue_config_lane_label(lane: &ComputeLane) -> &'static str {
    lane.as_str()
}

/// Scan the job registry for durable `queue.pending` markers, oldest first.
fn pending_queue_run_ids(config: &Config) -> Vec<String> {
    let jobs_root = config.state_root.join("jobs");
    let Ok(entries) = fs::read_dir(&jobs_root) else {
        return Vec::new();
    };
    let mut pending: Vec<(u128, String)> = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        let marker = dir.join("queue.pending");
        if !marker.exists() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(dir.join("job.json")) else {
            continue;
        };
        let Ok(job) = serde_json::from_str::<JobRecord>(&raw) else {
            continue;
        };
        pending.push((job.created_at_ms, job.run_id));
    }
    pending.sort_by_key(|(created_at, _)| *created_at);
    pending.into_iter().map(|(_, run_id)| run_id).collect()
}

/// Internal tick worker loop. Drains pending queued jobs on the configured lane
/// with bounded concurrency. Requires no external driver.
fn run_queue_tick_loop(config: Config) {
    let tick = Duration::from_millis(config.queue.tick_ms);
    loop {
        if let Err(error) = drain_queue_once(&config) {
            eprintln!("{}", redact_for_log(&error));
        }
        thread::sleep(tick);
    }
}

/// One drain pass: dispatch pending jobs up to the remaining concurrency budget.
/// Returns the number of jobs dispatched this pass (for tests).
fn drain_queue_once(config: &Config) -> Result<usize, String> {
    let mut dispatched = 0usize;
    for run_id in pending_queue_run_ids(config) {
        let in_flight = QUEUE_IN_FLIGHT.load(Ordering::SeqCst);
        if in_flight >= config.queue.max_concurrency {
            break;
        }
        if dispatch_queued_job(config, &run_id)? {
            dispatched += 1;
        }
    }
    Ok(dispatched)
}

/// Dispatch a single pending job: claim it (durable marker removal + lane
/// default + dequeue event), increment the in-flight counter, and spawn the
/// worker. Returns true if dispatched.
fn dispatch_queued_job(config: &Config, run_id: &str) -> Result<bool, String> {
    let Some(request) = claim_queued_job(config, run_id)? else {
        return Ok(false);
    };

    QUEUE_IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
    let worker_config = config.clone();
    thread::spawn(move || {
        let result = run_codex_worker(worker_config, request);
        QUEUE_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        if let Err(error) = result {
            eprintln!("{}", redact_for_log(&error));
        }
    });

    Ok(true)
}

/// Claim a pending queued job without spawning the worker: validate it is still
/// queued/uncanceled, apply the queue's default lane when the caller did not pin
/// one, remove the durable marker (so a restart cannot double-dispatch), and
/// emit a `cloud.run.dequeued` event. Returns the request to run, or `None` if
/// the job is not claimable.
fn claim_queued_job(config: &Config, run_id: &str) -> Result<Option<ControlRequest>, String> {
    let marker = queue_pending_path(config, run_id);
    if !marker.exists() {
        return Ok(None);
    }
    let mut job = match load_job_record(config, run_id) {
        Ok(job) => job,
        Err(_) => {
            // Orphaned marker without a job record; drop it.
            let _ = fs::remove_file(&marker);
            return Ok(None);
        }
    };
    if job.cancel_requested || job.status != "queued" {
        let _ = fs::remove_file(&marker);
        return Ok(None);
    }

    // Apply the queue's default lane when the caller did not pin one, so the
    // worker drives the configured lane (default cloud-gcp) unattended.
    if job.request.lane.is_none() {
        job.request.lane = Some(config.queue.lane);
        job.updated_at_ms = now_ms()?;
        save_job_record(config, &job)?;
    }

    // Claim: remove the durable marker before spawning so a restart does not
    // double-dispatch this job.
    fs::remove_file(&marker).map_err(|error| format!("failed to claim queued job: {error}"))?;

    append_job_event(
        config,
        run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "lane": job.request.lane.as_ref().map(|lane| lane.as_str()),
            }))?),
            detail: Some("Queued run dispatched by the control daemon tick worker.".to_string()),
            digest: None,
            kind: "queued".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "queue".to_string(),
            summary: "Queued Codex run dispatched unattended.".to_string(),
            type_: "cloud.run.dequeued".to_string(),
        },
    )?;

    Ok(Some(job.request))
}

fn start_training_run_async(
    config: &Config,
    assignment: TrainingRunAssignment,
) -> Result<ControlResponse, String> {
    let now = now_ms()?;
    assignment.validate_contract(now)?;
    if assignment.variants.len() != 1 {
        return Err("training assignment MVP supports exactly one variant".to_string());
    }
    if let Ok(existing) = load_job_record(config, &assignment.task_run_id) {
        return Ok(response_from_job(
            &existing,
            load_job_events(config, &assignment.task_run_id, 0)?,
        ));
    }
    let request = control_request_from_training_assignment(&assignment);
    let initial_events = training_assignment_initial_events(&assignment)?;
    let response =
        start_codex_run_async_with_initial_events(config, request.clone(), initial_events)?;
    fs::create_dir_all(job_dir(config, &request.run_id))
        .map_err(|error| format!("failed to create training assignment job dir: {error}"))?;
    write_json_file(
        &job_dir(config, &request.run_id).join("training-assignment.json"),
        &assignment,
    )?;
    send_pending_callbacks(config, &request.run_id)?;
    Ok(response)
}

fn start_artanis_bootstrap_async(
    config: &Config,
    assignment: ArtanisBootstrapAssignment,
) -> Result<ControlResponse, String> {
    let now = now_ms()?;
    assignment.validate_contract(now)?;
    if let Ok(existing) = load_job_record(config, &assignment.bootstrap_run_id) {
        return Ok(response_from_job(
            &existing,
            load_job_events(config, &assignment.bootstrap_run_id, 0)?,
        ));
    }
    let request = control_request_from_artanis_bootstrap_assignment(&assignment);
    let initial_events = artanis_bootstrap_initial_events(&assignment)?;
    let response =
        start_codex_run_async_with_initial_events(config, request.clone(), initial_events)?;
    fs::create_dir_all(job_dir(config, &request.run_id))
        .map_err(|error| format!("failed to create Artanis bootstrap job dir: {error}"))?;
    write_json_file(
        &job_dir(config, &request.run_id).join("artanis-bootstrap-assignment.json"),
        &assignment,
    )?;
    send_pending_callbacks(config, &request.run_id)?;
    Ok(response)
}

fn control_request_from_training_assignment(assignment: &TrainingRunAssignment) -> ControlRequest {
    ControlRequest {
        agent_runtime: None,
        auth_grant_ref: assignment.auth_grant_ref.clone(),
        github_write_connection_ref: None,
        github_write_grant_ref: None,
        github_work_order: None,
        goal: training_assignment_prompt(assignment),
        lane: None,
        owner_ref: None,
        provider_account_ref: assignment.provider_account_ref.clone(),
        repository: assignment.repository_ref.clone(),
        repository_clone_url: None,
        repository_ref: None,
        required_artifacts: Some(assignment.artifacts.required_artifacts.clone()),
        retention_mode: Some(
            retention_mode_label(&assignment.artifacts.retention_mode).to_string(),
        ),
        runner_id: assignment.target_node_id.clone(),
        run_id: assignment.task_run_id.clone(),
        sandbox_mode: Some("danger_full_access".to_string()),
        timeout_ms: Some(assignment.budget.timeout_ms.min(u64::MAX as u128) as u64),
    }
}

fn control_request_from_artanis_bootstrap_assignment(
    assignment: &ArtanisBootstrapAssignment,
) -> ControlRequest {
    ControlRequest {
        agent_runtime: None,
        auth_grant_ref: assignment.auth_grant_ref.clone(),
        github_write_connection_ref: None,
        github_write_grant_ref: None,
        github_work_order: None,
        goal: artanis_bootstrap_prompt(assignment),
        lane: None,
        owner_ref: None,
        provider_account_ref: assignment.provider_account_ref.clone(),
        repository: Some(assignment.repository_refs.join("+")),
        repository_clone_url: None,
        repository_ref: None,
        required_artifacts: Some(assignment.required_artifacts.clone()),
        retention_mode: Some(retention_mode_label(&assignment.retention_mode).to_string()),
        runner_id: assignment.target_node_id.clone(),
        run_id: assignment.bootstrap_run_id.clone(),
        sandbox_mode: Some("danger_full_access".to_string()),
        timeout_ms: Some(assignment.budget.timeout_ms.min(u64::MAX as u128) as u64),
    }
}

fn training_assignment_initial_events(
    assignment: &TrainingRunAssignment,
) -> Result<Vec<JobEventInput>, String> {
    let variant = assignment
        .variants
        .first()
        .ok_or_else(|| "training assignment requires one variant".to_string())?;
    let mut events = vec![
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "trainingRunId": &assignment.training_run_id,
                "benchmarkRunId": &assignment.benchmark_run_id,
                "taskRunId": &assignment.task_run_id,
                "dataset": &assignment.dataset.dataset_slug,
                "datasetVersion": &assignment.dataset.dataset_version,
                "taskRef": &assignment.dataset.task_ref,
                "retentionMode": retention_mode_label(&assignment.artifacts.retention_mode),
            }))?),
            detail: Some(format!(
                "{} {} queued for {}",
                assignment.dataset.dataset_slug,
                assignment.dataset.dataset_version,
                assignment.dataset.task_ref
            )),
            digest: None,
            kind: "training_assignment".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Training assignment validated for SHC Codex runner.".to_string(),
            type_: "training.assignment.validated".to_string(),
        },
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "adapterId": &assignment.codex_adapter.adapter_id,
                "packageName": &assignment.codex_adapter.package_name,
                "packageVersion": &assignment.codex_adapter.package_version,
                "packageDigest": &assignment.codex_adapter.package_digest,
                "agent": &variant.agent,
                "model": &variant.model,
            }))?),
            detail: Some(
                "Codex package adapter refs were validated without raw credential material."
                    .to_string(),
            ),
            digest: assignment.codex_adapter.package_digest.clone(),
            kind: "package".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Codex package adapter validated for benchmark run.".to_string(),
            type_: "benchmark.package.validated".to_string(),
        },
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "requiredArtifacts": &assignment.artifacts.required_artifacts,
                "artifactSinkRef": &assignment.artifacts.artifact_sink_ref,
                "retentionMode": retention_mode_label(&assignment.artifacts.retention_mode),
            }))?),
            detail: Some("Artifact retention policy attached to training run.".to_string()),
            digest: None,
            kind: "artifact_policy".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Training artifact policy attached.".to_string(),
            type_: "training.artifact_policy.attached".to_string(),
        },
    ];
    if let Some(signature_context) = &assignment.signature_context {
        events.push(JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "blueprintSignatureIds": &signature_context.blueprint_signature_ids,
                "packageDigest": &signature_context.package_digest,
                "selectorTraceRequired": signature_context.selector_trace_required,
            }))?),
            detail: Some("Blueprint/Probe signature context loaded for benchmark run.".to_string()),
            digest: signature_context.package_digest.clone(),
            kind: "signature_context".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Blueprint/Probe signature context loaded.".to_string(),
            type_: "signature.context.loaded".to_string(),
        });
    }
    Ok(events)
}

fn artanis_bootstrap_initial_events(
    assignment: &ArtanisBootstrapAssignment,
) -> Result<Vec<JobEventInput>, String> {
    let mut events = vec![
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "bootstrapRunId": &assignment.bootstrap_run_id,
                "objectiveId": &assignment.objective_id,
                "pylonLaunchId": &assignment.pylon_launch_id,
                "repositoryRefs": &assignment.repository_refs,
                "sourceRefs": &assignment.source_refs,
                "retentionMode": retention_mode_label(&assignment.retention_mode),
            }))?),
            detail: Some("Artanis bootstrap assignment accepted for SHC Codex execution.".to_string()),
            digest: None,
            kind: "artanis_bootstrap".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Artanis bootstrap assignment validated for SHC Codex runner.".to_string(),
            type_: "artanis.bootstrap.validated".to_string(),
        },
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "pylonCapabilityLabels": &assignment.pylon_capability_labels,
                "blueprintSignatureIds": &assignment.blueprint_signature_ids,
            }))?),
            detail: Some("Pylon capability labels and Blueprint signatures loaded from Artanis policy source.".to_string()),
            digest: None,
            kind: "capability_context".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Artanis Pylon and signature context loaded.".to_string(),
            type_: "artanis.capability_context.loaded".to_string(),
        },
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "requiredArtifacts": &assignment.required_artifacts,
                "artifactSinkRef": &assignment.artifact_sink_ref,
                "walletAuthority": assignment.wallet_authority,
            }))?),
            detail: Some("No-wallet artifact policy attached for Artanis bootstrap workroom.".to_string()),
            digest: None,
            kind: "artifact_policy".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "control".to_string(),
            summary: "Artanis bootstrap artifact policy attached.".to_string(),
            type_: "artanis.artifact_policy.attached".to_string(),
        },
    ];
    if let Some(settlement_intent) = &assignment.settlement_intent {
        events.push(JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "artanisRunId": &settlement_intent.artanis_run_id,
                "artanisAssignmentId": &settlement_intent.artanis_assignment_id,
                "settlementIntentId": &settlement_intent.settlement_intent_id,
                "publicReceiptId": &settlement_intent.public_receipt_id,
                "walletAuthority": assignment.wallet_authority,
                "pylonNip90Tags": {
                    "oa:artanis_run_id": &settlement_intent.artanis_run_id,
                    "oa:artanis_assignment_id": &settlement_intent.artanis_assignment_id,
                    "oa:settlement_intent_id": &settlement_intent.settlement_intent_id,
                },
            }))?),
            detail: Some(
                "Public-safe Artanis settlement intent attached without wallet authority."
                    .to_string(),
            ),
            digest: None,
            kind: "settlement_intent".to_string(),
            receipt_refs: settlement_intent
                .public_receipt_id
                .clone()
                .into_iter()
                .collect(),
            redacted: false,
            source: "control".to_string(),
            summary: "Artanis settlement intent attached for Pylon traceability.".to_string(),
            type_: "artanis.settlement_intent.attached".to_string(),
        });
    }
    Ok(events)
}

fn training_assignment_prompt(assignment: &TrainingRunAssignment) -> String {
    let variant = assignment
        .variants
        .first()
        .expect("training assignment validation requires one variant");
    let required_artifacts = assignment.artifacts.required_artifacts.join(", ");
    let signature_ids = assignment
        .signature_context
        .as_ref()
        .map(|context| context.blueprint_signature_ids.join(", "))
        .unwrap_or_else(|| "none".to_string());
    let signature_playbooks = training_signature_playbook_lines(assignment);
    format!(
        "Run a retained OpenAgents Benchmark Cloud task on this SHC workroom.\n\n\
Contract:\n\
- trainingRunId: {training_run_id}\n\
- benchmarkRunId: {benchmark_run_id}\n\
- taskRunId: {task_run_id}\n\
- dataset: {dataset_slug}@{dataset_version}\n\
- task: {task_ref}\n\
- variant: {variant_id}\n\
- agent: {agent}\n\
- model: {model}\n\
- codex package adapter: {adapter_id} {package_name}@{package_version}\n\
- retentionMode: {retention_mode}\n\
- selected Blueprint/Probe signatures: {signature_ids}\n\n\
Selected signature playbooks:\n\
{signature_playbooks}\n\n\
Use the fixed Terminal-Bench/Harbor shape if Harbor is available on the host:\n\
uvx --from harbor harbor run --dataset {dataset_slug}@{dataset_version} --task {task_ref} --agent codex --model {model} --agent-kwarg version={package_version} --agent-env CODEX_AUTH_JSON_PATH=<session-codex-auth-file> --n-concurrent 1 --n-attempts {max_attempts} --yes --debug\n\n\
Do not use API-key fallback. Do not print, copy, or archive Codex account material. Do not request wallet, cloud, or provider credentials. If the account is stale, stop and report that in the artifacts.\n\n\
Before finishing, create the required artifact files in the current directory: {required_artifacts}. The artifacts must include a normalized result, artifact manifest, proof bundle, and a human-readable result.md summary. Include package-rendered/validated/loaded evidence when available, signature selector evidence when available, verifier status, artifact paths, Codex turn.completed.usage or app-server ThreadTokenUsageUpdated evidence when available, and explicit usage_unavailable evidence only if Codex does not expose token usage.",
        training_run_id = assignment.training_run_id,
        benchmark_run_id = assignment.benchmark_run_id,
        task_run_id = assignment.task_run_id,
        dataset_slug = assignment.dataset.dataset_slug,
        dataset_version = assignment.dataset.dataset_version,
        task_ref = assignment.dataset.task_ref,
        variant_id = variant.variant_id,
        agent = variant.agent,
        model = variant.model,
        adapter_id = assignment.codex_adapter.adapter_id,
        package_name = assignment.codex_adapter.package_name,
        package_version = assignment.codex_adapter.package_version,
        retention_mode = retention_mode_label(&assignment.artifacts.retention_mode),
        signature_ids = signature_ids,
        signature_playbooks = signature_playbooks,
        max_attempts = assignment.budget.max_attempts,
        required_artifacts = required_artifacts,
    )
}

fn artanis_bootstrap_prompt(assignment: &ArtanisBootstrapAssignment) -> String {
    let repository_refs = assignment.repository_refs.join(", ");
    let source_refs = assignment.source_refs.join(", ");
    let pylon_capabilities = assignment.pylon_capability_labels.join(", ");
    let signature_ids = assignment.blueprint_signature_ids.join(", ");
    let required_artifacts = assignment.required_artifacts.join(", ");
    let settlement_intent = artanis_settlement_intent_prompt_block(assignment);
    format!(
        "Run the Artanis to Pylon launch bootstrap on this private SHC Codex workroom.\n\n\
Contract:\n\
- bootstrapRunId: {bootstrap_run_id}\n\
- workroomId: {workroom_id}\n\
- objectiveId: {objective_id}\n\
- objectiveSummary: {objective_summary}\n\
- pylonLaunchId: {pylon_launch_id}\n\
- repositories: {repository_refs}\n\
- sourceRefs: {source_refs}\n\
- Pylon capability labels: {pylon_capabilities}\n\
- Blueprint/Probe signatures: {signature_ids}\n\
- retentionMode: {retention_mode}\n\
- walletAuthority: false\n\n\
- settlementIntent:\n{settlement_intent}\n\n\
Imported Artanis source policy:\n\
- Artanis is the public training-program overseer, but this workroom is private by default.\n\
- Public output must be redacted projection only; do not expose raw prompts, raw logs, local paths, private repo contents, provider auth, wallet material, or internal fleet details.\n\
- The useful source material is the Artanis identity/objective, capability labels, GitHub repository allowlist, Program policy types, runner events, health gates, recovery commands, launch checks, promotion gates, and public projection rules.\n\
- Required coordination targets are Cloud SHC Codex workrooms, Vortex public/private projection and mission UI, OpenAgents/Pylon launch state, Psionic training/eval lanes, Probe/Blueprint signatures, benchmark evidence gates, and retained receipts.\n\n\
Work to perform:\n\
1. Inspect the provided source refs and repository context. Pull only the relevant Artanis policy/code concepts into a current implementation plan; do not revive deprecated repo behavior as an authority.\n\
2. Create a concrete next Pylon launch bootstrap plan: capability registry, assignment templates, health gates, dispatch blockers, benchmark gates, receipt fields, and public-safe projection events.\n\
3. Create a continual-learning loop plan: mine failed Codex/Pylon traces, select Blueprint signatures, run retained benchmark/eval replay, update signature packs, and record improvement receipts.\n\
4. If the settlement intent above is present and you create or draft a Pylon NIP-90 job, include `artanis_run_id`, `artanis_assignment_id`, and `settlement_intent_id` in the structured request JSON exactly as shown. Pylon publishes those as `oa:artanis_run_id`, `oa:artanis_assignment_id`, and `oa:settlement_intent_id` tags. These are traceability ids only; they are not wallet authority.\n\
5. Identify the first code changes that Vortex and Cloud need next, including API calls, workroom events, artifacts, and issue-sized tasks.\n\
6. If safe and directly applicable in the checked-out repo, make small scaffolding edits only when they preserve the no-wallet, no-secret, public-projection boundary. Otherwise emit work-order drafts instead of changing code.\n\n\
Before finishing, create these required artifacts in the current directory: {required_artifacts}.\n\
Artifact requirements:\n\
- result.md: operator-readable summary and next action.\n\
- artanis-source-map.json: source refs mapped to imported concepts.\n\
- pylon-launch-plan.json: launch steps, capability labels, gates, and acceptance criteria.\n\
- continual-learning-plan.json: trace mining, signature selection, benchmark replay, and promotion receipt loop.\n\
- signature-mining-plan.json: candidate signatures and evidence sources.\n\
- work-order-drafts.json: issue-sized implementation tasks for Cloud, Vortex, OpenAgents/Pylon, Psionic, and Probe/Blueprint.\n\
- artifact-manifest.json and proof-bundle.json: retained artifact refs, redaction status, and no-wallet/no-secret assertions.\n\n\
Do not use API-key fallback. Do not print, copy, or archive Codex account material. Do not request wallet, cloud, or provider credentials. If the account is stale, stop and report that in result.md and proof-bundle.json.",
        bootstrap_run_id = assignment.bootstrap_run_id,
        workroom_id = assignment.workroom_id,
        objective_id = assignment.objective_id,
        objective_summary = assignment.objective_summary,
        pylon_launch_id = assignment.pylon_launch_id,
        repository_refs = repository_refs,
        source_refs = source_refs,
        pylon_capabilities = pylon_capabilities,
        signature_ids = signature_ids,
        retention_mode = retention_mode_label(&assignment.retention_mode),
        settlement_intent = settlement_intent,
        required_artifacts = required_artifacts,
    )
}

fn artanis_settlement_intent_prompt_block(assignment: &ArtanisBootstrapAssignment) -> String {
    match &assignment.settlement_intent {
        Some(intent) => format!(
            "  artanis_run_id: {artanis_run_id}\n  artanis_assignment_id: {artanis_assignment_id}\n  settlement_intent_id: {settlement_intent_id}\n  public_receipt_id: {public_receipt_id}",
            artanis_run_id = intent.artanis_run_id.as_str(),
            artanis_assignment_id = intent.artanis_assignment_id.as_str(),
            settlement_intent_id = intent.settlement_intent_id.as_str(),
            public_receipt_id = intent.public_receipt_id.as_deref().unwrap_or("none"),
        ),
        None => "  none".to_string(),
    }
}

fn training_signature_playbook_lines(assignment: &TrainingRunAssignment) -> String {
    let Some(context) = &assignment.signature_context else {
        return "- none selected".to_string();
    };
    let mut lines = Vec::new();
    for signature_id in &context.blueprint_signature_ids {
        match signature_id.as_str() {
            "probe.signature.db-wal-recovery" | "coding.sqlite_wal_recovery" => {
                lines.push(format!("- {signature_id}:"));
                lines.push("  - Copy the SQLite DB, WAL, and SHM files as a matched set before opening SQLite.".to_string());
                lines.push("  - Open only the copied DB so SQLite cannot delete or checkpoint unreadable original sidecars.".to_string());
                lines.push(
                    "  - Run PRAGMA integrity_check and preserve sqlite-integrity.txt.".to_string(),
                );
                lines.push("  - Checkpoint or recover on the copied set, then record row counts or recovered-data digests.".to_string());
            }
            _ => lines.push(format!(
                "- {signature_id}: load package-provided playbook and preserve selector evidence."
            )),
        }
    }
    if lines.is_empty() {
        "- none selected".to_string()
    } else {
        lines.join("\n")
    }
}

fn retention_mode_label(mode: &TrainingRetentionMode) -> &'static str {
    match mode {
        TrainingRetentionMode::DurableArtifacts => "durable_artifacts",
        TrainingRetentionMode::RedactedOnly => "redacted_only",
        TrainingRetentionMode::LocalOnly => "local_only",
    }
}

fn agent_runtime_for_request(request: &ControlRequest) -> AgentRuntime {
    request.agent_runtime.unwrap_or(AgentRuntime::OpencodeCodex)
}

fn run_codex_worker(config: Config, request: ControlRequest) -> Result<(), String> {
    let agent_runtime = agent_runtime_for_request(&request);
    let runtime_label = agent_runtime.label();

    // GCE per-session lane: provision an ephemeral VM (gce_capacity_class.v1)
    // before the run, attach it, and release + emit a resource_usage_receipt +
    // cleanup receipt when the run reaches a terminal state. The fake/dry-run
    // provisioner keeps execution local; the live provisioner is gated by ADC.
    let mut gce_lease = if request.lane == Some(ComputeLane::CloudGcp) {
        match acquire_gce_lease_for_run(&config, &request)? {
            Some(lease) => Some(lease),
            None => None,
        }
    } else {
        None
    };

    update_job_status(&config, &request.run_id, "running")?;
    append_job_event(
        &config,
        &request.run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "agentRuntime": agent_runtime.as_str(),
                "runnerId": request.runner_id.clone(),
            }))?),
            detail: Some(format!(
                "Background worker started the {runtime_label} workroom turn."
            )),
            digest: None,
            kind: "started".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            source: "runner".to_string(),
            summary: format!("{runtime_label} run started on SHC runner."),
            type_: "cloud.run.started".to_string(),
        },
    )?;
    send_pending_callbacks_best_effort(&config, &request.run_id);

    let mirror_stop = Arc::new(AtomicBool::new(false));
    let mirror_handle = {
        let mirror_config = config.clone();
        let mirror_run_id = request.run_id.clone();
        let mirror_stop = Arc::clone(&mirror_stop);
        thread::spawn(move || {
            while !mirror_stop.load(Ordering::Relaxed) {
                if let Err(error) = mirror_runner_events_once(&mirror_config, &mirror_run_id) {
                    eprintln!("{}", redact_for_log(&error));
                }
                thread::sleep(Duration::from_millis(250));
            }
            if let Err(error) = mirror_runner_events_once(&mirror_config, &mirror_run_id) {
                eprintln!("{}", redact_for_log(&error));
            }
        })
    };
    let result = start_codex_run(&config, request.clone());
    mirror_stop.store(true, Ordering::Relaxed);
    if mirror_handle.join().is_err() {
        eprintln!("runner event mirror thread panicked");
    }
    if is_cancel_requested(&config, &request.run_id) {
        update_job_status(&config, &request.run_id, "canceled")?;
        append_job_event(
            &config,
            &request.run_id,
            JobEventInput {
                artifact_refs: Vec::new(),
                data_json: None,
                detail: Some(format!(
                    "Cancellation was requested while {runtime_label} was running."
                )),
                digest: None,
                kind: "canceled".to_string(),
                receipt_refs: Vec::new(),
                redacted: false,
                source: "control".to_string(),
                summary: format!("{runtime_label} run marked canceled by control daemon."),
                type_: "cloud.run.canceled".to_string(),
            },
        )?;
        finish_gce_lease(
            &config,
            &request,
            gce_lease.take(),
            "canceled",
            ReleaseReason::Manual,
        )?;
        send_pending_callbacks_best_effort(&config, &request.run_id);
        return Ok(());
    }

    match result {
        Ok(response) => {
            for event in response.events {
                append_control_event_once(&config, &request.run_id, event)?;
            }
            update_job_status(&config, &request.run_id, &response.status)?;
            let terminal_status = response.status.clone();
            append_job_event(
                &config,
                &request.run_id,
                JobEventInput {
                    artifact_refs: Vec::new(),
                    data_json: Some(json_string(&serde_json::json!({
                        "status": terminal_status,
                        "agentRuntime": agent_runtime.as_str(),
                    }))?),
                    detail: Some(format!("{runtime_label} worker reached a terminal state.")),
                    digest: None,
                    kind: terminal_status.clone(),
                    receipt_refs: Vec::new(),
                    redacted: false,
                    source: "runner".to_string(),
                    summary: format!("{runtime_label} run finished with status {terminal_status}."),
                    type_: format!("cloud.run.{terminal_status}"),
                },
            )?;
            finish_gce_lease(
                &config,
                &request,
                gce_lease.take(),
                &terminal_status,
                ReleaseReason::Manual,
            )?;
            send_pending_callbacks_best_effort(&config, &request.run_id);
            Ok(())
        }
        Err(error) => {
            update_job_status(&config, &request.run_id, "failed")?;
            append_job_event(
                &config,
                &request.run_id,
                JobEventInput {
                    artifact_refs: Vec::new(),
                    data_json: None,
                    detail: Some(redact_for_log(&error)),
                    digest: None,
                    kind: "failed".to_string(),
                    receipt_refs: Vec::new(),
                    redacted: contains_secret_marker(&error),
                    source: "runner".to_string(),
                    summary: format!("{runtime_label} run failed in SHC worker."),
                    type_: "cloud.run.failed".to_string(),
                },
            )?;
            finish_gce_lease(
                &config,
                &request,
                gce_lease.take(),
                "failed",
                ReleaseReason::Manual,
            )?;
            send_pending_callbacks_best_effort(&config, &request.run_id);
            Ok(())
        }
    }
}

/// Acquire an ephemeral GCE per-session VM lease for a `cloud-gcp` run and emit
/// a `gce.provision` event. Returns `None` (and emits a degraded event) when the
/// lease cannot be acquired so the run still proceeds on the local control host
/// rather than failing outright; SHC fallback selection happens earlier in
/// placement.
fn acquire_gce_lease_for_run(
    config: &Config,
    request: &ControlRequest,
) -> Result<Option<GceLease>, String> {
    let now = now_ms()?;
    let workroom_ref = format!("workroom_{}", safe_path_component(&request.run_id));
    let capacity_request = CapacityRequest {
        run_id: request.run_id.clone(),
        owner_ref: request
            .owner_ref
            .clone()
            .unwrap_or_else(|| format!("owner://run/{}", short_digest(&request.run_id))),
        gcp_project_ref: config.gce_project_ref.clone(),
        provisioner_identity_ref: config.gce_provisioner_identity_ref.clone(),
        caps: ComputeQuotaCaps::default(),
    };
    let (provisioner, effective_kind) = provisioner_for(config.gce_provisioner_kind);
    match GceLease::acquire(provisioner, &capacity_request, &workroom_ref, now) {
        Ok(mut lease) => {
            lease.mark_in_use();
            append_job_event(
                config,
                &request.run_id,
                JobEventInput {
                    artifact_refs: Vec::new(),
                    data_json: Some(json_string(&serde_json::json!({
                        "leaseRef": lease.lease_ref(),
                        "instanceRef": lease.instance_ref(),
                        "capacityClassId": lease.projection.capacity_class_id,
                        "provisionerKind": effective_kind.as_str(),
                        "provisionReceiptRef": lease.provision_receipt.receipt_digest,
                        "state": "in_use",
                    }))?),
                    detail: Some(
                        "GCE ephemeral per-session VM provisioned and attached to the run."
                            .to_string(),
                    ),
                    digest: Some(lease.provision_receipt.receipt_digest.clone()),
                    kind: "started".to_string(),
                    receipt_refs: vec![lease.provision_receipt.receipt_digest.clone()],
                    redacted: false,
                    source: "gce".to_string(),
                    summary: format!(
                        "GCE capacity lease ready on the {} provisioner.",
                        effective_kind.as_str()
                    ),
                    type_: "cloud.gce.provisioned".to_string(),
                },
            )?;
            Ok(Some(lease))
        }
        Err(error) => {
            append_job_event(
                config,
                &request.run_id,
                JobEventInput {
                    artifact_refs: Vec::new(),
                    data_json: None,
                    detail: Some(redact_for_log(&error)),
                    digest: None,
                    kind: "log".to_string(),
                    receipt_refs: Vec::new(),
                    redacted: contains_secret_marker(&error),
                    source: "gce".to_string(),
                    summary: "GCE provisioning unavailable; run continues on local control host."
                        .to_string(),
                    type_: "cloud.gce.degraded".to_string(),
                },
            )?;
            Ok(None)
        }
    }
}

/// Emit a `resource_usage_receipt.v1` for the run, then idempotently release the
/// GCE lease and emit the cleanup receipt as a `cleanup` event.
fn finish_gce_lease(
    config: &Config,
    request: &ControlRequest,
    lease: Option<GceLease>,
    terminal_status: &str,
    reason: ReleaseReason,
) -> Result<(), String> {
    let Some(mut lease) = lease else {
        return Ok(());
    };
    let now = now_ms()?;

    // Honor quota caps: if the lease exceeded its TTL or idle deadline, record
    // the policy-driven release reason rather than the caller's request reason.
    let reason = if lease.ttl_expired(now) {
        ReleaseReason::TtlExpired
    } else if lease.idle_expired(now) {
        ReleaseReason::IdleTimeout
    } else {
        reason
    };

    // Metered VM-seconds from the REAL lease wall-time: release_at − acquire_at,
    // whole seconds, saturating at 0. The acquire timestamp is the provision
    // receipt's emitted_at_ms recorded at lease acquire; `now` is release time.
    let acquire_at_ms = lease.provision_receipt.emitted_at_ms;
    let vm_seconds = u64::try_from(now.saturating_sub(acquire_at_ms) / 1_000).unwrap_or(u64::MAX);

    // Resource-usage receipt: refs-and-limits only (INVARIANTS "Placement And
    // Quota Routing"). The infra `compute_usage` sub-record carries genuinely
    // measured VM-seconds and a catalog/list-price-derived cost-plus-10% billing
    // input (cloud#92); no raw customer cost or raw identity.
    let workroom_id = format!("workroom_{}", safe_path_component(&request.run_id));
    let receipt = gce_resource_usage_receipt(
        request,
        &workroom_id,
        lease.lease_ref(),
        lease.instance_ref(),
        terminal_status,
        vm_seconds,
        now,
    );
    if let Err(error) = receipt.validate_contract() {
        return Err(format!("gce resource usage receipt is invalid: {error}"));
    }
    append_job_event(
        config,
        &request.run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "receiptId": receipt.receipt_id,
                "runRef": receipt.run_ref,
                "providerLane": "gcp",
                "leaseRef": lease.lease_ref(),
                "instanceRef": lease.instance_ref(),
                "vmSeconds": vm_seconds,
                "costInputMicrousd": receipt
                    .compute_usage
                    .as_ref()
                    .and_then(|usage| usage.cost_input_microusd),
                "costInputBasis": receipt
                    .compute_usage
                    .as_ref()
                    .map(|usage| usage.cost_input_basis.as_str()),
            }))?),
            detail: Some("Resource usage receipt captured for the GCE session.".to_string()),
            digest: Some(receipt.receipt_digest.clone()),
            kind: "receipt".to_string(),
            receipt_refs: vec![receipt.receipt_digest.clone()],
            redacted: false,
            source: "gce".to_string(),
            summary: "openagents.resource_usage_receipt.v1 emitted for GCE session.".to_string(),
            type_: "cloud.gce.resource_usage_receipt".to_string(),
        },
    )?;

    let cleanup = lease.release(reason, now)?;
    append_job_event(
        config,
        &request.run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(&serde_json::json!({
                "leaseRef": cleanup.lease_ref,
                "instanceRef": cleanup.instance_ref,
                "releaseReason": cleanup.release_reason,
                "deletedVm": cleanup.deleted_vm,
                "removedFirewallRule": cleanup.removed_firewall_rule,
                "revokedSshMetadata": cleanup.revoked_ssh_metadata,
                "result": format!("{:?}", cleanup.result).to_ascii_lowercase(),
            }))?),
            detail: Some("GCE ephemeral VM released and cleaned up.".to_string()),
            digest: Some(cleanup.receipt_digest.clone()),
            kind: "cleanup".to_string(),
            receipt_refs: vec![cleanup.receipt_digest.clone()],
            redacted: false,
            source: "gce".to_string(),
            summary: "GCE capacity lease released; cleanup receipt minted.".to_string(),
            type_: "cloud.gce.cleanup".to_string(),
        },
    )?;
    Ok(())
}

/// Build a refs-only `openagents.resource_usage_receipt.v1` for a GCE session.
/// Token counts are unavailable from this control-plane path, so the single
/// model usage record is marked `Unavailable` with a declared reason.
///
/// `vm_seconds` is the genuinely measured lease wall-time (`release_at −
/// acquire_at`). It drives the infra `compute_usage` sub-record: a
/// `cost_input_microusd = floor(vm_seconds × cost-plus-10% rate)` whose rate is
/// the GCP published list-price catalog rate (`GCE_RAW_PER_VM_SEC_NANOUSD`), so
/// the basis is `cost_plus_10pct_gcp_catalog` — the VM-seconds are measured, only
/// the rate is catalog-derived pending a live GCP Billing export (cloud#92).
fn gce_resource_usage_receipt(
    request: &ControlRequest,
    workroom_id: &str,
    lease_ref: &str,
    instance_ref: &str,
    terminal_status: &str,
    vm_seconds: u64,
    now_ms: u128,
) -> ResourceUsageReceipt {
    let run_ref = format!("run-ref://gce/{}", short_digest(&request.run_id));
    let receipt_id = format!("receipt://gce/{}", short_digest(lease_ref));
    let compute_usage = ComputeUsage::gce_catalog_from_vm_seconds(
        GCE_EPHEMERAL_CAPACITY_CLASS_ID,
        vm_seconds,
        &LaneCostModel::default(),
    );
    let receipt_digest = format!(
        "sha256:{}",
        full_sha256(&format!(
            "resource-usage|{run_ref}|{instance_ref}|{terminal_status}|{vm_seconds}|{now_ms}"
        ))
    );
    ResourceUsageReceipt {
        schema_version: RESOURCE_USAGE_RECEIPT_VERSION.to_string(),
        receipt_id,
        run_ref,
        workroom_id: workroom_id.to_string(),
        node_ref: format!("node-ref://gce/{}", short_digest(instance_ref)),
        provider_lane: ProviderLane::Gcp,
        host: ResourceHostSnapshot {
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            cpu: "gce-ephemeral-standard".to_string(),
            logical_cpu_count: 4,
            physical_cpu_count: None,
            memory_total_bytes: None,
            memory_available_bytes: None,
            disk_total_bytes: None,
            disk_available_bytes: None,
            accelerator_inventory: Vec::new(),
            virtualization: VirtualizationFacts {
                kvm_present: true,
                firecracker_candidate: false,
                container_runtime: None,
                cgroup_mode: None,
            },
        },
        run: RunResourceUsage {
            sandbox: request
                .sandbox_mode
                .clone()
                .unwrap_or_else(|| "danger_full_access".to_string()),
            image_or_profile_digest: format!("sha256:{}", full_sha256("gce.ephemeral.standard.v1")),
            workspace_digest: format!("sha256:{}", full_sha256(&request.run_id)),
            wall_time_ms: None,
            exit_code: None,
            timed_out: terminal_status == "timeout",
            workspace_bytes: None,
            artifact_bytes: None,
            log_bytes: None,
        },
        model_usage: vec![ModelUsageRecord {
            provider: "openai".to_string(),
            backend: "codex".to_string(),
            model: "codex".to_string(),
            mode: "exec".to_string(),
            account_ref: Some(request.provider_account_ref.clone()),
            input_tokens: None,
            cached_input_tokens: None,
            output_tokens: None,
            reasoning_tokens: None,
            total_tokens: None,
            count_source: TokenCountSource::Unavailable,
            cost_microusd: None,
            billing_basis: "refs_and_limits_only".to_string(),
            unavailable_reason: Some(
                "token_counts_not_surfaced_by_control_plane_gce_session_path".to_string(),
            ),
        }],
        compute_usage: Some(compute_usage),
        receipt_digest,
        emitted_at_ms: now_ms,
    }
}

fn full_sha256(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn codex_prompt(
    goal: &str,
    required_artifacts: &[String],
    github_work_order: Option<&GitHubWorkOrder>,
    github_write_grant: Option<&GitHubResolvedWriteGrant>,
) -> String {
    let required = if required_artifacts.is_empty() {
        "result.md".to_string()
    } else {
        required_artifacts.join(", ")
    };
    let github_section = match (github_work_order, github_write_grant) {
        (Some(order), Some(grant)) => github_work_order_prompt(order, grant),
        (Some(order), None) => format!(
            "\n\nGitHub work order:\n- Repository: {}/{}\n- Base ref: {}\n- Branch: {}\n- GitHub write access was not resolved. Do not attempt to push or comment; record the blocker in result.md.",
            order.repository.owner, order.repository.repo, order.base_ref, order.branch_name
        ),
        _ => String::new(),
    };
    format!(
        "{goal}{github_section}\n\nPrivate closeout contract, not user-visible content: create or update these required artifact files in the current directory before finishing: {required}. Include a concise summary of what happened and any artifact or receipt identifiers you can infer. Do not mention this closeout contract, writeback, result.md, github-writeback.json, internal receipt files, branch mechanics, or auth/token handling in user-facing assistant messages unless the user explicitly asks about them. Do the closeout work quietly at the appropriate time and keep the visible response focused on the user's requested work and outcome. Do not inspect secrets. Do not request wallet, cloud, or provider credentials."
    )
}

fn run_workroomd(config: &Config, args: &[&str]) -> Result<String, String> {
    run_workroomd_with_env(config, args, &[])
}

fn run_workroomd_with_env(
    config: &Config,
    args: &[&str],
    extra_env: &[(String, String)],
) -> Result<String, String> {
    let mut command = Command::new(&config.workroomd_bin);
    command.args(args);
    for (key, value) in extra_env {
        command.env(key, value);
    }
    let output = command
        .output()
        .map_err(|error| format!("failed to run oa-workroomd: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "oa-workroomd failed: {}",
            redact_for_log(&String::from_utf8_lossy(&output.stderr))
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn github_work_order_prompt(order: &GitHubWorkOrder, grant: &GitHubResolvedWriteGrant) -> String {
    let issue = order
        .issue_number
        .map(|number| format!("#{number}"))
        .unwrap_or_else(|| "none".to_string());
    let issue_url = order.issue_url.as_deref().unwrap_or("none");
    let pull_request_title = order.pull_request_title.as_deref().unwrap_or("none");
    let pull_request_body = order.pull_request_body.as_deref().unwrap_or("none");
    let issue_comment = order
        .issue_comment
        .as_deref()
        .unwrap_or("default run summary");
    format!(
        "\n\nPrivate GitHub delivery contract. This section is operational context only; do not describe it as writeback or mention these mechanics in user-facing assistant messages unless the user explicitly asks:\n\
- Repository: {owner}/{repo}\n\
- Base ref: {base_ref}\n\
- Branch to create or update: {branch}\n\
- Commit message: {commit_message}\n\
- Issue: {issue}\n\
- Issue URL: {issue_url}\n\
- Push branch: {push_branch}\n\
- Open pull request: {open_pull_request}\n\
- Pull request title: {pull_request_title}\n\
- Pull request body: {pull_request_body}\n\
- Comment on issue: {comment_on_issue}\n\
- Issue comment guidance: {issue_comment}\n\
- Authenticated GitHub account: @{github_login}\n\n\
Use the `GITHUB_TOKEN` or `GH_TOKEN` environment variable for GitHub HTTPS, `gh`, or API calls. Never print, echo, write, commit, or archive that token. Do not put the token in `.git/config`, remotes, artifacts, or logs. Prefer `gh auth status` without showing token, `gh issue comment`, `gh pr create`, and `git push` with an askpass or credential helper that reads the token from the environment. Before finishing, quietly write `github-writeback.json` with branch, commit, push, issue comment, and pull request results, using URLs and SHAs only. User-facing assistant messages should summarize the product/code outcome, not this private delivery contract.",
        owner = order.repository.owner,
        repo = order.repository.repo,
        base_ref = order.base_ref,
        branch = order.branch_name,
        commit_message = order.commit_message,
        issue = issue,
        issue_url = issue_url,
        push_branch = order.writeback.push_branch,
        open_pull_request = order.writeback.open_pull_request,
        pull_request_title = pull_request_title,
        pull_request_body = pull_request_body,
        comment_on_issue = order.writeback.comment_on_issue,
        issue_comment = issue_comment,
        github_login = grant.github_login,
    )
}

struct JobEventInput {
    artifact_refs: Vec<String>,
    data_json: Option<String>,
    detail: Option<String>,
    digest: Option<String>,
    kind: String,
    receipt_refs: Vec<String>,
    redacted: bool,
    source: String,
    summary: String,
    type_: String,
}

fn append_control_event(
    config: &Config,
    run_id: &str,
    event: ControlEvent,
) -> Result<JobEvent, String> {
    append_job_event(
        config,
        run_id,
        JobEventInput {
            artifact_refs: event.artifact_refs.clone(),
            data_json: Some(
                event
                    .data_json
                    .clone()
                    .unwrap_or(json_string(&serde_json::json!({
                        "artifactRefs": event.artifact_refs,
                        "detail": event.detail,
                        "receiptRefs": event.receipt_refs,
                    }))?),
            ),
            detail: event.detail,
            digest: None,
            kind: event.kind.clone(),
            receipt_refs: event.receipt_refs,
            redacted: event.redacted,
            source: "runner".to_string(),
            summary: event.summary,
            type_: if event.kind.contains('.') {
                event.kind
            } else {
                format!("runner.{}", event.kind)
            },
        },
    )
}

fn append_control_event_once(
    config: &Config,
    run_id: &str,
    event: ControlEvent,
) -> Result<Option<JobEvent>, String> {
    let candidate_type = if event.kind.contains('.') {
        event.kind.clone()
    } else {
        format!("runner.{}", event.kind)
    };
    let candidate_detail = event.detail.clone();
    let existing = load_job_events(config, run_id, 0)?;
    if existing.iter().any(|existing| {
        existing.type_ == candidate_type
            && existing.summary == event.summary
            && existing.detail == candidate_detail
    }) {
        return Ok(None);
    }

    append_control_event(config, run_id, event).map(Some)
}

fn mirror_runner_events_once(config: &Config, run_id: &str) -> Result<(), String> {
    let mut mirrored = 0usize;
    for event_log in runner_event_logs_for_run(config, run_id)? {
        let events = load_runner_event_log_from_path(&event_log)?;
        for event in normalized_runner_events(events) {
            if append_control_event_once(config, run_id, event)?.is_some() {
                mirrored += 1;
            }
        }
    }

    if mirrored > 0 {
        send_pending_callbacks(config, run_id)?;
    }

    Ok(())
}

fn runner_event_logs_for_run(config: &Config, run_id: &str) -> Result<Vec<PathBuf>, String> {
    let root = config.state_root.join(safe_path_component(run_id));
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(Vec::new());
    };
    let mut paths = Vec::new();

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("failed to read Codex run state dir: {error}"))?;
        let candidate = entry
            .path()
            .join("state")
            .join("openagents-runner-events.jsonl");
        if candidate.exists() {
            paths.push(candidate);
        }
    }

    paths.sort();
    Ok(paths)
}

fn append_job_event(
    config: &Config,
    run_id: &str,
    input: JobEventInput,
) -> Result<JobEvent, String> {
    let mut job = load_job_record(config, run_id)?;
    let now = now_ms()?;
    let sequence = job.last_sequence + 1;
    let event = JobEvent {
        artifact_refs: input.artifact_refs,
        created_at_ms: now,
        data_json: input.data_json,
        detail: input.detail,
        digest: input.digest,
        kind: input.kind,
        receipt_refs: input.receipt_refs,
        redacted: input.redacted,
        sequence,
        source: input.source,
        summary: input.summary,
        type_: input.type_,
    };
    fs::create_dir_all(job_dir(config, run_id))
        .map_err(|error| format!("failed to create Codex job event dir: {error}"))?;
    let raw = serde_json::to_string(&event).map_err(|error| error.to_string())?;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(job_events_path(config, run_id))
        .map_err(|error| format!("failed to open Codex job events file: {error}"))?;
    writeln!(file, "{raw}")
        .map_err(|error| format!("failed to append Codex job event: {error}"))?;
    job.last_sequence = sequence;
    job.updated_at_ms = now;
    save_job_record(config, &job)?;
    Ok(event)
}

fn load_job_events(config: &Config, run_id: &str, cursor: u64) -> Result<Vec<JobEvent>, String> {
    let path = job_events_path(config, run_id);
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<JobEvent>(line).map_err(|error| error.to_string()))
        .filter_map(|result| match result {
            Ok(event) if event.sequence > cursor => Some(Ok(event)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn load_job_record(config: &Config, run_id: &str) -> Result<JobRecord, String> {
    let path = job_record_path(config, run_id);
    let raw = fs::read_to_string(&path)
        .map_err(|_| format!("Codex run {run_id} was not found on this runner"))?;
    serde_json::from_str(&raw).map_err(|error| format!("invalid Codex job record: {error}"))
}

fn save_job_record(config: &Config, job: &JobRecord) -> Result<(), String> {
    fs::create_dir_all(job_dir(config, &job.run_id))
        .map_err(|error| format!("failed to create Codex job dir: {error}"))?;
    write_json_file(&job_record_path(config, &job.run_id), job)
}

fn update_job_status(config: &Config, run_id: &str, status: &str) -> Result<(), String> {
    let mut job = load_job_record(config, run_id)?;
    job.status = status.to_string();
    job.updated_at_ms = now_ms()?;
    save_job_record(config, &job)
}

fn request_job_cancel(config: &Config, run_id: &str, reason: &str) -> Result<(), String> {
    let mut job = load_job_record(config, run_id)?;
    job.cancel_requested = true;
    job.status = "canceled".to_string();
    job.updated_at_ms = now_ms()?;
    save_job_record(config, &job)?;
    fs::write(cancel_requested_path(config, run_id), reason)
        .map_err(|error| format!("failed to persist cancel request: {error}"))?;
    append_job_event(
        config,
        run_id,
        JobEventInput {
            artifact_refs: Vec::new(),
            data_json: Some(json_string(
                &serde_json::json!({ "reason": redact_for_log(reason) }),
            )?),
            detail: Some(redact_for_log(reason)),
            digest: None,
            kind: "canceled".to_string(),
            receipt_refs: Vec::new(),
            redacted: contains_secret_marker(reason),
            source: "control".to_string(),
            summary: "Cancellation requested for Codex run.".to_string(),
            type_: "cloud.run.cancel_requested".to_string(),
        },
    )?;
    Ok(())
}

fn is_cancel_requested(config: &Config, run_id: &str) -> bool {
    cancel_requested_path(config, run_id).exists()
        || load_job_record(config, run_id)
            .map(|job| job.cancel_requested)
            .unwrap_or(false)
}

fn response_from_job(job: &JobRecord, events: Vec<JobEvent>) -> ControlResponse {
    ControlResponse {
        events: events
            .into_iter()
            .map(|event| control_event_from_job_event(&event))
            .collect(),
        external_run_id: job.external_run_id.clone(),
        run: ControlRun {
            external_run_id: job.external_run_id.clone(),
            status: job.status.clone(),
        },
        status: job.status.clone(),
    }
}

fn control_event_from_job_event(event: &JobEvent) -> ControlEvent {
    ControlEvent {
        artifact_refs: event.artifact_refs.clone(),
        data_json: event.data_json.clone(),
        detail: event.detail.clone(),
        kind: event.kind.clone(),
        receipt_refs: event.receipt_refs.clone(),
        redacted: event.redacted,
        summary: event.summary.clone(),
    }
}

fn send_pending_callbacks(config: &Config, run_id: &str) -> Result<(), String> {
    let Some(ingest) = &config.event_ingest else {
        return Ok(());
    };
    let job = load_job_record(config, run_id)?;
    let mut pending = Vec::new();
    for event in load_job_events(config, run_id, 0)? {
        if !callback_sent_path(config, run_id, event.sequence).exists() {
            pending.push(event);
        }
    }
    if pending.is_empty() {
        return Ok(());
    }

    let url = event_ingest_url(&ingest.url, run_id);
    let body = serde_json::json!({
        "events": pending
            .iter()
            .map(vortex_event_from_job_event)
            .collect::<Vec<Value>>(),
        "externalRunId": job.external_run_id.clone(),
        "status": job.status.clone(),
        "toolCalls": pending
            .iter()
            .filter_map(vortex_tool_call_from_job_event)
            .collect::<Vec<Value>>(),
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("failed to build Vortex ingest client: {error}"))?;
    let response = client
        .post(url)
        .bearer_auth(&ingest.token)
        .json(&body)
        .send()
        .map_err(|error| format!("failed to post Codex run callbacks to Vortex: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Vortex Codex run ingest rejected callbacks with HTTP {}",
            response.status().as_u16()
        ));
    }
    fs::create_dir_all(callback_dir(config, run_id))
        .map_err(|error| format!("failed to create callback marker dir: {error}"))?;
    for event in pending {
        fs::write(callback_sent_path(config, run_id, event.sequence), "sent\n")
            .map_err(|error| format!("failed to write callback marker: {error}"))?;
    }
    Ok(())
}

fn send_pending_callbacks_best_effort(config: &Config, run_id: &str) {
    if let Err(error) = send_pending_callbacks(config, run_id) {
        eprintln!("{}", redact_for_log(&error));
    }
}

fn vortex_event_from_job_event(event: &JobEvent) -> Value {
    let data_json = event.data_json.clone().unwrap_or_else(|| {
        json_string(&serde_json::json!({
            "artifactRefs": event.artifact_refs,
            "createdAtMs": event.created_at_ms,
            "detail": event.detail,
            "kind": event.kind,
            "receiptRefs": event.receipt_refs,
            "redacted": event.redacted,
            "sequence": event.sequence,
        }))
        .unwrap_or_else(|_| "{}".to_string())
    });
    serde_json::json!({
        "dataJson": data_json,
        "digest": event.digest.clone(),
        "source": event.source.clone(),
        "summary": event.summary.clone(),
        "type": event.type_.clone(),
    })
}

fn vortex_tool_call_from_job_event(event: &JobEvent) -> Option<Value> {
    let tool_name = tool_name_from_event(event)?;
    Some(serde_json::json!({
        "callId": format!("{}:{}", event.type_, event.sequence),
        "completedAt": if is_tool_terminal_event(event.type_.as_str()) {
            Some(event.created_at_ms)
        } else {
            None
        },
        "digest": event.digest.clone(),
        "inputJson": tool_input_json_from_event(event),
        "outputJson": tool_output_json_from_event(event),
        "startedAt": Some(event.created_at_ms),
        "status": tool_status_from_event(event.type_.as_str()),
        "toolName": tool_name,
    }))
}

fn tool_name_from_event(event: &JobEvent) -> Option<String> {
    let event_type = event.type_.as_str();
    if let Some(name) = event_type.strip_prefix("shell.command.") {
        let _ = name;
        return Some("shell".to_string());
    }
    if event_type.starts_with("exec_command_") || event_type.starts_with("exec_command.") {
        return Some("shell".to_string());
    }
    if event_type.starts_with("tool_call") || event_type.starts_with("tool.") {
        if let Some(name) = detail_tool_name(event.detail.as_deref()) {
            return Some(name);
        }
        return Some("tool".to_string());
    }
    if event_type.starts_with("apply_patch") || event_type == "file_edit" {
        return Some("apply_patch".to_string());
    }
    if event_type == "artifact.created" || event_type == "artifact_set.completed" {
        return Some("artifact_writer".to_string());
    }
    None
}

fn detail_tool_name(detail: Option<&str>) -> Option<String> {
    let detail = detail?;
    let parsed = serde_json::from_str::<Value>(detail).ok()?;
    parsed
        .pointer("/name")
        .and_then(Value::as_str)
        .or_else(|| parsed.pointer("/toolName").and_then(Value::as_str))
        .map(|value| value.chars().take(120).collect())
}

fn tool_status_from_event(event_type: &str) -> &'static str {
    if event_type.ends_with(".started")
        || event_type.ends_with("_begin")
        || event_type == "tool_call_delta"
        || event_type == "exec_command_begin"
    {
        "running"
    } else if event_type.contains("failed") || event_type.ends_with(".failed") {
        "failed"
    } else if event_type.contains("cancel") {
        "canceled"
    } else {
        "completed"
    }
}

fn is_tool_terminal_event(event_type: &str) -> bool {
    matches!(
        tool_status_from_event(event_type),
        "completed" | "failed" | "canceled"
    )
}

fn tool_input_json_from_event(event: &JobEvent) -> Option<String> {
    let detail = event.detail.as_deref()?;
    if event.type_.contains("output") || event.type_.contains("completed") {
        return None;
    }
    json_string(&serde_json::json!({
        "detail": detail,
        "eventType": event.type_,
        "summary": event.summary,
    }))
    .ok()
}

fn tool_output_json_from_event(event: &JobEvent) -> Option<String> {
    let detail = event.detail.as_deref()?;
    if !(event.type_.contains("output")
        || event.type_.contains("completed")
        || event.type_.contains("end"))
    {
        return None;
    }
    json_string(&serde_json::json!({
        "detail": detail,
        "eventType": event.type_,
        "summary": event.summary,
    }))
    .ok()
}

fn sse_snapshot(job: &JobRecord, events: &[JobEvent]) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "event: snapshot\ndata: {}\n\n",
        serde_json::json!({
            "cursor": job.last_sequence,
            "run": {
                "externalRunId": job.external_run_id.clone(),
                "status": job.status.clone(),
            }
        })
    ));
    for event in events {
        output.push_str(&format!(
            "id: {}\nevent: codex_event\ndata: {}\n\n",
            event.sequence,
            serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string())
        ));
    }
    output.push_str(&format!(
        "event: heartbeat\ndata: {}\n\n",
        serde_json::json!({ "cursor": job.last_sequence })
    ));
    output
}

fn job_dir(config: &Config, run_id: &str) -> PathBuf {
    config
        .state_root
        .join("jobs")
        .join(safe_path_component(run_id))
}

fn job_record_path(config: &Config, run_id: &str) -> PathBuf {
    job_dir(config, run_id).join("job.json")
}

fn job_events_path(config: &Config, run_id: &str) -> PathBuf {
    job_dir(config, run_id).join("events.jsonl")
}

fn callback_dir(config: &Config, run_id: &str) -> PathBuf {
    job_dir(config, run_id).join("callbacks")
}

fn callback_sent_path(config: &Config, run_id: &str, sequence: u64) -> PathBuf {
    callback_dir(config, run_id).join(format!("{sequence}.sent"))
}

fn cancel_requested_path(config: &Config, run_id: &str) -> PathBuf {
    job_dir(config, run_id).join("cancel.requested")
}

fn resolve_codex_auth_grant(
    config: &Config,
    request: &ControlRequest,
    now: u128,
) -> Result<ResolvedCodexAuthGrant, String> {
    let Some(resolver) = &config.grant_resolver else {
        if config.local_auth_without_grant_resolver {
            let timeout_ms = request
                .timeout_ms
                .unwrap_or(300_000)
                .clamp(1, 60 * 60 * 1000);
            let grant = VortexResolvedGrant {
                expires_at: now + timeout_ms as u128,
                grant_ref: request.auth_grant_ref.clone(),
                provider: "chatgpt_codex".to_string(),
                provider_account_ref: request.provider_account_ref.clone(),
                provider_secret_ref: format!("codex-auth://{}", request.provider_account_ref),
                requested_action: Some("local-dev".to_string()),
                status: "issued".to_string(),
            };
            validate_resolved_grant(&grant, request, now)?;
            return Ok(ResolvedCodexAuthGrant {
                auth_material: None,
                grant,
            });
        }

        return Err("Vortex grant resolver is required for Codex VM runs".to_string());
    };

    let body = VortexGrantResolveRequest {
        auth_grant_ref: request.auth_grant_ref.as_str(),
        include_auth_material: true,
        provider_account_ref: request.provider_account_ref.as_str(),
        run_id: request.run_id.as_str(),
        runner_id: request.runner_id.as_str(),
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("failed to build Vortex grant resolver client: {error}"))?;
    let response = client
        .post(&resolver.url)
        .bearer_auth(&resolver.token)
        .json(&body)
        .send()
        .map_err(|error| format!("failed to resolve Vortex Codex auth grant: {error}"))?;
    let status = response.status();
    let text = response
        .text()
        .map_err(|error| format!("failed to read Vortex grant resolver response: {error}"))?;

    if !status.is_success() {
        return Err(format!(
            "Vortex grant resolver rejected Codex auth grant with HTTP {}",
            status.as_u16()
        ));
    }

    let resolved: VortexGrantResolveResponse = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse Vortex grant resolver response: {error}"))?;
    if resolved.status != "resolved" {
        return Err("Vortex grant resolver returned an unexpected status".to_string());
    }
    validate_resolved_grant(&resolved.grant, request, now)?;
    if let Some(auth_material) = &resolved.auth_material {
        validate_vortex_auth_material(auth_material)?;
    } else {
        return Err(
            "Vortex grant resolver did not include current ChatGPT/Codex auth material".to_string(),
        );
    }

    Ok(ResolvedCodexAuthGrant {
        auth_material: resolved.auth_material,
        grant: resolved.grant,
    })
}

fn validate_resolved_grant(
    grant: &VortexResolvedGrant,
    request: &ControlRequest,
    now: u128,
) -> Result<(), String> {
    if grant.provider != "chatgpt_codex" {
        return Err("Vortex grant is not for ChatGPT/Codex".to_string());
    }
    if grant.status != "issued" {
        return Err("Vortex grant is not issued".to_string());
    }
    if grant.grant_ref != request.auth_grant_ref {
        return Err("Vortex grant ref does not match assignment".to_string());
    }
    if grant.provider_account_ref != request.provider_account_ref {
        return Err("Vortex grant provider account does not match assignment".to_string());
    }
    if grant.expires_at <= now {
        return Err("Vortex grant is expired".to_string());
    }
    validate_provider_secret_ref(&grant.provider_secret_ref)?;

    Ok(())
}

fn validate_vortex_auth_material(auth_material: &VortexAuthMaterial) -> Result<(), String> {
    if auth_material.auth_content_env != "OPENCODE_AUTH_CONTENT" {
        return Err("Vortex auth material used an unsupported environment target".to_string());
    }

    let parsed = serde_json::from_str::<Value>(&auth_material.auth_content_json)
        .map_err(|error| format!("Vortex auth material is not valid JSON: {error}"))?;

    if contains_api_key_auth_material(&parsed) {
        return Err(
            "Vortex auth material appears to contain OpenAI API-key material; reconnect ChatGPT/Codex through Vortex instead"
                .to_string(),
        );
    }

    let openai = parsed
        .get("openai")
        .and_then(Value::as_object)
        .ok_or_else(|| "Vortex auth material is missing OpenAI OAuth content".to_string())?;
    let auth_type = openai
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let access = openai
        .get("access")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let refresh = openai
        .get("refresh")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if auth_type != "oauth" || access.trim().is_empty() || refresh.trim().is_empty() {
        return Err("Vortex auth material is not ChatGPT/Codex OAuth content".to_string());
    }

    Ok(())
}

fn resolve_github_write_grant(
    config: &Config,
    request: &ControlRequest,
    now: u128,
) -> Result<Option<GitHubResolvedWriteGrant>, String> {
    let Some(github_write_grant_ref) = request.github_write_grant_ref.as_deref() else {
        return Ok(None);
    };
    let Some(resolver) = &config.github_write_grant_resolver else {
        return Err(
            "GitHub write grant resolver is required for GitHub writeback runs".to_string(),
        );
    };
    let body = GitHubWriteGrantResolveRequest {
        github_write_grant_ref,
        run_id: request.run_id.as_str(),
        runner_id: request.runner_id.as_str(),
        runner_session_id: request.run_id.as_str(),
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("failed to build GitHub write grant resolver client: {error}"))?;
    let response = client
        .post(&resolver.url)
        .bearer_auth(&resolver.token)
        .json(&body)
        .send()
        .map_err(|error| format!("failed to resolve GitHub write grant: {error}"))?;
    let status = response.status();
    let text = response
        .text()
        .map_err(|error| format!("failed to read GitHub write grant resolver response: {error}"))?;

    if !status.is_success() {
        return Err(format!(
            "GitHub write grant resolver rejected grant with HTTP {}",
            status.as_u16()
        ));
    }

    let resolved: GitHubWriteGrantResolveResponse =
        serde_json::from_str(&text).map_err(|error| {
            format!("failed to parse GitHub write grant resolver response: {error}")
        })?;
    validate_github_write_grant(&resolved.grant, request, now)?;

    Ok(Some(resolved.grant))
}

fn validate_github_write_grant(
    grant: &GitHubResolvedWriteGrant,
    request: &ControlRequest,
    now: u128,
) -> Result<(), String> {
    if grant.status != "issued" {
        return Err("GitHub write grant is not issued".to_string());
    }
    if grant.grant_ref != request.github_write_grant_ref.clone().unwrap_or_default() {
        return Err("GitHub write grant ref does not match assignment".to_string());
    }
    if let Some(connection_ref) = &request.github_write_connection_ref {
        if grant.connection_ref != *connection_ref {
            return Err("GitHub write connection ref does not match assignment".to_string());
        }
    }
    if grant.expires_at <= now {
        return Err("GitHub write grant is expired".to_string());
    }
    if grant.credential.provider != "github" || grant.credential.token_type != "oauth" {
        return Err("GitHub write grant credential is not a GitHub OAuth token".to_string());
    }
    if !grant.credential.scopes.iter().any(|scope| scope == "repo")
        || !grant
            .credential
            .scopes
            .iter()
            .any(|scope| scope == "workflow")
    {
        return Err("GitHub write grant is missing required scopes".to_string());
    }
    if grant.credential.access_token.trim().is_empty()
        || contains_secret_marker(grant.credential.access_token.as_str())
    {
        return Err("GitHub write grant credential is invalid".to_string());
    }
    validate_github_secret_ref(&grant.materialization.auth_ref)?;
    if grant.materialization.git_credential_env != "GITHUB_TOKEN"
        || grant.materialization.provider != "github"
        || grant.materialization.remote_url_mode != "https_token"
        || !grant.materialization.scrub_after_closeout
    {
        return Err("GitHub write grant materialization plan is unsupported".to_string());
    }
    if let Some(runner_session_id) = &grant.runner_session_id {
        if runner_session_id != &request.run_id {
            return Err("GitHub write grant runner session does not match assignment".to_string());
        }
    }

    Ok(())
}

fn validate_github_secret_ref(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 240
        || trimmed.contains('\n')
        || contains_secret_marker(trimmed)
        || !trimmed.starts_with("github-write://")
    {
        return Err("GitHub write grant auth ref is not approved".to_string());
    }

    Ok(())
}

fn validate_github_work_order(
    order: &GitHubWorkOrder,
    request: &ControlRequest,
) -> Result<(), String> {
    if order.provider != "github" || order.repository.provider != "github" {
        return Err("GitHub work order provider must be github".to_string());
    }
    if let Some(repository) = &request.repository {
        let expected = format!("{}/{}", order.repository.owner, order.repository.repo);
        if repository != &expected {
            return Err("GitHub work order repository does not match request".to_string());
        }
    }
    for (field, value) in [
        ("repository owner", order.repository.owner.as_str()),
        ("repository name", order.repository.repo.as_str()),
        ("base ref", order.base_ref.as_str()),
        ("branch name", order.branch_name.as_str()),
        ("commit message", order.commit_message.as_str()),
    ] {
        if value.trim().is_empty() || contains_secret_marker(value) {
            return Err(format!("GitHub work order {field} is invalid"));
        }
    }
    if order.branch_name.starts_with('.')
        || order.branch_name.contains("..")
        || order.branch_name.contains('\\')
        || order.branch_name.len() > 160
    {
        return Err("GitHub work order branch is invalid".to_string());
    }
    if let Some(issue_number) = order.issue_number {
        if issue_number == 0 {
            return Err("GitHub work order issue number is invalid".to_string());
        }
    }
    for (field, value) in [
        ("issue URL", order.issue_url.as_deref()),
        ("issue comment", order.issue_comment.as_deref()),
        ("pull request title", order.pull_request_title.as_deref()),
        ("pull request body", order.pull_request_body.as_deref()),
    ] {
        if let Some(value) = value {
            if value.trim().is_empty() || contains_secret_marker(value) {
                return Err(format!("GitHub work order {field} is invalid"));
            }
        }
    }
    if order.writeback.open_pull_request && order.pull_request_title.is_none() {
        return Err("GitHub work order pull request title is required".to_string());
    }

    Ok(())
}

fn validate_provider_secret_ref(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    let allowed = [
        "secret://",
        "vault://",
        "gcp-secret://",
        "cloud-secret://",
        "provider-account://",
        "codex-auth://",
    ];

    if trimmed.is_empty()
        || trimmed.len() > 240
        || trimmed.contains('\n')
        || contains_secret_marker(trimmed)
        || !allowed.iter().any(|prefix| trimmed.starts_with(prefix))
    {
        return Err(
            "Vortex grant provider secret ref is not an approved secret reference".to_string(),
        );
    }

    Ok(())
}

fn codex_auth_cache_path(
    config: &Config,
    resolved: &ResolvedCodexAuthGrant,
    request: &ControlRequest,
    run_dir: &Path,
) -> Result<PathBuf, String> {
    if let Some(auth_material) = &resolved.auth_material {
        validate_vortex_auth_material(auth_material)?;
        let path = run_dir.join("codex-auth-material.json");
        write_secret_file(&path, auth_material.auth_content_json.as_bytes())?;

        return Ok(path);
    }

    if let Some(root) = &config.auth_json_root {
        let account_ref = resolved
            .grant
            .provider_secret_ref
            .strip_prefix("codex-auth://")
            .unwrap_or(request.provider_account_ref.as_str());

        if account_ref != request.provider_account_ref {
            return Err("Vortex grant provider secret ref does not match account ref".to_string());
        }
        validate_provider_account_path_component(account_ref)?;

        return Ok(root.join(account_ref).join("auth.json"));
    }

    config
        .auth_json_file
        .clone()
        .ok_or_else(|| "missing configured Codex auth cache path".to_string())
}

fn validate_provider_account_path_component(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 120
        || value.starts_with('.')
        || value.contains("..")
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("provider account ref is not safe for account-scoped auth path".to_string());
    }

    Ok(())
}

fn validate_codex_auth_cache_file(path: &Path) -> Result<(), String> {
    let raw = fs::read(path).map_err(|error| {
        format!("failed to read VM-side Codex auth cache from configured secret path: {error}")
    })?;

    if raw.is_empty() {
        return Err("VM-side Codex auth cache is empty".to_string());
    }

    let parsed = serde_json::from_slice::<Value>(&raw)
        .map_err(|error| format!("VM-side Codex auth cache is not valid JSON: {error}"))?;

    if contains_api_key_auth_material(&parsed) {
        return Err(
            "VM-side Codex auth cache appears to contain OpenAI API-key material; reconnect ChatGPT/Codex through Vortex instead"
                .to_string(),
        );
    }

    Ok(())
}

fn contains_api_key_auth_material(value: &Value) -> bool {
    contains_api_key_auth_material_at(value, None)
}

fn contains_api_key_auth_material_at(value: &Value, key: Option<&str>) -> bool {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            let lower = trimmed.to_ascii_lowercase();
            trimmed.starts_with("sk-")
                || matches_auth_mode_field(key)
                    && matches!(lower.as_str(), "apikey" | "api_key" | "api-key")
                || matches_api_key_field(key) && !trimmed.is_empty()
        }
        Value::Array(values) => values
            .iter()
            .any(|child| contains_api_key_auth_material_at(child, None)),
        Value::Object(values) => values.iter().any(|(child_key, child)| {
            let normalized_key = child_key.to_ascii_lowercase();
            if matches_api_key_field(Some(&normalized_key)) && !child.is_null() {
                match child {
                    Value::String(text) => !text.trim().is_empty(),
                    Value::Bool(false) => false,
                    Value::Array(values) if values.is_empty() => false,
                    Value::Object(values) if values.is_empty() => false,
                    _ => true,
                }
            } else {
                contains_api_key_auth_material_at(child, Some(&normalized_key))
            }
        }),
        _ => false,
    }
}

fn matches_api_key_field(key: Option<&str>) -> bool {
    let Some(key) = key else {
        return false;
    };
    matches!(
        key,
        "openai_api_key" | "openai-api-key" | "api_key" | "api-key" | "apikey" | "apiKey"
    )
}

fn matches_auth_mode_field(key: Option<&str>) -> bool {
    let Some(key) = key else {
        return false;
    };
    matches!(key, "auth_mode" | "authmode" | "auth-mode")
}

fn auth_grant_resolved_event(grant: &VortexResolvedGrant) -> ControlEvent {
    ControlEvent {
        artifact_refs: Vec::new(),
        data_json: None,
        detail: Some(format!(
            "Grant {} resolved for account {}; provider secret ref stayed server-side. Requested action: {}.",
            grant.grant_ref,
            grant.provider_account_ref,
            grant
                .requested_action
                .as_deref()
                .unwrap_or("codex-workroom")
        )),
        kind: "auth_grant_resolved".to_string(),
        receipt_refs: Vec::new(),
        redacted: false,
        summary: "Vortex ChatGPT/Codex account grant resolved without API-key fallback."
            .to_string(),
    }
}

fn github_write_grant_resolved_event(grant: &GitHubResolvedWriteGrant) -> ControlEvent {
    ControlEvent {
        artifact_refs: Vec::new(),
        data_json: None,
        detail: Some(format!(
            "GitHub write grant {} resolved for @{} with scopes {}; requested action: {}; token stayed process-local.",
            grant.grant_ref,
            grant.github_login,
            grant.credential.scopes.join(","),
            grant
                .requested_action
                .as_deref()
                .unwrap_or("github-writeback")
        )),
        kind: "github_write_grant_resolved".to_string(),
        receipt_refs: Vec::new(),
        redacted: false,
        summary: "OpenAgents GitHub write grant resolved for SHC workroom.".to_string(),
    }
}

fn github_write_environment(grant: &Option<GitHubResolvedWriteGrant>) -> Vec<(String, String)> {
    let Some(grant) = grant else {
        return Vec::new();
    };

    vec![
        (
            "GITHUB_TOKEN".to_string(),
            grant.credential.access_token.clone(),
        ),
        (
            "GH_TOKEN".to_string(),
            grant.credential.access_token.clone(),
        ),
        (
            "OPENAGENTS_GITHUB_WRITE_ENABLED".to_string(),
            "true".to_string(),
        ),
        (
            "OPENAGENTS_CODEX_REPO_CHECKOUT".to_string(),
            "enabled".to_string(),
        ),
        (
            "OPENAGENTS_GITHUB_WRITE_CONNECTION_REF".to_string(),
            grant.connection_ref.clone(),
        ),
        (
            "OPENAGENTS_GITHUB_LOGIN".to_string(),
            grant.github_login.clone(),
        ),
    ]
}

fn normalized_events(events: Vec<Value>, include_failure_if_empty: bool) -> Vec<ControlEvent> {
    let mut normalized = Vec::new();

    for event in events {
        let kind = event
            .pointer("/event_kind")
            .and_then(Value::as_str)
            .unwrap_or("log");
        if kind == "queued" || kind == "started" {
            continue;
        }
        let message = event
            .pointer("/message")
            .and_then(Value::as_str)
            .unwrap_or("runner event");
        let redacted = kind == "redacted" || contains_secret_marker(message);
        let artifact_refs = event
            .pointer("/artifact_ref")
            .and_then(Value::as_str)
            .map(|value| vec![value.to_string()])
            .unwrap_or_default();
        let receipt_refs = event
            .pointer("/receipt_ref")
            .and_then(Value::as_str)
            .map(|value| vec![value.to_string()])
            .unwrap_or_default();

        normalized.push(ControlEvent {
            artifact_refs,
            data_json: None,
            detail: Some(redact_for_log(message)),
            kind: if redacted { "redacted" } else { kind }.to_string(),
            receipt_refs,
            redacted,
            summary: summary_for_kind(kind),
        });
    }

    if normalized.is_empty() && include_failure_if_empty {
        normalized.push(failed_event("Codex runner failed before emitting events."));
    }

    normalized
}

fn normalized_runner_events(events: Vec<Value>) -> Vec<ControlEvent> {
    events
        .into_iter()
        .filter_map(|event| {
            let event_type = event
                .pointer("/type")
                .and_then(Value::as_str)
                .or_else(|| event.pointer("/event_type").and_then(Value::as_str))?;
            let summary = event
                .pointer("/summary")
                .and_then(Value::as_str)
                .unwrap_or("OpenAgents runner event captured.");
            let detail = event
                .pointer("/detail_excerpt")
                .and_then(Value::as_str)
                .map(redact_for_log);
            let redacted =
                event_type == "redacted" || detail.as_deref().is_some_and(contains_secret_marker);
            let raw_payload_json = event
                .pointer("/raw_payload_json")
                .and_then(Value::as_str)
                .filter(|value| !contains_secret_marker(value))
                .map(str::to_string);
            let artifact_refs = event
                .pointer("/artifact_refs")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let receipt_refs = event
                .pointer("/receipt_refs")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Some(ControlEvent {
                artifact_refs,
                data_json: raw_payload_json,
                detail,
                kind: event_type.to_string(),
                receipt_refs,
                redacted,
                summary: redact_for_log(summary),
            })
        })
        .collect()
}

fn load_event_log(state_dir: &Path) -> Result<Vec<Value>, String> {
    let path = state_dir.join("codex-run-events.jsonl");
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };

    Ok(raw
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect())
}

fn load_runner_event_log(state_dir: &Path) -> Result<Vec<Value>, String> {
    let path = state_dir.join("openagents-runner-events.jsonl");
    load_runner_event_log_from_path(&path)
}

fn load_runner_event_log_from_path(path: &Path) -> Result<Vec<Value>, String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };

    Ok(raw
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect())
}

fn summary_for_kind(kind: &str) -> String {
    match kind {
        "artifact" => "Codex VM artifact captured.".to_string(),
        "receipt" => "Codex VM receipt emitted.".to_string(),
        "completed" => "Codex VM workroom completed.".to_string(),
        "failed" => "Codex VM workroom failed.".to_string(),
        "timeout" => "Codex VM workroom timed out.".to_string(),
        "cleanup" => "Codex VM cleanup completed.".to_string(),
        "redacted" => "Codex VM emitted redacted output.".to_string(),
        _ => "Codex VM log captured.".to_string(),
    }
}

fn failed_event(message: &str) -> ControlEvent {
    ControlEvent {
        artifact_refs: Vec::new(),
        data_json: None,
        detail: Some(redact_for_log(message)),
        kind: "failed".to_string(),
        receipt_refs: Vec::new(),
        redacted: contains_secret_marker(message),
        summary: "Codex VM control run failed.".to_string(),
    }
}

fn parse_sandbox_mode(value: Option<&str>) -> CodexSandboxMode {
    match value {
        Some("read_only") | Some("read-only") => CodexSandboxMode::ReadOnly,
        Some("workspace_write") | Some("workspace-write") => CodexSandboxMode::WorkspaceWrite,
        _ => CodexSandboxMode::DangerFullAccess,
    }
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    let temp_path = path.with_extension(format!(
        "tmp-{}-{}-{}",
        std::process::id(),
        now_ms().unwrap_or(0),
        JSON_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&temp_path, format!("{raw}\n"))
        .map_err(|error| format!("failed to write {}: {error}", temp_path.display()))?;
    fs::rename(&temp_path, path)
        .map_err(|error| format!("failed to commit {}: {error}", path.display()))
}

fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp_path = path.with_extension(format!(
        "tmp-{}-{}-secret",
        std::process::id(),
        JSON_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&temp_path, bytes)
        .map_err(|error| format!("failed to write run-scoped auth material: {error}"))?;
    set_owner_only_permissions(&temp_path)?;
    fs::rename(&temp_path, path)
        .map_err(|error| format!("failed to commit run-scoped auth material: {error}"))?;
    set_owner_only_permissions(path)
}

fn set_owner_only_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to restrict file permissions: {error}"))?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

fn write_response<T: Serialize>(
    stream: &mut TcpStream,
    status: u16,
    value: &T,
) -> Result<(), String> {
    let body = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncache-control: no-store\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    )
    .map_err(|error| error.to_string())?;
    stream.write_all(&body).map_err(|error| error.to_string())
}

#[derive(Debug)]
struct HttpRequest {
    authorization: Option<String>,
    body: Vec<u8>,
    method: String,
    path: String,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let mut header_end = None;

    while header_end.is_none() && buffer.len() < MAX_BODY_BYTES {
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        header_end = find_header_end(&buffer);
    }

    let header_end = header_end.ok_or_else(|| "missing HTTP header terminator".to_string())?;
    let header = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let path = request_parts.next().unwrap_or_default().to_string();
    let mut content_length = 0_usize;
    let mut authorization = None;

    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            if name == "content-length" {
                content_length = value
                    .parse::<usize>()
                    .map_err(|_| "invalid content-length".to_string())?;
            } else if name == "authorization" {
                authorization = Some(value.to_string());
            }
        }
    }

    if content_length > MAX_BODY_BYTES {
        return Err("request body too large".to_string());
    }

    let body_start = header_end + 4;
    let mut body = buffer.get(body_start..).unwrap_or_default().to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(HttpRequest {
        authorization,
        body,
        method,
        path,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn authorized(header: &Option<String>, token: &str) -> bool {
    header
        .as_deref()
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|value| value.trim() == token)
}

fn split_path_query(path: &str) -> (&str, Option<&str>) {
    path.split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((path, None))
}

fn parse_run_action(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("/v1/codex-runs/")?;
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let run_id = parts.next()?;
    let action = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((run_id, action))
}

fn cursor_from_query(query: Option<&str>) -> u64 {
    query
        .unwrap_or_default()
        .split('&')
        .filter_map(|part| part.split_once('='))
        .find_map(|(key, value)| {
            if key == "cursor" {
                value.parse::<u64>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn json_string(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "yes"
        })
        .unwrap_or(false)
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Resolve the neutral env var first, falling back to the legacy Vortex-named
/// var when the neutral one is unset. The grant resolver endpoint is
/// caller-neutral now (cloud#87): Pylon-originated coding runs use the same
/// resolver as the deprecated Vortex codebase did, but the daemon no longer
/// depends on a Vortex-shaped caller or on Vortex-named configuration. The
/// Vortex credential/endpoint may be reused, but the Vortex codebase is
/// deprecated; prefer the neutral names so it can be removed without breaking
/// the loop.
fn neutral_env(neutral: &str, legacy: &str) -> Option<String> {
    optional_env(neutral).or_else(|| optional_env(legacy))
}

fn grant_resolver_from_env(
    allow_local_auth_without_grant_resolver: bool,
) -> Result<Option<CodexGrantResolver>, String> {
    let url = neutral_env("OA_CODEX_GRANT_RESOLVE_URL", "OA_VORTEX_GRANT_RESOLVE_URL");
    let token = neutral_env(
        "OA_CODEX_RUNNER_GRANT_TOKEN",
        "OA_VORTEX_CLOUD_RUNNER_GRANT_TOKEN",
    );

    match (url, token) {
        (Some(url), Some(token)) => Ok(Some(CodexGrantResolver { token, url })),
        (None, None) if allow_local_auth_without_grant_resolver => Ok(None),
        (None, None) => Err(
            "missing OA_CODEX_GRANT_RESOLVE_URL (or legacy OA_VORTEX_GRANT_RESOLVE_URL) and OA_CODEX_RUNNER_GRANT_TOKEN (or legacy OA_VORTEX_CLOUD_RUNNER_GRANT_TOKEN); set OA_CODEX_CONTROL_ALLOW_LOCAL_AUTH_ONLY=true only for local development".to_string(),
        ),
        _ => Err(
            "the grant resolve URL and runner grant token must be configured together (OA_CODEX_GRANT_RESOLVE_URL + OA_CODEX_RUNNER_GRANT_TOKEN, or the legacy OA_VORTEX_* pair)".to_string(),
        ),
    }
}

fn github_write_grant_resolver_from_env() -> Option<CodexGrantResolver> {
    let url = optional_env("OA_OPENAGENTS_GITHUB_WRITE_GRANT_RESOLVE_URL")
        .unwrap_or_else(|| "https://openagents.com/api/github-write/grants/resolve".to_string());
    let token = optional_env("OA_OPENAGENTS_GITHUB_WRITE_GRANT_TOKEN").or_else(|| {
        neutral_env(
            "OA_CODEX_RUNNER_GRANT_TOKEN",
            "OA_VORTEX_CLOUD_RUNNER_GRANT_TOKEN",
        )
    })?;

    Some(CodexGrantResolver { token, url })
}

fn event_ingest_from_env(control_token: &str) -> Result<Option<EventIngest>, String> {
    let Some(url) = neutral_env("OA_CODEX_EVENT_INGEST_URL", "OA_VORTEX_CODEX_INGEST_URL") else {
        return Ok(None);
    };
    let token = neutral_env(
        "OA_CODEX_EVENT_INGEST_TOKEN",
        "OA_VORTEX_CODEX_INGEST_TOKEN",
    )
    .unwrap_or_else(|| control_token.to_string());
    Ok(Some(EventIngest { token, url }))
}

fn event_ingest_url(base_url: &str, run_id: &str) -> String {
    if base_url.contains("{runId}") {
        return base_url.replace("{runId}", run_id);
    }
    format!(
        "{}/{}/events/ingest",
        base_url.trim_end_matches('/'),
        run_id
    )
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .map(|value| value.trim().to_string())
        .map_err(|_| format!("missing required env {name}"))
        .and_then(|value| {
            if value.is_empty() {
                Err(format!("missing required env {name}"))
            } else {
                Ok(value)
            }
        })
}

fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
}

fn external_run_id(runner_id: &str, run_id: &str) -> String {
    format!("shc-codex:{runner_id}:{run_id}")
}

fn safe_path_component(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars().take(96) {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        short_digest(value)
    } else {
        output
    }
}

fn short_digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn now_ms() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| error.to_string())
}

fn contains_secret_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("access_token")
        || lower.contains("refresh_token")
        || lower.contains("id_token")
        || lower.contains("authorization:")
        || lower.contains("bearer ")
        || lower.contains("auth.json")
        || lower.contains("private key")
        || value.contains("sk-")
}

fn redact_for_log(value: &str) -> String {
    if contains_secret_marker(value) {
        "[redacted:secret]".to_string()
    } else {
        value.chars().take(500).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sandbox_aliases() {
        assert_eq!(
            parse_sandbox_mode(Some("read_only")),
            CodexSandboxMode::ReadOnly
        );
        assert_eq!(
            parse_sandbox_mode(Some("workspace-write")),
            CodexSandboxMode::WorkspaceWrite
        );
        assert_eq!(
            parse_sandbox_mode(Some("danger_full_access")),
            CodexSandboxMode::DangerFullAccess
        );
        assert_eq!(parse_sandbox_mode(None), CodexSandboxMode::DangerFullAccess);
    }

    #[test]
    fn redacts_secret_markers() {
        assert_eq!(redact_for_log("access_token=abc"), "[redacted:secret]");
        assert_eq!(redact_for_log("plain status"), "plain status");
    }

    #[test]
    fn validates_vortex_secret_refs_without_raw_material() {
        assert!(validate_provider_secret_ref("secret://codex/provider-account_123").is_ok());
        assert!(validate_provider_secret_ref("codex-auth://provider-account_123").is_ok());
        assert!(validate_provider_secret_ref("sk-abcdefghijklmnopqrstuvwxyz").is_err());
        assert!(validate_provider_secret_ref("secret://codex/account\nBearer abc").is_err());
    }

    #[test]
    fn validates_github_writeback_without_persisting_token() {
        let request = ControlRequest {
            agent_runtime: None,
            auth_grant_ref: "codex-auth-grant_123".to_string(),
            github_write_connection_ref: Some("github-write_123".to_string()),
            github_write_grant_ref: Some("github-write-grant_123".to_string()),
            github_work_order: Some(test_github_work_order()),
            goal: "change code".to_string(),
            lane: None,
            owner_ref: None,
            provider_account_ref: "provider-account_abc123".to_string(),
            repository: Some("OpenAgentsInc/autopilot".to_string()),
            repository_clone_url: None,
            repository_ref: Some("main".to_string()),
            required_artifacts: None,
            retention_mode: None,
            runner_id: "oa-shc-katy-01".to_string(),
            run_id: "agent_run_123".to_string(),
            sandbox_mode: Some("danger_full_access".to_string()),
            timeout_ms: Some(60_000),
        };
        let grant = test_github_write_grant();

        validate_github_work_order(request.github_work_order.as_ref().unwrap(), &request).unwrap();
        validate_github_write_grant(&grant, &request, grant.expires_at - 1).unwrap();

        let prompt = codex_prompt(
            "change code",
            &["result.md".to_string(), "github-writeback.json".to_string()],
            request.github_work_order.as_ref(),
            Some(&grant),
        );
        let env = github_write_environment(&Some(grant.clone()));

        assert!(prompt.contains("Private GitHub delivery contract"));
        assert!(prompt.contains("do not describe it as writeback"));
        assert!(prompt.contains("Do not mention this closeout contract"));
        assert!(prompt.contains("Use the `GITHUB_TOKEN` or `GH_TOKEN` environment variable"));
        assert!(!prompt.contains(grant.credential.access_token.as_str()));
        assert!(env.iter().any(|(key, value)| {
            key == "GITHUB_TOKEN" && value == grant.credential.access_token.as_str()
        }));
        assert!(env.iter().any(|(key, value)| {
            key == "GH_TOKEN" && value == grant.credential.access_token.as_str()
        }));
        assert!(env
            .iter()
            .any(|(key, value)| { key == "OPENAGENTS_CODEX_REPO_CHECKOUT" && value == "enabled" }));
        assert_eq!(
            required_artifacts_for_request(&request),
            vec!["result.md".to_string(), "github-writeback.json".to_string()]
        );
        assert_eq!(
            assignment_repo_ref(&request),
            "OpenAgentsInc/autopilot@main"
        );
    }

    fn placement_assignment(lane: ComputeLane) -> PlacementAssignment {
        PlacementAssignment {
            contract_version: openagents_cloud_contract::PLACEMENT_ASSIGNMENT_VERSION.to_string(),
            run_id: "agent_run_99".to_string(),
            owner_ref: "owner://sha256/example".to_string(),
            provider_account_ref: "provider-account_abc123".to_string(),
            auth_grant_ref: "codex-auth-grant_123".to_string(),
            goal: "Create the requested artifact.".to_string(),
            lane,
            repository: Some("OpenAgentsInc/openagents".to_string()),
            sandbox_mode: None,
            wallet_authority: false,
            created_at_ms: 1,
            work_context_b64: None,
            work_context_ref: None,
        }
    }

    #[test]
    fn placement_isolation_gate_rejects_pooled_and_wallet_placements() {
        // CX-1 (openagents#8545): the same fail-closed gate `start_placement_async`
        // applies before binding a runner. A scoped placement passes; a pooled
        // (grant-less) or wallet-bearing placement is refused.
        let policy = AgentComputerIsolationPolicy::default();

        let scoped = placement_assignment(ComputeLane::CloudGcp);
        policy.validate_placement(&scoped, None).unwrap();

        let mut pooled = placement_assignment(ComputeLane::CloudGcp);
        pooled.auth_grant_ref = String::new();
        assert!(policy.validate_placement(&pooled, None).is_err());

        let mut wallet = placement_assignment(ComputeLane::CloudGcp);
        wallet.wallet_authority = true;
        assert!(policy.validate_placement(&wallet, None).is_err());
    }

    #[test]
    fn work_context_b64_guard_accepts_base64_and_rejects_injection() {
        // Real base64 (of `{"a":1}`) is accepted.
        assert!(is_valid_work_context_b64("eyJhIjoxfQ=="));
        // Empty, shell metacharacters, quotes, and whitespace are rejected so the
        // blob can never break out of the single-quoted guest command.
        assert!(!is_valid_work_context_b64(""));
        assert!(!is_valid_work_context_b64("abc'; rm -rf / #"));
        assert!(!is_valid_work_context_b64("a b c"));
        assert!(!is_valid_work_context_b64("$(whoami)"));
    }

    #[test]
    fn microvm_turn_runner_command_decodes_blob_and_runs_turn_runner() {
        let cmd = microvm_turn_runner_command("eyJhIjoxfQ==");
        assert_eq!(cmd[0], "bash");
        assert_eq!(cmd[1], "-lc");
        // Decodes into /tmp/wc.json and invokes the baked turn-runner against it,
        // writing artifacts under the shared VM artifact dir.
        assert!(cmd[2].contains("base64 -d > /tmp/wc.json"));
        assert!(cmd[2].contains("/opt/agent/turn-runner /tmp/wc.json"));
        assert!(cmd[2].contains(cloud_vm::VM_ARTIFACT_DIR));
    }

    #[test]
    fn placement_defaults_to_gce_cost_driven_and_builds_control_request() {
        // CND-042: default config is cost-driven (cost_driven=true), the report
        // recommends HOLD (expand=false), and GCE is cheaper, so Auto resolves
        // to GCE via the cost-driven path with a recorded cost basis.
        let config = test_config("placement-gce");
        assert!(config.placement_cost_driven);
        assert!(!config.placement_shc_pilot_expand);
        let assignment = placement_assignment(ComputeLane::Auto);
        let binding = resolve_placement_binding(&config, &assignment).unwrap();
        assert_eq!(
            binding.provider_lane,
            openagents_cloud_contract::ProviderLane::Gcp
        );
        assert_eq!(binding.lane, ComputeLane::CloudGcp);
        assert_eq!(
            binding.reason,
            openagents_cloud_contract::PlacementReason::CostDriven
        );
        assert!(binding.cost_driven);
        let basis = binding
            .cost_basis
            .as_ref()
            .expect("cost basis on cost-driven GCE");
        assert!(basis.gce_micro_usd_per_vm_sec < basis.shc_micro_usd_per_vm_sec);

        let request = control_request_from_placement(&assignment, &binding);
        assert_eq!(request.run_id, "agent_run_99");
        assert_eq!(request.runner_id, binding.runner_id);
        assert_eq!(request.lane, Some(ComputeLane::CloudGcp));
        assert_eq!(request.sandbox_mode.as_deref(), Some("danger_full_access"));
        assert_eq!(request.owner_ref.as_deref(), Some("owner://sha256/example"));
    }

    #[test]
    fn placement_policy_driven_when_cost_driven_disabled() {
        // OA_CODEX_PLACEMENT_COST_DRIVEN=false restores policy-driven Google
        // first with cost_driven=false and no cost basis.
        let mut config = test_config("placement-policy");
        config.placement_cost_driven = false;
        let assignment = placement_assignment(ComputeLane::Auto);
        let binding = resolve_placement_binding(&config, &assignment).unwrap();
        assert_eq!(binding.lane, ComputeLane::CloudGcp);
        assert_eq!(
            binding.reason,
            openagents_cloud_contract::PlacementReason::PolicyDefaultGce
        );
        assert!(!binding.cost_driven);
        assert!(binding.cost_basis.is_none());
    }

    #[test]
    fn placement_falls_back_to_shc_when_gce_disabled() {
        let mut config = test_config("placement-shc");
        config.placement_gce_available = false;
        let assignment = placement_assignment(ComputeLane::Auto);
        let binding = resolve_placement_binding(&config, &assignment).unwrap();
        assert_eq!(
            binding.provider_lane,
            openagents_cloud_contract::ProviderLane::Shc
        );
        assert_eq!(binding.runner_id, SHC_FALLBACK_RUNNER_ID);
    }

    #[test]
    fn placement_bound_event_is_refs_only() {
        let config = test_config("placement-event");
        let assignment = placement_assignment(ComputeLane::Auto);
        let binding = resolve_placement_binding(&config, &assignment).unwrap();
        let event = placement_bound_event(&binding);
        assert_eq!(event.kind, "placement.bound");
        assert!(!event.redacted);
        // The serialized binding must not contain raw secret material.
        let data = event.data_json.unwrap();
        assert!(!data.to_ascii_lowercase().contains("access_token"));
        assert!(!data.to_ascii_lowercase().contains("bearer "));
    }

    fn register_test_job(config: &Config, request: &ControlRequest) {
        let now = now_ms().unwrap();
        let job = JobRecord {
            cancel_requested: false,
            created_at_ms: now,
            external_run_id: external_run_id(&request.runner_id, &request.run_id),
            last_sequence: 0,
            request: request.clone(),
            run_id: request.run_id.clone(),
            runner_id: request.runner_id.clone(),
            status: "running".to_string(),
            updated_at_ms: now,
        };
        fs::create_dir_all(job_dir(config, &request.run_id)).unwrap();
        save_job_record(config, &job).unwrap();
    }

    #[test]
    fn gce_lane_provisions_runs_emits_receipt_and_cleans_up() {
        let config = test_config("gce-lifecycle");
        let mut request = test_request("run_gce_lifecycle");
        request.lane = Some(ComputeLane::CloudGcp);
        request.owner_ref = Some("owner://sha256/example".to_string());
        register_test_job(&config, &request);

        // Acquire (provision) -> in_use.
        let lease = acquire_gce_lease_for_run(&config, &request)
            .unwrap()
            .expect("fake provisioner should acquire a lease");
        assert_eq!(lease.projection.state, gce_capacity::LeaseState::InUse);
        let lease_ref = lease.lease_ref().to_string();
        let instance_ref = lease.instance_ref().to_string();

        // Simulate a completed run: emit resource_usage_receipt + release.
        finish_gce_lease(
            &config,
            &request,
            Some(lease),
            "completed",
            ReleaseReason::Manual,
        )
        .unwrap();

        let events = load_job_events(&config, &request.run_id, 0).unwrap();
        let kinds: Vec<&str> = events.iter().map(|e| e.type_.as_str()).collect();
        assert!(
            kinds.contains(&"cloud.gce.provisioned"),
            "expected provision event, got {kinds:?}"
        );
        assert!(
            kinds.contains(&"cloud.gce.resource_usage_receipt"),
            "expected resource usage receipt event, got {kinds:?}"
        );
        assert!(
            kinds.contains(&"cloud.gce.cleanup"),
            "expected cleanup event, got {kinds:?}"
        );

        // The receipt + cleanup events carry sha256 receipt refs and no secrets.
        let receipt_event = events
            .iter()
            .find(|e| e.type_ == "cloud.gce.resource_usage_receipt")
            .unwrap();
        assert!(receipt_event
            .receipt_refs
            .iter()
            .all(|r| r.starts_with("sha256:")));
        // The receipt event surfaces the metered VM-seconds and the
        // catalog/list-price-derived cost basis (cloud#92).
        let receipt_data = receipt_event.data_json.clone().unwrap();
        assert!(receipt_data.contains("\"vmSeconds\""));
        assert!(receipt_data.contains("\"costInputBasis\":\"cost_plus_10pct_gcp_catalog\""));
        let cleanup_event = events
            .iter()
            .find(|e| e.type_ == "cloud.gce.cleanup")
            .unwrap();
        let cleanup_data = cleanup_event.data_json.clone().unwrap();
        assert!(cleanup_data.contains("\"deletedVm\":true"));
        assert!(cleanup_data.contains(&lease_ref));
        assert!(cleanup_data.contains(&instance_ref));
        for event in &events {
            if let Some(data) = &event.data_json {
                let lower = data.to_ascii_lowercase();
                assert!(!lower.contains("access_token"));
                assert!(!lower.contains("bearer "));
                assert!(!lower.contains("projects/"));
            }
        }
    }

    #[test]
    fn gce_resource_usage_receipt_validates_as_refs_only() {
        let mut request = test_request("run_gce_receipt");
        request.lane = Some(ComputeLane::CloudGcp);
        let receipt = gce_resource_usage_receipt(
            &request,
            "workroom_run_gce_receipt",
            "gce-lease://cloud/session/abc",
            "gce-instance-ref://sha256/abc",
            "completed",
            120,
            12_345,
        );
        receipt.validate_contract().unwrap();
        assert_eq!(
            receipt.provider_lane,
            openagents_cloud_contract::ProviderLane::Gcp
        );
        // No per-token model cost is carried; cost lives in the infra compute_usage.
        assert!(receipt.model_usage[0].cost_microusd.is_none());
        assert_eq!(
            receipt.model_usage[0].count_source,
            openagents_cloud_contract::TokenCountSource::Unavailable
        );
        // The infra compute_usage carries the measured VM-seconds and a
        // catalog/list-price-derived cost-plus-10% billing input (cloud#92).
        let usage = receipt
            .compute_usage
            .as_ref()
            .expect("gce receipt should carry a compute_usage record");
        assert_eq!(usage.vm_seconds, 120);
        assert_eq!(
            usage.metering_source,
            openagents_cloud_contract::MeteringSource::NodeMeasured
        );
        assert_eq!(
            usage.cost_input_basis,
            openagents_cloud_contract::CostInputBasis::CostPlus10pctGcpCatalog
        );
        // 120 VM-sec × 5 micro-USD/VM-sec (cost-plus-10% over the catalog rate).
        assert_eq!(
            usage.cost_input_microusd,
            Some(120u128 * LaneCostModel::default().gce_micro_usd_per_vm_sec() as u128)
        );
    }

    #[test]
    fn gce_vm_seconds_measured_from_simulated_lease_wall_time() {
        // The fake provisioner has deterministic acquire/release timing: a lease
        // acquired at a fixed `acquire_at_ms` and released at a fixed later time
        // yields a deterministic measured VM-second count, exactly as
        // finish_gce_lease computes it (release_at − acquire_at, whole seconds).
        let request = CapacityRequest {
            run_id: "run_gce_walltime".to_string(),
            owner_ref: "owner://sha256/example".to_string(),
            gcp_project_ref: "gcp-project-ref://openagents/cloud-primary".to_string(),
            provisioner_identity_ref: "gce-provisioner://openagents/cloud".to_string(),
            caps: ComputeQuotaCaps::default(),
        };
        let (prov, _) = provisioner_for(ProvisionerKind::Fake);
        let acquire_at_ms: u128 = 1_000;
        let lease = GceLease::acquire(prov, &request, "workroom_run_gce_walltime", acquire_at_ms)
            .expect("fake lease acquires");
        assert_eq!(lease.provision_receipt.emitted_at_ms, acquire_at_ms);

        // Simulated release 137_250 ms after acquire -> floor(137.25) = 137 sec.
        let release_at_ms: u128 = acquire_at_ms + 137_250;
        let vm_seconds = u64::try_from(
            release_at_ms.saturating_sub(lease.provision_receipt.emitted_at_ms) / 1_000,
        )
        .unwrap();
        assert_eq!(vm_seconds, 137);

        let mut request_ctl = test_request("run_gce_walltime");
        request_ctl.lane = Some(ComputeLane::CloudGcp);
        let receipt = gce_resource_usage_receipt(
            &request_ctl,
            "workroom_run_gce_walltime",
            lease.lease_ref(),
            lease.instance_ref(),
            "completed",
            vm_seconds,
            release_at_ms,
        );
        receipt.validate_contract().unwrap();
        let usage = receipt.compute_usage.as_ref().unwrap();
        assert_eq!(usage.vm_seconds, 137);
        assert_eq!(
            usage.cost_input_basis,
            openagents_cloud_contract::CostInputBasis::CostPlus10pctGcpCatalog
        );
        assert_eq!(
            usage.cost_input_microusd,
            Some(137u128 * LaneCostModel::default().gce_micro_usd_per_vm_sec() as u128)
        );
    }

    #[test]
    fn gce_quota_caps_select_ttl_release_reason() {
        let config = test_config("gce-ttl");
        let mut request = test_request("run_gce_ttl");
        request.lane = Some(ComputeLane::CloudGcp);
        register_test_job(&config, &request);

        let lease = acquire_gce_lease_for_run(&config, &request)
            .unwrap()
            .expect("lease");
        // A time at/after the lease's own expiry forces the TTL-expired path.
        let expiry = lease.projection.expires_at_ms;
        assert!(lease.ttl_expired(expiry));
        assert!(lease.ttl_expired(expiry + 10_000));
        assert!(!lease.ttl_expired(expiry.saturating_sub(1)));
        // Idle deadline is the tighter inner bound.
        assert!(lease.projection.idle_deadline_at_ms <= expiry);
    }

    #[test]
    fn non_gce_lane_acquires_no_lease() {
        let config = test_config("gce-none");
        let request = test_request("run_local_lane"); // lane = None
        assert!(request.lane != Some(ComputeLane::CloudGcp));
        // The worker only provisions when lane is CloudGcp; confirm the guard.
        let lease = if request.lane == Some(ComputeLane::CloudGcp) {
            acquire_gce_lease_for_run(&config, &request).unwrap()
        } else {
            None
        };
        assert!(lease.is_none());
    }

    #[test]
    fn rejects_mismatched_github_write_grant() {
        let mut request = test_request("agent_run_123");
        request.github_write_connection_ref = Some("github-write_other".to_string());
        request.github_write_grant_ref = Some("github-write-grant_123".to_string());

        assert!(validate_github_write_grant(
            &test_github_write_grant(),
            &request,
            now_ms().unwrap()
        )
        .is_err());
    }

    #[test]
    fn selects_account_scoped_auth_cache_path() {
        let config = Config {
            auth_json_file: None,
            auth_json_root: Some(PathBuf::from("/var/lib/openagents/codex-auth")),
            bind: DEFAULT_BIND.to_string(),
            codex_bin: PathBuf::from(DEFAULT_CODEX_BIN),
            github_write_grant_resolver: None,
            local_auth_without_grant_resolver: false,
            provider_account_ref: None,
            opencode_bin: PathBuf::from(DEFAULT_OPENCODE_BIN),
            state_root: PathBuf::from(DEFAULT_STATE_ROOT),
            token: "token".to_string(),
            grant_resolver: None,
            event_ingest: None,
            workroomd_bin: PathBuf::from(DEFAULT_WORKROOMD_BIN),
            placement_gce_available: true,
            placement_shc_runner_id: SHC_FALLBACK_RUNNER_ID.to_string(),
            placement_cost_driven: true,
            placement_shc_pilot_expand: false,
            gce_provisioner_kind: ProvisionerKind::Fake,
            cloud_vm_provisioner_kind: cloud_vm::ProvisionerKind::Fake,
            gce_project_ref: "gcp-project-ref://openagents/cloud-primary".to_string(),
            gce_provisioner_identity_ref: "gce-provisioner://openagents/cloud".to_string(),
            queue: QueueConfig {
                enabled: false,
                lane: ComputeLane::CloudGcp,
                max_concurrency: 1,
                tick_ms: 2000,
            },
        };
        let request = ControlRequest {
            agent_runtime: None,
            auth_grant_ref: "codex-auth-grant_123".to_string(),
            github_write_connection_ref: None,
            github_write_grant_ref: None,
            github_work_order: None,
            goal: "test".to_string(),
            lane: None,
            owner_ref: None,
            provider_account_ref: "provider-account_abc123".to_string(),
            repository: None,
            repository_clone_url: None,
            repository_ref: None,
            required_artifacts: None,
            retention_mode: None,
            runner_id: "runner".to_string(),
            run_id: "run".to_string(),
            sandbox_mode: None,
            timeout_ms: None,
        };
        let resolved = ResolvedCodexAuthGrant {
            auth_material: None,
            grant: VortexResolvedGrant {
                expires_at: 2,
                grant_ref: request.auth_grant_ref.clone(),
                provider: "chatgpt_codex".to_string(),
                provider_account_ref: request.provider_account_ref.clone(),
                provider_secret_ref: "codex-auth://provider-account_abc123".to_string(),
                requested_action: None,
                status: "issued".to_string(),
            },
        };

        assert_eq!(
            codex_auth_cache_path(&config, &resolved, &request, &config.state_root).unwrap(),
            PathBuf::from("/var/lib/openagents/codex-auth/provider-account_abc123/auth.json")
        );
    }

    #[test]
    fn writes_resolver_auth_material_to_run_scoped_file() {
        let config = test_config("resolver-auth-material");
        let request = test_request("codex_run_material");
        let run_dir = config.state_root.join("run");
        fs::create_dir_all(&run_dir).unwrap();
        let resolved = ResolvedCodexAuthGrant {
            auth_material: Some(VortexAuthMaterial {
                auth_content_env: "OPENCODE_AUTH_CONTENT".to_string(),
                auth_content_json:
                    r#"{"openai":{"type":"oauth","access":"fresh-access","refresh":"fresh-refresh","expires":0}}"#
                        .to_string(),
            }),
            grant: VortexResolvedGrant {
                expires_at: now_ms().unwrap() + 60_000,
                grant_ref: request.auth_grant_ref.clone(),
                provider: "chatgpt_codex".to_string(),
                provider_account_ref: request.provider_account_ref.clone(),
                provider_secret_ref: "codex-auth://provider-account_abc123".to_string(),
                requested_action: None,
                status: "issued".to_string(),
            },
        };

        let path = codex_auth_cache_path(&config, &resolved, &request, &run_dir).unwrap();

        assert_eq!(path, run_dir.join("codex-auth-material.json"));
        assert!(fs::read_to_string(path).unwrap().contains("fresh-refresh"));
    }

    #[test]
    fn refuses_unsafe_account_scoped_auth_cache_path() {
        assert!(validate_provider_account_path_component("provider-account_ok").is_ok());
        assert!(validate_provider_account_path_component("../provider-account_bad").is_err());
        assert!(validate_provider_account_path_component("provider/account_bad").is_err());
        assert!(validate_provider_account_path_component(".provider-account_bad").is_err());
    }

    #[test]
    fn rejects_api_key_auth_cache_material() {
        assert!(contains_api_key_auth_material(
            &serde_json::json!({"OPENAI_API_KEY":"sk-abcdefghijklmnopqrstuvwxyz"})
        ));
        assert!(contains_api_key_auth_material(
            &serde_json::json!({"auth_mode":"apiKey"})
        ));
        assert!(!contains_api_key_auth_material(
            &serde_json::json!({"auth_mode":"chatgpt","account":{"planType":"pro"},"supportedLoginModes":["apiKey","chatgpt"]})
        ));
    }

    #[test]
    fn filters_initial_events_for_vortex() {
        let events = vec![
            serde_json::json!({"event_kind":"queued","message":"queued"}),
            serde_json::json!({"event_kind":"started","message":"started"}),
            serde_json::json!({"event_kind":"artifact","message":"artifact captured","artifact_ref":"sha256:abc","receipt_ref":"sha256:def"}),
        ];
        let normalized = normalized_events(events, false);
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].kind, "artifact");
        assert_eq!(normalized[0].artifact_refs, vec!["sha256:abc"]);
    }

    #[test]
    fn normalizes_openagents_runner_events_for_vortex() {
        let events = vec![serde_json::json!({
            "type": "shell.command.started",
            "summary": "Shell command started.",
            "detail_excerpt": "printf hello",
            "raw_payload_json": "{\"cmd\":\"printf hello\",\"type\":\"exec_command_begin\"}",
            "artifact_refs": [],
            "receipt_refs": []
        })];
        let normalized = normalized_runner_events(events);
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].kind, "shell.command.started");
        assert_eq!(normalized[0].summary, "Shell command started.");
        assert_eq!(
            normalized[0].data_json.as_deref(),
            Some("{\"cmd\":\"printf hello\",\"type\":\"exec_command_begin\"}")
        );
    }

    #[test]
    fn vortex_ingest_payload_preserves_event_data_and_tool_calls() {
        let event = JobEvent {
            artifact_refs: Vec::new(),
            created_at_ms: 1_780_000_000_000,
            data_json: None,
            detail: Some(r#"{"name":"apply_patch","result":"ok"}"#.to_string()),
            digest: Some("sha256:tool".to_string()),
            kind: "tool.call.completed".to_string(),
            receipt_refs: Vec::new(),
            redacted: false,
            sequence: 7,
            source: "codex".to_string(),
            summary: "Tool call completed: apply_patch.".to_string(),
            type_: "tool.call.completed".to_string(),
        };

        let vortex_event = vortex_event_from_job_event(&event);
        let data_json = vortex_event
            .pointer("/dataJson")
            .and_then(Value::as_str)
            .expect("event data json");
        assert!(data_json.contains("\"sequence\":7"));
        assert!(data_json.contains("\"kind\":\"tool.call.completed\""));

        let tool_call = vortex_tool_call_from_job_event(&event).expect("tool call");
        assert_eq!(
            tool_call.pointer("/toolName").and_then(Value::as_str),
            Some("apply_patch")
        );
        assert_eq!(
            tool_call.pointer("/status").and_then(Value::as_str),
            Some("completed")
        );
        assert!(tool_call
            .pointer("/outputJson")
            .and_then(Value::as_str)
            .is_some_and(|payload| payload.contains("apply_patch")));
    }

    #[test]
    fn training_assignment_renders_bounded_codex_request() {
        let assignment = training_assignment_fixture();
        assignment.validate_contract(now_ms().unwrap()).unwrap();

        let request = control_request_from_training_assignment(&assignment);

        assert_eq!(request.run_id, "taskrun.db-wal-recovery.001");
        assert_eq!(request.provider_account_ref, "provider-account_abc123");
        assert_eq!(request.sandbox_mode.as_deref(), Some("danger_full_access"));
        assert_eq!(
            request.required_artifacts.as_ref().unwrap(),
            &vec![
                "result.md".to_string(),
                "benchmark-result.json".to_string(),
                "artifact-manifest.json".to_string(),
                "proof-bundle.json".to_string(),
            ]
        );
        assert!(request.goal.contains("terminal-bench/db-wal-recovery"));
        assert!(request
            .goal
            .contains("Copy the SQLite DB, WAL, and SHM files as a matched set"));
        assert!(request.goal.contains("Open only the copied DB"));
        assert!(request
            .goal
            .contains("CODEX_AUTH_JSON_PATH=<session-codex-auth-file>"));
        assert!(!request.goal.contains("sk-"));
    }

    #[test]
    fn artanis_bootstrap_renders_bounded_codex_request() {
        let assignment = artanis_bootstrap_fixture();
        assignment.validate_contract(now_ms().unwrap()).unwrap();

        let request = control_request_from_artanis_bootstrap_assignment(&assignment);

        assert_eq!(request.run_id, "artanis.bootstrap.pylon-launch.001");
        assert_eq!(request.provider_account_ref, "provider-account_admin_codex");
        assert_eq!(request.sandbox_mode.as_deref(), Some("danger_full_access"));
        assert!(request
            .repository
            .as_deref()
            .unwrap()
            .contains("OpenAgentsInc/cloud"));
        assert!(request.goal.contains("Artanis to Pylon launch bootstrap"));
        assert!(request.goal.contains("qwen_legal_adapter_training"));
        assert!(request.goal.contains("continual-learning loop"));
        assert!(request.goal.contains("work-order drafts"));
        assert!(request.goal.contains("walletAuthority: false"));
        assert!(request
            .goal
            .contains("artanis_assignment_id: artanis.assignment.pylon-launch.001"));
        assert!(request
            .goal
            .contains("settlement_intent_id: settlement.intent.pylon-launch.001"));
        assert!(request.goal.contains("oa:artanis_assignment_id"));
        assert!(!request.goal.contains("auth.json"));
        assert!(!request.goal.contains("private_topology"));
        assert!(!request.goal.contains("sk-"));
    }

    #[test]
    fn persists_job_registry_and_event_cursor() {
        let config = test_config("registry");
        let request = test_request("codex_run_registry");
        let job = test_job(&request);
        save_job_record(&config, &job).unwrap();

        append_job_event(
            &config,
            &request.run_id,
            JobEventInput {
                artifact_refs: Vec::new(),
                data_json: None,
                detail: Some("queued".to_string()),
                digest: None,
                kind: "queued".to_string(),
                receipt_refs: Vec::new(),
                redacted: false,
                source: "control".to_string(),
                summary: "queued".to_string(),
                type_: "cloud.run.queued".to_string(),
            },
        )
        .unwrap();
        append_job_event(
            &config,
            &request.run_id,
            JobEventInput {
                artifact_refs: vec!["local://result.md".to_string()],
                data_json: None,
                detail: Some("artifact".to_string()),
                digest: None,
                kind: "artifact".to_string(),
                receipt_refs: Vec::new(),
                redacted: false,
                source: "runner".to_string(),
                summary: "artifact".to_string(),
                type_: "runner.artifact".to_string(),
            },
        )
        .unwrap();

        let reloaded = load_job_record(&config, &request.run_id).unwrap();
        assert_eq!(reloaded.last_sequence, 2);
        assert_eq!(
            load_job_events(&config, &request.run_id, 0).unwrap().len(),
            2
        );
        let after_first = load_job_events(&config, &request.run_id, 1).unwrap();
        assert_eq!(after_first.len(), 1);
        assert_eq!(after_first[0].kind, "artifact");
    }

    #[test]
    fn cancel_marks_job_and_persists_recovery_marker() {
        let config = test_config("cancel");
        let request = test_request("codex_run_cancel");
        save_job_record(&config, &test_job(&request)).unwrap();

        request_job_cancel(&config, &request.run_id, "operator canceled").unwrap();

        let job = load_job_record(&config, &request.run_id).unwrap();
        assert_eq!(job.status, "canceled");
        assert!(job.cancel_requested);
        assert!(is_cancel_requested(&config, &request.run_id));
        let events = load_job_events(&config, &request.run_id, 0).unwrap();
        assert_eq!(events[0].type_, "cloud.run.cancel_requested");
    }

    #[test]
    fn async_start_is_idempotent_for_existing_job() {
        let config = test_config("duplicate");
        let request = test_request("codex_run_duplicate");
        let job = test_job(&request);
        save_job_record(&config, &job).unwrap();
        append_job_event(
            &config,
            &request.run_id,
            JobEventInput {
                artifact_refs: Vec::new(),
                data_json: None,
                detail: None,
                digest: None,
                kind: "queued".to_string(),
                receipt_refs: Vec::new(),
                redacted: false,
                source: "control".to_string(),
                summary: "queued".to_string(),
                type_: "cloud.run.queued".to_string(),
            },
        )
        .unwrap();

        let response = start_codex_run_async(&config, request).unwrap();

        assert_eq!(response.status, "queued");
        assert_eq!(
            load_job_record(&config, "codex_run_duplicate")
                .unwrap()
                .last_sequence,
            1
        );
    }

    #[test]
    fn follow_up_turn_records_waiting_state_when_run_is_active() {
        let config = test_config("follow-up-active");
        let request = test_request("codex_run_follow_up");
        let mut job = test_job(&request);
        job.status = "running".to_string();
        save_job_record(&config, &job).unwrap();

        queue_follow_up_turn(
            &config,
            &request.run_id,
            "continue with tighter tests",
            Some("codex-auth-grant_next".to_string()),
        )
        .unwrap();

        let updated = load_job_record(&config, &request.run_id).unwrap();
        assert_eq!(updated.status, "waiting_for_input");
        assert_eq!(updated.request.auth_grant_ref, request.auth_grant_ref);
    }

    #[cfg(unix)]
    #[test]
    fn async_start_runs_fake_workroomd_to_completion() {
        use std::os::unix::fs::PermissionsExt;

        let config = test_config("fake-workroomd");
        let provider_dir = config
            .auth_json_root
            .as_ref()
            .unwrap()
            .join("provider-account_abc123");
        fs::create_dir_all(&provider_dir).unwrap();
        fs::write(
            provider_dir.join("auth.json"),
            r#"{"auth_mode":"chatgpt","account":{"email":"test@example.com"}}"#,
        )
        .unwrap();
        let fake_workroomd = config.state_root.join("fake-workroomd.sh");
        fs::write(
            &fake_workroomd,
            r#"#!/bin/sh
case "$*" in
  *"codex auth materialize"*) echo '{"status":"materialized"}'; exit 0;;
  *"codex auth status"*) echo '{"status":"healthy"}'; exit 0;;
  *"codex run"*) case "$*" in *"--agent-runtime opencode_codex"*"--opencode-bin"*) ;; *) echo 'missing opencode_codex runtime flags' >&2; exit 2;; esac; sleep 0.05; echo '{"state":{"status":"completed","events":[{"event_kind":"artifact","message":"result ready","artifact_ref":"local://result.md","receipt_ref":"local://closeout.json"},{"event_kind":"completed","message":"done"}]},"runner_events":[{"type":"shell.command.started","summary":"Shell command started.","detail_excerpt":"printf hello","artifact_refs":[],"receipt_refs":[]},{"type":"turn.completed","summary":"Codex turn completed with token usage.","raw_payload_json":"{\"type\":\"turn.completed\",\"usage\":{\"cached_input_tokens\":12,\"input_tokens\":42,\"output_tokens\":5,\"reasoning_output_tokens\":0}}","artifact_refs":[],"receipt_refs":[]}]}'; exit 0;;
esac
echo '{"status":"ok"}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_workroomd).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_workroomd, permissions).unwrap();
        let mut config = config;
        config.workroomd_bin = fake_workroomd;
        let request = test_request("codex_run_fake");

        let response = start_codex_run_async(&config, request.clone()).unwrap();
        assert_eq!(response.status, "queued");

        let mut final_status = String::new();
        for _ in 0..200 {
            final_status = load_job_record(&config, &request.run_id).unwrap().status;
            if final_status == "completed" || final_status == "failed" {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        assert_eq!(final_status, "completed");
        let events = load_job_events(&config, &request.run_id, 0).unwrap();
        assert!(events.iter().any(|event| event.kind == "artifact"));
        assert!(events
            .iter()
            .any(|event| event.type_ == "shell.command.started"));
        assert!(events.iter().any(|event| event.type_ == "turn.completed"));
        assert!(events.iter().any(|event| event
            .data_json
            .as_deref()
            .is_some_and(|payload| payload.contains("\"usage\""))));
        assert!(events
            .iter()
            .any(|event| event.type_ == "cloud.run.completed"));
    }

    #[cfg(unix)]
    #[test]
    fn start_run_can_force_raw_codex_runtime() {
        use std::os::unix::fs::PermissionsExt;

        let config = test_config("raw-codex-runtime");
        let provider_dir = config
            .auth_json_root
            .as_ref()
            .unwrap()
            .join("provider-account_abc123");
        fs::create_dir_all(&provider_dir).unwrap();
        fs::write(
            provider_dir.join("auth.json"),
            r#"{"auth_mode":"chatgpt","account":{"email":"test@example.com"}}"#,
        )
        .unwrap();
        let fake_workroomd = config.state_root.join("fake-raw-codex-workroomd.sh");
        fs::write(
            &fake_workroomd,
            r#"#!/bin/sh
case "$*" in
  *"codex auth materialize"*) echo '{"status":"materialized"}'; exit 0;;
  *"codex auth status"*) echo '{"status":"healthy"}'; exit 0;;
  *"codex run"*) case "$*" in *"--agent-runtime codex"*) ;; *) echo 'missing raw codex runtime flag' >&2; exit 2;; esac; echo '{"state":{"status":"completed","events":[{"event_kind":"completed","message":"done"}]},"runner_events":[{"type":"turn.completed","summary":"Codex turn completed with token usage.","raw_payload_json":"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":4,\"output_tokens\":2}}","artifact_refs":[],"receipt_refs":[]}]}'; exit 0;;
esac
echo '{"status":"ok"}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_workroomd).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_workroomd, permissions).unwrap();
        let mut config = config;
        config.workroomd_bin = fake_workroomd;
        let mut request = test_request("codex_run_raw");
        request.agent_runtime = Some(AgentRuntime::Codex);

        let response = start_codex_run(&config, request).unwrap();

        assert_eq!(response.status, "completed");
        assert!(response
            .events
            .iter()
            .any(|event| event.kind == "completed"));
    }

    #[cfg(unix)]
    #[test]
    fn async_worker_mirrors_runner_events_before_completion() {
        use std::os::unix::fs::PermissionsExt;

        let config = test_config("fake-live-workroomd");
        let provider_dir = config
            .auth_json_root
            .as_ref()
            .unwrap()
            .join("provider-account_abc123");
        fs::create_dir_all(&provider_dir).unwrap();
        fs::write(
            provider_dir.join("auth.json"),
            r#"{"auth_mode":"chatgpt","account":{"email":"test@example.com"}}"#,
        )
        .unwrap();
        let fake_workroomd = config.state_root.join("fake-live-workroomd.sh");
        fs::write(
            &fake_workroomd,
            r#"#!/bin/sh
state_dir=""
previous=""
for arg in "$@"; do
  if [ "$previous" = "--state-dir" ]; then state_dir="$arg"; fi
  previous="$arg"
done
case "$*" in
  *"codex auth materialize"*) echo '{"status":"materialized"}'; exit 0;;
  *"codex auth status"*) echo '{"status":"healthy"}'; exit 0;;
  *"codex run"*)
    mkdir -p "$state_dir"
    echo '{"schema_version":"openagents.runner_event.v1","external_event_id":"runner.codex_run_live.1","sequence":1,"event_type":"shell.command.started","source":"codex","summary":"Live shell command started.","detail_excerpt":"printf live","raw_payload_digest":null,"artifact_refs":[],"receipt_refs":[],"emitted_at_ms":1}' > "$state_dir/openagents-runner-events.jsonl"
    sleep 1
    echo '{"state":{"status":"completed","events":[{"event_kind":"completed","message":"done"}]},"runner_events":[{"type":"shell.command.started","summary":"Live shell command started.","detail_excerpt":"printf live","artifact_refs":[],"receipt_refs":[]}]}'
    exit 0
    ;;
esac
echo '{"status":"ok"}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_workroomd).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_workroomd, permissions).unwrap();
        let mut config = config;
        config.workroomd_bin = fake_workroomd;
        let request = test_request("codex_run_live");

        let response = start_codex_run_async(&config, request.clone()).unwrap();
        assert_eq!(response.status, "queued");

        let mut saw_live_event_before_terminal = false;
        for _ in 0..40 {
            let events = load_job_events(&config, &request.run_id, 0).unwrap();
            let has_live = events
                .iter()
                .any(|event| event.type_ == "shell.command.started");
            let has_terminal = events
                .iter()
                .any(|event| event.type_ == "cloud.run.completed");
            if has_live && !has_terminal {
                saw_live_event_before_terminal = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert!(saw_live_event_before_terminal);

        let mut final_status = String::new();
        for _ in 0..80 {
            final_status = load_job_record(&config, &request.run_id).unwrap().status;
            if final_status == "completed" || final_status == "failed" {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert_eq!(final_status, "completed");
    }

    #[cfg(unix)]
    #[test]
    fn training_assignment_runs_fake_workroomd_to_completion() {
        use std::os::unix::fs::PermissionsExt;

        let config = test_config("fake-training-workroomd");
        let provider_dir = config
            .auth_json_root
            .as_ref()
            .unwrap()
            .join("provider-account_abc123");
        fs::create_dir_all(&provider_dir).unwrap();
        fs::write(
            provider_dir.join("auth.json"),
            r#"{"auth_mode":"chatgpt","account":{"email":"test@example.com"}}"#,
        )
        .unwrap();
        let fake_workroomd = config.state_root.join("fake-training-workroomd.sh");
        fs::write(
            &fake_workroomd,
            r#"#!/bin/sh
case "$*" in
  *"codex auth materialize"*) echo '{"status":"materialized"}'; exit 0;;
  *"codex auth status"*) echo '{"status":"healthy"}'; exit 0;;
  *"codex run"*) sleep 0.05; echo '{"state":{"status":"completed","events":[{"event_kind":"artifact","message":"benchmark result ready","artifact_ref":"local://benchmark-result.json","receipt_ref":"local://proof-bundle.json"},{"event_kind":"completed","message":"done"}]},"runner_events":[{"type":"artifact.created","summary":"Benchmark artifact created.","artifact_refs":["local://benchmark-result.json"],"receipt_refs":[]},{"type":"turn.completed","summary":"Codex turn completed with token usage.","raw_payload_json":"{\"type\":\"turn.completed\",\"usage\":{\"cached_input_tokens\":12,\"input_tokens\":42,\"output_tokens\":5,\"reasoning_output_tokens\":0}}","artifact_refs":[],"receipt_refs":[]}]}'; exit 0;;
esac
echo '{"status":"ok"}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_workroomd).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_workroomd, permissions).unwrap();
        let mut config = config;
        config.workroomd_bin = fake_workroomd;
        let assignment = training_assignment_fixture();

        let response = start_training_run_async(&config, assignment.clone()).unwrap();
        assert_eq!(response.status, "queued");

        let mut final_status = String::new();
        for _ in 0..200 {
            final_status = load_job_record(&config, &assignment.task_run_id)
                .unwrap()
                .status;
            if final_status == "completed" || final_status == "failed" {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        assert_eq!(final_status, "completed");
        assert!(job_dir(&config, &assignment.task_run_id)
            .join("training-assignment.json")
            .exists());
        let job = load_job_record(&config, &assignment.task_run_id).unwrap();
        assert_eq!(
            job.request.required_artifacts.unwrap(),
            assignment.artifacts.required_artifacts
        );
        let events = load_job_events(&config, &assignment.task_run_id, 0).unwrap();
        assert!(events
            .iter()
            .any(|event| event.type_ == "training.assignment.validated"));
        assert!(events
            .iter()
            .any(|event| event.type_ == "benchmark.package.validated"));
        assert!(events
            .iter()
            .any(|event| event.type_ == "signature.context.loaded"));
        assert!(events.iter().any(|event| event.type_ == "artifact.created"));
        assert!(events.iter().any(|event| event.type_ == "turn.completed"));
    }

    #[cfg(unix)]
    #[test]
    fn artanis_bootstrap_runs_fake_workroomd_to_completion() {
        use std::os::unix::fs::PermissionsExt;

        let config = test_config("fake-artanis-workroomd");
        let provider_dir = config
            .auth_json_root
            .as_ref()
            .unwrap()
            .join("provider-account_admin_codex");
        fs::create_dir_all(&provider_dir).unwrap();
        fs::write(
            provider_dir.join("auth.json"),
            r#"{"auth_mode":"chatgpt","account":{"email":"admin@example.com"}}"#,
        )
        .unwrap();
        let fake_workroomd = config.state_root.join("fake-artanis-workroomd.sh");
        fs::write(
            &fake_workroomd,
            r#"#!/bin/sh
case "$*" in
  *"codex auth materialize"*) echo '{"status":"materialized"}'; exit 0;;
  *"codex auth status"*) echo '{"status":"healthy"}'; exit 0;;
  *"codex run"*) sleep 0.05; echo '{"state":{"status":"completed","events":[{"event_kind":"artifact","message":"Artanis bootstrap plan ready","artifact_ref":"local://pylon-launch-plan.json","receipt_ref":"local://proof-bundle.json"},{"event_kind":"completed","message":"done"}]},"runner_events":[{"type":"artifact.created","summary":"Pylon launch artifact created.","artifact_refs":["local://pylon-launch-plan.json"],"receipt_refs":[]},{"type":"artifact.created","summary":"Continual learning artifact created.","artifact_refs":["local://continual-learning-plan.json"],"receipt_refs":[]},{"type":"turn.completed","summary":"Codex turn completed with token usage.","raw_payload_json":"{\"type\":\"turn.completed\",\"usage\":{\"cached_input_tokens\":12,\"input_tokens\":42,\"output_tokens\":5,\"reasoning_output_tokens\":0}}","artifact_refs":[],"receipt_refs":[]}]}'; exit 0;;
esac
echo '{"status":"ok"}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_workroomd).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_workroomd, permissions).unwrap();
        let mut config = config;
        config.workroomd_bin = fake_workroomd;
        let assignment = artanis_bootstrap_fixture();

        let response = start_artanis_bootstrap_async(&config, assignment.clone()).unwrap();
        assert_eq!(response.status, "queued");

        let mut final_status = String::new();
        for _ in 0..200 {
            final_status = load_job_record(&config, &assignment.bootstrap_run_id)
                .unwrap()
                .status;
            if final_status == "completed" || final_status == "failed" {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        assert_eq!(final_status, "completed");
        assert!(job_dir(&config, &assignment.bootstrap_run_id)
            .join("artanis-bootstrap-assignment.json")
            .exists());
        let job = load_job_record(&config, &assignment.bootstrap_run_id).unwrap();
        assert_eq!(
            job.request.required_artifacts.unwrap(),
            assignment.required_artifacts
        );
        let events = load_job_events(&config, &assignment.bootstrap_run_id, 0).unwrap();
        assert!(events
            .iter()
            .any(|event| event.type_ == "artanis.bootstrap.validated"));
        assert!(events
            .iter()
            .any(|event| event.type_ == "artanis.capability_context.loaded"));
        assert!(events
            .iter()
            .any(|event| event.type_ == "artanis.artifact_policy.attached"));
        assert!(events
            .iter()
            .any(|event| event.type_ == "artanis.settlement_intent.attached"
                && event
                    .data_json
                    .as_deref()
                    .is_some_and(|data| data.contains("artanis.assignment.pylon-launch.001")
                        && data.contains("oa:settlement_intent_id"))));
        assert!(events.iter().any(|event| event.type_ == "artifact.created"));
        assert!(events.iter().any(|event| event.type_ == "turn.completed"));
    }

    // Serializes tests that read/write the global QUEUE_IN_FLIGHT counter.
    static QUEUE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn enqueue_test_request(config: &Config, run_id: &str) -> ControlRequest {
        let mut request = test_request(run_id);
        request.lane = None;
        enqueue_codex_run(config, request.clone()).unwrap();
        request
    }

    #[test]
    fn enqueue_persists_pending_marker_and_queued_status() {
        let config = test_config("queue-enqueue");
        let request = enqueue_test_request(&config, "queue_run_1");

        // Durable marker + job record persisted; no worker spawned.
        assert!(queue_pending_path(&config, &request.run_id).exists());
        let job = load_job_record(&config, &request.run_id).unwrap();
        assert_eq!(job.status, "queued");

        let events = load_job_events(&config, &request.run_id, 0).unwrap();
        assert!(events.iter().any(|e| e.type_ == "cloud.run.enqueued"));

        // pending_queue_run_ids surfaces it.
        assert_eq!(
            pending_queue_run_ids(&config),
            vec!["queue_run_1".to_string()]
        );
    }

    #[test]
    fn pending_queue_run_ids_returns_oldest_first() {
        let config = test_config("queue-order");
        // created_at_ms uses millisecond clock; force distinct timestamps by
        // writing records directly with controlled created_at values.
        for (run_id, created_at) in [("b", 200u128), ("a", 100u128), ("c", 300u128)] {
            let job = JobRecord {
                cancel_requested: false,
                created_at_ms: created_at,
                external_run_id: external_run_id("runner", run_id),
                last_sequence: 0,
                request: {
                    let mut r = test_request(run_id);
                    r.lane = None;
                    r
                },
                run_id: run_id.to_string(),
                runner_id: "runner".to_string(),
                status: "queued".to_string(),
                updated_at_ms: created_at,
            };
            fs::create_dir_all(job_dir(&config, run_id)).unwrap();
            save_job_record(&config, &job).unwrap();
            fs::write(queue_pending_path(&config, run_id), "pending\n").unwrap();
        }
        assert_eq!(
            pending_queue_run_ids(&config),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn claim_applies_default_lane_removes_marker_and_is_idempotent() {
        let config = test_config("queue-claim");
        let request = enqueue_test_request(&config, "queue_claim_1");
        assert!(request.lane.is_none());

        let claimed = claim_queued_job(&config, &request.run_id)
            .unwrap()
            .expect("first claim should succeed");
        // Default queue lane (cloud-gcp) applied because the caller pinned none.
        assert_eq!(claimed.lane, Some(ComputeLane::CloudGcp));
        // Durable marker removed so a restart cannot double-dispatch.
        assert!(!queue_pending_path(&config, &request.run_id).exists());
        let events = load_job_events(&config, &request.run_id, 0).unwrap();
        assert!(events.iter().any(|e| e.type_ == "cloud.run.dequeued"));

        // Second claim is a no-op (marker gone).
        assert!(claim_queued_job(&config, &request.run_id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn claim_preserves_caller_pinned_lane() {
        let config = test_config("queue-claim-pinned");
        let mut request = test_request("queue_claim_pinned");
        request.lane = Some(ComputeLane::CloudShc);
        enqueue_codex_run(&config, request.clone()).unwrap();

        let claimed = claim_queued_job(&config, &request.run_id)
            .unwrap()
            .expect("claim");
        assert_eq!(claimed.lane, Some(ComputeLane::CloudShc));
    }

    #[test]
    fn drain_dispatches_nothing_when_budget_is_full() {
        let _guard = QUEUE_TEST_LOCK.lock().unwrap();
        let mut config = test_config("queue-budget-full");
        config.queue.max_concurrency = 2;
        for run_id in ["q1", "q2", "q3"] {
            enqueue_test_request(&config, run_id);
        }
        assert_eq!(pending_queue_run_ids(&config).len(), 3);

        // Pre-set the in-flight counter to the budget: no job is dispatched and
        // all durable markers remain for the next pass. Deterministic: no real
        // worker is spawned on this path.
        QUEUE_IN_FLIGHT.store(config.queue.max_concurrency, Ordering::SeqCst);
        let dispatched = drain_queue_once(&config).unwrap();
        assert_eq!(dispatched, 0);
        assert_eq!(pending_queue_run_ids(&config).len(), 3);

        QUEUE_IN_FLIGHT.store(0, Ordering::SeqCst);
    }

    #[test]
    fn drain_claims_up_to_concurrency_budget() {
        let _guard = QUEUE_TEST_LOCK.lock().unwrap();
        // Bound the claimable count without spawning real workers by claiming
        // through claim_queued_job directly while honoring the budget rule.
        let mut config = test_config("queue-budget-claim");
        config.queue.max_concurrency = 2;
        for run_id in ["c1", "c2", "c3"] {
            enqueue_test_request(&config, run_id);
        }

        // Emulate one drain pass at the claim layer (no spawned workers): claim
        // up to the budget, treating each claim as one in-flight unit.
        QUEUE_IN_FLIGHT.store(0, Ordering::SeqCst);
        let mut claimed = 0usize;
        for run_id in pending_queue_run_ids(&config) {
            if QUEUE_IN_FLIGHT.load(Ordering::SeqCst) >= config.queue.max_concurrency {
                break;
            }
            if claim_queued_job(&config, &run_id).unwrap().is_some() {
                QUEUE_IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
                claimed += 1;
            }
        }
        assert_eq!(claimed, 2);
        assert_eq!(pending_queue_run_ids(&config).len(), 1);

        QUEUE_IN_FLIGHT.store(0, Ordering::SeqCst);
    }

    #[test]
    fn pending_markers_survive_for_restart_resume() {
        // A new Config pointed at the same state root sees the persisted pending
        // job after a simulated restart (no in-process worker state needed).
        let config = test_config("queue-restart");
        enqueue_test_request(&config, "queue_restart_1");

        let restarted = Config {
            state_root: config.state_root.clone(),
            ..test_config("queue-restart-unused")
        };
        assert_eq!(
            pending_queue_run_ids(&restarted),
            vec!["queue_restart_1".to_string()]
        );
        // Claim drains it deterministically (no worker spawn on this path).
        assert!(claim_queued_job(&restarted, "queue_restart_1")
            .unwrap()
            .is_some());
        assert!(pending_queue_run_ids(&restarted).is_empty());
    }

    fn test_config(label: &str) -> Config {
        let state_root = test_temp_dir(label);
        let auth_json_root = state_root.join("auth");
        fs::create_dir_all(&auth_json_root).unwrap();
        Config {
            auth_json_file: None,
            auth_json_root: Some(auth_json_root),
            bind: DEFAULT_BIND.to_string(),
            codex_bin: PathBuf::from("/usr/bin/false"),
            github_write_grant_resolver: None,
            local_auth_without_grant_resolver: true,
            provider_account_ref: None,
            opencode_bin: PathBuf::from(DEFAULT_OPENCODE_BIN),
            state_root,
            token: "token".to_string(),
            grant_resolver: None,
            event_ingest: None,
            workroomd_bin: PathBuf::from(DEFAULT_WORKROOMD_BIN),
            placement_gce_available: true,
            placement_shc_runner_id: SHC_FALLBACK_RUNNER_ID.to_string(),
            placement_cost_driven: true,
            placement_shc_pilot_expand: false,
            gce_provisioner_kind: ProvisionerKind::Fake,
            cloud_vm_provisioner_kind: cloud_vm::ProvisionerKind::Fake,
            gce_project_ref: "gcp-project-ref://openagents/cloud-primary".to_string(),
            gce_provisioner_identity_ref: "gce-provisioner://openagents/cloud".to_string(),
            queue: QueueConfig {
                enabled: false,
                lane: ComputeLane::CloudGcp,
                max_concurrency: 1,
                tick_ms: 2000,
            },
        }
    }

    fn test_temp_dir(label: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "oa-codex-control-{label}-{}-{}",
            std::process::id(),
            now_ms().unwrap()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_request(run_id: &str) -> ControlRequest {
        ControlRequest {
            agent_runtime: None,
            auth_grant_ref: "codex-auth-grant_123".to_string(),
            github_write_connection_ref: None,
            github_write_grant_ref: None,
            github_work_order: None,
            goal: "write result".to_string(),
            lane: None,
            owner_ref: None,
            provider_account_ref: "provider-account_abc123".to_string(),
            repository: Some("OpenAgentsInc/vortex".to_string()),
            repository_clone_url: None,
            repository_ref: Some("main".to_string()),
            required_artifacts: None,
            retention_mode: None,
            runner_id: "oa-shc-katy-01".to_string(),
            run_id: run_id.to_string(),
            sandbox_mode: Some("danger_full_access".to_string()),
            timeout_ms: Some(60_000),
        }
    }

    fn test_github_work_order() -> GitHubWorkOrder {
        GitHubWorkOrder {
            provider: "github".to_string(),
            repository: GitHubRepositoryRef {
                provider: "github".to_string(),
                owner: "OpenAgentsInc".to_string(),
                repo: "autopilot".to_string(),
                ref_name: "main".to_string(),
            },
            base_ref: "main".to_string(),
            branch_name: "openagents/autopilot-123".to_string(),
            commit_message: "Address test issue".to_string(),
            issue_comment: Some("Implemented by OpenAgents Autopilot.".to_string()),
            issue_number: Some(123),
            issue_url: Some("https://github.com/OpenAgentsInc/autopilot/issues/123".to_string()),
            pull_request_body: Some("Autopilot result for issue #123.".to_string()),
            pull_request_title: Some("Address #123".to_string()),
            writeback: GitHubWritebackPlan {
                comment_on_issue: true,
                open_pull_request: true,
                push_branch: true,
            },
        }
    }

    fn test_github_write_grant() -> GitHubResolvedWriteGrant {
        GitHubResolvedWriteGrant {
            connection_ref: "github-write_123".to_string(),
            credential: GitHubResolvedCredential {
                access_token: "gho_example_token_for_test".to_string(),
                provider: "github".to_string(),
                scopes: vec!["repo".to_string(), "workflow".to_string()],
                token_type: "oauth".to_string(),
            },
            expires_at: now_ms().unwrap() + 60_000,
            github_login: "AtlantisPleb".to_string(),
            grant_ref: "github-write-grant_123".to_string(),
            materialization: GitHubWriteMaterialization {
                auth_ref: "github-write://github-write_123".to_string(),
                git_credential_env: "GITHUB_TOKEN".to_string(),
                provider: "github".to_string(),
                remote_url_mode: "https_token".to_string(),
                scrub_after_closeout: true,
            },
            requested_action: Some("autopilot_mission".to_string()),
            runner_session_id: Some("agent_run_123".to_string()),
            status: "issued".to_string(),
        }
    }

    fn test_job(request: &ControlRequest) -> JobRecord {
        let now = now_ms().unwrap();
        JobRecord {
            cancel_requested: false,
            created_at_ms: now,
            external_run_id: external_run_id(&request.runner_id, &request.run_id),
            last_sequence: 0,
            request: request.clone(),
            run_id: request.run_id.clone(),
            runner_id: request.runner_id.clone(),
            status: "queued".to_string(),
            updated_at_ms: now,
        }
    }

    fn training_assignment_fixture() -> TrainingRunAssignment {
        let mut assignment: TrainingRunAssignment = serde_json::from_str(include_str!(
            "../../../fixtures/cloud/training_run_assignment_v1/terminal-bench-retained.json"
        ))
        .unwrap();
        assignment.created_at_ms = now_ms().unwrap();
        assignment
    }

    fn artanis_bootstrap_fixture() -> ArtanisBootstrapAssignment {
        let mut assignment: ArtanisBootstrapAssignment = serde_json::from_str(include_str!(
            "../../../fixtures/cloud/artanis_bootstrap_assignment_v1/pylon-launch-bootstrap.json"
        ))
        .unwrap();
        assignment.created_at_ms = now_ms().unwrap();
        assignment
    }
}
