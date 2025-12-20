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

use autopilot::analyze;
use autopilot::replay;
use autopilot::rlog::RlogWriter;
use autopilot::timestamp::{date_dir, filename, generate_slug};
use autopilot::trajectory::{StepType, Trajectory};
use autopilot::{extract_session_id_from_json, extract_session_id_from_rlog};
use autopilot::TrajectoryCollector;

/// Minimum available memory in bytes before we kill the process (2 GB)
const MIN_AVAILABLE_MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Check if system has enough available memory
/// Returns (available_bytes, is_ok)
fn check_memory() -> (u64, bool) {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    let available = sys.available_memory();
    (available, available >= MIN_AVAILABLE_MEMORY_BYTES)
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
        let mut signals = signal_hook::iterator::Signals::new(&[
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
        ]).expect("Failed to create signal handler");

        for sig in signals.forever() {
            cleanup_mcp_json();
            // Note: lockfile intentionally NOT cleaned up here - stale lockfile indicates crash
            // Re-raise signal to ensure proper exit
            signal_hook::low_level::raise(sig).ok();
            std::process::exit(128 + sig);
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
            let db_path = cwd.join("autopilot.db");
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

        /// Path to issues database (default: autopilot.db in cwd)
        #[arg(long)]
        issues_db: Option<PathBuf>,

        /// Full auto mode: continuously work on issues and discover new work
        #[arg(long, default_value_t = default_full_auto())]
        full_auto: bool,

        /// Launch desktop UI alongside autopilot
        #[arg(long, default_value_t = default_ui())]
        ui: bool,
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

        /// Path to issues database (default: autopilot.db in cwd)
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
}

#[derive(Subcommand)]
enum IssueCommands {
    /// List issues
    List {
        /// Filter by status (open, in_progress, done)
        #[arg(short, long)]
        status: Option<String>,

        /// Path to issues database (default: autopilot.db in cwd)
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

        /// Path to issues database (default: autopilot.db in cwd)
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

        /// Path to issues database (default: autopilot.db in cwd)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Mark an issue as complete
    Complete {
        /// Issue number
        #[arg(required = true)]
        number: i32,

        /// Path to issues database (default: autopilot.db in cwd)
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

        /// Path to issues database (default: autopilot.db in cwd)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Get the next ready issue
    Ready {
        /// Filter by agent (claude or codex)
        #[arg(short, long)]
        agent: Option<String>,

        /// Path to issues database (default: autopilot.db in cwd)
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

        /// Path to issues database (default: autopilot.db in cwd)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// List all projects
    List {
        /// Path to issues database (default: autopilot.db in cwd)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Remove a project
    Remove {
        /// Project name
        #[arg(required = true)]
        name: String,

        /// Path to issues database (default: autopilot.db in cwd)
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

        /// Path to issues database (default: autopilot.db in cwd)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show session details
    Show {
        /// Session ID (or prefix)
        #[arg(required = true)]
        id: String,

        /// Path to issues database (default: autopilot.db in cwd)
        #[arg(long)]
        db: Option<PathBuf>,
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
        } => {
            run_task(
                prompt, project, cwd, agent, model, max_turns, max_budget, output_dir, slug, dry_run, verbose,
                with_issues, issues_db, full_auto, ui,
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

/// Get default max_budget from environment or fallback to 5.0
fn default_max_budget() -> f64 {
    std::env::var("AUTOPILOT_MAX_BUDGET")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5.0)
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

/// Full auto mode prompt suffix
const FULL_AUTO_PROMPT: &str = r#"

FULL AUTO MODE - CRITICAL AUTONOMOUS LOOP INSTRUCTIONS:

You are in FULLY AUTONOMOUS mode. You MUST follow this exact loop:

LOOP START:
1. Call issue_ready to get the next available issue
2. If issue exists: claim → implement → test → commit → PUSH → complete
3. IMMEDIATELY call issue_ready again (NO SUMMARIES, NO PAUSES)
4. GOTO LOOP START

IF issue_ready returns "No ready issues available":
- Analyze codebase to identify the next logical improvement
- Create a new issue with issue_create
- Claim and implement it
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
        let (available_mem, mem_ok) = check_memory();
        if !mem_ok {
            println!("\n{} Available memory ({}) below threshold ({}) - stopping to prevent crash",
                "MEMORY:".red().bold(),
                format_bytes(available_mem),
                format_bytes(MIN_AVAILABLE_MEMORY_BYTES));
            anyhow::bail!("Insufficient memory: {} available, {} required",
                format_bytes(available_mem),
                format_bytes(MIN_AVAILABLE_MEMORY_BYTES));
        }

        // Log memory status periodically
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
                let (avail, ok) = check_memory();
                if message_count % 100 == 0 {
                    println!("{} Memory: {}", "MEM:".dimmed(), format_bytes(avail));
                }
                if !ok {
                    println!("\n{} Memory critical ({}) - aborting", "MEMORY:".red().bold(), format_bytes(avail));
                    anyhow::bail!("Memory critical during execution: {} available", format_bytes(avail));
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
        let default_db = cwd.join("autopilot.db");
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

        // Set up for continuation - include FULL_AUTO_PROMPT again to reinforce instructions
        println!("{} Continuing with new query (attempt {})", "AUTO:".yellow().bold(), continuation_count);

        current_prompt = if has_more_work {
            format!("{}\n\nCONTINUE: You stopped prematurely. There are still issues to work on. Call issue_ready NOW. DO NOT output any text first - immediately call issue_ready.", FULL_AUTO_PROMPT)
        } else {
            format!("{}\n\nCONTINUE: You stopped prematurely. No issues are ready. Create a new issue with issue_create, then claim and implement it. DO NOT output any text first.", FULL_AUTO_PROMPT)
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
) -> Result<()> {
    // Load project if specified
    let (cwd, issues_db, project_id) = if let Some(project_name) = project {
        use issues::{db, project};

        let default_db = std::env::current_dir()?.join("autopilot.db");
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
        (cwd.unwrap_or_else(|| std::env::current_dir().unwrap()), issues_db, None)
    };

    // Create session record if we have a project
    let session_id = if let Some(ref proj_id) = project_id {
        use issues::{db, session};

        let default_db = cwd.join("autopilot.db");
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
        let stdout = child.stdout.take().expect("Failed to get stdout");
        let reader = BufReader::new(stdout);

        let mut port = None;
        for line in reader.lines().take(20) {
            if let Ok(line) = line {
                // Look for "DESKTOP_PORT=PORT"
                if let Some(rest) = line.strip_prefix("DESKTOP_PORT=") {
                    if let Ok(p) = rest.trim().parse::<u16>() {
                        port = Some(p);
                        break;
                    }
                }
            }
        }

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
        format!("{}{}", prompt, FULL_AUTO_PROMPT)
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

    // Setup query options with hooks
    let plan_mode_hook = std::sync::Arc::new(PlanModeHook);
    let plan_hook_matcher = HookCallbackMatcher::new().hook(plan_mode_hook);

    let compaction_hook = std::sync::Arc::new(CompactionHook);
    let compact_hook_matcher = HookCallbackMatcher::new().hook(compaction_hook);

    let mut hooks = std::collections::HashMap::new();
    hooks.insert(HookEvent::PreToolUse, vec![plan_hook_matcher]);
    hooks.insert(HookEvent::PreCompact, vec![compact_hook_matcher]);

    let options = QueryOptions::new()
        .model(&model)
        .max_turns(max_turns)
        .max_budget_usd(max_budget)
        .cwd(&cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .hooks(hooks)
        .dangerously_skip_permissions(true);

    // Write .mcp.json file for issue tracking MCP server if requested
    if with_issues {
        let mcp_json_path = cwd.join(".mcp.json");
        let default_issues_db = cwd.join("autopilot.db");
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
        std::fs::write(&mcp_json_path, serde_json::to_string_pretty(&mcp_config).unwrap())?;
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
            let default_db = cwd.join("autopilot.db");
            let db_path = issues_db.as_ref().unwrap_or(&default_db);
            if let Ok(conn) = db::init_db(db_path) {
                let _ = session::update_session_trajectory(&conn, sess_id, &json_path.display().to_string());
            }
        }
    }

    // Update session status on completion
    if let Some(ref sess_id) = session_id {
        use issues::{db, session, SessionStatus};
        let default_db = cwd.join("autopilot.db");
        let db_path = issues_db.as_ref().unwrap_or(&default_db);
        if let Ok(conn) = db::init_db(db_path) {
            let status = if trajectory.result.as_ref().map(|r| r.success).unwrap_or(false) {
                SessionStatus::Completed
            } else {
                SessionStatus::Failed
            };
            let _ = session::update_session_status(&conn, sess_id, status);
            let _ = session::update_session_metrics(&conn, sess_id, trajectory.usage.cost_usd, 0); // TODO: track issues completed
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
    let cwd = cwd.unwrap_or_else(|| std::env::current_dir().unwrap());

    // Track original trajectory path for appending logs
    let original_trajectory_path = trajectory.clone();

    // Get session_id from trajectory file or use --continue
    let session_id = if continue_last {
        println!("{} Continuing most recent session...", "Resume:".cyan().bold());
        None
    } else {
        let path = trajectory.expect("trajectory path required");
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
        .max_budget_usd(max_budget)
        .cwd(&cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .dangerously_skip_permissions(true);

    if let Some(ref id) = session_id {
        options.resume = Some(id.clone());
    } else {
        options.continue_session = true;
    }

    // Setup MCP for issue tracking if requested
    if with_issues {
        let mcp_json_path = cwd.join(".mcp.json");
        let db_path = issues_db
            .unwrap_or_else(|| cwd.join("autopilot.db"))
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

        std::fs::write(&mcp_json_path, serde_json::to_string_pretty(&mcp_config).unwrap())?;
        MCP_JSON_PATH.set(mcp_json_path).ok();
    }

    // Determine output paths - append to original files if resuming from a file
    let (rlog_path, json_path) = if let Some(ref orig_path) = original_trajectory_path {
        // Use same directory and derive paths from original
        let parent = orig_path.parent().unwrap_or(std::path::Path::new("."));
        let stem = orig_path.file_stem().and_then(|s| s.to_str()).unwrap_or("resumed");
        (
            parent.join(format!("{}.rlog", stem)),
            parent.join(format!("{}.json", stem)),
        )
    } else {
        // Create new files in standard location for --continue-last
        let output_dir = PathBuf::from("docs/logs").join(date_dir());
        std::fs::create_dir_all(&output_dir)?;
        let slug = format!("resumed-{}", chrono::Utc::now().format("%H%M"));
        (
            output_dir.join(filename(&slug, "rlog")),
            output_dir.join(filename(&slug, "json")),
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
        let dir = if path.is_dir() { path } else { path.parent().unwrap().to_path_buf() };

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

    let default_db = std::env::current_dir()?.join("autopilot.db");

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

    let default_db = std::env::current_dir()?.join("autopilot.db");

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

    let default_db = std::env::current_dir()?.join("autopilot.db");

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
            db,
        } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let priority = Priority::from_str(&priority);
            let issue_type = IssueType::from_str(&issue_type);

            let created = issue::create_issue(&conn, &title, description.as_deref(), priority, issue_type, Some(&agent))?;

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
    }

    Ok(())
}
