use anyhow::Result;
use openagents::server::services::github_issue::GitHubService;
use std::env;

#[tokio::test]
async fn test_branch_creation() -> Result<()> {
    // Skip if no GitHub token available
    if env::var("GITHUB_TOKEN").is_err() {
        println!("Skipping test_branch_creation - no GITHUB_TOKEN available");
        return Ok(());
    }

    let github_token = env::var("GITHUB_TOKEN")?;
    let github_service = GitHubService::new(Some(github_token))?;

    // Test branch name generation
    let issue_number = 123;
    let branch_name = format!("solver/issue-{}", issue_number);
    assert_eq!(branch_name, "solver/issue-123");

    // Test branch creation (only in CI with write permissions)
    if env::var("CI").is_ok() {
        github_service
            .create_branch(
                "OpenAgentsInc",
                "openagents",
                &format!("test/{}", branch_name),
                "main",
            )
            .await?;
    }

    Ok(())
}

#[tokio::test]
async fn test_pr_creation() -> Result<()> {
    // Skip if no GitHub token available
    if env::var("GITHUB_TOKEN").is_err() {
        println!("Skipping test_pr_creation - no GITHUB_TOKEN available");
        return Ok(());
    }

    let github_token = env::var("GITHUB_TOKEN")?;
    let github_service = GitHubService::new(Some(github_token))?;

    // Test PR title/description generation
    let issue_number = 123;
    let title = format!("Implement solution for #{}", issue_number);
    let description = format!(
        "Automated solution for issue #{}\n\nImplemented by the OpenAgents solver.",
        issue_number
    );

    assert_eq!(title, "Implement solution for #123");
    assert!(description.contains("Automated solution for issue #123"));
    assert!(description.contains("Implemented by the OpenAgents solver"));

    // Test PR creation (only in CI with write permissions)
    if env::var("CI").is_ok() {
        let test_branch = format!("test/solver/issue-{}", issue_number);

        // Create test branch first
        github_service
            .create_branch("OpenAgentsInc", "openagents", &test_branch, "main")
            .await?;

        // Create PR
        github_service
            .create_pull_request(
                "OpenAgentsInc",
                "openagents",
                &test_branch,
                "main",
                &title,
                &description,
            )
            .await?;
    }

    Ok(())
}
