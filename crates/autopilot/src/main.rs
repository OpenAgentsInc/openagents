//! Autopilot CLI - Run autonomous tasks with Claude and log trajectories

use anyhow::Result;
use clap::{Parser, Subcommand};
use claude_agent_sdk::{
    QueryOptions, SdkMessage, SettingSource, query,
    HookCallback, HookCallbackMatcher, HookEvent, HookInput, HookOutput,
    SyncHookOutput, unstable_v2_create_session,
};
use async_trait::async_trait;
use colored::*;
use futures::StreamExt;
use serde_json::json;
use std::path::PathBuf;
use std::sync::OnceLock;

use autopilot::apm::APMTier;
use autopilot::analyze;
use autopilot::replay;
use autopilot::rlog::RlogWriter;
use autopilot::timestamp::{date_dir, filename, generate_slug};
use autopilot::trajectory::{StepType, Trajectory};
use autopilot::{extract_session_id_from_json, extract_session_id_from_rlog};
use autopilot::TrajectoryCollector;
use autopilot::trajectory_publisher::{TrajectoryPublishConfig, TrajectorySessionPublisher};
use autopilot::nip_sa_trajectory::TrajectoryPublisher as NipSaTrajectoryPublisher;

/// Minimum available memory in bytes before we abort (500 MB)
/// Note: macOS reports "available" memory conservatively - it doesn't count
/// reclaimable cached/inactive memory. 500MB is enough to start Claude.
const MIN_AVAILABLE_MEMORY_BYTES: u64 = 500 * 1024 * 1024;

/// Threshold to trigger memory cleanup (1.5 GB)
/// We try to free memory when we drop below this
const MEMORY_CLEANUP_THRESHOLD_BYTES: u64 = 1536 * 1024 * 1024;

/// Check if system has enough available memory
/// Returns (available_bytes, needs_cleanup, is_critical)
fn check_memory() -> (u64, bool, bool) {
    use sysinfo::System;
    let sys = System::new_all();
    let available = sys.available_memory();
    // If we get 0, something went wrong - don't abort, just return ok
    if available == 0 {
        return (0, false, false);
    }
    let needs_cleanup = available < MEMORY_CLEANUP_THRESHOLD_BYTES;
    let is_critical = available < MIN_AVAILABLE_MEMORY_BYTES;
    (available, needs_cleanup, is_critical)
}

/// List top memory-consuming processes and optionally kill Claude-related ones
fn check_and_kill_memory_hogs() -> u64 {
    use sysinfo::{System, Signal};

    let mut sys = System::new_all();
    sys.refresh_all();

    let available = sys.available_memory();
    let total = sys.total_memory();
    let used = total - available;

    println!("\n{}", "=".repeat(60).yellow());
    println!("{} Memory Status", "MEM:".yellow().bold());
    println!("  Total:     {}", format_bytes(total));
    println!("  Used:      {}", format_bytes(used));
    println!("  Available: {}", format_bytes(available));
    println!();

    // Collect processes with memory info
    let mut processes: Vec<_> = sys.processes()
        .iter()
        .map(|(pid, proc)| {
            let mem = proc.memory();
            let name = proc.name().to_string_lossy().to_string();
            (*pid, name, mem)
        })
        .collect();

    // Sort by memory usage descending
    processes.sort_by(|a, b| b.2.cmp(&a.2));

    println!("{} Top 15 Memory Hogs:", "PROCS:".yellow().bold());
    for (i, (pid, name, mem)) in processes.iter().take(15).enumerate() {
        let is_claude = name.to_lowercase().contains("claude") || name.to_lowercase().contains("node");
        let marker = if is_claude { " ← CLAUDE/NODE".red().bold().to_string() } else { String::new() };
        println!("  {:2}. {:>10}  {:6}  {}{}",
            i + 1,
            format_bytes(*mem),
            pid,
            name,
            marker
        );
    }

    // Find and kill stale claude/node processes (but not ourselves)
    let current_pid = std::process::id();
    let mut killed = 0;

    for (pid, name, mem) in processes.iter() {
        let name_lower = name.to_lowercase();
        // Kill node processes using > 500MB that aren't critical
        if name_lower.contains("node") && *mem > 500 * 1024 * 1024 {
            // Skip if it might be our parent process
            if pid.as_u32() == current_pid {
                continue;
            }

            if let Some(proc) = sys.process(*pid) {
                println!("{} Killing {} (PID {}, using {})",
                    "KILL:".red().bold(), name, pid, format_bytes(*mem));
                if proc.kill_with(Signal::Term).unwrap_or(false) {
                    killed += 1;
                }
            }
        }
    }

    if killed > 0 {
        println!("{} Killed {} memory hog processes", "CLEANUP:".green().bold(), killed);
        // Give processes time to die and memory to be reclaimed
        std::thread::sleep(std::time::Duration::from_secs(3));

        // Re-check memory after cleanup
        sys.refresh_memory();
        let new_available = sys.available_memory();
        println!("{} Memory after cleanup: {} (was {})",
            "MEM:".green().bold(),
            format_bytes(new_available),
            format_bytes(available));
        println!("{}", "=".repeat(60).yellow());
        return new_available;
    }

    println!("{}", "=".repeat(60).yellow());

    available
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}

/// Global storage for .mcp.json path to enable cleanup on panic/signal
static MCP_JSON_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Global storage for lockfile path to enable cleanup on panic/signal
static LOCKFILE_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Clean up .mcp.json file if it exists
fn cleanup_mcp_json() {
    if let Some(path) = MCP_JSON_PATH.get() {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Clean up lockfile if it exists
fn cleanup_lockfile() {
    if let Some(path) = LOCKFILE_PATH.get() {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Setup signal handlers and panic hook for cleanup
fn setup_cleanup_handlers() {
    // Setup panic hook to cleanup .mcp.json and lockfile
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        cleanup_mcp_json();
        // Note: lockfile intentionally NOT cleaned up here - stale lockfile indicates crash
        default_panic(info);
    }));

    // Setup signal handlers for SIGINT and SIGTERM
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));

    // Use iterator-based signal handling for cleanup
    std::thread::spawn(|| {
        match signal_hook::iterator::Signals::new(&[
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
        ]) {
            Ok(mut signals) => {
                if let Some(sig) = signals.forever().next() {
                    cleanup_mcp_json();
                    // Note: lockfile intentionally NOT cleaned up here - stale lockfile indicates crash
                    // Re-raise signal to ensure proper exit
                    signal_hook::low_level::raise(sig).ok();
                    std::process::exit(128 + sig);
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to create signal handler: {}", e);
                // Continue without signal handling
            }
        }
    });
}

/// Lockfile data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Lockfile {
    issue_number: Option<i32>,
    session_id: Option<String>,
    rlog_path: Option<String>,
    started_at: String,
}

/// Get the lockfile path in ~/.autopilot/run.lock
fn get_lockfile_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".autopilot").join("run.lock")
}

/// Write lockfile with run information
fn write_lockfile(
    issue_number: Option<i32>,
    session_id: Option<String>,
    rlog_path: Option<PathBuf>,
) -> std::io::Result<()> {
    let lockfile_path = get_lockfile_path();

    // Ensure parent directory exists
    if let Some(parent) = lockfile_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let lockfile = Lockfile {
        issue_number,
        session_id,
        rlog_path: rlog_path.map(|p| p.display().to_string()),
        started_at: chrono::Utc::now().to_rfc3339(),
    };

    let json = serde_json::to_string_pretty(&lockfile)?;
    std::fs::write(&lockfile_path, json)?;

    // Store path for cleanup
    LOCKFILE_PATH.set(lockfile_path).ok();

    Ok(())
}

/// Check for stale lockfile and block issue if found
async fn check_and_handle_stale_lockfile(cwd: &PathBuf) -> Result<()> {
    let lockfile_path = get_lockfile_path();

    if !lockfile_path.exists() {
        return Ok(());
    }

    // Read the lockfile
    let content = std::fs::read_to_string(&lockfile_path)?;
    let lockfile: Lockfile = serde_json::from_str(&content)?;

    eprintln!("{} Found stale lockfile from {}", "Warning:".yellow(), lockfile.started_at);

    // If there's an issue number, block it via MCP
    if let Some(issue_num) = lockfile.issue_number {
        eprintln!("{} Attempting to block issue #{} due to crash", "Crash:".red().bold(), issue_num);

        // Try to use the issues MCP to block the issue
        // Check if .mcp.json exists (issues tracking enabled)
        let mcp_json_path = cwd.join(".mcp.json");
        if mcp_json_path.exists() {
            // Use the handle_issue_command to block
            let reason = format!(
                "Autopilot crashed during execution. Session started at {}. Rlog: {:?}",
                lockfile.started_at,
                lockfile.rlog_path
            );

            use issues::{db, issue};
            let db_path = autopilot::default_db_path();
            let conn = db::init_db(&db_path)?;

            if let Some(i) = issue::get_issue_by_number(&conn, issue_num)? {
                if issue::block_issue(&conn, &i.id, &reason)? {
                    eprintln!("{} Blocked issue #{}", "✓".green(), issue_num);

                    // Print resume hint
                    if let Some(ref rlog) = lockfile.rlog_path {
                        eprintln!();
                        eprintln!("{}", "=".repeat(60).yellow());
                        eprintln!("{} To resume crashed session:", "→".cyan());
                        eprintln!("  {}", format!("autopilot resume {}", rlog).cyan());
                        eprintln!("{}", "=".repeat(60).yellow());
                    }
                } else {
                    eprintln!("{} Could not block issue #{}", "✗".red(), issue_num);
                }
            }
        }
    }

    // Remove the stale lockfile
    std::fs::remove_file(&lockfile_path)?;
    eprintln!("{} Removed stale lockfile", "Cleanup:".cyan());

    Ok(())
}

#[derive(Parser)]
#[command(name = "autopilot")]
#[command(about = "Run autonomous tasks with Claude and log trajectories")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a task and log the trajectory
    Run {
        /// The task/prompt to execute
        #[arg(required = true)]
        prompt: String,

        /// Project name (loads cwd and issues_db from project)
        #[arg(short, long)]
        project: Option<String>,

        /// Working directory (default: current directory or from project)
        #[arg(short, long)]
        cwd: Option<PathBuf>,

        /// Agent to use (claude or codex)
        #[arg(long, default_value = "claude")]
        agent: String,

        /// Model to use (sonnet, opus, haiku, or full model ID)
        #[arg(short, long, default_value_t = default_model())]
        model: String,

        /// Maximum turns
        #[arg(long, default_value_t = default_max_turns())]
        max_turns: u32,

        /// Maximum budget in USD
        #[arg(long, default_value_t = default_max_budget())]
        max_budget: f64,

        /// Output directory for logs (default: docs/logs/YYYYMMDD/)
        #[arg(short, long)]
        output_dir: Option<PathBuf>,

        /// Custom slug for filename (auto-generated if not provided)
        #[arg(long)]
        slug: Option<String>,

        /// Skip saving output files (just stream to stdout)
        #[arg(long)]
        dry_run: bool,

        /// Verbose output (show all messages)
        #[arg(short, long)]
        verbose: bool,

        /// Enable issue tracking tools via MCP
        #[arg(long)]
        with_issues: bool,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        issues_db: Option<PathBuf>,

        /// Full auto mode: continuously work on issues and discover new work
        #[arg(long, default_value_t = default_full_auto())]
        full_auto: bool,

        /// Launch desktop UI alongside autopilot
        #[arg(long, default_value_t = default_ui())]
        ui: bool,

        /// Publish trajectory to Nostr relays (NIP-SA kind:38030/38031)
        #[arg(long)]
        publish_trajectory: bool,
    },
    /// Replay a saved trajectory for debugging
    Replay {
        /// Path to trajectory JSON file
        #[arg(required = true)]
        trajectory: PathBuf,

        /// View mode: interactive (default), list, or summary
        #[arg(short, long, default_value = "interactive")]
        mode: String,
    },
    /// Compare two trajectories side-by-side
    Compare {
        /// Path to first trajectory JSON file
        #[arg(required = true)]
        trajectory1: PathBuf,

        /// Path to second trajectory JSON file
        #[arg(required = true)]
        trajectory2: PathBuf,
    },
    /// Analyze trajectory metrics
    Analyze {
        /// Path to trajectory JSON file or directory
        #[arg(required = true)]
        path: PathBuf,

        /// Aggregate metrics across all files in directory
        #[arg(long)]
        aggregate: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Resume a previous session
    Resume {
        /// Path to .json or .rlog trajectory file
        #[arg(required_unless_present = "continue_last")]
        trajectory: Option<PathBuf>,

        /// Continue most recent session (no file needed)
        #[arg(long, short = 'c')]
        continue_last: bool,

        /// Working directory (default: from trajectory or current)
        #[arg(short = 'd', long)]
        cwd: Option<PathBuf>,

        /// Additional prompt to send on resume
        #[arg(short, long)]
        prompt: Option<String>,

        /// Maximum budget in USD
        #[arg(long, default_value_t = default_max_budget())]
        max_budget: f64,

        /// Enable issue tracking tools via MCP
        #[arg(long)]
        with_issues: bool,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        issues_db: Option<PathBuf>,
    },
    /// Manage issues
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },
    /// Manage projects
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    /// View sessions
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    /// Manage directives
    Directive {
        #[command(subcommand)]
        command: DirectiveCommands,
    },
    /// Manage metrics
    Metrics {
        #[command(subcommand)]
        command: MetricsCommands,
    },
    /// Show APM (Actions Per Minute) statistics
    Apm {
        /// Time window to display (session, 1h, 6h, 1d, 1w, 1m, lifetime)
        #[arg(short, long)]
        window: Option<String>,

        /// Source to display (autopilot, claude_code, combined)
        #[arg(short, long)]
        source: Option<String>,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,
    },
    /// Run performance benchmarks
    Benchmark {
        /// Specific benchmark to run (e.g., B-001)
        benchmark_id: Option<String>,

        /// Category to run (e.g., file-ops, git)
        #[arg(short, long)]
        category: Option<String>,

        /// Compare against baseline version
        #[arg(short, long)]
        baseline: Option<String>,

        /// Save results as baseline version
        #[arg(short, long)]
        save_baseline: Option<String>,

        /// List all baselines
        #[arg(long)]
        list_baselines: bool,

        /// Path to benchmarks database (default: autopilot-benchmarks.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Workspace directory for benchmark execution
        #[arg(short, long)]
        workspace: Option<PathBuf>,
    },
    /// Manage trajectory logs
    Logs {
        #[command(subcommand)]
        command: LogsCommands,
    },
}

