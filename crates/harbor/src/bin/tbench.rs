//! Terminal-Bench CLI Wrapper
//!
//! Entry point for Harbor to invoke agents for Terminal-Bench evaluation.
//! Executes a task using Claude Code CLI in headless mode and outputs:
//! - events.jsonl: Streaming events during execution
//! - trajectory.json: ATIF v1.4 format trajectory
//! - metrics.json: Token usage, cost, timing, tool stats
//!
//! # Usage
//!
//! ```bash
//! tbench \
//!     --instruction "Task description" \
//!     --output-dir /logs/agent \
//!     --timeout 3600
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use harbor::{
    Agent, ClaudeResult, EventRecorder, StepSource, TBenchMetrics, TokenUsage,
    TrajectoryBuilder, generate_session_id, timestamp,
};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

/// Terminal-Bench CLI Wrapper for Harbor agent evaluation
#[derive(Parser, Debug)]
#[command(name = "tbench")]
#[command(about = "Run agents for Terminal-Bench evaluation")]
#[command(version)]
struct Args {
    /// Task instruction/description to execute
    #[arg(short, long)]
    instruction: String,

    /// Directory to write output files (events.jsonl, trajectory.json, metrics.json)
    #[arg(short, long)]
    output_dir: PathBuf,

    /// Timeout in seconds (default: 3600)
    #[arg(short, long, default_value = "3600")]
    timeout: u64,

    /// Working directory for task execution
    #[arg(short, long)]
    cwd: Option<PathBuf>,

