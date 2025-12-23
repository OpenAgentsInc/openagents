//! Git operations module

pub mod branch;
pub mod clone;
pub mod diff;
pub mod patch;
pub mod rebase;
pub mod remote;

pub use branch::{checkout_branch, create_branch, current_branch, delete_branch, list_branches};
pub use clone::{clone_repository, get_repository_path, get_workspace_path, is_repository_cloned};
pub use diff::{diff_commits, diff_stats, diff_working_directory, generate_patch, get_status, DiffStats, FileChange, FileStatus};
pub use patch::{apply_patch, can_apply_patch};
pub use rebase::{abort_rebase, has_rebase_conflicts, rebase_branch, rebase_commit};
pub use remote::{add_remote, fetch_remote, get_remote_url, list_remotes, push_branch};
