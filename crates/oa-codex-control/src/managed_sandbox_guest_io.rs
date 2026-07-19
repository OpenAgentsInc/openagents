//! Fail-closed guest I/O adapter for one exact managed-sandbox generation.
//!
//! The configured executable owns the guest transport and must apply no-follow,
//! beneath-root file access plus process-tree cleanup. This adapter validates
//! every request and receipt, bounds driver lifetime/output, and never invokes
//! a shell on the control host.

use std::env;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const SCHEMA_VERSION: &str = "openagents.managed_sandbox_guest_io.v1";
const RECEIPT_SCHEMA_VERSION: &str = "openagents.managed_sandbox_guest_io_receipt.v1";
const ARTIFACT_RECEIPT_SCHEMA_VERSION: &str = "openagents.managed_sandbox_artifact_receipt.v1";
const MAX_FILE_BYTES: u64 = 1_048_576;
const MAX_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: u64 = 256 * 1024;
const MAX_DURATION_MILLIS: u64 = 60 * 60 * 1_000;
const MAX_PROCESSES: u64 = 64;
const MAX_DRIVER_RESPONSE_BYTES: u64 = 24 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestIoAction {
    ReadFile,
    WriteFile,
    ExecuteCommand,
    ReadArtifact,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuestIoLimits {
    pub workspace_root_ref: String,
    pub max_file_bytes: u64,
    pub max_artifact_bytes: u64,
    pub max_output_bytes: u64,
    pub max_duration_millis: u64,
    pub max_cpu_millis: u64,
    pub max_processes: u64,
    pub max_network_bytes: u64,
    pub network_policy_ref: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedSandboxGuestIoRequest {
    pub schema_version: String,
    pub action: GuestIoAction,
    pub operation_ref: String,
    pub idempotency_ref: String,
    pub actor_ref: String,
    pub owner_ref: String,
    pub tenant_ref: String,
    pub program_ref: String,
    pub work_unit_ref: String,
    pub sandbox_ref: String,
    pub resource_generation: u64,
    pub capability_ref: String,
    pub capability_state: String,
    pub capability_expires_at: String,
    pub requested_at: String,
    pub limits: GuestIoLimits,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub content_digest: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub command_digest: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub timeout_millis: Option<u64>,
    #[serde(default)]
    pub retention_until: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuestIoReceipt {
    pub schema_version: String,
    pub receipt_ref: String,
    pub operation_ref: String,
    pub sandbox_ref: String,
    pub resource_generation: u64,
    pub capability_ref: String,
    pub action: GuestIoAction,
    pub outcome: String,
    pub path_digest: String,
    pub started_at: String,
    pub finished_at: String,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub cpu_millis: u64,
    pub network_bytes: u64,
    #[serde(default)]
    pub process_ref: Option<String>,
    pub process_terminated: bool,
    pub descendants_remaining: u64,
    pub scratch_cleaned: bool,
    pub ingress_closed: bool,
    pub egress_denied: bool,
    pub path_policy: String,
    pub symlink_traversal: bool,
    pub secret_scan: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactReceipt {
    pub schema_version: String,
    pub artifact_ref: String,
    pub content_digest: String,
    pub byte_length: u64,
    pub source_generation: u64,
    pub source_path_digest: String,
    pub retention_until: String,
    pub content_type: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedSandboxGuestIoResponse {
    pub schema_version: String,
    pub action: GuestIoAction,
    pub operation_ref: String,
    pub sandbox_ref: String,
    pub resource_generation: u64,
    pub receipt: GuestIoReceipt,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub content_digest: Option<String>,
    #[serde(default)]
    pub byte_length: Option<u64>,
    #[serde(default)]
    pub binary: Option<bool>,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub signal: Option<String>,
    #[serde(default)]
    pub stdout: Option<String>,
    #[serde(default)]
    pub stderr: Option<String>,
    #[serde(default)]
    pub stdout_truncated: Option<bool>,
    #[serde(default)]
    pub stderr_truncated: Option<bool>,
    #[serde(default)]
    pub timed_out: Option<bool>,
    #[serde(default)]
    pub cancelled: Option<bool>,
    #[serde(default)]
    pub duration_millis: Option<u64>,
    #[serde(default)]
    pub max_processes_observed: Option<u64>,
    #[serde(default)]
    pub content_base64: Option<String>,
    #[serde(default)]
    pub artifact: Option<ArtifactReceipt>,
}

#[derive(Debug)]
pub struct GuestIoError {
    status: u16,
    code: &'static str,
    reason_ref: &'static str,
}

impl GuestIoError {
    fn new(status: u16, code: &'static str, reason_ref: &'static str) -> Self {
        Self {
            status,
            code,
            reason_ref,
        }
    }

    fn invalid(reason_ref: &'static str) -> Self {
        Self::new(400, "invalid_request", reason_ref)
    }

    fn conflict(reason_ref: &'static str) -> Self {
        Self::new(409, "guest_io_conflict", reason_ref)
    }

    fn unavailable(reason_ref: &'static str) -> Self {
        Self::new(503, "guest_io_unavailable", reason_ref)
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn response(&self) -> Value {
        json!({
            "schemaVersion": "openagents.managed_sandbox_guest_io_error.v1",
            "code": self.code,
            "reasonRef": self.reason_ref,
            "retryable": self.status >= 500,
        })
    }
}

impl ManagedSandboxGuestIoRequest {
    fn validate(&self) -> Result<(), GuestIoError> {
        if self.schema_version != SCHEMA_VERSION || self.resource_generation == 0 {
            return Err(GuestIoError::invalid(
                "guest_io_schema_or_generation_invalid",
            ));
        }
        for value in [
            &self.operation_ref,
            &self.idempotency_ref,
            &self.actor_ref,
            &self.owner_ref,
            &self.tenant_ref,
            &self.program_ref,
            &self.work_unit_ref,
            &self.sandbox_ref,
            &self.capability_ref,
            &self.limits.workspace_root_ref,
            &self.limits.network_policy_ref,
        ] {
            validate_ref(value)?;
        }
        validate_timestamp(&self.requested_at)?;
        validate_timestamp(&self.capability_expires_at)?;
        if self.capability_state != "active" || self.capability_expires_at <= self.requested_at {
            return Err(GuestIoError::new(
                403,
                "capability_denied",
                "guest_io_capability_expired_or_revoked",
            ));
        }
        self.limits.validate()?;
        match self.action {
            GuestIoAction::ReadFile => {
                validate_path(required(&self.path, "read_path_required")?)?;
                validate_encoding(required(&self.encoding, "read_encoding_required")?)?;
                self.require_absent(&[
                    &self.content,
                    &self.content_digest,
                    &self.command,
                    &self.command_digest,
                    &self.cwd,
                    &self.retention_until,
                ])?;
                if self.timeout_millis.is_some() {
                    return Err(GuestIoError::invalid("read_payload_invalid"));
                }
            }
            GuestIoAction::WriteFile => {
                validate_path(required(&self.path, "write_path_required")?)?;
                let encoding = required(&self.encoding, "write_encoding_required")?;
                validate_encoding(encoding)?;
                let content = required(&self.content, "write_content_required")?;
                let bytes = decode_content(encoding, content)?;
                if bytes.len() as u64 > self.limits.max_file_bytes
                    || digest(&bytes) != *required(&self.content_digest, "write_digest_required")?
                    || forbidden_material(&bytes)
                {
                    return Err(GuestIoError::invalid("write_content_refused"));
                }
                self.require_absent(&[
                    &self.command,
                    &self.command_digest,
                    &self.cwd,
                    &self.retention_until,
                ])?;
                if self.timeout_millis.is_some() {
                    return Err(GuestIoError::invalid("write_payload_invalid"));
                }
            }
            GuestIoAction::ExecuteCommand => {
                let command = required(&self.command, "command_required")?;
                if command.trim().is_empty()
                    || command.len() > 16_384
                    || forbidden_material(command.as_bytes())
                    || digest(command.as_bytes())
                        != *required(&self.command_digest, "command_digest_required")?
                {
                    return Err(GuestIoError::invalid("command_refused"));
                }
                validate_path(required(&self.cwd, "command_cwd_required")?)?;
                let timeout = self
                    .timeout_millis
                    .ok_or_else(|| GuestIoError::invalid("command_timeout_required"))?;
                if timeout == 0 || timeout > self.limits.max_duration_millis {
                    return Err(GuestIoError::invalid("command_timeout_out_of_bounds"));
                }
                self.require_absent(&[
                    &self.path,
                    &self.encoding,
                    &self.content,
                    &self.content_digest,
                    &self.retention_until,
                ])?;
            }
            GuestIoAction::ReadArtifact => {
                validate_path(required(&self.path, "artifact_path_required")?)?;
                validate_timestamp(required(
                    &self.retention_until,
                    "artifact_retention_required",
                )?)?;
                self.require_absent(&[
                    &self.encoding,
                    &self.content,
                    &self.content_digest,
                    &self.command,
                    &self.command_digest,
                    &self.cwd,
                ])?;
                if self.timeout_millis.is_some() {
                    return Err(GuestIoError::invalid("artifact_payload_invalid"));
                }
            }
        }
        Ok(())
    }

    fn require_absent(&self, values: &[&Option<String>]) -> Result<(), GuestIoError> {
        if values.iter().any(|value| value.is_some()) {
            Err(GuestIoError::invalid("guest_io_action_payload_conflict"))
        } else {
            Ok(())
        }
    }

    fn effective_path(&self) -> &str {
        self.path
            .as_deref()
            .or(self.cwd.as_deref())
            .expect("validated guest I/O path")
    }
}

impl GuestIoLimits {
    fn validate(&self) -> Result<(), GuestIoError> {
        if self.max_file_bytes == 0
            || self.max_file_bytes > MAX_FILE_BYTES
            || self.max_artifact_bytes == 0
            || self.max_artifact_bytes > MAX_ARTIFACT_BYTES
            || self.max_output_bytes == 0
            || self.max_output_bytes > MAX_OUTPUT_BYTES
            || self.max_duration_millis == 0
            || self.max_duration_millis > MAX_DURATION_MILLIS
            || self.max_cpu_millis == 0
            || self.max_cpu_millis > self.max_duration_millis
            || self.max_processes == 0
            || self.max_processes > MAX_PROCESSES
            || self.max_network_bytes != 0
            || self.network_policy_ref != "network-policy.managed-sandbox.deny-all"
        {
            return Err(GuestIoError::invalid("guest_io_limits_not_admitted"));
        }
        Ok(())
    }
}

pub fn execute(
    request: ManagedSandboxGuestIoRequest,
) -> Result<ManagedSandboxGuestIoResponse, GuestIoError> {
    request.validate()?;
    let driver = env::var("OA_MANAGED_SANDBOX_IO_DRIVER")
        .map_err(|_| GuestIoError::unavailable("guest_io_driver_not_configured"))?;
    if !Path::new(&driver).is_absolute() {
        return Err(GuestIoError::unavailable(
            "guest_io_driver_path_not_absolute",
        ));
    }
    execute_with_driver(Path::new(&driver), request)
}

fn execute_with_driver(
    driver: &Path,
    request: ManagedSandboxGuestIoRequest,
) -> Result<ManagedSandboxGuestIoResponse, GuestIoError> {
    let request_bytes = serde_json::to_vec(&request)
        .map_err(|_| GuestIoError::unavailable("guest_io_request_encode_failed"))?;
    let mut command = Command::new(driver);
    command
        .arg("--managed-sandbox-guest-io")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|_| GuestIoError::unavailable("guest_io_driver_spawn_failed"))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| GuestIoError::unavailable("guest_io_driver_stdin_missing"))?
        .write_all(&request_bytes)
        .map_err(|_| GuestIoError::unavailable("guest_io_driver_write_failed"))?;
    drop(child.stdin.take());
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| GuestIoError::unavailable("guest_io_driver_stdout_missing"))?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .by_ref()
            .take(MAX_DRIVER_RESPONSE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let allowed = request
        .timeout_millis
        .unwrap_or(30_000)
        .saturating_add(2_000)
        .min(MAX_DURATION_MILLIS + 2_000);
    let deadline = Instant::now() + Duration::from_millis(allowed);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                kill_process_tree(&mut child);
                let _ = child.wait();
                let _ = reader.join();
                return Err(GuestIoError::unavailable("guest_io_driver_timed_out"));
            }
            Err(_) => {
                kill_process_tree(&mut child);
                let _ = child.wait();
                let _ = reader.join();
                return Err(GuestIoError::unavailable("guest_io_driver_wait_failed"));
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| GuestIoError::unavailable("guest_io_driver_read_panicked"))?
        .map_err(|_| GuestIoError::unavailable("guest_io_driver_read_failed"))?;
    if !status.success() || bytes.len() as u64 > MAX_DRIVER_RESPONSE_BYTES {
        return Err(GuestIoError::unavailable("guest_io_driver_refused"));
    }
    let response: ManagedSandboxGuestIoResponse = serde_json::from_slice(&bytes)
        .map_err(|_| GuestIoError::unavailable("guest_io_driver_response_invalid"))?;
    validate_response(&request, &response)?;
    Ok(response)
}

fn kill_process_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        let _ = libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
}

fn validate_response(
    request: &ManagedSandboxGuestIoRequest,
    response: &ManagedSandboxGuestIoResponse,
) -> Result<(), GuestIoError> {
    if response.schema_version != SCHEMA_VERSION
        || response.action != request.action
        || response.operation_ref != request.operation_ref
        || response.sandbox_ref != request.sandbox_ref
        || response.resource_generation != request.resource_generation
    {
        return Err(GuestIoError::conflict("guest_io_response_scope_conflict"));
    }
    validate_receipt(request, &response.receipt)?;
    match request.action {
        GuestIoAction::ReadFile => {
            let encoding = required(&response.encoding, "read_encoding_missing")?;
            if encoding != required(&request.encoding, "read_encoding_required")? {
                return Err(GuestIoError::conflict("read_encoding_conflict"));
            }
            let content = required(&response.content, "read_content_missing")?;
            let bytes = decode_content(encoding, content)?;
            let length = response
                .byte_length
                .ok_or_else(|| GuestIoError::conflict("read_length_missing"))?;
            if bytes.len() as u64 != length
                || length > request.limits.max_file_bytes
                || digest(&bytes) != *required(&response.content_digest, "read_digest_missing")?
                || forbidden_material(&bytes)
                || response.binary.is_none()
                || (response.binary.unwrap_or(false) && encoding != "base64")
                || response.receipt.bytes_read != length
            {
                return Err(GuestIoError::conflict("read_content_receipt_conflict"));
            }
        }
        GuestIoAction::WriteFile => {
            let content = required(&request.content, "write_content_required")?;
            let encoding = required(&request.encoding, "write_encoding_required")?;
            let length = decode_content(encoding, content)?.len() as u64;
            if response.byte_length != Some(length)
                || response.content_digest != request.content_digest
                || response.receipt.bytes_written != length
            {
                return Err(GuestIoError::conflict("write_receipt_conflict"));
            }
        }
        GuestIoAction::ExecuteCommand => {
            let stdout = required(&response.stdout, "command_stdout_missing")?;
            let stderr = required(&response.stderr, "command_stderr_missing")?;
            let duration = response
                .duration_millis
                .ok_or_else(|| GuestIoError::conflict("command_duration_missing"))?;
            let processes = response
                .max_processes_observed
                .ok_or_else(|| GuestIoError::conflict("command_process_count_missing"))?;
            if response.success.is_none()
                || response.stdout_truncated.is_none()
                || response.stderr_truncated.is_none()
                || response.timed_out.is_none()
                || response.cancelled.is_none()
                || stdout.len().saturating_add(stderr.len()) as u64
                    > request.limits.max_output_bytes
                || forbidden_material(stdout.as_bytes())
                || forbidden_material(stderr.as_bytes())
                || duration > request.timeout_millis.unwrap_or(0)
                || processes > request.limits.max_processes
                || response.timed_out != response.cancelled
                || response.receipt.process_ref.is_none()
            {
                return Err(GuestIoError::conflict("command_receipt_conflict"));
            }
        }
        GuestIoAction::ReadArtifact => {
            let encoded = required(&response.content_base64, "artifact_content_missing")?;
            let bytes = BASE64
                .decode(encoded)
                .map_err(|_| GuestIoError::conflict("artifact_base64_invalid"))?;
            let artifact = response
                .artifact
                .as_ref()
                .ok_or_else(|| GuestIoError::conflict("artifact_receipt_missing"))?;
            let content_digest = digest(&bytes);
            let expected_ref = format!("artifact.sha256.{}", &content_digest[7..]);
            if bytes.len() as u64 > request.limits.max_artifact_bytes
                || forbidden_material(&bytes)
                || artifact.schema_version != ARTIFACT_RECEIPT_SCHEMA_VERSION
                || artifact.artifact_ref != expected_ref
                || artifact.content_digest != content_digest
                || artifact.byte_length != bytes.len() as u64
                || artifact.source_generation != request.resource_generation
                || artifact.source_path_digest != digest(request.effective_path().as_bytes())
                || artifact.retention_until
                    != *required(&request.retention_until, "artifact_retention_required")?
                || artifact.retention_until <= response.receipt.finished_at
                || artifact.content_type.is_empty()
                || artifact.content_type.len() > 256
                || artifact.content_type.contains('\r')
                || artifact.content_type.contains('\n')
                || artifact.evidence_refs.is_empty()
                || response.receipt.bytes_read != artifact.byte_length
            {
                return Err(GuestIoError::conflict("artifact_receipt_conflict"));
            }
            for evidence_ref in &artifact.evidence_refs {
                validate_ref(evidence_ref)?;
            }
        }
    }
    Ok(())
}

fn validate_receipt(
    request: &ManagedSandboxGuestIoRequest,
    receipt: &GuestIoReceipt,
) -> Result<(), GuestIoError> {
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION
        || receipt.operation_ref != request.operation_ref
        || receipt.sandbox_ref != request.sandbox_ref
        || receipt.resource_generation != request.resource_generation
        || receipt.capability_ref != request.capability_ref
        || receipt.action != request.action
        || receipt.outcome != "succeeded"
        || receipt.path_digest != digest(request.effective_path().as_bytes())
        || receipt.cpu_millis > request.limits.max_cpu_millis
        || receipt.network_bytes > request.limits.max_network_bytes
        || !receipt.process_terminated
        || receipt.descendants_remaining != 0
        || !receipt.scratch_cleaned
        || !receipt.ingress_closed
        || !receipt.egress_denied
        || receipt.path_policy != "resolved_beneath_workspace_root"
        || receipt.symlink_traversal
        || receipt.secret_scan != "clean"
        || receipt.evidence_refs.is_empty()
        || receipt.started_at < request.requested_at
        || receipt.finished_at < receipt.started_at
    {
        return Err(GuestIoError::conflict("guest_io_receipt_invalid"));
    }
    validate_ref(&receipt.receipt_ref)?;
    validate_timestamp(&receipt.started_at)?;
    validate_timestamp(&receipt.finished_at)?;
    for evidence_ref in &receipt.evidence_refs {
        validate_ref(evidence_ref)?;
    }
    if let Some(process_ref) = &receipt.process_ref {
        validate_ref(process_ref)?;
    }
    Ok(())
}

fn required<'a>(
    value: &'a Option<String>,
    reason: &'static str,
) -> Result<&'a String, GuestIoError> {
    value.as_ref().ok_or_else(|| GuestIoError::invalid(reason))
}

