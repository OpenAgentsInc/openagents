//! Fail-closed adapter for managed-sandbox Phase 2 checkpoint effects.
//!
//! The configured executable owns the Google Cloud and guest implementation.
//! This adapter validates the exact private wire contract, bounds the driver,
//! and never invokes a shell on the control host.

use std::collections::HashSet;
use std::env;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::managed_sandbox_runtime::{self, RuntimePhase};

const TARGET_SCHEMA_VERSION: &str = "openagents.managed_sandbox_phase2_target.v1";
const DRIVER_ERROR_SCHEMA_VERSION: &str = "openagents.managed_sandbox_phase2_driver_error.v1";
const COMMAND_SCHEMA_VERSION: &str = "openagents.managed_sandbox_phase2_command.v1";
const CHECKPOINT_SCHEMA_VERSION: &str = "openagents.managed_sandbox_content_checkpoint.v1";
const CHECKPOINT_STOP_SCHEMA_VERSION: &str = "openagents.managed_sandbox_checkpoint_stop.v1";
const CHECKPOINT_DELETE_SCHEMA_VERSION: &str =
    "openagents.managed_sandbox_checkpoint_delete_receipt.v1";
const FORK_RECEIPT_SCHEMA_VERSION: &str = "openagents.managed_sandbox_fork_receipt.v1";
const RESTORE_RECEIPT_SCHEMA_VERSION: &str = "openagents.managed_sandbox_restore_receipt.v1";
const MAX_DRIVER_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_DRIVER_DURATION: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedSandboxPhase2Action {
    CreateCheckpoint,
    ArchiveWithCheckpoint,
    VerifyCheckpoint,
    ObserveResourceGeneration,
    ForkFromCheckpoint,
    RestoreCheckpoint,
    DeleteCheckpoint,
    CreatePrivateIngress,
    RevokePrivateIngress,
    ExpirePrivateIngress,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedSandboxPhase2Request {
    pub schema_version: String,
    pub action: ManagedSandboxPhase2Action,
    pub request_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedSandboxPhase2Response {
    pub schema_version: String,
    pub action: ManagedSandboxPhase2Action,
    pub request_ref: String,
    pub result: Value,
}

#[derive(Debug)]
pub struct Phase2Error {
    status: u16,
    code: &'static str,
    reason_ref: String,
}

impl Phase2Error {
    fn new(status: u16, code: &'static str, reason_ref: impl Into<String>) -> Self {
        Self {
            status,
            code,
            reason_ref: reason_ref.into(),
        }
    }

    fn invalid(reason_ref: impl Into<String>) -> Self {
        Self::new(400, "invalid_request", reason_ref)
    }

    fn conflict(reason_ref: impl Into<String>) -> Self {
        Self::new(409, "phase2_scope_conflict", reason_ref)
    }

    fn unavailable(reason_ref: impl Into<String>) -> Self {
        Self::new(503, "phase2_unavailable", reason_ref)
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn response(&self) -> Value {
        json!({
            "schemaVersion": "openagents.managed_sandbox_phase2_target_error.v1",
            "code": self.code,
            "reasonRef": self.reason_ref,
            "retryable": self.status >= 500,
        })
    }
}

impl ManagedSandboxPhase2Request {
    fn validate(&self) -> Result<(), Phase2Error> {
        if self.schema_version != TARGET_SCHEMA_VERSION {
            return Err(Phase2Error::invalid("phase2_target_schema_invalid"));
        }
        validate_ref(&self.request_ref)?;
        let encoded = serde_json::to_vec(self)
            .map_err(|_| Phase2Error::invalid("phase2_request_encode_failed"))?;
        if forbidden_private_material(&encoded) {
            return Err(Phase2Error::invalid(
                "phase2_request_contains_private_material",
            ));
        }

        match self.action {
            ManagedSandboxPhase2Action::CreateCheckpoint => {
                self.require_shape(true, false, false, false)?;
                let command = self.command_object("CreateCheckpoint")?;
                validate_create_command(command)?;
                require_equal(
                    &self.request_ref,
                    string(command, "commandRef")?,
                    "phase2_request_ref_conflict",
                )
            }
            ManagedSandboxPhase2Action::ArchiveWithCheckpoint => {
                self.require_shape(true, false, false, false)?;
                let command = self.command_object("ArchiveWithCheckpoint")?;
                validate_archive_command(command)?;
                require_equal(
                    &self.request_ref,
                    string(command, "commandRef")?,
                    "phase2_request_ref_conflict",
                )
            }
            ManagedSandboxPhase2Action::VerifyCheckpoint => {
                self.require_shape(false, true, false, false)?;
                let checkpoint = self.checkpoint_object()?;
                validate_checkpoint(checkpoint)?;
                require_equal(
                    &self.request_ref,
                    string(checkpoint, "checkpointRef")?,
                    "phase2_request_ref_conflict",
                )
            }
            ManagedSandboxPhase2Action::ObserveResourceGeneration => {
                self.require_shape(false, false, false, true)?;
                let owner_ref = self
                    .owner_ref
                    .as_deref()
                    .ok_or_else(|| Phase2Error::invalid("phase2_owner_ref_missing"))?;
                let tenant_ref = self
                    .tenant_ref
                    .as_deref()
                    .ok_or_else(|| Phase2Error::invalid("phase2_tenant_ref_missing"))?;
                let sandbox_ref = self
                    .sandbox_ref
                    .as_deref()
                    .ok_or_else(|| Phase2Error::invalid("phase2_sandbox_ref_missing"))?;
                for value in [owner_ref, tenant_ref, sandbox_ref] {
                    validate_ref(value)?;
                }
                require_equal(
                    &self.request_ref,
                    sandbox_ref,
                    "phase2_request_ref_conflict",
                )
            }
            ManagedSandboxPhase2Action::ForkFromCheckpoint => {
                self.require_shape(true, true, false, false)?;
                let command = self.command_object("ForkFromCheckpoint")?;
                let checkpoint = self.checkpoint_object()?;
                validate_fork_command(command)?;
                validate_checkpoint(checkpoint)?;
                validate_command_checkpoint_binding(command, checkpoint, true)?;
                require_equal(
                    &self.request_ref,
                    string(command, "commandRef")?,
                    "phase2_request_ref_conflict",
                )
            }
            ManagedSandboxPhase2Action::RestoreCheckpoint => {
                self.require_shape(true, true, false, false)?;
                let command = self.command_object("RestoreCheckpoint")?;
                let checkpoint = self.checkpoint_object()?;
                validate_restore_command(command)?;
                validate_checkpoint(checkpoint)?;
                validate_command_checkpoint_binding(command, checkpoint, false)?;
                require_equal(
                    &self.request_ref,
                    string(command, "commandRef")?,
                    "phase2_request_ref_conflict",
                )
            }
            ManagedSandboxPhase2Action::DeleteCheckpoint => {
                self.require_shape(true, true, false, false)?;
                let command = self.command_object("DeleteCheckpoint")?;
                let checkpoint = self.checkpoint_object()?;
                validate_delete_command(command)?;
                validate_checkpoint(checkpoint)?;
                validate_basic_command_checkpoint_binding(command, checkpoint)?;
                require_equal(
                    &self.request_ref,
                    string(command, "commandRef")?,
                    "phase2_request_ref_conflict",
                )
            }
            ManagedSandboxPhase2Action::CreatePrivateIngress => {
                self.require_shape(true, false, false, false)?;
                let command = self.command_object("CreatePrivateIngress")?;
                validate_create_private_ingress_command(command)?;
                require_equal(
                    &self.request_ref,
                    string(command, "commandRef")?,
                    "phase2_request_ref_conflict",
                )
            }
            ManagedSandboxPhase2Action::RevokePrivateIngress => {
                self.require_shape(true, false, true, false)?;
                let command = self.command_object("RevokePrivateIngress")?;
                validate_terminal_private_ingress_command(command)?;
                validate_ingress_command_capability_binding(command, self.capability_object()?)?;
                require_equal(
                    &self.request_ref,
                    string(command, "commandRef")?,
                    "phase2_request_ref_conflict",
                )
            }
            ManagedSandboxPhase2Action::ExpirePrivateIngress => {
                self.require_shape(true, false, true, false)?;
                let command = self.command_object("ExpirePrivateIngress")?;
                validate_terminal_private_ingress_command(command)?;
                validate_ingress_command_capability_binding(command, self.capability_object()?)?;
                require_equal(
                    &self.request_ref,
                    string(command, "commandRef")?,
                    "phase2_request_ref_conflict",
                )
            }
        }
    }

    fn require_shape(
        &self,
        command: bool,
        checkpoint: bool,
        capability: bool,
        scope: bool,
    ) -> Result<(), Phase2Error> {
        if self.command.is_some() != command
            || self.checkpoint.is_some() != checkpoint
            || self.capability.is_some() != capability
            || self.owner_ref.is_some() != scope
            || self.tenant_ref.is_some() != scope
            || self.sandbox_ref.is_some() != scope
        {
            return Err(Phase2Error::invalid("phase2_action_payload_conflict"));
        }
        Ok(())
    }

    fn command_object(&self, expected_tag: &str) -> Result<&Map<String, Value>, Phase2Error> {
        let command = object(
            self.command
                .as_ref()
                .ok_or_else(|| Phase2Error::invalid("phase2_command_missing"))?,
            "phase2_command_invalid",
        )?;
        if string(command, "_tag")? != expected_tag
            || string(command, "schema")? != COMMAND_SCHEMA_VERSION
        {
            return Err(Phase2Error::invalid("phase2_command_kind_invalid"));
        }
        Ok(command)
    }

    fn checkpoint_object(&self) -> Result<&Map<String, Value>, Phase2Error> {
        object(
            self.checkpoint
                .as_ref()
                .ok_or_else(|| Phase2Error::invalid("phase2_checkpoint_missing"))?,
            "phase2_checkpoint_invalid",
        )
    }

    fn capability_object(&self) -> Result<&Map<String, Value>, Phase2Error> {
        object(
            self.capability
                .as_ref()
                .ok_or_else(|| Phase2Error::invalid("phase2_capability_missing"))?,
            "phase2_capability_invalid",
        )
    }
}

pub fn execute(
    state_root: &Path,
    request: ManagedSandboxPhase2Request,
) -> Result<ManagedSandboxPhase2Response, Phase2Error> {
    request.validate()?;
    if matches!(
        request.action,
        ManagedSandboxPhase2Action::CreatePrivateIngress
            | ManagedSandboxPhase2Action::RevokePrivateIngress
            | ManagedSandboxPhase2Action::ExpirePrivateIngress
    ) {
        let result = managed_sandbox_runtime::execute_private_ingress(state_root, &request)
            .map_err(|error| {
                if error.status() < 500 {
                    Phase2Error::conflict("phase2_private_ingress_conflict")
                } else {
                    Phase2Error::unavailable("phase2_private_ingress_failed")
                }
            })?;
        return Ok(ManagedSandboxPhase2Response {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action: request.action,
            request_ref: request.request_ref,
            result,
        });
    }
    let driver = env::var("OA_MANAGED_SANDBOX_PHASE2_DRIVER")
        .map_err(|_| Phase2Error::unavailable("phase2_driver_not_configured"))?;
    validate_driver_path(Path::new(&driver))?;
    if request.action == ManagedSandboxPhase2Action::ArchiveWithCheckpoint {
        return execute_archive_with_checkpoint(
            state_root,
            Path::new(&driver),
            request,
            MAX_DRIVER_DURATION,
        );
    }
    if request.action == ManagedSandboxPhase2Action::RestoreCheckpoint {
        return execute_restore_checkpoint(
            state_root,
            Path::new(&driver),
            request,
            MAX_DRIVER_DURATION,
        );
    }
    if request.action == ManagedSandboxPhase2Action::ForkFromCheckpoint {
        return execute_fork_from_checkpoint(
            state_root,
            Path::new(&driver),
            request,
            MAX_DRIVER_DURATION,
        );
    }
    execute_with_driver(Path::new(&driver), request, MAX_DRIVER_DURATION)
}

fn execute_fork_from_checkpoint(
    state_root: &Path,
    driver: &Path,
    request: ManagedSandboxPhase2Request,
    timeout: Duration,
) -> Result<ManagedSandboxPhase2Response, Phase2Error> {
    execute_fork_from_checkpoint_with(
        driver,
        request,
        timeout,
        |owner_ref,
         tenant_ref,
         source_sandbox_ref,
         source_resource_generation,
         command_ref,
         checkpoint_ref,
         source_capability_refs| {
            managed_sandbox_runtime::prepare_checkpoint_fork(
                state_root,
                owner_ref,
                tenant_ref,
                source_sandbox_ref,
                source_resource_generation,
                command_ref,
                checkpoint_ref,
                source_capability_refs,
            )
            .map_err(|error| {
                if error.status() < 500 {
                    Phase2Error::conflict(format!("phase2_fork_prepare_{}", error.code()))
                } else {
                    Phase2Error::unavailable(format!("phase2_fork_prepare_{}", error.code()))
                }
            })
        },
        |owner_ref, tenant_ref, fork_sandbox_ref, command_ref, succeeded| {
            managed_sandbox_runtime::finish_checkpoint_fork(
                state_root,
                owner_ref,
                tenant_ref,
                fork_sandbox_ref,
                command_ref,
                succeeded,
            )
            .map_err(|_| Phase2Error::unavailable("phase2_fork_finalize_failed"))
        },
    )
}

fn execute_fork_from_checkpoint_with<P, F>(
    driver: &Path,
    request: ManagedSandboxPhase2Request,
    timeout: Duration,
    prepare: P,
    finish: F,
) -> Result<ManagedSandboxPhase2Response, Phase2Error>
where
    P: FnOnce(
        &str,
        &str,
        &str,
        u64,
        &str,
        &str,
        &[String],
    ) -> Result<managed_sandbox_runtime::CheckpointForkContext, Phase2Error>,
    F: FnOnce(&str, &str, &str, &str, bool) -> Result<(), Phase2Error>,
{
    let command = request.command_object("ForkFromCheckpoint")?;
    request.checkpoint_object()?;
    let owner_ref = string(command, "ownerRef")?.to_string();
    let tenant_ref = string(command, "tenantRef")?.to_string();
    let source_sandbox_ref = string(command, "expectedSourceSandboxRef")?.to_string();
    let source_resource_generation = number(command, "expectedSourceResourceGeneration")?;
    let command_ref = string(command, "commandRef")?.to_string();
    let checkpoint_ref = string(command, "checkpointRef")?.to_string();
    let source_capability_refs =
        capability_ref_array(value(command, "sourceCapabilityRefs")?, false)?;
    let runtime_context = prepare(
        &owner_ref,
        &tenant_ref,
        &source_sandbox_ref,
        source_resource_generation,
        &command_ref,
        &checkpoint_ref,
        &source_capability_refs,
    )?;
    let fork_sandbox_ref = runtime_context.fork_sandbox_ref.clone();
    let mut wire = serde_json::to_value(&request)
        .map_err(|_| Phase2Error::unavailable("phase2_driver_request_encode_failed"))?;
    wire["runtimeContext"] = serde_json::to_value(runtime_context)
        .map_err(|_| Phase2Error::unavailable("phase2_fork_context_encode_failed"))?;
    let response = execute_with_driver_wire(driver, &wire, &request, timeout);
    let succeeded = response.is_ok();
    finish(
        &owner_ref,
        &tenant_ref,
        &fork_sandbox_ref,
        &command_ref,
        succeeded,
    )?;
    response
}

fn execute_restore_checkpoint(
    state_root: &Path,
    driver: &Path,
    request: ManagedSandboxPhase2Request,
    timeout: Duration,
) -> Result<ManagedSandboxPhase2Response, Phase2Error> {
    execute_restore_checkpoint_with(
        driver,
        request,
        timeout,
        |owner_ref,
         tenant_ref,
         sandbox_ref,
         command_ref,
         checkpoint_ref,
         checkpoint_source_generation,
         source_capability_refs| {
            managed_sandbox_runtime::prepare_checkpoint_restore(
                state_root,
                owner_ref,
                tenant_ref,
                sandbox_ref,
                command_ref,
                checkpoint_ref,
                checkpoint_source_generation,
                source_capability_refs,
            )
            .map_err(|error| {
                if error.status() < 500 {
                    Phase2Error::conflict("phase2_restore_prepare_conflict")
                } else {
                    Phase2Error::unavailable("phase2_restore_prepare_failed")
                }
            })
        },
        |owner_ref, tenant_ref, sandbox_ref, command_ref, succeeded| {
            managed_sandbox_runtime::finish_checkpoint_restore(
                state_root,
                owner_ref,
                tenant_ref,
                sandbox_ref,
                command_ref,
                succeeded,
            )
            .map_err(|_| Phase2Error::unavailable("phase2_restore_finalize_failed"))
        },
    )
}

fn execute_restore_checkpoint_with<P, F>(
    driver: &Path,
    request: ManagedSandboxPhase2Request,
    timeout: Duration,
    prepare: P,
    finish: F,
) -> Result<ManagedSandboxPhase2Response, Phase2Error>
where
    P: FnOnce(
        &str,
        &str,
        &str,
        &str,
        &str,
        u64,
        &[String],
    ) -> Result<managed_sandbox_runtime::CheckpointRestoreContext, Phase2Error>,
    F: FnOnce(&str, &str, &str, &str, bool) -> Result<(), Phase2Error>,
{
    let command = request.command_object("RestoreCheckpoint")?;
    let checkpoint = request.checkpoint_object()?;
    let owner_ref = string(command, "ownerRef")?.to_string();
    let tenant_ref = string(command, "tenantRef")?.to_string();
    let sandbox_ref = string(command, "destinationSandboxRef")?.to_string();
    let command_ref = string(command, "commandRef")?.to_string();
    let checkpoint_ref = string(command, "checkpointRef")?.to_string();
    let checkpoint_source_generation = number(checkpoint, "sourceResourceGeneration")?;
    let source_capability_refs =
        capability_ref_array(value(command, "sourceCapabilityRefs")?, false)?;
    let runtime_context = prepare(
        &owner_ref,
        &tenant_ref,
        &sandbox_ref,
        &command_ref,
        &checkpoint_ref,
        checkpoint_source_generation,
        &source_capability_refs,
    )?;
    let mut wire = serde_json::to_value(&request)
        .map_err(|_| Phase2Error::unavailable("phase2_driver_request_encode_failed"))?;
    wire["runtimeContext"] = serde_json::to_value(runtime_context)
        .map_err(|_| Phase2Error::unavailable("phase2_restore_context_encode_failed"))?;
    let response = execute_with_driver_wire(driver, &wire, &request, timeout);
    let succeeded = response.is_ok();
    finish(
        &owner_ref,
        &tenant_ref,
        &sandbox_ref,
        &command_ref,
        succeeded,
    )?;
    response
}

fn execute_archive_with_checkpoint(
    state_root: &Path,
    driver: &Path,
    request: ManagedSandboxPhase2Request,
    timeout: Duration,
) -> Result<ManagedSandboxPhase2Response, Phase2Error> {
    execute_archive_with_checkpoint_and_stop(
        driver,
        request,
        timeout,
        |owner_ref, tenant_ref, sandbox_ref, generation, stop_ref| {
            managed_sandbox_runtime::stop_after_checkpoint(
                state_root,
                owner_ref,
                tenant_ref,
                sandbox_ref,
                generation,
                stop_ref,
            )
            .map(|receipt| (receipt.phase, receipt.generation))
            .map_err(|_| ())
        },
    )
}

fn execute_archive_with_checkpoint_and_stop<F>(
    driver: &Path,
    request: ManagedSandboxPhase2Request,
    timeout: Duration,
    stop: F,
) -> Result<ManagedSandboxPhase2Response, Phase2Error>
where
    F: FnOnce(&str, &str, &str, u64, &str) -> Result<(RuntimePhase, u64), ()>,
{
    let archive = request.command_object("ArchiveWithCheckpoint")?;
    let owner_ref = string(archive, "ownerRef")?.to_string();
    let tenant_ref = string(archive, "tenantRef")?.to_string();
    let sandbox_ref = string(archive, "sourceSandboxRef")?.to_string();
    let stop_ref = string(archive, "stopRef")?.to_string();
    let generation = number(archive, "sourceResourceGeneration")?;
    let mut create_command = archive.clone();
    create_command.remove("stopRef");
    create_command.insert(
        "_tag".to_string(),
        Value::String("CreateCheckpoint".to_string()),
    );
    let create_request = ManagedSandboxPhase2Request {
        schema_version: request.schema_version.clone(),
        action: ManagedSandboxPhase2Action::CreateCheckpoint,
        request_ref: request.request_ref.clone(),
        command: Some(Value::Object(create_command)),
        checkpoint: None,
        capability: None,
        owner_ref: None,
        tenant_ref: None,
        sandbox_ref: None,
    };
    create_request.validate()?;
    let checkpoint_response = execute_with_driver(driver, create_request, timeout)?;
    let checkpoint = checkpoint_response.result;
    let checkpoint_object = object(&checkpoint, "phase2_archive_checkpoint_invalid")?;
    let observed_at = string(checkpoint_object, "verifiedAt")?.to_string();
    let stop = stop(&owner_ref, &tenant_ref, &sandbox_ref, generation, &stop_ref);
    let result = match stop {
        Ok((RuntimePhase::Stopped, stopped_generation)) if stopped_generation == generation => {
            json!({
                "_tag": "Archived",
                "schema": CHECKPOINT_STOP_SCHEMA_VERSION,
                "stopRef": stop_ref,
                "sandboxRef": sandbox_ref,
                "resourceGeneration": generation,
                "observedAt": observed_at,
                "evidenceRefs": [stop_ref],
                "checkpoint": checkpoint,
                "lifecycle": "stopped",
                "archiveClaim": "allowed",
            })
        }
        _ => {
            let cleanup_request = archive_cleanup_request(archive, checkpoint.clone())?;
            let cleanup_response = execute_with_driver(driver, cleanup_request, timeout)
                .map_err(|_| Phase2Error::unavailable("phase2_archive_cleanup_failed"))?;
            let cleanup = object(
                &cleanup_response.result,
                "phase2_archive_cleanup_receipt_invalid",
            )?;
            let cleanup_receipt_ref = string(cleanup, "receiptRef")?;
            json!({
                "_tag": "CheckpointFailed",
                "schema": CHECKPOINT_STOP_SCHEMA_VERSION,
                "stopRef": stop_ref,
                "sandboxRef": sandbox_ref,
                "resourceGeneration": generation,
                "observedAt": observed_at,
                "evidenceRefs": [stop_ref, cleanup_receipt_ref],
                "attemptedCheckpointRef": string(archive, "checkpointRef")?,
                "errorRef": "error.sbx10.archive.stop_failed_checkpoint_cleaned",
                "lifecycle": "recovery_required",
                "archiveClaim": "forbidden",
            })
        }
    };
    let response = ManagedSandboxPhase2Response {
        schema_version: TARGET_SCHEMA_VERSION.to_string(),
        action: request.action,
        request_ref: request.request_ref.clone(),
        result,
    };
    validate_response(&request, &response)?;
    Ok(response)
}

fn archive_cleanup_request(
    archive: &Map<String, Value>,
    checkpoint: Value,
) -> Result<ManagedSandboxPhase2Request, Phase2Error> {
    let identity = Sha256::digest(
        format!(
            "archive-cleanup|{}|{}|{}|{}",
            string(archive, "ownerRef")?,
            string(archive, "tenantRef")?,
            string(archive, "checkpointRef")?,
            string(archive, "stopRef")?,
        )
        .as_bytes(),
    );
    let identity = format!("{identity:x}");
    let command_ref = format!("command.sbx10.archive-cleanup.{}", &identity[..32]);
    let command = json!({
        "_tag": "DeleteCheckpoint",
        "schema": COMMAND_SCHEMA_VERSION,
        "commandRef": command_ref,
        "idempotencyRef": format!("idempotency.sbx10.archive-cleanup.{}", &identity[..32]),
        "ownerRef": string(archive, "ownerRef")?,
        "tenantRef": string(archive, "tenantRef")?,
        "requestedAt": string(object(&checkpoint, "phase2_archive_checkpoint_invalid")?, "verifiedAt")?,
        "checkpointRef": string(archive, "checkpointRef")?,
        "reason": "sandbox_teardown"
    });
    let request = ManagedSandboxPhase2Request {
        schema_version: TARGET_SCHEMA_VERSION.to_string(),
        action: ManagedSandboxPhase2Action::DeleteCheckpoint,
        request_ref: command_ref,
        command: Some(command),
        checkpoint: Some(checkpoint),
        capability: None,
        owner_ref: None,
        tenant_ref: None,
        sandbox_ref: None,
    };
    request.validate()?;
    Ok(request)
}

fn validate_driver_path(driver: &Path) -> Result<(), Phase2Error> {
    if !driver.is_absolute() {
        return Err(Phase2Error::unavailable("phase2_driver_path_not_absolute"));
    }
    Ok(())
}

fn execute_with_driver(
    driver: &Path,
    request: ManagedSandboxPhase2Request,
    timeout: Duration,
) -> Result<ManagedSandboxPhase2Response, Phase2Error> {
    let wire = serde_json::to_value(&request)
        .map_err(|_| Phase2Error::unavailable("phase2_driver_request_encode_failed"))?;
    execute_with_driver_wire(driver, &wire, &request, timeout)
}

fn execute_with_driver_wire(
    driver: &Path,
    wire: &Value,
    request: &ManagedSandboxPhase2Request,
    timeout: Duration,
) -> Result<ManagedSandboxPhase2Response, Phase2Error> {
    validate_driver_path(driver)?;
    let request_bytes = serde_json::to_vec(wire)
        .map_err(|_| Phase2Error::unavailable("phase2_driver_request_encode_failed"))?;
    let mut command = Command::new(driver);
    command
        .arg("--managed-sandbox-phase2")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|_| Phase2Error::unavailable("phase2_driver_spawn_failed"))?;
    let write_result = child
        .stdin
        .as_mut()
        .ok_or_else(|| Phase2Error::unavailable("phase2_driver_stdin_missing"))
        .and_then(|stdin| {
            stdin
                .write_all(&request_bytes)
                .map_err(|_| Phase2Error::unavailable("phase2_driver_write_failed"))
        });
    if let Err(error) = write_result {
        kill_process_tree(&mut child);
        let _ = child.wait();
        return Err(error);
    }
    drop(child.stdin.take());

    let Some(mut stdout) = child.stdout.take() else {
        kill_process_tree(&mut child);
        let _ = child.wait();
        return Err(Phase2Error::unavailable("phase2_driver_stdout_missing"));
    };
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .by_ref()
            .take(MAX_DRIVER_RESPONSE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });

    let deadline = Instant::now() + timeout;
    let mut early_bytes = None;
    let status = loop {
        if early_bytes.is_none() {
            match receiver.try_recv() {
                Ok(Ok(bytes)) if bytes.len() as u64 > MAX_DRIVER_RESPONSE_BYTES => {
                    kill_process_tree(&mut child);
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(Phase2Error::unavailable("phase2_driver_response_too_large"));
                }
                Ok(Ok(bytes)) => early_bytes = Some(bytes),
                Ok(Err(_)) => {
                    kill_process_tree(&mut child);
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(Phase2Error::unavailable("phase2_driver_read_failed"));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    kill_process_tree(&mut child);
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(Phase2Error::unavailable("phase2_driver_read_failed"));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                kill_process_tree(&mut child);
                let _ = child.wait();
                let _ = reader.join();
                return Err(Phase2Error::unavailable("phase2_driver_timed_out"));
            }
            Err(_) => {
                kill_process_tree(&mut child);
                let _ = child.wait();
                let _ = reader.join();
                return Err(Phase2Error::unavailable("phase2_driver_wait_failed"));
            }
        }
    };
    let bytes = match early_bytes {
        Some(bytes) => bytes,
        None => receiver
            .recv()
            .map_err(|_| Phase2Error::unavailable("phase2_driver_read_failed"))?
            .map_err(|_| Phase2Error::unavailable("phase2_driver_read_failed"))?,
    };
    let _ = reader.join();
    if !status.success() {
        return Err(Phase2Error::unavailable(driver_refusal_reason(&bytes)));
    }
    if bytes.len() as u64 > MAX_DRIVER_RESPONSE_BYTES {
        return Err(Phase2Error::unavailable("phase2_driver_response_too_large"));
    }
    if forbidden_private_material(&bytes) {
        return Err(Phase2Error::conflict(
            "phase2_driver_response_contains_private_material",
        ));
    }
    let response: ManagedSandboxPhase2Response = serde_json::from_slice(&bytes)
        .map_err(|_| Phase2Error::unavailable("phase2_driver_response_invalid"))?;
    validate_response(request, &response)?;
    Ok(response)
}

fn driver_refusal_reason(bytes: &[u8]) -> String {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return "phase2_driver_refused".to_string();
    };
    if value.get("schemaVersion").and_then(Value::as_str) != Some(DRIVER_ERROR_SCHEMA_VERSION) {
        return "phase2_driver_refused".to_string();
    }
    let Some(reason) = value.get("reasonRef").and_then(Value::as_str) else {
        return "phase2_driver_refused".to_string();
    };
    if reason.is_empty()
        || reason.len() > 80
        || !reason
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return "phase2_driver_refused".to_string();
    }
    format!("phase2_driver_{reason}")
}

