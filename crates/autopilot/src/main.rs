//! Autopilot CLI - Run autonomous tasks with Claude and log trajectories

use anyhow::Result;
use clap::{Parser, Subcommand};
use claude_agent_sdk::{QueryOptions, SdkMessage, SettingSource, query};
use colored::*;
use futures::StreamExt;
use std::path::PathBuf;

use autopilot::rlog::RlogWriter;
use autopilot::timestamp::{date_dir, filename, generate_slug};
use autopilot::trajectory::{StepType, Trajectory};
use autopilot::TrajectoryCollector;

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

        /// Model to use
        #[arg(short, long, default_value = "claude-sonnet-4-5-20250929")]
        model: String,

        /// Maximum turns
        #[arg(long, default_value = "50")]
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
    },
}

#[tokio::main]
async fn main() -> Result<()> {
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
        } => {
            run_task(
                prompt, cwd, model, max_turns, max_budget, output_dir, slug, dry_run, verbose,
            )
            .await
        }
    }
}

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
) -> Result<()> {
    let cwd = cwd.unwrap_or_else(|| std::env::current_dir().unwrap());

    // Get git info
    let repo_sha = get_git_sha(&cwd).unwrap_or_else(|_| "unknown".to_string());
    let branch = get_git_branch(&cwd).ok();

    // Generate slug
    let slug = slug.unwrap_or_else(|| generate_slug(&prompt));

    // Setup output directory
    let output_dir = output_dir.unwrap_or_else(|| PathBuf::from("docs/logs").join(date_dir()));

    println!("{} {}", "Running:".cyan().bold(), prompt);
    println!("{} {}", "Model:".dimmed(), model);
    println!("{} {}", "CWD:".dimmed(), cwd.display());
    println!();

    // Create trajectory collector
    let mut collector = TrajectoryCollector::new(
        prompt.clone(),
        model.clone(),
        cwd.display().to_string(),
        repo_sha,
        branch,
    );

    // Setup query options
    let options = QueryOptions::new()
        .model(&model)
        .max_turns(max_turns)
        .max_budget_usd(max_budget)
        .cwd(&cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .dangerously_skip_permissions(true);

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
            print!(
                "\r{} {} ({:.1}s)    ",
                "Working:".yellow(),
                p.tool_name,
                p.elapsed_time_seconds
            );
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
        SdkMessage::Result(_) => {
            println!();
        }
        SdkMessage::Assistant(a) => {
            // Show tool calls
            if let Some(content) = a.message.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        let tool = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        print!("\r{} {}           ", "Calling:".blue(), tool);
                        use std::io::Write;
                        std::io::stdout().flush().ok();
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
