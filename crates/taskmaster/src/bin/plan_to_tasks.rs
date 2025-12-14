//! CLI for converting Claude plan files to taskmaster tasks
//!
//! Usage:
//!   plan-to-tasks [OPTIONS]
//!
//! Examples:
//!   # Process 3 most recent plans
//!   plan-to-tasks
//!
//!   # Dry run on specific plan
//!   plan-to-tasks --plan hashed-plotting-clover --dry-run
//!
//!   # Process from custom directory
//!   plan-to-tasks --dir /path/to/.claude --limit 5

use clap::Parser;
use colored::Colorize;
use std::path::PathBuf;

use taskmaster::plan_to_tasks::{
    convert_to_tasks, default_claude_dir, discover_plan_by_name, discover_plans,
    parse_plan_with_llm, print_summary,
};
use taskmaster::SqliteRepository;

#[derive(Parser, Debug)]
#[command(name = "plan-to-tasks")]
#[command(about = "Convert Claude plan files to taskmaster tasks")]
#[command(version)]
struct Args {
    /// Claude directory (default: ~/.claude)
    #[arg(short, long, env = "CLAUDE_DIR")]
    dir: Option<PathBuf>,

    /// Number of recent plans to process
    #[arg(short = 'n', long, default_value = "1")]
    limit: usize,

    /// Process specific plan by name (without .md extension)
    #[arg(short, long)]
    plan: Option<String>,

    /// Parse and show tasks without creating them
    #[arg(long)]
    dry_run: bool,

    /// Taskmaster database path
    #[arg(long, env = "TASKMASTER_DB", default_value = ".openagents/taskmaster.db")]
    db: PathBuf,

    /// Issue ID prefix
    #[arg(long, env = "TASKMASTER_PREFIX", default_value = "tm")]
    prefix: String,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Set up tracing if verbose
    if args.verbose {
        tracing_subscriber::fmt::init();
    }

    let claude_dir = args.dir.unwrap_or_else(default_claude_dir);

    println!(
        "{} Looking for plans in {}",
        "->".blue(),
        claude_dir.display()
    );

    // Discover plans
    let plans = if let Some(plan_name) = &args.plan {
        vec![discover_plan_by_name(&claude_dir, plan_name)?]
    } else {
        discover_plans(&claude_dir, args.limit)?
    };

    if plans.is_empty() {
        println!("{} No plan files found", "!".yellow());
        return Ok(());
    }

    println!(
        "{} Found {} plan(s) to process",
        "->".blue(),
        plans.len()
    );

    // Open taskmaster database
    let repo = if args.dry_run {
        SqliteRepository::in_memory()?
    } else {
        // Ensure directory exists
        if let Some(parent) = args.db.parent() {
            std::fs::create_dir_all(parent)?;
        }
        SqliteRepository::open(&args.db)?
    };

    let mut total_created = 0;
    let mut total_skipped = 0;

    for plan in plans {
        println!(
            "\n{} Processing: {}",
            "=>".green(),
            plan.name.cyan()
        );

        if args.verbose {
            println!("   Path: {}", plan.path.display());
            println!("   Size: {} bytes", plan.content.len());
        }

        // Parse with LLM
        println!("   {} Parsing with Claude...", "->".blue());
        let parsed = match parse_plan_with_llm(&plan.content, &plan.name).await {
            Ok(p) => p,
            Err(e) => {
                println!("   {} Failed to parse: {}", "!".red(), e);
                continue;
            }
        };

        println!(
            "   {} Extracted {} tasks from '{}'",
            "->".blue(),
            parsed.tasks.len().to_string().green(),
            parsed.title
        );

        if args.verbose {
            for task in &parsed.tasks {
                println!(
                    "      - [{}] {} ({})",
                    task.priority,
                    task.title,
                    task.issue_type
                );
            }
        }

        // Convert to taskmaster issues
        println!("   {} Converting to taskmaster issues...", "->".blue());
        let result = convert_to_tasks(&parsed, &plan.name, &repo, &args.prefix, args.dry_run)?;

        total_created += result.created.len();
        total_skipped += result.skipped.len();

        print_summary(&result, args.dry_run);
    }

    // Final summary
    println!("\n{}", "=".repeat(50));
    println!(
        "{} Total: {} created, {} skipped",
        if args.dry_run {
            "[DRY RUN]".yellow()
        } else {
            "[DONE]".green()
        },
        total_created.to_string().green(),
        total_skipped.to_string().yellow()
    );

    if !args.dry_run && total_created > 0 {
        println!(
            "\nTasks saved to: {}",
            args.db.display().to_string().cyan()
        );
        println!("Run `taskmaster list` to view them.");
    }

    Ok(())
}
