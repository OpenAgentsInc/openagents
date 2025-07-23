use crate::claude_code::models::{ClaudeConversation, ClaudeError};
use chrono::{DateTime, Utc};
use dirs_next;
use log::info;
use std::path::{Path, PathBuf};
use std::process::Command;
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
        info!("Starting Claude Code binary discovery...");

        // First, check if already discovered
        if let Some(ref path) = self.binary_path {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Strategy 1: Check PATH using login shell
        if let Some(path) = self.check_path_with_shell().await? {
            info!("Found Claude in PATH: {:?}", path);
            self.binary_path = Some(path.clone());
            return Ok(path);
        }

        // Strategy 2: Check common installation locations
        let common_paths = vec![
            "/usr/local/bin/claude",
            "/opt/homebrew/bin/claude",
            "/usr/local/opt/claude/bin/claude",
        ];

        for path_str in common_paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                info!("Found Claude at: {:?}", path);
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
        let output = Command::new("/bin/bash")
            .args(&["-l", "-c", "which claude"])
            .output()
            .map_err(|e| ClaudeError::Other(format!("Failed to run which command: {}", e)))?;

        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            info!("which claude returned: {}", path_str);
            if !path_str.is_empty() && !path_str.contains("not found") {
                let path = PathBuf::from(path_str);
                if path.exists() {
                    // Don't resolve symlinks - use the path as returned by which
                    info!("Using claude binary from PATH: {}", path.display());
                    return Ok(Some(path));
                }
            }
        }

        Ok(None)
    }

    /// Discover Claude data directory
    pub async fn discover_data_directory(&mut self) -> Result<PathBuf, ClaudeError> {
        info!("Searching for Claude data directory...");

        if let Some(ref path) = self.data_path {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        if let Some(home_dir) = dirs_next::home_dir() {
            let claude_dir = home_dir.join(".claude");
            if claude_dir.exists() && claude_dir.is_dir() {
                info!("Found Claude data directory: {:?}", claude_dir);
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

    pub fn get_binary_path(&self) -> Option<&PathBuf> {
        self.binary_path.as_ref()
    }

    pub fn get_data_path(&self) -> Option<&PathBuf> {
        self.data_path.as_ref()
    }
}