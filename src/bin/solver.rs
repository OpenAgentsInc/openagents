use anyhow::Result;
use openagents::solver::{Cli, Config, GitHubContext, PlanningContext, SolutionContext};
use termcolor::Color;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    // Initialize GitHub context
    let github = GitHubContext::new(&cli.repo, config.github_token)?;

    // Fetch issue details
    let issue = github.get_issue(cli.issue).await?;
    println!("\nIssue #{}: {}", issue.number, issue.title);
    if let Some(body) = &issue.body {
        println!("Description:\n{}\n", body);
    }

    // Initialize solution context
    let solution = SolutionContext::new(
        cli.issue,
        config.openrouter_api_key,
        Some(config.github_token.clone()),
    )?;

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
    let implementation_plan = planning
        .generate_plan(
            cli.issue,
            &issue.title,
            issue.body.as_deref().unwrap_or("No description provided"),
            &map,
        )
        .await?;

    // Post implementation plan as comment if in live mode
    if cli.live {
        let comment = format!(
            "# Implementation Plan\n\n\
            Based on the analysis of the issue and codebase, here's the proposed implementation plan:\n\n\
            {}\n\n\
            I'll now proceed with implementing this solution.",
            implementation_plan
        );
        github.post_comment(cli.issue, &comment).await?;
    }

    // TODO: Generate solution
    openagents::solver::display::print_colored("\nGenerating solution...\n", Color::Blue)?;
    // Implementation will go here

    // Create pull request if in live mode
    if cli.live && !solution.modified_files.is_empty() {
        let title = format!("Implement solution for #{}", cli.issue);
        let description = format!(
            "Automated solution for issue #{}\n\n\
            Implementation Plan:\n{}\n\n\
            Implemented by the OpenAgents solver.",
            cli.issue, implementation_plan
        );
        github
            .create_pull_request(&branch_name, "main", &title, &description)
            .await?;
    }

    // Clean up
    solution.cleanup();

    Ok(())
}