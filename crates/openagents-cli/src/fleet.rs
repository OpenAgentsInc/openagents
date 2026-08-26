//! `oa deploy`: the operator fleet promotion client.
//!
//! A port of `packages/openagents-cli/src/fleet-client.ts`. It speaks only the
//! operator API — `POST/GET /api/v1/admin/forge/targets` — behind the same
//! error envelope every other command family reads. It never touches
//! `/admin/forge`, SSH, or any internal RPC, and it adds only what a terminal
//! caller cannot do for itself: an idempotent re-send after a failed
//! transport, and bounded polling of the status resource to a terminal state.
//!
//! The privileged scope is `deployments:promote`. `forge:write` cannot
//! promote, and neither can a Git credential or a browser session, so a 401 or
//! 403 from any route here is answered with the command that obtains one
//! rather than a bare status.

use crate::tracker::{ApiError, TrackerClient};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// The one route family this client speaks.
pub const FLEET_TARGETS_PATH: &str = "admin/forge/targets";

/// The privileged scope the server requires.
pub const OPERATOR_SCOPE: &str = "deployments:promote";

/// The states polling stops on.
///
/// The server marks `live`, `failed`, and `reverted` terminal;
/// `needs_rolling_replace` additionally ends automatic execution and waits on
/// an operator, so a poll that reached it would otherwise never return.
pub const TERMINAL_STATES: &[&str] = &["live", "failed", "reverted", "needs_rolling_replace"];

/// How many times a promotion is re-sent after a failed transport.
pub const PROMOTE_TRANSPORT_RETRIES: usize = 2;

/// Bounded backoff: 2s, 4s, 8s, then every 10s until the deadline.
pub const POLL_BASE_DELAY_MS: u64 = 2_000;
pub const POLL_MAXIMUM_DELAY_MS: u64 = 10_000;

const RETRY_DELAY_MS: u64 = 500;

