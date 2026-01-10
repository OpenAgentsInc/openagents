//! Main application state and event handling.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as TokioCommand;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use async_trait::async_trait;
use arboard::Clipboard;
use web_time::Instant;
use wgpui::input::{Key as UiKey, Modifiers as UiModifiers, NamedKey as UiNamedKey};
use wgpui::components::{Component, EventContext, EventResult};
use wgpui::components::hud::{Command as PaletteCommand, CommandPalette};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, InputEvent, Point, TextSystem,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use claude_agent_sdk::error::Result as SdkResult;
use claude_agent_sdk::permissions::{CallbackPermissionHandler, PermissionRequest};
use claude_agent_sdk::protocol::{PermissionMode, PermissionResult};
use claude_agent_sdk::{
    query_with_permissions, AgentDefinition, AgentModel, BaseHookInput, HookCallback, HookDecision,
    HookEvent, HookInput, HookOutput, HookSpecificOutput, QueryOptions, SdkMessage, SettingSource,
    SyncHookOutput, UserPromptSubmitSpecificOutput, SessionStartSpecificOutput,
    PostToolUseSpecificOutput,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use claude_agent_sdk::protocol::McpServerStatus;

// Autopilot/Adjutant
use crate::autopilot_loop::DspyStage;
use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::molecules::{CheckpointRestore, SessionAction};
use wgpui::components::organisms::{
    ChildTool, DiffLine, DiffLineKind, DiffToolCall, EventData, EventInspector, InspectorView, PermissionDialog,
    SearchMatch, SearchToolCall, TagData, TerminalToolCall, ToolCallCard,
};

use crate::commands::{parse_command, Command};
use crate::keybindings::{default_keybindings, match_action, Action as KeyAction, Keybinding};
use crate::panels::PanelLayout;

use crate::app::autopilot::AutopilotState;
use crate::app::catalog::CatalogState;
use crate::app::chat::ChatState;
use crate::app::config::SettingsState;
use crate::app::permissions::{
    coder_mode_default_allow, coder_mode_label, extract_bash_command, is_read_only_tool,
    load_permission_config, parse_coder_mode, pattern_matches, PermissionPending, PermissionState,
};
use crate::app::session::SessionState;
use crate::app::tools::ToolsState;
use crate::app::chat::{ChatMessage, ChatSelection, ChatSelectionPoint, MessageRole};
use crate::app::catalog::{
    build_hook_map, expand_env_vars_in_value, load_agent_entries,
    load_hook_config, load_hook_scripts, load_mcp_project_servers, load_skill_entries,
    parse_mcp_server_config, save_hook_config, AgentEntry, AgentSource, HookConfig,
    HookRuntimeConfig, HookScriptEntry, HookScriptSource, McpServerEntry, McpServerSource,
    SkillEntry, SkillSource,
};
use crate::app::config::{
    config_dir, config_file, keybindings_file, mcp_project_file, session_messages_dir,
    sessions_dir, CoderSettings, SettingsItem, SettingsTab, StoredKeybinding,
    StoredModifiers,
};
use crate::app::events::CoderMode;
use crate::app::events::{
    convert_key_for_binding, convert_key_for_input, convert_modifiers, convert_mouse_button,
    keybinding_labels, key_from_string, key_to_string, CommandAction, ModalState, QueryControl,
    ResponseEvent,
};
use crate::app::parsing::{build_context_injection, build_todo_context, expand_prompt_text};
use crate::app::session::{
    apply_session_history_limit, load_session_index, save_session_index, RateLimitInfo, RateLimits,
    SessionEntry, SessionInfo, SessionUsageStats,
};
use crate::app::tools::tool_result_output;
use crate::app::ui::{
    agent_list_layout, agent_modal_content_top, hook_event_layout, modal_y_in_content,
    new_session_button_bounds, render_app, session_list_layout, sidebar_layout,
    skill_list_layout, skill_modal_content_top, ThemeSetting, INPUT_PADDING, OUTPUT_PADDING,
    SESSION_MODAL_HEIGHT, STATUS_BAR_HEIGHT,
};
use crate::app::AppState;
use crate::app::{
    build_input, build_markdown_config, build_markdown_renderer, format_relative_time,
    hook_event_label, now_timestamp, settings_rows, truncate_bytes, truncate_preview, HookLogEntry,
    HookModalView, HookSetting, ModelOption, SettingsInputMode, SettingsSnapshot,
};

const BUG_REPORT_URL: &str = "https://github.com/OpenAgentsInc/openagents/issues/new";

mod command_palette_ids {
    pub const HELP: &str = "help.open";
    pub const SETTINGS: &str = "settings.open";
    pub const MODEL_PICKER: &str = "model.open";
    pub const SESSION_LIST: &str = "session.list";
    pub const SESSION_FORK: &str = "session.fork";
    pub const SESSION_EXPORT: &str = "session.export";
    pub const CLEAR_CONVERSATION: &str = "session.clear";
    pub const UNDO_LAST: &str = "session.undo";
    pub const COMPACT_CONTEXT: &str = "context.compact";
    pub const INTERRUPT_REQUEST: &str = "request.interrupt";
    pub const PERMISSION_RULES: &str = "permissions.rules";
    pub const MODE_CYCLE: &str = "mode.cycle";
    pub const MODE_BYPASS: &str = "mode.bypass";
    pub const MODE_PLAN: &str = "mode.plan";
    pub const MODE_AUTOPILOT: &str = "mode.autopilot";
    pub const TOOLS_LIST: &str = "tools.list";
    pub const MCP_CONFIG: &str = "mcp.open";
    pub const MCP_RELOAD: &str = "mcp.reload";
    pub const MCP_STATUS: &str = "mcp.status";
    pub const AGENTS_LIST: &str = "agents.list";
    pub const AGENT_CLEAR: &str = "agents.clear";
    pub const AGENT_RELOAD: &str = "agents.reload";
    pub const SKILLS_LIST: &str = "skills.list";
    pub const SKILLS_RELOAD: &str = "skills.reload";
    pub const HOOKS_OPEN: &str = "hooks.open";
    pub const HOOKS_RELOAD: &str = "hooks.reload";
    pub const SIDEBAR_LEFT: &str = "sidebar.left";
    pub const SIDEBAR_RIGHT: &str = "sidebar.right";
    pub const SIDEBAR_TOGGLE: &str = "sidebar.toggle";
    pub const BUG_REPORT: &str = "bug.report";
    pub const KITCHEN_SINK: &str = "dev.kitchen_sink";
}

fn clamp_font_size(size: f32) -> f32 {
    size.clamp(12.0, 18.0)
}

fn normalize_settings(settings: &mut CoderSettings) {
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
async fn fetch_rate_limits() -> Option<RateLimits> {
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

const HOOK_LOG_LIMIT: usize = 200;
const HOOK_SCRIPT_TIMEOUT_SECS: u64 = 12;
const HOOK_OUTPUT_TRUNCATE: usize = 2000;
const HOOK_BLOCK_PATTERNS: [&str; 3] = ["rm -rf /", "sudo", "> /dev/"];
const TOOL_HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug)]
pub(crate) enum HookCallbackKind {
    ToolBlocker,
    ToolLogger,
    OutputTruncator,
    ContextEnforcer,
    Script(HookScriptEntry),
}

pub(crate) struct CoderHookCallback {
    kind: HookCallbackKind,
    runtime: Arc<HookRuntimeConfig>,
}

impl CoderHookCallback {
    pub(crate) fn new(kind: HookCallbackKind, runtime: Arc<HookRuntimeConfig>) -> Self {
        Self { kind, runtime }
    }
}

#[async_trait]
impl HookCallback for CoderHookCallback {
    async fn call(&self, input: HookInput, tool_use_id: Option<String>) -> SdkResult<HookOutput> {
        let event = hook_event_from_input(&input);
        let tool_name = hook_tool_name(&input);
        let matcher = match &self.kind {
            HookCallbackKind::Script(entry) => entry.matcher.clone(),
            _ => None,
        };

        let summary: String;
        let mut error = None;
        let mut sources = Vec::new();
        let mut output = HookOutput::Sync(SyncHookOutput::continue_execution());
        let mut log_output = true;

        match &self.kind {
            HookCallbackKind::ToolBlocker => {
                sources.push("builtin:tool_blocker".to_string());
                let (next_output, next_summary) = hook_tool_blocker(&input);
                output = next_output;
                summary = next_summary;
            }
            HookCallbackKind::ToolLogger => {
                sources.push("builtin:tool_logger".to_string());
                summary = hook_tool_logger_summary(&input);
            }
            HookCallbackKind::OutputTruncator => {
                sources.push("builtin:output_truncator".to_string());
                let (next_output, next_summary) = hook_output_truncator(&input);
                output = next_output;
                summary = next_summary;
            }
            HookCallbackKind::ContextEnforcer => {
                sources.extend(hook_context_sources(&self.runtime.config));
                let (next_output, next_summary) =
                    hook_context_enforcer(&self.runtime, &input);
                output = next_output;
                summary = next_summary;
            }
            HookCallbackKind::Script(entry) => {
                sources.push(hook_script_source_label(entry));
                match run_hook_script(entry, &input, tool_use_id.as_deref(), &self.runtime).await {
                    Ok(next_output) => {
                        output = next_output;
                        summary = format!("Script {} completed.", entry.path.display());
                    }
                    Err(err) => {
                        summary = format!("Script {} failed.", entry.path.display());
                        error = Some(err);
                        log_output = false;
                    }
                }
            }
        }

        let output_ref = if log_output { Some(&output) } else { None };
        log_hook_event(
            &self.runtime,
            event,
            summary,
            tool_name,
            matcher,
            &input,
            output_ref,
            error,
            sources,
        );

        Ok(output)
    }
}

fn parse_legacy_model_setting(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(model_id) = line.strip_prefix("model = \"").and_then(|s| s.strip_suffix("\"")) {
            return Some(model_id.to_string());
        }
    }
    None
}

