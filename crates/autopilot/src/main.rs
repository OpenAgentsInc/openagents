//! Autopilot CLI - Run autonomous tasks with Claude and log trajectories

use anyhow::Result;
use clap::{Parser, Subcommand};
use claude_agent_sdk::{QueryOptions, SdkMessage, SettingSource, query};
use colored::*;
use futures::StreamExt;
use serde_json::json;
use std::path::PathBuf;
use std::sync::OnceLock;

use autopilot::replay;
use autopilot::rlog::RlogWriter;
use autopilot::timestamp::{date_dir, filename, generate_slug};
use autopilot::trajectory::{StepType, Trajectory};
use autopilot::TrajectoryCollector;

/// Global storage for .mcp.json path to enable cleanup on panic/signal
static MCP_JSON_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Clean up .mcp.json file if it exists
fn cleanup_mcp_json() {
    if let Some(path) = MCP_JSON_PATH.get() {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Setup signal handlers and panic hook for cleanup
fn setup_cleanup_handlers() {
    // Setup panic hook to cleanup .mcp.json
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        cleanup_mcp_json();
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
            // Re-raise signal to ensure proper exit
            signal_hook::low_level::raise(sig).ok();
            std::process::exit(128 + sig);
        }
    });
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

        /// Working directory (default: current directory)
        #[arg(short, long)]
        cwd: Option<PathBuf>,

        /// Model to use (sonnet, opus, haiku, or full model ID)
        #[arg(short, long, default_value_t = default_model())]
        model: String,

        /// Maximum turns
        #[arg(long, default_value_t = default_max_turns())]
        max_turns: u32,

        /// Maximum budget in USD
        #[arg(long, default_value = "5.0")]
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
    /// Manage issues
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
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
            cwd,
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
        } => {
            run_task(
                prompt, cwd, model, max_turns, max_budget, output_dir, slug, dry_run, verbose,
                with_issues, issues_db, full_auto,
            )
            .await
        }
        Commands::Replay { trajectory, mode } => {
            replay_trajectory(trajectory, mode).await
        }
        Commands::Compare { trajectory1, trajectory2 } => {
            compare_trajectories(trajectory1, trajectory2).await
        }
        Commands::Issue { command } => {
            handle_issue_command(command).await
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

/// Get default max_turns from environment or fallback to 50
fn default_max_turns() -> u32 {
    std::env::var("AUTOPILOT_MAX_TURNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
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

/// Full auto mode prompt suffix
const FULL_AUTO_PROMPT: &str = r#"

FULL AUTO MODE: You are in autonomous mode. Your workflow:
1. Call issue_ready to get the next available issue
2. If an issue exists: claim it, implement it, test it, commit AND PUSH to main, complete it
3. After completing, call issue_ready again for the next issue
4. When issue_ready returns "No ready issues available":
   - Analyze the codebase to understand the project direction
   - Identify the most logical next improvement or feature
   - Create a new issue using issue_create with your recommendation
   - Immediately claim and implement it
5. Continue this loop until max_budget is reached

IMPORTANT: Always `git push origin main` after each commit. Each issue should be pushed before moving to the next.

Never stop. Always keep working. Always keep improving.
"#;

async fn run_task(
    prompt: String,
    cwd: Option<PathBuf>,
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
) -> Result<()> {
    let cwd = cwd.unwrap_or_else(|| std::env::current_dir().unwrap());

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
    if !dry_run {
        std::fs::create_dir_all(&output_dir)?;
        let rlog_path = output_dir.join(filename(&slug, "rlog"));
        if let Err(e) = collector.enable_streaming(&rlog_path) {
            eprintln!("Warning: Failed to enable rlog streaming: {}", e);
        } else {
            println!("{} {} {}", "Streaming to:".dimmed(), rlog_path.display(), "(tail -f to watch)".dimmed());
        }
    }

    // Setup query options
    let options = QueryOptions::new()
        .model(&model)
        .max_turns(max_turns)
        .max_budget_usd(max_budget)
        .cwd(&cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .dangerously_skip_permissions(true);

    // Write .mcp.json file for issue tracking MCP server if requested
    if with_issues {
        let mcp_json_path = cwd.join(".mcp.json");
        let db_path = issues_db
            .unwrap_or_else(|| cwd.join("autopilot.db"))
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

    // Execute query
    let mut stream = query(&prompt, options).await?;

    while let Some(msg) = stream.next().await {
        let msg = msg?;

        // Collect trajectory
        collector.process_message(&msg);

        // Print progress
        if verbose {
            print_message(&msg);
        } else {
            print_progress(&msg);
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
    }

    // Cleanup .mcp.json on normal exit
    cleanup_mcp_json();

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
                "{} {} ({:.1}s)",
                "Working:".yellow(),
                p.tool_name,
                p.elapsed_time_seconds
            );
        }
        SdkMessage::Result(_) => {
            println!("{}", "Complete".green());
        }
        SdkMessage::Assistant(a) => {
            if let Some(content) = a.message.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match block_type {
                        "text" => {
                            // Show what the agent is thinking/saying
                            let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            let first_line = text.lines().next().unwrap_or("");
                            let truncated = if first_line.len() > 80 {
                                format!("{}...", &first_line[..77])
                            } else {
                                first_line.to_string()
                            };
                            if !truncated.is_empty() {
                                println!("{} {}", "Agent:".green(), truncated);
                            }
                        }
                        "tool_use" => {
                            let tool = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let input = block.get("input");

                            // Extract context based on tool type
                            let context = match tool {
                                "Bash" => input
                                    .and_then(|i| i.get("command"))
                                    .and_then(|c| c.as_str())
                                    .map(|c| {
                                        let truncated = if c.len() > 60 { format!("{}...", &c[..57]) } else { c.to_string() };
                                        format!("$ {}", truncated)
                                    }),
                                "Read" | "Write" | "Edit" => input
                                    .and_then(|i| i.get("file_path"))
                                    .and_then(|p| p.as_str())
                                    .map(|p| {
                                        // Show just filename
                                        p.rsplit('/').next().unwrap_or(p).to_string()
                                    }),
                                "Glob" => input
                                    .and_then(|i| i.get("pattern"))
                                    .and_then(|p| p.as_str())
                                    .map(|p| p.to_string()),
                                "Grep" => input
                                    .and_then(|i| i.get("pattern"))
                                    .and_then(|p| p.as_str())
                                    .map(|p| format!("/{}/", p)),
                                "Task" => input
                                    .and_then(|i| i.get("description"))
                                    .and_then(|d| d.as_str())
                                    .map(|d| d.to_string()),
                                _ => None,
                            };

                            match context {
                                Some(ctx) => println!("{} {} {}", "Tool:".blue(), tool.yellow(), ctx.dimmed()),
                                None => println!("{} {}", "Tool:".blue(), tool.yellow()),
                            }
                        }
                        _ => {}
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
                println!("{:<6} {:<10} {:<8} {:<50}", "Number", "Status", "Priority", "Title");
                println!("{}", "-".repeat(80));
                for i in issues {
                    let status_str = i.status.as_str();
                    let blocked = if i.is_blocked { " [BLOCKED]" } else { "" };
                    println!(
                        "{:<6} {:<10} {:<8} {}{}",
                        i.number,
                        status_str,
                        i.priority.as_str(),
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
            db,
        } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let priority = Priority::from_str(&priority);
            let issue_type = IssueType::from_str(&issue_type);

            let created = issue::create_issue(&conn, &title, description.as_deref(), priority, issue_type)?;

            println!(
                "{} Created issue #{}: {}",
                "✓".green(),
                created.number,
                created.title
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
        IssueCommands::Ready { db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            match issue::get_next_ready_issue(&conn)? {
                Some(i) => {
                    println!("{} Next ready issue:", "→".cyan());
                    println!("  Number:   #{}", i.number);
                    println!("  Title:    {}", i.title);
                    println!("  Priority: {}", i.priority.as_str());
                    println!("  Type:     {}", i.issue_type.as_str());
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
