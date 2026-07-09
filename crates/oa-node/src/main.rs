use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use openagents_cloud_contract::{
    CapabilityState, CloudNodeSnapshot, DesiredMode, ForgeAssignment, ForgeAssignmentDecision,
    ForgeAssignmentKind, ForgeAssignmentReceipt, HostFacts, ObservedStatus, ProbeCloseoutReceipt,
    ProbeCloseoutStatus, ProbeWorkerAttachment, ProductCapability, PsionicExecutionReceipt,
    PsionicExecutionStatus, PsionicWorkerAttachment, PsionicWorkerKind, SandboxProfileSummary,
    SettlementPolicy, FORGE_ASSIGNMENT_RECEIPT_VERSION, PROBE_CLOSEOUT_RECEIPT_VERSION,
    PSIONIC_EXECUTION_RECEIPT_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sysinfo::{Disks, System};

const NODE_STATE_SCHEMA_VERSION: &str = "openagents.oa_node.local_state.v1";
const ADMIN_STORE_SCHEMA_VERSION: &str = "openagents.oa_node.admin_store.v1";
const DEFAULT_SERVICE_NAME: &str = "openagents-oa-node";
const ENV_OA_NODE_HOME: &str = "OPENAGENTS_CLOUD_NODE_HOME";
const ADMIN_STORE_FILE: &str = "admin-store.json";
const HEALTH_EVENTS_FILE: &str = "health-events.jsonl";
const FORGE_ASSIGNMENT_RECEIPTS_FILE: &str = "forge-assignment-receipts.jsonl";
const SANDBOX_PROFILE_POLICIES_FILE: &str = "sandbox-profile-policies.json";
const PSIONIC_WORKERS_FILE: &str = "psionic-workers.json";
const PSIONIC_EXECUTION_RECEIPTS_FILE: &str = "psionic-execution-receipts.jsonl";
const PROBE_WORKER_FILE: &str = "probe-worker.json";
const PROBE_CLOSEOUT_RECEIPTS_FILE: &str = "probe-closeout-receipts.jsonl";
const SERVICE_EVENTS_FILE: &str = "service-events.jsonl";
const UPDATE_RECEIPTS_FILE: &str = "update-receipts.jsonl";
const QUARANTINE_RECEIPTS_FILE: &str = "quarantine-receipts.jsonl";
const SETTLEMENT_RECEIPTS_FILE: &str = "settlement-receipts.jsonl";
const BROKER_REDACTION_RECEIPTS_FILE: &str = "broker-redaction-receipts.jsonl";
const BROKER_REDACTED_ARTIFACT_DIR: &str = "broker-redacted-artifacts";
const CAPABILITY_DETECTION_SCHEMA_VERSION: &str = "openagents.oa_node.capability_detection.v1";
const NEXUS_REGISTRY_SCHEMA_VERSION: &str = "openagents.oa_node.nexus_registry.v1";
const SERVICE_MANAGER_SCHEMA_VERSION: &str = "openagents.oa_node.service_manager.v1";
const UPDATE_RECEIPT_SCHEMA_VERSION: &str = "openagents.oa_node.update_receipt.v1";
const UPDATE_POLICY_SCHEMA_VERSION: &str = "openagents.oa_node.update_policy.v1";
const QUARANTINE_RECEIPT_SCHEMA_VERSION: &str = "openagents.oa_node.quarantine_receipt.v1";
const QUARANTINE_STATUS_SCHEMA_VERSION: &str = "openagents.oa_node.quarantine_status.v1";
const SETTLEMENT_RECEIPT_SCHEMA_VERSION: &str = "openagents.oa_node.settlement_receipt.v1";
const SETTLEMENT_STATUS_SCHEMA_VERSION: &str = "openagents.oa_node.settlement_status.v1";
const BROKER_REDACTION_RECEIPT_SCHEMA_VERSION: &str =
    "openagents.oa_node.broker_redaction_receipt.v1";
const BROKER_REDACTION_OUTPUT_SCHEMA_VERSION: &str = "openagents.oa_node.broker_redaction.v1";
const SANDBOX_PROFILE_POLICY_SCHEMA_VERSION: &str = "openagents.oa_node.sandbox_profile_policy.v1";
const FAKE_SECRET_MARKER: &str = "OPENAGENTS_FAKE_SECRET_OK";
const ENV_PSIONIC_ENDPOINT: &str = "OPENAGENTS_PSIONIC_ENDPOINT";

const HELP: &str = "\
oa-node

Private managed OpenAgents Cloud node daemon scaffold.

Usage:
  oa-node --help
  oa-node --version
  oa-node init --org <id> [--node-id <id>] [--signing-key-ref <ref>] [--state-dir <path>] [--json]
  oa-node status [--state-dir <path>] [--json]
  oa-node doctor [--state-dir <path>] [--json]
  oa-node detect [--json]
  oa-node nexus register --base-url <url> [--state-dir <path>] [--json]
  oa-node nexus heartbeat --base-url <url> [--state-dir <path>] [--json]
  oa-node forge assignment receive --file <path> [--state-dir <path>] [--json]
  oa-node sandbox profile register --profile-id <id> --profile-digest <sha256:...> --execution-class <class> --network-policy <policy> --filesystem-policy <policy> --timeout-ms <ms> --max-artifact-bytes <bytes> --secret-policy <policy> [--not-ready] [--state-dir <path>] [--json]
  oa-node sandbox profile list [--state-dir <path>] [--json]
  oa-node psionic attach --file <path> [--state-dir <path>] [--json]
  oa-node psionic receipt append --product <id> --worker <id> --assignment <id> --evidence-digest <sha256:...> --status <succeeded|failed|refused> [--profile-digest <sha256:...>] [--state-dir <path>] [--json]
  oa-node probe attach --file <path> [--state-dir <path>] [--json]
  oa-node probe closeout append --workroom <id> --worker <id> --artifact <ref> --status <succeeded|failed|refused> [--state-dir <path>] [--json]
  oa-node service install --manager <launchd|systemd> [--service-name <name>] [--state-dir <path>] [--json]
  oa-node service <start|stop|restart|status|uninstall> [--state-dir <path>] [--json]
  oa-node update status [--state-dir <path>] [--json]
  oa-node update policy set --channel <name> [--pin <version>] [--defer] [--state-dir <path>] [--json]
  oa-node update apply --target-version <version> --signer <ref> --signature-digest <sha256:...> [--result <succeeded|failed>] [--state-dir <path>] [--json]
  oa-node update rollback --target-version <version> --signer <ref> --signature-digest <sha256:...> [--state-dir <path>] [--json]
  oa-node quarantine status [--state-dir <path>] [--json]
  oa-node quarantine enter --reason <reason> --workroom-policy <pause|migrate|close> [--workroom <id> ...] [--state-dir <path>] [--json]
  oa-node quarantine exit --reason <reason> [--state-dir <path>] [--json]
  oa-node settlement status [--state-dir <path>] [--json]
  oa-node settlement mode set <no-wallet|internal-accounting> [--treasury-ref <ref>] [--nexus-ref <ref>] [--state-dir <path>] [--json]
  oa-node settlement receipt append --amount-microusd <amount> --treasury-ref <ref> --nexus-ref <ref> [--state-dir <path>] [--json]
  oa-node broker redact --kind <headers|url|env|config|log|receipt> --input <path> [--state-dir <path>] [--json]
  oa-node admin desired-mode get [--state-dir <path>] [--json]
  oa-node admin desired-mode set <offline|online|paused|quarantined> [--state-dir <path>] [--json]
  oa-node admin health append --severity <level> --code <code> --detail <text> [--state-dir <path>] [--json]
  oa-node admin health list [--state-dir <path>] [--json]
";

#[derive(Clone, Debug, Eq, PartialEq)]
enum Command {
    Help,
    Version,
    Init(InitArgs),
    Status(StatusArgs),
    Doctor(StatusArgs),
    Detect(DetectArgs),
    Nexus(NexusCommand),
    Forge(ForgeCommand),
    Sandbox(SandboxCommand),
    Psionic(PsionicCommand),
    Probe(ProbeCommand),
    Service(ServiceCommand),
    Update(UpdateCommand),
    Quarantine(QuarantineCommand),
    Settlement(SettlementCommand),
    Broker(BrokerCommand),
    Admin(AdminCommand),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InitArgs {
    org_id: String,
    node_id: Option<String>,
    signing_key_ref: Option<String>,
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StatusArgs {
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DetectArgs {
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum NexusCommand {
    Register(NexusArgs),
    Heartbeat(NexusArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NexusArgs {
    base_url: String,
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ForgeCommand {
    AssignmentReceive(ForgeAssignmentReceiveArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SandboxCommand {
    ProfileRegister(SandboxProfileRegisterArgs),
    ProfileList(StatusArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ForgeAssignmentReceiveArgs {
    file: PathBuf,
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SandboxProfileRegisterArgs {
    state_dir: PathBuf,
    profile_id: String,
    profile_digest: String,
    execution_class: String,
    network_policy: String,
    filesystem_policy: String,
    timeout_limit_ms: u64,
    max_artifact_bytes: u64,
    secret_policy: String,
    ready: bool,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PsionicCommand {
    Attach(PsionicAttachArgs),
    ReceiptAppend(PsionicReceiptAppendArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PsionicAttachArgs {
    file: PathBuf,
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PsionicReceiptAppendArgs {
    state_dir: PathBuf,
    product_id: String,
    worker_id: String,
    assignment_id: String,
    evidence_digest: String,
    profile_digest: Option<String>,
    status: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProbeCommand {
    Attach(ProbeAttachArgs),
    CloseoutAppend(ProbeCloseoutAppendArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbeAttachArgs {
    file: PathBuf,
    state_dir: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbeCloseoutAppendArgs {
    state_dir: PathBuf,
    workroom_id: String,
    worker_id: String,
    artifact_refs: Vec<String>,
    status: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ServiceCommand {
    Install(ServiceInstallArgs),
    Action(ServiceActionArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ServiceInstallArgs {
    state_dir: PathBuf,
    service_manager: String,
    service_name: Option<String>,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ServiceActionArgs {
    state_dir: PathBuf,
    action: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum UpdateCommand {
    Status(StatusArgs),
    PolicySet(UpdatePolicySetArgs),
    Apply(UpdateApplyArgs),
    Rollback(UpdateRollbackArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UpdatePolicySetArgs {
    state_dir: PathBuf,
    channel: String,
    pinned_version: Option<String>,
    deferred: bool,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UpdateApplyArgs {
    state_dir: PathBuf,
    target_version: String,
    signer: String,
    signature_digest: String,
    result: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UpdateRollbackArgs {
    state_dir: PathBuf,
    target_version: String,
    signer: String,
    signature_digest: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum QuarantineCommand {
    Status(StatusArgs),
    Enter(QuarantineEnterArgs),
    Exit(QuarantineExitArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QuarantineEnterArgs {
    state_dir: PathBuf,
    reason: String,
    workroom_policy: String,
    workrooms: Vec<String>,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QuarantineExitArgs {
    state_dir: PathBuf,
    reason: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SettlementCommand {
    Status(StatusArgs),
    ModeSet(SettlementModeSetArgs),
    ReceiptAppend(SettlementReceiptAppendArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BrokerCommand {
    Redact(BrokerRedactArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BrokerRedactArgs {
    state_dir: PathBuf,
    kind: String,
    input: PathBuf,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SettlementModeSetArgs {
    state_dir: PathBuf,
    mode: String,
    treasury_ref: Option<String>,
    nexus_ref: Option<String>,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SettlementReceiptAppendArgs {
    state_dir: PathBuf,
    amount_microusd: u64,
    treasury_ref: String,
    nexus_ref: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AdminCommand {
    DesiredModeGet(StatusArgs),
    DesiredModeSet(DesiredModeSetArgs),
    HealthAppend(HealthAppendArgs),
    HealthList(StatusArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DesiredModeSetArgs {
    state_dir: PathBuf,
    desired_mode: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HealthAppendArgs {
    state_dir: PathBuf,
    severity: String,
    code: String,
    detail: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct LocalNodeState {
    schema_version: String,
    identity: LocalNodeIdentity,
    service: LocalNodeService,
    paths: LocalNodePaths,
    created_at_ms: u128,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct LocalNodeIdentity {
    node_id: String,
    org_id: String,
    operator_identity: String,
    account_or_org_binding: String,
    signing_key_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct LocalNodeService {
    service_name: String,
    desired_mode: String,
    observed_status: String,
    service_manager: String,
    update_channel: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct LocalNodePaths {
    state_dir: String,
    state_file: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct InitOutput {
    schema_version: String,
    initialized: bool,
    existing: bool,
    state_dir: String,
    state_file: String,
    identity: InitIdentityOutput,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct InitIdentityOutput {
    node_id: String,
    org_id: String,
    operator_identity: String,
    account_or_org_binding: String,
    signing_key_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AdminStore {
    schema_version: String,
    desired_mode: String,
    observed_status: String,
    inventory: AdminInventory,
    updates: AdminUpdates,
    quarantine: AdminQuarantine,
    #[serde(default = "default_admin_settlement")]
    settlement: AdminSettlement,
    receipt_cursors: AdminReceiptCursors,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_degradation_reason: Option<String>,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AdminInventory {
    last_detected_at_ms: Option<u128>,
    items: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AdminUpdates {
    channel: String,
    current_version: Option<String>,
    pending_update: Option<String>,
    last_checked_at_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pinned_version: Option<String>,
    #[serde(default)]
    deferred: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AdminQuarantine {
    quarantined: bool,
    reason: Option<String>,
    since_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workroom_policy: Option<String>,
    #[serde(default)]
    affected_workrooms: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AdminSettlement {
    mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    treasury_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nexus_ref: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AdminReceiptCursors {
    health_event_count: u64,
    job_receipt_cursor: Option<String>,
    artifact_receipt_cursor: Option<String>,
    accounting_receipt_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AdminHealthEvent {
    event_id: String,
    occurred_at_ms: u128,
    severity: String,
    code: String,
    detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ServiceEvent {
    schema_version: String,
    event_id: String,
    occurred_at_ms: u128,
    action: String,
    service_name: String,
    service_manager: String,
    service_status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ServiceOutput {
    schema_version: String,
    action: String,
    service: LocalNodeService,
    event_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct UpdateReceipt {
    schema_version: String,
    receipt_id: String,
    action: String,
    previous_version: Option<String>,
    target_version: String,
    signer: String,
    signature_digest: String,
    result: String,
    receipt_digest: String,
    emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct UpdateStatusOutput {
    schema_version: String,
    updates: AdminUpdates,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct UpdatePolicyOutput {
    schema_version: String,
    updates: AdminUpdates,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct QuarantineReceipt {
    schema_version: String,
    receipt_id: String,
    action: String,
    reason: String,
    workroom_policy: String,
    affected_workrooms: Vec<String>,
    result: String,
    receipt_digest: String,
    emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct QuarantineStatusOutput {
    schema_version: String,
    quarantine: AdminQuarantine,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct QuarantineActionOutput {
    schema_version: String,
    quarantine: AdminQuarantine,
    receipt: QuarantineReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SettlementReceipt {
    schema_version: String,
    receipt_id: String,
    mode: String,
    amount_microusd: u64,
    treasury_ref: String,
    nexus_ref: String,
    result: String,
    receipt_digest: String,
    emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SettlementStatusOutput {
    schema_version: String,
    settlement: AdminSettlement,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SettlementReceiptOutput {
    schema_version: String,
    settlement: AdminSettlement,
    receipt: SettlementReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct BrokerRedactionReceipt {
    schema_version: String,
    receipt_id: String,
    kind: String,
    input_digest: String,
    redacted_artifact_path: String,
    redacted_digest: String,
    receipt_digest: String,
    emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct BrokerRedactionOutput {
    schema_version: String,
    receipt: BrokerRedactionReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SandboxProfilePolicy {
    schema_version: String,
    profile_id: String,
    profile_digest: String,
    execution_class: String,
    network_policy: String,
    filesystem_policy: String,
    timeout_limit_ms: u64,
    max_artifact_bytes: u64,
    secret_policy: String,
    ready: bool,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SandboxProfileRegisterOutput {
    schema_version: String,
    profile: SandboxProfilePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SandboxProfileListOutput {
    schema_version: String,
    profiles: Vec<SandboxProfilePolicy>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AdminStoreLoad {
    store: AdminStore,
    health_events: Vec<AdminHealthEvent>,
    forge_assignment_receipts: Vec<ForgeAssignmentReceipt>,
    sandbox_profiles: Vec<SandboxProfilePolicy>,
    psionic_attachment: Option<PsionicWorkerAttachment>,
    psionic_execution_receipts: Vec<PsionicExecutionReceipt>,
    probe_attachment: Option<ProbeWorkerAttachment>,
    probe_closeout_receipts: Vec<ProbeCloseoutReceipt>,
    degraded_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DesiredModeOutput {
    schema_version: String,
    desired_mode: String,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct HealthEventsOutput {
    schema_version: String,
    events: Vec<AdminHealthEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct CapabilityDetectionReport {
    schema_version: String,
    host: HostFacts,
    present_hardware: PresentHardwareReport,
    sellable_capabilities: Vec<SellableCapabilityReport>,
    degraded_backends: Vec<DetectedBackendFailure>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PresentHardwareReport {
    os: String,
    arch: String,
    cpu_brand: String,
    logical_cpu_count: u64,
    physical_cpu_count: Option<u64>,
    memory_total_bytes: u64,
    disk_total_bytes: u64,
    disk_available_bytes: u64,
    accelerators: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SellableCapabilityReport {
    capability_id: String,
    present_hardware: bool,
    backend_ready: bool,
    eligible: bool,
    reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DetectedBackendFailure {
    backend: String,
    reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct NexusRegistryEnvelope {
    schema_version: String,
    action: String,
    node_id: String,
    org_id: String,
    snapshot_digest: String,
    observed_status: String,
    desired_mode: String,
    signature: NexusSignature,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct NexusSignature {
    algorithm: String,
    signing_key_ref: String,
    digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct NexusRegistryResponse {
    status: String,
    desired_mode: Option<String>,
    registration_expires_at_ms: Option<u128>,
    detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct NexusRegistryOutput {
    schema_version: String,
    action: String,
    status: String,
    degraded: bool,
    snapshot_digest: String,
    desired_mode: Option<String>,
    registration_expires_at_ms: Option<u128>,
    detail: Option<String>,
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
        Command::Version => Ok(Some(format!("oa-node {}", env!("CARGO_PKG_VERSION")))),
        Command::Init(args) => init_node(args).map(Some),
        Command::Status(args) => status_node(args).map(Some),
        Command::Doctor(args) => doctor_node(args).map(Some),
        Command::Detect(args) => detect_node(args).map(Some),
        Command::Nexus(command) => run_nexus_command(command).map(Some),
        Command::Forge(command) => run_forge_command(command).map(Some),
        Command::Sandbox(command) => run_sandbox_command(command).map(Some),
        Command::Psionic(command) => run_psionic_command(command).map(Some),
        Command::Probe(command) => run_probe_command(command).map(Some),
        Command::Service(command) => run_service_command(command).map(Some),
        Command::Update(command) => run_update_command(command).map(Some),
        Command::Quarantine(command) => run_quarantine_command(command).map(Some),
        Command::Settlement(command) => run_settlement_command(command).map(Some),
        Command::Broker(command) => run_broker_command(command).map(Some),
        Command::Admin(command) => run_admin_command(command).map(Some),
    }
}

fn parse_command(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        None => Ok(Command::Help),
        Some("--help" | "-h") => Ok(Command::Help),
        Some("--version" | "-V") => Ok(Command::Version),
        Some("init") => parse_init_args(&args[1..]).map(Command::Init),
        Some("status") => parse_status_args(&args[1..]).map(Command::Status),
        Some("doctor") => parse_status_args(&args[1..]).map(Command::Doctor),
        Some("detect") => parse_detect_args(&args[1..]).map(Command::Detect),
        Some("nexus") => parse_nexus_command(&args[1..]).map(Command::Nexus),
        Some("forge") => parse_forge_command(&args[1..]).map(Command::Forge),
        Some("sandbox") => parse_sandbox_command(&args[1..]).map(Command::Sandbox),
        Some("psionic") => parse_psionic_command(&args[1..]).map(Command::Psionic),
        Some("probe") => parse_probe_command(&args[1..]).map(Command::Probe),
        Some("service") => parse_service_command(&args[1..]).map(Command::Service),
        Some("update") => parse_update_command(&args[1..]).map(Command::Update),
        Some("quarantine") => parse_quarantine_command(&args[1..]).map(Command::Quarantine),
        Some("settlement") => parse_settlement_command(&args[1..]).map(Command::Settlement),
        Some("broker") => parse_broker_command(&args[1..]).map(Command::Broker),
        Some("admin") => parse_admin_command(&args[1..]).map(Command::Admin),
        Some(other) => Err(format!("unknown command or flag: {other}")),
    }
}

fn parse_init_args(args: &[String]) -> Result<InitArgs, String> {
    let mut org_id = None;
    let mut node_id = None;
    let mut signing_key_ref = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--org" => {
                org_id = Some(required_value(args, index, "--org")?);
                index += 2;
            }
            "--node-id" => {
                node_id = Some(required_value(args, index, "--node-id")?);
                index += 2;
            }
            "--signing-key-ref" => {
                signing_key_ref = Some(required_value(args, index, "--signing-key-ref")?);
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
            other => return Err(format!("unexpected init argument: {other}")),
        }
    }

    let org_id = org_id.ok_or_else(|| "init requires --org <id>".to_string())?;
    if org_id.trim().is_empty() {
        return Err("init --org must not be empty".to_string());
    }

    Ok(InitArgs {
        org_id,
        node_id,
        signing_key_ref,
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        json,
    })
}

fn parse_status_args(args: &[String]) -> Result<StatusArgs, String> {
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
            other => return Err(format!("unexpected status argument: {other}")),
        }
    }

    Ok(StatusArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        json,
    })
}

fn parse_admin_command(args: &[String]) -> Result<AdminCommand, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("desired-mode"), Some("get")) => {
            parse_status_args(&args[2..]).map(AdminCommand::DesiredModeGet)
        }
        (Some("desired-mode"), Some("set")) => {
            parse_desired_mode_set_args(&args[2..]).map(AdminCommand::DesiredModeSet)
        }
        (Some("health"), Some("append")) => {
            parse_health_append_args(&args[2..]).map(AdminCommand::HealthAppend)
        }
        (Some("health"), Some("list")) => {
            parse_status_args(&args[2..]).map(AdminCommand::HealthList)
        }
        _ => Err("expected admin desired-mode|get|set or admin health|append|list".to_string()),
    }
}

fn parse_detect_args(args: &[String]) -> Result<DetectArgs, String> {
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            other => return Err(format!("unexpected detect argument: {other}")),
        }
    }
    Ok(DetectArgs { json })
}

fn parse_nexus_command(args: &[String]) -> Result<NexusCommand, String> {
    match args.first().map(String::as_str) {
        Some("register") => parse_nexus_args(&args[1..]).map(NexusCommand::Register),
        Some("heartbeat") => parse_nexus_args(&args[1..]).map(NexusCommand::Heartbeat),
        _ => Err("expected nexus register or nexus heartbeat".to_string()),
    }
}

fn parse_nexus_args(args: &[String]) -> Result<NexusArgs, String> {
    let mut base_url = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--base-url" => {
                base_url = Some(required_value(args, index, "--base-url")?);
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
            other => return Err(format!("unexpected nexus argument: {other}")),
        }
    }
    let base_url = base_url.ok_or_else(|| "nexus command requires --base-url".to_string())?;
    if base_url.trim().is_empty() {
        return Err("nexus --base-url must not be empty".to_string());
    }
    Ok(NexusArgs {
        base_url,
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        json,
    })
}

fn parse_forge_command(args: &[String]) -> Result<ForgeCommand, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("assignment"), Some("receive")) => {
            parse_forge_assignment_receive_args(&args[2..]).map(ForgeCommand::AssignmentReceive)
        }
        _ => Err("expected forge assignment receive".to_string()),
    }
}

fn parse_forge_assignment_receive_args(
    args: &[String],
) -> Result<ForgeAssignmentReceiveArgs, String> {
    let mut file = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
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
            other => return Err(format!("unexpected forge assignment argument: {other}")),
        }
    }
    let file = file.ok_or_else(|| "forge assignment receive requires --file".to_string())?;
    Ok(ForgeAssignmentReceiveArgs {
        file,
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        json,
    })
}

fn parse_sandbox_command(args: &[String]) -> Result<SandboxCommand, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("profile"), Some("register")) => {
            parse_sandbox_profile_register_args(&args[2..]).map(SandboxCommand::ProfileRegister)
        }
        (Some("profile"), Some("list")) => {
            parse_status_args(&args[2..]).map(SandboxCommand::ProfileList)
        }
        _ => Err("expected sandbox profile register or sandbox profile list".to_string()),
    }
}

fn parse_sandbox_profile_register_args(
    args: &[String],
) -> Result<SandboxProfileRegisterArgs, String> {
    let mut profile_id = None;
    let mut profile_digest = None;
    let mut execution_class = None;
    let mut network_policy = None;
    let mut filesystem_policy = None;
    let mut timeout_limit_ms = None;
    let mut max_artifact_bytes = None;
    let mut secret_policy = None;
    let mut state_dir = None;
    let mut ready = true;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--profile-id" => {
                profile_id = Some(required_value(args, index, "--profile-id")?);
                index += 2;
            }
            "--profile-digest" => {
                profile_digest = Some(required_value(args, index, "--profile-digest")?);
                index += 2;
            }
            "--execution-class" => {
                execution_class = Some(required_value(args, index, "--execution-class")?);
                index += 2;
            }
            "--network-policy" => {
                network_policy = Some(required_value(args, index, "--network-policy")?);
                index += 2;
            }
            "--filesystem-policy" => {
                filesystem_policy = Some(required_value(args, index, "--filesystem-policy")?);
                index += 2;
            }
            "--timeout-ms" => {
                timeout_limit_ms = Some(parse_u64(
                    &required_value(args, index, "--timeout-ms")?,
                    "--timeout-ms",
                )?);
                index += 2;
            }
            "--max-artifact-bytes" => {
                max_artifact_bytes = Some(parse_u64(
                    &required_value(args, index, "--max-artifact-bytes")?,
                    "--max-artifact-bytes",
                )?);
                index += 2;
            }
            "--secret-policy" => {
                secret_policy = Some(required_value(args, index, "--secret-policy")?);
                index += 2;
            }
            "--not-ready" => {
                ready = false;
                index += 1;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected sandbox profile argument: {other}")),
        }
    }
    Ok(SandboxProfileRegisterArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        profile_id: profile_id
            .ok_or_else(|| "sandbox profile register requires --profile-id".to_string())?,
        profile_digest: profile_digest
            .ok_or_else(|| "sandbox profile register requires --profile-digest".to_string())?,
        execution_class: execution_class
            .ok_or_else(|| "sandbox profile register requires --execution-class".to_string())?,
        network_policy: network_policy
            .ok_or_else(|| "sandbox profile register requires --network-policy".to_string())?,
        filesystem_policy: filesystem_policy
            .ok_or_else(|| "sandbox profile register requires --filesystem-policy".to_string())?,
        timeout_limit_ms: timeout_limit_ms
            .ok_or_else(|| "sandbox profile register requires --timeout-ms".to_string())?,
        max_artifact_bytes: max_artifact_bytes
            .ok_or_else(|| "sandbox profile register requires --max-artifact-bytes".to_string())?,
        secret_policy: secret_policy
            .ok_or_else(|| "sandbox profile register requires --secret-policy".to_string())?,
        ready,
        json,
    })
}

fn parse_psionic_command(args: &[String]) -> Result<PsionicCommand, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("attach"), _) => parse_psionic_attach_args(&args[1..]).map(PsionicCommand::Attach),
        (Some("receipt"), Some("append")) => {
            parse_psionic_receipt_append_args(&args[2..]).map(PsionicCommand::ReceiptAppend)
        }
        _ => Err("expected psionic attach or psionic receipt append".to_string()),
    }
}

fn parse_psionic_attach_args(args: &[String]) -> Result<PsionicAttachArgs, String> {
    let mut file = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
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
            other => return Err(format!("unexpected psionic attach argument: {other}")),
        }
    }
    let file = file.ok_or_else(|| "psionic attach requires --file".to_string())?;
    Ok(PsionicAttachArgs {
        file,
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        json,
    })
}

fn parse_psionic_receipt_append_args(args: &[String]) -> Result<PsionicReceiptAppendArgs, String> {
    let mut product_id = None;
    let mut worker_id = None;
    let mut assignment_id = None;
    let mut evidence_digest = None;
    let mut profile_digest = None;
    let mut status = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--product" => {
                product_id = Some(required_value(args, index, "--product")?);
                index += 2;
            }
            "--worker" => {
                worker_id = Some(required_value(args, index, "--worker")?);
                index += 2;
            }
            "--assignment" => {
                assignment_id = Some(required_value(args, index, "--assignment")?);
                index += 2;
            }
            "--evidence-digest" => {
                evidence_digest = Some(required_value(args, index, "--evidence-digest")?);
                index += 2;
            }
            "--profile-digest" => {
                profile_digest = Some(required_value(args, index, "--profile-digest")?);
                index += 2;
            }
            "--status" => {
                status = Some(required_value(args, index, "--status")?);
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
            other => return Err(format!("unexpected psionic receipt argument: {other}")),
        }
    }
    Ok(PsionicReceiptAppendArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        product_id: product_id.ok_or_else(|| "psionic receipt requires --product".to_string())?,
        worker_id: worker_id.ok_or_else(|| "psionic receipt requires --worker".to_string())?,
        assignment_id: assignment_id
            .ok_or_else(|| "psionic receipt requires --assignment".to_string())?,
        evidence_digest: evidence_digest
            .ok_or_else(|| "psionic receipt requires --evidence-digest".to_string())?,
        profile_digest,
        status: status.ok_or_else(|| "psionic receipt requires --status".to_string())?,
        json,
    })
}

fn parse_probe_command(args: &[String]) -> Result<ProbeCommand, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("attach"), _) => parse_probe_attach_args(&args[1..]).map(ProbeCommand::Attach),
        (Some("closeout"), Some("append")) => {
            parse_probe_closeout_append_args(&args[2..]).map(ProbeCommand::CloseoutAppend)
        }
        _ => Err("expected probe attach or probe closeout append".to_string()),
    }
}

fn parse_probe_attach_args(args: &[String]) -> Result<ProbeAttachArgs, String> {
    let mut file = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
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
            other => return Err(format!("unexpected probe attach argument: {other}")),
        }
    }
    let file = file.ok_or_else(|| "probe attach requires --file".to_string())?;
    Ok(ProbeAttachArgs {
        file,
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        json,
    })
}

fn parse_probe_closeout_append_args(args: &[String]) -> Result<ProbeCloseoutAppendArgs, String> {
    let mut workroom_id = None;
    let mut worker_id = None;
    let mut artifact_refs = Vec::new();
    let mut status = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--workroom" => {
                workroom_id = Some(required_value(args, index, "--workroom")?);
                index += 2;
            }
            "--worker" => {
                worker_id = Some(required_value(args, index, "--worker")?);
                index += 2;
            }
            "--artifact" => {
                artifact_refs.push(required_value(args, index, "--artifact")?);
                index += 2;
            }
            "--status" => {
                status = Some(required_value(args, index, "--status")?);
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
            other => return Err(format!("unexpected probe closeout argument: {other}")),
        }
    }
    if artifact_refs.is_empty() {
        return Err("probe closeout append requires at least one --artifact".to_string());
    }
    Ok(ProbeCloseoutAppendArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        workroom_id: workroom_id.ok_or_else(|| "probe closeout requires --workroom".to_string())?,
        worker_id: worker_id.ok_or_else(|| "probe closeout requires --worker".to_string())?,
        artifact_refs,
        status: status.ok_or_else(|| "probe closeout requires --status".to_string())?,
        json,
    })
}

fn parse_service_command(args: &[String]) -> Result<ServiceCommand, String> {
    match args.first().map(String::as_str) {
        Some("install") => parse_service_install_args(&args[1..]).map(ServiceCommand::Install),
        Some(action @ ("start" | "stop" | "restart" | "status" | "uninstall")) => {
            parse_service_action_args(action, &args[1..]).map(ServiceCommand::Action)
        }
        _ => Err("expected service install/start/stop/restart/status/uninstall".to_string()),
    }
}

fn parse_service_install_args(args: &[String]) -> Result<ServiceInstallArgs, String> {
    let mut service_manager = None;
    let mut service_name = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--manager" => {
                service_manager = Some(required_value(args, index, "--manager")?);
                index += 2;
            }
            "--service-name" => {
                service_name = Some(required_value(args, index, "--service-name")?);
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
            other => return Err(format!("unexpected service install argument: {other}")),
        }
    }
    let service_manager =
        service_manager.ok_or_else(|| "service install requires --manager".to_string())?;
    validate_service_manager(service_manager.as_str())?;
    if let Some(name) = &service_name {
        validate_service_name(name)?;
    }
    Ok(ServiceInstallArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        service_manager,
        service_name,
        json,
    })
}

fn parse_service_action_args(action: &str, args: &[String]) -> Result<ServiceActionArgs, String> {
    let status_args = parse_status_args(args)?;
    Ok(ServiceActionArgs {
        state_dir: status_args.state_dir,
        action: action.to_string(),
        json: status_args.json,
    })
}

fn parse_update_command(args: &[String]) -> Result<UpdateCommand, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("status"), _) => parse_status_args(&args[1..]).map(UpdateCommand::Status),
        (Some("policy"), Some("set")) => {
            parse_update_policy_set_args(&args[2..]).map(UpdateCommand::PolicySet)
        }
        (Some("apply"), _) => parse_update_apply_args(&args[1..]).map(UpdateCommand::Apply),
        (Some("rollback"), _) => {
            parse_update_rollback_args(&args[1..]).map(UpdateCommand::Rollback)
        }
        _ => Err(
            "expected update status, update policy set, update apply, or update rollback"
                .to_string(),
        ),
    }
}

fn parse_update_policy_set_args(args: &[String]) -> Result<UpdatePolicySetArgs, String> {
    let mut channel = None;
    let mut pinned_version = None;
    let mut deferred = false;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--channel" => {
                channel = Some(required_value(args, index, "--channel")?);
                index += 2;
            }
            "--pin" => {
                pinned_version = Some(required_value(args, index, "--pin")?);
                index += 2;
            }
            "--defer" => {
                deferred = true;
                index += 1;
            }
            "--state-dir" => {
                state_dir = Some(PathBuf::from(required_value(args, index, "--state-dir")?));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => return Err(format!("unexpected update policy argument: {other}")),
        }
    }
    let channel = channel.ok_or_else(|| "update policy set requires --channel".to_string())?;
    validate_update_label(channel.as_str(), "channel")?;
    if let Some(pin) = &pinned_version {
        validate_update_label(pin, "pin")?;
    }
    Ok(UpdatePolicySetArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        channel,
        pinned_version,
        deferred,
        json,
    })
}

fn parse_update_apply_args(args: &[String]) -> Result<UpdateApplyArgs, String> {
    let mut target_version = None;
    let mut signer = None;
    let mut signature_digest = None;
    let mut result = "succeeded".to_string();
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target-version" => {
                target_version = Some(required_value(args, index, "--target-version")?);
                index += 2;
            }
            "--signer" => {
                signer = Some(required_value(args, index, "--signer")?);
                index += 2;
            }
            "--signature-digest" => {
                signature_digest = Some(required_value(args, index, "--signature-digest")?);
                index += 2;
            }
            "--result" => {
                result = required_value(args, index, "--result")?;
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
            other => return Err(format!("unexpected update apply argument: {other}")),
        }
    }
    validate_update_result(result.as_str())?;
    Ok(UpdateApplyArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        target_version: target_version
            .ok_or_else(|| "update apply requires --target-version".to_string())?,
        signer: signer.ok_or_else(|| "update apply requires --signer".to_string())?,
        signature_digest: signature_digest
            .ok_or_else(|| "update apply requires --signature-digest".to_string())?,
        result,
        json,
    })
}

fn parse_update_rollback_args(args: &[String]) -> Result<UpdateRollbackArgs, String> {
    let apply = parse_update_apply_args(args)?;
    Ok(UpdateRollbackArgs {
        state_dir: apply.state_dir,
        target_version: apply.target_version,
        signer: apply.signer,
        signature_digest: apply.signature_digest,
        json: apply.json,
    })
}

fn parse_quarantine_command(args: &[String]) -> Result<QuarantineCommand, String> {
    match args.first().map(String::as_str) {
        Some("status") => parse_status_args(&args[1..]).map(QuarantineCommand::Status),
        Some("enter") => parse_quarantine_enter_args(&args[1..]).map(QuarantineCommand::Enter),
        Some("exit") => parse_quarantine_exit_args(&args[1..]).map(QuarantineCommand::Exit),
        _ => Err("expected quarantine status, quarantine enter, or quarantine exit".to_string()),
    }
}

fn parse_quarantine_enter_args(args: &[String]) -> Result<QuarantineEnterArgs, String> {
    let mut reason = None;
    let mut workroom_policy = None;
    let mut workrooms = Vec::new();
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--reason" => {
                reason = Some(required_value(args, index, "--reason")?);
                index += 2;
            }
            "--workroom-policy" => {
                workroom_policy = Some(required_value(args, index, "--workroom-policy")?);
                index += 2;
            }
            "--workroom" => {
                workrooms.push(required_value(args, index, "--workroom")?);
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
            other => return Err(format!("unexpected quarantine enter argument: {other}")),
        }
    }
    let reason = reason.ok_or_else(|| "quarantine enter requires --reason".to_string())?;
    let workroom_policy =
        workroom_policy.ok_or_else(|| "quarantine enter requires --workroom-policy".to_string())?;
    validate_quarantine_reason(reason.as_str())?;
    validate_workroom_policy(workroom_policy.as_str())?;
    for workroom in &workrooms {
        validate_quarantine_reason(workroom)?;
    }
    Ok(QuarantineEnterArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        reason,
        workroom_policy,
        workrooms,
        json,
    })
}

fn parse_quarantine_exit_args(args: &[String]) -> Result<QuarantineExitArgs, String> {
    let mut reason = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--reason" => {
                reason = Some(required_value(args, index, "--reason")?);
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
            other => return Err(format!("unexpected quarantine exit argument: {other}")),
        }
    }
    let reason = reason.ok_or_else(|| "quarantine exit requires --reason".to_string())?;
    validate_quarantine_reason(reason.as_str())?;
    Ok(QuarantineExitArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        reason,
        json,
    })
}

fn parse_settlement_command(args: &[String]) -> Result<SettlementCommand, String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
        args.get(2).map(String::as_str),
    ) {
        (Some("status"), _, _) => parse_status_args(&args[1..]).map(SettlementCommand::Status),
        (Some("mode"), Some("set"), _) => {
            parse_settlement_mode_set_args(&args[2..]).map(SettlementCommand::ModeSet)
        }
        (Some("receipt"), Some("append"), _) => {
            parse_settlement_receipt_append_args(&args[2..]).map(SettlementCommand::ReceiptAppend)
        }
        _ => Err(
            "expected settlement status, settlement mode set, or settlement receipt append"
                .to_string(),
        ),
    }
}

fn parse_settlement_mode_set_args(args: &[String]) -> Result<SettlementModeSetArgs, String> {
    let mode = args
        .first()
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .ok_or_else(|| "settlement mode set requires a mode".to_string())?;
    validate_managed_settlement_mode(mode.as_str())?;
    let mut treasury_ref = None;
    let mut nexus_ref = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--treasury-ref" => {
                treasury_ref = Some(required_value(args, index, "--treasury-ref")?);
                index += 2;
            }
            "--nexus-ref" => {
                nexus_ref = Some(required_value(args, index, "--nexus-ref")?);
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
            other => return Err(format!("unexpected settlement mode argument: {other}")),
        }
    }
    if let Some(ref_value) = &treasury_ref {
        validate_settlement_ref(ref_value, "treasury_ref")?;
    }
    if let Some(ref_value) = &nexus_ref {
        validate_settlement_ref(ref_value, "nexus_ref")?;
    }
    Ok(SettlementModeSetArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        mode,
        treasury_ref,
        nexus_ref,
        json,
    })
}

fn parse_settlement_receipt_append_args(
    args: &[String],
) -> Result<SettlementReceiptAppendArgs, String> {
    let mut amount_microusd = None;
    let mut treasury_ref = None;
    let mut nexus_ref = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--amount-microusd" => {
                let raw = required_value(args, index, "--amount-microusd")?;
                amount_microusd = Some(raw.parse::<u64>().map_err(|error| {
                    format!("settlement --amount-microusd must be an integer: {error}")
                })?);
                index += 2;
            }
            "--treasury-ref" => {
                treasury_ref = Some(required_value(args, index, "--treasury-ref")?);
                index += 2;
            }
            "--nexus-ref" => {
                nexus_ref = Some(required_value(args, index, "--nexus-ref")?);
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
            other => return Err(format!("unexpected settlement receipt argument: {other}")),
        }
    }
    let treasury_ref = treasury_ref
        .ok_or_else(|| "settlement receipt append requires --treasury-ref".to_string())?;
    let nexus_ref =
        nexus_ref.ok_or_else(|| "settlement receipt append requires --nexus-ref".to_string())?;
    validate_settlement_ref(treasury_ref.as_str(), "treasury_ref")?;
    validate_settlement_ref(nexus_ref.as_str(), "nexus_ref")?;
    Ok(SettlementReceiptAppendArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        amount_microusd: amount_microusd
            .ok_or_else(|| "settlement receipt append requires --amount-microusd".to_string())?,
        treasury_ref,
        nexus_ref,
        json,
    })
}

fn parse_broker_command(args: &[String]) -> Result<BrokerCommand, String> {
    match args.first().map(String::as_str) {
        Some("redact") => parse_broker_redact_args(&args[1..]).map(BrokerCommand::Redact),
        _ => Err("expected broker redact".to_string()),
    }
}

fn parse_broker_redact_args(args: &[String]) -> Result<BrokerRedactArgs, String> {
    let mut kind = None;
    let mut input = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--kind" => {
                kind = Some(required_value(args, index, "--kind")?);
                index += 2;
            }
            "--input" => {
                input = Some(PathBuf::from(required_value(args, index, "--input")?));
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
            other => return Err(format!("unexpected broker redact argument: {other}")),
        }
    }
    let kind = kind.ok_or_else(|| "broker redact requires --kind".to_string())?;
    validate_broker_kind(kind.as_str())?;
    Ok(BrokerRedactArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        kind,
        input: input.ok_or_else(|| "broker redact requires --input".to_string())?,
        json,
    })
}

fn parse_desired_mode_set_args(args: &[String]) -> Result<DesiredModeSetArgs, String> {
    let desired_mode = args
        .first()
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .ok_or_else(|| "desired-mode set requires a mode".to_string())?;
    parse_desired_mode_label(desired_mode.as_str())?;
    let status_args = parse_status_args(&args[1..])?;
    Ok(DesiredModeSetArgs {
        state_dir: status_args.state_dir,
        desired_mode,
        json: status_args.json,
    })
}

fn parse_health_append_args(args: &[String]) -> Result<HealthAppendArgs, String> {
    let mut severity = None;
    let mut code = None;
    let mut detail = None;
    let mut state_dir = None;
    let mut json = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--severity" => {
                severity = Some(required_value(args, index, "--severity")?);
                index += 2;
            }
            "--code" => {
                code = Some(required_value(args, index, "--code")?);
                index += 2;
            }
            "--detail" => {
                detail = Some(required_value(args, index, "--detail")?);
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
            other => return Err(format!("unexpected health append argument: {other}")),
        }
    }

    let severity = severity.ok_or_else(|| "health append requires --severity".to_string())?;
    let code = code.ok_or_else(|| "health append requires --code".to_string())?;
    let detail = detail.ok_or_else(|| "health append requires --detail".to_string())?;
    if severity.trim().is_empty() || code.trim().is_empty() || detail.trim().is_empty() {
        return Err("health append severity, code, and detail must not be empty".to_string());
    }

    Ok(HealthAppendArgs {
        state_dir: state_dir.unwrap_or(default_state_dir()?),
        severity,
        code,
        detail,
        json,
    })
}

fn required_value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index + 1)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_u64(value: &str, flag: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid value for {flag}: {error}"))
}

fn default_state_dir() -> Result<PathBuf, String> {
    if let Ok(path) = env::var(ENV_OA_NODE_HOME) {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = env::var("HOME").map_err(|_| {
        format!("set {ENV_OA_NODE_HOME} or HOME so oa-node can resolve a state directory")
    })?;
    Ok(PathBuf::from(home).join(".openagents/cloud/oa-node"))
}

fn init_node(args: InitArgs) -> Result<String, String> {
    fs::create_dir_all(&args.state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(&args.state_dir)
        )
    })?;

    let state_file = state_file(&args.state_dir);
    let (state, existing) = if state_file.exists() {
        let existing = load_state(&state_file)?;
        validate_existing_state(&existing, &args)?;
        (existing, true)
    } else {
        let now_ms = now_epoch_ms()?;
        let node_id = args
            .node_id
            .clone()
            .unwrap_or_else(|| generated_node_id(args.org_id.as_str(), now_ms));
        let signing_key_ref = args
            .signing_key_ref
            .clone()
            .unwrap_or_else(|| default_signing_key_ref(node_id.as_str()));
        let state = LocalNodeState {
            schema_version: NODE_STATE_SCHEMA_VERSION.to_string(),
            identity: LocalNodeIdentity {
                node_id,
                org_id: args.org_id.clone(),
                operator_identity: args.org_id.clone(),
                account_or_org_binding: args.org_id.clone(),
                signing_key_ref,
            },
            service: LocalNodeService {
                service_name: DEFAULT_SERVICE_NAME.to_string(),
                desired_mode: "offline".to_string(),
                observed_status: "offline".to_string(),
                service_manager: "manual".to_string(),
                update_channel: "local-dev".to_string(),
            },
            paths: LocalNodePaths {
                state_dir: path_label(&args.state_dir),
                state_file: path_label(&state_file),
            },
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        };
        save_state(&state_file, &state)?;
        (state, false)
    };
    ensure_admin_store(&args.state_dir)?;

    let output = InitOutput {
        schema_version: state.schema_version.clone(),
        initialized: true,
        existing,
        state_dir: state.paths.state_dir.clone(),
        state_file: state.paths.state_file.clone(),
        identity: InitIdentityOutput {
            node_id: state.identity.node_id,
            org_id: state.identity.org_id,
            operator_identity: state.identity.operator_identity,
            account_or_org_binding: state.identity.account_or_org_binding,
            signing_key_ref: "configured".to_string(),
        },
    };

    if args.json {
        serde_json::to_string_pretty(&output)
            .map_err(|error| format!("failed to serialize init output: {error}"))
    } else {
        Ok(format!(
            "initialized: {}\nexisting: {}\nnode_id: {}\norg_id: {}\nstate_dir: {}\nsigning_key_ref: configured",
            output.initialized,
            output.existing,
            output.identity.node_id,
            output.identity.org_id,
            output.state_dir
        ))
    }
}

fn validate_existing_state(state: &LocalNodeState, args: &InitArgs) -> Result<(), String> {
    if state.schema_version != NODE_STATE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported oa-node state schema '{}'",
            state.schema_version
        ));
    }
    if state.identity.org_id != args.org_id {
        return Err(format!(
            "existing oa-node state is bound to org '{}', not '{}'",
            state.identity.org_id, args.org_id
        ));
    }
    if let Some(node_id) = &args.node_id {
        if state.identity.node_id != *node_id {
            return Err(format!(
                "existing oa-node state is bound to node '{}', not '{}'",
                state.identity.node_id, node_id
            ));
        }
    }
    if let Some(signing_key_ref) = &args.signing_key_ref {
        if state.identity.signing_key_ref != *signing_key_ref {
            return Err(
                "existing oa-node state uses a different signing-key reference".to_string(),
            );
        }
    }
    Ok(())
}

fn status_node(args: StatusArgs) -> Result<String, String> {
    let snapshot = state_file(&args.state_dir)
        .exists()
        .then(|| load_state(&state_file(&args.state_dir)))
        .transpose()?
        .map(|state| {
            load_admin_store(&args.state_dir).map(|admin| snapshot_from_state(&state, &admin))
        })
        .transpose()?
        .unwrap_or_else(CloudNodeSnapshot::managed_scaffold);
    snapshot
        .validate_contract()
        .map_err(|error| format!("cloud node status violates contract: {error}"))?;

    if args.json {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| format!("failed to serialize cloud node status: {error}"))
    } else {
        Ok(format!(
            "service: {DEFAULT_SERVICE_NAME}\nstatus: {:?}\ncontract: {}\nnode_id: {}\noperator_identity: {}\nready: false",
            snapshot.lifecycle.observed_status,
            snapshot.contract_version,
            snapshot.identity.node_id,
            snapshot.identity.operator_identity
        ))
    }
}

fn doctor_node(args: StatusArgs) -> Result<String, String> {
    let state_path = state_file(&args.state_dir);
    let state_exists = state_path.exists();
    let admin_status = if admin_store_path(&args.state_dir).exists() {
        "pass"
    } else {
        "missing"
    };
    if args.json {
        Ok(format!(
            "{{\"service\":\"oa-node\",\"checks\":[{{\"name\":\"repo_scaffold\",\"status\":\"pass\"}},{{\"name\":\"local_state\",\"status\":\"{}\"}},{{\"name\":\"admin_store\",\"status\":\"{}\"}}]}}",
            if state_exists { "pass" } else { "missing" },
            admin_status
        ))
    } else {
        Ok(format!(
            "repo_scaffold: pass\nlocal_state: {}\nadmin_store: {}",
            if state_exists { "pass" } else { "missing" },
            admin_status
        ))
    }
}

fn detect_node(args: DetectArgs) -> Result<String, String> {
    let report = detect_capabilities();
    render_json_or_human(
        args.json,
        &report,
        format!(
            "os: {}\narch: {}\nlogical_cpu_count: {}\nsellable_capability_count: {}\ndegraded_backend_count: {}",
            report.present_hardware.os,
            report.present_hardware.arch,
            report.present_hardware.logical_cpu_count,
            report.sellable_capabilities.len(),
            report.degraded_backends.len()
        ),
    )
}

fn run_service_command(command: ServiceCommand) -> Result<String, String> {
    match command {
        ServiceCommand::Install(args) => install_service(args),
        ServiceCommand::Action(args) => service_action(args),
    }
}

fn install_service(args: ServiceInstallArgs) -> Result<String, String> {
    let mut state = load_state(&state_file(&args.state_dir))?;
    ensure_admin_store(&args.state_dir)?;
    state.service.service_manager = args.service_manager;
    state.service.service_name = args
        .service_name
        .unwrap_or_else(|| DEFAULT_SERVICE_NAME.to_string());
    state.service.observed_status = "installed".to_string();
    state.updated_at_ms = now_epoch_ms()?;
    save_state(&state_file(&args.state_dir), &state)?;
    set_admin_observed_status(&args.state_dir, "offline")?;
    let event = append_service_event(&args.state_dir, "install", &state.service)?;
    append_service_health_event(&args.state_dir, &event)?;
    render_service_output(args.json, "install", state.service, Some(event.event_id))
}

fn service_action(args: ServiceActionArgs) -> Result<String, String> {
    let mut state = load_state(&state_file(&args.state_dir))?;
    ensure_admin_store(&args.state_dir)?;
    match args.action.as_str() {
        "status" => render_service_output(args.json, "status", state.service, None),
        "start" => {
            ensure_service_installed(&state)?;
            state.service.observed_status = "running".to_string();
            state.updated_at_ms = now_epoch_ms()?;
            save_state(&state_file(&args.state_dir), &state)?;
            set_admin_observed_status(&args.state_dir, "online")?;
            let event = append_service_event(&args.state_dir, "start", &state.service)?;
            append_service_health_event(&args.state_dir, &event)?;
            render_service_output(args.json, "start", state.service, Some(event.event_id))
        }
        "stop" => {
            ensure_service_installed(&state)?;
            state.service.observed_status = "stopped".to_string();
            state.updated_at_ms = now_epoch_ms()?;
            save_state(&state_file(&args.state_dir), &state)?;
            set_admin_observed_status(&args.state_dir, "offline")?;
            let event = append_service_event(&args.state_dir, "stop", &state.service)?;
            append_service_health_event(&args.state_dir, &event)?;
            render_service_output(args.json, "stop", state.service, Some(event.event_id))
        }
        "restart" => {
            ensure_service_installed(&state)?;
            state.service.observed_status = "running".to_string();
            state.updated_at_ms = now_epoch_ms()?;
            save_state(&state_file(&args.state_dir), &state)?;
            set_admin_observed_status(&args.state_dir, "online")?;
            let event = append_service_event(&args.state_dir, "restart", &state.service)?;
            append_service_health_event(&args.state_dir, &event)?;
            render_service_output(args.json, "restart", state.service, Some(event.event_id))
        }
        "uninstall" => {
            ensure_service_installed(&state)?;
            state.service.service_manager = "manual".to_string();
            state.service.observed_status = "uninstalled".to_string();
            state.updated_at_ms = now_epoch_ms()?;
            save_state(&state_file(&args.state_dir), &state)?;
            set_admin_observed_status(&args.state_dir, "offline")?;
            let event = append_service_event(&args.state_dir, "uninstall", &state.service)?;
            append_service_health_event(&args.state_dir, &event)?;
            render_service_output(args.json, "uninstall", state.service, Some(event.event_id))
        }
        other => Err(format!("unsupported service action: {other}")),
    }
}

fn render_service_output(
    json: bool,
    action: &str,
    service: LocalNodeService,
    event_id: Option<String>,
) -> Result<String, String> {
    let output = ServiceOutput {
        schema_version: SERVICE_MANAGER_SCHEMA_VERSION.to_string(),
        action: action.to_string(),
        service,
        event_id,
    };
    render_json_or_human(
        json,
        &output,
        format!(
            "action: {}\nservice_name: {}\nservice_manager: {}\nservice_status: {}",
            output.action,
            output.service.service_name,
            output.service.service_manager,
            output.service.observed_status
        ),
    )
}

fn ensure_service_installed(state: &LocalNodeState) -> Result<(), String> {
    if state.service.service_manager == "manual" || state.service.observed_status == "uninstalled" {
        return Err("service is not installed; run service install first".to_string());
    }
    Ok(())
}

fn set_admin_observed_status(state_dir: &Path, observed_status: &str) -> Result<(), String> {
    let mut store = load_admin_store(state_dir)?.store;
    store.observed_status = observed_status.to_string();
    store.updated_at_ms = now_epoch_ms()?;
    save_admin_store(state_dir, &store)
}

fn append_service_event(
    state_dir: &Path,
    action: &str,
    service: &LocalNodeService,
) -> Result<ServiceEvent, String> {
    validate_service_event_fields(action, service)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let occurred_at_ms = now_epoch_ms()?;
    let event = ServiceEvent {
        schema_version: SERVICE_MANAGER_SCHEMA_VERSION.to_string(),
        event_id: format!("service.{}.{}", sanitize_identifier(action), occurred_at_ms),
        occurred_at_ms,
        action: action.to_string(),
        service_name: service.service_name.clone(),
        service_manager: service.service_manager.clone(),
        service_status: service.observed_status.clone(),
    };
    validate_service_event(&event)?;
    let line = serde_json::to_string(&event)
        .map_err(|error| format!("failed to serialize service event: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(service_events_path(state_dir))
        .map_err(|error| format!("failed to open service event log: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to append service event: {error}"))?;
    Ok(event)
}

fn append_service_health_event(state_dir: &Path, event: &ServiceEvent) -> Result<(), String> {
    append_health_event_record(
        state_dir,
        "info",
        format!("service_{}", event.action).as_str(),
        format!(
            "service_name={};manager={};status={}",
            event.service_name, event.service_manager, event.service_status
        )
        .as_str(),
    )
    .map(|_| ())
}

fn validate_service_manager(manager: &str) -> Result<(), String> {
    match manager {
        "launchd" | "systemd" => Ok(()),
        other => Err(format!("unsupported service manager: {other}")),
    }
}

fn validate_service_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("service name must not be empty".to_string());
    }
    if name.contains('/') || name.contains('\\') || contains_forbidden_service_marker(name) {
        return Err("service name must be bounded and must not contain secret markers".to_string());
    }
    Ok(())
}

fn validate_service_event_fields(action: &str, service: &LocalNodeService) -> Result<(), String> {
    validate_service_name(service.service_name.as_str())?;
    if service.service_manager != "manual" {
        validate_service_manager(service.service_manager.as_str())?;
    }
    for value in [
        action,
        service.service_manager.as_str(),
        service.observed_status.as_str(),
    ] {
        if value.trim().is_empty() || contains_forbidden_service_marker(value) {
            return Err("service event field is invalid".to_string());
        }
    }
    Ok(())
}

fn validate_service_event(event: &ServiceEvent) -> Result<(), String> {
    if event.schema_version != SERVICE_MANAGER_SCHEMA_VERSION {
        return Err(format!(
            "unsupported service event schema '{}'",
            event.schema_version
        ));
    }
    for value in [
        event.event_id.as_str(),
        event.action.as_str(),
        event.service_name.as_str(),
        event.service_manager.as_str(),
        event.service_status.as_str(),
    ] {
        if value.trim().is_empty() || contains_forbidden_service_marker(value) {
            return Err("service event contains forbidden marker".to_string());
        }
    }
    Ok(())
}

fn contains_forbidden_service_marker(value: &str) -> bool {
    contains_forbidden_control_marker(value)
}

fn run_update_command(command: UpdateCommand) -> Result<String, String> {
    match command {
        UpdateCommand::Status(args) => update_status(args),
        UpdateCommand::PolicySet(args) => update_policy_set(args),
        UpdateCommand::Apply(args) => update_apply(args),
        UpdateCommand::Rollback(args) => update_rollback(args),
    }
}

fn update_status(args: StatusArgs) -> Result<String, String> {
    let admin = load_admin_store(&args.state_dir)?;
    let output = UpdateStatusOutput {
        schema_version: UPDATE_POLICY_SCHEMA_VERSION.to_string(),
        updates: admin.store.updates,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "channel: {}\ncurrent_version: {}\npending_update: {}\npinned_version: {}\ndeferred: {}",
            output.updates.channel,
            output.updates.current_version.as_deref().unwrap_or("unknown"),
            output.updates.pending_update.as_deref().unwrap_or("none"),
            output.updates.pinned_version.as_deref().unwrap_or("none"),
            output.updates.deferred
        ),
    )
}

fn update_policy_set(args: UpdatePolicySetArgs) -> Result<String, String> {
    let mut state = load_state(&state_file(&args.state_dir))?;
    ensure_admin_store(&args.state_dir)?;
    let mut store = load_admin_store(&args.state_dir)?.store;
    store.updates.channel = args.channel.clone();
    store.updates.pinned_version = args.pinned_version;
    store.updates.deferred = args.deferred;
    store.updates.last_checked_at_ms = Some(now_epoch_ms()?);
    store.updated_at_ms = store
        .updates
        .last_checked_at_ms
        .unwrap_or(store.updated_at_ms);
    state.service.update_channel = args.channel;
    state.updated_at_ms = store.updated_at_ms;
    save_state(&state_file(&args.state_dir), &state)?;
    save_admin_store(&args.state_dir, &store)?;
    let output = UpdatePolicyOutput {
        schema_version: UPDATE_POLICY_SCHEMA_VERSION.to_string(),
        updates: store.updates,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "channel: {}\npinned_version: {}\ndeferred: {}",
            output.updates.channel,
            output.updates.pinned_version.as_deref().unwrap_or("none"),
            output.updates.deferred
        ),
    )
}

fn update_apply(args: UpdateApplyArgs) -> Result<String, String> {
    validate_update_request(
        args.target_version.as_str(),
        args.signer.as_str(),
        args.signature_digest.as_str(),
    )?;
    ensure_admin_store(&args.state_dir)?;
    let mut store = load_admin_store(&args.state_dir)?.store;
    let previous_version = store.updates.current_version.clone();
    let result = update_apply_result(&mut store, &args)?;
    let emitted_at_ms = now_epoch_ms()?;
    store.updates.last_checked_at_ms = Some(emitted_at_ms);
    store.updated_at_ms = emitted_at_ms;
    save_admin_store(&args.state_dir, &store)?;
    apply_update_result_health(&args.state_dir, result.as_str())?;
    let receipt = build_update_receipt(
        "apply",
        previous_version,
        args.target_version,
        args.signer,
        args.signature_digest,
        result,
    )?;
    append_update_receipt(&args.state_dir, &receipt)?;
    render_json_or_human(
        args.json,
        &receipt,
        format!(
            "action: {}\nprevious_version: {}\ntarget_version: {}\nresult: {}\nreceipt_digest: {}",
            receipt.action,
            receipt.previous_version.as_deref().unwrap_or("none"),
            receipt.target_version,
            receipt.result,
            receipt.receipt_digest
        ),
    )
}

fn update_apply_result(store: &mut AdminStore, args: &UpdateApplyArgs) -> Result<String, String> {
    if store.updates.deferred {
        store.updates.pending_update = Some(args.target_version.clone());
        return Ok("deferred".to_string());
    }
    if let Some(pin) = &store.updates.pinned_version {
        if pin != &args.target_version {
            store.updates.pending_update = Some(args.target_version.clone());
            return Ok("deferred_pinned".to_string());
        }
    }
    if args.result == "failed" {
        store.updates.pending_update = None;
        if store.updates.current_version.is_some() {
            store.observed_status = "degraded".to_string();
            store.last_degradation_reason = Some("update_failed_rolled_back".to_string());
            return Ok("rolled_back".to_string());
        }
        store.observed_status = "quarantined".to_string();
        store.quarantine.quarantined = true;
        store.quarantine.reason = Some("update_failed_without_previous_version".to_string());
        store.quarantine.since_ms = Some(now_epoch_ms()?);
        return Ok("quarantined".to_string());
    }
    store.updates.current_version = Some(args.target_version.clone());
    store.updates.pending_update = None;
    store.observed_status = "online".to_string();
    store.last_degradation_reason = None;
    Ok("succeeded".to_string())
}

fn update_rollback(args: UpdateRollbackArgs) -> Result<String, String> {
    validate_update_request(
        args.target_version.as_str(),
        args.signer.as_str(),
        args.signature_digest.as_str(),
    )?;
    ensure_admin_store(&args.state_dir)?;
    let mut store = load_admin_store(&args.state_dir)?.store;
    let previous_version = store.updates.current_version.clone();
    store.updates.current_version = Some(args.target_version.clone());
    store.updates.pending_update = None;
    store.observed_status = "online".to_string();
    store.last_degradation_reason = None;
    let emitted_at_ms = now_epoch_ms()?;
    store.updates.last_checked_at_ms = Some(emitted_at_ms);
    store.updated_at_ms = emitted_at_ms;
    save_admin_store(&args.state_dir, &store)?;
    append_health_event_record(
        &args.state_dir,
        "info",
        "update_rollback",
        format!("target_version={}", args.target_version).as_str(),
    )?;
    let receipt = build_update_receipt(
        "rollback",
        previous_version,
        args.target_version,
        args.signer,
        args.signature_digest,
        "rolled_back".to_string(),
    )?;
    append_update_receipt(&args.state_dir, &receipt)?;
    render_json_or_human(
        args.json,
        &receipt,
        format!(
            "action: {}\ntarget_version: {}\nresult: {}\nreceipt_digest: {}",
            receipt.action, receipt.target_version, receipt.result, receipt.receipt_digest
        ),
    )
}

fn apply_update_result_health(state_dir: &Path, result: &str) -> Result<(), String> {
    let (severity, code) = match result {
        "succeeded" => ("info", "update_succeeded"),
        "rolled_back" => ("warning", "update_failed_rolled_back"),
        "quarantined" => ("error", "update_failed_quarantined"),
        "deferred" => ("info", "update_deferred"),
        "deferred_pinned" => ("info", "update_deferred_pinned"),
        other => return Err(format!("unsupported update result: {other}")),
    };
    append_health_event_record(
        state_dir,
        severity,
        code,
        format!("result={result}").as_str(),
    )
    .map(|_| ())
}

fn build_update_receipt(
    action: &str,
    previous_version: Option<String>,
    target_version: String,
    signer: String,
    signature_digest: String,
    result: String,
) -> Result<UpdateReceipt, String> {
    validate_update_request(
        target_version.as_str(),
        signer.as_str(),
        signature_digest.as_str(),
    )?;
    validate_update_result(result.as_str())?;
    let emitted_at_ms = now_epoch_ms()?;
    let encoded = serde_json::to_vec(&(
        UPDATE_RECEIPT_SCHEMA_VERSION,
        action,
        &previous_version,
        target_version.as_str(),
        signer.as_str(),
        signature_digest.as_str(),
        result.as_str(),
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize update receipt digest material: {error}"))?;
    let receipt = UpdateReceipt {
        schema_version: UPDATE_RECEIPT_SCHEMA_VERSION.to_string(),
        receipt_id: format!(
            "update.{action}.{}.{}",
            sanitize_identifier(&target_version),
            emitted_at_ms
        ),
        action: action.to_string(),
        previous_version,
        target_version,
        signer,
        signature_digest,
        result,
        receipt_digest: sha256_prefixed(&encoded),
        emitted_at_ms,
    };
    validate_update_receipt(&receipt)?;
    Ok(receipt)
}

fn append_update_receipt(state_dir: &Path, receipt: &UpdateReceipt) -> Result<(), String> {
    validate_update_receipt(receipt)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize update receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(update_receipts_path(state_dir))
        .map_err(|error| format!("failed to open update receipt log: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to append update receipt: {error}"))
}

fn validate_update_request(
    target_version: &str,
    signer: &str,
    signature_digest: &str,
) -> Result<(), String> {
    validate_update_label(target_version, "target_version")?;
    validate_update_label(signer, "signer")?;
    if !signature_digest.starts_with("sha256:")
        || contains_forbidden_control_marker(signature_digest)
    {
        return Err("signature digest must be a sha256 digest reference".to_string());
    }
    Ok(())
}

fn validate_update_label(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("update {label} must not be empty"));
    }
    if contains_forbidden_control_marker(value) {
        return Err(format!("update {label} contains forbidden marker"));
    }
    Ok(())
}

fn validate_update_result(result: &str) -> Result<(), String> {
    match result {
        "succeeded" | "failed" | "rolled_back" | "quarantined" | "deferred" | "deferred_pinned" => {
            Ok(())
        }
        other => Err(format!("unsupported update result: {other}")),
    }
}

fn validate_update_receipt(receipt: &UpdateReceipt) -> Result<(), String> {
    if receipt.schema_version != UPDATE_RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported update receipt schema '{}'",
            receipt.schema_version
        ));
    }
    validate_update_label(receipt.action.as_str(), "action")?;
    if let Some(previous) = &receipt.previous_version {
        validate_update_label(previous, "previous_version")?;
    }
    validate_update_request(
        receipt.target_version.as_str(),
        receipt.signer.as_str(),
        receipt.signature_digest.as_str(),
    )?;
    validate_update_result(receipt.result.as_str())?;
    if !receipt.receipt_digest.starts_with("sha256:") {
        return Err("update receipt digest must start with sha256:".to_string());
    }
    Ok(())
}

fn contains_forbidden_control_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "secret-token",
        "bearer ",
        "api_key",
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

fn run_quarantine_command(command: QuarantineCommand) -> Result<String, String> {
    match command {
        QuarantineCommand::Status(args) => quarantine_status(args),
        QuarantineCommand::Enter(args) => quarantine_enter(args),
        QuarantineCommand::Exit(args) => quarantine_exit(args),
    }
}

fn quarantine_status(args: StatusArgs) -> Result<String, String> {
    let admin = load_admin_store(&args.state_dir)?;
    let output = QuarantineStatusOutput {
        schema_version: QUARANTINE_STATUS_SCHEMA_VERSION.to_string(),
        quarantine: admin.store.quarantine,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "quarantined: {}\nreason: {}\nworkroom_policy: {}\naffected_workroom_count: {}",
            output.quarantine.quarantined,
            output.quarantine.reason.as_deref().unwrap_or("none"),
            output
                .quarantine
                .workroom_policy
                .as_deref()
                .unwrap_or("none"),
            output.quarantine.affected_workrooms.len()
        ),
    )
}

fn quarantine_enter(args: QuarantineEnterArgs) -> Result<String, String> {
    ensure_admin_store(&args.state_dir)?;
    let mut store = load_admin_store(&args.state_dir)?.store;
    let occurred_at_ms = now_epoch_ms()?;
    let affected_workrooms = if args.workrooms.is_empty() {
        vec!["all_active_workrooms".to_string()]
    } else {
        args.workrooms
    };
    store.quarantine.quarantined = true;
    store.quarantine.reason = Some(args.reason.clone());
    store.quarantine.since_ms = Some(occurred_at_ms);
    store.quarantine.workroom_policy = Some(args.workroom_policy.clone());
    store.quarantine.affected_workrooms = affected_workrooms.clone();
    store.desired_mode = "quarantined".to_string();
    store.observed_status = "quarantined".to_string();
    store.updated_at_ms = occurred_at_ms;
    save_admin_store(&args.state_dir, &store)?;
    append_health_event_record(
        &args.state_dir,
        "warning",
        "quarantine_entered",
        format!(
            "reason={};workroom_policy={};affected_workrooms={}",
            args.reason,
            args.workroom_policy,
            affected_workrooms.len()
        )
        .as_str(),
    )?;
    let receipt = build_quarantine_receipt(
        "enter",
        args.reason,
        args.workroom_policy,
        affected_workrooms,
        "new_work_blocked".to_string(),
    )?;
    append_quarantine_receipt(&args.state_dir, &receipt)?;
    let output = QuarantineActionOutput {
        schema_version: QUARANTINE_STATUS_SCHEMA_VERSION.to_string(),
        quarantine: store.quarantine,
        receipt,
    };
    render_quarantine_action_output(args.json, output)
}

fn quarantine_exit(args: QuarantineExitArgs) -> Result<String, String> {
    ensure_admin_store(&args.state_dir)?;
    let mut store = load_admin_store(&args.state_dir)?.store;
    let prior_policy = store
        .quarantine
        .workroom_policy
        .clone()
        .unwrap_or_else(|| "none".to_string());
    let prior_workrooms = store.quarantine.affected_workrooms.clone();
    store.quarantine.quarantined = false;
    store.quarantine.reason = Some(args.reason.clone());
    store.quarantine.since_ms = None;
    store.quarantine.workroom_policy = None;
    store.quarantine.affected_workrooms = Vec::new();
    store.desired_mode = "offline".to_string();
    store.observed_status = "offline".to_string();
    store.updated_at_ms = now_epoch_ms()?;
    save_admin_store(&args.state_dir, &store)?;
    append_health_event_record(
        &args.state_dir,
        "info",
        "quarantine_exited",
        format!("reason={}", args.reason).as_str(),
    )?;
    let receipt = build_quarantine_receipt(
        "exit",
        args.reason,
        prior_policy,
        prior_workrooms,
        "released".to_string(),
    )?;
    append_quarantine_receipt(&args.state_dir, &receipt)?;
    let output = QuarantineActionOutput {
        schema_version: QUARANTINE_STATUS_SCHEMA_VERSION.to_string(),
        quarantine: store.quarantine,
        receipt,
    };
    render_quarantine_action_output(args.json, output)
}

fn render_quarantine_action_output(
    json: bool,
    output: QuarantineActionOutput,
) -> Result<String, String> {
    render_json_or_human(
        json,
        &output,
        format!(
            "quarantined: {}\naction: {}\nresult: {}\nreceipt_digest: {}",
            output.quarantine.quarantined,
            output.receipt.action,
            output.receipt.result,
            output.receipt.receipt_digest
        ),
    )
}

fn build_quarantine_receipt(
    action: &str,
    reason: String,
    workroom_policy: String,
    affected_workrooms: Vec<String>,
    result: String,
) -> Result<QuarantineReceipt, String> {
    validate_quarantine_reason(action)?;
    validate_quarantine_reason(reason.as_str())?;
    if workroom_policy != "none" {
        validate_workroom_policy(workroom_policy.as_str())?;
    }
    validate_quarantine_reason(result.as_str())?;
    for workroom in &affected_workrooms {
        validate_quarantine_reason(workroom)?;
    }
    let emitted_at_ms = now_epoch_ms()?;
    let encoded = serde_json::to_vec(&(
        QUARANTINE_RECEIPT_SCHEMA_VERSION,
        action,
        reason.as_str(),
        workroom_policy.as_str(),
        &affected_workrooms,
        result.as_str(),
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize quarantine receipt digest material: {error}"))?;
    let receipt = QuarantineReceipt {
        schema_version: QUARANTINE_RECEIPT_SCHEMA_VERSION.to_string(),
        receipt_id: format!("quarantine.{action}.{emitted_at_ms}"),
        action: action.to_string(),
        reason,
        workroom_policy,
        affected_workrooms,
        result,
        receipt_digest: sha256_prefixed(&encoded),
        emitted_at_ms,
    };
    validate_quarantine_receipt(&receipt)?;
    Ok(receipt)
}

fn append_quarantine_receipt(state_dir: &Path, receipt: &QuarantineReceipt) -> Result<(), String> {
    validate_quarantine_receipt(receipt)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize quarantine receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(quarantine_receipts_path(state_dir))
        .map_err(|error| format!("failed to open quarantine receipt log: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("failed to append quarantine receipt: {error}"))
}

fn validate_workroom_policy(policy: &str) -> Result<(), String> {
    match policy {
        "pause" | "migrate" | "close" => Ok(()),
        other => Err(format!("unsupported quarantine workroom policy: {other}")),
    }
}

fn validate_quarantine_reason(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("quarantine field must not be empty".to_string());
    }
    if contains_forbidden_control_marker(value) {
        return Err("quarantine field contains forbidden marker".to_string());
    }
    Ok(())
}

fn validate_quarantine_receipt(receipt: &QuarantineReceipt) -> Result<(), String> {
    if receipt.schema_version != QUARANTINE_RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported quarantine receipt schema '{}'",
            receipt.schema_version
        ));
    }
    for value in [
        receipt.receipt_id.as_str(),
        receipt.action.as_str(),
        receipt.reason.as_str(),
        receipt.workroom_policy.as_str(),
        receipt.result.as_str(),
        receipt.receipt_digest.as_str(),
    ] {
        validate_quarantine_reason(value)?;
    }
    for workroom in &receipt.affected_workrooms {
        validate_quarantine_reason(workroom)?;
    }
    if !receipt.receipt_digest.starts_with("sha256:") {
        return Err("quarantine receipt digest must start with sha256:".to_string());
    }
    Ok(())
}

fn run_settlement_command(command: SettlementCommand) -> Result<String, String> {
    match command {
        SettlementCommand::Status(args) => settlement_status(args),
        SettlementCommand::ModeSet(args) => settlement_mode_set(args),
        SettlementCommand::ReceiptAppend(args) => settlement_receipt_append(args),
    }
}

fn settlement_status(args: StatusArgs) -> Result<String, String> {
    let admin = load_admin_store(&args.state_dir)?;
    let output = SettlementStatusOutput {
        schema_version: SETTLEMENT_STATUS_SCHEMA_VERSION.to_string(),
        settlement: admin.store.settlement,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "mode: {}\ntreasury_ref: {}\nnexus_ref: {}",
            output.settlement.mode,
            output.settlement.treasury_ref.as_deref().unwrap_or("none"),
            output.settlement.nexus_ref.as_deref().unwrap_or("none")
        ),
    )
}