fn load_settings() -> CoderSettings {
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

fn save_settings(settings: &CoderSettings) {
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
fn auto_start_llama_server() -> Option<Child> {
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

fn settings_model_option(settings: &CoderSettings) -> ModelOption {
    settings
        .model
        .as_deref()
        .map(ModelOption::from_id)
        .unwrap_or(ModelOption::Opus)
}

fn update_settings_model(settings: &mut CoderSettings, model: ModelOption) {
    settings.model = Some(model.model_id().to_string());
}

fn load_keybindings() -> Vec<Keybinding> {
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

fn save_keybindings(bindings: &[Keybinding]) {
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

fn parse_mcp_status(value: &Value) -> Result<Vec<McpServerStatus>, String> {
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

/// Main application
pub struct CoderApp {
    state: Option<AppState>,
    runtime_handle: tokio::runtime::Handle,
}

impl CoderApp {
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            state: None,
            runtime_handle,
        }
    }
}

impl ApplicationHandler for CoderApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Coder")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 600));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);
            let clipboard = Rc::new(RefCell::new(Clipboard::new().ok()));
            let mut event_context = EventContext::new();
            let read_clip = clipboard.clone();
            let write_clip = clipboard.clone();
            event_context.set_clipboard(
                move || read_clip.borrow_mut().as_mut()?.get_text().ok(),
                move |text| {
                    if let Some(clip) = write_clip.borrow_mut().as_mut() {
                        let _ = clip.set_text(text);
                    }
                },
            );
            let (command_palette_tx, command_palette_rx) = mpsc::unbounded_channel();
            let command_palette = CommandPalette::new()
                .max_visible_items(8)
                .mono(true)
                .on_select(move |command| {
                    let _ = command_palette_tx.send(command.id.clone());
                });
            let settings = load_settings();
            let input = build_input(&settings);

            let selected_model = settings_model_option(&settings);
            let mut session_index = load_session_index();
            let removed_sessions =
                apply_session_history_limit(&mut session_index, settings.session_history_limit);
            if !removed_sessions.is_empty() {
                let _ = save_session_index(&session_index);
            }
            let permission_config = load_permission_config();
            let coder_mode = permission_config.coder_mode;
            let permission_default_allow =
                coder_mode_default_allow(coder_mode, permission_config.default_allow);
            let coder_mode_label_str = coder_mode_label(coder_mode).to_string();
            let cwd = std::env::current_dir().unwrap_or_default();
            let (mcp_project_servers, mcp_project_error) = load_mcp_project_servers(&cwd);
            let mcp_project_path = Some(mcp_project_file(&cwd));
            let agent_catalog = load_agent_entries(&cwd);
            let skill_catalog = load_skill_entries(&cwd);
            let hook_config = load_hook_config();
            let hook_catalog = load_hook_scripts(&cwd);

            // Auto-start llama-server if available but not running
            let llama_server_process = auto_start_llama_server();

            // Detect available LM providers on startup (after potential auto-start)
            let available_providers = adjutant::dspy::lm_config::detect_all_providers();
            tracing::info!("Available LM providers: {:?}", available_providers);

            // Boot OANIX on startup (async, will be cached when ready)
            tracing::info!("Booting OANIX runtime...");
            let (oanix_tx, oanix_rx) = mpsc::unbounded_channel();
            let oanix_manifest_rx = Some(oanix_rx);
            tokio::spawn(async move {
                match oanix::boot().await {
                    Ok(manifest) => {
                        tracing::info!("OANIX booted on startup, workspace: {:?}",
                            manifest.workspace.as_ref().map(|w| &w.root));
                        let _ = oanix_tx.send(manifest);
                    }
                    Err(e) => {
                        tracing::warn!("OANIX boot failed on startup: {}", e);
                    }
                }
            });

            // Fetch rate limits on startup
            let (rate_limit_tx, rate_limit_rx) = mpsc::unbounded_channel();
            tokio::spawn(async move {
                if let Some(limits) = fetch_rate_limits().await {
                    let _ = rate_limit_tx.send(limits);
                }
            });

            AppState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                event_context,
                clipboard,
                command_palette,
                command_palette_action_rx: Some(command_palette_rx),
                input,
                mouse_pos: (0.0, 0.0),
                modifiers: ModifiersState::default(),
                last_tick: Instant::now(),
                modal_state: ModalState::None,
                panel_layout: PanelLayout::Single,
                left_sidebar_open: false,
                right_sidebar_open: false,
                new_session_button_hovered: false,
                chat: ChatState::new(&settings),
                tools: ToolsState::new(),
                session: SessionState::new(
                    selected_model,
                    coder_mode_label_str,
                    session_index,
                    Some(rate_limit_rx),
                ),
                catalogs: CatalogState::new(
                    agent_catalog,
                    skill_catalog,
                    hook_config,
                    hook_catalog,
                    mcp_project_servers,
                    mcp_project_error,
                    mcp_project_path,
                ),
                settings: SettingsState::new(settings, load_keybindings(), selected_model),
                permissions: PermissionState::new(
                    coder_mode,
                    permission_default_allow,
                    permission_config.allow_tools,
                    permission_config.deny_tools,
                    permission_config.bash_allow_patterns,
                    permission_config.bash_deny_patterns,
                ),
                autopilot: AutopilotState::new(oanix_manifest_rx, available_providers),
                llama_server_process,
                show_kitchen_sink: false,
                kitchen_sink_scroll: 0.0,
            }
        });

        let window_clone = state.window.clone();
        self.state = Some(state);
        tracing::info!("Window initialized");

        // Request initial redraw
        window_clone.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Poll for SDK responses first
        self.poll_responses();
        self.poll_permissions();
        self.poll_command_palette_actions();
        self.poll_session_actions();
        self.poll_agent_actions();
        self.poll_skill_actions();
        self.poll_hook_inspector_actions();
        self.poll_oanix_manifest();
        self.poll_autopilot_history();
        self.poll_rate_limits();

        let Some(state) = &mut self.state else {
            return;
        };

        let scale_factor = state.window.scale_factor() as f32;
        let logical_width = state.config.width as f32 / scale_factor;
        let logical_height = state.config.height as f32 / scale_factor;

        let sidebar_layout = sidebar_layout(
            logical_width,
            logical_height,
            state.left_sidebar_open,
            state.right_sidebar_open,
        );
        let content_x = sidebar_layout.main.origin.x + OUTPUT_PADDING;
        // Input bounds above status bar (max width 768px, centered)
        let max_input_width = 768.0_f32;
        let available_input_width = sidebar_layout.main.size.width - INPUT_PADDING * 2.0;
        let input_width = available_input_width.min(max_input_width);
        let input_x = sidebar_layout.main.origin.x + (sidebar_layout.main.size.width - input_width) / 2.0;
        // Set max width for text wrapping, then calculate dynamic height
        state.input.set_max_width(input_width);
        let input_height = state.input.current_height().max(40.0);
        let input_bounds = Bounds::new(
            input_x,
            logical_height - input_height - INPUT_PADDING - STATUS_BAR_HEIGHT,
            input_width,
            input_height,
        );
        let permission_open = state.permissions.permission_dialog
            .as_ref()
            .map(|dialog| dialog.is_open())
            .unwrap_or(false);
        let permission_bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                state.config.width = size.width.max(1);
                state.config.height = size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers = modifiers.state();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32 / scale_factor;
                let y = position.y as f32 / scale_factor;
                state.mouse_pos = (x, y);
                if permission_open {
                    if let Some(dialog) = state.permissions.permission_dialog.as_mut() {
                        let input_event = InputEvent::MouseMove { x, y };
                        let _ = dialog.event(&input_event, permission_bounds, &mut state.event_context);
                    }
                    state.window.request_redraw();
                    return;
                }
                if state.command_palette.is_open() {
                    return;
                }
                if matches!(state.modal_state, ModalState::SessionList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SessionList { selected } => *selected,
                        _ => 0,
                    };
                    if state.session.session_cards.len() != state.session.session_index.len() {
                        state.session.refresh_session_cards(state.chat.is_thinking);
                    }
                    let checkpoint_height = if state.session.checkpoint_entries.is_empty() {
                        0.0
                    } else {
                        state.session.checkpoint_restore.size_hint().1.unwrap_or(0.0)
                    };
                    let layout = session_list_layout(
                        logical_width,
                        logical_height,
                        state.session.session_cards.len(),
                        selected,
                        checkpoint_height,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.session.session_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if let Some(bounds) = layout.checkpoint_bounds {
                        if matches!(
                            state.session.checkpoint_restore
                                .event(&input_event, bounds, &mut state.event_context),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::AgentList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::AgentList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.agent_cards.len() != state.catalogs.agent_entries.len() {
                        state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = agent_modal_content_top(modal_y, state);
                    let layout = agent_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.agent_cards.len(),
                        selected,
                        content_top,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.agent_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::SkillList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SkillList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.skill_cards.len() != state.catalogs.skill_entries.len() {
                        state.catalogs.refresh_skill_cards();
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = skill_modal_content_top(modal_y, state);
                    let layout = skill_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.skill_cards.len(),
                        selected,
                        content_top,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.skill_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(
                    state.modal_state,
                    ModalState::Hooks {
                        view: HookModalView::Events,
                        ..
                    }
                ) {
                    let selected = match &state.modal_state {
                        ModalState::Hooks { selected, .. } => *selected,
                        _ => 0,
                    };
                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        selected,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                        if matches!(
                            inspector.event(
                                &input_event,
                                layout.inspector_bounds,
                                &mut state.event_context
                            ),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }

                // Track hover state for left sidebar button
                if state.left_sidebar_open {
                    if let Some(left_bounds) = sidebar_layout.left {
                        let btn_bounds = new_session_button_bounds(left_bounds);
                        let was_hovered = state.new_session_button_hovered;
                        state.new_session_button_hovered = btn_bounds.contains(Point::new(x, y));
                        if was_hovered != state.new_session_button_hovered {
                            // Change cursor to pointer when hovering button
                            let cursor = if state.new_session_button_hovered {
                                CursorIcon::Pointer
                            } else {
                                CursorIcon::Default
                            };
                            state.window.set_cursor(cursor);
                            state.window.request_redraw();
                        }
                    }
                } else if state.new_session_button_hovered {
                    // Reset cursor when sidebar closes
                    state.new_session_button_hovered = false;
                    state.window.set_cursor(CursorIcon::Default);
                }

                let input_event = InputEvent::MouseMove { x, y };
                let chat_layout = state.build_chat_layout(&sidebar_layout, logical_height);
                if state.chat.chat_context_menu.is_open() {
                    if matches!(
                        state.chat.chat_context_menu.event(
                            &input_event,
                            Bounds::new(0.0, 0.0, logical_width, logical_height),
                            &mut state.event_context,
                        ),
                        EventResult::Handled
                    ) {
                        state.window.request_redraw();
                        return;
                    }
                }
                if state.chat.chat_selection_dragging {
                    if let Some(point) = state.chat_selection_point_at(&chat_layout, x, y) {
                        if let Some(selection) = &mut state.chat.chat_selection {
                            if selection.focus.message_index != point.message_index
                                || selection.focus.offset != point.offset
                            {
                                selection.focus = point;
                                state.window.request_redraw();
                            }
                        }
                    }
                }
                // Handle events for inline tools
                let mut tools_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                            if matches!(
                                tool.card
                                    .event(&input_event, block.card_bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                tools_handled = true;
                            }
                            if tool.sync_expanded_from_card() {
                                tools_handled = true;
                            }
                            if let Some(detail_bounds) = block.detail_bounds {
                                if matches!(
                                    tool.detail
                                        .event(&input_event, detail_bounds, &mut state.event_context),
                                    EventResult::Handled
                                ) {
                                    tools_handled = true;
                                }
                            }
                        }
                    }
                }
                if tools_handled {
                    state.window.request_redraw();
                }
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                let (x, y) = state.mouse_pos;
                let modifiers = wgpui::Modifiers::default();
                let input_event = if button_state == ElementState::Pressed {
                    InputEvent::MouseDown {
                        button: convert_mouse_button(button),
                        x,
                        y,
                        modifiers,
                    }
                } else {
                    InputEvent::MouseUp {
                        button: convert_mouse_button(button),
                        x,
                        y,
                    }
                };
                if permission_open {
                    if let Some(dialog) = state.permissions.permission_dialog.as_mut() {
                        let _ =
                            dialog.event(&input_event, permission_bounds, &mut state.event_context);
                    }
                    state.window.request_redraw();
                    return;
                }
                if state.command_palette.is_open() {
                    let palette_bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);
                    let _ = state
                        .command_palette
                        .event(&input_event, palette_bounds, &mut state.event_context);
                    state.window.request_redraw();
                    return;
                }
                if matches!(state.modal_state, ModalState::SessionList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SessionList { selected } => *selected,
                        _ => 0,
                    };
                    if state.session.session_cards.len() != state.session.session_index.len() {
                        state.session.refresh_session_cards(state.chat.is_thinking);
                    }
                    let checkpoint_height = if state.session.checkpoint_entries.is_empty() {
                        0.0
                    } else {
                        state.session.checkpoint_restore.size_hint().1.unwrap_or(0.0)
                    };
                    let layout = session_list_layout(
                        logical_width,
                        logical_height,
                        state.session.session_cards.len(),
                        selected,
                        checkpoint_height,
                    );
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.session.session_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if let Some(bounds) = layout.checkpoint_bounds {
                        if matches!(
                            state.session.checkpoint_restore
                                .event(&input_event, bounds, &mut state.event_context),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::AgentList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::AgentList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.agent_cards.len() != state.catalogs.agent_entries.len() {
                        state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = agent_modal_content_top(modal_y, state);
                    let layout = agent_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.agent_cards.len(),
                        selected,
                        content_top,
                    );
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.agent_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::SkillList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SkillList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.skill_cards.len() != state.catalogs.skill_entries.len() {
                        state.catalogs.refresh_skill_cards();
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = skill_modal_content_top(modal_y, state);
                    let layout = skill_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.skill_cards.len(),
                        selected,
                        content_top,
                    );
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.skill_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(
                    state.modal_state,
                    ModalState::Hooks {
                        view: HookModalView::Events,
                        ..
                    }
                ) {
                    let selected_index = match &state.modal_state {
                        ModalState::Hooks { selected, .. } => *selected,
                        _ => 0,
                    };
                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        selected_index,
                    );
                    let mut handled = false;
                    if button_state == ElementState::Released {
                        if layout.list_bounds.contains(Point::new(x, y)) {
                            for (index, bounds) in &layout.row_bounds {
                                if bounds.contains(Point::new(x, y)) {
                                    state.modal_state = ModalState::Hooks {
                                        view: HookModalView::Events,
                                        selected: *index,
                                    };
                                    state.sync_hook_inspector(*index);
                                    handled = true;
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                        if matches!(
                            inspector.event(
                                &input_event,
                                layout.inspector_bounds,
                                &mut state.event_context
                            ),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }

                // Handle click on left sidebar "New Session" button
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Left)
                    && state.left_sidebar_open
                {
                    if let Some(left_bounds) = sidebar_layout.left {
                        let btn_bounds = new_session_button_bounds(left_bounds);
                        if btn_bounds.contains(Point::new(x, y)) {
                            state.start_new_session();
                            state.input.focus();
                            state.window.request_redraw();
                            return;
                        }
                    }
                }

                let chat_layout = state.build_chat_layout(
                    &sidebar_layout,
                    logical_height,
                );
                if state.chat.chat_context_menu.is_open() {
                    if matches!(
                        state.chat.chat_context_menu.event(
                            &input_event,
                            Bounds::new(0.0, 0.0, logical_width, logical_height),
                            &mut state.event_context,
                        ),
                        EventResult::Handled
                    ) {
                        if let Some(action) = state.chat.chat_context_menu.take_selected() {
                            state.handle_chat_menu_action(&action, &chat_layout);
                            state.chat.chat_context_menu_target = None;
                        }
                        state.window.request_redraw();
                        return;
                    }
                }
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Left)
                {
                    if let Some(point) = state.chat_selection_point_at(&chat_layout, x, y) {
                        if state.modifiers.shift_key() {
                            if let Some(selection) = &mut state.chat.chat_selection {
                                selection.focus = point;
                            } else {
                                state.chat.chat_selection = Some(ChatSelection {
                                    anchor: point,
                                    focus: point,
                                });
                            }
                        } else {
                            state.chat.chat_selection = Some(ChatSelection {
                                anchor: point,
                                focus: point,
                            });
                        }
                        state.chat.chat_selection_dragging = true;
                        state.window.request_redraw();
                    } else {
                        state.chat.chat_selection = None;
                    }
                }
                if button_state == ElementState::Released
                    && matches!(button, winit::event::MouseButton::Left)
                {
                    state.chat.chat_selection_dragging = false;
                }
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Right)
                {
                    if let Some(point) = state.chat_selection_point_at(&chat_layout, x, y) {
                        if !state.chat_selection_contains(point) {
                            state.chat.chat_selection = Some(ChatSelection {
                                anchor: point,
                                focus: point,
                            });
                        }
                        state.chat.chat_selection_dragging = false;
                        let copy_enabled = state.chat.chat_selection
                            .as_ref()
                            .is_some_and(|sel| !sel.is_empty())
                            || chat_layout.message_layouts.get(point.message_index).is_some();
                        state.open_chat_context_menu(
                            Point::new(x, y),
                            Some(point.message_index),
                            copy_enabled,
                        );
                        state.window.request_redraw();
                        return;
                    }
                }
                // Handle mouse events for inline tools
                let mut tools_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                            if matches!(
                                tool.card
                                    .event(&input_event, block.card_bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                tools_handled = true;
                            }
                            if tool.sync_expanded_from_card() {
                                tools_handled = true;
                            }
                            if let Some(detail_bounds) = block.detail_bounds {
                                if matches!(
                                    tool.detail
                                        .event(&input_event, detail_bounds, &mut state.event_context),
                                    EventResult::Handled
                                ) {
                                    tools_handled = true;
                                }
                            }
                        }
                    }
                }
                if tools_handled {
                    state.window.request_redraw();
                }
                if button_state == ElementState::Released
                    && !state.session.session_info.permission_mode.is_empty()
                {
                    let status_y = logical_height - STATUS_BAR_HEIGHT - 2.0;
                    let mode_text = format!("[{}]", state.session.session_info.permission_mode);
                    let mode_width = mode_text.len() as f32 * 6.6;
                    let mode_bounds = Bounds::new(
                        content_x,
                        status_y - 4.0,
                        mode_width,
                        STATUS_BAR_HEIGHT + 8.0,
                    );
                    if mode_bounds.contains(Point::new(x, y)) {
                        state
                            .permissions
                            .cycle_coder_mode(&mut state.session.session_info);
                        state.window.request_redraw();
                        return;
                    }
                }
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if permission_open {
                    return;
                }
                if state.command_palette.is_open() {
                    return;
                }
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };
                // Kitchen sink scroll handling
                if state.show_kitchen_sink {
                    state.kitchen_sink_scroll = (state.kitchen_sink_scroll - dy * 40.0).max(0.0);
                    state.window.request_redraw();
                    return;
                }
                if matches!(
                    state.modal_state,
                    ModalState::Hooks {
                        view: HookModalView::Events,
                        ..
                    }
                ) {
                    let selected = match &state.modal_state {
                        ModalState::Hooks { selected, .. } => *selected,
                        _ => 0,
                    };
                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        selected,
                    );
                    let mouse_point = Point::new(state.mouse_pos.0, state.mouse_pos.1);
                    if layout.inspector_bounds.contains(mouse_point) {
                        let input_event = InputEvent::Scroll { dx: 0.0, dy: dy * 40.0 };
                        if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                            if matches!(
                                inspector.event(
                                    &input_event,
                                    layout.inspector_bounds,
                                    &mut state.event_context
                                ),
                                EventResult::Handled
                            ) {
                                state.window.request_redraw();
                                return;
                            }
                        }
                    } else if layout.list_bounds.contains(mouse_point) {
                        let mut next_selected = selected;
                        if dy > 0.0 {
                            next_selected = next_selected.saturating_add(1);
                        } else if dy < 0.0 {
                            next_selected = next_selected.saturating_sub(1);
                        }
                        if !state.catalogs.hook_event_log.is_empty() {
                            next_selected = next_selected.min(state.catalogs.hook_event_log.len() - 1);
                        } else {
                            next_selected = 0;
                        }
                        if next_selected != selected {
                            state.modal_state = ModalState::Hooks {
                                view: HookModalView::Events,
                                selected: next_selected,
                            };
                            state.sync_hook_inspector(next_selected);
                            state.window.request_redraw();
                        }
                        return;
                    }
                }
                let chat_layout = state.build_chat_layout(&sidebar_layout, logical_height);
                // Handle scroll events for inline tools
                let mouse_point = Point::new(state.mouse_pos.0, state.mouse_pos.1);
                let scroll_input_event = InputEvent::Scroll { dx: 0.0, dy: dy * 40.0 };
                let mut scroll_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if block.card_bounds.contains(mouse_point) {
                            if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                                if matches!(
                                    tool.card
                                        .event(&scroll_input_event, block.card_bounds, &mut state.event_context),
                                    EventResult::Handled
                                ) {
                                    scroll_handled = true;
                                }
                                if let Some(detail_bounds) = block.detail_bounds {
                                    if matches!(
                                        tool.detail
                                            .event(&scroll_input_event, detail_bounds, &mut state.event_context),
                                        EventResult::Handled
                                    ) {
                                        scroll_handled = true;
                                    }
                                }
                            }
                        }
                    }
                }
                if scroll_handled {
                    state.window.request_redraw();
                    return;
                }
                // Scroll the message area (positive dy = scroll up, negative = scroll down)
                state.chat.scroll_offset = (state.chat.scroll_offset - dy * 40.0).max(0.0);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if key_event.state == ElementState::Pressed {
                    if permission_open {
                        return;
                    }

                    if state.command_palette.is_open() {
                        if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                            let modifiers = convert_modifiers(&state.modifiers);
                            let input_event = InputEvent::KeyDown { key, modifiers };
                            let palette_bounds =
                                Bounds::new(0.0, 0.0, logical_width, logical_height);
                            let _ = state.command_palette.event(
                                &input_event,
                                palette_bounds,
                                &mut state.event_context,
                            );
                            state.window.request_redraw();
                        }
                        return;
                    }

                    // Kitchen sink overlay - handle Escape to close
                    if state.show_kitchen_sink {
                        if let WinitKey::Named(WinitNamedKey::Escape) = &key_event.logical_key {
                            state.show_kitchen_sink = false;
                            state.window.request_redraw();
                            return;
                        }
                        // Consume all other keys while kitchen sink is open
                        return;
                    }

                    // Autopilot loop interrupt - Escape stops autonomous execution
                    if matches!(state.permissions.coder_mode, CoderMode::Autopilot) {
                        if let WinitKey::Named(WinitNamedKey::Escape) = &key_event.logical_key {
                            if state.chat.is_thinking {
                                // Signal interrupt to the autopilot loop
                                state.autopilot.autopilot_interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                                tracing::info!("Autopilot: interrupt requested by user");
                                state.window.request_redraw();
                                return;
                            }
                        }
                    }

                    if let WinitKey::Named(WinitNamedKey::F1) = &key_event.logical_key {
                        if matches!(state.modal_state, ModalState::Help) {
                            state.modal_state = ModalState::None;
                        } else {
                            state.open_help();
                        }
                        state.window.request_redraw();
                        return;
                    }
                    if handle_modal_input(state, &key_event.logical_key) {
                        return;
                    }

                    let modifiers = convert_modifiers(&state.modifiers);

                    if state.chat.chat_context_menu.is_open() {
                        if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                            let input_event = InputEvent::KeyDown { key, modifiers };
                            if matches!(
                                state.chat.chat_context_menu.event(
                                    &input_event,
                                    Bounds::new(0.0, 0.0, logical_width, logical_height),
                                    &mut state.event_context,
                                ),
                                EventResult::Handled
                            ) {
                                if let Some(action) = state.chat.chat_context_menu.take_selected() {
                                    let chat_layout = state.build_chat_layout(
                                        &sidebar_layout,
                                        logical_height,
                                    );
                                    state.handle_chat_menu_action(&action, &chat_layout);
                                    state.chat.chat_context_menu_target = None;
                                }
                                state.window.request_redraw();
                                return;
                            }
                        }
                    }

                    if state.handle_chat_shortcut(
                        &key_event.logical_key,
                        modifiers,
                        &sidebar_layout,
                        logical_height,
                    ) {
                        state.window.request_redraw();
                        return;
                    }

                    if let Some(key) = convert_key_for_binding(&key_event.logical_key) {
                        if let Some(action) = match_action(&key, modifiers, &state.settings.keybindings) {
                            match action {
                                KeyAction::Interrupt => state.interrupt_query(),
                                KeyAction::OpenCommandPalette => {
                                    state.open_command_palette();
                                }
                                KeyAction::OpenSettings => state.open_config(),
                                KeyAction::ToggleLeftSidebar => state.toggle_left_sidebar(),
                                KeyAction::ToggleRightSidebar => state.toggle_right_sidebar(),
                                KeyAction::ToggleSidebars => state.toggle_sidebars(),
                            }
                            state.window.request_redraw();
                            return;
                        }
                    }

                    if let WinitKey::Named(WinitNamedKey::Tab) = &key_event.logical_key {
                        if state.modifiers.shift_key() {
                            state
                                .permissions
                                .cycle_coder_mode(&mut state.session.session_info);
                            state.window.request_redraw();
                            return;
                        }
                    }

                    // Check for Enter key to submit (but not Shift+Enter, which inserts newline)
                    if let WinitKey::Named(WinitNamedKey::Enter) = &key_event.logical_key {
                        if !state.modifiers.shift_key() {
                            let mut action = CommandAction::None;
                            let mut submit_prompt = None;

                            {
                                let prompt = state.input.get_value().to_string();
                                if prompt.trim().is_empty() {
                                    return;
                                }

                                if let Some(command) = parse_command(&prompt) {
                                    state.settings.command_history.push(prompt);
                                    state.input.set_value("");
                                    action = handle_command(state, command);
                                } else if !state.chat.is_thinking {
                                    state.settings.command_history.push(prompt.clone());
                                    state.input.set_value("");
                                    submit_prompt = Some(prompt);
                                } else {
                                    return;
                                }
                            }

                            if let CommandAction::SubmitPrompt(prompt) = action {
                                self.submit_prompt(prompt);
                            } else if let Some(prompt) = submit_prompt {
                                self.submit_prompt(prompt);
                            }

                            if let Some(s) = &self.state {
                                s.window.request_redraw();
                            }
                            return;
                        }
                        // Shift+Enter falls through to input handler below
                    }

                    if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                        let input_event = InputEvent::KeyDown { key, modifiers };
                        state
                            .input
                            .event(&input_event, input_bounds, &mut state.event_context);
                        state.window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Continuously request redraws when input is focused for cursor blinking
        if let Some(state) = &self.state {
            if state.input.is_focused() {
                state.window.request_redraw();
            }
        }
    }
}

