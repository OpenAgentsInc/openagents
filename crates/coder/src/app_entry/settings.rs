use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use claude_agent_sdk::protocol::McpServerStatus;
use serde_json::Value;
use wgpui::input::Modifiers as UiModifiers;

use crate::app::config::{
    config_dir, config_file, keybindings_file, CoderSettings, StoredKeybinding, StoredModifiers,
};
use crate::app::events::{key_from_string, key_to_string};
use crate::app::session::{RateLimitInfo, RateLimits};
use crate::app::ModelOption;
use crate::keybindings::{default_keybindings, Action as KeyAction, Keybinding};

pub(super) fn clamp_font_size(size: f32) -> f32 {
    size.clamp(12.0, 18.0)
}

pub(super) fn normalize_settings(settings: &mut CoderSettings) {
    settings.font_size = clamp_font_size(settings.font_size);
}

/// Format a reset timestamp as relative time (e.g., "3d", "5h", "30m")
fn format_reset_time(timestamp: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = timestamp - now;

    if diff <= 0 {
        return "soon".to_string();
    }
    if diff < 3600 {
        return format!("{}m", diff / 60);
    }
    if diff < 86400 {
        return format!("{}h", diff / 3600);
    }
    format!("{}d", diff / 86400)
}

/// Parse rate limit headers from Anthropic API response
/// Supports multiple header formats:
/// - anthropic-ratelimit-unified-* (Claude Code format)
/// - x-ratelimit-* (standard Anthropic API)
fn parse_rate_limit_headers(headers: &reqwest::header::HeaderMap) -> Option<RateLimits> {
    let mut limits = RateLimits::default();

    // Helper to get header as string
    let get_header = |name: &str| -> Option<&str> {
        headers.get(name)?.to_str().ok()
    };

    // Try unified format first: anthropic-ratelimit-unified-7d-utilization (0-1 range)
    let unified_claims = [
        ("7d", "weekly"),
        ("7ds", "sonnet"),
        ("7do", "opus"),
        ("5h", "session"),
    ];

    for (claim, name) in unified_claims {
        let util_header = format!("anthropic-ratelimit-unified-{}-utilization", claim);
        let reset_header = format!("anthropic-ratelimit-unified-{}-reset", claim);

        if let Some(util_str) = get_header(&util_header) {
            if let Ok(util_val) = util_str.parse::<f64>() {
                let reset = get_header(&reset_header)
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(format_reset_time)
                    .unwrap_or_default();

                let info = RateLimitInfo {
                    name: name.to_string(),
                    percent_used: util_val * 100.0,
                    resets_at: reset,
                };

                if limits.primary.is_none() {
                    limits.primary = Some(info);
                } else if limits.secondary.is_none() {
                    limits.secondary = Some(info);
                    break;
                }
            }
        }
    }

    // Try standard x-ratelimit headers (public Anthropic API)
    if limits.primary.is_none() {
        if let (Some(limit_str), Some(remaining_str)) = (
            get_header("x-ratelimit-limit-requests"),
            get_header("x-ratelimit-remaining-requests"),
        ) {
            if let (Ok(limit), Ok(remaining)) = (
                limit_str.parse::<i64>(),
                remaining_str.parse::<i64>(),
            ) {
                if limit > 0 {
                    let used = limit - remaining;
                    let percent = (used as f64 / limit as f64) * 100.0;

                    // Parse reset time
                    let reset = get_header("x-ratelimit-reset-requests")
                        .map(|s| {
                            // Format might be "60s" or ISO timestamp
                            if s.ends_with('s') {
                                s.trim_end_matches('s')
                                    .parse::<i64>()
                                    .ok()
                                    .map(|secs| {
                                        let now = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .map(|d| d.as_secs() as i64)
                                            .unwrap_or(0);
                                        format_reset_time(now + secs)
                                    })
                                    .unwrap_or_default()
                            } else {
                                s.to_string()
                            }
                        })
                        .unwrap_or_default();

                    limits.primary = Some(RateLimitInfo {
                        name: "requests".to_string(),
                        percent_used: percent,
                        resets_at: reset,
                    });
                }
            }
        }

        // Also try token limits
        if let (Some(limit_str), Some(remaining_str)) = (
            get_header("x-ratelimit-limit-tokens"),
            get_header("x-ratelimit-remaining-tokens"),
        ) {
            if let (Ok(limit), Ok(remaining)) = (
                limit_str.parse::<i64>(),
                remaining_str.parse::<i64>(),
            ) {
                if limit > 0 {
                    let used = limit - remaining;
                    let percent = (used as f64 / limit as f64) * 100.0;

                    let reset = get_header("x-ratelimit-reset-tokens")
                        .map(|s| {
                            if s.ends_with('s') {
                                s.trim_end_matches('s')
                                    .parse::<i64>()
                                    .ok()
                                    .map(|secs| {
                                        let now = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .map(|d| d.as_secs() as i64)
                                            .unwrap_or(0);
                                        format_reset_time(now + secs)
                                    })
                                    .unwrap_or_default()
                            } else {
                                s.to_string()
                            }
                        })
                        .unwrap_or_default();

                    let info = RateLimitInfo {
                        name: "tokens".to_string(),
                        percent_used: percent,
                        resets_at: reset,
                    };

                    if limits.primary.is_none() {
                        limits.primary = Some(info);
                    } else if limits.secondary.is_none() {
                        limits.secondary = Some(info);
                    }
                }
            }
        }
    }

    if limits.primary.is_some() || limits.secondary.is_some() {
        Some(limits)
    } else {
        None
    }
}

