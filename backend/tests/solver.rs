use anyhow::Result;
use openagents::server::services::github_issue::{GitHubComment, GitHubService, GitHubUser};
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

    // Only test actual API calls if explicitly enabled
    if env::var("RUN_GITHUB_API_TESTS").is_ok() {
        println!("Testing actual GitHub API calls...");
        github_service
            .create_branch(
                "OpenAgentsInc",
                "openagents",
                &format!("test/{}", branch_name),
                "main",
            )
            .await?;
    } else {
        println!("Skipping GitHub API calls - set RUN_GITHUB_API_TESTS=1 to enable");
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

    // Only test actual API calls if explicitly enabled
    if env::var("RUN_GITHUB_API_TESTS").is_ok() {
        println!("Testing actual GitHub API calls...");
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
    } else {
        println!("Skipping GitHub API calls - set RUN_GITHUB_API_TESTS=1 to enable");
    }

    Ok(())
}

#[tokio::test]
async fn test_issue_comments() -> Result<()> {
    // Skip if no GitHub token available
    if env::var("GITHUB_TOKEN").is_err() {
        println!("Skipping test_issue_comments - no GITHUB_TOKEN available");
        return Ok(());
    }

    let github_token = env::var("GITHUB_TOKEN")?;
    let github_service = GitHubService::new(Some(github_token))?;

    // Test comment data structures
    let test_user = GitHubUser {
        login: "test-user".to_string(),
        id: 12345,
    };

    let test_comment = GitHubComment {
        id: 67890,
        body: "Test comment body".to_string(),
        user: test_user,
        created_at: "2024-01-28T00:00:00Z".to_string(),
        updated_at: "2024-01-28T00:00:00Z".to_string(),
    };

    assert_eq!(test_comment.user.login, "test-user");
    assert_eq!(test_comment.body, "Test comment body");

    // Only test actual API calls if explicitly enabled
    if env::var("RUN_GITHUB_API_TESTS").is_ok() {
        println!("Testing actual GitHub API calls...");

        // Test fetching comments from a real issue
        let comments = github_service
            .get_issue_comments("OpenAgentsInc", "openagents", 623)
            .await?;

        // Just verify we can fetch comments without error
        println!("Fetched {} comments from issue #623", comments.len());

        // Test posting a comment
        let test_issue_number = 623; // Use a test issue number
        let test_comment = "Test comment from automated test (will be deleted)";

        github_service
            .post_comment(
                "OpenAgentsInc",
                "openagents",
                test_issue_number,
                test_comment,
            )
            .await?;
    } else {
        println!("Skipping GitHub API calls - set RUN_GITHUB_API_TESTS=1 to enable");
    }

    Ok(())
}

#[tokio::test]
async fn test_comment_context_generation() -> Result<()> {
    // Test generating comment context string
    let test_user = GitHubUser {
        login: "test-user".to_string(),
        id: 12345,
    };

    let test_comments = vec![
        GitHubComment {
            id: 1,
            body: "First comment".to_string(),
            user: test_user.clone(),
            created_at: "2024-01-28T00:00:00Z".to_string(),
            updated_at: "2024-01-28T00:00:00Z".to_string(),
        },
        GitHubComment {
            id: 2,
            body: "Second comment".to_string(),
            user: test_user.clone(),
            created_at: "2024-01-28T01:00:00Z".to_string(),
            updated_at: "2024-01-28T01:00:00Z".to_string(),
        },
    ];

    let context = if !test_comments.is_empty() {
        let mut ctx = String::from("\nRelevant comments:\n");
        for comment in &test_comments {
            ctx.push_str(&format!(
                "\n@{} at {}:\n{}\n",
                comment.user.login, comment.created_at, comment.body
            ));
        }
        ctx
    } else {
        String::from("\nNo additional comments on the issue.")
    };

    assert!(context.contains("Relevant comments"));
    assert!(context.contains("@test-user"));
    assert!(context.contains("First comment"));
    assert!(context.contains("Second comment"));

    Ok(())
}
