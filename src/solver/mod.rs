pub mod changes;
pub mod cli;
pub mod config;
pub mod context;
pub mod display;
pub mod file_list;
pub mod github;
pub mod json;
pub mod planning;
pub mod solution;
pub mod state;
pub mod streaming;
pub mod types;

// Re-export specific types instead of globs to avoid conflicts
pub use changes::generation::*;
pub use cli::*;
pub use config::*;
pub use context::*;
pub use display::*;
pub use file_list::*;
pub use github::*;
pub use json::*;
pub use planning::*;
pub use state::{SolverFile, SolverState, SolverStatus};
pub use streaming::*;
pub use types::{Change, ChangeError, ChangeResult, FileState};

// Re-export octocrab types that we use
pub use octocrab::models::issues::{Comment, Issue};