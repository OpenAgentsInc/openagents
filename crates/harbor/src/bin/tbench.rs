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
//! # Standard mode (final JSON output)
//! tbench \
//!     --instruction "Task description" \
//!     --output-dir /logs/agent \
//!     --timeout 3600
//!
//! # Streaming mode (real-time JSON events to stdout)
//! tbench \
//!     --instruction "Task description" \
//!     --output-dir /logs/agent \
//!     --stream
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use harbor::{
    Agent, EventRecorder, Observation, ObservationResult, Step, StepSource, StreamEvent,
    TBenchMetrics, TokenUsage, ToolCall, Trajectory, TrajectoryExtra, generate_session_id,
    timestamp, ATIF_SCHEMA_VERSION, FinalMetrics,
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

    /// Enable streaming mode: emit JSON events to stdout for real-time UI
    #[arg(long)]
    stream: bool,

    /// Enable verbose output (ignored in stream mode)
    #[arg(short, long)]
    verbose: bool,

    /// Model to use (overrides default)
    #[arg(long)]
    model: Option<String>,
}

/// Accumulated results from parsing stream-json output
#[derive(Debug, Default)]
struct StreamResults {
    success: bool,
    turns: u32,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_creation_tokens: u64,
    cost_usd: Option<f64>,
    error: Option<String>,
    session_id: Option<String>,
    steps: Vec<Step>,
    current_turn: u32,
    step_id: u32,
    pending_tool_calls: Vec<ToolCall>,
}

impl StreamResults {
    fn new() -> Self {
        Self::default()
    }

    fn add_user_step(&mut self, instruction: &str) {
        self.step_id += 1;
        self.steps.push(Step {
            step_id: self.step_id,
            timestamp: timestamp(),
            source: StepSource::User,
            message: instruction.to_string(),
            tool_calls: None,
            observation: None,
        });
    }

    fn add_agent_step(&mut self, text: &str) {
        self.step_id += 1;
        // Attach any pending tool calls to this step
        let tool_calls = if self.pending_tool_calls.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.pending_tool_calls))
        };
        self.steps.push(Step {
            step_id: self.step_id,
            timestamp: timestamp(),
            source: StepSource::Agent,
            message: text.to_string(),
            tool_calls,
            observation: None,
        });
    }

    fn add_tool_call(&mut self, id: &str, name: &str, args: Option<serde_json::Value>) {
        self.pending_tool_calls.push(ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args,
            result: None,
        });
    }

    fn add_tool_result(&mut self, id: &str, output: Option<String>, is_error: bool) {
        // Add as observation step
        self.step_id += 1;
        self.steps.push(Step {
            step_id: self.step_id,
            timestamp: timestamp(),
            source: StepSource::System,
            message: format!("Tool result for {}", id),
            tool_calls: None,
            observation: Some(Observation {
                observation_type: "tool_result".to_string(),
                content: output.clone().unwrap_or_default(),
                result: Some(ObservationResult {
                    success: !is_error,
                    output: if is_error { None } else { output.clone() },
                    error: if is_error { output } else { None },
                }),
            }),
        });
    }
}