fn settlement_mode_set(args: SettlementModeSetArgs) -> Result<String, String> {
    ensure_admin_store(&args.state_dir)?;
    let mut store = load_admin_store(&args.state_dir)?.store;
    store.settlement.mode = args.mode;
    store.settlement.treasury_ref = args.treasury_ref;
    store.settlement.nexus_ref = args.nexus_ref;
    store.updated_at_ms = now_epoch_ms()?;
    save_admin_store(&args.state_dir, &store)?;
    let output = SettlementStatusOutput {
        schema_version: SETTLEMENT_STATUS_SCHEMA_VERSION.to_string(),
        settlement: store.settlement,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "mode: {}\ntreasury_ref: {}\nnexus_ref: {}",
            output.settlement.mode,
            output.settlement.treasury_ref.as_deref().unwrap_or("none"),
            output.settlement.nexus_ref.as_deref().unwrap_or("none")
        ),
    )
}

fn settlement_receipt_append(args: SettlementReceiptAppendArgs) -> Result<String, String> {
    ensure_admin_store(&args.state_dir)?;
    let mut store = load_admin_store(&args.state_dir)?.store;
    if store.settlement.mode != "internal-accounting" {
        return Err("settlement receipts require internal-accounting mode".to_string());
    }
    let receipt = build_settlement_receipt(
        store.settlement.mode.as_str(),
        args.amount_microusd,
        args.treasury_ref,
        args.nexus_ref,
    )?;
    append_settlement_receipt(&args.state_dir, &receipt)?;
    store.settlement.treasury_ref = Some(receipt.treasury_ref.clone());
    store.settlement.nexus_ref = Some(receipt.nexus_ref.clone());
    store.receipt_cursors.accounting_receipt_cursor = Some(receipt.receipt_digest.clone());
    store.updated_at_ms = receipt.emitted_at_ms;
    save_admin_store(&args.state_dir, &store)?;
    let output = SettlementReceiptOutput {
        schema_version: SETTLEMENT_STATUS_SCHEMA_VERSION.to_string(),
        settlement: store.settlement,
        receipt,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "mode: {}\namount_microusd: {}\nreceipt_digest: {}",
            output.settlement.mode, output.receipt.amount_microusd, output.receipt.receipt_digest
        ),
    )
}