fn kill_process_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        let _ = libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
}

fn validate_response(
    request: &ManagedSandboxPhase2Request,
    response: &ManagedSandboxPhase2Response,
) -> Result<(), Phase2Error> {
    if response.schema_version != TARGET_SCHEMA_VERSION
        || response.action != request.action
        || response.request_ref != request.request_ref
    {
        return Err(Phase2Error::conflict(
            "phase2_driver_response_scope_conflict",
        ));
    }
    let result = object(&response.result, "phase2_driver_result_invalid")
        .map_err(|_| Phase2Error::conflict("phase2_driver_result_invalid"))?;
    let validation = (|| match request.action {
        ManagedSandboxPhase2Action::CreateCheckpoint => {
            validate_checkpoint(result)?;
            validate_created_checkpoint_binding(request.command_object("CreateCheckpoint")?, result)
        }
        ManagedSandboxPhase2Action::ArchiveWithCheckpoint => {
            validate_archive_result(request.command_object("ArchiveWithCheckpoint")?, result)
        }
        ManagedSandboxPhase2Action::VerifyCheckpoint => {
            validate_verify_result(request.checkpoint_object()?, result)
        }
        ManagedSandboxPhase2Action::ObserveResourceGeneration => {
            validate_generation_result(request, result)
        }
        ManagedSandboxPhase2Action::ForkFromCheckpoint => validate_fork_result(
            request.command_object("ForkFromCheckpoint")?,
            request.checkpoint_object()?,
            result,
        ),
        ManagedSandboxPhase2Action::RestoreCheckpoint => validate_restore_result(
            request.command_object("RestoreCheckpoint")?,
            request.checkpoint_object()?,
            result,
        ),
        ManagedSandboxPhase2Action::DeleteCheckpoint => validate_delete_result(
            request.command_object("DeleteCheckpoint")?,
            request.checkpoint_object()?,
            result,
        ),
        ManagedSandboxPhase2Action::CreatePrivateIngress
        | ManagedSandboxPhase2Action::RevokePrivateIngress
        | ManagedSandboxPhase2Action::ExpirePrivateIngress => Err(Phase2Error::invalid(
            "phase2_private_ingress_is_native_only",
        )),
    })();
    validation.map_err(|_| Phase2Error::conflict("phase2_driver_result_invalid"))
}

