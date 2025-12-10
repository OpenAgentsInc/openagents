//! HillClimber CLI - MAP-based overnight optimization for Terminal-Bench.
//!
//! Usage:
//!   hillclimber --tasks regex-log --max-runs 100
//!   hillclimber --show-stats
//!   hillclimber run --tasks task1,task2 --max-runs 50
//!   hillclimber stats --task regex-log
//!   hillclimber list
//!   hillclimber export --output best_configs.json

use clap::{Parser, Subcommand};
use hillclimber::{
    format_score, HillClimberRunner, HillClimberStore, TerminalBenchTask, VerificationConfig,
};
use std::path::PathBuf;
use std::sync::Arc;

/// HillClimber: MAP-based overnight optimization for Terminal-Bench.
#[derive(Parser, Debug)]
#[command(name = "hillclimber")]
#[command(about = "MAP-based overnight optimization for Terminal-Bench tasks")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Task IDs to optimize (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    tasks: Vec<String>,

    /// Maximum number of runs before stopping
    #[arg(short, long, default_value = "100")]
    max_runs: u32,

    /// Sleep duration between runs (milliseconds)
    #[arg(long, default_value = "5000")]
    sleep_ms: u64,

    /// Show what would happen without executing
    #[arg(long)]
    dry_run: bool,

    /// Show stats and exit
    #[arg(long)]
    show_stats: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Database path
    #[arg(long, default_value = ".openagents/openagents.db")]
    database: PathBuf,

    /// Workspace directory
    #[arg(long, default_value = ".")]
    workspace: PathBuf,

    /// Maximum turns per run
    #[arg(long, default_value = "30")]
    max_turns: u32,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run tasks continuously
    Run {
        /// Task IDs to run (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        tasks: Vec<String>,

        /// Maximum number of runs
        #[arg(short, long, default_value = "100")]
        max_runs: u32,
    },

    /// Show statistics
    Stats {
        /// Show stats for a specific task
        #[arg(short, long)]
        task: Option<String>,
    },

    /// Export best configs to JSON
    Export {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
    },

    /// List all tasks with their status
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    // Ensure database directory exists
    if let Some(parent) = cli.database.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Open store
    let store = HillClimberStore::open(&cli.database)?;

    // Handle subcommands
    if let Some(command) = cli.command {
        match command {
            Commands::Run { tasks, max_runs } => {
                run_tasks(&cli.database, tasks, max_runs, cli.sleep_ms, cli.verbose, &cli.workspace, cli.max_turns).await?;
            }
            Commands::Stats { task } => {
                show_stats(&store, task)?;
            }
            Commands::Export { output } => {
                export_configs(&store, &output)?;
            }
            Commands::List => {
                list_tasks(&store)?;
            }
        }
        return Ok(());
    }

    // Handle legacy flags
    if cli.show_stats {
        show_stats(&store, None)?;
        return Ok(());
    }

    if cli.dry_run {
        println!("=== Dry Run ===");
        println!("Tasks: {:?}", cli.tasks);
        println!("Max runs: {}", cli.max_runs);
        println!("Sleep: {}ms", cli.sleep_ms);
        println!("Workspace: {:?}", cli.workspace);
        println!("Max turns: {}", cli.max_turns);
        return Ok(());
    }

    if !cli.tasks.is_empty() {
        run_tasks(&cli.database, cli.tasks, cli.max_runs, cli.sleep_ms, cli.verbose, &cli.workspace, cli.max_turns).await?;
    } else {
        println!("HillClimber: MAP-based overnight optimization for Terminal-Bench");
        println!();
        println!("Usage:");
        println!("  hillclimber --tasks task1,task2 --max-runs 100");
        println!("  hillclimber --show-stats");
        println!("  hillclimber run --tasks task1 --max-runs 50");
        println!("  hillclimber stats");
        println!("  hillclimber list");
        println!();
        println!("Use --help for more options.");
    }

    Ok(())
}

