//! Trajectory replay and debugging tools

use crate::trajectory::{Step, StepType, Trajectory};
use anyhow::{Context, Result};
use colored::*;
use std::io::{self, Write};
use std::path::Path;

/// Load a trajectory from a JSON file
pub fn load_trajectory(path: &Path) -> Result<Trajectory> {
    let content = std::fs::read_to_string(path).context(format!(
        "Failed to read trajectory file: {}",
        path.display()
    ))?;

    let trajectory: Trajectory =
        serde_json::from_str(&content).context("Failed to parse trajectory JSON")?;

    Ok(trajectory)
}

/// Interactive replay mode
pub fn interactive_replay(trajectory: &Trajectory) -> Result<()> {
    print_trajectory_header(trajectory);

    let mut current_step = 0;
    let total_steps = trajectory.steps.len();

    loop {
        // Clear screen
        print!("\x1B[2J\x1B[1;1H");

        // Print header
        print_trajectory_header(trajectory);
        println!();

        // Print current step
        if current_step < total_steps {
            print_step(&trajectory.steps[current_step], current_step, total_steps);
        } else {
            println!("{}", "=".repeat(80).dimmed());
            println!("{}", "End of trajectory".cyan().bold());
            print_trajectory_summary(trajectory);
        }

        // Print navigation help
        println!();
        println!("{}", "=".repeat(80).dimmed());
        println!(
            "{} {} | {} {} | {} {} | {} {} | {} {}",
            "[n]".green().bold(),
            "next",
            "[p]".yellow().bold(),
            "prev",
            "[g]".blue().bold(),
            "goto",
            "[f]".magenta().bold(),
            "filter",
            "[q]".red().bold(),
            "quit"
        );
        print!("{} ", "Command:".cyan());
        io::stdout().flush()?;

        // Read command
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input {
            "n" | "" => {
                if current_step < total_steps {
                    current_step += 1;
                }
            }
            "p" => {
                if current_step > 0 {
                    current_step -= 1;
                }
            }
            "g" => {
                print!("Go to step (1-{}): ", total_steps);
                io::stdout().flush()?;
                let mut num = String::new();
                io::stdin().read_line(&mut num)?;
                if let Ok(n) = num.trim().parse::<usize>() {
                    if n > 0 && n <= total_steps {
                        current_step = n - 1;
                    }
                }
            }
            "f" => {
                filter_view(trajectory)?;
            }
            "q" => break,
            _ => {}
        }
    }

    Ok(())
}

