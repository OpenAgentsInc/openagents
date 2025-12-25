//! Autopilot CLI subcommands
//!
//! Wraps autopilot CLI behavior for the unified binary.

use clap::Subcommand;
use chrono::Utc;
use issues::{db, issue};
use issues::issue::{IssueType, Priority, Status};
use std::path::PathBuf;
use std::process::Command;
use which::which;

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

        /// Agent to use (claude, codex, gpt-oss)
        #[arg(long, default_value = "claude")]
        agent: String,

        /// Model to use (sonnet, opus, haiku, or full model ID)
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

    /// Replay a saved trajectory
    Replay {
        /// Path to trajectory file
        trajectory: PathBuf,
    },

    /// View APM (Actions Per Minute) statistics
    #[command(subcommand)]
    Apm(ApmCommands),

    /// Forward any other subcommand to the autopilot binary
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
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

fn resolve_autopilot_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("OPENAGENTS_AUTOPILOT_BIN") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    which("autopilot").ok()
}

fn run_autopilot_bin(args: &[String]) -> anyhow::Result<()> {
    let bin = resolve_autopilot_bin().ok_or_else(|| {
        anyhow::anyhow!(
            "autopilot binary not found. Set OPENAGENTS_AUTOPILOT_BIN or install the autopilot binary."
        )
    })?;

    let status = Command::new(bin).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("autopilot exited with status {}", status)
    }
}

fn run_issue_command(cmd: IssueCommands) -> anyhow::Result<()> {
    let db_path = autopilot::default_db_path();
    let conn = db::init_db(&db_path)?;

    match cmd {
        IssueCommands::List { status } => {
            let status = status.as_deref().map(Status::from_str);
            let issues = issue::list_issues(&conn, status)?;
            if issues.is_empty() {
                println!("No issues found");
                return Ok(());
            }
            for item in issues {
                println!(
                    "#{} [{}] {}",
                    item.number,
                    item.status.as_str(),
                    item.title
                );
            }
            Ok(())
        }
        IssueCommands::Create { title, body } => {
            let created = issue::create_issue(
                &conn,
                &title,
                body.as_deref(),
                Priority::Medium,
                IssueType::Task,
                None,
                None,
                None,
            )?;
            println!("Created issue #{}: {}", created.number, created.title);
            Ok(())
        }
        IssueCommands::Show { number } => {
            if let Some(item) = issue::get_issue_by_number(&conn, number)? {
                println!("#{} {}", item.number, item.title);
                println!("Status: {}", item.status.as_str());
                println!("Priority: {}", item.priority.as_str());
                println!("Type: {}", item.issue_type.as_str());
                println!("Agent: {}", item.agent);
                if let Some(ref directive) = item.directive_id {
                    println!("Directive: {}", directive);
                }
                if let Some(ref project) = item.project_id {
                    println!("Project: {}", project);
                }
                if item.is_blocked {
                    println!("Blocked: yes");
                    if let Some(ref reason) = item.blocked_reason {
                        println!("Blocked reason: {}", reason);
                    }
                }
                if let Some(ref desc) = item.description {
                    println!();
                    println!("{}", desc);
                }
            } else {
                println!("Issue #{} not found", number);
            }
            Ok(())
        }
        IssueCommands::Claim { number } => {
            if let Some(item) = issue::get_issue_by_number(&conn, number)? {
                let run_id = format!("manual-{}", Utc::now().timestamp());
                if issue::claim_issue(&conn, &item.id, &run_id)? {
                    println!("Claimed issue #{}: {}", number, item.title);
                } else {
                    println!("Could not claim issue #{} (already claimed or blocked)", number);
                }
            } else {
                println!("Issue #{} not found", number);
            }
            Ok(())
        }
        IssueCommands::Complete { number } => {
            if let Some(item) = issue::get_issue_by_number(&conn, number)? {
                if issue::complete_issue(&conn, &item.id)? {
                    println!("Completed issue #{}: {}", number, item.title);
                } else {
                    println!("Could not complete issue #{}", number);
                }
            } else {
                println!("Issue #{} not found", number);
            }
            Ok(())
        }
    }
}

pub fn run(cmd: AutopilotCommands) -> anyhow::Result<()> {
    match cmd {
        AutopilotCommands::Run {
            prompt,
            project,
            cwd,
            agent,
            model,
            with_issues,
            full_auto,
        } => {
            let mut args = vec!["run".to_string(), prompt];
            if let Some(project) = project {
                args.push("--project".to_string());
                args.push(project);
            }
            if let Some(cwd) = cwd {
                args.push("--cwd".to_string());
                args.push(cwd.display().to_string());
            }
            args.push("--agent".to_string());
            args.push(agent);
            args.push("--model".to_string());
            args.push(model);
            if with_issues {
                args.push("--with-issues".to_string());
            }
            if full_auto {
                args.push("--full-auto".to_string());
            }
            run_autopilot_bin(&args)
        }
        AutopilotCommands::Resume {
            trajectory,
            continue_last,
            prompt,
        } => {
            let mut args = vec!["resume".to_string()];
            if let Some(path) = trajectory {
                args.push(path.display().to_string());
            }
            if continue_last {
                args.push("--continue-last".to_string());
            }
            if let Some(prompt) = prompt {
                args.push("--prompt".to_string());
                args.push(prompt);
            }
            run_autopilot_bin(&args)
        }
        AutopilotCommands::Metrics(metrics_cmd) => {
            let mut args = vec!["metrics".to_string()];
            match metrics_cmd {
                MetricsCommands::Show => {
                    args.push("stats".to_string());
                }
                MetricsCommands::Import { path } => {
                    args.push("import".to_string());
                    args.push(path.display().to_string());
                }
                MetricsCommands::Analyze => {
                    args.push("analyze".to_string());
                }
            }
            run_autopilot_bin(&args)
        }
        AutopilotCommands::Issue(issue_cmd) => run_issue_command(issue_cmd),
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
        AutopilotCommands::Benchmark { .. } => {
            anyhow::bail!("Benchmark runner not wired yet for unified CLI")
        }
        AutopilotCommands::Passthrough(args) => run_autopilot_bin(&args),
    }
}
