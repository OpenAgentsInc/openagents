//! Task system for OpenAgents
//!
//! Implements user stories TASK-001 through TASK-034:
//! - Task CRUD operations (TASK-001..006, TASK-010..016, TASK-020..026)
//! - Dependency resolution (TASK-030..034)
//!
//! # Example
//!
//! ```no_run
//! use tasks::{TaskRepository, SqliteRepository, TaskCreate, TaskStatus, TaskPriority, TaskType};
//!
//! let repo = SqliteRepository::open(":memory:").unwrap();
//! repo.init().unwrap();
//!
//! let task = TaskCreate {
//!     title: "Implement feature".into(),
//!     description: Some("Add new capability".into()),
//!     priority: TaskPriority::High,
//!     task_type: TaskType::Feature,
//!     ..Default::default()
//! };
//!
//! let created = repo.create(task).unwrap();
//! println!("Created task: {}", created.id);
//!
//! // Get ready tasks (open, no blocking deps)
//! let ready = repo.ready_tasks(Default::default()).unwrap();
//! ```

mod types;
mod repository;
mod sqlite;

pub use types::*;
pub use repository::*;
pub use sqlite::*;
