//! Shared utilities for the Codex agent
//!
//! This module combines several utility crates from the original Codex CLI:
//! - `absolute_path` - Path normalization and absolute path handling
//! - `string` - String utilities
//! - `git` - Git operations
//! - `async_utils` - Async utilities
//! - `image` - Image processing (stubbed)

pub mod absolute_path;
pub mod async_utils;
pub mod git;
pub mod image;
pub mod string;

// Re-exports for convenience
pub use absolute_path::{AbsolutePathBuf, AbsolutePathBufGuard};
pub use git::GhostCommit;