/// Load OAuth access token from Claude credentials
fn load_claude_oauth_token() -> Option<String> {
    // Try Linux keyring via secret-tool first
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("secret-tool")
            .args(["lookup", "service", "Claude Code-credentials"])
            .output()
        {
            if output.status.success() {
                if let Ok(json_str) = String::from_utf8(output.stdout) {
                    if let Ok(json) = serde_json::from_str::<Value>(&json_str) {
                        if let Some(token) = json
                            .get("claudeAiOauth")
                            .and_then(|o| o.get("accessToken"))
                            .and_then(|v| v.as_str())
                        {
                            tracing::info!("Loaded OAuth token from Linux keyring");
                            return Some(token.to_string());
                        }
                    }
                }
            }
        }
    }

    // Try macOS keychain
    #[cfg(target_os = "macos")]
    {
        let username = std::env::var("USER").ok()?;
        if let Ok(output) = std::process::Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                "Claude Code-credentials",
                "-a",
                &username,
                "-w",
            ])
            .output()
        {
            if output.status.success() {
                if let Ok(json_str) = String::from_utf8(output.stdout) {
                    if let Ok(json) = serde_json::from_str::<Value>(&json_str.trim()) {
                        if let Some(token) = json
                            .get("claudeAiOauth")
                            .and_then(|o| o.get("accessToken"))
                            .and_then(|v| v.as_str())
                        {
                            tracing::info!("Loaded OAuth token from macOS keychain");
                            return Some(token.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fall back to file-based credentials
    let home = std::env::var("HOME").ok()?;
    let paths = [
        format!("{}/.claude/.credentials.json", home),
        format!("{}/.claude/.credentials", home),
    ];

    for path in &paths {
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(json) = serde_json::from_str::<Value>(&contents) {
                if let Some(token) = json
                    .get("claudeAiOauth")
                    .and_then(|o| o.get("accessToken"))
                    .and_then(|v| v.as_str())
                {
                    tracing::info!("Loaded OAuth token from {}", path);
                    return Some(token.to_string());
                }
            }
        }
    }

    tracing::warn!("No Claude OAuth credentials found");
    None
}

/// Fetch rate limits by making a minimal API call using OAuth
pub(super) async fn fetch_rate_limits() -> Option<RateLimits> {
    // Try OAuth first, fall back to API key
    let (auth_header, auth_value) = if let Some(token) = load_claude_oauth_token() {
        ("authorization", format!("Bearer {}", token))
    } else if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
        ("x-api-key", api_key)
    } else {
        tracing::warn!("No OAuth token or API key available for rate limit fetch");
        return None;
    };

    tracing::info!("Fetching rate limits...");

    let client = reqwest::Client::new();
    let mut request = client
        .post("https://api.anthropic.com/v1/messages")
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .header(auth_header, &auth_value);

    // Add OAuth beta header if using OAuth
    if auth_header == "authorization" {
        request = request.header("anthropic-beta", "oauth-2025-04-20");
    }

    let response = match request
        .body(r#"{"model":"claude-3-haiku-20240307","max_tokens":1,"messages":[{"role":"user","content":"x"}]}"#)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Rate limit fetch failed: {}", e);
            return None;
        }
    };

    // Log response headers for debugging
    for (name, value) in response.headers() {
        if name.as_str().contains("ratelimit") || name.as_str().contains("limit") {
            tracing::info!("Rate limit header: {} = {:?}", name, value);
        }
    }

    let limits = parse_rate_limit_headers(response.headers());
    if let Some(ref l) = limits {
        if let Some(ref p) = l.primary {
            tracing::info!("Rate limit: {} {:.1}% used, resets {}", p.name, p.percent_used, p.resets_at);
        }
    } else {
        tracing::warn!("No rate limit data found in response headers");
    }
    limits
}
fn parse_legacy_model_setting(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(model_id) = line.strip_prefix("model = \"").and_then(|s| s.strip_suffix("\"")) {
            return Some(model_id.to_string());
        }
    }
    None
}

pub(super) fn load_settings() -> CoderSettings {
    let path = config_file();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(mut settings) = toml::from_str::<CoderSettings>(&content) {
            normalize_settings(&mut settings);
            return settings;
        }
        let mut settings = CoderSettings::default();
        settings.model = parse_legacy_model_setting(&content);
        normalize_settings(&mut settings);
        return settings;
    }
    CoderSettings::default()
}

