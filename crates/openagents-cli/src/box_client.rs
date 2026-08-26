//! Box sandbox lifecycle, durable runs, and fanout admission.
//!
//! The Rust port of `packages/openagents-cli/src/box-client.ts`. Boxes are
//! conversation-owned cloud VMs with a hard provisioning quota, so this client
//! never manufactures one:
//!
//! - A conversation that cannot be resolved is a refusal naming `--conversation`,
//!   not the literal string `main`. The version this replaces defaulted to
//!   `main`, which is not a conversation id, and then answered the resulting
//!   non-2xx with an empty list — indistinguishable from "you have no boxes".
//! - A refused provision is a refusal. There is no placeholder box record and
//!   no invented `box_id`, because a caller who believes a box exists will
//!   spend the quota trying to reach it.
//! - `exec` reports the box's exit status. A transport or authorization failure
//!   is an error, not exit code 1 with the failure text pushed into `stderr`,
//!   which is what a real failing command looks like.

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::tracker::{error_fields, error_sentence, header_request_id, urlencode, ApiError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoxRecord {
    pub box_id: String,
    pub label: Option<String>,
    pub state: String,
    pub setup_status: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub stopped_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoxCommandResult {
    pub box_id: String,
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoxRunRecord {
    pub id: String,
    pub box_id: String,
    pub command: String,
    pub state: String,
    pub exit_status: Option<i64>,
    pub timed_out: Option<bool>,
    pub output_offset: Option<u64>,
    pub output_base_offset: Option<u64>,
    pub failure_reason: Option<String>,
    pub admitted_at: Option<String>,
    pub dispatched_at: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub deadline_at: Option<String>,
    pub cancellation_requested_at: Option<String>,
    pub cancellation_effective_at: Option<String>,
}

impl BoxRunRecord {
    /// True once the server will produce no further output for this run.
    ///
    /// These are the server's own terminal states, from
    /// `OpenAgents.Box.Run`'s `@terminal_states` — `completed failed cancelled
    /// timed_out lost`. Getting this list wrong is not cosmetic: the whole
    /// list existed to end `--follow`, and an earlier version of it matched
    /// `succeeded`, `canceled`, and `expired`, none of which the server ever
    /// sends, while missing `completed`, which is what a run that worked ends
    /// in. Against production that made `oa box runs output --follow` spin
    /// past the end of a successful run until the API refused a request. The
    /// spellings that never appear are deliberately not kept "just in case":
    /// carrying them is what made the mistake survive a passing test.
    pub fn finished(&self) -> bool {
        matches!(
            self.state.as_str(),
            "completed" | "failed" | "cancelled" | "timed_out" | "lost"
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoxRunOutput {
    pub run_id: String,
    pub output: String,
    /// The offset to pass to the next read to resume where this one stopped.
    pub next_offset: u64,
    /// True when the box dropped bytes before the requested offset.
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoxFanoutItem {
    pub position: u64,
    pub label: String,
    pub state: String,
    pub box_id: Option<String>,
    pub queue_reason: Option<String>,
    pub estimated_burn_rate_microusd: Option<i64>,
    pub admitted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoxFanoutPlan {
    pub id: String,
    pub requested_count: u64,
    pub admitted: Vec<BoxFanoutItem>,
    pub queued: Vec<BoxFanoutItem>,
    pub effective_limits: Value,
    pub budgeted: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

pub struct BoxClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(String::from)
}

fn parse_box(value: &Value) -> BoxRecord {
    BoxRecord {
        box_id: text(value, "box_id")
            .or_else(|| text(value, "id"))
            .unwrap_or_default(),
        label: text(value, "label"),
        state: text(value, "state").unwrap_or_else(|| "unknown".to_string()),
        setup_status: text(value, "setup_status").unwrap_or_else(|| "unknown".to_string()),
        created_at: text(value, "created_at").unwrap_or_default(),
        updated_at: text(value, "updated_at"),
        stopped_at: text(value, "stopped_at"),
    }
}

fn parse_run(value: &Value, fallback_box: &str, fallback_run: &str) -> BoxRunRecord {
    BoxRunRecord {
        id: text(value, "id").unwrap_or_else(|| fallback_run.to_string()),
        box_id: text(value, "box_id").unwrap_or_else(|| fallback_box.to_string()),
        command: text(value, "command").unwrap_or_default(),
        state: text(value, "state").unwrap_or_else(|| "unknown".to_string()),
        exit_status: value.get("exit_status").and_then(Value::as_i64),
        timed_out: value.get("timed_out").and_then(Value::as_bool),
        output_offset: value.get("output_offset").and_then(Value::as_u64),
        output_base_offset: value.get("output_base_offset").and_then(Value::as_u64),
        failure_reason: text(value, "failure_reason"),
        admitted_at: text(value, "admitted_at"),
        dispatched_at: text(value, "dispatched_at"),
        started_at: text(value, "started_at"),
        finished_at: text(value, "finished_at"),
        deadline_at: text(value, "deadline_at"),
        cancellation_requested_at: text(value, "cancellation_requested_at"),
        cancellation_effective_at: text(value, "cancellation_effective_at"),
    }
}

fn parse_fanout_item(value: &Value) -> BoxFanoutItem {
    BoxFanoutItem {
        position: value.get("position").and_then(Value::as_u64).unwrap_or(0),
        label: text(value, "label").unwrap_or_default(),
        state: text(value, "state").unwrap_or_else(|| "unknown".to_string()),
        box_id: text(value, "box_id"),
        queue_reason: text(value, "queue_reason"),
        estimated_burn_rate_microusd: value
            .get("estimated_burn_rate_microusd")
            .and_then(Value::as_i64),
        admitted_at: text(value, "admitted_at"),
    }
}

fn parse_plan(value: &Value, fallback_id: &str, fallback_count: u64) -> BoxFanoutPlan {
    let rows = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_array)
            .map(|items| items.iter().map(parse_fanout_item).collect())
            .unwrap_or_default()
    };
    BoxFanoutPlan {
        id: text(value, "id").unwrap_or_else(|| fallback_id.to_string()),
        requested_count: value
            .get("requested_count")
            .and_then(Value::as_u64)
            .unwrap_or(fallback_count),
        admitted: rows("admitted"),
        queued: rows("queued"),
        effective_limits: value
            .get("effective_limits")
            .cloned()
            .unwrap_or(Value::Null),
        budgeted: value
            .get("budgeted")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        created_at: text(value, "created_at"),
        updated_at: text(value, "updated_at"),
    }
}

impl BoxClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(tok) = &self.token {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", tok)) {
                map.insert(AUTHORIZATION, val);
            }
        }
        map
    }

    async fn request(
        &self,
        operation: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
        accepted: &[u16],
    ) -> Result<Value, ApiError> {
        let url = format!("{}/{}", self.api_base, path.trim_start_matches('/'));
        let mut builder = match method {
            "GET" => self.http.get(&url),
            "POST" => self.http.post(&url),
            "DELETE" => self.http.delete(&url),
            other => {
                return Err(ApiError::Input(format!(
                    "{} is not an HTTP method this client sends.",
                    other
                )))
            }
        }
        .headers(self.headers());
        if let Some(payload) = body {
            builder = builder.json(&payload);
        }

        crate::diag::request(method, &url);
        let response = builder.send().await.map_err(|e| {
            crate::diag::transport(&url, &e.to_string());
            ApiError::Transport {
                operation: operation.to_string(),
                why: e.to_string(),
            }
        })?;
        let status = response.status().as_u16();
        crate::diag::response(status, &url);
        // Read before the body is consumed; the header outranks the body's own
        // `request_id`, as it does in the TypeScript transport.
        let header_id = header_request_id(&response);
        let text = response.text().await.map_err(|e| ApiError::Transport {
            operation: operation.to_string(),
            why: e.to_string(),
        })?;

        if !accepted.contains(&status) {
            let message = error_sentence(&text, status);
            crate::diag::refused(status, &message);
            let (code, body_id) = error_fields(&text);
            return Err(ApiError::Refused {
                operation: operation.to_string(),
                status,
                message,
                code,
                request_id: header_id.or(body_id),
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| ApiError::Malformed {
            operation: operation.to_string(),
            why: e.to_string(),
        })
    }

    /// Resolves the account's conversation, or refuses naming the flag that
    /// unblocks the caller.
    ///
    /// `/conversation` is the route a `box:control` token can reach, and it
    /// creates the account's conversation when there is none. `/user` sits
    /// behind `forge:write`, and a deployment that predates `/conversation`
    /// still answers there, so both are tried before refusing. Neither
    /// answering is a refusal — not a default conversation id.
    pub async fn resolve_conversation_id(&self) -> Result<String, ApiError> {
        let named = self
            .request("resolve user conversation", "GET", "conversation", None, &[200])
            .await;
        if let Ok(body) = &named {
            if let Some(id) = text(body, "conversation_id") {
                return Ok(id);
            }
        }

        let user = self
            .request("resolve user conversation", "GET", "user", None, &[200])
            .await;
        if let Ok(body) = &user {
            let id = text(body, "conversation_id")
                .or_else(|| {
                    body.get("openagents")
                        .and_then(|v| text(v, "conversation_id"))
                })
                .or_else(|| body.get("user").and_then(|v| text(v, "conversation_id")));
            if let Some(id) = id {
                return Ok(id);
            }
        }

        // A route that broke is not a deployment that has no conversation.
        // Only a refusal the *server* authored — it read the token and said no
        // — means "ask for the conversation another way". A 5xx, a gateway
        // error page, or a dead socket means the request never got an answer,
        // and reporting either of those as "this deployment does not report a
        // conversation for the account" sends the reader to fix a
        // configuration that was never wrong. Observed against production: a
        // transient 502 on `GET /api/v1/conversation` printed exactly that
        // sentence, for an account whose conversation resolved fine a minute
        // earlier and a minute later.
        let mut status = 200;
        for outcome in [named, user] {
            match outcome {
                Ok(_) => {}
                Err(ApiError::Refused { status: code, .. }) if code < 500 => {
                    if status == 200 {
                        status = code;
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Err(ApiError::Refused {
            operation: "resolve user conversation".to_string(),
            status,
            message: "This deployment does not report a conversation for the account. \
                      Pass --conversation <conversation_id> to name the conversation to use."
                .to_string(),
            // This refusal is the client's summary of two the server sent, so
            // no single `code` or request id belongs to it. The status is the
            // server's, and the status is what the ladder reads.
            code: None,
            request_id: None,
        })
    }

    /// The conversation the caller named, or the one the account reports.
    pub async fn conversation_id(&self, named: Option<&str>) -> Result<String, ApiError> {
        match named {
            Some(id) if !id.trim().is_empty() => Ok(id.trim().to_string()),
            _ => self.resolve_conversation_id().await,
        }
    }

    fn boxes_path(conversation: &str) -> String {
        format!("conversations/{}/boxes", urlencode(conversation))
    }

    fn box_path(conversation: &str, box_id: &str) -> String {
        format!("{}/{}", Self::boxes_path(conversation), urlencode(box_id))
    }

    fn runs_path(conversation: &str, box_id: &str) -> String {
        format!("{}/runs", Self::box_path(conversation, box_id))
    }

    fn run_path(conversation: &str, box_id: &str, run_id: &str) -> String {
        format!(
            "{}/{}",
            Self::runs_path(conversation, box_id),
            urlencode(run_id)
        )
    }

    pub async fn list_boxes(&self, conversation: &str) -> Result<Vec<BoxRecord>, ApiError> {
        let body = self
            .request(
                "list conversation boxes",
                "GET",
                &Self::boxes_path(conversation),
                None,
                &[200],
            )
            .await?;
        let rows = body
            .get("boxes")
            .and_then(Value::as_array)
            .ok_or_else(|| ApiError::Malformed {
                operation: "list conversation boxes".to_string(),
                why: "no `boxes` array in the response".to_string(),
            })?;
        Ok(rows.iter().map(parse_box).collect())
    }

    pub async fn create_box(
        &self,
        conversation: &str,
        label: Option<&str>,
    ) -> Result<BoxRecord, ApiError> {
        let payload = match label {
            Some(name) => json!({ "label": name }),
            None => json!({}),
        };
        let body = self
            .request(
                "create box",
                "POST",
                &Self::boxes_path(conversation),
                Some(payload),
                &[201],
            )
            .await?;
        Ok(parse_box(body.get("box").unwrap_or(&body)))
    }

    pub async fn view_box(&self, conversation: &str, box_id: &str) -> Result<BoxRecord, ApiError> {
        let body = self
            .request(
                "view box",
                "GET",
                &Self::box_path(conversation, box_id),
                None,
                &[200],
            )
            .await?;
        Ok(parse_box(body.get("box").unwrap_or(&body)))
    }

    pub async fn execute_command(
        &self,
        conversation: &str,
        box_id: &str,
        command: &str,
        timeout_seconds: Option<u64>,
    ) -> Result<BoxCommandResult, ApiError> {
        let mut payload = json!({ "command": command });
        if let Some(seconds) = timeout_seconds {
            payload["timeout_seconds"] = json!(seconds);
        }
        let body = self
            .request(
                "run box command",
                "POST",
                &format!("{}/commands", Self::box_path(conversation, box_id)),
                Some(payload),
                &[200],
            )
            .await?;
        let result = body.get("result").unwrap_or(&body);
        Ok(BoxCommandResult {
            box_id: text(result, "box_id").unwrap_or_else(|| box_id.to_string()),
            // The server sends the box's exit status. A missing one is not
            // success, and it is not a failure of the command either, so it is
            // reported as -1 the way the TypeScript client reports it.
            exit_code: result.get("exit_code").and_then(Value::as_i64).unwrap_or(-1),
            stdout: text(result, "stdout").unwrap_or_default(),
            stderr: text(result, "stderr").unwrap_or_default(),
            timed_out: result
                .get("timed_out")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            stdout_truncated: result
                .get("stdout_truncated")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            stderr_truncated: result
                .get("stderr_truncated")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    pub async fn stop_box(&self, conversation: &str, box_id: &str) -> Result<BoxRecord, ApiError> {
        let body = self
            .request(
                "stop box",
                "POST",
                &format!("{}/stop", Self::box_path(conversation, box_id)),
                // An empty object, not no body. A `POST` carrying
                // `Content-Type: application/json` and no `Content-Length` is
                // rejected by the edge with 411 before it reaches the
                // application, so the caller never sees the server's answer.
                // The TypeScript client sends no body here and earns the same
                // 411; observed against production on 2026-08-26.
                Some(json!({})),
                &[200],
            )
            .await?;
        Ok(parse_box(body.get("box").unwrap_or(&body)))
    }

    pub async fn start_run(
        &self,
        conversation: &str,
        box_id: &str,
        command: &str,
        idempotency_key: Option<&str>,
    ) -> Result<BoxRunRecord, ApiError> {
        let key = match idempotency_key {
            Some(value) => value.to_string(),
            None => fresh_idempotency_key(),
        };
        let body = self
            .request(
                "start box run",
                "POST",
                &Self::runs_path(conversation, box_id),
                Some(json!({ "command": command, "idempotency_key": key })),
                &[200, 202],
            )
            .await?;
        Ok(parse_run(body.get("run").unwrap_or(&body), box_id, ""))
    }

    pub async fn list_runs(
        &self,
        conversation: &str,
        box_id: &str,
    ) -> Result<Vec<BoxRunRecord>, ApiError> {
        let body = self
            .request(
                "list box runs",
                "GET",
                &Self::runs_path(conversation, box_id),
                None,
                &[200],
            )
            .await?;
        let rows = body
            .get("runs")
            .and_then(Value::as_array)
            .ok_or_else(|| ApiError::Malformed {
                operation: "list box runs".to_string(),
                why: "no `runs` array in the response".to_string(),
            })?;
        Ok(rows.iter().map(|row| parse_run(row, box_id, "")).collect())
    }

    pub async fn view_run(
        &self,
        conversation: &str,
        box_id: &str,
        run_id: &str,
    ) -> Result<BoxRunRecord, ApiError> {
        let body = self
            .request(
                "view box run",
                "GET",
                &Self::run_path(conversation, box_id, run_id),
                None,
                &[200],
            )
            .await?;
        Ok(parse_run(body.get("run").unwrap_or(&body), box_id, run_id))
    }

    /// Reads a window of a run's output.
    ///
    /// The server nests the read under `output`: the envelope is
    /// `{"run_id": …, "output": {"output": …, "next_offset": …, "truncated": …}}`.
    /// A deployment answering flat is read flat.
    pub async fn run_output(
        &self,
        conversation: &str,
        box_id: &str,
        run_id: &str,
        offset: Option<u64>,
    ) -> Result<BoxRunOutput, ApiError> {
        let query = match offset {
            Some(value) => format!("?offset={}", value),
            None => String::new(),
        };
        let body = self
            .request(
                "get box run output",
                "GET",
                &format!(
                    "{}/output{}",
                    Self::run_path(conversation, box_id, run_id),
                    query
                ),
                None,
                &[200],
            )
            .await?;
        let nested = body.get("output").cloned().unwrap_or(Value::Null);
        let flat = nested.as_str().map(String::from);
        Ok(BoxRunOutput {
            run_id: text(&body, "run_id").unwrap_or_else(|| run_id.to_string()),
            output: flat
                .or_else(|| text(&nested, "output"))
                .unwrap_or_default(),
            next_offset: nested
                .get("next_offset")
                .and_then(Value::as_u64)
                .unwrap_or(offset.unwrap_or(0)),
            truncated: nested
                .get("truncated")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    /// Read a run's output until the run reaches a terminal state.
    ///
    /// The route publishes no event stream, so following it is a poll: read
    /// from the last offset, hand whatever is new to `sink`, then ask the run
    /// whether it has finished. One further read after the terminal state is
    /// what keeps the last bytes from being dropped between the final write and
    /// the state change.
    ///
    /// Every read that fails ends the follow. A poll that swallowed a refusal
    /// would hand the caller a truncated log as though it were the whole run,
    /// which is the same failure as printing an empty list for a refused read.
    ///
    /// Returns the terminal run record and the offset the reader stopped at.
    pub async fn follow_run_output<F>(
        &self,
        conversation: &str,
        box_id: &str,
        run_id: &str,
        offset: Option<u64>,
        interval: std::time::Duration,
        mut sink: F,
    ) -> Result<(BoxRunRecord, u64), ApiError>
    where
        F: FnMut(&BoxRunOutput),
    {
        let mut cursor = offset;
        loop {
            let chunk = self
                .run_output(conversation, box_id, run_id, cursor)
                .await?;
            sink(&chunk);
            cursor = Some(chunk.next_offset);

            let run = self.view_run(conversation, box_id, run_id).await?;
            if run.finished() {
                let tail = self
                    .run_output(conversation, box_id, run_id, cursor)
                    .await?;
                if !tail.output.is_empty() {
                    sink(&tail);
                }
                return Ok((run, tail.next_offset));
            }
            // Sleep on every pass, including the ones that carried output.
            // This used to sleep only when the offset had not advanced, so a
            // run that printed steadily was followed by an unthrottled loop
            // issuing two requests per iteration as fast as the network
            // allowed. `--interval-ms` is the poll rate the caller asked for;
            // it is not a rate that applies only when nothing is happening.
            tokio::time::sleep(interval).await;
        }
    }

    pub async fn cancel_run(
        &self,
        conversation: &str,
        box_id: &str,
        run_id: &str,
    ) -> Result<BoxRunRecord, ApiError> {
        let body = self
            .request(
                "cancel box run",
                "POST",
                &format!("{}/cancel", Self::run_path(conversation, box_id, run_id)),
                // See `stop_box`: an empty object rather than no body, so the
                // edge does not answer 411 in place of the server.
                Some(json!({})),
                &[200, 202],
            )
            .await?;
        Ok(parse_run(body.get("run").unwrap_or(&body), box_id, run_id))
    }

    pub async fn fanout(
        &self,
        conversation: &str,
        count: u64,
        labels: &[String],
        budgeted: bool,
    ) -> Result<BoxFanoutPlan, ApiError> {
        if count < 1 {
            return Err(ApiError::Input("--count must be at least 1.".to_string()));
        }
        let mut payload = json!({ "count": count, "budgeted": budgeted });
        if !labels.is_empty() {
            payload["labels"] = json!(labels);
        }
        let body = self
            .request(
                "request box fanout",
                "POST",
                &format!("{}/fanout", Self::boxes_path(conversation)),
                Some(payload),
                &[200, 202],
            )
            .await?;
        Ok(parse_plan(body.get("plan").unwrap_or(&body), "", count))
    }

    pub async fn view_fanout(
        &self,
        conversation: &str,
        request_id: &str,
    ) -> Result<BoxFanoutPlan, ApiError> {
        let body = self
            .request(
                "view box fanout",
                "GET",
                &format!(
                    "{}/fanout/{}",
                    Self::boxes_path(conversation),
                    urlencode(request_id)
                ),
                None,
                &[200],
            )
            .await?;
        Ok(parse_plan(body.get("plan").unwrap_or(&body), request_id, 0))
    }
}

/// A fresh idempotency key for a durable run.
///
/// The crate has no UUID dependency, so this is a v4-shaped identifier built
/// from the clock, the process, and a per-process counter, hashed so the parts
/// do not leak into the key. It names nothing about the run; it only has to be
/// distinct from the last one, which is what the server uses it for.
fn fresh_idempotency_key() -> String {
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seed = format!(
        "{}:{}:{}",
        nanos,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let digest = Sha256::digest(seed.as_bytes());
    let hex: String = digest.iter().take(16).map(|b| format!("{:02x}", b)).collect();
    format!(
        "{}-{}-4{}-a{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[13..16],
        &hex[17..20],
        &hex[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotency_keys_do_not_repeat() {
        let first = fresh_idempotency_key();
        let second = fresh_idempotency_key();
        assert_ne!(first, second);
        assert_eq!(first.len(), 36);
    }

    #[test]
    fn a_finished_run_is_recognised_by_its_state() {
        let run = |state: &str| BoxRunRecord {
            id: "r".into(),
            box_id: "b".into(),
            command: "true".into(),
            state: state.into(),
            exit_status: None,
            timed_out: None,
            output_offset: None,
            output_base_offset: None,
            failure_reason: None,
            admitted_at: None,
            dispatched_at: None,
            started_at: None,
            finished_at: None,
            deadline_at: None,
            cancellation_requested_at: None,
            cancellation_effective_at: None,
        };
        // `OpenAgents.Box.Run` declares
        //   @states          admitted dispatched running completed failed
        //                    cancelled timed_out lost
        //   @terminal_states completed failed cancelled timed_out lost
        // Nothing else is a run state, so every state is checked here and the
        // split is asserted both ways. The version of this test that only
        // asserted `succeeded` and `failed` passed while `--follow` could not
        // end a successful run, because `succeeded` is not a state the server
        // has and `completed`, the one it uses, was not in the list.
        for state in ["completed", "failed", "cancelled", "timed_out", "lost"] {
            assert!(run(state).finished(), "{state} is a terminal run state");
        }
        for state in ["admitted", "dispatched", "running"] {
            assert!(
                !run(state).finished(),
                "{state} is a live run state and must keep --follow polling"
            );
        }
        // Spellings the server never sends. Treating one as terminal would end
        // a follow early on a run that was still producing output; the reason
        // they are named is that three of them were once in the list.
        for state in ["succeeded", "canceled", "expired", "queued", ""] {
            assert!(
                !run(state).finished(),
                "{state:?} is not a state OpenAgents.Box.Run can hold"
            );
        }
    }
}
