use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use openagents_cloud_contract::{
    CodexAuthDecision, CodexAuthGrant, CodexAuthReceipt, CodexAuthReceiptKind, CodexSandboxMode,
    CodexWorkroomAssignment, CodexWorkroomDecision, CodexWorkroomEvent, CodexWorkroomEventKind,
    ModelUsageRecord, ProviderLane, ResourceHostSnapshot, ResourceUsageReceipt, RunResourceUsage,
    TokenCountSource, VirtualizationFacts, WorkroomSnapshot, CODEX_AUTH_RECEIPT_VERSION,
    CODEX_WORKROOM_EVENT_VERSION, RESOURCE_USAGE_RECEIPT_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sysinfo::{Disks, System};

const METADATA_SCHEMA_VERSION: &str = "openagents.oa_workroomd.metadata.v1";
const METADATA_ACCESS_SCHEMA_VERSION: &str = "openagents.oa_workroomd.metadata_access.v1";
const GATEWAY_POLICY_SCHEMA_VERSION: &str = "openagents.oa_workroomd.gateway_policy.v1";
const GATEWAY_ACCESS_SCHEMA_VERSION: &str = "openagents.oa_workroomd.gateway_access.v1";
const INGRESS_STATE_SCHEMA_VERSION: &str = "openagents.oa_workroomd.ingress_state.v1";
const ARTIFACT_STATE_SCHEMA_VERSION: &str = "openagents.oa_workroomd.artifact_state.v1";
const ARTIFACT_RECEIPT_SCHEMA_VERSION: &str = "openagents.oa_workroomd.artifact_receipt.v1";
const CLOSEOUT_MANIFEST_SCHEMA_VERSION: &str = "openagents.oa_workroomd.closeout_manifest.v1";
const LIFECYCLE_STATE_SCHEMA_VERSION: &str = "openagents.oa_workroomd.lifecycle_state.v1";
const LIFECYCLE_RECEIPT_SCHEMA_VERSION: &str = "openagents.oa_workroomd.lifecycle_receipt.v1";
const CODEX_AUTH_STATE_SCHEMA_VERSION: &str = "openagents.oa_workroomd.codex_auth_state.v1";
const CODEX_RUN_STATE_SCHEMA_VERSION: &str = "openagents.oa_workroomd.codex_run_state.v1";
const ENV_OA_WORKROOM_HOME: &str = "OPENAGENTS_CLOUD_WORKROOM_HOME";
const METADATA_FILE: &str = "workroom-metadata.json";
const METADATA_ACCESS_LOG_FILE: &str = "metadata-access.jsonl";
const GATEWAY_POLICY_FILE: &str = "gateway-policy.json";
const GATEWAY_ACCESS_LOG_FILE: &str = "gateway-access.jsonl";
const INGRESS_STATE_FILE: &str = "ingress-state.json";
const ARTIFACT_STATE_FILE: &str = "artifact-state.json";
const ARTIFACT_RECEIPT_LOG_FILE: &str = "artifact-receipts.jsonl";
const CLOSEOUT_MANIFEST_FILE: &str = "closeout-manifest.json";
const ARTIFACT_OBJECT_ROOT: &str = "artifacts/sha256";
const LIFECYCLE_STATE_FILE: &str = "lifecycle-state.json";
const LIFECYCLE_RECEIPT_LOG_FILE: &str = "lifecycle-receipts.jsonl";
const CODEX_AUTH_STATE_FILE: &str = "codex-auth-state.json";
const CODEX_AUTH_RECEIPT_LOG_FILE: &str = "codex-auth-receipts.jsonl";
const CODEX_AUTH_HOME_ROOT: &str = "codex-auth";
const CODEX_RUN_STATE_FILE: &str = "codex-run-state.json";
const CODEX_RUN_EVENT_LOG_FILE: &str = "codex-run-events.jsonl";
const CODEX_SESSION_STATE_FILE: &str = "codex-session-state.json";
const CODEX_SESSION_EVENT_LOG_FILE: &str = "codex-session-events.jsonl";
const CODEX_WORKSPACE_ROOT: &str = "codex-workspaces";
const CODEX_SESSION_SCHEMA_VERSION: &str = "openagents.oa_workroomd.codex_session_state.v1";
const OPENAGENTS_RUNNER_EVENT_SCHEMA_VERSION: &str = "openagents.runner_event.v1";
const OPENAGENTS_RUNNER_EVENT_LOG_FILE: &str = "openagents-runner-events.jsonl";
const RESOURCE_USAGE_RECEIPT_LOG_FILE: &str = "resource-usage-receipts.jsonl";

const HELP: &str = "\
oa-workroomd

Private OpenAgents Cloud workroom sidecar scaffold.

Usage:
  oa-workroomd --help
  oa-workroomd --version
  oa-workroomd status [--json]
  oa-workroomd doctor [--json]
  oa-workroomd metadata init --workroom <id> --program <id> --repo <repo> --template <id> --budget <policy> --deadline <value> --trust-tier <tier> --capability <name> [--capability <name> ...] [--state-dir <path>] [--json]
  oa-workroomd metadata get [--state-dir <path>] [--json]
  oa-workroomd gateway policy init [--state-dir <path>] [--json]
  oa-workroomd gateway access --gateway <model|artifacts|receipts|memory|email|settlement> --capability <name> [--state-dir <path>] [--json]
  oa-workroomd gateway revoke --capability <name> [--state-dir <path>] [--json]
  oa-workroomd ingress status [--state-dir <path>] [--json]
  oa-workroomd ingress set --visibility <private|collaborators|public> [--preview-url <url>] [--custom-domain <domain>] [--state-dir <path>] [--json]
  oa-workroomd ingress collaborator grant --identity <id> [--state-dir <path>] [--json]
  oa-workroomd ingress token mint --label <label> [--state-dir <path>] [--json]
  oa-workroomd ingress revoke --target <target> [--state-dir <path>] [--json]
  oa-workroomd artifacts status [--state-dir <path>] [--json]
  oa-workroomd artifacts policy init --required <name> [--required <name> ...] [--state-dir <path>] [--json]
  oa-workroomd artifacts upload --name <name> --file <path> [--state-dir <path>] [--json]
  oa-workroomd closeout submit [--state-dir <path>] [--json]
  oa-workroomd lifecycle status [--state-dir <path>] [--json]
  oa-workroomd lifecycle <create|start|pause|resume|expose|closeout|archive|destroy> [--state-dir <path>] [--json]
  oa-workroomd codex auth materialize --grant-file <path> --auth-json-file <path> [--state-dir <path>] [--json]
  oa-workroomd codex auth status [--codex-bin <path>] [--state-dir <path>] [--json]
  oa-workroomd codex auth scrub [--state-dir <path>] [--json]
  oa-workroomd codex run --assignment-file <path> [--agent-runtime <codex|opencode_codex>] [--codex-bin <path>] [--opencode-bin <path>] [--state-dir <path>] [--json|--stream-jsonl]
  oa-workroomd codex session create --assignment-file <path> [--ttl-ms <ms>] [--state-dir <path>] [--json]
  oa-workroomd codex session start-turn --grant-file <path> --auth-json-file <path> [--codex-bin <path>] [--state-dir <path>] [--json|--stream-jsonl]
  oa-workroomd codex session continue-turn --prompt <text> --grant-file <path> --auth-json-file <path> [--codex-bin <path>] [--state-dir <path>] [--json|--stream-jsonl]
  oa-workroomd codex session cancel-turn [--reason <text>] [--state-dir <path>] [--json]
  oa-workroomd codex session status [--state-dir <path>] [--json]
  oa-workroomd codex session events [--cursor <n>] [--state-dir <path>] [--json|--stream-jsonl]
  oa-workroomd codex session closeout [--state-dir <path>] [--json]
  oa-workroomd codex session archive [--state-dir <path>] [--json]
  oa-workroomd codex session destroy [--state-dir <path>] [--json]
";

#[derive(Clone, Debug, Eq, PartialEq)]
enum Command {
    Help,
    Version,
    Status { json: bool },
    Doctor { json: bool },
    MetadataInit(MetadataInitArgs),
    MetadataGet(MetadataGetArgs),
    GatewayPolicyInit(GatewayPolicyArgs),
    GatewayAccess(GatewayAccessArgs),
    GatewayRevoke(GatewayRevokeArgs),
    IngressStatus(IngressArgs),
    IngressSet(IngressSetArgs),
    IngressCollaboratorGrant(IngressCollaboratorGrantArgs),
    IngressTokenMint(IngressTokenMintArgs),
    IngressRevoke(IngressRevokeArgs),
    ArtifactStatus(ArtifactArgs),
    ArtifactPolicyInit(ArtifactPolicyInitArgs),
    ArtifactUpload(ArtifactUploadArgs),
    CloseoutSubmit(CloseoutArgs),
    LifecycleStatus(LifecycleArgs),
    LifecycleAction(LifecycleActionArgs),
    CodexAuthMaterialize(CodexAuthMaterializeArgs),
    CodexAuthStatus(CodexAuthStatusArgs),
    CodexAuthScrub(CodexAuthScrubArgs),
    CodexRun(CodexRunArgs),
    CodexSessionCreate(CodexSessionCreateArgs),
    CodexSessionTurn(CodexSessionTurnArgs),
    CodexSessionCancel(CodexSessionCancelArgs),
    CodexSessionStatus(CodexSessionStatusArgs),
    CodexSessionEvents(CodexSessionEventsArgs),
    CodexSessionCloseout(CodexSessionSimpleArgs),
    CodexSessionArchive(CodexSessionSimpleArgs),
    CodexSessionDestroy(CodexSessionSimpleArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MetadataInitArgs {
    state_dir: PathBuf,
    metadata: WorkroomMetadata,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MetadataGetArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GatewayPolicyArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GatewayAccessArgs {
    state_dir: PathBuf,
    gateway: String,
    capability: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GatewayRevokeArgs {
    state_dir: PathBuf,
    capability: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IngressArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IngressSetArgs {
    state_dir: PathBuf,
    visibility: String,
    preview_url: Option<String>,
    custom_domain: Option<String>,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IngressCollaboratorGrantArgs {
    state_dir: PathBuf,
    identity: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IngressTokenMintArgs {
    state_dir: PathBuf,
    label: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IngressRevokeArgs {
    state_dir: PathBuf,
    target: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArtifactArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArtifactPolicyInitArgs {
    state_dir: PathBuf,
    required_artifacts: Vec<String>,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArtifactUploadArgs {
    state_dir: PathBuf,
    name: String,
    file: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CloseoutArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LifecycleArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LifecycleActionArgs {
    state_dir: PathBuf,
    action: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexAuthMaterializeArgs {
    state_dir: PathBuf,
    grant_file: PathBuf,
    auth_json_file: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexAuthStatusArgs {
    state_dir: PathBuf,
    codex_bin: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexAuthScrubArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AgentRuntime {
    Codex,
    OpencodeCodex,
}

impl AgentRuntime {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "codex" => Ok(Self::Codex),
            "opencode_codex" => Ok(Self::OpencodeCodex),
            other => Err(format!("unsupported agent runtime: {other}")),
        }
    }

    fn process_label(self) -> &'static str {
        match self {
            Self::Codex => "codex exec",
            Self::OpencodeCodex => "opencode run",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::OpencodeCodex => "OpenCode/Codex",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexRunArgs {
    state_dir: PathBuf,
    assignment_file: PathBuf,
    agent_runtime: AgentRuntime,
    codex_bin: PathBuf,
    opencode_bin: PathBuf,
    json: bool,
    stream_jsonl: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexSessionCreateArgs {
    state_dir: PathBuf,
    assignment_file: PathBuf,
    ttl_ms: Option<u128>,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexSessionTurnArgs {
    state_dir: PathBuf,
    grant_file: PathBuf,
    auth_json_file: PathBuf,
    codex_bin: PathBuf,
    prompt: Option<String>,
    json: bool,
    stream_jsonl: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexSessionCancelArgs {
    state_dir: PathBuf,
    reason: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexSessionStatusArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexSessionEventsArgs {
    state_dir: PathBuf,
    cursor: u64,
    json: bool,
    stream_jsonl: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexSessionSimpleArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct WorkroomMetadata {
    schema_version: String,
    workroom_id: String,
    program_id: String,
    repo: String,
    template_id: String,
    budget: String,
    deadline: String,
    trust_tier: String,
    capability_names: Vec<String>,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct MetadataAccessEvent {
    schema_version: String,
    event_id: String,
    occurred_at_ms: u128,
    workroom_id: String,
    access_kind: String,
    decision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct GatewayPolicy {
    schema_version: String,
    gateways: Vec<GatewayRule>,
    revoked_capabilities: Vec<String>,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct GatewayRule {
    gateway: String,
    capability_names: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct GatewayAccessDecision {
    schema_version: String,
    gateway: String,
    capability: String,
    decision: String,
    reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct GatewayAccessEvent {
    schema_version: String,
    event_id: String,
    occurred_at_ms: u128,
    gateway: String,
    capability: String,
    decision: String,
    reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct IngressState {
    schema_version: String,
    visibility: String,
    preview_url: Option<String>,
    custom_domain: Option<String>,
    collaborator_grants: Vec<String>,
    endpoint_token_digests: Vec<String>,
    receipts: Vec<IngressReceipt>,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct IngressReceipt {
    receipt_id: String,
    event_kind: String,
    detail: String,
    digest: String,
    occurred_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ArtifactState {
    schema_version: String,
    required_artifacts: Vec<String>,
    artifacts: Vec<ArtifactRecord>,
    closeout: Option<CloseoutManifest>,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ArtifactRecord {
    name: String,
    content_digest: String,
    object_path: String,
    size_bytes: u64,
    uploaded_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ArtifactDigestRef {
    name: String,
    content_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ArtifactReceipt {
    schema_version: String,
    receipt_id: String,
    event_kind: String,
    artifact_name: String,
    artifact_digest: String,
    manifest_digest: Option<String>,
    receipt_digest: String,
    emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CloseoutManifest {
    schema_version: String,
    manifest_id: String,
    required_artifacts: Vec<String>,
    artifact_digests: Vec<ArtifactDigestRef>,
    receipt_digests: Vec<String>,
    status: String,
    created_at_ms: u128,
    manifest_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ArtifactUploadOutput {
    artifact: ArtifactRecord,
    receipt: ArtifactReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CloseoutOutput {
    manifest: CloseoutManifest,
    receipt: ArtifactReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct LifecycleState {
    schema_version: String,
    state: String,
    receipts: Vec<LifecycleReceipt>,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct LifecycleReceipt {
    schema_version: String,
    receipt_id: String,
    action: String,
    from_state: String,
    to_state: String,
    receipt_digest: String,
    emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct LifecycleActionOutput {
    state: LifecycleState,
    receipt: LifecycleReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CodexAuthState {
    schema_version: String,
    grant: CodexAuthGrant,
    codex_home: String,
    auth_json_path: String,
    auth_json_digest: String,
    login_status: Option<String>,
    receipts: Vec<CodexAuthReceipt>,
    materialized_at_ms: u128,
    status_checked_at_ms: Option<u128>,
    scrubbed_at_ms: Option<u128>,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CodexAuthActionOutput {
    state: CodexAuthState,
    receipt: CodexAuthReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CodexRunState {
    schema_version: String,
    assignment: CodexWorkroomAssignment,
    workspace_dir: String,
    status: String,
    artifact_refs: Vec<CodexRunArtifactRef>,
    receipt_refs: Vec<String>,
    events: Vec<CodexWorkroomEvent>,
    started_at_ms: u128,
    completed_at_ms: Option<u128>,
    cleanup_at_ms: Option<u128>,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CodexRunArtifactRef {
    name: String,
    content_digest: String,
    object_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CodexRunOutput {
    state: CodexRunState,
    auth_receipts: Vec<CodexAuthReceipt>,
    runner_events: Vec<OpenAgentsRunnerEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CodexSessionState {
    schema_version: String,
    assignment: CodexWorkroomAssignment,
    workspace_dir: String,
    status: String,
    turn_index: u64,
    artifact_refs: Vec<CodexRunArtifactRef>,
    receipt_refs: Vec<String>,
    events: Vec<CodexWorkroomEvent>,
    created_at_ms: u128,
    updated_at_ms: u128,
    archived_at_ms: Option<u128>,
    destroyed_at_ms: Option<u128>,
    expires_at_ms: Option<u128>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CodexSessionOutput {
    session: CodexSessionState,
    auth_receipts: Vec<CodexAuthReceipt>,
    runner_events: Vec<OpenAgentsRunnerEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CodexSessionEventsOutput {
    events: Vec<CodexWorkroomEvent>,
    next_cursor: u64,
    runner_events: Vec<OpenAgentsRunnerEvent>,
    session_status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct OpenAgentsRunnerEvent {
    schema_version: String,
    external_event_id: String,
    sequence: u64,
    #[serde(rename = "type")]
    event_type: String,
    source: String,
    summary: String,
    detail_excerpt: Option<String>,
    raw_payload_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    raw_payload_json: Option<String>,
    artifact_refs: Vec<String>,
    receipt_refs: Vec<String>,
    emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexExecutionResult {
    exit_code: Option<i32>,
    artifact_completed: bool,
    timed_out: bool,
    wall_time_ms: u128,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

const DEFAULT_OPENCODE_CODEX_MODEL: &str = "openai/gpt-5.5";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedTokenUsage {
    source_event_type: String,
    provider: Option<String>,
    backend: String,
    count_source: TokenCountSource,
    billing_basis: String,
    model: Option<String>,
    input_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

fn main() {
    match run(env::args().skip(1).collect()) {
        Ok(Some(output)) => println!("{output}"),
        Ok(None) => {}
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{HELP}");
            std::process::exit(2);
        }
    }
}

fn run(args: Vec<String>) -> Result<Option<String>, String> {
    match parse_command(&args)? {
        Command::Help => Ok(Some(HELP.to_string())),
        Command::Version => Ok(Some(format!("oa-workroomd {}", env!("CARGO_PKG_VERSION")))),
        Command::Status { json } => status(json).map(Some),
        Command::Doctor { json } => Ok(Some(doctor(json))),
        Command::MetadataInit(args) => metadata_init(args).map(Some),
        Command::MetadataGet(args) => metadata_get(args).map(Some),
        Command::GatewayPolicyInit(args) => gateway_policy_init(args).map(Some),
        Command::GatewayAccess(args) => gateway_access(args).map(Some),
        Command::GatewayRevoke(args) => gateway_revoke(args).map(Some),
        Command::IngressStatus(args) => ingress_status(args).map(Some),
        Command::IngressSet(args) => ingress_set(args).map(Some),
        Command::IngressCollaboratorGrant(args) => ingress_collaborator_grant(args).map(Some),
        Command::IngressTokenMint(args) => ingress_token_mint(args).map(Some),
        Command::IngressRevoke(args) => ingress_revoke(args).map(Some),
        Command::ArtifactStatus(args) => artifact_status(args).map(Some),
        Command::ArtifactPolicyInit(args) => artifact_policy_init(args).map(Some),
        Command::ArtifactUpload(args) => artifact_upload(args).map(Some),
        Command::CloseoutSubmit(args) => closeout_submit(args).map(Some),
        Command::LifecycleStatus(args) => lifecycle_status(args).map(Some),
        Command::LifecycleAction(args) => lifecycle_action(args).map(Some),
        Command::CodexAuthMaterialize(args) => codex_auth_materialize(args).map(Some),
        Command::CodexAuthStatus(args) => codex_auth_status(args).map(Some),
        Command::CodexAuthScrub(args) => codex_auth_scrub(args).map(Some),
        Command::CodexRun(args) => codex_run(args).map(Some),
        Command::CodexSessionCreate(args) => codex_session_create(args).map(Some),
        Command::CodexSessionTurn(args) => codex_session_turn(args).map(Some),
        Command::CodexSessionCancel(args) => codex_session_cancel(args).map(Some),
        Command::CodexSessionStatus(args) => codex_session_status(args).map(Some),
        Command::CodexSessionEvents(args) => codex_session_events(args).map(Some),
        Command::CodexSessionCloseout(args) => codex_session_closeout(args).map(Some),
        Command::CodexSessionArchive(args) => codex_session_archive(args).map(Some),
        Command::CodexSessionDestroy(args) => codex_session_destroy(args).map(Some),
    }
}

fn parse_command(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        None => Ok(Command::Help),
        Some("--help" | "-h") => Ok(Command::Help),
        Some("--version" | "-V") => Ok(Command::Version),
        Some("status") => parse_json_flag(&args[1..]).map(|json| Command::Status { json }),
        Some("doctor") => parse_json_flag(&args[1..]).map(|json| Command::Doctor { json }),
        Some("metadata") => parse_metadata_command(&args[1..]),
        Some("gateway") => parse_gateway_command(&args[1..]),
        Some("ingress") => parse_ingress_command(&args[1..]),
        Some("artifacts") => parse_artifacts_command(&args[1..]),
        Some("closeout") => parse_closeout_command(&args[1..]),
        Some("lifecycle") => parse_lifecycle_command(&args[1..]),
        Some("codex") => parse_codex_command(&args[1..]),
        Some(other) => Err(format!("unknown command or flag: {other}")),
    }
}

fn parse_json_flag(args: &[String]) -> Result<bool, String> {
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            other => return Err(format!("unexpected argument: {other}")),
        }
    }
    Ok(json)
}

fn parse_metadata_command(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        Some("init") => parse_metadata_init_args(&args[1..]).map(Command::MetadataInit),
        Some("get") => parse_metadata_get_args(&args[1..]).map(Command::MetadataGet),
        _ => Err("expected metadata init or metadata get".to_string()),
    }
}

fn parse_metadata_init_args(args: &[String]) -> Result<MetadataInitArgs, String> {
    let mut workroom_id = None;
    let mut program_id = None;
    let mut repo = None;
    let mut template_id = None;
    let mut budget = None;
    let mut deadline = None;
    let mut trust_tier = None;
    let mut capability_names = Vec::new();
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--workroom" => {
                workroom_id = Some(required_value(args, index, "--workroom")?);
                index += 2;
            }
            "--program" => {
                program_id = Some(required_value(args, index, "--program")?);
                index += 2;
            }
            "--repo" => {
                repo = Some(required_value(args, index, "--repo")?);
                index += 2;
            }
            "--template" => {
                template_id = Some(required_value(args, index, "--template")?);
                index += 2;
            }
            "--budget" => {
                budget = Some(required_value(args, index, "--budget")?);
                index += 2;
            }
            "--deadline" => {
                deadline = Some(required_value(args, index, "--deadline")?);
                index += 2;
            }
            "--trust-tier" => {
                trust_tier = Some(required_value(args, index, "--trust-tier")?);
                index += 2;
            }
            "--capability" => {
                capability_names.push(required_value(args, index, "--capability")?);
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected metadata init argument: {other}")),
        }
    }
    if capability_names.is_empty() {
        return Err("metadata init requires at least one --capability".to_string());
    }
    let metadata = WorkroomMetadata {
        schema_version: METADATA_SCHEMA_VERSION.to_string(),
        workroom_id: workroom_id.ok_or_else(|| "metadata init requires --workroom".to_string())?,
        program_id: program_id.ok_or_else(|| "metadata init requires --program".to_string())?,
        repo: repo.ok_or_else(|| "metadata init requires --repo".to_string())?,
        template_id: template_id.ok_or_else(|| "metadata init requires --template".to_string())?,
        budget: budget.ok_or_else(|| "metadata init requires --budget".to_string())?,
        deadline: deadline.ok_or_else(|| "metadata init requires --deadline".to_string())?,
        trust_tier: trust_tier.ok_or_else(|| "metadata init requires --trust-tier".to_string())?,
        capability_names,
        updated_at_ms: now_epoch_ms()?,
    };
    validate_metadata(&metadata)?;
    Ok(MetadataInitArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        metadata,
        json,
    })
}

fn parse_metadata_get_args(args: &[String]) -> Result<MetadataGetArgs, String> {
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected metadata get argument: {other}")),
        }
    }
    Ok(MetadataGetArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        json,
    })
}

fn parse_gateway_command(args: &[String]) -> Result<Command, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("policy"), Some("init")) => {
            parse_gateway_policy_args(&args[2..]).map(Command::GatewayPolicyInit)
        }
        (Some("access"), _) => parse_gateway_access_args(&args[1..]).map(Command::GatewayAccess),
        (Some("revoke"), _) => parse_gateway_revoke_args(&args[1..]).map(Command::GatewayRevoke),
        _ => Err("expected gateway policy init, gateway access, or gateway revoke".to_string()),
    }
}

fn parse_gateway_policy_args(args: &[String]) -> Result<GatewayPolicyArgs, String> {
    let metadata_args = parse_metadata_get_args(args)?;
    Ok(GatewayPolicyArgs {
        state_dir: metadata_args.state_dir,
        json: metadata_args.json,
    })
}

fn parse_gateway_access_args(args: &[String]) -> Result<GatewayAccessArgs, String> {
    let mut gateway = None;
    let mut capability = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--gateway" => {
                gateway = Some(required_value(args, index, "--gateway")?);
                index += 2;
            }
            "--capability" => {
                capability = Some(required_value(args, index, "--capability")?);
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected gateway access argument: {other}")),
        }
    }
    Ok(GatewayAccessArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        gateway: gateway.ok_or_else(|| "gateway access requires --gateway".to_string())?,
        capability: capability.ok_or_else(|| "gateway access requires --capability".to_string())?,
        json,
    })
}

fn parse_gateway_revoke_args(args: &[String]) -> Result<GatewayRevokeArgs, String> {
    let mut capability = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--capability" => {
                capability = Some(required_value(args, index, "--capability")?);
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected gateway revoke argument: {other}")),
        }
    }
    Ok(GatewayRevokeArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        capability: capability.ok_or_else(|| "gateway revoke requires --capability".to_string())?,
        json,
    })
}

fn parse_ingress_command(args: &[String]) -> Result<Command, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
        args.get(2).map(String::as_str),
    ) {
        (Some("status"), _, _) => parse_ingress_args(&args[1..]).map(Command::IngressStatus),
        (Some("set"), _, _) => parse_ingress_set_args(&args[1..]).map(Command::IngressSet),
        (Some("collaborator"), Some("grant"), _) => {
            parse_ingress_collaborator_grant_args(&args[2..])
                .map(Command::IngressCollaboratorGrant)
        }
        (Some("token"), Some("mint"), _) => {
            parse_ingress_token_mint_args(&args[2..]).map(Command::IngressTokenMint)
        }
        (Some("revoke"), _, _) => parse_ingress_revoke_args(&args[1..]).map(Command::IngressRevoke),
        _ => Err(
            "expected ingress status, ingress set, ingress collaborator grant, ingress token mint, or ingress revoke"
                .to_string(),
        ),
    }
}

fn parse_ingress_args(args: &[String]) -> Result<IngressArgs, String> {
    let metadata_args = parse_metadata_get_args(args)?;
    Ok(IngressArgs {
        state_dir: metadata_args.state_dir,
        json: metadata_args.json,
    })
}

fn parse_ingress_set_args(args: &[String]) -> Result<IngressSetArgs, String> {
    let mut visibility = None;
    let mut preview_url = None;
    let mut custom_domain = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--visibility" => {
                visibility = Some(required_value(args, index, "--visibility")?);
                index += 2;
            }
            "--preview-url" => {
                preview_url = Some(required_value(args, index, "--preview-url")?);
                index += 2;
            }
            "--custom-domain" => {
                custom_domain = Some(required_value(args, index, "--custom-domain")?);
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected ingress set argument: {other}")),
        }
    }
    Ok(IngressSetArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        visibility: visibility.ok_or_else(|| "ingress set requires --visibility".to_string())?,
        preview_url,
        custom_domain,
        json,
    })
}

fn parse_ingress_collaborator_grant_args(
    args: &[String],
) -> Result<IngressCollaboratorGrantArgs, String> {
    let mut identity = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--identity" => {
                identity = Some(required_value(args, index, "--identity")?);
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected ingress collaborator argument: {other}")),
        }
    }
    Ok(IngressCollaboratorGrantArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        identity: identity
            .ok_or_else(|| "ingress collaborator grant requires --identity".to_string())?,
        json,
    })
}

fn parse_ingress_token_mint_args(args: &[String]) -> Result<IngressTokenMintArgs, String> {
    let mut label = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--label" => {
                label = Some(required_value(args, index, "--label")?);
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected ingress token argument: {other}")),
        }
    }
    Ok(IngressTokenMintArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        label: label.ok_or_else(|| "ingress token mint requires --label".to_string())?,
        json,
    })
}

fn parse_ingress_revoke_args(args: &[String]) -> Result<IngressRevokeArgs, String> {
    let mut target = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                target = Some(required_value(args, index, "--target")?);
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected ingress revoke argument: {other}")),
        }
    }
    Ok(IngressRevokeArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        target: target.ok_or_else(|| "ingress revoke requires --target".to_string())?,
        json,
    })
}

fn parse_artifacts_command(args: &[String]) -> Result<Command, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("status"), _) => parse_artifact_args(&args[1..]).map(Command::ArtifactStatus),
        (Some("policy"), Some("init")) => {
            parse_artifact_policy_init_args(&args[2..]).map(Command::ArtifactPolicyInit)
        }
        (Some("upload"), _) => parse_artifact_upload_args(&args[1..]).map(Command::ArtifactUpload),
        _ => {
            Err("expected artifacts status, artifacts policy init, or artifacts upload".to_string())
        }
    }
}

fn parse_closeout_command(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        Some("submit") => parse_closeout_args(&args[1..]).map(Command::CloseoutSubmit),
        _ => Err("expected closeout submit".to_string()),
    }
}

fn parse_artifact_args(args: &[String]) -> Result<ArtifactArgs, String> {
    let metadata_args = parse_metadata_get_args(args)?;
    Ok(ArtifactArgs {
        state_dir: metadata_args.state_dir,
        json: metadata_args.json,
    })
}

fn parse_artifact_policy_init_args(args: &[String]) -> Result<ArtifactPolicyInitArgs, String> {
    let mut required_artifacts = Vec::new();
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--required" => {
                required_artifacts.push(required_value(args, index, "--required")?);
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected artifacts policy argument: {other}")),
        }
    }
    if required_artifacts.is_empty() {
        return Err("artifacts policy init requires at least one --required".to_string());
    }
    Ok(ArtifactPolicyInitArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        required_artifacts,
        json,
    })
}

fn parse_artifact_upload_args(args: &[String]) -> Result<ArtifactUploadArgs, String> {
    let mut name = None;
    let mut file = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--name" => {
                name = Some(required_value(args, index, "--name")?);
                index += 2;
            }
            "--file" => {
                file = Some(PathBuf::from(required_value(args, index, "--file")?));
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected artifacts upload argument: {other}")),
        }
    }
    Ok(ArtifactUploadArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        name: name.ok_or_else(|| "artifacts upload requires --name".to_string())?,
        file: file.ok_or_else(|| "artifacts upload requires --file".to_string())?,
        json,
    })
}

fn parse_closeout_args(args: &[String]) -> Result<CloseoutArgs, String> {
    let metadata_args = parse_metadata_get_args(args)?;
    Ok(CloseoutArgs {
        state_dir: metadata_args.state_dir,
        json: metadata_args.json,
    })
}

fn parse_lifecycle_command(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        Some("status") => parse_lifecycle_args(&args[1..]).map(Command::LifecycleStatus),
        Some(action @ ("create" | "start" | "pause" | "resume" | "expose" | "closeout"
        | "archive" | "destroy")) => {
            parse_lifecycle_action_args(action, &args[1..]).map(Command::LifecycleAction)
        }
        _ => Err(
            "expected lifecycle status or lifecycle create/start/pause/resume/expose/closeout/archive/destroy"
                .to_string(),
        ),
    }
}

fn parse_lifecycle_args(args: &[String]) -> Result<LifecycleArgs, String> {
    let metadata_args = parse_metadata_get_args(args)?;
    Ok(LifecycleArgs {
        state_dir: metadata_args.state_dir,
        json: metadata_args.json,
    })
}

fn parse_lifecycle_action_args(
    action: &str,
    args: &[String],
) -> Result<LifecycleActionArgs, String> {
    let lifecycle_args = parse_lifecycle_args(args)?;
    Ok(LifecycleActionArgs {
        state_dir: lifecycle_args.state_dir,
        action: action.to_string(),
        json: lifecycle_args.json,
    })
}

fn parse_codex_command(args: &[String]) -> Result<Command, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
        args.get(2).map(String::as_str),
    ) {
        (Some("auth"), Some("materialize"), _) => {
            parse_codex_auth_materialize_args(&args[2..]).map(Command::CodexAuthMaterialize)
        }
        (Some("auth"), Some("status"), _) => {
            parse_codex_auth_status_args(&args[2..]).map(Command::CodexAuthStatus)
        }
        (Some("auth"), Some("scrub"), _) => {
            parse_codex_auth_scrub_args(&args[2..]).map(Command::CodexAuthScrub)
        }
        (Some("session"), Some("create"), _) => {
            parse_codex_session_create_args(&args[2..]).map(Command::CodexSessionCreate)
        }
        (Some("session"), Some("start-turn"), _) => {
            parse_codex_session_turn_args(None, &args[2..]).map(Command::CodexSessionTurn)
        }
        (Some("session"), Some("continue-turn"), _) => {
            parse_codex_session_turn_args(Some("continue-turn"), &args[2..])
                .map(Command::CodexSessionTurn)
        }
        (Some("session"), Some("cancel-turn"), _) => {
            parse_codex_session_cancel_args(&args[2..]).map(Command::CodexSessionCancel)
        }
        (Some("session"), Some("status"), _) => {
            parse_codex_session_status_args(&args[2..]).map(Command::CodexSessionStatus)
        }
        (Some("session"), Some("events"), _) => {
            parse_codex_session_events_args(&args[2..]).map(Command::CodexSessionEvents)
        }
        (Some("session"), Some("closeout"), _) => {
            parse_codex_session_simple_args(&args[2..]).map(Command::CodexSessionCloseout)
        }
        (Some("session"), Some("archive"), _) => {
            parse_codex_session_simple_args(&args[2..]).map(Command::CodexSessionArchive)
        }
        (Some("session"), Some("destroy"), _) => {
            parse_codex_session_simple_args(&args[2..]).map(Command::CodexSessionDestroy)
        }
        (Some("run"), _, _) => parse_codex_run_args(&args[1..]).map(Command::CodexRun),
        _ => Err(
            "expected codex auth materialize/status/scrub, codex run, or codex session command"
                .to_string(),
        ),
    }
}

fn parse_codex_auth_materialize_args(args: &[String]) -> Result<CodexAuthMaterializeArgs, String> {
    let mut state_dir = None;
    let mut grant_file = None;
    let mut auth_json_file = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--grant-file" => {
                grant_file = Some(PathBuf::from(required_value(args, index, "--grant-file")?));
                index += 2;
            }
            "--auth-json-file" => {
                auth_json_file = Some(PathBuf::from(required_value(
                    args,
                    index,
                    "--auth-json-file",
                )?));
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => {
                return Err(format!(
                    "unexpected codex auth materialize argument: {other}"
                ))
            }
        }
    }
    Ok(CodexAuthMaterializeArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        grant_file: grant_file
            .ok_or_else(|| "codex auth materialize requires --grant-file".to_string())?,
        auth_json_file: auth_json_file
            .ok_or_else(|| "codex auth materialize requires --auth-json-file".to_string())?,
        json,
    })
}

