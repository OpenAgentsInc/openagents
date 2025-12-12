//! Core types for the taskmaster crate
//!
//! This module contains all the data types used throughout taskmaster,
//! ported from Beads with SQLite storage in mind.

mod comment;
mod dependency;
mod event;
mod execution;
mod filter;
mod issue;
mod issue_type;
mod priority;
mod stats;
mod status;

// Re-export all types
pub use comment::{Comment, CommentCreate};
pub use dependency::{
    Dependency, DependencyRef, DependencyTree, DependencyTreeNode, DependencyType,
    ParseDependencyTypeError,
};
pub use event::{EventType, IssueEvent, ParseEventTypeError};
pub use execution::{
    ExecutionContext, ExecutionMode, ExecutionState, ParseExecutionModeError,
    ParseExecutionStateError,
};
pub use filter::{
    AssigneeFilter, DuplicateGroup, IssueFilter, LabelCount, LabelExpr, LabelFilter, SortPolicy,
    StaleFilter,
};
pub use issue::{
    IdMethod, Issue, IssueCreate, IssueUpdate, ValidationError, DEFAULT_TOMBSTONE_TTL_DAYS,
    MIN_TOMBSTONE_TTL_DAYS,
};
pub use issue_type::{IssueType, ParseIssueTypeError};
pub use priority::{ParsePriorityError, Priority};
pub use stats::{
    CleanupResult, CompactionResult, DoctorCategory, DoctorProblem, DoctorReport, DoctorSeverity,
    IssueStats, MigrationResult, PriorityCounts, RepairReport, StatsSnapshot, StatusCounts,
    TypeCounts,
};
pub use status::{IssueStatus, ParseStatusError};