fn build_settlement_receipt(
    mode: &str,
    amount_microusd: u64,
    treasury_ref: String,
    nexus_ref: String,
) -> Result<SettlementReceipt, String> {
    validate_managed_settlement_mode(mode)?;
    validate_settlement_ref(treasury_ref.as_str(), "treasury_ref")?;
    validate_settlement_ref(nexus_ref.as_str(), "nexus_ref")?;
    if amount_microusd == 0 {
        return Err("settlement amount must be greater than zero".to_string());
    }
    let emitted_at_ms = now_epoch_ms()?;
    let encoded = serde_json::to_vec(&(
        SETTLEMENT_RECEIPT_SCHEMA_VERSION,
        mode,
        amount_microusd,
        treasury_ref.as_str(),
        nexus_ref.as_str(),
        "reconciled",
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize settlement receipt digest material: {error}"))?;
    let receipt = SettlementReceipt {
        schema_version: SETTLEMENT_RECEIPT_SCHEMA_VERSION.to_string(),
        receipt_id: format!("settlement.{}.{}", sanitize_identifier(mode), emitted_at_ms),
        mode: mode.to_string(),
        amount_microusd,
        treasury_ref,
        nexus_ref,
        result: "reconciled".to_string(),
        receipt_digest: sha256_prefixed(&encoded),
        emitted_at_ms,
    };
    validate_settlement_receipt(&receipt)?;
    Ok(receipt)
}

fn append_settlement_receipt(state_dir: &Path, receipt: &SettlementReceipt) -> Result<(), String> {
    validate_settlement_receipt(receipt)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize settlement receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(settlement_receipts_path(state_dir))
        .map_err(|error| format!("failed to open settlement receipt log: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("failed to append settlement receipt: {error}"))
}

fn validate_managed_settlement_mode(mode: &str) -> Result<(), String> {
    match mode {
        "no-wallet" | "internal-accounting" => Ok(()),
        "contributor-wallet" => {
            Err("contributor-wallet mode belongs to public Pylon, not managed cloud".to_string())
        }
        other => Err(format!("unsupported managed settlement mode: {other}")),
    }
}

fn validate_settlement_ref(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("settlement {label} must not be empty"));
    }
    if contains_forbidden_control_marker(value) {
        return Err(format!("settlement {label} contains forbidden marker"));
    }
    Ok(())
}

fn validate_settlement_receipt(receipt: &SettlementReceipt) -> Result<(), String> {
    if receipt.schema_version != SETTLEMENT_RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported settlement receipt schema '{}'",
            receipt.schema_version
        ));
    }
    validate_managed_settlement_mode(receipt.mode.as_str())?;
    validate_settlement_ref(receipt.treasury_ref.as_str(), "treasury_ref")?;
    validate_settlement_ref(receipt.nexus_ref.as_str(), "nexus_ref")?;
    validate_settlement_ref(receipt.result.as_str(), "result")?;
    if receipt.amount_microusd == 0 {
        return Err("settlement amount must be greater than zero".to_string());
    }
    if !receipt.receipt_digest.starts_with("sha256:") {
        return Err("settlement receipt digest must start with sha256:".to_string());
    }
    Ok(())
}

