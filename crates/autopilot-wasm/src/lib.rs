//! WASM bindings for Autopilot replay viewer
//!
//! Provides replay bundle parsing and secret redaction for the web-based viewer.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Log entry from JSONL session log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub session_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// A publishable replay bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBundle {
    pub version: String,
    pub id: String,
    pub created_at: String,
    pub metadata: ReplayMetadata,
    pub timeline: Vec<TimelineEvent>,
    pub receipts: ReplayReceipts,
}

/// Metadata about the autopilot run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayMetadata {
    pub issue_url: Option<String>,
    pub pr_url: Option<String>,
    pub duration_seconds: u64,
    pub playback_speed: f32,
    pub demo_duration_seconds: u64,
    pub model: String,
    pub cost_usd: Option<f64>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
}

/// An event in the timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    /// Timestamp in milliseconds from start
    pub t: u64,
    /// Event type
    #[serde(rename = "type")]
    pub event_type: String,
    /// Optional tool name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Event data
    pub data: serde_json::Value,
}

/// Results and verification info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayReceipts {
    pub tests_run: Option<usize>,
    pub tests_passed: Option<usize>,
    pub ci_status: Option<String>,
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

/// Parse JSONL content into a ReplayBundle
///
/// Takes the raw JSONL string content and returns a JavaScript-compatible object.
#[wasm_bindgen]
pub fn parse_jsonl(content: &str) -> Result<JsValue, JsValue> {
    let entries = parse_entries(content)?;
    let bundle = create_bundle(entries)?;

    serde_wasm_bindgen::to_value(&bundle)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}

/// Parse a JSON replay bundle
///
/// Takes a JSON string and returns a JavaScript-compatible object.
#[wasm_bindgen]
pub fn parse_bundle(json: &str) -> Result<JsValue, JsValue> {
    let bundle: ReplayBundle = serde_json::from_str(json)
        .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

    serde_wasm_bindgen::to_value(&bundle)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}

/// Redact secrets from a replay bundle JSON string
///
/// Returns the redacted JSON string.
#[wasm_bindgen]
pub fn redact_bundle(json: &str) -> Result<String, JsValue> {
    let mut bundle: ReplayBundle = serde_json::from_str(json)
        .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

    for event in &mut bundle.timeline {
        redact_value(&mut event.data);
    }

    serde_json::to_string_pretty(&bundle)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}

/// Get timeline duration in milliseconds
#[wasm_bindgen]
pub fn get_duration(json: &str) -> Result<u64, JsValue> {
    let bundle: ReplayBundle = serde_json::from_str(json)
        .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

    Ok(bundle.timeline.last().map(|e| e.t).unwrap_or(0))
}

/// Get events up to a specific timestamp
#[wasm_bindgen]
pub fn get_events_until(json: &str, timestamp_ms: u64) -> Result<JsValue, JsValue> {
    let bundle: ReplayBundle = serde_json::from_str(json)
        .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

    let events: Vec<_> = bundle
        .timeline
        .into_iter()
        .filter(|e| e.t <= timestamp_ms)
        .collect();

    serde_wasm_bindgen::to_value(&events)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}

/// Get the current event at a specific timestamp
#[wasm_bindgen]
pub fn get_current_event(json: &str, timestamp_ms: u64) -> Result<JsValue, JsValue> {
    let bundle: ReplayBundle = serde_json::from_str(json)
        .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

    let event = bundle
        .timeline
        .into_iter()
        .filter(|e| e.t <= timestamp_ms)
        .last();

    serde_wasm_bindgen::to_value(&event)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}

fn parse_entries(content: &str) -> Result<Vec<LogEntry>, JsValue> {
    let mut entries = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let entry: LogEntry = serde_json::from_str(line)
            .map_err(|e| JsValue::from_str(&format!("Parse error on line: {}", e)))?;
        entries.push(entry);
    }

    if entries.is_empty() {
        return Err(JsValue::from_str("Session log is empty"));
    }

    Ok(entries)
}