/// Parse a single line of Claude stream-json output
fn parse_stream_line(
    line: &str,
    results: &mut StreamResults,
    stream_mode: bool,
    event_recorder: &mut EventRecorder,
) -> Result<()> {
    let json: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Ok(()), // Skip non-JSON lines
    };

    let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match event_type {
        "assistant" => {
            // Parse content blocks from message.content array
            if let Some(message) = json.get("message") {
                if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                    for block in content {
                        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match block_type {
                            "text" => {
                                let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                if !text.is_empty() {
                                    results.current_turn += 1;
                                    results.turns = results.current_turn;

                                    if stream_mode {
                                        StreamEvent::Assistant {
                                            turn: results.current_turn,
                                            text: text.to_string(),
                                        }.emit();
                                    }

                                    event_recorder.record("assistant", serde_json::json!({
                                        "turn": results.current_turn,
                                        "text": text,
                                    }))?;

                                    results.add_agent_step(text);
                                }
                            }
                            "tool_use" => {
                                let tool_name = block.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let tool_id = block.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                let args = block.get("input").cloned();

                                if stream_mode {
                                    StreamEvent::ToolUse {
                                        tool: tool_name.to_string(),
                                        id: tool_id.to_string(),
                                    }.emit();
                                }

                                event_recorder.record("tool_use", serde_json::json!({
                                    "tool": tool_name,
                                    "id": tool_id,
                                }))?;

                                results.add_tool_call(tool_id, tool_name, args);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        "user" => {
            // Parse tool_result from user message content
            if let Some(message) = json.get("message") {
                if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                    for block in content {
                        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if block_type == "tool_result" {
                            let tool_id = block.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("");
                            let is_error = block.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
                            let content_str = block.get("content").and_then(|v| v.as_str()).map(String::from);

                            if stream_mode {
                                StreamEvent::ToolResult {
                                    id: tool_id.to_string(),
                                    output: if is_error { None } else { content_str.clone() },
                                    error: if is_error { content_str.clone() } else { None },
                                }.emit();
                            }

                            event_recorder.record("tool_result", serde_json::json!({
                                "id": tool_id,
                                "is_error": is_error,
                            }))?;

                            results.add_tool_result(tool_id, content_str, is_error);
                        }
                    }
                }
            }
        }
        "result" => {
            let subtype = json.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
            results.success = subtype == "success";
            results.session_id = json.get("session_id").and_then(|v| v.as_str()).map(String::from);
            results.turns = json.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            results.cost_usd = json.get("total_cost_usd").and_then(|v| v.as_f64());

            if let Some(usage) = json.get("usage") {
                results.input_tokens = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                results.output_tokens = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                results.cache_read_tokens = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                results.cache_creation_tokens = usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            }

            if !results.success && !subtype.is_empty() && subtype != "success" {
                results.error = Some(format!("Claude finished with: {}", subtype));
            }
        }
        _ => {}
    }

    Ok(())
}

/// Extract text content from assistant message
#[allow(dead_code)]
fn extract_assistant_text(json: &serde_json::Value) -> String {
    if let Some(message) = json.get("message") {
        if let Some(content) = message.get("content") {
            if let Some(text) = content.as_str() {
                return text.to_string();
            }
            if let Some(arr) = content.as_array() {
                return arr
                    .iter()
                    .filter_map(|block| {
                        if block.get("type")?.as_str()? == "text" {
                            block.get("text")?.as_str().map(String::from)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
    }
    String::new()
}

/// Run Claude CLI with streaming output
async fn run_claude_streaming(
    instruction: &str,
    cwd: &PathBuf,
    timeout_secs: u64,
    max_turns: u32,
    stream_mode: bool,
    model: Option<&str>,
    event_recorder: &mut EventRecorder,
) -> Result<StreamResults> {
    let mut args = vec![
        "--dangerously-skip-permissions".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(), // Required for stream-json with -p
        "--max-turns".to_string(),
        max_turns.to_string(),
        "-p".to_string(),
        instruction.to_string(),
    ];

    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }

    let mut child = Command::new("claude")
        .args(&args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn claude CLI. Is it installed?")?;

    let stdout = child.stdout.take().expect("stdout was captured");
    let mut stdout_reader = BufReader::new(stdout).lines();

    let mut results = StreamResults::new();
    results.add_user_step(instruction);

    // Process with timeout
    let process_result = timeout(Duration::from_secs(timeout_secs), async {
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            if let Err(e) = parse_stream_line(&line, &mut results, stream_mode, event_recorder) {
                eprintln!("Error parsing line: {}", e);
            }
        }
        child.wait().await
    }).await;

    match process_result {
        Ok(Ok(status)) => {
            if !status.success() && results.error.is_none() {
                results.error = Some(format!("Claude exited with code {}", status.code().unwrap_or(-1)));
                results.success = false;
            }
        }
        Ok(Err(e)) => {
            results.error = Some(format!("Process error: {}", e));
            results.success = false;
        }
        Err(_) => {
            let _ = child.kill().await;
            results.error = Some(format!("Timeout after {}s", timeout_secs));
            results.success = false;
        }
    }

    Ok(results)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize telemetry (suppress in stream mode)
    if !args.stream {
        if args.verbose {
            telemetry::init_with_filter("tbench", "debug");
        } else {
            telemetry::init_default("tbench");
        }
    }

    let start_time = Instant::now();
    let start_time_iso = timestamp();
    let session_id = generate_session_id();

    // Ensure output directory exists
    std::fs::create_dir_all(&args.output_dir)
        .context("Failed to create output directory")?;

    // Initialize event recorder
    let mut event_recorder = EventRecorder::new(&args.output_dir)
        .context("Failed to create event recorder")?;

    let cwd = args.cwd.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Emit start event
    if args.stream {
        StreamEvent::RunStart {
            session_id: session_id.clone(),
            instruction: args.instruction.chars().take(200).collect(),
        }.emit();
    } else {
        println!();
        println!("=== Terminal-Bench Run ===");
        println!("Session: {}", session_id);
        println!("Instruction: {}...", args.instruction.chars().take(100).collect::<String>());
        println!("Output: {}", args.output_dir.display());
        println!("CWD: {}", cwd.display());
        println!("Timeout: {}s", args.timeout);
        println!("Max turns: {}", args.max_turns);
        if let Some(ref m) = args.model {
            println!("Model: {}", m);
        }
        println!("===========================");
        println!();
    }

    event_recorder.record("run_start", serde_json::json!({
        "session_id": session_id,
        "instruction": args.instruction,
        "cwd": cwd.display().to_string(),
        "timeout": args.timeout,
        "model": args.model,
    }))?;

    // Run Claude CLI with streaming
    let results = run_claude_streaming(
        &args.instruction,
        &cwd,
        args.timeout,
        args.max_turns,
        args.stream,
        args.model.as_deref(),
        &mut event_recorder,
    ).await?;

    let duration_ms = start_time.elapsed().as_millis() as u64;
    let end_time_iso = timestamp();

    // Emit completion event
    if args.stream {
        StreamEvent::Complete {
            success: results.success,
            turns: results.turns,
            cost: results.cost_usd,
            error: results.error.clone(),
        }.emit();
    }

    event_recorder.record("run_complete", serde_json::json!({
        "success": results.success,
        "turns": results.turns,
        "cost_usd": results.cost_usd,
        "error": results.error,
    }))?;

    // Build and write trajectory
    let trajectory = Trajectory {
        schema_version: ATIF_SCHEMA_VERSION.to_string(),
        session_id: session_id.clone(),
        agent: Agent::claude_code("2.0.58"),
        steps: results.steps,
        final_metrics: FinalMetrics {
            total_prompt_tokens: results.input_tokens,
            total_completion_tokens: results.output_tokens,
            total_cost_usd: results.cost_usd,
            total_steps: results.turns,
        },
        extra: Some(TrajectoryExtra {
            instruction: args.instruction.clone(),
            start_time: start_time_iso.clone(),
            end_time: end_time_iso.clone(),
            success: results.success,
        }),
    };

    let trajectory_path = args.output_dir.join("trajectory.json");
    let trajectory_json = serde_json::to_string_pretty(&trajectory)?;
    std::fs::write(&trajectory_path, trajectory_json)?;

    // Build and write metrics
    let metrics = TBenchMetrics {
        instruction: args.instruction.clone(),
        success: results.success,
        start_time: start_time_iso,
        end_time: end_time_iso,
        duration_ms,
        turns: results.turns,
        tokens: TokenUsage {
            input: results.input_tokens,
            output: results.output_tokens,
            cache_read: results.cache_read_tokens,
            cache_creation: results.cache_creation_tokens,
            total: results.input_tokens + results.output_tokens,
        },
        cost: results.cost_usd,
        error: results.error.clone(),
    };
    metrics.write_to_file(&args.output_dir)?;

    // Print summary (non-stream mode only)
    if !args.stream {
        println!();
        println!("=== Run Complete ===");
        println!("Success: {}", results.success);
        println!("Turns: {}", results.turns);
        println!("Duration: {:.1}s", duration_ms as f64 / 1000.0);
        println!("Tokens: {} in / {} out", results.input_tokens, results.output_tokens);
        if let Some(cost) = results.cost_usd {
            println!("Cost: ${:.4}", cost);
        }
        println!("Output: {}", args.output_dir.display());
        println!("  - trajectory.json");
        println!("  - events.jsonl");
        println!("  - metrics.json");
        if let Some(ref error) = results.error {
            println!("Error: {}", error);
        }
        println!("====================");
        println!();
    }

    // Exit with appropriate code
    std::process::exit(if results.success { 0 } else { 1 });
}
