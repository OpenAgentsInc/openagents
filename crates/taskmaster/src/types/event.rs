//! Event types for audit trail

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Event type - categorizes audit trail events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Issue was created
    Created,
    /// Issue was updated
    Updated,
    /// Status changed
    StatusChanged,
    /// Comment was added
    Commented,
    /// Issue was closed
    Closed,
    /// Issue was reopened
    Reopened,
    /// Dependency was added
    DependencyAdded,
    /// Dependency was removed
    DependencyRemoved,
    /// Label was added
    LabelAdded,
    /// Label was removed
    LabelRemoved,
    /// Issue was compacted
    Compacted,
    /// Issue was tombstoned (soft-deleted)
    Tombstoned,
    /// Issue was restored from tombstone
    Restored,
    /// Issue was purged (permanently deleted)
    Purged,
    /// Issue was migrated from old system
    Migrated,
}

impl EventType {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Created => "created",
            EventType::Updated => "updated",
            EventType::StatusChanged => "status_changed",
            EventType::Commented => "commented",
            EventType::Closed => "closed",
            EventType::Reopened => "reopened",
            EventType::DependencyAdded => "dependency_added",
            EventType::DependencyRemoved => "dependency_removed",
            EventType::LabelAdded => "label_added",
            EventType::LabelRemoved => "label_removed",
            EventType::Compacted => "compacted",
            EventType::Tombstoned => "tombstoned",
            EventType::Restored => "restored",
            EventType::Purged => "purged",
            EventType::Migrated => "migrated",
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            EventType::Created => "Issue created",
            EventType::Updated => "Issue updated",
            EventType::StatusChanged => "Status changed",
            EventType::Commented => "Comment added",
            EventType::Closed => "Issue closed",
            EventType::Reopened => "Issue reopened",
            EventType::DependencyAdded => "Dependency added",
            EventType::DependencyRemoved => "Dependency removed",
            EventType::LabelAdded => "Label added",
            EventType::LabelRemoved => "Label removed",
            EventType::Compacted => "Issue compacted",
            EventType::Tombstoned => "Issue deleted",
            EventType::Restored => "Issue restored",
            EventType::Purged => "Issue permanently deleted",
            EventType::Migrated => "Issue migrated",
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for EventType {
    type Err = ParseEventTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "created" => Ok(EventType::Created),
            "updated" => Ok(EventType::Updated),
            "status_changed" => Ok(EventType::StatusChanged),
            "commented" => Ok(EventType::Commented),
            "closed" => Ok(EventType::Closed),
            "reopened" => Ok(EventType::Reopened),
            "dependency_added" => Ok(EventType::DependencyAdded),
            "dependency_removed" => Ok(EventType::DependencyRemoved),
            "label_added" => Ok(EventType::LabelAdded),
            "label_removed" => Ok(EventType::LabelRemoved),
            "compacted" => Ok(EventType::Compacted),
            "tombstoned" => Ok(EventType::Tombstoned),
            "restored" => Ok(EventType::Restored),
            "purged" => Ok(EventType::Purged),
            "migrated" => Ok(EventType::Migrated),
            _ => Err(ParseEventTypeError(s.to_string())),
        }
    }
}

/// Error when parsing an invalid event type
#[derive(Debug, Clone)]
pub struct ParseEventTypeError(pub String);

impl fmt::Display for ParseEventTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid event type '{}'", self.0)
    }
}

impl std::error::Error for ParseEventTypeError {}

/// An audit event for an issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueEvent {
    /// Unique event ID (auto-increment)
    pub id: i64,
    /// Issue this event belongs to
    pub issue_id: String,
    /// Type of event
    pub event_type: EventType,
    /// Actor who triggered the event (user/agent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// Field that was changed (for updates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_name: Option<String>,
    /// Old value (for updates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<String>,
    /// New value (for updates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<String>,
    /// Additional metadata as JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
    /// When the event occurred
    pub created_at: DateTime<Utc>,
}

impl IssueEvent {
    /// Create a new event
    pub fn new(issue_id: impl Into<String>, event_type: EventType) -> Self {
        Self {
            id: 0, // Will be set by database
            issue_id: issue_id.into(),
            event_type,
            actor: None,
            field_name: None,
            old_value: None,
            new_value: None,
            metadata: None,
            created_at: Utc::now(),
        }
    }

    /// Set actor
    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Set field change
    pub fn field_change(
        mut self,
        field: impl Into<String>,
        old: Option<String>,
        new: Option<String>,
    ) -> Self {
        self.field_name = Some(field.into());
        self.old_value = old;
        self.new_value = new;
        self
    }

    /// Set metadata
    pub fn metadata(mut self, meta: impl Into<String>) -> Self {
        self.metadata = Some(meta.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_parse() {
        assert_eq!("created".parse::<EventType>().unwrap(), EventType::Created);
        assert_eq!(
            "status_changed".parse::<EventType>().unwrap(),
            EventType::StatusChanged
        );
        assert!("invalid".parse::<EventType>().is_err());
    }

    #[test]
    fn test_issue_event_builder() {
        let event = IssueEvent::new("issue-1", EventType::StatusChanged)
            .actor("alice")
            .field_change("status", Some("open".into()), Some("closed".into()));

        assert_eq!(event.issue_id, "issue-1");
        assert_eq!(event.event_type, EventType::StatusChanged);
        assert_eq!(event.actor, Some("alice".to_string()));
        assert_eq!(event.field_name, Some("status".to_string()));
        assert_eq!(event.old_value, Some("open".to_string()));
        assert_eq!(event.new_value, Some("closed".to_string()));
    }
}