fn parse_codex_auth_status_args(args: &[String]) -> Result<CodexAuthStatusArgs, String> {
    let mut state_dir = None;
    let mut codex_bin = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--codex-bin" => {
                codex_bin = Some(PathBuf::from(required_value(args, index, "--codex-bin")?));
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected codex auth status argument: {other}")),
        }
    }
    Ok(CodexAuthStatusArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        codex_bin: codex_bin.unwrap_or_else(|| PathBuf::from("codex")),
        json,
    })
}

fn parse_codex_auth_scrub_args(args: &[String]) -> Result<CodexAuthScrubArgs, String> {
    let metadata_args = parse_metadata_get_args(args)?;
    Ok(CodexAuthScrubArgs {
        state_dir: metadata_args.state_dir,
        json: metadata_args.json,
    })
}

fn parse_codex_run_args(args: &[String]) -> Result<CodexRunArgs, String> {
    let mut state_dir = None;
    let mut assignment_file = None;
    let mut agent_runtime = None;
    let mut codex_bin = None;
    let mut opencode_bin = None;
    let mut json = false;
    let mut stream_jsonl = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--assignment-file" => {
                assignment_file = Some(PathBuf::from(required_value(
                    args,
                    index,
                    "--assignment-file",
                )?));
                index += 2;
            }
            "--agent-runtime" => {
                agent_runtime = Some(AgentRuntime::parse(
                    required_value(args, index, "--agent-runtime")?.as_str(),
                )?);
                index += 2;
            }
            "--codex-bin" => {
                codex_bin = Some(PathBuf::from(required_value(args, index, "--codex-bin")?));
                index += 2;
            }
            "--opencode-bin" => {
                opencode_bin = Some(PathBuf::from(required_value(
                    args,
                    index,
                    "--opencode-bin",
                )?));
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            "--stream-jsonl" => {
                stream_jsonl = true;
                index += 1;
            }
            other => return Err(format!("unexpected codex run argument: {other}")),
        }
    }
    if json && stream_jsonl {
        return Err("codex run accepts --json or --stream-jsonl, not both".to_string());
    }
    Ok(CodexRunArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        assignment_file: assignment_file
            .ok_or_else(|| "codex run requires --assignment-file".to_string())?,
        agent_runtime: agent_runtime.unwrap_or(AgentRuntime::Codex),
        codex_bin: codex_bin.unwrap_or_else(|| PathBuf::from("codex")),
        opencode_bin: opencode_bin.unwrap_or_else(|| PathBuf::from("opencode")),
        json,
        stream_jsonl,
    })
}

fn parse_codex_session_create_args(args: &[String]) -> Result<CodexSessionCreateArgs, String> {
    let mut state_dir = None;
    let mut assignment_file = None;
    let mut ttl_ms = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--assignment-file" => {
                assignment_file = Some(PathBuf::from(required_value(
                    args,
                    index,
                    "--assignment-file",
                )?));
                index += 2;
            }
            "--ttl-ms" => {
                ttl_ms = Some(
                    required_value(args, index, "--ttl-ms")?
                        .parse::<u128>()
                        .map_err(|_| "invalid --ttl-ms".to_string())?,
                );
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected codex session create argument: {other}")),
        }
    }
    Ok(CodexSessionCreateArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        assignment_file: assignment_file
            .ok_or_else(|| "codex session create requires --assignment-file".to_string())?,
        ttl_ms,
        json,
    })
}

fn parse_codex_session_turn_args(
    prompt_required_for: Option<&str>,
    args: &[String],
) -> Result<CodexSessionTurnArgs, String> {
    let mut state_dir = None;
    let mut grant_file = None;
    let mut auth_json_file = None;
    let mut codex_bin = None;
    let mut prompt = None;
    let mut json = false;
    let mut stream_jsonl = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--grant-file" => {
                grant_file = Some(PathBuf::from(required_value(args, index, "--grant-file")?));
                index += 2;
            }
            "--auth-json-file" => {
                auth_json_file = Some(PathBuf::from(required_value(
                    args,
                    index,
                    "--auth-json-file",
                )?));
                index += 2;
            }
            "--codex-bin" => {
                codex_bin = Some(PathBuf::from(required_value(args, index, "--codex-bin")?));
                index += 2;
            }
            "--prompt" => {
                prompt = Some(required_value(args, index, "--prompt")?);
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            "--stream-jsonl" => {
                stream_jsonl = true;
                index += 1;
            }
            other => return Err(format!("unexpected codex session turn argument: {other}")),
        }
    }
    if json && stream_jsonl {
        return Err("codex session turn accepts --json or --stream-jsonl, not both".to_string());
    }
    if prompt_required_for.is_some() && prompt.as_deref().unwrap_or_default().trim().is_empty() {
        return Err("codex session continue-turn requires --prompt".to_string());
    }
    Ok(CodexSessionTurnArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        grant_file: grant_file
            .ok_or_else(|| "codex session turn requires --grant-file".to_string())?,
        auth_json_file: auth_json_file
            .ok_or_else(|| "codex session turn requires --auth-json-file".to_string())?,
        codex_bin: codex_bin.unwrap_or_else(|| PathBuf::from("codex")),
        prompt,
        json,
        stream_jsonl,
    })
}

fn parse_codex_session_cancel_args(args: &[String]) -> Result<CodexSessionCancelArgs, String> {
    let mut state_dir = None;
    let mut reason = "operator_requested_cancel".to_string();
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--reason" => {
                reason = required_value(args, index, "--reason")?;
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected codex session cancel argument: {other}")),
        }
    }
    Ok(CodexSessionCancelArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        reason,
        json,
    })
}

fn parse_codex_session_status_args(args: &[String]) -> Result<CodexSessionStatusArgs, String> {
    let metadata_args = parse_metadata_get_args(args)?;
    Ok(CodexSessionStatusArgs {
        state_dir: metadata_args.state_dir,
        json: metadata_args.json,
    })
}

fn parse_codex_session_events_args(args: &[String]) -> Result<CodexSessionEventsArgs, String> {
    let mut state_dir = None;
    let mut cursor = 0_u64;
    let mut json = false;
    let mut stream_jsonl = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--cursor" => {
                cursor = required_value(args, index, "--cursor")?
                    .parse::<u64>()
                    .map_err(|_| "invalid --cursor".to_string())?;
                index += 2;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            "--stream-jsonl" => {
                stream_jsonl = true;
                index += 1;
            }
            other => return Err(format!("unexpected codex session events argument: {other}")),
        }
    }
    if json && stream_jsonl {
        return Err("codex session events accepts --json or --stream-jsonl, not both".to_string());
    }
    Ok(CodexSessionEventsArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        cursor,
        json,
        stream_jsonl,
    })
}

fn parse_codex_session_simple_args(args: &[String]) -> Result<CodexSessionSimpleArgs, String> {
    let metadata_args = parse_metadata_get_args(args)?;
    Ok(CodexSessionSimpleArgs {
        state_dir: metadata_args.state_dir,
        json: metadata_args.json,
    })
}

fn required_value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index + 1)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn status(json: bool) -> Result<String, String> {
    let snapshot = WorkroomSnapshot::scaffold();
    if json {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| format!("failed to serialize workroom status: {error}"))
    } else {
        Ok(format!(
            "service: oa-workroomd\nstatus: {:?}\ncontract: {}\nwallet_authority: {}\nready: false",
            snapshot.lifecycle.observed_state,
            snapshot.contract_version,
            snapshot.runtime.wallet_authority
        ))
    }
}

fn doctor(json: bool) -> String {
    if json {
        "{\"service\":\"oa-workroomd\",\"checks\":[{\"name\":\"repo_scaffold\",\"status\":\"pass\"},{\"name\":\"workroom_runtime\",\"status\":\"not_implemented\"}]}".to_string()
    } else {
        "repo_scaffold: pass\nworkroom_runtime: not_implemented".to_string()
    }
}