fn create_bundle(entries: Vec<LogEntry>) -> Result<ReplayBundle, JsValue> {
    let session_id = entries
        .first()
        .map(|e| e.session_id.clone())
        .ok_or_else(|| JsValue::from_str("No entries"))?;

    let start_time = chrono::DateTime::parse_from_rfc3339(&entries.first().unwrap().timestamp)
        .map_err(|e| JsValue::from_str(&format!("Invalid timestamp: {}", e)))?;

    let end_time = chrono::DateTime::parse_from_rfc3339(&entries.last().unwrap().timestamp)
        .map_err(|e| JsValue::from_str(&format!("Invalid timestamp: {}", e)))?;

    let duration_seconds = (end_time - start_time).num_seconds() as u64;

    let mut timeline = Vec::new();

    for entry in &entries {
        let timestamp = chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
            .map_err(|e| JsValue::from_str(&format!("Invalid timestamp: {}", e)))?;

        let t_ms = (timestamp - start_time).num_milliseconds() as u64;

        let event = match entry.event_type.as_str() {
            "tool_use" => {
                let tool = entry
                    .data
                    .get("tool")
                    .and_then(|t| t.as_str())
                    .map(String::from);

                TimelineEvent {
                    t: t_ms,
                    event_type: "tool_call".to_string(),
                    tool,
                    data: entry.data.clone(),
                }
            }
            "tool_result" => {
                let tool = entry
                    .data
                    .get("tool")
                    .and_then(|t| t.as_str())
                    .map(String::from);

                TimelineEvent {
                    t: t_ms,
                    event_type: "tool_result".to_string(),
                    tool,
                    data: entry.data.clone(),
                }
            }
            "assistant" => TimelineEvent {
                t: t_ms,
                event_type: "assistant".to_string(),
                tool: None,
                data: entry.data.clone(),
            },
            "phase_start" => TimelineEvent {
                t: t_ms,
                event_type: "phase_start".to_string(),
                tool: None,
                data: entry.data.clone(),
            },
            "phase_end" => TimelineEvent {
                t: t_ms,
                event_type: "phase_end".to_string(),
                tool: None,
                data: entry.data.clone(),
            },
            _ => continue,
        };

        timeline.push(event);
    }

    let model = entries
        .iter()
        .find_map(|e| {
            if e.event_type == "phase_start" && e.phase == "planning" {
                e.data
                    .get("model")
                    .and_then(|m| m.as_str())
                    .map(String::from)
            } else {
                None
            }
        })
        .unwrap_or_else(|| "claude-sonnet-4-5-20250929".to_string());

    let playback_speed = 2.0;
    let demo_duration_seconds = (duration_seconds as f32 / playback_speed) as u64;

    let receipts = extract_receipts(&entries);

    Ok(ReplayBundle {
        version: "1.0".to_string(),
        id: format!("replay_{}", session_id),
        created_at: start_time.to_rfc3339(),
        metadata: ReplayMetadata {
            issue_url: None,
            pr_url: None,
            duration_seconds,
            playback_speed,
            demo_duration_seconds,
            model,
            cost_usd: None,
            tokens_in: None,
            tokens_out: None,
        },
        timeline,
        receipts,
    })
}

fn extract_receipts(entries: &[LogEntry]) -> ReplayReceipts {
    let mut tests_run = None;
    let mut tests_passed = None;
    let mut ci_status = None;
    let mut files_changed = 0;

    for entry in entries {
        if entry.event_type == "result" && entry.phase == "verification" {
            if let Some(checks) = entry.data.get("checks").and_then(|c| c.as_object()) {
                if let Some(tests) = checks.get("tests_passing") {
                    tests_run = tests
                        .get("total")
                        .and_then(|t| t.as_u64())
                        .map(|n| n as usize);
                    tests_passed = tests
                        .get("passed")
                        .and_then(|t| t.as_u64())
                        .map(|n| n as usize);
                }

                if let Some(ci) = checks.get("ci_status") {
                    ci_status = ci.get("status").and_then(|s| s.as_str()).map(String::from);
                }
            }
        }

        if entry.event_type == "tool_result" {
            if let Some(tool) = entry.data.get("tool").and_then(|t| t.as_str()) {
                if tool == "Edit" || tool == "Write" {
                    files_changed += 1;
                }
            }
        }
    }

    ReplayReceipts {
        tests_run,
        tests_passed,
        ci_status,
        files_changed,
        lines_added: 0,
        lines_removed: 0,
    }
}

fn redact_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => {
            *s = redact_string(s);
        }
        serde_json::Value::Object(obj) => {
            for (key, val) in obj.iter_mut() {
                if is_secret_field(key) {
                    *val = serde_json::Value::String("[REDACTED]".to_string());
                } else {
                    redact_value(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_value(item);
            }
        }
        _ => {}
    }
}

fn is_secret_field(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("token")
        || lower.contains("key")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("auth")
}

fn redact_string(s: &str) -> String {
    // API keys (OpenAI style)
    let re_api = regex::Regex::new(r"sk-[a-zA-Z0-9]{48}").unwrap();
    let s = re_api.replace_all(s, "sk-[REDACTED]");

    // GitHub tokens
    let re_gh = regex::Regex::new(r"gh[ps]_[a-zA-Z0-9]{36}").unwrap();
    let s = re_gh.replace_all(&s, "gh_[REDACTED]");

    // Anthropic keys
    let re_anthropic = regex::Regex::new(r"sk-ant-[a-zA-Z0-9-]{95}").unwrap();
    let s = re_anthropic.replace_all(&s, "sk-ant-[REDACTED]");

    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_api_key() {
        let input = "Using API key sk-abc123def456ghi789jkl012mno345pqr678stu901vwx234";
        let output = redact_string(input);
        assert!(output.contains("sk-[REDACTED]"));
    }

    #[test]
    fn test_is_secret_field() {
        assert!(is_secret_field("api_key"));
        assert!(is_secret_field("github_token"));
        assert!(!is_secret_field("username"));
    }
}
