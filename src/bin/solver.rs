use anyhow::{Result, Context as _};
use clap::Parser;
use openagents::solver::{Cli, Config, GitHubContext, PlanningContext, SolutionContext};
use termcolor::Color;
use tracing::{debug, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("openagents=debug".parse()?)
            .add_directive("solver=debug".parse()?))
        .init();

    let cli = Cli::parse();
    let config = Config::load().context("Failed to load configuration")?;

    // Clone tokens before moving
    let github_token = config.github_token.clone();
    let openrouter_api_key = config.openrouter_api_key.clone();

    // Initialize GitHub context
    let github = GitHubContext::new(&cli.repo, github_token.clone())
        .context("Failed to initialize GitHub context")?;

    // Fetch issue details and comments
    info!("Fetching issue #{} from {}", cli.issue, cli.repo);
    let issue = github.get_issue(cli.issue).await
        .context("Failed to fetch issue details")?;
    println!("\nIssue #{}: {}", issue.number, issue.title);
    if let Some(body) = &issue.body {
        println!("Description:\n{}\n", body);
    }

    // Fetch and display comments
    let comments = github.get_issue_comments(cli.issue).await
        .context("Failed to fetch issue comments")?;
    if !comments.is_empty() {
        println!("\nComments ({}):", comments.len());
        for comment in &comments {
            println!("\nFrom @{} at {}:", comment.user.login, comment.created_at);
            println!("{}\n", comment.body);
        }
    }

    // Initialize solution context
    info!("Initializing solution context");
    let mut solution = SolutionContext::new(cli.issue, openrouter_api_key, Some(github_token.clone()))
        .context("Failed to initialize solution context")?;

    // Clone repository and generate map
    let repo_url = format!("https://github.com/{}", cli.repo);
    info!("Cloning repository: {}", repo_url);
    solution.clone_repository(&repo_url)
        .context("Failed to clone repository")?;
    debug!("Repository cloned to: {:?}", solution.temp_dir);

    let map = solution.generate_repo_map();
    info!("Generated repository map ({} chars)", map.len());
    debug!("Repository map:\n{}", map);

    // Create branch if in live mode
    let branch_name = format!("issue-{}", cli.issue);
    if cli.live {
        info!("Creating branch: {}", branch_name);
        github.create_branch(&branch_name, "main").await
            .context("Failed to create branch")?;
    }

    // Generate implementation plan
    let planning = PlanningContext::new()
        .context("Failed to initialize planning context")?;

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

    info!("Generating implementation plan");
    debug!("=== CONTEXT SENT TO LLM ===");
    debug!("Issue Title: {}", issue.title);
    debug!("Issue Number: #{}", issue.number);
    debug!("\nFull Context (including comments):");
    debug!("{}", issue.body.as_deref().unwrap_or("No description provided"));
    debug!("{}", comments_context);
    debug!("=== END CONTEXT ===\n");

    let prompt_length = map.len() + issue.title.len() + 
        issue.body.as_deref().unwrap_or("").len() + 
        comments_context.len() + 500; // 500 for template text
    info!("Sending prompt to OpenRouter ({} chars)...", prompt_length);
    info!("Waiting for OpenRouter response...\n");

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
        .await
        .context("Failed to generate implementation plan")?;

    info!("Implementation plan generated");
    debug!("Plan:\n{}", implementation_plan);

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
        github.post_comment(cli.issue, &comment).await
            .context("Failed to post implementation plan comment")?;
    }

    // Generate solution
    openagents::solver::display::print_colored("\nGenerating solution...\n", Color::Blue)?;

    // 1. Generate list of files to modify
    openagents::solver::display::print_colored("\nIdentifying files to modify...\n", Color::Blue)?;
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
        openagents::solver::display::print_colored(
            &format!("\nProcessing {}...\n", file_path),
            Color::Blue
        )?;
        
        // Generate changes
        openagents::solver::display::print_colored("Generating changes...\n", Color::Green)?;
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
        openagents::solver::display::print_colored("Applying changes...\n", Color::Green)?;
        info!("Applying changes to {}", file_path);
        solution.apply_changes(&changes)
            .with_context(|| format!("Failed to apply changes to {}", file_path))?;
    }

    // Create pull request if in live mode
    if cli.live && !solution.modified_files.is_empty() {
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