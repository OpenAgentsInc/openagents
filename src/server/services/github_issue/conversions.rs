use super::{GitHubIssue, GitHubComment};
use octocrab::models::issues::{Issue, Comment};

impl From<GitHubIssue> for Issue {
    fn from(issue: GitHubIssue) -> Self {
        Issue {
            number: issue.number,
            title: issue.title,
            body: issue.body,
            state: issue.state,
            html_url: issue.html_url,
            ..Default::default()
        }
    }
}

impl From<GitHubComment> for Comment {
    fn from(comment: GitHubComment) -> Self {
        Comment {
            id: comment.id,
            body: Some(comment.body),
            user: comment.user,
            created_at: comment.created_at.parse().unwrap_or_default(),
            updated_at: comment.updated_at.parse().unwrap_or_default(),
            ..Default::default()
        }
    }
}