/// Print trajectory header information
fn print_trajectory_header(trajectory: &Trajectory) {
    println!("{}", "=".repeat(80).dimmed());
    println!("{}", "Trajectory Replay".cyan().bold());
    println!("{}", "=".repeat(80).dimmed());
    println!("{} {}", "Session:".dimmed(), trajectory.session_id);
    println!("{} {}", "Model:".dimmed(), trajectory.model);
    println!(
        "{} {}",
        "Prompt:".dimmed(),
        truncate(&trajectory.prompt, 100)
    );
    println!("{} {}", "CWD:".dimmed(), trajectory.cwd);
    println!("{} {}", "Commit:".dimmed(), trajectory.repo_sha);
    if let Some(ref branch) = trajectory.branch {
        println!("{} {}", "Branch:".dimmed(), branch);
    }
    println!(
        "{} {}",
        "Started:".dimmed(),
        trajectory.started_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!("{}", "=".repeat(80).dimmed());
}

/// Print a single step
fn print_step(step: &Step, index: usize, total: usize) {
    println!();
    println!(
        "{} {} {} {}",
        "Step".cyan().bold(),
        format!("{}/{}", index + 1, total).yellow(),
        "-".dimmed(),
        step.timestamp.format("%H:%M:%S").to_string().dimmed()
    );
    println!("{}", "-".repeat(80).dimmed());

    // Print step type and content
    match &step.step_type {
        StepType::User { content } => {
            println!("{} {}", "USER".cyan().bold(), "message");
            println!();
            println!("{}", content);
        }
        StepType::Assistant { content } => {
            println!("{} {}", "ASSISTANT".green().bold(), "response");
            println!();
            println!("{}", content);
        }
        StepType::Thinking { content, signature } => {
            println!("{} {}", "THINKING".yellow().bold(), "block");
            if let Some(sig) = signature {
                println!("{} {}", "Signature:".dimmed(), sig);
            }
            println!();
            println!("{}", content);
        }
        StepType::ToolCall {
            tool,
            tool_id,
            input,
        } => {
            println!(
                "{} {} {}",
                "TOOL CALL".blue().bold(),
                "→".dimmed(),
                tool.bright_blue()
            );
            println!("{} {}", "ID:".dimmed(), &tool_id[..tool_id.len().min(12)]);
            println!();
            println!("{}", "Input:".dimmed());
            println!(
                "{}",
                serde_json::to_string_pretty(input).unwrap_or_default()
            );
        }
        StepType::ToolResult {
            tool_id,
            success,
            output,
        } => {
            let status = if *success {
                "SUCCESS".green()
            } else {
                "FAILED".red()
            };
            println!(
                "{} {} {}",
                "TOOL RESULT".magenta().bold(),
                "→".dimmed(),
                status
            );
            println!("{} {}", "ID:".dimmed(), &tool_id[..tool_id.len().min(12)]);
            println!();
            if let Some(out) = output {
                println!("{}", "Output:".dimmed());
                println!("{}", truncate(out, 2000));
            } else {
                println!("{}", "(no output)".dimmed().italic());
            }
        }
        StepType::SystemInit { model } => {
            println!("{} {}", "SYSTEM INIT".bright_black().bold(), model);
        }
        StepType::SystemStatus { status } => {
            println!("{} {}", "SYSTEM STATUS".bright_black().bold(), status);
        }
        StepType::Subagent {
            agent_id,
            agent_type,
            status,
            summary,
        } => {
            use crate::trajectory::SubagentStatus;
            let status_str = match status {
                SubagentStatus::Started => "started".yellow(),
                SubagentStatus::Done => "done".green(),
                SubagentStatus::Error => "error".red(),
            };
            println!(
                "{} {}:{} [{}]",
                "SUBAGENT".bright_cyan().bold(),
                agent_type,
                agent_id,
                status_str
            );
            if let Some(s) = summary {
                println!("{}", "Summary:".dimmed());
                println!("{}", s);
            }
        }
    }

    // Print token info if available
    if step.tokens_in.is_some() || step.tokens_out.is_some() || step.tokens_cached.is_some() {
        println!();
        println!("{}", "Tokens:".dimmed());
        if let Some(tin) = step.tokens_in {
            print!("  in: {} ", tin);
        }
        if let Some(tout) = step.tokens_out {
            print!("  out: {} ", tout);
        }
        if let Some(tcached) = step.tokens_cached {
            print!("  cached: {} ", tcached);
        }
        println!();
    }
}

/// Filter view - show only specific step types
fn filter_view(trajectory: &Trajectory) -> Result<()> {
    println!();
    println!("Filter by type:");
    println!("  {} thinking", "[1]".yellow());
    println!("  {} tool calls", "[2]".blue());
    println!("  {} tool results", "[3]".magenta());
    println!("  {} assistant responses", "[4]".green());
    println!("  {} all", "[5]".cyan());
    print!("Choose: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let filter: Box<dyn Fn(&Step) -> bool> = match input.trim() {
        "1" => Box::new(|s| matches!(&s.step_type, StepType::Thinking { .. })),
        "2" => Box::new(|s| matches!(&s.step_type, StepType::ToolCall { .. })),
        "3" => Box::new(|s| matches!(&s.step_type, StepType::ToolResult { .. })),
        "4" => Box::new(|s| matches!(&s.step_type, StepType::Assistant { .. })),
        "5" => Box::new(|_| true),
        _ => return Ok(()),
    };

    // Clear screen
    print!("\x1B[2J\x1B[1;1H");

    print_trajectory_header(trajectory);
    println!();

    let filtered: Vec<_> = trajectory
        .steps
        .iter()
        .enumerate()
        .filter(|(_, s)| filter(s))
        .collect();

    println!("{} {} steps", "Showing".cyan(), filtered.len());
    println!();

    for (idx, step) in filtered {
        print_step_compact(step, idx + 1);
        println!();
    }

    println!();
    println!("{}", "Press Enter to continue...".dimmed());
    let mut _tmp = String::new();
    io::stdin().read_line(&mut _tmp)?;

    Ok(())
}

/// Print a compact version of a step
fn print_step_compact(step: &Step, step_num: usize) {
    let type_label = match &step.step_type {
        StepType::User { .. } => "USER".cyan(),
        StepType::Assistant { .. } => "ASST".green(),
        StepType::Thinking { .. } => "THINK".yellow(),
        StepType::ToolCall { tool, .. } => {
            return println!(
                "[{}] {} {} {}",
                step_num,
                "TOOL".blue(),
                "→".dimmed(),
                tool.bright_blue()
            );
        }
        StepType::ToolResult { success, .. } => {
            let status = if *success { "OK".green() } else { "FAIL".red() };
            return println!("[{}] {} {}", step_num, "RESULT".magenta(), status);
        }
        StepType::SystemInit { .. } => "INIT".bright_black(),
        StepType::SystemStatus { .. } => "STATUS".bright_black(),
        StepType::Subagent {
            agent_id,
            agent_type,
            ..
        } => {
            return println!(
                "[{}] {} {}:{}",
                step_num,
                "SUBAGENT".bright_cyan(),
                agent_type,
                agent_id
            );
        }
    };

    if let Some(content) = step.content() {
        println!("[{}] {} {}", step_num, type_label, truncate(content, 100));
    } else {
        println!("[{}] {}", step_num, type_label);
    }
}

/// Print trajectory summary
fn print_trajectory_summary(trajectory: &Trajectory) {
    println!();
    println!("{}", "Summary".cyan().bold());
    println!(
        "  Tokens:   {} in / {} out",
        trajectory.usage.input_tokens, trajectory.usage.output_tokens
    );
    println!("  Cached:   {}", trajectory.usage.cache_read_tokens);
    println!("  Cost:     ${:.4}", trajectory.usage.cost_usd);
    println!("  Steps:    {}", trajectory.steps.len());

    if let Some(ref result) = trajectory.result {
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

        if !result.errors.is_empty() {
            println!();
            println!("{}", "Errors:".red().bold());
            for err in &result.errors {
                println!("  - {}", err);
            }
        }
    }
}

/// List view - show all steps in a compact list
pub fn list_steps(trajectory: &Trajectory) -> Result<()> {
    print_trajectory_header(trajectory);
    println!();

    for (idx, step) in trajectory.steps.iter().enumerate() {
        print_step_compact(step, idx + 1);
    }

    println!();
    print_trajectory_summary(trajectory);

    Ok(())
}

/// Summary view - just show the header and summary
pub fn summary_view(trajectory: &Trajectory) -> Result<()> {
    print_trajectory_header(trajectory);
    print_trajectory_summary(trajectory);
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max - 3).collect::<String>())
    }
}

