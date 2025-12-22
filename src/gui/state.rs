//! Unified application state

use std::sync::Arc;
use tokio::sync::RwLock;

use super::ws::WsBroadcaster;

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
        }
    }
}

/// Fetch Claude info by running CLI and parsing stats
pub async fn fetch_claude_info() -> ClaudeInfo {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let mut info = ClaudeInfo::default();

    // First, read stats-cache.json for usage data
    let stats_path = shellexpand::tilde("~/.claude/stats-cache.json").to_string();
    if let Ok(content) = tokio::fs::read_to_string(&stats_path).await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            info.total_sessions = json.get("totalSessions").and_then(|v| v.as_u64());
            info.total_messages = json.get("totalMessages").and_then(|v| v.as_u64());

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
                    let usage = ModelUsage {
                        model: model.clone(),
                        input_tokens: stats.get("inputTokens").and_then(|v| v.as_u64()).unwrap_or(0),
                        output_tokens: stats.get("outputTokens").and_then(|v| v.as_u64()).unwrap_or(0),
                        cache_read_tokens: stats.get("cacheReadInputTokens").and_then(|v| v.as_u64()).unwrap_or(0),
                        cache_creation_tokens: stats.get("cacheCreationInputTokens").and_then(|v| v.as_u64()).unwrap_or(0),
                    };
                    info.model_usage.push(usage);
                }
                // Sort by output tokens descending (most used models first)
                info.model_usage.sort_by(|a, b| b.output_tokens.cmp(&a.output_tokens));
            }
        }
    }

    // Now check auth by running CLI
    let claude_path = shellexpand::tilde("~/.claude/local/claude").to_string();

    let child_result = Command::new(&claude_path)
        .args(["-p", "x", "--output-format", "stream-json", "--verbose"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let mut child = match child_result {
        Ok(c) => c,
        Err(_) => {
            info.loading = false;
            return info;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            info.loading = false;
            return info;
        }
    };

    let mut reader = BufReader::new(stdout).lines();

    // Read lines looking for the init message
    while let Ok(Some(line)) = reader.next_line().await {
        if line.contains("\"subtype\":\"init\"") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                info.authenticated = true;
                info.model = json.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
                info.version = json.get("claude_code_version").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
            let _ = child.kill().await;
            break;
        }

        // Also check for result which means query completed (we're authed)
        if line.contains("\"type\":\"result\"") {
            info.authenticated = true;
            let _ = child.kill().await;
            break;
        }
    }

    let _ = child.wait().await;
    info.loading = false;
    info
}
