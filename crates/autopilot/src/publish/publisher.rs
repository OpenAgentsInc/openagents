//! Replay Publisher
//!
//! Publishes replays to storage for the demo funnel.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::{PublicationStatus, ReplayMetadata, SecretRedactor};

/// Configuration for the replay publisher
#[derive(Debug, Clone)]
pub struct ReplayPublisherConfig {
    /// Base storage path for published replays
    pub storage_path: PathBuf,
    /// Base URL for accessing published replays
    pub base_url: String,
    /// Enable secret redaction
    pub redact_secrets: bool,
    /// Maximum file size to publish (bytes)
    pub max_file_size: u64,
}

impl Default for ReplayPublisherConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from("published_replays"),
            base_url: "https://openagents.com/replays".to_string(),
            redact_secrets: true,
            max_file_size: 10 * 1024 * 1024, // 10 MB
        }
    }
}

impl ReplayPublisherConfig {
    /// Create with custom storage path
    pub fn with_storage_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.storage_path = path.into();
        self
    }

    /// Create with custom base URL
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Disable secret redaction (not recommended)
    pub fn without_redaction(mut self) -> Self {
        self.redact_secrets = false;
        self
    }
}

/// A published replay
#[derive(Debug, Clone)]
pub struct PublishedReplay {
    /// Metadata about the replay
    pub metadata: ReplayMetadata,
    /// Public URL to access the replay
    pub url: String,
    /// Storage path
    pub storage_path: PathBuf,
    /// Original file path
    pub original_path: PathBuf,
    /// Size of the published file
    pub file_size: u64,
    /// Number of secrets redacted
    pub secrets_redacted: u32,
    /// When published
    pub published_at: u64,
}

impl PublishedReplay {
    /// Get the share URL
    pub fn share_url(&self) -> String {
        self.url.clone()
    }

    /// Check if any secrets were redacted
    pub fn had_secrets(&self) -> bool {
        self.secrets_redacted > 0
    }
}

/// Replay publisher for the demo funnel
pub struct ReplayPublisher {
    config: ReplayPublisherConfig,
    redactor: SecretRedactor,
}

