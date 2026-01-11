//! Session replay from rlog files
//!
//! Provides functionality to replay recorded rlog sessions as ACP notifications,
//! enabling session review, debugging, and UI testing.

use std::path::Path;
use std::time::Duration;

use agent_client_protocol_schema as acp;
use tokio::sync::mpsc;

use crate::converters::rlog::rlog_line_to_notification;
use crate::error::{AcpError, Result};

/// Replay configuration options
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// Delay between notifications (simulates real-time)
    pub delay_ms: u64,
    /// Whether to simulate timing based on timestamps in the rlog
    pub use_timestamps: bool,
    /// Speed multiplier (1.0 = real-time, 2.0 = 2x speed)
    pub speed_multiplier: f64,
    /// Whether to skip empty lines
    pub skip_empty: bool,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            delay_ms: 50,
            use_timestamps: false,
            speed_multiplier: 1.0,
            skip_empty: true,
        }
    }
}

impl ReplayConfig {
    /// Create a new replay config with instant playback (no delays)
    pub fn instant() -> Self {
        Self {
            delay_ms: 0,
            use_timestamps: false,
            speed_multiplier: 1.0,
            skip_empty: true,
        }
    }

    /// Create a replay config that simulates real-time based on timestamps
    pub fn realtime() -> Self {
        Self {
            delay_ms: 100,
            use_timestamps: true,
            speed_multiplier: 1.0,
            skip_empty: true,
        }
    }

    /// Set the speed multiplier
    pub fn speed(mut self, multiplier: f64) -> Self {
        self.speed_multiplier = multiplier;
        self
    }

    /// Set the base delay between notifications
    pub fn delay(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }
}

/// Session replayer that streams rlog content as ACP notifications
pub struct RlogReplay {
    session_id: acp::SessionId,
    config: ReplayConfig,
}

impl RlogReplay {
    /// Create a new replayer for a session
    pub fn new(session_id: acp::SessionId) -> Self {
        Self {
            session_id,
            config: ReplayConfig::default(),
        }
    }

    /// Set the replay configuration
    pub fn with_config(mut self, config: ReplayConfig) -> Self {
        self.config = config;
        self
    }

    /// Replay an rlog file, sending notifications to the provided channel
    pub async fn replay_file(
        &self,
        path: &Path,
        tx: mpsc::Sender<acp::SessionNotification>,
    ) -> Result<ReplayStats> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| AcpError::Other(format!("Failed to read rlog file: {}", e)))?;

        self.replay_content(&content, tx).await
    }

    /// Replay rlog content from a string
    pub async fn replay_content(
        &self,
        content: &str,
        tx: mpsc::Sender<acp::SessionNotification>,
    ) -> Result<ReplayStats> {
        let mut stats = ReplayStats::default();
        let lines: Vec<&str> = content.lines().collect();

        // Skip header (between --- markers)
        let body_start = find_body_start(&lines);

        for line in lines.iter().skip(body_start) {
            stats.lines_processed += 1;

            // Skip empty lines if configured
            if self.config.skip_empty && line.trim().is_empty() {
                stats.lines_skipped += 1;
                continue;
            }

            // Convert line to ACP notification
            if let Some(notification) = rlog_line_to_notification(&self.session_id, line) {
                stats.notifications_sent += 1;

                // Send notification
                if tx.send(notification).await.is_err() {
                    // Receiver dropped, stop replay
                    break;
                }

                // Apply delay if configured
                if self.config.delay_ms > 0 {
                    let delay = (self.config.delay_ms as f64 / self.config.speed_multiplier) as u64;
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            } else {
                stats.lines_skipped += 1;
            }
        }

        Ok(stats)
    }

    /// Replay and collect all notifications (useful for testing)
    pub async fn replay_to_vec(&self, content: &str) -> Result<Vec<acp::SessionNotification>> {
        let (tx, mut rx) = mpsc::channel(1024);

        // Spawn replay task
        let content_owned = content.to_string();
        let session_id = self.session_id.clone();
        let config = self.config.clone();

        let replay_task = tokio::spawn(async move {
            let replayer = RlogReplay::new(session_id).with_config(config);
            replayer.replay_content(&content_owned, tx).await
        });

        // Collect notifications
        let mut notifications = Vec::new();
        while let Some(notif) = rx.recv().await {
            notifications.push(notif);
        }

        // Wait for replay to complete
        replay_task
            .await
            .map_err(|e| AcpError::Other(format!("Replay task failed: {}", e)))??;

        Ok(notifications)
    }
}