impl AppState {
    fn build_command_palette_commands(&self) -> Vec<PaletteCommand> {
        let mut commands = Vec::new();
        let mut push_command = |id: &str,
                                label: &str,
                                description: &str,
                                category: &str,
                                keybinding: Option<String>| {
            let mut command = PaletteCommand::new(id, label)
                .description(description)
                .category(category);
            if let Some(keys) = keybinding {
                command = command.keybinding(keys);
            }
            commands.push(command);
        };

        let interrupt_keys = keybinding_labels(&self.settings.keybindings, KeyAction::Interrupt, "Ctrl+C");
        push_command(
            command_palette_ids::INTERRUPT_REQUEST,
            "Interrupt Request",
            "Stop the active response stream",
            "Request",
            Some(interrupt_keys),
        );

        push_command(
            command_palette_ids::HELP,
            "Open Help",
            "Show hotkeys and feature overview",
            "Navigation",
            Some("F1".to_string()),
        );

        let settings_keys = keybinding_labels(&self.settings.keybindings, KeyAction::OpenSettings, "Ctrl+,");
        push_command(
            command_palette_ids::SETTINGS,
            "Open Settings",
            "Configure Coder preferences",
            "Navigation",
            Some(settings_keys),
        );

        push_command(
            command_palette_ids::MODEL_PICKER,
            "Select Model",
            "Choose the model for this session",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::SESSION_LIST,
            "Open Session List",
            "Resume or fork previous sessions",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::AGENTS_LIST,
            "Open Agents",
            "Browse available agents",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::SKILLS_LIST,
            "Open Skills",
            "Browse available skills",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::HOOKS_OPEN,
            "Open Hooks",
            "Manage hook configuration",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::TOOLS_LIST,
            "Open Tool List",
            "Review available tools",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::MCP_CONFIG,
            "Open MCP Servers",
            "Manage MCP configuration",
            "Navigation",
            None,
        );

        push_command(
            command_palette_ids::CLEAR_CONVERSATION,
            "Clear Conversation",
            "Reset the current chat history",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::UNDO_LAST,
            "Undo Last Exchange",
            "Remove the most recent exchange",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::COMPACT_CONTEXT,
            "Compact Context",
            "Summarize older context into a shorter prompt",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::SESSION_FORK,
            "Fork Session",
            "Create a new branch of this session",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::SESSION_EXPORT,
            "Export Session",
            "Export conversation to markdown",
            "Session",
            None,
        );

        push_command(
            command_palette_ids::MODE_CYCLE,
            "Cycle Mode",
            "Rotate through modes (Bypass/Plan/Autopilot)",
            "Mode",
            Some("Shift+Tab".to_string()),
        );
        push_command(
            command_palette_ids::MODE_BYPASS,
            "Mode: Bypass Permissions",
            "Auto-approve all tool use",
            "Mode",
            None,
        );
        push_command(
            command_palette_ids::MODE_PLAN,
            "Mode: Plan",
            "Read-only mode, deny write operations",
            "Mode",
            None,
        );
        push_command(
            command_palette_ids::MODE_AUTOPILOT,
            "Mode: Autopilot",
            "Use DSPy/Adjutant for autonomous execution",
            "Mode",
            None,
        );
        push_command(
            command_palette_ids::PERMISSION_RULES,
            "Open Permission Rules",
            "Manage tool allow/deny rules",
            "Permissions",
            None,
        );

        let left_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::ToggleLeftSidebar, "Ctrl+[");
        let right_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::ToggleRightSidebar, "Ctrl+]");
        let toggle_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::ToggleSidebars, "Ctrl+\\");
        push_command(
            command_palette_ids::SIDEBAR_LEFT,
            "Open Left Sidebar",
            "Show the left sidebar",
            "Layout",
            Some(left_keys),
        );
        push_command(
            command_palette_ids::SIDEBAR_RIGHT,
            "Open Right Sidebar",
            "Show the right sidebar",
            "Layout",
            Some(right_keys),
        );
        push_command(
            command_palette_ids::SIDEBAR_TOGGLE,
            "Toggle Sidebars",
            "Show or hide both sidebars",
            "Layout",
            Some(toggle_keys),
        );

        push_command(
            command_palette_ids::MCP_RELOAD,
            "Reload MCP Config",
            "Reload MCP servers from project config",
            "MCP",
            None,
        );
        push_command(
            command_palette_ids::MCP_STATUS,
            "Refresh MCP Status",
            "Fetch MCP server status",
            "MCP",
            None,
        );

        push_command(
            command_palette_ids::AGENT_CLEAR,
            "Clear Active Agent",
            "Stop using the active agent",
            "Agents",
            None,
        );
        push_command(
            command_palette_ids::AGENT_RELOAD,
            "Reload Agents",
            "Reload agent definitions from disk",
            "Agents",
            None,
        );

        push_command(
            command_palette_ids::SKILLS_RELOAD,
            "Reload Skills",
            "Reload skills from disk",
            "Skills",
            None,
        );

        push_command(
            command_palette_ids::HOOKS_RELOAD,
            "Reload Hooks",
            "Reload hook scripts from disk",
            "Hooks",
            None,
        );

        push_command(
            command_palette_ids::BUG_REPORT,
            "Report a Bug",
            "Open the issue tracker",
            "Diagnostics",
            None,
        );

        push_command(
            command_palette_ids::KITCHEN_SINK,
            "Kitchen Sink",
            "Show all UI component variations",
            "Developer",
            None,
        );

        commands
    }

    fn open_command_palette(&mut self) {
        self.modal_state = ModalState::None;
        if self.chat.chat_context_menu.is_open() {
            self.chat.chat_context_menu.close();
            self.chat.chat_context_menu_target = None;
        }
        self.command_palette.set_commands(self.build_command_palette_commands());
        self.command_palette.open();
    }

    fn open_model_picker(&mut self) {
        let current_idx = ModelOption::all()
            .iter()
            .position(|m| *m == self.settings.selected_model)
            .unwrap_or(0);
        self.modal_state = ModalState::ModelPicker { selected: current_idx };
    }

    fn open_session_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (checkpoint_tx, checkpoint_rx) = mpsc::unbounded_channel();
        self.session.session_action_tx = Some(action_tx);
        self.session.session_action_rx = Some(action_rx);
        self.session.checkpoint_action_tx = Some(checkpoint_tx);
        self.session.checkpoint_action_rx = Some(checkpoint_rx);
        self.session.refresh_session_cards(self.chat.is_thinking);
        self.session.refresh_checkpoint_restore(&self.chat.messages);
        let selected = self.session.session_index
            .iter()
            .position(|entry| entry.id == self.session.session_info.session_id)
            .unwrap_or(0);
        self.modal_state = ModalState::SessionList { selected };
    }

    fn open_agent_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.agent_action_tx = Some(action_tx);
        self.catalogs.agent_action_rx = Some(action_rx);
        self.catalogs.refresh_agent_cards(self.chat.is_thinking);
        let selected = self.catalogs.active_agent
            .as_ref()
            .and_then(|name| {
                self.catalogs.agent_entries
                    .iter()
                    .position(|entry| entry.name == *name)
            })
            .unwrap_or(0);
        self.modal_state = ModalState::AgentList { selected };
    }

    fn open_skill_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.skill_action_tx = Some(action_tx);
        self.catalogs.skill_action_rx = Some(action_rx);
        self.catalogs.refresh_skill_cards();
        self.modal_state = ModalState::SkillList { selected: 0 };
    }

    fn open_tool_list(&mut self) {
        self.modal_state = ModalState::ToolList { selected: 0 };
    }

    fn open_permission_rules(&mut self) {
        self.modal_state = ModalState::PermissionRules;
    }

    fn open_config(&mut self) {
        self.modal_state = ModalState::Config {
            tab: SettingsTab::General,
            selected: 0,
            search: String::new(),
            input_mode: SettingsInputMode::Normal,
        };
    }

    fn persist_settings(&self) {
        save_settings(&self.settings.coder_settings);
    }

    fn apply_settings(&mut self) {
        normalize_settings(&mut self.settings.coder_settings);
        let current_value = self.input.get_value().to_string();
        let focused = self.input.is_focused();
        self.input = build_input(&self.settings.coder_settings);
        self.input.set_value(current_value);
        if focused {
            self.input.focus();
        }
        self.chat.markdown_renderer = build_markdown_renderer(&self.settings.coder_settings);
        self.chat.streaming_markdown.set_markdown_config(build_markdown_config(&self.settings.coder_settings));
    }

    fn update_selected_model(&mut self, model: ModelOption) {
        self.settings.selected_model = model;
        self.session.session_info.model = self.settings.selected_model.model_id().to_string();
        update_settings_model(&mut self.settings.coder_settings, self.settings.selected_model);
        self.persist_settings();
    }

    fn toggle_left_sidebar(&mut self) {
        self.left_sidebar_open = !self.left_sidebar_open;
    }

    fn toggle_right_sidebar(&mut self) {
        self.right_sidebar_open = !self.right_sidebar_open;
    }

    fn toggle_sidebars(&mut self) {
        let should_open = !(self.left_sidebar_open && self.right_sidebar_open);
        self.left_sidebar_open = should_open;
        self.right_sidebar_open = should_open;
    }

    fn apply_session_history_limit(&mut self) {
        let removed =
            apply_session_history_limit(&mut self.session.session_index, self.settings.coder_settings.session_history_limit);
        if !removed.is_empty() {
            let _ = save_session_index(&self.session.session_index);
            for removed_id in removed {
                let _ = fs::remove_dir_all(session_messages_dir(&removed_id));
            }
            self.session.refresh_session_cards(self.chat.is_thinking);
        }
    }

    fn open_mcp_config(&mut self) {
        self.modal_state = ModalState::McpConfig { selected: 0 };
    }

    fn open_help(&mut self) {
        self.modal_state = ModalState::Help;
    }

    fn open_hooks(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.hook_inspector_action_tx = Some(action_tx);
        self.catalogs.hook_inspector_action_rx = Some(action_rx);
        self.modal_state = ModalState::Hooks {
            view: HookModalView::Config,
            selected: 0,
        };
    }

    fn reload_hooks(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let catalog = load_hook_scripts(&cwd);
        self.catalogs.hook_scripts = catalog.entries;
        self.catalogs.hook_project_path = catalog.project_path;
        self.catalogs.hook_user_path = catalog.user_path;
        self.catalogs.hook_load_error = catalog.error;
    }

    fn toggle_hook_setting(&mut self, setting: HookSetting) {
        match setting {
            HookSetting::ToolBlocker => {
                self.catalogs.hook_config.tool_blocker = !self.catalogs.hook_config.tool_blocker;
            }
            HookSetting::ToolLogger => {
                self.catalogs.hook_config.tool_logger = !self.catalogs.hook_config.tool_logger;
            }
            HookSetting::OutputTruncator => {
                self.catalogs.hook_config.output_truncator = !self.catalogs.hook_config.output_truncator;
            }
            HookSetting::ContextInjection => {
                self.catalogs.hook_config.context_injection = !self.catalogs.hook_config.context_injection;
            }
            HookSetting::TodoEnforcer => {
                self.catalogs.hook_config.todo_enforcer = !self.catalogs.hook_config.todo_enforcer;
            }
        }
        save_hook_config(&self.catalogs.hook_config);
    }

    fn clear_hook_log(&mut self) {
        self.catalogs.hook_event_log.clear();
        self.catalogs.hook_inspector = None;
        if let ModalState::Hooks { view, selected } = &mut self.modal_state {
            if *view == HookModalView::Events {
                *selected = 0;
            }
        }
    }

    fn reload_agents(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let catalog = load_agent_entries(&cwd);
        self.catalogs.agent_entries = catalog.entries;
        self.catalogs.agent_project_path = catalog.project_path;
        self.catalogs.agent_user_path = catalog.user_path;
        self.catalogs.agent_load_error = catalog.error;
        if let Some(active) = self.catalogs.active_agent.clone() {
            if !self.catalogs.agent_entries.iter().any(|entry| entry.name == active) {
                self.catalogs.active_agent = None;
                self.push_system_message(format!(
                    "Active agent {} no longer available.",
                    active
                ));
            }
        }
        self.catalogs.refresh_agent_cards(self.chat.is_thinking);
    }

    fn reload_skills(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let catalog = load_skill_entries(&cwd);
        self.catalogs.skill_entries = catalog.entries;
        self.catalogs.skill_project_path = catalog.project_path;
        self.catalogs.skill_user_path = catalog.user_path;
        self.catalogs.skill_load_error = catalog.error;
        self.catalogs.refresh_skill_cards();
    }

    fn reload_mcp_project_servers(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let (servers, error) = load_mcp_project_servers(&cwd);
        self.catalogs.mcp_project_servers = servers;
        self.catalogs.mcp_project_error = error;
        self.catalogs.mcp_project_path = Some(mcp_project_file(&cwd));
    }

    fn request_mcp_status(&mut self) {
        if let Some(tx) = &self.chat.query_control_tx {
            let _ = tx.send(QueryControl::FetchMcpStatus);
        } else {
            self.push_system_message("No active session for MCP status.".to_string());
        }
    }

    fn handle_session_card_action(&mut self, action: SessionAction, session_id: String) {
        match action {
            SessionAction::Select | SessionAction::Resume => {
                self.begin_session_resume(session_id);
                self.modal_state = ModalState::None;
            }
            SessionAction::Fork => {
                self.begin_session_fork_from(session_id);
                self.modal_state = ModalState::None;
            }
            SessionAction::Delete => {
                self.push_system_message("Session delete not implemented yet.".to_string());
            }
        }
    }

    fn handle_agent_card_action(&mut self, action: AgentCardAction, agent_id: String) {
        match action {
            AgentCardAction::Select => {
                self.set_active_agent_by_name(&agent_id);
                self.modal_state = ModalState::None;
            }
            AgentCardAction::ToggleActive => {
                if self.catalogs.active_agent.as_deref() == Some(agent_id.as_str()) {
                    self.clear_active_agent();
                } else {
                    self.set_active_agent_by_name(&agent_id);
                }
            }
        }
    }

    fn handle_skill_card_action(&mut self, action: SkillCardAction, skill_id: String) {
        match action {
            SkillCardAction::View => {
                if let Some(index) = self.catalogs.skill_entries
                    .iter()
                    .position(|entry| entry.info.id == skill_id)
                {
                    if matches!(self.modal_state, ModalState::SkillList { .. }) {
                        self.modal_state = ModalState::SkillList { selected: index };
                    }
                }
            }
            SkillCardAction::Install => {
                if let Some(entry) = self.catalogs.skill_entries
                    .iter()
                    .find(|entry| entry.info.id == skill_id)
                {
                    self.push_system_message(format!(
                        "Skill {} is already installed at {}.",
                        entry.info.name,
                        entry.path.display()
                    ));
                }
            }
        }
    }

    fn set_active_agent_by_name(&mut self, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            self.push_system_message("Agent name is required.".to_string());
            return;
        }
        if let Some(entry) = self.catalogs.agent_entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(trimmed))
        {
            self.set_active_agent(Some(entry.name.clone()));
        } else {
            self.push_system_message(format!("Unknown agent: {}.", trimmed));
        }
    }

    fn clear_active_agent(&mut self) {
        self.set_active_agent(None);
    }

    fn set_active_agent(&mut self, agent: Option<String>) {
        let next = agent.and_then(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        if next == self.catalogs.active_agent {
            return;
        }
        self.catalogs.active_agent = next.clone();
        if let Some(name) = next {
            self.push_system_message(format!("Active agent set to {}.", name));
        } else {
            self.push_system_message("Active agent cleared.".to_string());
        }
        self.catalogs.refresh_agent_cards(self.chat.is_thinking);
    }

    fn agent_definitions_for_query(&self) -> HashMap<String, AgentDefinition> {
        let mut agents = HashMap::new();
        for entry in &self.catalogs.agent_entries {
            agents.insert(entry.name.clone(), entry.definition.clone());
        }
        agents
    }

    fn setting_sources_for_query(&self) -> Vec<SettingSource> {
        let mut sources = Vec::new();
        if self.catalogs.skill_entries
            .iter()
            .any(|entry| entry.source == SkillSource::Project)
        {
            sources.push(SettingSource::Project);
        }
        if self.catalogs.skill_entries
            .iter()
            .any(|entry| entry.source == SkillSource::User)
        {
            sources.push(SettingSource::User);
        }
        sources
    }

    fn push_hook_log(&mut self, entry: HookLogEntry) {
        self.catalogs.hook_event_log.insert(0, entry);
        if self.catalogs.hook_event_log.len() > HOOK_LOG_LIMIT {
            self.catalogs.hook_event_log.truncate(HOOK_LOG_LIMIT);
        }
        if let ModalState::Hooks {
            view: HookModalView::Events,
            selected,
        } = &mut self.modal_state
        {
            *selected = 0;
            self.sync_hook_inspector(0);
        }
    }

    fn sync_hook_inspector(&mut self, selected: usize) {
        let Some(entry) = self.catalogs.hook_event_log.get(selected) else {
            self.catalogs.hook_inspector = None;
            return;
        };

        let event = hook_log_event_data(entry);
        let view = self.catalogs.hook_inspector_view;
        let mut inspector = EventInspector::new(event).view(view);
        if let Some(tx) = self.catalogs.hook_inspector_action_tx.clone() {
            inspector = inspector.on_view_change(move |view| {
                let _ = tx.send(view);
            });
        }
        self.catalogs.hook_inspector = Some(inspector);
    }

    fn handle_checkpoint_restore(&mut self, index: usize) {
        if let Some(entry) = self.session.checkpoint_entries.get(index) {
            self.request_rewind_files(entry.user_message_id.clone());
        }
    }

    fn begin_session_fork_from(&mut self, session_id: String) {
        let session_id = session_id.trim().to_string();
        if session_id.is_empty() {
            self.push_system_message("Session id is required to fork.".to_string());
            return;
        }
        self.session.pending_resume_session = Some(session_id.clone());
        self.session.pending_fork_session = true;
        self.session.session_info.session_id = session_id.clone();
        if let Some(entry) = self.session.session_index.iter().find(|entry| entry.id == session_id) {
            self.session.session_info.model = entry.model.clone();
        }
        match self
            .session
            .restore_session(&session_id, &mut self.chat, &mut self.tools)
        {
            Ok(()) => self.push_system_message(format!(
                "Loaded cached history for session {}.",
                session_id
            )),
            Err(_) => {
                self.chat.messages.clear();
                self.push_system_message(format!(
                    "No local history for session {} yet.",
                    session_id
                ));
            }
        }
        self.push_system_message(format!(
            "Next message will fork session {}.",
            session_id
        ));
        self.session.refresh_session_cards(self.chat.is_thinking);
    }

    fn attach_user_message_id(&mut self, uuid: String) {
        if let Some(message) = self.chat.messages
            .iter_mut()
            .rev()
            .find(|msg| matches!(msg.role, MessageRole::User) && msg.uuid.is_none())
        {
            message.uuid = Some(uuid);
            self.session.refresh_checkpoint_restore(&self.chat.messages);
        }
    }

    fn request_rewind_files(&mut self, user_message_id: String) {
        if let Some(tx) = &self.chat.query_control_tx {
            let _ = tx.send(QueryControl::RewindFiles { user_message_id: user_message_id.clone() });
            self.push_system_message(format!(
                "Requested checkpoint restore for message {}.",
                truncate_preview(&user_message_id, 12)
            ));
        } else {
            self.push_system_message("No active request to rewind.".to_string());
        }
    }

    fn clear_conversation(&mut self) {
        if self.chat.is_thinking {
            self.push_system_message(
                "Cannot clear while a response is in progress.".to_string(),
            );
            return;
        }
        self.chat.messages.clear();
        self.chat.streaming_markdown.reset();
        self.chat.scroll_offset = 0.0;
        self.tools.current_tool_name = None;
        self.tools.current_tool_input.clear();
        self.tools.current_tool_use_id = None;
        self.tools.tool_history.clear();
        self.session.session_info.session_id.clear();
        self.session.session_info.tool_count = 0;
        self.session.session_info.tools.clear();
        self.session.pending_resume_session = None;
        self.session.pending_fork_session = false;
        self.session.checkpoint_entries.clear();
        self.session.checkpoint_restore = CheckpointRestore::new();
        self.session.refresh_session_cards(self.chat.is_thinking);
    }

    fn start_new_session(&mut self) {
        if self.chat.is_thinking {
            self.push_system_message("Cannot start new session while processing.".to_string());
            return;
        }
        self.chat.messages.clear();
        self.chat.streaming_markdown.reset();
        self.chat.scroll_offset = 0.0;
        self.tools.current_tool_name = None;
        self.tools.current_tool_input.clear();
        self.tools.current_tool_use_id = None;
        self.tools.tool_history.clear();
        self.session.session_usage = SessionUsageStats::default();
        self.session.session_info.session_id.clear();
        self.session.session_info.tool_count = 0;
        self.session.session_info.tools.clear();
        self.session.pending_resume_session = None;
        self.session.pending_fork_session = false;
        self.session.checkpoint_entries.clear();
        self.session.checkpoint_restore = CheckpointRestore::new();
        self.session.refresh_session_cards(self.chat.is_thinking);
        self.push_system_message("Started new session.".to_string());
    }

    fn undo_last_exchange(&mut self) {
        if self.chat.is_thinking {
            self.push_system_message(
                "Cannot undo while a response is in progress.".to_string(),
            );
            return;
        }

        let mut removed = 0;
        while matches!(self.chat.messages.last(), Some(ChatMessage { role: MessageRole::Assistant, .. })) {
            self.chat.messages.pop();
            removed += 1;
        }
        if matches!(self.chat.messages.last(), Some(ChatMessage { role: MessageRole::User, .. })) {
            self.chat.messages.pop();
            removed += 1;
        }

        if removed == 0 {
            self.push_system_message("Nothing to undo.".to_string());
        } else {
            self.session.refresh_checkpoint_restore(&self.chat.messages);
        }
    }

    fn interrupt_query(&mut self) {
        if let Some(tx) = &self.chat.query_control_tx {
            let _ = tx.send(QueryControl::Interrupt);
        } else {
            self.push_system_message("No active request to interrupt.".to_string());
        }
    }

    #[allow(dead_code)]
    fn abort_query(&mut self) {
        if let Some(tx) = &self.chat.query_control_tx {
            let _ = tx.send(QueryControl::Abort);
        } else {
            self.push_system_message("No active request to cancel.".to_string());
        }
    }

    fn begin_session_resume(&mut self, session_id: String) {
        let session_id = session_id.trim().to_string();
        if session_id.is_empty() {
            self.push_system_message("Session id is required to resume.".to_string());
            return;
        }
        self.session.pending_resume_session = Some(session_id.clone());
        self.session.pending_fork_session = false;
        self.session.session_info.session_id = session_id.clone();
        if let Some(entry) = self.session.session_index.iter().find(|entry| entry.id == session_id) {
            self.session.session_info.model = entry.model.clone();
        }
        match self
            .session
            .restore_session(&session_id, &mut self.chat, &mut self.tools)
        {
            Ok(()) => self.push_system_message(format!(
                "Loaded cached history for session {}.",
                session_id
            )),
            Err(_) => {
                self.chat.messages.clear();
                self.push_system_message(format!(
                    "No local history for session {} yet.",
                    session_id
                ));
            }
        }
        self.session.refresh_session_cards(self.chat.is_thinking);
    }

    fn begin_session_fork(&mut self) {
        if self.session.session_info.session_id.trim().is_empty() {
            self.push_system_message("No active session to fork.".to_string());
            return;
        }
        self.session.pending_resume_session = Some(self.session.session_info.session_id.clone());
        self.session.pending_fork_session = true;
        self.push_system_message("Next message will fork the current session.".to_string());
    }

    fn export_session(&mut self) {
        if self.chat.messages.is_empty() {
            self.push_system_message("No messages to export yet.".to_string());
            return;
        }
        match export_session_markdown(self) {
            Ok(path) => self.push_system_message(format!(
                "Exported session to {}.",
                path.display()
            )),
            Err(err) => self.push_system_message(format!(
                "Failed to export session: {}.",
                err
            )),
        }
    }

    fn push_system_message(&mut self, message: String) {
        self.chat.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: message,
            document: None,
            uuid: None,
            metadata: None,
        });
    }
}