fn metadata_init(args: MetadataInitArgs) -> Result<String, String> {
    fs::create_dir_all(&args.state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(&args.state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(&args.metadata)
        .map_err(|error| format!("failed to serialize metadata: {error}"))?;
    fs::write(metadata_path(&args.state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write metadata: {error}"))?;
    render_metadata(args.json, &args.metadata)
}

fn metadata_get(args: MetadataGetArgs) -> Result<String, String> {
    let metadata = load_metadata(&args.state_dir)?;
    validate_metadata(&metadata)?;
    append_metadata_access_event(&args.state_dir, metadata.workroom_id.as_str())?;
    render_metadata(args.json, &metadata)
}

fn render_metadata(json: bool, metadata: &WorkroomMetadata) -> Result<String, String> {
    if json {
        serde_json::to_string_pretty(metadata)
            .map_err(|error| format!("failed to serialize metadata: {error}"))
    } else {
        Ok(format!(
            "workroom_id: {}\nprogram_id: {}\nrepo: {}\ntemplate_id: {}\ncapability_count: {}",
            metadata.workroom_id,
            metadata.program_id,
            metadata.repo,
            metadata.template_id,
            metadata.capability_names.len()
        ))
    }
}

fn load_metadata(state_dir: &Path) -> Result<WorkroomMetadata, String> {
    let path = metadata_path(state_dir);
    if !path.exists() {
        return Ok(default_metadata()?);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read metadata {}: {error}", path_label(&path)))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("failed to parse metadata {}: {error}", path_label(&path)))
}

fn default_metadata() -> Result<WorkroomMetadata, String> {
    let snapshot = WorkroomSnapshot::scaffold();
    Ok(WorkroomMetadata {
        schema_version: METADATA_SCHEMA_VERSION.to_string(),
        workroom_id: snapshot.identity.workroom_id,
        program_id: snapshot.identity.program_id,
        repo: "repo.local.scaffold".to_string(),
        template_id: snapshot.identity.template_id,
        budget: "not_configured".to_string(),
        deadline: "not_configured".to_string(),
        trust_tier: "local_dev".to_string(),
        capability_names: snapshot
            .capabilities
            .iter()
            .map(|capability| capability.capability.clone())
            .collect(),
        updated_at_ms: now_epoch_ms()?,
    })
}

fn validate_metadata(metadata: &WorkroomMetadata) -> Result<(), String> {
    if metadata.schema_version != METADATA_SCHEMA_VERSION {
        return Err(format!(
            "unsupported metadata schema '{}'",
            metadata.schema_version
        ));
    }
    let mut values = vec![
        metadata.workroom_id.as_str(),
        metadata.program_id.as_str(),
        metadata.repo.as_str(),
        metadata.template_id.as_str(),
        metadata.budget.as_str(),
        metadata.deadline.as_str(),
        metadata.trust_tier.as_str(),
    ];
    values.extend(metadata.capability_names.iter().map(String::as_str));
    for value in values {
        if value.trim().is_empty() {
            return Err("metadata fields must not be empty".to_string());
        }
        if contains_forbidden_metadata(value) {
            return Err(
                "metadata contains secret, wallet, token, or private topology marker".to_string(),
            );
        }
    }
    Ok(())
}

fn contains_forbidden_metadata(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "secret-token",
        "bearer ",
        "api_key",
        "openai_api_key",
        "access_token",
        "refresh_token",
        "id_token",
        "device_code",
        "code_verifier",
        "password",
        "wallet_seed",
        "private_key",
        "tailscale",
        "tailnet",
        "private_topology",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn append_metadata_access_event(state_dir: &Path, workroom_id: &str) -> Result<(), String> {
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let occurred_at_ms = now_epoch_ms()?;
    let event = MetadataAccessEvent {
        schema_version: METADATA_ACCESS_SCHEMA_VERSION.to_string(),
        event_id: format!("metadata.access.{workroom_id}.{occurred_at_ms}"),
        occurred_at_ms,
        workroom_id: workroom_id.to_string(),
        access_kind: "metadata_get".to_string(),
        decision: "allowed".to_string(),
    };
    let line = serde_json::to_string(&event)
        .map_err(|error| format!("failed to serialize metadata access event: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(metadata_access_log_path(state_dir))
        .map_err(|error| format!("failed to open metadata access log: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to append metadata access: {error}"))
}

fn gateway_policy_init(args: GatewayPolicyArgs) -> Result<String, String> {
    let policy = default_gateway_policy()?;
    save_gateway_policy(&args.state_dir, &policy)?;
    render_json_or_human(
        args.json,
        &policy,
        format!(
            "gateway_count: {}\nrevoked_count: {}",
            policy.gateways.len(),
            policy.revoked_capabilities.len()
        ),
    )
}

fn gateway_access(args: GatewayAccessArgs) -> Result<String, String> {
    let policy = load_gateway_policy(&args.state_dir)?;
    let (decision, reason) =
        evaluate_gateway_access(&policy, args.gateway.as_str(), args.capability.as_str());
    let output = GatewayAccessDecision {
        schema_version: GATEWAY_ACCESS_SCHEMA_VERSION.to_string(),
        gateway: args.gateway.clone(),
        capability: args.capability.clone(),
        decision: decision.to_string(),
        reason: reason.to_string(),
    };
    append_gateway_access_event(&args.state_dir, &output)?;
    render_json_or_human(
        args.json,
        &output,
        format!(
            "gateway: {}\ncapability: {}\ndecision: {}\nreason: {}",
            output.gateway, output.capability, output.decision, output.reason
        ),
    )
}

fn gateway_revoke(args: GatewayRevokeArgs) -> Result<String, String> {
    if contains_forbidden_metadata(args.capability.as_str()) {
        return Err("capability contains forbidden marker".to_string());
    }
    let mut policy = load_gateway_policy(&args.state_dir)?;
    if !policy
        .revoked_capabilities
        .iter()
        .any(|capability| capability == &args.capability)
    {
        policy.revoked_capabilities.push(args.capability.clone());
    }
    policy.updated_at_ms = now_epoch_ms()?;
    save_gateway_policy(&args.state_dir, &policy)?;
    render_json_or_human(
        args.json,
        &policy,
        format!(
            "revoked_capability: {}\nrevoked_count: {}",
            args.capability,
            policy.revoked_capabilities.len()
        ),
    )
}

fn evaluate_gateway_access<'a>(
    policy: &'a GatewayPolicy,
    gateway: &str,
    capability: &str,
) -> (&'a str, &'a str) {
    if policy
        .revoked_capabilities
        .iter()
        .any(|revoked| revoked == capability)
    {
        return ("denied", "capability_revoked");
    }
    let Some(rule) = policy.gateways.iter().find(|rule| rule.gateway == gateway) else {
        return ("denied", "unknown_gateway");
    };
    if rule
        .capability_names
        .iter()
        .any(|allowed| allowed == capability)
    {
        ("allowed", "capability_allowed")
    } else {
        ("denied", "capability_not_allowed_for_gateway")
    }
}

fn append_gateway_access_event(
    state_dir: &Path,
    decision: &GatewayAccessDecision,
) -> Result<(), String> {
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let occurred_at_ms = now_epoch_ms()?;
    let event = GatewayAccessEvent {
        schema_version: GATEWAY_ACCESS_SCHEMA_VERSION.to_string(),
        event_id: format!(
            "gateway.access.{}.{}.{}",
            decision.gateway, decision.capability, occurred_at_ms
        ),
        occurred_at_ms,
        gateway: decision.gateway.clone(),
        capability: decision.capability.clone(),
        decision: decision.decision.clone(),
        reason: decision.reason.clone(),
    };
    validate_gateway_event(&event)?;
    let line = serde_json::to_string(&event)
        .map_err(|error| format!("failed to serialize gateway access event: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(gateway_access_log_path(state_dir))
        .map_err(|error| format!("failed to open gateway access log: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to append gateway access: {error}"))
}

fn validate_gateway_event(event: &GatewayAccessEvent) -> Result<(), String> {
    for value in [
        event.event_id.as_str(),
        event.gateway.as_str(),
        event.capability.as_str(),
        event.decision.as_str(),
        event.reason.as_str(),
    ] {
        if contains_forbidden_metadata(value) {
            return Err("gateway audit event contains forbidden marker".to_string());
        }
    }
    Ok(())
}

fn load_gateway_policy(state_dir: &Path) -> Result<GatewayPolicy, String> {
    let path = gateway_policy_path(state_dir);
    if !path.exists() {
        return default_gateway_policy();
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read gateway policy {}: {error}",
            path_label(&path)
        )
    })?;
    let policy = serde_json::from_str::<GatewayPolicy>(&raw).map_err(|error| {
        format!(
            "failed to parse gateway policy {}: {error}",
            path_label(&path)
        )
    })?;
    validate_gateway_policy(&policy)?;
    Ok(policy)
}

fn save_gateway_policy(state_dir: &Path, policy: &GatewayPolicy) -> Result<(), String> {
    validate_gateway_policy(policy)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(policy)
        .map_err(|error| format!("failed to serialize gateway policy: {error}"))?;
    fs::write(gateway_policy_path(state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write gateway policy: {error}"))
}

fn default_gateway_policy() -> Result<GatewayPolicy, String> {
    Ok(GatewayPolicy {
        schema_version: GATEWAY_POLICY_SCHEMA_VERSION.to_string(),
        gateways: vec![
            gateway_rule("model", "model.gateway"),
            gateway_rule("artifacts", "artifact.write"),
            gateway_rule("receipts", "receipt.write"),
            gateway_rule("memory", "memory.read"),
            gateway_rule("email", "email.send_receive"),
            gateway_rule("settlement", "settlement.metadata"),
        ],
        revoked_capabilities: Vec::new(),
        updated_at_ms: now_epoch_ms()?,
    })
}

fn gateway_rule(gateway: &str, capability: &str) -> GatewayRule {
    GatewayRule {
        gateway: gateway.to_string(),
        capability_names: vec![capability.to_string()],
    }
}

fn validate_gateway_policy(policy: &GatewayPolicy) -> Result<(), String> {
    if policy.schema_version != GATEWAY_POLICY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported gateway policy schema '{}'",
            policy.schema_version
        ));
    }
    for rule in &policy.gateways {
        if contains_forbidden_metadata(rule.gateway.as_str())
            || rule
                .capability_names
                .iter()
                .any(|capability| contains_forbidden_metadata(capability))
        {
            return Err("gateway policy contains forbidden marker".to_string());
        }
    }
    Ok(())
}

fn render_json_or_human<T: Serialize>(
    json: bool,
    value: &T,
    human: String,
) -> Result<String, String> {
    if json {
        serde_json::to_string_pretty(value)
            .map_err(|error| format!("failed to serialize output: {error}"))
    } else {
        Ok(human)
    }
}

fn ingress_status(args: IngressArgs) -> Result<String, String> {
    let state = load_ingress_state(&args.state_dir)?;
    render_json_or_human(
        args.json,
        &state,
        format!(
            "visibility: {}\ntoken_count: {}\nreceipt_count: {}",
            state.visibility,
            state.endpoint_token_digests.len(),
            state.receipts.len()
        ),
    )
}

fn ingress_set(args: IngressSetArgs) -> Result<String, String> {
    validate_visibility(args.visibility.as_str())?;
    for value in [
        Some(args.visibility.as_str()),
        args.preview_url.as_deref(),
        args.custom_domain.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if contains_forbidden_metadata(value) {
            return Err("ingress input contains forbidden marker".to_string());
        }
    }
    let mut state = load_ingress_state(&args.state_dir)?;
    state.visibility = args.visibility;
    if args.preview_url.is_some() {
        state.preview_url = args.preview_url;
    }
    if args.custom_domain.is_some() {
        state.custom_domain = args.custom_domain;
    }
    let event_kind = if state.visibility == "private" {
        "ingress_updated"
    } else {
        "preview_exposed"
    };
    push_ingress_receipt(&mut state, event_kind, "visibility_or_domain_changed")?;
    save_ingress_state(&args.state_dir, &state)?;
    render_json_or_human(
        args.json,
        &state,
        format!(
            "visibility: {}\nreceipt_count: {}",
            state.visibility,
            state.receipts.len()
        ),
    )
}

fn ingress_collaborator_grant(args: IngressCollaboratorGrantArgs) -> Result<String, String> {
    if contains_forbidden_metadata(args.identity.as_str()) {
        return Err("ingress collaborator identity contains forbidden marker".to_string());
    }
    let mut state = load_ingress_state(&args.state_dir)?;
    if !state
        .collaborator_grants
        .iter()
        .any(|identity| identity == &args.identity)
    {
        state.collaborator_grants.push(args.identity.clone());
    }
    let exposes_preview = state.visibility == "private";
    if exposes_preview {
        state.visibility = "collaborators".to_string();
    }
    push_ingress_receipt(&mut state, "collaborator_granted", args.identity.as_str())?;
    if exposes_preview {
        push_ingress_receipt(&mut state, "preview_exposed", "collaborator_visibility")?;
    }
    save_ingress_state(&args.state_dir, &state)?;
    render_json_or_human(
        args.json,
        &state,
        format!(
            "visibility: {}\ncollaborator_count: {}\nreceipt_count: {}",
            state.visibility,
            state.collaborator_grants.len(),
            state.receipts.len()
        ),
    )
}

fn ingress_token_mint(args: IngressTokenMintArgs) -> Result<String, String> {
    if contains_forbidden_metadata(args.label.as_str()) {
        return Err("ingress token label contains forbidden marker".to_string());
    }
    let mut state = load_ingress_state(&args.state_dir)?;
    let digest = sha256_prefixed(format!("{}:{}", args.label, now_epoch_ms()?).as_bytes());
    state.endpoint_token_digests.push(digest);
    push_ingress_receipt(&mut state, "endpoint_token_minted", args.label.as_str())?;
    save_ingress_state(&args.state_dir, &state)?;
    render_json_or_human(
        args.json,
        &state,
        format!(
            "token_count: {}\nreceipt_count: {}",
            state.endpoint_token_digests.len(),
            state.receipts.len()
        ),
    )
}

fn ingress_revoke(args: IngressRevokeArgs) -> Result<String, String> {
    if contains_forbidden_metadata(args.target.as_str()) {
        return Err("ingress revoke target contains forbidden marker".to_string());
    }
    let mut state = load_ingress_state(&args.state_dir)?;
    state
        .endpoint_token_digests
        .retain(|digest| digest != &args.target);
    state
        .collaborator_grants
        .retain(|identity| identity != &args.target);
    if state.custom_domain.as_deref() == Some(args.target.as_str()) {
        state.custom_domain = None;
    }
    if matches!(
        args.target.as_str(),
        "public" | "collaborators" | "visibility"
    ) {
        state.visibility = "private".to_string();
        state.preview_url = None;
    }
    push_ingress_receipt(&mut state, "ingress_revoked", args.target.as_str())?;
    save_ingress_state(&args.state_dir, &state)?;
    render_json_or_human(
        args.json,
        &state,
        format!(
            "revoked: {}\nreceipt_count: {}",
            args.target,
            state.receipts.len()
        ),
    )
}

fn push_ingress_receipt(
    state: &mut IngressState,
    event_kind: &str,
    detail: &str,
) -> Result<(), String> {
    let occurred_at_ms = now_epoch_ms()?;
    let material = format!(
        "{}:{}:{}:{}:{}",
        INGRESS_STATE_SCHEMA_VERSION, event_kind, detail, state.visibility, occurred_at_ms
    );
    let receipt = IngressReceipt {
        receipt_id: format!("ingress.{event_kind}.{occurred_at_ms}"),
        event_kind: event_kind.to_string(),
        detail: detail.to_string(),
        digest: sha256_prefixed(material.as_bytes()),
        occurred_at_ms,
    };
    validate_ingress_receipt(&receipt)?;
    state.receipts.push(receipt);
    state.updated_at_ms = occurred_at_ms;
    Ok(())
}

fn load_ingress_state(state_dir: &Path) -> Result<IngressState, String> {
    let path = ingress_state_path(state_dir);
    if !path.exists() {
        return default_ingress_state();
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read ingress state {}: {error}",
            path_label(&path)
        )
    })?;
    let state = serde_json::from_str::<IngressState>(&raw).map_err(|error| {
        format!(
            "failed to parse ingress state {}: {error}",
            path_label(&path)
        )
    })?;
    validate_ingress_state(&state)?;
    Ok(state)
}

fn save_ingress_state(state_dir: &Path, state: &IngressState) -> Result<(), String> {
    validate_ingress_state(state)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize ingress state: {error}"))?;
    fs::write(ingress_state_path(state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write ingress state: {error}"))
}

fn default_ingress_state() -> Result<IngressState, String> {
    Ok(IngressState {
        schema_version: INGRESS_STATE_SCHEMA_VERSION.to_string(),
        visibility: "private".to_string(),
        preview_url: None,
        custom_domain: None,
        collaborator_grants: Vec::new(),
        endpoint_token_digests: Vec::new(),
        receipts: Vec::new(),
        updated_at_ms: now_epoch_ms()?,
    })
}

fn validate_ingress_state(state: &IngressState) -> Result<(), String> {
    if state.schema_version != INGRESS_STATE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported ingress schema '{}'",
            state.schema_version
        ));
    }
    validate_visibility(state.visibility.as_str())?;
    for value in [
        Some(state.visibility.as_str()),
        state.preview_url.as_deref(),
        state.custom_domain.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if contains_forbidden_metadata(value) {
            return Err("ingress state contains forbidden marker".to_string());
        }
    }
    if state
        .collaborator_grants
        .iter()
        .any(|identity| contains_forbidden_metadata(identity) || identity.trim().is_empty())
    {
        return Err("ingress state contains forbidden collaborator grant".to_string());
    }
    if state
        .endpoint_token_digests
        .iter()
        .any(|digest| !digest.starts_with("sha256:") || contains_forbidden_metadata(digest))
    {
        return Err("ingress state contains invalid endpoint token digest".to_string());
    }
    for receipt in &state.receipts {
        validate_ingress_receipt(receipt)?;
    }
    Ok(())
}

fn validate_ingress_receipt(receipt: &IngressReceipt) -> Result<(), String> {
    for value in [
        receipt.receipt_id.as_str(),
        receipt.event_kind.as_str(),
        receipt.detail.as_str(),
        receipt.digest.as_str(),
    ] {
        if contains_forbidden_metadata(value) {
            return Err("ingress receipt contains forbidden marker".to_string());
        }
    }
    if !receipt.digest.starts_with("sha256:") {
        return Err("ingress receipt digest must start with sha256:".to_string());
    }
    Ok(())
}

fn validate_visibility(visibility: &str) -> Result<(), String> {
    match visibility {
        "private" | "collaborators" | "public" => Ok(()),
        other => Err(format!("unsupported ingress visibility: {other}")),
    }
}

fn artifact_status(args: ArtifactArgs) -> Result<String, String> {
    let state = load_artifact_state(&args.state_dir)?;
    render_json_or_human(
        args.json,
        &state,
        format!(
            "required_count: {}\nartifact_count: {}\ncloseout: {}",
            state.required_artifacts.len(),
            state.artifacts.len(),
            state
                .closeout
                .as_ref()
                .map(|manifest| manifest.status.as_str())
                .unwrap_or("not_submitted")
        ),
    )
}

fn artifact_policy_init(args: ArtifactPolicyInitArgs) -> Result<String, String> {
    let mut required_artifacts = Vec::new();
    for name in args.required_artifacts {
        validate_artifact_name(name.as_str())?;
        if !required_artifacts.iter().any(|existing| existing == &name) {
            required_artifacts.push(name);
        }
    }
    let mut state = load_artifact_state(&args.state_dir)?;
    state.required_artifacts = required_artifacts;
    state.updated_at_ms = now_epoch_ms()?;
    save_artifact_state(&args.state_dir, &state)?;
    render_json_or_human(
        args.json,
        &state,
        format!(
            "required_count: {}\nartifact_count: {}",
            state.required_artifacts.len(),
            state.artifacts.len()
        ),
    )
}

fn artifact_upload(args: ArtifactUploadArgs) -> Result<String, String> {
    validate_artifact_name(args.name.as_str())?;
    let bytes = fs::read(&args.file).map_err(|error| {
        format!(
            "failed to read artifact file {}: {error}",
            path_label(&args.file)
        )
    })?;
    let content_digest = sha256_prefixed(&bytes);
    let object_path = artifact_object_relative_path(content_digest.as_str())?;
    let object_absolute_path = args.state_dir.join(&object_path);
    if let Some(parent) = object_absolute_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create artifact object dir {}: {error}",
                path_label(parent)
            )
        })?;
    }
    if !object_absolute_path.exists() {
        fs::write(&object_absolute_path, &bytes)
            .map_err(|error| format!("failed to write artifact object: {error}"))?;
    }

    let mut state = load_artifact_state(&args.state_dir)?;
    let uploaded_at_ms = now_epoch_ms()?;
    let record = ArtifactRecord {
        name: args.name.clone(),
        content_digest,
        object_path,
        size_bytes: bytes.len() as u64,
        uploaded_at_ms,
    };
    validate_artifact_record(&record)?;
    state
        .artifacts
        .retain(|artifact| artifact.name != record.name);
    state.artifacts.push(record.clone());
    state.closeout = None;
    state.updated_at_ms = uploaded_at_ms;
    let receipt = build_artifact_receipt(
        "artifact_uploaded",
        record.name.as_str(),
        record.content_digest.as_str(),
        None,
    )?;
    append_artifact_receipt(&args.state_dir, &receipt)?;
    save_artifact_state(&args.state_dir, &state)?;
    let output = ArtifactUploadOutput {
        artifact: record,
        receipt,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "artifact: {}\ncontent_digest: {}\nreceipt_digest: {}",
            output.artifact.name, output.artifact.content_digest, output.receipt.receipt_digest
        ),
    )
}

fn closeout_submit(args: CloseoutArgs) -> Result<String, String> {
    let mut state = load_artifact_state(&args.state_dir)?;
    if state.required_artifacts.is_empty() {
        return Err("closeout blocked; no required artifacts configured".to_string());
    }
    let missing = missing_required_artifacts(&state);
    if !missing.is_empty() {
        return Err(format!(
            "closeout blocked; missing required artifacts: {}",
            missing.join(", ")
        ));
    }

    let receipts = load_artifact_receipts(&args.state_dir)?;
    let mut artifact_digests = Vec::new();
    let mut receipt_digests = Vec::new();
    for required in &state.required_artifacts {
        let artifact = state
            .artifacts
            .iter()
            .find(|artifact| artifact.name == *required)
            .ok_or_else(|| format!("closeout blocked; missing required artifact: {required}"))?;
        let receipt = receipts
            .iter()
            .rev()
            .find(|receipt| {
                receipt.event_kind == "artifact_uploaded"
                    && receipt.artifact_name == artifact.name
                    && receipt.artifact_digest == artifact.content_digest
            })
            .ok_or_else(|| {
                format!(
                    "closeout blocked; missing artifact receipt for required artifact: {required}"
                )
            })?;
        artifact_digests.push(ArtifactDigestRef {
            name: artifact.name.clone(),
            content_digest: artifact.content_digest.clone(),
        });
        receipt_digests.push(receipt.receipt_digest.clone());
    }

    let manifest = build_closeout_manifest(
        state.required_artifacts.clone(),
        artifact_digests,
        receipt_digests,
    )?;
    save_closeout_manifest(&args.state_dir, &manifest)?;
    let receipt = build_artifact_receipt(
        "closeout_submitted",
        "closeout_manifest",
        manifest.manifest_digest.as_str(),
        Some(manifest.manifest_digest.as_str()),
    )?;
    append_artifact_receipt(&args.state_dir, &receipt)?;
    state.closeout = Some(manifest.clone());
    state.updated_at_ms = receipt.emitted_at_ms;
    save_artifact_state(&args.state_dir, &state)?;
    let output = CloseoutOutput { manifest, receipt };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "status: {}\nmanifest_digest: {}\nreceipt_digest: {}",
            output.manifest.status, output.manifest.manifest_digest, output.receipt.receipt_digest
        ),
    )
}

fn build_closeout_manifest(
    required_artifacts: Vec<String>,
    artifact_digests: Vec<ArtifactDigestRef>,
    receipt_digests: Vec<String>,
) -> Result<CloseoutManifest, String> {
    let created_at_ms = now_epoch_ms()?;
    let manifest_id = format!("closeout.manifest.{created_at_ms}");
    let status = "submitted".to_string();
    let encoded = serde_json::to_vec(&(
        CLOSEOUT_MANIFEST_SCHEMA_VERSION,
        manifest_id.as_str(),
        &required_artifacts,
        &artifact_digests,
        &receipt_digests,
        status.as_str(),
        created_at_ms,
    ))
    .map_err(|error| format!("failed to serialize closeout manifest digest material: {error}"))?;
    let manifest = CloseoutManifest {
        schema_version: CLOSEOUT_MANIFEST_SCHEMA_VERSION.to_string(),
        manifest_id,
        required_artifacts,
        artifact_digests,
        receipt_digests,
        status,
        created_at_ms,
        manifest_digest: sha256_prefixed(&encoded),
    };
    validate_closeout_manifest(&manifest)?;
    Ok(manifest)
}

fn missing_required_artifacts(state: &ArtifactState) -> Vec<String> {
    state
        .required_artifacts
        .iter()
        .filter(|required| {
            !state
                .artifacts
                .iter()
                .any(|artifact| artifact.name == **required)
        })
        .cloned()
        .collect()
}

fn build_artifact_receipt(
    event_kind: &str,
    artifact_name: &str,
    artifact_digest: &str,
    manifest_digest: Option<&str>,
) -> Result<ArtifactReceipt, String> {
    validate_artifact_name(artifact_name)?;
    if !artifact_digest.starts_with("sha256:") {
        return Err("artifact receipt digest reference must start with sha256:".to_string());
    }
    let emitted_at_ms = now_epoch_ms()?;
    let encoded = serde_json::to_vec(&(
        ARTIFACT_RECEIPT_SCHEMA_VERSION,
        event_kind,
        artifact_name,
        artifact_digest,
        manifest_digest,
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize artifact receipt digest material: {error}"))?;
    let receipt = ArtifactReceipt {
        schema_version: ARTIFACT_RECEIPT_SCHEMA_VERSION.to_string(),
        receipt_id: format!("artifact.{event_kind}.{artifact_name}.{emitted_at_ms}"),
        event_kind: event_kind.to_string(),
        artifact_name: artifact_name.to_string(),
        artifact_digest: artifact_digest.to_string(),
        manifest_digest: manifest_digest.map(str::to_string),
        receipt_digest: sha256_prefixed(&encoded),
        emitted_at_ms,
    };
    validate_artifact_receipt(&receipt)?;
    Ok(receipt)
}

fn append_artifact_receipt(state_dir: &Path, receipt: &ArtifactReceipt) -> Result<(), String> {
    validate_artifact_receipt(receipt)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize artifact receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(artifact_receipt_log_path(state_dir))
        .map_err(|error| format!("failed to open artifact receipt log: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to append artifact receipt: {error}"))
}

fn load_artifact_state(state_dir: &Path) -> Result<ArtifactState, String> {
    let path = artifact_state_path(state_dir);
    if !path.exists() {
        return default_artifact_state();
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read artifact state {}: {error}",
            path_label(&path)
        )
    })?;
    let state = serde_json::from_str::<ArtifactState>(&raw).map_err(|error| {
        format!(
            "failed to parse artifact state {}: {error}",
            path_label(&path)
        )
    })?;
    validate_artifact_state(&state)?;
    Ok(state)
}

fn save_artifact_state(state_dir: &Path, state: &ArtifactState) -> Result<(), String> {
    validate_artifact_state(state)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize artifact state: {error}"))?;
    fs::write(artifact_state_path(state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write artifact state: {error}"))
}

fn default_artifact_state() -> Result<ArtifactState, String> {
    Ok(ArtifactState {
        schema_version: ARTIFACT_STATE_SCHEMA_VERSION.to_string(),
        required_artifacts: Vec::new(),
        artifacts: Vec::new(),
        closeout: None,
        updated_at_ms: now_epoch_ms()?,
    })
}

fn load_artifact_receipts(state_dir: &Path) -> Result<Vec<ArtifactReceipt>, String> {
    let path = artifact_receipt_log_path(state_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read artifact receipt log {}: {error}",
            path_label(&path)
        )
    })?;
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let receipt = serde_json::from_str::<ArtifactReceipt>(line)
                .map_err(|error| format!("failed to parse artifact receipt: {error}"))?;
            validate_artifact_receipt(&receipt)?;
            Ok(receipt)
        })
        .collect()
}

fn save_closeout_manifest(state_dir: &Path, manifest: &CloseoutManifest) -> Result<(), String> {
    validate_closeout_manifest(manifest)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("failed to serialize closeout manifest: {error}"))?;
    fs::write(closeout_manifest_path(state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write closeout manifest: {error}"))
}

fn validate_artifact_state(state: &ArtifactState) -> Result<(), String> {
    if state.schema_version != ARTIFACT_STATE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported artifact state schema '{}'",
            state.schema_version
        ));
    }
    for required in &state.required_artifacts {
        validate_artifact_name(required)?;
    }
    for artifact in &state.artifacts {
        validate_artifact_record(artifact)?;
    }
    if let Some(manifest) = &state.closeout {
        validate_closeout_manifest(manifest)?;
    }
    Ok(())
}

fn validate_artifact_record(record: &ArtifactRecord) -> Result<(), String> {
    validate_artifact_name(record.name.as_str())?;
    if !record.content_digest.starts_with("sha256:") {
        return Err("artifact content_digest must start with sha256:".to_string());
    }
    if record.object_path.trim().is_empty()
        || !record.object_path.starts_with(ARTIFACT_OBJECT_ROOT)
        || contains_forbidden_metadata(record.object_path.as_str())
    {
        return Err("artifact object_path is invalid".to_string());
    }
    Ok(())
}

fn validate_artifact_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("artifact name must not be empty".to_string());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("artifact name must be a bounded name, not a path".to_string());
    }
    if contains_forbidden_metadata(name) {
        return Err("artifact name contains forbidden marker".to_string());
    }
    Ok(())
}

fn validate_artifact_receipt(receipt: &ArtifactReceipt) -> Result<(), String> {
    if receipt.schema_version != ARTIFACT_RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported artifact receipt schema '{}'",
            receipt.schema_version
        ));
    }
    validate_artifact_name(receipt.artifact_name.as_str())?;
    for value in [
        receipt.receipt_id.as_str(),
        receipt.event_kind.as_str(),
        receipt.artifact_digest.as_str(),
        receipt.receipt_digest.as_str(),
    ] {
        if contains_forbidden_metadata(value) {
            return Err("artifact receipt contains forbidden marker".to_string());
        }
    }
    if !receipt.artifact_digest.starts_with("sha256:")
        || !receipt.receipt_digest.starts_with("sha256:")
    {
        return Err("artifact receipt digests must start with sha256:".to_string());
    }
    if let Some(manifest_digest) = &receipt.manifest_digest {
        if !manifest_digest.starts_with("sha256:") {
            return Err("manifest digest must start with sha256:".to_string());
        }
    }
    Ok(())
}

fn validate_closeout_manifest(manifest: &CloseoutManifest) -> Result<(), String> {
    if manifest.schema_version != CLOSEOUT_MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported closeout manifest schema '{}'",
            manifest.schema_version
        ));
    }
    for required in &manifest.required_artifacts {
        validate_artifact_name(required)?;
    }
    for artifact in &manifest.artifact_digests {
        validate_artifact_name(artifact.name.as_str())?;
        if !artifact.content_digest.starts_with("sha256:") {
            return Err("closeout artifact digest must start with sha256:".to_string());
        }
    }
    if manifest
        .receipt_digests
        .iter()
        .any(|digest| !digest.starts_with("sha256:") || contains_forbidden_metadata(digest))
    {
        return Err("closeout receipt digest is invalid".to_string());
    }
    if manifest.status != "submitted" {
        return Err(format!("unsupported closeout status '{}'", manifest.status));
    }
    if !manifest.manifest_digest.starts_with("sha256:") {
        return Err("closeout manifest digest must start with sha256:".to_string());
    }
    Ok(())
}

fn artifact_object_relative_path(content_digest: &str) -> Result<String, String> {
    let digest = content_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| "artifact digest must start with sha256:".to_string())?;
    Ok(format!("{ARTIFACT_OBJECT_ROOT}/{digest}"))
}

fn lifecycle_status(args: LifecycleArgs) -> Result<String, String> {
    let state = load_lifecycle_state(&args.state_dir)?;
    render_json_or_human(
        args.json,
        &state,
        format!(
            "state: {}\nreceipt_count: {}",
            state.state,
            state.receipts.len()
        ),
    )
}

fn lifecycle_action(args: LifecycleActionArgs) -> Result<String, String> {
    let mut state = load_lifecycle_state(&args.state_dir)?;
    let from_state = state.state.clone();
    let to_state = next_lifecycle_state(from_state.as_str(), args.action.as_str())?;
    if args.action == "closeout" || args.action == "destroy" {
        let closeout = closeout_policy_status(&args.state_dir)?;
        if closeout.required && !closeout.satisfied {
            return Err(format!(
                "lifecycle {} blocked; required closeout policy is not satisfied",
                args.action
            ));
        }
    }
    let receipt = build_lifecycle_receipt(args.action.as_str(), from_state.as_str(), to_state)?;
    state.state = to_state.to_string();
    state.updated_at_ms = receipt.emitted_at_ms;
    state.receipts.push(receipt.clone());
    save_lifecycle_state(&args.state_dir, &state)?;
    append_lifecycle_receipt(&args.state_dir, &receipt)?;
    let output = LifecycleActionOutput { state, receipt };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "state: {}\naction: {}\nreceipt_digest: {}",
            output.state.state, output.receipt.action, output.receipt.receipt_digest
        ),
    )
}

fn next_lifecycle_state(from_state: &str, action: &str) -> Result<&'static str, String> {
    match (from_state, action) {
        ("not_created", "create") => Ok("created"),
        ("created", "start") | ("paused", "start") | ("paused", "resume") => Ok("running"),
        ("running", "pause") | ("exposed", "pause") => Ok("paused"),
        ("running", "expose") => Ok("exposed"),
        ("running", "closeout") | ("paused", "closeout") | ("exposed", "closeout") => {
            Ok("closed_out")
        }
        ("closed_out", "archive") => Ok("archived"),
        ("closed_out", "destroy") | ("archived", "destroy") => Ok("destroyed"),
        ("destroyed", _) => Err("lifecycle state destroyed is terminal".to_string()),
        _ => Err(format!(
            "invalid lifecycle transition: action {action} from {from_state}"
        )),
    }
}

struct CloseoutPolicyStatus {
    required: bool,
    satisfied: bool,
}

fn closeout_policy_status(state_dir: &Path) -> Result<CloseoutPolicyStatus, String> {
    let artifact_state = load_artifact_state(state_dir)?;
    let required = !artifact_state.required_artifacts.is_empty();
    let satisfied = !required
        || artifact_state
            .closeout
            .as_ref()
            .is_some_and(|manifest| manifest.status == "submitted");
    Ok(CloseoutPolicyStatus {
        required,
        satisfied,
    })
}

fn build_lifecycle_receipt(
    action: &str,
    from_state: &str,
    to_state: &str,
) -> Result<LifecycleReceipt, String> {
    validate_lifecycle_label(action, "action")?;
    validate_lifecycle_label(from_state, "from_state")?;
    validate_lifecycle_label(to_state, "to_state")?;
    let emitted_at_ms = now_epoch_ms()?;
    let encoded = serde_json::to_vec(&(
        LIFECYCLE_RECEIPT_SCHEMA_VERSION,
        action,
        from_state,
        to_state,
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize lifecycle receipt digest material: {error}"))?;
    let receipt = LifecycleReceipt {
        schema_version: LIFECYCLE_RECEIPT_SCHEMA_VERSION.to_string(),
        receipt_id: format!("lifecycle.{action}.{from_state}.{to_state}.{emitted_at_ms}"),
        action: action.to_string(),
        from_state: from_state.to_string(),
        to_state: to_state.to_string(),
        receipt_digest: sha256_prefixed(&encoded),
        emitted_at_ms,
    };
    validate_lifecycle_receipt(&receipt)?;
    Ok(receipt)
}

fn append_lifecycle_receipt(state_dir: &Path, receipt: &LifecycleReceipt) -> Result<(), String> {
    validate_lifecycle_receipt(receipt)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize lifecycle receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(lifecycle_receipt_log_path(state_dir))
        .map_err(|error| format!("failed to open lifecycle receipt log: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to append lifecycle receipt: {error}"))
}

fn load_lifecycle_state(state_dir: &Path) -> Result<LifecycleState, String> {
    let path = lifecycle_state_path(state_dir);
    if !path.exists() {
        return default_lifecycle_state();
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read lifecycle state {}: {error}",
            path_label(&path)
        )
    })?;
    let state = serde_json::from_str::<LifecycleState>(&raw).map_err(|error| {
        format!(
            "failed to parse lifecycle state {}: {error}",
            path_label(&path)
        )
    })?;
    validate_lifecycle_state(&state)?;
    Ok(state)
}

fn save_lifecycle_state(state_dir: &Path, state: &LifecycleState) -> Result<(), String> {
    validate_lifecycle_state(state)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize lifecycle state: {error}"))?;
    fs::write(lifecycle_state_path(state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write lifecycle state: {error}"))
}

fn default_lifecycle_state() -> Result<LifecycleState, String> {
    Ok(LifecycleState {
        schema_version: LIFECYCLE_STATE_SCHEMA_VERSION.to_string(),
        state: "not_created".to_string(),
        receipts: Vec::new(),
        updated_at_ms: now_epoch_ms()?,
    })
}

fn validate_lifecycle_state(state: &LifecycleState) -> Result<(), String> {
    if state.schema_version != LIFECYCLE_STATE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported lifecycle state schema '{}'",
            state.schema_version
        ));
    }
    validate_lifecycle_label(state.state.as_str(), "state")?;
    match state.state.as_str() {
        "not_created" | "created" | "running" | "paused" | "exposed" | "closed_out"
        | "archived" | "destroyed" => {}
        other => return Err(format!("unsupported lifecycle state: {other}")),
    }
    for receipt in &state.receipts {
        validate_lifecycle_receipt(receipt)?;
    }
    Ok(())
}

