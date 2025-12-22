//! Unified application state

use std::sync::Arc;
use tokio::sync::RwLock;

use claude_agent_sdk::AccountInfo;

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

/// Unified application state shared across all routes
pub struct AppState {
    /// WebSocket broadcaster for real-time updates
    pub broadcaster: Arc<WsBroadcaster>,

    /// Currently active tab
    #[allow(dead_code)] // Future feature - tabs not yet implemented in UI
    pub active_tab: RwLock<Tab>,

    /// Full auto mode enabled
    pub full_auto: RwLock<bool>,

    /// Claude account info (fetched on startup)
    pub claude_account: RwLock<Option<AccountInfo>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            broadcaster: Arc::new(WsBroadcaster::new(64)),
            active_tab: RwLock::new(Tab::default()),
            full_auto: RwLock::new(false),
            claude_account: RwLock::new(None),
        }
    }
}

/// Fetch Claude account info by running CLI with a simple prompt
pub async fn fetch_claude_account_info() -> Option<AccountInfo> {
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

    // Read lines looking for the init message
    while let Ok(Some(line)) = reader.next_line().await {
        if line.contains("\"subtype\":\"init\"") {
            // Parse init message to check auth
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                let api_key_source = json.get("apiKeySource").and_then(|v| v.as_str());
                let model = json.get("model").and_then(|v| v.as_str());

                // If we got a model, we're authenticated
                if model.is_some() {
                    // Kill the process since we have what we need
                    let _ = child.kill().await;

                    let token_source = match api_key_source {
                        Some("none") | None => Some("oauth".to_string()),
                        Some(other) => Some(other.to_string()),
                    };

                    return Some(AccountInfo {
                        email: Some("Authenticated".to_string()),
                        organization: None,
                        subscription_type: Some("claude-max".to_string()),
                        token_source,
                        api_key_source: api_key_source.map(|s| s.to_string()),
                    });
                }
            }
            break;
        }

        // Also check for result which means query completed (we're authed)
        if line.contains("\"type\":\"result\"") {
            let _ = child.kill().await;

            return Some(AccountInfo {
                email: Some("Authenticated".to_string()),
                organization: None,
                subscription_type: Some("claude-max".to_string()),
                token_source: Some("oauth".to_string()),
                api_key_source: None,
            });
        }
    }

    // Wait for process to finish
    let _ = child.wait().await;

    None
}
