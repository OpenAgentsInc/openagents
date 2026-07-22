//! Fail-closed Google Cloud runtime for `openagents.managed_sandbox.v1`.
//!
//! This module composes the existing GCE control seam into the native managed-
//! sandbox lifecycle. It is deliberately separate from the legacy placement
//! lane: a managed-sandbox request never continues on the control host and
//! never promotes the deterministic fake provisioner to `ready`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::managed_sandbox_guest_io::{GuestIoAction, GuestIoLimits, ManagedSandboxGuestIoRequest};
use crate::managed_sandbox_phase2::{ManagedSandboxPhase2Action, ManagedSandboxPhase2Request};

const SCHEMA_VERSION: &str = "openagents.managed_sandbox_runtime.v1";
const MAX_OPERATION_RECORDS: usize = 128;
const MAX_CAPABILITY_REFS: usize = 32;
const MAX_TTL_MS: u64 = 24 * 60 * 60 * 1_000;
const GCE_METADATA_SERVER_CIDR: &str = "169.254.169.254/32";
const GUEST_IO_EXECUTABLE: &str = "/opt/openagents-managed-sandbox/managed-sandbox-guest-io.py";
const LIVE_ACTIVE_SANDBOX_FILTER: &str =
    "labels.openagents-managed=managed-sandbox AND status!=TERMINATED";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePhase {
    Provisioning,
    Ready,
    Stopping,
    Stopped,
    Resuming,
    Failed,
    RecoveryRequired,
    Deleting,
    Deleted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeAction {
    Create,
    Probe,
    Stop,
    Resume,
    Delete,
    Reconcile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityPolicy {
    pub min_capacity: u32,
    pub max_capacity: u32,
    pub prewarm_capacity: u32,
    pub concurrent_capacity_cap: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetPolicy {
    pub sandbox_budget_microusd: u64,
    pub program_budget_microusd: u64,
    pub max_hourly_cost_microusd: u64,
}

/// Exact public-safe profile admitted before the first provider effect.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSandboxRuntimeProfile {
    pub profile_ref: String,
    pub profile_digest: String,
    pub target_ref: String,
    pub provisioner_ref: String,
    pub region: String,
    pub machine_class: String,
    pub isolation_class: String,
    pub image_ref: String,
    pub image_digest: String,
    pub network_policy_ref: String,
    pub control_identity_ref: String,
    pub guest_identity_ref: String,
    pub ttl_ms: u64,
    pub capacity: CapacityPolicy,
    pub budget: BudgetPolicy,
    pub capability_refs: Vec<String>,
}

impl ManagedSandboxRuntimeProfile {
    fn validate(&self) -> Result<(), RuntimeError> {
        for (field, value) in [
            ("profileRef", self.profile_ref.as_str()),
            ("profileDigest", self.profile_digest.as_str()),
            ("targetRef", self.target_ref.as_str()),
            ("provisionerRef", self.provisioner_ref.as_str()),
            ("region", self.region.as_str()),
            ("machineClass", self.machine_class.as_str()),
            ("isolationClass", self.isolation_class.as_str()),
            ("imageRef", self.image_ref.as_str()),
            ("imageDigest", self.image_digest.as_str()),
            ("networkPolicyRef", self.network_policy_ref.as_str()),
            ("controlIdentityRef", self.control_identity_ref.as_str()),
            ("guestIdentityRef", self.guest_identity_ref.as_str()),
        ] {
            validate_public_ref(field, value)?;
        }
        if self.target_ref != "target://openagents/google-cloud/managed-sandbox" {
            return Err(RuntimeError::validation("target_not_admitted"));
        }
        if self.isolation_class != "gce_vm" {
            return Err(RuntimeError::validation("isolation_class_not_admitted"));
        }
        if !valid_sha256_ref(&self.image_digest) || !valid_sha256_ref(&self.profile_digest) {
            return Err(RuntimeError::validation(
                "image_and_profile_digests_must_be_sha256",
            ));
        }
        if self.ttl_ms == 0 || self.ttl_ms > MAX_TTL_MS {
            return Err(RuntimeError::validation("ttl_out_of_bounds"));
        }
        if self.capacity.min_capacity != 0 || self.capacity.prewarm_capacity != 0 {
            return Err(RuntimeError::validation(
                "phase_1_min_and_prewarm_capacity_must_be_zero",
            ));
        }
        if self.capacity.max_capacity == 0
            || self.capacity.concurrent_capacity_cap == 0
            || self.capacity.concurrent_capacity_cap > self.capacity.max_capacity
        {
            return Err(RuntimeError::validation("capacity_policy_invalid"));
        }
        if self.budget.sandbox_budget_microusd == 0
            || self.budget.program_budget_microusd == 0
            || self.budget.max_hourly_cost_microusd == 0
            || self.budget.sandbox_budget_microusd > self.budget.program_budget_microusd
        {
            return Err(RuntimeError::validation("budget_policy_invalid"));
        }
        let worst_case = cost_microusd(self.ttl_ms, self.budget.max_hourly_cost_microusd);
        if worst_case > self.budget.sandbox_budget_microusd
            || worst_case > self.budget.program_budget_microusd
        {
            return Err(RuntimeError::new(
                409,
                "budget_refused",
                "declared TTL exceeds the admitted sandbox or program budget",
            ));
        }
        if self.capability_refs.is_empty() || self.capability_refs.len() > MAX_CAPABILITY_REFS {
            return Err(RuntimeError::validation(
                "capability_ref_count_out_of_bounds",
            ));
        }
        for capability_ref in &self.capability_refs {
            validate_public_ref("capabilityRef", capability_ref)?;
            if !capability_ref.starts_with("capability-ref://run/") {
                return Err(RuntimeError::validation(
                    "capabilities_must_be_run_scoped_refs",
                ));
            }
        }
        if self.guest_identity_ref != "identity-ref://openagents/managed-sandbox/guest-none" {
            return Err(RuntimeError::validation(
                "phase_1_guest_identity_must_be_none",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSandboxRuntimeRequest {
    pub operation_ref: String,
    pub idempotency_ref: String,
    pub actor_ref: String,
    pub owner_ref: String,
    pub tenant_ref: String,
    pub program_ref: String,
    pub work_unit_ref: String,
    pub sandbox_ref: String,
    pub expected_generation: u64,
    pub action: RuntimeAction,
    #[serde(default)]
    pub profile: Option<ManagedSandboxRuntimeProfile>,
}

impl ManagedSandboxRuntimeRequest {
    fn validate(&self) -> Result<(), RuntimeError> {
        for (field, value) in [
            ("operationRef", self.operation_ref.as_str()),
            ("idempotencyRef", self.idempotency_ref.as_str()),
            ("actorRef", self.actor_ref.as_str()),
            ("ownerRef", self.owner_ref.as_str()),
            ("tenantRef", self.tenant_ref.as_str()),
            ("programRef", self.program_ref.as_str()),
            ("workUnitRef", self.work_unit_ref.as_str()),
            ("sandboxRef", self.sandbox_ref.as_str()),
        ] {
            validate_public_ref(field, value)?;
        }
        match self.action {
            RuntimeAction::Create => {
                if self.expected_generation != 0 {
                    return Err(RuntimeError::new(
                        409,
                        "generation_conflict",
                        "create requires expected generation zero",
                    ));
                }
                self.profile
                    .as_ref()
                    .ok_or_else(|| RuntimeError::validation("create_requires_profile"))?
                    .validate()?;
            }
            _ if self.profile.is_some() => {
                return Err(RuntimeError::validation(
                    "profile_is_only_admitted_on_create",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderOwnership {
    resource_name: String,
    firewall_name: String,
    #[serde(default)]
    broker_egress_firewall_name: String,
    #[serde(default)]
    metadata_egress_firewall_name: String,
    #[serde(default)]
    control_ingress_firewall_name: String,
    #[serde(default)]
    ingress_deny_firewall_name: String,
    disk_name: String,
    resource_ref: String,
    firewall_ref: String,
    disk_ref: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadinessObservation {
    provider_running: bool,
    guest_marker_observed: bool,
    image_admitted: bool,
    no_external_ip: bool,
    no_guest_service_account: bool,
    egress_default_deny: bool,
    broker_egress_only: bool,
    #[serde(default)]
    metadata_egress_only: bool,
    control_ingress_only: bool,
    metadata_restricted: bool,
}

impl ReadinessObservation {
    fn is_ready(&self) -> bool {
        self.provider_running
            && self.guest_marker_observed
            && self.image_admitted
            && self.no_external_ip
            && self.no_guest_service_account
            && self.egress_default_deny
            && self.broker_egress_only
            && self.metadata_egress_only
            && self.control_ingress_only
            && self.metadata_restricted
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanupObservation {
    zero_compute: bool,
    zero_firewall: bool,
    zero_scratch: bool,
    zero_ingress: bool,
    zero_grants: bool,
}

impl CleanupObservation {
    fn is_clean(&self) -> bool {
        self.zero_compute
            && self.zero_firewall
            && self.zero_scratch
            && self.zero_ingress
            && self.zero_grants
    }
}

trait ManagedSandboxProvider {
    fn kind(&self) -> &'static str;
    fn admit(&self, profile: &ManagedSandboxRuntimeProfile) -> Result<(), RuntimeError>;
    fn plan(&self, sandbox_ref: &str) -> ProviderOwnership;
    fn count_active(&self) -> Result<u32, RuntimeError>;
    fn create(
        &self,
        ownership: &ProviderOwnership,
        profile: &ManagedSandboxRuntimeProfile,
        generation: u64,
    ) -> Result<ReadinessObservation, RuntimeError>;
    fn probe(
        &self,
        ownership: &ProviderOwnership,
        generation: u64,
    ) -> Result<ReadinessObservation, RuntimeError>;
    fn stop(&self, ownership: &ProviderOwnership) -> Result<bool, RuntimeError>;
    fn resume(
        &self,
        ownership: &ProviderOwnership,
        generation: u64,
    ) -> Result<ReadinessObservation, RuntimeError>;
    fn cleanup(&self, ownership: &ProviderOwnership) -> Result<CleanupObservation, RuntimeError>;
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationRecord {
    operation_ref: String,
    idempotency_ref: String,
    fingerprint: String,
    response: ManagedSandboxRuntimeReceipt,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingCheckpointRestore {
    command_ref: String,
    checkpoint_ref: String,
    checkpoint_source_generation: u64,
    restored_resource_generation: u64,
    runtime_capability_refs: Vec<String>,
    restored_capability_refs: Vec<String>,
    #[serde(default)]
    completed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingCheckpointFork {
    command_ref: String,
    checkpoint_ref: String,
    source_sandbox_ref: String,
    source_resource_generation: u64,
    fork_capability_refs: Vec<String>,
    cleanup_obligation_ref: String,
    #[serde(default)]
    completed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivateIngressRecord {
    capability_ref: String,
    create_command_ref: String,
    create_idempotency_ref: String,
    create_fingerprint: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
    active_response: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_command_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_idempotency_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_response: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeJournal {
    schema_version: String,
    owner_ref: String,
    tenant_ref: String,
    program_ref: String,
    work_unit_ref: String,
    sandbox_ref: String,
    generation: u64,
    phase: RuntimePhase,
    profile: ManagedSandboxRuntimeProfile,
    ownership: ProviderOwnership,
    provider_kind: String,
    cleanup_owner_ref: String,
    created_at_ms: u64,
    running_started_at_ms: Option<u64>,
    accrued_running_ms: u64,
    readiness: ReadinessObservation,
    cleanup: CleanupObservation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending_checkpoint_restore: Option<PendingCheckpointRestore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending_checkpoint_fork: Option<PendingCheckpointFork>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    private_ingress: Vec<PrivateIngressRecord>,
    operations: Vec<OperationRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSandboxRuntimeReceipt {
    pub schema_version: String,
    pub receipt_ref: String,
    pub operation_ref: String,
    pub action: RuntimeAction,
    pub sandbox_ref: String,
    pub generation: u64,
    pub phase: RuntimePhase,
    pub target_ref: String,
    pub profile_ref: String,
    pub profile_digest: String,
    pub image_ref: String,
    pub image_digest: String,
    pub isolation_class: String,
    pub network_policy_ref: String,
    pub control_identity_ref: String,
    pub guest_identity_ref: String,
    pub resource_ref: String,
    pub firewall_ref: String,
    pub disk_ref: String,
    pub provider_kind: String,
    pub readiness_observed: bool,
    pub cleanup_observed: bool,
    pub measured_running_ms: u64,
    pub measured_cost_microusd: u64,
    pub sandbox_budget_microusd: u64,
    pub program_budget_microusd: u64,
    pub emitted_at_ms: u64,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RuntimeError {
    status: u16,
    code: &'static str,
    message: String,
}

impl RuntimeError {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn validation(code: &'static str) -> Self {
        Self::new(400, code, "managed-sandbox runtime request was refused")
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn response(&self) -> Value {
        json!({
            "schemaVersion": "openagents.managed_sandbox_runtime_error.v1",
            "error": self.code,
            "message": self.message,
            "status": "failed"
        })
    }
}

/// Execute one provider-side lifecycle operation under the daemon's durable
/// state root. The only production provider is the exact live GCE profile.
pub fn execute(
    state_root: &Path,
    request: ManagedSandboxRuntimeRequest,
) -> Result<ManagedSandboxRuntimeReceipt, RuntimeError> {
    let provider = LiveGceManagedSandboxProvider::from_env()?;
    execute_with_provider(state_root, &provider, request, now_ms()?)
}

/// Stop one ready sandbox only after a verified Phase 2 checkpoint exists.
///
/// The durable runtime journal supplies the program and work-unit scope. The
/// caller cannot replace those values or stop a stale provider generation.
pub fn stop_after_checkpoint(
    state_root: &Path,
    owner_ref: &str,
    tenant_ref: &str,
    sandbox_ref: &str,
    expected_generation: u64,
    stop_ref: &str,
) -> Result<ManagedSandboxRuntimeReceipt, RuntimeError> {
    let provider = LiveGceManagedSandboxProvider::from_env()?;
    stop_after_checkpoint_with_provider(
        state_root,
        &provider,
        owner_ref,
        tenant_ref,
        sandbox_ref,
        expected_generation,
        stop_ref,
        now_ms()?,
    )
}

#[allow(clippy::too_many_arguments)]
fn stop_after_checkpoint_with_provider(
    state_root: &Path,
    provider: &dyn ManagedSandboxProvider,
    owner_ref: &str,
    tenant_ref: &str,
    sandbox_ref: &str,
    expected_generation: u64,
    stop_ref: &str,
    now: u64,
) -> Result<ManagedSandboxRuntimeReceipt, RuntimeError> {
    let journal = load_journal(&journal_path(state_root, sandbox_ref))?.ok_or_else(|| {
        RuntimeError::new(
            404,
            "resource_not_found",
            "sandbox runtime journal was not found",
        )
    })?;
    if journal.owner_ref != owner_ref
        || journal.tenant_ref != tenant_ref
        || journal.sandbox_ref != sandbox_ref
    {
        return Err(RuntimeError::new(
            403,
            "scope_mismatch",
            "checkpoint stop scope does not match durable ownership",
        ));
    }
    if journal.generation != expected_generation {
        return Err(RuntimeError::new(
            409,
            "generation_conflict",
            "checkpoint stop generation does not match durable ownership",
        ));
    }
    execute_with_provider(
        state_root,
        provider,
        ManagedSandboxRuntimeRequest {
            operation_ref: stop_ref.to_string(),
            idempotency_ref: format!(
                "idempotency-ref://sha256/{}",
                full_digest(format!("checkpoint-stop|{stop_ref}"))
            ),
            actor_ref: owner_ref.to_string(),
            owner_ref: journal.owner_ref,
            tenant_ref: journal.tenant_ref,
            program_ref: journal.program_ref,
            work_unit_ref: journal.work_unit_ref,
            sandbox_ref: journal.sandbox_ref,
            expected_generation,
            action: RuntimeAction::Stop,
            profile: None,
        },
        now,
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointRestoreContext {
    pub schema: &'static str,
    pub owner_ref: String,
    pub tenant_ref: String,
    pub sandbox_ref: String,
    pub resource_generation: u64,
    pub restored_capability_refs: Vec<String>,
}

pub fn execute_private_ingress(
    state_root: &Path,
    request: &ManagedSandboxPhase2Request,
) -> Result<Value, RuntimeError> {
    execute_private_ingress_at(state_root, request, now_ms()?)
}

#[allow(clippy::too_many_arguments)]
pub fn authorize_private_preview(
    state_root: &Path,
    capability: &Value,
    audience_ref: &str,
    preview_path: &str,
    encoding: &str,
    operation_ref: &str,
) -> Result<ManagedSandboxGuestIoRequest, RuntimeError> {
    authorize_private_preview_at(
        state_root,
        capability,
        audience_ref,
        preview_path,
        encoding,
        operation_ref,
        now_ms()?,
    )
}

#[allow(clippy::too_many_arguments)]
fn authorize_private_preview_at(
    state_root: &Path,
    capability: &Value,
    audience_ref: &str,
    preview_path: &str,
    encoding: &str,
    operation_ref: &str,
    now: u64,
) -> Result<ManagedSandboxGuestIoRequest, RuntimeError> {
    let guest_preview_path = preview_path
        .strip_prefix("/workspace/")
        .filter(|path| {
            !path.is_empty()
                && !path.contains('\\')
                && !path
                    .split('/')
                    .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        })
        .map(|path| format!("workspace/{path}"))
        .ok_or_else(|| RuntimeError::validation("private_preview_path_invalid"))?;
    let capability = capability
        .as_object()
        .ok_or_else(|| RuntimeError::validation("private_preview_capability_invalid"))?;
    let capability_ref = runtime_string(capability, "capabilityRef")?;
    let sandbox_ref = runtime_string(capability, "sandboxRef")?;
    let owner_ref = runtime_string(capability, "ownerRef")?;
    let generation = runtime_number(capability, "resourceGeneration")?;
    if runtime_string(capability, "audienceRef")? != audience_ref
        || runtime_string(capability, "kind")? != "preview"
        || runtime_string(capability, "_tag")? != "Active"
    {
        return Err(RuntimeError::new(
            403,
            "private_preview_scope_mismatch",
            "private preview audience or capability kind was refused",
        ));
    }
    let journal = load_journal(&journal_path(state_root, sandbox_ref))?.ok_or_else(|| {
        RuntimeError::new(
            404,
            "private_preview_not_found",
            "private preview sandbox was not found",
        )
    })?;
    if journal.owner_ref != owner_ref
        || journal.sandbox_ref != sandbox_ref
        || journal.generation != generation
        || journal.phase != RuntimePhase::Ready
    {
        return Err(RuntimeError::new(
            409,
            "private_preview_generation_conflict",
            "private preview requires the current ready generation",
        ));
    }
    let record = journal
        .private_ingress
        .iter()
        .find(|record| record.capability_ref == capability_ref)
        .ok_or_else(|| {
            RuntimeError::new(
                404,
                "private_preview_not_found",
                "private preview capability was not found",
            )
        })?;
    if record.terminal_response.is_some() {
        return Err(RuntimeError::new(
            410,
            "private_preview_revoked",
            "private preview capability is no longer active",
        ));
    }
    if now >= record.expires_at_ms {
        return Err(RuntimeError::new(
            410,
            "private_preview_expired",
            "private preview capability expired",
        ));
    }
    if digest_json(&Value::Object(capability.clone()))? != digest_json(&record.active_response)? {
        return Err(RuntimeError::new(
            409,
            "private_preview_capability_conflict",
            "private preview capability bytes do not match durable state",
        ));
    }
    let requested_at = unix_ms_to_iso(now)?;
    let expires_at = unix_ms_to_iso(record.expires_at_ms)?;
    let idempotency_digest = full_digest(format!(
        "private-preview|{capability_ref}|{audience_ref}|{preview_path}|{encoding}|{operation_ref}"
    ));
    Ok(ManagedSandboxGuestIoRequest {
        schema_version: "openagents.managed_sandbox_guest_io.v1".to_string(),
        action: GuestIoAction::ReadFile,
        operation_ref: operation_ref.to_string(),
        idempotency_ref: format!("idempotency.sbx10.preview.{}", &idempotency_digest[..32]),
        actor_ref: audience_ref.to_string(),
        owner_ref: journal.owner_ref,
        tenant_ref: journal.tenant_ref,
        program_ref: journal.program_ref,
        work_unit_ref: journal.work_unit_ref,
        sandbox_ref: journal.sandbox_ref,
        resource_generation: journal.generation,
        capability_ref: capability_ref.to_string(),
        capability_state: "active".to_string(),
        capability_expires_at: expires_at,
        requested_at,
        limits: GuestIoLimits {
            workspace_root_ref: "workspace.managed-sandbox".to_string(),
            max_file_bytes: 1_048_576,
            max_artifact_bytes: 1_048_576,
            max_output_bytes: 1_048_576,
            max_duration_millis: 30_000,
            max_cpu_millis: 30_000,
            max_processes: 1,
            max_network_bytes: 0,
            network_policy_ref: "network-policy.managed-sandbox.deny-all".to_string(),
        },
        path: Some(guest_preview_path),
        encoding: Some(encoding.to_string()),
        content: None,
        content_digest: None,
        command: None,
        command_digest: None,
        cwd: None,
        timeout_millis: None,
        retention_until: None,
    })
}

fn execute_private_ingress_at(
    state_root: &Path,
    request: &ManagedSandboxPhase2Request,
    now: u64,
) -> Result<Value, RuntimeError> {
    let command = request
        .command
        .as_ref()
        .and_then(Value::as_object)
        .ok_or_else(|| RuntimeError::validation("private_ingress_command_missing"))?;
    let owner_ref = runtime_string(command, "ownerRef")?;
    let tenant_ref = runtime_string(command, "tenantRef")?;
    let sandbox_ref = runtime_string(command, "sandboxRef")?;
    let generation = runtime_number(command, "resourceGeneration")?;
    let path = journal_path(state_root, sandbox_ref);
    let mut journal = load_journal(&path)?.ok_or_else(|| {
        RuntimeError::new(
            404,
            "resource_not_found",
            "private ingress sandbox was not found",
        )
    })?;
    if journal.owner_ref != owner_ref
        || journal.tenant_ref != tenant_ref
        || journal.sandbox_ref != sandbox_ref
    {
        return Err(RuntimeError::new(
            403,
            "scope_mismatch",
            "private ingress scope does not match durable ownership",
        ));
    }
    if journal.generation != generation || journal.phase != RuntimePhase::Ready {
        return Err(RuntimeError::new(
            409,
            "generation_conflict",
            "private ingress requires the current ready generation",
        ));
    }
    let fingerprint = digest_json(command)?;
    match request.action {
        ManagedSandboxPhase2Action::CreatePrivateIngress => {
            let command_ref = runtime_string(command, "commandRef")?;
            let idempotency_ref = runtime_string(command, "idempotencyRef")?;
            if let Some(existing) = journal.private_ingress.iter().find(|record| {
                record.create_command_ref == command_ref
                    || record.create_idempotency_ref == idempotency_ref
            }) {
                if existing.create_fingerprint == fingerprint {
                    return Ok(existing.active_response.clone());
                }
                return Err(RuntimeError::new(
                    409,
                    "idempotency_conflict",
                    "private ingress create identity was reused with different bytes",
                ));
            }
            if journal
                .private_ingress
                .iter()
                .filter(|record| record.terminal_response.is_none())
                .count()
                >= MAX_CAPABILITY_REFS
            {
                return Err(RuntimeError::new(
                    409,
                    "private_ingress_limit",
                    "the sandbox has too many active private ingress capabilities",
                ));
            }
            let ttl_seconds = runtime_number(command, "ttlSeconds")?;
            if !(1..=900).contains(&ttl_seconds) {
                return Err(RuntimeError::validation("private_ingress_ttl_invalid"));
            }
            let digest = full_digest(format!(
                "private-ingress|{owner_ref}|{tenant_ref}|{sandbox_ref}|{generation}|{command_ref}|{idempotency_ref}"
            ));
            let capability_ref = format!("capability.sbx10.ingress.{}", &digest[..32]);
            let issued_at = unix_ms_to_iso(now)?;
            let expires_at_ms = now.saturating_add(ttl_seconds.saturating_mul(1_000));
            let expires_at = unix_ms_to_iso(expires_at_ms)?;
            let access_url_digest = format!(
                "sha256:{}",
                full_digest(format!(
                    "https://openagents.com/api/managed-sandboxes/private-ingress/{capability_ref}"
                ))
            );
            let response = json!({
                "_tag": "Active",
                "schema": "openagents.managed_sandbox_private_ingress.v1",
                "capabilityRef": capability_ref,
                "sandboxRef": sandbox_ref,
                "resourceGeneration": generation,
                "ownerRef": owner_ref,
                "audienceRef": runtime_string(command, "audienceRef")?,
                "kind": runtime_string(command, "kind")?,
                "issuedAt": issued_at,
                "expiresAt": expires_at,
                "ttlSeconds": ttl_seconds,
                "accessUrlDigest": access_url_digest,
                "accessUrlAtRest": "redacted",
                "audiencePolicy": "owner_scoped_explicit_audience",
                "publicAccess": false,
                "permanentRoute": false,
                "vnc": "unsupported",
                "auditRefs": [format!("audit.sbx10.ingress.create.{}", &digest[..32])]
            });
            journal.private_ingress.push(PrivateIngressRecord {
                capability_ref,
                create_command_ref: command_ref.to_string(),
                create_idempotency_ref: idempotency_ref.to_string(),
                create_fingerprint: fingerprint,
                issued_at_ms: now,
                expires_at_ms,
                active_response: response.clone(),
                terminal_command_ref: None,
                terminal_idempotency_ref: None,
                terminal_fingerprint: None,
                terminal_response: None,
            });
            save_journal(&path, &journal)?;
            Ok(response)
        }
        ManagedSandboxPhase2Action::RevokePrivateIngress
        | ManagedSandboxPhase2Action::ExpirePrivateIngress => {
            let capability_ref = runtime_string(command, "capabilityRef")?;
            let command_ref = runtime_string(command, "commandRef")?;
            let idempotency_ref = runtime_string(command, "idempotencyRef")?;
            let record = journal
                .private_ingress
                .iter_mut()
                .find(|record| record.capability_ref == capability_ref)
                .ok_or_else(|| {
                    RuntimeError::new(
                        404,
                        "private_ingress_not_found",
                        "private ingress capability was not found",
                    )
                })?;
            if let Some(response) = &record.terminal_response {
                if record.terminal_command_ref.as_deref() == Some(command_ref)
                    && record.terminal_idempotency_ref.as_deref() == Some(idempotency_ref)
                    && record.terminal_fingerprint.as_deref() == Some(fingerprint.as_str())
                {
                    return Ok(response.clone());
                }
                return Err(RuntimeError::new(
                    409,
                    "private_ingress_terminal_conflict",
                    "private ingress capability is already terminal",
                ));
            }
            let supplied = request
                .capability
                .as_ref()
                .ok_or_else(|| RuntimeError::validation("private_ingress_capability_missing"))?;
            if digest_json(supplied)? != digest_json(&record.active_response)? {
                return Err(RuntimeError::new(
                    409,
                    "private_ingress_capability_conflict",
                    "private ingress terminal command does not bind active bytes",
                ));
            }
            let expired = now >= record.expires_at_ms;
            if now < record.issued_at_ms {
                return Err(RuntimeError::new(
                    409,
                    "private_ingress_clock_conflict",
                    "private ingress terminal time precedes issuance",
                ));
            }
            let terminal_state = match request.action {
                ManagedSandboxPhase2Action::RevokePrivateIngress if !expired => "revoked",
                ManagedSandboxPhase2Action::ExpirePrivateIngress if expired => "expired",
                ManagedSandboxPhase2Action::RevokePrivateIngress => {
                    return Err(RuntimeError::new(
                        409,
                        "private_ingress_expired",
                        "expired private ingress cannot be revoked",
                    ));
                }
                ManagedSandboxPhase2Action::ExpirePrivateIngress => {
                    return Err(RuntimeError::new(
                        409,
                        "private_ingress_not_expired",
                        "private ingress expiry is not due",
                    ));
                }
                _ => unreachable!("matched terminal ingress action"),
            };
            let terminal_digest = full_digest(format!(
                "private-ingress-{terminal_state}|{capability_ref}|{command_ref}"
            ));
            let mut response = record.active_response.clone();
            let object = response.as_object_mut().ok_or_else(|| {
                RuntimeError::new(500, "journal_corrupt", "private ingress state is invalid")
            })?;
            object.insert("_tag".to_string(), json!("Cleaned"));
            object.insert("terminalState".to_string(), json!(terminal_state));
            object.insert("cleanedAt".to_string(), json!(unix_ms_to_iso(now)?));
            object.insert(
                "cleanupReceiptRef".to_string(),
                json!(format!(
                    "receipt.sbx10.ingress.cleanup.{}",
                    &terminal_digest[..32]
                )),
            );
            let audits = object
                .get_mut("auditRefs")
                .and_then(Value::as_array_mut)
                .ok_or_else(|| {
                    RuntimeError::new(500, "journal_corrupt", "private ingress audit is invalid")
                })?;
            audits.push(json!(format!(
                "audit.sbx10.ingress.{terminal_state}.{}",
                &terminal_digest[..32]
            )));
            record.terminal_command_ref = Some(command_ref.to_string());
            record.terminal_idempotency_ref = Some(idempotency_ref.to_string());
            record.terminal_fingerprint = Some(fingerprint);
            record.terminal_response = Some(response.clone());
            save_journal(&path, &journal)?;
            Ok(response)
        }
        _ => Err(RuntimeError::validation("private_ingress_action_invalid")),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointForkContext {
    pub schema: &'static str,
    pub owner_ref: String,
    pub tenant_ref: String,
    pub source_sandbox_ref: String,
    pub source_resource_generation: u64,
    pub fork_sandbox_ref: String,
    pub fork_resource_generation: u64,
    pub fork_capability_refs: Vec<String>,
    pub cleanup_obligation_ref: String,
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_checkpoint_fork(
    state_root: &Path,
    owner_ref: &str,
    tenant_ref: &str,
    source_sandbox_ref: &str,
    source_resource_generation: u64,
    command_ref: &str,
    checkpoint_ref: &str,
    source_capability_refs: &[String],
) -> Result<CheckpointForkContext, RuntimeError> {
    let provider = LiveGceManagedSandboxProvider::from_env()?;
    prepare_checkpoint_fork_with_provider(
        state_root,
        &provider,
        owner_ref,
        tenant_ref,
        source_sandbox_ref,
        source_resource_generation,
        command_ref,
        checkpoint_ref,
        source_capability_refs,
        now_ms()?,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_checkpoint_fork_with_provider(
    state_root: &Path,
    provider: &dyn ManagedSandboxProvider,
    owner_ref: &str,
    tenant_ref: &str,
    source_sandbox_ref: &str,
    source_resource_generation: u64,
    command_ref: &str,
    checkpoint_ref: &str,
    source_capability_refs: &[String],
    now: u64,
) -> Result<CheckpointForkContext, RuntimeError> {
    let source = load_journal(&journal_path(state_root, source_sandbox_ref))?.ok_or_else(|| {
        RuntimeError::new(
            404,
            "resource_not_found",
            "fork source runtime journal was not found",
        )
    })?;
    if source.owner_ref != owner_ref
        || source.tenant_ref != tenant_ref
        || source.sandbox_ref != source_sandbox_ref
    {
        return Err(RuntimeError::new(
            403,
            "scope_mismatch",
            "fork source scope does not match durable ownership",
        ));
    }
    if source.generation != source_resource_generation || source.phase != RuntimePhase::Stopped {
        return Err(RuntimeError::new(
            409,
            "fork_source_stale",
            "fork source generation is not stopped and current",
        ));
    }
    if source.profile.capability_refs != source_capability_refs {
        return Err(RuntimeError::new(
            409,
            "fork_capability_conflict",
            "fork source capabilities do not match durable ownership",
        ));
    }

    let fork_digest = full_digest(format!(
        "fork|{owner_ref}|{tenant_ref}|{source_sandbox_ref}|{source_resource_generation}|{checkpoint_ref}|{command_ref}"
    ));
    let fork_sandbox_ref = format!("sandbox.sbx10.fork.{}", &fork_digest[..32]);
    let runtime_capability_ref = format!("capability-ref://run/fork-{fork_digest}");
    let fork_capability_ref = format!("capability.sbx10.fork.{}", &fork_digest[..32]);
    if source_capability_refs
        .iter()
        .any(|source_ref| source_ref == &fork_capability_ref)
    {
        return Err(RuntimeError::new(
            409,
            "fork_capability_conflict",
            "fork capability identity overlaps the source",
        ));
    }
    let cleanup_obligation_ref = format!("cleanup.sbx10.fork.{}", &fork_digest[..32]);
    let path = journal_path(state_root, &fork_sandbox_ref);

    if let Some(existing) = load_journal(&path)? {
        let existing = reconcile_unsettled_checkpoint_fork(
            state_root,
            provider,
            existing,
            owner_ref,
            &fork_digest,
            now,
        )?;
        return attach_or_replay_checkpoint_fork(
            &path,
            existing,
            owner_ref,
            tenant_ref,
            source_sandbox_ref,
            source_resource_generation,
            command_ref,
            checkpoint_ref,
            &runtime_capability_ref,
            &fork_capability_ref,
            &cleanup_obligation_ref,
            &source,
        );
    }

    let mut profile = source.profile.clone();
    profile.capability_refs = vec![runtime_capability_ref.clone()];
    profile.profile_digest = format!("sha256:{}", "0".repeat(64));
    profile.profile_digest = digest_json(&profile)?;
    let create = execute_with_provider(
        state_root,
        provider,
        ManagedSandboxRuntimeRequest {
            operation_ref: format!("operation.sbx10.fork.{}", &fork_digest[..32]),
            idempotency_ref: format!("idempotency.sbx10.fork.{}", &fork_digest[..32]),
            actor_ref: owner_ref.to_string(),
            owner_ref: source.owner_ref.clone(),
            tenant_ref: source.tenant_ref.clone(),
            program_ref: source.program_ref.clone(),
            work_unit_ref: source.work_unit_ref.clone(),
            sandbox_ref: fork_sandbox_ref.clone(),
            expected_generation: 0,
            action: RuntimeAction::Create,
            profile: Some(profile),
        },
        now,
    );
    if let Err(error) = create {
        if error.status() == 409 {
            if let Some(existing) = load_journal(&path)? {
                let existing = reconcile_unsettled_checkpoint_fork(
                    state_root,
                    provider,
                    existing,
                    owner_ref,
                    &fork_digest,
                    now,
                )?;
                return attach_or_replay_checkpoint_fork(
                    &path,
                    existing,
                    owner_ref,
                    tenant_ref,
                    source_sandbox_ref,
                    source_resource_generation,
                    command_ref,
                    checkpoint_ref,
                    &runtime_capability_ref,
                    &fork_capability_ref,
                    &cleanup_obligation_ref,
                    &source,
                );
            }
        }
        return Err(error);
    }
    let create = create.expect("checked successful fork create");
    if create.phase != RuntimePhase::Ready || create.generation != 1 {
        return Err(RuntimeError::new(
            503,
            "fork_create_failed",
            "fork runtime did not become ready",
        ));
    }
    let mut fork = load_journal(&path)?.ok_or_else(|| {
        RuntimeError::new(
            500,
            "journal_read_failed",
            "fork runtime journal disappeared",
        )
    })?;
    fork.pending_checkpoint_fork = Some(PendingCheckpointFork {
        command_ref: command_ref.to_string(),
        checkpoint_ref: checkpoint_ref.to_string(),
        source_sandbox_ref: source_sandbox_ref.to_string(),
        source_resource_generation,
        fork_capability_refs: vec![fork_capability_ref],
        cleanup_obligation_ref,
        completed: false,
    });
    save_journal(&path, &fork)?;
    checkpoint_fork_context_from_existing(
        fork,
        owner_ref,
        tenant_ref,
        source_sandbox_ref,
        source_resource_generation,
        command_ref,
        checkpoint_ref,
    )
}

fn reconcile_unsettled_checkpoint_fork(
    state_root: &Path,
    provider: &dyn ManagedSandboxProvider,
    journal: RuntimeJournal,
    owner_ref: &str,
    fork_digest: &str,
    now: u64,
) -> Result<RuntimeJournal, RuntimeError> {
    if journal.pending_checkpoint_fork.is_some() || journal.phase != RuntimePhase::Provisioning {
        return Ok(journal);
    }
    let receipt = execute_with_provider(
        state_root,
        provider,
        ManagedSandboxRuntimeRequest {
            operation_ref: format!("operation.sbx10.fork-reconcile.{}", &fork_digest[..32]),
            idempotency_ref: format!("idempotency.sbx10.fork-reconcile.{}", &fork_digest[..32]),
            actor_ref: owner_ref.to_string(),
            owner_ref: journal.owner_ref.clone(),
            tenant_ref: journal.tenant_ref.clone(),
            program_ref: journal.program_ref.clone(),
            work_unit_ref: journal.work_unit_ref.clone(),
            sandbox_ref: journal.sandbox_ref.clone(),
            expected_generation: journal.generation,
            action: RuntimeAction::Reconcile,
            profile: None,
        },
        now,
    )?;
    if receipt.phase != RuntimePhase::Ready {
        return Err(RuntimeError::new(
            503,
            "fork_create_failed",
            "fork runtime reconciliation did not become ready",
        ));
    }
    load_journal(&journal_path(state_root, &journal.sandbox_ref))?.ok_or_else(|| {
        RuntimeError::new(
            500,
            "journal_read_failed",
            "reconciled fork runtime journal disappeared",
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn attach_or_replay_checkpoint_fork(
    path: &Path,
    mut journal: RuntimeJournal,
    owner_ref: &str,
    tenant_ref: &str,
    source_sandbox_ref: &str,
    source_resource_generation: u64,
    command_ref: &str,
    checkpoint_ref: &str,
    runtime_capability_ref: &str,
    fork_capability_ref: &str,
    cleanup_obligation_ref: &str,
    source: &RuntimeJournal,
) -> Result<CheckpointForkContext, RuntimeError> {
    if journal.pending_checkpoint_fork.is_none() {
        if journal.owner_ref != owner_ref
            || journal.tenant_ref != tenant_ref
            || journal.program_ref != source.program_ref
            || journal.work_unit_ref != source.work_unit_ref
            || journal.generation != 1
            || journal.phase != RuntimePhase::Ready
            || journal.profile.capability_refs != [runtime_capability_ref]
        {
            return Err(RuntimeError::new(
                409,
                "fork_conflict",
                "fork identity is already in use",
            ));
        }
        journal.pending_checkpoint_fork = Some(PendingCheckpointFork {
            command_ref: command_ref.to_string(),
            checkpoint_ref: checkpoint_ref.to_string(),
            source_sandbox_ref: source_sandbox_ref.to_string(),
            source_resource_generation,
            fork_capability_refs: vec![fork_capability_ref.to_string()],
            cleanup_obligation_ref: cleanup_obligation_ref.to_string(),
            completed: false,
        });
        save_journal(path, &journal)?;
    }
    checkpoint_fork_context_from_existing(
        journal,
        owner_ref,
        tenant_ref,
        source_sandbox_ref,
        source_resource_generation,
        command_ref,
        checkpoint_ref,
    )
}

#[allow(clippy::too_many_arguments)]
fn checkpoint_fork_context_from_existing(
    journal: RuntimeJournal,
    owner_ref: &str,
    tenant_ref: &str,
    source_sandbox_ref: &str,
    source_resource_generation: u64,
    command_ref: &str,
    checkpoint_ref: &str,
) -> Result<CheckpointForkContext, RuntimeError> {
    let pending = journal.pending_checkpoint_fork.as_ref().ok_or_else(|| {
        RuntimeError::new(409, "fork_conflict", "fork identity is already in use")
    })?;
    if journal.owner_ref != owner_ref
        || journal.tenant_ref != tenant_ref
        || journal.phase != RuntimePhase::Ready
        || journal.generation != 1
        || pending.command_ref != command_ref
        || pending.checkpoint_ref != checkpoint_ref
        || pending.source_sandbox_ref != source_sandbox_ref
        || pending.source_resource_generation != source_resource_generation
    {
        return Err(RuntimeError::new(
            409,
            "fork_conflict",
            "fork replay does not match durable intent",
        ));
    }
    Ok(CheckpointForkContext {
        schema: "openagents.managed_sandbox_phase2_fork_context.v1",
        owner_ref: journal.owner_ref,
        tenant_ref: journal.tenant_ref,
        source_sandbox_ref: pending.source_sandbox_ref.clone(),
        source_resource_generation: pending.source_resource_generation,
        fork_sandbox_ref: journal.sandbox_ref,
        fork_resource_generation: journal.generation,
        fork_capability_refs: pending.fork_capability_refs.clone(),
        cleanup_obligation_ref: pending.cleanup_obligation_ref.clone(),
    })
}

pub fn finish_checkpoint_fork(
    state_root: &Path,
    owner_ref: &str,
    tenant_ref: &str,
    fork_sandbox_ref: &str,
    command_ref: &str,
    succeeded: bool,
) -> Result<(), RuntimeError> {
    let provider = LiveGceManagedSandboxProvider::from_env()?;
    finish_checkpoint_fork_with_provider(
        state_root,
        &provider,
        owner_ref,
        tenant_ref,
        fork_sandbox_ref,
        command_ref,
        succeeded,
        now_ms()?,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_checkpoint_fork_with_provider(
    state_root: &Path,
    provider: &dyn ManagedSandboxProvider,
    owner_ref: &str,
    tenant_ref: &str,
    fork_sandbox_ref: &str,
    command_ref: &str,
    succeeded: bool,
    now: u64,
) -> Result<(), RuntimeError> {
    let path = journal_path(state_root, fork_sandbox_ref);
    let mut journal = load_journal(&path)?.ok_or_else(|| {
        RuntimeError::new(
            404,
            "resource_not_found",
            "fork runtime journal was not found",
        )
    })?;
    if journal.owner_ref != owner_ref
        || journal.tenant_ref != tenant_ref
        || journal.sandbox_ref != fork_sandbox_ref
        || journal
            .pending_checkpoint_fork
            .as_ref()
            .is_none_or(|pending| pending.command_ref != command_ref)
    {
        return Err(RuntimeError::new(
            409,
            "fork_scope_conflict",
            "fork completion does not match durable intent",
        ));
    }
    if succeeded && journal.phase == RuntimePhase::Ready {
        if let Some(pending) = &mut journal.pending_checkpoint_fork {
            pending.completed = true;
        }
        return save_journal(&path, &journal);
    }
    let delete = ManagedSandboxRuntimeRequest {
        operation_ref: format!(
            "operation.sbx10.fork-cleanup.{}",
            &full_digest(command_ref)[..32]
        ),
        idempotency_ref: format!(
            "idempotency.sbx10.fork-cleanup.{}",
            &full_digest(command_ref)[..32]
        ),
        actor_ref: owner_ref.to_string(),
        owner_ref: journal.owner_ref.clone(),
        tenant_ref: journal.tenant_ref.clone(),
        program_ref: journal.program_ref.clone(),
        work_unit_ref: journal.work_unit_ref.clone(),
        sandbox_ref: journal.sandbox_ref.clone(),
        expected_generation: journal.generation,
        action: RuntimeAction::Delete,
        profile: None,
    };
    let receipt = execute_with_provider(state_root, provider, delete, now)?;
    if receipt.phase != RuntimePhase::Deleted || !receipt.cleanup_observed {
        return Err(RuntimeError::new(
            503,
            "fork_cleanup_recovery_required",
            "failed fork cleanup is not complete",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_checkpoint_restore(
    state_root: &Path,
    owner_ref: &str,
    tenant_ref: &str,
    sandbox_ref: &str,
    command_ref: &str,
    checkpoint_ref: &str,
    checkpoint_source_generation: u64,
    source_capability_refs: &[String],
) -> Result<CheckpointRestoreContext, RuntimeError> {
    let provider = LiveGceManagedSandboxProvider::from_env()?;
    prepare_checkpoint_restore_with_provider(
        state_root,
        &provider,
        owner_ref,
        tenant_ref,
        sandbox_ref,
        command_ref,
        checkpoint_ref,
        checkpoint_source_generation,
        source_capability_refs,
        now_ms()?,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_checkpoint_restore_with_provider(
    state_root: &Path,
    provider: &dyn ManagedSandboxProvider,
    owner_ref: &str,
    tenant_ref: &str,
    sandbox_ref: &str,
    command_ref: &str,
    checkpoint_ref: &str,
    checkpoint_source_generation: u64,
    source_capability_refs: &[String],
    now: u64,
) -> Result<CheckpointRestoreContext, RuntimeError> {
    let path = journal_path(state_root, sandbox_ref);
    let mut journal = load_journal(&path)?.ok_or_else(|| {
        RuntimeError::new(
            404,
            "resource_not_found",
            "restore destination runtime journal was not found",
        )
    })?;
    if journal.owner_ref != owner_ref
        || journal.tenant_ref != tenant_ref
        || journal.sandbox_ref != sandbox_ref
    {
        return Err(RuntimeError::new(
            403,
            "scope_mismatch",
            "restore destination scope does not match durable ownership",
        ));
    }
    if journal
        .pending_checkpoint_restore
        .as_ref()
        .is_some_and(|pending| pending.completed && journal.phase == RuntimePhase::Stopped)
    {
        journal.pending_checkpoint_restore = None;
        save_journal(&path, &journal)?;
    }
    if let Some(pending) = &journal.pending_checkpoint_restore {
        if pending.command_ref != command_ref
            || pending.checkpoint_ref != checkpoint_ref
            || pending.checkpoint_source_generation != checkpoint_source_generation
        {
            return Err(RuntimeError::new(
                409,
                "restore_conflict",
                "a different checkpoint restore already owns the destination",
            ));
        }
        if journal.phase != RuntimePhase::Ready
            || journal.generation != pending.restored_resource_generation
        {
            return Err(RuntimeError::new(
                409,
                "restore_recovery_required",
                "the prepared checkpoint restore is not ready",
            ));
        }
        return Ok(restore_context(&journal, pending));
    }
    if journal.phase != RuntimePhase::Stopped {
        return Err(invalid_phase("checkpoint restore", journal.phase));
    }
    let restored_resource_generation = journal.generation.saturating_add(1);
    if restored_resource_generation <= checkpoint_source_generation {
        return Err(RuntimeError::new(
            409,
            "restore_generation_not_advanced",
            "destination generation does not advance the checkpoint source",
        ));
    }
    let capability_digest = full_digest(format!(
        "restore|{command_ref}|{checkpoint_ref}|{sandbox_ref}|{restored_resource_generation}"
    ));
    let runtime_capability_ref = format!("capability-ref://run/restore-{capability_digest}");
    let restored_capability_ref = format!("capability.sbx10.restore.{}", &capability_digest[..32]);
    if source_capability_refs
        .iter()
        .any(|source| source == &restored_capability_ref)
    {
        return Err(RuntimeError::new(
            409,
            "restore_capability_conflict",
            "restored capability identity overlaps the source",
        ));
    }
    journal.profile.capability_refs = vec![runtime_capability_ref.clone()];
    journal.profile.profile_digest = format!("sha256:{}", "0".repeat(64));
    journal.profile.profile_digest = digest_json(&journal.profile)?;
    journal.pending_checkpoint_restore = Some(PendingCheckpointRestore {
        command_ref: command_ref.to_string(),
        checkpoint_ref: checkpoint_ref.to_string(),
        checkpoint_source_generation,
        restored_resource_generation,
        runtime_capability_refs: vec![runtime_capability_ref],
        restored_capability_refs: vec![restored_capability_ref],
        completed: false,
    });
    save_journal(&path, &journal)?;
    let receipt = execute_with_provider(
        state_root,
        provider,
        ManagedSandboxRuntimeRequest {
            operation_ref: restore_resume_operation_ref(command_ref),
            idempotency_ref: format!(
                "idempotency.sbx10.restore.{}",
                &full_digest(format!("restore|{command_ref}"))[..32]
            ),
            actor_ref: owner_ref.to_string(),
            owner_ref: journal.owner_ref,
            tenant_ref: journal.tenant_ref,
            program_ref: journal.program_ref,
            work_unit_ref: journal.work_unit_ref,
            sandbox_ref: journal.sandbox_ref,
            expected_generation: journal.generation,
            action: RuntimeAction::Resume,
            profile: None,
        },
        now,
    )?;
    let journal = load_journal(&path)?.ok_or_else(|| {
        RuntimeError::new(500, "journal_read_failed", "restore journal disappeared")
    })?;
    let pending = journal.pending_checkpoint_restore.as_ref().ok_or_else(|| {
        RuntimeError::new(500, "restore_intent_lost", "restore intent disappeared")
    })?;
    if receipt.phase != RuntimePhase::Ready
        || receipt.generation != restored_resource_generation
        || journal.phase != RuntimePhase::Ready
    {
        return Err(RuntimeError::new(
            409,
            "restore_recovery_required",
            "destination resume did not become ready",
        ));
    }
    Ok(restore_context(&journal, pending))
}

fn restore_resume_operation_ref(command_ref: &str) -> String {
    format!(
        "operation.sbx10.restore.{}",
        &full_digest(command_ref)[..32]
    )
}

fn restore_context(
    journal: &RuntimeJournal,
    pending: &PendingCheckpointRestore,
) -> CheckpointRestoreContext {
    CheckpointRestoreContext {
        schema: "openagents.managed_sandbox_phase2_restore_context.v1",
        owner_ref: journal.owner_ref.clone(),
        tenant_ref: journal.tenant_ref.clone(),
        sandbox_ref: journal.sandbox_ref.clone(),
        resource_generation: pending.restored_resource_generation,
        restored_capability_refs: pending.restored_capability_refs.clone(),
    }
}

pub fn finish_checkpoint_restore(
    state_root: &Path,
    owner_ref: &str,
    tenant_ref: &str,
    sandbox_ref: &str,
    command_ref: &str,
    succeeded: bool,
) -> Result<(), RuntimeError> {
    let path = journal_path(state_root, sandbox_ref);
    let mut journal = load_journal(&path)?.ok_or_else(|| {
        RuntimeError::new(404, "resource_not_found", "restore journal was not found")
    })?;
    if journal.owner_ref != owner_ref
        || journal.tenant_ref != tenant_ref
        || journal.sandbox_ref != sandbox_ref
        || journal
            .pending_checkpoint_restore
            .as_ref()
            .is_none_or(|pending| pending.command_ref != command_ref)
    {
        return Err(RuntimeError::new(
            409,
            "restore_scope_conflict",
            "restore completion does not match durable intent",
        ));
    }
    if succeeded && journal.phase == RuntimePhase::Ready {
        if let Some(pending) = &mut journal.pending_checkpoint_restore {
            pending.completed = true;
        }
    } else {
        journal.phase = RuntimePhase::RecoveryRequired;
    }
    save_journal(&path, &journal)
}

fn execute_with_provider(
    state_root: &Path,
    provider: &dyn ManagedSandboxProvider,
    request: ManagedSandboxRuntimeRequest,
    now: u64,
) -> Result<ManagedSandboxRuntimeReceipt, RuntimeError> {
    request.validate()?;
    let fingerprint = digest_json(&request)?;
    let path = journal_path(state_root, &request.sandbox_ref);
    let existing = load_journal(&path)?;

    if let Some(journal) = &existing {
        for operation in &journal.operations {
            if operation.operation_ref == request.operation_ref
                || operation.idempotency_ref == request.idempotency_ref
            {
                if operation.fingerprint == fingerprint {
                    return Ok(operation.response.clone());
                }
                return Err(RuntimeError::new(
                    409,
                    "idempotency_conflict",
                    "operation or idempotency identity was reused with different bytes",
                ));
            }
        }
    }

    match request.action {
        RuntimeAction::Create => execute_create(state_root, provider, request, fingerprint, now),
        _ => execute_existing(state_root, provider, request, fingerprint, now, existing),
    }
}

fn execute_create(
    state_root: &Path,
    provider: &dyn ManagedSandboxProvider,
    request: ManagedSandboxRuntimeRequest,
    fingerprint: String,
    now: u64,
) -> Result<ManagedSandboxRuntimeReceipt, RuntimeError> {
    let path = journal_path(state_root, &request.sandbox_ref);
    if load_journal(&path)?.is_some() {
        return Err(RuntimeError::new(
            409,
            "resource_conflict",
            "sandbox already has provider ownership",
        ));
    }
    let profile = request.profile.clone().expect("validated create profile");
    provider.admit(&profile)?;
    if provider.kind() != "live_gce" {
        return Err(RuntimeError::new(
            503,
            "live_provider_unavailable",
            "fake or unavailable providers cannot satisfy managed-sandbox readiness",
        ));
    }
    let active = provider.count_active()?;
    if active >= profile.capacity.concurrent_capacity_cap {
        return Err(RuntimeError::new(
            409,
            "capacity_refused",
            "the exact managed-sandbox capacity class is full; substitution is forbidden",
        ));
    }
    let worst_case = cost_microusd(profile.ttl_ms, profile.budget.max_hourly_cost_microusd);
    let committed_program_cost =
        u128::from(worst_case).saturating_mul(u128::from(active.saturating_add(1)));
    if committed_program_cost > u128::from(profile.budget.program_budget_microusd) {
        return Err(RuntimeError::new(
            409,
            "program_budget_refused",
            "active capacity plus this sandbox exceeds the admitted program budget",
        ));
    }

    let ownership = provider.plan(&request.sandbox_ref);
    let mut journal = RuntimeJournal {
        schema_version: SCHEMA_VERSION.to_string(),
        owner_ref: request.owner_ref.clone(),
        tenant_ref: request.tenant_ref.clone(),
        program_ref: request.program_ref.clone(),
        work_unit_ref: request.work_unit_ref.clone(),
        sandbox_ref: request.sandbox_ref.clone(),
        generation: 1,
        phase: RuntimePhase::Provisioning,
        profile,
        ownership,
        provider_kind: provider.kind().to_string(),
        cleanup_owner_ref: format!(
            "cleanup-owner-ref://sha256/{}",
            short_digest(&request.sandbox_ref)
        ),
        created_at_ms: now,
        running_started_at_ms: None,
        accrued_running_ms: 0,
        readiness: ReadinessObservation::default(),
        cleanup: CleanupObservation::default(),
        pending_checkpoint_restore: None,
        pending_checkpoint_fork: None,
        private_ingress: Vec::new(),
        operations: Vec::new(),
    };
    // Cleanup ownership is durable before either the firewall or VM is created.
    save_journal(&path, &journal)?;

    match provider.create(&journal.ownership, &journal.profile, 1) {
        Ok(readiness) if readiness.is_ready() => {
            journal.readiness = readiness;
            journal.phase = RuntimePhase::Ready;
            journal.running_started_at_ms = Some(now);
        }
        Ok(readiness) => {
            journal.readiness = readiness;
            settle_failed_create(provider, &mut journal);
        }
        Err(_) => settle_failed_create(provider, &mut journal),
    }
    let error_code = match journal.phase {
        RuntimePhase::Ready => None,
        RuntimePhase::Failed => Some("provision_failed_clean".to_string()),
        _ => Some("cleanup_recovery_required".to_string()),
    };
    let receipt = receipt_for(&journal, &request, now, error_code);
    record_operation(&mut journal, request, fingerprint, receipt.clone());
    save_journal(&path, &journal)?;
    Ok(receipt)
}

fn settle_failed_create(provider: &dyn ManagedSandboxProvider, journal: &mut RuntimeJournal) {
    match provider.cleanup(&journal.ownership) {
        Ok(cleanup) if cleanup.is_clean() => {
            journal.cleanup = cleanup;
            journal.phase = RuntimePhase::Failed;
        }
        Ok(cleanup) => {
            journal.cleanup = cleanup;
            journal.phase = RuntimePhase::RecoveryRequired;
        }
        Err(_) => journal.phase = RuntimePhase::RecoveryRequired,
    }
}

fn execute_existing(
    state_root: &Path,
    provider: &dyn ManagedSandboxProvider,
    request: ManagedSandboxRuntimeRequest,
    fingerprint: String,
    now: u64,
    existing: Option<RuntimeJournal>,
) -> Result<ManagedSandboxRuntimeReceipt, RuntimeError> {
    let mut journal = existing.ok_or_else(|| {
        RuntimeError::new(
            404,
            "resource_not_found",
            "sandbox runtime journal was not found",
        )
    })?;
    verify_scope(&journal, &request)?;
    if request.expected_generation != journal.generation {
        return Err(RuntimeError::new(
            409,
            "generation_conflict",
            "expected generation does not match provider ownership",
        ));
    }
    if journal
        .pending_checkpoint_restore
        .as_ref()
        .is_some_and(|pending| {
            !pending.completed
                && (request.action != RuntimeAction::Resume
                    || request.operation_ref != restore_resume_operation_ref(&pending.command_ref))
        })
    {
        return Err(RuntimeError::new(
            409,
            "restore_in_progress",
            "the destination is reserved for checkpoint restore completion",
        ));
    }
    if request.action == RuntimeAction::Delete
        && journal
            .private_ingress
            .iter()
            .any(|record| record.terminal_response.is_none())
    {
        return Err(RuntimeError::new(
            409,
            "private_ingress_active",
            "active private ingress must be revoked or expired before delete",
        ));
    }
    if journal
        .pending_checkpoint_fork
        .as_ref()
        .is_some_and(|pending| !pending.completed && request.action != RuntimeAction::Delete)
    {
        return Err(RuntimeError::new(
            409,
            "fork_in_progress",
            "the fork runtime is reserved for checkpoint content installation",
        ));
    }
    provider.admit(&journal.profile)?;
    if provider.kind() != journal.provider_kind || provider.kind() != "live_gce" {
        return Err(RuntimeError::new(
            503,
            "provider_identity_mismatch",
            "the exact admitted provider is unavailable; substitution is forbidden",
        ));
    }

    let mut error_code = None;
    match request.action {
        RuntimeAction::Probe => {
            if journal.phase != RuntimePhase::Ready {
                return Err(invalid_phase("probe", journal.phase));
            }
            match provider.probe(&journal.ownership, journal.generation) {
                Ok(readiness) if readiness.is_ready() => journal.readiness = readiness,
                Ok(readiness) => {
                    journal.readiness = readiness;
                    journal.phase = RuntimePhase::RecoveryRequired;
                    error_code = Some("readiness_lost".to_string());
                }
                Err(_) => {
                    journal.phase = RuntimePhase::RecoveryRequired;
                    error_code = Some("probe_failed".to_string());
                }
            }
        }
        RuntimeAction::Stop => {
            if journal.phase != RuntimePhase::Ready {
                return Err(invalid_phase("stop", journal.phase));
            }
            journal.phase = RuntimePhase::Stopping;
            save_journal(&journal_path(state_root, &request.sandbox_ref), &journal)?;
            match provider.stop(&journal.ownership) {
                Ok(true) => {
                    accrue_running_time(&mut journal, now);
                    journal.phase = RuntimePhase::Stopped;
                    journal.readiness = ReadinessObservation::default();
                }
                _ => {
                    journal.phase = RuntimePhase::RecoveryRequired;
                    error_code = Some("stop_recovery_required".to_string());
                }
            }
        }
        RuntimeAction::Resume => {
            if journal.phase != RuntimePhase::Stopped {
                return Err(invalid_phase("resume", journal.phase));
            }
            journal.phase = RuntimePhase::Resuming;
            journal.generation = journal.generation.saturating_add(1);
            save_journal(&journal_path(state_root, &request.sandbox_ref), &journal)?;
            match provider.resume(&journal.ownership, journal.generation) {
                Ok(readiness) if readiness.is_ready() => {
                    journal.readiness = readiness;
                    journal.phase = RuntimePhase::Ready;
                    journal.running_started_at_ms = Some(now);
                }
                Ok(readiness) => {
                    journal.readiness = readiness;
                    journal.phase = RuntimePhase::RecoveryRequired;
                    error_code = Some("resume_recovery_required".to_string());
                }
                Err(_) => {
                    journal.phase = RuntimePhase::RecoveryRequired;
                    error_code = Some("resume_recovery_required".to_string());
                }
            }
        }
        RuntimeAction::Delete => {
            if !matches!(
                journal.phase,
                RuntimePhase::Ready
                    | RuntimePhase::Stopped
                    | RuntimePhase::Failed
                    | RuntimePhase::RecoveryRequired
                    | RuntimePhase::Deleting
            ) {
                return Err(invalid_phase("delete", journal.phase));
            }
            begin_and_observe_cleanup(state_root, provider, &request, &mut journal, now);
            if journal.phase == RuntimePhase::RecoveryRequired {
                error_code = Some("cleanup_recovery_required".to_string());
            }
        }
        RuntimeAction::Reconcile => match journal.phase {
            RuntimePhase::Provisioning | RuntimePhase::Resuming => {
                match provider.probe(&journal.ownership, journal.generation) {
                    Ok(readiness) if readiness.is_ready() => {
                        journal.readiness = readiness;
                        journal.phase = RuntimePhase::Ready;
                        journal.running_started_at_ms.get_or_insert(now);
                    }
                    Ok(readiness) => {
                        journal.readiness = readiness;
                        settle_failed_create(provider, &mut journal);
                    }
                    Err(_) => settle_failed_create(provider, &mut journal),
                }
                if journal.phase != RuntimePhase::Ready {
                    error_code = Some(if journal.phase == RuntimePhase::Failed {
                        "reconciled_failed_clean".to_string()
                    } else {
                        "cleanup_recovery_required".to_string()
                    });
                }
            }
            RuntimePhase::Stopping => match provider.stop(&journal.ownership) {
                Ok(true) => {
                    accrue_running_time(&mut journal, now);
                    journal.phase = RuntimePhase::Stopped;
                    journal.readiness = ReadinessObservation::default();
                }
                _ => {
                    journal.phase = RuntimePhase::RecoveryRequired;
                    error_code = Some("stop_recovery_required".to_string());
                }
            },
            RuntimePhase::Deleting | RuntimePhase::RecoveryRequired | RuntimePhase::Failed => {
                begin_and_observe_cleanup(state_root, provider, &request, &mut journal, now);
                if journal.phase == RuntimePhase::RecoveryRequired {
                    error_code = Some("cleanup_recovery_required".to_string());
                }
            }
            RuntimePhase::Ready => match provider.probe(&journal.ownership, journal.generation) {
                Ok(readiness) if readiness.is_ready() => journal.readiness = readiness,
                Ok(readiness) => {
                    journal.readiness = readiness;
                    journal.phase = RuntimePhase::RecoveryRequired;
                    error_code = Some("readiness_lost".to_string());
                }
                Err(_) => {
                    journal.phase = RuntimePhase::RecoveryRequired;
                    error_code = Some("probe_failed".to_string());
                }
            },
            RuntimePhase::Stopped | RuntimePhase::Deleted => {}
        },
        RuntimeAction::Create => unreachable!("create handled separately"),
    }

    let receipt = receipt_for(&journal, &request, now, error_code);
    record_operation(&mut journal, request, fingerprint, receipt.clone());
    save_journal(&journal_path(state_root, &journal.sandbox_ref), &journal)?;
    Ok(receipt)
}

fn begin_and_observe_cleanup(
    state_root: &Path,
    provider: &dyn ManagedSandboxProvider,
    request: &ManagedSandboxRuntimeRequest,
    journal: &mut RuntimeJournal,
    now: u64,
) {
    journal.phase = RuntimePhase::Deleting;
    // A failed save is handled by the caller's final durable write. The
    // pre-effect save normally establishes cleanup intent for restart replay.
    if save_journal(&journal_path(state_root, &request.sandbox_ref), journal).is_err() {
        journal.phase = RuntimePhase::RecoveryRequired;
        return;
    }
    match provider.cleanup(&journal.ownership) {
        Ok(cleanup) if cleanup.is_clean() => {
            accrue_running_time(journal, now);
            journal.cleanup = cleanup;
            journal.phase = RuntimePhase::Deleted;
            journal.readiness = ReadinessObservation::default();
        }
        Ok(cleanup) => {
            journal.cleanup = cleanup;
            journal.phase = RuntimePhase::RecoveryRequired;
        }
        Err(_) => journal.phase = RuntimePhase::RecoveryRequired,
    }
}

fn verify_scope(
    journal: &RuntimeJournal,
    request: &ManagedSandboxRuntimeRequest,
) -> Result<(), RuntimeError> {
    if journal.owner_ref != request.owner_ref
        || journal.tenant_ref != request.tenant_ref
        || journal.program_ref != request.program_ref
        || journal.work_unit_ref != request.work_unit_ref
        || journal.sandbox_ref != request.sandbox_ref
    {
        return Err(RuntimeError::new(
            403,
            "scope_mismatch",
            "provider operation scope does not match durable ownership",
        ));
    }
    Ok(())
}

fn invalid_phase(action: &str, phase: RuntimePhase) -> RuntimeError {
    RuntimeError::new(
        409,
        "lifecycle_conflict",
        format!("{action} is not admitted from {phase:?}"),
    )
}

fn accrue_running_time(journal: &mut RuntimeJournal, now: u64) {
    if let Some(started) = journal.running_started_at_ms.take() {
        journal.accrued_running_ms = journal
            .accrued_running_ms
            .saturating_add(now.saturating_sub(started));
    }
}

fn receipt_for(
    journal: &RuntimeJournal,
    request: &ManagedSandboxRuntimeRequest,
    now: u64,
    error_code: Option<String>,
) -> ManagedSandboxRuntimeReceipt {
    let running_ms = journal.accrued_running_ms.saturating_add(
        journal
            .running_started_at_ms
            .map(|started| now.saturating_sub(started))
            .unwrap_or(0),
    );
    let cost = cost_microusd(running_ms, journal.profile.budget.max_hourly_cost_microusd);
    let material = format!(
        "{}|{}|{}|{:?}|{}|{}",
        request.operation_ref,
        journal.sandbox_ref,
        journal.generation,
        journal.phase,
        running_ms,
        cost
    );
    ManagedSandboxRuntimeReceipt {
        schema_version: SCHEMA_VERSION.to_string(),
        receipt_ref: format!("receipt-ref://sha256/{}", full_digest(&material)),
        operation_ref: request.operation_ref.clone(),
        action: request.action,
        sandbox_ref: journal.sandbox_ref.clone(),
        generation: journal.generation,
        phase: journal.phase,
        target_ref: journal.profile.target_ref.clone(),
        profile_ref: journal.profile.profile_ref.clone(),
        profile_digest: journal.profile.profile_digest.clone(),
        image_ref: journal.profile.image_ref.clone(),
        image_digest: journal.profile.image_digest.clone(),
        isolation_class: journal.profile.isolation_class.clone(),
        network_policy_ref: journal.profile.network_policy_ref.clone(),
        control_identity_ref: journal.profile.control_identity_ref.clone(),
        guest_identity_ref: journal.profile.guest_identity_ref.clone(),
        resource_ref: journal.ownership.resource_ref.clone(),
        firewall_ref: journal.ownership.firewall_ref.clone(),
        disk_ref: journal.ownership.disk_ref.clone(),
        provider_kind: journal.provider_kind.clone(),
        readiness_observed: journal.phase == RuntimePhase::Ready && journal.readiness.is_ready(),
        cleanup_observed: journal.cleanup.is_clean(),
        measured_running_ms: running_ms,
        measured_cost_microusd: cost,
        sandbox_budget_microusd: journal.profile.budget.sandbox_budget_microusd,
        program_budget_microusd: journal.profile.budget.program_budget_microusd,
        emitted_at_ms: now,
        error_code,
    }
}

fn record_operation(
    journal: &mut RuntimeJournal,
    request: ManagedSandboxRuntimeRequest,
    fingerprint: String,
    response: ManagedSandboxRuntimeReceipt,
) {
    journal.operations.push(OperationRecord {
        operation_ref: request.operation_ref,
        idempotency_ref: request.idempotency_ref,
        fingerprint,
        response,
    });
    if journal.operations.len() > MAX_OPERATION_RECORDS {
        let excess = journal.operations.len() - MAX_OPERATION_RECORDS;
        journal.operations.drain(0..excess);
    }
}

fn journal_path(state_root: &Path, sandbox_ref: &str) -> PathBuf {
    state_root
        .join("managed-sandbox-runtime")
        .join(format!("{}.json", full_digest(sandbox_ref)))
}

fn load_journal(path: &Path) -> Result<Option<RuntimeJournal>, RuntimeError> {
    match fs::read(path) {
        Ok(bytes) => {
            let journal: RuntimeJournal = serde_json::from_slice(&bytes).map_err(|_| {
                RuntimeError::new(
                    500,
                    "journal_corrupt",
                    "managed-sandbox provider journal is not valid",
                )
            })?;
            if journal.schema_version != SCHEMA_VERSION {
                return Err(RuntimeError::new(
                    500,
                    "journal_version_mismatch",
                    "managed-sandbox provider journal version is not admitted",
                ));
            }
            Ok(Some(journal))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(RuntimeError::new(
            500,
            "journal_read_failed",
            "managed-sandbox provider journal could not be read",
        )),
    }
}

fn save_journal(path: &Path, journal: &RuntimeJournal) -> Result<(), RuntimeError> {
    let parent = path.parent().ok_or_else(|| {
        RuntimeError::new(
            500,
            "journal_path_invalid",
            "provider journal path is invalid",
        )
    })?;
    fs::create_dir_all(parent).map_err(|_| {
        RuntimeError::new(
            500,
            "journal_directory_failed",
            "provider journal directory could not be created",
        )
    })?;
    let bytes = serde_json::to_vec_pretty(journal).map_err(|_| {
        RuntimeError::new(
            500,
            "journal_encode_failed",
            "provider journal could not be encoded",
        )
    })?;
    let temporary = path.with_extension(format!("tmp-{}", now_ms().unwrap_or(0)));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| {
            RuntimeError::new(
                500,
                "journal_write_failed",
                "provider journal could not be opened",
            )
        })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| {
                RuntimeError::new(
                    500,
                    "journal_write_failed",
                    "provider journal permissions could not be set",
                )
            })?;
    }
    file.write_all(&bytes).map_err(|_| {
        RuntimeError::new(
            500,
            "journal_write_failed",
            "provider journal could not be written",
        )
    })?;
    file.sync_all().map_err(|_| {
        RuntimeError::new(
            500,
            "journal_sync_failed",
            "provider journal could not be synchronized",
        )
    })?;
    fs::rename(&temporary, path).map_err(|_| {
        let _ = fs::remove_file(&temporary);
        RuntimeError::new(
            500,
            "journal_commit_failed",
            "provider journal could not be committed",
        )
    })?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| {
            RuntimeError::new(
                500,
                "journal_sync_failed",
                "provider journal directory could not be synchronized",
            )
        })
}

#[derive(Clone, Debug)]
struct LiveGceManagedSandboxConfig {
    project_id: String,
    zone: String,
    region: String,
    machine_class: String,
    image_project: String,
    image_name: String,
    image_id: String,
    image_digest: String,
    network: String,
    profile_ref: String,
    profile_digest: String,
    provisioner_ref: String,
    network_policy_ref: String,
    control_identity_ref: String,
    control_internal_ip: String,
    broker_port: u16,
    gcloud_bin: String,
}

#[derive(Clone, Debug)]
struct LiveGceManagedSandboxProvider {
    config: LiveGceManagedSandboxConfig,
}

fn guest_io_probe_args(project_id: &str, zone: &str, resource_name: &str) -> Vec<String> {
    vec![
        "compute".to_string(),
        "ssh".to_string(),
        format!("openagents@{resource_name}"),
        "--project".to_string(),
        project_id.to_string(),
        "--zone".to_string(),
        zone.to_string(),
        "--internal-ip".to_string(),
        "--quiet".to_string(),
        "--ssh-key-expire-after=10m".to_string(),
        "--ssh-flag=-oBatchMode=yes".to_string(),
        "--ssh-flag=-oConnectTimeout=5".to_string(),
        "--ssh-flag=-oConnectionAttempts=1".to_string(),
        "--ssh-flag=-oStrictHostKeyChecking=no".to_string(),
        "--ssh-flag=-oUserKnownHostsFile=/dev/null".to_string(),
        "--command".to_string(),
        format!("test -x {GUEST_IO_EXECUTABLE} && test -d /workspace"),
    ]
}

fn egress_tcp_allow_is_scoped(
    firewall: &Value,
    destination_cidr: &str,
    port: &str,
    target_tag: &str,
) -> bool {
    firewall.get("direction").and_then(Value::as_str) == Some("EGRESS")
        && firewall.get("priority").and_then(Value::as_u64) == Some(900)
        && firewall.get("disabled").and_then(Value::as_bool) != Some(true)
        && firewall
            .get("destinationRanges")
            .and_then(Value::as_array)
            .is_some_and(|ranges| ranges.len() == 1 && ranges[0].as_str() == Some(destination_cidr))
        && firewall
            .get("allowed")
            .and_then(Value::as_array)
            .is_some_and(|rules| {
                rules.len() == 1
                    && rules[0].get("IPProtocol").and_then(Value::as_str) == Some("tcp")
                    && rules[0]
                        .get("ports")
                        .and_then(Value::as_array)
                        .is_some_and(|ports| ports.len() == 1 && ports[0].as_str() == Some(port))
            })
        && firewall
            .get("targetTags")
            .and_then(Value::as_array)
            .is_some_and(|tags| tags.len() == 1 && tags[0].as_str() == Some(target_tag))
}

fn control_ingress_is_scoped(
    ingress_allow: &Value,
    ingress_deny: &Value,
    control_internal_ip: &str,
    target_tag: &str,
) -> bool {
    let control_cidr = format!("{control_internal_ip}/32");
    let exact_target = |firewall: &Value| {
        firewall
            .get("targetTags")
            .and_then(Value::as_array)
            .is_some_and(|tags| tags.len() == 1 && tags[0].as_str() == Some(target_tag))
    };
    ingress_allow.get("direction").and_then(Value::as_str) == Some("INGRESS")
        && ingress_allow.get("priority").and_then(Value::as_u64) == Some(900)
        && ingress_allow.get("disabled").and_then(Value::as_bool) != Some(true)
        && ingress_allow
            .get("sourceRanges")
            .and_then(Value::as_array)
            .is_some_and(|ranges| {
                ranges.len() == 1 && ranges[0].as_str() == Some(control_cidr.as_str())
            })
        && ingress_allow
            .get("allowed")
            .and_then(Value::as_array)
            .is_some_and(|rules| {
                rules.iter().any(|rule| {
                    rule.get("IPProtocol").and_then(Value::as_str) == Some("tcp")
                        && rule
                            .get("ports")
                            .and_then(Value::as_array)
                            .is_some_and(|ports| {
                                ports.len() == 1 && ports[0].as_str() == Some("22")
                            })
                })
            })
        && exact_target(ingress_allow)
        && ingress_deny.get("direction").and_then(Value::as_str) == Some("INGRESS")
        && ingress_deny.get("priority").and_then(Value::as_u64) == Some(1000)
        && ingress_deny.get("disabled").and_then(Value::as_bool) != Some(true)
        && ingress_deny
            .get("sourceRanges")
            .and_then(Value::as_array)
            .is_some_and(|ranges| ranges.len() == 1 && ranges[0].as_str() == Some("0.0.0.0/0"))
        && exact_target(ingress_deny)
        && ingress_deny
            .get("denied")
            .and_then(Value::as_array)
            .is_some_and(|rules| {
                rules
                    .iter()
                    .any(|rule| rule.get("IPProtocol").and_then(Value::as_str) == Some("all"))
            })
}

impl LiveGceManagedSandboxProvider {
    fn from_env() -> Result<Self, RuntimeError> {
        if optional_env("OA_MANAGED_SANDBOX_PROVISIONER").as_deref() != Some("live_gce") {
            return Err(RuntimeError::new(
                503,
                "live_provider_unavailable",
                "managed-sandbox live GCE provisioner is default-off",
            ));
        }
        if !env_flag("OA_CODEX_GCE_USE_METADATA_ADC")
            || std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_ok()
        {
            return Err(RuntimeError::new(
                503,
                "keyless_control_identity_required",
                "live managed-sandbox provisioning requires the control VM metadata identity and refuses downloadable keys",
            ));
        }
        let required = |key: &str| {
            optional_env(key).ok_or_else(|| {
                RuntimeError::new(
                    503,
                    "live_profile_incomplete",
                    format!("required managed-sandbox setting {key} is absent"),
                )
            })
        };
        Ok(Self {
            config: LiveGceManagedSandboxConfig {
                project_id: required("OA_MANAGED_SANDBOX_PROJECT_ID")?,
                zone: required("OA_MANAGED_SANDBOX_ZONE")?,
                region: required("OA_MANAGED_SANDBOX_REGION")?,
                machine_class: required("OA_MANAGED_SANDBOX_MACHINE_CLASS")?,
                image_project: required("OA_MANAGED_SANDBOX_IMAGE_PROJECT")?,
                image_name: required("OA_MANAGED_SANDBOX_IMAGE_NAME")?,
                image_id: required("OA_MANAGED_SANDBOX_IMAGE_ID")?,
                image_digest: required("OA_MANAGED_SANDBOX_IMAGE_DIGEST")?,
                network: required("OA_MANAGED_SANDBOX_NETWORK")?,
                profile_ref: required("OA_MANAGED_SANDBOX_PROFILE_REF")?,
                profile_digest: required("OA_MANAGED_SANDBOX_PROFILE_DIGEST")?,
                provisioner_ref: required("OA_MANAGED_SANDBOX_PROVISIONER_REF")?,
                network_policy_ref: required("OA_MANAGED_SANDBOX_NETWORK_POLICY_REF")?,
                control_identity_ref: required("OA_MANAGED_SANDBOX_CONTROL_IDENTITY_REF")?,
                control_internal_ip: required("OA_MANAGED_SANDBOX_CONTROL_INTERNAL_IP")?,
                broker_port: required("OA_MANAGED_SANDBOX_PROVIDER_BROKER_PORT")?
                    .parse::<u16>()
                    .map_err(|_| {
                        RuntimeError::new(
                            503,
                            "live_profile_incomplete",
                            "managed-sandbox provider broker port is invalid",
                        )
                    })?,
                gcloud_bin: optional_env("OA_MANAGED_SANDBOX_GCLOUD_BIN")
                    .unwrap_or_else(|| "gcloud".to_string()),
            },
        })
    }

    fn gcloud(&self, args: &[String]) -> Result<String, RuntimeError> {
        let output = Command::new(&self.config.gcloud_bin)
            .args(args)
            .output()
            .map_err(|_| {
                RuntimeError::new(
                    503,
                    "gcloud_unavailable",
                    "GCE control command is unavailable",
                )
            })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(RuntimeError::new(
                503,
                "gcp_operation_failed",
                "the exact GCE provider operation failed",
            ))
        }
    }

    fn instance_args(&self, ownership: &ProviderOwnership) -> Vec<String> {
        vec![
            ownership.resource_name.clone(),
            "--project".to_string(),
            self.config.project_id.clone(),
            "--zone".to_string(),
            self.config.zone.clone(),
        ]
    }

    fn startup_script(ownership: &ProviderOwnership, generation: u64) -> String {
        let marker = short_digest(&format!(
            "{}|{}|{}",
            ownership.resource_ref, ownership.disk_ref, generation
        ));
        format!(
            "#!/bin/sh\nset -eu\numask 077\nfor _ in $(seq 1 60); do\n  if /bin/systemctl is-active --quiet ssh.service && test -x {GUEST_IO_EXECUTABLE}; then break; fi\n  sleep 1\ndone\n/bin/systemctl is-active --quiet ssh.service\ntest -x {GUEST_IO_EXECUTABLE}\n/usr/sbin/iptables -C OUTPUT -d 169.254.169.254/32 -m owner --uid-owner openagents -j REJECT\nprintf 'OA_MSB_READY:{marker}:{generation}\\n' >/dev/ttyS0\nprintf 'OA_MSB_PROBE:{marker}:{generation}\\n' >/dev/ttyS0\n"
        )
    }

    fn guest_io_ready_once(&self, ownership: &ProviderOwnership) -> bool {
        self.gcloud(&guest_io_probe_args(
            &self.config.project_id,
            &self.config.zone,
            &ownership.resource_name,
        ))
        .is_ok()
    }

    fn marker(ownership: &ProviderOwnership, generation: u64, kind: &str) -> String {
        let marker = short_digest(&format!(
            "{}|{}|{}",
            ownership.resource_ref, ownership.disk_ref, generation
        ));
        format!("OA_MSB_{kind}:{marker}:{generation}")
    }

    fn serial_contains_marker(
        &self,
        ownership: &ProviderOwnership,
        generation: u64,
        marker_kind: &str,
    ) -> Result<bool, RuntimeError> {
        let serial = self.gcloud(&[
            "compute".to_string(),
            "instances".to_string(),
            "get-serial-port-output".to_string(),
            ownership.resource_name.clone(),
            "--project".to_string(),
            self.config.project_id.clone(),
            "--zone".to_string(),
            self.config.zone.clone(),
            "--port".to_string(),
            "1".to_string(),
        ])?;
        Ok(serial.contains(&Self::marker(ownership, generation, marker_kind)))
    }

    fn observe(
        &self,
        ownership: &ProviderOwnership,
        generation: u64,
        marker_kind: &str,
    ) -> Result<ReadinessObservation, RuntimeError> {
        let mut args = vec![
            "compute".to_string(),
            "instances".to_string(),
            "describe".to_string(),
        ];
        args.extend(self.instance_args(ownership));
        args.extend([
            "--format".to_string(),
            "json(status,tags.items,networkInterfaces.accessConfigs,serviceAccounts,metadata.items)"
                .to_string(),
        ]);
        let description: Value = serde_json::from_str(&self.gcloud(&args)?).map_err(|_| {
            RuntimeError::new(
                503,
                "provider_observation_invalid",
                "GCE observation was not valid JSON",
            )
        })?;
        let provider_running = description.get("status").and_then(Value::as_str) == Some("RUNNING");
        let no_external_ip = description
            .pointer("/networkInterfaces/0/accessConfigs")
            .and_then(Value::as_array)
            .map(|items| items.is_empty())
            .unwrap_or(true);
        let no_guest_service_account = description
            .get("serviceAccounts")
            .and_then(Value::as_array)
            .map(|items| items.is_empty())
            .unwrap_or(true);
        let metadata = description
            .pointer("/metadata/items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let metadata_value = |key: &str| {
            metadata.iter().find_map(|item| {
                (item.get("key").and_then(Value::as_str) == Some(key))
                    .then(|| item.get("value").and_then(Value::as_str))
                    .flatten()
            })
        };
        let metadata_restricted = metadata_value("block-project-ssh-keys") == Some("TRUE")
            && metadata_value("enable-oslogin") == Some("FALSE")
            && metadata_value("disable-legacy-endpoints") == Some("TRUE");

        let observe_firewall = |name: &str| -> Result<Value, RuntimeError> {
            let output = self.gcloud(&[
                "compute".to_string(),
                "firewall-rules".to_string(),
                "describe".to_string(),
                name.to_string(),
                "--project".to_string(),
                self.config.project_id.clone(),
                "--format".to_string(),
                "json(direction,priority,allowed,denied,sourceRanges,sourceServiceAccounts,destinationRanges,targetTags,disabled)".to_string(),
            ])?;
            serde_json::from_str(&output).map_err(|_| {
                RuntimeError::new(
                    503,
                    "provider_observation_invalid",
                    "GCE firewall observation was not valid JSON",
                )
            })
        };
        let egress_deny = observe_firewall(&ownership.firewall_name)?;
        let egress_default_deny = egress_deny.get("direction").and_then(Value::as_str)
            == Some("EGRESS")
            && egress_deny.get("priority").and_then(Value::as_u64) == Some(1000)
            && egress_deny.get("disabled").and_then(Value::as_bool) != Some(true)
            && egress_deny
                .get("destinationRanges")
                .and_then(Value::as_array)
                .is_some_and(|ranges| {
                    ranges
                        .iter()
                        .any(|range| range.as_str() == Some("0.0.0.0/0"))
                })
            && egress_deny
                .get("denied")
                .and_then(Value::as_array)
                .is_some_and(|rules| {
                    rules
                        .iter()
                        .any(|rule| rule.get("IPProtocol").and_then(Value::as_str) == Some("all"))
                });
        let broker_egress = observe_firewall(&ownership.broker_egress_firewall_name)?;
        let broker_egress_only = egress_tcp_allow_is_scoped(
            &broker_egress,
            &format!("{}/32", self.config.control_internal_ip),
            &self.config.broker_port.to_string(),
            &ownership.resource_name,
        );
        let metadata_egress = observe_firewall(&ownership.metadata_egress_firewall_name)?;
        let metadata_egress_only = egress_tcp_allow_is_scoped(
            &metadata_egress,
            GCE_METADATA_SERVER_CIDR,
            "80",
            &ownership.resource_name,
        );
        let ingress_allow = observe_firewall(&ownership.control_ingress_firewall_name)?;
        let ingress_deny = observe_firewall(&ownership.ingress_deny_firewall_name)?;
        let control_ingress_only = control_ingress_is_scoped(
            &ingress_allow,
            &ingress_deny,
            &self.config.control_internal_ip,
            &ownership.resource_name,
        );

        Ok(ReadinessObservation {
            provider_running,
            guest_marker_observed: self.serial_contains_marker(
                ownership,
                generation,
                marker_kind,
            )?,
            image_admitted: true,
            no_external_ip,
            no_guest_service_account,
            egress_default_deny,
            broker_egress_only,
            metadata_egress_only,
            control_ingress_only,
            metadata_restricted,
        })
    }

    fn wait_for_ready(
        &self,
        ownership: &ProviderOwnership,
        generation: u64,
    ) -> Result<ReadinessObservation, RuntimeError> {
        // Poll only the serial marker while the guest boots. A full observation
        // performs six provider reads and repeating it can exceed the bridge's
        // bounded request timeout before a receipt is returned.
        for _ in 0..45 {
            if self
                .serial_contains_marker(ownership, generation, "READY")
                .unwrap_or(false)
                && self.guest_io_ready_once(ownership)
            {
                return self.observe(ownership, generation, "READY");
            }
            thread::sleep(Duration::from_secs(2));
        }
        let mut observation = self.observe(ownership, generation, "READY")?;
        observation.guest_marker_observed &= self.guest_io_ready_once(ownership);
        Ok(observation)
    }

    fn count_named(
        &self,
        collection: &str,
        name: &str,
        zone_scoped: bool,
    ) -> Result<usize, RuntimeError> {
        if name.is_empty() {
            return Ok(0);
        }
        let mut args = vec![
            "compute".to_string(),
            collection.to_string(),
            "list".to_string(),
        ];
        args.extend(["--project".to_string(), self.config.project_id.clone()]);
        if zone_scoped {
            args.extend(["--zones".to_string(), self.config.zone.clone()]);
        }
        args.extend([
            "--filter".to_string(),
            format!("name={name}"),
            "--format".to_string(),
            "value(name)".to_string(),
        ]);
        Ok(self
            .gcloud(&args)?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count())
    }

    fn delete_named(&self, collection: &str, name: &str, zone_scoped: bool) {
        if name.is_empty() {
            return;
        }
        let mut args = vec![
            "compute".to_string(),
            collection.to_string(),
            "delete".to_string(),
            name.to_string(),
        ];
        args.extend(["--project".to_string(), self.config.project_id.clone()]);
        if zone_scoped {
            args.extend(["--zone".to_string(), self.config.zone.clone()]);
        }
        args.push("--quiet".to_string());
        let _ = self.gcloud(&args);
    }
}

impl ManagedSandboxProvider for LiveGceManagedSandboxProvider {
    fn kind(&self) -> &'static str {
        "live_gce"
    }

    fn admit(&self, profile: &ManagedSandboxRuntimeProfile) -> Result<(), RuntimeError> {
        profile.validate()?;
        let exact = profile.profile_ref == self.config.profile_ref
            && profile.profile_digest == self.config.profile_digest
            && profile.provisioner_ref == self.config.provisioner_ref
            && profile.region == self.config.region
            && profile.machine_class == self.config.machine_class
            && profile.image_digest == self.config.image_digest
            && profile.network_policy_ref == self.config.network_policy_ref
            && profile.control_identity_ref == self.config.control_identity_ref;
        if !exact {
            return Err(RuntimeError::new(
                409,
                "profile_not_admitted",
                "requested runtime profile does not match the exact deployed GCE profile",
            ));
        }
        let descriptor = self.gcloud(&[
            "compute".to_string(),
            "images".to_string(),
            "describe".to_string(),
            self.config.image_name.clone(),
            "--project".to_string(),
            self.config.image_project.clone(),
            "--format".to_string(),
            "json(id,name,status)".to_string(),
        ])?;
        let descriptor: Value = serde_json::from_str(&descriptor).map_err(|_| {
            RuntimeError::new(
                503,
                "image_observation_invalid",
                "GCE image observation was not valid JSON",
            )
        })?;
        let id = descriptor
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let name = descriptor
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status = descriptor
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let observed_digest = format!(
            "sha256:{}",
            full_digest(format!("{}|{}|{}", self.config.image_project, name, id))
        );
        if id != self.config.image_id
            || name != self.config.image_name
            || status != "READY"
            || observed_digest != self.config.image_digest
        {
            return Err(RuntimeError::new(
                409,
                "image_not_admitted",
                "GCE image identity or immutable identity digest did not match admission",
            ));
        }
        Ok(())
    }

    fn plan(&self, sandbox_ref: &str) -> ProviderOwnership {
        let suffix = short_digest(sandbox_ref);
        ProviderOwnership {
            resource_name: format!("oa-msb-{suffix}"),
            firewall_name: format!("oa-msb-egress-{suffix}"),
            broker_egress_firewall_name: format!("oa-msb-broker-{suffix}"),
            metadata_egress_firewall_name: format!("oa-msb-metadata-{suffix}"),
            control_ingress_firewall_name: format!("oa-msb-ssh-{suffix}"),
            ingress_deny_firewall_name: format!("oa-msb-ingress-{suffix}"),
            disk_name: format!("oa-msb-{suffix}"),
            resource_ref: format!(
                "gce-instance-ref://sha256/{}",
                full_digest(format!("resource|{sandbox_ref}"))
            ),
            firewall_ref: format!(
                "gce-firewall-ref://sha256/{}",
                full_digest(format!("firewall|{sandbox_ref}"))
            ),
            disk_ref: format!(
                "gce-disk-ref://sha256/{}",
                full_digest(format!("disk|{sandbox_ref}"))
            ),
        }
    }

    fn count_active(&self) -> Result<u32, RuntimeError> {
        let output = self.gcloud(&[
            "compute".to_string(),
            "instances".to_string(),
            "list".to_string(),
            "--project".to_string(),
            self.config.project_id.clone(),
            "--filter".to_string(),
            LIVE_ACTIVE_SANDBOX_FILTER.to_string(),
            "--format".to_string(),
            "value(name)".to_string(),
        ])?;
        u32::try_from(
            output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
        )
        .map_err(|_| {
            RuntimeError::new(
                503,
                "capacity_observation_failed",
                "capacity count overflowed",
            )
        })
    }

    fn create(
        &self,
        ownership: &ProviderOwnership,
        _profile: &ManagedSandboxRuntimeProfile,
        generation: u64,
    ) -> Result<ReadinessObservation, RuntimeError> {
        self.gcloud(&[
            "compute".to_string(),
            "firewall-rules".to_string(),
            "create".to_string(),
            ownership.broker_egress_firewall_name.clone(),
            "--project".to_string(),
            self.config.project_id.clone(),
            "--direction".to_string(),
            "EGRESS".to_string(),
            "--action".to_string(),
            "ALLOW".to_string(),
            "--rules".to_string(),
            format!("tcp:{}", self.config.broker_port),
            "--priority".to_string(),
            "900".to_string(),
            "--destination-ranges".to_string(),
            format!("{}/32", self.config.control_internal_ip),
            "--target-tags".to_string(),
            ownership.resource_name.clone(),
        ])?;
        self.gcloud(&[
            "compute".to_string(),
            "firewall-rules".to_string(),
            "create".to_string(),
            ownership.metadata_egress_firewall_name.clone(),
            "--project".to_string(),
            self.config.project_id.clone(),
            "--direction".to_string(),
            "EGRESS".to_string(),
            "--action".to_string(),
            "ALLOW".to_string(),
            "--rules".to_string(),
            "tcp:80".to_string(),
            "--priority".to_string(),
            "900".to_string(),
            "--destination-ranges".to_string(),
            GCE_METADATA_SERVER_CIDR.to_string(),
            "--target-tags".to_string(),
            ownership.resource_name.clone(),
        ])?;
        self.gcloud(&[
            "compute".to_string(),
            "firewall-rules".to_string(),
            "create".to_string(),
            ownership.firewall_name.clone(),
            "--project".to_string(),
            self.config.project_id.clone(),
            "--direction".to_string(),
            "EGRESS".to_string(),
            "--action".to_string(),
            "DENY".to_string(),
            "--rules".to_string(),
            "all".to_string(),
            "--priority".to_string(),
            "1000".to_string(),
            "--destination-ranges".to_string(),
            "0.0.0.0/0".to_string(),
            "--target-tags".to_string(),
            ownership.resource_name.clone(),
        ])?;
        self.gcloud(&[
            "compute".to_string(),
            "firewall-rules".to_string(),
            "create".to_string(),
            ownership.control_ingress_firewall_name.clone(),
            "--project".to_string(),
            self.config.project_id.clone(),
            "--direction".to_string(),
            "INGRESS".to_string(),
            "--action".to_string(),
            "ALLOW".to_string(),
            "--rules".to_string(),
            "tcp:22".to_string(),
            "--priority".to_string(),
            "900".to_string(),
            "--source-ranges".to_string(),
            format!("{}/32", self.config.control_internal_ip),
            "--target-tags".to_string(),
            ownership.resource_name.clone(),
        ])?;
        self.gcloud(&[
            "compute".to_string(),
            "firewall-rules".to_string(),
            "create".to_string(),
            ownership.ingress_deny_firewall_name.clone(),
            "--project".to_string(),
            self.config.project_id.clone(),
            "--direction".to_string(),
            "INGRESS".to_string(),
            "--action".to_string(),
            "DENY".to_string(),
            "--rules".to_string(),
            "all".to_string(),
            "--priority".to_string(),
            "1000".to_string(),
            "--source-ranges".to_string(),
            "0.0.0.0/0".to_string(),
            "--target-tags".to_string(),
            ownership.resource_name.clone(),
        ])?;
        let labels = format!(
            "openagents-managed=managed-sandbox,openagents-contract=managed-sandbox-v1,openagents-sandbox-ref=d-{}",
            short_digest(&ownership.resource_ref)
        );
        let metadata = format!(
            "block-project-ssh-keys=TRUE,enable-oslogin=FALSE,disable-legacy-endpoints=TRUE,serial-port-enable=TRUE,startup-script={}",
            Self::startup_script(ownership, generation)
        );
        self.gcloud(&[
            "compute".to_string(),
            "instances".to_string(),
            "create".to_string(),
            ownership.resource_name.clone(),
            "--project".to_string(),
            self.config.project_id.clone(),
            "--zone".to_string(),
            self.config.zone.clone(),
            "--machine-type".to_string(),
            self.config.machine_class.clone(),
            "--image".to_string(),
            self.config.image_name.clone(),
            "--image-project".to_string(),
            self.config.image_project.clone(),
            "--network".to_string(),
            self.config.network.clone(),
            "--no-address".to_string(),
            "--no-service-account".to_string(),
            "--no-scopes".to_string(),
            "--no-restart-on-failure".to_string(),
            "--shielded-secure-boot".to_string(),
            "--shielded-vtpm".to_string(),
            "--shielded-integrity-monitoring".to_string(),
            "--boot-disk-auto-delete".to_string(),
            "--tags".to_string(),
            format!("{},oa-managed-sandbox-guest", ownership.resource_name),
            "--labels".to_string(),
            labels,
            "--metadata".to_string(),
            metadata,
        ])?;
        self.wait_for_ready(ownership, generation)
    }

    fn probe(
        &self,
        ownership: &ProviderOwnership,
        generation: u64,
    ) -> Result<ReadinessObservation, RuntimeError> {
        self.observe(ownership, generation, "PROBE")
    }

    fn stop(&self, ownership: &ProviderOwnership) -> Result<bool, RuntimeError> {
        let mut args = vec![
            "compute".to_string(),
            "instances".to_string(),
            "stop".to_string(),
        ];
        args.extend(self.instance_args(ownership));
        args.push("--quiet".to_string());
        self.gcloud(&args)?;
        let mut describe = vec![
            "compute".to_string(),
            "instances".to_string(),
            "describe".to_string(),
        ];
        describe.extend(self.instance_args(ownership));
        describe.extend(["--format".to_string(), "value(status)".to_string()]);
        Ok(self.gcloud(&describe)?.trim() == "TERMINATED")
    }

    fn resume(
        &self,
        ownership: &ProviderOwnership,
        generation: u64,
    ) -> Result<ReadinessObservation, RuntimeError> {
        let mut metadata = vec![
            "compute".to_string(),
            "instances".to_string(),
            "add-metadata".to_string(),
        ];
        metadata.extend(self.instance_args(ownership));
        metadata.extend([
            "--metadata".to_string(),
            format!(
                "startup-script={}",
                Self::startup_script(ownership, generation)
            ),
        ]);
        self.gcloud(&metadata)?;
        let mut start = vec![
            "compute".to_string(),
            "instances".to_string(),
            "start".to_string(),
        ];
        start.extend(self.instance_args(ownership));
        self.gcloud(&start)?;
        self.wait_for_ready(ownership, generation)
    }

    fn cleanup(&self, ownership: &ProviderOwnership) -> Result<CleanupObservation, RuntimeError> {
        self.delete_named("instances", &ownership.resource_name, true);
        self.delete_named("firewall-rules", &ownership.firewall_name, false);
        self.delete_named(
            "firewall-rules",
            &ownership.broker_egress_firewall_name,
            false,
        );
        self.delete_named(
            "firewall-rules",
            &ownership.metadata_egress_firewall_name,
            false,
        );
        self.delete_named(
            "firewall-rules",
            &ownership.control_ingress_firewall_name,
            false,
        );
        self.delete_named(
            "firewall-rules",
            &ownership.ingress_deny_firewall_name,
            false,
        );
        let mut observation = CleanupObservation {
            zero_grants: true,
            ..CleanupObservation::default()
        };
        for _ in 0..15 {
            observation.zero_compute =
                self.count_named("instances", &ownership.resource_name, true)? == 0;
            observation.zero_firewall = [
                &ownership.firewall_name,
                &ownership.broker_egress_firewall_name,
                &ownership.metadata_egress_firewall_name,
                &ownership.control_ingress_firewall_name,
                &ownership.ingress_deny_firewall_name,
            ]
            .iter()
            .all(|name| {
                self.count_named("firewall-rules", name, false)
                    .is_ok_and(|count| count == 0)
            });
            observation.zero_ingress = [
                &ownership.control_ingress_firewall_name,
                &ownership.ingress_deny_firewall_name,
            ]
            .iter()
            .all(|name| {
                self.count_named("firewall-rules", name, false)
                    .is_ok_and(|count| count == 0)
            });
            observation.zero_scratch = self.count_named("disks", &ownership.disk_name, true)? == 0;
            if observation.is_clean() {
                break;
            }
            thread::sleep(Duration::from_secs(2));
        }
        Ok(observation)
    }
}

fn validate_public_ref(field: &str, value: &str) -> Result<(), RuntimeError> {
    if value.trim().is_empty() || value.len() > 512 || contains_forbidden_material(value) {
        return Err(RuntimeError::new(
            400,
            "invalid_public_ref",
            format!("{field} is empty, oversized, or contains forbidden material"),
        ));
    }
    Ok(())
}

fn contains_forbidden_material(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("access_token")
        || lower.contains("refresh_token")
        || lower.contains("id_token")
        || lower.contains("bearer ")
        || lower.contains("private key")
        || lower.contains("-----begin")
        || lower.contains("google_application_credentials")
        || lower.contains("serviceaccount.com")
        || lower.contains("googleapis.com/compute")
        || lower.contains("projects/")
        || lower.starts_with('/')
        || value.contains("sk-")
}

fn valid_sha256_ref(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|digest| digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()))
}

fn cost_microusd(running_ms: u64, hourly_rate_microusd: u64) -> u64 {
    let numerator = u128::from(running_ms).saturating_mul(u128::from(hourly_rate_microusd));
    let rounded = numerator.saturating_add(3_600_000 - 1) / 3_600_000;
    u64::try_from(rounded).unwrap_or(u64::MAX)
}

fn digest_json<T: Serialize>(value: &T) -> Result<String, RuntimeError> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        RuntimeError::new(
            500,
            "fingerprint_failed",
            "operation fingerprint could not be encoded",
        )
    })?;
    Ok(format!("sha256:{}", full_digest(&bytes)))
}

fn full_digest(value: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_ref());
    format!("{:x}", hasher.finalize())
}

fn runtime_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::validation("private_ingress_field_invalid"))
}

fn runtime_number(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<u64, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| RuntimeError::validation("private_ingress_field_invalid"))
}

fn unix_ms_to_iso(timestamp_ms: u64) -> Result<String, RuntimeError> {
    let seconds = timestamp_ms / 1_000;
    let millis = timestamp_ms % 1_000;
    let days = i64::try_from(seconds / 86_400)
        .map_err(|_| RuntimeError::new(500, "clock_failed", "timestamp exceeds date bounds"))?;
    let second_of_day = seconds % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    if !(0..=9_999).contains(&year) {
        return Err(RuntimeError::new(
            500,
            "clock_failed",
            "timestamp exceeds date bounds",
        ));
    }
    let hour = second_of_day / 3_600;
    let minute = (second_of_day % 3_600) / 60;
    let second = second_of_day % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z"
    ))
}

fn short_digest(value: &str) -> String {
    full_digest(value)[..20].to_string()
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_flag(key: &str) -> bool {
    optional_env(key).is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn now_ms() -> Result<u64, RuntimeError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RuntimeError::new(500, "clock_failed", "system clock is before Unix epoch"))?;
    u64::try_from(duration.as_millis())
        .map_err(|_| RuntimeError::new(500, "clock_failed", "system clock exceeds runtime bounds"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug)]
    struct TestProvider {
        state: Arc<Mutex<TestProviderState>>,
    }

    #[derive(Clone, Debug, Default)]
    struct TestProviderState {
        active: u32,
        fail_create: bool,
        fail_stop: bool,
        fail_resume: bool,
        degraded_cleanup: bool,
        probe_ready: bool,
        create_calls: u32,
        cleanup_calls: u32,
    }

    impl TestProvider {
        fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(TestProviderState {
                    probe_ready: true,
                    ..TestProviderState::default()
                })),
            }
        }

        fn ready() -> ReadinessObservation {
            ReadinessObservation {
                provider_running: true,
                guest_marker_observed: true,
                image_admitted: true,
                no_external_ip: true,
                no_guest_service_account: true,
                egress_default_deny: true,
                broker_egress_only: true,
                metadata_egress_only: true,
                control_ingress_only: true,
                metadata_restricted: true,
            }
        }

        fn clean() -> CleanupObservation {
            CleanupObservation {
                zero_compute: true,
                zero_firewall: true,
                zero_scratch: true,
                zero_ingress: true,
                zero_grants: true,
            }
        }
    }

    impl ManagedSandboxProvider for TestProvider {
        fn kind(&self) -> &'static str {
            "live_gce"
        }
        fn admit(&self, profile: &ManagedSandboxRuntimeProfile) -> Result<(), RuntimeError> {
            profile.validate()
        }
        fn plan(&self, sandbox_ref: &str) -> ProviderOwnership {
            ProviderOwnership {
                resource_name: format!("private-{}", short_digest(sandbox_ref)),
                firewall_name: format!("private-fw-{}", short_digest(sandbox_ref)),
                broker_egress_firewall_name: format!(
                    "private-broker-fw-{}",
                    short_digest(sandbox_ref)
                ),
                metadata_egress_firewall_name: format!(
                    "private-metadata-fw-{}",
                    short_digest(sandbox_ref)
                ),
                control_ingress_firewall_name: format!(
                    "private-ssh-fw-{}",
                    short_digest(sandbox_ref)
                ),
                ingress_deny_firewall_name: format!(
                    "private-ingress-fw-{}",
                    short_digest(sandbox_ref)
                ),
                disk_name: format!("private-disk-{}", short_digest(sandbox_ref)),
                resource_ref: "gce-instance-ref://sha256/abc".to_string(),
                firewall_ref: "gce-firewall-ref://sha256/abc".to_string(),
                disk_ref: "gce-disk-ref://sha256/abc".to_string(),
            }
        }
        fn count_active(&self) -> Result<u32, RuntimeError> {
            Ok(self.state.lock().unwrap().active)
        }
        fn create(
            &self,
            _: &ProviderOwnership,
            _: &ManagedSandboxRuntimeProfile,
            _: u64,
        ) -> Result<ReadinessObservation, RuntimeError> {
            let mut state = self.state.lock().unwrap();
            state.create_calls += 1;
            if state.fail_create {
                return Err(RuntimeError::new(503, "injected", "injected"));
            }
            state.active += 1;
            Ok(Self::ready())
        }
        fn probe(
            &self,
            _: &ProviderOwnership,
            _: u64,
        ) -> Result<ReadinessObservation, RuntimeError> {
            Ok(if self.state.lock().unwrap().probe_ready {
                Self::ready()
            } else {
                ReadinessObservation::default()
            })
        }
        fn stop(&self, _: &ProviderOwnership) -> Result<bool, RuntimeError> {
            Ok(!self.state.lock().unwrap().fail_stop)
        }
        fn resume(
            &self,
            _: &ProviderOwnership,
            _: u64,
        ) -> Result<ReadinessObservation, RuntimeError> {
            if self.state.lock().unwrap().fail_resume {
                Err(RuntimeError::new(503, "injected", "injected"))
            } else {
                Ok(Self::ready())
            }
        }
        fn cleanup(&self, _: &ProviderOwnership) -> Result<CleanupObservation, RuntimeError> {
            let mut state = self.state.lock().unwrap();
            state.cleanup_calls += 1;
            state.active = 0;
            Ok(if state.degraded_cleanup {
                CleanupObservation::default()
            } else {
                Self::clean()
            })
        }
    }

    fn profile() -> ManagedSandboxRuntimeProfile {
        ManagedSandboxRuntimeProfile {
            profile_ref: "profile-ref://openagents/managed-sandbox/gce-e2-small-v1".to_string(),
            profile_digest: format!("sha256:{}", "b".repeat(64)),
            target_ref: "target://openagents/google-cloud/managed-sandbox".to_string(),
            provisioner_ref: "provisioner-ref://openagents/oa-codex-control/gce-v1".to_string(),
            region: "us-central1".to_string(),
            machine_class: "e2-small".to_string(),
            isolation_class: "gce_vm".to_string(),
            image_ref: "gce-image-ref://sha256/example".to_string(),
            image_digest: format!("sha256:{}", "a".repeat(64)),
            network_policy_ref: "network-policy-ref://openagents/managed-sandbox/broker-only-v1"
                .to_string(),
            control_identity_ref: "identity-ref://openagents/managed-sandbox/control".to_string(),
            guest_identity_ref: "identity-ref://openagents/managed-sandbox/guest-none".to_string(),
            ttl_ms: 60_000,
            capacity: CapacityPolicy {
                min_capacity: 0,
                max_capacity: 2,
                prewarm_capacity: 0,
                concurrent_capacity_cap: 2,
            },
            budget: BudgetPolicy {
                sandbox_budget_microusd: 1_000,
                program_budget_microusd: 10_000,
                max_hourly_cost_microusd: 36_000,
            },
            capability_refs: vec!["capability-ref://run/probe".to_string()],
        }
    }

    fn request(
        action: RuntimeAction,
        operation: &str,
        expected_generation: u64,
    ) -> ManagedSandboxRuntimeRequest {
        ManagedSandboxRuntimeRequest {
            operation_ref: format!("operation-ref://{operation}"),
            idempotency_ref: format!("idempotency-ref://{operation}"),
            actor_ref: "principal-ref://owner/test".to_string(),
            owner_ref: "owner-ref://test".to_string(),
            tenant_ref: "tenant-ref://test".to_string(),
            program_ref: "program-ref://managed-sandbox".to_string(),
            work_unit_ref: "work-unit-ref://test".to_string(),
            sandbox_ref: "sandbox-ref://test".to_string(),
            expected_generation,
            action,
            profile: (action == RuntimeAction::Create).then(profile),
        }
    }

    fn temporary_root(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("oa-msb-runtime-{name}-{}", now_ms().unwrap()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn live_capacity_filter_uses_gce_scalar_comparison_syntax() {
        assert_eq!(
            LIVE_ACTIVE_SANDBOX_FILTER,
            "labels.openagents-managed=managed-sandbox AND status!=TERMINATED"
        );
        assert!(!LIVE_ACTIVE_SANDBOX_FILTER.contains("!=("));
    }

    #[test]
    fn ingress_clock_formats_exact_utc_milliseconds() {
        assert_eq!(unix_ms_to_iso(0).unwrap(), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            unix_ms_to_iso(1_753_146_000_123).unwrap(),
            "2025-07-22T01:00:00.123Z"
        );
    }

    #[test]
    fn live_readiness_probes_the_exact_guest_io_path_over_internal_ssh() {
        let args = guest_io_probe_args("project-1", "us-central1-a", "oa-msb-generation");
        assert_eq!(
            &args[..3],
            ["compute", "ssh", "openagents@oa-msb-generation"]
        );
        assert!(args.contains(&"--internal-ip".to_string()));
        assert!(args.contains(&"--ssh-key-expire-after=10m".to_string()));
        assert!(!args.contains(&"--ssh-key-expiration=10m".to_string()));
        assert!(args.contains(&"--ssh-flag=-oBatchMode=yes".to_string()));
        assert!(args.contains(&"--ssh-flag=-oConnectTimeout=5".to_string()));
        assert_eq!(
            args.last().map(String::as_str),
            Some(
                "test -x /opt/openagents-managed-sandbox/managed-sandbox-guest-io.py && test -d /workspace"
            )
        );
    }

    #[test]
    fn metadata_egress_requires_exact_link_local_v1_path() {
        let metadata = json!({
            "direction": "EGRESS",
            "priority": 900,
            "allowed": [{ "IPProtocol": "tcp", "ports": ["80"] }],
            "destinationRanges": ["169.254.169.254/32"],
            "targetTags": ["oa-msb-generation"],
            "disabled": false
        });
        assert!(egress_tcp_allow_is_scoped(
            &metadata,
            GCE_METADATA_SERVER_CIDR,
            "80",
            "oa-msb-generation"
        ));

        let mut broad_destination = metadata.clone();
        broad_destination["destinationRanges"] = json!(["169.254.0.0/16"]);
        assert!(!egress_tcp_allow_is_scoped(
            &broad_destination,
            GCE_METADATA_SERVER_CIDR,
            "80",
            "oa-msb-generation"
        ));

        let mut extra_port = metadata;
        extra_port["allowed"][0]["ports"] = json!(["80", "443"]);
        assert!(!egress_tcp_allow_is_scoped(
            &extra_port,
            GCE_METADATA_SERVER_CIDR,
            "80",
            "oa-msb-generation"
        ));

        let ownership = TestProvider::new().plan("sandbox-ref://metadata-guard");
        let startup = LiveGceManagedSandboxProvider::startup_script(&ownership, 7);
        assert!(startup.contains(
            "iptables -C OUTPUT -d 169.254.169.254/32 -m owner --uid-owner openagents -j REJECT"
        ));
        assert!(startup.contains("systemctl is-active --quiet ssh.service"));
        assert!(startup.contains(GUEST_IO_EXECUTABLE));
        assert!(startup.find("ssh.service").unwrap() < startup.find("OA_MSB_READY").unwrap());
        assert!(startup.find("iptables -C").unwrap() < startup.find("OA_MSB_READY").unwrap());
    }

    #[test]
    fn control_ingress_requires_exact_reserved_ip_and_generation_target() {
        let allow = json!({
            "direction": "INGRESS",
            "priority": 900,
            "allowed": [{ "IPProtocol": "tcp", "ports": ["22"] }],
            "sourceRanges": ["10.128.15.196/32"],
            "targetTags": ["oa-msb-generation"],
            "disabled": false
        });
        let deny = json!({
            "direction": "INGRESS",
            "priority": 1000,
            "denied": [{ "IPProtocol": "all" }],
            "sourceRanges": ["0.0.0.0/0"],
            "targetTags": ["oa-msb-generation"],
            "disabled": false
        });
        assert!(control_ingress_is_scoped(
            &allow,
            &deny,
            "10.128.15.196",
            "oa-msb-generation"
        ));

        let mut service_account_source = allow.clone();
        service_account_source["sourceRanges"] = Value::Null;
        service_account_source["sourceServiceAccounts"] =
            json!(["oa-codex-control@example.iam.gserviceaccount.com"]);
        assert!(!control_ingress_is_scoped(
            &service_account_source,
            &deny,
            "10.128.15.196",
            "oa-msb-generation"
        ));

        let mut broad_source = allow.clone();
        broad_source["sourceRanges"] = json!(["10.128.0.0/20"]);
        assert!(!control_ingress_is_scoped(
            &broad_source,
            &deny,
            "10.128.15.196",
            "oa-msb-generation"
        ));

        let mut wrong_target = allow;
        wrong_target["targetTags"] = json!(["oa-msb-other-generation"]);
        assert!(!control_ingress_is_scoped(
            &wrong_target,
            &deny,
            "10.128.15.196",
            "oa-msb-generation"
        ));
    }

    #[test]
    fn create_probe_stop_resume_delete_is_generation_fenced_and_refs_only() {
        let root = temporary_root("lifecycle");
        let provider = TestProvider::new();
        let create = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        assert_eq!(create.phase, RuntimePhase::Ready);
        assert!(create.readiness_observed);
        assert_eq!(create.generation, 1);

        let probe = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Probe, "probe", 1),
            2_000,
        )
        .unwrap();
        assert!(probe.readiness_observed);
        let stop = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Stop, "stop", 1),
            3_000,
        )
        .unwrap();
        assert_eq!(stop.phase, RuntimePhase::Stopped);
        let resume = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Resume, "resume", 1),
            4_000,
        )
        .unwrap();
        assert_eq!(resume.phase, RuntimePhase::Ready);
        assert_eq!(resume.generation, 2);
        let stop2 = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Stop, "stop2", 2),
            5_000,
        )
        .unwrap();
        assert_eq!(stop2.phase, RuntimePhase::Stopped);
        let delete = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Delete, "delete", 2),
            6_000,
        )
        .unwrap();
        assert_eq!(delete.phase, RuntimePhase::Deleted);
        assert!(delete.cleanup_observed);
        let json = serde_json::to_string(&delete).unwrap().to_ascii_lowercase();
        assert!(!json.contains("private-"));
        assert!(!json.contains("serviceaccount.com"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checkpoint_stop_uses_durable_scope_and_replays_after_stop() {
        let root = temporary_root("checkpoint-stop");
        let provider = TestProvider::new();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();

        let stale = stop_after_checkpoint_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            2,
            "stop-ref://checkpoint",
            2_000,
        )
        .unwrap_err();
        assert_eq!(stale.code, "generation_conflict");
        let wrong_owner = stop_after_checkpoint_with_provider(
            &root,
            &provider,
            "owner-ref://other",
            "tenant-ref://test",
            "sandbox-ref://test",
            1,
            "stop-ref://checkpoint",
            2_000,
        )
        .unwrap_err();
        assert_eq!(wrong_owner.code, "scope_mismatch");

        let stopped = stop_after_checkpoint_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            1,
            "stop-ref://checkpoint",
            3_000,
        )
        .unwrap();
        assert_eq!(stopped.phase, RuntimePhase::Stopped);
        let replay = stop_after_checkpoint_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            1,
            "stop-ref://checkpoint",
            9_000,
        )
        .unwrap();
        assert_eq!(replay, stopped);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checkpoint_stop_failure_persists_recovery_required() {
        let root = temporary_root("checkpoint-stop-failure");
        let provider = TestProvider::new();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        provider.state.lock().unwrap().fail_stop = true;

        let receipt = stop_after_checkpoint_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            1,
            "stop-ref://checkpoint-failure",
            3_000,
        )
        .unwrap();
        assert_eq!(receipt.phase, RuntimePhase::RecoveryRequired);
        assert_eq!(
            receipt.error_code.as_deref(),
            Some("stop_recovery_required")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checkpoint_restore_prepares_fresh_runtime_grants_and_replays() {
        let root = temporary_root("checkpoint-restore");
        let provider = TestProvider::new();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Stop, "stop", 1),
            2_000,
        )
        .unwrap();
        let source_capabilities = vec!["capability.sbx10.source".to_string()];
        let context = prepare_checkpoint_restore_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            "command.sbx10.restore",
            "checkpoint.sbx10.restore",
            1,
            &source_capabilities,
            3_000,
        )
        .unwrap();
        assert_eq!(context.resource_generation, 2);
        assert_eq!(context.restored_capability_refs.len(), 1);
        assert!(!source_capabilities.contains(&context.restored_capability_refs[0]));
        let journal = load_journal(&journal_path(&root, "sandbox-ref://test"))
            .unwrap()
            .unwrap();
        assert_eq!(journal.phase, RuntimePhase::Ready);
        assert_eq!(journal.profile.capability_refs.len(), 1);
        assert!(journal.profile.capability_refs[0].starts_with("capability-ref://run/restore-"));
        assert!(journal.pending_checkpoint_restore.is_some());
        let blocked = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Probe, "probe-during-restore", 2),
            4_000,
        )
        .unwrap_err();
        assert_eq!(blocked.code, "restore_in_progress");

        let replay = prepare_checkpoint_restore_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            "command.sbx10.restore",
            "checkpoint.sbx10.restore",
            1,
            &source_capabilities,
            9_000,
        )
        .unwrap();
        assert_eq!(replay, context);
        finish_checkpoint_restore(
            &root,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            "command.sbx10.restore",
            true,
        )
        .unwrap();
        assert!(load_journal(&journal_path(&root, "sandbox-ref://test"))
            .unwrap()
            .unwrap()
            .pending_checkpoint_restore
            .is_some_and(|pending| pending.completed));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checkpoint_restore_resume_failure_requires_recovery() {
        let root = temporary_root("checkpoint-restore-failure");
        let provider = TestProvider::new();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Stop, "stop", 1),
            2_000,
        )
        .unwrap();
        provider.state.lock().unwrap().fail_resume = true;

        let error = prepare_checkpoint_restore_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            "command.sbx10.restore-failure",
            "checkpoint.sbx10.restore-failure",
            1,
            &[],
            3_000,
        )
        .unwrap_err();
        assert_eq!(error.code, "restore_recovery_required");
        let journal = load_journal(&journal_path(&root, "sandbox-ref://test"))
            .unwrap()
            .unwrap();
        assert_eq!(journal.phase, RuntimePhase::RecoveryRequired);
        assert!(journal.pending_checkpoint_restore.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checkpoint_fork_creates_a_new_runtime_with_fresh_grants_and_replays() {
        let root = temporary_root("checkpoint-fork");
        let provider = TestProvider::new();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        stop_after_checkpoint_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            1,
            "stop-ref://checkpoint",
            1_500,
        )
        .unwrap();
        let source_capabilities = vec!["capability-ref://run/probe".to_string()];
        let context = prepare_checkpoint_fork_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            1,
            "command.sbx10.fork",
            "checkpoint.sbx10.fork",
            &source_capabilities,
            2_000,
        )
        .unwrap();
        assert_ne!(context.fork_sandbox_ref, "sandbox-ref://test");
        assert_eq!(context.fork_resource_generation, 1);
        assert_eq!(context.fork_capability_refs.len(), 1);
        assert!(!source_capabilities.contains(&context.fork_capability_refs[0]));
        let fork_path = journal_path(&root, &context.fork_sandbox_ref);
        let mut fork_journal = load_journal(&fork_path).unwrap().unwrap();
        assert_eq!(fork_journal.phase, RuntimePhase::Ready);
        assert!(fork_journal.profile.capability_refs[0].starts_with("capability-ref://run/fork-"));
        assert!(fork_journal.pending_checkpoint_fork.is_some());

        // Simulate a process stop after provider create and before lifecycle settlement.
        fork_journal.pending_checkpoint_fork = None;
        fork_journal.phase = RuntimePhase::Provisioning;
        save_journal(&fork_path, &fork_journal).unwrap();

        let replay = prepare_checkpoint_fork_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            1,
            "command.sbx10.fork",
            "checkpoint.sbx10.fork",
            &source_capabilities,
            9_000,
        )
        .unwrap();
        assert_eq!(replay, context);
        assert_eq!(provider.state.lock().unwrap().create_calls, 2);

        finish_checkpoint_fork_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            &context.fork_sandbox_ref,
            "command.sbx10.fork",
            true,
            10_000,
        )
        .unwrap();
        assert!(
            load_journal(&journal_path(&root, &context.fork_sandbox_ref))
                .unwrap()
                .unwrap()
                .pending_checkpoint_fork
                .is_some_and(|pending| pending.completed)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_checkpoint_fork_deletes_the_new_runtime() {
        let root = temporary_root("checkpoint-fork-failure");
        let provider = TestProvider::new();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        stop_after_checkpoint_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            1,
            "stop-ref://checkpoint-failure",
            1_500,
        )
        .unwrap();
        let source_capabilities = vec!["capability-ref://run/probe".to_string()];
        let context = prepare_checkpoint_fork_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            "sandbox-ref://test",
            1,
            "command.sbx10.fork-failure",
            "checkpoint.sbx10.fork-failure",
            &source_capabilities,
            2_000,
        )
        .unwrap();
        finish_checkpoint_fork_with_provider(
            &root,
            &provider,
            "owner-ref://test",
            "tenant-ref://test",
            &context.fork_sandbox_ref,
            "command.sbx10.fork-failure",
            false,
            3_000,
        )
        .unwrap();
        let journal = load_journal(&journal_path(&root, &context.fork_sandbox_ref))
            .unwrap()
            .unwrap();
        assert_eq!(journal.phase, RuntimePhase::Deleted);
        assert!(journal.cleanup.is_clean());
        let _ = fs::remove_dir_all(root);
    }

    fn ingress_request(
        action: ManagedSandboxPhase2Action,
        suffix: &str,
        capability: Option<Value>,
    ) -> ManagedSandboxPhase2Request {
        let command = match action {
            ManagedSandboxPhase2Action::CreatePrivateIngress => json!({
                "_tag": "CreatePrivateIngress",
                "schema": "openagents.managed_sandbox_phase2_command.v1",
                "commandRef": format!("command.sbx10.ingress.{suffix}"),
                "idempotencyRef": format!("idempotency.sbx10.ingress.{suffix}"),
                "ownerRef": "owner-ref://test",
                "tenantRef": "tenant-ref://test",
                "requestedAt": "2026-07-22T01:00:00.000Z",
                "sandboxRef": "sandbox-ref://test",
                "resourceGeneration": 1,
                "audienceRef": "audience-ref://owner/device-1",
                "kind": "preview",
                "ttlSeconds": 300
            }),
            ManagedSandboxPhase2Action::RevokePrivateIngress
            | ManagedSandboxPhase2Action::ExpirePrivateIngress => {
                let active = capability.as_ref().unwrap();
                json!({
                    "_tag": if action == ManagedSandboxPhase2Action::RevokePrivateIngress {
                        "RevokePrivateIngress"
                    } else {
                        "ExpirePrivateIngress"
                    },
                    "schema": "openagents.managed_sandbox_phase2_command.v1",
                    "commandRef": format!("command.sbx10.ingress.{suffix}"),
                    "idempotencyRef": format!("idempotency.sbx10.ingress.{suffix}"),
                    "ownerRef": "owner-ref://test",
                    "tenantRef": "tenant-ref://test",
                    "requestedAt": "2026-07-22T01:01:00.000Z",
                    "capabilityRef": active["capabilityRef"],
                    "sandboxRef": "sandbox-ref://test",
                    "resourceGeneration": 1
                })
            }
            _ => unreachable!("private ingress request helper"),
        };
        ManagedSandboxPhase2Request {
            schema_version: "openagents.managed_sandbox_phase2_target.v1".to_string(),
            action,
            request_ref: command["commandRef"].as_str().unwrap().to_string(),
            command: Some(command),
            checkpoint: None,
            capability,
            owner_ref: None,
            tenant_ref: None,
            sandbox_ref: None,
        }
    }

    #[test]
    fn private_ingress_is_generation_fenced_replayable_and_digest_only() {
        let root = temporary_root("private-ingress");
        let provider = TestProvider::new();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        let create = ingress_request(
            ManagedSandboxPhase2Action::CreatePrivateIngress,
            "create",
            None,
        );
        let active = execute_private_ingress_at(&root, &create, 1_753_146_000_000).unwrap();
        let replay = execute_private_ingress_at(&root, &create, 1_753_146_010_000).unwrap();
        assert_eq!(replay, active);
        assert_eq!(active["_tag"], "Active");
        assert_eq!(active["resourceGeneration"], 1);
        assert_eq!(active["ttlSeconds"], 300);
        assert_eq!(active["accessUrlAtRest"], "redacted");
        assert_eq!(active["publicAccess"], false);
        assert_eq!(active["permanentRoute"], false);
        assert_eq!(active["vnc"], "unsupported");
        let encoded = serde_json::to_string(&active).unwrap();
        assert!(!encoded.contains("https://"));
        assert!(!encoded.contains("10.128."));
        assert!(!encoded.contains("gce"));

        let mut conflict = create.clone();
        conflict.command.as_mut().unwrap()["audienceRef"] = json!("audience-ref://other/device");
        assert_eq!(
            execute_private_ingress_at(&root, &conflict, 1_753_146_020_000)
                .unwrap_err()
                .code,
            "idempotency_conflict"
        );
        let revoke = ingress_request(
            ManagedSandboxPhase2Action::RevokePrivateIngress,
            "revoke",
            Some(active.clone()),
        );
        let cleaned = execute_private_ingress_at(&root, &revoke, 1_753_146_060_000).unwrap();
        assert_eq!(cleaned["_tag"], "Cleaned");
        assert_eq!(cleaned["terminalState"], "revoked");
        assert_eq!(cleaned["auditRefs"].as_array().unwrap().len(), 2);
        assert_eq!(
            execute_private_ingress_at(&root, &revoke, 1_753_146_070_000).unwrap(),
            cleaned
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn private_preview_authorization_binds_audience_bytes_generation_and_revocation() {
        let root = temporary_root("private-preview-authorization");
        let provider = TestProvider::new();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        let create = ingress_request(
            ManagedSandboxPhase2Action::CreatePrivateIngress,
            "preview-create",
            None,
        );
        let start = 1_753_146_000_000;
        let active = execute_private_ingress_at(&root, &create, start).unwrap();
        let authorized = authorize_private_preview_at(
            &root,
            &active,
            "audience-ref://owner/device-1",
            "/workspace/preview.html",
            "utf8",
            "operation.sbx10.preview.test",
            start + 1_000,
        )
        .unwrap();
        assert_eq!(authorized.action, GuestIoAction::ReadFile);
        assert_eq!(authorized.resource_generation, 1);
        assert_eq!(authorized.capability_ref, active["capabilityRef"]);
        assert_eq!(authorized.actor_ref, "audience-ref://owner/device-1");
        assert_eq!(authorized.path.as_deref(), Some("workspace/preview.html"));
        assert_eq!(authorized.limits.max_network_bytes, 0);
        assert_eq!(
            authorized.limits.network_policy_ref,
            "network-policy.managed-sandbox.deny-all"
        );

        assert_eq!(
            authorize_private_preview_at(
                &root,
                &active,
                "audience-ref://owner/device-1",
                "/workspace/../private",
                "utf8",
                "operation.sbx10.preview.traversal",
                start + 2_000,
            )
            .unwrap_err()
            .code,
            "private_preview_path_invalid"
        );
        assert_eq!(
            authorize_private_preview_at(
                &root,
                &active,
                "audience-ref://other/device",
                "/workspace/preview.html",
                "utf8",
                "operation.sbx10.preview.other",
                start + 2_000,
            )
            .unwrap_err()
            .code,
            "private_preview_scope_mismatch"
        );
        let mut altered = active.clone();
        altered["accessUrlDigest"] = json!(format!("sha256:{}", "0".repeat(64)));
        assert_eq!(
            authorize_private_preview_at(
                &root,
                &altered,
                "audience-ref://owner/device-1",
                "/workspace/preview.html",
                "utf8",
                "operation.sbx10.preview.altered",
                start + 3_000,
            )
            .unwrap_err()
            .code,
            "private_preview_capability_conflict"
        );

        let revoke = ingress_request(
            ManagedSandboxPhase2Action::RevokePrivateIngress,
            "preview-revoke",
            Some(active.clone()),
        );
        execute_private_ingress_at(&root, &revoke, start + 4_000).unwrap();
        assert_eq!(
            authorize_private_preview_at(
                &root,
                &active,
                "audience-ref://owner/device-1",
                "/workspace/preview.html",
                "utf8",
                "operation.sbx10.preview.revoked",
                start + 5_000,
            )
            .unwrap_err()
            .code,
            "private_preview_revoked"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn private_ingress_expiry_and_delete_fail_closed() {
        let root = temporary_root("private-ingress-expiry");
        let provider = TestProvider::new();
        execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        let create = ingress_request(
            ManagedSandboxPhase2Action::CreatePrivateIngress,
            "expiry-create",
            None,
        );
        let start = 1_753_146_000_000;
        let active = execute_private_ingress_at(&root, &create, start).unwrap();
        assert_eq!(
            authorize_private_preview_at(
                &root,
                &active,
                "audience-ref://owner/device-1",
                "/workspace/preview.html",
                "utf8",
                "operation.sbx10.preview.expired",
                start + 300_000,
            )
            .unwrap_err()
            .code,
            "private_preview_expired"
        );
        let delete = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Delete, "delete-with-ingress", 1),
            start + 1_000,
        )
        .unwrap_err();
        assert_eq!(delete.code, "private_ingress_active");
        let expire = ingress_request(
            ManagedSandboxPhase2Action::ExpirePrivateIngress,
            "expire",
            Some(active),
        );
        assert_eq!(
            execute_private_ingress_at(&root, &expire, start + 299_999)
                .unwrap_err()
                .code,
            "private_ingress_not_expired"
        );
        let cleaned = execute_private_ingress_at(&root, &expire, start + 300_000).unwrap();
        assert_eq!(cleaned["terminalState"], "expired");
        let deleted = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Delete, "delete-after-expiry", 1),
            start + 301_000,
        )
        .unwrap();
        assert_eq!(deleted.phase, RuntimePhase::Deleted);
        assert!(deleted.cleanup_observed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ready_resource_can_be_deleted_without_a_prior_stop() {
        let root = temporary_root("delete-ready");
        let provider = TestProvider::new();
        let create = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        assert_eq!(create.phase, RuntimePhase::Ready);

        let delete = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Delete, "delete-ready", 1),
            2_000,
        )
        .unwrap();
        assert_eq!(delete.phase, RuntimePhase::Deleted);
        assert!(delete.cleanup_observed);
        assert_eq!(provider.state.lock().unwrap().cleanup_calls, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exact_retry_is_replayed_without_second_provider_effect() {
        let root = temporary_root("replay");
        let provider = TestProvider::new();
        let create_request = request(RuntimeAction::Create, "create", 0);
        let first = execute_with_provider(&root, &provider, create_request.clone(), 1_000).unwrap();
        let second = execute_with_provider(&root, &provider, create_request, 9_000).unwrap();
        assert_eq!(first, second);
        assert_eq!(provider.state.lock().unwrap().create_calls, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn idempotency_byte_drift_and_generation_drift_refuse() {
        let root = temporary_root("fences");
        let provider = TestProvider::new();
        let original = request(RuntimeAction::Create, "create", 0);
        execute_with_provider(&root, &provider, original.clone(), 1_000).unwrap();
        let mut drift = original;
        drift.actor_ref = "principal-ref://owner/drift".to_string();
        assert_eq!(
            execute_with_provider(&root, &provider, drift, 2_000)
                .unwrap_err()
                .status(),
            409
        );
        assert_eq!(
            execute_with_provider(
                &root,
                &provider,
                request(RuntimeAction::Stop, "stop", 9),
                2_000
            )
            .unwrap_err()
            .status(),
            409
        );
        let mut wrong_scope = request(RuntimeAction::Probe, "wrong-scope", 1);
        wrong_scope.owner_ref = "owner-ref://different".to_string();
        assert_eq!(
            execute_with_provider(&root, &provider, wrong_scope, 2_000)
                .unwrap_err()
                .status(),
            403
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn partial_create_cleans_or_reports_recovery_required() {
        let root = temporary_root("partial-clean");
        let provider = TestProvider::new();
        provider.state.lock().unwrap().fail_create = true;
        let clean_failure = execute_with_provider(
            &root,
            &provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        assert_eq!(clean_failure.phase, RuntimePhase::Failed);
        assert!(clean_failure.cleanup_observed);

        let root2 = temporary_root("partial-recovery");
        let provider2 = TestProvider::new();
        {
            let mut state = provider2.state.lock().unwrap();
            state.fail_create = true;
            state.degraded_cleanup = true;
        }
        let recovery = execute_with_provider(
            &root2,
            &provider2,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        assert_eq!(recovery.phase, RuntimePhase::RecoveryRequired);
        assert!(!recovery.cleanup_observed);
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(root2);
    }

    #[test]
    fn capacity_budget_capability_and_fake_readiness_fail_closed() {
        let root = temporary_root("admission");
        let provider = TestProvider::new();
        provider.state.lock().unwrap().active = 2;
        assert_eq!(
            execute_with_provider(
                &root,
                &provider,
                request(RuntimeAction::Create, "create", 0),
                1_000
            )
            .unwrap_err()
            .status(),
            409
        );

        let budget_root = temporary_root("program-budget");
        let budget_provider = TestProvider::new();
        budget_provider.state.lock().unwrap().active = 1;
        let mut over_program_budget = request(RuntimeAction::Create, "budget", 0);
        over_program_budget
            .profile
            .as_mut()
            .unwrap()
            .budget
            .program_budget_microusd = 1_000;
        assert_eq!(
            execute_with_provider(&budget_root, &budget_provider, over_program_budget, 1_000)
                .unwrap_err()
                .status(),
            409
        );

        let mut invalid = request(RuntimeAction::Create, "invalid", 0);
        invalid.profile.as_mut().unwrap().capability_refs =
            vec!["capability-ref://ambient/provider".to_string()];
        assert_eq!(
            execute_with_provider(&root, &provider, invalid, 1_000)
                .unwrap_err()
                .status(),
            400
        );

        struct FakeProvider(TestProvider);
        impl ManagedSandboxProvider for FakeProvider {
            fn kind(&self) -> &'static str {
                "fake"
            }
            fn admit(&self, p: &ManagedSandboxRuntimeProfile) -> Result<(), RuntimeError> {
                self.0.admit(p)
            }
            fn plan(&self, s: &str) -> ProviderOwnership {
                self.0.plan(s)
            }
            fn count_active(&self) -> Result<u32, RuntimeError> {
                Ok(0)
            }
            fn create(
                &self,
                o: &ProviderOwnership,
                p: &ManagedSandboxRuntimeProfile,
                g: u64,
            ) -> Result<ReadinessObservation, RuntimeError> {
                self.0.create(o, p, g)
            }
            fn probe(
                &self,
                o: &ProviderOwnership,
                g: u64,
            ) -> Result<ReadinessObservation, RuntimeError> {
                self.0.probe(o, g)
            }
            fn stop(&self, o: &ProviderOwnership) -> Result<bool, RuntimeError> {
                self.0.stop(o)
            }
            fn resume(
                &self,
                o: &ProviderOwnership,
                g: u64,
            ) -> Result<ReadinessObservation, RuntimeError> {
                self.0.resume(o, g)
            }
            fn cleanup(&self, o: &ProviderOwnership) -> Result<CleanupObservation, RuntimeError> {
                self.0.cleanup(o)
            }
        }
        let fake = FakeProvider(TestProvider::new());
        assert_eq!(
            execute_with_provider(
                &root,
                &fake,
                request(RuntimeAction::Create, "fake", 0),
                1_000
            )
            .unwrap_err()
            .status(),
            503
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(budget_root);
    }

    #[test]
    fn probe_stop_and_resume_faults_never_report_ready_and_reconcile_cleanup() {
        let probe_root = temporary_root("probe-fault");
        let probe_provider = TestProvider::new();
        execute_with_provider(
            &probe_root,
            &probe_provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        probe_provider.state.lock().unwrap().probe_ready = false;
        let probe = execute_with_provider(
            &probe_root,
            &probe_provider,
            request(RuntimeAction::Probe, "probe", 1),
            2_000,
        )
        .unwrap();
        assert_eq!(probe.phase, RuntimePhase::RecoveryRequired);
        assert!(!probe.readiness_observed);
        let reconciled = execute_with_provider(
            &probe_root,
            &probe_provider,
            request(RuntimeAction::Reconcile, "reconcile", 1),
            3_000,
        )
        .unwrap();
        assert_eq!(reconciled.phase, RuntimePhase::Deleted);
        assert!(reconciled.cleanup_observed);

        let stop_root = temporary_root("stop-fault");
        let stop_provider = TestProvider::new();
        execute_with_provider(
            &stop_root,
            &stop_provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        stop_provider.state.lock().unwrap().fail_stop = true;
        let stop = execute_with_provider(
            &stop_root,
            &stop_provider,
            request(RuntimeAction::Stop, "stop", 1),
            2_000,
        )
        .unwrap();
        assert_eq!(stop.phase, RuntimePhase::RecoveryRequired);
        assert!(!stop.readiness_observed);

        let resume_root = temporary_root("resume-fault");
        let resume_provider = TestProvider::new();
        execute_with_provider(
            &resume_root,
            &resume_provider,
            request(RuntimeAction::Create, "create", 0),
            1_000,
        )
        .unwrap();
        execute_with_provider(
            &resume_root,
            &resume_provider,
            request(RuntimeAction::Stop, "stop", 1),
            2_000,
        )
        .unwrap();
        resume_provider.state.lock().unwrap().fail_resume = true;
        let resume = execute_with_provider(
            &resume_root,
            &resume_provider,
            request(RuntimeAction::Resume, "resume", 1),
            3_000,
        )
        .unwrap();
        assert_eq!(resume.phase, RuntimePhase::RecoveryRequired);
        assert!(!resume.readiness_observed);
        assert_eq!(resume.generation, 2);

        let _ = fs::remove_dir_all(probe_root);
        let _ = fs::remove_dir_all(stop_root);
        let _ = fs::remove_dir_all(resume_root);
    }
}