fn run_broker_command(command: BrokerCommand) -> Result<String, String> {
    match command {
        BrokerCommand::Redact(args) => broker_redact(args),
    }
}

fn broker_redact(args: BrokerRedactArgs) -> Result<String, String> {
    validate_broker_kind(args.kind.as_str())?;
    let raw = fs::read_to_string(&args.input).map_err(|error| {
        format!(
            "failed to read broker input {}: {error}",
            path_label(&args.input)
        )
    })?;
    let has_secret = looks_like_broker_secret(raw.as_str());
    if has_secret && !raw.contains(FAKE_SECRET_MARKER) {
        return Err(
            "broker input contains secret-looking data without fake-secret marker".to_string(),
        );
    }
    let redacted = redact_broker_payload(raw.as_str());
    if looks_like_broker_secret(redacted.as_str()) {
        return Err("broker redaction failed to remove secret-looking data".to_string());
    }
    fs::create_dir_all(args.state_dir.join(BROKER_REDACTED_ARTIFACT_DIR)).map_err(|error| {
        format!(
            "failed to create broker artifact dir {}: {error}",
            path_label(&args.state_dir.join(BROKER_REDACTED_ARTIFACT_DIR))
        )
    })?;
    let input_digest = sha256_prefixed(raw.as_bytes());
    let redacted_digest = sha256_prefixed(redacted.as_bytes());
    let artifact_name = format!(
        "{}-{}.txt",
        args.kind,
        redacted_digest.trim_start_matches("sha256:")
    );
    let artifact_path = PathBuf::from(BROKER_REDACTED_ARTIFACT_DIR).join(artifact_name);
    fs::write(args.state_dir.join(&artifact_path), redacted.as_bytes())
        .map_err(|error| format!("failed to write broker redacted artifact: {error}"))?;
    let receipt = build_broker_redaction_receipt(
        args.kind,
        input_digest,
        path_label(&artifact_path),
        redacted_digest,
    )?;
    append_broker_redaction_receipt(&args.state_dir, &receipt)?;
    let output = BrokerRedactionOutput {
        schema_version: BROKER_REDACTION_OUTPUT_SCHEMA_VERSION.to_string(),
        receipt,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "kind: {}\nredacted_artifact_path: {}\nreceipt_digest: {}",
            output.receipt.kind,
            output.receipt.redacted_artifact_path,
            output.receipt.receipt_digest
        ),
    )
}