fn validate_lifecycle_receipt(receipt: &LifecycleReceipt) -> Result<(), String> {
    if receipt.schema_version != LIFECYCLE_RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported lifecycle receipt schema '{}'",
            receipt.schema_version
        ));
    }
    for (value, label) in [
        (receipt.receipt_id.as_str(), "receipt_id"),
        (receipt.action.as_str(), "action"),
        (receipt.from_state.as_str(), "from_state"),
        (receipt.to_state.as_str(), "to_state"),
        (receipt.receipt_digest.as_str(), "receipt_digest"),
    ] {
        validate_lifecycle_label(value, label)?;
    }
    if !receipt.receipt_digest.starts_with("sha256:") {
        return Err("lifecycle receipt digest must start with sha256:".to_string());
    }
    Ok(())
}

fn validate_lifecycle_label(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("lifecycle {label} must not be empty"));
    }
    if contains_forbidden_metadata(value) {
        return Err(format!("lifecycle {label} contains forbidden marker"));
    }
    Ok(())
}

fn codex_auth_materialize(args: CodexAuthMaterializeArgs) -> Result<String, String> {
    let now = now_epoch_ms()?;
    let grant = load_codex_auth_grant(&args.grant_file)?;
    grant.validate_for_session(now)?;
    let auth_json = fs::read(&args.auth_json_file).map_err(|error| {
        format!(
            "failed to read codex auth material {}: {error}",
            path_label(&args.auth_json_file)
        )
    })?;
    if auth_json.is_empty() {
        return Err("codex auth material is empty".to_string());
    }

    let codex_home = codex_auth_home(&args.state_dir, grant.grant_ref.as_str());
    fs::create_dir_all(&codex_home).map_err(|error| {
        format!(
            "failed to create session CODEX_HOME {}: {error}",
            path_label(&codex_home)
        )
    })?;
    let auth_json_path = codex_home.join("auth.json");
    fs::write(&auth_json_path, &auth_json)
        .map_err(|error| format!("failed to write session codex auth file: {error}"))?;
    set_owner_only_permissions(&auth_json_path)?;

    let auth_json_digest = sha256_prefixed(&auth_json);
    let receipt = build_codex_auth_receipt(
        &grant,
        CodexAuthReceiptKind::GrantMaterialized,
        CodexAuthDecision::Accepted,
        "session_codex_home_created",
        codex_home.as_path(),
    )?;
    let state = CodexAuthState {
        schema_version: CODEX_AUTH_STATE_SCHEMA_VERSION.to_string(),
        grant,
        codex_home: path_label(&codex_home),
        auth_json_path: path_label(&auth_json_path),
        auth_json_digest,
        login_status: None,
        receipts: vec![receipt.clone()],
        materialized_at_ms: now,
        status_checked_at_ms: None,
        scrubbed_at_ms: None,
        updated_at_ms: now,
    };
    save_codex_auth_state(&args.state_dir, &state)?;
    append_codex_auth_receipt(&args.state_dir, &receipt)?;
    let output = CodexAuthActionOutput { state, receipt };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "codex_home_digest: {}\nauth_json_digest: {}\nreceipt_digest: {}",
            output.receipt.codex_home_digest,
            output.state.auth_json_digest,
            output.receipt.receipt_digest
        ),
    )
}

fn codex_auth_status(args: CodexAuthStatusArgs) -> Result<String, String> {
    let mut state = load_codex_auth_state(&args.state_dir)?;
    let now = now_epoch_ms()?;
    let (decision, reason) = if state.grant.expires_at_ms <= now {
        (
            CodexAuthDecision::Failed,
            "codex_auth_grant_expired".to_string(),
        )
    } else if state.scrubbed_at_ms.is_some() {
        (
            CodexAuthDecision::Failed,
            "codex_auth_material_scrubbed".to_string(),
        )
    } else {
        run_codex_login_status(&args.codex_bin, state.codex_home.as_str())
    };
    let receipt = build_codex_auth_receipt(
        &state.grant,
        CodexAuthReceiptKind::LoginStatusChecked,
        decision.clone(),
        reason.as_str(),
        Path::new(state.codex_home.as_str()),
    )?;
    state.login_status = Some(reason.clone());
    state.status_checked_at_ms = Some(now);
    state.updated_at_ms = now;
    state.receipts.push(receipt.clone());
    save_codex_auth_state(&args.state_dir, &state)?;
    append_codex_auth_receipt(&args.state_dir, &receipt)?;
    let output = CodexAuthActionOutput { state, receipt };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "decision: {:?}\nreason: {}\nreceipt_digest: {}",
            decision, reason, output.receipt.receipt_digest
        ),
    )
}

fn codex_auth_scrub(args: CodexAuthScrubArgs) -> Result<String, String> {
    let (state, receipt, reason) = scrub_codex_auth_state(&args.state_dir)?;
    let output = CodexAuthActionOutput { state, receipt };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "scrubbed: true\nreason: {reason}\nreceipt_digest: {}",
            output.receipt.receipt_digest
        ),
    )
}

fn codex_session_create(args: CodexSessionCreateArgs) -> Result<String, String> {
    let now = now_epoch_ms()?;
    let assignment = load_codex_workroom_assignment(&args.assignment_file)?;
    assignment.validate_contract(now)?;
    fs::create_dir_all(&args.state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(&args.state_dir)
        )
    })?;
    let workspace_dir = codex_workspace_dir(&args.state_dir, assignment.assignment_id.as_str());
    fs::create_dir_all(&workspace_dir)
        .map_err(|error| format!("failed to create codex session workspace: {error}"))?;
    let mut session = CodexSessionState {
        schema_version: CODEX_SESSION_SCHEMA_VERSION.to_string(),
        assignment,
        workspace_dir: path_label(&workspace_dir),
        status: "created".to_string(),
        turn_index: 0,
        artifact_refs: Vec::new(),
        receipt_refs: Vec::new(),
        events: Vec::new(),
        created_at_ms: now,
        updated_at_ms: now,
        archived_at_ms: None,
        destroyed_at_ms: None,
        expires_at_ms: args.ttl_ms.map(|ttl| now + ttl),
    };
    push_codex_session_event(
        &args.state_dir,
        &mut session,
        CodexWorkroomEventKind::Queued,
        CodexWorkroomDecision::Accepted,
        "codex session created and workspace reserved",
        None,
        None,
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "run.queued",
        "runner",
        "Codex session created and queued.",
        None,
        Vec::new(),
        Vec::new(),
    )?;
    save_codex_session_state(&args.state_dir, &session)?;
    render_json_or_human(
        args.json,
        &CodexSessionOutput {
            session,
            auth_receipts: Vec::new(),
            runner_events: load_openagents_runner_events(&args.state_dir)?,
        },
        "codex session created".to_string(),
    )
}

fn codex_session_turn(args: CodexSessionTurnArgs) -> Result<String, String> {
    let mut session = load_codex_session_state(&args.state_dir)?;
    ensure_session_can_run(&session)?;
    if let Some(prompt) = &args.prompt {
        if contains_forbidden_metadata(prompt.as_str()) {
            return Err("codex session prompt contains forbidden marker".to_string());
        }
        session.assignment.prompt = prompt.clone();
        session.assignment.created_at_ms = now_epoch_ms()?;
        session
            .assignment
            .validate_contract(session.assignment.created_at_ms)?;
    }

    let materialized = codex_auth_materialize(CodexAuthMaterializeArgs {
        state_dir: args.state_dir.clone(),
        grant_file: args.grant_file.clone(),
        auth_json_file: args.auth_json_file.clone(),
        json: true,
    })?;
    let materialized_output: CodexAuthActionOutput = serde_json::from_str(&materialized)
        .map_err(|error| format!("failed to parse codex auth materialize output: {error}"))?;
    session.assignment.auth_grant_ref = materialized_output.state.grant.grant_ref.clone();
    session.assignment.provider_account_ref =
        materialized_output.state.grant.provider_account_ref.clone();
    session.assignment.validate_contract(now_epoch_ms()?)?;
    let mut auth_receipts = vec![materialized_output.receipt];

    session.status = "running".to_string();
    session.turn_index += 1;
    let started_message = format!("codex session turn {} started", session.turn_index);
    push_codex_session_event(
        &args.state_dir,
        &mut session,
        CodexWorkroomEventKind::Started,
        CodexWorkroomDecision::Accepted,
        started_message.as_str(),
        None,
        None,
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "turn.started",
        "runner",
        "Codex session turn started.",
        Some(started_message.as_str()),
        Vec::new(),
        Vec::new(),
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "run.heartbeat",
        "runner",
        "Codex session runner heartbeat recorded.",
        Some(started_message.as_str()),
        Vec::new(),
        Vec::new(),
    )?;
    save_codex_session_state(&args.state_dir, &session)?;

    let turn_result = codex_session_turn_inner(&args, &mut session, &mut auth_receipts);
    let scrub_result = scrub_codex_auth_state(&args.state_dir).map(|(_, receipt, _)| receipt);
    match scrub_result {
        Ok(receipt) => auth_receipts.push(receipt),
        Err(error) => {
            push_codex_session_event(
                &args.state_dir,
                &mut session,
                CodexWorkroomEventKind::Cleanup,
                CodexWorkroomDecision::Failed,
                redacted_error("codex auth scrub failed", error.as_str()).as_str(),
                None,
                None,
            )?;
            if turn_result.is_ok() {
                session.status = "failed".to_string();
            }
        }
    }

    if let Err(error) = turn_result {
        if session.status != "timeout" {
            session.status = "failed".to_string();
        }
        push_codex_session_event(
            &args.state_dir,
            &mut session,
            CodexWorkroomEventKind::Failed,
            CodexWorkroomDecision::Failed,
            redacted_error("codex session turn failed", error.as_str()).as_str(),
            None,
            None,
        )?;
        append_openagents_runner_event(
            &args.state_dir,
            session.assignment.assignment_id.as_str(),
            session.assignment.workroom_id.as_str(),
            if session.status == "timeout" {
                "run.timed_out"
            } else {
                "run.failed"
            },
            "runner",
            "Codex session turn failed.",
            Some(redacted_error("turn failed", error.as_str()).as_str()),
            Vec::new(),
            Vec::new(),
        )?;
        save_codex_session_state(&args.state_dir, &session)?;
        return Err(error);
    }

    if session.status != "failed" {
        session.status = "idle".to_string();
    }
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "run.waiting_for_input",
        "runner",
        "Codex session turn completed and is waiting for input.",
        None,
        Vec::new(),
        Vec::new(),
    )?;
    save_codex_session_state(&args.state_dir, &session)?;
    let output = CodexSessionOutput {
        session: session.clone(),
        auth_receipts,
        runner_events: load_openagents_runner_events(&args.state_dir)?,
    };
    if args.stream_jsonl {
        render_session_events_jsonl(&session.events)
    } else {
        render_json_or_human(
            args.json,
            &output,
            format!(
                "status: {}\nturn_index: {}\nevent_count: {}",
                output.session.status,
                output.session.turn_index,
                output.session.events.len()
            ),
        )
    }
}

fn codex_session_turn_inner(
    args: &CodexSessionTurnArgs,
    session: &mut CodexSessionState,
    auth_receipts: &mut Vec<CodexAuthReceipt>,
) -> Result<(), String> {
    let status_receipt =
        verify_codex_auth_for_run(&args.state_dir, &args.codex_bin, &session.assignment)?;
    auth_receipts.push(status_receipt);
    let workspace_dir = PathBuf::from(session.workspace_dir.clone());
    prepare_codex_workspace(&args.state_dir, &workspace_dir, &session.assignment)?;
    let auth_state = load_codex_auth_state(&args.state_dir)?;
    let execution = run_codex_exec(
        &args.state_dir,
        &args.codex_bin,
        &workspace_dir,
        auth_state.codex_home.as_str(),
        &session.assignment,
    )?;
    record_codex_session_output_events(&args.state_dir, session, "stdout", &execution.stdout)?;
    record_codex_session_output_events(&args.state_dir, session, "stderr", &execution.stderr)?;

    let usage_receipt = append_resource_usage_receipt(
        &args.state_dir,
        &session.assignment,
        &workspace_dir,
        &execution,
    )?;
    session
        .receipt_refs
        .push(usage_receipt.receipt_digest.clone());
    push_codex_session_event(
        &args.state_dir,
        session,
        CodexWorkroomEventKind::Receipt,
        CodexWorkroomDecision::Accepted,
        "codex session resource usage receipt emitted",
        None,
        Some(usage_receipt.receipt_digest.as_str()),
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "resource.usage.captured",
        "runner",
        "Resource and model usage receipt captured.",
        None,
        Vec::new(),
        vec![usage_receipt.receipt_digest.clone()],
    )?;
    if observed_codex_token_usage(&execution).is_none() {
        append_openagents_runner_event(
            &args.state_dir,
            session.assignment.assignment_id.as_str(),
            session.assignment.workroom_id.as_str(),
            "usage.unavailable",
            "runner",
            "Subscription-backed Codex usage metrics were unavailable.",
            None,
            Vec::new(),
            vec![usage_receipt.receipt_digest.clone()],
        )?;
    }
    if execution.artifact_completed {
        append_openagents_runner_event(
            &args.state_dir,
            session.assignment.assignment_id.as_str(),
            session.assignment.workroom_id.as_str(),
            "artifact_set.completed",
            "runner",
            "Required artifact set completed before Codex process exit.",
            None,
            Vec::new(),
            Vec::new(),
        )?;
    }

    if execution.timed_out {
        session.status = "timeout".to_string();
        push_codex_session_event(
            &args.state_dir,
            session,
            CodexWorkroomEventKind::Timeout,
            CodexWorkroomDecision::Failed,
            "codex session turn timeout",
            None,
            None,
        )?;
        return Err("codex exec timed out".to_string());
    }
    if execution.exit_code != Some(0) {
        session.status = "failed".to_string();
        push_codex_session_event(
            &args.state_dir,
            session,
            CodexWorkroomEventKind::Failed,
            CodexWorkroomDecision::Failed,
            format!(
                "codex session turn failed exit {}",
                execution.exit_code.unwrap_or(-1)
            )
            .as_str(),
            None,
            None,
        )?;
        return Err(format!(
            "codex exec failed exit {}",
            execution.exit_code.unwrap_or(-1)
        ));
    }

    push_codex_session_event(
        &args.state_dir,
        session,
        CodexWorkroomEventKind::Completed,
        CodexWorkroomDecision::Accepted,
        format!("codex session turn {} completed", session.turn_index).as_str(),
        None,
        None,
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "message.completed",
        "runner",
        "Codex session turn completed.",
        None,
        Vec::new(),
        Vec::new(),
    )?;
    Ok(())
}

fn codex_session_cancel(args: CodexSessionCancelArgs) -> Result<String, String> {
    let mut session = load_codex_session_state(&args.state_dir)?;
    let message = redacted_error("codex session turn canceled", args.reason.as_str());
    session.status = "canceled".to_string();
    push_codex_session_event(
        &args.state_dir,
        &mut session,
        CodexWorkroomEventKind::Failed,
        CodexWorkroomDecision::Failed,
        message.as_str(),
        None,
        None,
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "run.cancelled",
        "runner",
        "Codex session turn was cancelled.",
        Some(message.as_str()),
        Vec::new(),
        Vec::new(),
    )?;
    save_codex_session_state(&args.state_dir, &session)?;
    render_json_or_human(
        args.json,
        &CodexSessionOutput {
            session,
            auth_receipts: Vec::new(),
            runner_events: load_openagents_runner_events(&args.state_dir)?,
        },
        "codex session turn canceled".to_string(),
    )
}

fn codex_session_status(args: CodexSessionStatusArgs) -> Result<String, String> {
    let session = load_codex_session_state(&args.state_dir)?;
    render_json_or_human(
        args.json,
        &CodexSessionOutput {
            session: session.clone(),
            auth_receipts: Vec::new(),
            runner_events: load_openagents_runner_events(&args.state_dir)?,
        },
        format!(
            "status: {}\nturn_index: {}\nworkspace: {}",
            session.status, session.turn_index, session.workspace_dir
        ),
    )
}

fn codex_session_events(args: CodexSessionEventsArgs) -> Result<String, String> {
    let session = load_codex_session_state(&args.state_dir)?;
    let events: Vec<_> = session
        .events
        .iter()
        .filter(|event| event.sequence > args.cursor)
        .cloned()
        .collect();
    if args.stream_jsonl {
        return render_session_events_jsonl(&events);
    }
    let next_cursor = events
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap_or(args.cursor);
    render_json_or_human(
        args.json,
        &CodexSessionEventsOutput {
            events,
            next_cursor,
            runner_events: load_openagents_runner_events(&args.state_dir)?,
            session_status: session.status,
        },
        format!("next_cursor: {next_cursor}"),
    )
}

fn codex_session_closeout(args: CodexSessionSimpleArgs) -> Result<String, String> {
    let mut session = load_codex_session_state(&args.state_dir)?;
    ensure_session_not_destroyed(&session)?;
    let workspace_dir = PathBuf::from(session.workspace_dir.clone());
    artifact_policy_init(ArtifactPolicyInitArgs {
        state_dir: args.state_dir.clone(),
        required_artifacts: session.assignment.required_artifacts.clone(),
        json: true,
    })?;
    for name in session.assignment.required_artifacts.clone() {
        let artifact_path = workspace_dir.join(&name);
        if !artifact_path.exists() {
            session.status = "failed".to_string();
            push_codex_session_event(
                &args.state_dir,
                &mut session,
                CodexWorkroomEventKind::Failed,
                CodexWorkroomDecision::Failed,
                format!("missing required artifact {name}").as_str(),
                None,
                None,
            )?;
            save_codex_session_state(&args.state_dir, &session)?;
            return Err(format!("missing required artifact {name}"));
        }
        let uploaded = artifact_upload(ArtifactUploadArgs {
            state_dir: args.state_dir.clone(),
            name: name.clone(),
            file: artifact_path,
            json: true,
        })?;
        let parsed: serde_json::Value = serde_json::from_str(&uploaded)
            .map_err(|error| format!("failed to parse artifact upload output: {error}"))?;
        let content_digest = parsed
            .pointer("/artifact/content_digest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "artifact upload output missing content_digest".to_string())?
            .to_string();
        let object_path = parsed
            .pointer("/artifact/object_path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "artifact upload output missing object_path".to_string())?
            .to_string();
        let receipt_digest = parsed
            .pointer("/receipt/receipt_digest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "artifact upload output missing receipt_digest".to_string())?
            .to_string();
        session.artifact_refs.push(CodexRunArtifactRef {
            name: name.clone(),
            content_digest: content_digest.clone(),
            object_path,
        });
        session.receipt_refs.push(receipt_digest.clone());
        push_codex_session_event(
            &args.state_dir,
            &mut session,
            CodexWorkroomEventKind::Artifact,
            CodexWorkroomDecision::Accepted,
            "session artifact captured",
            Some(content_digest.as_str()),
            Some(receipt_digest.as_str()),
        )?;
        append_openagents_runner_event(
            &args.state_dir,
            session.assignment.assignment_id.as_str(),
            session.assignment.workroom_id.as_str(),
            "artifact.created",
            "runner",
            "Session artifact captured.",
            Some(name.as_str()),
            vec![content_digest.clone()],
            vec![receipt_digest.clone()],
        )?;
    }
    let closeout = closeout_submit(CloseoutArgs {
        state_dir: args.state_dir.clone(),
        json: true,
    })?;
    let parsed: serde_json::Value = serde_json::from_str(&closeout)
        .map_err(|error| format!("failed to parse closeout output: {error}"))?;
    let closeout_receipt = parsed
        .pointer("/receipt/receipt_digest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "closeout output missing receipt_digest".to_string())?
        .to_string();
    session.receipt_refs.push(closeout_receipt.clone());
    session.status = "completed".to_string();
    push_codex_session_event(
        &args.state_dir,
        &mut session,
        CodexWorkroomEventKind::Receipt,
        CodexWorkroomDecision::Accepted,
        "session closeout receipt emitted",
        None,
        Some(closeout_receipt.as_str()),
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "receipt.created",
        "runner",
        "Session closeout receipt emitted.",
        None,
        Vec::new(),
        vec![closeout_receipt.clone()],
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "run.completed",
        "runner",
        "Codex session closeout completed.",
        None,
        Vec::new(),
        vec![closeout_receipt.clone()],
    )?;
    save_codex_session_state(&args.state_dir, &session)?;
    render_json_or_human(
        args.json,
        &CodexSessionOutput {
            session,
            auth_receipts: Vec::new(),
            runner_events: load_openagents_runner_events(&args.state_dir)?,
        },
        "codex session closeout completed".to_string(),
    )
}

fn codex_session_archive(args: CodexSessionSimpleArgs) -> Result<String, String> {
    let mut session = load_codex_session_state(&args.state_dir)?;
    ensure_session_not_destroyed(&session)?;
    session.status = "archived".to_string();
    session.archived_at_ms = Some(now_epoch_ms()?);
    push_codex_session_event(
        &args.state_dir,
        &mut session,
        CodexWorkroomEventKind::Cleanup,
        CodexWorkroomDecision::Accepted,
        "codex session archived for retention policy",
        None,
        None,
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "run.completed",
        "runner",
        "Codex session archived.",
        None,
        Vec::new(),
        Vec::new(),
    )?;
    save_codex_session_state(&args.state_dir, &session)?;
    render_json_or_human(
        args.json,
        &CodexSessionOutput {
            session,
            auth_receipts: Vec::new(),
            runner_events: load_openagents_runner_events(&args.state_dir)?,
        },
        "codex session archived".to_string(),
    )
}

fn codex_session_destroy(args: CodexSessionSimpleArgs) -> Result<String, String> {
    let mut session = load_codex_session_state(&args.state_dir)?;
    let workspace_dir = PathBuf::from(session.workspace_dir.clone());
    cleanup_codex_workspace(&workspace_dir)?;
    let auth_root = args.state_dir.join(CODEX_AUTH_HOME_ROOT);
    if auth_root.exists() {
        fs::remove_dir_all(&auth_root)
            .map_err(|error| format!("failed to remove codex auth root: {error}"))?;
    }
    session.status = "destroyed".to_string();
    session.destroyed_at_ms = Some(now_epoch_ms()?);
    push_codex_session_event(
        &args.state_dir,
        &mut session,
        CodexWorkroomEventKind::Cleanup,
        CodexWorkroomDecision::Accepted,
        "codex session workspace and auth roots destroyed",
        None,
        None,
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        "run.completed",
        "runner",
        "Codex session destroyed.",
        None,
        Vec::new(),
        Vec::new(),
    )?;
    save_codex_session_state(&args.state_dir, &session)?;
    render_json_or_human(
        args.json,
        &CodexSessionOutput {
            session,
            auth_receipts: Vec::new(),
            runner_events: load_openagents_runner_events(&args.state_dir)?,
        },
        "codex session destroyed".to_string(),
    )
}

fn codex_run(args: CodexRunArgs) -> Result<String, String> {
    let now = now_epoch_ms()?;
    let assignment = load_codex_workroom_assignment(&args.assignment_file)?;
    assignment.validate_contract(now)?;
    fs::create_dir_all(&args.state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(&args.state_dir)
        )
    })?;

    let workspace_dir = codex_workspace_dir(&args.state_dir, assignment.assignment_id.as_str());
    let mut state = CodexRunState {
        schema_version: CODEX_RUN_STATE_SCHEMA_VERSION.to_string(),
        assignment,
        workspace_dir: path_label(&workspace_dir),
        status: "queued".to_string(),
        artifact_refs: Vec::new(),
        receipt_refs: Vec::new(),
        events: Vec::new(),
        started_at_ms: now,
        completed_at_ms: None,
        cleanup_at_ms: None,
        updated_at_ms: now,
    };
    push_codex_run_event(
        &args.state_dir,
        &mut state,
        CodexWorkroomEventKind::Queued,
        CodexWorkroomDecision::Accepted,
        "assignment accepted for codex vm runner",
        None,
        None,
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        state.assignment.assignment_id.as_str(),
        state.assignment.workroom_id.as_str(),
        "run.queued",
        "runner",
        "Codex one-shot run queued.",
        None,
        Vec::new(),
        Vec::new(),
    )?;
    save_codex_run_state(&args.state_dir, &state)?;

    let mut auth_receipts = Vec::new();
    let run_result = codex_run_inner(&args, &mut state, &mut auth_receipts);
    if let Err(error) = &run_result {
        if state.status != "failed" && state.status != "timeout" {
            state.status = "failed".to_string();
            push_codex_run_event(
                &args.state_dir,
                &mut state,
                CodexWorkroomEventKind::Failed,
                CodexWorkroomDecision::Failed,
                redacted_error("codex run failed", error.as_str()).as_str(),
                None,
                None,
            )?;
            append_openagents_runner_event(
                &args.state_dir,
                state.assignment.assignment_id.as_str(),
                state.assignment.workroom_id.as_str(),
                "run.failed",
                "runner",
                "Codex one-shot run failed.",
                Some(redacted_error("codex run failed", error.as_str()).as_str()),
                Vec::new(),
                Vec::new(),
            )?;
        }
    }

    // Git writeback (cloud#96): before the ephemeral workspace is torn down,
    // commit and push any local repo changes so a managed coding run on an
    // ephemeral VM does not lose its code on cleanup. This is gated: it only
    // runs when a GitHub write token is in the run environment and the workspace
    // is a git repo with staged-or-unstaged changes. Runs without a write token
    // behave exactly as before (no writeback). The emitted event/receipt is
    // refs-only (commit sha + branch); the token never enters events, receipts,
    // logs, git config, or remotes.
    if run_result.is_ok() {
        if let Err(error) = run_git_writeback(&args.state_dir, &workspace_dir, &mut state) {
            // Writeback failure must not corrupt run status silently, but it
            // also must not mask a successful run; surface it as a refs-only
            // failure event and mark the run failed so the loss is visible.
            push_codex_run_event(
                &args.state_dir,
                &mut state,
                CodexWorkroomEventKind::Failed,
                CodexWorkroomDecision::Failed,
                redacted_error("git writeback failed", error.as_str()).as_str(),
                None,
                None,
            )?;
            append_openagents_runner_event(
                &args.state_dir,
                state.assignment.assignment_id.as_str(),
                state.assignment.workroom_id.as_str(),
                "git.writeback.failed",
                "runner",
                "Git writeback failed before workspace teardown.",
                Some(redacted_error("git writeback failed", error.as_str()).as_str()),
                Vec::new(),
                Vec::new(),
            )?;
            state.status = "failed".to_string();
        }
    }

    let cleanup_result = cleanup_codex_workspace(&workspace_dir);
    let scrub_result = scrub_codex_auth_state(&args.state_dir).map(|(_, receipt, _)| receipt);

    match cleanup_result {
        Ok(cleanup_message) => {
            state.cleanup_at_ms = Some(now_epoch_ms()?);
            push_codex_run_event(
                &args.state_dir,
                &mut state,
                CodexWorkroomEventKind::Cleanup,
                CodexWorkroomDecision::Accepted,
                cleanup_message.as_str(),
                None,
                None,
            )?;
        }
        Err(error) => {
            push_codex_run_event(
                &args.state_dir,
                &mut state,
                CodexWorkroomEventKind::Cleanup,
                CodexWorkroomDecision::Failed,
                redacted_error("workspace cleanup failed", error.as_str()).as_str(),
                None,
                None,
            )?;
            if run_result.is_ok() {
                state.status = "failed".to_string();
            }
        }
    }

    match scrub_result {
        Ok(receipt) => auth_receipts.push(receipt),
        Err(error) => {
            push_codex_run_event(
                &args.state_dir,
                &mut state,
                CodexWorkroomEventKind::Cleanup,
                CodexWorkroomDecision::Failed,
                redacted_error("codex auth scrub failed", error.as_str()).as_str(),
                None,
                None,
            )?;
            if run_result.is_ok() {
                state.status = "failed".to_string();
            }
        }
    }

    state.completed_at_ms = Some(now_epoch_ms()?);
    state.updated_at_ms = state.completed_at_ms.unwrap_or(state.updated_at_ms);
    save_codex_run_state(&args.state_dir, &state)?;
    let output = CodexRunOutput {
        state: state.clone(),
        auth_receipts,
        runner_events: load_openagents_runner_events(&args.state_dir)?,
    };

    if let Err(error) = run_result {
        return Err(error);
    }
    if state.status != "completed" {
        return Err(format!("codex run finished with status {}", state.status));
    }
    if args.stream_jsonl {
        let mut lines = Vec::new();
        for event in &output.state.events {
            lines.push(
                serde_json::to_string(event)
                    .map_err(|error| format!("failed to serialize codex event: {error}"))?,
            );
        }
        Ok(lines.join("\n"))
    } else {
        render_json_or_human(
            args.json,
            &output,
            format!(
                "status: {}\nartifact_count: {}\nevent_count: {}",
                output.state.status,
                output.state.artifact_refs.len(),
                output.state.events.len()
            ),
        )
    }
}

fn codex_run_inner(
    args: &CodexRunArgs,
    state: &mut CodexRunState,
    auth_receipts: &mut Vec<CodexAuthReceipt>,
) -> Result<(), String> {
    let runtime = args.agent_runtime;
    let process_label = runtime.process_label();
    let runtime_name = runtime.display_name();
    let status_receipt =
        verify_codex_auth_for_run(&args.state_dir, &args.codex_bin, &state.assignment)?;
    auth_receipts.push(status_receipt);

    let workspace_dir = PathBuf::from(state.workspace_dir.clone());
    prepare_codex_workspace(&args.state_dir, &workspace_dir, &state.assignment)?;
    state.status = "started".to_string();
    push_codex_run_event(
        &args.state_dir,
        state,
        CodexWorkroomEventKind::Started,
        CodexWorkroomDecision::Accepted,
        format!("{process_label} started in private no-wallet workspace").as_str(),
        None,
        None,
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        state.assignment.assignment_id.as_str(),
        state.assignment.workroom_id.as_str(),
        "run.started",
        "runner",
        format!("{runtime_name} one-shot run started.").as_str(),
        None,
        Vec::new(),
        Vec::new(),
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        state.assignment.assignment_id.as_str(),
        state.assignment.workroom_id.as_str(),
        "turn.started",
        "runner",
        format!("{runtime_name} one-shot turn started.").as_str(),
        None,
        Vec::new(),
        Vec::new(),
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        state.assignment.assignment_id.as_str(),
        state.assignment.workroom_id.as_str(),
        "run.heartbeat",
        "runner",
        format!("{runtime_name} one-shot runner heartbeat recorded.").as_str(),
        None,
        Vec::new(),
        Vec::new(),
    )?;

    let auth_state = load_codex_auth_state(&args.state_dir)?;
    let execution = match runtime {
        AgentRuntime::Codex => run_codex_exec(
            &args.state_dir,
            &args.codex_bin,
            &workspace_dir,
            auth_state.codex_home.as_str(),
            &state.assignment,
        ),
        AgentRuntime::OpencodeCodex => run_opencode_codex(
            &args.state_dir,
            &args.opencode_bin,
            &workspace_dir,
            auth_state.codex_home.as_str(),
            &state.assignment,
        ),
    }?;
    record_codex_output_events(&args.state_dir, state, "stdout", &execution.stdout)?;
    record_codex_output_events(&args.state_dir, state, "stderr", &execution.stderr)?;

    let usage_receipt = append_resource_usage_receipt(
        &args.state_dir,
        &state.assignment,
        &workspace_dir,
        &execution,
    )?;
    state
        .receipt_refs
        .push(usage_receipt.receipt_digest.clone());
    push_codex_run_event(
        &args.state_dir,
        state,
        CodexWorkroomEventKind::Receipt,
        CodexWorkroomDecision::Accepted,
        "codex run resource usage receipt emitted",
        None,
        Some(usage_receipt.receipt_digest.as_str()),
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        state.assignment.assignment_id.as_str(),
        state.assignment.workroom_id.as_str(),
        "resource.usage.captured",
        "runner",
        "Resource and model usage receipt captured.",
        None,
        Vec::new(),
        vec![usage_receipt.receipt_digest.clone()],
    )?;
    if observed_codex_token_usage(&execution).is_none() {
        append_openagents_runner_event(
            &args.state_dir,
            state.assignment.assignment_id.as_str(),
            state.assignment.workroom_id.as_str(),
            "usage.unavailable",
            "runner",
            "Subscription-backed Codex usage metrics were unavailable.",
            None,
            Vec::new(),
            vec![usage_receipt.receipt_digest.clone()],
        )?;
    }
    if execution.artifact_completed {
        append_openagents_runner_event(
            &args.state_dir,
            state.assignment.assignment_id.as_str(),
            state.assignment.workroom_id.as_str(),
            "artifact_set.completed",
            "runner",
            "Required artifact set completed before Codex process exit.",
            None,
            Vec::new(),
            Vec::new(),
        )?;
    }

    if execution.timed_out {
        state.status = "timeout".to_string();
        push_codex_run_event(
            &args.state_dir,
            state,
            CodexWorkroomEventKind::Timeout,
            CodexWorkroomDecision::Failed,
            format!("{process_label} timeout").as_str(),
            None,
            None,
        )?;
        append_openagents_runner_event(
            &args.state_dir,
            state.assignment.assignment_id.as_str(),
            state.assignment.workroom_id.as_str(),
            "run.timed_out",
            "runner",
            format!("{runtime_name} one-shot run timed out.").as_str(),
            None,
            Vec::new(),
            Vec::new(),
        )?;
        return Err(format!("{process_label} timed out"));
    }
    if execution.exit_code != Some(0) {
        state.status = "failed".to_string();
        push_codex_run_event(
            &args.state_dir,
            state,
            CodexWorkroomEventKind::Failed,
            CodexWorkroomDecision::Failed,
            format!(
                "{process_label} failed exit {}",
                execution.exit_code.unwrap_or(-1)
            )
            .as_str(),
            None,
            None,
        )?;
        append_openagents_runner_event(
            &args.state_dir,
            state.assignment.assignment_id.as_str(),
            state.assignment.workroom_id.as_str(),
            "run.failed",
            "runner",
            format!("{runtime_name} one-shot command failed.").as_str(),
            None,
            Vec::new(),
            Vec::new(),
        )?;
        return Err(format!(
            "{process_label} failed exit {}",
            execution.exit_code.unwrap_or(-1)
        ));
    }

    capture_codex_artifacts(&args.state_dir, &workspace_dir, state)?;
    state.status = "completed".to_string();
    push_codex_run_event(
        &args.state_dir,
        state,
        CodexWorkroomEventKind::Completed,
        CodexWorkroomDecision::Accepted,
        format!("{process_label} completed and closeout manifest submitted").as_str(),
        None,
        None,
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        state.assignment.assignment_id.as_str(),
        state.assignment.workroom_id.as_str(),
        "message.completed",
        "runner",
        format!("{runtime_name} one-shot turn completed.").as_str(),
        None,
        Vec::new(),
        Vec::new(),
    )?;
    append_openagents_runner_event(
        &args.state_dir,
        state.assignment.assignment_id.as_str(),
        state.assignment.workroom_id.as_str(),
        "run.completed",
        "runner",
        format!("{runtime_name} one-shot run completed.").as_str(),
        None,
        Vec::new(),
        Vec::new(),
    )?;
    Ok(())
}

fn load_codex_workroom_assignment(path: &Path) -> Result<CodexWorkroomAssignment, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read codex workroom assignment {}: {error}",
            path_label(path)
        )
    })?;
    serde_json::from_str::<CodexWorkroomAssignment>(&raw).map_err(|error| {
        format!(
            "failed to parse codex workroom assignment {}: {error}",
            path_label(path)
        )
    })
}

