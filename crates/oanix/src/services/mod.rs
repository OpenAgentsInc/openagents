//! FileService implementations
//!
//! Concrete implementations of the FileService trait for use in OANIX namespaces.
//!
//! ## Primitive Filesystems
//!
//! - [`MemFs`] - In-memory read/write filesystem
//! - [`MapFs`] - Static/immutable filesystem from embedded data
//! - [`FuncFs`] - Dynamic files computed via closures
//! - [`CowFs`] - Copy-on-Write overlay for snapshots
//!
//! ## Standard Services
//!
//! - [`TaskFs`] - Task specification, status, and results
//! - [`LogsFs`] - Structured logging with stdout/stderr/events
//! - [`WorkspaceFs`] - Real filesystem wrapper (native only)

mod cow_fs;
mod func_fs;
mod logs_fs;
mod map_fs;
mod mem_fs;
mod task_fs;

#[cfg(not(target_arch = "wasm32"))]
mod workspace_fs;

pub use cow_fs::CowFs;
pub use func_fs::{FuncFs, ReadFn, WriteFn};
pub use logs_fs::{LogEvent, LogLevel, LogsFs};
pub use map_fs::{MapFs, MapFsBuilder};
pub use mem_fs::MemFs;
pub use task_fs::{TaskFs, TaskMeta, TaskSpec, TaskStatus};

#[cfg(not(target_arch = "wasm32"))]
pub use workspace_fs::WorkspaceFs;

// Internal re-export for sibling modules
pub(crate) use mem_fs::now as mem_fs_now;
