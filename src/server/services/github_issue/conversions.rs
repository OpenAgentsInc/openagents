use super::{GitHubIssue, GitHubComment, GitHubUser};
use chrono::{DateTime, Utc};
use url::Url;

// Convert our GitHubIssue to octocrab's Issue
impl From<GitHubIssue> for octocrab::models::issues::Issue {
    fn from(issue: GitHubIssue) -> Self {
        let html_url = Url::parse(&issue.html_url).unwrap_or_else(|_| {
            Url::parse("https://github.com").unwrap()
        });

        let state = match issue.state.as_str() {
            "open" => octocrab::models::IssueState::Open,
            "closed" => octocrab::models::IssueState::Closed,
            _ => octocrab::models::IssueState::Open,
        };

        Self {
            id: octocrab::models::IssueId(0), // Not used
            number: issue.number as u64,
            title: issue.title,
            body: issue.body,
            state,
            html_url,
            user: octocrab::models::Author::default(),
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
        }
    }
}

// Convert our GitHubUser to octocrab's Author
impl From<GitHubUser> for octocrab::models::Author {
    fn from(user: GitHubUser) -> Self {
        Self {
            login: user.login,
            id: user.id,
            node_id: String::new(),
            avatar_url: Url::parse("https://github.com").unwrap(),
            gravatar_id: None,
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
        }
    }
}

// Convert our GitHubComment to octocrab's Comment
impl From<GitHubComment> for octocrab::models::issues::Comment {
    fn from(comment: GitHubComment) -> Self {
        let created_at = DateTime::parse_from_rfc3339(&comment.created_at)
            .unwrap_or_default()
            .with_timezone(&Utc);

        let updated_at = DateTime::parse_from_rfc3339(&comment.updated_at)
            .unwrap_or_default()
            .with_timezone(&Utc);

        Self {
            id: octocrab::models::CommentId(comment.id),
            node_id: String::new(),
            url: Url::parse("https://github.com").unwrap(),
            html_url: Url::parse("https://github.com").unwrap(),
            body: Some(comment.body),
            user: comment.user.into(),
            created_at,
            updated_at: Some(updated_at),
            issue_url: None,
            body_text: None,
            body_html: None,
        }
    }
}