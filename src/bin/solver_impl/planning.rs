use anyhow::{Context as _, Result};
use openagents::solver::{Cli, PlanningContext, handle_plan_stream, Issue, Comment};
use tracing::{debug, info};

pub async fn handle_planning(
    cli: &Cli,
    issue: &Issue,
    comments: &[Comment],
    repo_map: &str,
) -> Result<String> {
    // Generate implementation plan
    let planning = PlanningContext::new().context("Failed to initialize planning context")?;

    // Include comments in the context for planning
    let comments_context = if !comments.is_empty() {
        let mut context = String::from("\nRelevant comments:\n");
        for comment in comments {
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
    debug!(
        "{}",
        issue.body.as_deref().unwrap_or("No description provided")
    );
    debug!("{}", comments_context);
    debug!("=== END CONTEXT ===\n");

    // Get streaming plan generation
    let stream = planning
        .generate_plan(
            cli.issue,
            &issue.title,
            &format!(
                "{}\n{}",
                issue.body.as_deref().unwrap_or("No description provided"),
                comments_context
            ),
            repo_map,
        )
        .await;

    // Handle the stream and get the final plan
    let implementation_plan = handle_plan_stream(stream).await?;

    info!("Implementation plan generated");
    debug!("Plan:\n{}", implementation_plan);

    Ok(implementation_plan)
}