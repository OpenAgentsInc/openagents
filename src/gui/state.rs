//! Unified application state

use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use super::ws::WsBroadcaster;

/// Running autopilot process state
pub struct AutopilotProcess {
    /// The child process handle
    pub child: Child,
    /// Task reading stdout/stderr and broadcasting
    pub output_task: JoinHandle<()>,
    /// Channel to signal shutdown to output task
    pub shutdown_tx: tokio::sync::mpsc::Sender<()>,
}

/// Tab identifiers for navigation
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)] // Future feature - tabs not yet implemented in UI
pub enum Tab {
    #[default]
    Wallet,
    Marketplace,
    Autopilot,
    AgentGit,
    Daemon,
    Settings,
}

impl Tab {
    #[allow(dead_code)] // Future feature - tabs not yet implemented in UI
    pub fn as_str(&self) -> &'static str {
        match self {
            Tab::Wallet => "wallet",
            Tab::Marketplace => "marketplace",
            Tab::Autopilot => "autopilot",
            Tab::AgentGit => "agentgit",
            Tab::Daemon => "daemon",
            Tab::Settings => "settings",
        }
    }
}

/// Per-model usage statistics
#[derive(Clone, Default)]
pub struct ModelUsage {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub web_search_requests: u64,
    pub cost_usd: f64,
    pub context_window: u64,
}

/// Claude status info
#[derive(Clone, Default)]
pub struct ClaudeInfo {
    /// Whether we're still loading info
    pub loading: bool,
    /// Whether authenticated
    pub authenticated: bool,
    /// Current model
    pub model: Option<String>,
    /// Claude Code version
    pub version: Option<String>,
    /// Total sessions
    pub total_sessions: Option<u64>,
    /// Total messages
    pub total_messages: Option<u64>,
    /// Today's token count
    pub today_tokens: Option<u64>,
    /// Per-model usage
    pub model_usage: Vec<ModelUsage>,
}

/// Unified application state shared across all routes
pub struct AppState {
    /// WebSocket broadcaster for real-time updates
    pub broadcaster: Arc<WsBroadcaster>,

    /// Currently active tab
    #[allow(dead_code)] // Future feature - tabs not yet implemented in UI
    pub active_tab: RwLock<Tab>,

    /// Full auto mode enabled
    pub full_auto: RwLock<bool>,

    /// Claude status info
    pub claude_info: RwLock<ClaudeInfo>,

    /// Running autopilot process (if Full Auto is ON)
    pub autopilot_process: RwLock<Option<AutopilotProcess>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            broadcaster: Arc::new(WsBroadcaster::new(64)),
            active_tab: RwLock::new(Tab::default()),
            full_auto: RwLock::new(false),
            claude_info: RwLock::new(ClaudeInfo {
                loading: true,
                ..Default::default()
            }),
            autopilot_process: RwLock::new(None),
        }
    }
}

