//! Git operations module

pub mod branch;
pub mod clone;
pub mod diff;
pub mod patch;
pub mod rebase;
pub mod remote;

pub use branch::{create_branch, current_branch};
pub use clone::{clone_repository, get_repository_path, get_workspace_path, is_repository_cloned};
pub use diff::{FileChange, FileStatus, diff_commits, generate_patch, get_status};
pub use patch::apply_patch;
pub use rebase::{abort_rebase, has_rebase_conflicts, rebase_commit};
pub use remote::push_branch;
