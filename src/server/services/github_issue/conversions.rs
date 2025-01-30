use super::{GitHubIssue, GitHubComment, GitHubUser};
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
        let minimal_user = octocrab::models::Author::new(
            user.login,
            octocrab::models::UserId(0),
            Url::parse("https://github.com").unwrap()
        );

        // Use octocrab's public constructor
        let issue = octocrab::models::issues::Issue::new(
            octocrab::models::IssueId(0),
            issue.number.try_into()?,
            issue.title,
            Some(issue.body.unwrap_or_default()),
            state,
            html_url.clone(),
            minimal_user,
        );

        Ok(issue)
    }
}

// Convert our GitHubUser to octocrab's Author
impl TryFrom<GitHubUser> for octocrab::models::Author {
    type Error = anyhow::Error;

    fn try_from(user: GitHubUser) -> Result<Self, Self::Error> {
        let author = octocrab::models::Author::new(
            user.login,
            octocrab::models::UserId(user.id.try_into()?),
            Url::parse("https://github.com").unwrap()
        );

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

        let comment = octocrab::models::issues::Comment::new(
            octocrab::models::CommentId(comment.id.try_into()?),
            Some(comment.body),
            comment.user.try_into()?,
            created_at,
            Some(updated_at)
        );

        Ok(comment)
    }
}
