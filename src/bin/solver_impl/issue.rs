use anyhow::Result;
use openagents::server::services::github_issue::GitHubService;
use octocrab::models::issues::{Issue, Comment as IssueComment};

pub async fn handle_issue(
    github: &GitHubService,
    owner: &str,
    name: &str,
    issue_num: i32,
) -> Result<(Issue, Vec<IssueComment>)> {
    let issue = github.get_issue(owner, name, issue_num).await?;
    let comments = github.get_issue_comments(owner, name, issue_num).await?;
    Ok((issue, comments))
}