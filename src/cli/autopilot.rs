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