/// Reads the lifecycle state off a target body.
pub fn target_status(target: &Value) -> String {
    target
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

/// Whether polling has nothing further to learn about this target.
pub fn terminal_status(status: &str) -> bool {
    TERMINAL_STATES.contains(&status)
}

pub fn target_id(target: &Value) -> String {
    match target.get("id") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn string_field(target: &Value, key: &str) -> String {
    match target.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// Environment, repository, full SHA, target ID, state, and status URL come
/// before any success wording, so the operator reads what was promoted before
/// reading how it went.
pub fn target_human(target: &Value) -> Vec<String> {
    vec![
        format!("Environment: {}", string_field(target, "environment")),
        format!("Repository:  {}", string_field(target, "repo")),
        format!("SHA:         {}", string_field(target, "sha")),
        format!("Target:      {}", target_id(target)),
        format!("State:       {}", target_status(target)),
        format!("Status URL:  {}", string_field(target, "status_url")),
    ]
}

pub fn failure_code(target: &Value) -> Option<String> {
    target
        .get("error_code")
        .and_then(Value::as_str)
        .map(String::from)
}

/// The `--json` document. Same schema names and same derived fields as the
/// TypeScript, so a script reads one shape from either binary.
pub fn target_document(
    schema: &str,
    target: &Value,
    nonterminal: &str,
    extra: &[(&str, Value)],
) -> Value {
    let status = target_status(target);
    let mut map = serde_json::Map::new();
    map.insert("schema".into(), Value::String(schema.into()));
    for (key, value) in extra {
        map.insert((*key).into(), value.clone());
    }
    map.insert(
        "outcome".into(),
        Value::String(if terminal_status(&status) {
            status.clone()
        } else {
            nonterminal.to_string()
        }),
    );
    map.insert("live".into(), Value::Bool(status == "live"));
    map.insert("terminal".into(), Value::Bool(terminal_status(&status)));
    map.insert(
        "failure_code".into(),
        failure_code(target)
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    map.insert("target".into(), target.clone());
    Value::Object(map)
}

pub fn terminal_human(target: &Value) -> String {
    match target_status(target).as_str() {
        "live" => "The fleet target is live.".to_string(),
        "needs_rolling_replace" => {
            "The target needs an operator-driven rolling replacement; the automatic lanes stopped."
                .to_string()
        }
        other => format!("The fleet target reached {other}."),
    }
}

/// One row of `deploy list`.
pub fn target_row(target: &Value) -> String {
    format!(
        "{:<38}{:<22}{}  {}",
        target_id(target),
        target_status(target),
        string_field(target, "sha"),
        string_field(target, "promoted_at")
    )
}

/// An operator refused for standing or scope needs the exact next command, not
/// a bare status. The remediation names the privileged scope and says plainly
/// that `forge:write` is not it.
pub fn operator_remediation(error: ApiError) -> ApiError {
    match error {
        ApiError::Refused {
            operation,
            status: status @ (401 | 403),
            message,
        } => ApiError::Refused {
            operation,
            status,
            message: format!(
                "{message} Fleet promotion requires an operator API token holding \
                 {OPERATOR_SCOPE}; forge:write cannot promote, and neither can a Git credential \
                 or a browser session. An operator obtains one with: oa auth login --scope \
                 {OPERATOR_SCOPE}"
            ),
        },
        other => other,
    }
}

/// A full 40-character commit SHA. Branch names and abbreviations are refused.
pub fn full_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub struct FleetClient {
    tracker: TrackerClient,
}

pub struct PromoteInput {
    pub repo: String,
    pub sha: String,
    pub environment: String,
    /// Generated once by the caller and reused across automatic retries.
    pub idempotency_key: String,
    pub expected_current_target_id: Option<String>,
}

pub struct PromoteResult {
    /// True when the server answered `202 Accepted` with a new target.
    pub accepted: bool,
    /// True when the idempotency key replayed an existing identical promotion.
    pub replayed: bool,
    pub target: Value,
}

impl FleetClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            tracker: TrackerClient::new(api_base, token),
        }
    }

    pub async fn view(&self, id: &str) -> Result<Value, ApiError> {
        self.tracker
            .request(
                "read a fleet target",
                "GET",
                &format!("{FLEET_TARGETS_PATH}/{}", urlencode(id)),
                None,
                &[200],
            )
            .await
            .map_err(operator_remediation)
    }

    pub async fn list(&self, repo: Option<&str>, limit: Option<u32>) -> Result<Value, ApiError> {
        let mut query: Vec<String> = Vec::new();
        if let Some(repo) = repo {
            query.push(format!("repo={}", urlencode(repo)));
        }
        if let Some(limit) = limit {
            query.push(format!("limit={limit}"));
        }
        let path = if query.is_empty() {
            FLEET_TARGETS_PATH.to_string()
        } else {
            format!("{FLEET_TARGETS_PATH}?{}", query.join("&"))
        };
        self.tracker
            .request("list fleet targets", "GET", &path, None, &[200])
            .await
            .map_err(operator_remediation)
    }

    /// One promotion attempt.
    ///
    /// `202` admits a new target; `200` replays the identical promotion the
    /// same key already named. The distinction is the answer to "did I just
    /// deploy, or had I already?", so it is read off the status rather than
    /// folded into a shared accepted-status helper.
    async fn promote_attempt(&self, input: &PromoteInput) -> Result<PromoteResult, ApiError> {
        let mut body = json!({
            "repo": input.repo,
            "sha": input.sha,
            "environment": input.environment,
            "idempotency_key": input.idempotency_key,
        });
        if let Some(expected) = &input.expected_current_target_id {
            body["expected_current_target_id"] = Value::String(expected.clone());
        }
        // `202` admits a new target; `200` replays the identical promotion the
        // same key already named. The distinction is the answer to "did I just
        // deploy, or had I already?", so it is read off the status the server
        // chose. The body cannot answer it — the server returns the target
        // either way — which is why this asks for the status rather than
        // guessing from a marker the server may not send.
        let (status, target) = self
            .tracker
            .request_with_status(
                "promote a fleet target",
                "POST",
                FLEET_TARGETS_PATH,
                Some(body),
                &[200, 202],
            )
            .await?;
        Ok(PromoteResult {
            accepted: status == 202,
            replayed: status == 200,
            target,
        })
    }

    /// The idempotency key travels in the body, so every attempt names the
    /// same promotion and a re-send can never deploy twice. Only a failed
    /// transport is retried — the request may never have reached the server; a
    /// refusal the server actually made is final.
    pub async fn promote(&self, input: &PromoteInput) -> Result<PromoteResult, ApiError> {
        let mut attempt = 0usize;
        loop {
            match self.promote_attempt(input).await {
                Ok(result) => return Ok(result),
                Err(ApiError::Transport { operation, why }) => {
                    if attempt >= PROMOTE_TRANSPORT_RETRIES {
                        return Err(ApiError::Transport { operation, why });
                    }
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS * attempt as u64))
                        .await;
                }
                Err(other) => return Err(operator_remediation(other)),
            }
        }
    }

    /// Poll the status resource until the target reaches a terminal state, or
    /// the deadline passes.
    ///
    /// A timeout is not a failed deployment: the target keeps running, so the
    /// error says so and names the command that resumes watching.
    pub async fn wait(&self, id: &str, timeout: Duration) -> Result<Value, ApiError> {
        let started = Instant::now();
        let mut attempt = 0u32;
        loop {
            let target = self.view(id).await?;
            if terminal_status(&target_status(&target)) {
                return Ok(target);
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return Err(ApiError::Input(format!(
                    "The fleet target {id} was still {} after {} seconds. It keeps running; \
                     resume with: oa deploy view {id} --wait",
                    target_status(&target),
                    timeout.as_secs()
                )));
            }
            let delay = poll_delay(attempt);
            let remaining = timeout - elapsed;
            tokio::time::sleep(delay.min(remaining)).await;
            attempt += 1;
        }
    }
}