impl ReplayPublisher {
    /// Create a new replay publisher
    pub fn new(config: ReplayPublisherConfig) -> Self {
        Self {
            config,
            redactor: SecretRedactor::default(),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(ReplayPublisherConfig::default())
    }

    /// Publish a replay file
    ///
    /// This is a mock implementation for now.
    /// In production, it would upload to storage.
    pub fn publish(
        &self,
        replay_path: &Path,
        metadata: ReplayMetadata,
    ) -> Result<PublishedReplay, PublishError> {
        // Verify file exists
        if !replay_path.exists() {
            return Err(PublishError::FileNotFound(replay_path.to_path_buf()));
        }

        // Check file size
        let file_size = std::fs::metadata(replay_path)
            .map(|m| m.len())
            .unwrap_or(0);
        if file_size > self.config.max_file_size {
            return Err(PublishError::FileTooLarge {
                size: file_size,
                max: self.config.max_file_size,
            });
        }

        // Read content
        let content = std::fs::read_to_string(replay_path)
            .map_err(|e| PublishError::IoError(e.to_string()))?;

        // Redact secrets if enabled
        let (redacted_content, secrets_redacted) = if self.config.redact_secrets {
            let redacted = self.redactor.redact(&content);
            let count = self.redactor.count_redactions(&content);
            (redacted, count as u32)
        } else {
            (content, 0)
        };

        // Generate storage path
        let storage_filename = format!("{}.rlog", metadata.id);
        let storage_path = self.config.storage_path.join(&storage_filename);

        // Simulate writing to storage
        // In production, this would upload to cloud storage

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let url = format!("{}/{}", self.config.base_url, metadata.id);

        Ok(PublishedReplay {
            metadata: metadata.with_status(PublicationStatus::Published),
            url,
            storage_path,
            original_path: replay_path.to_path_buf(),
            file_size: redacted_content.len() as u64,
            secrets_redacted,
            published_at: now,
        })
    }

    /// Publish a replay from content string
    pub fn publish_content(
        &self,
        content: &str,
        metadata: ReplayMetadata,
    ) -> Result<PublishedReplay, PublishError> {
        // Check content size
        let content_size = content.len() as u64;
        if content_size > self.config.max_file_size {
            return Err(PublishError::FileTooLarge {
                size: content_size,
                max: self.config.max_file_size,
            });
        }

        // Redact secrets if enabled
        let (redacted_content, secrets_redacted) = if self.config.redact_secrets {
            let redacted = self.redactor.redact(content);
            let count = self.redactor.count_redactions(content);
            (redacted, count as u32)
        } else {
            (content.to_string(), 0)
        };

        let storage_filename = format!("{}.rlog", metadata.id);
        let storage_path = self.config.storage_path.join(&storage_filename);

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let url = format!("{}/{}", self.config.base_url, metadata.id);

        Ok(PublishedReplay {
            metadata: metadata.with_status(PublicationStatus::Published),
            url,
            storage_path,
            original_path: PathBuf::new(),
            file_size: redacted_content.len() as u64,
            secrets_redacted,
            published_at: now,
        })
    }

    /// Unpublish a replay
    pub fn unpublish(&self, replay_id: &str) -> Result<(), PublishError> {
        // In production, this would remove from storage or update status
        Ok(())
    }

    /// Get the URL for a replay ID
    pub fn get_url(&self, replay_id: &str) -> String {
        format!("{}/{}", self.config.base_url, replay_id)
    }
}

/// Errors that can occur during publishing
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("File too large: {size} bytes (max: {max} bytes)")]
    FileTooLarge { size: u64, max: u64 },

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Already published")]
    AlreadyPublished,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ReplayPublisherConfig::default();
        assert!(config.redact_secrets);
        assert_eq!(config.max_file_size, 10 * 1024 * 1024);
    }

    #[test]
    fn test_config_builder() {
        let config = ReplayPublisherConfig::default()
            .with_storage_path("/tmp/replays")
            .with_base_url("https://example.com")
            .without_redaction();

        assert_eq!(config.storage_path, PathBuf::from("/tmp/replays"));
        assert_eq!(config.base_url, "https://example.com");
        assert!(!config.redact_secrets);
    }

    #[test]
    fn test_publisher_get_url() {
        let publisher = ReplayPublisher::default_config();
        let url = publisher.get_url("replay_123");
        assert!(url.contains("replay_123"));
    }

    #[test]
    fn test_publish_content() {
        let publisher = ReplayPublisher::default_config();
        let metadata = ReplayMetadata::new("Test Replay");

        let content = "This is a test replay content";
        let result = publisher.publish_content(content, metadata);

        assert!(result.is_ok());
        let published = result.unwrap();
        assert!(published.url.contains("replay_"));
        assert!(published.metadata.is_public());
    }

    #[test]
    fn test_publish_content_with_secrets() {
        let publisher = ReplayPublisher::default_config();
        let metadata = ReplayMetadata::new("Test Replay");

        // Use realistic-length tokens that match the regex patterns
        let content = "Using API key: sk-abcdefghijklmnopqrstuvwxyz0123456789 and token ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let result = publisher.publish_content(content, metadata);

        assert!(result.is_ok());
        let published = result.unwrap();
        assert!(published.had_secrets());
        assert!(published.secrets_redacted > 0);
    }

    #[test]
    fn test_publish_content_too_large() {
        let config = ReplayPublisherConfig::default();
        let mut config = config;
        config.max_file_size = 10; // Very small limit

        let publisher = ReplayPublisher::new(config);
        let metadata = ReplayMetadata::new("Test");

        let content = "This content is definitely longer than 10 bytes";
        let result = publisher.publish_content(content, metadata);

        assert!(matches!(result, Err(PublishError::FileTooLarge { .. })));
    }

    #[test]
    fn test_published_replay_share_url() {
        let publisher = ReplayPublisher::default_config();
        let metadata = ReplayMetadata::new("Test");

        let published = publisher.publish_content("test", metadata).unwrap();
        let share_url = published.share_url();

        assert!(!share_url.is_empty());
        assert!(share_url.starts_with("https://"));
    }
}
