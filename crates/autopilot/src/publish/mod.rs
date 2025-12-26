//! Replay Publishing Module
//!
//! Publishes replays for the demo funnel and homepage display.
//! Handles secret redaction before publishing.

use std::path::PathBuf;
use std::time::SystemTime;

pub mod publisher;
pub mod redactor;

pub use publisher::{PublishedReplay, ReplayPublisher, ReplayPublisherConfig};
pub use redactor::{RedactionPattern, SecretRedactor};

/// Publication status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicationStatus {
    /// Draft, not yet published
    Draft,
    /// Published and visible
    Published,
    /// Unlisted (accessible via link only)
    Unlisted,
    /// Archived
    Archived,
}

impl PublicationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PublicationStatus::Draft => "draft",
            PublicationStatus::Published => "published",
            PublicationStatus::Unlisted => "unlisted",
            PublicationStatus::Archived => "archived",
        }
    }
}

/// Metadata for a published replay
#[derive(Debug, Clone)]
pub struct ReplayMetadata {
    /// Unique identifier
    pub id: String,
    /// Title/description
    pub title: String,
    /// Repository (owner/repo)
    pub repository: Option<String>,
    /// Issue/PR reference
    pub issue_ref: Option<String>,
    /// When the replay was created
    pub created_at: u64,
    /// Duration in seconds
    pub duration_secs: u32,
    /// Number of steps in the replay
    pub step_count: u32,
    /// Number of tool calls
    pub tool_call_count: u32,
    /// Total tokens used
    pub total_tokens: u64,
    /// Publication status
    pub status: PublicationStatus,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl ReplayMetadata {
    /// Create new metadata with generated ID
    pub fn new(title: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id: format!("replay_{}", now),
            title: title.into(),
            repository: None,
            issue_ref: None,
            created_at: now,
            duration_secs: 0,
            step_count: 0,
            tool_call_count: 0,
            total_tokens: 0,
            status: PublicationStatus::Draft,
            tags: Vec::new(),
        }
    }

    /// Set repository
    pub fn with_repository(mut self, repo: impl Into<String>) -> Self {
        self.repository = Some(repo.into());
        self
    }

    /// Set issue reference
    pub fn with_issue(mut self, issue: impl Into<String>) -> Self {
        self.issue_ref = Some(issue.into());
        self
    }

    /// Set statistics
    pub fn with_stats(
        mut self,
        duration_secs: u32,
        step_count: u32,
        tool_call_count: u32,
        total_tokens: u64,
    ) -> Self {
        self.duration_secs = duration_secs;
        self.step_count = step_count;
        self.tool_call_count = tool_call_count;
        self.total_tokens = total_tokens;
        self
    }

    /// Add a tag
    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set status
    pub fn with_status(mut self, status: PublicationStatus) -> Self {
        self.status = status;
        self
    }

    /// Check if the replay is public
    pub fn is_public(&self) -> bool {
        matches!(
            self.status,
            PublicationStatus::Published | PublicationStatus::Unlisted
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publication_status() {
        assert_eq!(PublicationStatus::Published.as_str(), "published");
        assert_eq!(PublicationStatus::Draft.as_str(), "draft");
    }

    #[test]
    fn test_replay_metadata_new() {
        let meta = ReplayMetadata::new("Test Replay");
        assert!(meta.id.starts_with("replay_"));
        assert_eq!(meta.title, "Test Replay");
        assert!(meta.created_at > 0);
    }

    #[test]
    fn test_replay_metadata_builder() {
        let meta = ReplayMetadata::new("Test")
            .with_repository("owner/repo")
            .with_issue("#123")
            .with_stats(60, 10, 5, 5000)
            .add_tag("demo")
            .with_status(PublicationStatus::Published);

        assert_eq!(meta.repository, Some("owner/repo".to_string()));
        assert_eq!(meta.issue_ref, Some("#123".to_string()));
        assert_eq!(meta.duration_secs, 60);
        assert!(meta.tags.contains(&"demo".to_string()));
        assert!(meta.is_public());
    }

    #[test]
    fn test_is_public() {
        let meta = ReplayMetadata::new("Test");
        assert!(!meta.is_public()); // Draft

        let meta = meta.with_status(PublicationStatus::Published);
        assert!(meta.is_public());

        let meta = ReplayMetadata::new("Test").with_status(PublicationStatus::Unlisted);
        assert!(meta.is_public());

        let meta = ReplayMetadata::new("Test").with_status(PublicationStatus::Archived);
        assert!(!meta.is_public());
    }
}
