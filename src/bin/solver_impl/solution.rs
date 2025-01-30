use anyhow::{Context as _, Result};
use octocrab::models::issues::{Issue, Comment};
use openagents::solver::{
    Cli, GitHubContext, SolutionContext,
    print_colored,  // Use re-exported function
};
use termcolor::Color;
use tracing::{debug, info, warn};

pub async fn handle_solution(
    cli: &Cli,
    issue: &Issue,
    comments: &[Comment],
    implementation_plan: &str,
    github_token: String,
    openrouter_api_key: String,
) -> Result<()> {
    // Initialize solution context
    info!("Initializing solution context");
    let mut solution = SolutionContext::new(cli.issue, openrouter_api_key, Some(github_token.clone()))
        .context("Failed to initialize solution context")?;

    // Initialize GitHub context for branch/PR operations
    let github = GitHubContext::new(&cli.repo, github_token)
        .context("Failed to initialize GitHub context")?;

    // Clone repository and generate map
    let repo_url = format!("https://github.com/{}", cli.repo);
    info!("Cloning repository: {}", repo_url);
    solution
        .clone_repository(&repo_url)
        .context("Failed to clone repository")?;
    debug!("Repository cloned to: {:?}", solution.temp_dir);

    let map = solution.generate_repo_map();
    info!("Generated repository map ({} chars)", map.len());
    debug!("Repository map:\n{}", map);

    // Create and checkout branch if in live mode
    let branch_name = format!("issue-{}", cli.issue);
    if cli.live {
        info!("Creating branch: {}", branch_name);
        github
            .create_branch(&branch_name, "main")
            .await
            .context("Failed to create branch")?;

        // Verify branch was created
        debug!("Verifying branch creation");
        if !github
            .service
            .check_branch_exists(&github.owner, &github.repo, &branch_name)
            .await?
        {
            return Err(anyhow::anyhow!("Failed to verify branch creation"));
        }
        info!("Branch creation verified");

        // Checkout the branch locally
        info!("Checking out branch locally");
        solution
            .checkout_branch(&branch_name)
            .context("Failed to checkout branch")?;
        debug!("Branch checked out successfully");
    }

    // Post implementation plan as comment if in live mode
    if cli.live {
        info!("Posting implementation plan as comment");
        let comment = format!(
            "# Implementation Plan\n\n\
            Based on the analysis of the issue, comments, and codebase, here's the proposed implementation plan:\n\n\
            {}\n\n\
            I'll now proceed with implementing this solution.",
            implementation_plan
        );
        github
            .post_comment(cli.issue, &comment)
            .await
            .context("Failed to post implementation plan comment")?;
    }

    // Generate solution
    print_colored("\nGenerating solution...\n", Color::Blue)?;

    // Include comments in context
    let comments_context = if !comments.is_empty() {
        let mut context = String::from("\nRelevant comments:\n");
        for comment in comments {
            context.push_str(&format!(
                "\n@{} at {}:\n{}\n",
                comment.user.login, 
                comment.created_at,
                comment.body.as_deref().unwrap_or("No comment body")
            ));
        }
        context
    } else {
        String::from("\nNo additional comments on the issue.")
    };

    // 1. Generate list of files to modify
    print_colored("\nIdentifying files to modify...\n", Color::Blue)?;
    let (files, file_reasoning) = solution
        .generate_file_list(
            &issue.title,
            &format!(
                "{}\n{}",
                issue.body.as_deref().unwrap_or("No description provided"),
                comments_context
            ),
        )
        .await
        .context("Failed to generate file list")?;

    println!("\nFiles to modify:");
    for file in &files {
        println!("- {}", file);
    }
    println!("\nReasoning:\n{}\n", file_reasoning);

    if files.is_empty() {
        warn!("No files identified for modification");
    }

    // 2. For each file, generate and apply changes
    for file_path in files {
        print_colored(
            &format!("\nProcessing {}...\n", file_path),
            Color::Blue,
        )?;

        // Generate changes
        print_colored("Generating changes...\n", Color::Green)?;
        info!("Generating changes for {}", file_path);
        let (changes, change_reasoning) = solution
            .generate_changes(
                &file_path,
                &issue.title,
                &format!(
                    "{}\n{}",
                    issue.body.as_deref().unwrap_or("No description provided"),
                    comments_context
                ),
            )
            .await
            .with_context(|| format!("Failed to generate changes for {}", file_path))?;

        println!("\nChange reasoning:\n{}\n", change_reasoning);
        debug!("Generated {} changes for {}", changes.len(), file_path);

        // Apply changes
        print_colored("Applying changes...\n", Color::Green)?;
        info!("Applying changes to {}", file_path);
        solution
            .apply_changes(&changes)
            .with_context(|| format!("Failed to apply changes to {}", file_path))?;
    }

    // Commit changes if in live mode
    if cli.live && !solution.modified_files.is_empty() {
        info!("Committing changes");
        let commit_message = format!(
            "Implement solution for #{}\n\n{}",
            cli.issue, implementation_plan
        );
        solution
            .commit_changes(&commit_message)
            .context("Failed to commit changes")?;
        debug!("Changes committed successfully");

        // Verify branch has commits
        debug!("Verifying branch has commits");
        if !github
            .service
            .check_branch_has_commits(&github.owner, &github.repo, &branch_name)
            .await?
        {
            return Err(anyhow::anyhow!("Branch has no commits - cannot create PR"));
        }
        info!("Branch commits verified");

        // Create pull request
        info!("Creating pull request");
        let title = format!("Implement solution for #{}", cli.issue);
        let description = format!(
            "Automated solution for issue #{}\n\n\
            Implementation Plan:\n{}\n\n\
            Modified Files:\n{}\n\n\
            Implemented by the OpenAgents solver.",
            cli.issue,
            implementation_plan,
            solution.modified_files.join("\n")
        );

        debug!("Creating PR with title: {}", title);
        debug!("PR description length: {} chars", description.len());
        debug!("Modified files: {:?}", solution.modified_files);

        github
            .create_pull_request(&branch_name, "main", &title, &description)
            .await
            .context("Failed to create pull request")?;
        info!("Pull request created successfully");
    }

    // Clean up
    info!("Cleaning up temporary files");
    solution.cleanup();

    Ok(())
}