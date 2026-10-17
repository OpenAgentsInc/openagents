//! Host policy settings and accounting shared with terminal callers.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

/// A host-installed command hook. The host supplies the socket environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookBridge {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub socket: PathBuf,
}

impl HookBridge {
    pub fn command(&self, format: &str) -> String {
        std::iter::once(self.program.display().to_string())
            .chain(self.args.clone())
            .chain(["--format".into(), format.into()])
            .map(|word| shell_word(&word))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn claude_settings(&self) -> String {
        json!({"hooks":{"PreToolUse":[{"matcher":"*","hooks":[{
            "type":"command", "command": self.command("claude"), "timeout":600
        }]}]}})
        .to_string()
    }

    /// Arguments for retained exec-mode callers. Native app-server runs use
    /// `codex_thread_config`, whose trust setting is a request override.
    pub fn codex_args(&self) -> Vec<String> {
        let command = serde_json::to_string(&self.command("codex")).unwrap_or_default();
        vec![
            "-c".into(),
            format!("hooks.PreToolUse=[{{hooks=[{{type=\"command\",command={command}}}]}}]"),
            "--dangerously-bypass-hook-trust".into(),
        ]
    }

    fn codex_thread_config(&self) -> serde_json::Map<String, Value> {
        json!({
            "hooks.PreToolUse": [{"hooks":[{"type":"command","command": self.command("codex")}]}],
            "bypass_hook_trust": true,
        })
        .as_object()
        .unwrap()
        .clone()
    }
}

fn shell_word(text: &str) -> String {
    if !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_'))
    {
        text.into()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

/// Settings chosen by the local execution host, outside the wire request.
#[derive(Clone, Default)]
pub struct NativeOptions {
    pub session_id: Option<String>,
    /// Native history directory selected by the host for resume recovery.
    pub claude_config_root: Option<PathBuf>,
    pub tools: Option<String>,
    pub effort: Option<String>,
    pub system_prompt_file: Option<PathBuf>,
    pub append_system_prompt_file: Option<PathBuf>,
    /// Codex's append mode uses developer instructions rather than a file.
    pub append_instructions: Option<String>,
    /// Explicit operator-provided Codex settings, in its `key=value` syntax.
    pub codex_settings: Vec<String>,
    pub hooks: Option<HookBridge>,
}

impl NativeOptions {
    pub(crate) fn claude_command(&self, command: &mut crate::process::Command) {
        for (flag, value) in [
            ("--session-id", &self.session_id),
            ("--tools", &self.tools),
            ("--effort", &self.effort),
        ] {
            if let Some(value) = value {
                command.args([flag, value]);
            }
        }
        if let Some(path) = &self.system_prompt_file {
            command.arg("--system-prompt-file").arg(path);
        }
        if let Some(path) = &self.append_system_prompt_file {
            command.arg("--append-system-prompt-file").arg(path);
        }
        if let Some(hooks) = &self.hooks {
            command.args(["--settings", &hooks.claude_settings()]);
        }
    }

    pub(crate) fn codex_command(&self, command: &mut crate::process::Command) {
        for setting in &self.codex_settings {
            command.args(["-c", setting]);
        }
        if let Some(effort) = &self.effort {
            command.args(["-c", &format!("model_reasoning_effort={effort}")]);
        }
        if let Some(path) = &self.system_prompt_file {
            command.args(["-c", &format!("model_instructions_file={}", json!(path))]);
        }
    }

    pub(crate) fn codex_thread_config(&self) -> serde_json::Map<String, Value> {
        self.hooks
            .as_ref()
            .map(HookBridge::codex_thread_config)
            .unwrap_or_default()
    }
}

/// Provider-reported accounting. Missing figures stay unknown.
#[derive(Clone, Debug, Default)]
pub struct Accounting {
    pub failure: Option<Failure>,
    pub model: Option<String>,
    pub version: Option<String>,
    pub api_key_source: Option<String>,
    pub usage: Option<Value>,
    pub model_usage: Option<Value>,
    pub total_cost_usd: Option<f64>,
    pub num_turns: Option<u64>,
    pub duration_ms: Option<u64>,
    pub duration_api_ms: Option<u64>,
    pub input_per_call: Vec<u64>,
    pub api_calls: Option<u64>,
    pub resets_at: Option<u64>,
    pub limit_window: Option<String>,
}

/// Native failure metadata used by the host's refusal and retry policy.
#[derive(Clone, Debug)]
pub struct Failure {
    pub kind: String,
    pub status: Option<u64>,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Default)]
pub(crate) struct AccountingReader {
    pub snapshot: Accounting,
    calls: BTreeMap<String, u64>,
    call_order: Vec<String>,
    opencode_messages: BTreeMap<String, OpenCodeUsage>,
    exits: BTreeMap<String, i64>,
    changes: BTreeMap<String, Vec<(String, String)>>,
    incomplete_changes: bool,
}

#[derive(Default)]
struct OpenCodeUsage {
    input: u64,
    output: u64,
    read: u64,
    write: u64,
    reasoning: u64,
    cost: Option<f64>,
}

impl AccountingReader {
    fn exit(&mut self, id: &Value, code: &Value) {
        if let (Some(id), Some(code)) = (id.as_str(), code.as_i64())
            && id.len() <= 256
            && (self.exits.len() < 4096 || self.exits.contains_key(id))
        {
            self.exits.insert(id.into(), code);
        }
    }

    pub(crate) fn take_exit_code(&mut self, id: &str) -> Option<i64> {
        self.exits.remove(id)
    }

    pub(crate) fn take_changes(&mut self, id: &str) -> Result<Vec<(String, String)>, String> {
        if self.incomplete_changes {
            return Err("The agent's changed files could not all be read.".into());
        }
        Ok(self.changes.remove(id).unwrap_or_default())
    }

    fn file_changes(&mut self, item: &Value) {
        let Some(id) = item["id"].as_str().filter(|id| id.len() <= 256) else {
            self.incomplete_changes = true;
            return;
        };
        if matches!(
            item["status"].as_str(),
            Some("failed" | "errored" | "declined")
        ) {
            self.changes.remove(id);
            return;
        }
        let Some(changes) = item["changes"].as_array() else {
            return;
        };
        if changes.len() > 64 || (self.changes.len() >= 256 && !self.changes.contains_key(id)) {
            self.incomplete_changes = true;
            return;
        }
        let mut files = Vec::with_capacity(changes.len());
        for change in changes {
            let Some(path) = change["path"].as_str().filter(|path| path.len() <= 4096) else {
                self.incomplete_changes = true;
                return;
            };
            let kind = change["kind"]
                .as_str()
                .or_else(|| change["kind"]["type"].as_str())
                .unwrap_or("update");
            files.push((path.into(), kind.chars().take(32).collect()));
        }
        self.changes.insert(id.into(), files);
    }

    pub(crate) fn claude(&mut self, line: &str) {
        let Ok(frame) = serde_json::from_str::<Value>(line) else {
            return;
        };
        match frame["type"].as_str() {
            Some("system") if frame["subtype"] == "init" => {
                self.snapshot.model = text(&frame, "model");
                self.snapshot.version = text(&frame, "claude_code_version");
                self.snapshot.api_key_source = text(&frame, "apiKeySource");
            }
            Some("assistant") => {
                if let Some(id) = frame.pointer("/message/id").and_then(Value::as_str)
                    && let Some(usage) = frame.pointer("/message/usage").filter(|v| v.is_object())
                    && id.len() <= 256
                    && (self.calls.contains_key(id) || self.calls.len() < 4096)
                {
                    if !self.calls.contains_key(id) {
                        self.call_order.push(id.into());
                    }
                    let input = count(usage, "input_tokens")
                        .saturating_add(count(usage, "cache_read_input_tokens"))
                        .saturating_add(count(usage, "cache_creation_input_tokens"));
                    self.calls.insert(id.into(), input);
                    self.snapshot.api_calls = Some(self.calls.len() as u64);
                    self.snapshot.input_per_call =
                        self.call_order.iter().map(|id| self.calls[id]).collect();
                }
            }
            Some("result") => {
                self.snapshot.total_cost_usd = cost(&frame["total_cost_usd"]);
                self.snapshot.num_turns = frame["num_turns"].as_u64();
                self.snapshot.duration_ms = frame["duration_ms"].as_u64();
                self.snapshot.duration_api_ms = frame["duration_api_ms"].as_u64();
                self.snapshot.usage = frame.get("usage").cloned();
                self.snapshot.model_usage = frame.get("modelUsage").cloned();
            }
            Some("rate_limit_event")
                if frame
                    .pointer("/rate_limit_info/status")
                    .and_then(Value::as_str)
                    == Some("rejected") =>
            {
                self.snapshot.resets_at = frame
                    .pointer("/rate_limit_info/resetsAt")
                    .and_then(Value::as_u64);
                self.snapshot.limit_window = frame
                    .pointer("/rate_limit_info/rateLimitType")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            _ => {}
        }
    }

    pub(crate) fn codex(&mut self, line: &str) {
        let Ok(frame) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if matches!(
            frame["method"].as_str(),
            Some("item/started" | "item/completed")
        ) {
            let item = &frame["params"]["item"];
            if matches!(item["type"].as_str(), Some("fileChange" | "file_change")) {
                self.file_changes(item);
            }
        }
        if frame["method"] == "item/completed" {
            self.exit(
                &frame["params"]["item"]["id"],
                &frame["params"]["item"]["exitCode"],
            );
        }
        if frame["method"] != "thread/tokenUsage/updated" {
            return;
        }
        let params = &frame["params"];
        let usage = params
            .get("tokenUsage")
            .or_else(|| params.get("token_usage"));
        let Some(total) = usage.and_then(|v| v.get("total")) else {
            return;
        };
        let Some(input) = number(total, "inputTokens", "input_tokens") else {
            return;
        };
        let Some(output) = number(total, "outputTokens", "output_tokens") else {
            return;
        };
        let cached = number(total, "cachedInputTokens", "cached_input_tokens")
            .unwrap_or_default()
            .min(input);
        self.snapshot.usage = Some(json!({
            "input_tokens": input.saturating_sub(cached),
            "cache_read_input_tokens": cached,
            "cache_creation_input_tokens": 0,
            "output_tokens": output,
            "reasoning_output_tokens": number(total,"reasoningOutputTokens","reasoning_output_tokens").unwrap_or_default(),
        }));
    }

    pub(crate) fn opencode(&mut self, event: &Value, session: &str) {
        let event = event.get("payload").unwrap_or(event);
        if event["type"] == "session.error" {
            let properties = &event["properties"];
            if properties["sessionID"].is_null() || properties["sessionID"] == session {
                let error = &properties["error"];
                let headers = error
                    .pointer("/data/responseHeaders")
                    .and_then(Value::as_object);
                let header = |name: &str| {
                    headers
                        .and_then(|h| h.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)))
                        .and_then(|(_, value)| value.as_str())
                        .and_then(|value| value.trim().parse::<f64>().ok())
                        .filter(|value| value.is_finite() && *value >= 0.)
                };
                self.snapshot.failure = Some(Failure {
                    kind: error["name"]
                        .as_str()
                        .unwrap_or_default()
                        .chars()
                        .take(256)
                        .collect(),
                    status: error.pointer("/data/statusCode").and_then(Value::as_u64),
                    retry_after_seconds: header("retry-after-ms")
                        .map(|v| (v / 1000.).ceil() as u64)
                        .or_else(|| header("retry-after").map(|v| v.ceil() as u64)),
                });
            }
        }
        if event["type"] == "message.part.updated" {
            let part = &event["properties"]["part"];
            if part["sessionID"] == session && part["type"] == "tool" {
                self.exit(
                    part.get("callID").unwrap_or(&part["id"]),
                    &part["state"]["metadata"]["exit"],
                );
            }
        }
        if event["type"] != "message.updated" {
            return;
        }
        let info = &event["properties"]["info"];
        if info["role"] != "assistant" || info["sessionID"] != session {
            return;
        }
        let Some(id) = info["id"].as_str() else {
            return;
        };
        let tokens = &info["tokens"];
        if id.len() > 256 || !tokens["input"].is_u64() || !tokens["output"].is_u64() {
            return;
        }
        if !self.opencode_messages.contains_key(id) && self.opencode_messages.len() >= 4096 {
            return;
        }
        self.opencode_messages.insert(
            id.into(),
            OpenCodeUsage {
                input: count(tokens, "input"),
                output: count(tokens, "output"),
                read: count(&tokens["cache"], "read"),
                write: count(&tokens["cache"], "write"),
                reasoning: count(tokens, "reasoning"),
                cost: cost(&info["cost"]),
            },
        );
        let mut input = 0u64;
        let mut output = 0u64;
        let mut read = 0u64;
        let mut write = 0u64;
        let mut reasoning = 0u64;
        let mut cost = Some(0.);
        self.snapshot.input_per_call.clear();
        for message in self.opencode_messages.values() {
            input = input.saturating_add(message.input);
            output = output.saturating_add(message.output);
            read = read.saturating_add(message.read);
            write = write.saturating_add(message.write);
            reasoning = reasoning.saturating_add(message.reasoning);
            self.snapshot.input_per_call.push(
                message
                    .input
                    .saturating_add(message.read)
                    .saturating_add(message.write),
            );
            cost = cost
                .zip(message.cost)
                .map(|(total, next)| total + next)
                .filter(|v| v.is_finite());
        }
        self.snapshot.api_calls = Some(self.opencode_messages.len() as u64);
        self.snapshot.num_turns = self.snapshot.api_calls;
        self.snapshot.total_cost_usd = cost;
        self.snapshot.usage = Some(
            json!({"input_tokens":input,"output_tokens":output,"cache_read_input_tokens":read,"cache_creation_input_tokens":write,"reasoning_output_tokens":reasoning}),
        );
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}
fn count(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or_default()
}
fn number(value: &Value, camel: &str, snake: &str) -> Option<u64> {
    value
        .get(camel)
        .or_else(|| value.get(snake))
        .and_then(Value::as_u64)
}

fn cost(value: &Value) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite() && *v >= 0.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hook_config_uses_thread_trust_override() {
        let hook = HookBridge {
            program: "/path with space/hook".into(),
            args: vec!["it's safe".into()],
            socket: "/socket".into(),
        };
        let config = hook.codex_thread_config();
        assert!(
            hook.command("codex")
                .starts_with("'/path with space/hook' ")
        );
        assert_eq!(config["bypass_hook_trust"], true);
        assert_eq!(
            config["hooks.PreToolUse"][0]["hooks"][0]["command"],
            hook.command("codex")
        );
    }
    #[test]
    fn claude_updates_calls_without_counting_streamed_repeats() {
        let mut reader = AccountingReader::default();
        for (id, input) in [("z", 10), ("a", 20), ("z", 30)] {
            reader.claude(&json!({"type":"assistant","message":{"id":id,"usage":{"input_tokens":input,"cache_read_input_tokens":5}}}).to_string());
        }
        assert_eq!(reader.snapshot.input_per_call, [35, 25]);
        assert_eq!(reader.snapshot.api_calls, Some(2));
        reader.claude(r#"{"type":"result","total_cost_usd":-1}"#);
        assert_eq!(reader.snapshot.total_cost_usd, None);
    }
    #[test]
    fn codex_missing_counters_stay_unknown_and_cached_input_is_not_double_counted() {
        let mut reader = AccountingReader::default();
        reader.codex(
            r#"{"method":"thread/tokenUsage/updated","params":{"tokenUsage":{"total":{}}}}"#,
        );
        assert!(reader.snapshot.usage.is_none());
        reader.codex(r#"{"method":"thread/tokenUsage/updated","params":{"tokenUsage":{"total":{"inputTokens":100,"cachedInputTokens":40,"outputTokens":8}}}}"#);
        assert_eq!(
            reader.snapshot.usage.unwrap(),
            json!({"input_tokens":60,"cache_read_input_tokens":40,"cache_creation_input_tokens":0,"output_tokens":8,"reasoning_output_tokens":0})
        );
    }
    #[test]
    fn opencode_replaces_message_snapshots_and_excludes_other_sessions() {
        let mut reader = AccountingReader::default();
        let event = |session, id, input, cost| json!({"type":"message.updated","properties":{"info":{"sessionID":session,"id":id,"role":"assistant","tokens":{"input":input,"output":4},"cost":cost}}});
        reader.opencode(&event("ours", "a", 10, Some(0.1)), "ours");
        reader.opencode(&event("ours", "a", 20, Some(0.2)), "ours");
        reader.opencode(&event("other", "b", 500, Some(99.)), "ours");
        assert_eq!(reader.snapshot.total_cost_usd, Some(0.2));
        assert_eq!(reader.snapshot.usage.as_ref().unwrap()["input_tokens"], 20);
        reader.opencode(&event("ours", "c", 30, None), "ours");
        assert_eq!(reader.snapshot.api_calls, Some(2));
        assert_eq!(reader.snapshot.total_cost_usd, None);
    }
    #[test]
    fn native_failure_metadata_preserves_status_and_retry_headers() {
        let mut reader = AccountingReader::default();
        let event = |session| json!({"type":"session.error","properties":{"sessionID":session,"error":{"name":"APIError","data":{"statusCode":429,"responseHeaders":{"Retry-After-Ms":"1501"}}}}});
        reader.opencode(&event("other"), "ours");
        assert!(reader.snapshot.failure.is_none());
        reader.opencode(&event("ours"), "ours");
        let failure = reader.snapshot.failure.unwrap();
        assert_eq!(failure.kind, "APIError");
        assert_eq!(failure.status, Some(429));
        assert_eq!(failure.retry_after_seconds, Some(2));
    }

    #[test]
    fn codex_multi_file_changes_survive_the_display_call_reduction() {
        let mut reader = AccountingReader::default();
        reader.codex(r#"{"method":"item/completed","params":{"item":{"type":"fileChange","id":"patch","status":"completed","changes":[{"path":"a.rs","kind":"add"},{"path":"b.rs","kind":{"type":"update"}}]}}}"#);
        assert_eq!(
            reader.take_changes("patch").unwrap(),
            [
                ("a.rs".into(), "add".into()),
                ("b.rs".into(), "update".into())
            ]
        );
        assert!(reader.take_changes("patch").unwrap().is_empty());
        reader.file_changes(&json!({"id":"large","changes":(0..65).map(|i|json!({"path":i.to_string()})).collect::<Vec<_>>()}));
        assert!(reader.take_changes("large").is_err());
    }

    #[test]
    fn native_exit_codes_are_scoped_bounded_and_taken_once() {
        let mut reader = AccountingReader::default();
        reader.codex(
            r#"{"method":"item/completed","params":{"item":{"id":"codex-tool","exitCode":17}}}"#,
        );
        assert_eq!(reader.take_exit_code("codex-tool"), Some(17));
        assert_eq!(reader.take_exit_code("codex-tool"), None);
        let event = |session| json!({"type":"message.part.updated","properties":{"part":{"sessionID":session,"id":"part","callID":"call","type":"tool","state":{"metadata":{"exit":2}}}}});
        reader.opencode(&event("other"), "ours");
        assert_eq!(reader.take_exit_code("call"), None);
        reader.opencode(&event("ours"), "ours");
        assert_eq!(reader.take_exit_code("call"), Some(2));
        for i in 0..4100 {
            reader.exit(&json!(i.to_string()), &json!(1));
        }
        assert_eq!(reader.exits.len(), 4096);
    }

    #[test]
    fn accounting_bounds_provider_identifiers_and_call_count() {
        let mut reader = AccountingReader::default();
        for i in 0..4100 {
            reader.claude(
                &json!({"type":"assistant","message":{"id":i.to_string(),"usage":{}}}).to_string(),
            );
        }
        reader.claude(
            &json!({"type":"assistant","message":{"id":"x".repeat(257),"usage":{}}}).to_string(),
        );
        assert_eq!(reader.calls.len(), 4096);
        assert_eq!(reader.call_order.len(), 4096);
    }
}