pub(super) fn save_settings(settings: &CoderSettings) {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_ok() {
        if let Ok(content) = toml::to_string_pretty(settings) {
            let _ = fs::write(config_file(), content);
        }
    }
}

/// Auto-start llama-server if not already running.
///
/// Returns the child process handle if started, None if already running or unable to start.
pub(super) fn auto_start_llama_server() -> Option<Child> {
    // Check if already running on port 8000 or 8080
    if adjutant::dspy::lm_config::check_llamacpp_available() {
        tracing::info!("llama-server already running, skipping auto-start");
        return None;
    }

    // Find llama-server binary
    let binary = find_llama_server_binary()?;
    tracing::info!("Found llama-server at: {}", binary.display());

    // Find a usable model
    let model = find_gguf_model()?;
    tracing::info!("Found GGUF model at: {}", model.display());

    // Start llama-server on port 8000
    let port = 8000;
    tracing::info!("Starting llama-server on port {}...", port);

    match ProcessCommand::new(&binary)
        .arg("-m")
        .arg(&model)
        .arg("--port")
        .arg(port.to_string())
        .arg("--ctx-size")
        .arg("8192")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            tracing::info!("llama-server started with PID {}", child.id());
            // Give it a moment to bind the port
            std::thread::sleep(std::time::Duration::from_millis(500));
            Some(child)
        }
        Err(e) => {
            tracing::warn!("Failed to start llama-server: {}", e);
            None
        }
    }
}