#[derive(Subcommand)]
enum IssueCommands {
    /// List issues
    List {
        /// Filter by status (open, in_progress, done)
        #[arg(short, long)]
        status: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Create a new issue
    Create {
        /// Issue title
        #[arg(required = true)]
        title: String,

        /// Issue description
        #[arg(short, long)]
        description: Option<String>,

        /// Priority (urgent, high, medium, low)
        #[arg(short, long, default_value = "medium")]
        priority: String,

        /// Issue type (task, bug, feature)
        #[arg(short = 't', long, default_value = "task")]
        issue_type: String,

        /// Agent to assign (claude or codex)
        #[arg(short, long, default_value = "claude")]
        agent: String,

        /// Directive to link to (e.g., d-001)
        #[arg(long)]
        directive: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Claim an issue
    Claim {
        /// Issue number
        #[arg(required = true)]
        number: i32,

        /// Run ID (default: manual-<timestamp>)
        #[arg(short, long)]
        run_id: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Mark an issue as complete
    Complete {
        /// Issue number
        #[arg(required = true)]
        number: i32,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Block an issue
    Block {
        /// Issue number
        #[arg(required = true)]
        number: i32,

        /// Reason for blocking
        #[arg(required = true)]
        reason: String,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Get the next ready issue
    Ready {
        /// Filter by agent (claude or codex)
        #[arg(short, long)]
        agent: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Export issues to JSON
    Export {
        /// Output file path (default: .openagents/issues.json)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include completed issues
        #[arg(long)]
        include_completed: bool,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Import issues from JSON
    Import {
        /// Input file path (default: .openagents/issues.json)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Force update existing issues with same UUID
        #[arg(long)]
        force: bool,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// Add a new project
    Add {
        /// Project name
        #[arg(required = true)]
        name: String,

        /// Project path
        #[arg(short, long, required = true)]
        path: PathBuf,

        /// Project description
        #[arg(short, long)]
        description: Option<String>,

        /// Default model (sonnet, opus, haiku)
        #[arg(short, long)]
        model: Option<String>,

        /// Default budget in USD
        #[arg(short, long)]
        budget: Option<f64>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// List all projects
    List {
        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Remove a project
    Remove {
        /// Project name
        #[arg(required = true)]
        name: String,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum SessionCommands {
    /// List sessions
    List {
        /// Filter by project name
        #[arg(short, long)]
        project: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show session details
    Show {
        /// Session ID (or prefix)
        #[arg(required = true)]
        id: String,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum DirectiveCommands {
    /// List all directives
    List {
        /// Filter by status (active, paused, completed)
        #[arg(short, long)]
        status: Option<String>,

        /// Path to issues database (for progress calculation)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show directive details
    Show {
        /// Directive ID (e.g., 'd-001')
        #[arg(required = true)]
        id: String,

        /// Path to issues database (for progress calculation)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Create a new directive
    Create {
        /// Directive ID (e.g., 'd-001')
        #[arg(required = true)]
        id: String,

        /// Directive title
        #[arg(required = true)]
        title: String,

        /// Priority (urgent, high, medium, low)
        #[arg(short, long, default_value = "medium")]
        priority: String,
    },
    /// Pause a directive
    Pause {
        /// Directive ID
        #[arg(required = true)]
        id: String,
    },
    /// Complete a directive
    Complete {
        /// Directive ID
        #[arg(required = true)]
        id: String,
    },
    /// Resume a paused directive
    Resume {
        /// Directive ID
        #[arg(required = true)]
        id: String,
    },
}

#[derive(Subcommand)]
enum MetricsCommands {
    /// Import metrics from trajectory logs
    Import {
        /// Directory containing trajectory logs (default: docs/logs/YYYYMMDD)
        #[arg(required = true)]
        log_dir: PathBuf,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show detailed metrics for a session
    Show {
        /// Session ID
        #[arg(required = true)]
        session_id: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show quick statistics for a session (concise view)
    Stats {
        /// Session ID (default: most recent session)
        session_id: Option<String>,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// List all recorded sessions
    List {
        /// Filter by status (completed, crashed, budget_exhausted, max_turns, running)
        #[arg(short, long)]
        status: Option<String>,

        /// Limit number of results
        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Analyze aggregate metrics and detect regressions
    Analyze {
        /// Time period (7d, 30d, last-week, this-week)
        #[arg(short, long, default_value = "7d")]
        period: String,

        /// Compare two date ranges (format: YYYY-MM-DD..YYYY-MM-DD)
        #[arg(long)]
        compare: Option<String>,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show trends by comparing periods
    Trends {
        /// Recent period (7d, 30d, last-week, this-week)
        #[arg(short = 'r', long, default_value = "this-week")]
        recent: String,

        /// Baseline period (7d, 30d, last-week, this-week)
        #[arg(short = 'b', long, default_value = "last-week")]
        baseline: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Automatically create improvement issues from detected anomalies
    CreateIssues {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        issues_db: Option<PathBuf>,

        /// Dry run - show what would be created without creating issues
        #[arg(long)]
        dry_run: bool,
    },

    /// Start web dashboard for metrics visualization
    Dashboard {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Port to bind to (default: 3000)
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },

    /// Run learning pipeline to analyze sessions and propose improvements
    Learn {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Specific session IDs to analyze (default: last 50 sessions)
        #[arg(long)]
        sessions: Vec<String>,

        /// Number of recent sessions to analyze if no specific sessions provided
        #[arg(long, default_value_t = 50)]
        limit: usize,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Export metrics to JSON/CSV for external analysis
    Export {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Time period (7d, 30d, last-week, this-week, or 'all')
        #[arg(short, long, default_value = "all")]
        period: String,

        /// Output format (json or csv)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Baseline management commands
    #[command(subcommand)]
    Baseline(BaselineCommands),

    /// Backfill APM data for existing sessions
    BackfillApm {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Alert management commands
    #[command(subcommand)]
    Alerts(AlertCommands),

    /// Show aggregate metrics for a specific issue
    IssueMetrics {
        /// Issue number
        #[arg(required = true)]
        issue_number: i32,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show aggregate metrics for a specific directive
    DirectiveMetrics {
        /// Directive ID (e.g., d-004)
        #[arg(required = true)]
        directive_id: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// List all issues with their aggregate metrics
    ByIssue {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// List all directives with their aggregate metrics
    ByDirective {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show improvement velocity over time
    Velocity {
        /// Time period to analyze (7d, 30d, this-week, last-week)
        #[arg(short, long, default_value = "this-week")]
        period: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Number of historical snapshots to show
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum AlertCommands {
    /// List all configured alert rules
    List {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Add a new alert rule
    Add {
        /// Metric name to monitor
        #[arg(long)]
        metric: String,

        /// Alert type (threshold, regression, rate_of_change)
        #[arg(long)]
        alert_type: String,

        /// Severity (warning, error, critical)
        #[arg(long)]
        severity: String,

        /// Threshold value
        #[arg(long)]
        threshold: f64,

        /// Description of what this alert detects
        #[arg(long)]
        description: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Remove an alert rule
    Remove {
        /// Alert rule ID
        rule_id: i64,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Show alert history
    History {
        /// Filter by session ID
        #[arg(long)]
        session: Option<String>,

        /// Filter by metric name
        #[arg(long)]
        metric: Option<String>,

        /// Limit number of results
        #[arg(short, long, default_value_t = 50)]
        limit: usize,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum BaselineCommands {
    /// Update baselines from recent sessions
    Update {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Number of recent sessions to use for baseline calculation
        #[arg(long, default_value_t = 100)]
        sessions: usize,
    },

    /// Show current baselines
    Show {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Check for regressions against baselines
    Check {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Number of recent sessions to check
        #[arg(long, default_value_t = 20)]
        sessions: usize,
    },

    /// Generate baseline report
    Report {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Output file (default: docs/autopilot/BASELINES.md)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum LogsCommands {
    /// Show log statistics
    Stats {
        /// Logs directory (default: docs/logs)
        #[arg(short, long)]
        logs_dir: Option<PathBuf>,
    },
    /// Archive old logs (compress to .gz)
    Archive {
        /// Age in days before archiving (default: 30)
        #[arg(short, long, default_value_t = 30)]
        days: i64,

        /// Logs directory (default: docs/logs)
        #[arg(short, long)]
        logs_dir: Option<PathBuf>,

        /// Dry run (show what would be archived)
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean up old archived logs
    Cleanup {
        /// Age in days before deletion (default: 90)
        #[arg(short, long, default_value_t = 90)]
        days: i64,

        /// Logs directory (default: docs/logs)
        #[arg(short, long)]
        logs_dir: Option<PathBuf>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Dry run (show what would be deleted)
        #[arg(long)]
        dry_run: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup cleanup handlers for signals and panics
    setup_cleanup_handlers();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            prompt,
            project,
            cwd,
            agent,
            model,
            max_turns,
            max_budget,
            output_dir,
            slug,
            dry_run,
            verbose,
            with_issues,
            issues_db,
            full_auto,
            ui,
            publish_trajectory,
        } => {
            run_task(
                prompt, project, cwd, agent, model, max_turns, max_budget, output_dir, slug, dry_run, verbose,
                with_issues, issues_db, full_auto, ui, publish_trajectory,
            )
            .await
        }
        Commands::Replay { trajectory, mode } => {
            replay_trajectory(trajectory, mode).await
        }
        Commands::Compare { trajectory1, trajectory2 } => {
            compare_trajectories(trajectory1, trajectory2).await
        }
        Commands::Analyze { path, aggregate, json } => {
            analyze_trajectories(path, aggregate, json).await
        }
        Commands::Resume {
            trajectory,
            continue_last,
            cwd,
            prompt,
            max_budget,
            with_issues,
            issues_db,
        } => {
            resume_task(
                trajectory, continue_last, cwd, prompt, max_budget, with_issues, issues_db,
            )
            .await
        }
        Commands::Issue { command } => {
            handle_issue_command(command).await
        }
        Commands::Project { command } => {
            handle_project_command(command).await
        }
        Commands::Session { command } => {
            handle_session_command(command).await
        }
        Commands::Directive { command } => {
            handle_directive_command(command).await
        }
        Commands::Metrics { command } => {
            handle_metrics_command(command).await
        }
        Commands::Apm { window, source, metrics_db } => {
            handle_apm_command(window, source, metrics_db).await
        }
        Commands::Benchmark {
            benchmark_id,
            category,
            baseline,
            save_baseline,
            list_baselines,
            db,
            workspace,
        } => {
            handle_benchmark_command(
                benchmark_id,
                category,
                baseline,
                save_baseline,
                list_baselines,
                db,
                workspace,
            )
            .await
        }
        Commands::Logs { command } => handle_logs_command(command).await,
    }
}

/// Get default model from environment or fallback to "sonnet"
fn default_model() -> String {
    std::env::var("AUTOPILOT_MODEL").unwrap_or_else(|_| "sonnet".to_string())
}

/// Get default full_auto from environment or fallback to false
fn default_full_auto() -> bool {
    std::env::var("AUTOPILOT_FULL_AUTO")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Get default ui from environment or fallback to false
fn default_ui() -> bool {
    std::env::var("AUTOPILOT_UI")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Get default max_turns from environment or fallback to 9999 (effectively unlimited)
fn default_max_turns() -> u32 {
    std::env::var("AUTOPILOT_MAX_TURNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9999)
}

/// Get default max_budget from environment or fallback to 0.0 (no constraint)
fn default_max_budget() -> f64 {
    std::env::var("AUTOPILOT_MAX_BUDGET")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0)
}

/// Map friendly model names to full model IDs
fn resolve_model(model: &str) -> String {
    match model.to_lowercase().as_str() {
        "sonnet" => "claude-sonnet-4-5-20250929".to_string(),
        "opus" => "claude-opus-4-5-20251101".to_string(),
        "haiku" => "claude-haiku-4-20250514".to_string(),
        // If not a friendly name, assume it's a full model ID
        _ => model.to_string(),
    }
}

/// Compaction hook to provide custom instructions
struct CompactionHook;

#[async_trait]
impl HookCallback for CompactionHook {
    async fn call(
        &self,
        input: HookInput,
        _tool_use_id: Option<String>,
    ) -> Result<HookOutput, claude_agent_sdk::Error> {
        if let HookInput::PreCompact(compact_input) = input {
            // Detect appropriate strategy based on session (simple heuristic for now)
            let strategy = if compact_input.base.session_id.contains("auto") {
                autopilot::compaction::CompactionStrategy::Autonomous
            } else {
                autopilot::compaction::CompactionStrategy::Detailed
            };

            let custom_instructions = autopilot::compaction::generate_compaction_prompt(
                strategy,
                compact_input.custom_instructions.as_deref(),
            );

            eprintln!("🔄 Compaction triggered ({})",
                if matches!(compact_input.trigger, claude_agent_sdk::CompactTrigger::Auto) {
                    "auto"
                } else {
                    "manual"
                });
            eprintln!("📝 Using strategy: {}", strategy.as_str());

            return Ok(HookOutput::Sync(SyncHookOutput {
                decision: Some(claude_agent_sdk::HookDecision::Approve),
                system_message: Some(custom_instructions),
                hook_specific_output: None,
                ..Default::default()
            }));
        }

        Ok(HookOutput::Sync(SyncHookOutput {
            decision: Some(claude_agent_sdk::HookDecision::Approve),
            system_message: None,
            hook_specific_output: None,
            ..Default::default()
        }))
    }
}

/// Plan mode hook to enforce restrictions
struct PlanModeHook;

#[async_trait]
impl HookCallback for PlanModeHook {
    async fn call(
        &self,
        input: HookInput,
        _tool_use_id: Option<String>,
    ) -> Result<HookOutput, claude_agent_sdk::Error> {
        match input {
            HookInput::PreToolUse(pre) => {
                // Check if this tool is allowed in plan mode
                if let Err(reason) = autopilot::planmode::is_tool_allowed_in_plan_mode(
                    &pre.tool_name,
                    &pre.tool_input,
                ) {
                    // Block the tool
                    return Ok(SyncHookOutput::block(&reason).into());
                }
                // Allow the tool
                Ok(SyncHookOutput::continue_execution().into())
            }
            _ => Ok(SyncHookOutput::continue_execution().into()),
        }
    }
}

/// Load active directives and format as a summary for the prompt
fn load_directive_summary(cwd: &std::path::Path) -> String {
    use issues::directive;

    let directives_dir = cwd.join(".openagents/directives");
    let directives = match directive::get_active_directives(&directives_dir) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };

    if directives.is_empty() {
        return String::new();
    }

    let mut summary = String::from("\n\nACTIVE DIRECTIVES (high-level goals guiding your work):\n");
    for d in &directives {
        // Extract first line of Goal section if present
        let goal_line = d.body.lines()
            .skip_while(|l| !l.starts_with("## Goal"))
            .nth(2)  // Skip "## Goal" and blank line
            .unwrap_or("")
            .trim();

        summary.push_str(&format!(
            "\n[{}] {} (priority: {})\n  {}\n",
            d.id, d.title, d.priority.as_str(),
            if goal_line.is_empty() { "See directive for details" } else { goal_line }
        ));
    }
    summary.push_str("\nUse directive_get <id> for full details, success criteria, and phases.\n");
    summary.push_str("When creating issues, link them with: issue_create title=\"...\" directive_id=\"<id>\"\n");

    summary
}

/// Full auto mode prompt suffix (without directives - those are added dynamically)
const FULL_AUTO_PROMPT_BASE: &str = r#"

FULL AUTO MODE - CRITICAL AUTONOMOUS LOOP INSTRUCTIONS:

You are in FULLY AUTONOMOUS mode. You MUST follow this exact loop:

LOOP START:
1. Call issue_ready to get the next available issue
2. If issue exists:
   - Check if the issue is linked to a directive (has directive_id)
   - If linked, review that directive with directive_get to understand the bigger picture
   - Implement → test → commit → PUSH → complete
3. IMMEDIATELY call issue_ready again (NO SUMMARIES, NO PAUSES)
4. GOTO LOOP START

IF issue_ready returns "No ready issues available":
- Review the active directives shown above
- Pick the highest priority directive that needs work
- Create 1-3 specific, actionable issues linked to it using:
  issue_create title="..." directive_id="<id>"
- Claim and implement the new issue
- GOTO LOOP START

CRITICAL RULES - VIOLATION MEANS FAILURE:
- NEVER output a "session summary" or "issues completed" message
- NEVER stop to reflect on progress
- NEVER say "I've completed X issues" - this is a STOP SIGNAL, do NOT emit it
- After EVERY issue_complete, your VERY NEXT action MUST be issue_ready
- The ONLY valid reasons to stop: budget exhausted error, max turns error, or system crash
- If you output text without immediately calling issue_ready after completing an issue, YOU HAVE FAILED

IMPORTANT: Always `git push origin main` after each commit.

This is an INFINITE LOOP. You run until the system stops you. DO NOT STOP YOURSELF.
"#;

/// Run Claude agent with the given prompt
async fn run_claude_agent(
    prompt: &str,
    options: QueryOptions,
    collector: &mut TrajectoryCollector,
    verbose: bool,
    ui_port: Option<u16>,
) -> Result<()> {
    let mut stream = query(prompt, options).await?;

    while let Some(msg) = stream.next().await {
        let msg = msg?;

        // Collect trajectory
        collector.process_message(&msg);

        // Stream to desktop UI if enabled
        if let Some(port) = ui_port {
            if let Some(html) = autopilot::ui_renderer::render_sdk_message(&msg) {
                let _ = stream_to_desktop(port, html.into_string()).await;
            }
        }

        // Print progress
        if verbose {
            print_message(&msg);
        } else {
            print_progress(&msg);
        }
    }

    Ok(())
}

/// Full-auto loop: keeps running until budget exhausted
/// If agent stops prematurely, we detect it and force continuation
async fn run_full_auto_loop(
    initial_prompt: &str,
    options: QueryOptions,
    collector: &mut TrajectoryCollector,
    verbose: bool,
    ui_port: Option<u16>,
    cwd: &PathBuf,
    issues_db: Option<&PathBuf>,
) -> Result<()> {
    use issues::{db, issue};

    let mut continuation_count = 0;
    const MAX_CONTINUATIONS: u32 = 1000; // Safety limit
    let mut current_prompt = initial_prompt.to_string();

    loop {
        // Check memory at start of each iteration
        let (available_mem, needs_cleanup, is_critical) = check_memory();

        if needs_cleanup || is_critical {
            println!("\n{} Memory {} ({}) - checking for processes to kill...",
                "MEMORY:".yellow().bold(),
                if is_critical { "critical" } else { "low" },
                format_bytes(available_mem));

            // Try to free memory by killing hogs with retry
            let mut new_avail = check_and_kill_memory_hogs();

            // If still critical after cleanup, wait and retry a few times
            // macOS can take a moment to reclaim memory
            if new_avail < MIN_AVAILABLE_MEMORY_BYTES {
                for retry in 1..=3 {
                    println!("{} Waiting for memory to be reclaimed (attempt {}/3)...",
                        "MEM:".yellow().bold(), retry);
                    std::thread::sleep(std::time::Duration::from_secs(2));

                    let (new_check, _, still_critical) = check_memory();
                    new_avail = new_check;

                    if !still_critical {
                        println!("{} Memory recovered to {} - continuing",
                            "MEMORY:".green().bold(), format_bytes(new_avail));
                        break;
                    }
                }
            }

            // Final check after all retries
            if new_avail < MIN_AVAILABLE_MEMORY_BYTES {
                println!("\n{} Still insufficient memory ({}) after cleanup - aborting",
                    "MEMORY:".red().bold(),
                    format_bytes(new_avail));
                anyhow::bail!("Insufficient memory: {} available, {} required",
                    format_bytes(new_avail),
                    format_bytes(MIN_AVAILABLE_MEMORY_BYTES));
            } else {
                println!("{} Memory recovered to {} - continuing", "MEMORY:".green().bold(), format_bytes(new_avail));
            }
        }

        // Log memory status periodically (every iteration or every 5)
        if continuation_count % 5 == 0 {
            println!("{} Available memory: {}", "MEM:".dimmed(), format_bytes(available_mem));
        }

        // Use query() directly - same approach as run_claude_agent which works
        // For continuations, set continue_session=true to resume conversation
        let mut query_options = options.clone();
        if continuation_count > 0 {
            query_options.continue_session = true;
        }

        let mut stream = query(&current_prompt, query_options).await?;

        // Process messages until stream ends
        let mut budget_exhausted = false;
        let mut max_turns_reached = false;
        let mut message_count = 0;

        while let Some(msg) = stream.next().await {
            message_count += 1;

            // Check memory every 10 messages
            if message_count % 10 == 0 {
                let (avail, needs_cleanup, is_critical) = check_memory();
                if message_count % 100 == 0 {
                    println!("{} Memory: {}", "MEM:".dimmed(), format_bytes(avail));
                }
                if is_critical {
                    println!("\n{} Memory critical ({}) - attempting cleanup", "MEMORY:".yellow().bold(), format_bytes(avail));
                    let mut new_avail = check_and_kill_memory_hogs();

                    // Retry a couple times for memory to be reclaimed
                    if new_avail < MIN_AVAILABLE_MEMORY_BYTES {
                        for _ in 1..=2 {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            let (check, _, still_critical) = check_memory();
                            new_avail = check;
                            if !still_critical { break; }
                        }
                    }

                    if new_avail < MIN_AVAILABLE_MEMORY_BYTES {
                        anyhow::bail!("Memory critical after cleanup: {} available", format_bytes(new_avail));
                    }
                } else if needs_cleanup && message_count % 50 == 0 {
                    // Proactive cleanup when memory is getting low (not critical)
                    println!("{} Memory getting low ({}) - proactive cleanup", "MEM:".yellow(), format_bytes(avail));
                    check_and_kill_memory_hogs();
                }
            }
            let msg = msg?;
            collector.process_message(&msg);

            // Check if this is a result message indicating session end
            if let SdkMessage::Result(ref result) = msg {
                // Check for budget/turns exhaustion based on result type
                match result {
                    claude_agent_sdk::SdkResultMessage::ErrorMaxBudget(_) => {
                        budget_exhausted = true;
                    }
                    claude_agent_sdk::SdkResultMessage::ErrorMaxTurns(_) => {
                        max_turns_reached = true;
                    }
                    claude_agent_sdk::SdkResultMessage::ErrorDuringExecution(e) => {
                        // Check error messages for budget/turn related errors
                        for err in &e.errors {
                            let err_lower = err.to_lowercase();
                            if err_lower.contains("budget") || err_lower.contains("cost") {
                                budget_exhausted = true;
                            }
                            if err_lower.contains("turn") || err_lower.contains("max_turn") {
                                max_turns_reached = true;
                            }
                        }
                    }
                    claude_agent_sdk::SdkResultMessage::Success(_) => {
                        // Success means the agent decided to stop - we may need to continue
                    }
                    _ => {}
                }
            }

            // Stream to UI
            if let Some(port) = ui_port {
                if let Some(html) = autopilot::ui_renderer::render_sdk_message(&msg) {
                    let _ = stream_to_desktop(port, html.into_string()).await;
                }
            }

            // Print progress
            if verbose {
                print_message(&msg);
            } else {
                print_progress(&msg);
            }
        }

        // If budget or turns exhausted, we're done
        if budget_exhausted {
            println!("\n{} Budget exhausted - stopping full-auto loop", "STOP:".red().bold());
            break;
        }
        if max_turns_reached {
            println!("\n{} Max turns reached - stopping full-auto loop", "STOP:".red().bold());
            break;
        }

        // Safety limit
        continuation_count += 1;
        if continuation_count >= MAX_CONTINUATIONS {
            println!("\n{} Max continuations ({}) reached", "STOP:".yellow().bold(), MAX_CONTINUATIONS);
            break;
        }

        // Check if there are more issues to work on
        let default_db = autopilot::default_db_path();
        let db_path = issues_db.unwrap_or(&default_db);

        let has_more_work = if let Ok(conn) = db::init_db(db_path) {
            issue::get_next_ready_issue(&conn, Some("claude"))?.is_some()
        } else {
            false
        };

        if !has_more_work {
            // No more issues - but in full-auto we should create new work
            println!("\n{} No ready issues - sending continuation to create work", "AUTO:".cyan().bold());
        } else {
            println!("\n{} Issues still available - forcing continuation", "AUTO:".cyan().bold());
        }

        // Set up for continuation - include directive summary and FULL_AUTO_PROMPT again
        println!("{} Continuing with new query (attempt {})", "AUTO:".yellow().bold(), continuation_count);

        let directive_summary = load_directive_summary(&cwd);
        current_prompt = if has_more_work {
            format!("{}{}\n\nCONTINUE: You stopped prematurely. There are still issues to work on. Call issue_ready NOW. DO NOT output any text first - immediately call issue_ready.", directive_summary, FULL_AUTO_PROMPT_BASE)
        } else {
            format!("{}{}\n\nCONTINUE: You stopped prematurely. No issues are ready. Review the directives above and create a new issue linked to one. DO NOT output any text first.", directive_summary, FULL_AUTO_PROMPT_BASE)
        };
    }

    Ok(())
}

/// Run Codex agent with the given prompt
async fn run_codex_agent(
    prompt: &str,
    cwd: &PathBuf,
    _max_turns: u32,
    _max_budget: f64,
    _collector: &mut TrajectoryCollector,
    verbose: bool,
) -> Result<()> {
    use codex_agent_sdk::{Codex, SandboxMode, ThreadOptions, TurnOptions};

    let codex = Codex::new();
    let thread_options = ThreadOptions {
        working_directory: Some(cwd.clone()),
        sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        ..Default::default()
    };

    let mut thread = codex.start_thread(thread_options);
    let mut streamed = thread.run_streamed(prompt, TurnOptions::default()).await?;

    let mut turn_items = Vec::new();
    let mut usage = None;

    while let Some(event_result) = streamed.next().await {
        let event = event_result?;

        // Add to trajectory collector
        _collector.process_codex_event(&event);

        // Process events for console output
        match &event {
            codex_agent_sdk::ThreadEvent::ThreadStarted(e) => {
                if verbose {
                    println!("{} Thread started: {}", "Codex:".cyan().bold(), e.thread_id);
                }
            }
            codex_agent_sdk::ThreadEvent::TurnStarted(_) => {
                if verbose {
                    println!("{} Turn started", "Codex:".dimmed());
                }
            }
            codex_agent_sdk::ThreadEvent::ItemStarted(e) => {
                if verbose {
                    println!("{} Item started: {:?}", "Codex:".dimmed(), e.item.details);
                }
            }
            codex_agent_sdk::ThreadEvent::ItemUpdated(_) => {
                // Progress updates
            }
            codex_agent_sdk::ThreadEvent::ItemCompleted(e) => {
                turn_items.push(e.item.clone());

                use codex_agent_sdk::ThreadItemDetails;
                match &e.item.details {
                    ThreadItemDetails::AgentMessage(msg) => {
                        if verbose {
                            println!("{} {}", "Agent:".cyan().bold(), msg.text);
                        }
                    }
                    ThreadItemDetails::CommandExecution(cmd) => {
                        println!("{} Executing: {}", "Command:".yellow().bold(), cmd.command);
                        if verbose && !cmd.aggregated_output.is_empty() {
                            println!("{}", cmd.aggregated_output);
                        }
                    }
                    ThreadItemDetails::FileChange(file) => {
                        println!("{} {} file(s) changed", "File change:".green().bold(), file.changes.len());
                        if verbose {
                            for change in &file.changes {
                                println!("  {}", change.path);
                            }
                        }
                    }
                    ThreadItemDetails::Reasoning(reasoning) => {
                        if verbose {
                            println!("{} {}", "Reasoning:".magenta().dimmed(), reasoning.text);
                        }
                    }
                    _ => {}
                }
            }
            codex_agent_sdk::ThreadEvent::TurnCompleted(e) => {
                usage = Some(e.usage.clone());
                if verbose {
                    println!("{} Turn completed", "Codex:".green().bold());
                    println!("  Input tokens: {}", e.usage.input_tokens);
                    println!("  Output tokens: {}", e.usage.output_tokens);
                }
            }
            codex_agent_sdk::ThreadEvent::TurnFailed(e) => {
                eprintln!("{} Turn failed: {}", "Error:".red().bold(), e.error.message);
                anyhow::bail!("Codex turn failed: {}", e.error.message);
            }
            codex_agent_sdk::ThreadEvent::Error(e) => {
                eprintln!("{} {}", "Error:".red().bold(), e.message);
                anyhow::bail!("Codex error: {}", e.message);
            }
        }
    }

    // Add summary to trajectory collector
    // Note: TrajectoryCollector expects SdkMessage format, but for now we can add a simple result
    // This would need proper adapter in the future for full Codex trajectory support
    if let Some(usage) = usage {
        // Add usage tracking
        // For now, just print - full trajectory integration would need TrajectoryEvent adapter
        println!("{} Total tokens: {}", "Usage:".dimmed(),
            usage.input_tokens + usage.output_tokens);
    }

    Ok(())
}

/// Publish trajectory to Nostr relays
async fn publish_trajectory_to_nostr(
    trajectory: &Trajectory,
    session_id: Option<&String>,
) -> Result<()> {
    use anyhow::Context;
    use bip39::Mnemonic;
    use nostr::TrajectoryVisibility;
    use std::str::FromStr;
    use std::sync::Arc;
    use wallet::core::UnifiedIdentity;
    use wallet::storage::config::WalletConfig;
    use wallet::storage::keychain::SecureKeychain;

    // Load wallet config to get relay URLs
    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        anyhow::bail!("No Nostr relays configured in wallet.toml");
    }

    // Try to load identity from keychain
    let identity = if SecureKeychain::has_mnemonic() {
        let mnemonic_str = SecureKeychain::retrieve_mnemonic()?;
        let mnemonic = Mnemonic::from_str(&mnemonic_str)?;
        Arc::new(UnifiedIdentity::from_mnemonic(mnemonic)?)
    } else {
        eprintln!("{} No wallet identity found. Run 'openagents wallet init' first.", "Warning:".yellow());
        anyhow::bail!("No wallet identity found");
    };

    // Create trajectory publish config
    let publish_config = TrajectoryPublishConfig::new(config.nostr.relays.clone());

    // Publish TrajectorySession (kind:38030)
    let tick_id = session_id
        .map(|s| s.clone())
        .unwrap_or_else(|| trajectory.session_id.clone());

    let started_at = trajectory.started_at.timestamp() as u64;

    let session_publisher = TrajectorySessionPublisher::with_identity(publish_config, identity.clone());

    println!("{} Publishing trajectory session to Nostr relays...", "Publishing:".cyan());

    match session_publisher
        .publish_session(&trajectory.session_id, &tick_id, &trajectory.model, started_at)
        .await
    {
        Ok(Some(event_id)) => {
            println!("{} Trajectory session published: {}", "✓".green(), event_id);
        }
        Ok(None) => {
            eprintln!("{} Trajectory session publishing was skipped", "Warning:".yellow());
        }
        Err(e) => {
            eprintln!("{} Failed to publish trajectory session: {}", "Error:".red(), e);
            return Err(e);
        }
    }

    // Publish individual trajectory events (kind:38031)
    let nip_sa_publisher = NipSaTrajectoryPublisher::new(&trajectory.session_id, &tick_id);
    let events = nip_sa_publisher.trajectory_to_events(trajectory);

    // Create session with trajectory hash
    let _session_with_hash = nip_sa_publisher.create_session_with_hash(
        trajectory,
        &events,
        TrajectoryVisibility::Public,
    );

    println!("{} Publishing {} trajectory events...", "Publishing:".cyan(), events.len());

    // Publish each event to relays
    use nostr_client::{PoolConfig, RelayPool};
    let pool_config = PoolConfig::default();
    let pool = RelayPool::new(pool_config);

    // Connect to relays
    for relay_url in &config.nostr.relays {
        pool.add_relay(relay_url)
            .await
            .with_context(|| format!("Failed to add relay: {}", relay_url))?;
    }

    // Wait for connections
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let mut published_count = 0;

    for (i, trajectory_event) in events.iter().enumerate() {
        // Build Nostr event
        let tags = vec![
            vec!["session_id".to_string(), trajectory_event.session_id.clone()],
            vec!["tick_id".to_string(), trajectory_event.tick_id.clone()],
            vec!["sequence".to_string(), trajectory_event.sequence.to_string()],
        ];

        let content_json = trajectory_event
            .content
            .to_json()
            .with_context(|| format!("Failed to serialize trajectory event {}", i))?;

        let template = nostr::EventTemplate {
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            kind: autopilot::nip_sa_trajectory::TrajectoryPublisher::event_kind(),
            tags,
            content: content_json,
        };

        // Sign event
        let event = identity
            .sign_event(template)
            .with_context(|| format!("Failed to sign trajectory event {}", i))?;

        // Publish to relays
        match pool.publish(&event).await {
            Ok(results) => {
                let success_count = results.iter().filter(|r| r.accepted).count();
                if success_count > 0 {
                    published_count += 1;
                }
            }
            Err(e) => {
                eprintln!("{} Failed to publish event {}: {}", "Warning:".yellow(), i, e);
            }
        }
    }

    // Disconnect from pool
    let _ = pool.disconnect_all().await;

    println!(
        "{} Published {}/{} trajectory events to Nostr relays",
        "✓".green(),
        published_count,
        events.len()
    );

    Ok(())
}

async fn run_task(
    prompt: String,
    project: Option<String>,
    cwd: Option<PathBuf>,
    agent: String,
    model: String,
    max_turns: u32,
    max_budget: f64,
    output_dir: Option<PathBuf>,
    slug: Option<String>,
    dry_run: bool,
    verbose: bool,
    with_issues: bool,
    issues_db: Option<PathBuf>,
    full_auto: bool,
    ui: bool,
    publish_trajectory: bool,
) -> Result<()> {
    // Load project if specified
    let (cwd, issues_db, project_id) = if let Some(project_name) = project {
        use issues::{db, project};

        let default_db = autopilot::default_db_path();
        let conn = db::init_db(&default_db)?;

        match project::get_project_by_name(&conn, &project_name)? {
            Some(proj) => {
                println!("{} Loading project '{}'", "Project:".cyan().bold(), proj.name);
                println!("{} {}", "Path:".dimmed(), proj.path);
                (
                    PathBuf::from(&proj.path),
                    Some(PathBuf::from(&proj.path).join("autopilot.db")),
                    Some(proj.id)
                )
            }
            None => {
                eprintln!("{} Project '{}' not found", "Error:".red(), project_name);
                eprintln!("Run `cargo autopilot project list` to see available projects");
                std::process::exit(1);
            }
        }
    } else {
        (cwd.unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        }), issues_db, None)
    };

    // Create session record if we have a project
    let session_id = if let Some(ref proj_id) = project_id {
        use issues::{db, session};

        let default_db = autopilot::default_db_path();
        let db_path = issues_db.as_ref().unwrap_or(&default_db);
        let conn = db::init_db(db_path)?;

        let pid = std::process::id() as i32;
        let session = session::create_session(&conn, proj_id, &prompt, &model, Some(pid))?;

        println!("{} Session ID: {}", "Session:".dimmed(), &session.id[..8]);
        Some(session.id)
    } else {
        None
    };

    // Check for stale lockfile and handle crash recovery
    check_and_handle_stale_lockfile(&cwd).await?;

    // Launch desktop UI if requested
    let _ui_port: Option<u16> = if ui {
        println!("{} Launching desktop UI...", "UI:".cyan().bold());

        // Spawn desktop app as subprocess
        let mut child = std::process::Command::new("cargo")
            .args(["run", "--release", "-p", "desktop"])
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Wait for server to start by reading stdout for the port
        use std::io::{BufRead, BufReader};
        let port = if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);

            let mut port = None;
            for line in reader.lines().take(20).flatten() {
                // Look for "DESKTOP_PORT=PORT"
                if let Some(rest) = line.strip_prefix("DESKTOP_PORT=") {
                    if let Ok(p) = rest.trim().parse::<u16>() {
                        port = Some(p);
                        break;
                    }
                }
            }
            port
        } else {
            eprintln!("Warning: Failed to get stdout from desktop process");
            None
        };

        if let Some(p) = port {
            println!("{} Desktop running at http://127.0.0.1:{}/autopilot", "UI:".cyan().bold(), p);
            // Open browser
            let _ = std::process::Command::new("open")
                .arg(format!("http://127.0.0.1:{}/autopilot", p))
                .spawn();
            Some(p)
        } else {
            eprintln!("{} Failed to detect desktop UI port", "Warning:".yellow());
            // Kill the child process
            let _ = child.kill();
            None
        }
    } else {
        None
    };

    // Resolve friendly model names to full model IDs
    let model = resolve_model(&model);

    // Get git info
    let repo_sha = get_git_sha(&cwd).unwrap_or_else(|_| "unknown".to_string());
    let branch = get_git_branch(&cwd).ok();

    // Generate slug
    let slug = slug.unwrap_or_else(|| generate_slug(&prompt));

    // Setup output directory
    let output_dir = output_dir.unwrap_or_else(|| PathBuf::from("docs/logs").join(date_dir()));

    // Enhance prompt for full-auto mode
    let prompt = if full_auto {
        let directive_summary = load_directive_summary(&cwd);
        format!("{}{}{}", prompt, directive_summary, FULL_AUTO_PROMPT_BASE)
    } else {
        prompt
    };

    println!("{} {}", "Running:".cyan().bold(), prompt.lines().next().unwrap_or(&prompt));
    println!("{} {}", "Model:".dimmed(), model);
    println!("{} {}", "CWD:".dimmed(), cwd.display());
    if full_auto {
        println!("{} {}", "Mode:".magenta().bold(), "FULL AUTO");
    }
    println!();

    // Create trajectory collector
    let mut collector = TrajectoryCollector::new(
        prompt.clone(),
        model.clone(),
        cwd.display().to_string(),
        repo_sha,
        branch,
    );

    // Enable streaming rlog output (unless in dry-run mode)
    let rlog_path = if !dry_run {
        std::fs::create_dir_all(&output_dir)?;
        let rlog_path = output_dir.join(filename(&slug, "rlog"));
        if let Err(e) = collector.enable_streaming(&rlog_path) {
            eprintln!("Warning: Failed to enable rlog streaming: {}", e);
            None
        } else {
            println!("{} {} {}", "Streaming to:".dimmed(), rlog_path.display(), "(tail -f to watch)".dimmed());
            Some(rlog_path)
        }
    } else {
        None
    };

    // Enable JSONL streaming for full data capture (alongside rlog)
    if !dry_run {
        let jsonl_path = output_dir.join(filename(&slug, "jsonl"));
        if let Err(e) = collector.enable_jsonl_streaming(&jsonl_path) {
            eprintln!("Warning: Failed to enable JSONL streaming: {}", e);
        } else {
            println!("{} {} {}", "Full data:".dimmed(), jsonl_path.display(), "(APM source)".dimmed());
        }
    }

    // Setup query options with hooks
    let plan_mode_hook = std::sync::Arc::new(PlanModeHook);
    let plan_hook_matcher = HookCallbackMatcher::new().hook(plan_mode_hook);

    let compaction_hook = std::sync::Arc::new(CompactionHook);
    let compact_hook_matcher = HookCallbackMatcher::new().hook(compaction_hook);

    let mut hooks = std::collections::HashMap::new();
    hooks.insert(HookEvent::PreToolUse, vec![plan_hook_matcher]);
    hooks.insert(HookEvent::PreCompact, vec![compact_hook_matcher]);

    let mut options = QueryOptions::new()
        .model(&model)
        .max_turns(max_turns)
        .cwd(&cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .hooks(hooks)
        .dangerously_skip_permissions(true);

    // Only set budget constraint if explicitly specified (> 0)
    if max_budget > 0.0 {
        options = options.max_budget_usd(max_budget);
    }

    // Write .mcp.json file for issue tracking MCP server if requested
    if with_issues {
        let mcp_json_path = cwd.join(".mcp.json");
        let default_issues_db = autopilot::default_db_path();
        let db_path = issues_db
            .as_ref()
            .unwrap_or(&default_issues_db)
            .display()
            .to_string();

        println!("{} {}", "Issues DB:".dimmed(), db_path);

        // Build MCP server configuration
        let mcp_config = json!({
            "mcpServers": {
                "issues": {
                    "command": "cargo",
                    "args": ["run", "--release", "-p", "issues-mcp"],
                    "env": {
                        "ISSUES_DB": db_path
                    }
                }
            }
        });

        // Write .mcp.json file
        let json = serde_json::to_string_pretty(&mcp_config)
            .expect("Failed to serialize MCP config to JSON");
        std::fs::write(&mcp_json_path, json)?;
        println!("{} {}", "MCP config:".dimmed(), mcp_json_path.display());

        // Store path for cleanup on panic/signal
        MCP_JSON_PATH.set(mcp_json_path).ok();
    }

    // Write lockfile to track this run (for crash recovery)
    // Note: session_id will be written to the collector later and available in the rlog
    // For now we write basic info, issue_number would need to be passed as a parameter
    if let Err(e) = write_lockfile(None, None, rlog_path.clone()) {
        eprintln!("Warning: Failed to write lockfile: {}", e);
    }

    // Dispatch to appropriate agent
    // In full-auto mode, we loop and force continuation if the agent stops prematurely
    if full_auto && agent == "claude" {
        run_full_auto_loop(
            &prompt,
            options,
            &mut collector,
            verbose,
            _ui_port,
            &cwd,
            issues_db.as_ref(),
        )
        .await?;
    } else {
        match agent.as_str() {
            "claude" => {
                run_claude_agent(
                    &prompt,
                    options,
                    &mut collector,
                    verbose,
                    _ui_port,
                )
                .await?;
            }
            "codex" => {
                run_codex_agent(
                    &prompt,
                    &cwd,
                    max_turns,
                    max_budget,
                    &mut collector,
                    verbose,
                )
                .await?;
            }
            _ => {
                anyhow::bail!("Unknown agent: {}. Use 'claude' or 'codex'", agent);
            }
        }
    }

    let trajectory = collector.finish();

    println!();
    println!("{}", "=".repeat(60).dimmed());
    print_summary(&trajectory);

    // Extract and store metrics
    store_trajectory_metrics(&trajectory);

    // Save outputs
    if !dry_run {
        std::fs::create_dir_all(&output_dir)?;

        // Write .rlog
        let mut rlog_writer = RlogWriter::new();
        let rlog_content = rlog_writer.write(&trajectory);
        let rlog_path = output_dir.join(filename(&slug, "rlog"));
        std::fs::write(&rlog_path, &rlog_content)?;
        println!("{} {}", "Saved:".green(), rlog_path.display());

        // Write .json
        let json_content = trajectory.to_json();
        let json_path = output_dir.join(filename(&slug, "json"));
        std::fs::write(&json_path, &json_content)?;
        println!("{} {}", "Saved:".green(), json_path.display());

        // Print resume hints if session failed or was interrupted
        if let Some(ref result) = trajectory.result {
            let is_budget_error = result.errors.iter().any(|e| e.contains("budget") || e.contains("Budget"));
            let is_max_turns = result.errors.iter().any(|e| e.contains("max_turns") || e.contains("turns"));

            if !result.success && (is_budget_error || is_max_turns || !result.errors.is_empty()) {
                println!();
                println!("{}", "=".repeat(60).yellow());
                println!("{} Session interrupted", "⚠".yellow().bold());

                if is_budget_error {
                    println!("  Reason: Budget exhausted");
                } else if is_max_turns {
                    println!("  Reason: Max turns reached");
                } else if !result.errors.is_empty() {
                    println!("  Reason: {}", result.errors[0]);
                }

                println!();
                println!("{} To resume this session:", "→".cyan());
                println!("  {}", format!("autopilot resume {}", json_path.display()).cyan());
                println!("  or");
                println!("  {}", "autopilot resume --continue-last".cyan());
                println!("{}", "=".repeat(60).yellow());
            }
        }

        // Update session with trajectory path if we have a session
        if let Some(ref sess_id) = session_id {
            use issues::{db, session};
            let default_db = autopilot::default_db_path();
            let db_path = issues_db.as_ref().unwrap_or(&default_db);
            if let Ok(conn) = db::init_db(db_path) {
                let _ = session::update_session_trajectory(&conn, sess_id, &json_path.display().to_string());
            }
        }
    }

    // Update session status on completion
    if let Some(ref sess_id) = session_id {
        use issues::{db, session, SessionStatus};
        let default_db = autopilot::default_db_path();
        let db_path = issues_db.as_ref().unwrap_or(&default_db);
        if let Ok(conn) = db::init_db(db_path) {
            let status = if trajectory.result.as_ref().map(|r| r.success).unwrap_or(false) {
                SessionStatus::Completed
            } else {
                SessionStatus::Failed
            };
            let _ = session::update_session_status(&conn, sess_id, status);
            let issues_completed = trajectory.result.as_ref().map(|r| r.issues_completed as i32).unwrap_or(0);
            let _ = session::update_session_metrics(&conn, sess_id, trajectory.usage.cost_usd, issues_completed);
        }
    }

    // Publish trajectory to Nostr relays if enabled
    if publish_trajectory {
        if let Err(e) = publish_trajectory_to_nostr(&trajectory, session_id.as_ref()).await {
            eprintln!("{} Failed to publish trajectory: {}", "Warning:".yellow(), e);
        }
    }

    // Cleanup .mcp.json and lockfile on normal exit
    cleanup_mcp_json();
    cleanup_lockfile();

    Ok(())
}

fn print_message(msg: &SdkMessage) {
    match msg {
        SdkMessage::Assistant(a) => {
            // Parse content blocks
            if let Some(content) = a.message.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match block_type {
                        "thinking" => {
                            let text = block
                                .get("thinking")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            println!(
                                "{} {}",
                                "THINK".yellow(),
                                truncate(text, 100)
                            );
                        }
                        "text" => {
                            let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            println!("{} {}", "ASST".green(), truncate(text, 100));
                        }
                        "tool_use" => {
                            let tool = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            println!("{} {}", "TOOL".blue(), tool);
                        }
                        _ => {}
                    }
                }
            }
        }
        SdkMessage::User(u) => {
            if let Some(content) = u.message.get("content") {
                match content {
                    serde_json::Value::String(s) => {
                        println!("{} {}", "USER".cyan(), truncate(s, 100));
                    }
                    serde_json::Value::Array(arr) => {
                        for block in arr {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                let tool_id = block
                                    .get("tool_use_id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("");
                                let is_error = block
                                    .get("is_error")
                                    .and_then(|e| e.as_bool())
                                    .unwrap_or(false);
                                let status = if is_error { "ERROR" } else { "OK" };
                                println!(
                                    "{} {} [{}]",
                                    "RSLT".magenta(),
                                    &tool_id[..tool_id.len().min(8)],
                                    status
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        SdkMessage::System(s) => println!("{} {:?}", "SYS ".yellow(), s),
        SdkMessage::Result(r) => println!("{} {:?}", "DONE".cyan().bold(), r),
        SdkMessage::ToolProgress(p) => {
            println!(
                "{} {} ({:.1}s)",
                "PROG".dimmed(),
                p.tool_name,
                p.elapsed_time_seconds
            );
        }
        _ => {}
    }
}

fn print_progress(msg: &SdkMessage) {
    match msg {
        SdkMessage::ToolProgress(p) => {
            println!(
                "  {} ({:.1}s)",
                "working...".yellow().dimmed(),
                p.elapsed_time_seconds
            );
        }
        SdkMessage::Result(_) => {
            println!("{}", "@end".green().bold());
        }
        SdkMessage::Assistant(a) => {
            if let Some(content) = a.message.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match block_type {
                        "text" => {
                            // Show what the agent is saying (recorder-style)
                            let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            let first_line = text.lines().next().unwrap_or("");
                            let truncated = if first_line.len() > 100 {
                                format!("{}...", &first_line[..97])
                            } else {
                                first_line.to_string()
                            };
                            if !truncated.is_empty() {
                                println!("{} {}", "a:".green(), truncated.dimmed());
                            }
                        }
                        "thinking" => {
                            // Show thinking (recorder-style)
                            let text = block.get("thinking").and_then(|t| t.as_str()).unwrap_or("");
                            let first_line = text.lines().next().unwrap_or("");
                            let truncated = if first_line.len() > 80 {
                                format!("{}...", &first_line[..77])
                            } else {
                                first_line.to_string()
                            };
                            if !truncated.is_empty() {
                                println!("{} {}", "th:".yellow(), truncated.dimmed());
                            }
                        }
                        "tool_use" => {
                            let tool = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let tool_id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                            let input = block.get("input");

                            // Format tool args (same as rlog)
                            let args = match tool {
                                "Bash" => input
                                    .and_then(|i| i.get("command"))
                                    .and_then(|c| c.as_str())
                                    .map(|c| {
                                        let truncated = if c.len() > 50 { format!("{}...", &c[..47]) } else { c.to_string() };
                                        format!("cmd=\"{}\"", truncated)
                                    })
                                    .unwrap_or_default(),
                                "Read" | "Write" | "Edit" => input
                                    .and_then(|i| i.get("file_path"))
                                    .and_then(|p| p.as_str())
                                    .map(|p| format!("file_path={}", p))
                                    .unwrap_or_default(),
                                "Glob" => input
                                    .and_then(|i| i.get("pattern"))
                                    .and_then(|p| p.as_str())
                                    .map(|p| format!("pattern=\"{}\"", p))
                                    .unwrap_or_default(),
                                "Grep" => input
                                    .and_then(|i| i.get("pattern"))
                                    .and_then(|p| p.as_str())
                                    .map(|p| {
                                        let truncated = if p.len() > 30 { format!("{}...", &p[..27]) } else { p.to_string() };
                                        format!("pattern=\"{}\"", truncated)
                                    })
                                    .unwrap_or_default(),
                                "Task" => input
                                    .and_then(|i| i.get("description"))
                                    .and_then(|d| d.as_str())
                                    .map(|d| {
                                        let truncated = if d.len() > 40 { format!("{}...", &d[..37]) } else { d.to_string() };
                                        format!("desc=\"{}\"", truncated)
                                    })
                                    .unwrap_or_default(),
                                _ => String::new(),
                            };

                            // Get short tool ID (last 8 chars)
                            let id_short = if tool_id.len() > 8 {
                                &tool_id[tool_id.len() - 8..]
                            } else {
                                tool_id
                            };

                            // Print in recorder style: t!:ToolName id=xxx args → [running]
                            let args_str = if args.is_empty() {
                                String::new()
                            } else {
                                format!(" {}", args)
                            };
                            println!(
                                "{} {} {}{} {}",
                                "t!:".blue().bold(),
                                tool.cyan(),
                                format!("id={}", id_short).dimmed(),
                                args_str.dimmed(),
                                "→ [running]".yellow()
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        SdkMessage::User(u) => {
            // Show tool results in recorder style
            if let Some(content) = u.message.get("content") {
                if let serde_json::Value::Array(arr) = content {
                    for block in arr {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                            let tool_id = block
                                .get("tool_use_id")
                                .and_then(|i| i.as_str())
                                .unwrap_or("");
                            let is_error = block
                                .get("is_error")
                                .and_then(|e| e.as_bool())
                                .unwrap_or(false);

                            // Get short tool ID (last 8 chars)
                            let id_short = if tool_id.len() > 8 {
                                &tool_id[tool_id.len() - 8..]
                            } else {
                                tool_id
                            };

                            // Get output content
                            let output = block
                                .get("content")
                                .and_then(|c| {
                                    if let Some(s) = c.as_str() {
                                        Some(s.to_string())
                                    } else if let Some(arr) = c.as_array() {
                                        arr.first()
                                            .and_then(|b| b.get("text"))
                                            .and_then(|t| t.as_str())
                                            .map(|s| s.to_string())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_default();

                            let first_line = output.lines().next().unwrap_or("");
                            let truncated = if first_line.len() > 60 {
                                format!("{}...", &first_line[..57])
                            } else {
                                first_line.to_string()
                            };

                            // Print in recorder style: o: id=xxx → [ok]/[error] output
                            let status = if is_error {
                                "[error]".red()
                            } else {
                                "[ok]".green()
                            };

                            let output_str = if truncated.is_empty() {
                                String::new()
                            } else {
                                format!(" {}", truncated.dimmed())
                            };

                            println!(
                                "{} {} {} {}{}",
                                "o:".magenta(),
                                format!("id={}", id_short).dimmed(),
                                "→".dimmed(),
                                status,
                                output_str
                            );
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn print_summary(traj: &Trajectory) {
    use autopilot::apm::APMTier;

    println!("{}", "Summary".cyan().bold());
    println!("  Session:  {}", traj.session_id);
    println!(
        "  Tokens:   {} in / {} out",
        traj.usage.input_tokens, traj.usage.output_tokens
    );
    println!("  Cached:   {}", traj.usage.cache_read_tokens);
    println!("  Cost:     ${:.4}", traj.usage.cost_usd);

    if let Some(ref result) = traj.result {
        println!("  Duration: {}ms", result.duration_ms);
        println!("  Turns:    {}", result.num_turns);
        println!(
            "  Success:  {}",
            if result.success {
                "yes".green()
            } else {
                "no".red()
            }
        );

        // Display APM if available
        if let Some(apm) = result.apm {
            let tier = APMTier::from_apm(apm);
            let colored_apm = match tier {
                APMTier::Elite => format!("{:.1}", apm).yellow().bold(),
                APMTier::HighPerformance => format!("{:.1}", apm).green().bold(),
                APMTier::Productive => format!("{:.1}", apm).green(),
                APMTier::Active => format!("{:.1}", apm).blue(),
                APMTier::Baseline => format!("{:.1}", apm).dimmed(),
            };
            println!("  APM:      {} ({})", colored_apm, tier.name().dimmed());
        }
    }

    // Count steps by type
    let mut tool_calls = 0;
    let mut thinking = 0;
    let mut assistant = 0;
    for step in &traj.steps {
        match &step.step_type {
            StepType::ToolCall { .. } => tool_calls += 1,
            StepType::Thinking { .. } => thinking += 1,
            StepType::Assistant { .. } => assistant += 1,
            _ => {}
        }
    }
    println!("  Steps:    {} total", traj.steps.len());
    println!(
        "            {} tool calls, {} thinking, {} responses",
        tool_calls, thinking, assistant
    );
}

/// Store trajectory metrics in the metrics database
fn store_trajectory_metrics(trajectory: &Trajectory) {
    use autopilot::metrics::{extract_metrics_from_trajectory, MetricsDb, default_db_path};

    match extract_metrics_from_trajectory(trajectory) {
        Ok((session_metrics, tool_call_metrics)) => {
            match MetricsDb::open(default_db_path()) {
                Ok(db) => {
                    // Store session metrics
                    if let Err(e) = db.store_session(&session_metrics) {
                        eprintln!("Warning: Failed to store session metrics: {}", e);
                        return;
                    }

                    // Store tool call metrics
                    let mut stored = 0;
                    let mut errors = 0;
                    for tool_call in &tool_call_metrics {
                        match db.store_tool_call(tool_call) {
                            Ok(_) => stored += 1,
                            Err(e) => {
                                eprintln!("Warning: Failed to store tool call: {}", e);
                                errors += 1;
                            }
                        }
                    }

                    println!(
                        "{}",
                        format!(
                            "✓ Stored metrics: {} tool calls ({} errors)",
                            stored, errors
                        )
                        .green()
                    );

                    // Detect and report anomalies (PostRun hook behavior)
                    match db.detect_anomalies(&session_metrics) {
                        Ok(anomalies) => {
                            if !anomalies.is_empty() {
                                println!("\n{}", "⚠ Anomalies Detected:".yellow().bold());
                                for anomaly in &anomalies {
                                    let severity_str = match anomaly.severity {
                                        autopilot::metrics::AnomalySeverity::Critical => "CRITICAL".red().bold(),
                                        autopilot::metrics::AnomalySeverity::Error => "ERROR".red(),
                                        autopilot::metrics::AnomalySeverity::Warning => "WARNING".yellow(),
                                    };
                                    println!(
                                        "  [{}] {}: expected {:.3}, got {:.3}",
                                        severity_str,
                                        anomaly.dimension,
                                        anomaly.expected_value,
                                        anomaly.actual_value
                                    );
                                }

                                // Store anomalies in database
                                for anomaly in &anomalies {
                                    if let Err(e) = db.store_anomaly(anomaly) {
                                        eprintln!("Warning: Failed to store anomaly: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to detect anomalies: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to open metrics database: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to extract metrics from trajectory: {}", e);
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    if first_line.chars().count() <= max {
        first_line.to_string()
    } else {
        format!(
            "{}...",
            first_line.chars().take(max - 3).collect::<String>()
        )
    }
}

fn get_git_sha(cwd: &PathBuf) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(cwd)
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_git_branch(cwd: &PathBuf) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Stream HTML fragment to desktop app /events endpoint
async fn stream_to_desktop(port: u16, html: String) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/events", port);

    let _ = client
        .post(&url)
        .header("Content-Type", "text/html")
        .body(html)
        .send()
        .await;

    Ok(())
}

/// Resume a previous autopilot session
async fn resume_task(
    trajectory: Option<PathBuf>,
    continue_last: bool,
    cwd: Option<PathBuf>,
    prompt: Option<String>,
    max_budget: f64,
    with_issues: bool,
    issues_db: Option<PathBuf>,
) -> Result<()> {
    let cwd = cwd.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });

    // Track original trajectory path for appending logs
    let original_trajectory_path = trajectory.clone();

    // Get session_id from trajectory file or use --continue
    let session_id = if continue_last {
        println!("{} Continuing most recent session...", "Resume:".cyan().bold());
        None
    } else {
        let path = trajectory.ok_or_else(|| anyhow::anyhow!("trajectory path required for resume"))?;
        println!("{} Loading session from {:?}", "Resume:".cyan().bold(), path);

        let id = if path.extension().and_then(|e| e.to_str()) == Some("json") {
            extract_session_id_from_json(&path)?
        } else {
            // Try rlog, fall back to error
            extract_session_id_from_rlog(&path)?.ok_or_else(|| {
                anyhow::anyhow!(
                    "No session_id in rlog header. Use --continue-last to resume most recent session."
                )
            })?
        };

        println!("{} session_id={}", "Resume:".dimmed(), &id[..id.len().min(8)]);
        Some(id)
    };

    // Build QueryOptions with resume
    let mut options = QueryOptions::new()
        .cwd(&cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .dangerously_skip_permissions(true);

    // Only set budget constraint if explicitly specified (> 0)
    if max_budget > 0.0 {
        options = options.max_budget_usd(max_budget);
    }

    if let Some(ref id) = session_id {
        options.resume = Some(id.clone());
    } else {
        options.continue_session = true;
    }

    // Setup MCP for issue tracking if requested
    if with_issues {
        let mcp_json_path = cwd.join(".mcp.json");
        let db_path = issues_db
            .unwrap_or_else(|| autopilot::default_db_path())
            .display()
            .to_string();

        let mcp_config = json!({
            "mcpServers": {
                "issues": {
                    "command": "cargo",
                    "args": ["run", "--release", "-p", "issues-mcp"],
                    "env": {
                        "ISSUES_DB": db_path
                    }
                }
            }
        });

        let json = serde_json::to_string_pretty(&mcp_config)
            .expect("Failed to serialize MCP config to JSON");
        std::fs::write(&mcp_json_path, json)?;
        MCP_JSON_PATH.set(mcp_json_path).ok();
    }

    // Determine output paths - append to original files if resuming from a file
    let (rlog_path, json_path, jsonl_path) = if let Some(ref orig_path) = original_trajectory_path {
        // Use same directory and derive paths from original
        let parent = orig_path.parent().unwrap_or(std::path::Path::new("."));
        let stem = orig_path.file_stem().and_then(|s| s.to_str()).unwrap_or("resumed");
        (
            parent.join(format!("{}.rlog", stem)),
            parent.join(format!("{}.json", stem)),
            parent.join(format!("{}.jsonl", stem)),
        )
    } else {
        // Create new files in standard location for --continue-last
        let output_dir = PathBuf::from("docs/logs").join(date_dir());
        std::fs::create_dir_all(&output_dir)?;
        let slug = format!("resumed-{}", chrono::Utc::now().format("%H%M"));
        (
            output_dir.join(filename(&slug, "rlog")),
            output_dir.join(filename(&slug, "json")),
            output_dir.join(filename(&slug, "jsonl")),
        )
    };

    // Get git info
    let repo_sha = get_git_sha(&cwd).unwrap_or_else(|_| "unknown".to_string());
    let branch = get_git_branch(&cwd).ok();

    // Create trajectory collector for the resumed session
    let resume_prompt = prompt.clone().unwrap_or_else(|| "Continue from where you left off.".to_string());
    let mut collector = TrajectoryCollector::new(
        format!("[RESUMED] {}", resume_prompt),
        "resumed".to_string(), // model not known in resume
        cwd.display().to_string(),
        repo_sha,
        branch,
    );

    // Set session_id if we have it
    if let Some(ref id) = session_id {
        collector.set_session_id(id.clone());
    }

    // Enable streaming rlog output
    if let Err(e) = collector.enable_streaming(&rlog_path) {
        eprintln!("Warning: Failed to enable rlog streaming: {}", e);
    } else {
        println!("{} {} {}", "Streaming to:".dimmed(), rlog_path.display(), "(tail -f to watch)".dimmed());
    }

    // Enable JSONL streaming for full data capture
    if let Err(e) = collector.enable_jsonl_streaming(&jsonl_path) {
        eprintln!("Warning: Failed to enable JSONL streaming: {}", e);
    } else {
        println!("{} {} {}", "Full data:".dimmed(), jsonl_path.display(), "(APM source)".dimmed());
    }

    // Create session
    let mut session = unstable_v2_create_session(options).await?;

    // Send prompt if provided
    if let Some(p) = prompt {
        println!("{} Sending: {}", "Resume:".cyan().bold(), p);
        session.send(&p).await?;
    } else {
        // Send a continue message
        session.send("Continue from where you left off.").await?;
    }

    println!("{} Resumed, streaming...", "Resume:".green().bold());
    println!();

    // Process messages - collect trajectory AND print progress
    while let Some(msg) = session.receive().next().await {
        let msg = msg?;
        collector.process_message(&msg);
        print_progress(&msg);
    }

    let trajectory = collector.finish();

    println!();
    println!("{}", "=".repeat(60).dimmed());
    print_summary(&trajectory);

    // Extract and store metrics
    store_trajectory_metrics(&trajectory);

    // Save outputs
    // Write .rlog
    let mut rlog_writer = RlogWriter::new();
    let rlog_content = rlog_writer.write(&trajectory);
    std::fs::write(&rlog_path, &rlog_content)?;
    println!("{} {}", "Saved:".green(), rlog_path.display());

    // Write .json
    let json_content = trajectory.to_json();
    std::fs::write(&json_path, &json_content)?;
    println!("{} {}", "Saved:".green(), json_path.display());

    // Cleanup
    cleanup_mcp_json();
    cleanup_lockfile();

    println!();
    println!("{}", "Session ended.".green());

    Ok(())
}

async fn replay_trajectory(trajectory_path: PathBuf, mode: String) -> Result<()> {
    // Load trajectory
    let trajectory = replay::load_trajectory(&trajectory_path)?;

    // Run appropriate viewer based on mode
    match mode.as_str() {
        "interactive" | "i" => replay::interactive_replay(&trajectory)?,
        "list" | "l" => replay::list_steps(&trajectory)?,
        "summary" | "s" => replay::summary_view(&trajectory)?,
        _ => {
            eprintln!("Unknown mode: {}. Use interactive, list, or summary.", mode);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn compare_trajectories(trajectory1: PathBuf, trajectory2: PathBuf) -> Result<()> {
    replay::compare_trajectories(&trajectory1, &trajectory2)?;
    Ok(())
}

async fn analyze_trajectories(path: PathBuf, aggregate: bool, json_output: bool) -> Result<()> {
    if aggregate || path.is_dir() {
        // Aggregate mode: analyze all JSON files in directory
        let dir = if path.is_dir() {
            path
        } else {
            path.parent()
                .expect("Path should have a parent directory")
                .to_path_buf()
        };

        let mut analyses = Vec::new();

        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match analyze::load_trajectory(&path) {
                    Ok(trajectory) => {
                        analyses.push(analyze::analyze_trajectory(&trajectory));
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load {}: {}", path.display(), e);
                    }
                }
            }
        }

        if analyses.is_empty() {
            println!("No trajectory files found in {}", dir.display());
            return Ok(());
        }

        let aggregate_analysis = analyze::aggregate_analyses(&analyses);

        if json_output {
            println!("{}", serde_json::to_string_pretty(&aggregate_analysis)?);
        } else {
            analyze::print_aggregate(&aggregate_analysis);
        }
    } else {
        // Single file mode
        let trajectory = analyze::load_trajectory(&path)?;
        let analysis = analyze::analyze_trajectory(&trajectory);

        if json_output {
            println!("{}", serde_json::to_string_pretty(&analysis)?);
        } else {
            analyze::print_analysis(&analysis);
        }
    }

    Ok(())
}

async fn handle_session_command(command: SessionCommands) -> Result<()> {
    use issues::{db, project, session};

    let default_db = autopilot::default_db_path();

    match command {
        SessionCommands::List { project: proj_name, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Get project_id if project name is provided
            let project_id = if let Some(ref name) = proj_name {
                match project::get_project_by_name(&conn, name)? {
                    Some(p) => Some(p.id),
                    None => {
                        eprintln!("{} Project '{}' not found", "Error:".red(), name);
                        std::process::exit(1);
                    }
                }
            } else {
                None
            };

            let sessions = session::list_sessions(&conn, project_id.as_deref())?;

            if sessions.is_empty() {
                println!("No sessions found");
            } else {
                println!("{:<10} {:<10} {:<40} {:<10} {:<8}", "ID", "Status", "Prompt", "Budget", "Issues");
                println!("{}", "-".repeat(85));
                for s in sessions {
                    let id_short = if s.id.len() > 8 { &s.id[..8] } else { &s.id };
                    let prompt_short = if s.prompt.len() > 38 {
                        format!("{}...", &s.prompt[..35])
                    } else {
                        s.prompt.clone()
                    };
                    println!(
                        "{:<10} {:<10} {:<40} ${:<9.2} {}",
                        id_short,
                        s.status.as_str(),
                        prompt_short,
                        s.budget_spent,
                        s.issues_completed
                    );
                }
            }
        }
        SessionCommands::Show { id, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Try to find session by ID or prefix
            let sessions = session::list_sessions(&conn, None)?;
            let matching: Vec<_> = sessions.iter().filter(|s| s.id.starts_with(&id)).collect();

            match matching.len() {
                0 => {
                    eprintln!("{} Session '{}' not found", "Error:".red(), id);
                    std::process::exit(1);
                }
                1 => {
                    let s = matching[0];
                    println!("{} Session {}", "→".cyan(), &s.id[..8]);
                    println!("  Status:     {}", s.status.as_str());
                    println!("  Prompt:     {}", s.prompt);
                    println!("  Model:      {}", s.model);
                    if let Some(pid) = s.pid {
                        println!("  PID:        {}", pid);
                    }
                    println!("  Started:    {}", s.started_at);
                    if let Some(ref ended) = s.ended_at {
                        println!("  Ended:      {}", ended);
                    }
                    println!("  Budget:     ${:.4}", s.budget_spent);
                    println!("  Issues:     {}", s.issues_completed);
                    if let Some(ref path) = s.trajectory_path {
                        println!("  Trajectory: {}", path);
                    }
                }
                _ => {
                    eprintln!("{} Multiple sessions match '{}'. Please be more specific:", "Error:".yellow(), id);
                    for s in matching {
                        eprintln!("  {}", &s.id[..16]);
                    }
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

async fn handle_project_command(command: ProjectCommands) -> Result<()> {
    use issues::{db, project, session};

    let default_db = autopilot::default_db_path();

    match command {
        ProjectCommands::Add {
            name,
            path,
            description,
            model,
            budget,
            db,
        } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Validate path exists
            if !path.exists() {
                eprintln!("{} Path does not exist: {}", "Error:".red(), path.display());
                std::process::exit(1);
            }

            let created = project::create_project(
                &conn,
                &name,
                &path.display().to_string(),
                description.as_deref(),
                model.as_deref(),
                budget,
            )?;

            println!(
                "{} Created project '{}'",
                "✓".green(),
                created.name
            );
            println!("  Path:   {}", created.path);
            if let Some(ref desc) = created.description {
                println!("  Desc:   {}", desc);
            }
            if let Some(ref m) = created.default_model {
                println!("  Model:  {}", m);
            }
            if let Some(b) = created.default_budget {
                println!("  Budget: ${}", b);
            }
        }
        ProjectCommands::List { db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let projects = project::list_projects(&conn)?;

            if projects.is_empty() {
                println!("No projects found");
                println!("\nCreate a project with:");
                println!("  cargo autopilot project add <name> --path <directory>");
            } else {
                println!("{:<20} {:<40} {:<10}", "Name", "Path", "Sessions");
                println!("{}", "-".repeat(75));
                for p in projects {
                    // Count sessions for this project
                    let sessions = session::list_sessions(&conn, Some(&p.id))?;
                    let session_count = sessions.len();

                    println!(
                        "{:<20} {:<40} {}",
                        p.name,
                        p.path,
                        session_count
                    );
                }
            }
        }
        ProjectCommands::Remove { name, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            if let Some(p) = project::get_project_by_name(&conn, &name)? {
                if project::delete_project(&conn, &p.id)? {
                    println!("{} Removed project '{}'", "✓".green(), name);
                    println!("  Note: Project files remain at {}", p.path);
                } else {
                    eprintln!("{} Could not remove project '{}'", "✗".red(), name);
                }
            } else {
                eprintln!("{} Project '{}' not found", "✗".red(), name);
            }
        }
    }

    Ok(())
}

async fn handle_issue_command(command: IssueCommands) -> Result<()> {
    use issues::{db, issue, IssueType, Priority, Status};

    let default_db = autopilot::default_db_path();

    match command {
        IssueCommands::List { status, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let status_filter = status.as_deref().map(|s| match s {
                "open" => Status::Open,
                "in_progress" => Status::InProgress,
                "done" => Status::Done,
                _ => Status::Open,
            });

            let issues = issue::list_issues(&conn, status_filter)?;

            if issues.is_empty() {
                println!("No issues found");
            } else {
                println!("{:<6} {:<10} {:<8} {:<8} {:<50}", "Number", "Status", "Priority", "Agent", "Title");
                println!("{}", "-".repeat(90));
                for i in issues {
                    let status_str = i.status.as_str();
                    let blocked = if i.is_blocked { " [BLOCKED]" } else { "" };
                    println!(
                        "{:<6} {:<10} {:<8} {:<8} {}{}",
                        i.number,
                        status_str,
                        i.priority.as_str(),
                        i.agent,
                        i.title,
                        blocked
                    );
                }
            }
        }
        IssueCommands::Create {
            title,
            description,
            priority,
            issue_type,
            agent,
            directive,
            db,
        } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let priority = Priority::from_str(&priority);
            let issue_type = IssueType::from_str(&issue_type);

            let created = issue::create_issue(&conn, &title, description.as_deref(), priority, issue_type, Some(&agent), directive.as_deref())?;

            println!(
                "{} Created issue #{}: {} (agent: {})",
                "✓".green(),
                created.number,
                created.title,
                created.agent
            );
        }
        IssueCommands::Claim { number, run_id, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let run_id = run_id.unwrap_or_else(|| {
                format!("manual-{}", chrono::Utc::now().timestamp())
            });

            if let Some(i) = issue::get_issue_by_number(&conn, number)? {
                if issue::claim_issue(&conn, &i.id, &run_id)? {
                    println!("{} Claimed issue #{}: {}", "✓".green(), number, i.title);
                } else {
                    println!(
                        "{} Could not claim issue #{} (already claimed or blocked)",
                        "✗".red(),
                        number
                    );
                }
            } else {
                println!("{} Issue #{} not found", "✗".red(), number);
            }
        }
        IssueCommands::Complete { number, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            if let Some(i) = issue::get_issue_by_number(&conn, number)? {
                if issue::complete_issue(&conn, &i.id)? {
                    println!("{} Completed issue #{}: {}", "✓".green(), number, i.title);
                } else {
                    println!("{} Could not complete issue #{}", "✗".red(), number);
                }
            } else {
                println!("{} Issue #{} not found", "✗".red(), number);
            }
        }
        IssueCommands::Block { number, reason, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            if let Some(i) = issue::get_issue_by_number(&conn, number)? {
                if issue::block_issue(&conn, &i.id, &reason)? {
                    println!("{} Blocked issue #{}: {}", "✓".green(), number, reason);
                } else {
                    println!("{} Could not block issue #{}", "✗".red(), number);
                }
            } else {
                println!("{} Issue #{} not found", "✗".red(), number);
            }
        }
        IssueCommands::Ready { agent, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            match issue::get_next_ready_issue(&conn, agent.as_deref())? {
                Some(i) => {
                    println!("{} Next ready issue:", "→".cyan());
                    println!("  Number:   #{}", i.number);
                    println!("  Title:    {}", i.title);
                    println!("  Priority: {}", i.priority.as_str());
                    println!("  Type:     {}", i.issue_type.as_str());
                    println!("  Agent:    {}", i.agent);
                    if let Some(ref desc) = i.description {
                        println!("  Description:");
                        for line in desc.lines() {
                            println!("    {}", line);
                        }
                    }
                }
                None => {
                    println!("No ready issues available");
                }
            }
        }
        IssueCommands::Export { output, include_completed, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Get all issues (filtering will be done during export)
            let issues = issue::list_issues(&conn, None)?;

            // Filter out completed issues if not requested
            let issues_to_export: Vec<_> = if include_completed {
                issues
            } else {
                issues.into_iter().filter(|i| i.status != Status::Done).collect()
            };

            // Serialize to JSON
            let json = serde_json::to_string_pretty(&issues_to_export)?;

            // Determine output path
            let output_path = output.unwrap_or_else(|| {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                cwd.join(".openagents").join("issues.json")
            });

            // Ensure parent directory exists
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Write JSON file
            std::fs::write(&output_path, json)?;

            println!("{} Exported {} issues to {}",
                "✓".green(),
                issues_to_export.len(),
                output_path.display()
            );
        }
        IssueCommands::Import { input, force, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Determine input path
            let input_path = input.unwrap_or_else(|| {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                cwd.join(".openagents").join("issues.json")
            });

            // Check if file exists
            if !input_path.exists() {
                eprintln!("{} File not found: {}", "✗".red(), input_path.display());
                std::process::exit(1);
            }

            // Read and parse JSON
            let json = std::fs::read_to_string(&input_path)?;
            let imported_issues: Vec<issue::Issue> = serde_json::from_str(&json)?;

            let mut imported = 0;
            let mut skipped = 0;
            let mut updated = 0;

            for imported_issue in imported_issues {
                // Check if issue with same UUID already exists
                if let Some(_existing) = issue::get_issue_by_id(&conn, &imported_issue.id)? {
                    if force {
                        // Update existing issue
                        issue::update_issue(
                            &conn,
                            &imported_issue.id,
                            Some(&imported_issue.title),
                            imported_issue.description.as_deref(),
                            Some(imported_issue.priority),
                            Some(imported_issue.issue_type),
                        )?;
                        updated += 1;
                    } else {
                        // Skip - UUID already exists
                        skipped += 1;
                    }
                } else {
                    // Insert new issue - need to preserve all fields including number
                    // We need to use raw SQL since create_issue() generates new UUIDs and numbers
                    let now = chrono::Utc::now().to_rfc3339();
                    let sql = r#"
                        INSERT INTO issues (
                            id, number, title, description, status, priority, issue_type, agent,
                            is_blocked, blocked_reason, claimed_by, claimed_at,
                            created_at, updated_at, completed_at
                        )
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                    "#;

                    // Build params vec to work with execute() method on Connection
                    conn.execute(
                        sql,
                        &[
                            &imported_issue.id as &str,
                            &imported_issue.number.to_string() as &str,
                            &imported_issue.title as &str,
                            &imported_issue.description.as_deref().unwrap_or("") as &str,
                            imported_issue.status.as_str(),
                            imported_issue.priority.as_str(),
                            imported_issue.issue_type.as_str(),
                            &imported_issue.agent as &str,
                            &if imported_issue.is_blocked { "1" } else { "0" },
                            &imported_issue.blocked_reason.as_deref().unwrap_or("") as &str,
                            &imported_issue.claimed_by.as_deref().unwrap_or("") as &str,
                            &imported_issue.claimed_at.map(|dt| dt.to_rfc3339()).unwrap_or_default() as &str,
                            &imported_issue.created_at.to_rfc3339() as &str,
                            &now as &str,
                            &imported_issue.completed_at.map(|dt| dt.to_rfc3339()).unwrap_or_default() as &str,
                        ],
                    ).map_err(|e| anyhow::anyhow!("Failed to insert issue: {}", e))?;
                    imported += 1;

                    // Update issue counter if needed
                    let current_counter: i32 = conn.query_row(
                        "SELECT next_number FROM issue_counter WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )?;
                    if imported_issue.number >= current_counter {
                        conn.execute(
                            "UPDATE issue_counter SET next_number = ? WHERE id = 1",
                            [imported_issue.number + 1],
                        )?;
                    }
                }
            }

            println!("{} Import complete:", "✓".green());
            println!("  Imported: {}", imported);
            if updated > 0 {
                println!("  Updated:  {}", updated);
            }
            if skipped > 0 {
                println!("  Skipped:  {} (use --force to update)", skipped);
            }
        }
    }

    Ok(())
}

async fn handle_directive_command(command: DirectiveCommands) -> Result<()> {
    use issues::{db, directive, DirectiveStatus};

    let default_db = autopilot::default_db_path();
    let directives_dir = std::env::current_dir()?.join(".openagents/directives");

    match command {
        DirectiveCommands::List { status, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let all_directives = directive::load_directives(&directives_dir)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            // Filter by status if specified
            let directives: Vec<_> = if let Some(ref status_str) = status {
                let filter_status = DirectiveStatus::from_str(status_str);
                all_directives.into_iter().filter(|d| d.status == filter_status).collect()
            } else {
                all_directives
            };

            if directives.is_empty() {
                if directives_dir.exists() {
                    println!("No directives found");
                } else {
                    println!("No directives directory found. Create {} to get started.", directives_dir.display());
                }
            } else {
                println!("{:<12} {:<8} {:<10} {:<40} {}", "ID", "Status", "Progress", "Title", "Priority");
                println!("{}", "-".repeat(85));
                for d in directives {
                    let progress = directive::calculate_progress(&conn, &d.id);
                    let progress_str = if progress.total_issues > 0 {
                        format!("{}/{} ({}%)", progress.completed_issues, progress.total_issues, progress.percentage())
                    } else {
                        "0/0".to_string()
                    };
                    let title_short = if d.title.len() > 38 {
                        format!("{}...", &d.title[..35])
                    } else {
                        d.title.clone()
                    };
                    println!(
                        "{:<12} {:<8} {:<10} {:<40} {}",
                        d.id,
                        d.status.as_str(),
                        progress_str,
                        title_short,
                        d.priority.as_str()
                    );
                }
            }
        }
        DirectiveCommands::Show { id, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            match directive::get_directive_by_id(&directives_dir, &id)
                .map_err(|e| anyhow::anyhow!("{}", e))?
            {
                Some(d) => {
                    let progress = directive::calculate_progress(&conn, &d.id);
                    let linked_issues = directive::list_issues_by_directive(&conn, &d.id)?;

                    println!("{} Directive {}", "→".cyan(), d.id);
                    println!("  Title:    {}", d.title);
                    println!("  Status:   {}", d.status.as_str());
                    println!("  Priority: {}", d.priority.as_str());
                    println!("  Created:  {}", d.created);
                    println!("  Updated:  {}", d.updated);
                    println!();
                    println!("{} Progress: {}/{} issues ({}%)", "→".cyan(), progress.completed_issues, progress.total_issues, progress.percentage());
                    if progress.in_progress_issues > 0 {
                        println!("  In progress: {}", progress.in_progress_issues);
                    }
                    if progress.blocked_issues > 0 {
                        println!("  Blocked: {}", progress.blocked_issues);
                    }
                    println!();
                    println!("{} Body:", "→".cyan());
                    for line in d.body.lines() {
                        println!("  {}", line);
                    }
                    if !linked_issues.is_empty() {
                        println!();
                        println!("{} Linked Issues:", "→".cyan());
                        for i in linked_issues {
                            let status_icon = match i.status.as_str() {
                                "done" => "✓".green(),
                                "in_progress" => "●".yellow(),
                                _ => "○".white(),
                            };
                            println!("  {} #{} - {} [{}]", status_icon, i.number, i.title, i.status.as_str());
                        }
                    }
                }
                None => {
                    eprintln!("{} Directive '{}' not found", "Error:".red(), id);
                    std::process::exit(1);
                }
            }
        }
        DirectiveCommands::Create { id, title, priority } => {
            let priority = match priority.as_str() {
                "urgent" => directive::DirectivePriority::Urgent,
                "high" => directive::DirectivePriority::High,
                "low" => directive::DirectivePriority::Low,
                _ => directive::DirectivePriority::Medium,
            };

            let body = "## Goal\n\nDescribe the goal here.\n\n## Success Criteria\n\n- [ ] Criterion 1\n- [ ] Criterion 2";

            let d = issues::Directive::create(&directives_dir, &id, &title, priority, body)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("{} Created directive '{}'", "✓".green(), d.id);
            println!("  Title: {}", d.title);
            println!("  File:  {:?}", d.file_path);
            println!();
            println!("Edit the file to add your goal and success criteria.");
        }
        DirectiveCommands::Pause { id } => {
            let mut d = directive::get_directive_by_id(&directives_dir, &id)
                .map_err(|e| anyhow::anyhow!("{}", e))?
                .ok_or_else(|| anyhow::anyhow!("Directive '{}' not found", id))?;

            d.set_status(DirectiveStatus::Paused)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("{} Paused directive '{}'", "⏸".yellow(), d.id);
        }
        DirectiveCommands::Complete { id } => {
            let mut d = directive::get_directive_by_id(&directives_dir, &id)
                .map_err(|e| anyhow::anyhow!("{}", e))?
                .ok_or_else(|| anyhow::anyhow!("Directive '{}' not found", id))?;

            d.set_status(DirectiveStatus::Completed)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("{} Completed directive '{}'", "✓".green(), d.id);
        }
        DirectiveCommands::Resume { id } => {
            let mut d = directive::get_directive_by_id(&directives_dir, &id)
                .map_err(|e| anyhow::anyhow!("{}", e))?
                .ok_or_else(|| anyhow::anyhow!("Directive '{}' not found", id))?;

            d.set_status(DirectiveStatus::Active)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("{} Resumed directive '{}' (now active)", "▶".green(), d.id);
        }
    }

    Ok(())
}

/// Handle metrics commands
async fn handle_metrics_command(command: MetricsCommands) -> Result<()> {
    use autopilot::metrics::{extract_metrics_from_json_file, MetricsDb, default_db_path};

    match command {
        MetricsCommands::Import { log_dir, db } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            println!("{} Opening metrics database: {:?}", "📊".cyan(), db_path);

            // Find all .json files in the directory
            let json_files: Vec<_> = std::fs::read_dir(&log_dir)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.path().extension().and_then(|s| s.to_str()) == Some("json")
                })
                .map(|entry| entry.path())
                .collect();

            if json_files.is_empty() {
                println!("{} No JSON trajectory files found in {:?}", "⚠".yellow(), log_dir);
                return Ok(());
            }

            println!("{} Found {} trajectory files", "🔍".cyan(), json_files.len());
            println!();

            let mut imported = 0;
            let mut skipped = 0;
            let mut errors = 0;

            for (i, json_file) in json_files.iter().enumerate() {
                let filename = json_file.file_name().unwrap().to_string_lossy();
                print!("[{}/{}] Importing {}... ", i + 1, json_files.len(), filename);

                match extract_metrics_from_json_file(&json_file) {
                    Ok((session_metrics, tool_call_metrics)) => {
                        // Check if session already exists
                        if metrics_db.get_session(&session_metrics.id)?.is_some() {
                            println!("{}", "SKIPPED (already exists)".yellow());
                            skipped += 1;
                            continue;
                        }

                        // Store session metrics
                        metrics_db.store_session(&session_metrics)?;

                        // Store tool call metrics
                        for tool_call in &tool_call_metrics {
                            metrics_db.store_tool_call(tool_call)?;
                        }

                        println!(
                            "{} ({} tools, {} errors)",
                            "✓".green(),
                            tool_call_metrics.len(),
                            session_metrics.tool_errors
                        );
                        imported += 1;
                    }
                    Err(e) => {
                        println!("{} {}", "✗".red(), e);
                        errors += 1;
                    }
                }
            }

            println!();
            println!("{}", "=".repeat(60));
            println!("{} Import complete:", "📊".cyan().bold());
            println!("  Imported: {}", imported.to_string().green());
            println!("  Skipped:  {}", skipped.to_string().yellow());
            println!("  Errors:   {}", errors.to_string().red());
            println!("{}", "=".repeat(60));
        }
        MetricsCommands::Show { session_id, db } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            let session = metrics_db
                .get_session(&session_id)?
                .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", session_id))?;

            let tool_calls = metrics_db.get_tool_calls(&session_id)?;

            println!("{}", "=".repeat(60));
            println!("{} Session: {}", "📊".cyan().bold(), session.id);
            println!("{}", "=".repeat(60));
            println!();
            println!("{} Model:      {}", "🤖", session.model);
            println!("{} Timestamp:  {}", "📅", session.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
            println!("{} Duration:   {:.1}s", "⏱", session.duration_seconds);
            println!("{} Status:     {:?}", "📍", session.final_status);
            println!();
            println!("{} Tokens:", "💰".cyan().bold());
            println!("  Input:   {}", format_number(session.tokens_in));
            println!("  Output:  {}", format_number(session.tokens_out));
            println!("  Cached:  {}", format_number(session.tokens_cached));
            println!("  Cost:    ${:.4}", session.cost_usd);
            println!();
            println!("{} Tasks:", "📋".cyan().bold());
            println!("  Claimed:   {}", session.issues_claimed);
            println!("  Completed: {}", session.issues_completed);
            println!();
            println!("{} Tool Calls:", "🔧".cyan().bold());
            println!("  Total:  {}", session.tool_calls);
            println!("  Errors: {} ({:.1}%)",
                session.tool_errors,
                if session.tool_calls > 0 {
                    (session.tool_errors as f64 / session.tool_calls as f64) * 100.0
                } else {
                    0.0
                }
            );
            println!();
            println!("{} Performance:", "⚡".cyan().bold());
            println!("  Messages: {}", session.messages);
            if let Some(apm) = session.apm {
                let tier = APMTier::from_apm(apm);
                let colored_apm = match tier {
                    APMTier::Elite => format!("{:.2}", apm).yellow().bold(),
                    APMTier::HighPerformance => format!("{:.2}", apm).green().bold(),
                    APMTier::Productive => format!("{:.2}", apm).green(),
                    APMTier::Active => format!("{:.2}", apm).blue(),
                    APMTier::Baseline => format!("{:.2}", apm).dimmed(),
                };
                println!("  APM:      {} ({})", colored_apm, tier.name().dimmed());
            } else {
                println!("  APM:      {}", "Not calculated".dimmed());
            }
            println!();

            if !tool_calls.is_empty() {
                println!("{} Tool Call Breakdown:", "🔧".cyan().bold());
                for (i, tc) in tool_calls.iter().take(10).enumerate() {
                    let status = if tc.success {
                        "✓".green()
                    } else {
                        format!("✗ ({})", tc.error_type.as_deref().unwrap_or("unknown")).red()
                    };
                    println!(
                        "  {:2}. {} {:20} {}ms",
                        i + 1,
                        status,
                        tc.tool_name,
                        tc.duration_ms
                    );
                }
                if tool_calls.len() > 10 {
                    println!("  ... and {} more", tool_calls.len() - 10);
                }
            }

            println!();

            // Show anomalies if any detected
            let anomalies = metrics_db.get_anomalies(&session_id)?;
            if !anomalies.is_empty() {
                println!("{} Anomalies Detected:", "⚠".yellow().bold());
                for anomaly in &anomalies {
                    let severity_str = match anomaly.severity {
                        autopilot::metrics::AnomalySeverity::Critical => "CRITICAL".red().bold(),
                        autopilot::metrics::AnomalySeverity::Error => "ERROR".red(),
                        autopilot::metrics::AnomalySeverity::Warning => "WARNING".yellow(),
                    };
                    println!(
                        "  [{}] {}: expected {:.3}, got {:.3}",
                        severity_str,
                        anomaly.dimension,
                        anomaly.expected_value,
                        anomaly.actual_value
                    );
                }
                println!();
            }

            // Show comparison to baselines
            println!("{} Comparison to Baselines:", "📈".cyan().bold());

            // Tool error rate
            if session.tool_calls > 0 {
                let error_rate = (session.tool_errors as f64) / (session.tool_calls as f64);
                if let Ok(Some(baseline)) = metrics_db.get_baseline("tool_error_rate") {
                    let deviation = ((error_rate - baseline.mean) / baseline.stddev).abs();
                    let status = if deviation > 2.0 {
                        format!("({:.1}σ above baseline)", deviation).red()
                    } else if error_rate > baseline.mean {
                        format!("({:.1}σ above baseline)", deviation).yellow()
                    } else {
                        format!("({:.1}σ below baseline)", deviation).green()
                    };
                    println!(
                        "  Tool error rate:   {:.1}% vs {:.1}% baseline {}",
                        error_rate * 100.0,
                        baseline.mean * 100.0,
                        status
                    );
                }
            }

            // Tokens per issue
            if session.issues_completed > 0 {
                let total_tokens = session.tokens_in + session.tokens_out;
                let tokens_per_issue = (total_tokens as f64) / (session.issues_completed as f64);
                if let Ok(Some(baseline)) = metrics_db.get_baseline("tokens_per_issue") {
                    let deviation = ((tokens_per_issue - baseline.mean) / baseline.stddev).abs();
                    let status = if deviation > 2.0 {
                        format!("({:.1}σ from baseline)", deviation).yellow()
                    } else {
                        format!("({:.1}σ from baseline)", deviation).dimmed()
                    };
                    println!(
                        "  Tokens per issue:  {} vs {} baseline {}",
                        format_number(tokens_per_issue as i64),
                        format_number(baseline.mean as i64),
                        status
                    );
                }
            }

            // Cost per issue
            if session.issues_completed > 0 {
                let cost_per_issue = session.cost_usd / (session.issues_completed as f64);
                if let Ok(Some(baseline)) = metrics_db.get_baseline("cost_per_issue") {
                    let deviation = ((cost_per_issue - baseline.mean) / baseline.stddev).abs();
                    let status = if deviation > 2.0 {
                        format!("({:.1}σ from baseline)", deviation).yellow()
                    } else {
                        format!("({:.1}σ from baseline)", deviation).dimmed()
                    };
                    println!(
                        "  Cost per issue:    ${:.4} vs ${:.4} baseline {}",
                        cost_per_issue,
                        baseline.mean,
                        status
                    );
                }
            }

            println!();
            println!("{} Prompt:", "📝".cyan().bold());
            let prompt_preview = if session.prompt.len() > 200 {
                format!("{}...", &session.prompt[..200])
            } else {
                session.prompt
            };
            println!("{}", prompt_preview);
            println!("{}", "=".repeat(60));
        }
        MetricsCommands::Stats { session_id, db } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Get session (either specified or most recent)
            let session = if let Some(sid) = session_id {
                metrics_db
                    .get_session(&sid)?
                    .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", sid))?
            } else {
                // Get most recent session
                let sessions = metrics_db.get_all_sessions()?;
                sessions
                    .into_iter()
                    .max_by_key(|s| s.timestamp)
                    .ok_or_else(|| anyhow::anyhow!("No sessions found in database"))?
            };

            // Concise one-line format
            let error_rate = if session.tool_calls > 0 {
                (session.tool_errors as f64 / session.tool_calls as f64) * 100.0
            } else {
                0.0
            };

            println!(
                "{} {} | {}s | {} → {} issues | {}k tokens | ${:.3} | {}/{} tools ({:.1}% err) | {:?}",
                "📊".cyan(),
                session.id,
                session.duration_seconds,
                session.issues_claimed,
                session.issues_completed,
                (session.tokens_in + session.tokens_out) / 1000,
                session.cost_usd,
                session.tool_calls - session.tool_errors,
                session.tool_calls,
                error_rate,
                session.final_status
            );

            // Show comparison to baseline
            if session.tool_calls > 0 {
                if let Ok(Some(baseline)) = metrics_db.get_baseline("tool_error_rate") {
                    let actual_error_rate = session.tool_errors as f64 / session.tool_calls as f64;
                    let deviation = ((actual_error_rate - baseline.mean) / baseline.stddev).abs();
                    if deviation > 2.0 {
                        println!("  ⚠️  Error rate {:.1}σ above baseline", deviation);
                    } else if actual_error_rate < baseline.mean - baseline.stddev {
                        println!("  ✨ Error rate {:.1}σ below baseline", deviation);
                    }
                }
            }
        }
        MetricsCommands::List { status, limit, db } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            let sessions = metrics_db.get_all_sessions()?;

            let filtered: Vec<_> = if let Some(status_filter) = status {
                sessions
                    .into_iter()
                    .filter(|s| format!("{:?}", s.final_status).to_lowercase() == status_filter.to_lowercase())
                    .take(limit)
                    .collect()
            } else {
                sessions.into_iter().take(limit).collect()
            };

            if filtered.is_empty() {
                println!("{} No sessions found", "⚠".yellow());
                return Ok(());
            }

            println!("{}", "=".repeat(100));
            println!("{} Sessions (showing {} of total)", "📊".cyan().bold(), filtered.len());
            println!("{}", "=".repeat(100));
            println!(
                "{:20} {:8} {:12} {:>8} {:>8} {:>6} {:>6}  {}",
                "TIMESTAMP", "MODEL", "STATUS", "TOKENS", "COST", "TOOLS", "ERRS", "PROMPT"
            );
            println!("{}", "-".repeat(100));

            for session in &filtered {
                let prompt_preview = if session.prompt.len() > 30 {
                    format!("{}...", &session.prompt[..27])
                } else {
                    session.prompt.clone()
                };

                let status_str = format!("{:?}", session.final_status);
                let status_colored = match session.final_status {
                    autopilot::metrics::SessionStatus::Completed => status_str.green(),
                    autopilot::metrics::SessionStatus::Crashed => status_str.red(),
                    autopilot::metrics::SessionStatus::BudgetExhausted => status_str.yellow(),
                    autopilot::metrics::SessionStatus::MaxTurns => status_str.yellow(),
                    autopilot::metrics::SessionStatus::Running => status_str.cyan(),
                };

                println!(
                    "{:20} {:8} {:12} {:>8} ${:>7.4} {:>6} {:>6}  {}",
                    session.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    session.model,
                    status_colored,
                    format_number(session.tokens_in + session.tokens_out),
                    session.cost_usd,
                    session.tool_calls,
                    session.tool_errors,
                    prompt_preview
                );
            }

            println!("{}", "=".repeat(100));
        }
        MetricsCommands::Analyze { period, compare, db } => {
            use autopilot::analyze::{
                calculate_aggregate_stats_from_sessions, detect_regressions,
                get_sessions_in_period, get_slowest_tools, get_top_error_tools,
            };

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Handle compare mode if specified
            if let Some(compare_str) = compare {
                return handle_compare_analysis(&metrics_db, &compare_str);
            }

            // Parse period
            let time_period = parse_time_period(&period)?;

            // Get sessions
            let sessions = get_sessions_in_period(&metrics_db, time_period)?;

            if sessions.is_empty() {
                println!("{} No sessions found in {}", "⚠".yellow(), time_period.name());
                return Ok(());
            }

            // Calculate aggregate stats
            let stats = calculate_aggregate_stats_from_sessions(&sessions);

            // Detect regressions
            let regressions = detect_regressions(&metrics_db, time_period)?;

            // Get top error tools
            let top_errors = get_top_error_tools(&metrics_db, time_period, 5)?;

            // Get slowest tools
            let slowest_tools = get_slowest_tools(&metrics_db, time_period, 5)?;

            // Print report
            println!("{}", "=".repeat(80));
            println!(
                "{} Metrics Analysis: {}",
                "📊".cyan().bold(),
                time_period.name()
            );
            println!("{}", "=".repeat(80));
            println!();
            println!("{} Overview:", "📈".cyan().bold());
            println!("  Sessions:       {}", sessions.len());
            println!();

            // Print aggregate statistics
            println!("{} Aggregate Statistics:", "📊".cyan().bold());
            let metrics_order = vec![
                "tool_error_rate",
                "completion_rate",
                "tokens_per_issue",
                "cost_per_issue",
                "duration_per_issue",
                "session_duration",
            ];

            for metric_name in metrics_order {
                if let Some(stat) = stats.get(metric_name) {
                    let formatted = format_metric_value(metric_name, stat.mean);
                    println!(
                        "  {:20} mean={} p50={} p90={}",
                        metric_name,
                        formatted,
                        format_metric_value(metric_name, stat.median),
                        format_metric_value(metric_name, stat.p90)
                    );
                }
            }
            println!();

            // Print regressions
            if !regressions.is_empty() {
                println!("{} Regressions Detected:", "⚠".red().bold());
                for reg in &regressions {
                    use autopilot::analyze::RegressionSeverity;
                    let severity_text = match reg.severity {
                        RegressionSeverity::Critical => "CRITICAL".red().bold(),
                        RegressionSeverity::Error => "ERROR".red(),
                        RegressionSeverity::Warning => "WARNING".yellow(),
                    };
                    println!(
                        "  {} {:20} {:.1}% worse, {:.1}σ (baseline: {}, current: {})",
                        severity_text,
                        reg.dimension,
                        reg.percent_worse,
                        reg.deviation_sigma,
                        format_metric_value(&reg.dimension, reg.baseline_value),
                        format_metric_value(&reg.dimension, reg.current_value)
                    );
                }
                println!();
            } else {
                println!("{} No regressions detected", "✓".green().bold());
                println!();
            }

            // Print top error tools
            if !top_errors.is_empty() {
                println!("{} Top Error Tools:", "🔧".cyan().bold());
                for (tool, count) in &top_errors {
                    println!("  {:30} {} errors", tool, count);
                }
                println!();
            }

            // Print slowest tools
            if !slowest_tools.is_empty() {
                println!("{} Slowest Tools (avg duration):", "⏱".cyan().bold());
                for (tool, avg_ms, count) in &slowest_tools {
                    println!("  {:30} {:.0}ms avg (n={})", tool, avg_ms, count);
                }
                println!();
            }

            println!("{}", "=".repeat(80));
        }
        MetricsCommands::Trends { recent, baseline, db } => {
            use autopilot::analyze::{detect_trends, TrendDirection};

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Parse periods
            let recent_period = parse_time_period(&recent)?;
            let baseline_period = Some(parse_time_period(&baseline)?);

            // Detect trends
            let trends = detect_trends(&metrics_db, recent_period, baseline_period)?;

            if trends.is_empty() {
                println!("{} No trend data available", "⚠".yellow());
                return Ok(());
            }

            // Print trends report
            println!("{}", "=".repeat(80));
            println!(
                "{} Trend Analysis: {} vs {}",
                "📈".cyan().bold(),
                recent_period.name(),
                baseline_period.unwrap().name()
            );
            println!("{}", "=".repeat(80));
            println!();

            for trend in &trends {
                let direction_icon = match trend.direction {
                    TrendDirection::Improving => "↑".green(),
                    TrendDirection::Stable => "→".yellow(),
                    TrendDirection::Degrading => "↓".red(),
                };

                let direction_str = match trend.direction {
                    TrendDirection::Improving => "IMPROVING".green().bold(),
                    TrendDirection::Stable => "STABLE".yellow(),
                    TrendDirection::Degrading => "DEGRADING".red().bold(),
                };

                println!("{} {} {}", direction_icon, trend.dimension, direction_str);
                println!(
                    "    Recent:   {} (n={})",
                    format_metric_value(&trend.dimension, trend.recent.mean),
                    trend.recent.count
                );
                if let Some(ref base) = trend.baseline {
                    println!(
                        "    Baseline: {} (n={})",
                        format_metric_value(&trend.dimension, base.mean),
                        base.count
                    );
                    if trend.percent_change.abs() > 0.1 {
                        let change_str = format!("{:+.1}%", trend.percent_change);
                        let change_colored = if trend.direction == TrendDirection::Improving {
                            change_str.green()
                        } else if trend.direction == TrendDirection::Degrading {
                            change_str.red()
                        } else {
                            change_str.normal()
                        };
                        println!("    Change:   {}", change_colored);
                    }
                }
                println!();
            }

            println!("{}", "=".repeat(80));
        }
        MetricsCommands::Dashboard {
            metrics_db,
            port,
        } => {
            use autopilot::dashboard::start_dashboard;

            let db_path = metrics_db
                .unwrap_or_else(default_db_path)
                .to_string_lossy()
                .to_string();

            println!("{}", "=".repeat(80));
            println!("{} Starting Autopilot Metrics Dashboard", "📊".cyan().bold());
            println!("{}", "=".repeat(80));
            println!();
            println!("  Database: {}", db_path);
            println!("  URL: http://127.0.0.1:{}", port);
            println!();
            println!("  Press Ctrl+C to stop");
            println!();

            start_dashboard(&db_path, port).await?;
        }
        MetricsCommands::Learn {
            metrics_db,
            sessions,
            limit,
            format,
        } => {
            use autopilot::learning::LearningPipeline;

            let db_path = metrics_db.unwrap_or_else(default_db_path);
            let db = MetricsDb::open(&db_path)?;

            println!("{}", "=".repeat(80));
            println!("{} Autopilot Learning Pipeline", "🧠".cyan().bold());
            println!("{}", "=".repeat(80));
            println!();

            // Get session IDs to analyze
            let session_ids: Vec<String> = if sessions.is_empty() {
                println!("{} Analyzing last {} sessions...", "📊".cyan(), limit);
                let recent = db.get_recent_sessions(limit)?;
                recent.into_iter().map(|s| s.id).collect()
            } else {
                println!("{} Analyzing {} specific sessions...", "📊".cyan(), sessions.len());
                sessions
            };

            if session_ids.is_empty() {
                println!("{} No sessions found to analyze", "⚠️".yellow());
                return Ok(());
            }

            // Run the learning pipeline
            let pipeline = LearningPipeline::new(&db);
            let report = pipeline.run(&session_ids)?;

            // Output results
            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                _ => {
                    println!();
                    println!("{} {} improvements detected", "✨".green(), report.improvements.len());
                    println!();

                    for improvement in &report.improvements {
                        println!("{} {:?} (severity: {}/10)", "⚠️".yellow(), improvement.improvement_type, improvement.severity);
                        println!("  Description: {}", improvement.description);
                        println!("  Proposed fix: {}", improvement.proposed_fix);
                        println!("  Evidence: {} items", improvement.evidence.len());
                        println!();
                    }

                    if !report.prompt_updates.is_empty() {
                        println!("{} {} prompt updates proposed", "📝".cyan(), report.prompt_updates.len());
                        for update in &report.prompt_updates {
                            println!("  {}: {}", update.file_path, update.section);
                            println!("    {}", update.rationale);
                        }
                        println!();
                    }

                    if !report.hook_updates.is_empty() {
                        println!("{} {} hook updates proposed", "🪝".cyan(), report.hook_updates.len());
                        for update in &report.hook_updates {
                            println!("  {}", update.hook_name);
                            println!("    {}", update.rationale);
                        }
                        println!();
                    }

                    if !report.issues_created.is_empty() {
                        println!("{} {} issues created", "📋".cyan(), report.issues_created.len());
                        println!();
                    }
                }
            }
        }
        MetricsCommands::Export { db, period, format, output } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Get sessions based on period
            let sessions = if period == "all" {
                metrics_db.get_all_sessions()?
            } else {
                use autopilot::analyze::{get_sessions_in_period, TimePeriod};
                let time_period = match period.as_str() {
                    "7d" => TimePeriod::Last7Days,
                    "30d" => TimePeriod::Last30Days,
                    "last-week" => TimePeriod::LastWeek,
                    "this-week" => TimePeriod::ThisWeek,
                    _ => TimePeriod::Last7Days,
                };
                get_sessions_in_period(&metrics_db, time_period)?
            };

            if sessions.is_empty() {
                eprintln!("No sessions found for period: {}", period);
                return Ok(());
            }

            // Build export data
            let export_data = if format == "csv" {
                // CSV format: header + rows
                let mut csv = String::new();
                csv.push_str("session_id,timestamp,model,duration_s,tokens_in,tokens_out,tokens_cached,cost_usd,issues_claimed,issues_completed,tool_calls,tool_errors,final_status\n");

                for session in &sessions {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},{},{},{},{:?}\n",
                        session.id,
                        session.timestamp.to_rfc3339(),
                        session.model,
                        session.duration_seconds,
                        session.tokens_in,
                        session.tokens_out,
                        session.tokens_cached,
                        session.cost_usd,
                        session.issues_claimed,
                        session.issues_completed,
                        session.tool_calls,
                        session.tool_errors,
                        session.final_status
                    ));
                }
                csv
            } else {
                // JSON format
                serde_json::to_string_pretty(&sessions)?
            };

            // Output to file or stdout
            if let Some(output_path) = output {
                std::fs::write(&output_path, export_data)?;
                eprintln!("Exported {} sessions to {}", sessions.len(), output_path.display());
            } else {
                println!("{}", export_data);
            }
        }
        MetricsCommands::BackfillApm { db } => {
            use autopilot::metrics::backfill_apm_for_sessions;
            use colored::Colorize;

            let db_path = db.unwrap_or_else(default_db_path);

            println!("{} Backfilling APM data for existing sessions...", "📊".cyan());
            println!("{} Database: {:?}", "📂".dimmed(), db_path);
            println!();

            match backfill_apm_for_sessions(&db_path) {
                Ok(count) => {
                    println!("{} Updated APM for {} sessions", "✅".green(), count);
                    if count == 0 {
                        println!("{}", "All sessions already have APM calculated".dimmed());
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to backfill APM: {}", "❌".red(), e);
                    std::process::exit(1);
                }
            }
        }

        MetricsCommands::Baseline(cmd) => {
            use autopilot::metrics::baseline::{BaselineCalculator, BaselineComparator, BaselineReportGenerator};

            let db_path = match cmd {
                BaselineCommands::Update { ref metrics_db, .. } |
                BaselineCommands::Show { ref metrics_db, .. } |
                BaselineCommands::Check { ref metrics_db, .. } |
                BaselineCommands::Report { ref metrics_db, .. } => {
                    metrics_db.clone().unwrap_or_else(default_db_path)
                }
            };
            let db = MetricsDb::open(&db_path)?;

            match cmd {
                BaselineCommands::Update { .. } => {
                    println!("{}", "=".repeat(80));
                    println!("{} Updating Baselines", "📊".cyan().bold());
                    println!("{}", "=".repeat(80));
                    println!();

                    let calculator = BaselineCalculator::new(&db);
                    let count = calculator.update_all_baselines()?;

                    println!("{} Updated {} baseline metrics", "✅".green(), count);
                }
                BaselineCommands::Show { format, .. } => {
                    use autopilot::metrics::baseline::MetricDimension;

                    println!("{}", "=".repeat(80));
                    println!("{} Current Baselines", "📊".cyan().bold());
                    println!("{}", "=".repeat(80));
                    println!();

                    match format.as_str() {
                        "json" => {
                            let mut baselines = std::collections::HashMap::new();
                            for dimension in MetricDimension::all() {
                                if let Ok(Some(baseline)) = db.get_baseline(dimension.as_str()) {
                                    baselines.insert(dimension.as_str(), baseline);
                                }
                            }
                            println!("{}", serde_json::to_string_pretty(&baselines)?);
                        }
                        _ => {
                            for dimension in MetricDimension::all() {
                                if let Ok(Some(baseline)) = db.get_baseline(dimension.as_str()) {
                                    println!("{}", dimension.as_str());
                                    println!("  Mean:    {:.4}", baseline.mean);
                                    println!("  StdDev:  {:.4}", baseline.stddev);
                                    println!("  p50:     {:.4}", baseline.p50);
                                    println!("  p90:     {:.4}", baseline.p90);
                                    println!("  p99:     {:.4}", baseline.p99);
                                    println!("  Samples: {}", baseline.sample_count);
                                    println!();
                                }
                            }
                        }
                    }
                }
                BaselineCommands::Check { sessions, .. } => {
                    println!("{}", "=".repeat(80));
                    println!("{} Checking for Regressions", "🔍".cyan().bold());
                    println!("{}", "=".repeat(80));
                    println!();

                    let recent_sessions = db.get_recent_sessions(sessions)?;
                    let comparator = BaselineComparator::new(&db);
                    let regressions = comparator.detect_regressions(&recent_sessions)?;

                    if regressions.is_empty() {
                        println!("{} No regressions detected", "✅".green());
                    } else {
                        println!("{} {} regressions detected:", "⚠️".yellow(), regressions.len());
                        println!();

                        for regression in &regressions {
                            println!("{} {:?} ({:?})", "🔴".red(), regression.dimension, regression.severity);
                            println!("  Baseline: {:.4}", regression.baseline_value);
                            println!("  Current:  {:.4}", regression.current_value);
                            println!("  Change:   {:+.2}%", regression.percent_change);
                            println!();
                        }
                    }
                }
                BaselineCommands::Report { output, .. } => {
                    let generator = BaselineReportGenerator::new(&db);
                    let report = generator.generate_report()?;

                    let output_path = output.unwrap_or_else(|| {
                        PathBuf::from("docs/autopilot/BASELINES.md")
                    });

                    // Ensure parent directory exists
                    if let Some(parent) = output_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    std::fs::write(&output_path, report)?;

                    println!("{}", "=".repeat(80));
                    println!("{} Baseline Report Generated", "📝".cyan().bold());
                    println!("{}", "=".repeat(80));
                    println!();
                    println!("  Output: {}", output_path.display());
                }
            }
        }
        MetricsCommands::CreateIssues {
            metrics_db,
            issues_db,
            dry_run,
        } => {
            use autopilot::auto_issues::{create_issues, detect_all_patterns, generate_issues, Pattern};

            let metrics_db_path = metrics_db.unwrap_or_else(default_db_path);
            let metrics = MetricsDb::open(&metrics_db_path)?;

            let issues_db_path = issues_db.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("autopilot.db")
            });

            println!("{}", "=".repeat(80));
            println!(
                "{} Automated Issue Creation from Pattern Detection",
                "🤖".cyan().bold()
            );
            println!("{}", "=".repeat(80));
            println!();

            // Detect all patterns (both anomaly and tool error patterns)
            println!("{} Detecting patterns...", "🔍".cyan());
            let patterns = detect_all_patterns(&metrics)?;

            if patterns.is_empty() {
                println!("{} No patterns detected", "✓".green());
                println!("  All metrics appear normal, or issues have already been created.");
                println!();
                return Ok(());
            }

            // Count pattern types
            let anomaly_count = patterns.iter().filter(|p| matches!(p, Pattern::Anomaly(_))).count();
            let tool_error_count = patterns.iter().filter(|p| matches!(p, Pattern::ToolError(_))).count();

            println!("{} Found {} patterns:", "📊".cyan(), patterns.len());
            if anomaly_count > 0 {
                println!("  - {} anomaly patterns", anomaly_count);
            }
            if tool_error_count > 0 {
                println!("  - {} tool error patterns", tool_error_count);
            }
            println!();

            // Generate issues
            let improvement_issues = generate_issues(patterns);

            println!("{} Proposed Issues:", "📝".cyan().bold());
            for (i, issue) in improvement_issues.iter().enumerate() {
                let priority_colored = match issue.priority.as_str() {
                    "urgent" => issue.priority.red().bold(),
                    "high" => issue.priority.yellow(),
                    _ => issue.priority.normal(),
                };
                println!(
                    "\n{}. {} [{}]",
                    i + 1,
                    issue.title.bold(),
                    priority_colored
                );
                // Show pattern-specific details
                match &issue.pattern {
                    Pattern::Anomaly(p) => {
                        println!("   Type: Anomaly pattern");
                        println!("   Dimension: {} ({} sessions, {:?} severity)",
                            p.dimension,
                            p.occurrence_count,
                            p.severity
                        );
                    }
                    Pattern::ToolError(p) => {
                        println!("   Type: Tool error pattern");
                        println!("   Tool: {} ({:.1}% error rate, {} failures)",
                            p.tool_name,
                            p.error_rate * 100.0,
                            p.failed_calls
                        );
                    }
                }
            }
            println!();

            if dry_run {
                println!("{} Dry run mode - no issues created", "ℹ".cyan());
                println!();
                return Ok(());
            }

            // Create issues
            println!("{} Creating issues...", "🚀".cyan());
            let issue_numbers = create_issues(&issues_db_path, &improvement_issues, &metrics)?;

            println!();
            println!("{}", "=".repeat(80));
            println!(
                "{} Created {} improvement issues linked to d-004",
                "✓".green().bold(),
                issue_numbers.len()
            );
            println!();
            println!("Issue numbers: {}", issue_numbers.iter()
                .map(|n| format!("#{}", n))
                .collect::<Vec<_>>()
                .join(", "));
            println!();
            println!("View issues: cargo autopilot issue list");
            println!("{}", "=".repeat(80));
        }

        MetricsCommands::Alerts(cmd) => {
            use autopilot::alerts;
            use autopilot::metrics::MetricsDb;

            let db_path = match &cmd {
                AlertCommands::List { db, .. } |
                AlertCommands::Add { db, .. } |
                AlertCommands::Remove { db, .. } |
                AlertCommands::History { db, .. } => db.as_ref().map(|p| p.clone()).unwrap_or_else(default_db_path),
            };

            let metrics_db = MetricsDb::open(&db_path)?;
            let conn = metrics_db.connection();

            match cmd {
                AlertCommands::List { .. } => {
                    let rules = alerts::list_alert_rules(conn)?;

                    if rules.is_empty() {
                        println!("{} No alert rules configured", "ℹ".cyan());
                        println!("\nAdd a rule with: cargo autopilot metrics alerts add \\");
                        println!("    --metric tool_error_rate \\");
                        println!("    --alert-type threshold \\");
                        println!("    --severity critical \\");
                        println!("    --threshold 0.10 \\");
                        println!("    --description \"High tool error rate\"");
                        return Ok(());
                    }

                    println!("{} Alert Rules", "📋".cyan().bold());
                    println!();
                    for rule in rules {
                        let enabled = if rule.enabled { "✓".green() } else { "✗".red() };
                        let severity_colored = match rule.severity {
                            alerts::AlertSeverity::Warning => "warning".yellow(),
                            alerts::AlertSeverity::Error => "error".red(),
                            alerts::AlertSeverity::Critical => "critical".red().bold(),
                        };
                        println!("{} Rule #{}: {} [{}]", enabled, rule.id, rule.metric_name, severity_colored);
                        println!("   Type: {:?}, Threshold: {:.2}", rule.alert_type, rule.threshold);
                        println!("   Description: {}", rule.description);
                        println!();
                    }
                }

                AlertCommands::Add { metric, alert_type, severity, threshold, description, .. } => {
                    let alert_type = alerts::AlertType::from_str(&alert_type)
                        .ok_or_else(|| anyhow::anyhow!("Invalid alert type. Use: threshold, regression, or rate_of_change"))?;
                    let severity = alerts::AlertSeverity::from_str(&severity)
                        .ok_or_else(|| anyhow::anyhow!("Invalid severity. Use: warning, error, or critical"))?;

                    let rule_id = alerts::add_alert_rule(conn, &metric, alert_type, severity, threshold, &description)?;

                    println!("{} Created alert rule #{}", "✓".green(), rule_id);
                    println!("  Metric: {}", metric);
                    println!("  Type: {:?}", alert_type);
                    println!("  Severity: {:?}", severity);
                    println!("  Threshold: {:.2}", threshold);
                }

                AlertCommands::Remove { rule_id, .. } => {
                    alerts::remove_alert_rule(conn, rule_id)?;
                    println!("{} Removed alert rule #{}", "✓".green(), rule_id);
                }

                AlertCommands::History { session, metric, limit, .. } => {
                    let alerts = alerts::get_alert_history(
                        conn,
                        session.as_deref(),
                        metric.as_deref(),
                        Some(limit),
                    )?;

                    if alerts.is_empty() {
                        println!("{} No alerts fired", "✓".green());
                        return Ok(());
                    }

                    println!("{} Alert History ({} alerts)", "📜".cyan().bold(), alerts.len());
                    println!();
                    for alert in alerts {
                        let severity_colored = match alert.severity {
                            alerts::AlertSeverity::Warning => "WARNING".yellow(),
                            alerts::AlertSeverity::Error => "ERROR".red(),
                            alerts::AlertSeverity::Critical => "CRITICAL".red().bold(),
                        };
                        println!("[{}] {}", severity_colored, alert.fired_at.format("%Y-%m-%d %H:%M:%S"));
                        println!("  {}", alert.message);
                        println!("  Session: {}", alert.session_id);
                        println!();
                    }
                }
            }
        }

        MetricsCommands::IssueMetrics { issue_number, db, format } => {
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            match metrics_db.get_issue_metrics(issue_number)? {
                Some(metrics) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&metrics)?);
                    } else {
                        println!("{} Issue #{} Metrics", "📊".cyan().bold(), issue_number);
                        println!();
                        println!("Sessions:        {}", metrics.sessions_count);
                        println!("Duration:        {:.1}s total, {:.1}s avg", metrics.total_duration_seconds, metrics.avg_duration_seconds);
                        println!("Tokens:          {} total, {:.0} avg", format_number(metrics.total_tokens), metrics.avg_tokens);
                        println!("Cost:            ${:.4} total, ${:.4} avg", metrics.total_cost_usd, metrics.avg_cost_usd);
                        println!("Tool Calls:      {}", metrics.tool_calls);
                        println!("Tool Errors:     {} ({:.1}% error rate)", metrics.tool_errors, metrics.error_rate);
                    }
                }
                None => {
                    println!("{} No metrics found for issue #{}", "ℹ".yellow(), issue_number);
                }
            }
        }

        MetricsCommands::DirectiveMetrics { directive_id, db, format } => {
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            match metrics_db.get_directive_metrics(&directive_id)? {
                Some(metrics) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&metrics)?);
                    } else {
                        println!("{} Directive {} Metrics", "📊".cyan().bold(), directive_id);
                        println!();
                        println!("Sessions:          {}", metrics.sessions_count);
                        println!("Issues Completed:  {}", metrics.issues_completed);
                        println!("Duration:          {:.1}s total, {:.1}s avg", metrics.total_duration_seconds, metrics.avg_duration_seconds);
                        println!("Tokens:            {} total, {:.0} avg", format_number(metrics.total_tokens), metrics.avg_tokens);
                        println!("Cost:              ${:.4} total, ${:.4} avg", metrics.total_cost_usd, metrics.avg_cost_usd);
                        println!("Tool Calls:        {}", metrics.tool_calls);
                        println!("Tool Errors:       {} ({:.1}% error rate)", metrics.tool_errors, metrics.error_rate);
                    }
                }
                None => {
                    println!("{} No metrics found for directive {}", "ℹ".yellow(), directive_id);
                }
            }
        }

        MetricsCommands::ByIssue { db, format } => {
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            let all_metrics = metrics_db.get_all_issue_metrics()?;

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&all_metrics)?);
            } else {
                if all_metrics.is_empty() {
                    println!("{} No issue metrics available", "ℹ".yellow());
                    return Ok(());
                }

                println!("{} Metrics by Issue", "📊".cyan().bold());
                println!();
                println!("{:<8} {:>10} {:>12} {:>12} {:>10} {:>10}",
                    "Issue", "Sessions", "Duration", "Tokens", "Cost", "Error %");
                println!("{}", "─".repeat(80));

                for metric in all_metrics {
                    println!("{:<8} {:>10} {:>12.1}s {:>12} ${:>9.4} {:>9.1}%",
                        format!("#{}", metric.issue_number),
                        metric.sessions_count,
                        metric.avg_duration_seconds,
                        format_number(metric.avg_tokens as i64),
                        metric.avg_cost_usd,
                        metric.error_rate
                    );
                }
            }
        }

        MetricsCommands::ByDirective { db, format } => {
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            let all_metrics = metrics_db.get_all_directive_metrics()?;

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&all_metrics)?);
            } else {
                if all_metrics.is_empty() {
                    println!("{} No directive metrics available", "ℹ".yellow());
                    return Ok(());
                }

                println!("{} Metrics by Directive", "📊".cyan().bold());
                println!();
                println!("{:<12} {:>10} {:>12} {:>12} {:>12} {:>10} {:>10}",
                    "Directive", "Sessions", "Issues", "Duration", "Tokens", "Cost", "Error %");
                println!("{}", "─".repeat(95));

                for metric in all_metrics {
                    println!("{:<12} {:>10} {:>12} {:>12.1}s {:>12} ${:>9.4} {:>9.1}%",
                        metric.directive_id,
                        metric.sessions_count,
                        metric.issues_completed,
                        metric.avg_duration_seconds,
                        format_number(metric.avg_tokens as i64),
                        metric.avg_cost_usd,
                        metric.error_rate
                    );
                }
            }
        }

        MetricsCommands::Velocity { period, db, limit } => {
            use autopilot::analyze::calculate_velocity;
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Parse period
            let time_period = parse_time_period(&period)?;

            // Calculate current velocity
            let velocity = calculate_velocity(&metrics_db, time_period)?;

            // Store the snapshot
            metrics_db.store_velocity_snapshot(&velocity)?;

            // Get historical snapshots
            let snapshots = metrics_db.get_velocity_snapshots(limit)?;

            // Print report
            println!("{}", "=".repeat(80));
            println!("{} Improvement Velocity", "🚀".cyan().bold());
            println!("{}", "=".repeat(80));
            println!();

            // Current velocity
            println!("{} Current Period: {}", "📊".cyan(), velocity.period);
            println!("  Velocity Score:    {:.2} (-1.0 to 1.0)", velocity.velocity_score);
            println!("  Issues Completed:  {}", velocity.issues_completed.to_string().cyan());
            println!("  Improving Metrics: {}", velocity.improving_metrics.to_string().green());
            println!("  Stable Metrics:    {}", velocity.stable_metrics);
            println!("  Degrading Metrics: {}", velocity.degrading_metrics.to_string().red());
            println!();

            // Celebrate improvements!
            if velocity.velocity_score > 0.5 {
                println!("{} {} Great work! Autopilot is significantly improving!", "🎉".cyan().bold(), "CELEBRATION:".green().bold());
                println!("  {} metrics are improving, showing strong upward momentum!", velocity.improving_metrics);
                println!();
            } else if velocity.velocity_score > 0.2 {
                println!("{} {} Autopilot is getting better!", "✨".cyan(), "Progress:".green().bold());
                println!("  Positive improvements detected across key metrics.");
                println!();
            } else if velocity.velocity_score < -0.3 {
                println!("{} {} Attention needed - metrics are degrading.", "⚠️".yellow().bold(), "Warning:".yellow().bold());
                println!("  Consider investigating recent changes and running diagnostics.");
                println!();
            }

            // Key metrics
            if !velocity.key_metrics.is_empty() {
                println!("{} Key Metrics:", "🔑".cyan().bold());
                for metric in &velocity.key_metrics {
                    let direction_icon = match metric.direction.as_str() {
                        "improving" => "📈".green(),
                        "degrading" => "📉".red(),
                        _ => "➡️".yellow(),
                    };
                    println!(
                        "  {:<25} {} {:>7.1}%",
                        metric.dimension,
                        direction_icon,
                        metric.percent_change
                    );
                }
                println!();
            }

            // Historical trend
            if snapshots.len() > 1 {
                println!("{} Historical Velocity:", "📈".cyan().bold());
                for snapshot in &snapshots {
                    let score_color = if snapshot.velocity_score > 0.3 {
                        snapshot.velocity_score.to_string().green()
                    } else if snapshot.velocity_score < -0.3 {
                        snapshot.velocity_score.to_string().red()
                    } else {
                        snapshot.velocity_score.to_string().yellow()
                    };

                    println!(
                        "  {} | {:>8} | Score: {}",
                        snapshot.timestamp.format("%Y-%m-%d %H:%M"),
                        snapshot.period,
                        score_color
                    );
                }
                println!();
            }

            println!("{}", "=".repeat(80));
        }
    }

    Ok(())
}

/// Format large numbers with commas
fn format_number(n: i64) -> String {
    n.to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(std::str::from_utf8)
        .collect::<Result<Vec<&str>, _>>()
        .unwrap()
        .join(",")
}

/// Parse time period string to TimePeriod enum
fn parse_time_period(period_str: &str) -> Result<autopilot::analyze::TimePeriod> {
    use autopilot::analyze::TimePeriod;

    match period_str {
        "7d" => Ok(TimePeriod::Last7Days),
        "30d" => Ok(TimePeriod::Last30Days),
        "last-week" => Ok(TimePeriod::LastWeek),
        "this-week" => Ok(TimePeriod::ThisWeek),
        _ => Err(anyhow::anyhow!(
            "Invalid period: {}. Valid options: 7d, 30d, last-week, this-week",
            period_str
        )),
    }
}

/// Format metric value for display
fn format_metric_value(metric_name: &str, value: f64) -> String {
    match metric_name {
        "tool_error_rate" | "completion_rate" => format!("{:.1}%", value * 100.0),
        "cost_per_issue" => format!("${:.4}", value),
        "duration_per_issue" => format!("{:.1}s", value),
        "session_duration" => format!("{:.1}s", value),
        "tokens_per_issue" => format!("{:.0}", value),
        _ => format!("{:.2}", value),
    }
}

/// Handle compare analysis between two date ranges
fn handle_compare_analysis(metrics_db: &autopilot::metrics::MetricsDb, compare_str: &str) -> Result<()> {
    use anyhow::Context;
    use autopilot::analyze::{calculate_aggregate_stats_from_sessions, get_sessions_between_dates};
    use chrono::NaiveDate;

    // Parse date range (format: YYYY-MM-DD..YYYY-MM-DD)
    let parts: Vec<&str> = compare_str.split("..").collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid compare format. Expected: YYYY-MM-DD..YYYY-MM-DD");
    }

    let start_date = NaiveDate::parse_from_str(parts[0], "%Y-%m-%d")
        .with_context(|| format!("Failed to parse start date: {}", parts[0]))?;
    let end_date = NaiveDate::parse_from_str(parts[1], "%Y-%m-%d")
        .with_context(|| format!("Failed to parse end date: {}", parts[1]))?;

    // Convert to DateTime<Utc>
    let start = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end = end_date.and_hms_opt(23, 59, 59).unwrap().and_utc();

    // Get sessions in the date range
    let sessions = get_sessions_between_dates(metrics_db, start, end)?;

    if sessions.is_empty() {
        println!("{} No sessions found between {} and {}",
            "⚠".yellow(), parts[0], parts[1]);
        return Ok(());
    }

    // Calculate aggregate stats
    let stats = calculate_aggregate_stats_from_sessions(&sessions);

    // Print report
    println!("{}", "=".repeat(80));
    println!("{} Metrics Comparison: {} to {}",
        "📊".cyan().bold(), parts[0], parts[1]);
    println!("{}", "=".repeat(80));
    println!();
    println!("{} Overview:", "📈".cyan().bold());
    println!("  Sessions:       {}", sessions.len());
    println!("  Date Range:     {} to {}", parts[0], parts[1]);
    println!();

    // Print aggregate statistics
    println!("{} Aggregate Statistics:", "📊".cyan().bold());
    let metrics_order = vec![
        "tool_error_rate",
        "completion_rate",
        "tokens_per_issue",
        "cost_per_issue",
        "duration_per_issue",
        "session_duration",
    ];

    for metric_name in metrics_order {
        if let Some(stat) = stats.get(metric_name) {
            let formatted = format_metric_value(metric_name, stat.mean);
            println!(
                "  {:20} mean={} p50={} p90={}",
                metric_name,
                formatted,
                format_metric_value(metric_name, stat.median),
                format_metric_value(metric_name, stat.p90)
            );
        }
    }
    println!();
    println!("{}", "=".repeat(80));

    Ok(())
}

/// Handle benchmark command
async fn handle_benchmark_command(
    benchmark_id: Option<String>,
    category: Option<String>,
    baseline: Option<String>,
    save_baseline: Option<String>,
    list_baselines: bool,
    db: Option<PathBuf>,
    workspace: Option<PathBuf>,
) -> Result<()> {
    use autopilot::benchmark::{
        BenchmarkRunner, BenchmarkDatabase,
        B001SimpleFileEdit, B002MultiFileEdit, B003StructRename,
        B004SimpleCommit, B005BranchWorkflow,
        B006IssueWorkflow, B007MultiStepRefactor, B008TestDrivenFix,
        B009DocumentationGeneration, B010DependencyUpdate,
        B011ErrorRecovery, B012ContextGathering, B013CrossFileConsistency,
        B014PerformanceOptimization, B015SecurityFix,
    };

    let db_path = db.unwrap_or_else(|| PathBuf::from("autopilot-benchmarks.db"));
    let workspace_path = workspace.unwrap_or_else(|| PathBuf::from("benchmark-workspace"));
    let version = save_baseline.clone().unwrap_or_else(|| "current".to_string());

    let mut runner = BenchmarkRunner::new(workspace_path.clone(), db_path.clone(), version.clone())?;

    // All available tasks
    let all_tasks: Vec<(&str, Box<dyn autopilot::benchmark::BenchmarkTask>)> = vec![
        ("file-ops", Box::new(B001SimpleFileEdit)),
        ("file-ops", Box::new(B002MultiFileEdit)),
        ("file-ops", Box::new(B003StructRename)),
        ("git", Box::new(B004SimpleCommit)),
        ("git", Box::new(B005BranchWorkflow)),
        ("autopilot", Box::new(B006IssueWorkflow)),
        ("file-ops", Box::new(B007MultiStepRefactor)),
        ("testing", Box::new(B008TestDrivenFix)),
        ("docs", Box::new(B009DocumentationGeneration)),
        ("tooling", Box::new(B010DependencyUpdate)),
        ("resilience", Box::new(B011ErrorRecovery)),
        ("exploration", Box::new(B012ContextGathering)),
        ("refactor", Box::new(B013CrossFileConsistency)),
        ("optimization", Box::new(B014PerformanceOptimization)),
        ("security", Box::new(B015SecurityFix)),
    ];

    // Handle list baselines
    if list_baselines {
        println!("\n{}", "═".repeat(80));
        println!("{} Benchmark Baselines", "📊".cyan().bold());
        println!("{}", "═".repeat(80));
        println!();

        // Query all distinct versions with baselines
        let conn = rusqlite::Connection::open(&db_path)?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT version FROM benchmark_baselines ORDER BY version"
        )?;
        let versions: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?;

        if versions.is_empty() {
            println!("  No baselines found. Run benchmarks with --save-baseline to create one.");
        } else {
            for version in versions {
                // Get sample count and last updated for this version
                let (count, updated): (i64, String) = conn.query_row(
                    "SELECT COUNT(*), MAX(updated_at) FROM benchmark_baselines WHERE version = ?1",
                    [&version],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                println!("  {} - {} benchmarks (updated: {})",
                    version.cyan().bold(), count, updated);
            }
        }
        println!();
        return Ok(());
    }

    // Determine which tasks to run
    let tasks_to_run: Vec<&Box<dyn autopilot::benchmark::BenchmarkTask>> = if let Some(id) = &benchmark_id {
        all_tasks.iter().filter(|(_, t)| t.id() == id).map(|(_, t)| t).collect()
    } else if let Some(cat) = &category {
        all_tasks.iter().filter(|(c, _)| *c == cat).map(|(_, t)| t).collect()
    } else {
        all_tasks.iter().map(|(_, t)| t).collect()
    };

    if tasks_to_run.is_empty() {
        println!("No benchmarks match the specified criteria");
        return Ok(());
    }

    println!("{}", "=".repeat(80));
    println!("{} Autopilot Benchmark Suite", "🏁".cyan().bold());
    println!("{}", "=".repeat(80));
    println!();
    println!("  Version: {}", version);
    println!("  Database: {}", db_path.display());
    println!("  Workspace: {}", workspace_path.display());
    println!("  Tasks: {}", tasks_to_run.len());
    println!();

    // Run benchmarks
    let mut results = Vec::new();
    for task in tasks_to_run {
        match runner.run_benchmark(task.as_ref()).await {
            Ok(result) => {
                let status = if result.success {
                    "PASS".green().bold()
                } else {
                    "FAIL".red().bold()
                };
                println!("  {} {} - {}", status, result.benchmark_id, task.name());
                results.push(result);
            }
            Err(e) => {
                println!("  {} {} - Error: {}", "ERR".yellow().bold(), task.id(), e);
            }
        }
    }

    println!();
    println!("{}", "=".repeat(80));
    println!("{} Results", "📊".cyan().bold());
    println!("{}", "=".repeat(80));
    println!();

    let passed = results.iter().filter(|r| r.success).count();
    let failed = results.len() - passed;

    println!("  Total: {}", results.len());
    println!("  Passed: {}", passed.to_string().green().bold());
    if failed > 0 {
        println!("  Failed: {}", failed.to_string().red().bold());
    }
    println!();

    // Save baseline if requested
    if save_baseline.is_some() {
        let mut db = BenchmarkDatabase::open(&db_path)?;
        println!("{}", "─".repeat(80));
        println!("{} Saving Baseline", "💾".cyan().bold());
        println!("{}", "─".repeat(80));
        println!();
        println!("  Version: {}", version.cyan().bold());
        db.update_baseline(&version)?;
        println!("  ✓ Baseline metrics computed and stored");
        println!();
    }

    // Compare to baseline if requested
    if let Some(baseline_ver) = baseline {
        let report = runner.compare_to_baseline(&results, &baseline_ver)?;
        report.print();
    }

    Ok(())
}

/// Handle APM command
async fn handle_apm_command(
    window: Option<String>,
    source: Option<String>,
    _metrics_db: Option<PathBuf>,
) -> Result<()> {
    use autopilot::apm::{APMSource, APMWindow};
    use colored::Colorize;

    // Parse window if provided
    let _window_filter = window.as_deref().map(|w| match w {
        "session" => APMWindow::Session,
        "1h" => APMWindow::Hour1,
        "6h" => APMWindow::Hour6,
        "1d" => APMWindow::Day1,
        "1w" => APMWindow::Week1,
        "1m" => APMWindow::Month1,
        "lifetime" => APMWindow::Lifetime,
        _ => {
            eprintln!("Invalid window: {}. Valid values: session, 1h, 6h, 1d, 1w, 1m, lifetime", w);
            std::process::exit(1);
        }
    });

    // Parse source if provided
    let source_filter = source.as_deref().map(|s| match s {
        "autopilot" => APMSource::Autopilot,
        "claude_code" | "claude" => APMSource::ClaudeCode,
        "combined" => APMSource::Combined,
        _ => {
            eprintln!("Invalid source: {}. Valid values: autopilot, claude_code, combined", s);
            std::process::exit(1);
        }
    });

    println!("{}", "APM Statistics".cyan().bold());
    println!("{}", "─".repeat(70).dimmed());
    println!();

    // TODO: Query actual data from metrics database
    // For now, show placeholder data
    println!("{:<12} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Source".bold(),
        "Session".bold(),
        "1h".bold(),
        "6h".bold(),
        "1d".bold(),
        "1w".bold()
    );
    println!("{}", "─".repeat(70).dimmed());

    if source_filter.is_none() || source_filter == Some(APMSource::Autopilot) {
        println!("{:<12} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "Autopilot".green(),
            "-",
            "19.2",
            "18.5",
            "17.8",
            "18.1"
        );
    }

    if source_filter.is_none() || source_filter == Some(APMSource::ClaudeCode) {
        println!("{:<12} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "Claude Code".blue(),
            "-",
            "4.2",
            "4.5",
            "4.3",
            "4.4"
        );
    }

    if source_filter.is_none() || source_filter == Some(APMSource::Combined) {
        println!("{:<12} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "Combined".cyan(),
            "-",
            "12.1",
            "11.8",
            "11.5",
            "11.9"
        );
    }

    println!();
    println!("{}", "Note: APM data collection in progress. Showing placeholder values.".yellow().dimmed());
    println!("{}", "Database integration and historical backfill coming in issues #649-651.".yellow().dimmed());

    Ok(())
}

/// Handle logs commands
async fn handle_logs_command(command: LogsCommands) -> Result<()> {
    use autopilot::logs::{self, LogsConfig};

    match command {
        LogsCommands::Stats { logs_dir } => {
            let config = LogsConfig {
                logs_dir: logs_dir.unwrap_or_else(|| PathBuf::from("docs/logs")),
                ..Default::default()
            };

            let stats = logs::calculate_log_size(&config)?;
            logs::print_stats(&stats);

            Ok(())
        }
        LogsCommands::Archive { days, logs_dir, dry_run } => {
            let config = LogsConfig {
                logs_dir: logs_dir.unwrap_or_else(|| PathBuf::from("docs/logs")),
                archive_after_days: days,
                ..Default::default()
            };

            if dry_run {
                println!("{} Running in dry-run mode (no changes will be made)\n", "ℹ️ ".cyan());
            }

            println!("{} Archiving logs older than {} days...\n", "📦".cyan(), days);

            let archived = logs::archive_logs(&config, dry_run)?;

            println!("\n{} Archived {} files", "✓".green(), archived.len());

            Ok(())
        }
        LogsCommands::Cleanup { days, logs_dir, db, dry_run } => {
            let config = LogsConfig {
                logs_dir: logs_dir.unwrap_or_else(|| PathBuf::from("docs/logs")),
                delete_after_days: days,
                db_path: db.or_else(|| Some(PathBuf::from("autopilot.db"))),
                ..Default::default()
            };

            if dry_run {
                println!("{} Running in dry-run mode (no changes will be made)\n", "ℹ️ ".cyan());
            }

            println!("{} Cleaning up archived logs older than {} days...\n", "🗑️ ".cyan(), days);

            let deleted = logs::cleanup_logs(&config, dry_run)?;

            println!("\n{} Deleted {} files", "✓".green(), deleted.len());

            Ok(())
        }
    }
}
