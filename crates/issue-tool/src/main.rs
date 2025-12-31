use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use issues::{Priority, Status, db, issue};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "issue-tool")]
#[command(about = "Lightweight CLI for issue database operations")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
    /// List issues
    List {
        /// Filter by status (open, in_progress, done)
        #[arg(short, long)]
        status: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Get next ready issue
    Ready {
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create {
            title,
            description,
            priority,
            issue_type,
            agent,
            directive,
            db,
        } => {
            let db_path = resolve_db_path(db)?;
            let conn = db::init_db(&db_path)?;

            // Parse priority manually
            let priority = match priority.to_lowercase().as_str() {
                "urgent" => Priority::Urgent,
                "high" => Priority::High,
                "medium" => Priority::Medium,
                "low" => Priority::Low,
                _ => anyhow::bail!("Invalid priority. Use: urgent, high, medium, low"),
            };

            // Parse issue type manually
            let issue_type = match issue_type.to_lowercase().as_str() {
                "task" => issues::IssueType::Task,
                "bug" => issues::IssueType::Bug,
                "feature" => issues::IssueType::Feature,
                _ => anyhow::bail!("Invalid issue type. Use: task, bug, feature"),
            };

            let new_issue = issue::create_issue(
                &conn,
                &title,
                description.as_deref(),
                priority,
                issue_type,
                Some(&agent),
                directive.as_deref(),
                None, // project_id
            )?;

            println!(
                "{} Created issue #{}: {}",
                "✓".green(),
                new_issue.number,
                new_issue.title
            );
            Ok(())
        }
        Commands::List { status, db } => {
            let db_path = resolve_db_path(db)?;
            let conn = db::init_db(&db_path)?;

            let filter_status = status
                .map(|s| match s.to_lowercase().as_str() {
                    "open" => Ok(Status::Open),
                    "in_progress" | "inprogress" => Ok(Status::InProgress),
                    "done" => Ok(Status::Done),
                    _ => Err(anyhow::anyhow!(
                        "Invalid status. Use: open, in_progress, done"
                    )),
                })
                .transpose()?;

            let issues = issue::list_issues(&conn, filter_status)?;

            if issues.is_empty() {
                println!("No issues found");
                return Ok(());
            }

            for iss in issues {
                let status_str = match iss.status {
                    Status::Open => "OPEN".blue(),
                    Status::InProgress => "IN PROGRESS".yellow(),
                    Status::Done => "DONE".green(),
                };

                println!(
                    "#{} [{}] [{}] {}",
                    iss.number,
                    status_str,
                    format!("{:?}", iss.priority).cyan(),
                    iss.title
                );
            }

            Ok(())
        }
        Commands::Ready { db } => {
            let db_path = resolve_db_path(db)?;
            let conn = db::init_db(&db_path)?;

            match issue::get_next_ready_issue(&conn, None)? {
                Some(iss) => {
                    println!("→ Next ready issue:");
                    println!("  Number:   #{}", iss.number);
                    println!("  Title:    {}", iss.title);
                    println!("  Priority: {:?}", iss.priority);
                    println!("  Type:     {:?}", iss.issue_type);
                    println!("  Agent:    {}", iss.agent);
                    if let Some(desc) = iss.description {
                        println!("  Description:\n    {}", desc.replace('\n', "\n    "));
                    }
                }
                None => {
                    println!("No ready issues available");
                }
            }

            Ok(())
        }
        Commands::Claim { number, run_id, db } => {
            let db_path = resolve_db_path(db)?;
            let conn = db::init_db(&db_path)?;

            let run_id = run_id
                .unwrap_or_else(|| format!("manual-{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

            let iss = issue::get_issue_by_number(&conn, number)?.context("Issue not found")?;

            issue::claim_issue(&conn, &iss.id, &run_id)?;

            println!("{} Claimed issue #{}: {}", "✓".green(), number, iss.title);
            Ok(())
        }
        Commands::Complete { number, db } => {
            let db_path = resolve_db_path(db)?;
            let conn = db::init_db(&db_path)?;

            let iss = issue::get_issue_by_number(&conn, number)?.context("Issue not found")?;

            issue::complete_issue(&conn, &iss.id)?;

            println!("{} Completed issue #{}: {}", "✓".green(), number, iss.title);
            Ok(())
        }
        Commands::Block { number, reason, db } => {
            let db_path = resolve_db_path(db)?;
            let conn = db::init_db(&db_path)?;

            let iss = issue::get_issue_by_number(&conn, number)?.context("Issue not found")?;

            issue::block_issue(&conn, &iss.id, &reason)?;

            println!(
                "{} Blocked issue #{}: {} (Reason: {})",
                "⚠".yellow(),
                number,
                iss.title,
                reason
            );
            Ok(())
        }
    }
}

fn resolve_db_path(db: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = db {
        return Ok(path);
    }

    // Default to .openagents/autopilot.db in current directory
    Ok(PathBuf::from(".openagents/autopilot.db"))
}