    /// Maximum number of turns (default: 300)
    #[arg(long, default_value = "300")]
    max_turns: u32,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Run Claude CLI and collect results
async fn run_claude_cli(
    instruction: &str,
    cwd: &PathBuf,
    timeout_secs: u64,
    max_turns: u32,
    verbose: bool,
) -> Result<ClaudeResult> {
    let args = vec![
        "--print".to_string(),
        "--dangerously-skip-permissions".to_string(),
        "--output-format".to_string(),
        "json".to_string(),
        "--max-turns".to_string(),
        max_turns.to_string(),
        "-p".to_string(),
        instruction.to_string(),
    ];

    if verbose {
        println!("Running: claude {}", args.join(" ").chars().take(100).collect::<String>());
    }

    let mut child = Command::new("claude")
        .args(&args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn claude CLI. Is it installed?")?;

    let stdout = child.stdout.take().expect("stdout was captured");
    let stderr = child.stderr.take().expect("stderr was captured");

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut stdout_buf = String::new();
    let mut stderr_buf = String::new();

    // Read output with timeout
    let result = timeout(Duration::from_secs(timeout_secs), async {
        loop {
            tokio::select! {
                line = stdout_reader.next_line() => {
                    match line {
                        Ok(Some(text)) => {
                            if verbose {
                                println!("{}", text);
                            }
                            stdout_buf.push_str(&text);
                            stdout_buf.push('\n');
                        }
                        Ok(None) => break,
                        Err(e) => {
                            if verbose {
                                eprintln!("stdout error: {}", e);
                            }
                            break;
                        }
                    }
                }
                line = stderr_reader.next_line() => {
                    match line {
                        Ok(Some(text)) => {
                            stderr_buf.push_str(&text);
                            stderr_buf.push('\n');
                        }
                        Ok(None) => {}
                        Err(_) => {}
                    }
                }
            }
        }

        child.wait().await
    }).await;

    match result {
        Ok(Ok(status)) => {
            let exit_code = status.code().unwrap_or(-1);
            Ok(ClaudeResult::parse_json(&stdout_buf, exit_code, &stderr_buf))
        }
        Ok(Err(e)) => {
            Ok(ClaudeResult {
                success: false,
                error: Some(format!("Process error: {}", e)),
                ..Default::default()
            })
        }
        Err(_) => {
            // Timeout - kill the process
            let _ = child.kill().await;
            Ok(ClaudeResult {
                success: false,
                error: Some(format!("Timeout after {}s", timeout_secs)),
                ..Default::default()
            })
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize telemetry
    if args.verbose {
        telemetry::init_with_filter("tbench", "debug");
    } else {
        telemetry::init_default("tbench");
    }

    let start_time = Instant::now();
    let start_time_iso = timestamp();

    // Ensure output directory exists
    std::fs::create_dir_all(&args.output_dir)
        .context("Failed to create output directory")?;

    // Initialize event recorder
    let mut event_recorder = EventRecorder::new(&args.output_dir)
        .context("Failed to create event recorder")?;

    let cwd = args.cwd.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    event_recorder.record("run_start", serde_json::json!({
        "instruction": args.instruction,
        "cwd": cwd.display().to_string(),
        "timeout": args.timeout,
    }))?;

    // Create trajectory builder
    let session_id = generate_session_id();
    let agent = Agent::claude_code("2.0.58");
    let mut trajectory_builder = TrajectoryBuilder::new(&session_id, agent, &args.instruction);

    println!();
    println!("=== Terminal-Bench Run ===");
    println!("Instruction: {}...", args.instruction.chars().take(100).collect::<String>());
    println!("Output: {}", args.output_dir.display());
    println!("CWD: {}", cwd.display());
    println!("Timeout: {}s", args.timeout);
    println!("Max turns: {}", args.max_turns);

    // Check for API keys
    let has_api_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
    let has_oauth = std::env::var("ANTHROPIC_OAUTH_TOKEN").is_ok();
    println!("ANTHROPIC_API_KEY: {}", if has_api_key { "set" } else { "not set" });
    println!("ANTHROPIC_OAUTH_TOKEN: {}", if has_oauth { "set" } else { "not set" });
    println!("===========================");
    println!();

    // Run Claude CLI
    let result = run_claude_cli(
        &args.instruction,
        &cwd,
        args.timeout,
        args.max_turns,
        args.verbose,
    ).await?;

    event_recorder.record("run_complete", serde_json::json!({
        "success": result.success,
        "turns": result.turns,
        "error": result.error,
    }))?;

    // Add final response to trajectory
    if result.success {
        trajectory_builder.add_step(StepSource::Agent, "Task completed successfully.");
    } else {
        trajectory_builder.add_step(
            StepSource::System,
            &format!("Task failed: {}", result.error.as_deref().unwrap_or("Unknown error")),
        );
    }

    let duration_ms = start_time.elapsed().as_millis() as u64;
    let end_time_iso = timestamp();

    // Build and write trajectory
    trajectory_builder.write_to_file(
        &args.output_dir,
        result.success,
        result.input_tokens,
        result.output_tokens,
        result.cost_usd,
    )?;

    // Build and write metrics
    let metrics = TBenchMetrics {
        instruction: args.instruction.clone(),
        success: result.success,
        start_time: start_time_iso,
        end_time: end_time_iso,
        duration_ms,
        turns: result.turns,
        tokens: TokenUsage {
            input: result.input_tokens,
            output: result.output_tokens,
            cache_read: result.cache_read_tokens,
            cache_creation: result.cache_creation_tokens,
            total: result.input_tokens + result.output_tokens,
        },
        cost: result.cost_usd,
        error: result.error.clone(),
    };
    metrics.write_to_file(&args.output_dir)?;

    println!();
    println!("=== Run Complete ===");
    println!("Success: {}", result.success);
    println!("Turns: {}", result.turns);
    println!("Duration: {:.1}s", duration_ms as f64 / 1000.0);
    println!("Tokens: {} in / {} out", result.input_tokens, result.output_tokens);
    if let Some(cost) = metrics.cost {
        println!("Cost: ${:.4}", cost);
    }
    println!("Output: {}", args.output_dir.display());
    if let Some(ref error) = result.error {
        println!("Error: {}", error);
    }
    println!("====================");
    println!();

    // Exit with appropriate code
    std::process::exit(if result.success { 0 } else { 1 });
}