fn validate_create_command(command: &Map<String, Value>) -> Result<(), Phase2Error> {
    exact_keys(
        command,
        &[
            "_tag",
            "schema",
            "commandRef",
            "idempotencyRef",
            "ownerRef",
            "tenantRef",
            "requestedAt",
            "checkpointRef",
            "sourceSandboxRef",
            "sourceResourceGeneration",
            "sourceImageDigest",
            "sourceToolchainDigest",
            "repositoryRef",
            "repositoryRevisionRef",
            "repositoryPostImageDigest",
            "formatRef",
            "retainedUntil",
        ],
        "phase2_create_command_fields_invalid",
    )?;
    validate_command_base(command)?;
    validate_checkpoint_source_fields(command)
}

fn validate_archive_command(command: &Map<String, Value>) -> Result<(), Phase2Error> {
    exact_keys(
        command,
        &[
            "_tag",
            "schema",
            "commandRef",
            "idempotencyRef",
            "ownerRef",
            "tenantRef",
            "requestedAt",
            "checkpointRef",
            "sourceSandboxRef",
            "sourceResourceGeneration",
            "sourceImageDigest",
            "sourceToolchainDigest",
            "repositoryRef",
            "repositoryRevisionRef",
            "repositoryPostImageDigest",
            "formatRef",
            "retainedUntil",
            "stopRef",
        ],
        "phase2_archive_command_fields_invalid",
    )?;
    validate_command_base(command)?;
    validate_checkpoint_source_fields(command)?;
    validate_ref(string(command, "stopRef")?)
}

fn validate_fork_command(command: &Map<String, Value>) -> Result<(), Phase2Error> {
    exact_keys(
        command,
        &[
            "_tag",
            "schema",
            "commandRef",
            "idempotencyRef",
            "ownerRef",
            "tenantRef",
            "requestedAt",
            "checkpointRef",
            "expectedSourceSandboxRef",
            "expectedSourceResourceGeneration",
            "sourceCapabilityRefs",
        ],
        "phase2_fork_command_fields_invalid",
    )?;
    validate_command_base(command)?;
    validate_ref(string(command, "checkpointRef")?)?;
    validate_ref(string(command, "expectedSourceSandboxRef")?)?;
    number(command, "expectedSourceResourceGeneration")?;
    validate_capability_ref_array(value(command, "sourceCapabilityRefs")?, false)
}

fn validate_restore_command(command: &Map<String, Value>) -> Result<(), Phase2Error> {
    exact_keys(
        command,
        &[
            "_tag",
            "schema",
            "commandRef",
            "idempotencyRef",
            "ownerRef",
            "tenantRef",
            "requestedAt",
            "checkpointRef",
            "destinationSandboxRef",
            "expectedSourceResourceGeneration",
            "admittedServiceRefs",
            "sourceCapabilityRefs",
        ],
        "phase2_restore_command_fields_invalid",
    )?;
    validate_command_base(command)?;
    validate_ref(string(command, "checkpointRef")?)?;
    validate_ref(string(command, "destinationSandboxRef")?)?;
    number(command, "expectedSourceResourceGeneration")?;
    validate_ref_array(value(command, "admittedServiceRefs")?, false)?;
    validate_capability_ref_array(value(command, "sourceCapabilityRefs")?, false)
}

fn validate_delete_command(command: &Map<String, Value>) -> Result<(), Phase2Error> {
    exact_keys(
        command,
        &[
            "_tag",
            "schema",
            "commandRef",
            "idempotencyRef",
            "ownerRef",
            "tenantRef",
            "requestedAt",
            "checkpointRef",
            "reason",
        ],
        "phase2_delete_command_fields_invalid",
    )?;
    validate_command_base(command)?;
    validate_ref(string(command, "checkpointRef")?)?;
    if !matches!(
        string(command, "reason")?,
        "owner_requested" | "retention_expired" | "sandbox_teardown"
    ) {
        return Err(Phase2Error::invalid("phase2_delete_reason_invalid"));
    }
    Ok(())
}

fn validate_create_private_ingress_command(
    command: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    exact_keys(
        command,
        &[
            "_tag",
            "schema",
            "commandRef",
            "idempotencyRef",
            "ownerRef",
            "tenantRef",
            "requestedAt",
            "sandboxRef",
            "resourceGeneration",
            "audienceRef",
            "kind",
            "ttlSeconds",
        ],
        "phase2_private_ingress_create_fields_invalid",
    )?;
    validate_command_base(command)?;
    for field in ["sandboxRef", "audienceRef"] {
        validate_ref(string(command, field)?)?;
    }
    number(command, "resourceGeneration")?;
    let ttl = number(command, "ttlSeconds")?;
    if !(1..=900).contains(&ttl) || !matches!(string(command, "kind")?, "desktop" | "preview") {
        return Err(Phase2Error::invalid(
            "phase2_private_ingress_create_invalid",
        ));
    }
    Ok(())
}

