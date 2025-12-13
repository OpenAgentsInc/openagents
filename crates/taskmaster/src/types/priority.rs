//! Issue priority types

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Issue priority - 0 is highest (critical), 4 is lowest (backlog)
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[repr(u8)]
pub enum Priority {
    /// P0 - Critical, drop everything
    Critical = 0,
    /// P1 - High priority
    High = 1,
    /// P2 - Medium priority (default)
    #[default]
    Medium = 2,
    /// P3 - Low priority
    Low = 3,
    /// P4 - Backlog, do when time permits
    Backlog = 4,
}

impl Priority {
    /// Get numeric value (0-4)
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Create from numeric value
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Priority::Critical),
            1 => Some(Priority::High),
            2 => Some(Priority::Medium),
            3 => Some(Priority::Low),
            4 => Some(Priority::Backlog),
            _ => None,
        }
    }

    /// Get the string representation (P0, P1, etc.)
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Critical => "P0",
            Priority::High => "P1",
            Priority::Medium => "P2",
            Priority::Low => "P3",
            Priority::Backlog => "P4",
        }
    }

    /// Get human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            Priority::Critical => "Critical",
            Priority::High => "High",
            Priority::Medium => "Medium",
            Priority::Low => "Low",
            Priority::Backlog => "Backlog",
        }
    }

    /// Get all priorities in order (highest to lowest)
    pub fn all() -> &'static [Priority] {
        &[
            Priority::Critical,
            Priority::High,
            Priority::Medium,
            Priority::Low,
            Priority::Backlog,
        ]
    }

    /// Check if this is a high priority (P0 or P1)
    pub fn is_high(&self) -> bool {
        matches!(self, Priority::Critical | Priority::High)
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Priority {
    type Err = ParsePriorityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "0" | "p0" | "critical" => Ok(Priority::Critical),
            "1" | "p1" | "high" => Ok(Priority::High),
            "2" | "p2" | "medium" => Ok(Priority::Medium),
            "3" | "p3" | "low" => Ok(Priority::Low),
            "4" | "p4" | "backlog" => Ok(Priority::Backlog),
            _ => Err(ParsePriorityError(s.to_string())),
        }
    }
}

impl From<Priority> for i32 {
    fn from(p: Priority) -> i32 {
        p.as_u8() as i32
    }
}

impl TryFrom<i32> for Priority {
    type Error = ParsePriorityError;

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        if v < 0 || v > 4 {
            return Err(ParsePriorityError(v.to_string()));
        }
        Priority::from_u8(v as u8).ok_or_else(|| ParsePriorityError(v.to_string()))
    }
}

/// Error when parsing an invalid priority
#[derive(Debug, Clone)]
pub struct ParsePriorityError(pub String);

impl fmt::Display for ParsePriorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid priority '{}', expected 0-4, P0-P4, or critical/high/medium/low/backlog",
            self.0
        )
    }
}

impl std::error::Error for ParsePriorityError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical < Priority::High);
        assert!(Priority::High < Priority::Medium);
        assert!(Priority::Medium < Priority::Low);
        assert!(Priority::Low < Priority::Backlog);
    }

    #[test]
    fn test_priority_parse() {
        assert_eq!("P0".parse::<Priority>().unwrap(), Priority::Critical);
        assert_eq!("p1".parse::<Priority>().unwrap(), Priority::High);
        assert_eq!("2".parse::<Priority>().unwrap(), Priority::Medium);
        assert_eq!("critical".parse::<Priority>().unwrap(), Priority::Critical);
        assert!("invalid".parse::<Priority>().is_err());
        assert!("5".parse::<Priority>().is_err());
    }

    #[test]
    fn test_priority_from_u8() {
        assert_eq!(Priority::from_u8(0), Some(Priority::Critical));
        assert_eq!(Priority::from_u8(4), Some(Priority::Backlog));
        assert_eq!(Priority::from_u8(5), None);
    }
}
