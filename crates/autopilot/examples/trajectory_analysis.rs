//! Trajectory analysis example
//!
//! This example demonstrates how to parse and analyze trajectory
//! logs to extract insights: token usage, costs, tool usage patterns,
//! and performance metrics.
//!
//! Run with:
//! ```bash
//! cargo run --example trajectory_analysis
//! ```

use autopilot::trajectory::{StepType, TokenUsage, Trajectory, TrajectoryResult};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Autopilot Trajectory Analysis Example");
    println!("=========================================\n");

    // Create a sample trajectory for demonstration
    let trajectory = create_sample_trajectory();

    // Analyze the trajectory
    analyze_trajectory(&trajectory);

    Ok(())
}

/// Create a sample trajectory with realistic data
fn create_sample_trajectory() -> Trajectory {
    let mut traj = Trajectory::new(
        "Fix clippy warnings in workspace".to_string(),
        "claude-sonnet-4-5".to_string(),
        "/home/user/project".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
    );

    traj.session_id = "sess_001".to_string();

    // Simulate a conversation with multiple tool calls
    traj.add_step(StepType::User {
        content: "Fix clippy warnings in workspace".to_string(),
    });

    traj.add_step(StepType::Assistant {
        content: "I'll check for clippy warnings first.".to_string(),
    });

    traj.add_step(StepType::ToolCall {
        tool: "Bash".to_string(),
        tool_id: "tool_1".to_string(),
        input: serde_json::json!({
            "command": "cargo clippy --workspace"
        }),
    });

    traj.add_step(StepType::ToolResult {
        tool_id: "tool_1".to_string(),
        success: true,
        output: Some("warning: redundant clone\n --> src/lib.rs:42".to_string()),
    });

    traj.add_step(StepType::Assistant {
        content: "Found a redundant clone. Let me fix it.".to_string(),
    });

    traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool_id: "tool_2".to_string(),
        input: serde_json::json!({
            "file_path": "/home/user/project/src/lib.rs"
        }),
    });

    traj.add_step(StepType::ToolResult {
        tool_id: "tool_2".to_string(),
        success: true,
        output: Some("[file contents...]".to_string()),
    });

    traj.add_step(StepType::ToolCall {
        tool: "Edit".to_string(),
        tool_id: "tool_3".to_string(),
        input: serde_json::json!({
            "file_path": "/home/user/project/src/lib.rs",
            "old_string": "content.clone()",
            "new_string": "content"
        }),
    });

    traj.add_step(StepType::ToolResult {
        tool_id: "tool_3".to_string(),
        success: true,
        output: Some("File updated successfully".to_string()),
    });

    traj.add_step(StepType::Assistant {
        content: "Fixed the redundant clone. All clippy warnings resolved.".to_string(),
    });

    // Set token usage
    traj.usage = TokenUsage {
        input_tokens: 4100,
        output_tokens: 135,
        cache_read_tokens: 850,
        cache_creation_tokens: 650,
        cost_usd: 0.0234,
    };

    // Add final result
    traj.result = Some(TrajectoryResult {
        success: true,
        duration_ms: 12450,
        num_turns: 3,
        result_text: Some("All clippy warnings fixed".to_string()),
        errors: vec![],
        issues_completed: 1,
    });

    traj
}

/// Analyze a trajectory and print insights
fn analyze_trajectory(trajectory: &Trajectory) {
    println!("Session Analysis");
    println!("================\n");

    // Basic info
    println!("Metadata:");
    println!("  Prompt: {}", trajectory.prompt);
    println!("  Model: {}", trajectory.model);
    println!("  Session ID: {}", trajectory.session_id);
    println!(
        "  Branch: {}",
        trajectory.branch.as_deref().unwrap_or("unknown")
    );
    println!();

    // Step counts by type
    println!("Step Breakdown:");
    let mut step_counts: HashMap<&str, usize> = HashMap::new();

    for step in &trajectory.steps {
        let step_type = match &step.step_type {
            StepType::User { .. } => "User",
            StepType::Assistant { .. } => "Assistant",
            StepType::ToolCall { .. } => "ToolCall",
            StepType::ToolResult { .. } => "ToolResult",
            StepType::Thinking { .. } => "Thinking",
            StepType::SystemInit { .. } => "SystemInit",
            StepType::SystemStatus { .. } => "SystemStatus",
        };
        *step_counts.entry(step_type).or_insert(0) += 1;
    }

    for (step_type, count) in step_counts.iter() {
        println!("  {}: {}", step_type, count);
    }
    println!();

    // Tool usage analysis
    println!("Tool Usage:");
    let mut tool_counts: HashMap<String, usize> = HashMap::new();

    for step in &trajectory.steps {
        if let StepType::ToolCall { tool, .. } = &step.step_type {
            *tool_counts.entry(tool.clone()).or_insert(0) += 1;
        }
    }

    let mut tools: Vec<_> = tool_counts.iter().collect();
    tools.sort_by(|a, b| b.1.cmp(a.1));

    for (tool, count) in tools {
        println!("  {}: {} calls", tool, count);
    }
    println!();

    // Token usage and cost
    println!("Token Usage:");
    println!("  Input: {}", trajectory.usage.input_tokens);
    println!("  Output: {}", trajectory.usage.output_tokens);
    println!("  Cache reads: {}", trajectory.usage.cache_read_tokens);
    println!(
        "  Cache creation: {}",
        trajectory.usage.cache_creation_tokens
    );
    println!();

    println!("Cost Analysis:");
    println!("  Total: ${:.4}", trajectory.usage.cost_usd);

    if !trajectory.steps.is_empty() {
        let total_tokens = trajectory.usage.input_tokens + trajectory.usage.output_tokens;
        let avg_tokens_per_step = total_tokens as f64 / trajectory.steps.len() as f64;
        println!("  Avg tokens/step: {:.0}", avg_tokens_per_step);
    }
    println!();

    // Result analysis
    if let Some(result) = &trajectory.result {
        println!("Execution Results:");
        println!("  Success: {}", result.success);
        println!(
            "  Duration: {}ms ({:.1}s)",
            result.duration_ms,
            result.duration_ms as f64 / 1000.0
        );
        println!("  Turns: {}", result.num_turns);
        println!("  Issues completed: {}", result.issues_completed);

        if !result.errors.is_empty() {
            println!("\n⚠️  Errors:");
            for error in &result.errors {
                println!("    - {}", error);
            }
        }
        println!();
    }

    // Tool success rate
    let mut tool_calls = 0;
    let mut tool_successes = 0;

    for step in &trajectory.steps {
        if let StepType::ToolResult { success, .. } = &step.step_type {
            tool_calls += 1;
            if *success {
                tool_successes += 1;
            }
        }
    }

    if tool_calls > 0 {
        let success_rate = (tool_successes as f64 / tool_calls as f64) * 100.0;
        println!(
            "Tool Success Rate: {:.1}% ({}/{})",
            success_rate, tool_successes, tool_calls
        );
    }

    println!("\n✓ Analysis complete!");
}