fn validate_terminal_private_ingress_command(
    command: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    exact_keys(
        command,
        &[
            "_tag",
            "schema",
            "commandRef",
            "idempotencyRef",
            "ownerRef",
            "tenantRef",
            "requestedAt",
            "capabilityRef",
            "sandboxRef",
            "resourceGeneration",
        ],
        "phase2_private_ingress_terminal_fields_invalid",
    )?;
    validate_command_base(command)?;
    validate_ref(string(command, "capabilityRef")?)?;
    validate_ref(string(command, "sandboxRef")?)?;
    number(command, "resourceGeneration")?;
    Ok(())
}

fn validate_ingress_command_capability_binding(
    command: &Map<String, Value>,
    capability: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    for field in ["capabilityRef", "sandboxRef", "resourceGeneration"] {
        if value(command, field)? != value(capability, field)? {
            return Err(Phase2Error::conflict(
                "phase2_private_ingress_scope_conflict",
            ));
        }
    }
    require_equal(
        string(command, "ownerRef")?,
        string(capability, "ownerRef")?,
        "phase2_private_ingress_scope_conflict",
    )
}

fn validate_command_base(command: &Map<String, Value>) -> Result<(), Phase2Error> {
    for field in ["commandRef", "idempotencyRef", "ownerRef", "tenantRef"] {
        validate_ref(string(command, field)?)?;
    }
    validate_timestamp(string(command, "requestedAt")?)
}

fn validate_checkpoint_source_fields(command: &Map<String, Value>) -> Result<(), Phase2Error> {
    for field in [
        "checkpointRef",
        "sourceSandboxRef",
        "repositoryRef",
        "repositoryRevisionRef",
        "formatRef",
    ] {
        validate_ref(string(command, field)?)?;
    }
    number(command, "sourceResourceGeneration")?;
    for field in [
        "sourceImageDigest",
        "sourceToolchainDigest",
        "repositoryPostImageDigest",
    ] {
        validate_digest(string(command, field)?)?;
    }
    validate_timestamp(string(command, "retainedUntil")?)
}

fn validate_checkpoint(checkpoint: &Map<String, Value>) -> Result<(), Phase2Error> {
    exact_keys(
        checkpoint,
        &[
            "schema",
            "checkpointRef",
            "ownerRef",
            "tenantRef",
            "sourceSandboxRef",
            "sourceResourceGeneration",
            "sourceImageDigest",
            "sourceToolchainDigest",
            "repositoryRef",
            "repositoryRevisionRef",
            "repositoryPostImageDigest",
            "contentDigest",
            "contentBytes",
            "formatRef",
            "state",
            "completedAt",
            "verifiedAt",
            "retainedUntil",
            "deleteOnExpiry",
            "omissions",
            "evidenceRefs",
        ],
        "phase2_checkpoint_fields_invalid",
    )?;
    if string(checkpoint, "schema")? != CHECKPOINT_SCHEMA_VERSION
        || string(checkpoint, "state")? != "completed"
        || !boolean(checkpoint, "deleteOnExpiry")?
    {
        return Err(Phase2Error::invalid("phase2_checkpoint_state_invalid"));
    }
    for field in [
        "checkpointRef",
        "ownerRef",
        "tenantRef",
        "sourceSandboxRef",
        "repositoryRef",
        "repositoryRevisionRef",
        "formatRef",
    ] {
        validate_ref(string(checkpoint, field)?)?;
    }
    number(checkpoint, "sourceResourceGeneration")?;
    number(checkpoint, "contentBytes")?;
    for field in [
        "sourceImageDigest",
        "sourceToolchainDigest",
        "repositoryPostImageDigest",
        "contentDigest",
    ] {
        validate_digest(string(checkpoint, field)?)?;
    }
    let completed = string(checkpoint, "completedAt")?;
    let verified = string(checkpoint, "verifiedAt")?;
    let retained = string(checkpoint, "retainedUntil")?;
    for timestamp in [completed, verified, retained] {
        validate_timestamp(timestamp)?;
    }
    if normalized_timestamp(verified) < normalized_timestamp(completed)
        || normalized_timestamp(retained) <= normalized_timestamp(verified)
    {
        return Err(Phase2Error::invalid("phase2_checkpoint_time_order_invalid"));
    }
    validate_omissions(value(checkpoint, "omissions")?)?;
    validate_ref_array(value(checkpoint, "evidenceRefs")?, false)
}

fn validate_omissions(value: &Value) -> Result<(), Phase2Error> {
    let omissions = object(value, "phase2_checkpoint_omissions_invalid")?;
    let fields = [
        "credentials",
        "accountSecrets",
        "providerHiddenState",
        "processMemory",
        "processTable",
        "ptyState",
        "sockets",
        "ports",
        "networkIdentity",
    ];
    exact_keys(omissions, &fields, "phase2_checkpoint_omissions_invalid")?;
    if fields
        .iter()
        .any(|field| string(omissions, field).ok() != Some("excluded"))
    {
        return Err(Phase2Error::invalid("phase2_checkpoint_omissions_invalid"));
    }
    Ok(())
}

fn validate_basic_command_checkpoint_binding(
    command: &Map<String, Value>,
    checkpoint: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    for field in ["ownerRef", "tenantRef", "checkpointRef"] {
        require_equal(
            string(command, field)?,
            string(checkpoint, field)?,
            "phase2_command_checkpoint_scope_conflict",
        )?;
    }
    Ok(())
}

fn validate_command_checkpoint_binding(
    command: &Map<String, Value>,
    checkpoint: &Map<String, Value>,
    fork: bool,
) -> Result<(), Phase2Error> {
    validate_basic_command_checkpoint_binding(command, checkpoint)?;
    require_equal_number(
        number(command, "expectedSourceResourceGeneration")?,
        number(checkpoint, "sourceResourceGeneration")?,
        "phase2_command_checkpoint_generation_conflict",
    )?;
    if fork {
        require_equal(
            string(command, "expectedSourceSandboxRef")?,
            string(checkpoint, "sourceSandboxRef")?,
            "phase2_command_checkpoint_source_conflict",
        )?;
    }
    Ok(())
}

fn validate_created_checkpoint_binding(
    command: &Map<String, Value>,
    checkpoint: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    for field in [
        "checkpointRef",
        "ownerRef",
        "tenantRef",
        "sourceSandboxRef",
        "sourceImageDigest",
        "sourceToolchainDigest",
        "repositoryRef",
        "repositoryRevisionRef",
        "repositoryPostImageDigest",
        "formatRef",
        "retainedUntil",
    ] {
        require_equal(
            string(command, field)?,
            string(checkpoint, field)?,
            "phase2_created_checkpoint_scope_conflict",
        )?;
    }
    require_equal_number(
        number(command, "sourceResourceGeneration")?,
        number(checkpoint, "sourceResourceGeneration")?,
        "phase2_created_checkpoint_scope_conflict",
    )
}

fn validate_archive_result(
    command: &Map<String, Value>,
    result: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    match string(result, "_tag")? {
        "Archived" => {
            exact_keys(
                result,
                &[
                    "_tag",
                    "schema",
                    "stopRef",
                    "sandboxRef",
                    "resourceGeneration",
                    "observedAt",
                    "evidenceRefs",
                    "checkpoint",
                    "lifecycle",
                    "archiveClaim",
                ],
                "phase2_archive_result_fields_invalid",
            )?;
            validate_stop_base(command, result)?;
            if string(result, "lifecycle")? != "stopped"
                || string(result, "archiveClaim")? != "allowed"
            {
                return Err(Phase2Error::conflict("phase2_archive_result_invalid"));
            }
            let checkpoint = object(
                value(result, "checkpoint")?,
                "phase2_archive_checkpoint_invalid",
            )?;
            validate_checkpoint(checkpoint)?;
            validate_created_checkpoint_binding(command, checkpoint)
        }
        "CheckpointFailed" => {
            exact_keys(
                result,
                &[
                    "_tag",
                    "schema",
                    "stopRef",
                    "sandboxRef",
                    "resourceGeneration",
                    "observedAt",
                    "evidenceRefs",
                    "attemptedCheckpointRef",
                    "errorRef",
                    "lifecycle",
                    "archiveClaim",
                ],
                "phase2_archive_result_fields_invalid",
            )?;
            validate_stop_base(command, result)?;
            validate_ref(string(result, "errorRef")?)?;
            require_equal(
                string(command, "checkpointRef")?,
                string(result, "attemptedCheckpointRef")?,
                "phase2_archive_checkpoint_scope_conflict",
            )?;
            if string(result, "lifecycle")? != "recovery_required"
                || string(result, "archiveClaim")? != "forbidden"
            {
                return Err(Phase2Error::conflict("phase2_archive_result_invalid"));
            }
            Ok(())
        }
        _ => Err(Phase2Error::conflict("phase2_archive_result_invalid")),
    }
}

fn validate_stop_base(
    command: &Map<String, Value>,
    result: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    if string(result, "schema")? != CHECKPOINT_STOP_SCHEMA_VERSION {
        return Err(Phase2Error::conflict("phase2_archive_schema_conflict"));
    }
    require_equal(
        string(command, "stopRef")?,
        string(result, "stopRef")?,
        "phase2_archive_stop_scope_conflict",
    )?;
    require_equal(
        string(command, "sourceSandboxRef")?,
        string(result, "sandboxRef")?,
        "phase2_archive_sandbox_scope_conflict",
    )?;
    require_equal_number(
        number(command, "sourceResourceGeneration")?,
        number(result, "resourceGeneration")?,
        "phase2_archive_generation_scope_conflict",
    )?;
    validate_timestamp(string(result, "observedAt")?)?;
    validate_ref_array(value(result, "evidenceRefs")?, false)
}

fn validate_verify_result(
    checkpoint: &Map<String, Value>,
    result: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    exact_keys(
        result,
        &["verified", "checkpointRef", "contentDigest", "evidenceRefs"],
        "phase2_verify_result_fields_invalid",
    )?;
    boolean(result, "verified")?;
    require_equal(
        string(checkpoint, "checkpointRef")?,
        string(result, "checkpointRef")?,
        "phase2_verify_checkpoint_scope_conflict",
    )?;
    require_equal(
        string(checkpoint, "contentDigest")?,
        string(result, "contentDigest")?,
        "phase2_verify_digest_scope_conflict",
    )?;
    validate_ref_array(value(result, "evidenceRefs")?, true)
}

fn validate_generation_result(
    request: &ManagedSandboxPhase2Request,
    result: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    exact_keys(
        result,
        &[
            "ownerRef",
            "tenantRef",
            "sandboxRef",
            "resourceGeneration",
            "evidenceRefs",
        ],
        "phase2_generation_result_fields_invalid",
    )?;
    for (expected, field) in [
        (request.owner_ref.as_deref(), "ownerRef"),
        (request.tenant_ref.as_deref(), "tenantRef"),
        (request.sandbox_ref.as_deref(), "sandboxRef"),
    ] {
        require_equal(
            expected.ok_or_else(|| Phase2Error::invalid("phase2_scope_missing"))?,
            string(result, field)?,
            "phase2_generation_scope_conflict",
        )?;
    }
    number(result, "resourceGeneration")?;
    validate_ref_array(value(result, "evidenceRefs")?, true)
}

