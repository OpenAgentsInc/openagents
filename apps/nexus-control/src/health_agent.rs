use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::health::NexusHealthPredicate;
use crate::{HealthSnapshotCommand, NexusHealthSnapshot};

const DEFAULT_NEXUS_BASE_URL: &str = "https://nexus.openagents.com";
const DEFAULT_TIMEOUT_MS: u64 = 8_000;
const DEFAULT_PROJECT_ID: &str = "openagents";
const DEFAULT_ACTOR_ID: &str = "nexus-health-agent";

const ENV_NEXUS_BASE_URL: &str = "NEXUS_HEALTH_AGENT_NEXUS_BASE_URL";
const ENV_TIMEOUT_MS: &str = "NEXUS_HEALTH_AGENT_TIMEOUT_MS";
const ENV_FORGE_BASE_URL: &str = "NEXUS_HEALTH_AGENT_FORGE_BASE_URL";
const ENV_FORGE_BEARER_TOKEN: &str = "NEXUS_HEALTH_AGENT_FORGE_BEARER_TOKEN";
const ENV_FORGE_ACTOR_JWT: &str = "NEXUS_HEALTH_AGENT_FORGE_ACTOR_JWT";
const ENV_PROJECT_ID: &str = "NEXUS_HEALTH_AGENT_PROJECT_ID";
const ENV_ACTOR_ID: &str = "NEXUS_HEALTH_AGENT_ACTOR_ID";