fn redact_broker_payload(raw: &str) -> String {
    raw.lines()
        .map(|line| {
            let line_without_fake_marker = line.replace(FAKE_SECRET_MARKER, "");
            if looks_like_broker_secret(line_without_fake_marker.as_str()) {
                "[REDACTED]".to_string()
            } else if line.contains(FAKE_SECRET_MARKER) {
                line.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_broker_redaction_receipt(
    kind: String,
    input_digest: String,
    redacted_artifact_path: String,
    redacted_digest: String,
) -> Result<BrokerRedactionReceipt, String> {
    validate_broker_kind(kind.as_str())?;
    validate_broker_receipt_field(redacted_artifact_path.as_str(), "redacted_artifact_path")?;
    for digest in [input_digest.as_str(), redacted_digest.as_str()] {
        if !digest.starts_with("sha256:") {
            return Err("broker redaction digest must start with sha256:".to_string());
        }
    }
    let emitted_at_ms = now_epoch_ms()?;
    let encoded = serde_json::to_vec(&(
        BROKER_REDACTION_RECEIPT_SCHEMA_VERSION,
        kind.as_str(),
        input_digest.as_str(),
        redacted_artifact_path.as_str(),
        redacted_digest.as_str(),
        emitted_at_ms,
    ))
    .map_err(|error| format!("failed to serialize broker receipt digest material: {error}"))?;
    let receipt = BrokerRedactionReceipt {
        schema_version: BROKER_REDACTION_RECEIPT_SCHEMA_VERSION.to_string(),
        receipt_id: format!("broker.redaction.{}.{}", kind, emitted_at_ms),
        kind,
        input_digest,
        redacted_artifact_path,
        redacted_digest,
        receipt_digest: sha256_prefixed(&encoded),
        emitted_at_ms,
    };
    validate_broker_redaction_receipt(&receipt)?;
    Ok(receipt)
}

fn append_broker_redaction_receipt(
    state_dir: &Path,
    receipt: &BrokerRedactionReceipt,
) -> Result<(), String> {
    validate_broker_redaction_receipt(receipt)?;
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize broker redaction receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(broker_redaction_receipts_path(state_dir))
        .map_err(|error| format!("failed to open broker redaction receipt log: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("failed to append broker redaction receipt: {error}"))
}

fn validate_broker_kind(kind: &str) -> Result<(), String> {
    match kind {
        "headers" | "url" | "env" | "config" | "log" | "receipt" => Ok(()),
        other => Err(format!("unsupported broker redaction kind: {other}")),
    }
}

fn validate_broker_redaction_receipt(receipt: &BrokerRedactionReceipt) -> Result<(), String> {
    if receipt.schema_version != BROKER_REDACTION_RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported broker redaction receipt schema '{}'",
            receipt.schema_version
        ));
    }
    validate_broker_kind(receipt.kind.as_str())?;
    for (value, label) in [
        (receipt.receipt_id.as_str(), "receipt_id"),
        (
            receipt.redacted_artifact_path.as_str(),
            "redacted_artifact_path",
        ),
        (receipt.input_digest.as_str(), "input_digest"),
        (receipt.redacted_digest.as_str(), "redacted_digest"),
        (receipt.receipt_digest.as_str(), "receipt_digest"),
    ] {
        validate_broker_receipt_field(value, label)?;
    }
    for digest in [
        receipt.input_digest.as_str(),
        receipt.redacted_digest.as_str(),
        receipt.receipt_digest.as_str(),
    ] {
        if !digest.starts_with("sha256:") {
            return Err("broker receipt digests must start with sha256:".to_string());
        }
    }
    Ok(())
}

fn validate_broker_receipt_field(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("broker receipt {label} must not be empty"));
    }
    if looks_like_broker_secret(value) {
        return Err(format!(
            "broker receipt {label} contains secret-looking data"
        ));
    }
    Ok(())
}

fn looks_like_broker_secret(value: &str) -> bool {
    contains_forbidden_control_marker(value) || value.to_ascii_lowercase().contains("sk-")
}

fn run_nexus_command(command: NexusCommand) -> Result<String, String> {
    let (action, args) = match command {
        NexusCommand::Register(args) => ("register", args),
        NexusCommand::Heartbeat(args) => ("heartbeat", args),
    };
    let state = load_state(&state_file(&args.state_dir))?;
    let admin = load_admin_store(&args.state_dir)?;
    let snapshot = snapshot_from_state(&state, &admin);
    snapshot
        .validate_contract()
        .map_err(|error| format!("cloud node status violates contract: {error}"))?;

    let envelope = nexus_registry_envelope(action, &state, &snapshot);
    let response = post_nexus_registry(args.base_url.as_str(), action, &envelope);
    let output = apply_nexus_registry_response(&args.state_dir, action, &snapshot, response)?;

    render_json_or_human(
        args.json,
        &output,
        format!(
            "action: {}\nstatus: {}\ndegraded: {}\nsnapshot_digest: {}",
            output.action, output.status, output.degraded, output.snapshot_digest
        ),
    )
}

fn nexus_registry_envelope(
    action: &str,
    state: &LocalNodeState,
    snapshot: &CloudNodeSnapshot,
) -> NexusRegistryEnvelope {
    let observed_status = observed_status_label(&snapshot.lifecycle.observed_status).to_string();
    let desired_mode = desired_mode_label(&snapshot.lifecycle.desired_mode).to_string();
    let signing_payload = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        NEXUS_REGISTRY_SCHEMA_VERSION,
        action,
        state.identity.node_id,
        state.identity.org_id,
        snapshot.evidence.current_snapshot_digest,
        observed_status,
        desired_mode,
        state.identity.signing_key_ref
    );
    NexusRegistryEnvelope {
        schema_version: NEXUS_REGISTRY_SCHEMA_VERSION.to_string(),
        action: action.to_string(),
        node_id: state.identity.node_id.clone(),
        org_id: state.identity.org_id.clone(),
        snapshot_digest: snapshot.evidence.current_snapshot_digest.clone(),
        observed_status,
        desired_mode,
        signature: NexusSignature {
            algorithm: "sha256-ref-bound-mvp".to_string(),
            signing_key_ref: state.identity.signing_key_ref.clone(),
            digest: sha256_prefixed(signing_payload.as_bytes()),
        },
    }
}

fn post_nexus_registry(
    base_url: &str,
    action: &str,
    envelope: &NexusRegistryEnvelope,
) -> NexusRegistryResponse {
    let url = nexus_registry_url(base_url, action);
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return NexusRegistryResponse {
                status: "rejected".to_string(),
                desired_mode: None,
                registration_expires_at_ms: None,
                detail: Some(format!("nexus_client_unavailable: {error}")),
            };
        }
    };

    let response = match client.post(url.as_str()).json(envelope).send() {
        Ok(response) => response,
        Err(error) => {
            return NexusRegistryResponse {
                status: "rejected".to_string(),
                desired_mode: None,
                registration_expires_at_ms: None,
                detail: Some(format!("nexus_transport_error: {error}")),
            };
        }
    };

    let status = response.status();
    let parsed = response.json::<NexusRegistryResponse>();
    match parsed {
        Ok(parsed) if status.is_success() => parsed,
        Ok(mut parsed) => {
            if parsed.status.trim().is_empty() {
                parsed.status = "rejected".to_string();
            }
            if parsed.detail.is_none() {
                parsed.detail = Some(format!("nexus_http_status: {status}"));
            }
            parsed
        }
        Err(error) => NexusRegistryResponse {
            status: "rejected".to_string(),
            desired_mode: None,
            registration_expires_at_ms: None,
            detail: Some(format!(
                "nexus_response_invalid: status={status} error={error}"
            )),
        },
    }
}

