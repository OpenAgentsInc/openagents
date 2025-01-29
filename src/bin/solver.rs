use anyhow::Result;
use clap::Parser;
use openagents::solver::{Cli, Config, GitHubContext, PlanningContext, SolutionContext};
use std::fs;
use termcolor::Color;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    // Clone tokens before moving
    let github_token = config.github_token.clone();
    let openrouter_api_key = config.openrouter_api_key.clone();

    // Initialize GitHub context
    let github = GitHubContext::new(&cli.repo, github_token.clone())?;

    // Fetch issue details and comments
    let issue = github.get_issue(cli.issue).await?;
    println!("\nIssue #{}: {}", issue.number, issue.title);
    if let Some(body) = &issue.body {
        println!("Description:\n{}\n", body);
    }

    // Fetch and display comments
    let comments = github.get_issue_comments(cli.issue).await?;
    if !comments.is_empty() {
        println!("\nComments ({}):", comments.len());
        for comment in &comments {
            println!("\nFrom @{} at {}:", comment.user.login, comment.created_at);
            println!("{}\n", comment.body);
        }
    }

    // Initialize solution context
    let mut solution = SolutionContext::new(cli.issue, openrouter_api_key, Some(github_token.clone()))?;

    // Clone repository and generate map
    let repo_url = format!("https://github.com/{}", cli.repo);
    solution.clone_repository(&repo_url)?;
    let map = solution.generate_repo_map();
    println!("\nRepository map generated ({} chars)", map.len());

    // Create branch if in live mode
    let branch_name = format!("solver/issue-{}", cli.issue);
    if cli.live {
        github.create_branch(&branch_name, "main").await?;
    }

    // Generate implementation plan
    let planning = PlanningContext::new()?;

    // Include comments in the context for planning
    let comments_context = if !comments.is_empty() {
        let mut context = String::from("\nRelevant comments:\n");
        for comment in &comments {
            context.push_str(&format!(
                "\n@{} at {}:\n{}\n",
                comment.user.login, comment.created_at, comment.body
            ));
        }
        context
    } else {
        String::from("\nNo additional comments on the issue.")
    };

    let implementation_plan = planning
        .generate_plan(
            cli.issue,
            &issue.title,
            &format!(
                "{}\n{}",
                issue.body.as_deref().unwrap_or("No description provided"),
                comments_context
            ),
            &map,
        )
        .await?;

    // Post implementation plan as comment if in live mode
    if cli.live {
        let comment = format!(
            "# Implementation Plan\n\n\
            Based on the analysis of the issue, comments, and codebase, here's the proposed implementation plan:\n\n\
            {}\n\n\
            I'll now proceed with implementing this solution.",
            implementation_plan
        );
        github.post_comment(cli.issue, &comment).await?;
    }

    // Generate solution
    openagents::solver::display::print_colored("\nGenerating solution...\n", Color::Blue)?;

    // 1. Generate list of files to modify
    openagents::solver::display::print_colored("\nIdentifying files to modify...\n", Color::Blue)?;
    let files = solution.generate_file_list(
        &issue.title,
        &format!(
            "{}\n{}",
            issue.body.as_deref().unwrap_or("No description provided"),
            comments_context
        ),
        &map,
    ).await?;

    println!("\nFiles to modify:");
    for file in &files {
        println!("- {}", file);
    }

    // 2. For each file, generate and apply changes
    for file_path in files {
        openagents::solver::display::print_colored(
            &format!("\nProcessing {}...\n", file_path),
            Color::Blue
        )?;
        
        // Read current content
        let file_path_buf = solution.temp_dir.join(&file_path);
        let current_content = fs::read_to_string(&file_path_buf)?;
        
        // Generate changes
        openagents::solver::display::print_colored("Generating changes...\n", Color::Green)?;
        let changes = solution.generate_changes(
            &file_path,
            &current_content,
            &issue.title,
            &format!(
                "{}\n{}",
                issue.body.as_deref().unwrap_or("No description provided"),
                comments_context
            ),
        ).await?;
        
        // Apply changes
        openagents::solver::display::print_colored("Applying changes...\n", Color::Green)?;
        solution.apply_changes(&changes)?;
    }

    // Create pull request if in live mode
    if cli.live && !solution.modified_files.is_empty() {
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
        github
            .create_pull_request(&branch_name, "main", &title, &description)
            .await?;
    }

    // Clean up
    solution.cleanup();

    Ok(())
}