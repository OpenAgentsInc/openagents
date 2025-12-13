//! Statistics types for issue analytics

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::issue_type::IssueType;
use super::priority::Priority;
use super::status::IssueStatus;

/// Current statistics for the issue database
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueStats {
    /// Total number of issues (excluding tombstones)
    pub total_issues: usize,
    /// Issues by status
    pub by_status: StatusCounts,
    /// Issues by priority
    pub by_priority: PriorityCounts,
    /// Issues by type
    pub by_type: TypeCounts,
    /// Number of ready issues (open with no blockers)
    pub ready_issues: usize,
    /// Number of tombstoned issues
    pub tombstone_issues: usize,
    /// Average time to close (hours)
    pub avg_time_to_close_hours: Option<f64>,
    /// When these stats were computed
    pub computed_at: DateTime<Utc>,
}

impl IssueStats {
    /// Create empty stats
    pub fn new() -> Self {
        Self {
            computed_at: Utc::now(),
            ..Default::default()
        }
    }
}

/// Issue counts by status
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusCounts {
    pub open: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub closed: usize,
    pub tombstone: usize,
}

impl StatusCounts {
    /// Get count for a specific status
    pub fn get(&self, status: IssueStatus) -> usize {
        match status {
            IssueStatus::Open => self.open,
            IssueStatus::InProgress => self.in_progress,
            IssueStatus::Blocked => self.blocked,
            IssueStatus::Closed => self.closed,
            IssueStatus::Tombstone => self.tombstone,
        }
    }

    /// Set count for a specific status
    pub fn set(&mut self, status: IssueStatus, count: usize) {
        match status {
            IssueStatus::Open => self.open = count,
            IssueStatus::InProgress => self.in_progress = count,
            IssueStatus::Blocked => self.blocked = count,
            IssueStatus::Closed => self.closed = count,
            IssueStatus::Tombstone => self.tombstone = count,
        }
    }

    /// Get total active (non-terminal) issues
    pub fn active(&self) -> usize {
        self.open + self.in_progress + self.blocked
    }
}

/// Issue counts by priority
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PriorityCounts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub backlog: usize,
}

impl PriorityCounts {
    /// Get count for a specific priority
    pub fn get(&self, priority: Priority) -> usize {
        match priority {
            Priority::Critical => self.critical,
            Priority::High => self.high,
            Priority::Medium => self.medium,
            Priority::Low => self.low,
            Priority::Backlog => self.backlog,
        }
    }

    /// Set count for a specific priority
    pub fn set(&mut self, priority: Priority, count: usize) {
        match priority {
            Priority::Critical => self.critical = count,
            Priority::High => self.high = count,
            Priority::Medium => self.medium = count,
            Priority::Low => self.low = count,
            Priority::Backlog => self.backlog = count,
        }
    }

    /// Get total high priority (P0 + P1) issues
    pub fn high_priority(&self) -> usize {
        self.critical + self.high
    }
}

/// Issue counts by type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeCounts {
    pub bug: usize,
    pub feature: usize,
    pub task: usize,
    pub epic: usize,
    pub chore: usize,
}

impl TypeCounts {
    /// Get count for a specific type
    pub fn get(&self, issue_type: IssueType) -> usize {
        match issue_type {
            IssueType::Bug => self.bug,
            IssueType::Feature => self.feature,
            IssueType::Task => self.task,
            IssueType::Epic => self.epic,
            IssueType::Chore => self.chore,
        }
    }

    /// Set count for a specific type
    pub fn set(&mut self, issue_type: IssueType, count: usize) {
        match issue_type {
            IssueType::Bug => self.bug = count,
            IssueType::Feature => self.feature = count,
            IssueType::Task => self.task = count,
            IssueType::Epic => self.epic = count,
            IssueType::Chore => self.chore = count,
        }
    }
}

/// Statistics snapshot for historical tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    /// Snapshot ID
    pub id: i64,
    /// Date of snapshot
    pub snapshot_date: DateTime<Utc>,
    /// Total issues at snapshot time
    pub total_issues: usize,
    /// Open count
    pub open_count: usize,
    /// In progress count
    pub in_progress_count: usize,
    /// Blocked count
    pub blocked_count: usize,
    /// Closed count
    pub closed_count: usize,
    /// Tombstone count
    pub tombstone_count: usize,
    /// Average time to close (hours)
    pub avg_time_to_close_hours: Option<f64>,
    /// Label distribution as JSON
    pub labels_json: Option<String>,
    /// Priority distribution as JSON
    pub priority_json: Option<String>,
    /// When this snapshot was created
    pub created_at: DateTime<Utc>,
}