fn apply_nexus_registry_response(
    state_dir: &Path,
    action: &str,
    snapshot: &CloudNodeSnapshot,
    response: NexusRegistryResponse,
) -> Result<NexusRegistryOutput, String> {
    let mut store = load_admin_store(state_dir)?.store;
    let normalized_status = response.status.trim().to_ascii_lowercase();
    let mut output_status = if normalized_status.is_empty() {
        "rejected".to_string()
    } else {
        normalized_status.clone()
    };
    let detail = response.detail.clone();
    let mut degraded = false;
    let mut degradation_event = None;

    if normalized_status == "accepted" {
        if let Some(desired_mode) = &response.desired_mode {
            match parse_desired_mode_label(desired_mode.as_str()) {
                Ok(_) => store.desired_mode = desired_mode.clone(),
                Err(error) => {
                    degraded = true;
                    output_status = "rejected".to_string();
                    let code = "nexus_registration_rejected";
                    let reason =
                        format!("{code}: accepted response carried invalid desired_mode: {error}");
                    store.observed_status = "degraded".to_string();
                    store.last_degradation_reason = Some(reason.clone());
                    degradation_event = Some((code, reason));
                }
            }
        }
        if !degraded {
            if store
                .last_degradation_reason
                .as_deref()
                .is_some_and(|reason| reason.starts_with("nexus_registration_"))
            {
                store.observed_status = "offline".to_string();
            }
            store.last_degradation_reason = None;
        }
    } else {
        degraded = true;
        let code = nexus_degradation_code(normalized_status.as_str());
        let reason = format!(
            "{}: {}",
            code,
            detail
                .clone()
                .unwrap_or_else(|| "registry did not accept this node".to_string())
        );
        store.observed_status = "degraded".to_string();
        store.last_degradation_reason = Some(reason.clone());
        degradation_event = Some((code, reason));
    }

    store.updated_at_ms = now_epoch_ms()?;
    save_admin_store(state_dir, &store)?;
    if let Some((code, reason)) = degradation_event {
        append_health_event_record(state_dir, "error", code, reason.as_str())?;
    }

    Ok(NexusRegistryOutput {
        schema_version: NEXUS_REGISTRY_SCHEMA_VERSION.to_string(),
        action: action.to_string(),
        status: output_status,
        degraded,
        snapshot_digest: snapshot.evidence.current_snapshot_digest.clone(),
        desired_mode: response.desired_mode,
        registration_expires_at_ms: response.registration_expires_at_ms,
        detail,
    })
}

fn nexus_degradation_code(status: &str) -> &'static str {
    match status {
        "stale" => "nexus_registration_stale",
        "expired" => "nexus_registration_expired",
        "rejected" => "nexus_registration_rejected",
        _ => "nexus_registration_rejected",
    }
}

fn nexus_registry_url(base_url: &str, action: &str) -> String {
    format!("{}/v1/cloud/nodes/{action}", base_url.trim_end_matches('/'))
}

fn run_forge_command(command: ForgeCommand) -> Result<String, String> {
    match command {
        ForgeCommand::AssignmentReceive(args) => receive_forge_assignment(args),
    }
}

fn receive_forge_assignment(args: ForgeAssignmentReceiveArgs) -> Result<String, String> {
    let raw = fs::read_to_string(&args.file).map_err(|error| {
        format!(
            "failed to read forge assignment {}: {error}",
            path_label(&args.file)
        )
    })?;
    let assignment = serde_json::from_str::<ForgeAssignment>(&raw).map_err(|error| {
        format!(
            "failed to parse forge assignment {}: {error}",
            path_label(&args.file)
        )
    })?;
    let assignment_digest = sha256_prefixed(raw.as_bytes());
    let validation_error = assignment.validate_contract().err();
    let state_path = state_file(&args.state_dir);
    let state = state_path
        .exists()
        .then(|| load_state(&state_path))
        .transpose()?;
    let admin = state
        .as_ref()
        .map(|_| load_admin_store(&args.state_dir))
        .transpose()?;
    let (decision, reason, node_id) = forge_assignment_decision(
        &assignment,
        state.as_ref(),
        admin.as_ref(),
        validation_error,
    );
    let receipt = build_forge_assignment_receipt(
        &assignment,
        node_id.as_str(),
        decision,
        reason.as_str(),
        assignment_digest.as_str(),
    )?;
    append_forge_assignment_receipt(&args.state_dir, &receipt)?;

    render_json_or_human(
        args.json,
        &receipt,
        format!(
            "assignment_id: {}\ndecision: {}\nreason: {}\nreceipt_digest: {}",
            receipt.assignment_id,
            forge_assignment_decision_label(&receipt.decision),
            receipt.reason,
            receipt.receipt_digest
        ),
    )
}

fn forge_assignment_decision(
    assignment: &ForgeAssignment,
    state: Option<&LocalNodeState>,
    admin: Option<&AdminStoreLoad>,
    validation_error: Option<String>,
) -> (ForgeAssignmentDecision, String, String) {
    let node_id = state
        .map(|state| state.identity.node_id.clone())
        .unwrap_or_else(|| "node.uninitialized".to_string());
    if let Some(error) = validation_error {
        return (
            ForgeAssignmentDecision::Refused,
            format!("assignment_invalid: {error}"),
            node_id,
        );
    }

    let Some(state) = state else {
        return (
            ForgeAssignmentDecision::Refused,
            "node_uninitialized".to_string(),
            node_id,
        );
    };
    if assignment
        .node_id
        .as_deref()
        .is_some_and(|target| target != state.identity.node_id)
    {
        return (
            ForgeAssignmentDecision::Refused,
            "target_node_mismatch".to_string(),
            node_id,
        );
    }
    if assignment.assignment_kind == ForgeAssignmentKind::OpenEndedLabor {
        return (
            ForgeAssignmentDecision::Refused,
            "open_ended_labor_must_route_to_probe_or_forge".to_string(),
            node_id,
        );
    }

    let Some(admin) = admin else {
        return (
            ForgeAssignmentDecision::Refused,
            "admin_store_unavailable".to_string(),
            node_id,
        );
    };
    let observed_status = observed_status_from_admin(admin);
    if matches!(
        observed_status,
        ObservedStatus::Degraded | ObservedStatus::Quarantined
    ) {
        return (
            ForgeAssignmentDecision::Refused,
            format!(
                "node_not_available: {}",
                observed_status_label(&observed_status)
            ),
            node_id,
        );
    }
    let desired_mode =
        parse_desired_mode_label(admin.store.desired_mode.as_str()).unwrap_or(DesiredMode::Offline);
    if desired_mode != DesiredMode::Online {
        return (
            ForgeAssignmentDecision::Refused,
            format!("node_not_online: {}", desired_mode_label(&desired_mode)),
            node_id,
        );
    }
    if is_sandbox_assignment(assignment) {
        return sandbox_forge_assignment_decision(
            assignment,
            admin.sandbox_profiles.as_slice(),
            node_id,
        );
    }
    if assignment.assignment_kind == ForgeAssignmentKind::Workroom
        && assignment.capability.capability_id == "workroom.sidecar.scaffold"
    {
        return (
            ForgeAssignmentDecision::Accepted,
            "accepted_for_workroom_scaffold".to_string(),
            node_id,
        );
    }

    (
        ForgeAssignmentDecision::Refused,
        format!(
            "unsupported_capability: {}",
            assignment.capability.capability_id
        ),
        node_id,
    )
}

fn is_sandbox_assignment(assignment: &ForgeAssignment) -> bool {
    assignment.assignment_kind == ForgeAssignmentKind::Worker
        && assignment.capability.capability_id.starts_with("sandbox.")
}

fn sandbox_forge_assignment_decision(
    assignment: &ForgeAssignment,
    profiles: &[SandboxProfilePolicy],
    node_id: String,
) -> (ForgeAssignmentDecision, String, String) {
    let Some(policy) = assignment.sandbox.as_ref() else {
        return (
            ForgeAssignmentDecision::Refused,
            "sandbox_policy_missing".to_string(),
            node_id,
        );
    };
    if policy.profile_id != assignment.template.runtime_profile {
        return (
            ForgeAssignmentDecision::Refused,
            "sandbox_runtime_profile_mismatch".to_string(),
            node_id,
        );
    }
    if policy.execution_class != assignment.capability.capability_id {
        return (
            ForgeAssignmentDecision::Refused,
            "sandbox_execution_class_mismatch".to_string(),
            node_id,
        );
    }
    let Some(profile) = profiles
        .iter()
        .find(|profile| profile.profile_id == policy.profile_id)
    else {
        return (
            ForgeAssignmentDecision::Refused,
            format!("sandbox_profile_not_registered: {}", policy.profile_id),
            node_id,
        );
    };
    if !profile.ready {
        return (
            ForgeAssignmentDecision::Refused,
            format!("sandbox_profile_not_ready: {}", profile.profile_id),
            node_id,
        );
    }
    for (label, requested, declared) in [
        (
            "profile_digest",
            policy.profile_digest.as_str(),
            profile.profile_digest.as_str(),
        ),
        (
            "execution_class",
            policy.execution_class.as_str(),
            profile.execution_class.as_str(),
        ),
        (
            "network_policy",
            policy.network_policy.as_str(),
            profile.network_policy.as_str(),
        ),
        (
            "filesystem_policy",
            policy.filesystem_policy.as_str(),
            profile.filesystem_policy.as_str(),
        ),
        (
            "secret_policy",
            policy.secret_policy.as_str(),
            profile.secret_policy.as_str(),
        ),
    ] {
        if requested != declared {
            return (
                ForgeAssignmentDecision::Refused,
                format!("sandbox_{label}_mismatch"),
                node_id,
            );
        }
    }
    if assignment.budget.max_runtime_ms > profile.timeout_limit_ms {
        return (
            ForgeAssignmentDecision::Refused,
            "sandbox_timeout_exceeds_profile".to_string(),
            node_id,
        );
    }
    if assignment.budget.max_artifact_bytes > profile.max_artifact_bytes {
        return (
            ForgeAssignmentDecision::Refused,
            "sandbox_artifact_budget_exceeds_profile".to_string(),
            node_id,
        );
    }
    (
        ForgeAssignmentDecision::Accepted,
        "accepted_for_sandbox_profile".to_string(),
        node_id,
    )
}

fn build_forge_assignment_receipt(
    assignment: &ForgeAssignment,
    node_id: &str,
    decision: ForgeAssignmentDecision,
    reason: &str,
    assignment_digest: &str,
) -> Result<ForgeAssignmentReceipt, String> {
    let emitted_at_ms = now_epoch_ms()?;
    let receipt_id = format!(
        "forge.assignment.{}.{}",
        sanitize_identifier(assignment.assignment_id.as_str()),
        emitted_at_ms
    );
    let digest_material = serde_json::json!({
        "schema_version": FORGE_ASSIGNMENT_RECEIPT_VERSION,
        "receipt_id": receipt_id,
        "assignment_id": assignment.assignment_id,
        "node_id": node_id,
        "decision": forge_assignment_decision_label(&decision),
        "reason": reason,
        "assignment_digest": assignment_digest,
        "emitted_at_ms": emitted_at_ms,
    });
    let encoded = serde_json::to_vec(&digest_material)
        .map_err(|error| format!("failed to serialize forge receipt digest material: {error}"))?;
    let receipt = ForgeAssignmentReceipt {
        schema_version: FORGE_ASSIGNMENT_RECEIPT_VERSION.to_string(),
        receipt_id,
        assignment_id: assignment.assignment_id.clone(),
        node_id: node_id.to_string(),
        decision,
        reason: reason.to_string(),
        assignment_digest: assignment_digest.to_string(),
        receipt_digest: sha256_prefixed(encoded.as_slice()),
        emitted_at_ms,
    };
    receipt
        .validate_contract()
        .map_err(|error| format!("forge assignment receipt violates contract: {error}"))?;
    Ok(receipt)
}

fn append_forge_assignment_receipt(
    state_dir: &Path,
    receipt: &ForgeAssignmentReceipt,
) -> Result<(), String> {
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize forge assignment receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(forge_assignment_receipts_path(state_dir))
        .map_err(|error| format!("failed to open forge assignment receipt log: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("failed to append forge assignment receipt: {error}"))?;

    let mut store = load_admin_store(state_dir)?.store;
    store.receipt_cursors.job_receipt_cursor = Some(receipt.receipt_digest.clone());
    store.updated_at_ms = receipt.emitted_at_ms;
    save_admin_store(state_dir, &store)
}

fn forge_assignment_decision_label(decision: &ForgeAssignmentDecision) -> &'static str {
    match decision {
        ForgeAssignmentDecision::Accepted => "accepted",
        ForgeAssignmentDecision::Refused => "refused",
    }
}

fn run_sandbox_command(command: SandboxCommand) -> Result<String, String> {
    match command {
        SandboxCommand::ProfileRegister(args) => register_sandbox_profile(args),
        SandboxCommand::ProfileList(args) => list_sandbox_profiles(args),
    }
}

fn register_sandbox_profile(args: SandboxProfileRegisterArgs) -> Result<String, String> {
    let profile = SandboxProfilePolicy {
        schema_version: SANDBOX_PROFILE_POLICY_SCHEMA_VERSION.to_string(),
        profile_id: args.profile_id,
        profile_digest: args.profile_digest,
        execution_class: args.execution_class,
        network_policy: args.network_policy,
        filesystem_policy: args.filesystem_policy,
        timeout_limit_ms: args.timeout_limit_ms,
        max_artifact_bytes: args.max_artifact_bytes,
        secret_policy: args.secret_policy,
        ready: args.ready,
        updated_at_ms: now_epoch_ms()?,
    };
    validate_sandbox_profile_policy(&profile)?;
    let mut profiles = load_sandbox_profiles(&args.state_dir)?;
    profiles.retain(|existing| existing.profile_id != profile.profile_id);
    profiles.push(profile.clone());
    profiles.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
    save_sandbox_profiles(&args.state_dir, profiles.as_slice())?;

    let output = SandboxProfileRegisterOutput {
        schema_version: SANDBOX_PROFILE_POLICY_SCHEMA_VERSION.to_string(),
        profile,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "profile_id: {}\nprofile_digest: {}\nready: {}",
            output.profile.profile_id, output.profile.profile_digest, output.profile.ready
        ),
    )
}

fn list_sandbox_profiles(args: StatusArgs) -> Result<String, String> {
    let profiles = load_sandbox_profiles(&args.state_dir)?;
    let output = SandboxProfileListOutput {
        schema_version: SANDBOX_PROFILE_POLICY_SCHEMA_VERSION.to_string(),
        profiles,
    };
    render_json_or_human(
        args.json,
        &output,
        format!("sandbox_profile_count: {}", output.profiles.len()),
    )
}

fn validate_sandbox_profile_policy(profile: &SandboxProfilePolicy) -> Result<(), String> {
    if profile.schema_version != SANDBOX_PROFILE_POLICY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported sandbox profile schema '{}'",
            profile.schema_version
        ));
    }
    for (field, value) in [
        ("profile_id", profile.profile_id.as_str()),
        ("profile_digest", profile.profile_digest.as_str()),
        ("execution_class", profile.execution_class.as_str()),
        ("network_policy", profile.network_policy.as_str()),
        ("filesystem_policy", profile.filesystem_policy.as_str()),
        ("secret_policy", profile.secret_policy.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("sandbox profile {field} must not be empty"));
        }
        if contains_forbidden_control_marker(value) {
            return Err(format!(
                "sandbox profile {field} contains secret-looking data"
            ));
        }
    }
    if !profile.profile_digest.starts_with("sha256:") {
        return Err("sandbox profile digest must start with sha256:".to_string());
    }
    if profile.timeout_limit_ms == 0 {
        return Err("sandbox profile timeout limit must be greater than zero".to_string());
    }
    if profile.max_artifact_bytes == 0 {
        return Err("sandbox profile max artifact bytes must be greater than zero".to_string());
    }
    Ok(())
}

fn run_psionic_command(command: PsionicCommand) -> Result<String, String> {
    match command {
        PsionicCommand::Attach(args) => attach_psionic_workers(args),
        PsionicCommand::ReceiptAppend(args) => append_psionic_execution_receipt_command(args),
    }
}

