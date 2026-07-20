//! Private, fail-closed adapter from managed-sandbox turns to a guest runtime.
//!
//! The configured executable is an app-owned helper that uses the ordinary
//! Codex or Claude SDK in the isolated guest. Each invocation is a short
//! control operation (`dispatch`, `sync`, or `interrupt`); a turn itself has no
//! silence timeout or arbitrary wall deadline. The helper owns provider-private
//! process/session handles and returns only the native public-safe event plane.

use std::env;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const SCHEMA_VERSION: &str = "openagents.managed_sandbox_turn_runtime.v1";
const MAX_PROMPT_BYTES: usize = 100_000;
const MAX_EVENTS: usize = 1_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnAction {
    Dispatch,
    Sync,
    Interrupt,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIdentity {
    provider: String,
    model_ref: String,
    harness_ref: String,
    #[serde(default)]
    reasoning_effort: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSandboxTurnRuntimeRequest {
    pub schema_version: String,
    pub action: TurnAction,
    pub actor_ref: String,
    pub owner_ref: String,
    pub tenant_ref: String,
    pub program_ref: String,
    pub work_unit_ref: String,
    pub sandbox_ref: String,
    pub turn_ref: String,
    pub expected_resource_generation: u64,
    pub prompt_digest: String,
    pub runtime: RuntimeIdentity,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub after_turn_sequence: Option<u64>,
    #[serde(default)]
    pub reason_ref: Option<String>,
    #[serde(default)]
    pub idempotency_ref: Option<String>,
    #[serde(default)]
    pub provider_capability_token: Option<String>,
    #[serde(default)]
    pub provider_model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeUsage {
    input_tokens: u64,
    output_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cached_input_tokens: Option<u64>,
    provider_usage_ref: String,
    exact: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "_tag")]
enum RuntimeEvent {
    RuntimeStarted {
        #[serde(rename = "turnRef")]
        turn_ref: String,
        #[serde(rename = "resourceGeneration")]
        resource_generation: u64,
        #[serde(rename = "turnEventSequence")]
        turn_event_sequence: u64,
        #[serde(rename = "observedAt")]
        observed_at: String,
    },
    RuntimeTextDelta {
        #[serde(rename = "turnRef")]
        turn_ref: String,
        #[serde(rename = "resourceGeneration")]
        resource_generation: u64,
        #[serde(rename = "turnEventSequence")]
        turn_event_sequence: u64,
        #[serde(rename = "observedAt")]
        observed_at: String,
        content: String,
    },
    RuntimeToolStarted {
        #[serde(rename = "turnRef")]
        turn_ref: String,
        #[serde(rename = "resourceGeneration")]
        resource_generation: u64,
        #[serde(rename = "turnEventSequence")]
        turn_event_sequence: u64,
        #[serde(rename = "observedAt")]
        observed_at: String,
        #[serde(rename = "toolCallRef")]
        tool_call_ref: String,
        #[serde(rename = "toolName")]
        tool_name: String,
    },
    RuntimeToolCompleted {
        #[serde(rename = "turnRef")]
        turn_ref: String,
        #[serde(rename = "resourceGeneration")]
        resource_generation: u64,
        #[serde(rename = "turnEventSequence")]
        turn_event_sequence: u64,
        #[serde(rename = "observedAt")]
        observed_at: String,
        #[serde(rename = "toolCallRef")]
        tool_call_ref: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        outcome: String,
        #[serde(rename = "evidenceRefs")]
        evidence_refs: Vec<String>,
    },
    RuntimeUsageRecorded {
        #[serde(rename = "turnRef")]
        turn_ref: String,
        #[serde(rename = "resourceGeneration")]
        resource_generation: u64,
        #[serde(rename = "turnEventSequence")]
        turn_event_sequence: u64,
        #[serde(rename = "observedAt")]
        observed_at: String,
        usage: RuntimeUsage,
    },
    RuntimeInterruptRequested {
        #[serde(rename = "turnRef")]
        turn_ref: String,
        #[serde(rename = "resourceGeneration")]
        resource_generation: u64,
        #[serde(rename = "turnEventSequence")]
        turn_event_sequence: u64,
        #[serde(rename = "observedAt")]
        observed_at: String,
        #[serde(rename = "reasonRef")]
        reason_ref: String,
    },
    RuntimeSettled {
        #[serde(rename = "turnRef")]
        turn_ref: String,
        #[serde(rename = "resourceGeneration")]
        resource_generation: u64,
        #[serde(rename = "turnEventSequence")]
        turn_event_sequence: u64,
        #[serde(rename = "observedAt")]
        observed_at: String,
        #[serde(rename = "finishReason")]
        finish_reason: String,
        #[serde(default)]
        usage: Option<RuntimeUsage>,
    },
    RuntimeFailed {
        #[serde(rename = "turnRef")]
        turn_ref: String,
        #[serde(rename = "resourceGeneration")]
        resource_generation: u64,
        #[serde(rename = "turnEventSequence")]
        turn_event_sequence: u64,
        #[serde(rename = "observedAt")]
        observed_at: String,
        #[serde(rename = "errorRef")]
        error_ref: String,
        retryable: bool,
    },
    RuntimeInterrupted {
        #[serde(rename = "turnRef")]
        turn_ref: String,
        #[serde(rename = "resourceGeneration")]
        resource_generation: u64,
        #[serde(rename = "turnEventSequence")]
        turn_event_sequence: u64,
        #[serde(rename = "observedAt")]
        observed_at: String,
        #[serde(rename = "reasonRef")]
        reason_ref: String,
    },
}

impl RuntimeEvent {
    fn coordinates(&self) -> (&str, u64, u64, &str) {
        match self {
            Self::RuntimeStarted {
                turn_ref,
                resource_generation,
                turn_event_sequence,
                observed_at,
            }
            | Self::RuntimeTextDelta {
                turn_ref,
                resource_generation,
                turn_event_sequence,
                observed_at,
                ..
            }
            | Self::RuntimeToolStarted {
                turn_ref,
                resource_generation,
                turn_event_sequence,
                observed_at,
                ..
            }
            | Self::RuntimeToolCompleted {
                turn_ref,
                resource_generation,
                turn_event_sequence,
                observed_at,
                ..
            }
            | Self::RuntimeUsageRecorded {
                turn_ref,
                resource_generation,
                turn_event_sequence,
                observed_at,
                ..
            }
            | Self::RuntimeInterruptRequested {
                turn_ref,
                resource_generation,
                turn_event_sequence,
                observed_at,
                ..
            }
            | Self::RuntimeSettled {
                turn_ref,
                resource_generation,
                turn_event_sequence,
                observed_at,
                ..
            }
            | Self::RuntimeFailed {
                turn_ref,
                resource_generation,
                turn_event_sequence,
                observed_at,
                ..
            }
            | Self::RuntimeInterrupted {
                turn_ref,
                resource_generation,
                turn_event_sequence,
                observed_at,
                ..
            } => (
                turn_ref,
                *resource_generation,
                *turn_event_sequence,
                observed_at,
            ),
        }
    }

    fn is_started(&self) -> bool {
        matches!(self, Self::RuntimeStarted { .. })
    }

    fn is_interrupt_requested(&self) -> bool {
        matches!(self, Self::RuntimeInterruptRequested { .. })
    }

    fn validate_payload(&self) -> Result<(), TurnRuntimeError> {
        match self {
            Self::RuntimeTextDelta { content, .. } if content.len() > 65_536 => {
                Err(TurnRuntimeError::invalid("runtime_text_delta_too_large"))
            }
            Self::RuntimeToolStarted {
                tool_call_ref,
                tool_name,
                ..
            } => {
                validate_ref("toolCallRef", tool_call_ref)?;
                validate_ref("toolName", tool_name)
            }
            Self::RuntimeToolCompleted {
                tool_call_ref,
                tool_name,
                outcome,
                evidence_refs,
                ..
            } => {
                validate_ref("toolCallRef", tool_call_ref)?;
                validate_ref("toolName", tool_name)?;
                if !matches!(outcome.as_str(), "succeeded" | "failed" | "refused") {
                    return Err(TurnRuntimeError::invalid("runtime_tool_outcome_invalid"));
                }
                for evidence_ref in evidence_refs {
                    validate_ref("evidenceRef", evidence_ref)?;
                }
                Ok(())
            }
            Self::RuntimeUsageRecorded { usage, .. }
            | Self::RuntimeSettled {
                usage: Some(usage), ..
            } => validate_ref("providerUsageRef", &usage.provider_usage_ref),
            Self::RuntimeInterruptRequested { reason_ref, .. }
            | Self::RuntimeInterrupted { reason_ref, .. } => validate_ref("reasonRef", reason_ref),
            Self::RuntimeSettled { finish_reason, .. } => {
                if matches!(
                    finish_reason.as_str(),
                    "structural_completion" | "lease_guardrail" | "budget_guardrail"
                ) {
                    Ok(())
                } else {
                    Err(TurnRuntimeError::invalid("runtime_finish_reason_invalid"))
                }
            }
            Self::RuntimeFailed { error_ref, .. } => validate_ref("errorRef", error_ref),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSandboxTurnRuntimeResponse {
    pub schema_version: String,
    pub turn_ref: String,
    pub resource_generation: u64,
    events: Vec<RuntimeEvent>,
}

#[derive(Debug)]
pub struct TurnRuntimeError {
    status: u16,
    code: &'static str,
    reason_ref: &'static str,
}

impl TurnRuntimeError {
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

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn response(&self) -> Value {
        json!({
            "schemaVersion": "openagents.managed_sandbox_turn_runtime_error.v1",
            "code": self.code,
            "reasonRef": self.reason_ref,
            "retryable": self.status >= 500,
        })
    }
}

impl ManagedSandboxTurnRuntimeRequest {
    fn validate(&self) -> Result<(), TurnRuntimeError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(TurnRuntimeError::invalid("schema_version_not_admitted"));
        }
        for (field, value) in [
            ("actorRef", self.actor_ref.as_str()),
            ("ownerRef", self.owner_ref.as_str()),
            ("tenantRef", self.tenant_ref.as_str()),
            ("programRef", self.program_ref.as_str()),
            ("workUnitRef", self.work_unit_ref.as_str()),
            ("sandboxRef", self.sandbox_ref.as_str()),
            ("turnRef", self.turn_ref.as_str()),
            ("modelRef", self.runtime.model_ref.as_str()),
            ("harnessRef", self.runtime.harness_ref.as_str()),
        ] {
            validate_ref(field, value)?;
        }
        if !matches!(self.runtime.provider.as_str(), "codex" | "claude") {
            return Err(TurnRuntimeError::invalid("runtime_provider_not_admitted"));
        }
        if !valid_sha256(&self.prompt_digest) {
            return Err(TurnRuntimeError::invalid("prompt_digest_invalid"));
        }
        match self.action {
            TurnAction::Dispatch => {
                let prompt = self
                    .prompt
                    .as_deref()
                    .ok_or_else(|| TurnRuntimeError::invalid("dispatch_prompt_required"))?;
                if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT_BYTES {
                    return Err(TurnRuntimeError::invalid("dispatch_prompt_out_of_bounds"));
                }
                if self.after_turn_sequence.unwrap_or(0) != 0 {
                    return Err(TurnRuntimeError::invalid(
                        "dispatch_sequence_must_start_at_zero",
                    ));
                }
                let capability = self.provider_capability_token.as_deref().ok_or_else(|| {
                    TurnRuntimeError::invalid("dispatch_provider_capability_required")
                })?;
                if capability.len() < 32
                    || capability.len() > 16_384
                    || capability.chars().any(char::is_whitespace)
                {
                    return Err(TurnRuntimeError::invalid(
                        "dispatch_provider_capability_invalid",
                    ));
                }
                let provider_model = self
                    .provider_model
                    .as_deref()
                    .ok_or_else(|| TurnRuntimeError::invalid("dispatch_provider_model_required"))?;
                validate_provider_model(provider_model)?;
            }
            TurnAction::Sync
                if self.prompt.is_some()
                    || self.reason_ref.is_some()
                    || self.provider_capability_token.is_some()
                    || self.provider_model.is_some() =>
            {
                return Err(TurnRuntimeError::invalid("sync_payload_invalid"));
            }
            TurnAction::Interrupt => {
                validate_ref(
                    "reasonRef",
                    self.reason_ref
                        .as_deref()
                        .ok_or_else(|| TurnRuntimeError::invalid("interrupt_reason_required"))?,
                )?;
                if self.provider_capability_token.is_some() || self.provider_model.is_some() {
                    return Err(TurnRuntimeError::invalid("interrupt_payload_invalid"));
                }
                validate_ref(
                    "idempotencyRef",
                    self.idempotency_ref.as_deref().ok_or_else(|| {
                        TurnRuntimeError::invalid("interrupt_idempotency_required")
                    })?,
                )?;
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn execute(
    request: ManagedSandboxTurnRuntimeRequest,
) -> Result<ManagedSandboxTurnRuntimeResponse, TurnRuntimeError> {
    request.validate()?;
    let driver = env::var("OA_MANAGED_SANDBOX_TURN_DRIVER").map_err(|_| {
        TurnRuntimeError::new(503, "runtime_unavailable", "turn_driver_not_configured")
    })?;
    if !Path::new(&driver).is_absolute() {
        return Err(TurnRuntimeError::new(
            503,
            "runtime_unavailable",
            "turn_driver_path_not_absolute",
        ));
    }
    execute_with_driver(Path::new(&driver), request)
}

fn execute_with_driver(
    driver: &Path,
    request: ManagedSandboxTurnRuntimeRequest,
) -> Result<ManagedSandboxTurnRuntimeResponse, TurnRuntimeError> {
    let request_bytes = serde_json::to_vec(&request)
        .map_err(|_| TurnRuntimeError::new(500, "runtime_failure", "turn_request_encode_failed"))?;
    let mut child = Command::new(driver)
        .arg("--managed-sandbox-turn")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            TurnRuntimeError::new(503, "runtime_unavailable", "turn_driver_spawn_failed")
        })?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| TurnRuntimeError::new(500, "runtime_failure", "turn_driver_stdin_missing"))?
        .write_all(&request_bytes)
        .map_err(|_| {
            TurnRuntimeError::new(503, "runtime_unavailable", "turn_driver_write_failed")
        })?;
    drop(child.stdin.take());
    let output = child.wait_with_output().map_err(|_| {
        TurnRuntimeError::new(503, "runtime_unavailable", "turn_driver_wait_failed")
    })?;
    if !output.status.success() {
        return Err(TurnRuntimeError::new(
            503,
            "runtime_unavailable",
            "turn_driver_refused",
        ));
    }
    let response: ManagedSandboxTurnRuntimeResponse = serde_json::from_slice(&output.stdout)
        .map_err(|_| {
            TurnRuntimeError::new(503, "runtime_unavailable", "turn_driver_response_invalid")
        })?;
    validate_response(&request, &response)?;
    Ok(response)
}

fn validate_response(
    request: &ManagedSandboxTurnRuntimeRequest,
    response: &ManagedSandboxTurnRuntimeResponse,
) -> Result<(), TurnRuntimeError> {
    if response.schema_version != SCHEMA_VERSION
        || response.turn_ref != request.turn_ref
        || response.resource_generation != request.expected_resource_generation
        || response.events.len() > MAX_EVENTS
    {
        return Err(TurnRuntimeError::new(
            409,
            "runtime_conflict",
            "turn_response_scope_conflict",
        ));
    }
    let after = request.after_turn_sequence.unwrap_or(0);
    if matches!(request.action, TurnAction::Dispatch | TurnAction::Interrupt)
        && response.events.is_empty()
    {
        return Err(TurnRuntimeError::new(
            503,
            "runtime_unavailable",
            "turn_response_missing_admission_event",
        ));
    }
    if request.action == TurnAction::Dispatch
        && !response
            .events
            .first()
            .is_some_and(RuntimeEvent::is_started)
    {
        return Err(TurnRuntimeError::new(
            409,
            "runtime_conflict",
            "dispatch_did_not_start_turn",
        ));
    }
    if request.action == TurnAction::Interrupt
        && !response
            .events
            .first()
            .is_some_and(RuntimeEvent::is_interrupt_requested)
    {
        return Err(TurnRuntimeError::new(
            409,
            "runtime_conflict",
            "interrupt_not_visible",
        ));
    }
    for (offset, event) in response.events.iter().enumerate() {
        let (turn_ref, generation, sequence, observed_at) = event.coordinates();
        if turn_ref != request.turn_ref
            || generation != request.expected_resource_generation
            || sequence != after + offset as u64 + 1
            || !observed_at.ends_with('Z')
        {
            return Err(TurnRuntimeError::new(
                409,
                "runtime_conflict",
                "turn_event_coordinate_conflict",
            ));
        }
        event.validate_payload()?;
    }
    Ok(())
}

fn validate_ref(_field: &str, value: &str) -> Result<(), TurnRuntimeError> {
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
        return Err(TurnRuntimeError::invalid("public_ref_invalid"));
    }
    Ok(())
}

fn validate_provider_model(value: &str) -> Result<(), TurnRuntimeError> {
    if value.len() < 3
        || value.len() > 256
        || !value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-' | '@')
        })
    {
        return Err(TurnRuntimeError::invalid("provider_model_invalid"));
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn request(action: TurnAction, provider: &str, after: u64) -> ManagedSandboxTurnRuntimeRequest {
        ManagedSandboxTurnRuntimeRequest {
            schema_version: SCHEMA_VERSION.to_string(),
            action,
            actor_ref: "agent.sbx04.test".to_string(),
            owner_ref: "owner.sbx04.test".to_string(),
            tenant_ref: "tenant.sbx04.test".to_string(),
            program_ref: "program.managed_agent_sandboxes".to_string(),
            work_unit_ref: "work.sbx04.test".to_string(),
            sandbox_ref: "sandbox.sbx04.test".to_string(),
            turn_ref: "turn.sbx04.test".to_string(),
            expected_resource_generation: 3,
            prompt_digest: format!("sha256:{}", "a".repeat(64)),
            runtime: RuntimeIdentity {
                provider: provider.to_string(),
                model_ref: format!("model.{provider}.test"),
                harness_ref: format!("harness.{provider}.sdk.v1"),
                reasoning_effort: None,
            },
            prompt: (action == TurnAction::Dispatch).then(|| "Inspect the project.".to_string()),
            after_turn_sequence: Some(after),
            reason_ref: (action == TurnAction::Interrupt).then(|| "reason.sbx04.stop".to_string()),
            idempotency_ref: (action == TurnAction::Interrupt)
                .then(|| "idempotency.sbx04.stop".to_string()),
            provider_capability_token: (action == TurnAction::Dispatch)
                .then(|| "private.signed.capability.token.with.sufficient.length".to_string()),
            provider_model: (action == TurnAction::Dispatch)
                .then(|| format!("{provider}-provider-model")),
        }
    }

    #[cfg(unix)]
    fn driver(response: &Value) -> std::path::PathBuf {
        let root = env::temp_dir().join(format!(
            "oa-sbx04-turn-driver-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("driver root");
        let path = root.join("driver.sh");
        let encoded = serde_json::to_string(response)
            .expect("response")
            .replace('\'', "'\\''");
        fs::write(
            &path,
            format!("#!/bin/sh\ncat >/dev/null\nprintf '%s' '{encoded}'\n"),
        )
        .expect("driver");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).expect("permissions");
        path
    }

    #[test]
    #[cfg(unix)]
    fn admits_codex_and_claude_sdk_driver_events_without_waiting_for_silence() {
        for provider in ["codex", "claude"] {
            let response = json!({
                "schemaVersion": SCHEMA_VERSION,
                "turnRef": "turn.sbx04.test",
                "resourceGeneration": 3,
                "events": [
                    {
                        "_tag": "RuntimeStarted",
                        "turnRef": "turn.sbx04.test",
                        "resourceGeneration": 3,
                        "turnEventSequence": 1,
                        "observedAt": "2026-07-19T19:30:00.000Z"
                    }
                ]
            });
            let result = execute_with_driver(
                &driver(&response),
                request(TurnAction::Dispatch, provider, 0),
            )
            .expect("admitted");
            assert_eq!(result.events.len(), 1);
        }
    }

    #[test]
    #[cfg(unix)]
    fn omits_absent_optional_usage_fields_from_public_turn_responses() {
        let response = json!({
            "schemaVersion": SCHEMA_VERSION,
            "turnRef": "turn.sbx04.test",
            "resourceGeneration": 3,
            "events": [
                {
                    "_tag": "RuntimeUsageRecorded",
                    "turnRef": "turn.sbx04.test",
                    "resourceGeneration": 3,
                    "turnEventSequence": 2,
                    "observedAt": "2026-07-19T19:30:01.000Z",
                    "usage": {
                        "inputTokens": 4,
                        "outputTokens": 2,
                        "providerUsageRef": "provider.usage.sha256.test",
                        "exact": true
                    }
                }
            ]
        });
        let result =
            execute_with_driver(&driver(&response), request(TurnAction::Sync, "claude", 1))
                .expect("admitted");
        let encoded = serde_json::to_value(result).expect("response serialization");
        assert_eq!(
            encoded["events"][0]["usage"].get("cachedInputTokens"),
            None,
            "an absent optional field must be omitted rather than serialized as null"
        );
    }

    #[test]
    #[cfg(unix)]
    fn rejects_stale_or_invisible_interrupt_events() {
        let response = json!({
            "schemaVersion": SCHEMA_VERSION,
            "turnRef": "turn.sbx04.test",
            "resourceGeneration": 2,
            "events": []
        });
        let error = execute_with_driver(
            &driver(&response),
            request(TurnAction::Interrupt, "codex", 4),
        )
        .expect_err("conflict");
        assert_eq!(error.status(), 409);
    }
}
