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

        let issue = octocrab::models::issues::IssueBuilder::new()
            .id(octocrab::models::IssueId(0))
            .number(issue.number.try_into()?)
            .title(issue.title)
            .body(issue.body.unwrap_or_default())
            .state(state)
            .html_url(html_url.clone())
            .url(html_url.clone())
            .repository_url(html_url.clone())
            .labels_url(html_url.clone())
            .comments_url(html_url.clone())
            .events_url(html_url.clone())
            .node_id(String::new())
            .author_association(String::new())
            .created_at(Utc::now())
            .updated_at(Utc::now())
            .build()?;

        Ok(issue)
    }
}

// Convert our GitHubUser to octocrab's Author
impl TryFrom<GitHubUser> for octocrab::models::Author {
    type Error = anyhow::Error;

    fn try_from(user: GitHubUser) -> Result<Self, Self::Error> {
        let author = octocrab::models::AuthorBuilder::new()
            .login(user.login)
            .id(octocrab::models::UserId(user.id.try_into()?))
            .node_id(String::new())
            .avatar_url(Url::parse("https://github.com").unwrap())
            .gravatar_id(String::new())
            .url(Url::parse("https://github.com").unwrap())
            .html_url(Url::parse("https://github.com").unwrap())
            .followers_url(Url::parse("https://github.com").unwrap())
            .following_url(Url::parse("https://github.com").unwrap())
            .gists_url(Url::parse("https://github.com").unwrap())
            .starred_url(Url::parse("https://github.com").unwrap())
            .subscriptions_url(Url::parse("https://github.com").unwrap())
            .organizations_url(Url::parse("https://github.com").unwrap())
            .repos_url(Url::parse("https://github.com").unwrap())
            .events_url(Url::parse("https://github.com").unwrap())
            .received_events_url(Url::parse("https://github.com").unwrap())
            .type_field("User")
            .site_admin(false)
            .build()?;

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

        let comment = octocrab::models::issues::CommentBuilder::new()
            .id(octocrab::models::CommentId(comment.id.try_into()?))
            .node_id(String::new())
            .url(Url::parse("https://github.com").unwrap())
            .html_url(Url::parse("https://github.com").unwrap())
            .body(comment.body)
            .user(comment.user.try_into()?)
            .created_at(created_at)
            .updated_at(updated_at)
            .build()?;

        Ok(comment)
    }
}