/// Result of compaction operation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompactionResult {
    /// Number of issues compacted
    pub compacted_count: usize,
    /// Bytes saved
    pub bytes_saved: usize,
    /// Issues that were skipped (too recent)
    pub skipped_count: usize,
    /// Errors encountered
    pub errors: Vec<String>,
}

/// Result of tombstone cleanup operation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CleanupResult {
    /// Number of tombstones purged
    pub purged_count: usize,
    /// Number of tombstones still within TTL
    pub retained_count: usize,
    /// Errors encountered
    pub errors: Vec<String>,
}

/// Problem found by doctor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorProblem {
    /// Problem ID
    pub id: String,
    /// Issue ID (if applicable)
    pub issue_id: Option<String>,
    /// Problem category
    pub category: DoctorCategory,
    /// Human-readable description
    pub description: String,
    /// Severity level
    pub severity: DoctorSeverity,
    /// Whether this can be auto-repaired
    pub repairable: bool,
}

/// Category of doctor problems
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorCategory {
    /// Invalid issue data
    InvalidData,
    /// Orphan dependency (references non-existent issue)
    OrphanDependency,
    /// Circular dependency
    CircularDependency,
    /// Missing timestamp
    MissingTimestamp,
    /// Inconsistent state
    InconsistentState,
    /// Stale in_progress
    StaleInProgress,
    /// Duplicate content hash
    DuplicateContent,
}

/// Severity of doctor problems
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorSeverity {
    /// Informational only
    Info,
    /// Warning - should be fixed
    Warning,
    /// Error - must be fixed
    Error,
    /// Critical - data integrity at risk
    Critical,
}

/// Doctor report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DoctorReport {
    /// Problems found
    pub problems: Vec<DoctorProblem>,
    /// Total issues checked
    pub issues_checked: usize,
    /// Total dependencies checked
    pub dependencies_checked: usize,
    /// Whether the database is healthy
    pub healthy: bool,
    /// When the check was run
    pub checked_at: DateTime<Utc>,
}

impl DoctorReport {
    /// Create a new healthy report
    pub fn new() -> Self {
        Self {
            checked_at: Utc::now(),
            healthy: true,
            ..Default::default()
        }
    }

    /// Add a problem
    pub fn add_problem(&mut self, problem: DoctorProblem) {
        if problem.severity >= DoctorSeverity::Error {
            self.healthy = false;
        }
        self.problems.push(problem);
    }

    /// Get count by severity
    pub fn count_by_severity(&self, severity: DoctorSeverity) -> usize {
        self.problems
            .iter()
            .filter(|p| p.severity == severity)
            .count()
    }

    /// Get repairable problems
    pub fn repairable(&self) -> Vec<&DoctorProblem> {
        self.problems.iter().filter(|p| p.repairable).collect()
    }
}

/// Repair report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepairReport {
    /// Problems that were repaired
    pub repaired: Vec<String>,
    /// Problems that failed to repair
    pub failed: Vec<(String, String)>,
    /// When the repair was run
    pub repaired_at: DateTime<Utc>,
}

impl RepairReport {
    /// Create a new repair report
    pub fn new() -> Self {
        Self {
            repaired_at: Utc::now(),
            ..Default::default()
        }
    }
}

/// Migration report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MigrationResult {
    /// Migrations applied
    pub applied: Vec<String>,
    /// Current schema version
    pub current_version: u32,
    /// Whether migration succeeded
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_counts() {
        let mut counts = StatusCounts::default();
        counts.set(IssueStatus::Open, 10);
        counts.set(IssueStatus::InProgress, 5);
        counts.set(IssueStatus::Blocked, 2);
        counts.set(IssueStatus::Closed, 100);

        assert_eq!(counts.get(IssueStatus::Open), 10);
        assert_eq!(counts.active(), 17);
    }

    #[test]
    fn test_doctor_report() {
        let mut report = DoctorReport::new();
        assert!(report.healthy);

        report.add_problem(DoctorProblem {
            id: "p1".to_string(),
            issue_id: Some("i1".to_string()),
            category: DoctorCategory::OrphanDependency,
            description: "Orphan dep".to_string(),
            severity: DoctorSeverity::Warning,
            repairable: true,
        });
        assert!(report.healthy); // Still healthy (only warning)

        report.add_problem(DoctorProblem {
            id: "p2".to_string(),
            issue_id: Some("i2".to_string()),
            category: DoctorCategory::InvalidData,
            description: "Invalid data".to_string(),
            severity: DoctorSeverity::Error,
            repairable: false,
        });
        assert!(!report.healthy); // Now unhealthy (error)

        assert_eq!(report.count_by_severity(DoctorSeverity::Warning), 1);
        assert_eq!(report.count_by_severity(DoctorSeverity::Error), 1);
        assert_eq!(report.repairable().len(), 1);
    }
}