const SECRET_MARKERS: &[&str] = &[
    "api_key",
    "authorization",
    "bearer",
    "credential",
    "env_file",
    "mnemonic",
    "password",
    "preimage",
    "private_key",
    "raw_env",
    "secret",
    "seed",
    "token",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusHealthAgentCommand {
    pub nexus_base_url: String,
    pub timeout_ms: u64,
    pub forge_base_url: Option<String>,
    pub forge_bearer_token: Option<String>,
    pub forge_actor_jwt: Option<String>,
    pub project_id: String,
    pub actor_id: String,
    pub fake_nexus: bool,
    pub fake_forge: bool,
    pub dry_run: bool,
    pub pretty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthAgentReport {
    pub schema_version: u32,
    pub generated_at_unix_ms: u64,
    pub mode: String,
    pub status: String,
    pub dry_run: bool,
    pub fake_nexus: bool,
    pub fake_forge: bool,
    pub project_id: String,
    pub actor_id: String,
    pub nexus_base_url: String,
    pub snapshot_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<NexusHealthSnapshot>,
    pub evidence_artifacts: Vec<HealthEvidenceArtifact>,
    pub forge_work_order_request: ForgeHealthWorkOrderRequest,
    pub forge_event_request: ForgeHealthEventRequest,
    pub forge_writes: Vec<ForgeWriteResult>,
    pub redaction: HealthAgentRedactionReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthEvidenceArtifact {
    pub artifact_kind: String,
    pub content_sha256: String,
    pub storage: String,
    pub summary: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeHealthWorkOrderRequest {
    pub project_id: String,
    pub idempotency_key: String,
    pub work_order_kind: String,
    pub title: String,
    pub description: String,
    pub priority: String,
    pub subsystem: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incident_class: Option<String>,
    pub severity: String,
    pub health_state: String,
    pub resource: String,
    pub requested_outputs: Value,
    pub verification_policy: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeHealthEventRequest {
    pub project_id: String,
    pub idempotency_key: String,
    pub event_type: String,
    pub subsystem: String,
    pub severity: String,
    pub health_state: String,
    pub resource: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incident_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_report_id: Option<String>,
    pub actor_kind: String,
    pub actor_id: String,
    pub summary: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeWriteResult {
    pub operation: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthAgentRedactionReport {
    pub payload_sensitive_keys_absent: bool,
    pub output_sensitive_strings_absent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthAgentActionPlan {
    pub action_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge_lease_id: Option<String>,
}

pub fn nexus_health_agent_usage() -> &'static str {
    "nexus-health-agent [--nexus-base-url <url>] [--forge-base-url <url>] [--timeout-ms <ms>] [--project-id <id>] [--actor-id <id>] [--fake-nexus] [--fake-forge] [--dry-run] [--pretty|--json]"
}

pub fn parse_nexus_health_agent_command(args: &[String]) -> Result<NexusHealthAgentCommand> {
    let mut command = NexusHealthAgentCommand::from_env()?;
    let mut index = 1;
    while let Some(arg) = args.get(index).map(String::as_str) {
        match arg {
            "--nexus-base-url" => {
                index += 1;
                command.nexus_base_url = required_arg(args, index, "--nexus-base-url")?.to_string();
            }
            "--forge-base-url" => {
                index += 1;
                command.forge_base_url =
                    Some(required_arg(args, index, "--forge-base-url")?.to_string());
            }
            "--timeout-ms" => {
                index += 1;
                command.timeout_ms =
                    parse_positive_u64(required_arg(args, index, "--timeout-ms")?, "--timeout-ms")?;
            }
            "--project-id" => {
                index += 1;
                command.project_id = required_arg(args, index, "--project-id")?.to_string();
            }
            "--actor-id" => {
                index += 1;
                command.actor_id = required_arg(args, index, "--actor-id")?.to_string();
            }
            "--fake-nexus" => command.fake_nexus = true,
            "--fake-forge" => command.fake_forge = true,
            "--dry-run" => command.dry_run = true,
            "--pretty" => command.pretty = true,
            "--json" => command.pretty = false,
            "--help" | "-h" => bail!("usage: {}", nexus_health_agent_usage()),
            other => bail!("unknown nexus-health-agent option `{other}`"),
        }
        index += 1;
    }
    command.validate()?;
    Ok(command)
}

pub async fn run_nexus_health_agent(
    command: &NexusHealthAgentCommand,
) -> Result<NexusHealthAgentReport> {
    command.validate()?;
    let generated_at_unix_ms = now_unix_ms();
    let snapshot_outcome = capture_snapshot(command).await;
    let evidence_artifact = health_snapshot_evidence(&snapshot_outcome);
    let health_context = HealthEventContext::from_snapshot_outcome(&snapshot_outcome);
    let monitor_key = format!(
        "nexus-health-monitor-{}-{}",
        command.project_id,
        stable_value_digest(&evidence_artifact.payload)
    );
    let mut work_order_request = ForgeHealthWorkOrderRequest {
        project_id: command.project_id.clone(),
        idempotency_key: monitor_key.clone(),
        work_order_kind: "nexus.health.monitor".to_string(),
        title: format!("Nexus health monitor: {}", health_context.health_state),
        description: health_context.summary.clone(),
        priority: priority_for_severity(health_context.severity.as_str()).to_string(),
        subsystem: health_context.subsystem.clone(),
        incident_class: health_context.incident_class.clone(),
        severity: health_context.severity.clone(),
        health_state: health_context.health_state.clone(),
        resource: health_context.resource.clone(),
        requested_outputs: json!([
            "health.event",
            "health.evidence_bundle",
            "health.verification_report"
        ]),
        verification_policy: json!({
            "required_checks": [
                "nexus.health.snapshot_present",
                "nexus.health.event_appended",
                "nexus.health.recovery_not_attempted_without_lease"
            ],
            "recovery_authority": "monitor_only"
        }),
    };
    redact_value_in_place(&mut work_order_request.requested_outputs);
    redact_value_in_place(&mut work_order_request.verification_policy);

    let mut event_request = ForgeHealthEventRequest {
        project_id: command.project_id.clone(),
        idempotency_key: format!("{monitor_key}-event"),
        event_type: health_context.event_type.clone(),
        subsystem: health_context.subsystem,
        severity: health_context.severity,
        health_state: health_context.health_state,
        resource: health_context.resource,
        incident_id: health_context.incident_id,
        work_order_id: None,
        evidence_bundle_id: None,
        verification_report_id: None,
        actor_kind: "health_worker".to_string(),
        actor_id: command.actor_id.clone(),
        summary: health_context.summary,
        payload: json!({
            "schema_version": 1,
            "monitor_only": true,
            "evidence_artifacts": [{
                "artifact_kind": evidence_artifact.artifact_kind,
                "content_sha256": evidence_artifact.content_sha256,
                "storage": evidence_artifact.storage,
                "summary": evidence_artifact.summary,
            }],
            "snapshot_status": snapshot_outcome.status(),
            "classification": snapshot_outcome.classification_value(),
            "verification_gates": snapshot_outcome.verification_gates_value(),
            "failed_predicates": snapshot_outcome.failed_predicates_value(),
        }),
    };
    redact_value_in_place(&mut event_request.payload);

    let mut forge_writes = Vec::new();
    if command.dry_run || command.fake_forge {
        forge_writes.push(ForgeWriteResult {
            operation: "forge.health.work_order.create".to_string(),
            status: if command.dry_run {
                "dry_run_planned".to_string()
            } else {
                "fake_created".to_string()
            },
            id: Some("fake-forge-work-nexus-health-monitor".to_string()),
            error: None,
            response: Some(json!({
                "work_order_id": "fake-forge-work-nexus-health-monitor",
                "work_order_state": "queued",
                "kind": work_order_request.work_order_kind,
                "idempotent_replay": false,
            })),
        });
        event_request.work_order_id = Some("fake-forge-work-nexus-health-monitor".to_string());
        forge_writes.push(ForgeWriteResult {
            operation: "forge.health.event.append".to_string(),
            status: if command.dry_run {
                "dry_run_planned".to_string()
            } else {
                "fake_appended".to_string()
            },
            id: Some("fake-forge-health-event-nexus-health-monitor".to_string()),
            error: None,
            response: Some(json!({
                "event": {
                    "id": "fake-forge-health-event-nexus-health-monitor",
                    "idempotency_key": event_request.idempotency_key,
                }
            })),
        });
    } else {
        forge_writes = write_forge_health_event(command, &work_order_request, &mut event_request)
            .await
            .unwrap_or_else(|error| {
                vec![ForgeWriteResult {
                    operation: "forge.health.write".to_string(),
                    status: "failed".to_string(),
                    id: None,
                    error: Some(redact_sensitive_text(error.to_string().as_str())),
                    response: None,
                }]
            });
    }

    let mut report = NexusHealthAgentReport {
        schema_version: 1,
        generated_at_unix_ms,
        mode: "monitor_only".to_string(),
        status: report_status(snapshot_outcome.status(), &forge_writes).to_string(),
        dry_run: command.dry_run,
        fake_nexus: command.fake_nexus,
        fake_forge: command.fake_forge,
        project_id: command.project_id.clone(),
        actor_id: command.actor_id.clone(),
        nexus_base_url: command.nexus_base_url.clone(),
        snapshot_status: snapshot_outcome.status().to_string(),
        snapshot_error: snapshot_outcome.error(),
        snapshot: snapshot_outcome.snapshot(),
        evidence_artifacts: vec![evidence_artifact],
        forge_work_order_request: work_order_request,
        forge_event_request: event_request,
        forge_writes,
        redaction: HealthAgentRedactionReport {
            payload_sensitive_keys_absent: true,
            output_sensitive_strings_absent: true,
        },
    };
    redact_report(&mut report);
    let report_value = serde_json::to_value(&report).context("serialize health agent report")?;
    report.redaction = HealthAgentRedactionReport {
        payload_sensitive_keys_absent: !value_contains_secret_key(&report_value),
        output_sensitive_strings_absent: !value_contains_secret_string(&report_value),
    };
    Ok(report)
}

pub fn validate_health_agent_action_plan(plan: &HealthAgentActionPlan) -> Result<()> {
    if plan.action_kind == "monitor" {
        return Ok(());
    }
    if plan
        .forge_lease_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        Ok(())
    } else {
        bail!(
            "health-agent mutating action `{}` requires a Forge controller lease",
            plan.action_kind
        )
    }
}

impl NexusHealthAgentCommand {
    fn from_env() -> Result<Self> {
        Ok(Self {
            nexus_base_url: env_string(ENV_NEXUS_BASE_URL)
                .unwrap_or_else(|| DEFAULT_NEXUS_BASE_URL.to_string()),
            timeout_ms: env_string(ENV_TIMEOUT_MS)
                .map(|value| parse_positive_u64(&value, ENV_TIMEOUT_MS))
                .transpose()?
                .unwrap_or(DEFAULT_TIMEOUT_MS),
            forge_base_url: env_string(ENV_FORGE_BASE_URL),
            forge_bearer_token: env_string(ENV_FORGE_BEARER_TOKEN),
            forge_actor_jwt: env_string(ENV_FORGE_ACTOR_JWT),
            project_id: env_string(ENV_PROJECT_ID)
                .unwrap_or_else(|| DEFAULT_PROJECT_ID.to_string()),
            actor_id: env_string(ENV_ACTOR_ID).unwrap_or_else(|| DEFAULT_ACTOR_ID.to_string()),
            fake_nexus: false,
            fake_forge: false,
            dry_run: false,
            pretty: false,
        })
    }

    fn validate(&self) -> Result<()> {
        normalize_base_url(self.nexus_base_url.as_str())
            .with_context(|| format!("invalid Nexus base URL `{}`", self.nexus_base_url))?;
        if string_contains_secret_marker(self.nexus_base_url.as_str()) {
            bail!("Nexus base URL must not contain secret-shaped values");
        }
        if self.timeout_ms == 0 {
            bail!("timeout must be greater than zero");
        }
        if self.project_id.trim().is_empty() {
            bail!("project id must not be empty");
        }
        if self.actor_id.trim().is_empty() {
            bail!("actor id must not be empty");
        }
        if !(self.dry_run || self.fake_forge) {
            let forge_base_url = self
                .forge_base_url
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "{ENV_FORGE_BASE_URL} is required outside dry-run/fake-forge mode"
                    )
                })?;
            normalize_base_url(forge_base_url)
                .with_context(|| format!("invalid Forge base URL `{forge_base_url}`"))?;
            if string_contains_secret_marker(forge_base_url) {
                bail!("Forge base URL must not contain secret-shaped values");
            }
            if self
                .forge_bearer_token
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_none()
            {
                bail!("{ENV_FORGE_BEARER_TOKEN} is required outside dry-run/fake-forge mode");
            }
            if self
                .forge_actor_jwt
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_none()
            {
                bail!("{ENV_FORGE_ACTOR_JWT} is required outside dry-run/fake-forge mode");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum SnapshotOutcome {
    Captured(NexusHealthSnapshot),
    Failed(String),
}

impl SnapshotOutcome {
    fn status(&self) -> &'static str {
        match self {
            Self::Captured(_) => "captured",
            Self::Failed(_) => "failed",
        }
    }

    fn error(&self) -> Option<String> {
        match self {
            Self::Captured(_) => None,
            Self::Failed(error) => Some(redact_sensitive_text(error)),
        }
    }

    fn snapshot(&self) -> Option<NexusHealthSnapshot> {
        match self {
            Self::Captured(snapshot) => Some(snapshot.clone()),
            Self::Failed(_) => None,
        }
    }

    fn classification_value(&self) -> Value {
        match self {
            Self::Captured(snapshot) => {
                serde_json::to_value(&snapshot.classification).unwrap_or_else(|_| json!({}))
            }
            Self::Failed(error) => json!({
                "state": "incident",
                "summary": redact_sensitive_text(error),
                "highest_severity": "critical",
            }),
        }
    }

    fn verification_gates_value(&self) -> Value {
        match self {
            Self::Captured(snapshot) => {
                serde_json::to_value(&snapshot.verification_gates).unwrap_or_else(|_| json!({}))
            }
            Self::Failed(_) => json!({}),
        }
    }

    fn failed_predicates_value(&self) -> Value {
        match self {
            Self::Captured(snapshot) => {
                serde_json::to_value(&snapshot.classification.failed_predicates)
                    .unwrap_or_else(|_| json!([]))
            }
            Self::Failed(error) => json!([{
                "predicate_id": "health_snapshot_captured",
                "severity": "critical",
                "status": "failed",
                "detail": redact_sensitive_text(error),
                "remediation_hint": "Fix health snapshot command configuration or network access before attempting recovery."
            }]),
        }
    }
}

#[derive(Debug, Clone)]
struct HealthEventContext {
    event_type: String,
    subsystem: String,
    severity: String,
    health_state: String,
    resource: String,
    incident_class: Option<String>,
    incident_id: Option<String>,
    summary: String,
}

impl HealthEventContext {
    fn from_snapshot_outcome(outcome: &SnapshotOutcome) -> Self {
        match outcome {
            SnapshotOutcome::Failed(error) => Self {
                event_type: "nexus.health.snapshot_failed".to_string(),
                subsystem: "nexus".to_string(),
                severity: "critical".to_string(),
                health_state: "incident".to_string(),
                resource: "nexus-relay".to_string(),
                incident_class: Some("public_endpoint_failure".to_string()),
                incident_id: Some("incident-nexus-health-snapshot".to_string()),
                summary: format!(
                    "Nexus health snapshot failed: {}",
                    redact_sensitive_text(error)
                ),
            },
            SnapshotOutcome::Captured(snapshot) => {
                let first_failed = snapshot.classification.failed_predicates.first();
                let severity = valid_severity(snapshot.classification.highest_severity.as_str());
                let health_state = valid_health_state(snapshot.classification.state.as_str());
                let (subsystem, resource, incident_class) = classify_failed_predicate(first_failed);
                let event_type =
                    if health_state == "healthy" || health_state == "verified_closed" {
                        "nexus.health.snapshot_healthy"
                    } else {
                        "nexus.health.issue_detected"
                    }
                    .to_string();
                let incident_id = (health_state != "healthy" && health_state != "verified_closed")
                    .then(|| incident_id_for(&resource, health_state.as_str()));
                Self {
                    event_type,
                    subsystem,
                    severity,
                    health_state,
                    resource,
                    incident_class,
                    incident_id,
                    summary: snapshot.classification.summary.clone(),
                }
            }
        }
    }
}

async fn capture_snapshot(command: &NexusHealthAgentCommand) -> SnapshotOutcome {
    let snapshot_command = HealthSnapshotCommand {
        base_url: command.nexus_base_url.clone(),
        timeout_ms: command.timeout_ms,
        fake: command.fake_nexus,
        pretty: false,
    };
    match crate::run_health_snapshot_command(&snapshot_command).await {
        Ok(output) => serde_json::from_str::<NexusHealthSnapshot>(&output)
            .map(SnapshotOutcome::Captured)
            .unwrap_or_else(|error| SnapshotOutcome::Failed(error.to_string())),
        Err(error) => SnapshotOutcome::Failed(error.to_string()),
    }
}

fn health_snapshot_evidence(outcome: &SnapshotOutcome) -> HealthEvidenceArtifact {
    let mut payload = match outcome {
        SnapshotOutcome::Captured(snapshot) => {
            serde_json::to_value(snapshot).unwrap_or_else(|_| {
                json!({
                    "snapshot_status": "serialization_failed"
                })
            })
        }
        SnapshotOutcome::Failed(error) => json!({
            "snapshot_status": "failed",
            "snapshot_error": redact_sensitive_text(error),
        }),
    };
    redact_value_in_place(&mut payload);
    HealthEvidenceArtifact {
        artifact_kind: "nexus.health.snapshot".to_string(),
        content_sha256: stable_value_digest(&payload),
        storage: "inline.redacted".to_string(),
        summary: match outcome {
            SnapshotOutcome::Captured(snapshot) => {
                format!("Nexus health snapshot: {}", snapshot.classification.state)
            }
            SnapshotOutcome::Failed(_) => "Nexus health snapshot failed".to_string(),
        },
        payload,
    }
}

async fn write_forge_health_event(
    command: &NexusHealthAgentCommand,
    work_order_request: &ForgeHealthWorkOrderRequest,
    event_request: &mut ForgeHealthEventRequest,
) -> Result<Vec<ForgeWriteResult>> {
    let forge_base_url = normalize_base_url(
        command
            .forge_base_url
            .as_deref()
            .context("Forge base URL missing")?,
    )?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(command.timeout_ms))
        .build()
        .context("build Forge health-agent HTTP client")?;
    let bearer_token = command
        .forge_bearer_token
        .as_deref()
        .context("Forge bearer token missing")?;
    let actor_jwt = command
        .forge_actor_jwt
        .as_deref()
        .context("Forge actor JWT missing")?;
    let mut writes = Vec::new();

    let work_order_url = forge_base_url
        .join("v1/health/work-orders")
        .context("build Forge health work-order URL")?;
    let work_order_response = client
        .post(work_order_url)
        .bearer_auth(bearer_token)
        .header("x-forge-actor", actor_jwt)
        .header(
            "x-request-id",
            format!("{}-work-order", work_order_request.idempotency_key),
        )
        .json(work_order_request)
        .send()
        .await
        .context("send Forge health work-order request")?;
    let work_order_status = work_order_response.status();
    let mut work_order_body = response_json(work_order_response).await;
    redact_value_in_place(&mut work_order_body);
    if work_order_status.is_success() {
        let work_order_id = work_order_body
            .get("work_order_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        event_request.work_order_id = work_order_id.clone();
        writes.push(ForgeWriteResult {
            operation: "forge.health.work_order.create".to_string(),
            status: "created".to_string(),
            id: work_order_id,
            error: None,
            response: Some(work_order_body),
        });
    } else {
        writes.push(ForgeWriteResult {
            operation: "forge.health.work_order.create".to_string(),
            status: "failed".to_string(),
            id: None,
            error: Some(format!("http_status_{}", work_order_status.as_u16())),
            response: Some(work_order_body),
        });
        return Ok(writes);
    }

    let event_url = forge_base_url
        .join("v1/health/events")
        .context("build Forge health event URL")?;
    let event_response = client
        .post(event_url)
        .bearer_auth(bearer_token)
        .header("x-forge-actor", actor_jwt)
        .header(
            "x-request-id",
            format!("{}-append", event_request.idempotency_key),
        )
        .json(event_request)
        .send()
        .await
        .context("send Forge health event request")?;
    let event_status = event_response.status();
    let mut event_body = response_json(event_response).await;
    redact_value_in_place(&mut event_body);
    let event_id = event_body
        .get("event")
        .and_then(|event| event.get("id").or_else(|| event.get("health_event_id")))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    writes.push(ForgeWriteResult {
        operation: "forge.health.event.append".to_string(),
        status: if event_status.is_success() {
            "appended".to_string()
        } else {
            "failed".to_string()
        },
        id: event_id,
        error: (!event_status.is_success())
            .then(|| format!("http_status_{}", event_status.as_u16())),
        response: Some(event_body),
    });

    Ok(writes)
}

async fn response_json(response: reqwest::Response) -> Value {
    response.json::<Value>().await.unwrap_or_else(
        |error| json!({"response_parse_error": redact_sensitive_text(error.to_string().as_str())}),
    )
}

fn classify_failed_predicate(
    predicate: Option<&NexusHealthPredicate>,
) -> (String, String, Option<String>) {
    let Some(predicate) = predicate else {
        return ("nexus".to_string(), "nexus-relay".to_string(), None);
    };
    match predicate.predicate_id.as_str() {
        "cloudflare_edge_failure" => (
            "nexus.edge".to_string(),
            "nexus-cloudflared".to_string(),
            Some("cloudflare_edge_failure".to_string()),
        ),
        "public_endpoint_reachable" => (
            "nexus.edge".to_string(),
            "nexus-cloudflared".to_string(),
            Some("public_endpoint_failure".to_string()),
        ),
        "treasury_enabled"
        | "treasury_not_degraded"
        | "wallet_connected"
        | "treasury_balance_runway"
        | "payout_loop_healthy"
        | "payout_dispatch_fresh"
        | "payout_confirmation_fresh"
        | "accepted_work_payout_queue_healthy" => (
            "nexus.treasury".to_string(),
            "nexus-treasury-wallet".to_string(),
            Some(treasury_incident_class(predicate.predicate_id.as_str()).to_string()),
        ),
        "training_launch_healthy" | "training_dispatch_active" => (
            "nexus.training".to_string(),
            "nexus-training-dispatcher".to_string(),
            Some("training_dispatch_failure".to_string()),
        ),
        "website_stats_fresh" => (
            "openagents.web".to_string(),
            "openagents-com-stats-projection".to_string(),
            Some("website_stats_stale".to_string()),
        ),
        "gcp_vm_available" => (
            "nexus.relay".to_string(),
            "nexus-mainnet-1".to_string(),
            Some("vm_down".to_string()),
        ),
        "nexus_relay_restart_loop_absent" => (
            "nexus.relay".to_string(),
            "nexus-relay".to_string(),
            Some("relay_restart_loop".to_string()),
        ),
        "nexus_oom_absent" => (
            "nexus.relay".to_string(),
            "nexus-relay".to_string(),
            Some("oom_indicator".to_string()),
        ),
        _ => ("nexus".to_string(), "nexus-relay".to_string(), None),
    }
}

fn treasury_incident_class(predicate_id: &str) -> &'static str {
    match predicate_id {
        "wallet_connected" => "wallet_disconnected",
        "treasury_balance_runway" => "low_balance_runway",
        "payout_dispatch_fresh" => "payout_dispatch_stall",
        "payout_confirmation_fresh" => "payout_confirmation_stall",
        _ => "treasury_degraded",
    }
}

fn priority_for_severity(severity: &str) -> &'static str {
    match severity {
        "critical" => "critical",
        "error" => "high",
        "warning" => "medium",
        _ => "low",
    }
}

fn valid_severity(severity: &str) -> String {
    match severity {
        "critical" | "error" | "warning" | "info" => severity.to_string(),
        _ => "info".to_string(),
    }
}

fn valid_health_state(state: &str) -> String {
    match state {
        "healthy" | "watch" | "degraded" | "incident" | "recovering" | "needs_operator"
        | "verified_closed" => state.to_string(),
        _ => "watch".to_string(),
    }
}

fn incident_id_for(resource: &str, health_state: &str) -> String {
    format!("incident-{resource}-{health_state}").replace('_', "-")
}

fn report_status(snapshot_status: &str, forge_writes: &[ForgeWriteResult]) -> &'static str {
    if forge_writes.iter().any(|write| write.status == "failed") {
        "forge_write_failed"
    } else if snapshot_status == "failed" {
        "snapshot_failed_event_ready"
    } else {
        "completed"
    }
}

fn redact_report(report: &mut NexusHealthAgentReport) {
    if let Some(error) = report.snapshot_error.as_mut() {
        *error = redact_sensitive_text(error);
    }
    for artifact in &mut report.evidence_artifacts {
        redact_value_in_place(&mut artifact.payload);
    }
    redact_value_in_place(&mut report.forge_work_order_request.requested_outputs);
    redact_value_in_place(&mut report.forge_work_order_request.verification_policy);
    redact_value_in_place(&mut report.forge_event_request.payload);
    for write in &mut report.forge_writes {
        if let Some(error) = write.error.as_mut() {
            *error = redact_sensitive_text(error);
        }
        if let Some(response) = write.response.as_mut() {
            redact_value_in_place(response);
        }
    }
}

fn redact_value_in_place(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut replacement = Map::new();
            let mut redacted_count = 0_u64;
            let original = std::mem::take(map);
            for (key, mut nested) in original {
                if key_contains_secret_marker(&key) {
                    redacted_count = redacted_count.saturating_add(1);
                    continue;
                }
                redact_value_in_place(&mut nested);
                replacement.insert(key, nested);
            }
            if redacted_count > 0 {
                replacement.insert(
                    "removed_sensitive_field_count".to_string(),
                    json!(redacted_count),
                );
            }
            *map = replacement;
        }
        Value::Array(items) => {
            for item in items {
                redact_value_in_place(item);
            }
        }
        Value::String(raw) if string_contains_secret_marker(raw) => {
            *raw = "[redacted]".to_string();
        }
        _ => {}
    }
}

fn value_contains_secret_key(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, nested)| {
            key_contains_secret_marker(key) || value_contains_secret_key(nested)
        }),
        Value::Array(items) => items.iter().any(value_contains_secret_key),
        _ => false,
    }
}

