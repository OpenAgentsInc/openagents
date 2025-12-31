//! Shared types for GitAfter WGPUI.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitafterTab {
    Repos,
    Issues,
    PullRequests,
}

impl fmt::Display for GitafterTab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitafterTab::Repos => write!(f, "Repos"),
            GitafterTab::Issues => write!(f, "Issues"),
            GitafterTab::PullRequests => write!(f, "PRs"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepoSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub pubkey: String,
    pub address: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct IssueSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub author: String,
    pub created_at: String,
    pub bounty_sats: Option<u64>,
    pub repo_address: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct PrSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub author: String,
    pub created_at: String,
    pub repo_address: Option<String>,
    pub repo_identifier: Option<String>,
    pub commit_id: Option<String>,
    pub clone_url: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum GitafterCommand {
    LoadRepositories {
        limit: usize,
    },
    LoadIssues {
        repo_address: Option<String>,
        limit: usize,
    },
    LoadPullRequests {
        repo_address: Option<String>,
        limit: usize,
    },
    LoadPullRequestDiff {
        pr_id: String,
        repo_identifier: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum GitafterUpdate {
    RepositoriesLoaded {
        repos: Vec<RepoSummary>,
    },
    IssuesLoaded {
        issues: Vec<IssueSummary>,
    },
    PullRequestsLoaded {
        pull_requests: Vec<PrSummary>,
    },
    PullRequestDiffLoaded {
        pr_id: String,
        diff: Option<String>,
    },
    ConnectionStatus {
        status: ConnectionStatus,
        message: Option<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Error,
}

impl ConnectionStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ConnectionStatus::Connecting => "Connecting",
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::Error => "Error",
        }
    }
}
