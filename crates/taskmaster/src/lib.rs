//! Taskmaster - Full-featured issue tracker for OpenAgents
//!
//! This crate provides a complete issue tracking system ported from Beads,
//! with SQLite storage backend.
//!
//! # Features
//!
//! - **Full Issue Lifecycle**: open → in_progress → blocked → closed → tombstone
//! - **Rich Filtering**: AND/OR label filtering, date ranges, full-text search
//! - **Dependency Management**: blocks, related, parent-child, discovered-from
//! - **Tombstone Support**: Soft delete with TTL-based expiration
//! - **Compaction**: Archive old closed issues
//! - **Events/Audit Trail**: Track all mutations
//! - **Statistics**: Aggregate metrics and health checks
//!
//! # Example
//!
//! ```no_run
//! use taskmaster::{SqliteRepository, IssueRepository, IssueCreate, Priority};
//!
//! let repo = SqliteRepository::open("taskmaster.db").unwrap();
//!
//! // Create an issue
//! let issue = repo.create(
//!     IssueCreate::new("Fix the bug")
//!         .description("Something is broken")
//!         .priority(Priority::High)
//!         .label("urgent"),
//!     "tm"
//! ).unwrap();
//!
//! // Start working
//! repo.start(&issue.id, Some("alice")).unwrap();
//!
//! // Close when done
//! repo.close(&issue.id, Some("Fixed in PR #123"), vec![], Some("alice")).unwrap();
//! ```

pub mod plan_to_tasks;
pub mod repository;
pub mod storage;
pub mod types;

// Re-export main types
pub use repository::{IssueRepository, Result, TaskmasterError};
pub use storage::SqliteRepository;
pub use types::*;
