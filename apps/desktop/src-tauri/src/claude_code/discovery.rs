use crate::claude_code::models::{ClaudeConversation, ClaudeError, UnifiedSession};
use crate::claude_code::convex_client::ConvexClient;
use chrono::{DateTime, Utc};
use dirs_next;
use log::{info, error};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::env;
use tokio::fs as async_fs;

pub struct ClaudeDiscovery {
    binary_path: Option<PathBuf>,
    data_path: Option<PathBuf>,
}

impl ClaudeDiscovery {
    pub fn new() -> Self {
        Self {
            binary_path: None,
            data_path: None,
        }
    }

    /// Discover Claude Code binary location
    pub async fn discover_binary(&mut self) -> Result<PathBuf, ClaudeError> {
        // First, check if already discovered
        if let Some(ref path) = self.binary_path {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Strategy 1: Check for claude in fnm paths first (most common with modern setup)
        if let Some(home) = dirs_next::home_dir() {
            let fnm_base = home.join(".local/state/fnm_multishells");
            if fnm_base.exists() {
                // Find any fnm shell directory
                if let Ok(entries) = async_fs::read_dir(&fnm_base).await {
                    let mut entries = entries;
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let claude_path = entry.path().join("bin/claude");
                        if claude_path.exists() {
                            // Verify it's not the old version
                            if let Ok(output) = Command::new(&claude_path)
                                .arg("--version")
                                .output()
                            {
                                let version_str = String::from_utf8_lossy(&output.stdout);
                                if !version_str.contains("0.2.") {
                                    info!("Claude Code v{} ready at {}", 
                                          version_str.trim(), claude_path.display());
                                    self.binary_path = Some(claude_path.clone());
                                    return Ok(claude_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Strategy 2: Check PATH using login shell
        if let Some(path) = self.check_path_with_shell().await? {
            info!("Claude Code ready at {}", path.display());
            self.binary_path = Some(path.clone());
            return Ok(path);
        }

        // Strategy 3: Check common installation locations
        let common_paths = vec![
            "/usr/local/bin/claude",
            "/opt/homebrew/bin/claude",
            "/usr/local/opt/claude/bin/claude",
        ];

        for path_str in common_paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                info!("Claude Code ready at {}", path.display());
                self.binary_path = Some(path.clone());
                return Ok(path);
            }
        }

        // If we get here, claude wasn't found
        info!("Claude Code binary not found via standard methods");
        info!("Please ensure 'claude' is installed and available in your PATH");
        info!("You can install it with: npm install -g @anthropic-ai/claude-code");

        Err(ClaudeError::BinaryNotFound)
    }

    /// Check PATH using login shell
    async fn check_path_with_shell(&self) -> Result<Option<PathBuf>, ClaudeError> {
        // First try to get the PATH from the current shell environment
        let shell_cmd = if std::env::var("SHELL").unwrap_or_default().contains("zsh") {
            "/bin/zsh"
        } else {
            "/bin/bash"
        };
        
        // Run which claude with full environment
        let output = Command::new(shell_cmd)
            .args(&["-l", "-i", "-c", "which claude"])
            .output()
            .map_err(|e| ClaudeError::Other(format!("Failed to run which command: {}", e)))?;

        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            info!("which claude returned: {}", path_str);
            
            if !path_str.is_empty() && !path_str.contains("not found") {
                let path = PathBuf::from(&path_str);
                
                // Skip if it's the old node_modules version
                if path_str.contains("node_modules/.bin/claude") {
                    info!("Skipping old node_modules claude binary");
                    
                    // Try to find the correct one - first get the full PATH
                    let path_output = Command::new(shell_cmd)
                        .args(&["-l", "-i", "-c", "echo $PATH"])
                        .output()
                        .map_err(|e| ClaudeError::Other(format!("Failed to get PATH: {}", e)))?;
                    
                    if path_output.status.success() {
                        let full_path = String::from_utf8_lossy(&path_output.stdout);
                        
                        // Check each directory in PATH for claude
                        for dir in full_path.trim().split(':') {
                            let claude_path = PathBuf::from(dir).join("claude");
                            if claude_path.exists() && !dir.contains("node_modules") {
                                // Check version to make sure it's not old
                                if let Ok(version_output) = Command::new(&claude_path)
                                    .arg("--version")
                                    .output()
                                {
                                    let version_str = String::from_utf8_lossy(&version_output.stdout);
                                    if !version_str.contains("0.2.") {
                                        return Ok(Some(claude_path));
                                    }
                                }
                            }
                        }
                    }
                    
                    // Also try ~/.local/bin/claude (common location)
                    if let Some(home) = dirs_next::home_dir() {
                        let local_claude = home.join(".local/bin/claude");
                        if local_claude.exists() {
                            return Ok(Some(local_claude));
                        }
                    }
                } else if path.exists() {
                    return Ok(Some(path));
                }
            }
        }

        Ok(None)
    }

    /// Discover Claude data directory
    pub async fn discover_data_directory(&mut self) -> Result<PathBuf, ClaudeError> {
        if let Some(ref path) = self.data_path {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        if let Some(home_dir) = dirs_next::home_dir() {
            let claude_dir = home_dir.join(".claude");
            if claude_dir.exists() && claude_dir.is_dir() {
                self.data_path = Some(claude_dir.clone());
                return Ok(claude_dir);
            }
        }

        Err(ClaudeError::Other("Claude data directory not found".to_string()))
    }

    /// Load historical conversations from Claude data directory
    pub async fn load_conversations(&self, limit: usize) -> Result<Vec<ClaudeConversation>, ClaudeError> {
        let data_path = self.data_path.as_ref()
            .ok_or_else(|| ClaudeError::Other("Data directory not discovered yet".to_string()))?;

        let projects_path = data_path.join("projects");
        if !projects_path.exists() {
            return Ok(vec![]);
        }

        let mut conversations = Vec::new();

        if let Ok(entries) = async_fs::read_dir(&projects_path).await {
            let mut entries = entries;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(conv_files) = async_fs::read_dir(&path).await {
                        let mut conv_files = conv_files;
                        while let Ok(Some(file_entry)) = conv_files.next_entry().await {
                            let file_path = file_entry.path();
                            if file_path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                                if let Ok(conversation) = self.parse_conversation(&file_path).await {
                                    conversations.push(conversation);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort by timestamp, most recent first
        conversations.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Return limited number of conversations
        Ok(conversations.into_iter().take(limit).collect())
    }

    /// Parse a single conversation file
    async fn parse_conversation(&self, path: &Path) -> Result<ClaudeConversation, ClaudeError> {
        let content = async_fs::read_to_string(path).await?;
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

        let mut timestamp = Utc::now();
        let mut cwd = String::new();
        let mut first_message = String::new();
        let mut summary = None;

        // Parse JSONL lines
        for line in &lines {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                // Look for timestamp
                if let Some(ts_str) = json.get("timestamp").and_then(|v| v.as_str()) {
                    if let Ok(ts) = DateTime::parse_from_rfc3339(ts_str) {
                        timestamp = ts.with_timezone(&Utc);
                    }
                }

                // Look for working directory
                if let Some(dir) = json.get("cwd").and_then(|v| v.as_str()) {
                    cwd = dir.to_string();
                }

                // Look for first user message
                if first_message.is_empty() {
                    if let Some(msg) = json.get("message").and_then(|v| v.as_object()) {
                        if msg.get("role").and_then(|v| v.as_str()) == Some("user") {
                            if let Some(content) = msg.get("content") {
                                if let Some(text) = content.as_str() {
                                    first_message = text.chars().take(100).collect();
                                } else if let Some(content_array) = content.as_array() {
                                    for item in content_array {
                                        if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                                            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                                first_message = text.chars().take(100).collect();
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Look for summary
                if json.get("type").and_then(|v| v.as_str()) == Some("summary") {
                    if let Some(s) = json.get("summary").and_then(|v| v.as_str()) {
                        summary = Some(s.to_string());
                    }
                }
            }
        }

        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let project_name = path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
            .replace("-Users-", "~/")
            .replace("-", "/");

        Ok(ClaudeConversation {
            id: file_name,
            project_name,
            timestamp,
            first_message,
            message_count: lines.len(),
            file_path: path.to_string_lossy().to_string(),
            working_directory: cwd,
            summary,
        })
    }

    /// Load unified session history from both local files and Convex
    pub async fn load_unified_sessions(
        &mut self, 
        limit: usize, 
        user_id: Option<String>
    ) -> Result<Vec<UnifiedSession>, ClaudeError> {
        let mut all_sessions = Vec::new();

        // Load .env files following Convex quickstart pattern  
        // Try multiple locations since working directory might vary
        info!("Current working directory: {:?}", std::env::current_dir());
        dotenvy::from_filename("../.env.local").ok(); // Parent directory
        dotenvy::from_filename("../.env").ok();       // Parent directory
        dotenvy::from_filename(".env.local").ok();    // Current directory  
        dotenvy::dotenv().ok();                       // Current directory
        
        // Debug: Check what CONVEX_URL we have after loading
        match env::var("CONVEX_URL") {
            Ok(url) => info!("Found CONVEX_URL after dotenv loading: {}", url),
            Err(_) => {
                info!("CONVEX_URL still not found after dotenv loading");
                
                // As a fallback, try to manually read the .env file
                if let Ok(env_content) = std::fs::read_to_string("../.env") {
                    info!("Found ../.env file, content preview: {}", 
                          env_content.lines().take(3).collect::<Vec<_>>().join("; "));
                    
                    // Simple manual parsing for CONVEX_URL
                    for line in env_content.lines() {
                        if line.starts_with("CONVEX_URL=") {
                            let url = line.trim_start_matches("CONVEX_URL=");
                            info!("Manually found CONVEX_URL in .env: {}", url);
                            std::env::set_var("CONVEX_URL", url);
                            break;
                        }
                    }
                } else {
                    info!("Could not read ../.env file either");
                }
            }
        }

        // Load local Claude Code CLI conversations
        // First, discover the data directory if not already done
        if self.data_path.is_none() {
            if let Err(e) = self.discover_data_directory().await {
                info!("Could not discover Claude data directory: {}", e);
            }
        }
        
        match self.load_conversations(limit).await {
            Ok(local_conversations) => {
                info!("Loaded {} local Claude Code conversations", local_conversations.len());
                for conv in local_conversations {
                    all_sessions.push(UnifiedSession::from(conv));
                }
            }
            Err(e) => {
                // Log but don't fail - we can still show Convex sessions
                info!("Could not load local conversations: {}", e);
            }
        }

        // Load Convex sessions using environment variable (following quickstart pattern)
        match env::var("CONVEX_URL") {
            Ok(deployment_url) => {
                info!("Found CONVEX_URL environment variable: {}", deployment_url);
                match ConvexClient::new(&deployment_url).await {
                    Ok(mut convex_client) => {
                        match convex_client.get_sessions(Some(limit), user_id).await {
                            Ok(convex_sessions) => {
                                info!("Loaded {} Convex sessions", convex_sessions.len());
                                for session in convex_sessions {
                                    all_sessions.push(UnifiedSession::from(session));
                                }
                            }
                            Err(e) => {
                                // Log but don't fail - we can still show local sessions
                                error!("Could not load Convex sessions: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        // Log but don't fail - we can still show local sessions
                        error!("Could not create Convex client: {}", e);
                    }
                }
            }
            Err(_) => {
                info!("CONVEX_URL environment variable not found, skipping Convex sessions");
            }
        }

        // Sort by timestamp, most recent first
        all_sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Deduplicate sessions that might appear in both sources
        // This is a simple approach - could be enhanced with better matching logic
        let mut unique_sessions = Vec::new();
        let mut seen_titles = std::collections::HashSet::new();

        for session in all_sessions {
            // Create a simple key for deduplication
            let key = format!("{}:{}", 
                session.project_path.as_deref().unwrap_or(""), 
                session.title.chars().take(50).collect::<String>()
            );
            
            if !seen_titles.contains(&key) {
                seen_titles.insert(key);
                unique_sessions.push(session);
            }
        }

        // Return limited number of sessions
        let result: Vec<UnifiedSession> = unique_sessions.into_iter().take(limit).collect();
        info!("Returning {} unified sessions", result.len());
        Ok(result)
    }

}