/// Statistics from a replay session
#[derive(Debug, Default, Clone)]
pub struct ReplayStats {
    /// Total lines processed
    pub lines_processed: usize,
    /// Lines that were skipped (empty or unparseable)
    pub lines_skipped: usize,
    /// Notifications successfully sent
    pub notifications_sent: usize,
}

/// Find where the body starts (after the header)
fn find_body_start(lines: &[&str]) -> usize {
    let mut in_header = false;
    let mut header_end = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if !in_header {
                in_header = true;
            } else {
                header_end = i + 1;
                break;
            }
        }
    }

    header_end
}

/// Parse header metadata from rlog content
pub fn parse_rlog_header(content: &str) -> Option<RlogHeader> {
    let lines: Vec<&str> = content.lines().collect();

    // Find header boundaries
    let start = lines.iter().position(|l| l.trim() == "---")?;
    let end = lines
        .iter()
        .skip(start + 1)
        .position(|l| l.trim() == "---")?
        + start
        + 1;

    // Extract header content
    let header_content = lines[start + 1..end].join("\n");

    // Parse YAML
    serde_yaml::from_str(&header_content).ok()
}

/// Parsed rlog header information
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RlogHeader {
    pub format: String,
    pub id: String,
    pub repo_sha: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub client_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RLOG: &str = r#"---
format: rlog/1
id: test-session
repo_sha: abc123def
model: codex-sonnet-4
cwd: /test/path
---

u: Hello, please help me
th: Let me analyze this request sig=abc12345
a: I'll help you with that
t!:Read id=call_1 file="/test.txt" → [running]
o: id=call_1 → [ok] file contents here
td: [completed] Read file [in_progress] Process data
"#;

    #[tokio::test]
    async fn test_replay_to_vec() {
        let session_id = acp::SessionId::new("test");
        let replayer = RlogReplay::new(session_id).with_config(ReplayConfig::instant());

        let notifications = replayer.replay_to_vec(SAMPLE_RLOG).await.unwrap();

        // Should have: user, thought, agent, tool call, observation, plan
        assert!(notifications.len() >= 5);

        // First should be user message
        assert!(matches!(
            notifications[0].update,
            acp::SessionUpdate::UserMessageChunk(_)
        ));
    }

    #[tokio::test]
    async fn test_parse_header() {
        let header = parse_rlog_header(SAMPLE_RLOG).unwrap();

        assert_eq!(header.format, "rlog/1");
        assert_eq!(header.id, "test-session");
        assert_eq!(header.repo_sha, "abc123def");
        assert_eq!(header.model, Some("codex-sonnet-4".to_string()));
        assert_eq!(header.cwd, Some("/test/path".to_string()));
    }

    #[tokio::test]
    async fn test_replay_stats() {
        let session_id = acp::SessionId::new("test");
        let replayer = RlogReplay::new(session_id).with_config(ReplayConfig::instant());

        let (tx, mut rx) = mpsc::channel(100);

        // Consume in background
        let consumer = tokio::spawn(async move {
            let mut count = 0;
            while rx.recv().await.is_some() {
                count += 1;
            }
            count
        });

        let stats = replayer.replay_content(SAMPLE_RLOG, tx).await.unwrap();

        let received = consumer.await.unwrap();

        assert!(stats.lines_processed > 0);
        assert_eq!(stats.notifications_sent, received);
    }

    #[test]
    fn test_find_body_start() {
        let lines = vec!["---", "format: rlog/1", "id: test", "---", "", "u: Hello"];
        assert_eq!(find_body_start(&lines), 4);
    }
}