/// Compare two trajectories side-by-side
pub fn compare_trajectories(path1: &Path, path2: &Path) -> Result<()> {
    let traj1 = load_trajectory(path1)?;
    let traj2 = load_trajectory(path2)?;

    println!("{}", "=".repeat(120).dimmed());
    println!("{}", "Trajectory Comparison".cyan().bold());
    println!("{}", "=".repeat(120).dimmed());

    // Compare metadata
    println!(
        "{:<50} {:<50}",
        "Trajectory 1".yellow().bold(),
        "Trajectory 2".blue().bold()
    );
    println!("{}", "-".repeat(120).dimmed());
    println!(
        "{:<50} {:<50}",
        format!("Session: {}", &traj1.session_id[..20]),
        format!("Session: {}", &traj2.session_id[..20])
    );
    println!(
        "{:<50} {:<50}",
        format!("Model: {}", traj1.model),
        format!("Model: {}", traj2.model)
    );
    println!(
        "{:<50} {:<50}",
        format!("Prompt: {}", truncate(&traj1.prompt, 40)),
        format!("Prompt: {}", truncate(&traj2.prompt, 40))
    );
    println!(
        "{:<50} {:<50}",
        format!("Started: {}", traj1.started_at.format("%Y-%m-%d %H:%M:%S")),
        format!("Started: {}", traj2.started_at.format("%Y-%m-%d %H:%M:%S"))
    );

    println!();
    println!("{}", "Metrics Comparison".cyan().bold());
    println!("{}", "-".repeat(120).dimmed());

    // Compare usage
    println!(
        "{:<50} {:<50}",
        format!("Input tokens: {}", traj1.usage.input_tokens),
        format!("Input tokens: {}", traj2.usage.input_tokens)
    );
    println!(
        "{:<50} {:<50}",
        format!("Output tokens: {}", traj1.usage.output_tokens),
        format!("Output tokens: {}", traj2.usage.output_tokens)
    );
    println!(
        "{:<50} {:<50}",
        format!("Cached tokens: {}", traj1.usage.cache_read_tokens),
        format!("Cached tokens: {}", traj2.usage.cache_read_tokens)
    );
    println!(
        "{:<50} {:<50}",
        format!("Cost: ${:.4}", traj1.usage.cost_usd),
        format!("Cost: ${:.4}", traj2.usage.cost_usd)
    );
    println!(
        "{:<50} {:<50}",
        format!("Total steps: {}", traj1.steps.len()),
        format!("Total steps: {}", traj2.steps.len())
    );

    // Compare results
    if let (Some(r1), Some(r2)) = (&traj1.result, &traj2.result) {
        println!(
            "{:<50} {:<50}",
            format!("Duration: {}ms", r1.duration_ms),
            format!("Duration: {}ms", r2.duration_ms)
        );
        println!(
            "{:<50} {:<50}",
            format!("Turns: {}", r1.num_turns),
            format!("Turns: {}", r2.num_turns)
        );
        println!(
            "{:<50} {:<50}",
            format!(
                "Success: {}",
                if r1.success {
                    "yes".green()
                } else {
                    "no".red()
                }
            ),
            format!(
                "Success: {}",
                if r2.success {
                    "yes".green()
                } else {
                    "no".red()
                }
            )
        );
    }

    // Compare step types distribution
    println!();
    println!("{}", "Step Type Distribution".cyan().bold());
    println!("{}", "-".repeat(120).dimmed());

    let counts1 = count_step_types(&traj1);
    let counts2 = count_step_types(&traj2);

    println!(
        "{:<30} {:<20} {:<20}",
        "Type", "Trajectory 1", "Trajectory 2"
    );
    println!("{:<30} {:<20} {:<20}", "Thinking", counts1.0, counts2.0);
    println!("{:<30} {:<20} {:<20}", "Tool Calls", counts1.1, counts2.1);
    println!("{:<30} {:<20} {:<20}", "Tool Results", counts1.2, counts2.2);
    println!(
        "{:<30} {:<20} {:<20}",
        "Assistant Responses", counts1.3, counts2.3
    );

    // Show tool usage comparison
    println!();
    println!("{}", "Tool Usage Comparison".cyan().bold());
    println!("{}", "-".repeat(120).dimmed());

    let tools1 = count_tools(&traj1);
    let tools2 = count_tools(&traj2);

    // Get all unique tool names
    let mut all_tools: Vec<_> = tools1.keys().chain(tools2.keys()).collect();
    all_tools.sort();
    all_tools.dedup();

    println!(
        "{:<30} {:<20} {:<20}",
        "Tool", "Trajectory 1", "Trajectory 2"
    );
    for tool in all_tools {
        let count1 = tools1.get(tool).unwrap_or(&0);
        let count2 = tools2.get(tool).unwrap_or(&0);

        let diff = if count1 != count2 {
            let diff_value = (*count1 as i64).saturating_sub(*count2 as i64);
            format!(" ({:+})", diff_value).yellow()
        } else {
            "".normal()
        };

        println!("{:<30} {:<20} {:<20}{}", tool, count1, count2, diff);
    }

    Ok(())
}

fn count_step_types(traj: &Trajectory) -> (usize, usize, usize, usize) {
    let mut thinking = 0;
    let mut tool_calls = 0;
    let mut tool_results = 0;
    let mut assistant = 0;

    for step in &traj.steps {
        match &step.step_type {
            StepType::Thinking { .. } => thinking += 1,
            StepType::ToolCall { .. } => tool_calls += 1,
            StepType::ToolResult { .. } => tool_results += 1,
            StepType::Assistant { .. } => assistant += 1,
            _ => {}
        }
    }

    (thinking, tool_calls, tool_results, assistant)
}

fn count_tools(traj: &Trajectory) -> std::collections::HashMap<String, usize> {
    let mut counts = std::collections::HashMap::new();

    for step in &traj.steps {
        if let StepType::ToolCall { tool, .. } = &step.step_type {
            *counts.entry(tool.clone()).or_insert(0) += 1;
        }
    }

    counts
}
