//! Real-time streaming of ACP notifications to rlog files
//!
//! Provides functionality to write ACP session notifications to rlog format
//! in real-time, enabling session recording and trajectory collection.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_client_protocol_schema as acp;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::converters::rlog::notification_to_rlog_line;
use crate::error::{AcpError, Result};

/// Configuration for rlog streaming
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Whether to flush after each write
    pub flush_immediately: bool,
    /// Whether to include timestamps in lines
    pub include_timestamps: bool,
    /// Maximum line length before truncation
    pub max_line_length: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            flush_immediately: true,
            include_timestamps: false,
            max_line_length: 500,
        }
    }
}

/// Header information for a new rlog file
#[derive(Debug, Clone)]
pub struct RlogHeaderInfo {
    pub session_id: String,
    pub repo_sha: String,
    pub mode: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub cwd: Option<String>,
    pub client_version: Option<String>,
}

impl RlogHeaderInfo {
    /// Create a new header with required fields
    pub fn new(session_id: impl Into<String>, repo_sha: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            repo_sha: repo_sha.into(),
            mode: None,
            model: None,
            agent: None,
            cwd: None,
            client_version: None,
        }
    }

    /// Set the mode
    pub fn mode(mut self, mode: impl Into<String>) -> Self {
        self.mode = Some(mode.into());
        self
    }

    /// Set the model
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the agent
    pub fn agent(mut self, agent: impl Into<String>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    /// Set the working directory
    pub fn cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set the client version
    pub fn client_version(mut self, version: impl Into<String>) -> Self {
        self.client_version = Some(version.into());
        self
    }

    /// Render the header as YAML
    fn render(&self) -> String {
        let mut lines = vec![
            "---".to_string(),
            "format: rlog/1".to_string(),
            format!("id: {}", self.session_id),
            format!("repo_sha: {}", self.repo_sha),
        ];

        if let Some(ref mode) = self.mode {
            lines.push(format!("mode: {}", mode));
        }
        if let Some(ref model) = self.model {
            lines.push(format!("model: {}", model));
        }
        if let Some(ref agent) = self.agent {
            lines.push(format!("agent: {}", agent));
        }
        if let Some(ref cwd) = self.cwd {
            lines.push(format!("cwd: {}", cwd));
        }
        if let Some(ref version) = self.client_version {
            lines.push(format!("client_version: \"{}\"", version));
        }

        lines.push("---".to_string());
        lines.push(String::new()); // Empty line after header

        lines.join("\n")
    }
}

/// Streams ACP notifications to an rlog file in real-time
pub struct RlogStreamer {
    file: Arc<Mutex<File>>,
    path: PathBuf,
    config: StreamConfig,
    lines_written: Arc<Mutex<usize>>,
}

impl RlogStreamer {
    /// Create a new streamer that writes to a file
    pub async fn create(path: impl AsRef<Path>, header: RlogHeaderInfo) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AcpError::Other(format!("Failed to create directory: {}", e)))?;
        }

        // Create file and write header
        let mut file = File::create(&path)
            .await
            .map_err(|e| AcpError::Other(format!("Failed to create rlog file: {}", e)))?;

        let header_content = header.render();
        file.write_all(header_content.as_bytes())
            .await
            .map_err(|e| AcpError::Other(format!("Failed to write header: {}", e)))?;

        file.flush()
            .await
            .map_err(|e| AcpError::Other(format!("Failed to flush: {}", e)))?;

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            path,
            config: StreamConfig::default(),
            lines_written: Arc::new(Mutex::new(0)),
        })
    }

    /// Set the streaming configuration
    pub fn with_config(mut self, config: StreamConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the path to the rlog file
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the number of lines written
    pub async fn lines_written(&self) -> usize {
        *self.lines_written.lock().await
    }

    /// Write an ACP notification to the rlog file
    pub async fn write_notification(&self, notification: &acp::SessionNotification) -> Result<()> {
        if let Some(line) = notification_to_rlog_line(notification) {
            self.write_line(&line).await?;
        }
        Ok(())
    }

    /// Write a raw line to the rlog file
    pub async fn write_line(&self, line: &str) -> Result<()> {
        let mut file = self.file.lock().await;

        // Truncate if needed
        let line = if line.len() > self.config.max_line_length {
            format!("{}...", &line[..self.config.max_line_length - 3])
        } else {
            line.to_string()
        };

        // Add timestamp if configured
        let line = if self.config.include_timestamps {
            let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
            format!("{} ts={}", line, ts)
        } else {
            line
        };

        // Write line with newline
        file.write_all(format!("{}\n", line).as_bytes())
            .await
            .map_err(|e| AcpError::Other(format!("Failed to write line: {}", e)))?;

        // Flush if configured
        if self.config.flush_immediately {
            file.flush()
                .await
                .map_err(|e| AcpError::Other(format!("Failed to flush: {}", e)))?;
        }

        // Update line count
        *self.lines_written.lock().await += 1;

        Ok(())
    }

    /// Write a comment line
    pub async fn write_comment(&self, comment: &str) -> Result<()> {
        self.write_line(&format!("# {}", comment)).await
    }

    /// Write a lifecycle event (@start, @end, etc.)
    pub async fn write_lifecycle(&self, event: &str) -> Result<()> {
        self.write_line(&format!("@{}", event)).await
    }

    /// Write a phase marker
    pub async fn write_phase(&self, phase: &str) -> Result<()> {
        self.write_line(&format!("@phase {}", phase)).await
    }

    /// Write a mode change
    pub async fn write_mode(&self, mode: &str) -> Result<()> {
        self.write_line(&format!("m: {}", mode)).await
    }

    /// Flush any buffered content
    pub async fn flush(&self) -> Result<()> {
        let mut file = self.file.lock().await;
        file.flush()
            .await
            .map_err(|e| AcpError::Other(format!("Failed to flush: {}", e)))
    }

    /// Finalize the rlog file with an @end marker
    pub async fn finalize(&self) -> Result<()> {
        self.write_lifecycle("end").await?;
        self.flush().await
    }
}