/// Fast check: installed + version + basic auth from stats file
/// This is instant - no API calls
pub async fn fetch_claude_info_fast() -> ClaudeInfo {
    use tokio::process::Command;

    let mut info = ClaudeInfo::default();
    let claude_path = shellexpand::tilde("~/.claude/local/claude").to_string();

    // Check if installed by running --version (instant)
    if let Ok(output) = Command::new(&claude_path)
        .arg("--version")
        .output()
        .await
        && output.status.success() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            // Parse "2.0.73 (Claude Code)" -> "2.0.73"
            if let Some(ver) = version_str.split_whitespace().next() {
                info.version = Some(ver.to_string());
            }
        }

    // Read stats-cache.json for auth check + usage data (instant file read)
    let stats_path = shellexpand::tilde("~/.claude/stats-cache.json").to_string();
    if let Ok(content) = tokio::fs::read_to_string(&stats_path).await
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            info.total_sessions = json.get("totalSessions").and_then(|v| v.as_u64());
            info.total_messages = json.get("totalMessages").and_then(|v| v.as_u64());

            // If we have sessions, user is authenticated
            if info.total_sessions.unwrap_or(0) > 0 {
                info.authenticated = true;
            }

            // Get today's tokens
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            if let Some(daily) = json.get("dailyModelTokens").and_then(|v| v.as_array()) {
                for day in daily.iter().rev() {
                    if day.get("date").and_then(|d| d.as_str()) == Some(&today) {
                        if let Some(by_model) = day.get("tokensByModel").and_then(|v| v.as_object()) {
                            let total: u64 = by_model.values()
                                .filter_map(|v| v.as_u64())
                                .sum();
                            info.today_tokens = Some(total);
                        }
                        break;
                    }
                }
            }

            // Get per-model usage
            if let Some(model_usage) = json.get("modelUsage").and_then(|v| v.as_object()) {
                for (model, stats) in model_usage {
                    let input_tokens = stats.get("inputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let output_tokens = stats.get("outputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let cache_read = stats.get("cacheReadInputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let cache_create = stats.get("cacheCreationInputTokens").and_then(|v| v.as_u64()).unwrap_or(0);

                    // Calculate cost from tokens (stats-cache.json doesn't store cost)
                    let cost_usd = calculate_model_cost(model, input_tokens, output_tokens, cache_read, cache_create);
                    let context_window = get_model_context_window(model);

                    let usage = ModelUsage {
                        model: model.clone(),
                        input_tokens,
                        output_tokens,
                        cache_read_tokens: cache_read,
                        cache_creation_tokens: cache_create,
                        web_search_requests: stats.get("webSearchRequests").and_then(|v| v.as_u64()).unwrap_or(0),
                        cost_usd,
                        context_window,
                    };
                    info.model_usage.push(usage);
                }
                // Sort by cost descending (most expensive models first)
                info.model_usage.sort_by(|a, b| b.cost_usd.partial_cmp(&a.cost_usd).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

    info.loading = false;
    info
}

/// Slow check: Actually runs CLI to get current model (makes API call)
/// Only call this if you need the current model name
pub async fn fetch_claude_model() -> Option<String> {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let claude_path = shellexpand::tilde("~/.claude/local/claude").to_string();

    let mut child = Command::new(&claude_path)
        .args(["-p", "x", "--output-format", "stream-json", "--verbose"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let stdout = child.stdout.take()?;
    let mut reader = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        if line.contains("\"subtype\":\"init\"")
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                let model = json.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
                let _ = child.kill().await;
                let _ = child.wait().await;
                return model;
            }
    }

    let _ = child.kill().await;
    let _ = child.wait().await;
    None
}

/// Calculate cost for a model based on token usage.
/// Prices per million tokens (as of Dec 2024):
/// - Opus 4.5: $15 input, $75 output, $1.50 cache read, $18.75 cache write
/// - Sonnet 4.5: $3 input, $15 output, $0.30 cache read, $3.75 cache write
/// - Sonnet 4: $3 input, $15 output, $0.30 cache read, $3.75 cache write
/// - Haiku 4.5: $0.80 input, $4 output, $0.08 cache read, $1 cache write
fn calculate_model_cost(
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_read: u64,
    cache_create: u64,
) -> f64 {
    let (input_price, output_price, cache_read_price, cache_write_price) =
        if model.contains("opus") {
            (15.0, 75.0, 1.5, 18.75)
        } else if model.contains("haiku") {
            (0.80, 4.0, 0.08, 1.0)
        } else {
            // sonnet (default)
            (3.0, 15.0, 0.30, 3.75)
        };

    let million = 1_000_000.0;
    (input_tokens as f64 / million * input_price)
        + (output_tokens as f64 / million * output_price)
        + (cache_read as f64 / million * cache_read_price)
        + (cache_create as f64 / million * cache_write_price)
}

/// Get context window size for a model.
fn get_model_context_window(model: &str) -> u64 {
    if model.contains("opus-4-5") || model.contains("sonnet-4-5") {
        200_000
    } else if model.contains("haiku") {
        200_000
    } else {
        200_000 // default
    }
}

/// Usage limit data from Claude Code CLI
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Some fields reserved for future use
pub struct UsageLimits {
    /// Current session usage percentage
    pub session_percent: Option<f64>,
    /// Current session reset time
    pub session_resets_at: Option<String>,
    /// Weekly (all models) usage percentage
    pub weekly_all_percent: Option<f64>,
    /// Weekly (all models) reset time
    pub weekly_all_resets_at: Option<String>,
    /// Weekly (Sonnet only) usage percentage - reserved for future use
    pub weekly_sonnet_percent: Option<f64>,
    /// Weekly (Sonnet only) reset time - reserved for future use
    pub weekly_sonnet_resets_at: Option<String>,
    /// Extra usage spent
    #[allow(dead_code)]
    pub extra_spent: Option<f64>,
    /// Extra usage limit
    #[allow(dead_code)]
    pub extra_limit: Option<f64>,
    /// Extra usage reset time
    pub extra_resets_at: Option<String>,
}

/// Get OAuth token from macOS Keychain.
/// Returns None if not on macOS or token not found.
async fn get_oauth_token() -> Option<String> {
    use tokio::process::Command;

    // Get OAuth token from macOS Keychain
    let output = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        tracing::warn!("Failed to get OAuth token from keychain: {:?}", output.status);
        return None;
    }

    let creds = String::from_utf8_lossy(&output.stdout);
    let creds: serde_json::Value = match serde_json::from_str(creds.trim()) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Failed to parse OAuth credentials JSON: {}", e);
            return None;
        }
    };
    creds.get("accessToken").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Fetch usage/quota limits directly from Anthropic API.
/// Makes a minimal API call with OAuth token to capture rate limit headers.
pub async fn fetch_usage_limits() -> Option<UsageLimits> {
    let token = match get_oauth_token().await {
        Some(t) => t,
        None => {
            tracing::warn!("No OAuth token available");
            return None;
        }
    };

    tracing::debug!("Making API call to get rate limits...");

    // Make minimal API call to capture rate limit headers
    let client = reqwest::Client::new();
    let response: reqwest::Response = match client
        .post("https://api.anthropic.com/v1/messages")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "oauth-2025-04-20")
        .json(&serde_json::json!({
            "model": "claude-3-haiku-20240307",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "x"}]
        }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("API call failed: {}", e);
            return None;
        }
    };

    tracing::debug!("API response status: {}", response.status());

    let headers = response.headers();
    let mut limits = UsageLimits::default();

    // Parse 5h (session) limit
    if let Some(util) = headers.get("anthropic-ratelimit-unified-5h-utilization") {
        if let Ok(pct) = util.to_str().unwrap_or("0").parse::<f64>() {
            limits.session_percent = Some(pct * 100.0);
        }
    }
    if let Some(reset) = headers.get("anthropic-ratelimit-unified-5h-reset") {
        if let Ok(ts) = reset.to_str().unwrap_or("0").parse::<i64>() {
            limits.session_resets_at = Some(format_unix_timestamp(ts));
        }
    }

    // Parse 7d (weekly) limit
    if let Some(util) = headers.get("anthropic-ratelimit-unified-7d-utilization") {
        if let Ok(pct) = util.to_str().unwrap_or("0").parse::<f64>() {
            limits.weekly_all_percent = Some(pct * 100.0);
        }
    }
    if let Some(reset) = headers.get("anthropic-ratelimit-unified-7d-reset") {
        if let Ok(ts) = reset.to_str().unwrap_or("0").parse::<i64>() {
            limits.weekly_all_resets_at = Some(format_unix_timestamp(ts));
        }
    }

    // Parse overage status
    if let Some(status) = headers.get("anthropic-ratelimit-unified-overage-status") {
        let status_str = status.to_str().unwrap_or("");
        if status_str == "allowed" {
            // Overage is enabled and being used
            limits.extra_spent = Some(0.0); // TODO: get actual spent amount
            limits.extra_limit = Some(100.0); // TODO: get actual limit
        }
        // If "rejected" with "out_of_credits", overage is not available
    }

    // Parse overage disabled reason
    if let Some(_reason) = headers.get("anthropic-ratelimit-unified-overage-disabled-reason") {
        // Overage is disabled (e.g., "out_of_credits")
        limits.extra_spent = None;
        limits.extra_limit = None;
    }

    // Return if we got any data
    if limits.session_percent.is_some() || limits.weekly_all_percent.is_some() {
        Some(limits)
    } else {
        None
    }
}

/// Format a Unix timestamp to human-readable reset time
fn format_unix_timestamp(timestamp: i64) -> String {
    use chrono::{Local, TimeZone};

    if let Some(dt) = Local.timestamp_opt(timestamp, 0).single() {
        dt.format("%b %d, %l:%M%P (%Z)").to_string().trim().to_string()
    } else {
        format!("{}", timestamp)
    }
}