fn verify_codex_auth_for_run(
    state_dir: &Path,
    codex_bin: &Path,
    assignment: &CodexWorkroomAssignment,
) -> Result<CodexAuthReceipt, String> {
    let mut state = load_codex_auth_state(state_dir)?;
    if state.grant.workroom_id != assignment.workroom_id {
        return Err("codex auth grant workroom does not match assignment".to_string());
    }
    if state.grant.provider_account_ref != assignment.provider_account_ref {
        return Err("codex auth grant provider account does not match assignment".to_string());
    }
    if state.grant.grant_ref != assignment.auth_grant_ref {
        return Err("codex auth grant ref does not match assignment".to_string());
    }
    let now = now_epoch_ms()?;
    if state.grant.expires_at_ms <= now {
        return Err("codex auth grant is expired".to_string());
    }
    if state.scrubbed_at_ms.is_some() {
        return Err("codex auth material is already scrubbed".to_string());
    }
    let (decision, reason) = run_codex_login_status(codex_bin, state.codex_home.as_str());
    let receipt = build_codex_auth_receipt(
        &state.grant,
        CodexAuthReceiptKind::LoginStatusChecked,
        decision.clone(),
        reason.as_str(),
        Path::new(state.codex_home.as_str()),
    )?;
    state.login_status = Some(reason.clone());
    state.status_checked_at_ms = Some(now);
    state.updated_at_ms = now;
    state.receipts.push(receipt.clone());
    save_codex_auth_state(state_dir, &state)?;
    append_codex_auth_receipt(state_dir, &receipt)?;
    if decision != CodexAuthDecision::Accepted {
        return Err(format!("codex auth status rejected: {reason}"));
    }
    Ok(receipt)
}

fn prepare_codex_workspace(
    state_dir: &Path,
    workspace_dir: &Path,
    assignment: &CodexWorkroomAssignment,
) -> Result<(), String> {
    fs::create_dir_all(workspace_dir).map_err(|error| {
        format!(
            "failed to create codex workspace {}: {error}",
            path_label(workspace_dir)
        )
    })?;
    prepare_github_askpass(state_dir)?;
    prepare_github_checkout(state_dir, workspace_dir, assignment)?;
    let sandbox_label = codex_sandbox_label(&assignment.sandbox);
    let agents = format!(
        "# OpenAgents Codex VM Workroom\n\n- Workroom: `{}`\n- Assignment: `{}`\n- Target node: `{}`\n- Wallet authority: none\n- Sandbox: `{}`\n\nProduce only the declared artifacts: {}.\nDo not request wallet keys, VM-global credentials, GCP credentials, or raw provider tokens.\n",
        assignment.workroom_id,
        assignment.assignment_id,
        assignment.target_node_id,
        sandbox_label,
        assignment.required_artifacts.join(", ")
    );
    let git_dir = workspace_dir.join(".git");
    let workroom_notes = if git_dir.exists() {
        git_dir.join("openagents-workroom.md")
    } else {
        workspace_dir.join("OPENAGENTS_WORKROOM.md")
    };
    fs::write(&workroom_notes, agents.as_str())
        .map_err(|error| format!("failed to write codex workroom notes: {error}"))?;
    let agents_path = workspace_dir.join("AGENTS.md");
    if !git_dir.exists() && !agents_path.exists() {
        fs::write(&agents_path, agents)
            .map_err(|error| format!("failed to write codex workspace AGENTS.md: {error}"))?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitHubCheckout {
    owner: String,
    repo: String,
    ref_name: String,
}

fn prepare_github_checkout(
    state_dir: &Path,
    workspace_dir: &Path,
    assignment: &CodexWorkroomAssignment,
) -> Result<(), String> {
    let Some(checkout) = github_checkout_from_repo_ref(assignment.repo_ref.as_str()) else {
        return Ok(());
    };

    if workspace_dir.join(".git").exists() {
        return Ok(());
    }

    if !workspace_is_empty(workspace_dir)? {
        append_openagents_runner_event(
            state_dir,
            assignment.assignment_id.as_str(),
            assignment.workroom_id.as_str(),
            "repo.checkout.skipped",
            "runner",
            "Repository checkout skipped because the workspace already has files.",
            Some(&format!(
                "{}/{}@{}",
                checkout.owner, checkout.repo, checkout.ref_name
            )),
            Vec::new(),
            Vec::new(),
        )?;
        return Ok(());
    }

    if env::var("OPENAGENTS_CODEX_REPO_CHECKOUT").ok().as_deref() != Some("enabled") {
        append_openagents_runner_event(
            state_dir,
            assignment.assignment_id.as_str(),
            assignment.workroom_id.as_str(),
            "repo.checkout.skipped",
            "runner",
            "Repository checkout skipped because checkout was not enabled for this run.",
            Some(&format!(
                "{}/{}@{}",
                checkout.owner, checkout.repo, checkout.ref_name
            )),
            Vec::new(),
            Vec::new(),
        )?;
        return Ok(());
    }

    if github_token_available().is_none() {
        append_openagents_runner_event(
            state_dir,
            assignment.assignment_id.as_str(),
            assignment.workroom_id.as_str(),
            "repo.checkout.failed",
            "runner",
            "Repository checkout failed because no GitHub token was available.",
            Some(&format!(
                "{}/{}@{}",
                checkout.owner, checkout.repo, checkout.ref_name
            )),
            Vec::new(),
            Vec::new(),
        )?;
        return Err("GitHub repository checkout requires GITHUB_TOKEN or GH_TOKEN".to_string());
    }

    append_openagents_runner_event(
        state_dir,
        assignment.assignment_id.as_str(),
        assignment.workroom_id.as_str(),
        "repo.checkout.started",
        "runner",
        "Repository checkout started.",
        Some(&format!(
            "{}/{}@{}",
            checkout.owner, checkout.repo, checkout.ref_name
        )),
        Vec::new(),
        Vec::new(),
    )?;
    let askpass_path = prepare_github_askpass(state_dir)?;
    clone_github_checkout(state_dir, workspace_dir, &checkout).map_err(|error| {
        if let Some(path) = askpass_path.as_ref() {
            let _ = fs::remove_file(path);
        }
        let detail = redacted_error("git checkout failed", error.as_str());
        let _ = append_openagents_runner_event(
            state_dir,
            assignment.assignment_id.as_str(),
            assignment.workroom_id.as_str(),
            "repo.checkout.failed",
            "runner",
            "Repository checkout failed.",
            Some(detail.as_str()),
            Vec::new(),
            Vec::new(),
        );
        error
    })?;
    if let Some(path) = askpass_path.as_ref() {
        let _ = fs::remove_file(path);
    }
    append_openagents_runner_event(
        state_dir,
        assignment.assignment_id.as_str(),
        assignment.workroom_id.as_str(),
        "repo.checkout.completed",
        "runner",
        "Repository checkout completed.",
        Some(&format!(
            "{}/{}@{}",
            checkout.owner, checkout.repo, checkout.ref_name
        )),
        Vec::new(),
        Vec::new(),
    )?;
    Ok(())
}

fn github_checkout_from_repo_ref(repo_ref: &str) -> Option<GitHubCheckout> {
    let trimmed = repo_ref.trim();
    if trimmed.is_empty() || trimmed == "none" {
        return None;
    }

    let without_scheme = trimmed
        .strip_prefix("https://github.com/")
        .unwrap_or(trimmed)
        .trim_end_matches(".git");
    let (repo_path, ref_name) = without_scheme
        .split_once('@')
        .map(|(repo_path, ref_name)| (repo_path, ref_name))
        .unwrap_or((without_scheme, "main"));
    let mut parts = repo_path.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if parts.next().is_some()
        || !is_safe_github_component(owner)
        || !is_safe_github_component(repo)
        || !is_safe_git_ref(ref_name)
    {
        return None;
    }

    Some(GitHubCheckout {
        owner: owner.to_string(),
        repo: repo.to_string(),
        ref_name: ref_name.to_string(),
    })
}

fn is_safe_github_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && !value.starts_with('.')
        && !value.contains("..")
        && !contains_forbidden_metadata(value)
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn is_safe_git_ref(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && !value.starts_with('.')
        && !value.contains("..")
        && !value.contains('\\')
        && !value.contains(' ')
        && !value.contains('\n')
        && !contains_forbidden_metadata(value)
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
}

fn workspace_is_empty(workspace_dir: &Path) -> Result<bool, String> {
    let mut entries = fs::read_dir(workspace_dir)
        .map_err(|error| format!("failed to read codex workspace: {error}"))?;
    Ok(entries.next().is_none())
}

fn github_token_available() -> Option<String> {
    // Order matters: the run-scoped write grant materialized by oa-codex-control
    // exports GITHUB_TOKEN/GH_TOKEN for the run; OA_CODEX_GITHUB_TOKEN is a
    // statically-configured fallback for operator-driven runs without a grant
    // resolver (cloud#96). All three are process-environment only and must never
    // be logged, persisted to git config/remotes, artifacts, or receipts.
    for name in ["GITHUB_TOKEN", "GH_TOKEN", "OA_CODEX_GITHUB_TOKEN"] {
        if env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_some()
        {
            return Some(name.to_string());
        }
    }

    None
}

fn prepare_github_askpass(state_dir: &Path) -> Result<Option<PathBuf>, String> {
    if github_token_available().is_none() {
        return Ok(None);
    }

    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {} for git askpass: {error}",
            path_label(state_dir)
        )
    })?;
    let path = git_askpass_path(state_dir);
    fs::write(
        &path,
        "#!/bin/sh\ncase \"$1\" in\n*Username*) printf '%s\\n' 'x-access-token' ;;\n*) if [ -n \"$GITHUB_TOKEN\" ]; then printf '%s\\n' \"$GITHUB_TOKEN\"; elif [ -n \"$GH_TOKEN\" ]; then printf '%s\\n' \"$GH_TOKEN\"; else printf '%s\\n' \"$OA_CODEX_GITHUB_TOKEN\"; fi ;;\nesac\n",
    )
    .map_err(|error| format!("failed to write git askpass helper: {error}"))?;
    set_owner_execute_permissions(&path)?;
    Ok(Some(path))
}

fn git_askpass_path(state_dir: &Path) -> PathBuf {
    state_dir.join("git-askpass.sh")
}

fn git_command_with_auth(state_dir: &Path) -> ProcessCommand {
    let mut command = ProcessCommand::new("git");
    let askpass = git_askpass_path(state_dir);
    if askpass.exists() {
        command
            .env("GIT_ASKPASS", askpass)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never");
    }
    command
}

fn clone_github_checkout(
    state_dir: &Path,
    workspace_dir: &Path,
    checkout: &GitHubCheckout,
) -> Result<(), String> {
    let url = format!(
        "https://github.com/{}/{}.git",
        checkout.owner, checkout.repo
    );
    let output = git_command_with_auth(state_dir)
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            checkout.ref_name.as_str(),
            "--",
            url.as_str(),
        ])
        .arg(workspace_dir)
        .output()
        .map_err(|error| format!("failed to spawn git clone: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let clone_error = String::from_utf8_lossy(&output.stderr).to_string();
    if workspace_dir.exists() && workspace_is_empty(workspace_dir)? {
        let _ = fs::remove_dir_all(workspace_dir);
    }
    let fallback = git_command_with_auth(state_dir)
        .args(["clone", "--depth", "1", "--", url.as_str()])
        .arg(workspace_dir)
        .output()
        .map_err(|error| format!("failed to spawn fallback git clone: {error}"))?;
    if !fallback.status.success() {
        return Err(redacted_error(
            "git clone failed",
            String::from_utf8_lossy(&fallback.stderr).as_ref(),
        ));
    }

    let checkout_output = git_command_with_auth(state_dir)
        .args(["checkout", checkout.ref_name.as_str()])
        .current_dir(workspace_dir)
        .output()
        .map_err(|error| format!("failed to spawn git checkout: {error}"))?;
    if checkout_output.status.success() {
        Ok(())
    } else {
        Err(redacted_error(
            "git checkout failed",
            format!(
                "{} {}",
                clone_error,
                String::from_utf8_lossy(&checkout_output.stderr)
            )
            .as_str(),
        ))
    }
}

/// Refs-only writeback outcome (commit sha + branch ref). Never carries a token
/// or remote URL with embedded credentials.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct GitWritebackReceipt {
    schema_version: String,
    assignment_id: String,
    workroom_id: String,
    repository: String,
    branch_ref: String,
    commit_sha: String,
    pushed: bool,
    emitted_at_ms: u128,
}

/// Commit and push any local workspace repo changes back to the target branch
/// before the ephemeral workspace is torn down (cloud#96).
///
/// Gating (no-op when any precondition is absent, matching pre-writeback
/// behavior):
/// - the workspace must be a git repo (`.git` present);
/// - a GitHub write token must be in the run environment
///   (`GITHUB_TOKEN` / `GH_TOKEN` from a run-scoped write grant, or the
///   statically-configured `OA_CODEX_GITHUB_TOKEN` fallback);
/// - `git status --porcelain` must report changes.
///
/// On success it emits a refs-only `git.writeback.completed` runner event and
/// writes a `git-writeback.json` receipt (commit sha + branch ref only). The
/// token is supplied to `git push` only through the askpass helper + process
/// environment; it never enters the commit, git config, remotes, events,
/// receipts, or logs.
fn run_git_writeback(
    state_dir: &Path,
    workspace_dir: &Path,
    state: &mut CodexRunState,
) -> Result<(), String> {
    let assignment_id = state.assignment.assignment_id.clone();
    let workroom_id = state.assignment.workroom_id.clone();

    if !workspace_dir.join(".git").exists() {
        append_openagents_runner_event(
            state_dir,
            assignment_id.as_str(),
            workroom_id.as_str(),
            "git.writeback.skipped",
            "runner",
            "Git writeback skipped because the workspace is not a git checkout.",
            None,
            Vec::new(),
            Vec::new(),
        )?;
        return Ok(());
    }

    if github_token_available().is_none() {
        append_openagents_runner_event(
            state_dir,
            assignment_id.as_str(),
            workroom_id.as_str(),
            "git.writeback.skipped",
            "runner",
            "Git writeback skipped because no GitHub write token was provided for this run.",
            None,
            Vec::new(),
            Vec::new(),
        )?;
        return Ok(());
    }

    let Some(checkout) = github_checkout_from_repo_ref(state.assignment.repo_ref.as_str()) else {
        append_openagents_runner_event(
            state_dir,
            assignment_id.as_str(),
            workroom_id.as_str(),
            "git.writeback.skipped",
            "runner",
            "Git writeback skipped because the assignment repo ref is not a writable target.",
            None,
            Vec::new(),
            Vec::new(),
        )?;
        return Ok(());
    };

    if !git_workspace_has_changes(state_dir, workspace_dir)? {
        append_openagents_runner_event(
            state_dir,
            assignment_id.as_str(),
            workroom_id.as_str(),
            "git.writeback.no_changes",
            "runner",
            "Git writeback found no local changes to push.",
            Some(&format!(
                "{}/{}@{}",
                checkout.owner, checkout.repo, checkout.ref_name
            )),
            Vec::new(),
            Vec::new(),
        )?;
        return Ok(());
    }

    let askpass_path = prepare_github_askpass(state_dir)?;
    let writeback =
        git_commit_and_push(state_dir, workspace_dir, &checkout, assignment_id.as_str());
    if let Some(path) = askpass_path.as_ref() {
        let _ = fs::remove_file(path);
    }
    let commit_sha = writeback?;

    let repository = format!("{}/{}", checkout.owner, checkout.repo);
    let receipt = GitWritebackReceipt {
        schema_version: "openagents.git_writeback.v1".to_string(),
        assignment_id: assignment_id.clone(),
        workroom_id: workroom_id.clone(),
        repository: repository.clone(),
        branch_ref: checkout.ref_name.clone(),
        commit_sha: commit_sha.clone(),
        pushed: true,
        emitted_at_ms: now_epoch_ms()?,
    };
    let receipt_path = state_dir.join("git-writeback.json");
    let receipt_json = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("failed to serialize git writeback receipt: {error}"))?;
    fs::write(&receipt_path, format!("{receipt_json}\n"))
        .map_err(|error| format!("failed to write git writeback receipt: {error}"))?;

    let payload = serde_json::json!({
        "repository": repository,
        "branchRef": checkout.ref_name,
        "commitSha": commit_sha,
        "pushed": true,
    })
    .to_string();
    append_openagents_runner_event_with_payload(
        state_dir,
        assignment_id.as_str(),
        workroom_id.as_str(),
        "git.writeback.completed",
        "runner",
        "Git writeback committed and pushed workspace changes.",
        Some(&format!("{repository}@{} {commit_sha}", checkout.ref_name)),
        Some(payload.as_str()),
        Vec::new(),
        Vec::new(),
    )?;
    Ok(())
}

