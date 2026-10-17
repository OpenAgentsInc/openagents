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

#[derive(Default)]
pub(crate) struct AccountingReader {
    pub snapshot: Accounting,
    calls: BTreeMap<String, u64>,
    call_order: Vec<String>,
    opencode_messages: BTreeMap<String, OpenCodeUsage>,
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
