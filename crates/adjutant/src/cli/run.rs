//! Run command - start the autopilot loop

use crate::cli::blocker::{analyze_blockers, print_blocker_summary};
use crate::cli::directive::work_on_directive;
use crate::{Adjutant, Task};
use clap::Args;
use oanix::{boot, WorkspaceManifest};

/// Run command arguments
#[derive(Args)]
pub struct RunArgs {
    /// Specific issue number to work on
    #[arg(short, long)]
    pub issue: Option<u32>,

    /// Run continuously, claiming and completing issues
    #[arg(short, long)]
    pub loop_mode: bool,

    /// Ad-hoc task description (instead of issue)
    pub task: Option<String>,
}

/// Run the autopilot loop
pub async fn run(args: RunArgs) -> anyhow::Result<()> {
    println!("OANIX v0.1.0 - OpenAgents NIX");
    println!("{}", "=".repeat(55));
    println!();
    println!("Booting...");

    // Boot OANIX
    let manifest = boot().await?;

    // Create Adjutant
    let mut adjutant = Adjutant::new(manifest.clone())?;

    // Determine what to work on
    if let Some(task_desc) = args.task {
        // Ad-hoc task
        let task = Task::new("adhoc", "Ad-hoc Task", task_desc);
        println!();
        println!("Working on ad-hoc task: {}", task.title);
        println!();

        let result = adjutant.execute(&task).await?;
        print_result(&result);
        return Ok(());
    }

    // Working on issues
    let workspace = manifest
        .workspace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No .openagents/ folder found"))?;

    if args.loop_mode {
        // Continuous loop mode
        println!();
        println!("Starting autopilot loop...");
        println!("Press Ctrl+C to stop");
        println!();

        loop {
            // Find next available issue
            let next_issue = workspace
                .issues
                .iter()
                .filter(|i| i.status == "open" && !i.is_blocked)
                .min_by_key(|i| match i.priority.as_str() {
                    "urgent" => 0,
                    "high" => 1,
                    "medium" => 2,
                    "low" => 3,
                    _ => 4,
                });

            match next_issue {
                Some(issue) => {
                    println!("Claiming issue #{}: {}", issue.number, issue.title);

                    // Create task from issue summary
                    // Note: In a full implementation, we'd load the full issue
                    let task = Task::new(
                        format!("#{}", issue.number),
                        &issue.title,
                        format!("Issue #{}: {}", issue.number, issue.title),
                    );

                    let result = adjutant.execute(&task).await?;
                    print_result(&result);

                    if !result.success {
                        println!("Task failed, waiting before retry...");
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    }
                }
                None => {
                    println!("No actionable issues. Waiting...");
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                    // Re-boot to refresh issue list
                    // In production, we'd have a more efficient way to check
                    break; // For now, exit rather than infinite loop
                }
            }
        }
    } else {
        // Single issue mode
        let issue = if let Some(number) = args.issue {
            // Specific issue requested
            Some(
                workspace
                    .issues
                    .iter()
                    .find(|i| i.number == number)
                    .ok_or_else(|| anyhow::anyhow!("Issue #{} not found", number))?,
            )
        } else {
            // Find next available issue
            workspace
                .issues
                .iter()
                .filter(|i| i.status == "open" && !i.is_blocked)
                .min_by_key(|i| match i.priority.as_str() {
                    "urgent" => 0,
                    "high" => 1,
                    "medium" => 2,
                    "low" => 3,
                    _ => 4,
                })
        };

        match issue {
            Some(issue) => {
                println!();
                println!("Working on issue #{}: {}", issue.number, issue.title);
                println!();

                // Create task from issue
                let task = Task::new(
                    format!("#{}", issue.number),
                    &issue.title,
                    format!("Issue #{}: {}", issue.number, issue.title),
                );

                let result = adjutant.execute(&task).await?;
                print_result(&result);
            }
            None => {
                // No actionable issues - use smart fallback
                find_work_when_blocked(workspace, &mut adjutant).await?;
            }
        }
    }

    Ok(())
}

fn print_result(result: &crate::TaskResult) {
    println!();
    println!("{}", "=".repeat(55));

    if result.success {
        println!("Task completed successfully");
    } else {
        println!("Task failed");
    }

    println!();
    println!("Summary: {}", result.summary);

    if !result.modified_files.is_empty() {
        println!();
        println!("Modified files:");
        for file in &result.modified_files {
            println!("  - {}", file);
        }
    }

    if let Some(hash) = &result.commit_hash {
        println!();
        println!("Commit: {}", hash);
    }

    if let Some(error) = &result.error {
        println!();
        println!("Error: {}", error);
    }
}

/// Find work when all issues are blocked.
///
/// This function analyzes WHY issues are blocked and either:
/// 1. Suggests unblocking work (e.g., implementing a missing crate)
/// 2. Falls back to working on the active directive
async fn find_work_when_blocked(
    workspace: &WorkspaceManifest,
    adjutant: &mut Adjutant,
) -> anyhow::Result<()> {
    println!();
    println!("No actionable issues. Analyzing blockers...");
    println!();

    // Collect blocked issues
    let blocked: Vec<_> = workspace
        .issues
        .iter()
        .filter(|i| i.is_blocked)
        .collect();

    if blocked.is_empty() {
        println!("No blocked issues found. All issues may be completed or in progress.");
        return Ok(());
    }

    // Print blocked issue summary
    print_blocker_summary(&blocked);
    println!();

    // Analyze blockers
    let analysis = analyze_blockers(&blocked);

    println!("Blocker analysis:");
    if analysis.needs_code > 0 {
        println!("  {} need code implemented first", analysis.needs_code);
    }
    if analysis.needs_infra > 0 {
        println!("  {} need infrastructure/setup", analysis.needs_infra);
    }
    if analysis.token_budget > 0 {
        println!("  {} are token budget issues", analysis.token_budget);
    }
    if analysis.needs_env > 0 {
        println!("  {} need special environment (GUI, etc.)", analysis.needs_env);
    }
    if analysis.architectural > 0 {
        println!("  {} have architectural concerns", analysis.architectural);
    }
    if analysis.dependencies > 0 {
        println!("  {} are waiting on dependencies", analysis.dependencies);
    }

    // Show suggested work if we have one
    if let Some(suggestion) = &analysis.suggested_work {
        println!();
        println!("Suggested unblocking work: {}", suggestion);

        // Create an ad-hoc task for the unblocking work
        let task = Task::new(
            "unblock",
            "Unblocking Work",
            format!(
                "Autopilot has determined that the following work would unblock multiple issues:\n\n\
                 {}\n\n\
                 Please analyze what's needed and implement it.",
                suggestion
            ),
        );

        println!();
        println!("Starting unblocking work...");
        println!();

        let result = adjutant.execute(&task).await?;
        print_result(&result);
        return Ok(());
    }

    // Fall back to directive-based work
    println!();
    println!("No clear unblocking path. Checking active directive...");

    if let Some(directive_id) = &workspace.active_directive {
        work_on_directive(workspace, directive_id, adjutant).await?;
    } else {
        println!();
        println!("No active directive set.");
        println!();
        println!("To proceed, either:");
        println!("  1. Unblock an issue manually");
        println!("  2. Set an active directive in .openagents/");
        println!("  3. Run with an ad-hoc task: autopilot run \"your task here\"");
    }

    Ok(())
}
