//! Autopilot CLI subcommands
//!
//! Wraps autopilot crate CLI functions for unified binary.
//! Note: The full autopilot CLI is extensive; this exposes core commands.

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum AutopilotCommands {
    /// Run a task and log the trajectory
    Run {
        /// The task/prompt to execute
        prompt: String,

        /// Project name (loads cwd and issues_db from project)
        #[arg(short, long)]
        project: Option<String>,

        /// Working directory (default: current directory or from project)
        #[arg(short, long)]
        cwd: Option<PathBuf>,

        /// Model to use (sonnet, opus, haiku)
        #[arg(short, long, default_value = "sonnet")]
        model: String,

        /// Enable issue tracking tools via MCP
        #[arg(long)]
        with_issues: bool,

        /// Full auto mode: continuously work on issues
        #[arg(long)]
        full_auto: bool,
    },

    /// Resume a previous session
    Resume {
        /// Path to .json or .rlog trajectory file
        trajectory: Option<PathBuf>,

        /// Continue most recent session
        #[arg(long, short = 'c')]
        continue_last: bool,

        /// Additional prompt to send on resume
        #[arg(short, long)]
        prompt: Option<String>,
    },

    /// Issue management
    #[command(subcommand)]
    Issue(IssueCommands),

    /// View and analyze metrics
    #[command(subcommand)]
    Metrics(MetricsCommands),

    /// Run benchmarks
    Benchmark {
        /// Specific benchmark to run
        benchmark_id: Option<String>,

        /// Category to run
        #[arg(short, long)]
        category: Option<String>,
    },

    /// Start the dashboard web UI
    Dashboard {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },

    /// Replay a saved trajectory
    Replay {
        /// Path to trajectory file
        trajectory: PathBuf,
    },

    /// View APM (Actions Per Minute) statistics
    #[command(subcommand)]
    Apm(ApmCommands),
}

#[derive(Subcommand)]
pub enum IssueCommands {
    /// List issues
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Create a new issue
    Create {
        /// Issue title
        title: String,
        /// Issue body
        #[arg(short, long)]
        body: Option<String>,
    },
    /// Show issue details
    Show {
        /// Issue number
        number: i32,
    },
    /// Claim an issue for work
    Claim {
        /// Issue number
        number: i32,
    },
    /// Mark issue as complete
    Complete {
        /// Issue number
        number: i32,
    },
}

#[derive(Subcommand)]
pub enum MetricsCommands {
    /// Show metrics summary
    Show,
    /// Import metrics from trajectories
    Import {
        /// Path to trajectory directory
        path: PathBuf,
    },
    /// Analyze metrics trends
    Analyze,
}

#[derive(Subcommand)]
pub enum ApmCommands {
    /// Show APM statistics for different time windows
    Stats {
        /// Source to display (autopilot, claude_code, combined)
        #[arg(short, long)]
        source: Option<String>,
    },
    /// List APM sessions
    Sessions {
        /// Source filter (autopilot, claude_code)
        #[arg(short, long)]
        source: Option<String>,
        /// Limit number of results
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
}

pub fn run(cmd: AutopilotCommands) -> anyhow::Result<()> {
    // The autopilot CLI logic is currently embedded in main.rs.
    // For the unified binary, we print a message directing to the legacy binary
    // until we refactor autopilot to expose its CLI as a library.
    match cmd {
        AutopilotCommands::Dashboard { port } => {
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(async {
                // Use workspace root db path
                let db_path = autopilot::default_db_path();
                let db_str = db_path.to_string_lossy();
                autopilot::dashboard::start_dashboard(&db_str, port).await
            })
        }
        AutopilotCommands::Replay { trajectory } => {
            // Load trajectory first, then replay
            let traj = autopilot::replay::load_trajectory(&trajectory)?;
            autopilot::replay::interactive_replay(&traj)
        }
        AutopilotCommands::Apm(apm_cmd) => {
            use rusqlite::Connection;
            use autopilot::apm_storage::{init_apm_tables, get_latest_snapshot, get_sessions_by_source};
            use autopilot::apm::{APMSource, APMTier, APMWindow};
            use colored::Colorize;

            let db_path = autopilot::default_db_path();
            let conn = Connection::open(&db_path)?;
            init_apm_tables(&conn)?;

            match apm_cmd {
                ApmCommands::Stats { source } => {
                    let source_filter = source.as_deref().map(|s| match s {
                        "autopilot" => APMSource::Autopilot,
                        "claude_code" | "claude" => APMSource::ClaudeCode,
                        "combined" => APMSource::Combined,
                        _ => {
                            eprintln!("Invalid source: {}. Valid values: autopilot, claude_code", s);
                            std::process::exit(1);
                        }
                    });

                    println!("{}", "APM Statistics".cyan().bold());
                    println!("{}", "─".repeat(70).dimmed());
                    println!();

                    let sources = if let Some(s) = source_filter {
                        vec![s]
                    } else {
                        vec![APMSource::Autopilot, APMSource::ClaudeCode]
                    };

                    for src in sources {
                        let latest = get_latest_snapshot(&conn, src, APMWindow::Lifetime)?;

                        if let Some(snap) = latest {
                            let tier = APMTier::from_apm(snap.apm);
                            println!(
                                "{:<15} {:>8.1} APM  ({}) - {} messages, {} tool calls",
                                format!("{:?}", src).green().bold(),
                                snap.apm,
                                tier.name().yellow(),
                                snap.messages,
                                snap.tool_calls
                            );
                        } else {
                            println!("{:<15} {}", format!("{:?}", src).dimmed(), "No data".dimmed());
                        }
                    }

                    println!();
                    Ok(())
                }
                ApmCommands::Sessions { source, limit } => {
                    let src = source.as_deref().map(|s| match s {
                        "claude_code" => APMSource::ClaudeCode,
                        _ => APMSource::Autopilot,
                    }).unwrap_or(APMSource::Autopilot);

                    let sessions = get_sessions_by_source(&conn, src)?;

                    println!("{}", "APM Sessions".cyan().bold());
                    println!("{}", "─".repeat(70).dimmed());
                    println!();

                    if sessions.is_empty() {
                        println!("{}", "No sessions found".dimmed());
                    } else {
                        for (id, start_time, end_time) in sessions.iter().take(limit) {
                            let status = if end_time.is_some() { "✓" } else { "•" };
                            println!(
                                "{} {:<20} {}",
                                status.green(),
                                &id[..id.len().min(20)],
                                start_time.format("%Y-%m-%d %H:%M:%S")
                            );
                        }
                        println!();
                        println!("Showing {} of {} sessions", sessions.len().min(limit), sessions.len());
                    }

                    println!();
                    Ok(())
                }
            }
        }
        _ => {
            // For commands that need the full autopilot infrastructure,
            // we need to refactor autopilot's main.rs to expose the logic.
            // For now, provide helpful message.
            println!("This command requires the full autopilot runtime.");
            println!("Use: cargo autopilot {} ...",
                match cmd {
                    AutopilotCommands::Run { .. } => "run",
                    AutopilotCommands::Resume { .. } => "resume",
                    AutopilotCommands::Issue(_) => "issue",
                    AutopilotCommands::Metrics(_) => "metrics",
                    AutopilotCommands::Benchmark { .. } => "benchmark",
                    _ => "help",
                }
            );
            println!("\nNote: Autopilot CLI integration is in progress.");
            Ok(())
        }
    }
}