/// 2s, 4s, 8s, then 10s.
pub fn poll_delay(attempt: u32) -> Duration {
    let scaled = POLL_BASE_DELAY_MS.saturating_mul(1u64 << attempt.min(16));
    Duration::from_millis(scaled.min(POLL_MAXIMUM_DELAY_MS))
}

/// Percent-encode a path or query segment.
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_full_sha_is_accepted() {
        assert!(full_sha("0123456789abcdef0123456789abcdef01234567"));
        assert!(!full_sha("0123456"));
        assert!(!full_sha("main"));
        assert!(!full_sha(&"z".repeat(40)));
    }

    #[test]
    fn the_four_terminal_states_stop_polling() {
        for state in ["live", "failed", "reverted", "needs_rolling_replace"] {
            assert!(terminal_status(state), "{state}");
        }
        for state in ["queued", "building", "rolling", "unknown"] {
            assert!(!terminal_status(state), "{state}");
        }
    }

    #[test]
    fn backoff_is_bounded() {
        assert_eq!(poll_delay(0), Duration::from_millis(2_000));
        assert_eq!(poll_delay(1), Duration::from_millis(4_000));
        assert_eq!(poll_delay(2), Duration::from_millis(8_000));
        assert_eq!(poll_delay(3), Duration::from_millis(10_000));
        assert_eq!(poll_delay(40), Duration::from_millis(10_000));
    }

    #[test]
    fn a_scope_refusal_names_the_command_that_fixes_it() {
        let error = operator_remediation(ApiError::Refused {
            operation: "list fleet targets".into(),
            status: 401,
            message: "Requires an API token carrying deployments:promote".into(),
        });
        let rendered = error.to_string();
        assert!(rendered.contains("deployments:promote"));
        assert!(rendered.contains("forge:write cannot promote"));
        assert!(rendered.contains("oa auth login --scope deployments:promote"));
    }

    #[test]
    fn a_non_auth_refusal_is_left_alone() {
        let error = operator_remediation(ApiError::Refused {
            operation: "read a fleet target".into(),
            status: 404,
            message: "No such target.".into(),
        });
        assert!(!error.to_string().contains("deployments:promote"));
    }

    #[test]
    fn the_document_carries_the_derived_state() {
        let target = json!({
            "id": "tgt-1",
            "status": "live",
            "sha": "a".repeat(40),
            "repo": "openagents.com",
            "environment": "production",
        });
        let document = target_document("openagents.fleet_target.v1", &target, "pending", &[]);
        assert_eq!(document["schema"], "openagents.fleet_target.v1");
        assert_eq!(document["outcome"], "live");
        assert_eq!(document["live"], true);
        assert_eq!(document["terminal"], true);
        assert_eq!(document["failure_code"], Value::Null);
        assert_eq!(document["target"]["id"], "tgt-1");

        let pending = json!({ "id": "tgt-2", "status": "building" });
        let document = target_document("openagents.fleet_target.v1", &pending, "pending", &[]);
        assert_eq!(document["outcome"], "pending");
        assert_eq!(document["live"], false);
        assert_eq!(document["terminal"], false);
    }

    #[test]
    fn the_human_view_leads_with_what_was_promoted() {
        let target = json!({
            "id": "tgt-1",
            "status": "queued",
            "sha": "b".repeat(40),
            "repo": "openagents.com",
            "environment": "production",
            "status_url": "https://openagents.com/api/v1/admin/forge/targets/tgt-1",
        });
        let lines = target_human(&target);
        assert!(lines[0].starts_with("Environment: production"));
        assert!(lines[1].starts_with("Repository:  openagents.com"));
        assert!(lines[3].contains("tgt-1"));
        assert!(lines[4].contains("queued"));
    }
}
