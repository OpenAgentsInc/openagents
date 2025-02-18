use super::{GitHubComment, GitHubIssue, GitHubUser};
use chrono::{DateTime, Utc};
use url::Url;

// Convert our GitHubIssue to octocrab's Issue
impl TryFrom<GitHubIssue> for octocrab::models::issues::Issue {
    type Error = anyhow::Error;

    fn try_from(issue: GitHubIssue) -> Result<Self, Self::Error> {
        let html_url = Url::parse(&issue.html_url)
            .unwrap_or_else(|_| Url::parse("https://github.com").unwrap());

        let state = match issue.state.as_str() {
            "open" => octocrab::models::IssueState::Open,
            "closed" => octocrab::models::IssueState::Closed,
            _ => octocrab::models::IssueState::Open,
        };

        // Create a minimal user since we don't have full user data
        let minimal_user = octocrab::models::Author::try_from(GitHubUser {
            login: "unknown".to_string(),
            id: 0,
        })?;

        // Use octocrab's serde to construct the issue
        let issue: octocrab::models::issues::Issue = serde_json::from_value(serde_json::json!({
            "id": 0,
            "number": issue.number,
            "title": issue.title,
            "body": issue.body.unwrap_or_default(),
            "body_text": null,
            "body_html": null,
            "state": state,
            "state_reason": null,
            "html_url": html_url,
            "user": minimal_user,
            "labels": [],
            "assignee": null,
            "assignees": [],
            "milestone": null,
            "locked": false,
            "active_lock_reason": null,
            "comments": 0,
            "pull_request": null,
            "closed_at": null,
            "created_at": Utc::now(),
            "updated_at": Utc::now(),
            "node_id": "",
            "url": html_url,
            "repository_url": html_url,
            "labels_url": html_url,
            "comments_url": html_url,
            "events_url": html_url,
            "author_association": ""
        }))?;

        Ok(issue)
    }
}

// Convert our GitHubUser to octocrab's Author
impl TryFrom<GitHubUser> for octocrab::models::Author {
    type Error = anyhow::Error;

    fn try_from(user: GitHubUser) -> Result<Self, Self::Error> {
        let author: octocrab::models::Author = serde_json::from_value(serde_json::json!({
            "login": user.login,
            "id": user.id,
            "node_id": "",
            "avatar_url": "https://github.com",
            "gravatar_id": "",
            "url": "https://github.com",
            "html_url": "https://github.com",
            "followers_url": "https://github.com",
            "following_url": "https://github.com",
            "gists_url": "https://github.com",
            "starred_url": "https://github.com",
            "subscriptions_url": "https://github.com",
            "organizations_url": "https://github.com",
            "repos_url": "https://github.com",
            "events_url": "https://github.com",
            "received_events_url": "https://github.com",
            "type": "User",
            "site_admin": false,
            "email": null,
            "patch_url": null
        }))?;

        Ok(author)
    }
}

// Convert our GitHubComment to octocrab's Comment
impl TryFrom<GitHubComment> for octocrab::models::issues::Comment {
    type Error = anyhow::Error;

    fn try_from(comment: GitHubComment) -> Result<Self, Self::Error> {
        let created_at = DateTime::parse_from_rfc3339(&comment.created_at)
            .unwrap_or_default()
            .with_timezone(&Utc);

        let updated_at = DateTime::parse_from_rfc3339(&comment.updated_at)
            .unwrap_or_default()
            .with_timezone(&Utc);

        let comment: octocrab::models::issues::Comment =
            serde_json::from_value(serde_json::json!({
                "id": comment.id,
                "node_id": "MDExOlB1bGxSZXF1ZXN0NTgzMTI5Nzcx",  // Placeholder node_id
                "url": "https://github.com",
                "html_url": "https://github.com",
                "body": comment.body,
                "user": {
                    "login": comment.user.login,
                    "id": comment.user.id,
                    "node_id": "MDQ6VXNlcjE=",  // Placeholder node_id
                    "avatar_url": "https://github.com",
                    "gravatar_id": "",
                    "url": "https://github.com",
                    "html_url": "https://github.com",
                    "followers_url": "https://github.com",
                    "following_url": "https://github.com",
                    "gists_url": "https://github.com",
                    "starred_url": "https://github.com",
                    "subscriptions_url": "https://github.com",
                    "organizations_url": "https://github.com",
                    "repos_url": "https://github.com",
                    "events_url": "https://github.com",
                    "received_events_url": "https://github.com",
                    "type": "User",
                    "site_admin": false,
                    "email": null,
                    "patch_url": null
                },
                "created_at": created_at,
                "updated_at": updated_at,
                "issue_url": "https://github.com",
                "body_text": null,
                "body_html": null
            }))?;

        Ok(comment)
    }
}
