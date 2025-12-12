//! Core Issue type - the main entity in the task system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

use super::dependency::DependencyRef;
use super::issue_type::IssueType;
use super::priority::Priority;
use super::status::IssueStatus;

/// Default tombstone TTL in days
pub const DEFAULT_TOMBSTONE_TTL_DAYS: u32 = 30;

/// Minimum tombstone TTL in days (to prevent accidental data loss)
pub const MIN_TOMBSTONE_TTL_DAYS: u32 = 7;

/// Full Issue entity with all 22+ fields from Beads
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    // =========================================================================
    // Primary Key
    // =========================================================================
    /// Unique issue ID (e.g., "tm-abc123", "tm-abc123.1" for children)
    pub id: String,

    // =========================================================================
    // Core Fields
    // =========================================================================
    /// Issue title (1-500 chars, required)
    pub title: String,

    /// Detailed description
    #[serde(default)]
    pub description: String,

    /// Design document or notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design: Option<String>,

    /// Acceptance criteria for verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_criteria: Option<String>,

    /// Free-form notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    // =========================================================================
    // Classification
    // =========================================================================
    /// Current status
    pub status: IssueStatus,

    /// Priority level (0 = highest)
    pub priority: Priority,

    /// Issue type classification
    #[serde(rename = "type")]
    pub issue_type: IssueType,

    // =========================================================================
    // Assignment
    // =========================================================================
    /// Assigned user or agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    // =========================================================================
    // Time Tracking
    // =========================================================================
    /// Estimated time in minutes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_minutes: Option<i32>,

    // =========================================================================
    // Compaction (Beads feature)
    // =========================================================================
    /// Compaction level (0 = not compacted)
    #[serde(default)]
    pub compaction_level: u32,

    // =========================================================================
    // Close Metadata
    // =========================================================================
    /// Reason provided when closing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_reason: Option<String>,

    // =========================================================================
    // Source Tracking
    // =========================================================================
    /// External reference (e.g., "gh-123", "jira-ABC")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_ref: Option<String>,

    /// Source repository name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_repo: Option<String>,

    /// Issue ID this was discovered from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovered_from: Option<String>,

    // =========================================================================
    // Deduplication
    // =========================================================================
    /// Content hash for deduplication (SHA256 of title+description)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,

    // =========================================================================
    // Timestamps
    // =========================================================================
    /// When created
    pub created_at: DateTime<Utc>,

    /// When last updated
    pub updated_at: DateTime<Utc>,

    /// When closed (if status is closed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Utc>>,

    // =========================================================================
    // Tombstone Fields (Beads soft-delete)
    // =========================================================================
    /// When tombstoned (soft-deleted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tombstoned_at: Option<DateTime<Utc>>,

    /// TTL for tombstone in days (default 30)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tombstone_ttl_days: Option<u32>,

    /// Reason for deletion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tombstone_reason: Option<String>,

    // =========================================================================
    // Relationships (denormalized for convenience)
    // =========================================================================
    /// Labels/tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,

    /// Dependencies
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<DependencyRef>,
}

impl Issue {
    /// Create a new issue with minimal required fields
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            title: title.into(),
            description: String::new(),
            design: None,
            acceptance_criteria: None,
            notes: None,
            status: IssueStatus::Open,
            priority: Priority::Medium,
            issue_type: IssueType::Task,
            assignee: None,
            estimated_minutes: None,
            compaction_level: 0,
            close_reason: None,
            external_ref: None,
            source_repo: None,
            discovered_from: None,
            content_hash: None,
            created_at: now,
            updated_at: now,
            closed_at: None,
            tombstoned_at: None,
            tombstone_ttl_days: None,
            tombstone_reason: None,
            labels: Vec::new(),
            deps: Vec::new(),
        }
    }

    /// Compute content hash from title and description
    pub fn compute_content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.title.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.description.as_bytes());
        hasher.update(b"\0");
        if let Some(design) = &self.design {
            hasher.update(design.as_bytes());
        }
        hasher.update(b"\0");
        if let Some(ac) = &self.acceptance_criteria {
            hasher.update(ac.as_bytes());
        }
        hasher.update(b"\0");
        if let Some(notes) = &self.notes {
            hasher.update(notes.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Check if issue is a tombstone (soft-deleted)
    pub fn is_tombstone(&self) -> bool {
        self.status == IssueStatus::Tombstone
    }

    /// Check if tombstone has expired its TTL
    pub fn is_tombstone_expired(&self) -> bool {
        if !self.is_tombstone() {
            return false;
        }

        let Some(tombstoned_at) = self.tombstoned_at else {
            return false;
        };

        let ttl_days = self.tombstone_ttl_days.unwrap_or(DEFAULT_TOMBSTONE_TTL_DAYS);
        let expiry = tombstoned_at + chrono::Duration::days(ttl_days as i64);
        Utc::now() > expiry
    }

    /// Validate issue data
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.title.is_empty() {
            return Err(ValidationError::TitleRequired);
        }
        if self.title.len() > 500 {
            return Err(ValidationError::TitleTooLong(self.title.len()));
        }

        // Closed issues must have closed_at
        if self.status == IssueStatus::Closed && self.closed_at.is_none() {
            return Err(ValidationError::ClosedWithoutTimestamp);
        }

        // Non-closed issues shouldn't have closed_at
        if self.status != IssueStatus::Closed && self.closed_at.is_some() {
            return Err(ValidationError::TimestampWithoutClosed);
        }

        // Tombstone issues must have tombstoned_at
        if self.status == IssueStatus::Tombstone && self.tombstoned_at.is_none() {
            return Err(ValidationError::TombstoneWithoutTimestamp);
        }

        // Non-tombstone issues shouldn't have tombstoned_at
        if self.status != IssueStatus::Tombstone && self.tombstoned_at.is_some() {
            return Err(ValidationError::TimestampWithoutTombstone);
        }

        if let Some(est) = self.estimated_minutes {
            if est < 0 {
                return Err(ValidationError::NegativeEstimate);
            }
        }

        Ok(())
    }

    /// Check if this issue has any blocking dependencies
    pub fn has_blocking_deps(&self) -> bool {
        self.deps.iter().any(|d| d.blocks_readiness())
    }
}

