//! Issue type classification

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Issue type - categorizes the kind of work
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IssueType {
    /// Bug fix - something is broken
    Bug,
    /// New feature - new capability
    Feature,
    /// General task (default)
    #[default]
    Task,
    /// Epic - large multi-issue initiative
    Epic,
    /// Chore - maintenance, cleanup, tech debt
    Chore,
}

impl IssueType {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueType::Bug => "bug",
            IssueType::Feature => "feature",
            IssueType::Task => "task",
            IssueType::Epic => "epic",
            IssueType::Chore => "chore",
        }
    }

    /// Get human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            IssueType::Bug => "Bug",
            IssueType::Feature => "Feature",
            IssueType::Task => "Task",
            IssueType::Epic => "Epic",
            IssueType::Chore => "Chore",
        }
    }

    /// Get emoji representation
    pub fn emoji(&self) -> &'static str {
        match self {
            IssueType::Bug => "B",
            IssueType::Feature => "F",
            IssueType::Task => "T",
            IssueType::Epic => "E",
            IssueType::Chore => "C",
        }
    }

    /// Get all issue types
    pub fn all() -> &'static [IssueType] {
        &[
            IssueType::Bug,
            IssueType::Feature,
            IssueType::Task,
            IssueType::Epic,
            IssueType::Chore,
        ]
    }

    /// Check if this is a container type (can have children)
    pub fn is_container(&self) -> bool {
        matches!(self, IssueType::Epic)
    }
}

impl fmt::Display for IssueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for IssueType {
    type Err = ParseIssueTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bug" => Ok(IssueType::Bug),
            "feature" => Ok(IssueType::Feature),
            "task" => Ok(IssueType::Task),
            "epic" => Ok(IssueType::Epic),
            "chore" => Ok(IssueType::Chore),
            _ => Err(ParseIssueTypeError(s.to_string())),
        }
    }
}

/// Error when parsing an invalid issue type
#[derive(Debug, Clone)]
pub struct ParseIssueTypeError(pub String);

impl fmt::Display for ParseIssueTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid issue type '{}', expected one of: bug, feature, task, epic, chore",
            self.0
        )
    }
}

impl std::error::Error for ParseIssueTypeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_type_parse() {
        assert_eq!("bug".parse::<IssueType>().unwrap(), IssueType::Bug);
        assert_eq!("Feature".parse::<IssueType>().unwrap(), IssueType::Feature);
        assert_eq!("EPIC".parse::<IssueType>().unwrap(), IssueType::Epic);
        assert!("invalid".parse::<IssueType>().is_err());
    }

    #[test]
    fn test_is_container() {
        assert!(IssueType::Epic.is_container());
        assert!(!IssueType::Task.is_container());
        assert!(!IssueType::Bug.is_container());
    }
}