fn git_workspace_has_changes(state_dir: &Path, workspace_dir: &Path) -> Result<bool, String> {
    let output = git_command_with_auth(state_dir)
        .args(["status", "--porcelain"])
        .current_dir(workspace_dir)
        .output()
        .map_err(|error| format!("failed to spawn git status: {error}"))?;
    if !output.status.success() {
        return Err(redacted_error(
            "git status failed",
            String::from_utf8_lossy(&output.stderr).as_ref(),
        ));
    }
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Stage all workspace changes, commit with a run-referencing message + the
/// standard Co-Authored-By trailer, and push to the target branch. Returns the
/// resulting commit sha. Committer identity is set per-invocation (`-c`), never
/// written to global git config; the token is never embedded in the remote URL
/// or commit.
fn git_commit_and_push(
    state_dir: &Path,
    workspace_dir: &Path,
    checkout: &GitHubCheckout,
    assignment_id: &str,
) -> Result<String, String> {
    let add = git_command_with_auth(state_dir)
        .args(["add", "--all", "--"])
        .arg(".")
        .current_dir(workspace_dir)
        .output()
        .map_err(|error| format!("failed to spawn git add: {error}"))?;
    if !add.status.success() {
        return Err(redacted_error(
            "git add failed",
            String::from_utf8_lossy(&add.stderr).as_ref(),
        ));
    }

    let commit_message = format!(
        "OpenAgents Codex workroom run {assignment_id}\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
    );
    let commit = git_command_with_auth(state_dir)
        .args([
            "-c",
            "user.name=OpenAgents Codex Runner",
            "-c",
            "user.email=codex-runner@openagents.com",
            "commit",
            "--no-gpg-sign",
            "-m",
        ])
        .arg(commit_message.as_str())
        .current_dir(workspace_dir)
        .output()
        .map_err(|error| format!("failed to spawn git commit: {error}"))?;
    if !commit.status.success() {
        return Err(redacted_error(
            "git commit failed",
            String::from_utf8_lossy(&commit.stderr).as_ref(),
        ));
    }

    let head = git_command_with_auth(state_dir)
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace_dir)
        .output()
        .map_err(|error| format!("failed to spawn git rev-parse: {error}"))?;
    if !head.status.success() {
        return Err(redacted_error(
            "git rev-parse failed",
            String::from_utf8_lossy(&head.stderr).as_ref(),
        ));
    }
    let commit_sha = String::from_utf8_lossy(&head.stdout).trim().to_string();
    if commit_sha.is_empty() || !commit_sha.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("git rev-parse returned an unexpected commit sha".to_string());
    }

    let refspec = format!("HEAD:refs/heads/{}", checkout.ref_name);
    let push = git_command_with_auth(state_dir)
        .args(["push", "origin"])
        .arg(refspec.as_str())
        .current_dir(workspace_dir)
        .output()
        .map_err(|error| format!("failed to spawn git push: {error}"))?;
    if !push.status.success() {
        return Err(redacted_error(
            "git push failed",
            String::from_utf8_lossy(&push.stderr).as_ref(),
        ));
    }

    Ok(commit_sha)
}

fn run_codex_exec(
    state_dir: &Path,
    codex_bin: &Path,
    workspace_dir: &Path,
    codex_home: &str,
    assignment: &CodexWorkroomAssignment,
) -> Result<CodexExecutionResult, String> {
    let sandbox = codex_sandbox_label(&assignment.sandbox);
    let mut command = ProcessCommand::new(codex_bin);
    command
        .args([
            "exec",
            "--skip-git-repo-check",
            "--json",
            "--sandbox",
            sandbox,
        ])
        .arg(assignment.prompt.as_str())
        .env("CODEX_HOME", codex_home)
        .current_dir(workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    run_agent_process(state_dir, workspace_dir, assignment, command, "codex exec")
}

fn run_opencode_codex(
    state_dir: &Path,
    opencode_bin: &Path,
    workspace_dir: &Path,
    codex_home: &str,
    assignment: &CodexWorkroomAssignment,
) -> Result<CodexExecutionResult, String> {
    let opencode_auth_content = opencode_auth_content_from_codex_home(codex_home)?;
    let model = opencode_codex_model()?;
    let mut command = ProcessCommand::new(opencode_bin);
    command
        .args(["run", "--format", "json", "--model", model.as_str()])
        .arg(assignment.prompt.as_str())
        .env("CODEX_HOME", codex_home)
        .env("OPENCODE_AUTH_CONTENT", opencode_auth_content)
        .current_dir(workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    run_agent_process(
        state_dir,
        workspace_dir,
        assignment,
        command,
        "opencode run",
    )
}

fn opencode_codex_model() -> Result<String, String> {
    let model = env::var("OA_OPENCODE_CODEX_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_OPENCODE_CODEX_MODEL.to_string());

    if contains_forbidden_metadata(model.as_str()) {
        return Err("OpenCode Codex model contains forbidden metadata".to_string());
    }

    if !model.starts_with("openai/") {
        return Err("OpenCode Codex model must use the openai provider".to_string());
    }

    Ok(model)
}

fn run_agent_process(
    state_dir: &Path,
    workspace_dir: &Path,
    assignment: &CodexWorkroomAssignment,
    mut command: ProcessCommand,
    process_label: &str,
) -> Result<CodexExecutionResult, String> {
    let askpass = git_askpass_path(state_dir);
    if askpass.exists() {
        command
            .env("GIT_ASKPASS", askpass)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never");
    }
    let mut child = command
        .spawn()
        .map_err(|_| format!("{process_label} spawn failed"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("{process_label} stdout pipe unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("{process_label} stderr pipe unavailable"))?;
    let stdout_handle = spawn_codex_output_reader(
        state_dir.to_path_buf(),
        assignment.clone(),
        "stdout",
        stdout,
    );
    let stderr_handle = spawn_codex_output_reader(
        state_dir.to_path_buf(),
        assignment.clone(),
        "stderr",
        stderr,
    );
    let timeout = assignment
        .timeout_ms
        .map(|ms| Duration::from_millis(ms as u64));
    let started = Instant::now();
    let artifact_complete_grace = Duration::from_secs(2);
    let mut artifact_complete_since: Option<Instant> = None;
    let mut artifact_completed = false;
    let mut timed_out = false;
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("{process_label} wait failed: {error}"))?
            .is_some()
        {
            break;
        }
        if required_codex_artifacts_ready(workspace_dir, &assignment.required_artifacts)? {
            match artifact_complete_since {
                Some(ready_since) if ready_since.elapsed() >= artifact_complete_grace => {
                    artifact_completed = true;
                    let _ = child.kill();
                    break;
                }
                Some(_) => {}
                None => artifact_complete_since = Some(Instant::now()),
            }
        } else {
            artifact_complete_since = None;
        }
        if let Some(timeout) = timeout {
            if started.elapsed() >= timeout {
                timed_out = true;
                let _ = child.kill();
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let status = child_wait(&mut child, process_label)?;
    let stdout = join_codex_output_reader(stdout_handle, "stdout")?;
    let stderr = join_codex_output_reader(stderr_handle, "stderr")?;
    Ok(CodexExecutionResult {
        exit_code: if artifact_completed {
            Some(0)
        } else {
            status.code()
        },
        artifact_completed,
        timed_out,
        wall_time_ms: started.elapsed().as_millis(),
        stdout,
        stderr,
    })
}

fn opencode_auth_content_from_codex_home(codex_home: &str) -> Result<String, String> {
    let auth_json_path = Path::new(codex_home).join("auth.json");
    let raw = fs::read_to_string(&auth_json_path).map_err(|error| {
        format!(
            "failed to read materialized codex auth cache {}: {error}",
            path_label(&auth_json_path)
        )
    })?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).map_err(|error| {
        format!(
            "failed to parse materialized codex auth cache {}: {error}",
            path_label(&auth_json_path)
        )
    })?;
    if value
        .get("openai")
        .and_then(|openai| openai.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("oauth")
    {
        return serde_json::to_string(&value)
            .map_err(|error| format!("failed to serialize OpenCode auth content: {error}"));
    }

    let access = find_auth_string_field(
        &value,
        &[
            "access",
            "access_token",
            "accessToken",
            "id_token",
            "idToken",
        ],
    )
    .ok_or_else(|| "materialized codex auth cache does not expose an access token".to_string())?;
    let refresh = find_auth_string_field(&value, &["refresh", "refresh_token", "refreshToken"])
        .ok_or_else(|| {
            "materialized codex auth cache does not expose a refresh token".to_string()
        })?;
    let expires = find_auth_u64_field(
        &value,
        &[
            "expires",
            "expires_at",
            "expiresAt",
            "expires_at_ms",
            "expiresAtMs",
            "expiry",
        ],
    )
    .map(normalize_opencode_expires)
    .unwrap_or(0);
    let account_id = find_auth_string_field(
        &value,
        &[
            "accountId",
            "account_id",
            "chatgpt_account_id",
            "chatgptAccountId",
        ],
    );

    let mut oauth = serde_json::Map::new();
    oauth.insert(
        "type".to_string(),
        serde_json::Value::String("oauth".to_string()),
    );
    oauth.insert("access".to_string(), serde_json::Value::String(access));
    oauth.insert("refresh".to_string(), serde_json::Value::String(refresh));
    oauth.insert(
        "expires".to_string(),
        serde_json::Value::Number(serde_json::Number::from(expires)),
    );
    if let Some(account_id) = account_id {
        oauth.insert(
            "accountId".to_string(),
            serde_json::Value::String(account_id),
        );
    }

    let mut root = serde_json::Map::new();
    root.insert("openai".to_string(), serde_json::Value::Object(oauth));
    serde_json::to_string(&serde_json::Value::Object(root))
        .map_err(|error| format!("failed to serialize OpenCode auth content: {error}"))
}

fn find_auth_string_field(value: &serde_json::Value, names: &[&str]) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for name in names {
                if let Some(value) = map.get(*name).and_then(serde_json::Value::as_str) {
                    let trimmed = value.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
            for child in map.values() {
                if let Some(value) = find_auth_string_field(child, names) {
                    return Some(value);
                }
            }
            None
        }
        serde_json::Value::Array(values) => {
            for child in values {
                if let Some(value) = find_auth_string_field(child, names) {
                    return Some(value);
                }
            }
            None
        }
        _ => None,
    }
}

fn find_auth_u64_field(value: &serde_json::Value, names: &[&str]) -> Option<u64> {
    match value {
        serde_json::Value::Object(map) => {
            for name in names {
                if let Some(value) = map.get(*name) {
                    if let Some(number) = value.as_u64() {
                        return Some(number);
                    }
                    if let Some(text) = value.as_str().and_then(|text| text.parse::<u64>().ok()) {
                        return Some(text);
                    }
                }
            }
            for child in map.values() {
                if let Some(value) = find_auth_u64_field(child, names) {
                    return Some(value);
                }
            }
            None
        }
        serde_json::Value::Array(values) => {
            for child in values {
                if let Some(value) = find_auth_u64_field(child, names) {
                    return Some(value);
                }
            }
            None
        }
        _ => None,
    }
}

fn normalize_opencode_expires(value: u64) -> u64 {
    if value < 10_000_000_000 {
        value.saturating_mul(1000)
    } else {
        value
    }
}

fn child_wait(child: &mut Child, process_label: &str) -> Result<std::process::ExitStatus, String> {
    child
        .wait()
        .map_err(|error| format!("{process_label} wait failed: {error}"))
}

fn spawn_codex_output_reader<R>(
    state_dir: PathBuf,
    assignment: CodexWorkroomAssignment,
    stream: &'static str,
    reader: R,
) -> thread::JoinHandle<Result<Vec<u8>, String>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffered = BufReader::new(reader);
        let mut collected = Vec::new();
        let mut line = Vec::new();
        loop {
            line.clear();
            let read = buffered
                .read_until(b'\n', &mut line)
                .map_err(|error| format!("failed to read codex {stream}: {error}"))?;
            if read == 0 {
                break;
            }
            collected.extend_from_slice(&line);
            record_codex_runner_output_events(&state_dir, &assignment, stream, &line)?;
        }
        Ok(collected)
    })
}

fn join_codex_output_reader(
    handle: thread::JoinHandle<Result<Vec<u8>, String>>,
    stream: &str,
) -> Result<Vec<u8>, String> {
    handle
        .join()
        .map_err(|_| format!("codex {stream} reader thread panicked"))?
}

fn required_codex_artifacts_ready(
    workspace_dir: &Path,
    required_artifacts: &[String],
) -> Result<bool, String> {
    if required_artifacts.is_empty() {
        return Ok(false);
    }
    for artifact in required_artifacts {
        validate_artifact_name(artifact.as_str())?;
        let metadata = match fs::metadata(workspace_dir.join(artifact)) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(format!(
                    "failed to inspect required artifact {artifact}: {error}"
                ))
            }
        };
        if !metadata.is_file() || metadata.len() == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn append_resource_usage_receipt(
    state_dir: &Path,
    assignment: &CodexWorkroomAssignment,
    workspace_dir: &Path,
    execution: &CodexExecutionResult,
) -> Result<ResourceUsageReceipt, String> {
    let receipt = build_resource_usage_receipt(state_dir, assignment, workspace_dir, execution)?;
    receipt.validate_contract()?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(&receipt)
        .map_err(|error| format!("failed to serialize resource usage receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(resource_usage_receipt_log_path(state_dir))
        .map_err(|error| format!("failed to open resource usage receipt log: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("failed to append resource usage receipt: {error}"))?;
    Ok(receipt)
}

fn build_resource_usage_receipt(
    state_dir: &Path,
    assignment: &CodexWorkroomAssignment,
    workspace_dir: &Path,
    execution: &CodexExecutionResult,
) -> Result<ResourceUsageReceipt, String> {
    let emitted_at_ms = now_epoch_ms()?;
    let host = current_resource_host_snapshot();
    let run = RunResourceUsage {
        sandbox: codex_sandbox_label(&assignment.sandbox).to_string(),
        image_or_profile_digest: sha256_prefixed(
            format!("codex-cli:{}", codex_sandbox_label(&assignment.sandbox)).as_bytes(),
        ),
        workspace_digest: sha256_prefixed(path_label(workspace_dir).as_bytes()),
        wall_time_ms: Some(execution.wall_time_ms),
        exit_code: execution.exit_code,
        timed_out: execution.timed_out,
        workspace_bytes: directory_size_if_exists(workspace_dir),
        artifact_bytes: directory_size_if_exists(&state_dir.join("artifacts")),
        log_bytes: resource_log_bytes(state_dir),
    };
    let model_usage = vec![model_usage_record_for_execution(assignment, execution)];
    let digest_material = serde_json::to_vec(&(
        RESOURCE_USAGE_RECEIPT_VERSION,
        assignment.assignment_id.as_str(),
        assignment.workroom_id.as_str(),
        assignment.target_node_id.as_str(),
        &host,
        &run,
        &model_usage,
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize resource receipt digest material: {error}"))?;
    let receipt = ResourceUsageReceipt {
        schema_version: RESOURCE_USAGE_RECEIPT_VERSION.to_string(),
        receipt_id: format!(
            "resource.usage.{}.{}",
            safe_path_component(assignment.assignment_id.as_str()),
            emitted_at_ms
        ),
        run_ref: assignment.assignment_id.clone(),
        workroom_id: assignment.workroom_id.clone(),
        node_ref: assignment.target_node_id.clone(),
        provider_lane: provider_lane_for_node(assignment.target_node_id.as_str()),
        host,
        run,
        model_usage,
        // This workroomd path does not meter VM-seconds (no GCE lease lifecycle);
        // the infra compute_usage sub-record is GCE-lane only (cloud#92).
        compute_usage: None,
        receipt_digest: sha256_prefixed(&digest_material),
        emitted_at_ms,
    };
    receipt.validate_contract()?;
    Ok(receipt)
}

fn model_usage_record_for_execution(
    assignment: &CodexWorkroomAssignment,
    execution: &CodexExecutionResult,
) -> ModelUsageRecord {
    if let Some(usage) = observed_codex_token_usage(execution) {
        let total_tokens = usage.total_tokens.or_else(|| {
            usage
                .input_tokens
                .zip(usage.output_tokens)
                .map(|(input, output)| input + output)
        });
        return ModelUsageRecord {
            provider: usage.provider.unwrap_or_else(|| "openai".to_string()),
            backend: usage.backend,
            model: usage
                .model
                .unwrap_or_else(|| "codex_subscription".to_string()),
            mode: usage.source_event_type,
            account_ref: Some(assignment.provider_account_ref.clone()),
            input_tokens: usage.input_tokens,
            cached_input_tokens: usage.cached_input_tokens,
            output_tokens: usage.output_tokens,
            reasoning_tokens: usage.reasoning_tokens,
            total_tokens,
            count_source: usage.count_source,
            cost_microusd: None,
            billing_basis: usage.billing_basis,
            unavailable_reason: None,
        };
    }

    ModelUsageRecord {
        provider: "openai".to_string(),
        backend: "codex".to_string(),
        model: "codex_subscription".to_string(),
        mode: "codex_exec".to_string(),
        account_ref: Some(assignment.provider_account_ref.clone()),
        input_tokens: None,
        cached_input_tokens: None,
        output_tokens: None,
        reasoning_tokens: None,
        total_tokens: None,
        count_source: TokenCountSource::Unavailable,
        cost_microusd: None,
        billing_basis: "chatgpt_subscription".to_string(),
        unavailable_reason: Some("subscription_backed_codex_no_token_counts".to_string()),
    }
}

fn observed_codex_token_usage(execution: &CodexExecutionResult) -> Option<ObservedTokenUsage> {
    let mut observed: Option<ObservedTokenUsage> = None;
    for bytes in [&execution.stdout, &execution.stderr] {
        for line in String::from_utf8_lossy(bytes).lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || contains_forbidden_metadata(trimmed) {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                continue;
            };
            if let Some(usage) = codex_token_usage_from_json_event(&value) {
                observed = Some(match observed {
                    Some(existing)
                        if existing.backend == "opencode" && usage.backend == "opencode" =>
                    {
                        merge_observed_token_usage(existing, usage)
                    }
                    _ => usage,
                });
            }
        }
    }
    observed
}

fn merge_observed_token_usage(
    left: ObservedTokenUsage,
    right: ObservedTokenUsage,
) -> ObservedTokenUsage {
    ObservedTokenUsage {
        source_event_type: "opencode.usage.aggregate".to_string(),
        provider: right.provider.or(left.provider),
        backend: right.backend,
        count_source: right.count_source,
        billing_basis: right.billing_basis,
        model: right.model.or(left.model),
        input_tokens: add_token_options(left.input_tokens, right.input_tokens),
        cached_input_tokens: add_token_options(left.cached_input_tokens, right.cached_input_tokens),
        cache_write_tokens: add_token_options(left.cache_write_tokens, right.cache_write_tokens),
        output_tokens: add_token_options(left.output_tokens, right.output_tokens),
        reasoning_tokens: add_token_options(left.reasoning_tokens, right.reasoning_tokens),
        total_tokens: add_token_options(left.total_tokens, right.total_tokens),
    }
}

fn add_token_options(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn codex_token_usage_from_json_event(value: &serde_json::Value) -> Option<ObservedTokenUsage> {
    let raw_event_type = codex_json_event_type(value);
    let source_event_type = normalized_usage_event_type(raw_event_type);
    match source_event_type {
        "turn.completed" => {
            let usage = value.pointer("/usage")?;
            observed_codex_usage_from_value(source_event_type, usage, value)
        }
        "ThreadTokenUsageUpdated" => {
            let token_usage = value
                .pointer("/params/tokenUsage")
                .or_else(|| value.pointer("/params/token_usage"))
                .or_else(|| value.pointer("/payload/tokenUsage"))
                .or_else(|| value.pointer("/payload/token_usage"))
                .or_else(|| value.pointer("/tokenUsage"))
                .or_else(|| value.pointer("/token_usage"))?;
            let turn_usage = token_usage
                .pointer("/last")
                .or_else(|| token_usage.pointer("/lastTurn"))
                .or_else(|| token_usage.pointer("/last_turn"))
                .or_else(|| token_usage.pointer("/total"))
                .unwrap_or(token_usage);
            observed_codex_usage_from_value(source_event_type, turn_usage, value)
        }
        _ => opencode_token_usage_from_json_event(value, source_event_type),
    }
}

fn observed_codex_usage_from_value(
    source_event_type: &str,
    value: &serde_json::Value,
    root: &serde_json::Value,
) -> Option<ObservedTokenUsage> {
    let input_tokens = usage_u64(value, &["input_tokens", "inputTokens"]);
    let cached_input_tokens = usage_u64(value, &["cached_input_tokens", "cachedInputTokens"])
        .or_else(|| {
            usage_u64(
                value,
                &[
                    "input_tokens_details.cached_tokens",
                    "inputTokensDetails.cachedTokens",
                ],
            )
        });
    let output_tokens = usage_u64(value, &["output_tokens", "outputTokens"]);
    let reasoning_tokens = usage_u64(
        value,
        &[
            "reasoning_output_tokens",
            "reasoningOutputTokens",
            "reasoning_tokens",
            "reasoningTokens",
            "output_tokens_details.reasoning_tokens",
            "outputTokensDetails.reasoningTokens",
        ],
    );
    let total_tokens = usage_u64(value, &["total_tokens", "totalTokens"]);
    observed_token_usage(
        source_event_type,
        "codex",
        TokenCountSource::CodexReported,
        "chatgpt_subscription_observed_usage",
        token_usage_provider(value).or_else(|| token_usage_provider(root)),
        token_usage_model(value).or_else(|| token_usage_model(root)),
        input_tokens,
        cached_input_tokens,
        None,
        output_tokens,
        reasoning_tokens,
        total_tokens,
    )
}

fn opencode_token_usage_from_json_event(
    value: &serde_json::Value,
    source_event_type: &str,
) -> Option<ObservedTokenUsage> {
    let candidates = [
        value.pointer("/part"),
        value.pointer("/properties"),
        value.pointer("/data"),
        value.pointer("/message/info"),
        value.pointer("/message/info/assistant"),
        value.pointer("/assistant"),
        Some(value),
    ];
    let event_type = source_event_type;
    let source = if event_type == "step-finish"
        || value
            .pointer("/part/type")
            .and_then(serde_json::Value::as_str)
            == Some("step-finish")
    {
        Some("opencode.step-finish")
    } else if matches!(
        event_type,
        "session.next.step.ended" | "session.next.step.ended.1"
    ) {
        Some("opencode.session.next.step.ended")
    } else {
        None
    };

    for candidate in candidates.into_iter().flatten() {
        let tokens = candidate
            .pointer("/tokens")
            .or_else(|| value.pointer("/tokens"));
        let Some(tokens) = tokens else {
            continue;
        };
        let source_event_type = source.unwrap_or("opencode.usage");
        if let Some(usage) =
            observed_opencode_usage_from_tokens(source_event_type, tokens, value, candidate)
        {
            return Some(usage);
        }
    }
    None
}

fn observed_opencode_usage_from_tokens(
    source_event_type: &str,
    tokens: &serde_json::Value,
    root: &serde_json::Value,
    candidate: &serde_json::Value,
) -> Option<ObservedTokenUsage> {
    let input_tokens = usage_u64(tokens, &["input", "input_tokens", "inputTokens"]);
    let output_tokens = usage_u64(tokens, &["output", "output_tokens", "outputTokens"]);
    let reasoning_tokens = usage_u64(
        tokens,
        &["reasoning", "reasoning_tokens", "reasoningTokens"],
    );
    let cached_input_tokens = usage_u64(
        tokens,
        &[
            "cache.read",
            "cache_read",
            "cache_read_tokens",
            "cacheReadTokens",
        ],
    );
    let cache_write_tokens = usage_u64(
        tokens,
        &[
            "cache.write",
            "cache_write",
            "cache_write_tokens",
            "cacheWriteTokens",
            "cache_write_5m",
            "cache_write_5m_tokens",
            "cacheWrite5mTokens",
            "cache_write_1h",
            "cache_write_1h_tokens",
            "cacheWrite1hTokens",
        ],
    );
    let explicit_total = usage_u64(tokens, &["total", "total_tokens", "totalTokens"]);
    let computed_total = [
        input_tokens,
        output_tokens,
        reasoning_tokens,
        cached_input_tokens,
        cache_write_tokens,
    ]
    .into_iter()
    .flatten()
    .sum::<u64>();
    let total_tokens = explicit_total.or_else(|| {
        if computed_total > 0 {
            Some(computed_total)
        } else {
            None
        }
    });
    observed_token_usage(
        source_event_type,
        "opencode",
        TokenCountSource::ParsedFromStream,
        "opencode_observed_usage",
        token_usage_provider(candidate).or_else(|| token_usage_provider(root)),
        token_usage_model(candidate).or_else(|| token_usage_model(root)),
        input_tokens,
        cached_input_tokens,
        cache_write_tokens,
        output_tokens,
        reasoning_tokens,
        total_tokens,
    )
}

fn observed_token_usage(
    source_event_type: &str,
    backend: &str,
    count_source: TokenCountSource,
    billing_basis: &str,
    provider: Option<String>,
    model: Option<String>,
    input_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    total_tokens: Option<u64>,
) -> Option<ObservedTokenUsage> {
    if input_tokens.is_none()
        && cached_input_tokens.is_none()
        && cache_write_tokens.is_none()
        && output_tokens.is_none()
        && reasoning_tokens.is_none()
        && total_tokens.is_none()
    {
        return None;
    }
    Some(ObservedTokenUsage {
        source_event_type: source_event_type.to_string(),
        provider,
        backend: backend.to_string(),
        count_source,
        billing_basis: billing_basis.to_string(),
        model,
        input_tokens,
        cached_input_tokens,
        cache_write_tokens,
        output_tokens,
        reasoning_tokens,
        total_tokens,
    })
}

fn usage_u64(value: &serde_json::Value, paths: &[&str]) -> Option<u64> {
    for path in paths {
        let pointer = format!("/{}", path.replace('.', "/"));
        if let Some(number) = value.pointer(&pointer).and_then(serde_json::Value::as_u64) {
            return Some(number);
        }
    }
    None
}

fn token_usage_model(value: &serde_json::Value) -> Option<String> {
    value
        .pointer("/model")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .pointer("/modelID")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/model_id")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/model/id")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/provider/model")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/params/model")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/response/model")
                .and_then(serde_json::Value::as_str)
        })
        .filter(|model| !contains_forbidden_metadata(model))
        .map(str::to_string)
}

fn token_usage_provider(value: &serde_json::Value) -> Option<String> {
    value
        .pointer("/provider")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .pointer("/providerID")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/provider_id")
                .and_then(serde_json::Value::as_str)
        })
        .filter(|provider| !contains_forbidden_metadata(provider))
        .map(str::to_string)
}

fn codex_json_event_type(value: &serde_json::Value) -> &str {
    value
        .pointer("/type")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.pointer("/event").and_then(serde_json::Value::as_str))
        .or_else(|| value.pointer("/method").and_then(serde_json::Value::as_str))
        .unwrap_or("message")
}

fn normalized_usage_event_type(event_type: &str) -> &str {
    match event_type {
        "thread/tokenUsage/updated"
        | "thread.token_usage.updated"
        | "thread.tokenUsage.updated"
        | "ThreadTokenUsageUpdated" => "ThreadTokenUsageUpdated",
        other => other,
    }
}

fn current_resource_host_snapshot() -> ResourceHostSnapshot {
    let mut system = System::new_all();
    system.refresh_all();
    let disks = Disks::new_with_refreshed_list();
    let cpu_brand = system
        .cpus()
        .first()
        .map(|cpu| safe_receipt_token(cpu.brand()))
        .filter(|brand| !brand.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let logical_cpu_count = system.cpus().len().max(1) as u64;
    ResourceHostSnapshot {
        os: safe_receipt_token(
            System::long_os_version()
                .unwrap_or_else(|| std::env::consts::OS.to_string())
                .as_str(),
        ),
        arch: safe_receipt_token(System::cpu_arch().as_str()),
        cpu: cpu_brand,
        logical_cpu_count,
        physical_cpu_count: System::physical_core_count().map(|count| count as u64),
        memory_total_bytes: Some(system.total_memory()),
        memory_available_bytes: Some(system.available_memory()),
        disk_total_bytes: Some(disks.iter().map(|disk| disk.total_space()).sum::<u64>()),
        disk_available_bytes: Some(disks.iter().map(|disk| disk.available_space()).sum::<u64>()),
        accelerator_inventory: detect_accelerators_for_receipt(),
        virtualization: VirtualizationFacts {
            kvm_present: Path::new("/dev/kvm").exists(),
            firecracker_candidate: cfg!(target_os = "linux") && Path::new("/dev/kvm").exists(),
            container_runtime: detect_container_runtime(),
            cgroup_mode: detect_cgroup_mode(),
        },
    }
}

fn provider_lane_for_node(node_ref: &str) -> ProviderLane {
    let lower = node_ref.to_ascii_lowercase();
    if lower.contains("shc") {
        ProviderLane::Shc
    } else if lower.contains("gcp") || lower.contains("gce") {
        ProviderLane::Gcp
    } else if lower.contains("local") {
        ProviderLane::Local
    } else if lower.contains("provider") || lower.contains("pylon") {
        ProviderLane::Provider
    } else {
        ProviderLane::Unknown
    }
}

fn detect_accelerators_for_receipt() -> Vec<String> {
    let mut accelerators = Vec::new();
    if Path::new("/dev/nvidia0").exists() {
        accelerators.push("nvidia_gpu".to_string());
    }
    if Path::new("/dev/dri").exists() {
        accelerators.push("dri".to_string());
    }
    accelerators
}

fn detect_container_runtime() -> Option<String> {
    if Path::new("/.dockerenv").exists() {
        return Some("docker".to_string());
    }
    env::var("container")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| safe_receipt_token(value.as_str()))
}

fn detect_cgroup_mode() -> Option<String> {
    if Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
        Some("v2".to_string())
    } else if Path::new("/sys/fs/cgroup").exists() {
        Some("v1_or_hybrid".to_string())
    } else {
        None
    }
}

fn directory_size_if_exists(path: &Path) -> Option<u64> {
    if !path.exists() {
        return None;
    }
    directory_size(path).ok()
}

fn directory_size(path: &Path) -> Result<u64, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to stat {}: {error}", path_label(path)))?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in fs::read_dir(path)
        .map_err(|error| format!("failed to read dir {}: {error}", path_label(path)))?
    {
        let entry = entry.map_err(|error| format!("failed to read dir entry: {error}"))?;
        total = total.saturating_add(directory_size(entry.path().as_path())?);
    }
    Ok(total)
}