/// Validation errors for Issue
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    #[error("title is required")]
    TitleRequired,
    #[error("title must be 500 characters or less (got {0})")]
    TitleTooLong(usize),
    #[error("closed issues must have closed_at timestamp")]
    ClosedWithoutTimestamp,
    #[error("non-closed issues cannot have closed_at timestamp")]
    TimestampWithoutClosed,
    #[error("tombstone issues must have tombstoned_at timestamp")]
    TombstoneWithoutTimestamp,
    #[error("non-tombstone issues cannot have tombstoned_at timestamp")]
    TimestampWithoutTombstone,
    #[error("estimated_minutes cannot be negative")]
    NegativeEstimate,
}

/// Data for creating a new issue
#[derive(Debug, Clone, Default)]
pub struct IssueCreate {
    pub title: String,
    pub description: Option<String>,
    pub design: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub notes: Option<String>,
    pub priority: Priority,
    pub issue_type: IssueType,
    pub assignee: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub external_ref: Option<String>,
    pub source_repo: Option<String>,
    pub discovered_from: Option<String>,
    pub labels: Vec<String>,
    pub deps: Vec<DependencyRef>,
}

impl IssueCreate {
    /// Create a new IssueCreate with just a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    /// Set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set priority
    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Set issue type
    pub fn issue_type(mut self, issue_type: IssueType) -> Self {
        self.issue_type = issue_type;
        self
    }

    /// Set assignee
    pub fn assignee(mut self, assignee: impl Into<String>) -> Self {
        self.assignee = Some(assignee.into());
        self
    }

    /// Add a label
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Add a dependency
    pub fn dep(mut self, dep: DependencyRef) -> Self {
        self.deps.push(dep);
        self
    }
}

/// Data for updating an existing issue
#[derive(Debug, Clone, Default)]
pub struct IssueUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub design: Option<Option<String>>,
    pub acceptance_criteria: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub status: Option<IssueStatus>,
    pub priority: Option<Priority>,
    pub issue_type: Option<IssueType>,
    pub assignee: Option<Option<String>>,
    pub estimated_minutes: Option<Option<i32>>,
    pub close_reason: Option<Option<String>>,
    pub external_ref: Option<Option<String>>,
    pub labels: Option<Vec<String>>,
    pub deps: Option<Vec<DependencyRef>>,
}

impl IssueUpdate {
    /// Create an empty update
    pub fn new() -> Self {
        Self::default()
    }

    /// Set title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set status
    pub fn status(mut self, status: IssueStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Set priority
    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Set assignee
    pub fn assignee(mut self, assignee: Option<String>) -> Self {
        self.assignee = Some(assignee);
        self
    }

    /// Set close reason
    pub fn close_reason(mut self, reason: impl Into<String>) -> Self {
        self.close_reason = Some(Some(reason.into()));
        self
    }
}

/// ID generation method
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IdMethod {
    /// Hash-based (deterministic from title+description)
    Hash,
    /// Random UUID
    #[default]
    Random,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_new() {
        let issue = Issue::new("test-1", "Test Issue");
        assert_eq!(issue.id, "test-1");
        assert_eq!(issue.title, "Test Issue");
        assert_eq!(issue.status, IssueStatus::Open);
        assert_eq!(issue.priority, Priority::Medium);
    }

    #[test]
    fn test_issue_validate() {
        let issue = Issue::new("test-1", "Valid Title");
        assert!(issue.validate().is_ok());

        let mut empty_title = issue.clone();
        empty_title.title = String::new();
        assert!(matches!(
            empty_title.validate(),
            Err(ValidationError::TitleRequired)
        ));

        let mut long_title = issue.clone();
        long_title.title = "x".repeat(501);
        assert!(matches!(
            long_title.validate(),
            Err(ValidationError::TitleTooLong(_))
        ));
    }

    #[test]
    fn test_content_hash() {
        let issue1 = Issue::new("test-1", "Test Issue");
        let mut issue2 = Issue::new("test-2", "Test Issue");

        // Same content should have same hash
        assert_eq!(issue1.compute_content_hash(), issue2.compute_content_hash());

        // Different content should have different hash
        issue2.description = "Different description".to_string();
        assert_ne!(issue1.compute_content_hash(), issue2.compute_content_hash());
    }

    #[test]
    fn test_issue_create_builder() {
        let create = IssueCreate::new("Test Issue")
            .description("A description")
            .priority(Priority::High)
            .issue_type(IssueType::Bug)
            .assignee("alice")
            .label("urgent");

        assert_eq!(create.title, "Test Issue");
        assert_eq!(create.description, Some("A description".to_string()));
        assert_eq!(create.priority, Priority::High);
        assert_eq!(create.issue_type, IssueType::Bug);
        assert_eq!(create.assignee, Some("alice".to_string()));
        assert_eq!(create.labels, vec!["urgent".to_string()]);
    }
}
