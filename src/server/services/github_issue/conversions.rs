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
        let minimal_user = octocrab::models::Author {
            login: "unknown".to_string(),
            id: octocrab::models::UserId(0),
            node_id: String::new(),
            avatar_url: Url::parse("https://github.com").unwrap(),
            gravatar_id: String::new(),
            url: Url::parse("https://github.com").unwrap(),
            html_url: Url::parse("https://github.com").unwrap(),
            followers_url: Url::parse("https://github.com").unwrap(),
            following_url: Url::parse("https://github.com").unwrap(),
            gists_url: Url::parse("https://github.com").unwrap(),
            starred_url: Url::parse("https://github.com").unwrap(),
            subscriptions_url: Url::parse("https://github.com").unwrap(),
            organizations_url: Url::parse("https://github.com").unwrap(),
            repos_url: Url::parse("https://github.com").unwrap(),
            events_url: Url::parse("https://github.com").unwrap(),
            received_events_url: Url::parse("https://github.com").unwrap(),
            r#type: String::from("User"),
            site_admin: false,
            email: None,
            patch_url: None,
        };

        // Create issue with all required fields
        let issue = octocrab::models::issues::Issue {
            id: octocrab::models::IssueId(0),
            number: issue.number.try_into()?,
            title: issue.title,
            body: Some(issue.body.unwrap_or_default()),
            body_text: None,
            body_html: None,
            state,
            state_reason: None,
            html_url,
            user: minimal_user,
            labels: vec![],
            assignee: None,
            assignees: vec![],
            milestone: None,
            locked: false,
            active_lock_reason: None,
            comments: 0,
            pull_request: None,
            closed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            node_id: String::new(),
            url: html_url.clone(),
            repository_url: html_url.clone(),
            labels_url: html_url.clone(),
            comments_url: html_url.clone(),
            events_url: html_url.clone(),
            author_association: String::new(),
        };

        Ok(issue)
    }
}

// Convert our GitHubUser to octocrab's Author
impl TryFrom<GitHubUser> for octocrab::models::Author {
    type Error = anyhow::Error;

    fn try_from(user: GitHubUser) -> Result<Self, Self::Error> {
        let author = octocrab::models::Author {
            login: user.login,
            id: octocrab::models::UserId(user.id.try_into()?),
            node_id: String::new(),
            avatar_url: Url::parse("https://github.com").unwrap(),
            gravatar_id: String::new(),
            url: Url::parse("https://github.com").unwrap(),
            html_url: Url::parse("https://github.com").unwrap(),
            followers_url: Url::parse("https://github.com").unwrap(),
            following_url: Url::parse("https://github.com").unwrap(),
            gists_url: Url::parse("https://github.com").unwrap(),
            starred_url: Url::parse("https://github.com").unwrap(),
            subscriptions_url: Url::parse("https://github.com").unwrap(),
            organizations_url: Url::parse("https://github.com").unwrap(),
            repos_url: Url::parse("https://github.com").unwrap(),
            events_url: Url::parse("https://github.com").unwrap(),
            received_events_url: Url::parse("https://github.com").unwrap(),
            r#type: String::from("User"),
            site_admin: false,
            email: None,
            patch_url: None,
        };

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

        let comment = octocrab::models::issues::Comment {
            id: octocrab::models::CommentId(comment.id.try_into()?),
            node_id: String::new(),
            url: Url::parse("https://github.com").unwrap(),
            html_url: Url::parse("https://github.com").unwrap(),
            body: Some(comment.body),
            user: comment.user.try_into()?,
            created_at,
            updated_at: Some(updated_at),
            issue_url: None,
            body_text: None,
            body_html: None,
        };

        Ok(comment)
    }
}
