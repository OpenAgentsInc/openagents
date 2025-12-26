//! GitHub API integration for Autopilot
//!
//! This module provides GitHub OAuth authentication and API operations
//! for connecting external repositories to Autopilot.

pub mod client;
pub mod models;
pub mod oauth;
pub mod storage;

pub use client::{issue_branch_name, GitHubClient};
pub use models::*;
pub use oauth::GitHubOAuth;
pub use storage::*;