fn resource_log_bytes(state_dir: &Path) -> Option<u64> {
    let mut total = 0u64;
    let mut found = false;
    for path in [
        codex_run_event_log_path(state_dir),
        codex_session_event_log_path(state_dir),
        openagents_runner_event_log_path(state_dir),
    ] {
        if let Ok(metadata) = fs::metadata(path) {
            total = total.saturating_add(metadata.len());
            found = true;
        }
    }
    found.then_some(total)
}

fn safe_receipt_token(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars().take(96) {
        if ch.is_ascii_alphanumeric()
            || matches!(ch, '-' | '_' | '.' | ':' | '/' | '@' | '+' | '=' | '#')
        {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.trim_matches('_').is_empty() {
        "unknown".to_string()
    } else {
        out.trim_matches('_').to_string()
    }
}

fn codex_sandbox_label(sandbox: &CodexSandboxMode) -> &'static str {
    match sandbox {
        CodexSandboxMode::ReadOnly => "read-only",
        CodexSandboxMode::WorkspaceWrite => "workspace-write",
        CodexSandboxMode::DangerFullAccess => "danger-full-access",
    }
}

fn record_codex_output_events(
    state_dir: &Path,
    state: &mut CodexRunState,
    stream: &str,
    bytes: &[u8],
) -> Result<(), String> {
    if bytes.is_empty() {
        return Ok(());
    }
    let rendered = String::from_utf8_lossy(bytes);
    for line in rendered.lines().take(64) {
        if line.trim().is_empty() {
            continue;
        }
        let message = format!("{stream}: {}", truncate_for_event(line.trim()));
        let (kind, message) = if contains_forbidden_metadata(message.as_str()) {
            (
                CodexWorkroomEventKind::Redacted,
                format!("{stream}: redacted forbidden output"),
            )
        } else {
            (CodexWorkroomEventKind::Log, message)
        };
        push_codex_run_event(
            state_dir,
            state,
            kind,
            CodexWorkroomDecision::Accepted,
            message.as_str(),
            None,
            None,
        )?;
    }
    Ok(())
}

fn capture_codex_artifacts(
    state_dir: &Path,
    workspace_dir: &Path,
    state: &mut CodexRunState,
) -> Result<(), String> {
    artifact_policy_init(ArtifactPolicyInitArgs {
        state_dir: state_dir.to_path_buf(),
        required_artifacts: state.assignment.required_artifacts.clone(),
        json: true,
    })?;

    for name in state.assignment.required_artifacts.clone() {
        let artifact_path = workspace_dir.join(&name);
        if !artifact_path.exists() {
            state.status = "failed".to_string();
            push_codex_run_event(
                state_dir,
                state,
                CodexWorkroomEventKind::Failed,
                CodexWorkroomDecision::Failed,
                format!("missing required artifact {name}").as_str(),
                None,
                None,
            )?;
            return Err(format!("missing required artifact {name}"));
        }
        let uploaded = artifact_upload(ArtifactUploadArgs {
            state_dir: state_dir.to_path_buf(),
            name: name.clone(),
            file: artifact_path,
            json: true,
        })?;
        let parsed: serde_json::Value = serde_json::from_str(&uploaded)
            .map_err(|error| format!("failed to parse artifact upload output: {error}"))?;
        let content_digest = parsed
            .pointer("/artifact/content_digest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "artifact upload output missing content_digest".to_string())?
            .to_string();
        let object_path = parsed
            .pointer("/artifact/object_path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "artifact upload output missing object_path".to_string())?
            .to_string();
        let receipt_digest = parsed
            .pointer("/receipt/receipt_digest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "artifact upload output missing receipt_digest".to_string())?
            .to_string();
        state.artifact_refs.push(CodexRunArtifactRef {
            name: name.clone(),
            content_digest: content_digest.clone(),
            object_path,
        });
        state.receipt_refs.push(receipt_digest.clone());
        push_codex_run_event(
            state_dir,
            state,
            CodexWorkroomEventKind::Artifact,
            CodexWorkroomDecision::Accepted,
            "artifact captured",
            Some(content_digest.as_str()),
            Some(receipt_digest.as_str()),
        )?;
        append_openagents_runner_event(
            state_dir,
            state.assignment.assignment_id.as_str(),
            state.assignment.workroom_id.as_str(),
            "artifact.created",
            "runner",
            "Codex artifact captured.",
            Some(name.as_str()),
            vec![content_digest.clone()],
            vec![receipt_digest.clone()],
        )?;
    }

    let closeout = closeout_submit(CloseoutArgs {
        state_dir: state_dir.to_path_buf(),
        json: true,
    })?;
    let parsed: serde_json::Value = serde_json::from_str(&closeout)
        .map_err(|error| format!("failed to parse closeout output: {error}"))?;
    let closeout_receipt = parsed
        .pointer("/receipt/receipt_digest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "closeout output missing receipt_digest".to_string())?
        .to_string();
    state.receipt_refs.push(closeout_receipt.clone());
    push_codex_run_event(
        state_dir,
        state,
        CodexWorkroomEventKind::Receipt,
        CodexWorkroomDecision::Accepted,
        "closeout receipt emitted",
        None,
        Some(closeout_receipt.as_str()),
    )?;
    append_openagents_runner_event(
        state_dir,
        state.assignment.assignment_id.as_str(),
        state.assignment.workroom_id.as_str(),
        "receipt.created",
        "runner",
        "Codex closeout receipt emitted.",
        None,
        Vec::new(),
        vec![closeout_receipt],
    )?;
    Ok(())
}

fn push_codex_run_event(
    state_dir: &Path,
    state: &mut CodexRunState,
    event_kind: CodexWorkroomEventKind,
    decision: CodexWorkroomDecision,
    message: &str,
    artifact_ref: Option<&str>,
    receipt_ref: Option<&str>,
) -> Result<(), String> {
    let (event_kind, message) = if contains_forbidden_metadata(message) {
        (
            CodexWorkroomEventKind::Redacted,
            "redacted forbidden event metadata".to_string(),
        )
    } else {
        (event_kind, message.to_string())
    };
    let emitted_at_ms = now_epoch_ms()?;
    let sequence = state.events.len() as u64 + 1;
    let encoded = serde_json::to_vec(&(
        CODEX_WORKROOM_EVENT_VERSION,
        state.assignment.assignment_id.as_str(),
        state.assignment.workroom_id.as_str(),
        sequence,
        &event_kind,
        &decision,
        message.as_str(),
        artifact_ref,
        receipt_ref,
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize codex event digest material: {error}"))?;
    let event = CodexWorkroomEvent {
        schema_version: CODEX_WORKROOM_EVENT_VERSION.to_string(),
        event_id: format!(
            "codex.workroom.{}.{}.{emitted_at_ms}",
            safe_path_component(state.assignment.assignment_id.as_str()),
            sequence
        ),
        assignment_id: state.assignment.assignment_id.clone(),
        workroom_id: state.assignment.workroom_id.clone(),
        sequence,
        event_kind,
        decision,
        message,
        artifact_ref: artifact_ref.map(str::to_string),
        receipt_ref: receipt_ref.map(str::to_string),
        event_digest: sha256_prefixed(&encoded),
        emitted_at_ms,
    };
    event.validate_contract()?;
    append_codex_run_event(state_dir, &event)?;
    state.updated_at_ms = emitted_at_ms;
    state.receipt_refs.push(event.event_digest.clone());
    state.events.push(event);
    Ok(())
}

fn append_codex_run_event(state_dir: &Path, event: &CodexWorkroomEvent) -> Result<(), String> {
    event.validate_contract()?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(event)
        .map_err(|error| format!("failed to serialize codex run event: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(codex_run_event_log_path(state_dir))
        .map_err(|error| format!("failed to open codex run event log: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to append codex run event: {error}"))
}

fn save_codex_run_state(state_dir: &Path, state: &CodexRunState) -> Result<(), String> {
    validate_codex_run_state(state)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize codex run state: {error}"))?;
    fs::write(codex_run_state_path(state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write codex run state: {error}"))
}

fn validate_codex_run_state(state: &CodexRunState) -> Result<(), String> {
    if state.schema_version != CODEX_RUN_STATE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported codex run state schema '{}'",
            state.schema_version
        ));
    }
    if contains_forbidden_metadata(state.workspace_dir.as_str()) {
        return Err("codex run state contains forbidden workspace metadata".to_string());
    }
    for artifact in &state.artifact_refs {
        validate_artifact_name(artifact.name.as_str())?;
        if !artifact.content_digest.starts_with("sha256:") {
            return Err("codex run artifact digest must start with sha256:".to_string());
        }
        if contains_forbidden_metadata(artifact.object_path.as_str()) {
            return Err("codex run artifact object path contains forbidden marker".to_string());
        }
    }
    for receipt_ref in &state.receipt_refs {
        if !receipt_ref.starts_with("sha256:") || contains_forbidden_metadata(receipt_ref) {
            return Err("codex run receipt ref is invalid".to_string());
        }
    }
    for event in &state.events {
        event.validate_contract()?;
    }
    Ok(())
}

fn ensure_session_can_run(session: &CodexSessionState) -> Result<(), String> {
    ensure_session_not_destroyed(session)?;
    if session.status == "running" {
        return Err("codex session already has a running turn".to_string());
    }
    if session.status == "archived" {
        return Err("codex session is archived".to_string());
    }
    if let Some(expires_at_ms) = session.expires_at_ms {
        if expires_at_ms <= now_epoch_ms()? {
            return Err("codex session retention TTL has expired".to_string());
        }
    }
    Ok(())
}

fn ensure_session_not_destroyed(session: &CodexSessionState) -> Result<(), String> {
    if session.status == "destroyed" || session.destroyed_at_ms.is_some() {
        return Err("codex session is destroyed".to_string());
    }
    Ok(())
}

fn record_codex_session_output_events(
    state_dir: &Path,
    session: &mut CodexSessionState,
    stream: &str,
    bytes: &[u8],
) -> Result<(), String> {
    if bytes.is_empty() {
        return Ok(());
    }
    let rendered = String::from_utf8_lossy(bytes);
    for line in rendered.lines().take(64) {
        if line.trim().is_empty() {
            continue;
        }
        let message = format!("{stream}: {}", truncate_for_event(line.trim()));
        let (kind, message) = if contains_forbidden_metadata(message.as_str()) {
            (
                CodexWorkroomEventKind::Redacted,
                format!("{stream}: redacted forbidden output"),
            )
        } else {
            (CodexWorkroomEventKind::Log, message)
        };
        push_codex_session_event(
            state_dir,
            session,
            kind,
            CodexWorkroomDecision::Accepted,
            message.as_str(),
            None,
            None,
        )?;
    }
    Ok(())
}

fn push_codex_session_event(
    state_dir: &Path,
    session: &mut CodexSessionState,
    event_kind: CodexWorkroomEventKind,
    decision: CodexWorkroomDecision,
    message: &str,
    artifact_ref: Option<&str>,
    receipt_ref: Option<&str>,
) -> Result<(), String> {
    let (event_kind, message) = if contains_forbidden_metadata(message) {
        (
            CodexWorkroomEventKind::Redacted,
            "redacted forbidden event metadata".to_string(),
        )
    } else {
        (event_kind, message.to_string())
    };
    let emitted_at_ms = now_epoch_ms()?;
    let sequence = session.events.len() as u64 + 1;
    let encoded = serde_json::to_vec(&(
        CODEX_WORKROOM_EVENT_VERSION,
        session.assignment.assignment_id.as_str(),
        session.assignment.workroom_id.as_str(),
        sequence,
        &event_kind,
        &decision,
        message.as_str(),
        artifact_ref,
        receipt_ref,
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize codex session event digest material: {error}"))?;
    let event = CodexWorkroomEvent {
        schema_version: CODEX_WORKROOM_EVENT_VERSION.to_string(),
        event_id: format!(
            "codex.session.{}.{}.{emitted_at_ms}",
            safe_path_component(session.assignment.assignment_id.as_str()),
            sequence
        ),
        assignment_id: session.assignment.assignment_id.clone(),
        workroom_id: session.assignment.workroom_id.clone(),
        sequence,
        event_kind,
        decision,
        message,
        artifact_ref: artifact_ref.map(str::to_string),
        receipt_ref: receipt_ref.map(str::to_string),
        event_digest: sha256_prefixed(&encoded),
        emitted_at_ms,
    };
    event.validate_contract()?;
    append_codex_session_event(state_dir, &event)?;
    session.updated_at_ms = emitted_at_ms;
    session.receipt_refs.push(event.event_digest.clone());
    session.events.push(event);
    Ok(())
}

fn append_codex_session_event(state_dir: &Path, event: &CodexWorkroomEvent) -> Result<(), String> {
    event.validate_contract()?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(event)
        .map_err(|error| format!("failed to serialize codex session event: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(codex_session_event_log_path(state_dir))
        .map_err(|error| format!("failed to open codex session event log: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("failed to append codex session event: {error}"))
}

fn render_session_events_jsonl(events: &[CodexWorkroomEvent]) -> Result<String, String> {
    let mut lines = Vec::new();
    for event in events {
        lines.push(
            serde_json::to_string(event)
                .map_err(|error| format!("failed to serialize codex session event: {error}"))?,
        );
    }
    Ok(lines.join("\n"))
}

fn load_codex_session_state(state_dir: &Path) -> Result<CodexSessionState, String> {
    let path = codex_session_state_path(state_dir);
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read codex session state {}: {error}",
            path_label(&path)
        )
    })?;
    let state = serde_json::from_str::<CodexSessionState>(&raw).map_err(|error| {
        format!(
            "failed to parse codex session state {}: {error}",
            path_label(&path)
        )
    })?;
    validate_codex_session_state(&state)?;
    Ok(state)
}

fn save_codex_session_state(state_dir: &Path, state: &CodexSessionState) -> Result<(), String> {
    validate_codex_session_state(state)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize codex session state: {error}"))?;
    fs::write(
        codex_session_state_path(state_dir),
        format!("{serialized}\n"),
    )
    .map_err(|error| format!("failed to write codex session state: {error}"))
}

fn validate_codex_session_state(state: &CodexSessionState) -> Result<(), String> {
    if state.schema_version != CODEX_SESSION_SCHEMA_VERSION {
        return Err(format!(
            "unsupported codex session state schema '{}'",
            state.schema_version
        ));
    }
    state.assignment.validate_contract(state.created_at_ms)?;
    if contains_forbidden_metadata(state.workspace_dir.as_str()) {
        return Err("codex session workspace path contains forbidden marker".to_string());
    }
    for artifact in &state.artifact_refs {
        if !artifact.content_digest.starts_with("sha256:")
            || contains_forbidden_metadata(artifact.object_path.as_str())
        {
            return Err("codex session artifact ref is invalid".to_string());
        }
    }
    for receipt_ref in &state.receipt_refs {
        if !receipt_ref.starts_with("sha256:") || contains_forbidden_metadata(receipt_ref) {
            return Err("codex session receipt ref is invalid".to_string());
        }
    }
    for event in &state.events {
        event.validate_contract()?;
    }
    Ok(())
}

fn append_openagents_runner_event(
    state_dir: &Path,
    assignment_id: &str,
    workroom_id: &str,
    event_type: &str,
    source: &str,
    summary: &str,
    detail: Option<&str>,
    artifact_refs: Vec<String>,
    receipt_refs: Vec<String>,
) -> Result<OpenAgentsRunnerEvent, String> {
    append_openagents_runner_event_with_payload(
        state_dir,
        assignment_id,
        workroom_id,
        event_type,
        source,
        summary,
        detail,
        None,
        artifact_refs,
        receipt_refs,
    )
}

fn append_openagents_runner_event_with_payload(
    state_dir: &Path,
    assignment_id: &str,
    workroom_id: &str,
    event_type: &str,
    source: &str,
    summary: &str,
    detail: Option<&str>,
    raw_payload_json: Option<&str>,
    artifact_refs: Vec<String>,
    receipt_refs: Vec<String>,
) -> Result<OpenAgentsRunnerEvent, String> {
    if contains_forbidden_metadata(event_type)
        || contains_forbidden_metadata(source)
        || contains_forbidden_metadata(summary)
        || detail.is_some_and(contains_forbidden_metadata)
        || raw_payload_json.is_some_and(contains_forbidden_metadata)
    {
        return append_openagents_runner_event_with_payload(
            state_dir,
            assignment_id,
            workroom_id,
            "redacted",
            "runner",
            "Runner event was redacted before persistence.",
            None,
            None,
            Vec::new(),
            Vec::new(),
        );
    }
    let sequence = load_openagents_runner_events(state_dir)?.len() as u64 + 1;
    let emitted_at_ms = now_epoch_ms()?;
    let detail_excerpt = detail.map(truncate_for_event);
    let raw_payload_digest = detail_excerpt
        .as_ref()
        .map(|value| sha256_prefixed(value.as_bytes()));
    let event = OpenAgentsRunnerEvent {
        schema_version: OPENAGENTS_RUNNER_EVENT_SCHEMA_VERSION.to_string(),
        external_event_id: format!(
            "runner.{}.{}.{emitted_at_ms}",
            safe_path_component(assignment_id),
            sequence
        ),
        sequence,
        event_type: event_type.to_string(),
        source: source.to_string(),
        summary: summary.to_string(),
        detail_excerpt,
        raw_payload_digest,
        raw_payload_json: raw_payload_json.map(truncate_raw_payload_json),
        artifact_refs,
        receipt_refs,
        emitted_at_ms,
    };
    validate_openagents_runner_event(&event)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(&event)
        .map_err(|error| format!("failed to serialize runner event: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(openagents_runner_event_log_path(state_dir))
        .map_err(|error| format!("failed to open runner event log: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to append runner event: {error}"))?;
    let _ = workroom_id;
    Ok(event)
}

fn validate_openagents_runner_event(event: &OpenAgentsRunnerEvent) -> Result<(), String> {
    if event.schema_version != OPENAGENTS_RUNNER_EVENT_SCHEMA_VERSION {
        return Err("unsupported runner event schema".to_string());
    }
    for value in [
        event.event_type.as_str(),
        event.source.as_str(),
        event.summary.as_str(),
        event.detail_excerpt.as_deref().unwrap_or_default(),
        event.raw_payload_json.as_deref().unwrap_or_default(),
    ] {
        if contains_forbidden_metadata(value) {
            return Err("runner event contains forbidden marker".to_string());
        }
    }
    for artifact_ref in &event.artifact_refs {
        if contains_forbidden_metadata(artifact_ref) {
            return Err("runner event artifact ref contains forbidden marker".to_string());
        }
    }
    for receipt_ref in &event.receipt_refs {
        if contains_forbidden_metadata(receipt_ref) {
            return Err("runner event receipt ref contains forbidden marker".to_string());
        }
    }
    Ok(())
}

fn load_openagents_runner_events(state_dir: &Path) -> Result<Vec<OpenAgentsRunnerEvent>, String> {
    let path = openagents_runner_event_log_path(state_dir);
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let event = serde_json::from_str::<OpenAgentsRunnerEvent>(line)
                .map_err(|error| format!("failed to parse runner event: {error}"))?;
            validate_openagents_runner_event(&event)?;
            Ok(event)
        })
        .collect()
}

fn record_codex_runner_output_events(
    state_dir: &Path,
    assignment: &CodexWorkroomAssignment,
    stream: &str,
    bytes: &[u8],
) -> Result<(), String> {
    if bytes.is_empty() {
        return Ok(());
    }
    let rendered = String::from_utf8_lossy(bytes);
    for line in rendered.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if contains_forbidden_metadata(trimmed) {
            append_openagents_runner_event(
                state_dir,
                assignment.assignment_id.as_str(),
                assignment.workroom_id.as_str(),
                "redacted",
                "codex",
                "Codex output was redacted before runner-event persistence.",
                None,
                Vec::new(),
                Vec::new(),
            )?;
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            for event in codex_json_to_runner_events(&value, stream) {
                append_openagents_runner_event_with_payload(
                    state_dir,
                    assignment.assignment_id.as_str(),
                    assignment.workroom_id.as_str(),
                    event.event_type.as_str(),
                    "codex",
                    event.summary.as_str(),
                    event.detail.as_deref(),
                    event.raw_payload_json.as_deref(),
                    Vec::new(),
                    Vec::new(),
                )?;
            }
        }
    }
    Ok(())
}

struct RunnerEventDraft {
    event_type: String,
    summary: String,
    detail: Option<String>,
    raw_payload_json: Option<String>,
}

fn codex_json_to_runner_events(value: &serde_json::Value, stream: &str) -> Vec<RunnerEventDraft> {
    let event_type = normalized_usage_event_type(codex_json_event_type(value));
    let text = value
        .pointer("/delta")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .pointer("/properties/delta")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/message")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/content")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| value.pointer("/output").and_then(serde_json::Value::as_str))
        .or_else(|| {
            value
                .pointer("/properties/part/text")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/part/text")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/properties/part/content")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/part/content")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/properties/part/state/output")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/part/state/output")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/item/content/0/text")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/item/text")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/item/aggregated_output")
                .and_then(serde_json::Value::as_str)
        })
        .map(truncate_for_event);
    let command = value
        .pointer("/cmd")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .pointer("/command")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/properties/command")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/properties/part/state/input/command")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/part/state/input/command")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/properties/part/state/input/cmd")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/arguments/cmd")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/item/command")
                .and_then(serde_json::Value::as_str)
        })
        .map(truncate_for_event);
    let tool_name = value
        .pointer("/name")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .pointer("/tool_name")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/properties/part/tool")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/properties/part/toolName")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/item/name")
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or("tool");
    let part_type = value
        .pointer("/properties/part/type")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .pointer("/part/type")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/item/type")
                .and_then(serde_json::Value::as_str)
        });
    let part_status = value
        .pointer("/properties/part/state/status")
        .and_then(serde_json::Value::as_str);

    if let Some(usage) = codex_token_usage_from_json_event(value) {
        let summary = match usage.source_event_type.as_str() {
            "turn.completed" => "Codex turn completed with token usage.",
            "ThreadTokenUsageUpdated" => "Codex app-server token usage updated.",
            "opencode.step-finish" => "OpenCode step token usage captured.",
            "opencode.session.next.step.ended" => "OpenCode step-ended token usage captured.",
            _ => "Model token usage captured.",
        };
        return vec![RunnerEventDraft {
            event_type: usage.source_event_type.clone(),
            summary: summary.to_string(),
            detail: Some(token_usage_detail(usage)),
            raw_payload_json: raw_payload_json(value),
        }];
    }

    let draft = match event_type {
        "message_delta"
        | "response.output_text.delta"
        | "assistant_delta"
        | "tool_call_delta"
        | "function_call_delta"
        | "exec_command_output_delta"
        | "shell_output_delta"
        | "command_output"
        | "message.part.delta" => return Vec::new(),
        "message.part.updated" if part_type == Some("text") => RunnerEventDraft {
            event_type: "message.completed".to_string(),
            summary: "Assistant message updated.".to_string(),
            detail: text,
            raw_payload_json: raw_payload_json(value),
        },
        "text" if part_type == Some("text") => RunnerEventDraft {
            event_type: "message.completed".to_string(),
            summary: "Assistant message completed.".to_string(),
            detail: text,
            raw_payload_json: raw_payload_json(value),
        },
        "message.part.updated" if part_type == Some("reasoning") => RunnerEventDraft {
            event_type: "reasoning.completed".to_string(),
            summary: "Reasoning updated.".to_string(),
            detail: text,
            raw_payload_json: raw_payload_json(value),
        },
        "message.part.updated" if part_type == Some("tool") => {
            let completed = part_status == Some("completed");
            let failed = part_status == Some("error");
            RunnerEventDraft {
                event_type: if completed {
                    "tool.call.completed".to_string()
                } else if failed {
                    "tool.call.failed".to_string()
                } else {
                    "tool.call.started".to_string()
                },
                summary: if completed {
                    format!("Tool call completed: {tool_name}.")
                } else if failed {
                    format!("Tool call failed: {tool_name}.")
                } else {
                    format!("Tool call started: {tool_name}.")
                },
                detail: command.or(text),
                raw_payload_json: raw_payload_json(value),
            }
        }
        "message.part.updated" if part_type == Some("file") => RunnerEventDraft {
            event_type: "file.edit".to_string(),
            summary: "File part updated.".to_string(),
            detail: text.or(command),
            raw_payload_json: raw_payload_json(value),
        },
        "session.next.shell.started" => RunnerEventDraft {
            event_type: "shell.command.started".to_string(),
            summary: "Shell command started.".to_string(),
            detail: command.or(text),
            raw_payload_json: raw_payload_json(value),
        },
        "session.next.shell.ended" => RunnerEventDraft {
            event_type: "shell.command.completed".to_string(),
            summary: "Shell command completed.".to_string(),
            detail: command.or(text),
            raw_payload_json: raw_payload_json(value),
        },
        "item.started" if part_type.is_some_and(|kind| kind.contains("function_call")) => {
            RunnerEventDraft {
                event_type: "tool.call.started".to_string(),
                summary: format!("Tool call started: {tool_name}."),
                detail: command.or(text),
                raw_payload_json: raw_payload_json(value),
            }
        }
        "item.completed" if part_type.is_some_and(|kind| kind.contains("function_call")) => {
            RunnerEventDraft {
                event_type: "tool.call.completed".to_string(),
                summary: format!("Tool call completed: {tool_name}."),
                detail: command.or(text),
                raw_payload_json: raw_payload_json(value),
            }
        }
        "item.started" if part_type == Some("command_execution") => RunnerEventDraft {
            event_type: "shell.command.started".to_string(),
            summary: "Shell command started.".to_string(),
            detail: command.or(text),
            raw_payload_json: raw_payload_json(value),
        },
        "item.completed" if part_type == Some("command_execution") => RunnerEventDraft {
            event_type: "shell.command.completed".to_string(),
            summary: "Shell command completed.".to_string(),
            detail: command.or(text),
            raw_payload_json: raw_payload_json(value),
        },
        "item.started" | "item.completed" if part_type == Some("file_change") => RunnerEventDraft {
            event_type: "file.edit".to_string(),
            summary: "File change captured.".to_string(),
            detail: text.or(command),
            raw_payload_json: raw_payload_json(value),
        },
        "item.completed" if part_type == Some("message") || part_type == Some("agent_message") => {
            RunnerEventDraft {
                event_type: "message.completed".to_string(),
                summary: "Assistant message completed.".to_string(),
                detail: text,
                raw_payload_json: raw_payload_json(value),
            }
        }
        "message" | "message_completed" | "response.output_item.done" => RunnerEventDraft {
            event_type: "message.completed".to_string(),
            summary: "Assistant message completed.".to_string(),
            detail: text,
            raw_payload_json: raw_payload_json(value),
        },
        "tool_call" | "tool_call_started" | "function_call" => RunnerEventDraft {
            event_type: "tool.call.started".to_string(),
            summary: format!("Tool call started: {tool_name}."),
            detail: command.or(text),
            raw_payload_json: raw_payload_json(value),
        },
        "tool_call_completed" | "function_call_output" => RunnerEventDraft {
            event_type: "tool.call.completed".to_string(),
            summary: format!("Tool call completed: {tool_name}."),
            detail: command.or(text),
            raw_payload_json: raw_payload_json(value),
        },
        "exec_command_begin" | "shell_command_started" | "command_started" => RunnerEventDraft {
            event_type: "shell.command.started".to_string(),
            summary: "Shell command started.".to_string(),
            detail: command.or(text),
            raw_payload_json: raw_payload_json(value),
        },
        "exec_command_end" | "shell_command_completed" | "command_completed" => RunnerEventDraft {
            event_type: "shell.command.completed".to_string(),
            summary: "Shell command completed.".to_string(),
            detail: command.or(text),
            raw_payload_json: raw_payload_json(value),
        },
        "file_change" | "file_edit" | "patch" => RunnerEventDraft {
            event_type: "file.edit".to_string(),
            summary: "File edit event captured.".to_string(),
            detail: text.or(command),
            raw_payload_json: raw_payload_json(value),
        },
        "error" | "failed" => RunnerEventDraft {
            event_type: "run.failed".to_string(),
            summary: "Codex reported a failure event.".to_string(),
            detail: text.or(command),
            raw_payload_json: raw_payload_json(value),
        },
        _ => RunnerEventDraft {
            event_type: event_type.to_string(),
            summary: format!("{stream} JSON event captured."),
            detail: text.or(command),
            raw_payload_json: raw_payload_json(value),
        },
    };
    vec![draft]
}

