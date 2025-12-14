//! Full-Auto Mode CLI
//!
//! Run autonomous task execution with smart backend detection.
//!
//! Usage:
//!   auto              # Run single task with auto-detection
//!   auto --batch 5    # Run up to 5 tasks
//!   auto --continuous # Run until stopped or no more tasks
//!   auto --task <id>  # Run a specific task

use auto::{AutoConfig, AutoMode, AutoUpdate, ExecutionMode, TaskSource};
use futures::StreamExt;
use std::path::PathBuf;
use std::pin::pin;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse arguments
    let args: Vec<String> = std::env::args().collect();
    let config = parse_args(&args);

    println!("Full-Auto Mode v0.1.0");
    println!("=====================");
    println!();

    // Create and run auto mode
    match AutoMode::with_config(config).await {
        Ok(mut auto) => {
            // Show detection results
            let detection = auto.detection();
            println!("Detected backends: {:?}", detection.available_backends());
            if let Some(backend) = detection.selected_backend() {
                println!(
                    "Selected: {} ({})",
                    backend.display_name(),
                    detection.selection_reason()
                );
            }
            println!();

            // Run and stream updates
            let updates = auto.run();
            let mut updates = pin!(updates);
            while let Some(update) = updates.next().await {
                print_update(&update);

                if update.is_terminal() {
                    break;
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn parse_args(args: &[String]) -> AutoConfig {
    let mut config = AutoConfig::default();
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--batch" | "-b" => {
                if i + 1 < args.len() {
                    if let Ok(count) = args[i + 1].parse::<usize>() {
                        config.execution_mode = ExecutionMode::Batch { count };
                        i += 1;
                    }
                }
            }
            "--continuous" | "-c" => {
                config.execution_mode = ExecutionMode::Continuous;
            }
            "--task" | "-t" => {
                if i + 1 < args.len() {
                    config.task_source = TaskSource::Explicit {
                        task_ids: vec![args[i + 1].clone()],
                    };
                    i += 1;
                }
            }
            "--dir" | "-d" => {
                if i + 1 < args.len() {
                    config.working_directory = PathBuf::from(&args[i + 1]);
                    i += 1;
                }
            }
            "--no-commit" => {
                config.auto_commit = false;
            }
            "--no-taskmaster" => {
                config.update_taskmaster = false;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    config
}

fn print_help() {
    println!(
        r#"Full-Auto Mode - Autonomous task execution for OpenAgents

USAGE:
    auto [OPTIONS]

OPTIONS:
    -b, --batch <N>      Run up to N tasks
    -c, --continuous     Run until stopped or no more tasks
    -t, --task <ID>      Run a specific task
    -d, --dir <PATH>     Set working directory
    --no-commit          Don't auto-commit changes
    --no-taskmaster      Don't update taskmaster
    -h, --help           Show this help

ENVIRONMENT:
    ANTHROPIC_API_KEY    Use Anthropic API directly
    OPENROUTER_API_KEY   Use OpenRouter (can be in .env.local)
    OPENAI_API_KEY       Use OpenAI API

EXAMPLES:
    auto                          # Run single task
    auto --batch 5               # Run up to 5 tasks
    auto --continuous            # Run until stopped
    auto --task tm-123           # Run specific task
    auto --dir /path/to/project  # Use specific directory
"#
    );
}

fn print_update(update: &AutoUpdate) {
    match update {
        AutoUpdate::Initialized {
            backends_detected: _,
            selected_backend,
            working_directory,
        } => {
            println!(
                "Working directory: {}",
                working_directory.display()
            );
            if let Some(backend) = selected_backend {
                println!("Using backend: {}", backend.display_name());
            }
        }
        AutoUpdate::BackendSelected { backend, reason } => {
            println!("Backend: {} ({})", backend.display_name(), reason);
        }
        AutoUpdate::TasksDiscovered { count, source } => {
            println!("Found {} task(s) from {}", count, source);
            println!();
        }
        AutoUpdate::NoTasksFound { reason } => {
            println!("No tasks found: {}", reason);
        }
        AutoUpdate::TaskStarted {
            task_id,
            title,
            index,
            total,
        } => {
            println!("─────────────────────────────────────");
            println!("Task [{}/{}]: {}", index, total, task_id);
            println!("Title: {}", title);
            println!("─────────────────────────────────────");
        }
        AutoUpdate::TextDelta { task_id: _, delta } => {
            print!("{}", delta);
        }
        AutoUpdate::ReasoningDelta { task_id: _, delta } => {
            // Reasoning is typically hidden, but show a summary
            if delta.len() > 100 {
                print!(".");
            }
        }
        AutoUpdate::ToolStarted {
            task_id: _,
            tool_name,
            tool_call_id: _,
            input: _,
        } => {
            println!("\n[Tool: {}]", tool_name);
        }
        AutoUpdate::ToolCompleted {
            task_id: _,
            tool_call_id: _,
            output,
            is_error,
        } => {
            if *is_error {
                println!("[Error: {}]", output);
            } else if output.len() < 200 {
                println!("[Output: {}]", output);
            } else {
                println!("[Output: {} chars]", output.len());
            }
        }
        AutoUpdate::CommitCreated {
            task_id: _,
            sha,
            message,
        } => {
            println!("[Commit: {} - {}]", &sha[..8], message);
        }
        AutoUpdate::TaskCompleted {
            task_id,
            success,
            commits,
        } => {
            println!();
            if *success {
                println!("✓ Task {} completed ({} commits)", task_id, commits.len());
            } else {
                println!("✗ Task {} failed", task_id);
            }
            println!();
        }
        AutoUpdate::Finished {
            tasks_completed,
            tasks_failed,
        } => {
            println!("═════════════════════════════════════");
            println!(
                "Finished: {} completed, {} failed",
                tasks_completed, tasks_failed
            );
            println!("═════════════════════════════════════");
        }
        AutoUpdate::Error { error } => {
            eprintln!("Error: {}", error);
        }
        AutoUpdate::Cancelled { reason } => {
            println!(
                "Cancelled: {}",
                reason.as_deref().unwrap_or("User request")
            );
        }
    }
}