fn validate_ref(value: &str) -> Result<(), GuestIoError> {
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
        return Err(GuestIoError::invalid("public_ref_invalid"));
    }
    Ok(())
}

fn validate_timestamp(value: &str) -> Result<(), GuestIoError> {
    if value.len() < 20 || value.len() > 32 || !value.ends_with('Z') {
        return Err(GuestIoError::invalid("timestamp_invalid"));
    }
    Ok(())
}

fn validate_path(value: &str) -> Result<(), GuestIoError> {
    if value.len() > 1_024
        || value.contains('\0')
        || value.contains('\\')
        || value.starts_with('/')
        || value.ends_with('/')
        || (value != "workspace" && !value.starts_with("workspace/"))
        || value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(GuestIoError::invalid("path_not_beneath_workspace_root"));
    }
    Ok(())
}

fn validate_encoding(value: &str) -> Result<(), GuestIoError> {
    if matches!(value, "utf8" | "base64") {
        Ok(())
    } else {
        Err(GuestIoError::invalid("encoding_not_admitted"))
    }
}

fn decode_content(encoding: &str, value: &str) -> Result<Vec<u8>, GuestIoError> {
    if encoding == "utf8" {
        Ok(value.as_bytes().to_vec())
    } else {
        BASE64
            .decode(value)
            .map_err(|_| GuestIoError::invalid("content_base64_invalid"))
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn forbidden_material(bytes: &[u8]) -> bool {
    let value = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    [
        "-----begin private key-----",
        "-----begin rsa private key-----",
        "authorization: bearer ",
        "refresh_token",
        "client_secret",
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

    fn limits() -> GuestIoLimits {
        GuestIoLimits {
            workspace_root_ref: "workspace.managed-sandbox".to_string(),
            max_file_bytes: 1_048_576,
            max_artifact_bytes: 10_000_000,
            max_output_bytes: 131_072,
            max_duration_millis: 60_000,
            max_cpu_millis: 60_000,
            max_processes: 32,
            max_network_bytes: 0,
            network_policy_ref: "network-policy.managed-sandbox.deny-all".to_string(),
        }
    }

    fn request(action: GuestIoAction) -> ManagedSandboxGuestIoRequest {
        let (
            path,
            encoding,
            content,
            content_digest,
            command,
            command_digest,
            cwd,
            timeout,
            retention,
        ) = match action {
            GuestIoAction::ReadFile => (
                Some("workspace/a.txt".to_string()),
                Some("utf8".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            GuestIoAction::WriteFile => (
                Some("workspace/a.txt".to_string()),
                Some("utf8".to_string()),
                Some("hello".to_string()),
                Some(digest(b"hello")),
                None,
                None,
                None,
                None,
                None,
            ),
            GuestIoAction::ExecuteCommand => (
                None,
                None,
                None,
                None,
                Some("pwd".to_string()),
                Some(digest(b"pwd")),
                Some("workspace".to_string()),
                Some(1_000),
                None,
            ),
            GuestIoAction::ReadArtifact => (
                Some("workspace/a.bin".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("2026-07-20T20:00:00.000Z".to_string()),
            ),
        };
        ManagedSandboxGuestIoRequest {
            schema_version: SCHEMA_VERSION.to_string(),
            action,
            operation_ref: "operation.sbx05.test".to_string(),
            idempotency_ref: "idempotency.sbx05.test".to_string(),
            actor_ref: "agent.sbx05.test".to_string(),
            owner_ref: "owner.sbx05.test".to_string(),
            tenant_ref: "tenant.sbx05.test".to_string(),
            program_ref: "program.managed_agent_sandboxes".to_string(),
            work_unit_ref: "work.sbx05.test".to_string(),
            sandbox_ref: "sandbox.sbx05.test".to_string(),
            resource_generation: 2,
            capability_ref: "capability.sbx05.test".to_string(),
            capability_state: "active".to_string(),
            capability_expires_at: "2026-07-19T21:00:00.000Z".to_string(),
            requested_at: "2026-07-19T20:00:00.000Z".to_string(),
            limits: limits(),
            path,
            encoding,
            content,
            content_digest,
            command,
            command_digest,
            cwd,
            timeout_millis: timeout,
            retention_until: retention,
        }
    }

    fn receipt(request: &ManagedSandboxGuestIoRequest, read: u64, written: u64) -> GuestIoReceipt {
        GuestIoReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION.to_string(),
            receipt_ref: "receipt.sbx05.test".to_string(),
            operation_ref: request.operation_ref.clone(),
            sandbox_ref: request.sandbox_ref.clone(),
            resource_generation: request.resource_generation,
            capability_ref: request.capability_ref.clone(),
            action: request.action,
            outcome: "succeeded".to_string(),
            path_digest: digest(request.effective_path().as_bytes()),
            started_at: request.requested_at.clone(),
            finished_at: "2026-07-19T20:00:01.000Z".to_string(),
            bytes_read: read,
            bytes_written: written,
            cpu_millis: 1,
            network_bytes: 0,
            process_ref: (request.action == GuestIoAction::ExecuteCommand)
                .then(|| "process.sbx05.test".to_string()),
            process_terminated: true,
            descendants_remaining: 0,
            scratch_cleaned: true,
            ingress_closed: true,
            egress_denied: true,
            path_policy: "resolved_beneath_workspace_root".to_string(),
            symlink_traversal: false,
            secret_scan: "clean".to_string(),
            evidence_refs: vec!["evidence.sbx05.test".to_string()],
        }
    }

    fn base_response(
        request: &ManagedSandboxGuestIoRequest,
        receipt: GuestIoReceipt,
    ) -> ManagedSandboxGuestIoResponse {
        ManagedSandboxGuestIoResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            action: request.action,
            operation_ref: request.operation_ref.clone(),
            sandbox_ref: request.sandbox_ref.clone(),
            resource_generation: request.resource_generation,
            receipt,
            encoding: None,
            content: None,
            content_digest: None,
            byte_length: None,
            binary: None,
            success: None,
            exit_code: None,
            signal: None,
            stdout: None,
            stderr: None,
            stdout_truncated: None,
            stderr_truncated: None,
            timed_out: None,
            cancelled: None,
            duration_millis: None,
            max_processes_observed: None,
            content_base64: None,
            artifact: None,
        }
    }

    #[test]
    fn validates_bounded_file_command_and_content_addressed_artifact_receipts() {
        for action in [
            GuestIoAction::ReadFile,
            GuestIoAction::WriteFile,
            GuestIoAction::ExecuteCommand,
            GuestIoAction::ReadArtifact,
        ] {
            let request = request(action);
            request.validate().unwrap();
            let mut response = match action {
                GuestIoAction::ReadFile => {
                    let mut value = base_response(&request, receipt(&request, 5, 0));
                    value.encoding = Some("utf8".to_string());
                    value.content = Some("hello".to_string());
                    value.content_digest = Some(digest(b"hello"));
                    value.byte_length = Some(5);
                    value.binary = Some(false);
                    value
                }
                GuestIoAction::WriteFile => {
                    let mut value = base_response(&request, receipt(&request, 0, 5));
                    value.content_digest = Some(digest(b"hello"));
                    value.byte_length = Some(5);
                    value
                }
                GuestIoAction::ExecuteCommand => {
                    let mut value = base_response(&request, receipt(&request, 0, 10));
                    value.success = Some(true);
                    value.exit_code = Some(0);
                    value.stdout = Some("workspace\n".to_string());
                    value.stderr = Some(String::new());
                    value.stdout_truncated = Some(false);
                    value.stderr_truncated = Some(false);
                    value.timed_out = Some(false);
                    value.cancelled = Some(false);
                    value.duration_millis = Some(10);
                    value.max_processes_observed = Some(1);
                    value
                }
                GuestIoAction::ReadArtifact => {
                    let bytes = b"artifact";
                    let content_digest = digest(bytes);
                    let mut value =
                        base_response(&request, receipt(&request, bytes.len() as u64, 0));
                    value.content_base64 = Some(BASE64.encode(bytes));
                    value.artifact = Some(ArtifactReceipt {
                        schema_version: ARTIFACT_RECEIPT_SCHEMA_VERSION.to_string(),
                        artifact_ref: format!("artifact.sha256.{}", &content_digest[7..]),
                        content_digest,
                        byte_length: bytes.len() as u64,
                        source_generation: request.resource_generation,
                        source_path_digest: digest(request.effective_path().as_bytes()),
                        retention_until: request.retention_until.clone().unwrap(),
                        content_type: "application/octet-stream".to_string(),
                        evidence_refs: vec!["evidence.sbx05.test".to_string()],
                    });
                    value
                }
            };
            if action == GuestIoAction::ExecuteCommand {
                response.receipt.bytes_written = 10;
            }
            validate_response(&request, &response).unwrap();
        }
    }

    #[test]
    fn refuses_path_secret_quota_symlink_egress_and_process_faults() {
        let mut escaped = request(GuestIoAction::ReadFile);
        escaped.path = Some("workspace/link/../../etc/passwd".to_string());
        assert!(escaped.validate().is_err());

        let mut secret = request(GuestIoAction::WriteFile);
        secret.content = Some("-----BEGIN PRIVATE KEY-----".to_string());
        secret.content_digest = Some(digest(secret.content.as_ref().unwrap().as_bytes()));
        assert!(secret.validate().is_err());

        let mut expired = request(GuestIoAction::ReadFile);
        expired.capability_expires_at = expired.requested_at.clone();
        let error = expired.validate().unwrap_err();
        assert_eq!(error.status, 403);
        assert_eq!(error.reason_ref, "guest_io_capability_expired_or_revoked");

        let command = request(GuestIoAction::ExecuteCommand);
        let mut response = base_response(&command, receipt(&command, 0, 0));
        response.success = Some(true);
        response.exit_code = Some(0);
        response.stdout = Some("ok".to_string());
        response.stderr = Some(String::new());
        response.stdout_truncated = Some(false);
        response.stderr_truncated = Some(false);
        response.timed_out = Some(true);
        response.cancelled = Some(false);
        response.duration_millis = Some(10);
        response.max_processes_observed = Some(1);
        assert!(validate_response(&command, &response).is_err());
        response.cancelled = Some(true);
        response.receipt.symlink_traversal = true;
        assert!(validate_response(&command, &response).is_err());
        response.receipt.symlink_traversal = false;
        response.receipt.network_bytes = 1;
        assert!(validate_response(&command, &response).is_err());
        response.receipt.network_bytes = 0;
        response.receipt.descendants_remaining = 1;
        assert!(validate_response(&command, &response).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn kills_a_guest_io_driver_that_exceeds_the_declared_command_deadline() {
        let root = env::temp_dir().join(format!(
            "oa-sbx05-guest-io-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let driver = root.join("driver.sh");
        fs::write(&driver, "#!/bin/sh\ncat >/dev/null\nsleep 5\n").unwrap();
        fs::set_permissions(&driver, fs::Permissions::from_mode(0o700)).unwrap();
        let mut value = request(GuestIoAction::ExecuteCommand);
        value.timeout_millis = Some(1);
        value.limits.max_duration_millis = 1;
        value.limits.max_cpu_millis = 1;
        let started = Instant::now();
        let error = execute_with_driver(&driver, value).unwrap_err();
        assert_eq!(error.reason_ref, "guest_io_driver_timed_out");
        assert!(started.elapsed() < Duration::from_secs(4));
        fs::remove_dir_all(root).unwrap();
    }
}