fn value_contains_secret_string(value: &Value) -> bool {
    match value {
        Value::String(value) => string_contains_secret_marker(value),
        Value::Object(map) => map.values().any(value_contains_secret_string),
        Value::Array(items) => items.iter().any(value_contains_secret_string),
        _ => false,
    }
}

fn key_contains_secret_marker(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    SECRET_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn string_contains_secret_marker(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    SECRET_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn redact_sensitive_text(value: &str) -> String {
    if string_contains_secret_marker(value) {
        "[redacted]".to_string()
    } else {
        value.chars().take(320).collect()
    }
}

fn stable_value_digest(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

fn normalize_base_url(base_url: &str) -> Result<Url> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        bail!("base URL cannot be empty");
    }
    let normalized = if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/")
    };
    Url::parse(normalized.as_str()).with_context(|| format!("invalid base URL `{trimmed}`"))
}

fn env_string(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_arg<'a>(args: &'a [String], index: usize, option: &str) -> Result<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("{option} requires a value"))
}

fn parse_positive_u64(raw: &str, name: &str) -> Result<u64> {
    let parsed = raw
        .parse::<u64>()
        .with_context(|| format!("invalid {name} value `{raw}`"))?;
    if parsed == 0 {
        bail!("{name} must be greater than zero");
    }
    Ok(parsed)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use axum::extract::State;
    use axum::routing::post;
    use axum::{Json, Router};
    use serde_json::json;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    use super::*;

    #[test]
    fn parses_dry_run_command_without_forge_secrets() {
        let args = vec![
            "nexus-health-agent".to_string(),
            "--dry-run".to_string(),
            "--fake-nexus".to_string(),
            "--pretty".to_string(),
        ];
        let command = parse_nexus_health_agent_command(&args).expect("parse command");
        assert!(command.dry_run);
        assert!(command.fake_nexus);
        assert!(command.pretty);
        assert_eq!(command.nexus_base_url, DEFAULT_NEXUS_BASE_URL);
    }

    #[test]
    fn rejects_secret_shaped_base_urls() {
        let args = vec![
            "nexus-health-agent".to_string(),
            "--dry-run".to_string(),
            "--nexus-base-url".to_string(),
            "https://example.test/?token=abc".to_string(),
        ];
        assert!(parse_nexus_health_agent_command(&args).is_err());
    }

    #[tokio::test]
    async fn dry_run_fake_nexus_produces_monitor_evidence_without_secret_keys() {
        let command = NexusHealthAgentCommand {
            nexus_base_url: DEFAULT_NEXUS_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            forge_base_url: None,
            forge_bearer_token: None,
            forge_actor_jwt: None,
            project_id: "project-1".to_string(),
            actor_id: "nexus-health-runner".to_string(),
            fake_nexus: true,
            fake_forge: true,
            dry_run: true,
            pretty: false,
        };
        let report = run_nexus_health_agent(&command)
            .await
            .expect("run health agent");
        assert_eq!(report.status, "completed");
        assert_eq!(report.snapshot_status, "captured");
        assert_eq!(
            report.forge_work_order_request.work_order_kind,
            "nexus.health.monitor"
        );
        assert_eq!(
            report.forge_event_request.event_type,
            "nexus.health.snapshot_healthy"
        );
        assert_eq!(report.evidence_artifacts.len(), 1);
        let value = serde_json::to_value(&report).expect("serialize report");
        assert!(!value_contains_secret_key(&value));
        assert!(!value_contains_secret_string(&value));
        assert!(report.redaction.payload_sensitive_keys_absent);
        assert!(report.redaction.output_sensitive_strings_absent);
    }

    #[tokio::test]
    async fn fake_forge_integration_appends_health_event() {
        let fake_forge = FakeForge::spawn().await;
        let command = NexusHealthAgentCommand {
            nexus_base_url: DEFAULT_NEXUS_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            forge_base_url: Some(fake_forge.base_url.clone()),
            forge_bearer_token: Some("service-token".to_string()),
            forge_actor_jwt: Some("actor.jwt".to_string()),
            project_id: "project-1".to_string(),
            actor_id: "nexus-health-runner".to_string(),
            fake_nexus: true,
            fake_forge: false,
            dry_run: false,
            pretty: false,
        };

        let report = run_nexus_health_agent(&command)
            .await
            .expect("run health agent against fake Forge");
        assert_eq!(report.status, "completed");
        assert_eq!(report.forge_writes.len(), 2);
        assert_eq!(report.forge_writes[0].status, "created");
        assert_eq!(report.forge_writes[1].status, "appended");

        let requests = fake_forge.requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["path"], "/v1/health/work-orders");
        assert_eq!(requests[1]["path"], "/v1/health/events");
        assert_eq!(requests[1]["body"]["resource"], "nexus-relay");
        assert!(requests[1]["body"]["payload"]["evidence_artifacts"].is_array());
    }

    #[tokio::test]
    async fn public_endpoint_failures_still_produce_forge_event_plan() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        let command = NexusHealthAgentCommand {
            nexus_base_url: format!("http://{addr}"),
            timeout_ms: 50,
            forge_base_url: None,
            forge_bearer_token: None,
            forge_actor_jwt: None,
            project_id: "project-1".to_string(),
            actor_id: "nexus-health-runner".to_string(),
            fake_nexus: false,
            fake_forge: true,
            dry_run: true,
            pretty: false,
        };

        let report = run_nexus_health_agent(&command)
            .await
            .expect("endpoint failures should be represented in report");
        assert_eq!(report.snapshot_status, "captured");
        let snapshot = report.snapshot.expect("snapshot should exist");
        assert_eq!(snapshot.classification.state, "incident");
        assert_eq!(
            report.forge_event_request.event_type,
            "nexus.health.issue_detected"
        );
    }

    #[test]
    fn mutating_action_requires_forge_lease() {
        let missing = HealthAgentActionPlan {
            action_kind: "recover".to_string(),
            forge_lease_id: None,
        };
        assert!(validate_health_agent_action_plan(&missing).is_err());

        let monitor = HealthAgentActionPlan {
            action_kind: "monitor".to_string(),
            forge_lease_id: None,
        };
        assert!(validate_health_agent_action_plan(&monitor).is_ok());

        let leased = HealthAgentActionPlan {
            action_kind: "recover".to_string(),
            forge_lease_id: Some("forge-lease-123".to_string()),
        };
        assert!(validate_health_agent_action_plan(&leased).is_ok());
    }

    #[test]
    fn redaction_removes_secret_shaped_keys_and_strings() {
        let mut value = json!({
            "safe": "ok",
            "bearer_token": "do-not-store",
            "nested": {
                "message": "Authorization: Bearer abc"
            }
        });
        redact_value_in_place(&mut value);
        assert!(!value_contains_secret_key(&value));
        assert!(!value_contains_secret_string(&value));
        assert_eq!(value["safe"], "ok");
        assert_eq!(value["nested"]["message"], "[redacted]");
    }

    struct FakeForge {
        base_url: String,
        requests: Arc<Mutex<Vec<Value>>>,
    }

    impl FakeForge {
        async fn spawn() -> Self {
            let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
            let app = Router::new()
                .route("/v1/health/work-orders", post(fake_work_order))
                .route("/v1/health/events", post(fake_health_event))
                .with_state(requests.clone());
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr: SocketAddr = listener.local_addr().expect("local addr");
            tokio::spawn(async move {
                axum::serve(listener, app).await.expect("serve fake Forge");
            });
            Self {
                base_url: format!("http://{addr}"),
                requests,
            }
        }
    }

    async fn fake_work_order(
        State(requests): State<Arc<Mutex<Vec<Value>>>>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        requests
            .lock()
            .await
            .push(json!({"path": "/v1/health/work-orders", "body": body}));
        Json(json!({
            "work_order_id": "forge-work-fake-health",
            "work_order_state": "queued",
            "kind": "nexus.health.monitor",
            "idempotent_replay": false
        }))
    }

    async fn fake_health_event(
        State(requests): State<Arc<Mutex<Vec<Value>>>>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        requests
            .lock()
            .await
            .push(json!({"path": "/v1/health/events", "body": body}));
        Json(json!({
            "event": {
                "id": "forge-health-event-fake",
                "idempotency_key": "event-key"
            }
        }))
    }
}
