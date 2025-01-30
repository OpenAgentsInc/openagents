use super::{GitHubIssue, GitHubComment, GitHubUser};
use octocrab::models::issues::{Issue, Comment};
use octocrab::models::User;
use chrono::{DateTime, Utc};

impl From<GitHubIssue> for Issue {
    fn from(issue: GitHubIssue) -> Self {
        Issue {
            id: 0,  // Not used
            number: issue.number,
            title: issue.title,
            body: issue.body,
            state: issue.state,
            html_url: issue.html_url,
            user: User::default(),  // Not used
            labels: vec![],  // Not used
            assignee: None,  // Not used
            assignees: vec![],  // Not used
            milestone: None,  // Not used
            locked: false,  // Not used
            active_lock_reason: None,  // Not used
            comments: 0,  // Not used
            pull_request: None,  // Not used
            closed_at: None,  // Not used
            created_at: Utc::now(),  // Not used
            updated_at: Utc::now(),  // Not used
            closed_by: None,  // Not used
            author_association: String::new(),  // Not used
            draft: None,  // Not used
        }
    }
}

impl From<GitHubUser> for User {
    fn from(user: GitHubUser) -> Self {
        User {
            login: user.login,
            id: user.id,
            node_id: String::new(),  // Not used
            avatar_url: String::new(),  // Not used
            gravatar_id: None,  // Not used
            url: String::new(),  // Not used
            html_url: String::new(),  // Not used
            followers_url: String::new(),  // Not used
            following_url: String::new(),  // Not used
            gists_url: String::new(),  // Not used
            starred_url: String::new(),  // Not used
            subscriptions_url: String::new(),  // Not used
            organizations_url: String::new(),  // Not used
            repos_url: String::new(),  // Not used
            events_url: String::new(),  // Not used
            received_events_url: String::new(),  // Not used
            r#type: String::new(),  // Not used
            site_admin: false,  // Not used
            name: None,  // Not used
            email: None,  // Not used
            blog: None,  // Not used
            company: None,  // Not used
            location: None,  // Not used
            hireable: None,  // Not used
            bio: None,  // Not used
            twitter_username: None,  // Not used
            public_repos: None,  // Not used
            public_gists: None,  // Not used
            followers: None,  // Not used
            following: None,  // Not used
            created_at: None,  // Not used
            updated_at: None,  // Not used
            private_gists: None,  // Not used
            total_private_repos: None,  // Not used
            owned_private_repos: None,  // Not used
            disk_usage: None,  // Not used
            collaborators: None,  // Not used
            two_factor_authentication: None,  // Not used
        }
    }
}

impl From<GitHubComment> for Comment {
    fn from(comment: GitHubComment) -> Self {
        Comment {
            id: comment.id,
            node_id: String::new(),  // Not used
            url: String::new(),  // Not used
            html_url: String::new(),  // Not used
            body: Some(comment.body),
            user: comment.user.into(),
            created_at: DateTime::parse_from_rfc3339(&comment.created_at)
                .unwrap_or_default()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&comment.updated_at)
                .unwrap_or_default()
                .with_timezone(&Utc),
            issue_url: None,  // Not used
            author_association: String::new(),  // Not used
            performed_via_github_app: None,  // Not used
        }
    }
}