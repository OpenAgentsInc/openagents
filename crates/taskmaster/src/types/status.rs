//! Issue status types

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Issue status - 5-state model with tombstone support
///
/// State transitions:
/// - open -> in_progress, blocked, closed
/// - in_progress -> open, blocked, closed
/// - blocked -> open, closed
/// - closed -> open (reopen)
/// - any -> tombstone (soft delete)
/// - tombstone -> any (restore)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    /// Not started, ready to work
    #[default]
    Open,
    /// Active work in progress
    InProgress,
    /// Waiting on dependency or external blocker
    Blocked,
    /// Completed/resolved
    Closed,
    /// Soft-deleted with TTL (Beads tombstone)
    Tombstone,
}

impl IssueStatus {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueStatus::Open => "open",
            IssueStatus::InProgress => "in_progress",
            IssueStatus::Blocked => "blocked",
            IssueStatus::Closed => "closed",
            IssueStatus::Tombstone => "tombstone",
        }
    }

    /// Check if this is an active (workable) status
    pub fn is_active(&self) -> bool {
        matches!(self, IssueStatus::Open | IssueStatus::InProgress)
    }

    /// Check if this is a terminal status
    pub fn is_terminal(&self) -> bool {
        matches!(self, IssueStatus::Closed | IssueStatus::Tombstone)
    }

    /// Check if transition to target status is valid
    pub fn can_transition_to(&self, target: IssueStatus) -> bool {
        match (self, target) {
            // Same status is always valid (no-op)
            (a, b) if *a == b => true,

            // From open
            (IssueStatus::Open, IssueStatus::InProgress) => true,
            (IssueStatus::Open, IssueStatus::Blocked) => true,
            (IssueStatus::Open, IssueStatus::Closed) => true,

            // From in_progress
            (IssueStatus::InProgress, IssueStatus::Open) => true,
            (IssueStatus::InProgress, IssueStatus::Blocked) => true,
            (IssueStatus::InProgress, IssueStatus::Closed) => true,

            // From blocked
            (IssueStatus::Blocked, IssueStatus::Open) => true,
            (IssueStatus::Blocked, IssueStatus::Closed) => true,

            // From closed (reopen)
            (IssueStatus::Closed, IssueStatus::Open) => true,

            // Any state can become tombstone (soft delete)
            (_, IssueStatus::Tombstone) => true,

            // Tombstone can be restored to any active state
            (IssueStatus::Tombstone, IssueStatus::Open) => true,
            (IssueStatus::Tombstone, IssueStatus::InProgress) => true,
            (IssueStatus::Tombstone, IssueStatus::Blocked) => true,
            (IssueStatus::Tombstone, IssueStatus::Closed) => true,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Get all valid statuses
    pub fn all() -> &'static [IssueStatus] {
        &[
            IssueStatus::Open,
            IssueStatus::InProgress,
            IssueStatus::Blocked,
            IssueStatus::Closed,
            IssueStatus::Tombstone,
        ]
    }

    /// Get only active (non-terminal) statuses
    pub fn active_statuses() -> &'static [IssueStatus] {
        &[
            IssueStatus::Open,
            IssueStatus::InProgress,
            IssueStatus::Blocked,
        ]
    }
}

impl fmt::Display for IssueStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for IssueStatus {
    type Err = ParseStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(IssueStatus::Open),
            "in_progress" => Ok(IssueStatus::InProgress),
            "blocked" => Ok(IssueStatus::Blocked),
            "closed" => Ok(IssueStatus::Closed),
            "tombstone" => Ok(IssueStatus::Tombstone),
            _ => Err(ParseStatusError(s.to_string())),
        }
    }
}

/// Error when parsing an invalid status string
#[derive(Debug, Clone)]
pub struct ParseStatusError(pub String);

impl fmt::Display for ParseStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid status '{}', expected one of: open, in_progress, blocked, closed, tombstone",
            self.0
        )
    }
}

impl std::error::Error for ParseStatusError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_transitions() {
        // Valid transitions
        assert!(IssueStatus::Open.can_transition_to(IssueStatus::InProgress));
        assert!(IssueStatus::Open.can_transition_to(IssueStatus::Closed));
        assert!(IssueStatus::Closed.can_transition_to(IssueStatus::Open));
        assert!(IssueStatus::InProgress.can_transition_to(IssueStatus::Tombstone));
        assert!(IssueStatus::Tombstone.can_transition_to(IssueStatus::Open));

        // Invalid transitions
        assert!(!IssueStatus::Blocked.can_transition_to(IssueStatus::InProgress));
        assert!(!IssueStatus::Closed.can_transition_to(IssueStatus::InProgress));
    }

    #[test]
    fn test_status_parse() {
        assert_eq!("open".parse::<IssueStatus>().unwrap(), IssueStatus::Open);
        assert_eq!(
            "in_progress".parse::<IssueStatus>().unwrap(),
            IssueStatus::InProgress
        );
        assert!("invalid".parse::<IssueStatus>().is_err());
    }
}