impl CoderApp {
    fn submit_prompt(&mut self, prompt: String) {
        let Some(state) = &mut self.state else {
            return;
        };

        tracing::info!("Submitted prompt: {}", prompt);

        // Add user message to history
        state.chat.messages.push(ChatMessage {
            role: MessageRole::User,
            content: prompt.clone(),
            document: None,
            uuid: None,
            metadata: None,
        });

        if matches!(state.permissions.coder_mode, CoderMode::Autopilot) {
            crate::app::autopilot::submit_autopilot_prompt(&self.runtime_handle, state, prompt);
            return;
        }

        let cwd = std::env::current_dir().unwrap_or_default();
        let active_agent = state.catalogs.active_agent.clone();
        let expanded_prompt = match expand_prompt_text(&prompt, &cwd) {
            Ok(result) => result,
            Err(err) => {
                state.push_system_message(err);
                state.window.request_redraw();
                return;
            }
        };
        let expanded_prompt = if let Some(agent) = active_agent.as_ref() {
            format!("Use the {} subagent for this request.\n\n{}", agent, expanded_prompt)
        } else {
            expanded_prompt
        };

        // Create channel for receiving responses
        let (tx, rx) = mpsc::unbounded_channel();
        let (control_tx, mut control_rx) = mpsc::unbounded_channel();
        let (permission_tx, permission_rx) = mpsc::unbounded_channel();
        let (permission_action_tx, permission_action_rx) = mpsc::unbounded_channel();
        state.chat.response_rx = Some(rx);
        state.chat.query_control_tx = Some(control_tx);
        state.permissions.permission_requests_rx = Some(permission_rx);
        state.permissions.permission_action_tx = Some(permission_action_tx.clone());
        state.permissions.permission_action_rx = Some(permission_action_rx);
        state.permissions.permission_queue.clear();
        state.permissions.permission_pending = None;
        state.permissions.permission_dialog = None;
        state.chat.is_thinking = true;
        state.chat.streaming_markdown.reset();
        state.catalogs.refresh_agent_cards(state.chat.is_thinking);

        // Get window handle for triggering redraws from async task
        let window = state.window.clone();
        let model_id = state.settings.selected_model.model_id().to_string();
        let resume_session = state.session.pending_resume_session
            .take()
            .or_else(|| {
                if state.session.session_info.session_id.trim().is_empty() {
                    None
                } else {
                    Some(state.session.session_info.session_id.clone())
                }
            });
        let fork_session = state.session.pending_fork_session;
        state.session.pending_fork_session = false;
        let permission_mode = Some(state.permissions.coder_mode.to_sdk_permission_mode());
        let output_style = state.permissions.output_style.clone();
        let allowed_tools = state.permissions.tools_allowed.clone();
        let disallowed_tools = state.permissions.tools_disallowed.clone();
        let permission_allow_tools = state.permissions.permission_allow_tools.clone();
        let permission_deny_tools = state.permissions.permission_deny_tools.clone();
        let permission_allow_bash_patterns = state.permissions.permission_allow_bash_patterns.clone();
        let permission_deny_bash_patterns = state.permissions.permission_deny_bash_patterns.clone();
        let permission_default_allow = state.permissions.permission_default_allow;
        let mcp_servers = state.catalogs.merged_mcp_servers();
        let agent_definitions = state.agent_definitions_for_query();
        let setting_sources = state.setting_sources_for_query();
        let hook_config = state.catalogs.hook_config.clone();
        let hook_scripts = state.catalogs.hook_scripts.clone();
        let max_thinking_tokens = state.settings.coder_settings.max_thinking_tokens;
        let persist_session = state.settings.coder_settings.session_auto_save;

        // Spawn async query task
        let handle = self.runtime_handle.clone();
        handle.spawn(async move {
            let hook_cwd = cwd.clone();
            let mut options = QueryOptions::new()
                .cwd(cwd)
                .include_partial_messages(true) // Enable streaming deltas
                .model(&model_id);

            options.max_thinking_tokens = max_thinking_tokens;
            options.persist_session = persist_session;

            if let Some(mode) = permission_mode.clone() {
                options = options.permission_mode(mode);
            }
            if let Some(resume_id) = resume_session {
                options = options.resume(resume_id);
            }
            if fork_session {
                options = options.fork_session(true);
            }
            if !allowed_tools.is_empty() {
                options.allowed_tools = Some(allowed_tools);
            }
            if !disallowed_tools.is_empty() {
                options.disallowed_tools = Some(disallowed_tools);
            }
            if let Some(style) = output_style {
                options
                    .extra_args
                    .insert("output-style".to_string(), Some(style));
            }
            if !mcp_servers.is_empty() {
                options.mcp_servers = mcp_servers;
            }
            if !agent_definitions.is_empty() {
                options.agents = agent_definitions;
            }
            if !setting_sources.is_empty() {
                options.setting_sources = setting_sources;
            }
            if let Some(hooks) = build_hook_map(hook_cwd, hook_config, hook_scripts, tx.clone()) {
                options = options.hooks(hooks);
            }

            let permission_window = window.clone();
            let permissions = Arc::new(CallbackPermissionHandler::new(move |request: PermissionRequest| {
                let permission_tx = permission_tx.clone();
                let permission_window = permission_window.clone();
                let permission_mode = permission_mode.clone();
                let permission_allow_tools = permission_allow_tools.clone();
                let permission_deny_tools = permission_deny_tools.clone();
                let permission_allow_bash_patterns = permission_allow_bash_patterns.clone();
                let permission_deny_bash_patterns = permission_deny_bash_patterns.clone();
                async move {
                    let tool_name = request.tool_name.clone();

                    if tool_name == "Bash" {
                        if let Some(command) = extract_bash_command(&request.input) {
                            if permission_deny_bash_patterns
                                .iter()
                                .any(|pattern| pattern_matches(pattern, &command))
                            {
                                return Ok(PermissionResult::Deny {
                                    message: format!("Bash command denied by rule: {}", command),
                                    interrupt: None,
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            if permission_allow_bash_patterns
                                .iter()
                                .any(|pattern| pattern_matches(pattern, &command))
                            {
                                return Ok(PermissionResult::Allow {
                                    updated_input: request.input.clone(),
                                    updated_permissions: request.suggestions.clone(),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                        }
                    }

                    if permission_deny_tools.iter().any(|tool| tool == &tool_name) {
                        return Ok(PermissionResult::Deny {
                            message: format!("Tool {} is denied by rule.", tool_name),
                            interrupt: None,
                            tool_use_id: Some(request.tool_use_id.clone()),
                        });
                    }
                    if permission_allow_tools.iter().any(|tool| tool == &tool_name) {
                        return Ok(PermissionResult::Allow {
                            updated_input: request.input.clone(),
                            updated_permissions: request.suggestions.clone(),
                            tool_use_id: Some(request.tool_use_id.clone()),
                        });
                    }

                    if let Some(mode) = permission_mode.as_ref() {
                        match mode {
                            PermissionMode::BypassPermissions | PermissionMode::AcceptEdits => {
                                return Ok(PermissionResult::Allow {
                                    updated_input: request.input.clone(),
                                    updated_permissions: request.suggestions.clone(),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            PermissionMode::DontAsk => {
                                return Ok(PermissionResult::Deny {
                                    message: format!("Permission denied for tool {}.", tool_name),
                                    interrupt: Some(true),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            PermissionMode::Plan => {
                                if is_read_only_tool(&tool_name) {
                                    return Ok(PermissionResult::Allow {
                                        updated_input: request.input.clone(),
                                        updated_permissions: None,
                                        tool_use_id: Some(request.tool_use_id.clone()),
                                    });
                                }
                                return Ok(PermissionResult::Deny {
                                    message: format!("Plan mode denies tool {}.", tool_name),
                                    interrupt: Some(true),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            PermissionMode::Default => {}
                        }
                    }

                    if permission_default_allow {
                        return Ok(PermissionResult::Allow {
                            updated_input: request.input.clone(),
                            updated_permissions: request.suggestions.clone(),
                            tool_use_id: Some(request.tool_use_id.clone()),
                        });
                    }

                    let (respond_to, response_rx) = oneshot::channel();
                    let pending = PermissionPending { request, respond_to };
                    if permission_tx.send(pending).is_err() {
                        return Ok(PermissionResult::deny_and_interrupt(
                            "Permission prompt unavailable.",
                        ));
                    }
                    permission_window.request_redraw();
                    match response_rx.await {
                        Ok(result) => Ok(result),
                        Err(_) => Ok(PermissionResult::deny_and_interrupt(
                            "Permission prompt interrupted.",
                        )),
                    }
                }
            }));

            tracing::info!("Starting query...");

            match query_with_permissions(&expanded_prompt, options, permissions).await {
                Ok(mut stream) => {
                    tracing::info!("Query stream started");
                    let mut interrupt_requested = false;

                    loop {
                        if interrupt_requested {
                            if let Err(e) = stream.interrupt().await {
                                tracing::error!("Interrupt failed: {}", e);
                                let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                window.request_redraw();
                                break;
                            }
                            interrupt_requested = false;
                        }

                        tokio::select! {
                            Some(control) = control_rx.recv() => {
                                match control {
                                    QueryControl::Interrupt => {
                                        interrupt_requested = true;
                                    }
                                    QueryControl::RewindFiles { user_message_id } => {
                                        match stream.rewind_files(&user_message_id).await {
                                            Ok(()) => {
                                                let _ = tx.send(ResponseEvent::SystemMessage(
                                                    "Checkpoint restore requested.".to_string(),
                                                ));
                                            }
                                            Err(err) => {
                                                let _ = tx.send(ResponseEvent::SystemMessage(
                                                    format!("Checkpoint restore failed: {}", err),
                                                ));
                                            }
                                        }
                                        window.request_redraw();
                                    }
                                    QueryControl::Abort => {
                                        if let Err(e) = stream.abort().await {
                                            tracing::error!("Abort failed: {}", e);
                                            let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                        } else {
                                            let _ = tx.send(ResponseEvent::Error("Request aborted.".to_string()));
                                        }
                                        window.request_redraw();
                                        break;
                                    }
                                    QueryControl::FetchMcpStatus => {
                                        match stream.mcp_server_status().await {
                                            Ok(value) => {
                                                match parse_mcp_status(&value) {
                                                    Ok(servers) => {
                                                        let _ = tx.send(ResponseEvent::McpStatus {
                                                            servers,
                                                            error: None,
                                                        });
                                                    }
                                                    Err(err) => {
                                                        let _ = tx.send(ResponseEvent::McpStatus {
                                                            servers: Vec::new(),
                                                            error: Some(err),
                                                        });
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                let _ = tx.send(ResponseEvent::McpStatus {
                                                    servers: Vec::new(),
                                                    error: Some(err.to_string()),
                                                });
                                            }
                                        }
                                        window.request_redraw();
                                    }
                                }
                            }
                            msg = stream.next() => {
                                match msg {
                                    Some(Ok(SdkMessage::Assistant(m))) => {
                                        // Don't extract text here - we get it from STREAM_EVENT deltas
                                        // The ASSISTANT message contains the full text which would duplicate
                                        tracing::trace!("ASSISTANT: (skipping text extraction, using stream events)");
                                        tracing::trace!("  full message: {:?}", m.message);
                                    }
                                    Some(Ok(SdkMessage::StreamEvent(e))) => {
                                        tracing::trace!("STREAM_EVENT: {:?}", e.event);
                                        // Check for tool call start
                                        if let Some((tool_name, tool_id)) = extract_tool_call_start(&e.event) {
                                            tracing::debug!("  -> tool call start: {}", tool_name);
                                            let _ = tx.send(ResponseEvent::ToolCallStart {
                                                name: tool_name,
                                                tool_use_id: tool_id,
                                            });
                                            window.request_redraw();
                                        }
                                        // Check for tool input delta
                                        else if let Some(json) = extract_tool_input_delta(&e.event) {
                                            let _ = tx.send(ResponseEvent::ToolCallInput { json });
                                            window.request_redraw();
                                        }
                                        // Check for content_block_stop (tool call end)
                                        else if e.event.get("type").and_then(|t| t.as_str()) == Some("content_block_stop") {
                                            let _ = tx.send(ResponseEvent::ToolCallEnd);
                                            window.request_redraw();
                                        }
                                        // Extract streaming text delta
                                        else if let Some(text) = extract_stream_text(&e.event) {
                                            tracing::trace!("  -> stream text: {}", text);
                                            if tx.send(ResponseEvent::Chunk(text)).is_err() {
                                                break;
                                            }
                                            window.request_redraw();
                                        }
                                    }
                                    Some(Ok(SdkMessage::System(s))) => {
                                        tracing::debug!("SYSTEM: {:?}", s);
                                        // Extract init info
                                        if let claude_agent_sdk::SdkSystemMessage::Init(init) = s {
                                    let _ = tx.send(ResponseEvent::SystemInit {
                                        model: init.model.clone(),
                                        permission_mode: init.permission_mode.clone(),
                                        session_id: init.session_id.clone(),
                                        tool_count: init.tools.len(),
                                        tools: init.tools.clone(),
                                        output_style: init.output_style.clone(),
                                        slash_commands: init.slash_commands.clone(),
                                        mcp_servers: init.mcp_servers.clone(),
                                    });
                                    window.request_redraw();
                                }
                                    }
                                    Some(Ok(SdkMessage::User(u))) => {
                                        tracing::trace!("USER message received (tool result)");
                                        if let Some(uuid) = u.uuid.clone() {
                                            let _ = tx.send(ResponseEvent::UserMessageId { uuid });
                                            window.request_redraw();
                                        }
                                        // Extract tool results from USER messages
                                        if let Some(content) = u.message.get("content").and_then(|c| c.as_array()) {
                                            for item in content {
                                                if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                                    let tool_use_id = item
                                                        .get("tool_use_id")
                                                        .or_else(|| item.get("toolUseId"))
                                                        .or_else(|| item.get("toolUseID"))
                                                        .and_then(|v| v.as_str())
                                                        .map(|v| v.to_string())
                                                        .or_else(|| u.parent_tool_use_id.clone());
                                                    let is_error = item
                                                        .get("is_error")
                                                        .or_else(|| item.get("isError"))
                                                        .and_then(|e| e.as_bool())
                                                        .unwrap_or(false);
                                                    let content_value = item.get("content").cloned().unwrap_or(Value::Null);
                                                    let (result_content, exit_code, output_value) =
                                                        tool_result_output(&content_value, u.tool_use_result.as_ref());
                                                    let _ = tx.send(ResponseEvent::ToolResult {
                                                        content: result_content,
                                                        is_error,
                                                        tool_use_id,
                                                        exit_code,
                                                        output_value,
                                                    });
                                                    window.request_redraw();
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(SdkMessage::ToolProgress(tp))) => {
                                        tracing::trace!(
                                            "TOOL_PROGRESS: {} - {:.1}s",
                                            tp.tool_name,
                                            tp.elapsed_time_seconds
                                        );
                                        let _ = tx.send(ResponseEvent::ToolProgress {
                                            tool_use_id: tp.tool_use_id.clone(),
                                            tool_name: tp.tool_name.clone(),
                                            elapsed_secs: tp.elapsed_time_seconds,
                                        });
                                        window.request_redraw();
                                    }
                                    Some(Ok(SdkMessage::AuthStatus(a))) => {
                                        tracing::debug!("AUTH_STATUS: {:?}", a);
                                    }
                                    Some(Ok(SdkMessage::Result(_r))) => {
                                        tracing::debug!("RESULT received");
                                        let _ = tx.send(ResponseEvent::Complete { metadata: None });
                                        window.request_redraw();
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        tracing::error!("ERROR: {}", e);
                                        let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                        window.request_redraw();
                                        break;
                                    }
                                    None => {
                                        let _ = tx.send(ResponseEvent::Complete { metadata: None });
                                        window.request_redraw();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    tracing::info!("Query stream ended");
                }
                Err(e) => {
                    tracing::error!("Query failed to start: {}", e);
                    let _ = tx.send(ResponseEvent::Error(e.to_string()));
                    window.request_redraw();
                }
            }
        });
    }

    fn poll_responses(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut events = Vec::new();
        if let Some(rx) = &mut state.chat.response_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        } else {
            return;
        }

        let mut needs_redraw = false;

        for event in events {
            match event {
                ResponseEvent::Chunk(text) => {
                    state.chat.streaming_markdown.append(&text);
                    state.chat.streaming_markdown.tick();
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallStart { name, tool_use_id } => {
                    let message_index = state.chat.messages.len().saturating_sub(1);
                    state
                        .tools
                        .start_tool_call(name, tool_use_id, message_index);
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallInput { json } => {
                    state.tools.current_tool_input.push_str(&json);
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallEnd => {
                    state.tools.finalize_tool_input();
                    needs_redraw = true;
                }
                ResponseEvent::ToolResult {
                    content,
                    is_error,
                    tool_use_id,
                    exit_code,
                    output_value,
                } => {
                    state.tools.apply_tool_result(
                        tool_use_id,
                        content,
                        is_error,
                        exit_code,
                        output_value,
                    );
                    needs_redraw = true;
                }
                ResponseEvent::ToolProgress {
                    tool_use_id,
                    tool_name,
                    elapsed_secs,
                } => {
                    let message_index = state.chat.messages.len().saturating_sub(1);
                    state.tools.update_tool_progress(
                        tool_use_id,
                        tool_name,
                        elapsed_secs,
                        message_index,
                    );
                    needs_redraw = true;
                }
                ResponseEvent::UserMessageId { uuid } => {
                    state.attach_user_message_id(uuid);
                    needs_redraw = true;
                }
                ResponseEvent::SystemMessage(message) => {
                    state.push_system_message(message);
                    needs_redraw = true;
                }
                ResponseEvent::Complete { metadata } => {
                    // Complete and move to messages
                    state.chat.streaming_markdown.complete();
                    let source = state.chat.streaming_markdown.source().to_string();
                    if !source.is_empty() {
                        // Aggregate into session usage
                        if let Some(ref meta) = metadata {
                            if let Some(input) = meta.input_tokens {
                                state.session.session_usage.input_tokens += input;
                            }
                            if let Some(output) = meta.output_tokens {
                                state.session.session_usage.output_tokens += output;
                            }
                            if let Some(ms) = meta.duration_ms {
                                state.session.session_usage.duration_ms += ms;
                            }
                            // Cost estimation: ~$3/M input, ~$15/M output for Claude Opus
                            let cost = (meta.input_tokens.unwrap_or(0) as f64 * 3.0 / 1_000_000.0)
                                     + (meta.output_tokens.unwrap_or(0) as f64 * 15.0 / 1_000_000.0);
                            state.session.session_usage.total_cost_usd += cost;
                        }
                        state.session.session_usage.num_turns += 1;

                        let doc = state.chat.streaming_markdown.document().clone();
                        state.chat.messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: source,
                            document: Some(doc),
                            uuid: None,
                            metadata,
                        });
                    }
                    state.chat.streaming_markdown.reset();
                    state.session.record_session(
                        &state.settings.coder_settings,
                        &state.chat.messages,
                        state.chat.is_thinking,
                    );
                    state.tools.cancel_running_tools();
                    state.chat.is_thinking = false;
                    state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    state.chat.response_rx = None;
                    state.chat.query_control_tx = None;
                    state.permissions.permission_requests_rx = None;
                    state.permissions.permission_action_tx = None;
                    state.permissions.permission_action_rx = None;
                    state.permissions.permission_dialog = None;
                    state.permissions.permission_pending = None;
                    state.permissions.permission_queue.clear();
                    state.tools.current_tool_name = None;
                    state.tools.current_tool_input.clear();
                    state.tools.current_tool_use_id = None;
                    needs_redraw = true;
                    break;
                }
                ResponseEvent::Error(e) => {
                    state.chat.messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: format!("Error: {}", e),
                        document: None,
                        uuid: None,
                        metadata: None,
                    });
                    state.chat.streaming_markdown.reset();
                    state.session.record_session(
                        &state.settings.coder_settings,
                        &state.chat.messages,
                        state.chat.is_thinking,
                    );
                    state.tools.cancel_running_tools();
                    state.chat.is_thinking = false;
                    state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    state.chat.response_rx = None;
                    state.chat.query_control_tx = None;
                    state.permissions.permission_requests_rx = None;
                    state.permissions.permission_action_tx = None;
                    state.permissions.permission_action_rx = None;
                    state.permissions.permission_dialog = None;
                    state.permissions.permission_pending = None;
                    state.permissions.permission_queue.clear();
                    state.tools.current_tool_name = None;
                    state.tools.current_tool_input.clear();
                    state.tools.current_tool_use_id = None;
                    needs_redraw = true;
                    break;
                }
                ResponseEvent::SystemInit {
                    model,
                    permission_mode,
                    session_id,
                    tool_count,
                    tools,
                    output_style,
                    slash_commands,
                    mcp_servers,
                } => {
                    state.session.session_info = SessionInfo {
                        model,
                        permission_mode,
                        session_id,
                        tool_count,
                        tools,
                        output_style,
                        slash_commands,
                    };
                    state.catalogs.update_mcp_status(mcp_servers, None);
                    if let Some(parsed_mode) = parse_coder_mode(&state.session.session_info.permission_mode)
                    {
                        state.permissions.coder_mode = parsed_mode;
                        state.permissions.permission_default_allow =
                            coder_mode_default_allow(parsed_mode, state.permissions.permission_default_allow);
                    }
                    state.session.refresh_session_cards(state.chat.is_thinking);
                    needs_redraw = true;
                }
                ResponseEvent::McpStatus { servers, error } => {
                    state.catalogs.update_mcp_status(servers, error);
                    needs_redraw = true;
                }
                ResponseEvent::HookLog(entry) => {
                    state.push_hook_log(entry);
                    needs_redraw = true;
                }
                ResponseEvent::DspyStage(stage) => {
                    let message_index = state.chat.messages.len().saturating_sub(1);
                    state.tools.push_dspy_stage(stage, message_index);
                    needs_redraw = true;
                }
            }
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_permissions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;

        let mut pending_requests = Vec::new();
        if let Some(rx) = &mut state.permissions.permission_requests_rx {
            while let Ok(pending) = rx.try_recv() {
                pending_requests.push(pending);
            }
        }
        for pending in pending_requests {
            state.permissions.enqueue_permission_prompt(pending);
            needs_redraw = true;
        }

        let mut pending_actions = Vec::new();
        if let Some(rx) = &mut state.permissions.permission_action_rx {
            while let Ok(action) = rx.try_recv() {
                pending_actions.push(action);
            }
        }
        for action in pending_actions {
            state.permissions.handle_permission_action(action);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_command_palette_actions(&mut self) {
        let actions = {
            let Some(state) = &mut self.state else {
                return;
            };
            let mut actions = Vec::new();
            if let Some(rx) = &mut state.command_palette_action_rx {
                while let Ok(action) = rx.try_recv() {
                    actions.push(action);
                }
            }
            actions
        };

        if actions.is_empty() {
            return;
        }

        for action in actions {
            if let Some(prompt) = self.execute_command_palette_action(&action) {
                self.submit_prompt(prompt);
            }
        }

        if let Some(state) = &mut self.state {
            state.window.request_redraw();
        }
    }

    fn poll_session_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;

        let mut session_events = Vec::new();
        if let Some(rx) = &mut state.session.session_action_rx {
            while let Ok(event) = rx.try_recv() {
                session_events.push(event);
            }
        }
        for event in session_events {
            state.handle_session_card_action(event.action, event.session_id);
            needs_redraw = true;
        }

        let mut checkpoint_events = Vec::new();
        if let Some(rx) = &mut state.session.checkpoint_action_rx {
            while let Ok(index) = rx.try_recv() {
                checkpoint_events.push(index);
            }
        }
        for index in checkpoint_events {
            state.handle_checkpoint_restore(index);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_agent_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;
        let mut agent_events = Vec::new();
        if let Some(rx) = &mut state.catalogs.agent_action_rx {
            while let Ok(event) = rx.try_recv() {
                agent_events.push(event);
            }
        }
        for event in agent_events {
            state.handle_agent_card_action(event.action, event.agent_id);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_skill_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;
        let mut skill_events = Vec::new();
        if let Some(rx) = &mut state.catalogs.skill_action_rx {
            while let Ok(event) = rx.try_recv() {
                skill_events.push(event);
            }
        }
        for event in skill_events {
            state.handle_skill_card_action(event.action, event.skill_id);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_hook_inspector_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;
        let mut views = Vec::new();
        if let Some(rx) = &mut state.catalogs.hook_inspector_action_rx {
            while let Ok(view) = rx.try_recv() {
                views.push(view);
            }
        }
        for view in views {
            state.catalogs.hook_inspector_view = view;
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_oanix_manifest(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Check if we received a manifest to cache
        if let Some(rx) = &mut state.autopilot.oanix_manifest_rx {
            if let Ok(manifest) = rx.try_recv() {
                tracing::info!("Autopilot: cached OANIX manifest");
                state.autopilot.oanix_manifest = Some(manifest);
                state.autopilot.oanix_manifest_rx = None; // Done receiving
            }
        }
    }

    fn poll_autopilot_history(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Check if we received updated conversation history from Adjutant
        if let Some(rx) = &mut state.autopilot.autopilot_history_rx {
            if let Ok(updated_history) = rx.try_recv() {
                tracing::info!("Autopilot: updated conversation history ({} turns)", updated_history.len());
                state.autopilot.autopilot_history = updated_history;
                state.autopilot.autopilot_history_rx = None; // Done receiving
            }
        }
    }

    fn poll_rate_limits(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Check if we received rate limits
        if let Some(rx) = &mut state.session.rate_limit_rx {
            if let Ok(limits) = rx.try_recv() {
                state.session.rate_limits = limits;
                state.session.rate_limit_rx = None; // Done receiving (one-shot)
                state.window.request_redraw();
            }
        }
    }

    fn execute_command_palette_action(&mut self, command_id: &str) -> Option<String> {
        let Some(state) = &mut self.state else {
            return None;
        };

        let command_action = match command_id {
            command_palette_ids::HELP => {
                state.open_help();
                None
            }
            command_palette_ids::SETTINGS => Some(handle_command(state, Command::Config)),
            command_palette_ids::MODEL_PICKER => Some(handle_command(state, Command::Model)),
            command_palette_ids::SESSION_LIST => Some(handle_command(state, Command::SessionList)),
            command_palette_ids::SESSION_FORK => Some(handle_command(state, Command::SessionFork)),
            command_palette_ids::SESSION_EXPORT => Some(handle_command(state, Command::SessionExport)),
            command_palette_ids::CLEAR_CONVERSATION => Some(handle_command(state, Command::Clear)),
            command_palette_ids::UNDO_LAST => Some(handle_command(state, Command::Undo)),
            command_palette_ids::COMPACT_CONTEXT => Some(handle_command(state, Command::Compact)),
            command_palette_ids::INTERRUPT_REQUEST => {
                state.interrupt_query();
                None
            }
            command_palette_ids::PERMISSION_RULES => Some(handle_command(state, Command::PermissionRules)),
            command_palette_ids::MODE_CYCLE => {
                state
                    .permissions
                    .cycle_coder_mode(&mut state.session.session_info);
                None
            }
            command_palette_ids::MODE_BYPASS => {
                state.permissions.set_coder_mode(
                    CoderMode::BypassPermissions,
                    &mut state.session.session_info,
                );
                None
            }
            command_palette_ids::MODE_PLAN => {
                state
                    .permissions
                    .set_coder_mode(CoderMode::Plan, &mut state.session.session_info);
                None
            }
            command_palette_ids::MODE_AUTOPILOT => {
                state.permissions.set_coder_mode(
                    CoderMode::Autopilot,
                    &mut state.session.session_info,
                );
                None
            }
            command_palette_ids::TOOLS_LIST => Some(handle_command(state, Command::ToolsList)),
            command_palette_ids::MCP_CONFIG => Some(handle_command(state, Command::Mcp)),
            command_palette_ids::MCP_RELOAD => Some(handle_command(state, Command::McpReload)),
            command_palette_ids::MCP_STATUS => Some(handle_command(state, Command::McpStatus)),
            command_palette_ids::AGENTS_LIST => Some(handle_command(state, Command::Agents)),
            command_palette_ids::AGENT_CLEAR => Some(handle_command(state, Command::AgentClear)),
            command_palette_ids::AGENT_RELOAD => Some(handle_command(state, Command::AgentReload)),
            command_palette_ids::SKILLS_LIST => Some(handle_command(state, Command::Skills)),
            command_palette_ids::SKILLS_RELOAD => Some(handle_command(state, Command::SkillsReload)),
            command_palette_ids::HOOKS_OPEN => Some(handle_command(state, Command::Hooks)),
            command_palette_ids::HOOKS_RELOAD => Some(handle_command(state, Command::HooksReload)),
            command_palette_ids::SIDEBAR_LEFT => {
                state.toggle_left_sidebar();
                None
            }
            command_palette_ids::SIDEBAR_RIGHT => {
                state.toggle_right_sidebar();
                None
            }
            command_palette_ids::SIDEBAR_TOGGLE => {
                state.toggle_sidebars();
                None
            }
            command_palette_ids::BUG_REPORT => Some(handle_command(state, Command::Bug)),
            command_palette_ids::KITCHEN_SINK => {
                state.show_kitchen_sink = true;
                None
            }
            _ => None,
        };

        match command_action {
            Some(CommandAction::SubmitPrompt(prompt)) => Some(prompt),
            _ => None,
        }
    }

    fn render(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };
        render_app(state);
    }

fn handle_command(state: &mut AppState, command: Command) -> CommandAction {
    match command {
        Command::Help => {
            state.open_command_palette();
            CommandAction::None
        }
        Command::Clear => {
            state.clear_conversation();
            CommandAction::None
        }
        Command::Compact => {
            if state.chat.is_thinking {
                state.push_system_message("Cannot compact during an active request.".to_string());
                CommandAction::None
            } else {
                CommandAction::SubmitPrompt("/compact".to_string())
            }
        }
        Command::Model => {
            state.open_model_picker();
            CommandAction::None
        }
        Command::Undo => {
            state.undo_last_exchange();
            CommandAction::None
        }
        Command::Cancel => {
            state.interrupt_query();
            CommandAction::None
        }
        Command::Bug => {
            match open_url(BUG_REPORT_URL) {
                Ok(()) => state.push_system_message("Opened bug report in browser.".to_string()),
                Err(err) => state.push_system_message(format!(
                    "Failed to open browser: {} (URL: {}).",
                    err, BUG_REPORT_URL
                )),
            }
            CommandAction::None
        }
        Command::SessionList => {
            state.open_session_list();
            CommandAction::None
        }
        Command::SessionResume(id) => {
            state.begin_session_resume(id);
            CommandAction::None
        }
        Command::SessionFork => {
            state.begin_session_fork();
            CommandAction::None
        }
        Command::SessionExport => {
            state.export_session();
            CommandAction::None
        }
        Command::PermissionMode(mode) => {
            match parse_coder_mode(&mode) {
                Some(parsed) => state
                    .permissions
                    .set_coder_mode(parsed, &mut state.session.session_info),
                None => state.push_system_message(format!(
                    "Unknown mode: {}. Valid modes: bypass, plan, autopilot",
                    mode
                )),
            }
            CommandAction::None
        }
        Command::PermissionRules => {
            state.open_permission_rules();
            CommandAction::None
        }
        Command::PermissionAllow(tools) => {
            let message = state.permissions.add_permission_allow(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::PermissionDeny(tools) => {
            let message = state.permissions.add_permission_deny(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::ToolsList => {
            state.open_tool_list();
            CommandAction::None
        }
        Command::ToolsEnable(tools) => {
            let message = state.permissions.enable_tools(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::ToolsDisable(tools) => {
            let message = state.permissions.disable_tools(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::Config => {
            state.open_config();
            CommandAction::None
        }
        Command::OutputStyle(style) => {
            let trimmed = style.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
                let message = state.permissions.set_output_style(None);
                state.push_system_message(message);
                return CommandAction::None;
            }

            match resolve_output_style(trimmed) {
                Ok(Some(_path)) => {
                    let message = state
                        .permissions
                        .set_output_style(Some(trimmed.to_string()));
                    state.push_system_message(message);
                }
                Ok(None) => state.push_system_message(format!(
                    "Output style not found: {}.",
                    trimmed
                )),
                Err(err) => state.push_system_message(format!(
                    "Failed to load output style: {}.",
                    err
                )),
            }
            CommandAction::None
        }
        Command::Mcp => {
            state.open_mcp_config();
            CommandAction::None
        }
        Command::McpReload => {
            state.reload_mcp_project_servers();
            if let Some(err) = &state.catalogs.mcp_project_error {
                state.push_system_message(format!("MCP config reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded MCP project config.".to_string());
            }
            CommandAction::None
        }
        Command::McpStatus => {
            state.request_mcp_status();
            CommandAction::None
        }
        Command::McpAdd { name, config } => {
            let trimmed_name = name.trim();
            if trimmed_name.is_empty() {
                state.push_system_message("MCP add requires a server name.".to_string());
                return CommandAction::None;
            }
            let config_text = config.trim();
            if config_text.is_empty() {
                state.push_system_message("MCP add requires a JSON config.".to_string());
                return CommandAction::None;
            }
            match serde_json::from_str::<Value>(config_text) {
                Ok(value) => {
                    let expanded = expand_env_vars_in_value(&value);
                    match parse_mcp_server_config(trimmed_name, &expanded) {
                        Ok(server) => {
                            state.catalogs.add_runtime_mcp_server(trimmed_name.to_string(), server);
                            state.push_system_message(format!(
                                "Added MCP server {} (applies next request).",
                                trimmed_name
                            ));
                        }
                        Err(err) => state.push_system_message(format!(
                            "Failed to add MCP server {}: {}",
                            trimmed_name, err
                        )),
                    }
                }
                Err(err) => state.push_system_message(format!(
                    "Failed to parse MCP server JSON: {}",
                    err
                )),
            }
            CommandAction::None
        }
        Command::McpRemove(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                state.push_system_message("MCP remove requires a server name.".to_string());
                return CommandAction::None;
            }
            state.catalogs.remove_mcp_server(trimmed);
            state.push_system_message(format!(
                "Disabled MCP server {} (applies next request).",
                trimmed
            ));
            CommandAction::None
        }
        Command::Agents => {
            state.open_agent_list();
            CommandAction::None
        }
        Command::AgentSelect(name) => {
            state.set_active_agent_by_name(&name);
            CommandAction::None
        }
        Command::AgentClear => {
            state.clear_active_agent();
            CommandAction::None
        }
        Command::AgentReload => {
            state.reload_agents();
            if let Some(err) = &state.catalogs.agent_load_error {
                state.push_system_message(format!("Agent reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded agents from disk.".to_string());
            }
            CommandAction::None
        }
        Command::Skills => {
            state.open_skill_list();
            CommandAction::None
        }
        Command::SkillsReload => {
            state.reload_skills();
            if let Some(err) = &state.catalogs.skill_load_error {
                state.push_system_message(format!("Skill reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded skills from disk.".to_string());
            }
            CommandAction::None
        }
        Command::Hooks => {
            state.open_hooks();
            CommandAction::None
        }
        Command::HooksReload => {
            state.reload_hooks();
            if let Some(err) = &state.catalogs.hook_load_error {
                state.push_system_message(format!("Hook reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded hook scripts from disk.".to_string());
            }
            CommandAction::None
        }
        Command::Custom(name, args) => {
            if state.chat.is_thinking {
                state.push_system_message(
                    "Cannot run custom commands during an active request.".to_string(),
                );
                return CommandAction::None;
            }

            match load_custom_command(&name) {
                Ok(Some(template)) => {
                    let prompt = apply_custom_command_args(&template, &args);
                    CommandAction::SubmitPrompt(prompt)
                }
                Ok(None) => {
                    let mut message = format!("Unknown command: /{}", name);
                    if !args.is_empty() {
                        message.push(' ');
                        message.push_str(&args.join(" "));
                    }
                    state.push_system_message(message);
                    CommandAction::None
                }
                Err(err) => {
                    state.push_system_message(format!(
                        "Failed to load custom command /{}: {}.",
                        name, err
                    ));
                    CommandAction::None
                }
            }
        }
    }
}

fn handle_modal_input(state: &mut AppState, key: &WinitKey) -> bool {
    let empty_entries: Vec<McpServerEntry> = Vec::new();
    let mcp_entries = if matches!(state.modal_state, ModalState::McpConfig { .. }) {
        Some(state.catalogs.mcp_entries())
    } else {
        None
    };
    let settings_snapshot = SettingsSnapshot::from_state(state);
    match &mut state.modal_state {
        ModalState::ModelPicker { selected } => {
            let selected = *selected;
            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    let models = ModelOption::all();
                    state.update_selected_model(models[selected]);
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if selected > 0 {
                        state.modal_state = ModalState::ModelPicker { selected: selected - 1 };
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if selected + 1 < ModelOption::all().len() {
                        state.modal_state = ModalState::ModelPicker { selected: selected + 1 };
                    }
                }
                WinitKey::Character(c) => {
                    match c.as_str() {
                        "1" => {
                            state.settings.selected_model = ModelOption::Opus;
                        }
                        "2" => {
                            state.settings.selected_model = ModelOption::Sonnet;
                        }
                        "3" => {
                            state.settings.selected_model = ModelOption::Haiku;
                        }
                        _ => {}
                    }
                    if matches!(c.as_str(), "1" | "2" | "3") {
                        state.update_selected_model(state.settings.selected_model);
                        state.modal_state = ModalState::None;
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::SessionList { selected } => {
            let session_count = state.session.session_index.len();
            if session_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= session_count {
                *selected = session_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    if let Some(entry) = state.session.session_index.get(*selected).cloned() {
                        state.begin_session_resume(entry.id);
                    }
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < session_count {
                        *selected += 1;
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::AgentList { selected } => {
            let agent_count = state.catalogs.agent_entries.len();
            if agent_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.reload_agents();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= agent_count {
                *selected = agent_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    let selected_name = state.catalogs.agent_entries
                        .get(*selected)
                        .map(|entry| entry.name.clone());
                    if let Some(name) = selected_name {
                        state.set_active_agent_by_name(&name);
                    }
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < agent_count {
                        *selected += 1;
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.reload_agents();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::SkillList { selected } => {
            let skill_count = state.catalogs.skill_entries.len();
            if skill_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.reload_skills();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= skill_count {
                *selected = skill_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < skill_count {
                        *selected += 1;
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.reload_skills();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Hooks { view, selected } => {
            let mut sync_index = None;
            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Tab) => {
                    *view = match *view {
                        HookModalView::Config => HookModalView::Events,
                        HookModalView::Events => HookModalView::Config,
                    };
                    if *view == HookModalView::Events {
                        *selected = 0;
                        sync_index = Some(*selected);
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.reload_hooks();
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("c") => {
                    if *view == HookModalView::Events {
                        state.clear_hook_log();
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *view == HookModalView::Events && !state.catalogs.hook_event_log.is_empty() {
                        if *selected > 0 {
                            *selected -= 1;
                            sync_index = Some(*selected);
                        }
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *view == HookModalView::Events && !state.catalogs.hook_event_log.is_empty() {
                        if *selected + 1 < state.catalogs.hook_event_log.len() {
                            *selected += 1;
                            sync_index = Some(*selected);
                        }
                    }
                }
                WinitKey::Character(c) if *view == HookModalView::Config => match c.as_str() {
                    "1" => state.toggle_hook_setting(HookSetting::ToolBlocker),
                    "2" => state.toggle_hook_setting(HookSetting::ToolLogger),
                    "3" => state.toggle_hook_setting(HookSetting::OutputTruncator),
                    "4" => state.toggle_hook_setting(HookSetting::ContextInjection),
                    "5" => state.toggle_hook_setting(HookSetting::TodoEnforcer),
                    _ => {}
                },
                _ => {}
            }
            if let Some(index) = sync_index {
                state.sync_hook_inspector(index);
            }
            state.window.request_redraw();
            true
        }
        ModalState::ToolList { selected } => {
            let tool_count = state.session.session_info.tools.len();
            if tool_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= tool_count {
                *selected = tool_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < tool_count {
                        *selected += 1;
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::McpConfig { selected } => {
            let entries = mcp_entries.as_ref().unwrap_or(&empty_entries);
            if entries.is_empty() {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.reload_mcp_project_servers();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= entries.len() {
                *selected = entries.len() - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < entries.len() {
                        *selected += 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::Delete | WinitNamedKey::Backspace) => {
                    if let Some(entry) = entries.get(*selected) {
                        state.catalogs.remove_mcp_server(&entry.name);
                    }
                }
                WinitKey::Character(c) => match c.as_str() {
                    "r" | "R" => {
                        state.reload_mcp_project_servers();
                    }
                    "s" | "S" => {
                        state.request_mcp_status();
                    }
                    _ => {}
                },
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::PermissionRules => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Help => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter | WinitNamedKey::F1) => {
                    state.modal_state = ModalState::None;
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Config {
            tab,
            selected,
            search,
            input_mode,
        } => {
            let rows = settings_rows(&settings_snapshot, *tab, search);
            if rows.is_empty() {
                *selected = 0;
            } else if *selected >= rows.len() {
                *selected = rows.len().saturating_sub(1);
            }
            let current_item = rows.get(*selected).map(|row| row.item);
            let shift = state.modifiers.shift_key();
            let ctrl = state.modifiers.control_key();

            let mut change_tab = |forward: bool| {
                let tabs = SettingsTab::all();
                let current_index = tabs.iter().position(|entry| entry == tab).unwrap_or(0);
                let next_index = if forward {
                    (current_index + 1) % tabs.len()
                } else {
                    (current_index + tabs.len() - 1) % tabs.len()
                };
                *tab = tabs[next_index];
                *selected = 0;
            };

            match input_mode {
                SettingsInputMode::Search => match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        *input_mode = SettingsInputMode::Normal;
                    }
                    WinitKey::Named(WinitNamedKey::Backspace) => {
                        search.pop();
                        *selected = 0;
                    }
                    WinitKey::Character(c) => {
                        search.push_str(c.as_str());
                        *selected = 0;
                    }
                    WinitKey::Named(WinitNamedKey::Tab) => {
                        *input_mode = SettingsInputMode::Normal;
                        change_tab(!shift);
                    }
                    _ => {}
                },
                SettingsInputMode::Capture(action) => match key {
                    WinitKey::Named(WinitNamedKey::Escape) => {
                        *input_mode = SettingsInputMode::Normal;
                    }
                    WinitKey::Named(WinitNamedKey::Backspace | WinitNamedKey::Delete) => {
                        state.settings.keybindings.retain(|binding| binding.action != *action);
                        save_keybindings(&state.settings.keybindings);
                        *input_mode = SettingsInputMode::Normal;
                    }
                    _ => {
                        if let Some(binding_key) = convert_key_for_binding(key) {
                            let modifiers = convert_modifiers(&state.modifiers);
                            state.settings.keybindings.retain(|binding| {
                                binding.action != *action
                                    && !(binding.key == binding_key && binding.modifiers == modifiers)
                            });
                            state.settings.keybindings.push(Keybinding {
                                key: binding_key,
                                modifiers,
                                action: *action,
                            });
                            save_keybindings(&state.settings.keybindings);
                        }
                        *input_mode = SettingsInputMode::Normal;
                    }
                },
                SettingsInputMode::Normal => match key {
                    WinitKey::Named(WinitNamedKey::Escape) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Named(WinitNamedKey::Tab) => {
                        change_tab(!shift);
                    }
                    WinitKey::Named(WinitNamedKey::ArrowUp) => {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                    WinitKey::Named(WinitNamedKey::ArrowDown) => {
                        if *selected + 1 < rows.len() {
                            *selected += 1;
                        }
                    }
                    WinitKey::Character(c) if (ctrl && c.eq_ignore_ascii_case("f")) || c == "/" => {
                        *input_mode = SettingsInputMode::Search;
                    }
                    WinitKey::Named(WinitNamedKey::ArrowLeft)
                    | WinitKey::Named(WinitNamedKey::ArrowRight)
                    | WinitKey::Named(WinitNamedKey::Enter) => {
                        let forward = !matches!(key, WinitKey::Named(WinitNamedKey::ArrowLeft));
                        if let Some(item) = current_item {
                            match item {
                                SettingsItem::Theme => {
                                    state.settings.coder_settings.theme = if state.settings.coder_settings.theme == ThemeSetting::Dark {
                                        ThemeSetting::Light
                                    } else {
                                        ThemeSetting::Dark
                                    };
                                    state.apply_settings();
                                    state.persist_settings();
                                }
                                SettingsItem::FontSize => {
                                    let delta = if forward { 1.0 } else { -1.0 };
                                    state.settings.coder_settings.font_size =
                                        clamp_font_size(state.settings.coder_settings.font_size + delta);
                                    state.apply_settings();
                                    state.persist_settings();
                                }
                                SettingsItem::AutoScroll => {
                                    state.settings.coder_settings.auto_scroll = !state.settings.coder_settings.auto_scroll;
                                    state.persist_settings();
                                }
                                SettingsItem::DefaultModel => {
                                    let next = cycle_model(state.settings.selected_model, forward);
                                    state.update_selected_model(next);
                                }
                                SettingsItem::MaxThinkingTokens => {
                                    const THINKING_STEP: u32 = 256;
                                    const THINKING_MAX: u32 = 8192;
                                    let current = state.settings.coder_settings.max_thinking_tokens.unwrap_or(0);
                                    let next = if forward {
                                        let value = current.saturating_add(THINKING_STEP).min(THINKING_MAX);
                                        Some(value)
                                    } else if current <= THINKING_STEP {
                                        None
                                    } else {
                                        Some(current - THINKING_STEP)
                                    };
                                    state.settings.coder_settings.max_thinking_tokens = next;
                                    state.persist_settings();
                                }
                                SettingsItem::PermissionMode => {
                                    let next = cycle_coder_mode_standalone(state.permissions.coder_mode, forward);
                                    state.permissions.coder_mode = next;
                                    state.permissions.permission_default_allow =
                                        coder_mode_default_allow(next, state.permissions.permission_default_allow);
                                    state.session.session_info.permission_mode =
                                        coder_mode_label(next).to_string();
                                    state.permissions.persist_permission_config();
                                }
                                SettingsItem::PermissionDefaultAllow => {
                                    state.permissions.permission_default_allow = !state.permissions.permission_default_allow;
                                    state.permissions.persist_permission_config();
                                }
                                SettingsItem::PermissionRules
                                | SettingsItem::PermissionAllowList
                                | SettingsItem::PermissionDenyList
                                | SettingsItem::PermissionBashAllowList
                                | SettingsItem::PermissionBashDenyList => {
                                    state.open_permission_rules();
                                }
                                SettingsItem::SessionAutoSave => {
                                    state.settings.coder_settings.session_auto_save = !state.settings.coder_settings.session_auto_save;
                                    state.persist_settings();
                                    if state.settings.coder_settings.session_auto_save {
                                        state.apply_session_history_limit();
                                    }
                                }
                                SettingsItem::SessionHistoryLimit => {
                                    const HISTORY_STEP: usize = 10;
                                    const HISTORY_MAX: usize = 500;
                                    let current = state.settings.coder_settings.session_history_limit;
                                    let next = if forward {
                                        if current == 0 {
                                            HISTORY_STEP
                                        } else {
                                            (current + HISTORY_STEP).min(HISTORY_MAX)
                                        }
                                    } else if current <= HISTORY_STEP {
                                        0
                                    } else {
                                        current - HISTORY_STEP
                                    };
                                    state.settings.coder_settings.session_history_limit = next;
                                    state.persist_settings();
                                    state.apply_session_history_limit();
                                }
                                SettingsItem::SessionStoragePath | SettingsItem::McpSummary => {}
                                SettingsItem::McpOpenConfig => {
                                    state.open_mcp_config();
                                }
                                SettingsItem::McpReloadProject => {
                                    state.reload_mcp_project_servers();
                                    if let Some(err) = &state.catalogs.mcp_project_error {
                                        state.push_system_message(format!(
                                            "MCP reload warning: {}",
                                            err
                                        ));
                                    } else {
                                        state.push_system_message(
                                            "Reloaded MCP project config.".to_string(),
                                        );
                                    }
                                }
                                SettingsItem::McpRefreshStatus => {
                                    state.request_mcp_status();
                                }
                                SettingsItem::HookToolBlocker => {
                                    state.toggle_hook_setting(HookSetting::ToolBlocker);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookToolLogger => {
                                    state.toggle_hook_setting(HookSetting::ToolLogger);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookOutputTruncator => {
                                    state.toggle_hook_setting(HookSetting::OutputTruncator);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookContextInjection => {
                                    state.toggle_hook_setting(HookSetting::ContextInjection);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookTodoEnforcer => {
                                    state.toggle_hook_setting(HookSetting::TodoEnforcer);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookOpenPanel => {
                                    state.open_hooks();
                                }
                                SettingsItem::Keybinding(action) => {
                                    *input_mode = SettingsInputMode::Capture(action);
                                }
                                SettingsItem::KeybindingReset => {
                                    state.settings.keybindings = default_keybindings();
                                    save_keybindings(&state.settings.keybindings);
                                }
                            }
                        }
                    }
                    _ => {}
                },
            }
            state.window.request_redraw();
            true
        }
        ModalState::None => false,
    }
}

fn cycle_model(current: ModelOption, forward: bool) -> ModelOption {
    let models = ModelOption::all();
    let idx = models
        .iter()
        .position(|model| *model == current)
        .unwrap_or(0);
    let next = if forward {
        (idx + 1) % models.len()
    } else {
        (idx + models.len() - 1) % models.len()
    };
    models[next]
}

fn cycle_coder_mode_standalone(current: CoderMode, forward: bool) -> CoderMode {
    let modes = [
        CoderMode::BypassPermissions,
        CoderMode::Plan,
        CoderMode::Autopilot,
    ];
    let idx = match current {
        CoderMode::BypassPermissions => 0,
        CoderMode::Plan => 1,
        CoderMode::Autopilot => 2,
    };
    let next = if forward {
        (idx + 1) % modes.len()
    } else {
        (idx + modes.len() - 1) % modes.len()
    };
    modes[next]
}

fn hook_event_from_input(input: &HookInput) -> HookEvent {
    match input {
        HookInput::PreToolUse(_) => HookEvent::PreToolUse,
        HookInput::PostToolUse(_) => HookEvent::PostToolUse,
        HookInput::PostToolUseFailure(_) => HookEvent::PostToolUseFailure,
        HookInput::Notification(_) => HookEvent::Notification,
        HookInput::UserPromptSubmit(_) => HookEvent::UserPromptSubmit,
        HookInput::SessionStart(_) => HookEvent::SessionStart,
        HookInput::SessionEnd(_) => HookEvent::SessionEnd,
        HookInput::Stop(_) => HookEvent::Stop,
        HookInput::SubagentStart(_) => HookEvent::SubagentStart,
        HookInput::SubagentStop(_) => HookEvent::SubagentStop,
        HookInput::PreCompact(_) => HookEvent::PreCompact,
        HookInput::PermissionRequest(_) => HookEvent::PermissionRequest,
    }
}

fn hook_base_input(input: &HookInput) -> &BaseHookInput {
    match input {
        HookInput::PreToolUse(hook) => &hook.base,
        HookInput::PostToolUse(hook) => &hook.base,
        HookInput::PostToolUseFailure(hook) => &hook.base,
        HookInput::Notification(hook) => &hook.base,
        HookInput::UserPromptSubmit(hook) => &hook.base,
        HookInput::SessionStart(hook) => &hook.base,
        HookInput::SessionEnd(hook) => &hook.base,
        HookInput::Stop(hook) => &hook.base,
        HookInput::SubagentStart(hook) => &hook.base,
        HookInput::SubagentStop(hook) => &hook.base,
        HookInput::PreCompact(hook) => &hook.base,
        HookInput::PermissionRequest(hook) => &hook.base,
    }
}

fn hook_tool_name(input: &HookInput) -> Option<String> {
    match input {
        HookInput::PreToolUse(hook) => Some(hook.tool_name.clone()),
        HookInput::PostToolUse(hook) => Some(hook.tool_name.clone()),
        HookInput::PostToolUseFailure(hook) => Some(hook.tool_name.clone()),
        HookInput::PermissionRequest(hook) => Some(hook.tool_name.clone()),
        _ => None,
    }
}

fn hook_tool_input(input: &HookInput) -> Option<&Value> {
    match input {
        HookInput::PreToolUse(hook) => Some(&hook.tool_input),
        HookInput::PostToolUse(hook) => Some(&hook.tool_input),
        HookInput::PostToolUseFailure(hook) => Some(&hook.tool_input),
        HookInput::PermissionRequest(hook) => Some(&hook.tool_input),
        _ => None,
    }
}

fn hook_tool_response(input: &HookInput) -> Option<&Value> {
    match input {
        HookInput::PostToolUse(hook) => Some(&hook.tool_response),
        _ => None,
    }
}

fn hook_tool_error(input: &HookInput) -> Option<&str> {
    match input {
        HookInput::PostToolUseFailure(hook) => Some(hook.error.as_str()),
        _ => None,
    }
}

fn hook_tool_blocker(input: &HookInput) -> (HookOutput, String) {
    let tool_name = hook_tool_name(input).unwrap_or_else(|| "unknown".to_string());
    let mut summary = format!("ToolBlocker allowed {}.", tool_name);
    let mut sync = SyncHookOutput::continue_execution();

    let is_bash = tool_name.eq_ignore_ascii_case("bash");
    if !is_bash {
        return (HookOutput::Sync(sync), summary);
    }

    let Some(tool_input) = hook_tool_input(input) else {
        return (HookOutput::Sync(sync), summary);
    };
    let Some(command) = extract_bash_command(tool_input) else {
        return (HookOutput::Sync(sync), summary);
    };

    let lowered = command.to_ascii_lowercase();
    for pattern in HOOK_BLOCK_PATTERNS {
        if lowered.contains(&pattern.to_ascii_lowercase()) {
            let reason = format!(
                "Blocked dangerous command: {}",
                truncate_preview(&command, 160)
            );
            sync = SyncHookOutput {
                continue_execution: Some(false),
                decision: Some(HookDecision::Block),
                reason: Some(reason),
                ..Default::default()
            };
            summary = format!("ToolBlocker blocked {}.", tool_name);
            break;
        }
    }

    (HookOutput::Sync(sync), summary)
}

fn hook_tool_logger_summary(input: &HookInput) -> String {
    let tool_name = hook_tool_name(input).unwrap_or_else(|| "unknown".to_string());
    match hook_event_from_input(input) {
        HookEvent::PreToolUse => format!("ToolLogger pre {}.", tool_name),
        HookEvent::PostToolUse => format!("ToolLogger post {}.", tool_name),
        HookEvent::PostToolUseFailure => {
            if let Some(error) = hook_tool_error(input) {
                format!(
                    "ToolLogger failure {}: {}",
                    tool_name,
                    truncate_preview(error, 120)
                )
            } else {
                format!("ToolLogger failure {}.", tool_name)
            }
        }
        event => format!("ToolLogger {}.", hook_event_label(event)),
    }
}

fn hook_output_truncator(input: &HookInput) -> (HookOutput, String) {
    let tool_name = hook_tool_name(input).unwrap_or_else(|| "unknown".to_string());
    let Some(tool_response) = hook_tool_response(input) else {
        return (
            HookOutput::Sync(SyncHookOutput::continue_execution()),
            format!("OutputTruncator skipped {}.", tool_name),
        );
    };

    let response_text =
        serde_json::to_string(tool_response).unwrap_or_else(|_| tool_response.to_string());
    let response_len = response_text.len();
    if response_len <= HOOK_OUTPUT_TRUNCATE {
        return (
            HookOutput::Sync(SyncHookOutput::continue_execution()),
            format!("OutputTruncator ok for {}.", tool_name),
        );
    }

    let truncated = truncate_bytes(response_text, HOOK_OUTPUT_TRUNCATE);
    let mut sync = SyncHookOutput::continue_execution();
    sync.suppress_output = Some(true);
    sync.hook_specific_output = Some(HookSpecificOutput::PostToolUse(
        PostToolUseSpecificOutput {
            hook_event_name: HookEvent::PostToolUse.as_str().to_string(),
            additional_context: Some(format!(
                "Tool output truncated ({} bytes):\n{}",
                response_len, truncated
            )),
            updated_mcp_tool_output: None,
        },
    ));

    (
        HookOutput::Sync(sync),
        format!("OutputTruncator truncated {} output.", tool_name),
    )
}

fn hook_context_sources(config: &HookConfig) -> Vec<String> {
    let mut sources = Vec::new();
    if config.context_injection {
        sources.push("builtin:context_injection".to_string());
    }
    if config.todo_enforcer {
        sources.push("builtin:todo_enforcer".to_string());
    }
    sources
}

fn hook_context_enforcer(
    runtime: &HookRuntimeConfig,
    input: &HookInput,
) -> (HookOutput, String) {
    let event = hook_event_from_input(input);
    let mut sections = Vec::new();

    if runtime.config.context_injection {
        if let Some(context) = build_context_injection(&runtime.cwd) {
            sections.push(context);
        }
    }
    if runtime.config.todo_enforcer {
        if let Some(todo) = build_todo_context(&runtime.cwd) {
            sections.push(todo);
        }
    }

    if sections.is_empty() {
        return (
            HookOutput::Sync(SyncHookOutput::continue_execution()),
            "ContextEnforcer no context.".to_string(),
        );
    }

    let combined = sections.join("\n\n");
    let combined_len = combined.len();
    let hook_specific_output = match event {
        HookEvent::UserPromptSubmit => HookSpecificOutput::UserPromptSubmit(
            UserPromptSubmitSpecificOutput {
                hook_event_name: HookEvent::UserPromptSubmit.as_str().to_string(),
                additional_context: Some(combined),
            },
        ),
        HookEvent::SessionStart => HookSpecificOutput::SessionStart(SessionStartSpecificOutput {
            hook_event_name: HookEvent::SessionStart.as_str().to_string(),
            additional_context: Some(combined),
        }),
        _ => {
            return (
                HookOutput::Sync(SyncHookOutput::continue_execution()),
                "ContextEnforcer skipped.".to_string(),
            )
        }
    };

    let mut sync = SyncHookOutput::continue_execution();
    sync.hook_specific_output = Some(hook_specific_output);
    (
        HookOutput::Sync(sync),
        format!("ContextEnforcer injected {} bytes.", combined_len),
    )
}

fn hook_script_source_label(entry: &HookScriptEntry) -> String {
    let source = match entry.source {
        HookScriptSource::Project => "project",
        HookScriptSource::User => "user",
    };
    format!("script:{}:{}", source, entry.path.display())
}

fn hook_script_env(input: &HookInput, tool_use_id: Option<&str>) -> Vec<(String, String)> {
    let base = hook_base_input(input);
    let event = hook_event_from_input(input);
    let mut envs = vec![
        (
            "CLAUDE_HOOK_EVENT".to_string(),
            hook_event_label(event).to_string(),
        ),
        ("CLAUDE_SESSION_ID".to_string(), base.session_id.clone()),
        (
            "CLAUDE_TRANSCRIPT_PATH".to_string(),
            base.transcript_path.clone(),
        ),
        ("CLAUDE_CWD".to_string(), base.cwd.clone()),
    ];
    if let Some(mode) = &base.permission_mode {
        envs.push(("CLAUDE_PERMISSION_MODE".to_string(), mode.clone()));
    }
    if let Some(tool_name) = hook_tool_name(input) {
        envs.push(("CLAUDE_TOOL_NAME".to_string(), tool_name));
    }
    if let Some(tool_use_id) = tool_use_id {
        envs.push(("CLAUDE_TOOL_USE_ID".to_string(), tool_use_id.to_string()));
    }
    envs
}

async fn run_hook_script(
    entry: &HookScriptEntry,
    input: &HookInput,
    tool_use_id: Option<&str>,
    runtime: &HookRuntimeConfig,
) -> Result<HookOutput, String> {
    let mut command = TokioCommand::new(&entry.path);
    command
        .current_dir(&runtime.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in hook_script_env(input, tool_use_id) {
        command.env(key, value);
    }

    let mut child = command.spawn().map_err(|err| {
        format!(
            "Failed to spawn hook script {}: {}",
            entry.path.display(),
            err
        )
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(input)
            .map_err(|err| format!("Failed to serialize hook input: {}", err))?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|err| format!("Failed to write hook input: {}", err))?;
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(mut stdout) = stdout {
            let _ = stdout.read_to_end(&mut buffer).await;
        }
        buffer
    });
    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_end(&mut buffer).await;
        }
        buffer
    });

    let status = match timeout(Duration::from_secs(HOOK_SCRIPT_TIMEOUT_SECS), child.wait()).await {
        Ok(status) => status.map_err(|err| format!("Hook script failed: {}", err))?,
        Err(_) => {
            let _ = child.kill().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(format!(
                "Hook script {} timed out after {}s.",
                entry.path.display(),
                HOOK_SCRIPT_TIMEOUT_SECS
            ));
        }
    };

    let stdout_bytes = stdout_task
        .await
        .unwrap_or_default();
    let stderr_bytes = stderr_task
        .await
        .unwrap_or_default();

    let stdout_text = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();

    if !status.success() {
        let mut message = format!("Hook script exited with status {}", status);
        if !stderr_text.is_empty() {
            message.push_str(": ");
            message.push_str(&stderr_text);
        }
        return Err(message);
    }

    if stdout_text.is_empty() {
        return Ok(HookOutput::Sync(SyncHookOutput::continue_execution()));
    }

    serde_json::from_str::<HookOutput>(&stdout_text).map_err(|err| {
        format!(
            "Failed to parse hook output: {} (stdout: {})",
            err,
            truncate_preview(&stdout_text, 160)
        )
    })
}

fn truncate_hook_value(value: Value, max_bytes: usize) -> Value {
    match value {
        Value::String(text) => {
            if text.len() <= max_bytes {
                Value::String(text)
            } else {
                Value::String(truncate_bytes(text, max_bytes))
            }
        }
        other => {
            let raw = serde_json::to_string(&other).unwrap_or_else(|_| other.to_string());
            if raw.len() <= max_bytes {
                other
            } else {
                Value::String(truncate_bytes(raw, max_bytes))
            }
        }
    }
}

fn serialize_hook_value<T: Serialize>(value: &T, max_bytes: usize) -> Value {
    let serialized = serde_json::to_value(value).unwrap_or(Value::Null);
    truncate_hook_value(serialized, max_bytes)
}

fn log_hook_event(
    runtime: &HookRuntimeConfig,
    event: HookEvent,
    summary: String,
    tool_name: Option<String>,
    matcher: Option<String>,
    input: &HookInput,
    output: Option<&HookOutput>,
    error: Option<String>,
    sources: Vec<String>,
) {
    let id = format!(
        "hook-{}-{}",
        hook_event_label(event).to_ascii_lowercase(),
        runtime.counter.fetch_add(1, Ordering::SeqCst)
    );
    let entry = HookLogEntry {
        id,
        event,
        timestamp: now_timestamp(),
        summary,
        tool_name,
        matcher,
        input: serialize_hook_value(input, HOOK_OUTPUT_TRUNCATE),
        output: output.map(|value| serialize_hook_value(value, HOOK_OUTPUT_TRUNCATE)),
        error,
        sources,
    };
    let _ = runtime.log_tx.send(ResponseEvent::HookLog(entry));
}

fn hook_event_kind(event: HookEvent) -> u32 {
    match event {
        HookEvent::PreToolUse => 61001,
        HookEvent::PostToolUse => 61002,
        HookEvent::PostToolUseFailure => 61003,
        HookEvent::Notification => 61004,
        HookEvent::UserPromptSubmit => 61005,
        HookEvent::SessionStart => 61006,
        HookEvent::SessionEnd => 61007,
        HookEvent::Stop => 61008,
        HookEvent::SubagentStart => 61009,
        HookEvent::SubagentStop => 61010,
        HookEvent::PreCompact => 61011,
        HookEvent::PermissionRequest => 61012,
    }
}

fn value_preview(value: &Value, max_chars: usize) -> String {
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    truncate_preview(&text, max_chars)
}

fn hook_log_event_data(entry: &HookLogEntry) -> EventData {
    let mut tags = Vec::new();
    tags.push(TagData::new(
        "event",
        vec![hook_event_label(entry.event).to_string()],
    ));
    if let Some(tool) = &entry.tool_name {
        tags.push(TagData::new("tool", vec![tool.clone()]));
    }
    if let Some(matcher) = &entry.matcher {
        tags.push(TagData::new("matcher", vec![matcher.clone()]));
    }
    if !entry.sources.is_empty() {
        tags.push(TagData::new("sources", entry.sources.clone()));
    }
    if let Some(error) = &entry.error {
        tags.push(TagData::new("error", vec![error.clone()]));
    }
    tags.push(TagData::new(
        "input",
        vec![value_preview(&entry.input, 180)],
    ));
    if let Some(output) = &entry.output {
        tags.push(TagData::new(
            "output",
            vec![value_preview(output, 180)],
        ));
    }

    let mut content = entry.summary.clone();
    if let Some(error) = &entry.error {
        if !error.trim().is_empty() {
            content.push_str("\n");
            content.push_str(error);
        }
    }

    EventData::new(&entry.id, "hooks", hook_event_kind(entry.event))
        .content(content)
        .created_at(entry.timestamp)
        .tags(tags)
        .sig("")
        .verified(false)
}

fn resolve_output_style(name: &str) -> io::Result<Option<PathBuf>> {
    if name.trim().is_empty() {
        return Ok(None);
    }

    let file_name = if name.ends_with(".md") {
        name.to_string()
    } else {
        format!("{}.md", name)
    };

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(".claude").join("output-styles").join(&file_name));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".claude").join("output-styles").join(&file_name));
    }

    for path in candidates {
        if path.is_file() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn resolve_custom_command_path(name: &str) -> io::Result<Option<PathBuf>> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let file_name = if trimmed.ends_with(".md") {
        trimmed.to_string()
    } else {
        format!("{}.md", trimmed)
    };

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(".claude").join("commands").join(&file_name));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".claude").join("commands").join(&file_name));
    }

    for path in candidates {
        if path.is_file() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn load_custom_command(name: &str) -> io::Result<Option<String>> {
    let Some(path) = resolve_custom_command_path(name)? else {
        return Ok(None);
    };
    let content = fs::read_to_string(path)?;
    Ok(Some(content))
}

fn apply_custom_command_args(template: &str, args: &[String]) -> String {
    if args.is_empty() {
        return template.to_string();
    }
    let joined = args.join(" ");
    if template.contains("{{args}}") {
        template.replace("{{args}}", &joined)
    } else {
        format!("{}\n\n{}", template.trim_end(), joined)
    }
}

fn export_session_markdown(state: &AppState) -> io::Result<PathBuf> {
    let export_dir = config_dir().join("exports");
    fs::create_dir_all(&export_dir)?;
    let session_id = if state.session.session_info.session_id.is_empty() {
        "session".to_string()
    } else {
        state.session.session_info.session_id.clone()
    };
    let filename = format!("{}-{}.md", session_id, now_timestamp());
    let path = export_dir.join(filename);
    let mut file = fs::File::create(&path)?;

    writeln!(file, "# Coder Session {}", session_id)?;
    if !state.session.session_info.model.is_empty() {
        writeln!(file, "- Model: {}", state.session.session_info.model)?;
    }
    writeln!(file, "- Exported: {}", now_timestamp())?;
    writeln!(file)?;

    for message in &state.chat.messages {
        match message.role {
            MessageRole::User => {
                for line in message.content.lines() {
                    writeln!(file, "> {}", line)?;
                }
                writeln!(file)?;
            }
            MessageRole::Assistant => {
                writeln!(file, "{}", message.content)?;
                writeln!(file)?;
            }
        }
    }

    Ok(path)
}

fn open_url(url: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = ProcessCommand::new("open");
        cmd.arg(url);
        cmd
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut cmd = ProcessCommand::new("xdg-open");
        cmd.arg(url);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = ProcessCommand::new("cmd");
        cmd.args(["/C", "start", url]);
        cmd
    };

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}
