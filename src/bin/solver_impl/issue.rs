use anyhow::{Context as _, Result};
use octocrab::models::issues::{Comment, Issue};
use openagents::solver::{Cli, GitHubContext};
use tracing::info;

pub async fn handle_issue(cli: &Cli, github_token: &str) -> Result<(Issue, Vec<Comment>)> {
    // Initialize GitHub context
    let github = GitHubContext::new(&cli.repo, github_token.to_string())
        .context("Failed to initialize GitHub context")?;

    // Fetch issue details and comments
    info!("\nFetching issue #{} from {}", cli.issue, cli.repo);
    let issue = github
        .get_issue(cli.issue)
        .await
        .context("Failed to fetch issue details")?;
    println!("Solving issue #{}: {}", issue.number, issue.title);

    // Fetch and display comments
    let comments = github
        .get_issue_comments(cli.issue)
        .await
        .context("Failed to fetch issue comments")?;

    if !comments.is_empty() {
        println!("Appending {} comments", comments.len());
    }

    Ok((
        octocrab::models::issues::Issue::try_from(issue)?,
        comments
            .into_iter()
            .map(octocrab::models::issues::Comment::try_from)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}