async fn run_tasks(
    db_path: &PathBuf,
    task_ids: Vec<String>,
    max_runs: u32,
    sleep_ms: u64,
    verbose: bool,
    workspace: &PathBuf,
    max_turns: u32,
) -> anyhow::Result<()> {
    // Open store for runner
    let store = HillClimberStore::open(db_path)?;

    // Create tasks from IDs, loading description from instruction.md
    let mut tasks: Vec<TerminalBenchTask> = Vec::new();
    for id in &task_ids {
        // Look for instruction.md in the workspace
        let instruction_path = workspace.join("instruction.md");
        let description = if instruction_path.exists() {
            std::fs::read_to_string(&instruction_path)
                .unwrap_or_else(|_| format!("Task: {}", id))
        } else {
            // Try workspace/task_id/instruction.md pattern
            let task_instruction = workspace.join(id).join("instruction.md");
            if task_instruction.exists() {
                std::fs::read_to_string(&task_instruction)
                    .unwrap_or_else(|_| format!("Task: {}", id))
            } else {
                format!("Task: {}", id)
            }
        };

        tasks.push(TerminalBenchTask {
            id: id.clone(),
            description,
            source_path: Some(workspace.clone()),
            verification: VerificationConfig {
                verification_type: "test".to_string(),
                command: Some("pytest -v".to_string()),
                script: None,
            },
        });
    }

    if tasks.is_empty() {
        println!("No tasks to run. Specify tasks with --tasks or see --help.");
        return Ok(());
    }

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                       HILLCLIMBER STARTING                        ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║ Tasks: {:60} ║", task_ids.join(", "));
    println!("║ Max runs: {:57} ║", max_runs);
    println!("║ Sleep: {:57}ms ║", sleep_ms);
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let store_arc = Arc::new(store);
    let runner = HillClimberRunner::new(store_arc.clone());
    let runs = runner.run_loop(tasks, max_runs, sleep_ms, verbose, max_turns).await?;

    // Summary
    let passed = runs.iter().filter(|r| r.passed).count();
    let total = runs.len();

    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                          SUMMARY                                  ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║ Total runs: {:55} ║", total);
    println!("║ Passed: {:52} ({:5.1}%) ║", passed, passed as f64 / total.max(1) as f64 * 100.0);
    println!("╚══════════════════════════════════════════════════════════════════╝");

    // Best scores by task
    println!();
    println!("Best scores by task:");
    for task_id in &task_ids {
        if let Ok(Some(stats)) = store_arc.get_task_stats(task_id) {
            println!(
                "  {} : {} ({:.1}% pass rate)",
                task_id,
                format_score(stats.best_score),
                stats.pass_rate * 100.0
            );
        }
    }

    Ok(())
}

fn show_stats(store: &HillClimberStore, task_id: Option<String>) -> anyhow::Result<()> {
    if let Some(task_id) = task_id {
        // Show stats for specific task
        let stats = store.get_task_stats(&task_id)?;
        if let Some(stats) = stats {
            println!("╔══════════════════════════════════════════════════════════════════╗");
            println!("║ Task: {:60} ║", task_id);
            println!("╠══════════════════════════════════════════════════════════════════╣");
            println!("║ Total runs: {:55} ║", stats.total_runs);
            println!("║ Pass count: {:55} ║", stats.pass_count);
            println!("║ Pass rate: {:53.1}% ║", stats.pass_rate * 100.0);
            println!("║ Best score: {:55} ║", format_score(stats.best_score));
            println!("║ Avg turns: {:54.1} ║", stats.avg_turns);
            if let Some(last_run) = &stats.last_run_at {
                println!("║ Last run: {:57} ║", last_run);
            }
            println!("╚══════════════════════════════════════════════════════════════════╝");
        } else {
            println!("No stats found for task: {}", task_id);
        }
    } else {
        // Show overall stats
        let stats = store.get_stats()?;
        println!("╔══════════════════════════════════════════════════════════════════╗");
        println!("║                     HILLCLIMBER STATISTICS                        ║");
        println!("╠══════════════════════════════════════════════════════════════════╣");
        println!("║ Total runs: {:55} ║", stats.total_runs);
        println!("║ Total passes: {:53} ║", stats.total_passes);
        println!("║ Pass rate: {:53.1}% ║", stats.overall_pass_rate * 100.0);
        println!("║ Unique tasks: {:53} ║", stats.unique_tasks);
        println!("║ Unique configs: {:51} ║", stats.unique_configs);
        println!("╚══════════════════════════════════════════════════════════════════╝");

        if !stats.by_task.is_empty() {
            println!();
            println!("By task:");
            for (task_id, task_stats) in &stats.by_task {
                println!(
                    "  {} : {} runs, {:.1}% pass, best {}",
                    task_id,
                    task_stats.total_runs,
                    task_stats.pass_rate * 100.0,
                    format_score(task_stats.best_score)
                );
            }
        }
    }

    Ok(())
}

fn export_configs(store: &HillClimberStore, output: &PathBuf) -> anyhow::Result<()> {
    let configs = store.get_best_configs()?;

    if configs.is_empty() {
        println!("No configs ready for export.");
        println!("Run some tasks first to generate best configs.");
        return Ok(());
    }

    let json = serde_json::to_string_pretty(&configs)?;
    std::fs::write(output, json)?;

    println!("Exported {} configs to {:?}", configs.len(), output);

    for config in &configs {
        println!("  - Task: {}", config.task_id);
    }

    Ok(())
}

fn list_tasks(store: &HillClimberStore) -> anyhow::Result<()> {
    let stats = store.get_stats()?;

    if stats.by_task.is_empty() {
        println!("No tasks found. Run some tasks first with:");
        println!("  hillclimber --tasks task1,task2 --max-runs 10");
        return Ok(());
    }

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                          TASK LIST                                ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");

    for (task_id, task_stats) in &stats.by_task {
        let status = if task_stats.pass_rate >= 1.0 {
            "✓ SOLVED"
        } else if task_stats.pass_rate > 0.5 {
            "◐ CLOSE"
        } else if task_stats.pass_rate > 0.0 {
            "○ PROGRESS"
        } else {
            "✗ UNSOLVED"
        };

        println!(
            "║ {:10} {:52} ║",
            status,
            format!(
                "{} - {} runs, {:.0}% pass, {}",
                task_id,
                task_stats.total_runs,
                task_stats.pass_rate * 100.0,
                format_score(task_stats.best_score)
            )
        );
    }

    println!("╚══════════════════════════════════════════════════════════════════╝");

    Ok(())
}