fn token_usage_detail(usage: ObservedTokenUsage) -> String {
    let mut parts = Vec::new();
    if let Some(input_tokens) = usage.input_tokens {
        parts.push(format!("input: {input_tokens}"));
    }
    if let Some(cached_input_tokens) = usage.cached_input_tokens {
        parts.push(format!("cached input: {cached_input_tokens}"));
    }
    if let Some(cache_write_tokens) = usage.cache_write_tokens {
        parts.push(format!("cache write: {cache_write_tokens}"));
    }
    if let Some(output_tokens) = usage.output_tokens {
        parts.push(format!("output: {output_tokens}"));
    }
    if let Some(reasoning_tokens) = usage.reasoning_tokens {
        parts.push(format!("reasoning output: {reasoning_tokens}"));
    }
    if let Some(total_tokens) = usage.total_tokens {
        parts.push(format!("total: {total_tokens}"));
    }
    parts.join(", ")
}

fn raw_payload_json(value: &serde_json::Value) -> Option<String> {
    serde_json::to_string(value).ok()
}

fn truncate_raw_payload_json(value: &str) -> String {
    value.chars().take(16_384).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes tests that mutate process-global GitHub-token env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn execution_from_stdout(lines: &[serde_json::Value]) -> CodexExecutionResult {
        let stdout = lines
            .iter()
            .map(|value| serde_json::to_string(value).expect("fixture serializes"))
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes();
        CodexExecutionResult {
            exit_code: Some(0),
            artifact_completed: false,
            timed_out: false,
            wall_time_ms: 1,
            stdout,
            stderr: Vec::new(),
        }
    }

    #[test]
    fn extracts_opencode_step_finish_usage_for_any_provider_model() {
        let event = serde_json::json!({
            "provider": "anthropic",
            "model": "claude-sonnet-4-5",
            "part": {
                "type": "step-finish",
                "cost": 0.01,
                "tokens": {
                    "input": 100,
                    "output": 50,
                    "reasoning": 20,
                    "cache": { "read": 30, "write": 40 }
                }
            }
        });

        let usage = codex_token_usage_from_json_event(&event).expect("usage is extracted");
        assert_eq!(usage.backend, "opencode");
        assert_eq!(usage.source_event_type, "opencode.step-finish");
        assert_eq!(usage.provider.as_deref(), Some("anthropic"));
        assert_eq!(usage.model.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(usage.count_source, TokenCountSource::ParsedFromStream);
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.output_tokens, Some(50));
        assert_eq!(usage.reasoning_tokens, Some(20));
        assert_eq!(usage.cached_input_tokens, Some(30));
        assert_eq!(usage.cache_write_tokens, Some(40));
        assert_eq!(usage.total_tokens, Some(240));

        let runner_events = codex_json_to_runner_events(&event, "stdout");
        assert_eq!(runner_events.len(), 1);
        assert_eq!(runner_events[0].event_type, "opencode.step-finish");
        assert!(runner_events[0]
            .raw_payload_json
            .as_deref()
            .is_some_and(|payload| payload.contains("claude-sonnet-4-5")));
    }

    #[test]
    fn extracts_opencode_session_step_ended_usage_event() {
        let event = serde_json::json!({
            "id": "evt_test",
            "type": "session.next.step.ended",
            "providerID": "google",
            "modelID": "gemini-3-pro",
            "properties": {
                "timestamp": 1,
                "sessionID": "ses_test",
                "finish": "stop",
                "cost": 0.02,
                "tokens": {
                    "input": 11,
                    "output": 13,
                    "reasoning": 17,
                    "cache": { "read": 19, "write": 23 }
                }
            }
        });

        let usage = codex_token_usage_from_json_event(&event).expect("usage is extracted");
        assert_eq!(usage.backend, "opencode");
        assert_eq!(usage.source_event_type, "opencode.session.next.step.ended");
        assert_eq!(usage.provider.as_deref(), Some("google"));
        assert_eq!(usage.model.as_deref(), Some("gemini-3-pro"));
        assert_eq!(usage.total_tokens, Some(83));

        let runner_events = codex_json_to_runner_events(&event, "stdout");
        assert_eq!(runner_events.len(), 1);
        assert_eq!(
            runner_events[0].event_type,
            "opencode.session.next.step.ended"
        );
    }

    #[test]
    fn aggregates_multiple_opencode_step_usage_events() {
        let execution = execution_from_stdout(&[
            serde_json::json!({
                "provider": "openai",
                "model": "gpt-5.5",
                "part": {
                    "type": "step-finish",
                    "tokens": {
                        "input": 10,
                        "output": 20,
                        "reasoning": 30,
                        "cache": { "read": 40, "write": 50 }
                    }
                }
            }),
            serde_json::json!({
                "provider": "openai",
                "model": "gpt-5.5",
                "part": {
                    "type": "step-finish",
                    "tokens": {
                        "input": 1,
                        "output": 2,
                        "reasoning": 3,
                        "cache": { "read": 4, "write": 5 }
                    }
                }
            }),
        ]);

        let usage = observed_codex_token_usage(&execution).expect("usage is extracted");
        assert_eq!(usage.source_event_type, "opencode.usage.aggregate");
        assert_eq!(usage.input_tokens, Some(11));
        assert_eq!(usage.output_tokens, Some(22));
        assert_eq!(usage.reasoning_tokens, Some(33));
        assert_eq!(usage.cached_input_tokens, Some(44));
        assert_eq!(usage.cache_write_tokens, Some(55));
        assert_eq!(usage.total_tokens, Some(165));
    }

    #[test]
    fn bridges_codex_oauth_cache_to_opencode_auth_content() {
        let state_dir = test_temp_dir("opencode-auth-content");
        let codex_home = state_dir.join("codex-home");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("auth.json"),
            r#"{"tokens":{"access_token":"access_test","refresh_token":"refresh_test","expires_at":1780500000},"account":{"accountId":"acct_test"}}"#,
        )
        .unwrap();

        let auth_content =
            opencode_auth_content_from_codex_home(codex_home.to_str().unwrap()).unwrap();
        let parsed = serde_json::from_str::<serde_json::Value>(&auth_content).unwrap();

        assert_eq!(
            parsed
                .pointer("/openai/type")
                .and_then(serde_json::Value::as_str),
            Some("oauth")
        );
        assert_eq!(
            parsed
                .pointer("/openai/access")
                .and_then(serde_json::Value::as_str),
            Some("access_test")
        );
        assert_eq!(
            parsed
                .pointer("/openai/refresh")
                .and_then(serde_json::Value::as_str),
            Some("refresh_test")
        );
        assert_eq!(
            parsed
                .pointer("/openai/expires")
                .and_then(serde_json::Value::as_u64),
            Some(1_780_500_000_000_u64)
        );
        assert_eq!(
            parsed
                .pointer("/openai/accountId")
                .and_then(serde_json::Value::as_str),
            Some("acct_test")
        );
    }

    #[test]
    fn bridges_codex_oauth_cache_without_expiry_forces_opencode_refresh() {
        let state_dir = test_temp_dir("opencode-auth-content-no-expiry");
        let codex_home = state_dir.join("codex-home");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("auth.json"),
            r#"{"tokens":{"access_token":"access_test","refresh_token":"refresh_test","account_id":"acct_test"}}"#,
        )
        .unwrap();

        let auth_content =
            opencode_auth_content_from_codex_home(codex_home.to_str().unwrap()).unwrap();
        let parsed = serde_json::from_str::<serde_json::Value>(&auth_content).unwrap();

        assert_eq!(
            parsed
                .pointer("/openai/type")
                .and_then(serde_json::Value::as_str),
            Some("oauth")
        );
        assert_eq!(
            parsed
                .pointer("/openai/expires")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
        assert_eq!(
            parsed
                .pointer("/openai/accountId")
                .and_then(serde_json::Value::as_str),
            Some("acct_test")
        );
    }

    #[cfg(unix)]
    #[test]
    fn opencode_codex_runner_uses_opencode_binary_and_streams_events() {
        use std::os::unix::fs::PermissionsExt;

        let state_dir = test_temp_dir("opencode-runner");
        let workspace_dir = state_dir.join("workspace");
        let codex_home = state_dir.join("codex-home");
        fs::create_dir_all(&workspace_dir).unwrap();
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("auth.json"),
            r#"{"access_token":"access_test","refresh_token":"refresh_test","expires_at_ms":1780500000000,"accountId":"acct_test"}"#,
        )
        .unwrap();
        let fake_opencode = state_dir.join("fake-opencode.sh");
        fs::write(
            &fake_opencode,
            r#"#!/bin/sh
test "$1" = "run" || exit 2
test "$2" = "--format" || exit 3
test "$3" = "json" || exit 4
test "$4" = "--model" || exit 5
test "$5" = "openai/gpt-5.5" || exit 6
case "$OPENCODE_AUTH_CONTENT" in *'"openai"'*'"oauth"'*) ;; *) exit 7;; esac
echo '{"id":"evt_shell_started","type":"session.next.shell.started","properties":{"callID":"call_1","command":"pwd"}}'
echo '{"id":"evt_text","type":"text","part":{"id":"prt_text_1","type":"text","text":"I am Autopilot, the OpenAgents coding assistant."}}'
echo '{"id":"evt_step_ended","type":"session.next.step.ended","providerID":"openai","modelID":"gpt-5-codex","properties":{"tokens":{"input":3,"output":5,"reasoning":7,"cache":{"read":11,"write":13}}}}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_opencode).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_opencode, permissions).unwrap();
        let assignment = test_assignment("opencode_runtime");

        let execution = run_opencode_codex(
            &state_dir,
            &fake_opencode,
            &workspace_dir,
            codex_home.to_str().unwrap(),
            &assignment,
        )
        .unwrap();
        let usage = observed_codex_token_usage(&execution).unwrap();
        let runner_events = load_openagents_runner_events(&state_dir).unwrap();

        assert_eq!(execution.exit_code, Some(0));
        assert_eq!(usage.backend, "opencode");
        assert_eq!(usage.total_tokens, Some(39));
        assert!(runner_events
            .iter()
            .any(|event| event.event_type == "shell.command.started"));
        let text_event = runner_events
            .iter()
            .find(|event| event.event_type == "message.completed")
            .unwrap();
        assert_eq!(
            text_event.detail_excerpt.as_deref(),
            Some("I am Autopilot, the OpenAgents coding assistant.")
        );
        assert!(runner_events
            .iter()
            .any(|event| event.event_type == "opencode.session.next.step.ended"));
    }

    #[test]
    fn opencode_codex_model_defaults_to_openai_and_rejects_other_providers() {
        unsafe {
            env::remove_var("OA_OPENCODE_CODEX_MODEL");
        }
        assert_eq!(opencode_codex_model().unwrap(), "openai/gpt-5.5");

        unsafe {
            env::set_var("OA_OPENCODE_CODEX_MODEL", "openai/gpt-5.4");
        }
        assert_eq!(opencode_codex_model().unwrap(), "openai/gpt-5.4");

        unsafe {
            env::set_var("OA_OPENCODE_CODEX_MODEL", "anthropic/claude-sonnet-4-5");
        }
        assert_eq!(
            opencode_codex_model().unwrap_err(),
            "OpenCode Codex model must use the openai provider"
        );

        unsafe {
            env::remove_var("OA_OPENCODE_CODEX_MODEL");
        }
    }

    #[test]
    fn git_askpass_helper_reads_github_token_from_environment() {
        let _guard = ENV_LOCK.lock().unwrap();
        let state_dir = test_temp_dir("git-askpass");
        unsafe {
            env::set_var("GITHUB_TOKEN", "ghp_testtokenvalue");
            env::remove_var("GH_TOKEN");
        }

        let helper = prepare_github_askpass(&state_dir)
            .unwrap()
            .expect("askpass helper should be created");
        let contents = fs::read_to_string(&helper).unwrap();

        assert!(contents.contains("$GITHUB_TOKEN"));
        assert!(!contents.contains("ghp_testtokenvalue"));

        unsafe {
            env::remove_var("GITHUB_TOKEN");
        }
        fs::remove_dir_all(&state_dir).unwrap();
    }

    #[test]
    fn github_token_available_recognizes_oa_codex_fallback() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            env::remove_var("GITHUB_TOKEN");
            env::remove_var("GH_TOKEN");
            env::remove_var("OA_CODEX_GITHUB_TOKEN");
        }
        assert!(github_token_available().is_none());

        unsafe {
            env::set_var("OA_CODEX_GITHUB_TOKEN", "ghp_fallback");
        }
        assert_eq!(
            github_token_available().as_deref(),
            Some("OA_CODEX_GITHUB_TOKEN")
        );

        // GITHUB_TOKEN takes precedence over the static fallback.
        unsafe {
            env::set_var("GITHUB_TOKEN", "ghp_primary");
        }
        assert_eq!(github_token_available().as_deref(), Some("GITHUB_TOKEN"));

        unsafe {
            env::remove_var("GITHUB_TOKEN");
            env::remove_var("OA_CODEX_GITHUB_TOKEN");
        }
    }

    fn test_temp_dir(label: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "oa-workroomd-{label}-{}-{}",
            std::process::id(),
            now_epoch_ms().unwrap()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_assignment(assignment_id: &str) -> CodexWorkroomAssignment {
        CodexWorkroomAssignment {
            contract_version: "openagents.codex_workroom_assignment.v1".to_string(),
            assignment_id: assignment_id.to_string(),
            workroom_id: "workroom_test".to_string(),
            target_node_id: "oa-shc-test".to_string(),
            user_ref: "test-user".to_string(),
            organization_ref: None,
            project_ref: None,
            provider_account_ref: "provider-account_test".to_string(),
            auth_grant_ref: "codex-auth-grant_test".to_string(),
            repo_ref: "OpenAgentsInc/autopilot-omega@main".to_string(),
            prompt: "summarize environment".to_string(),
            required_artifacts: Vec::new(),
            sandbox: CodexSandboxMode::WorkspaceWrite,
            timeout_ms: Some(5_000),
            wallet_authority: false,
            created_at_ms: now_epoch_ms().unwrap(),
            audit_context: "test".to_string(),
        }
    }
}

fn cleanup_codex_workspace(workspace_dir: &Path) -> Result<String, String> {
    if workspace_dir.exists() {
        fs::remove_dir_all(workspace_dir)
            .map_err(|error| format!("failed to remove codex workspace: {error}"))?;
        Ok("codex workspace removed".to_string())
    } else {
        Ok("codex workspace already absent".to_string())
    }
}

fn redacted_error(prefix: &str, detail: &str) -> String {
    if contains_forbidden_metadata(detail) {
        format!("{prefix}: redacted")
    } else {
        format!("{prefix}: {}", truncate_for_event(detail))
    }
}

fn truncate_for_event(value: &str) -> String {
    value.chars().take(240).collect()
}

fn load_codex_auth_grant(path: &Path) -> Result<CodexAuthGrant, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read codex auth grant {}: {error}",
            path_label(path)
        )
    })?;
    serde_json::from_str::<CodexAuthGrant>(&raw).map_err(|error| {
        format!(
            "failed to parse codex auth grant {}: {error}",
            path_label(path)
        )
    })
}

fn scrub_codex_auth_state(
    state_dir: &Path,
) -> Result<(CodexAuthState, CodexAuthReceipt, &'static str), String> {
    let mut state = load_codex_auth_state(state_dir)?;
    let codex_home = PathBuf::from(state.codex_home.clone());
    let reason = if codex_home.exists() {
        fs::remove_dir_all(&codex_home)
            .map_err(|error| format!("failed to scrub session CODEX_HOME: {error}"))?;
        "session_codex_home_removed"
    } else {
        "session_codex_home_already_absent"
    };
    let now = now_epoch_ms()?;
    let receipt = build_codex_auth_receipt(
        &state.grant,
        CodexAuthReceiptKind::AuthMaterialScrubbed,
        CodexAuthDecision::Accepted,
        reason,
        codex_home.as_path(),
    )?;
    state.scrubbed_at_ms = Some(now);
    state.updated_at_ms = now;
    state.receipts.push(receipt.clone());
    save_codex_auth_state(state_dir, &state)?;
    append_codex_auth_receipt(state_dir, &receipt)?;
    Ok((state, receipt, reason))
}

fn run_codex_login_status(codex_bin: &Path, codex_home: &str) -> (CodexAuthDecision, String) {
    match std::process::Command::new(codex_bin)
        .args(["login", "status"])
        .env("CODEX_HOME", codex_home)
        .output()
    {
        Ok(output) if output.status.success() => (
            CodexAuthDecision::Accepted,
            "codex_login_status_ok".to_string(),
        ),
        Ok(output) => (
            CodexAuthDecision::Failed,
            format!(
                "codex_login_status_failed_exit_{}",
                output.status.code().unwrap_or(-1)
            ),
        ),
        Err(_) => (
            CodexAuthDecision::Failed,
            "codex_login_status_spawn_failed".to_string(),
        ),
    }
}

fn build_codex_auth_receipt(
    grant: &CodexAuthGrant,
    event_kind: CodexAuthReceiptKind,
    decision: CodexAuthDecision,
    reason: &str,
    codex_home: &Path,
) -> Result<CodexAuthReceipt, String> {
    if contains_forbidden_metadata(reason) {
        return Err("codex auth receipt reason contains forbidden marker".to_string());
    }
    let emitted_at_ms = now_epoch_ms()?;
    let codex_home_digest = sha256_prefixed(path_label(codex_home).as_bytes());
    let encoded = serde_json::to_vec(&(
        CODEX_AUTH_RECEIPT_VERSION,
        grant.workroom_id.as_str(),
        grant.grant_ref.as_str(),
        grant.provider_account_ref.as_str(),
        &event_kind,
        &decision,
        reason,
        codex_home_digest.as_str(),
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize codex auth receipt digest material: {error}"))?;
    let receipt = CodexAuthReceipt {
        schema_version: CODEX_AUTH_RECEIPT_VERSION.to_string(),
        receipt_id: format!(
            "codex.auth.{}.{}",
            safe_path_component(format!("{event_kind:?}").as_str()),
            emitted_at_ms
        ),
        workroom_id: grant.workroom_id.clone(),
        grant_ref: grant.grant_ref.clone(),
        provider_account_ref: grant.provider_account_ref.clone(),
        event_kind,
        decision,
        reason: reason.to_string(),
        codex_home_digest,
        receipt_digest: sha256_prefixed(&encoded),
        emitted_at_ms,
    };
    receipt.validate_contract()?;
    Ok(receipt)
}

fn load_codex_auth_state(state_dir: &Path) -> Result<CodexAuthState, String> {
    let path = codex_auth_state_path(state_dir);
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read codex auth state {}: {error}",
            path_label(&path)
        )
    })?;
    let state = serde_json::from_str::<CodexAuthState>(&raw).map_err(|error| {
        format!(
            "failed to parse codex auth state {}: {error}",
            path_label(&path)
        )
    })?;
    validate_codex_auth_state(&state)?;
    Ok(state)
}

fn save_codex_auth_state(state_dir: &Path, state: &CodexAuthState) -> Result<(), String> {
    validate_codex_auth_state(state)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize codex auth state: {error}"))?;
    fs::write(codex_auth_state_path(state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write codex auth state: {error}"))
}

fn append_codex_auth_receipt(state_dir: &Path, receipt: &CodexAuthReceipt) -> Result<(), String> {
    receipt.validate_contract()?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize codex auth receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(codex_auth_receipt_log_path(state_dir))
        .map_err(|error| format!("failed to open codex auth receipt log: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("failed to append codex auth receipt: {error}"))
}

fn validate_codex_auth_state(state: &CodexAuthState) -> Result<(), String> {
    if state.schema_version != CODEX_AUTH_STATE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported codex auth state schema '{}'",
            state.schema_version
        ));
    }
    if state.auth_json_digest.trim().is_empty() || !state.auth_json_digest.starts_with("sha256:") {
        return Err("codex auth state requires auth_json_digest".to_string());
    }
    if contains_forbidden_metadata(state.codex_home.as_str())
        || contains_forbidden_metadata(state.auth_json_path.as_str())
        || contains_forbidden_metadata(state.auth_json_digest.as_str())
    {
        return Err("codex auth state contains forbidden marker".to_string());
    }
    for receipt in &state.receipts {
        receipt.validate_contract()?;
    }
    Ok(())
}

fn set_owner_only_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to set owner-only permissions: {error}"))?;
    }
    Ok(())
}

fn set_owner_execute_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("failed to set owner-execute permissions: {error}"))?;
    }
    Ok(())
}

fn codex_auth_home(state_dir: &Path, grant_ref: &str) -> PathBuf {
    state_dir
        .join(CODEX_AUTH_HOME_ROOT)
        .join(safe_path_component(grant_ref))
}

fn safe_path_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(96)
        .collect::<String>()
}

fn default_state_dir() -> Result<PathBuf, String> {
    if let Ok(path) = env::var(ENV_OA_WORKROOM_HOME) {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    let home = env::var("HOME").map_err(|_| {
        format!("set {ENV_OA_WORKROOM_HOME} or HOME so oa-workroomd can resolve a state directory")
    })?;
    Ok(PathBuf::from(home).join(".openagents/cloud/oa-workroomd"))
}

fn metadata_path(state_dir: &Path) -> PathBuf {
    state_dir.join(METADATA_FILE)
}

fn metadata_access_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join(METADATA_ACCESS_LOG_FILE)
}

fn gateway_policy_path(state_dir: &Path) -> PathBuf {
    state_dir.join(GATEWAY_POLICY_FILE)
}

fn gateway_access_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join(GATEWAY_ACCESS_LOG_FILE)
}

fn ingress_state_path(state_dir: &Path) -> PathBuf {
    state_dir.join(INGRESS_STATE_FILE)
}

fn artifact_state_path(state_dir: &Path) -> PathBuf {
    state_dir.join(ARTIFACT_STATE_FILE)
}

fn artifact_receipt_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join(ARTIFACT_RECEIPT_LOG_FILE)
}

fn closeout_manifest_path(state_dir: &Path) -> PathBuf {
    state_dir.join(CLOSEOUT_MANIFEST_FILE)
}

fn lifecycle_state_path(state_dir: &Path) -> PathBuf {
    state_dir.join(LIFECYCLE_STATE_FILE)
}

fn lifecycle_receipt_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join(LIFECYCLE_RECEIPT_LOG_FILE)
}

fn codex_auth_state_path(state_dir: &Path) -> PathBuf {
    state_dir.join(CODEX_AUTH_STATE_FILE)
}

fn codex_auth_receipt_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join(CODEX_AUTH_RECEIPT_LOG_FILE)
}

fn codex_run_state_path(state_dir: &Path) -> PathBuf {
    state_dir.join(CODEX_RUN_STATE_FILE)
}

fn codex_run_event_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join(CODEX_RUN_EVENT_LOG_FILE)
}

fn openagents_runner_event_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join(OPENAGENTS_RUNNER_EVENT_LOG_FILE)
}

fn resource_usage_receipt_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join(RESOURCE_USAGE_RECEIPT_LOG_FILE)
}

fn codex_session_state_path(state_dir: &Path) -> PathBuf {
    state_dir.join(CODEX_SESSION_STATE_FILE)
}

fn codex_session_event_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join(CODEX_SESSION_EVENT_LOG_FILE)
}

fn codex_workspace_dir(state_dir: &Path, assignment_id: &str) -> PathBuf {
    state_dir
        .join(CODEX_WORKSPACE_ROOT)
        .join(safe_path_component(assignment_id))
}

fn now_epoch_ms() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| format!("system clock is before unix epoch: {error}"))
}

fn path_label(path: &Path) -> String {
    path.display().to_string()
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}