fn attach_psionic_workers(args: PsionicAttachArgs) -> Result<String, String> {
    let raw = fs::read_to_string(&args.file).map_err(|error| {
        format!(
            "failed to read psionic worker attachment {}: {error}",
            path_label(&args.file)
        )
    })?;
    let attachment = serde_json::from_str::<PsionicWorkerAttachment>(&raw).map_err(|error| {
        format!(
            "failed to parse psionic worker attachment {}: {error}",
            path_label(&args.file)
        )
    })?;
    attachment
        .validate_contract()
        .map_err(|error| format!("psionic worker attachment violates contract: {error}"))?;
    fs::create_dir_all(&args.state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(&args.state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(&attachment)
        .map_err(|error| format!("failed to serialize psionic worker attachment: {error}"))?;
    fs::write(
        psionic_workers_path(&args.state_dir),
        format!("{serialized}\n"),
    )
    .map_err(|error| format!("failed to write psionic worker attachment: {error}"))?;

    render_json_or_human(
        args.json,
        &attachment,
        format!(
            "psionic_worker_count: {}\nupdated_at_ms: {}",
            attachment.workers.len(),
            attachment.updated_at_ms
        ),
    )
}

fn append_psionic_execution_receipt_command(
    args: PsionicReceiptAppendArgs,
) -> Result<String, String> {
    let status = parse_psionic_execution_status(args.status.as_str())?;
    let receipt = build_psionic_execution_receipt(
        args.assignment_id.as_str(),
        args.product_id.as_str(),
        args.worker_id.as_str(),
        status,
        args.profile_digest.as_deref(),
        args.evidence_digest.as_str(),
    )?;
    append_psionic_execution_receipt(&args.state_dir, &receipt)?;
    render_json_or_human(
        args.json,
        &receipt,
        format!(
            "assignment_id: {}\nproduct_id: {}\nstatus: {}\nreceipt_digest: {}",
            receipt.assignment_id,
            receipt.product_id,
            psionic_execution_status_label(&receipt.status),
            receipt.receipt_digest
        ),
    )
}

fn build_psionic_execution_receipt(
    assignment_id: &str,
    product_id: &str,
    worker_id: &str,
    status: PsionicExecutionStatus,
    profile_digest: Option<&str>,
    evidence_digest: &str,
) -> Result<PsionicExecutionReceipt, String> {
    if !evidence_digest.starts_with("sha256:") {
        return Err("psionic evidence digest must start with sha256:".to_string());
    }
    if let Some(profile_digest) = profile_digest {
        if !profile_digest.starts_with("sha256:") {
            return Err("psionic profile digest must start with sha256:".to_string());
        }
    }
    if product_id.starts_with("sandbox.") && profile_digest.is_none() {
        return Err("sandbox psionic receipts require --profile-digest".to_string());
    }
    let emitted_at_ms = now_epoch_ms()?;
    let receipt_id = format!(
        "psionic.execution.{}.{}",
        sanitize_identifier(assignment_id),
        emitted_at_ms
    );
    let digest_material = serde_json::json!({
        "schema_version": PSIONIC_EXECUTION_RECEIPT_VERSION,
        "receipt_id": receipt_id,
        "assignment_id": assignment_id,
        "product_id": product_id,
        "worker_id": worker_id,
        "status": psionic_execution_status_label(&status),
        "profile_digest": profile_digest,
        "psionic_evidence_digest": evidence_digest,
        "emitted_at_ms": emitted_at_ms,
    });
    let encoded = serde_json::to_vec(&digest_material)
        .map_err(|error| format!("failed to serialize psionic receipt digest material: {error}"))?;
    let receipt = PsionicExecutionReceipt {
        schema_version: PSIONIC_EXECUTION_RECEIPT_VERSION.to_string(),
        receipt_id,
        assignment_id: assignment_id.to_string(),
        product_id: product_id.to_string(),
        worker_id: worker_id.to_string(),
        status,
        profile_digest: profile_digest.map(ToString::to_string),
        psionic_evidence_digest: evidence_digest.to_string(),
        receipt_digest: sha256_prefixed(encoded.as_slice()),
        emitted_at_ms,
    };
    receipt
        .validate_contract()
        .map_err(|error| format!("psionic execution receipt violates contract: {error}"))?;
    Ok(receipt)
}

fn append_psionic_execution_receipt(
    state_dir: &Path,
    receipt: &PsionicExecutionReceipt,
) -> Result<(), String> {
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize psionic execution receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(psionic_execution_receipts_path(state_dir))
        .map_err(|error| format!("failed to open psionic execution receipt log: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("failed to append psionic execution receipt: {error}"))?;

    let mut store = load_admin_store(state_dir)?.store;
    store.receipt_cursors.job_receipt_cursor = Some(receipt.receipt_digest.clone());
    store.updated_at_ms = receipt.emitted_at_ms;
    save_admin_store(state_dir, &store)
}

fn parse_psionic_execution_status(label: &str) -> Result<PsionicExecutionStatus, String> {
    match label {
        "succeeded" => Ok(PsionicExecutionStatus::Succeeded),
        "failed" => Ok(PsionicExecutionStatus::Failed),
        "refused" => Ok(PsionicExecutionStatus::Refused),
        other => Err(format!("unsupported psionic receipt status: {other}")),
    }
}

fn psionic_execution_status_label(status: &PsionicExecutionStatus) -> &'static str {
    match status {
        PsionicExecutionStatus::Succeeded => "succeeded",
        PsionicExecutionStatus::Failed => "failed",
        PsionicExecutionStatus::Refused => "refused",
    }
}

fn run_probe_command(command: ProbeCommand) -> Result<String, String> {
    match command {
        ProbeCommand::Attach(args) => attach_probe_worker(args),
        ProbeCommand::CloseoutAppend(args) => append_probe_closeout_command(args),
    }
}

fn attach_probe_worker(args: ProbeAttachArgs) -> Result<String, String> {
    let raw = fs::read_to_string(&args.file).map_err(|error| {
        format!(
            "failed to read probe worker attachment {}: {error}",
            path_label(&args.file)
        )
    })?;
    let attachment = serde_json::from_str::<ProbeWorkerAttachment>(&raw).map_err(|error| {
        format!(
            "failed to parse probe worker attachment {}: {error}",
            path_label(&args.file)
        )
    })?;
    attachment
        .validate_contract()
        .map_err(|error| format!("probe worker attachment violates contract: {error}"))?;
    fs::create_dir_all(&args.state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(&args.state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(&attachment)
        .map_err(|error| format!("failed to serialize probe worker attachment: {error}"))?;
    fs::write(
        probe_worker_path(&args.state_dir),
        format!("{serialized}\n"),
    )
    .map_err(|error| format!("failed to write probe worker attachment: {error}"))?;

    render_json_or_human(
        args.json,
        &attachment,
        format!(
            "workroom_id: {}\nworker_id: {}\ncapability_count: {}\nraw_secret_access: {}",
            attachment.workroom_id,
            attachment.worker_id,
            attachment.capability_names.len(),
            attachment.raw_secret_access
        ),
    )
}

fn append_probe_closeout_command(args: ProbeCloseoutAppendArgs) -> Result<String, String> {
    let status = parse_probe_closeout_status(args.status.as_str())?;
    let receipt = build_probe_closeout_receipt(
        args.workroom_id.as_str(),
        args.worker_id.as_str(),
        status,
        args.artifact_refs,
    )?;
    append_probe_closeout_receipt(&args.state_dir, &receipt)?;
    render_json_or_human(
        args.json,
        &receipt,
        format!(
            "workroom_id: {}\nworker_id: {}\nstatus: {}\nartifact_count: {}\nreceipt_digest: {}",
            receipt.workroom_id,
            receipt.worker_id,
            probe_closeout_status_label(&receipt.status),
            receipt.artifact_refs.len(),
            receipt.receipt_digest
        ),
    )
}

fn build_probe_closeout_receipt(
    workroom_id: &str,
    worker_id: &str,
    status: ProbeCloseoutStatus,
    artifact_refs: Vec<String>,
) -> Result<ProbeCloseoutReceipt, String> {
    let emitted_at_ms = now_epoch_ms()?;
    let receipt_id = format!(
        "probe.closeout.{}.{}",
        sanitize_identifier(workroom_id),
        emitted_at_ms
    );
    let digest_material = serde_json::json!({
        "schema_version": PROBE_CLOSEOUT_RECEIPT_VERSION,
        "receipt_id": receipt_id,
        "workroom_id": workroom_id,
        "worker_id": worker_id,
        "status": probe_closeout_status_label(&status),
        "artifact_refs": artifact_refs,
        "emitted_at_ms": emitted_at_ms,
    });
    let encoded = serde_json::to_vec(&digest_material)
        .map_err(|error| format!("failed to serialize probe closeout digest material: {error}"))?;
    let receipt = ProbeCloseoutReceipt {
        schema_version: PROBE_CLOSEOUT_RECEIPT_VERSION.to_string(),
        receipt_id,
        workroom_id: workroom_id.to_string(),
        worker_id: worker_id.to_string(),
        status,
        artifact_refs,
        receipt_digest: sha256_prefixed(encoded.as_slice()),
        emitted_at_ms,
    };
    receipt
        .validate_contract()
        .map_err(|error| format!("probe closeout receipt violates contract: {error}"))?;
    Ok(receipt)
}

fn append_probe_closeout_receipt(
    state_dir: &Path,
    receipt: &ProbeCloseoutReceipt,
) -> Result<(), String> {
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("failed to serialize probe closeout receipt: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(probe_closeout_receipts_path(state_dir))
        .map_err(|error| format!("failed to open probe closeout receipt log: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("failed to append probe closeout receipt: {error}"))?;

    let mut store = load_admin_store(state_dir)?.store;
    store.receipt_cursors.artifact_receipt_cursor = Some(receipt.receipt_digest.clone());
    store.updated_at_ms = receipt.emitted_at_ms;
    save_admin_store(state_dir, &store)
}

fn parse_probe_closeout_status(label: &str) -> Result<ProbeCloseoutStatus, String> {
    match label {
        "succeeded" => Ok(ProbeCloseoutStatus::Succeeded),
        "failed" => Ok(ProbeCloseoutStatus::Failed),
        "refused" => Ok(ProbeCloseoutStatus::Refused),
        other => Err(format!("unsupported probe closeout status: {other}")),
    }
}

fn probe_closeout_status_label(status: &ProbeCloseoutStatus) -> &'static str {
    match status {
        ProbeCloseoutStatus::Succeeded => "succeeded",
        ProbeCloseoutStatus::Failed => "failed",
        ProbeCloseoutStatus::Refused => "refused",
    }
}

fn snapshot_from_state(state: &LocalNodeState, admin: &AdminStoreLoad) -> CloudNodeSnapshot {
    let mut snapshot = CloudNodeSnapshot::managed_scaffold();
    let detection = detect_capabilities();
    snapshot.identity.node_id = state.identity.node_id.clone();
    snapshot.identity.operator_identity = state.identity.operator_identity.clone();
    snapshot.identity.account_or_org_binding = state.identity.account_or_org_binding.clone();
    snapshot.identity.signing_key_ref = state.identity.signing_key_ref.clone();
    snapshot.lifecycle.desired_mode =
        parse_desired_mode_label(admin.store.desired_mode.as_str()).unwrap_or(DesiredMode::Offline);
    snapshot.lifecycle.observed_status = observed_status_from_admin(admin);
    snapshot.lifecycle.degradation_reason = admin
        .degraded_reason
        .clone()
        .or_else(|| admin.store.last_degradation_reason.clone());
    snapshot.lifecycle.service_manager = state.service.service_manager.clone();
    snapshot.lifecycle.update_channel = state.service.update_channel.clone();
    snapshot.policy.settlement_policy =
        settlement_policy_from_mode(admin.store.settlement.mode.as_str())
            .unwrap_or(SettlementPolicy::NoWallet);
    if !admin.sandbox_profiles.is_empty() {
        snapshot.policy.sandbox_policy = "profile_enforced".to_string();
    }
    snapshot.host = detection.host;
    snapshot.capabilities.inference_products = detection
        .sellable_capabilities
        .iter()
        .filter(|capability| capability.capability_id.contains("inference"))
        .map(product_capability_from_detection)
        .collect();
    snapshot.capabilities.sandbox_profiles = admin
        .sandbox_profiles
        .iter()
        .map(|profile| SandboxProfileSummary {
            profile_id: profile.profile_id.clone(),
            profile_digest: profile.profile_digest.clone(),
            execution_class: profile.execution_class.clone(),
            ready: profile.ready,
        })
        .collect();
    snapshot.capabilities.ingress_support = CapabilityState {
        supported: false,
        enabled: false,
        ready: false,
        detail: Some("managed ingress detection not implemented".to_string()),
    };
    snapshot.capabilities.artifact_support = CapabilityState {
        supported: true,
        enabled: false,
        ready: false,
        detail: Some("artifact backend not configured".to_string()),
    };
    snapshot.evidence.current_snapshot_digest = snapshot_digest_from_state(state, &admin.store);
    snapshot.evidence.health_events = admin
        .health_events
        .iter()
        .map(|event| event.event_id.clone())
        .collect();
    snapshot.evidence.job_receipts = admin
        .forge_assignment_receipts
        .iter()
        .map(|receipt| receipt.receipt_digest.clone())
        .chain(
            admin
                .psionic_execution_receipts
                .iter()
                .map(|receipt| receipt.receipt_digest.clone()),
        )
        .collect();
    snapshot.evidence.artifact_receipts = admin
        .probe_closeout_receipts
        .iter()
        .map(|receipt| receipt.receipt_digest.clone())
        .collect();
    if let Some(receipt) = &admin.store.receipt_cursors.accounting_receipt_cursor {
        snapshot
            .evidence
            .payout_or_accounting_receipts
            .push(receipt.clone());
    }
    apply_psionic_attachment(&mut snapshot, admin.psionic_attachment.as_ref());
    snapshot
}

fn product_capability_from_detection(capability: &SellableCapabilityReport) -> ProductCapability {
    ProductCapability {
        product_id: capability.capability_id.clone(),
        enabled: capability.present_hardware,
        backend_ready: capability.backend_ready,
        eligible: capability.eligible,
        capability_summary: capability.reason.clone(),
    }
}

fn apply_psionic_attachment(
    snapshot: &mut CloudNodeSnapshot,
    attachment: Option<&PsionicWorkerAttachment>,
) {
    let Some(attachment) = attachment else {
        return;
    };
    snapshot
        .capabilities
        .inference_products
        .retain(|product| !product.product_id.starts_with("psionic."));
    snapshot
        .capabilities
        .training_products
        .retain(|product| !product.product_id.starts_with("psionic."));
    snapshot
        .capabilities
        .sandbox_profiles
        .retain(|profile| !profile.profile_id.starts_with("psionic."));

    for worker in &attachment.workers {
        let ready = worker.ready && !worker.crashed;
        let summary = if worker.crashed {
            format!(
                "worker_crashed: {}",
                worker.detail.as_deref().unwrap_or("no detail")
            )
        } else if ready {
            worker
                .detail
                .clone()
                .unwrap_or_else(|| "worker ready".to_string())
        } else {
            format!(
                "worker_not_ready: {}",
                worker.detail.as_deref().unwrap_or("no detail")
            )
        };
        match worker.worker_kind {
            PsionicWorkerKind::Inference => {
                snapshot
                    .capabilities
                    .inference_products
                    .push(ProductCapability {
                        product_id: worker.product_id.clone(),
                        enabled: true,
                        backend_ready: ready,
                        eligible: ready,
                        capability_summary: summary,
                    });
            }
            PsionicWorkerKind::Training => {
                snapshot
                    .capabilities
                    .training_products
                    .push(ProductCapability {
                        product_id: worker.product_id.clone(),
                        enabled: true,
                        backend_ready: ready,
                        eligible: ready,
                        capability_summary: summary,
                    });
            }
            PsionicWorkerKind::Sandbox => {
                snapshot
                    .capabilities
                    .sandbox_profiles
                    .push(SandboxProfileSummary {
                        profile_id: worker.product_id.clone(),
                        profile_digest: worker
                            .evidence_digest
                            .clone()
                            .unwrap_or_else(|| "sha256:psionic-sandbox-no-evidence".to_string()),
                        execution_class: "psionic.sandbox.exec".to_string(),
                        ready,
                    });
            }
        }
    }
}

fn detect_capabilities() -> CapabilityDetectionReport {
    let mut system = System::new_all();
    system.refresh_all();
    let disks = Disks::new_with_refreshed_list();

    let cpu_brand = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|brand| !brand.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let logical_cpu_count = system.cpus().len() as u64;
    let physical_cpu_count = System::physical_core_count().map(|count| count as u64);
    let memory_total_bytes = system.total_memory();
    let disk_total_bytes = disks.iter().map(|disk| disk.total_space()).sum::<u64>();
    let disk_available_bytes = disks.iter().map(|disk| disk.available_space()).sum::<u64>();
    let accelerators = detect_accelerators();

    let host = HostFacts {
        os: System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string()),
        arch: System::cpu_arch(),
        cpu: format!("{logical_cpu_count} logical cpu(s): {cpu_brand}"),
        memory: format!("{memory_total_bytes} bytes"),
        disk: format!("{disk_available_bytes}/{disk_total_bytes} bytes available"),
        accelerator_inventory: accelerators.clone(),
        site_or_power_metadata: System::host_name().map(|host| format!("host={host}")),
    };

    let present_hardware = PresentHardwareReport {
        os: host.os.clone(),
        arch: host.arch.clone(),
        cpu_brand,
        logical_cpu_count,
        physical_cpu_count,
        memory_total_bytes,
        disk_total_bytes,
        disk_available_bytes,
        accelerators,
    };

    let mut sellable_capabilities = Vec::new();
    let mut degraded_backends = Vec::new();
    let psionic_endpoint = env::var(ENV_PSIONIC_ENDPOINT)
        .ok()
        .filter(|value| !value.trim().is_empty());
    if psionic_endpoint.is_some() {
        sellable_capabilities.push(SellableCapabilityReport {
            capability_id: "psionic.managed.inference".to_string(),
            present_hardware: logical_cpu_count > 0,
            backend_ready: false,
            eligible: false,
            reason: "psionic endpoint configured but readiness probe not implemented".to_string(),
        });
        degraded_backends.push(DetectedBackendFailure {
            backend: "psionic".to_string(),
            reason: "readiness_probe_not_implemented".to_string(),
        });
    } else {
        sellable_capabilities.push(SellableCapabilityReport {
            capability_id: "psionic.managed.inference".to_string(),
            present_hardware: logical_cpu_count > 0,
            backend_ready: false,
            eligible: false,
            reason: "backend_not_configured".to_string(),
        });
        degraded_backends.push(DetectedBackendFailure {
            backend: "psionic".to_string(),
            reason: "backend_not_configured".to_string(),
        });
    }

    sellable_capabilities.push(SellableCapabilityReport {
        capability_id: "sandbox.posix.exec".to_string(),
        present_hardware: cfg!(unix),
        backend_ready: false,
        eligible: false,
        reason: "sandbox_profile_not_configured".to_string(),
    });

    CapabilityDetectionReport {
        schema_version: CAPABILITY_DETECTION_SCHEMA_VERSION.to_string(),
        host,
        present_hardware,
        sellable_capabilities,
        degraded_backends,
    }
}

fn detect_accelerators() -> Vec<String> {
    let mut accelerators = Vec::new();
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        accelerators.push("apple_silicon_unified_gpu".to_string());
    }
    accelerators
}

fn run_admin_command(command: AdminCommand) -> Result<String, String> {
    match command {
        AdminCommand::DesiredModeGet(args) => {
            let admin = load_admin_store(&args.state_dir)?;
            let output = DesiredModeOutput {
                schema_version: admin.store.schema_version,
                desired_mode: admin.store.desired_mode,
                updated_at_ms: admin.store.updated_at_ms,
            };
            render_json_or_human(
                args.json,
                &output,
                format!(
                    "desired_mode: {}\nupdated_at_ms: {}",
                    output.desired_mode, output.updated_at_ms
                ),
            )
        }
        AdminCommand::DesiredModeSet(args) => set_desired_mode(args),
        AdminCommand::HealthAppend(args) => append_health_event(args),
        AdminCommand::HealthList(args) => {
            let admin = load_admin_store(&args.state_dir)?;
            let output = HealthEventsOutput {
                schema_version: ADMIN_STORE_SCHEMA_VERSION.to_string(),
                events: admin.health_events,
            };
            render_json_or_human(
                args.json,
                &output,
                format!("health_event_count: {}", output.events.len()),
            )
        }
    }
}

fn set_desired_mode(args: DesiredModeSetArgs) -> Result<String, String> {
    fs::create_dir_all(&args.state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(&args.state_dir)
        )
    })?;
    let mut store = load_admin_store(&args.state_dir)?.store;
    store.desired_mode = args.desired_mode;
    store.updated_at_ms = now_epoch_ms()?;
    save_admin_store(&args.state_dir, &store)?;
    let output = DesiredModeOutput {
        schema_version: store.schema_version,
        desired_mode: store.desired_mode,
        updated_at_ms: store.updated_at_ms,
    };
    render_json_or_human(
        args.json,
        &output,
        format!(
            "desired_mode: {}\nupdated_at_ms: {}",
            output.desired_mode, output.updated_at_ms
        ),
    )
}

fn append_health_event(args: HealthAppendArgs) -> Result<String, String> {
    let event = append_health_event_record(
        &args.state_dir,
        args.severity.as_str(),
        args.code.as_str(),
        args.detail.as_str(),
    )?;
    render_json_or_human(
        args.json,
        &event,
        format!(
            "event_id: {}\nseverity: {}\ncode: {}",
            event.event_id, event.severity, event.code
        ),
    )
}

fn append_health_event_record(
    state_dir: &Path,
    severity: &str,
    code: &str,
    detail: &str,
) -> Result<AdminHealthEvent, String> {
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let mut store = load_admin_store(state_dir)?.store;
    let occurred_at_ms = now_epoch_ms()?;
    let event = AdminHealthEvent {
        event_id: format!("health.{}.{}", sanitize_identifier(code), occurred_at_ms),
        occurred_at_ms,
        severity: severity.to_string(),
        code: code.to_string(),
        detail: detail.to_string(),
    };
    let line = serde_json::to_string(&event)
        .map_err(|error| format!("failed to serialize health event: {error}"))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(health_events_path(state_dir))
        .map_err(|error| format!("failed to open health event log: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to append health event: {error}"))?;

    store.receipt_cursors.health_event_count =
        store.receipt_cursors.health_event_count.saturating_add(1);
    store.updated_at_ms = occurred_at_ms;
    save_admin_store(state_dir, &store)?;
    Ok(event)
}

fn render_json_or_human<T: Serialize>(
    json: bool,
    value: &T,
    human: String,
) -> Result<String, String> {
    if json {
        serde_json::to_string_pretty(value)
            .map_err(|error| format!("failed to serialize admin output: {error}"))
    } else {
        Ok(human)
    }
}

fn ensure_admin_store(state_dir: &Path) -> Result<(), String> {
    let path = admin_store_path(state_dir);
    if path.exists() {
        return Ok(());
    }
    save_admin_store(state_dir, &default_admin_store(now_epoch_ms()?))
}

fn load_admin_store(state_dir: &Path) -> Result<AdminStoreLoad, String> {
    let path = admin_store_path(state_dir);
    let mut degraded_reason = None;
    let mut store = if path.exists() {
        match load_admin_store_file(&path) {
            Ok(store) => store,
            Err(error) => {
                degraded_reason = Some(format!("admin_store_corrupt: {error}"));
                default_admin_store(now_epoch_ms()?)
            }
        }
    } else {
        degraded_reason = Some("admin_store_missing".to_string());
        default_admin_store(now_epoch_ms()?)
    };

    if store.schema_version != ADMIN_STORE_SCHEMA_VERSION {
        degraded_reason = Some(format!(
            "admin_store_schema_unsupported: {}",
            store.schema_version
        ));
        store = default_admin_store(now_epoch_ms()?);
    }

    let health_events = match load_health_events(state_dir) {
        Ok(events) => events,
        Err(error) => {
            degraded_reason = Some(format!("health_events_corrupt: {error}"));
            Vec::new()
        }
    };
    let forge_assignment_receipts = match load_forge_assignment_receipts(state_dir) {
        Ok(receipts) => receipts,
        Err(error) => {
            degraded_reason = Some(format!("forge_assignment_receipts_corrupt: {error}"));
            Vec::new()
        }
    };
    let sandbox_profiles = match load_sandbox_profiles(state_dir) {
        Ok(profiles) => profiles,
        Err(error) => {
            degraded_reason = Some(format!("sandbox_profiles_corrupt: {error}"));
            Vec::new()
        }
    };
    let psionic_attachment = match load_psionic_worker_attachment(state_dir) {
        Ok(attachment) => attachment,
        Err(error) => {
            degraded_reason = Some(format!("psionic_worker_attachment_corrupt: {error}"));
            None
        }
    };
    let psionic_execution_receipts = match load_psionic_execution_receipts(state_dir) {
        Ok(receipts) => receipts,
        Err(error) => {
            degraded_reason = Some(format!("psionic_execution_receipts_corrupt: {error}"));
            Vec::new()
        }
    };
    let probe_attachment = match load_probe_worker_attachment(state_dir) {
        Ok(attachment) => attachment,
        Err(error) => {
            degraded_reason = Some(format!("probe_worker_attachment_corrupt: {error}"));
            None
        }
    };
    let probe_closeout_receipts = match load_probe_closeout_receipts(state_dir) {
        Ok(receipts) => receipts,
        Err(error) => {
            degraded_reason = Some(format!("probe_closeout_receipts_corrupt: {error}"));
            Vec::new()
        }
    };

    Ok(AdminStoreLoad {
        store,
        health_events,
        forge_assignment_receipts,
        sandbox_profiles,
        psionic_attachment,
        psionic_execution_receipts,
        probe_attachment,
        probe_closeout_receipts,
        degraded_reason,
    })
}

fn load_admin_store_file(path: &Path) -> Result<AdminStore, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read admin store {}: {error}", path_label(path)))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("failed to parse admin store {}: {error}", path_label(path)))
}

fn save_admin_store(state_dir: &Path, store: &AdminStore) -> Result<(), String> {
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(store)
        .map_err(|error| format!("failed to serialize admin store: {error}"))?;
    fs::write(admin_store_path(state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write admin store: {error}"))
}

fn default_admin_store(now_ms: u128) -> AdminStore {
    AdminStore {
        schema_version: ADMIN_STORE_SCHEMA_VERSION.to_string(),
        desired_mode: "offline".to_string(),
        observed_status: "offline".to_string(),
        inventory: AdminInventory {
            last_detected_at_ms: None,
            items: Vec::new(),
        },
        updates: AdminUpdates {
            channel: "local-dev".to_string(),
            current_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            pending_update: None,
            last_checked_at_ms: None,
            pinned_version: None,
            deferred: false,
        },
        quarantine: AdminQuarantine {
            quarantined: false,
            reason: None,
            since_ms: None,
            workroom_policy: None,
            affected_workrooms: Vec::new(),
        },
        settlement: default_admin_settlement(),
        receipt_cursors: AdminReceiptCursors {
            health_event_count: 0,
            job_receipt_cursor: None,
            artifact_receipt_cursor: None,
            accounting_receipt_cursor: None,
        },
        last_degradation_reason: None,
        updated_at_ms: now_ms,
    }
}

fn default_admin_settlement() -> AdminSettlement {
    AdminSettlement {
        mode: "no-wallet".to_string(),
        treasury_ref: None,
        nexus_ref: None,
    }
}

fn load_health_events(state_dir: &Path) -> Result<Vec<AdminHealthEvent>, String> {
    let path = health_events_path(state_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read health event log {}: {error}",
            path_label(&path)
        )
    })?;
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<AdminHealthEvent>(line)
                .map_err(|error| format!("failed to parse health event: {error}"))
        })
        .collect()
}