fn validate_fork_result(
    command: &Map<String, Value>,
    checkpoint: &Map<String, Value>,
    result: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    exact_keys(
        result,
        &[
            "schema",
            "receiptRef",
            "ownerRef",
            "tenantRef",
            "checkpointRef",
            "sourceSandboxRef",
            "sourceResourceGeneration",
            "forkSandboxRef",
            "forkResourceGeneration",
            "sourceCapabilityRefs",
            "forkCapabilityRefs",
            "grantPolicy",
            "cleanupObligationRef",
            "stateTransfer",
            "processSessionContinuity",
            "outcome",
            "observedAt",
            "evidenceRefs",
        ],
        "phase2_fork_result_fields_invalid",
    )?;
    if string(result, "schema")? != FORK_RECEIPT_SCHEMA_VERSION
        || string(result, "grantPolicy")? != "mint_fresh"
        || string(result, "processSessionContinuity")? != "none"
        || string(result, "outcome")? != "created"
        || number(result, "forkResourceGeneration")? == 0
    {
        return Err(Phase2Error::conflict("phase2_fork_result_invalid"));
    }
    for field in ["ownerRef", "tenantRef", "checkpointRef"] {
        require_equal(
            string(command, field)?,
            string(result, field)?,
            "phase2_fork_scope_conflict",
        )?;
    }
    require_equal(
        string(checkpoint, "sourceSandboxRef")?,
        string(result, "sourceSandboxRef")?,
        "phase2_fork_source_scope_conflict",
    )?;
    require_equal_number(
        number(checkpoint, "sourceResourceGeneration")?,
        number(result, "sourceResourceGeneration")?,
        "phase2_fork_source_scope_conflict",
    )?;
    let source = capability_ref_array(value(result, "sourceCapabilityRefs")?, false)?;
    let expected_source = capability_ref_array(value(command, "sourceCapabilityRefs")?, false)?;
    if source != expected_source {
        return Err(Phase2Error::conflict("phase2_fork_source_grant_conflict"));
    }
    let fork = capability_ref_array(value(result, "forkCapabilityRefs")?, false)?;
    if !unique_and_disjoint(&source, &fork)
        || string(result, "forkSandboxRef")? == string(result, "sourceSandboxRef")?
    {
        return Err(Phase2Error::conflict("phase2_fork_identity_conflict"));
    }
    for field in ["receiptRef", "forkSandboxRef", "cleanupObligationRef"] {
        validate_ref(string(result, field)?)?;
    }
    validate_omissions(value(result, "stateTransfer")?)?;
    validate_timestamp(string(result, "observedAt")?)?;
    validate_ref_array(value(result, "evidenceRefs")?, false)
}

fn validate_restore_result(
    command: &Map<String, Value>,
    checkpoint: &Map<String, Value>,
    result: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    exact_keys(
        result,
        &[
            "schema",
            "receiptRef",
            "ownerRef",
            "tenantRef",
            "checkpointRef",
            "sandboxRef",
            "checkpointSourceGeneration",
            "restoredResourceGeneration",
            "admittedServiceRefs",
            "restartedServiceRefs",
            "sourceCapabilityRefs",
            "restoredCapabilityRefs",
            "grantPolicy",
            "processSessionContinuity",
            "processMemoryRestored",
            "ptyRestored",
            "socketsRestored",
            "outcome",
            "observedAt",
            "evidenceRefs",
        ],
        "phase2_restore_result_fields_invalid",
    )?;
    if string(result, "schema")? != RESTORE_RECEIPT_SCHEMA_VERSION
        || string(result, "grantPolicy")? != "mint_fresh"
        || string(result, "processSessionContinuity")? != "discontinuous"
        || boolean(result, "processMemoryRestored")?
        || boolean(result, "ptyRestored")?
        || boolean(result, "socketsRestored")?
        || string(result, "outcome")? != "restored"
    {
        return Err(Phase2Error::conflict("phase2_restore_result_invalid"));
    }
    for field in ["ownerRef", "tenantRef", "checkpointRef"] {
        require_equal(
            string(command, field)?,
            string(result, field)?,
            "phase2_restore_scope_conflict",
        )?;
    }
    require_equal(
        string(command, "destinationSandboxRef")?,
        string(result, "sandboxRef")?,
        "phase2_restore_destination_conflict",
    )?;
    let checkpoint_generation = number(checkpoint, "sourceResourceGeneration")?;
    require_equal_number(
        checkpoint_generation,
        number(result, "checkpointSourceGeneration")?,
        "phase2_restore_generation_conflict",
    )?;
    if number(result, "restoredResourceGeneration")? <= checkpoint_generation {
        return Err(Phase2Error::conflict(
            "phase2_restore_generation_not_advanced",
        ));
    }
    let admitted = ref_array(value(result, "admittedServiceRefs")?, false)?;
    let expected_admitted = ref_array(value(command, "admittedServiceRefs")?, false)?;
    let restarted = ref_array(value(result, "restartedServiceRefs")?, false)?;
    if admitted != expected_admitted
        || !is_unique(&admitted)
        || !is_unique_subset(&restarted, &admitted)
    {
        return Err(Phase2Error::conflict(
            "phase2_restore_service_scope_conflict",
        ));
    }
    let source = capability_ref_array(value(result, "sourceCapabilityRefs")?, false)?;
    let expected_source = capability_ref_array(value(command, "sourceCapabilityRefs")?, false)?;
    let restored = capability_ref_array(value(result, "restoredCapabilityRefs")?, false)?;
    if source != expected_source || !unique_and_disjoint_right(&source, &restored) {
        return Err(Phase2Error::conflict("phase2_restore_grant_scope_conflict"));
    }
    validate_ref(string(result, "receiptRef")?)?;
    validate_timestamp(string(result, "observedAt")?)?;
    validate_ref_array(value(result, "evidenceRefs")?, false)
}

fn validate_delete_result(
    command: &Map<String, Value>,
    checkpoint: &Map<String, Value>,
    result: &Map<String, Value>,
) -> Result<(), Phase2Error> {
    exact_keys(
        result,
        &[
            "schema",
            "receiptRef",
            "ownerRef",
            "tenantRef",
            "checkpointRef",
            "sourceSandboxRef",
            "sourceResourceGeneration",
            "contentDigest",
            "contentDeleted",
            "outcome",
            "reason",
            "deletedAt",
            "evidenceRefs",
        ],
        "phase2_delete_result_fields_invalid",
    )?;
    if string(result, "schema")? != CHECKPOINT_DELETE_SCHEMA_VERSION
        || !boolean(result, "contentDeleted")?
        || string(result, "outcome")? != "deleted"
    {
        return Err(Phase2Error::conflict("phase2_delete_result_invalid"));
    }
    for field in ["ownerRef", "tenantRef", "checkpointRef"] {
        require_equal(
            string(command, field)?,
            string(result, field)?,
            "phase2_delete_scope_conflict",
        )?;
    }
    for field in ["sourceSandboxRef", "contentDigest"] {
        require_equal(
            string(checkpoint, field)?,
            string(result, field)?,
            "phase2_delete_checkpoint_scope_conflict",
        )?;
    }
    require_equal_number(
        number(checkpoint, "sourceResourceGeneration")?,
        number(result, "sourceResourceGeneration")?,
        "phase2_delete_checkpoint_scope_conflict",
    )?;
    require_equal(
        string(command, "reason")?,
        string(result, "reason")?,
        "phase2_delete_reason_conflict",
    )?;
    validate_ref(string(result, "receiptRef")?)?;
    validate_timestamp(string(result, "deletedAt")?)?;
    validate_ref_array(value(result, "evidenceRefs")?, false)
}

fn object<'a>(
    value: &'a Value,
    reason: &'static str,
) -> Result<&'a Map<String, Value>, Phase2Error> {
    value
        .as_object()
        .ok_or_else(|| Phase2Error::invalid(reason))
}

fn exact_keys(
    object: &Map<String, Value>,
    expected: &[&str],
    reason: &'static str,
) -> Result<(), Phase2Error> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(Phase2Error::invalid(reason));
    }
    Ok(())
}

fn value<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a Value, Phase2Error> {
    object
        .get(field)
        .ok_or_else(|| Phase2Error::invalid("phase2_required_field_missing"))
}

fn string<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, Phase2Error> {
    value(object, field)?
        .as_str()
        .ok_or_else(|| Phase2Error::invalid("phase2_string_field_invalid"))
}

fn number(object: &Map<String, Value>, field: &str) -> Result<u64, Phase2Error> {
    value(object, field)?
        .as_u64()
        .ok_or_else(|| Phase2Error::invalid("phase2_number_field_invalid"))
}

fn boolean(object: &Map<String, Value>, field: &str) -> Result<bool, Phase2Error> {
    value(object, field)?
        .as_bool()
        .ok_or_else(|| Phase2Error::invalid("phase2_boolean_field_invalid"))
}

fn validate_ref(value: &str) -> Result<(), Phase2Error> {
    if value.len() < 3
        || value.len() > 256
        || !value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        })
    {
        return Err(Phase2Error::invalid("phase2_public_ref_invalid"));
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), Phase2Error> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
    {
        return Err(Phase2Error::invalid("phase2_digest_invalid"));
    }
    Ok(())
}

fn validate_timestamp(value: &str) -> Result<(), Phase2Error> {
    let bytes = value.as_bytes();
    if !matches!(bytes.len(), 20 | 24)
        || bytes.last() != Some(&b'Z')
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
        || (bytes.len() == 24 && bytes.get(19) != Some(&b'.'))
        || !bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7 | 10 | 13 | 16 | 19 | 23) || byte.is_ascii_digit()
        })
    {
        return Err(Phase2Error::invalid("phase2_timestamp_invalid"));
    }
    Ok(())
}

fn normalized_timestamp(value: &str) -> String {
    if value.len() == 20 {
        format!("{}.000Z", &value[..19])
    } else {
        value.to_string()
    }
}

fn ref_array(value: &Value, require_non_empty: bool) -> Result<Vec<String>, Phase2Error> {
    let values = value
        .as_array()
        .ok_or_else(|| Phase2Error::invalid("phase2_ref_array_invalid"))?;
    if values.len() > 64 || (require_non_empty && values.is_empty()) {
        return Err(Phase2Error::invalid("phase2_ref_array_invalid"));
    }
    values
        .iter()
        .map(|value| {
            let value = value
                .as_str()
                .ok_or_else(|| Phase2Error::invalid("phase2_ref_array_invalid"))?;
            validate_ref(value)?;
            Ok(value.to_string())
        })
        .collect()
}

fn validate_ref_array(value: &Value, require_non_empty: bool) -> Result<(), Phase2Error> {
    ref_array(value, require_non_empty).map(|_| ())
}

fn capability_ref_array(
    value: &Value,
    require_non_empty: bool,
) -> Result<Vec<String>, Phase2Error> {
    let values = value
        .as_array()
        .ok_or_else(|| Phase2Error::invalid("phase2_capability_ref_array_invalid"))?;
    if values.len() > 64 || (require_non_empty && values.is_empty()) {
        return Err(Phase2Error::invalid("phase2_capability_ref_array_invalid"));
    }
    values
        .iter()
        .map(|value| {
            let value = value
                .as_str()
                .ok_or_else(|| Phase2Error::invalid("phase2_capability_ref_array_invalid"))?;
            validate_capability_ref(value)?;
            Ok(value.to_string())
        })
        .collect()
}

fn validate_capability_ref_array(
    value: &Value,
    require_non_empty: bool,
) -> Result<(), Phase2Error> {
    capability_ref_array(value, require_non_empty).map(|_| ())
}

fn validate_capability_ref(value: &str) -> Result<(), Phase2Error> {
    const PREFIX: &str = "capability-ref://run/";
    if validate_ref(value).is_ok() {
        return Ok(());
    }
    let Some(suffix) = value.strip_prefix(PREFIX) else {
        return Err(Phase2Error::invalid("phase2_capability_ref_invalid"));
    };
    if suffix.len() < 3
        || suffix.len() > 128
        || !suffix.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        })
    {
        return Err(Phase2Error::invalid("phase2_capability_ref_invalid"));
    }
    Ok(())
}

fn unique_and_disjoint(left: &[String], right: &[String]) -> bool {
    let left_set: HashSet<&str> = left.iter().map(String::as_str).collect();
    let right_set: HashSet<&str> = right.iter().map(String::as_str).collect();
    left_set.len() == left.len()
        && right_set.len() == right.len()
        && left_set.is_disjoint(&right_set)
}

fn unique_and_disjoint_right(left: &[String], right: &[String]) -> bool {
    let left_set: HashSet<&str> = left.iter().map(String::as_str).collect();
    let right_set: HashSet<&str> = right.iter().map(String::as_str).collect();
    right_set.len() == right.len() && left_set.is_disjoint(&right_set)
}

fn is_unique(values: &[String]) -> bool {
    values
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>()
        .len()
        == values.len()
}

