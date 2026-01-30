use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tracing::warn;

#[derive(Clone, Debug)]
enum ProgressMode {
    Off,
    Http,
}

pub struct OpenClawProgressBridge {
    mode: ProgressMode,
    endpoint: String,
    api_token: Option<String>,
    session_id: Option<String>,
    tool_name: String,
    debounce_ms: u64,
    include_metadata: bool,
    run_id: String,
    client: reqwest::Client,
    last_status_sent: Mutex<Option<Instant>>,
    warned_missing_session: AtomicBool,
}

impl OpenClawProgressBridge {
    pub fn from_env() -> Option<Self> {
        let mode = read_env("OPENCLAW_PROGRESS_MODE").unwrap_or_else(|| "off".to_string());
        let mode = match mode.trim().to_lowercase().as_str() {
            "off" | "false" | "0" | "disabled" => ProgressMode::Off,
            "http" => ProgressMode::Http,
            other => {
                warn!(mode = %other, "unknown OPENCLAW_PROGRESS_MODE, defaulting to off");
                ProgressMode::Off
            }
        };

        if matches!(mode, ProgressMode::Off) {
            return None;
        }

        let endpoint = match resolve_endpoint() {
            Some(endpoint) => endpoint,
            None => {
                warn!("OPENCLAW_PROGRESS_MODE enabled but no endpoint configured");
                return None;
            }
        };

        let api_token = read_env("OPENCLAW_API_TOKEN");
        let session_id = read_env("OPENCLAW_SESSION_ID");
        let tool_name = read_env("OPENCLAW_PROGRESS_TOOL").unwrap_or_else(|| "sessions_send".to_string());
        let debounce_ms = read_env("OPENCLAW_PROGRESS_DEBOUNCE_MS")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1200);
        let include_metadata = read_env("OPENCLAW_PROGRESS_INCLUDE_METADATA")
            .map(|value| matches!(value.as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                warn!(error = %err, "failed to build OpenClaw HTTP client");
                return None;
            }
        };

        Some(Self {
            mode,
            endpoint,
            api_token,
            session_id,
            tool_name,
            debounce_ms,
            include_metadata,
            run_id: format!("autopilot-{}", uuid::Uuid::new_v4()),
            client,
            last_status_sent: Mutex::new(None),
            warned_missing_session: AtomicBool::new(false),
        })
    }

    pub fn emit_guidance_status(&self, thread_id: &str, signature: Option<&str>, text: &str) {
        let Some(payload) = self.build_payload(
            "guidance/status",
            thread_id,
            signature,
            text,
            None,
        ) else {
            return;
        };

        if !self.should_send_status() {
            return;
        }

        self.send_payload(payload);
    }

    pub fn emit_guidance_step(&self, thread_id: &str, signature: &str, text: &str, model: &str) {
        let Some(payload) = self.build_payload(
            "guidance/step",
            thread_id,
            Some(signature),
            text,
            Some(model),
        ) else {
            return;
        };

        self.send_payload(payload);
    }

    fn build_payload(
        &self,
        method: &str,
        thread_id: &str,
        signature: Option<&str>,
        text: &str,
        model: Option<&str>,
    ) -> Option<Value> {
        let session_id = match self.session_id.as_deref() {
            Some(value) if !value.trim().is_empty() => value.trim().to_string(),
            _ => {
                self.warn_missing_session();
                return None;
            }
        };

        let message = truncate_text(format_progress_message(method, signature, text), 400);
        let mut params = json!({
            "session_id": session_id,
            "message": message,
        });

        if self.include_metadata {
            let metadata = json!({
                "event": "autopilot.progress",
                "run_id": self.run_id,
                "thread_id": thread_id,
                "phase": method,
                "signature": signature,
                "model": model,
            });
            if let Value::Object(map) = &mut params {
                map.insert("metadata".to_string(), metadata);
            }
        }

        Some(json!({
            "tool": self.tool_name,
            "params": params,
        }))
    }

    fn should_send_status(&self) -> bool {
        if self.debounce_ms == 0 {
            return true;
        }
        let now = Instant::now();
        let mut last = self
            .last_status_sent
            .lock()
            .expect("openclaw status debounce lock");
        if let Some(previous) = *last {
            if now.duration_since(previous) < Duration::from_millis(self.debounce_ms) {
                return false;
            }
        }
        *last = Some(now);
        true
    }

    fn send_payload(&self, payload: Value) {
        if !matches!(self.mode, ProgressMode::Http) {
            return;
        }
        let client = self.client.clone();
        let endpoint = self.endpoint.clone();
        let api_token = self.api_token.clone();
        tokio::spawn(async move {
            let mut request = client.post(endpoint).json(&payload);
            if let Some(token) = api_token {
                request = request.bearer_auth(token);
            }
            match request.send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        warn!(status = %response.status(), "openclaw progress request failed");
                    }
                }
                Err(err) => {
                    warn!(error = %err, "openclaw progress request error");
                }
            }
        });
    }

    fn warn_missing_session(&self) {
        if !self
            .warned_missing_session
            .swap(true, Ordering::Relaxed)
        {
            warn!("OPENCLAW_SESSION_ID missing; skipping progress forwarding");
        }
    }
}

fn read_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_endpoint() -> Option<String> {
    if let Some(url) = read_env("OPENCLAW_PROGRESS_URL") {
        return Some(url);
    }
    let gateway = read_env("OPENCLAW_GATEWAY_URL")?;
    Some(join_url(&gateway, "tools/invoke"))
}

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{base}/{path}")
}

fn format_progress_message(method: &str, signature: Option<&str>, text: &str) -> String {
    let clean = text.trim();
    let label = match method {
        "guidance/status" => "Status",
        "guidance/step" => "Step",
        _ => "Update",
    };
    if let Some(sig) = signature {
        format!("{label} [{sig}]: {clean}")
    } else {
        format!("{label}: {clean}")
    }
}

fn truncate_text(mut text: String, max_len: usize) -> String {
    if text.len() <= max_len {
        return text;
    }
    text.truncate(max_len);
    text.push_str("...");
    text
}