/// Find llama-server binary in common locations.
fn find_llama_server_binary() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    // Check common locations
    let candidates = [
        home.join("code/llama.cpp/build/bin/llama-server"),
        home.join("code/llama.cpp/llama-server"),
        home.join("llama.cpp/build/bin/llama-server"),
        home.join("llama.cpp/llama-server"),
        home.join(".local/bin/llama-server"),
        PathBuf::from("/usr/local/bin/llama-server"),
        PathBuf::from("/usr/bin/llama-server"),
    ];

    for path in &candidates {
        if path.exists() && path.is_file() {
            return Some(path.clone());
        }
    }

    // Try which command
    if let Ok(output) = ProcessCommand::new("which")
        .arg("llama-server")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    None
}

/// Find a usable GGUF model file.
fn find_gguf_model() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    // Check llama.cpp cache first (where downloaded models go)
    let cache_dir = home.join(".cache/llama.cpp");
    if cache_dir.exists() {
        if let Ok(entries) = fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "gguf") {
                    // Skip vocab-only files (usually small)
                    if let Ok(meta) = fs::metadata(&path) {
                        // Real models are at least 100MB
                        if meta.len() > 100_000_000 {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    // Check models directory
    let models_dir = home.join("code/llama.cpp/models");
    if models_dir.exists() {
        if let Ok(entries) = fs::read_dir(&models_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "gguf") {
                    if let Ok(meta) = fs::metadata(&path) {
                        if meta.len() > 100_000_000 {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    None
}

pub(super) fn settings_model_option(settings: &CoderSettings) -> ModelOption {
    settings
        .model
        .as_deref()
        .map(ModelOption::from_id)
        .unwrap_or(ModelOption::Opus)
}

pub(super) fn update_settings_model(settings: &mut CoderSettings, model: ModelOption) {
    settings.model = Some(model.model_id().to_string());
}

pub(super) fn load_keybindings() -> Vec<Keybinding> {
    let path = keybindings_file();
    let Ok(content) = fs::read_to_string(&path) else {
        return default_keybindings();
    };
    let Ok(entries) = serde_json::from_str::<Vec<StoredKeybinding>>(&content) else {
        return default_keybindings();
    };
    let mut bindings = Vec::new();
    for entry in entries {
        let Some(action) = KeyAction::from_id(&entry.action) else {
            continue;
        };
        let Some(key) = key_from_string(&entry.key) else {
            continue;
        };
        let modifiers = UiModifiers {
            shift: entry.modifiers.shift,
            ctrl: entry.modifiers.ctrl,
            alt: entry.modifiers.alt,
            meta: entry.modifiers.meta,
        };
        bindings.push(Keybinding {
            key,
            modifiers,
            action,
        });
    }
    if bindings.is_empty() {
        default_keybindings()
    } else {
        bindings
    }
}

pub(super) fn save_keybindings(bindings: &[Keybinding]) {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_ok() {
        let entries: Vec<StoredKeybinding> = bindings
            .iter()
            .map(|binding| StoredKeybinding {
                action: binding.action.id().to_string(),
                key: key_to_string(&binding.key),
                modifiers: StoredModifiers {
                    shift: binding.modifiers.shift,
                    ctrl: binding.modifiers.ctrl,
                    alt: binding.modifiers.alt,
                    meta: binding.modifiers.meta,
                },
            })
            .collect();
        if let Ok(content) = serde_json::to_string_pretty(&entries) {
            let _ = fs::write(keybindings_file(), content);
        }
    }
}

pub(super) fn parse_mcp_status(value: &Value) -> Result<Vec<McpServerStatus>, String> {
    if let Some(servers_value) = value
        .get("mcp_servers")
        .or_else(|| value.get("servers"))
    {
        serde_json::from_value(servers_value.clone())
            .map_err(|err| format!("Failed to parse MCP status: {}", err))
    } else if value.is_array() {
        serde_json::from_value(value.clone())
            .map_err(|err| format!("Failed to parse MCP status: {}", err))
    } else {
        Err("Unexpected MCP status response".to_string())
    }
}
