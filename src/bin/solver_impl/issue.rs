use anyhow::Result;
use openagents::server::services::github_issue::{GitHubService, GitHubIssue, GitHubComment};
use octocrab::models::issues::{Issue, Comment};

pub async fn handle_issue(
    github: &GitHubService,
    owner: &str,
    name: &str,
    issue_num: i32,
) -> Result<(Issue, Vec<Comment>)> {
    let raw_issue: GitHubIssue = github.get_issue(owner, name, issue_num).await?;
    let raw_comments: Vec<GitHubComment> = github.get_issue_comments(owner, name, issue_num).await?;
    
    let issue = Issue::try_from(raw_issue)?;
    let comments = raw_comments.into_iter()
        .map(Comment::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    Ok((issue, comments))
}