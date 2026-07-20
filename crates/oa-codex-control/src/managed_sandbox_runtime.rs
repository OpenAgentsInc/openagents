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