fn is_unique_subset(values: &[String], allowed: &[String]) -> bool {
    let values_set: HashSet<&str> = values.iter().map(String::as_str).collect();
    let allowed_set: HashSet<&str> = allowed.iter().map(String::as_str).collect();
    values_set.len() == values.len() && values_set.is_subset(&allowed_set)
}

fn require_equal(expected: &str, received: &str, reason: &'static str) -> Result<(), Phase2Error> {
    if expected != received {
        return Err(Phase2Error::conflict(reason));
    }
    Ok(())
}

fn require_equal_number(
    expected: u64,
    received: u64,
    reason: &'static str,
) -> Result<(), Phase2Error> {
    if expected != received {
        return Err(Phase2Error::conflict(reason));
    }
    Ok(())
}

fn forbidden_private_material(bytes: &[u8]) -> bool {
    let value = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    [
        "-----begin private key-----",
        "-----begin rsa private key-----",
        "authorization: bearer ",
        "\"authorization\":",
        "\"token\":",
        "\"apikey\":",
        "\"refreshtoken\":",
        "\"mnemonic\":",
        "\"password\":",
        "\"credential\":",
        "\"secret\":",
        "\"localpath\":",
        "\"hostname\":",
        "\"processid\":",
        "\"providersessionid\":",
        "\"transporthandle\":",
        "\"socket\":",
        "\"pid\":",
        "\"authhome\":",
        "/users/",
        "/home/",
        ":\\users\\",
        "ghp_",
        "github_pat_",
        "sk-proj-",
    ]
    .iter()
    .any(|marker| value.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn omissions() -> Value {
        json!({
            "credentials": "excluded",
            "accountSecrets": "excluded",
            "providerHiddenState": "excluded",
            "processMemory": "excluded",
            "processTable": "excluded",
            "ptyState": "excluded",
            "sockets": "excluded",
            "ports": "excluded",
            "networkIdentity": "excluded"
        })
    }

    fn checkpoint() -> Value {
        json!({
            "schema": CHECKPOINT_SCHEMA_VERSION,
            "checkpointRef": "checkpoint.sbx10.control",
            "ownerRef": "owner.sbx10.control",
            "tenantRef": "tenant.sbx10.control",
            "sourceSandboxRef": "sandbox.sbx10.control",
            "sourceResourceGeneration": 7,
            "sourceImageDigest": digest('a'),
            "sourceToolchainDigest": digest('b'),
            "repositoryRef": "repository.openagents",
            "repositoryRevisionRef": "commit.0ff3ee7",
            "repositoryPostImageDigest": digest('c'),
            "contentDigest": digest('d'),
            "contentBytes": 16384,
            "formatRef": "format.sbx.content-tar.v1",
            "state": "completed",
            "completedAt": "2026-07-22T03:31:01.000Z",
            "verifiedAt": "2026-07-22T03:31:02.000Z",
            "retainedUntil": "2026-07-23T03:31:00.000Z",
            "deleteOnExpiry": true,
            "omissions": omissions(),
            "evidenceRefs": ["receipt.sbx10.control.verify"]
        })
    }

    fn create_command(tag: &str) -> Value {
        let mut command = json!({
            "_tag": tag,
            "schema": COMMAND_SCHEMA_VERSION,
            "commandRef": "command.sbx10.control.create",
            "idempotencyRef": "idempotency.sbx10.control.create",
            "ownerRef": "owner.sbx10.control",
            "tenantRef": "tenant.sbx10.control",
            "requestedAt": "2026-07-22T03:31:00.000Z",
            "checkpointRef": "checkpoint.sbx10.control",
            "sourceSandboxRef": "sandbox.sbx10.control",
            "sourceResourceGeneration": 7,
            "sourceImageDigest": digest('a'),
            "sourceToolchainDigest": digest('b'),
            "repositoryRef": "repository.openagents",
            "repositoryRevisionRef": "commit.0ff3ee7",
            "repositoryPostImageDigest": digest('c'),
            "formatRef": "format.sbx.content-tar.v1",
            "retainedUntil": "2026-07-23T03:31:00.000Z"
        });
        if tag == "ArchiveWithCheckpoint" {
            command["commandRef"] = json!("command.sbx10.control.archive");
            command["stopRef"] = json!("stop.sbx10.control.archive");
        }
        command
    }

    fn fork_command() -> Value {
        json!({
            "_tag": "ForkFromCheckpoint",
            "schema": COMMAND_SCHEMA_VERSION,
            "commandRef": "command.sbx10.control.fork",
            "idempotencyRef": "idempotency.sbx10.control.fork",
            "ownerRef": "owner.sbx10.control",
            "tenantRef": "tenant.sbx10.control",
            "requestedAt": "2026-07-22T03:32:00.000Z",
            "checkpointRef": "checkpoint.sbx10.control",
            "expectedSourceSandboxRef": "sandbox.sbx10.control",
            "expectedSourceResourceGeneration": 7,
            "sourceCapabilityRefs": ["capability-ref://run/source-sbx10"]
        })
    }

    fn restore_command() -> Value {
        json!({
            "_tag": "RestoreCheckpoint",
            "schema": COMMAND_SCHEMA_VERSION,
            "commandRef": "command.sbx10.control.restore",
            "idempotencyRef": "idempotency.sbx10.control.restore",
            "ownerRef": "owner.sbx10.control",
            "tenantRef": "tenant.sbx10.control",
            "requestedAt": "2026-07-22T03:33:00.000Z",
            "checkpointRef": "checkpoint.sbx10.control",
            "destinationSandboxRef": "sandbox.sbx10.control.restore",
            "expectedSourceResourceGeneration": 7,
            "admittedServiceRefs": ["service.agent-runtime"],
            "sourceCapabilityRefs": ["capability-ref://run/source-sbx10"]
        })
    }

    fn delete_command() -> Value {
        json!({
            "_tag": "DeleteCheckpoint",
            "schema": COMMAND_SCHEMA_VERSION,
            "commandRef": "command.sbx10.control.delete",
            "idempotencyRef": "idempotency.sbx10.control.delete",
            "ownerRef": "owner.sbx10.control",
            "tenantRef": "tenant.sbx10.control",
            "requestedAt": "2026-07-22T03:34:00.000Z",
            "checkpointRef": "checkpoint.sbx10.control",
            "reason": "owner_requested"
        })
    }

    fn request(action: ManagedSandboxPhase2Action) -> ManagedSandboxPhase2Request {
        let (request_ref, command, checkpoint, owner_ref, tenant_ref, sandbox_ref) = match action {
            ManagedSandboxPhase2Action::CreateCheckpoint => (
                "command.sbx10.control.create",
                Some(create_command("CreateCheckpoint")),
                None,
                None,
                None,
                None,
            ),
            ManagedSandboxPhase2Action::ArchiveWithCheckpoint => (
                "command.sbx10.control.archive",
                Some(create_command("ArchiveWithCheckpoint")),
                None,
                None,
                None,
                None,
            ),
            ManagedSandboxPhase2Action::VerifyCheckpoint => (
                "checkpoint.sbx10.control",
                None,
                Some(checkpoint()),
                None,
                None,
                None,
            ),
            ManagedSandboxPhase2Action::ObserveResourceGeneration => (
                "sandbox.sbx10.control",
                None,
                None,
                Some("owner.sbx10.control".to_string()),
                Some("tenant.sbx10.control".to_string()),
                Some("sandbox.sbx10.control".to_string()),
            ),
            ManagedSandboxPhase2Action::ForkFromCheckpoint => (
                "command.sbx10.control.fork",
                Some(fork_command()),
                Some(checkpoint()),
                None,
                None,
                None,
            ),
            ManagedSandboxPhase2Action::RestoreCheckpoint => (
                "command.sbx10.control.restore",
                Some(restore_command()),
                Some(checkpoint()),
                None,
                None,
                None,
            ),
            ManagedSandboxPhase2Action::DeleteCheckpoint => (
                "command.sbx10.control.delete",
                Some(delete_command()),
                Some(checkpoint()),
                None,
                None,
                None,
            ),
            ManagedSandboxPhase2Action::CreatePrivateIngress
            | ManagedSandboxPhase2Action::RevokePrivateIngress
            | ManagedSandboxPhase2Action::ExpirePrivateIngress => {
                unreachable!("native private ingress uses dedicated test requests")
            }
        };
        ManagedSandboxPhase2Request {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action,
            request_ref: request_ref.to_string(),
            command,
            checkpoint,
            capability: None,
            owner_ref,
            tenant_ref,
            sandbox_ref,
        }
    }

    fn result(action: ManagedSandboxPhase2Action) -> Value {
        match action {
            ManagedSandboxPhase2Action::CreateCheckpoint => checkpoint(),
            ManagedSandboxPhase2Action::ArchiveWithCheckpoint => json!({
                "_tag": "CheckpointFailed",
                "schema": CHECKPOINT_STOP_SCHEMA_VERSION,
                "stopRef": "stop.sbx10.control.archive",
                "sandboxRef": "sandbox.sbx10.control",
                "resourceGeneration": 7,
                "attemptedCheckpointRef": "checkpoint.sbx10.control",
                "errorRef": "error.sbx10.checkpoint.partial",
                "lifecycle": "recovery_required",
                "archiveClaim": "forbidden",
                "observedAt": "2026-07-22T03:32:01.000Z",
                "evidenceRefs": ["receipt.sbx10.checkpoint.partial"]
            }),
            ManagedSandboxPhase2Action::VerifyCheckpoint => json!({
                "verified": true,
                "checkpointRef": "checkpoint.sbx10.control",
                "contentDigest": digest('d'),
                "evidenceRefs": ["receipt.sbx10.checkpoint.readback"]
            }),
            ManagedSandboxPhase2Action::ObserveResourceGeneration => json!({
                "ownerRef": "owner.sbx10.control",
                "tenantRef": "tenant.sbx10.control",
                "sandboxRef": "sandbox.sbx10.control",
                "resourceGeneration": 7,
                "evidenceRefs": ["receipt.sbx10.generation"]
            }),
            ManagedSandboxPhase2Action::ForkFromCheckpoint => json!({
                "schema": FORK_RECEIPT_SCHEMA_VERSION,
                "receiptRef": "receipt.sbx10.fork",
                "ownerRef": "owner.sbx10.control",
                "tenantRef": "tenant.sbx10.control",
                "checkpointRef": "checkpoint.sbx10.control",
                "sourceSandboxRef": "sandbox.sbx10.control",
                "sourceResourceGeneration": 7,
                "forkSandboxRef": "sandbox.sbx10.control.fork",
                "forkResourceGeneration": 1,
                "sourceCapabilityRefs": ["capability-ref://run/source-sbx10"],
                "forkCapabilityRefs": ["capability.sbx10.fork"],
                "grantPolicy": "mint_fresh",
                "cleanupObligationRef": "cleanup.sbx10.fork",
                "stateTransfer": omissions(),
                "processSessionContinuity": "none",
                "outcome": "created",
                "observedAt": "2026-07-22T03:32:01.000Z",
                "evidenceRefs": ["receipt.sbx10.fork.identity"]
            }),
            ManagedSandboxPhase2Action::RestoreCheckpoint => json!({
                "schema": RESTORE_RECEIPT_SCHEMA_VERSION,
                "receiptRef": "receipt.sbx10.restore",
                "ownerRef": "owner.sbx10.control",
                "tenantRef": "tenant.sbx10.control",
                "checkpointRef": "checkpoint.sbx10.control",
                "sandboxRef": "sandbox.sbx10.control.restore",
                "checkpointSourceGeneration": 7,
                "restoredResourceGeneration": 8,
                "admittedServiceRefs": ["service.agent-runtime"],
                "restartedServiceRefs": ["service.agent-runtime"],
                "sourceCapabilityRefs": ["capability-ref://run/source-sbx10"],
                "restoredCapabilityRefs": ["capability.sbx10.restore"],
                "grantPolicy": "mint_fresh",
                "processSessionContinuity": "discontinuous",
                "processMemoryRestored": false,
                "ptyRestored": false,
                "socketsRestored": false,
                "outcome": "restored",
                "observedAt": "2026-07-22T03:33:01.000Z",
                "evidenceRefs": ["receipt.sbx10.restore.service"]
            }),
            ManagedSandboxPhase2Action::DeleteCheckpoint => json!({
                "schema": CHECKPOINT_DELETE_SCHEMA_VERSION,
                "receiptRef": "receipt.sbx10.delete",
                "ownerRef": "owner.sbx10.control",
                "tenantRef": "tenant.sbx10.control",
                "checkpointRef": "checkpoint.sbx10.control",
                "sourceSandboxRef": "sandbox.sbx10.control",
                "sourceResourceGeneration": 7,
                "contentDigest": digest('d'),
                "contentDeleted": true,
                "outcome": "deleted",
                "reason": "owner_requested",
                "deletedAt": "2026-07-22T03:34:01.000Z",
                "evidenceRefs": ["receipt.sbx10.delete.object"]
            }),
            ManagedSandboxPhase2Action::CreatePrivateIngress
            | ManagedSandboxPhase2Action::RevokePrivateIngress
            | ManagedSandboxPhase2Action::ExpirePrivateIngress => {
                unreachable!("native private ingress does not use driver fixtures")
            }
        }
    }

    #[test]
    fn validates_all_seven_actions_and_exact_result_bindings() {
        for action in [
            ManagedSandboxPhase2Action::CreateCheckpoint,
            ManagedSandboxPhase2Action::ArchiveWithCheckpoint,
            ManagedSandboxPhase2Action::VerifyCheckpoint,
            ManagedSandboxPhase2Action::ObserveResourceGeneration,
            ManagedSandboxPhase2Action::ForkFromCheckpoint,
            ManagedSandboxPhase2Action::RestoreCheckpoint,
            ManagedSandboxPhase2Action::DeleteCheckpoint,
        ] {
            let request = request(action);
            request.validate().unwrap();
            let response = ManagedSandboxPhase2Response {
                schema_version: TARGET_SCHEMA_VERSION.to_string(),
                action,
                request_ref: request.request_ref.clone(),
                result: result(action),
            };
            validate_response(&request, &response).unwrap();
        }
    }

    #[test]
    fn rejects_excess_fields_scope_conflicts_and_private_material() {
        let mut excess = request(ManagedSandboxPhase2Action::CreateCheckpoint);
        excess.command.as_mut().unwrap()["unexpected"] = json!(true);
        assert!(excess.validate().is_err());

        let request = request(ManagedSandboxPhase2Action::VerifyCheckpoint);
        let mut response = ManagedSandboxPhase2Response {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action: request.action,
            request_ref: request.request_ref.clone(),
            result: result(request.action),
        };
        response.result["checkpointRef"] = json!("checkpoint.sbx10.other");
        assert_eq!(
            validate_response(&request, &response).unwrap_err().status,
            409
        );
        response.result["checkpointRef"] = json!("checkpoint.sbx10.control");
        response.result["unexpected"] = json!(true);
        assert_eq!(
            validate_response(&request, &response).unwrap_err().status,
            409
        );
        response
            .result
            .as_object_mut()
            .unwrap()
            .remove("unexpected");
        response.result["localPath"] = json!("/Users/private/database.sock");
        let encoded = serde_json::to_vec(&response).unwrap();
        assert!(forbidden_private_material(&encoded));
    }

    #[test]
    fn validates_native_private_ingress_commands_and_exact_capability_binding() {
        let create_command = json!({
            "_tag": "CreatePrivateIngress",
            "schema": COMMAND_SCHEMA_VERSION,
            "commandRef": "command.sbx10.ingress.create",
            "idempotencyRef": "idempotency.sbx10.ingress.create",
            "ownerRef": "owner.sbx10.control",
            "tenantRef": "tenant.sbx10.control",
            "requestedAt": "2026-07-22T03:35:00.000Z",
            "sandboxRef": "sandbox.sbx10.control",
            "resourceGeneration": 7,
            "audienceRef": "audience.sbx10.owner-device",
            "kind": "preview",
            "ttlSeconds": 300
        });
        let create = ManagedSandboxPhase2Request {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action: ManagedSandboxPhase2Action::CreatePrivateIngress,
            request_ref: "command.sbx10.ingress.create".to_string(),
            command: Some(create_command.clone()),
            checkpoint: None,
            capability: None,
            owner_ref: None,
            tenant_ref: None,
            sandbox_ref: None,
        };
        create.validate().unwrap();
        let mut long_lived = create;
        long_lived.command.as_mut().unwrap()["ttlSeconds"] = json!(901);
        assert!(long_lived.validate().is_err());

        let capability = json!({
            "_tag": "Active",
            "schema": "openagents.managed_sandbox_private_ingress.v1",
            "capabilityRef": "capability.sbx10.ingress.test",
            "sandboxRef": "sandbox.sbx10.control",
            "resourceGeneration": 7,
            "ownerRef": "owner.sbx10.control",
            "audienceRef": "audience.sbx10.owner-device",
            "kind": "preview",
            "issuedAt": "2026-07-22T03:35:00.000Z",
            "expiresAt": "2026-07-22T03:40:00.000Z",
            "ttlSeconds": 300,
            "accessUrlDigest": digest('e'),
            "accessUrlAtRest": "redacted",
            "audiencePolicy": "owner_scoped_explicit_audience",
            "publicAccess": false,
            "permanentRoute": false,
            "vnc": "unsupported",
            "auditRefs": ["audit.sbx10.ingress.create"]
        });
        let terminal_command = json!({
            "_tag": "RevokePrivateIngress",
            "schema": COMMAND_SCHEMA_VERSION,
            "commandRef": "command.sbx10.ingress.revoke",
            "idempotencyRef": "idempotency.sbx10.ingress.revoke",
            "ownerRef": "owner.sbx10.control",
            "tenantRef": "tenant.sbx10.control",
            "requestedAt": "2026-07-22T03:36:00.000Z",
            "capabilityRef": "capability.sbx10.ingress.test",
            "sandboxRef": "sandbox.sbx10.control",
            "resourceGeneration": 7
        });
        let terminal = ManagedSandboxPhase2Request {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action: ManagedSandboxPhase2Action::RevokePrivateIngress,
            request_ref: "command.sbx10.ingress.revoke".to_string(),
            command: Some(terminal_command),
            checkpoint: None,
            capability: Some(capability),
            owner_ref: None,
            tenant_ref: None,
            sandbox_ref: None,
        };
        terminal.validate().unwrap();
        let mut mismatch = terminal;
        mismatch.capability.as_mut().unwrap()["resourceGeneration"] = json!(8);
        assert!(mismatch.validate().is_err());
    }

    #[test]
    fn rejects_a_relative_driver_path() {
        let error = validate_driver_path(Path::new("driver")).unwrap_err();
        assert_eq!(error.status, 503);
        assert_eq!(error.reason_ref, "phase2_driver_path_not_absolute");
    }

    #[cfg(unix)]
    #[test]
    fn executes_one_bounded_driver_without_exposing_stderr() {
        let root = temp_dir("success");
        let driver = root.join("driver.sh");
        let capture = root.join("request.json");
        let request = request(ManagedSandboxPhase2Action::VerifyCheckpoint);
        let response = ManagedSandboxPhase2Response {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action: request.action,
            request_ref: request.request_ref.clone(),
            result: result(request.action),
        };
        let response_json = serde_json::to_string(&response).unwrap();
        fs::write(
            &driver,
            format!(
                "#!/bin/sh\ncat >\"{}\"\nprintf '%s' '{}'\nprintf 'private /Users/owner/db.sock' >&2\n",
                capture.display(),
                response_json
            ),
        )
        .unwrap();
        fs::set_permissions(&driver, fs::Permissions::from_mode(0o700)).unwrap();

        let received =
            execute_with_driver(&driver, request.clone(), Duration::from_secs(2)).unwrap();
        assert_eq!(received, response);
        let captured: ManagedSandboxPhase2Request =
            serde_json::from_slice(&fs::read(&capture).unwrap()).unwrap();
        assert_eq!(captured, request);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn archive_captures_before_the_generation_fenced_runtime_stop() {
        let root = temp_dir("archive");
        let driver = root.join("driver.sh");
        let response = ManagedSandboxPhase2Response {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action: ManagedSandboxPhase2Action::CreateCheckpoint,
            request_ref: "command.sbx10.control.archive".to_string(),
            result: checkpoint(),
        };
        fs::write(
            &driver,
            format!(
                "#!/bin/sh\ncat >/dev/null\nprintf '%s' '{}'\n",
                serde_json::to_string(&response).unwrap()
            ),
        )
        .unwrap();
        fs::set_permissions(&driver, fs::Permissions::from_mode(0o700)).unwrap();

        let archived = execute_archive_with_checkpoint_and_stop(
            &driver,
            request(ManagedSandboxPhase2Action::ArchiveWithCheckpoint),
            Duration::from_secs(2),
            |owner, tenant, sandbox, generation, stop_ref| {
                assert_eq!(owner, "owner.sbx10.control");
                assert_eq!(tenant, "tenant.sbx10.control");
                assert_eq!(sandbox, "sandbox.sbx10.control");
                assert_eq!(generation, 7);
                assert_eq!(stop_ref, "stop.sbx10.control.archive");
                Ok((RuntimePhase::Stopped, 7))
            },
        )
        .unwrap();
        assert_eq!(archived.result["_tag"], "Archived");
        assert_eq!(archived.result["archiveClaim"], "allowed");
        assert_eq!(archived.result["checkpoint"], checkpoint());

        let archive_request = request(ManagedSandboxPhase2Action::ArchiveWithCheckpoint);
        let cleanup_request = archive_cleanup_request(
            archive_request
                .command
                .as_ref()
                .unwrap()
                .as_object()
                .unwrap(),
            checkpoint(),
        )
        .unwrap();
        let cleanup_response = ManagedSandboxPhase2Response {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action: ManagedSandboxPhase2Action::DeleteCheckpoint,
            request_ref: cleanup_request.request_ref.clone(),
            result: json!({
                "schema": CHECKPOINT_DELETE_SCHEMA_VERSION,
                "receiptRef": "receipt.sbx10.archive.cleanup",
                "ownerRef": "owner.sbx10.control",
                "tenantRef": "tenant.sbx10.control",
                "checkpointRef": "checkpoint.sbx10.control",
                "sourceSandboxRef": "sandbox.sbx10.control",
                "sourceResourceGeneration": 7,
                "contentDigest": digest('d'),
                "contentDeleted": true,
                "outcome": "deleted",
                "reason": "sandbox_teardown",
                "deletedAt": "2026-07-22T05:00:03.000Z",
                "evidenceRefs": ["evidence.sbx10.archive.cleanup"]
            }),
        };
        fs::write(
            &driver,
            format!(
                "#!/bin/sh\ninput=$(cat)\nif printf '%s' \"$input\" | grep -q '\"action\":\"delete_checkpoint\"'; then printf '%s' '{}'; else printf '%s' '{}'; fi\n",
                serde_json::to_string(&cleanup_response).unwrap(),
                serde_json::to_string(&response).unwrap(),
            ),
        )
        .unwrap();
        fs::set_permissions(&driver, fs::Permissions::from_mode(0o700)).unwrap();

        let failed = execute_archive_with_checkpoint_and_stop(
            &driver,
            archive_request,
            Duration::from_secs(2),
            |_, _, _, _, _| Err(()),
        )
        .unwrap();
        assert_eq!(failed.result["_tag"], "CheckpointFailed");
        assert_eq!(failed.result["archiveClaim"], "forbidden");
        assert_eq!(failed.result["lifecycle"], "recovery_required");
        assert_eq!(
            failed.result["evidenceRefs"],
            json!([
                "stop.sbx10.control.archive",
                "receipt.sbx10.archive.cleanup"
            ])
        );
        assert_eq!(
            failed.result["errorRef"],
            "error.sbx10.archive.stop_failed_checkpoint_cleaned"
        );

        fs::write(
            &driver,
            format!(
                "#!/bin/sh\ninput=$(cat)\nif printf '%s' \"$input\" | grep -q '\"action\":\"delete_checkpoint\"'; then exit 1; else printf '%s' '{}'; fi\n",
                serde_json::to_string(&response).unwrap(),
            ),
        )
        .unwrap();
        fs::set_permissions(&driver, fs::Permissions::from_mode(0o700)).unwrap();
        let cleanup_failed = execute_archive_with_checkpoint_and_stop(
            &driver,
            request(ManagedSandboxPhase2Action::ArchiveWithCheckpoint),
            Duration::from_secs(2),
            |_, _, _, _, _| Err(()),
        )
        .unwrap_err();
        assert_eq!(cleanup_failed.reason_ref, "phase2_archive_cleanup_failed");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn fork_sends_only_the_prepared_runtime_context_and_finalizes() {
        let root = temp_dir("fork");
        let driver = root.join("driver.sh");
        let capture = root.join("request.json");
        let response = ManagedSandboxPhase2Response {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action: ManagedSandboxPhase2Action::ForkFromCheckpoint,
            request_ref: "command.sbx10.control.fork".to_string(),
            result: result(ManagedSandboxPhase2Action::ForkFromCheckpoint),
        };
        fs::write(
            &driver,
            format!(
                "#!/bin/sh\ncat >'{}'\nprintf '%s' '{}'\n",
                capture.display(),
                serde_json::to_string(&response).unwrap()
            ),
        )
        .unwrap();
        fs::set_permissions(&driver, fs::Permissions::from_mode(0o700)).unwrap();
        let forked = execute_fork_from_checkpoint_with(
            &driver,
            request(ManagedSandboxPhase2Action::ForkFromCheckpoint),
            Duration::from_secs(2),
            |owner, tenant, source, generation, command_ref, checkpoint_ref, source_grants| {
                assert_eq!(owner, "owner.sbx10.control");
                assert_eq!(tenant, "tenant.sbx10.control");
                assert_eq!(source, "sandbox.sbx10.control");
                assert_eq!(generation, 7);
                assert_eq!(command_ref, "command.sbx10.control.fork");
                assert_eq!(checkpoint_ref, "checkpoint.sbx10.control");
                assert_eq!(source_grants, ["capability-ref://run/source-sbx10"]);
                Ok(managed_sandbox_runtime::CheckpointForkContext {
                    schema: "openagents.managed_sandbox_phase2_fork_context.v1",
                    owner_ref: owner.to_string(),
                    tenant_ref: tenant.to_string(),
                    source_sandbox_ref: source.to_string(),
                    source_resource_generation: generation,
                    fork_sandbox_ref: "sandbox.sbx10.control.fork".to_string(),
                    fork_resource_generation: 1,
                    fork_capability_refs: vec!["capability.sbx10.fork".to_string()],
                    cleanup_obligation_ref: "cleanup.sbx10.fork".to_string(),
                })
            },
            |owner, tenant, sandbox, command_ref, succeeded| {
                assert_eq!(owner, "owner.sbx10.control");
                assert_eq!(tenant, "tenant.sbx10.control");
                assert_eq!(sandbox, "sandbox.sbx10.control.fork");
                assert_eq!(command_ref, "command.sbx10.control.fork");
                assert!(succeeded);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(forked, response);
        let wire: Value = serde_json::from_slice(&fs::read(capture).unwrap()).unwrap();
        assert_eq!(
            wire["runtimeContext"],
            json!({
                "schema": "openagents.managed_sandbox_phase2_fork_context.v1",
                "ownerRef": "owner.sbx10.control",
                "tenantRef": "tenant.sbx10.control",
                "sourceSandboxRef": "sandbox.sbx10.control",
                "sourceResourceGeneration": 7,
                "forkSandboxRef": "sandbox.sbx10.control.fork",
                "forkResourceGeneration": 1,
                "forkCapabilityRefs": ["capability.sbx10.fork"],
                "cleanupObligationRef": "cleanup.sbx10.fork"
            })
        );
        assert!(wire.get("localPath").is_none());

        fs::write(&driver, "#!/bin/sh\ncat >/dev/null\nexit 1\n").unwrap();
        let finalized = std::cell::Cell::new(None);
        let failure = execute_fork_from_checkpoint_with(
            &driver,
            request(ManagedSandboxPhase2Action::ForkFromCheckpoint),
            Duration::from_secs(2),
            |owner, tenant, source, generation, _, _, _| {
                Ok(managed_sandbox_runtime::CheckpointForkContext {
                    schema: "openagents.managed_sandbox_phase2_fork_context.v1",
                    owner_ref: owner.to_string(),
                    tenant_ref: tenant.to_string(),
                    source_sandbox_ref: source.to_string(),
                    source_resource_generation: generation,
                    fork_sandbox_ref: "sandbox.sbx10.control.fork".to_string(),
                    fork_resource_generation: 1,
                    fork_capability_refs: vec!["capability.sbx10.fork".to_string()],
                    cleanup_obligation_ref: "cleanup.sbx10.fork".to_string(),
                })
            },
            |_, _, _, _, succeeded| {
                finalized.set(Some(succeeded));
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(failure.reason_ref, "phase2_driver_refused");
        assert_eq!(finalized.get(), Some(false));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn restore_sends_only_the_prepared_runtime_context_and_finalizes() {
        let root = temp_dir("restore");
        let driver = root.join("driver.sh");
        let capture = root.join("request.json");
        let response = ManagedSandboxPhase2Response {
            schema_version: TARGET_SCHEMA_VERSION.to_string(),
            action: ManagedSandboxPhase2Action::RestoreCheckpoint,
            request_ref: "command.sbx10.control.restore".to_string(),
            result: result(ManagedSandboxPhase2Action::RestoreCheckpoint),
        };
        fs::write(
            &driver,
            format!(
                "#!/bin/sh\ncat >'{}'\nprintf '%s' '{}'\n",
                capture.display(),
                serde_json::to_string(&response).unwrap()
            ),
        )
        .unwrap();
        fs::set_permissions(&driver, fs::Permissions::from_mode(0o700)).unwrap();
        let restored = execute_restore_checkpoint_with(
            &driver,
            request(ManagedSandboxPhase2Action::RestoreCheckpoint),
            Duration::from_secs(2),
            |owner, tenant, sandbox, command_ref, checkpoint_ref, generation, source| {
                assert_eq!(owner, "owner.sbx10.control");
                assert_eq!(tenant, "tenant.sbx10.control");
                assert_eq!(sandbox, "sandbox.sbx10.control.restore");
                assert_eq!(command_ref, "command.sbx10.control.restore");
                assert_eq!(checkpoint_ref, "checkpoint.sbx10.control");
                assert_eq!(generation, 7);
                assert_eq!(source, ["capability-ref://run/source-sbx10"]);
                Ok(managed_sandbox_runtime::CheckpointRestoreContext {
                    schema: "openagents.managed_sandbox_phase2_restore_context.v1",
                    owner_ref: owner.to_string(),
                    tenant_ref: tenant.to_string(),
                    sandbox_ref: sandbox.to_string(),
                    resource_generation: 8,
                    restored_capability_refs: vec!["capability.sbx10.restore".to_string()],
                })
            },
            |owner, tenant, sandbox, command_ref, succeeded| {
                assert_eq!(owner, "owner.sbx10.control");
                assert_eq!(tenant, "tenant.sbx10.control");
                assert_eq!(sandbox, "sandbox.sbx10.control.restore");
                assert_eq!(command_ref, "command.sbx10.control.restore");
                assert!(succeeded);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(restored, response);
        let wire: Value = serde_json::from_slice(&fs::read(capture).unwrap()).unwrap();
        assert_eq!(
            wire["runtimeContext"],
            json!({
                "schema": "openagents.managed_sandbox_phase2_restore_context.v1",
                "ownerRef": "owner.sbx10.control",
                "tenantRef": "tenant.sbx10.control",
                "sandboxRef": "sandbox.sbx10.control.restore",
                "resourceGeneration": 8,
                "restoredCapabilityRefs": ["capability.sbx10.restore"]
            })
        );
        assert!(wire.get("localPath").is_none());

        fs::write(&driver, "#!/bin/sh\ncat >/dev/null\nexit 1\n").unwrap();
        let finalized = std::cell::Cell::new(None);
        let failure = execute_restore_checkpoint_with(
            &driver,
            request(ManagedSandboxPhase2Action::RestoreCheckpoint),
            Duration::from_secs(2),
            |owner, tenant, sandbox, _, _, _, _| {
                Ok(managed_sandbox_runtime::CheckpointRestoreContext {
                    schema: "openagents.managed_sandbox_phase2_restore_context.v1",
                    owner_ref: owner.to_string(),
                    tenant_ref: tenant.to_string(),
                    sandbox_ref: sandbox.to_string(),
                    resource_generation: 8,
                    restored_capability_refs: vec!["capability.sbx10.restore".to_string()],
                })
            },
            |_, _, _, _, succeeded| {
                finalized.set(Some(succeeded));
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(failure.reason_ref, "phase2_driver_refused");
        assert_eq!(finalized.get(), Some(false));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn kills_a_driver_that_exceeds_the_fixed_deadline() {
        let root = temp_dir("timeout");
        let driver = root.join("driver.sh");
        fs::write(&driver, "#!/bin/sh\ncat >/dev/null\nsleep 5\n").unwrap();
        fs::set_permissions(&driver, fs::Permissions::from_mode(0o700)).unwrap();
        let error = execute_with_driver(
            &driver,
            request(ManagedSandboxPhase2Action::VerifyCheckpoint),
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert_eq!(error.status, 503);
        assert_eq!(error.reason_ref, "phase2_driver_timed_out");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounds_driver_output_and_redacts_a_failed_driver() {
        let oversized_root = temp_dir("oversized");
        let oversized_driver = oversized_root.join("driver.sh");
        fs::write(
            &oversized_driver,
            "#!/bin/sh\ncat >/dev/null\ndd if=/dev/zero bs=1048577 count=1 2>/dev/null\n",
        )
        .unwrap();
        fs::set_permissions(&oversized_driver, fs::Permissions::from_mode(0o700)).unwrap();
        let oversized = execute_with_driver(
            &oversized_driver,
            request(ManagedSandboxPhase2Action::VerifyCheckpoint),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert_eq!(oversized.status, 503);
        assert_eq!(oversized.reason_ref, "phase2_driver_response_too_large");
        fs::remove_dir_all(oversized_root).unwrap();

        let refused_root = temp_dir("refused");
        let refused_driver = refused_root.join("driver.sh");
        fs::write(
            &refused_driver,
            "#!/bin/sh\ncat >/dev/null\nprintf 'private /Users/owner/db.sock' >&2\nexit 1\n",
        )
        .unwrap();
        fs::set_permissions(&refused_driver, fs::Permissions::from_mode(0o700)).unwrap();
        let refused = execute_with_driver(
            &refused_driver,
            request(ManagedSandboxPhase2Action::VerifyCheckpoint),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert_eq!(refused.status, 503);
        assert_eq!(refused.reason_ref, "phase2_driver_refused");
        assert!(!serde_json::to_string(&refused.response())
            .unwrap()
            .contains("Users"));
        fs::remove_dir_all(refused_root).unwrap();

        let classified_root = temp_dir("classified-refusal");
        let classified_driver = classified_root.join("driver.sh");
        fs::write(
            &classified_driver,
            "#!/bin/sh\ncat >/dev/null\nprintf '%s' '{\"schemaVersion\":\"openagents.managed_sandbox_phase2_driver_error.v1\",\"reasonRef\":\"guest_checkpoint_cleanup_failed\"}'\nexit 2\n",
        )
        .unwrap();
        fs::set_permissions(&classified_driver, fs::Permissions::from_mode(0o700)).unwrap();
        let classified = execute_with_driver(
            &classified_driver,
            request(ManagedSandboxPhase2Action::VerifyCheckpoint),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert_eq!(
            classified.reason_ref,
            "phase2_driver_guest_checkpoint_cleanup_failed"
        );
        fs::remove_dir_all(classified_root).unwrap();

        assert_eq!(
            driver_refusal_reason(
                br#"{"schemaVersion":"openagents.managed_sandbox_phase2_driver_error.v1","reasonRef":"private /Users/owner"}"#,
            ),
            "phase2_driver_refused"
        );
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let root = env::temp_dir().join(format!(
            "oa-sbx10-phase2-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
