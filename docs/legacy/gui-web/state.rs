//! Unified application state

use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use super::routes::acp::AcpSessionInfo;
use super::ws::WsBroadcaster;
use acp_adapter::{AcpAgentConnection, PermissionRequestManager};

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
    GitAfter,
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
            Tab::GitAfter => "gitafter",
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

/// Daemon/worker status info
#[derive(Clone, Default)]
pub struct DaemonInfo {
    /// Whether we're connected to the daemon socket
    pub connected: bool,
    /// Worker status (running, stopped, restarting, failed)
    pub worker_status: String,
    /// Worker process ID
    pub worker_pid: Option<u32>,
    /// Worker uptime in seconds
    pub uptime_seconds: u64,
    /// Total restart count
    pub total_restarts: u64,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Available memory in bytes
    pub memory_available_bytes: u64,
    /// Total memory in bytes
    pub memory_total_bytes: u64,
    /// Last update timestamp
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
    /// Error message if connection failed
    pub error: Option<String>,
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

    /// Daemon status info
    pub daemon_info: RwLock<DaemonInfo>,

    /// Running autopilot process (if Full Auto is ON)
    pub autopilot_process: RwLock<Option<AutopilotProcess>>,

    /// ACP (Agent Client Protocol) sessions
    pub acp_sessions: RwLock<HashMap<String, AcpSessionInfo>>,

    /// ACP agent connections (session_id -> connection)
    pub acp_connections: RwLock<HashMap<String, Arc<tokio::sync::Mutex<AcpAgentConnection>>>>,

    /// ACP permission request manager
    pub permission_manager: Arc<PermissionRequestManager>,

    /// Currently selected agent ("claude" or "codex")
    pub selected_agent: RwLock<String>,

    /// Agent availability cache (agent_id -> available)
    pub agent_availability: RwLock<HashMap<String, bool>>,
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
            daemon_info: RwLock::new(DaemonInfo::default()),
            autopilot_process: RwLock::new(None),
            acp_sessions: RwLock::new(HashMap::new()),
            acp_connections: RwLock::new(HashMap::new()),
            permission_manager: Arc::new(PermissionRequestManager::new()),
            selected_agent: RwLock::new("claude".to_string()),
            agent_availability: RwLock::new(HashMap::new()),
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
fn get_model_context_window(_model: &str) -> u64 {
    // All current Claude models support 200K context window
    200_000
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

/// Get OAuth token from credentials store.
/// Supports Linux (file-based) and macOS (Keychain).
async fn get_oauth_token() -> Option<String> {
    // Try Linux credentials file first (~/.claude/.credentials.json)
    let creds_path = shellexpand::tilde("~/.claude/.credentials.json").to_string();
    if let Ok(content) = tokio::fs::read_to_string(&creds_path).await {
        if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(token) = creds
                .get("claudeAiOauth")
                .and_then(|v| v.get("accessToken"))
                .and_then(|v| v.as_str())
            {
                return Some(token.to_string());
            }
        }
    }

    // Fall back to macOS Keychain
    #[cfg(target_os = "macos")]
    {
        use tokio::process::Command;

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
        // Token is nested under claudeAiOauth.accessToken
        return creds
            .get("claudeAiOauth")
            .and_then(|v| v.get("accessToken"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }

    #[cfg(not(target_os = "macos"))]
    {
        tracing::warn!("No OAuth credentials found");
        None
    }
}

/// Type alias for cached usage limits with timestamp
type UsageCache = Arc<RwLock<Option<(UsageLimits, std::time::Instant)>>>;

/// Cache for usage limits to prevent excessive API calls.
/// Cached value expires after 30 seconds.
static USAGE_CACHE: once_cell::sync::Lazy<UsageCache> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(None)));

/// Fetch usage/quota limits from Anthropic's OAuth usage API.
/// This endpoint returns actual usage data including extra credits.
/// Results are cached for 30 seconds to avoid rate limits.
pub async fn fetch_usage_limits() -> Option<UsageLimits> {
    // Check cache first
    {
        let cache = USAGE_CACHE.read().await;
        if let Some((limits, cached_at)) = cache.as_ref()
            && cached_at.elapsed() < std::time::Duration::from_secs(30) {
                tracing::debug!("Returning cached usage limits (age: {:?})", cached_at.elapsed());
                return Some(limits.clone());
            }
    }
    let token = match get_oauth_token().await {
        Some(t) => t,
        None => {
            tracing::warn!("No OAuth token available");
            return None;
        }
    };

    tracing::debug!("Fetching usage data from /api/oauth/usage...");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to build reqwest client");
    let response = match client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Usage API call failed: {}", e);
            return None;
        }
    };

    if !response.status().is_success() {
        tracing::warn!("Usage API returned status: {}", response.status());
        return None;
    }

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("Failed to parse usage API response: {}", e);
            return None;
        }
    };

    tracing::debug!("Usage API response: {:?}", json);

    let mut limits = UsageLimits::default();

    // Parse 5h (session) limit
    if let Some(five_hour) = json.get("five_hour") {
        if let Some(util) = five_hour.get("utilization").and_then(|v| v.as_f64()) {
            limits.session_percent = Some(util);
        }
        if let Some(resets) = five_hour.get("resets_at").and_then(|v| v.as_str()) {
            limits.session_resets_at = Some(format_iso_timestamp(resets));
        }
    }

    // Parse 7d (weekly) limit
    if let Some(seven_day) = json.get("seven_day") {
        if let Some(util) = seven_day.get("utilization").and_then(|v| v.as_f64()) {
            limits.weekly_all_percent = Some(util);
        }
        if let Some(resets) = seven_day.get("resets_at").and_then(|v| v.as_str()) {
            limits.weekly_all_resets_at = Some(format_iso_timestamp(resets));
        }
    }

    // Parse extra usage (this is the actual credits data!)
    if let Some(extra) = json.get("extra_usage")
        && extra.get("is_enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
            // monthly_limit and used_credits are in CENTS
            if let Some(limit_cents) = extra.get("monthly_limit").and_then(|v| v.as_f64()) {
                limits.extra_limit = Some(limit_cents / 100.0); // Convert cents to dollars
            }
            if let Some(used_cents) = extra.get("used_credits").and_then(|v| v.as_f64()) {
                limits.extra_spent = Some(used_cents / 100.0); // Convert cents to dollars
            }
            // Note: extra_usage doesn't have a resets_at field in the API response
        }

    // Return if we got any data
    if limits.session_percent.is_some() || limits.weekly_all_percent.is_some() || limits.extra_spent.is_some() {
        // Cache the result
        let mut cache = USAGE_CACHE.write().await;
        *cache = Some((limits.clone(), std::time::Instant::now()));
        tracing::debug!("Cached fresh usage limits");
        Some(limits)
    } else {
        None
    }
}

/// Format an ISO 8601 timestamp to human-readable reset time
fn format_iso_timestamp(iso: &str) -> String {
    use chrono::{DateTime, Local};

    if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
        let local: DateTime<Local> = dt.into();
        local.format("%b %d, %l:%M%P").to_string().trim().to_string()
    } else {
        iso.to_string()
    }
}

