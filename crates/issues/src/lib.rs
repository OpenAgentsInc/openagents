//! Local issue tracking with SQLite
//!
//! This crate provides a simple issue tracking system backed by SQLite,
//! designed for use with autonomous agents like autopilot.
//!
//! # Example
//!
//! ```no_run
//! use issues::{db, issue};
//! use std::path::Path;
//!
//! // Initialize database
//! let conn = db::init_db(Path::new("autopilot.db")).unwrap();
//!
//! // Create an issue
//! let issue = issue::create_issue(
//!     &conn,
//!     "Fix the bug",
//!     Some("It crashes on startup"),
//!     issue::Priority::High,
//!     issue::IssueType::Bug,
//!     None, // agent (defaults to "claude")
//!     None, // directive_id
//!     None, // project_id
//! ).unwrap();
//!
//! // Claim the issue
//! issue::claim_issue(&conn, &issue.id, "run-123").unwrap();
//!
//! // Complete the issue
//! issue::complete_issue(&conn, &issue.id).unwrap();
//! ```

pub mod cache;
pub mod db;
pub mod directive;
pub mod issue;
pub mod project;
pub mod retry;
pub mod session;
pub mod validation;

// Re-export commonly used types
pub use cache::{CacheConfig, CacheStats, IssueCache};
pub use db::{init_db, init_memory_db};
pub use directive::{
    Directive, DirectiveError, DirectivePriority, DirectiveProgress, DirectiveStatus,
};
pub use issue::{Issue, IssueType, Priority, Status};
pub use project::Project;
pub use retry::with_retry;
pub use session::{Session, SessionStatus};
pub use validation::{ValidationError, validate_agent, validate_description, validate_title};