fn load_forge_assignment_receipts(state_dir: &Path) -> Result<Vec<ForgeAssignmentReceipt>, String> {
    let path = forge_assignment_receipts_path(state_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read forge assignment receipt log {}: {error}",
            path_label(&path)
        )
    })?;
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let receipt = serde_json::from_str::<ForgeAssignmentReceipt>(line)
                .map_err(|error| format!("failed to parse forge assignment receipt: {error}"))?;
            receipt
                .validate_contract()
                .map_err(|error| format!("forge assignment receipt violates contract: {error}"))?;
            Ok(receipt)
        })
        .collect()
}

fn load_sandbox_profiles(state_dir: &Path) -> Result<Vec<SandboxProfilePolicy>, String> {
    let path = sandbox_profiles_path(state_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read sandbox profile policy file {}: {error}",
            path_label(&path)
        )
    })?;
    let profiles = serde_json::from_str::<Vec<SandboxProfilePolicy>>(&raw).map_err(|error| {
        format!(
            "failed to parse sandbox profile policy file {}: {error}",
            path_label(&path)
        )
    })?;
    for profile in &profiles {
        validate_sandbox_profile_policy(profile)?;
    }
    Ok(profiles)
}

fn save_sandbox_profiles(
    state_dir: &Path,
    profiles: &[SandboxProfilePolicy],
) -> Result<(), String> {
    for profile in profiles {
        validate_sandbox_profile_policy(profile)?;
    }
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create state dir {}: {error}",
            path_label(state_dir)
        )
    })?;
    let serialized = serde_json::to_string_pretty(profiles)
        .map_err(|error| format!("failed to serialize sandbox profiles: {error}"))?;
    fs::write(sandbox_profiles_path(state_dir), format!("{serialized}\n"))
        .map_err(|error| format!("failed to write sandbox profiles: {error}"))
}

fn load_psionic_worker_attachment(
    state_dir: &Path,
) -> Result<Option<PsionicWorkerAttachment>, String> {
    let path = psionic_workers_path(state_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read psionic worker attachment {}: {error}",
            path_label(&path)
        )
    })?;
    let attachment = serde_json::from_str::<PsionicWorkerAttachment>(&raw).map_err(|error| {
        format!(
            "failed to parse psionic worker attachment {}: {error}",
            path_label(&path)
        )
    })?;
    attachment
        .validate_contract()
        .map_err(|error| format!("psionic worker attachment violates contract: {error}"))?;
    Ok(Some(attachment))
}

fn load_psionic_execution_receipts(
    state_dir: &Path,
) -> Result<Vec<PsionicExecutionReceipt>, String> {
    let path = psionic_execution_receipts_path(state_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read psionic execution receipt log {}: {error}",
            path_label(&path)
        )
    })?;
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let receipt = serde_json::from_str::<PsionicExecutionReceipt>(line)
                .map_err(|error| format!("failed to parse psionic execution receipt: {error}"))?;
            receipt
                .validate_contract()
                .map_err(|error| format!("psionic execution receipt violates contract: {error}"))?;
            Ok(receipt)
        })
        .collect()
}

fn load_probe_worker_attachment(state_dir: &Path) -> Result<Option<ProbeWorkerAttachment>, String> {
    let path = probe_worker_path(state_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read probe worker attachment {}: {error}",
            path_label(&path)
        )
    })?;
    let attachment = serde_json::from_str::<ProbeWorkerAttachment>(&raw).map_err(|error| {
        format!(
            "failed to parse probe worker attachment {}: {error}",
            path_label(&path)
        )
    })?;
    attachment
        .validate_contract()
        .map_err(|error| format!("probe worker attachment violates contract: {error}"))?;
    Ok(Some(attachment))
}

fn load_probe_closeout_receipts(state_dir: &Path) -> Result<Vec<ProbeCloseoutReceipt>, String> {
    let path = probe_closeout_receipts_path(state_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read probe closeout receipt log {}: {error}",
            path_label(&path)
        )
    })?;
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let receipt = serde_json::from_str::<ProbeCloseoutReceipt>(line)
                .map_err(|error| format!("failed to parse probe closeout receipt: {error}"))?;
            receipt
                .validate_contract()
                .map_err(|error| format!("probe closeout receipt violates contract: {error}"))?;
            Ok(receipt)
        })
        .collect()
}

fn observed_status_from_admin(admin: &AdminStoreLoad) -> ObservedStatus {
    if admin.degraded_reason.is_some() {
        return ObservedStatus::Degraded;
    }
    if admin.store.quarantine.quarantined {
        return ObservedStatus::Quarantined;
    }
    match admin.store.observed_status.as_str() {
        "online" => ObservedStatus::Online,
        "degraded" => ObservedStatus::Degraded,
        "quarantined" => ObservedStatus::Quarantined,
        "offline" => ObservedStatus::Offline,
        _ => ObservedStatus::Degraded,
    }
}

fn observed_status_label(status: &ObservedStatus) -> &'static str {
    match status {
        ObservedStatus::Unconfigured => "unconfigured",
        ObservedStatus::Offline => "offline",
        ObservedStatus::Online => "online",
        ObservedStatus::Degraded => "degraded",
        ObservedStatus::Quarantined => "quarantined",
    }
}

fn parse_desired_mode_label(label: &str) -> Result<DesiredMode, String> {
    match label {
        "offline" => Ok(DesiredMode::Offline),
        "online" => Ok(DesiredMode::Online),
        "paused" => Ok(DesiredMode::Paused),
        "quarantined" => Ok(DesiredMode::Quarantined),
        other => Err(format!("unsupported desired mode: {other}")),
    }
}

fn desired_mode_label(mode: &DesiredMode) -> &'static str {
    match mode {
        DesiredMode::Offline => "offline",
        DesiredMode::Online => "online",
        DesiredMode::Paused => "paused",
        DesiredMode::Quarantined => "quarantined",
    }
}

fn settlement_policy_from_mode(mode: &str) -> Result<SettlementPolicy, String> {
    match mode {
        "no-wallet" => Ok(SettlementPolicy::NoWallet),
        "internal-accounting" => Ok(SettlementPolicy::InternalAccounting),
        "contributor-wallet" => Ok(SettlementPolicy::ContributorWallet),
        other => Err(format!("unsupported settlement mode: {other}")),
    }
}

fn snapshot_digest_from_state(state: &LocalNodeState, store: &AdminStore) -> String {
    let digest_material = serde_json::json!({
        "schema_version": NODE_STATE_SCHEMA_VERSION,
        "admin_schema_version": ADMIN_STORE_SCHEMA_VERSION,
        "node_id": state.identity.node_id,
        "org_id": state.identity.org_id,
        "operator_identity": state.identity.operator_identity,
        "account_or_org_binding": state.identity.account_or_org_binding,
        "signing_key_ref": state.identity.signing_key_ref,
        "desired_mode": store.desired_mode,
        "observed_status": store.observed_status,
        "update_channel": store.updates.channel,
        "current_version": store.updates.current_version,
        "pending_update": store.updates.pending_update,
        "pinned_version": store.updates.pinned_version,
        "deferred": store.updates.deferred,
        "settlement_mode": store.settlement.mode,
        "accounting_receipt_cursor": store.receipt_cursors.accounting_receipt_cursor,
        "quarantined": store.quarantine.quarantined,
        "health_event_count": store.receipt_cursors.health_event_count,
        "state_updated_at_ms": state.updated_at_ms,
        "admin_updated_at_ms": store.updated_at_ms,
    });
    let encoded = serde_json::to_vec(&digest_material)
        .unwrap_or_else(|_| b"openagents-cloud-node-unserializable-digest-material".to_vec());
    sha256_prefixed(encoded.as_slice())
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn load_state(path: &Path) -> Result<LocalNodeState, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read oa-node state {}: {error}", path_label(path)))?;
    serde_json::from_str(&raw).map_err(|error| {
        format!(
            "failed to parse oa-node state {}: {error}",
            path_label(path)
        )
    })
}

fn save_state(path: &Path, state: &LocalNodeState) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize oa-node state: {error}"))?;
    fs::write(path, format!("{serialized}\n")).map_err(|error| {
        format!(
            "failed to write oa-node state {}: {error}",
            path_label(path)
        )
    })
}

fn state_file(state_dir: &Path) -> PathBuf {
    state_dir.join("node-state.json")
}

fn admin_store_path(state_dir: &Path) -> PathBuf {
    state_dir.join(ADMIN_STORE_FILE)
}

fn health_events_path(state_dir: &Path) -> PathBuf {
    state_dir.join(HEALTH_EVENTS_FILE)
}

fn forge_assignment_receipts_path(state_dir: &Path) -> PathBuf {
    state_dir.join(FORGE_ASSIGNMENT_RECEIPTS_FILE)
}

fn sandbox_profiles_path(state_dir: &Path) -> PathBuf {
    state_dir.join(SANDBOX_PROFILE_POLICIES_FILE)
}

fn psionic_workers_path(state_dir: &Path) -> PathBuf {
    state_dir.join(PSIONIC_WORKERS_FILE)
}

fn psionic_execution_receipts_path(state_dir: &Path) -> PathBuf {
    state_dir.join(PSIONIC_EXECUTION_RECEIPTS_FILE)
}

fn probe_worker_path(state_dir: &Path) -> PathBuf {
    state_dir.join(PROBE_WORKER_FILE)
}

fn probe_closeout_receipts_path(state_dir: &Path) -> PathBuf {
    state_dir.join(PROBE_CLOSEOUT_RECEIPTS_FILE)
}

fn service_events_path(state_dir: &Path) -> PathBuf {
    state_dir.join(SERVICE_EVENTS_FILE)
}

fn update_receipts_path(state_dir: &Path) -> PathBuf {
    state_dir.join(UPDATE_RECEIPTS_FILE)
}

fn quarantine_receipts_path(state_dir: &Path) -> PathBuf {
    state_dir.join(QUARANTINE_RECEIPTS_FILE)
}

fn settlement_receipts_path(state_dir: &Path) -> PathBuf {
    state_dir.join(SETTLEMENT_RECEIPTS_FILE)
}

fn broker_redaction_receipts_path(state_dir: &Path) -> PathBuf {
    state_dir.join(BROKER_REDACTION_RECEIPTS_FILE)
}

fn generated_node_id(org_id: &str, now_ms: u128) -> String {
    format!("oa-node.{}.{}", sanitize_identifier(org_id), now_ms)
}

fn default_signing_key_ref(node_id: &str) -> String {
    format!("local-keychain://openagents/cloud/oa-node/{node_id}")
}

fn sanitize_identifier(raw: &str) -> String {
    let sanitized = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "org".to_string()
    } else {
        sanitized
    }
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
