//! GitHub API models and types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A connected GitHub repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedRepo {
    pub id: i64,
    pub owner: String,
    pub repo: String,
    pub full_name: String,
    pub default_branch: String,
    pub languages: Vec<String>,
    pub connected_at: DateTime<Utc>,
}

/// GitHub repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: RepositoryOwner,
    pub description: Option<String>,
    pub default_branch: String,
    pub clone_url: String,
    pub ssh_url: String,
    pub private: bool,
    pub language: Option<String>,
}

/// Repository owner information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryOwner {
    pub login: String,
    pub id: u64,
    #[serde(rename = "type")]
    pub owner_type: String,
}

/// Detected language in a repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Language {
    pub name: String,
    pub bytes: i64,
    pub percentage: f64,
}

/// GitHub issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: IssueState,
    pub labels: Vec<Label>,
    pub assignees: Vec<User>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Issue state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    Open,
    Closed,
}

/// GitHub label
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

/// GitHub user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub login: String,
    pub id: u64,
}

/// Pull request creation request
#[derive(Debug, Clone, Serialize)]
pub struct CreatePullRequest {
    pub title: String,
    pub body: String,
    pub head: String,
    pub base: String,
    pub draft: bool,
}

/// GitHub pull request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: PullRequestState,
    pub head: PullRequestRef,
    pub base: PullRequestRef,
    pub html_url: String,
    pub mergeable: Option<bool>,
    pub merged: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Pull request state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PullRequestState {
    Open,
    Closed,
}

/// Pull request branch reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
}

/// PR status including CI checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRStatus {
    pub pr: PullRequest,
    pub check_status: CheckStatus,
    pub checks: Vec<CheckRun>,
}

/// Combined check status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pending,
    Success,
    Failure,
    Error,
}

/// Individual check run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

/// OAuth token response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}