/// In-memory rlog buffer for testing or temporary storage
pub struct RlogBuffer {
    lines: Arc<Mutex<Vec<String>>>,
    header: RlogHeaderInfo,
}

impl RlogBuffer {
    /// Create a new in-memory buffer
    pub fn new(header: RlogHeaderInfo) -> Self {
        Self {
            lines: Arc::new(Mutex::new(Vec::new())),
            header,
        }
    }

    /// Write an ACP notification to the buffer
    pub async fn write_notification(&self, notification: &acp::SessionNotification) {
        if let Some(line) = notification_to_rlog_line(notification) {
            self.lines.lock().await.push(line);
        }
    }

    /// Write a raw line to the buffer
    pub async fn write_line(&self, line: &str) {
        self.lines.lock().await.push(line.to_string());
    }

    /// Get all lines in the buffer
    pub async fn lines(&self) -> Vec<String> {
        self.lines.lock().await.clone()
    }

    /// Get the full rlog content as a string
    pub async fn to_string(&self) -> String {
        let mut content = self.header.render();
        for line in self.lines.lock().await.iter() {
            content.push_str(line);
            content.push('\n');
        }
        content
    }

    /// Save the buffer to a file
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = self.to_string().await;
        tokio::fs::write(path, content)
            .await
            .map_err(|e| AcpError::Other(format!("Failed to save buffer: {}", e)))
    }
}

/// Generate a default rlog filename based on current time
pub fn generate_rlog_filename() -> String {
    let now = chrono::Local::now();
    now.format("%Y%m%d_%H%M%S.rlog").to_string()
}

/// Generate a path for a new rlog file in the standard location
pub fn generate_rlog_path(base_dir: impl AsRef<Path>) -> PathBuf {
    let now = chrono::Local::now();
    let date_dir = now.format("%Y%m%d").to_string();
    let filename = generate_rlog_filename();

    base_dir.as_ref().join(date_dir).join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_render() {
        let header = RlogHeaderInfo::new("test-session", "abc123")
            .mode("auto")
            .model("codex-sonnet-4")
            .agent("codex")
            .cwd("/test/path");

        let rendered = header.render();

        assert!(rendered.contains("format: rlog/1"));
        assert!(rendered.contains("id: test-session"));
        assert!(rendered.contains("repo_sha: abc123"));
        assert!(rendered.contains("mode: auto"));
        assert!(rendered.contains("model: codex-sonnet-4"));
        assert!(rendered.contains("agent: codex"));
        assert!(rendered.contains("cwd: /test/path"));
    }

    #[tokio::test]
    async fn test_buffer_write() {
        let header = RlogHeaderInfo::new("test", "abc123");
        let buffer = RlogBuffer::new(header);

        buffer.write_line("u: Hello").await;
        buffer.write_line("a: Hi there").await;

        let lines = buffer.lines().await;
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "u: Hello");
        assert_eq!(lines[1], "a: Hi there");
    }

    #[tokio::test]
    async fn test_buffer_to_string() {
        let header = RlogHeaderInfo::new("test", "abc123").model("test-model");
        let buffer = RlogBuffer::new(header);

        buffer.write_line("u: Hello").await;

        let content = buffer.to_string().await;

        assert!(content.starts_with("---\n"));
        assert!(content.contains("format: rlog/1"));
        assert!(content.contains("u: Hello"));
    }

    #[tokio::test]
    async fn test_buffer_with_notification() {
        let header = RlogHeaderInfo::new("test", "abc123");
        let buffer = RlogBuffer::new(header);

        let session_id = acp::SessionId::new("test");
        let notification = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("Hello world".to_string()),
            ))),
        );

        buffer.write_notification(&notification).await;

        let lines = buffer.lines().await;
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "u: Hello world");
    }

    #[test]
    fn test_generate_filename() {
        let filename = generate_rlog_filename();
        assert!(filename.ends_with(".rlog"));
        assert!(filename.len() > 10);
    }

    #[test]
    fn test_generate_path() {
        let path = generate_rlog_path("/base/logs");
        assert!(path.to_string_lossy().starts_with("/base/logs/"));
        assert!(path.to_string_lossy().ends_with(".rlog"));
    }
}
